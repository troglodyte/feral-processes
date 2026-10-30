//! The program picker: the screen that spends a tamed program on a build
//! order, and the one keypress in the base loop that deletes a program from
//! the roster.

use super::*;
use crate::render::test_pet;

fn row_text(row: &Row) -> &str {
    match row {
        Row::Text(t) | Row::TextColored(t, _) => t,
        Row::Item { text, .. } => text,
    }
}

fn deploy(label: &str) -> BuildCommit {
    BuildCommit {
        label: label.to_string(),
        to_tier: None,
        structure: "assembler".to_string(),
        cost: Vec::new(),
        rebuild: false,
    }
}

fn upgrade(label: &str, to_tier: u32) -> BuildCommit {
    upgrade_costing(label, to_tier, Vec::new())
}

/// The same, with the bill the crew would fetch — the figure the list
/// this screen replaced used to carry per row.
fn upgrade_costing(label: &str, to_tier: u32, cost: Vec<String>) -> BuildCommit {
    BuildCommit {
        label: label.to_string(),
        to_tier: Some(to_tier),
        structure: "extractor".to_string(),
        cost,
        rebuild: false,
    }
}

fn rebuild(label: &str) -> BuildCommit {
    BuildCommit {
        rebuild: true,
        ..deploy(label)
    }
}

#[test]
fn a_rebuild_picker_names_the_rebuild_in_title_warning_and_footer() {
    let m = crate::text::ui_metrics(900.0);
    let mut game = crate::render::test_support::game_with_a_free_and_a_wielded_program(4010);
    let (_, shapes) = crate::paint::with_painter(|p| {
        draw_build_program(&mut game, Some(rebuild("Mining Node")), 0, None, p, &m);
    });
    let drawn = crate::paint::painted_text(&shapes).join("\n");
    assert!(drawn.contains("Rebuild a structure"), "{drawn}");
    assert!(drawn.contains("Rebuilding the Mining Node"), "{drawn}");
    assert!(drawn.contains("Which program rebuilds it?"), "{drawn}");
    assert!(!drawn.contains("Which program pays for it?"), "{drawn}");
}

/// A candidate whose machine really does have a rate to change.
fn candidate(name: &str, effect: BuildEffect) -> BuildCandidate {
    BuildCandidate {
        pet: test_pet(name, "w|a|m"),
        aptitude: "Assembly",
        label: "Excellent",
        effect,
    }
}

fn cycling(name: &str) -> BuildCandidate {
    candidate(
        name,
        BuildEffect::Cycle {
            shipped: 20,
            built: 18,
        },
    )
}

/// The tier is the engine's own answer, not a restated copy: this is the
/// number `App::handle_build_program_key` hands `programs_for_build`, so
/// a second rule here would offer a program the engine then refuses.
#[test]
fn the_picker_asks_the_engine_how_deep_a_program_it_needs() {
    assert_eq!(deploy("Fabricator").tier(), 1, "every deploy is tier 1");
    assert_eq!(upgrade("Mining Node", 4).tier(), 4);
}

/// The prompt names what is being paid for and says what paying costs.
/// **"Permanently" is the whole point of the sentence** — this keypress
/// deletes a program, and a picker that read like an equipment menu
/// would be inviting it.
#[test]
fn the_picker_names_the_structure_and_says_the_cost_is_permanent() {
    let rows = build_program_rows(Some(&deploy("Fabricator")), &[cycling("Sparkgrub")], 0);
    let text = rows.iter().map(row_text).collect::<Vec<_>>().join(" ");
    assert!(text.contains("Fabricator"), "{text}");
    assert!(text.contains("permanently"), "{text}");
}

/// An upgrade names the tier it is buying, because that is what decides
/// how deep a program it costs — "Extractor" alone would leave the
/// player reading a zone requirement with nothing to attach it to.
#[test]
fn an_upgrade_prompt_names_the_tier_the_program_is_buying() {
    let rows = build_program_rows(Some(&upgrade("Mining Node", 3)), &[], 0);
    let text = rows.iter().map(row_text).collect::<Vec<_>>().join(" ");
    assert!(text.contains("Mining Node"), "{text}");
    assert!(text.contains("Mk3"), "{text}");
    assert!(text.contains("permanently"), "{text}");
}

