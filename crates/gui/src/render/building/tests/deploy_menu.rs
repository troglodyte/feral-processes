use super::*;

#[test]
fn the_demolish_home_confirm_fits_its_small_popup() {
    assert_rows_fit(&demolish_home_rows(0), PopupSize::Small);
}

/// Every structure the game ships, as the deploy menu lists it. Read off
/// `assets/structures/` rather than hand-written because how long a
/// description runs is a property of the content: one authored longer
/// tomorrow has to clear the same width, and a fixture quoting today's
/// worst case would stop testing the wrap the moment the assets moved.
fn shipped_entries() -> Vec<BuildEntry> {
    let dir = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/structures"
    ));
    let (db, _) = StructureDb::load_dir(dir).expect("the shipped structures load");
    db.all()
        .map(|def| BuildEntry {
            name: def.name.clone(),
            cost: build_cost_label(&[]),
            deployed: WIDEST_DEPLOYED_TAG,
            description: def.description.clone(),
            category: def.category(),
            needs_program: def.needs_program(),
        })
        .collect()
}

/// The top level lists the Home as the structure it is and every other
/// category as one row naming how many it holds.
#[test]
fn the_deploy_menu_top_level_names_each_category_and_its_count() {
    let home = shipped_entries()
        .into_iter()
        .find(|e| e.category == StructureCategory::Home)
        .expect("the Home ships");
    let lines = [
        BuildLine::Structure(home),
        BuildLine::Category {
            category: StructureCategory::Extractor,
            count: 4,
        },
        BuildLine::Category {
            category: StructureCategory::Defence,
            count: 1,
        },
    ];
    let rows = build_top_rows(&lines, 1, None);
    let texts: Vec<&str> = rows.iter().map(row_text).collect();
    assert!(texts.iter().any(|t| t.starts_with("Home")), "{texts:?}");
    assert!(texts.contains(&"Extractors (4)"), "{texts:?}");
    assert!(texts.contains(&"Defence (1)"), "{texts:?}");
}

/// The shipped description that runs longest, which is the one both
/// deploy screens have to survive.
fn widest_shipped_description() -> String {
    shipped_entries()
        .into_iter()
        .map(|e| e.description)
        .max_by_key(|d| d.chars().count())
        .expect("the shipped assets define structures")
}

/// The deployed count the width fixtures carry. Nothing caps how many of
/// an uncapped structure a base may stand, so the tag is measured at the
/// widest figure a grid could plausibly reach rather than at the shipped
/// `max_deployed` ceilings — which are 1 and 3, and would leave the row
/// two characters narrower than the one a player can actually build to.
const WIDEST_DEPLOYED_TAG: u32 = 99;

/// A description longer than anything authored, so the wrap is what is
/// tested rather than the assets happening to be short enough.
fn synthetic_description() -> String {
    "Recompiles damaged structures across the whole base, itself included, ".repeat(6)
}

/// A row says how many of that structure the base already counts, and
/// says nothing at all when the answer is none.
///
/// The tag sits between the name and the cost rather than after it: a
/// cost line already reads `Bytecode Block (12/20)`, so `(3)` on the end
/// of one is a fourth cost fragment rather than a count.
#[test]
fn a_deploy_row_is_tagged_with_how_many_already_stand() {
    let entries = vec![
        BuildEntry {
            name: "Mining Node".to_string(),
            cost: "12 Core Fragments".to_string(),
            deployed: 3,
            description: "Cuts ore.".to_string(),
            category: StructureCategory::Extractor,
            needs_program: true,
        },
        BuildEntry {
            name: "Lathe".to_string(),
            cost: "12 Core Fragments".to_string(),
            deployed: 0,
            description: "Turns parts.".to_string(),
            category: StructureCategory::Extractor,
            needs_program: true,
        },
    ];
    let rows = build_menu_rows(&entries, 0, None);
    let texts: Vec<&str> = rows.iter().map(row_text).collect();

    assert!(
        texts
            .iter()
            .any(|t| t.contains("Mining Node (3) - 12 Core Fragments")),
        "a row with three standing says so: {texts:?}"
    );
    assert!(
        texts
            .iter()
            .any(|t| t.contains("Lathe - 12 Core Fragments")),
        "and a row with none is left alone: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t.contains("(0)")),
        "a fresh base is not a menu of zeroes: {texts:?}"
    );
}

