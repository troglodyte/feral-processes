//! `Mode::DownedPrograms`: the store reached from the pack, and the
//! tool-and-yield picker for one held program — see
//! `docs/superpowers/specs/2026-09-04-program-extraction-design.md`, section
//! 6.
//!
//! One popup, two pages — `App::pending_downed_program_index` decides which
//! — the shape `draw_develop`/`draw_develop_program` take across two
//! `Mode`s collapsed into one: there is no per-tier ladder here to keep
//! apart from the list the way a Kernel Ring's page needs.

use super::popup::*;
use super::*;
use feral_processes_engine::views::ExtractionPreview;

pub(super) fn draw_downed_programs(
    game: &Game,
    pending_index: Option<usize>,
    bulk: bool,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    match pending_index {
        Some(index) => draw_extraction_options(game, index, bulk, selected, refusal, painter, m),
        None => draw_downed_program_list(game, selected, refusal, painter, m),
    }
}

/// Page one: every held program, `Game::downed_program_rows`' own order.
///
/// The row does not print `grade` — it is an unbounded internal fold of the
/// three fields already on the line (condition, rarity, level) and adds no
/// figure a player can act on, only a scaleless number. It stays on
/// `views::DownedProgramRow` for the engine's own use (and so a screen that
/// does want it later doesn't have to widen the interface).
pub(super) fn downed_program_list_rows(game: &Game, selected: usize) -> Vec<Row> {
    let programs = game.downed_program_rows();
    let mut rows = vec![text_row(
        "Downed programs held for extraction. Pick one to see what a tool would give.",
    )];
    if programs.is_empty() {
        rows.push(text_row(
            "Nothing held — a kill sometimes leaves a program behind.",
        ));
    }
    for (i, p) in programs.iter().enumerate() {
        let mut label = format!(
            "[{}] {} Lv{}  cond {}%",
            menu_shortcut(i),
            p.name,
            p.level,
            p.condition,
        );
        if let Some(tier) = p.rarity.label() {
            label.push_str(&format!("  {tier}"));
        }
        if p.boss {
            label.push_str("  (Boss)");
        }
        rows.push(tier_row(label, i == selected, 0, p.rarity));
    }
    rows.push(text_row(""));
    // Only when a rig is actually standing beside the party: a key hint the
    // player cannot act on reads as a broken binding, and adjacency is what
    // `Game::load_teardown_rig` refuses on.
    if !programs.is_empty() && game.adjacent_teardown_rig().is_some() {
        rows.push(text_row(
            "[L] hand the whole store to the Teardown Rig beside you",
        ));
    }
    rows.push(text_row("Esc to go back"));
    rows
}