/// The refund is real — `return_build_holdings` hands the program back
/// through either destruction door while the order is still a build
/// request — so the prompt says how long the take-back lasts rather than
/// claiming the program never comes back.
#[test]
fn the_prompt_says_the_spend_is_final_only_once_the_structure_stands() {
    let text = deploy("Fabricator").prompt();
    assert!(text.contains("comes back"), "{text}");
    assert!(text.contains("stands"), "{text}");
}

/// The roster's own row format, not a bare list of names: this decision
/// is which program to delete, so it wants the stat line the party
/// screen shows.
#[test]
fn the_picker_draws_each_candidate_as_the_roster_draws_it() {
    let pets = [cycling("Sparkgrub"), cycling("Nibbler")];
    let rows = build_program_rows(Some(&deploy("Fabricator")), &pets, 0);
    let text = rows.iter().map(row_text).collect::<Vec<_>>().join("\n");
    assert!(text.contains("HP 22/28"), "the roster's stat line: {text}");
    assert!(text.contains("Sparkgrub"), "{text}");
    assert!(text.contains("Nibbler"), "{text}");
}

/// **The renderer draws what app-core counts, and never filters it a
/// second time.** `App::handle_build_program_key` resolves a keypress
/// against `programs_for_build`'s list by index, so a row dropped or
/// reordered here would spend the program on the row below the one the
/// player read.
#[test]
fn every_candidate_gets_a_row_in_the_order_it_arrived() {
    let pets: Vec<BuildCandidate> = ["Sparkgrub", "Nibbler", "Grinder"]
        .into_iter()
        .map(cycling)
        .collect();
    let rows = build_program_rows(Some(&deploy("Fabricator")), &pets, 0);
    let heads: Vec<&str> = rows
        .iter()
        .filter(|r| matches!(r, Row::Item { .. }))
        .map(row_text)
        .filter(|t| !t.starts_with(' '))
        .collect();
    assert_eq!(heads.len(), pets.len(), "one row per candidate: {heads:?}");
    for (i, pet) in pets.iter().enumerate() {
        assert!(
            heads[i].contains(&pet.pet.name),
            "row {i} is {:?}, not {}",
            heads[i],
            pet.pet.name
        );
    }
}

/// The quote is two whole tick figures and no percent sign — see
/// `views::BuildEffect` for why a percentage would be wrong on a short
/// cycle.
#[test]
fn a_picker_row_quotes_the_cycle_in_ticks() {
    let rows = build_program_rows(Some(&deploy("Fabricator")), &[cycling("Sparkgrub")], 0);
    let text = rows.iter().map(row_text).collect::<Vec<_>>().join("\n");
    assert!(
        text.contains("cycle 20 -> 18 ticks"),
        "both figures are not quoted: {text}"
    );
    assert!(
        text.contains("[Assembly: Excellent]"),
        "the aptitude tag is missing: {text}"
    );
    // The stat line's own `MIT 5%` is not the quote, so the assertion is
    // on the tag: a percentage *of the change* is the reading
    // `BuildEffect` exists to refuse.
    let quote = text
        .split("[Fabricator cycle")
        .nth(1)
        .expect("the cycle tag is drawn");
    assert!(
        !quote.split(']').next().unwrap_or_default().contains('%'),
        "the cycle quote is a percentage: {quote}"
    );
}

/// A build with no cycle says so once, above the list, and no row
/// carries a cycle tag — `Cycle { shipped: n, built: n }` is the right
/// answer for a cycle too short to move and the wrong one here.
#[test]
fn a_no_cycle_build_says_so_once_above_the_list() {
    let candidates: Vec<BuildCandidate> = ["Sparkgrub", "Nibbler", "Grinder"]
        .into_iter()
        .map(|n| candidate(n, BuildEffect::NoCycle))
        .collect();
    let rows = build_program_rows(Some(&deploy("Depot")), &candidates, 0);

    let joined = rows.iter().map(row_text).collect::<Vec<_>>().join(" ");
    assert_eq!(
        joined.matches("does not run a work cycle").count(),
        1,
        "the sentence is repeated or missing: {joined}"
    );
    // Pinned, not scrolled: `popup_layout` puts `Row::Text` in the
    // header, and a fact about the whole screen must not page away from
    // the list it is about.
    let note = rows
        .iter()
        .position(|r| row_text(r).contains("does not run a work cycle"))
        .expect("the sentence is drawn");
    let first_item = rows
        .iter()
        .position(|r| matches!(r, Row::Item { .. }))
        .expect("the rows are drawn");
    assert!(note < first_item, "the sentence is below the list");
    assert!(
        matches!(rows[note], Row::Text(_)),
        "the sentence would scroll with the list"
    );

    for row in rows.iter().filter(|r| matches!(r, Row::Item { .. })) {
        assert!(
            !row_text(row).contains("cycle"),
            "a row quoted a cycle a Depot does not have: {}",
            row_text(row)
        );
    }
}