/// `draw_row` clamps a row vertically and **never horizontally**, so a
/// structure's authored description — up to about 300 characters against
/// a `PopupSize::Large` body of roughly 114 — was drawn straight off the
/// right edge of the deploy menu in silence.
#[test]
fn no_deploy_menu_row_runs_past_the_popup_body() {
    let mut entries = shipped_entries();
    entries.push(BuildEntry {
        name: "Overlong Node".to_string(),
        cost: build_cost_label(&[]),
        deployed: WIDEST_DEPLOYED_TAG,
        description: synthetic_description(),
        category: StructureCategory::Utility,
        needs_program: true,
    });
    // All three forms: each greyed one carries a line the deployable one
    // does not, and an unwrapped sentence runs off the edge just as
    // silently. Both shortfall sentences, because they are different
    // lengths and only the longer of the two can be the one that
    // overflows.
    for shortfall in [
        None,
        Some(DEPLOY_NEEDS_A_PROGRAM),
        Some(DEPLOY_NEEDS_A_SECOND_PROGRAM),
    ] {
        for row in build_menu_rows(&entries, 0, shortfall) {
            let text = row_text(&row);
            assert!(
                text.chars().count() <= ROW_WRAP_COLUMNS,
                "a {} char deploy row runs past the {ROW_WRAP_COLUMNS} column body: {text:?}",
                text.chars().count()
            );
        }
    }
}

/// Wrapping a description costs rows, and `popup_layout` ends the
/// scrollable body at the *last* `Row::Item` — so a continuation line
/// emitted as `Row::Text` would strand the final structure's description
/// under the scroll indicator, detached from the row it describes. The
/// rule `build_menu_rows` documents, asserted over the real assets.
#[test]
fn every_deploy_description_stays_inside_the_scrollable_body() {
    let entries = shipped_entries();
    for (selected, shortfall) in [
        (0, None),
        (entries.len() - 1, None),
        (0, Some(DEPLOY_NEEDS_A_PROGRAM)),
        (0, Some(DEPLOY_NEEDS_A_SECOND_PROGRAM)),
    ] {
        let rows = build_menu_rows(&entries, selected, shortfall);
        let last_item = rows
            .iter()
            .rposition(|r| matches!(r, Row::Item { .. }))
            .expect("a structure is an item row");
        assert_eq!(
            last_item,
            rows.len() - 3,
            "the deploy menu pinned {} rows below its list instead of just \
             the key footer — a description is detached from the structure \
             it belongs to",
            rows.len() - last_item - 1
        );
    }
}

/// The deploy prompt draws the same description in a box with no scroll
/// at all — every row is `Row::Text`, so `popup_layout` pins the lot —
/// which is the other half of the same fault.
#[test]
fn no_deploy_prompt_row_runs_past_the_popup_body() {
    for description in [widest_shipped_description(), synthetic_description()] {
        let rows = build_direction_rows(
            "Fabricator",
            &description,
            &["Bytecode Block (12/20)".to_string()],
        );
        for row in &rows {
            let text = row_text(row);
            assert!(
                text.chars().count() <= ROW_WRAP_COLUMNS,
                "a {} char deploy prompt row runs past the {ROW_WRAP_COLUMNS} column body: {text:?}",
                text.chars().count()
            );
        }
        // Wrapped, not truncated: the prompt is the last thing the player
        // reads before placing the structure, so a description that lost
        // its tail would be worse than one that ran off the edge.
        let drawn: Vec<&str> = rows
            .iter()
            .flat_map(|r| row_text(r).split_whitespace())
            .collect();
        for word in description.split_whitespace() {
            assert!(drawn.contains(&word), "the wrap dropped {word:?}");
        }
    }
}

/// The prompt has no scroll, so a description tall enough to outgrow the
/// box loses its tail in silence — `the_tallest_memory_page_fits_its_popup`'s
/// trap, in the screen wrapping just moved rows into.
///
/// Swept across window heights rather than measured at one: `ui_metrics`
/// clamps the font at both ends, so below the clamp the box keeps
/// shrinking while the line height stops.
#[test]
fn the_tallest_deploy_prompt_fits_its_popup() {
    let rows = build_direction_rows(
        "Fabricator",
        &widest_shipped_description(),
        &["Bytecode Block (12/20)".to_string()],
    )
    .len();
    for h in (600..=2160).step_by(60) {
        let m = crate::text::ui_metrics(h as f32);
        let cap = popup_max_rows(h as f32, PopupSize::Large, &m);
        assert!(
            rows + REFUSAL_MAX_LINES <= cap,
            "the deploy prompt builds a {rows}-row page into a {cap}-row popup at {h}px"
        );
    }
}

