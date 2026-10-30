use super::*;

/// What the crew will have to fetch to raise `structure` a tier, priced
/// against the pack **and** the base's shelves.
///
/// `build_cost_display` and not `cost_display`, the rule the upgrade list
/// held before it: the crew fetches, so quoting the pack alone would price
/// the order against a store the verb does not read. `Game::upgrade_cost` is
/// the same door that list used, so the figure did not move when the screen
/// under it did.
pub(super) fn upgrade_cost_display(app: &mut App, structure: Entity) -> Vec<String> {
    let Some(game) = app.game.as_mut() else {
        return Vec::new();
    };
    let Some(cost) = game.upgrade_cost(structure) else {
        return Vec::new();
    };
    let status = game.player_status();
    let stock = game.base_stock();
    build_cost_display(game, &cost, &status.inventory, &stock)
}

/// The noun the prompt falls back to when the order's structure cannot be
/// named — a def id no longer in the catalogue, or an upgrade target that
/// left the scan radius between the pick and the frame. Neither is reachable
/// while the picker is up, and a prompt reading "the structure" is still a
/// warning where a missing prompt would be a blank confirmation.
pub(super) const UNNAMED_BUILD_TARGET: &str = "structure";

pub(super) const PROGRAM_PICKER_PROMPT: &str = "Which program pays for it?";

/// `PROGRAM_PICKER_PROMPT` for a wreck's rebuild, which names the job
/// rather than a payment.
pub(super) const REBUILD_PICKER_PROMPT: &str = "Which program rebuilds it?";

/// What the picker says, once and above the list, about a build that runs no
/// work cycle at all.
///
/// A fact about the whole screen belongs above the list rather than repeated
/// on every row, and the rows deliberately do **not** grey: every program is
/// equally valid here, which is exactly the message.
pub(super) const NO_CYCLE_NOTE: &str =
    "This build does not run a work cycle, so it does not care which program you spend.";

/// A configurable machine has no recipe to quote a pace for, but the program
/// you spend sets its pace whatever it is later set to make.
pub(super) const MACHINE_NOTE: &str =
    "The program you spend sets how fast this machine works, whatever you set it to make.";

/// The program picker's rows: the warning, then one entry per program the
/// order could be paid with.
///
/// **`candidates` is drawn exactly as it arrives.** It is
/// `Game::build_candidates`'s list, which is the one derivation of what
/// qualifies *and of the order it is in*, and `App::handle_build_program_key`
/// indexes that same call — so a row filtered out or reordered here would
/// make `[c]` spend the program the player read on row `d`.
///
/// The rows themselves are the party screen's, through `companion_rows`:
/// this keypress deletes a program from the roster, so the decision wants
/// the same stat line, tier colour and CRITICAL mark the roster shows, not a
/// bare list of names. The roster's **role headings do not come along**: this
/// list is sorted by the roll the build reads, so it interleaves roles and a
/// heading fired on the change would repeat and mean nothing. The warning
/// they carried — spending a program also empties its job — stays on the row
/// as `PetInfo::activity`.
pub(crate) fn build_program_rows(
    commit: Option<&BuildCommit>,
    candidates: &[BuildCandidate],
    selected: usize,
) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    if let Some(commit) = commit {
        // ORANGE, the colour `draw_remove_confirm` warns in, and wrapped at
        // the prose measure every other screen quotes prose at. All
        // `Row::Text`, so `popup_layout` pins them above the scrolling body
        // — the warning must not scroll off the list it is about.
        rows.extend(
            wrap_text(&commit.prompt(), DESCRIBE_WRAP_COLUMNS)
                .into_iter()
                .map(|line| Row::TextColored(line, ORANGE)),
        );
    }
    // Wrapped, and at `DESCRIBE_WRAP_COLUMNS` like the warning above it:
    // `draw_row` clips a row vertically and never horizontally, and a bill
    // naming three materials with their two figures each is already past the
    // body at one line.
    if let Some(commit) = commit.filter(|c| !c.cost.is_empty()) {
        rows.extend(
            wrap_text(
                &format!("Your crew fetches: {}", commit.cost.join(", ")),
                DESCRIBE_WRAP_COLUMNS,
            )
            .into_iter()
            .map(text_row),
        );
    }
    rows.push(text_row(if commit.is_some_and(|c| c.rebuild) {
        REBUILD_PICKER_PROMPT
    } else {
        PROGRAM_PICKER_PROMPT
    }));
    if !candidates.is_empty() && candidates.iter().all(|c| c.effect == BuildEffect::NoCycle) {
        rows.extend(
            wrap_text(NO_CYCLE_NOTE, DESCRIBE_WRAP_COLUMNS)
                .into_iter()
                .map(text_row),
        );
    }
    if !candidates.is_empty()
        && candidates
            .iter()
            .all(|c| c.effect == BuildEffect::ScalesRecipe)
    {
        rows.extend(
            wrap_text(MACHINE_NOTE, DESCRIBE_WRAP_COLUMNS)
                .into_iter()
                .map(text_row),
        );
    }
    for (i, c) in candidates.iter().enumerate() {
        let mut extra = vec![format!(" [{}: {}]", c.aptitude, c.label)];
        // Two whole tick figures rather than a percentage — see
        // `views::BuildEffect`. A `NoCycle` build carries no tag at all; the
        // sentence above the list has already said why.
        if let BuildEffect::Cycle { shipped, built } = c.effect {
            extra.push(format!(
                " [{} cycle {shipped} -> {built} ticks]",
                commit.map_or("machine", |c| c.label.as_str())
            ));
        }
        rows.extend(companion_rows(
            &c.pet,
            i == selected,
            &extra,
            ROW_WRAP_COLUMNS,
        ));
    }
    push_key_footer(&mut rows, &["Enter choose", "Esc cancel"]);
    rows
}