/// A configurable machine quotes no pace but never claims the program
/// does not matter.
#[test]
fn a_machine_build_says_the_program_sets_its_pace() {
    let candidates = vec![candidate("Sparkgrub", BuildEffect::ScalesRecipe)];
    let rows = build_program_rows(Some(&deploy("Factory")), &candidates, 0);
    let text = rows.iter().map(row_text).collect::<Vec<_>>().join(" ");
    assert!(text.contains("sets how fast this machine works"), "{text}");
    assert!(!text.contains("does not run a work cycle"), "{text}");
    assert!(!text.contains("cycle"), "{text}");
}

/// The role headings did not survive the sort, so what each program is
/// doing has to still be on its row — that is what keeps "spending it
/// also empties a job" on the screen.
#[test]
fn the_picker_still_says_what_each_program_is_doing() {
    let mut posted = cycling("Sparkgrub");
    posted.pet.activity = "working Mining Node".to_string();
    let mut idle = cycling("Nibbler");
    idle.pet.activity = "idle".to_string();

    let rows = build_program_rows(Some(&deploy("Fabricator")), &[posted, idle], 0);
    let text = rows.iter().map(row_text).collect::<Vec<_>>().join("\n");
    assert!(
        text.contains("working Mining Node"),
        "the post is not on the row: {text}"
    );
    assert!(text.contains("idle"), "{text}");
    for heading in ["Base staff", "In your party"] {
        assert!(
            !text.contains(heading),
            "a role heading survived the sort: {text}"
        );
    }
}

/// The pixel counterpart of `no_program_picker_row_runs_past_the_popup_body`,
/// in `no_deploy_row_overflows_its_popup_in_pixels`' shape — the worst
/// case now carries the aptitude tag, the cycle quote, an overlong
/// structure name and an overlong program name.
#[test]
fn no_picker_row_overflows_its_popup_in_pixels() {
    let candidates: Vec<BuildCandidate> = (0..6)
        .map(|i| {
            let mut c = candidate(
                &format!("Overclocked Overlong Program {i} 10"),
                BuildEffect::Cycle {
                    shipped: 9999,
                    built: 9999,
                },
            );
            c.aptitude = "Extraction";
            c.label = "Below Average";
            c.pet.quality = Some("Below Average (100%)".to_string());
            c.pet.assembly = Some("Below Average".to_string());
            c.pet.extraction = Some("Below Average".to_string());
            c.pet.activity = "guarding Contract Broker".to_string();
            c.pet.fusions = MAX_FUSIONS;
            c.pet.hp = 1;
            c.pet.max_hp = 1234;
            c.pet.atk = 1234;
            c.pet.power = 1234;
            c
        })
        .collect();
    let commits = [
        deploy("Recompiled Kernel Substrate Assembly Bay"),
        upgrade("Recompiled Kernel Substrate Assembly Bay", 9),
        // With a bill on it: three materials and their two figures each
        // is the longest line this screen can be handed, and it arrived
        // with `Mode::UpgradeDirection` — the list it replaced wrapped
        // its own rows and this one has to wrap too.
        upgrade_costing(
            "Recompiled Kernel Substrate Assembly Bay",
            9,
            vec![
                "Recompiled Bytecode Block (120/144)".to_string(),
                "Cache Grain Lattice (0/96)".to_string(),
                "Kernel Substrate Ingot (18/72)".to_string(),
            ],
        ),
    ];
    crate::paint::with_painter(|p| {
        let m = crate::text::ui_metrics(900.0);
        // 0.88 is `PopupSize::Large`'s width fraction, against the
        // 1440x900 geometry `ui_metrics` is calibrated for.
        let room = 1440.0 * 0.88 - m.pad * 2.0;
        for commit in &commits {
            for row in build_program_rows(Some(commit), &candidates, 0) {
                let text = match &row {
                    Row::Text(t) | Row::TextColored(t, _) => t.clone(),
                    // `draw_row`'s own prefix on an item row.
                    Row::Item { text, .. } => format!("     {text}"),
                };
                let drawn = p.measure_ui_advance(&text, m.font_size);
                assert!(
                    drawn <= room,
                    "a picker row overflows its popup by {:.0}px \
                     ({drawn:.0} into {room:.0}):\n{text}",
                    drawn - room
                );
            }
        }
    });
}