/// The other axis, in pixels rather than columns: `ROW_WRAP_COLUMNS` is
/// a proxy for the real box, and this is the measurement it stands in
/// for. Both deploy screens are `PopupSize::Large`.
#[test]
fn no_deploy_row_overflows_its_popup_in_pixels() {
    let mut entries = shipped_entries();
    entries.push(BuildEntry {
        name: "Overlong Node".to_string(),
        cost: build_cost_label(&[]),
        deployed: WIDEST_DEPLOYED_TAG,
        description: synthetic_description(),
        category: StructureCategory::Utility,
        needs_program: true,
    });
    let menu = build_menu_rows(&entries, 0, None);
    let greyed = build_menu_rows(&entries, 0, Some(DEPLOY_NEEDS_A_PROGRAM));
    let floored = build_menu_rows(&entries, 0, Some(DEPLOY_NEEDS_A_SECOND_PROGRAM));
    let prompt = build_direction_rows(
        "Fabricator",
        &widest_shipped_description(),
        &["Bytecode Block (12/20)".to_string()],
    );
    crate::paint::with_painter(|p| {
        let m = crate::text::ui_metrics(900.0);
        // 0.88 is `PopupSize::Large`'s width fraction, against the
        // 1440x900 geometry `ui_metrics` is calibrated for.
        let room = 1440.0 * 0.88 - m.pad * 2.0;
        for row in menu
            .iter()
            .chain(greyed.iter())
            .chain(floored.iter())
            .chain(prompt.iter())
        {
            let text = match row {
                Row::Text(t) | Row::TextColored(t, _) => t.clone(),
                // `draw_row`'s own prefix on an item row.
                Row::Item { text, .. } => format!("     {text}"),
            };
            let drawn = p.measure_ui_advance(&text, m.font_size);
            assert!(
                drawn <= room,
                "a deploy row overflows its popup by {:.0}px ({drawn:.0} into {room:.0}):\n{text}",
                drawn - room
            );
        }
    });
}

/// The deploy prompt used to be a bare compass, which meant the one screen
/// where a structure is actually placed was also the one that never said
/// what was being placed. Both halves matter: the identity, and the keys
/// that act on it.
#[test]
fn the_deploy_prompt_names_what_is_being_placed_and_still_says_how() {
    let rows = build_direction_rows(
        "Mining Node",
        "Extracts Core Fragments on a timer.",
        &["Core Fragment (5/12)".to_string()],
    );
    let text: Vec<&str> = rows.iter().map(row_text).collect();
    assert!(text.contains(&"Mining Node"), "{text:?}");
    assert!(
        text.contains(&"Extracts Core Fragments on a timer."),
        "{text:?}"
    );
    assert!(
        text.iter().any(|t| t.contains("Core Fragment (5/12)")),
        "the cost carries the have/need figures the refusal would otherwise be the first news of: {text:?}"
    );
    assert!(text.contains(&DIRECTION_PROMPT), "{text:?}");
    assert!(
        text.contains(&key_hints(DIRECTION_KEYS).as_str()),
        "{text:?}"
    );
}

/// A waived bill has no rows to join, and both screens that quote one
/// have to say so in words. Left to `join`, the menu row trailed off
/// after its dash and the prompt read "Costs " — which is what a screen
/// that failed to load a price looks like, not a free structure.
#[test]
fn a_waived_bill_reads_as_free_on_both_screens_that_quote_it() {
    assert_eq!(build_cost_label(&[]), "free");
    assert_eq!(
        build_cost_label(&["Core Fragment (5/12)".to_string()]),
        "Core Fragment (5/12)",
        "and a bill that exists is still quoted in full"
    );

    let rows = build_direction_rows("Contract Broker", "Posts work.", &[]);
    let text: Vec<&str> = rows.iter().map(row_text).collect();
    assert!(
        text.contains(&"Free to deploy"),
        "the deploy prompt says it in a sentence rather than quoting an empty bill: {text:?}"
    );
    assert!(
        !text.iter().any(|t| t.starts_with("Costs")),
        "and does not also quote a cost: {text:?}"
    );
}

/// The colour of each *structure* row, in entry order — the headings and
/// the wrapped descriptions are `Row::Item` too (they have to be, see
/// `build_menu_rows`), and only the lines naming a structure stand for
/// something the player picks.
fn entry_colors(rows: &[Row]) -> Vec<Color> {
    let names: Vec<String> = shipped_entries().into_iter().map(|e| e.name).collect();
    rows.iter()
        .filter_map(|r| match r {
            Row::Item { text, color, .. } if names.iter().any(|n| text.starts_with(n)) => {
                Some(*color)
            }
            _ => None,
        })
        .collect()
}

