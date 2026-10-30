use super::*;

/// What the acting body may do, for the keybar.
///
/// Built here rather than as a strip of its own: the keybar already rides
/// `log_pane`'s bottom border and already degrades through
/// `strip::fitting`, so a fight's action list is a content swap and costs
/// no layout at all.
///
/// **The mode is an argument because a fight is two screens.** The cursor
/// is not the board: the three action keys do nothing while it is open, and
/// the two that commit and cancel it appear nowhere else. Taken here rather
/// than branched on at `draw_playing_base`, so which bar a screen gets is
/// one derivation a test can ask rather than a condition in a renderer.
///
/// **And `auto` is an argument because it is app-core's**, not the engine's:
/// `TacticalView::player_turn` still says the party's turn is the player's
/// while auto-attack spends it, which is the whole shape of that feature. It
/// is read before `player_turn` because it outranks it — a board being driven
/// end to end has one thing worth saying at every instant of it, and "the
/// wild side is moving" is not it.
pub(crate) fn action_bar(mode: Mode, view: &TacticalView, auto: bool) -> Vec<(String, String)> {
    if mode == Mode::TacticalAim {
        // Movement first because it is what the player is doing; `fitting`
        // drops from the end, and Esc is the row that may go — a cursor
        // backed out of by the key every other screen backs out with is the
        // one thing here nobody has to be told twice.
        return vec![
            ("↑↓←→ numpad".to_string(), "aim".to_string()),
            ("Enter".to_string(), "confirm".to_string()),
            ("Esc".to_string(), "cancel".to_string()),
        ];
    }
    if auto {
        return vec![("any key".to_string(), "stop auto-attack".to_string())];
    }
    if !view.player_turn {
        return vec![(String::new(), "the wild side is moving".to_string())];
    }
    // A charger's turn is a decision, not an action: the only choices are
    // the two the engine accepts, so the bar names them and nothing it
    // would refuse.
    if acting_body(view).is_some_and(|b| b.charge.is_some()) {
        return vec![
            ("H".to_string(), "hold".to_string()),
            ("X".to_string(), "release".to_string()),
            ("E".to_string(), "end turn".to_string()),
            ("R".to_string(), "resolve".to_string()),
            ("A".to_string(), "auto".to_string()),
        ];
    }
    let mut rows = vec![
        (
            "↑↓←→ numpad".to_string(),
            format!("move ({} left)", view.allowance),
        ),
        ("a".to_string(), "attack".to_string()),
        // Between attack and special, so the bar reads in the order the
        // abstract fight's own action menu does: a, d, s.
        ("d".to_string(), "defend".to_string()),
        // `special`, not `routine`: this is the word the abstract fight's
        // own action menu builds (`Game::battle_action_options`), and one
        // fight model naming the same verb differently is what makes the
        // shared `s` stop reading as the same key.
        ("s".to_string(), "special".to_string()),
    ];
    // Only while the acting body is emulating (todo #100 Task 6) —
    // `Special`'s own reason: a body not emulating has nothing to revert,
    // and a permanently-there row would teach nothing new. Uppercase, and
    // between `s` and `E` for the same reason `battle_action_options`
    // offers the group model's own Revert row right after Special.
    if acting_body(view).is_some_and(|b| b.form.is_some()) {
        rows.push(("V".to_string(), "revert".to_string()));
    }
    // `battle_action_options`' own order again: attack, defend, special,
    // revert, use item — last among the row-opening keys and dropped on a
    // spent turn exactly as `a`/`d`/`s` are, since spending an item costs
    // the action too.
    rows.push(("U".to_string(), "use item".to_string()));
    rows.extend([
        ("E".to_string(), "end turn".to_string()),
        // Before `A`, which stays last per its own comment below.
        ("R".to_string(), "resolve".to_string()),
        // Last, because `strip::fitting` drops from the end and this is the
        // row that may go — but it does not go at 1280x720, which is
        // `the_action_bar_fits_the_log_pane`'s measurement and not a hope.
        // "auto", not "auto-attack": the leading `?` took the room.
        ("A".to_string(), "auto".to_string()),
    ]);
    if view.acted {
        // The auto and resolve rows survive a spent turn: arming either is
        // not an action, and a turn with nothing left to spend is exactly
        // when a player decides they would rather watch the rest —
        // `tactical_drive_turn` on a spent turn is safe (`tactical_attack`
        // and `tactical_step` both refuse when `acted`, so `[R]` just ends
        // the turn and carries on), and hiding it here would hide a key
        // that is not actually refused.
        rows.retain(|(k, _)| k == "E" || k == "R" || k == "A");
    }
    rows
}

/// The most characters of a prop's readout the keybar will carry.
/// `strip::fitting` stops at the first segment that does not fit, so an
/// unbounded readout — a modded piece id is as long as its author likes —
/// would blank the rest of the bar rather than just itself.
pub(super) const AIM_READOUT_CHARS: usize = 28;