/// The empty picker is a **first-class state**, not an edge.
/// `App::selected_index` returns `None` for a zero-length list, so a
/// player who reaches it with nothing to spend lands on a box with no
/// rows and Esc as the only exit — and, without this, nothing at all
/// saying why.
///
/// Three sentences because there are three reasons and only one of them
/// is depth. A deploy asks for tier 1, and every tamed program is from
/// zone 1 or deeper, so blaming depth there would be plainly false to a
/// player looking at a roster full of programs.
///
/// The roster floor answers ahead of both, and this pins that order: it
/// is the one reason that holds at *every* tier, so a base with one
/// zone 5 program asked for a zone 4 upgrade must not be told its
/// roster is too shallow.
#[test]
fn an_empty_picker_says_why_and_a_deploy_never_blames_depth() {
    assert_eq!(
        no_candidates_refusal(1, false),
        "No program on your roster is free to spend on this.",
        "at tier 1 the roster is deep enough by construction; being free is the question"
    );
    assert_eq!(
        no_candidates_refusal(4, false),
        "Nothing on your roster is from zone 4 or deeper."
    );
    assert_eq!(
        no_candidates_refusal(4, true),
        "Committing your last program would leave nobody to build.",
        "the floor answers first: a lone zone 5 program is not too shallow for a zone 4 \
         upgrade, and saying so would be plainly false"
    );
    assert_eq!(
        no_candidates_refusal(1, true),
        no_candidates_refusal(4, true),
        "the floor is about the base, so the tier cannot change what it says"
    );
    for last_program in [true, false] {
        assert!(
            !no_candidates_refusal(1, last_program).contains("Home"),
            "the picker is never reached for a Home, so it has no exemption to explain"
        );
    }
}

/// Both lines the frontend prints for the roster floor quote
/// `LAST_PROGRAM_CLAUSE`, which is `Game::commit_for_build`'s own
/// wording with the structure name — the one thing a menu cannot know —
/// taken off the end.
///
/// Held here because the alternative is what this branch has already
/// had to undo twice: screens each inventing their own phrasing of one
/// rule, so a player who meets the floor at the menu and again at the
/// confirm reads two different sentences and has to work out whether
/// they are the same refusal.
///
/// **Two lines where there were three.** The upgrade menu had one of its
/// own; `Mode::UpgradeDirection` has no list to head, and the floor now
/// reaches an upgrading player at the picker like every other refusal.
#[test]
fn the_last_program_lines_share_the_engines_clause() {
    for line in [
        DEPLOY_NEEDS_A_SECOND_PROGRAM,
        &no_candidates_refusal(1, true),
    ] {
        assert!(
            line.starts_with(LAST_PROGRAM_CLAUSE),
            "{line:?} does not open with the engine's own clause"
        );
    }
    assert!(
        DEPLOY_NEEDS_A_SECOND_PROGRAM.contains("Home"),
        "the deploy menu greys everything but Home, so its line has to say so"
    );
}

/// The bill the crew will fetch is on this screen, because
/// `Mode::UpgradeDirection` is the first screen that knows which machine
/// the order is for and so cannot carry it.
///
/// Priced against the pack **and** the shelves — `build_cost_display`'s
/// form, which `build_commit` reaches through `Game::upgrade_cost`, the
/// same door the list this replaced used.
#[test]
fn an_upgrade_commit_quotes_what_the_crew_has_to_fetch() {
    let commit = upgrade_costing("Mining Node", 2, vec!["Bytecode Block (4/12)".to_string()]);
    let rows = build_program_rows(Some(&commit), &[cycling("Sparkgrub")], 0);
    let drawn: Vec<&str> = rows.iter().map(row_text).collect();
    assert!(
        drawn.iter().any(|t| t.contains("Bytecode Block (4/12)")),
        "the order has to say what it will cost the base: {drawn:?}"
    );
}