/// A roster with nothing free to spend stops **every** deploy but Home,
/// so the menu says it once and greys the rest — rather than tagging
/// each row with a requirement that is the same on all of them.
///
/// Grey, never hide: the rows are still there and still pickable. A
/// structure that vanished from the catalogue reads as a bug, and the
/// pick lands on the picker, which says the same thing at length.
#[test]
fn a_roster_with_no_free_program_greys_the_deploy_menu() {
    let entries = shipped_entries();

    let open = build_menu_rows(&entries, 0, None);
    assert!(
        !open.iter().any(|r| row_text(r) == DEPLOY_NEEDS_A_PROGRAM),
        "a menu that can be used says nothing about a cost it can meet"
    );

    let shut = build_menu_rows(&entries, 0, Some(DEPLOY_NEEDS_A_PROGRAM));
    assert_eq!(
        shut.iter()
            .filter(|r| row_text(r) == DEPLOY_NEEDS_A_PROGRAM)
            .count(),
        1,
        "once for the screen, not once per row"
    );
    assert!(
        matches!(shut.first(), Some(Row::TextColored(..))),
        "the warning leads, above the list"
    );
    assert_eq!(
        shut.iter()
            .filter(|r| matches!(r, Row::Item { .. }))
            .count(),
        open.iter()
            .filter(|r| matches!(r, Row::Item { .. }))
            .count(),
        "nothing was hidden, so no row a keypress resolves to has moved"
    );

    let colors = entry_colors(&shut);
    assert_eq!(colors.len(), entries.len(), "one row per structure");
    for (entry, color) in entries.iter().zip(&colors) {
        assert_eq!(
            *color,
            if entry.needs_program { TEXT_DIM } else { TEXT },
            "{} reads wrong on a roster that can pay for nothing",
            entry.name
        );
    }
    assert!(
        entry_colors(&open).iter().all(|c| *c == TEXT),
        "and a usable menu is not dim anywhere"
    );
}

/// **Home never greys.** It is the one structure the engine waives the
/// program cost for, and it is the one a fresh run — which owns zero
/// programs by definition — has to be able to found. Greyed with the
/// rest, the very first screen of a new game would be a menu of dim rows
/// telling the player they cannot afford the thing they are about to do.
#[test]
fn home_stays_lit_on_a_roster_that_can_pay_for_nothing_else() {
    let entries = shipped_entries();
    let home = entries
        .iter()
        .position(|e| e.category == StructureCategory::Home)
        .expect("the shipped assets define a Home");
    assert_eq!(
        entry_colors(&build_menu_rows(&entries, 0, Some(DEPLOY_NEEDS_A_PROGRAM)))[home],
        TEXT
    );
    // And under the *other* shortfall too: the roster floor greys the
    // same rows for a different reason, and Home is exempt from the
    // program cost outright (`Game::structure_needs_program`), so a run
    // whose one program cannot be spent must still be able to found one.
    assert_eq!(
        entry_colors(&build_menu_rows(
            &entries,
            0,
            Some(DEPLOY_NEEDS_A_SECOND_PROGRAM)
        ))[home],
        TEXT
    );
}

/// **Nor does a Depot.** A storing structure costs no program either
/// (`StructureDef::needs_program`), and a shelf is exactly what a base
/// whose roster has nothing free still wants to be able to put up — so
/// greying it would dim the one row on the screen that is still worth
/// picking.
///
/// Asserted over every shipped structure the engine says is exempt
/// rather than over the id `"depot"`: the ladder runs to Mk6, the Zone
/// Portal is exempt too, and a seventh shelf is a file rather than a
/// code change.
#[test]
fn a_storing_structure_stays_lit_on_a_roster_that_can_pay_for_nothing() {
    let entries = shipped_entries();
    let colors = entry_colors(&build_menu_rows(&entries, 0, Some(DEPLOY_NEEDS_A_PROGRAM)));
    let shelves: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.needs_program && e.category != StructureCategory::Home)
        .map(|(i, _)| i)
        .collect();
    assert!(
        !shelves.is_empty(),
        "the shipped assets define at least one storing structure"
    );
    for i in shelves {
        assert_eq!(
            colors[i], TEXT,
            "{} costs no program, so a roster shortfall is not about it",
            entries[i].name
        );
    }
}