/// The keybar row naming the prop under the aim cursor, first so
/// `strip::fitting`, which drops from the end, never loses it. A body on the
/// cell outranks a prop, as it does when the swing commits. Cut to
/// `AIM_READOUT_CHARS`, since being first means a long one blocks the keys
/// behind it.
pub(crate) fn aim_readout(
    view: &TacticalView,
    cursor: Option<(i32, i32)>,
) -> Option<(String, String)> {
    let cell = cursor?;
    if view.bodies.iter().any(|b| b.cell == cell) {
        return None;
    }
    let prop = view.props.iter().find(|p| p.cell == cell)?;
    let mut text = prop_readout(prop);
    if text.chars().count() > AIM_READOUT_CHARS {
        text = text.chars().take(AIM_READOUT_CHARS - 1).collect::<String>() + "…";
    }
    Some((String::new(), text))
}

/// The keybar's rows for a fight: `action_bar`, with the aim cursor's prop
/// readout ahead of it while the cursor is open. One call for the renderer
/// and the fit test, so the bar measured is the bar drawn.
pub(crate) fn keybar_rows(
    mode: Mode,
    view: &TacticalView,
    auto: bool,
    cursor: Option<(i32, i32)>,
) -> Vec<(String, String)> {
    let mut rows = action_bar(mode, view, auto);
    // First of the keys, so it is the last to be cut; the aim readout below
    // is a description rather than a key and still leads.
    rows.insert(0, ("?".to_string(), "keys".to_string()));
    if mode == Mode::TacticalAim
        && let Some(row) = aim_readout(view, cursor)
    {
        rows.insert(0, row);
    }
    rows
}

/// Which consumable does the acting body spend? `[U]`'s picker, drawn as a
/// popup over the battle map.
///
/// `draw_battle_item_menu`'s shape and the same pool, `battle_usable_
/// items`: the pack a tactical fight draws from is the player's own
/// whichever body drinks it, so both screens list exactly what that call
/// says is usable and neither greys a row — everything shown here is
/// already known to be spendable.
pub(crate) fn draw_tactical_items(
    game: &Game,
    filter: &str,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let items = game.battle_usable_items();
    let mut rows = vec![text_row("Use which item? It costs this body its action.")];
    rows.extend(super::super::list_filter::filter_line(filter));
    let visible = tactical_items_visible(game, filter);
    if visible.is_empty() && !filter.is_empty() {
        rows.push(super::super::list_filter::no_match_row(filter));
    }
    for (i, item) in visible.iter().map(|&src| &items[src]).enumerate() {
        rows.push(item_row(game.item_name(item).to_string(), i == selected));
    }
    push_key_footer(
        &mut rows,
        &[
            "Type to filter",
            "Enter use",
            "Esc cancel (clears the filter first)",
        ],
    );
    draw_popup("Use an item", PopupSize::Large, &rows, refusal, painter, m);
}

/// The routine picker, drawn as a popup over the battle map.
///
/// `draw_field_routine`'s shape, and greyed rather than hidden for its
/// reason: the engine refuses an unavailable routine again on commit with
/// the reason on the status line, and a row that vanished would leave the
/// player wondering where a routine they installed went.
pub(crate) fn draw_tactical_routines(
    options: &[SpecialOption],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row("Run which routine?")];
    if options.is_empty() {
        rows.push(text_row("(nothing this body can run in a fight)"));
    }
    for (i, option) in options.iter().enumerate() {
        let label = format!("{} — {}", option.name, option.detail);
        rows.push(match &option.unavailable {
            None => item_row(label, i == selected),
            Some(reason) => spent_item_row(format!("{label} — {reason}"), i == selected),
        });
    }
    push_key_footer(&mut rows, &["Enter run", "Esc cancel"]);
    draw_popup(
        "Run a routine",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// Which learned image does Emulate invoke? `draw_tactical_routines`'s
/// shape one picker over — todo #100 Task 6. The figures shown are
/// `Game::emulation_options`'s own `atk`/`mitigation`, a call rather than a
/// second formula, so this can never quote a number invoking the row
/// installs a different one from.
pub(crate) fn draw_tactical_emulate(
    options: &[EmulationOption],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row("Invoke which image?")];
    for (i, option) in options.iter().enumerate() {
        rows.push(item_row(
            super::super::emulation_row_label(i, option, false),
            i == selected,
        ));
    }
    push_key_footer(&mut rows, &["Enter invoke", "Esc cancel"]);
    draw_popup(
        "Invoke an image",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// A finished tactical fight's results, drawn as a popup over the board it
/// was fought on — see `Mode::TacticalResult`.
pub(crate) fn draw_tactical_result(
    outcomes: &[LogEntry],
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Fight over",
        PopupSize::Large,
        &tactical_result_rows(outcomes),
        refusal,
        painter,
        m,
    );
}

/// The popup's rows: one per folded result, through the history screen's
/// own row builder so a result reads the same there and here.
///
/// Items and not text rows, because `draw_popup` pages an item span and
/// nothing else — a salvage tally and an XP line per fighter can outrun the
/// box, and a text-row page loses its tail in silence. Nothing is selected:
/// the page opens on its first line, and every line is in the log pane
/// once the popup is gone.
pub(crate) fn tactical_result_rows(outcomes: &[LogEntry]) -> Vec<Row> {
    let mut rows = if outcomes.is_empty() {
        vec![text_row("The fight is over.")]
    } else {
        history_rows(outcomes, usize::MAX)
    };
    rows.push(text_row(""));
    rows.push(text_row("Any key continue"));
    rows
}