/// A deploy does not repeat it: `draw_build_direction` has already
/// quoted the bill one screen earlier, where the structure was chosen
/// before the tile.
#[test]
fn a_deploy_commit_does_not_repeat_the_bill() {
    let rows = build_program_rows(Some(&deploy("Fabricator")), &[cycling("Sparkgrub")], 0);
    let drawn: Vec<&str> = rows.iter().map(row_text).collect();
    assert!(
        !drawn.iter().any(|t| t.starts_with("Your crew fetches")),
        "the deploy direction screen already said it: {drawn:?}"
    );
}

/// Nothing here runs off the right edge: `draw_row` clamps a row
/// vertically and never horizontally, so an unwrapped prompt is drawn
/// straight past the box in silence — the fault the deploy menu shipped
/// with, in the screen that inherits its shape.
#[test]
fn no_program_picker_row_runs_past_the_popup_body() {
    let pets: Vec<BuildCandidate> = (0..12)
        .map(|i| cycling(&format!("Overlong Program Name {i}")))
        .collect();
    let commits = [
        deploy("Recompiled Kernel Substrate Assembly Bay"),
        upgrade("Recompiled Kernel Substrate Assembly Bay", 9),
        // The bill `Mode::UpgradeDirection` pushed onto this screen —
        // unwrapped it is 118 columns, so this case is what holds the
        // wrap in place.
        upgrade_costing(
            "Recompiled Kernel Substrate Assembly Bay",
            9,
            vec![
                "Recompiled Bytecode Block (120/144)".to_string(),
                "Cache Grain Lattice (0/96)".to_string(),
                "Kernel Substrate Ingot (18/72)".to_string(),
            ],
        ),
    ];
    for commit in &commits {
        for row in build_program_rows(Some(commit), &pets, 0) {
            let text = row_text(&row);
            assert!(
                text.chars().count() <= ROW_WRAP_COLUMNS,
                "a {} char picker row runs past the {ROW_WRAP_COLUMNS} column body: {text:?}",
                text.chars().count()
            );
        }
    }
}

/// `popup_layout` ends the scrollable body at the *last* `Row::Item` and
/// pins whatever follows as a footer, so a continuation line under the
/// final program would be torn off it and drawn at the bottom of the
/// box. Nothing may follow the last item row.
#[test]
fn every_picker_row_stays_inside_the_scrollable_body() {
    let pets: Vec<BuildCandidate> = (0..6).map(|i| cycling(&format!("Program {i}"))).collect();
    for selected in [0, pets.len() - 1] {
        let rows = build_program_rows(Some(&deploy("Fabricator")), &pets, selected);
        let last = rows
            .iter()
            .rposition(|r| matches!(r, Row::Item { .. }))
            .expect("a candidate is an item row");
        assert_eq!(
            last,
            rows.len() - 3,
            "the picker pinned {} rows below its list instead of just the key footer",
            rows.len() - last - 1
        );
    }
}

/// The warning is pinned above the list rather than scrolling with it:
/// every prompt row is `Row::Text`, so `popup_layout` puts the lot in
/// the header. A warning that pages off the screen it is about is not a
/// warning.
#[test]
fn the_warning_is_pinned_above_the_candidates() {
    let pets: Vec<BuildCandidate> = (0..40).map(|i| cycling(&format!("Program {i}"))).collect();
    let rows = build_program_rows(Some(&deploy("Fabricator")), &pets, 39);
    let first_item = rows
        .iter()
        .position(|r| matches!(r, Row::Item { .. }))
        .expect("a candidate is an item row");
    let warning = rows
        .iter()
        .position(|r| row_text(r).contains("permanently"))
        .expect("the picker drew its warning");
    assert!(
        warning < first_item,
        "the warning fell into the scrolling body"
    );
}