fn draw_downed_program_list(
    game: &Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = downed_program_list_rows(game, selected);
    draw_popup(
        "Downed Programs",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// Page two: the program named by `index`, the bench that is working on it,
/// and every installed tool's own preview for it —
/// `Game::extraction_options`' order, names and figures, never re-derived
/// here. Phase 1 zipped the option list against `installed_tools()` to reach
/// a display name; the name rides the row now, so the renderer holds one
/// sequence rather than two that had to stay in step.
pub(super) fn extraction_options_rows(
    game: &Game,
    index: usize,
    bulk: bool,
    selected: usize,
) -> Vec<Row> {
    let programs = game.downed_program_rows();
    let Some(program) = programs.get(index) else {
        return vec![text_row("That program is gone.")];
    };
    let options = game.extraction_options(index);

    // What to say about an absent bench is the renderer's to word — the
    // engine answers `None` rather than building a "no bench" string.
    let bench = match game.extraction_bench() {
        Some(b) => format!(
            "{} tier {} — faster, and richer above tier 1.",
            b.name, b.tier
        ),
        None => "No extraction bench standing.".to_string(),
    };

    // **The header is the whole difference between the two intents**, since
    // the rows, the shortcuts and the previews below are identical under
    // both. A player who cannot tell them apart would hand the rig a program
    // they meant to strip by hand, or the reverse.
    let mut rows = if bulk {
        vec![
            text_row(format!(
                "Loading the Teardown Rig with all {} held programs — pick the tool it uses.",
                programs.len()
            )),
            // The figures below are one program's. Said out loud rather than
            // recomputed per program: a bulk load can carry ten different
            // levels and rarities, and a summed preview would quote a number
            // that no single strip ever pays.
            text_row(format!(
                "Figures are for the level {} {}; each program pays its own.",
                program.level, program.name
            )),
            text_row(bench),
            text_row(""),
        ]
    } else {
        vec![
            text_row(format!(
                "Extracting the level {} {} (condition {}%).",
                program.level, program.name, program.condition
            )),
            text_row(bench),
            text_row(""),
        ]
    };
    if options.is_empty() {
        rows.push(text_row("No tool is installed."));
    }
    for (i, option) in options.iter().enumerate() {
        let head_prefix = format!("[{}] {}", menu_shortcut(i), option.name);
        let ticks_suffix = format!("({} ticks)", option.ticks);
        // A `Gear` tool's candidate list can run far longer than the other
        // categories' — a boss species' whole droppable table, each row
        // naming an item and quoting a chance — so it sheds onto
        // `continuation_lines` instead of joining the row: the same shape
        // the routine and fuse pickers give a program's own kit, and for
        // the same reason (`push_extract_candidate`). Nothing clamps a row
        // horizontally, so an unwrapped join would run off the popup in
        // silence for the widest species.
        let (head, continuations) = match &option.preview {
            ExtractionPreview::Items(yields) if yields.is_empty() => (
                format!("{head_prefix}: nothing usable {ticks_suffix}"),
                Vec::new(),
            ),
            ExtractionPreview::Items(yields) => {
                let outcome = yields
                    .iter()
                    .map(|(item, qty)| format!("{qty} {}", game.item_name(item)))
                    .collect::<Vec<_>>()
                    .join(", ");
                (
                    format!("{head_prefix}: {outcome} {ticks_suffix}"),
                    Vec::new(),
                )
            }
            ExtractionPreview::Routine(names) => (
                format!(
                    "{head_prefix}: a routine — {} {ticks_suffix}",
                    names.join(" / ")
                ),
                Vec::new(),
            ),
            ExtractionPreview::NothingToLearn => (
                format!("{head_prefix}: nothing left to teach {ticks_suffix}"),
                Vec::new(),
            ),
            ExtractionPreview::Chances(chances) if chances.is_empty() => (
                format!("{head_prefix}: no gear to strip {ticks_suffix}"),
                Vec::new(),
            ),
            ExtractionPreview::Chances(chances) => {
                let outcome = chances
                    .iter()
                    .map(|(name, chance)| format!("{name} {:.0}%", chance * 100.0))
                    .collect::<Vec<_>>()
                    .join(", ");
                (
                    format!("{head_prefix} {ticks_suffix}"),
                    continuation_lines(&outcome),
                )
            }
        };
        rows.push(item_row(head, i == selected));
        for line in continuations {
            rows.push(colored_item_row(line, false, TEXT_DIM));
        }
    }
    rows.push(text_row(""));
    // `Q` is the per-row half of `L` on the list, and it is offered only
    // under hand intent: under bulk every row already loads the rig, so a
    // second key for it would say nothing.
    if !bulk && !options.is_empty() && game.adjacent_teardown_rig().is_some() {
        rows.push(text_row("[Q] hand this one to the Teardown Rig beside you"));
    }
    rows.push(text_row("Esc to go back"));
    rows
}

fn draw_extraction_options(
    game: &Game,
    index: usize,
    bulk: bool,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = extraction_options_rows(game, index, bulk, selected);
    draw_popup(
        "Downed Programs",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::DifficultyMode;
    use feral_processes_engine::items::DownedProgram;
    use feral_processes_engine::save;
    use feral_processes_engine::tools::{ToolDb, ToolId};
    use feral_processes_engine::tuning::{self, MAX_DOWNED_PROGRAMS, TOOL_SLOT_CAP};

    fn assets_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    fn program(species: &str, level: u32, rarity: Rarity) -> DownedProgram {
        DownedProgram {
            species: species.to_string(),
            level,
            rarity,
            boss: true,
            condition: 100,
            carried: None,
        }
    }

    /// The species id whose display name is longest in the shipped
    /// catalogue — derived rather than hand-picked, so a renamed or added
    /// species becomes the new worst case automatically instead of quietly
    /// leaving the census measuring a shorter string than the screen can
    /// actually draw.
    fn widest_species_id(game: &Game) -> String {
        game.species_defs()
            .into_iter()
            .max_by_key(|def| def.name.chars().count())
            .expect("the shipped catalogue defines at least one species")
            .id
    }

    /// The `Rarity` whose `label()` is longest — `widest_species_id`'s
    /// reason, and walked over `Rarity::ALL` rather than assumed to be the
    /// top rung: it isn't — `Gold`'s "Overclocked" (11) outruns
    /// `Prismatic`'s "Bare-Metal" (10).
    fn widest_rarity() -> Rarity {
        Rarity::ALL
            .into_iter()
            .max_by_key(|r| r.label().map(str::len).unwrap_or(0))
            .expect("Rarity::ALL is non-empty")
    }

    /// A real `Game` holding `held`, with `tools` installed and `bench`
    /// (a structure kind and its tier) standing if given — through a
    /// save/edit/load round trip, `app_holding_downed_programs`'s reason
    /// (`crates/app-core/src/tests/support.rs`): the engine exposes no way to
    /// hand-place a `DownedProgram` from outside itself, and `Game::world` is
    /// private to it besides — this crate could not reach in even if it
    /// wanted to.
    ///
    /// The path is keyed on an atomic counter, not just `seed` — the test
    /// binary runs cases as concurrent threads, and two calls sharing a
    /// seed shared one file and raced (`scratch_path`'s own reason, in
    /// app-core's fixtures).
    fn game_with_state(
        seed: u32,
        held: Vec<DownedProgram>,
        tools: Option<Vec<ToolId>>,
        bench: Option<(String, u32, (i32, i32))>,
    ) -> Game {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let assets = assets_dir();
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &assets).unwrap();
        let path = std::env::temp_dir().join(format!(
            "fp_gui_downed_programs_{seed}_{unique}_{}.sav",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut data = save::load_from_file(&path).unwrap();
        data.player.downed_programs = held;
        if let Some(tools) = tools {
            data.player.tools = tools;
        }
        // Base space, standing on `(0, 0)`. `extraction_bench_tier` does not
        // care where the party is, but `Game::adjacent_teardown_rig` does —
        // and the two rig key hints only draw when it answers `Some`, so a
        // fixture on the surface would measure a page a rig owner never
        // sees.
        data.locale = feral_processes_engine::resources::Locale::Base { x: 0, y: 0 };
        if let Some((kind, tier, position)) = bench {
            data.structures.push(save::StructureSave {
                kind,
                position,
                durability: None,
                tier: Some(tier),
                stock_input: Vec::new(),
                stock_output: Vec::new(),
                standing_work: false,
                standing_guard: false,
                denied_items: Vec::new(),
                power_fuel: tuning::POWER_UPKEEP_TICKS,
                build_quality: 1.0,
                racked: Vec::new(),
                hopper: Vec::new(),
                hopper_progress: 0,
            });
        }
        save::save_to_file(&path, &data).unwrap();
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);
        loaded
    }

    fn game_holding_downed_programs(seed: u32, held: Vec<DownedProgram>) -> Game {
        game_with_state(seed, held, None, None)
    }

    /// The list page's worst case: `MAX_DOWNED_PROGRAMS` rows, every one the
    /// longest-named species at the longest-labelled rarity and a boss (the
    /// only longer of the two tag options) — every axis derived from the
    /// shipped catalogue, not hand-picked, so a new species or a renamed
    /// rarity is covered automatically.
    fn tallest_and_widest_list_game() -> Game {
        let probe = Game::new(9698, DifficultyMode::Forgiving, &assets_dir()).unwrap();
        let species = widest_species_id(&probe);
        let rarity = widest_rarity();
        let held = vec![program(&species, 999, rarity); MAX_DOWNED_PROGRAMS];
        // A rig orthogonally east of the party, so the `[L]` hint row is in
        // the count. It is the tallest case: the hint draws only when one is
        // standing beside you, and a fixture without one would be a row short
        // of what a rig owner's screen actually builds.
        game_with_state(
            9700,
            held,
            None,
            Some(("teardown_rig".to_string(), 1, (1, 0))),
        )
    }

    /// Every shipped tool id, catalogue order — cycled into the slots below
    /// rather than two ids named here, so a new tool file (the Routine
    /// Reader was one) joins the worst case instead of being the row the
    /// census never drew. Loaded straight off disk because `Game` exposes no
    /// whole-catalogue accessor: `tool_rows` answers for what the player
    /// knows, which on a fresh run is the starter alone.
    fn every_shipped_tool_id() -> Vec<ToolId> {
        let (db, _) = ToolDb::load_dir(&assets_dir().join("tools")).unwrap();
        db.all().map(|def| def.id.clone()).collect()
    }

    /// The longest-named structure that improves an extraction, at the top of
    /// its own upgrade ladder — the widest bench line the header can draw,
    /// derived the way `widest_species_id` is. `None` if nothing ships with
    /// the flag, which its own engine census already fails on.
    fn widest_bench(probe: &Game) -> Option<(String, u32)> {
        probe
            .structure_defs()
            .into_iter()
            .filter(|def| def.extracts_programs)
            .max_by_key(|def| def.name.chars().count())
            .map(|def| {
                let tier = def.upgrade.as_ref().map(|u| u.max_tier).unwrap_or(1);
                (def.id, tier)
            })
    }

    /// The tool page's worst case: the same widest program above, run through
    /// `TOOL_SLOT_CAP` filled slots rather than the single starter tool a
    /// fresh run installs — spec section 6 names that cap as the asserted
    /// constraint, so a page that only ever sees one tool never exercises it.
    /// Exactly `TOOL_SLOT_CAP` tools ship as of phase 3, so the wrap below
    /// repeats nothing today; it is there so a removed tool file still
    /// fills every slot rather than quietly shrinking the fixture the cap
    /// is asserted against.
    ///
    /// A top-tier bench stands too, so the header line under test is the
    /// long one — "no bench standing" is the shorter of the two, and a
    /// census fitted against it would measure a string the screen only draws
    /// on a fresh run.
    fn tallest_and_widest_options_game() -> Game {
        let probe = Game::new(9698, DifficultyMode::Forgiving, &assets_dir()).unwrap();
        let species = widest_species_id(&probe);
        let rarity = widest_rarity();
        let held = vec![program(&species, 999, rarity)];
        let shipped = every_shipped_tool_id();
        let tools = (0..TOOL_SLOT_CAP as usize)
            .map(|i| shipped[i % shipped.len()].clone())
            .collect();
        // Beside the party, so the `[Q]` hint row joins the count — and the
        // widest bench *is* the Teardown Rig, so the long header line and
        // the hint are the same structure standing.
        let bench = widest_bench(&probe).map(|(kind, tier)| (kind, tier, (1, 0)));
        game_with_state(9702, held, Some(tools), bench)
    }

    /// This page has no scroll (spec section 6), so its height is a layout
    /// constraint — `the_tallest_memory_page_fits_its_popup`'s shape.
    /// `PopupSize::Large` at 720px has real headroom (28 rows, against 13
    /// for this store at the shipped cap of 10), so a `+1` mutation stays
    /// green: checked by hand, `MAX_DOWNED_PROGRAMS` had to reach 26 before
    /// this failed, and reverted — the constant itself is the thing under
    /// test, not a second copy of it here.
    #[test]
    fn the_tallest_downed_program_list_fits_its_popup_at_1280x720() {
        let game = tallest_and_widest_list_game();
        let rows = downed_program_list_rows(&game, 0).len();
        let m = ui_metrics(720.0);
        let cap = popup_max_rows(720.0, PopupSize::Large, &m);
        assert!(
            rows <= cap,
            "a full store ({MAX_DOWNED_PROGRAMS} programs) builds a {rows}-row page into a \
             {cap}-row popup at 1280x720"
        );
    }

    /// The other axis, `no_memory_row_overflows_its_popup`'s shape but over
    /// `Row::Item` rows rather than `Row::Text` — this page's list is
    /// exactly the row kind that trap warns a naive width census skips.
    #[test]
    fn no_downed_program_row_overflows_the_popup_body_at_1280x720() {
        let game = tallest_and_widest_list_game();
        let rows = downed_program_list_rows(&game, 0);
        let m = ui_metrics(720.0);
        let body = popup_body_width(1280.0, PopupSize::Large, &m);
        crate::paint::with_painter(|p| {
            for row in &rows {
                let label = row_label_text(row);
                let width = p.measure_ui_advance(&label, m.font_size);
                assert!(
                    width <= body,
                    "a downed-program row draws {width}px into a {body}px body at 1280x720: \
                     {label:?}"
                );
            }
        });
    }

    /// The tool page's own worst case: the widest program and the widest
    /// bench, so both header lines are included, run through
    /// `TOOL_SLOT_CAP` filled slots.
    ///
    /// Verified by mutation the way the list census above was, and with the
    /// same finding: `PopupSize::Large` at 720px holds 28 rows against this
    /// page's 9, so a `+1` slot stays green — the fixture had to reach 40
    /// slots (a 45-row page) before this failed, and was reverted. The cap
    /// constant is the thing under test, not a second copy of it here.
    #[test]
    fn the_tallest_extraction_options_page_fits_its_popup_at_1280x720() {
        let game = tallest_and_widest_options_game();
        let rows = extraction_options_rows(&game, 0, false, 0).len();
        let m = ui_metrics(720.0);
        let cap = popup_max_rows(720.0, PopupSize::Large, &m);
        assert!(
            rows <= cap,
            "the tool page builds a {rows}-row page into a {cap}-row popup at 1280x720"
        );
    }

    #[test]
    fn no_extraction_options_row_overflows_the_popup_body_at_1280x720() {
        let game = tallest_and_widest_options_game();
        let rows = extraction_options_rows(&game, 0, false, 0);
        let m = ui_metrics(720.0);
        let body = popup_body_width(1280.0, PopupSize::Large, &m);
        crate::paint::with_painter(|p| {
            for row in &rows {
                let label = row_label_text(row);
                let width = p.measure_ui_advance(&label, m.font_size);
                assert!(
                    width <= body,
                    "an extraction-options row draws {width}px into a {body}px body at \
                     1280x720: {label:?}"
                );
            }
        });
    }

    /// The two height censuses above are only worth their fixtures if the
    /// hint rows are really in them — a rig that failed to stand would make
    /// both measure a page one row shorter than a rig owner's, silently.
    #[test]
    fn the_worst_case_fixtures_really_draw_the_rig_hints() {
        let list = tallest_and_widest_list_game();
        assert!(
            list.adjacent_teardown_rig().is_some(),
            "the list fixture must stand a rig beside the party"
        );
        assert!(
            downed_program_list_rows(&list, 0)
                .iter()
                .any(|r| row_label_text(r).contains("[L]")),
            "the list's worst case must include the [L] hint row"
        );

        let options = tallest_and_widest_options_game();
        assert!(
            extraction_options_rows(&options, 0, false, 0)
                .iter()
                .any(|r| row_label_text(r).contains("[Q]")),
            "the tool page's worst case must include the [Q] hint row"
        );
    }

    /// **The header is the only thing that tells the two intents apart**, so
    /// a player who reads it wrong hands the rig a program they meant to
    /// strip by hand. Asserted as a difference rather than against a literal
    /// string, and with `[Q]` gone under bulk — every row already loads the
    /// rig there, so a second key for it would say nothing.
    #[test]
    fn bulk_intent_says_so_in_the_header_and_drops_the_per_row_hint() {
        let game = tallest_and_widest_options_game();
        let hand = extraction_options_rows(&game, 0, false, 0);
        let bulk = extraction_options_rows(&game, 0, true, 0);

        assert_ne!(
            row_label_text(&hand[0]),
            row_label_text(&bulk[0]),
            "the two intents must not open with the same line"
        );
        assert!(row_label_text(&bulk[0]).contains("Teardown Rig"));
        assert!(
            hand.iter().any(|r| row_label_text(r).contains("[Q]")),
            "hand intent offers the per-row rig key"
        );
        assert!(
            !bulk.iter().any(|r| row_label_text(r).contains("[Q]")),
            "bulk intent does not — every row already loads the rig"
        );
    }

    /// The list and the tool page must agree with what the engine actually
    /// counts — `Game::downed_program_rows().len()` — rather than the
    /// renderer keeping its own idea of how many rows there are.
    #[test]
    fn the_list_row_count_agrees_with_the_engine() {
        let game = game_holding_downed_programs(
            9701,
            vec![
                program("scrapper", 1, Rarity::Ordinary),
                program("scrapper", 2, Rarity::Ordinary),
                program("scrapper", 3, Rarity::Ordinary),
            ],
        );
        let rows = downed_program_list_rows(&game, 0);
        let item_rows = rows
            .iter()
            .filter(|r| matches!(r, Row::Item { .. }))
            .count();
        assert_eq!(item_rows, game.downed_program_rows().len());
    }

    /// A `Gear` tool's row names the item and quotes a `%` — the renderer's
    /// own new arm for `ExtractionPreview::Chances`, exercised through a
    /// real installed `harness_puller` rather than a hand-built preview, so
    /// the row and `Game::gear_chances` cannot quietly disagree.
    #[test]
    fn a_gear_row_names_the_item_and_quotes_a_percent() {
        let probe = Game::new(9698, DifficultyMode::Forgiving, &assets_dir()).unwrap();
        let (tool_db, _) = ToolDb::load_dir(&assets_dir().join("tools")).unwrap();
        let tool = tool_db.get("harness_puller").unwrap().clone();
        let species_id = probe
            .species_defs()
            .into_iter()
            .find(|s| {
                !probe
                    .gear_chances(&program(&s.id, 999, Rarity::Ordinary), &tool)
                    .is_empty()
            })
            .map(|s| s.id)
            .expect("at least one shipped species should drop gear");
        let expected = probe.gear_chances(&program(&species_id, 3, Rarity::Ordinary), &tool);
        let expected_item_name = probe.item_name(&expected[0].0).to_string();

        let held = vec![program(&species_id, 3, Rarity::Ordinary)];
        let game = game_with_state(
            9703,
            held,
            Some(vec![ToolId("harness_puller".to_string())]),
            None,
        );
        let rows = extraction_options_rows(&game, 0, false, 0);
        let head_index = rows
            .iter()
            .map(row_label_text)
            .position(|text| text.contains(&tool.name))
            .expect("the installed Gear tool should have a row");
        // The chance list sheds onto continuation lines below the tool's own
        // row (`continuation_lines`, the same shape the routine and fuse
        // pickers use), so the item name and its `%` land somewhere in the
        // span that follows the header rather than on the header itself.
        let joined: String = rows[head_index..]
            .iter()
            .map(row_label_text)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            joined.contains('%'),
            "a Gear tool's rows should quote a percent: {joined:?}"
        );
        assert!(
            joined.contains(&expected_item_name),
            "a Gear tool's rows should name its candidate item {expected_item_name:?}: {joined:?}"
        );
    }
}