/// The picker `Mode::BuildProgram` draws: which program pays for the order
/// `App::pending_build` is holding.
///
/// The empty list goes out through `draw_popup`'s refusal argument rather
/// than as a `Row`, which is what keeps it off the `Row::Item` span
/// `App::selected_index` resolves against — the panel grows a line instead
/// of covering one. A refusal the player's last keypress actually raised
/// wins over it: that one is news, and the empty-list sentence is a standing
/// fact the player can already see (there are no rows).
///
/// With no pending order there is nothing to name and nothing to spend, and
/// any key drops `App::handle_build_program_key` straight back to the map.
/// The box is still drawn — a blank window is a screen the player cannot
/// read their way out of — with the deploy tier as the stand-in, which is
/// the shallower of the two and so cannot promise a program the engine would
/// refuse.
pub(crate) fn draw_build_program(
    game: &mut Game,
    commit: Option<BuildCommit>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let (rows, nothing_qualifies) = build_program_page(game, commit.as_ref(), selected);
    draw_popup(
        build_program_title(commit.as_ref()),
        PopupSize::Large,
        &rows,
        refusal.or(nothing_qualifies.as_deref()),
        painter,
        m,
    );
}

pub(super) fn build_program_title(commit: Option<&BuildCommit>) -> &'static str {
    if commit.is_some_and(|c| c.rebuild) {
        "Rebuild a structure"
    } else {
        "Commit a program"
    }
}

/// The build-program screen's rows and the line it shows when no program
/// qualifies, shared by the popup and the structure hub's Rebuild page.
pub(crate) fn build_program_page(
    game: &mut Game,
    commit: Option<&BuildCommit>,
    selected: usize,
) -> (Vec<Row>, Option<String>) {
    let tier = commit.map_or(1, BuildCommit::tier);
    let candidates = match commit {
        Some(commit) => game.build_candidates(&commit.structure, commit.goal()),
        // With no pending order there is nothing to be for, so there is
        // nothing to quote — the box is still drawn, empty, rather than
        // leaving the player on a blank screen.
        None => Vec::new(),
    };
    // Asked before the list is handed away: the empty list is the same
    // empty list whichever rule emptied it, and only
    // `floored_by_the_last_program` tells a roster that has nothing
    // suitable from one that has exactly one program and may spend none of
    // it. See `no_candidates_refusal`.
    let last_program = floored_by_the_last_program(game);
    let rows = build_program_rows(commit, &candidates, selected);
    let nothing_qualifies = candidates
        .is_empty()
        .then(|| no_candidates_refusal(tier, last_program));
    (rows, nothing_qualifies)
}