/// **The only test in this file that drives `draw_build_program`
/// itself, rather than `build_program_rows` in isolation.** Every other
/// test above pins the row *content* against hand-built `PetInfo`
/// fixtures; none of them touch the three lines inside
/// `draw_build_program` that turn a real `Game` into that list —
/// `game.programs_for_build(tier)`, handed to `build_program_rows` as
/// `&candidates`. A prior review round found that gap: mutate
/// `&candidates` to `&[]`, or substitute `game.owned_pets()` for
/// `game.programs_for_build(tier)`, and every test in this crate still
/// passed — the one painted test that reaches this screen
/// (`an_empty_program_picker_says_why_it_is_empty`, `render/mod.rs`)
/// uses a fresh run with zero programs, so both mutations are
/// indistinguishable from the correct code on that fixture.
///
/// `game_with_a_free_and_a_wielded_program` (`render/test_support.rs`)
/// closes it: one program `programs_for_build` keeps, one it drops
/// because it's wielded. `&candidates -> &[]` paints neither name.
/// `owned_pets()` in place of `programs_for_build` paints both. Only the
/// real call, passed through untouched, paints exactly the free one.
#[test]
fn draw_build_program_only_lists_what_the_engine_will_accept() {
    let mut game = crate::render::test_support::game_with_a_free_and_a_wielded_program(4001);
    let m = crate::text::ui_metrics(900.0);
    let (_, shapes) = crate::paint::with_painter(|p| {
        draw_build_program(&mut game, Some(deploy("Fabricator")), 0, None, p, &m);
    });
    let drawn = crate::paint::painted_text(&shapes);
    let says = |want: &str| drawn.iter().any(|t| t.contains(want));
    assert!(
        says("Free Sparkgrub"),
        "programs_for_build(1) keeps this one: {drawn:?}"
    );
    assert!(
        !says("Wielded Sparkgrub"),
        "programs_for_build(1) drops a wielded program — owned_pets() would not: {drawn:?}"
    );
}

/// **The one-program base, end to end**, on all three screens that ask
/// the roster whether a build can be paid for.
///
/// This is the state every run sits in from its first tamed program
/// until its second, so for most players it is the *first* time these
/// screens are read with anything on the roster at all. Before R27 the
/// roster floor lived only in `Game::commit_for_build`: both menus lit,
/// the picker listed that program undimmed, and the refusal arrived
/// after the player had already confirmed spending it.
///
/// Driven through the real `draw_*` calls rather than the pure row
/// builders, `draw_build_program_only_lists_what_the_engine_will_accept`'s
/// reason: the row builders take the shortfall and the candidate list as
/// arguments, so every one of them is satisfied by a caller that asks
/// the wrong question. What is on trial here is the question.
///
/// And it asserts the *sentence*, not just the greying. Once
/// `programs_for_build` returns nothing, the old line —
/// `DEPLOY_NEEDS_A_PROGRAM`, "none of yours is free to spend" — would be
/// shown to a player looking at one perfectly free program, which is a
/// screen contradicting itself.
#[test]
fn a_one_program_base_greys_every_build_screen_and_says_which_rule_stopped_it() {
    let mut game = crate::render::test_support::game_with_a_single_program(4002);
    let m = crate::text::ui_metrics(900.0);

    assert!(
        floored_by_the_last_program(&mut game),
        "precondition: one owned program is the roster floor, not an empty roster"
    );

    // The deploy menu, opened on the Extractor's category: one line
    // for the screen, and every non-Home row dimmed.
    let (_, shapes) = crate::paint::with_painter(|p| {
        draw_build_menu(
            &mut game,
            Some(StructureCategory::Extractor),
            0,
            None,
            p,
            &m,
        );
    });
    let drawn = crate::paint::painted_text(&shapes);
    let says = |drawn: &[String], want: &str| drawn.iter().any(|t| t.contains(want));
    assert!(
        says(&drawn, DEPLOY_NEEDS_A_SECOND_PROGRAM),
        "the deploy menu has to name the rule that stopped it: {drawn:?}"
    );
    assert!(
        !says(&drawn, "None of yours is free to spend"),
        "and must not tell a player with one free program that none of theirs is free"
    );
    let dimmed = crate::paint::painted_runs_in(&shapes, TEXT_DIM, false);
    assert!(
        dimmed.iter().any(|t| t.contains("Extractor")),
        "an unaffordable structure row is greyed, not hidden: {dimmed:?}"
    );

    // The picker the greyed rows still lead to: nothing offered, and the
    // same rule named a third time.
    let (_, shapes) = crate::paint::with_painter(|p| {
        draw_build_program(&mut game, Some(deploy("Mining Node")), 0, None, p, &m);
    });
    let drawn = crate::paint::painted_text(&shapes);
    assert!(
        !says(&drawn, "Lone Sparkgrub"),
        "the picker must not offer the one program the engine will refuse: {drawn:?}"
    );
    assert!(
        says(&drawn, LAST_PROGRAM_CLAUSE),
        "and it says why it is empty: {drawn:?}"
    );
}
