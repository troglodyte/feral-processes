use super::*;

/// The gate lives in `Game::research_block`, so the flag is asserted
/// against a **live call** to it rather than hardcoded prose — the same
/// drift this repo keeps recording for `chain_break`.
#[test]
fn a_subject_gated_node_is_taken_on_and_flagged_without_a_pinned_subject() {
    let mut game = Game::new(4400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let def = game
        .world
        .resource::<ResearchDb>()
        .get("paging")
        .cloned()
        .expect("paging ships and is gated at zone 2 with no other prereqs");
    assert!(
        def.requires_subject,
        "the fixture is vacuous unless paging is actually gated"
    );
    let want = game
        .research_block(&def)
        .expect("nobody is pinned yet, so the gate must be live");

    game.discover_research("paging");
    game.select_research("paging")
        .expect("an unpinned subject flags the project rather than refusing it");

    assert_eq!(active_research(&game), Some("paging".to_string()));
    assert_eq!(
        game.research_readout(),
        Some(ResearchReadout::Blocked {
            name: def.name.clone(),
            reason: want,
        }),
        "the corner's flag is research_block's own sentence"
    );
}

/// **A reachability test, not only a refusal test.** A refusal can ship
/// permanent with every refusal test still green — pinning a program in the
/// pen must make the identical selection succeed.
#[test]
fn pinning_a_subject_clears_a_subject_gated_projects_flag() {
    let mut game = Game::new(4401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    game.discover_research("paging");
    game.select_research("paging").unwrap();
    assert!(matches!(
        game.research_readout(),
        Some(ResearchReadout::Blocked { .. })
    ));

    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);

    assert_eq!(active_research(&game), Some("paging".to_string()));
    assert!(
        matches!(
            game.research_readout(),
            Some(ResearchReadout::Earning { .. })
        ),
        "a subject standing in the pen must clear the flag: {:?}",
        game.research_readout()
    );
    assert_eq!(research_node(&game, "paging").blocked_by, None);
}

/// The strain mark's three terms, asserted one at a time: a subject-gated
/// project that is **earning** is the only state that stresses a body.
///
/// Read off the pin alone this would have been a second answer to "which body
/// is under study" — the fault `view_pinned_at`'s settle gate already closed
/// once, with the brackets latching at selection and riding the whole walk to
/// the pen. `Strained` is the same walk's answer, narrowed.
#[test]
fn an_earning_subject_gated_project_strains_its_subject() {
    let mut game = Game::new(4402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    let pen = game.study_pen(node).unwrap();
    let half = 4;
    let at_pen = |game: &Game| game.view_pinned_at(pen, half, half)[half as usize][half as usize];

    // Pinned, nothing selected: held, not worked.
    assert_eq!(at_pen(&game), PinMark::Settled);

    game.discover_research("paging");
    game.select_research("paging")
        .expect("a subject standing in the pen clears the gate");

    assert!(
        matches!(
            game.research_readout(),
            Some(ResearchReadout::Earning { .. })
        ),
        "the fixture is vacuous unless the project is actually earning"
    );
    assert_eq!(
        at_pen(&game),
        PinMark::Strained,
        "the body a subject-gated project is spending strains while it earns"
    );
}

/// The stall is `research_material_shortfall`'s, and the mark reads it through
/// `research_readout` rather than testing for it a second time — so a project
/// parked on an unpaid bill lets its subject go still.
#[test]
fn a_project_stalled_on_its_bill_lets_its_subject_settle() {
    let mut game = Game::new(4403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    let pen = game.study_pen(node).unwrap();
    let half = 4;
    let at_pen = |game: &Game| game.view_pinned_at(pen, half, half)[half as usize][half as usize];

    game.discover_research("paging");
    game.select_research("paging").unwrap();
    assert_eq!(at_pen(&game), PinMark::Strained);

    fill_research_progress(&mut game, "paging");

    assert!(
        matches!(
            game.research_readout(),
            Some(ResearchReadout::Stalled { .. })
        ),
        "precondition: nothing on the shelves pays paging's bill"
    );
    assert_eq!(
        at_pen(&game),
        PinMark::Settled,
        "a project waiting on a material line is not working its subject"
    );
}

/// `Blocked` strains only while data still flows: a project at full data
/// with a material line unbuilt is as still as a stalled one, whatever the
/// corner says it waits on.
#[test]
fn a_flagged_project_at_full_data_lets_its_subject_settle() {
    let mut game = Game::new(4408, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    let pen = game.study_pen(node).unwrap();
    let half = 4;
    let at_pen = |game: &Game| game.view_pinned_at(pen, half, half)[half as usize][half as usize];
    game.discover_research("paging");
    game.select_research("paging").unwrap();
    let materials = game
        .world
        .resource::<ResearchDb>()
        .get("paging")
        .unwrap()
        .materials
        .clone();
    for (item, _) in &materials {
        for machine in crate::game::base::work_orders::producers_of(&game, item) {
            game.world.despawn(machine);
        }
    }
    assert!(matches!(
        game.research_readout(),
        Some(ResearchReadout::Blocked { .. })
    ));
    assert_eq!(
        at_pen(&game),
        PinMark::Strained,
        "short of data, a flagged project is still working its subject"
    );

    fill_research_progress(&mut game, "paging");

    assert!(matches!(
        game.research_readout(),
        Some(ResearchReadout::Blocked { .. })
    ));
    assert_eq!(
        at_pen(&game),
        PinMark::Settled,
        "at full data nothing flows, so the body is still"
    );
}

/// A project that declares no `requires_subject` spends no body, however many
/// are pinned — the first of the three terms, and the one a mark derived from
/// "is research running" alone would miss.
#[test]
fn an_ungated_project_leaves_a_pinned_body_settled() {
    let mut game = Game::new(4404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    let pen = game.study_pen(node).unwrap();
    let half = 4;

    game.select_research("automation")
        .expect("automation is ungated");
    assert!(
        !game
            .world
            .resource::<ResearchDb>()
            .get("automation")
            .unwrap()
            .requires_subject,
        "the fixture is vacuous unless automation really spends no subject"
    );
    assert!(
        matches!(
            game.research_readout(),
            Some(ResearchReadout::Earning { .. })
        ),
        "and unless it is actually earning"
    );

    assert_eq!(
        game.view_pinned_at(pen, half, half)[half as usize][half as usize],
        PinMark::Settled,
    );
}

/// **Exactly one body strains, and it is the gate's own.** A second station
/// with its own settled subject keeps plain brackets: `Game::pinned_subject`
/// names the `(x, y)`-sorted first station's occupant, and that is the body
/// `Game::settle_research` will spend — so the mark cannot point at a
/// different program than the one being consumed.
#[test]
fn a_second_stations_subject_stays_settled_while_the_first_strains() {
    let mut game = Game::new(4405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let first = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    // Sorted **after** the first station's `(2, 2)`, so `study_station` keeps
    // naming the first one and the gate keeps naming its body — which is the
    // asymmetry under test. Its own floor is laid explicitly: the starting
    // pocket does not reach a second 2x2 machine, and `pin_subject` refuses a
    // pen with no floor under it.
    for x in 2..=5 {
        for y in -1..=1 {
            game.world.resource_mut::<BaseGrid>().lay_floor(x, y);
        }
    }
    let second = spawn_machine_at(&mut game, "research_node", 4, 0);
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 5);
    let first_pen = game.study_pen(first).unwrap();
    let second_pen = game.study_pen(second).unwrap();
    // The run's starting program wanders the base and may be standing on a
    // pen, which `pin_subject` refuses outright. Nothing about that refusal
    // is under test here.
    for (body, pos) in game.base_bodies() {
        if (pos.x, pos.y) == first_pen || (pos.x, pos.y) == second_pen {
            let mut p = game.world.get_mut::<Position>(body).unwrap();
            p.x = 0;
            p.y = 0;
        }
    }
    pin_subject_at_pen(&mut game, a, first);
    pin_subject_at_pen(&mut game, b, second);

    game.discover_research("paging");
    game.select_research("paging").unwrap();

    assert_eq!(
        game.pinned_subject(),
        Some(a),
        "the fixture is vacuous unless the gate names the first station's body"
    );
    // One window wide enough to hold both pens, so the two answers come out
    // of a single walk rather than two.
    let half = 24;
    let mark = |pen: (i32, i32)| {
        game.view_pinned_at(first_pen, half, half)[(pen.1 - first_pen.1 + half) as usize]
            [(pen.0 - first_pen.0 + half) as usize]
    };
    assert_eq!(mark(first_pen), PinMark::Strained);
    assert_eq!(mark(second_pen), PinMark::Settled);
}

/// The five bootstrap nodes — what gets a base running — stay selectable
/// with nobody pinned. Looped rather than named one at a time, so a mod or a
/// retune that grows the ungated set is covered for free.
#[test]
fn every_ungated_node_is_selectable_with_nobody_pinned() {
    let ungated: Vec<String> = {
        let game = Game::new(4402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world
            .resource::<ResearchDb>()
            .all()
            .filter(|d| d.tree == ResearchTree::Base && !d.requires_subject)
            .map(|d| d.id.clone())
            .collect()
    };
    assert!(
        !ungated.is_empty(),
        "the fixture is vacuous with nothing ungated"
    );
    for id in ungated {
        let mut game = Game::new(4403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        base_with_a_research_node(&mut game);
        research_prereqs_of(&mut game, &id);
        let zone = game
            .world
            .resource::<ResearchDb>()
            .get(&id)
            .expect("walked out of the same db")
            .min_zone;
        set_zone(&mut game, zone);

        game.select_research(&id)
            .unwrap_or_else(|e| panic!("{id} should be selectable with nobody pinned: {e}"));
    }
}

/// Every subject-gated node is refused without a subject, and the same pin
/// makes it reachable — the census that the 24/5 split actually behaves as
/// the refusal test above shows for one node.
#[test]
fn every_subject_gated_node_is_flagged_without_a_pinned_subject() {
    let gated: Vec<String> = {
        let game = Game::new(4404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world
            .resource::<ResearchDb>()
            .all()
            .filter(|d| d.requires_subject)
            .map(|d| d.id.clone())
            .collect()
    };
    assert!(
        !gated.is_empty(),
        "the fixture is vacuous with nothing gated"
    );
    for id in gated {
        let mut game = Game::new(4405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        discover_all_research(&mut game);
        base_with_a_research_node(&mut game);
        research_prereqs_of(&mut game, &id);
        let def = game
            .world
            .resource::<ResearchDb>()
            .get(&id)
            .cloned()
            .expect("walked out of the same db");
        set_zone(&mut game, def.min_zone);
        let want = game
            .research_block(&def)
            .unwrap_or_else(|| panic!("{id} should be blocked with nobody pinned"));

        game.select_research(&id)
            .unwrap_or_else(|e| panic!("{id} should be taken on with nobody pinned: {e}"));

        assert_eq!(
            game.research_readout(),
            Some(ResearchReadout::Blocked {
                name: def.name.clone(),
                reason: want,
            }),
            "{id}'s flag must be research_block's own sentence"
        );
    }
}

/// The third refusal on `Game::unpin_subject`: pulling the subject out from
/// under a project that needs it is refused, and abandoning the project is
/// how you change your mind — `select_research`'s "already active" refusal,
/// in shape.
#[test]
fn unpin_subject_is_refused_while_a_subject_gated_project_is_active() {
    let mut game = Game::new(4406, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    game.discover_research("paging");
    game.select_research("paging").unwrap();

    let err = game
        .unpin_subject(program)
        .expect_err("the active project still needs this subject");
    assert!(err.contains("abandon"), "unexpected error: {err}");
    assert!(
        game.world
            .get::<crate::components::UnderStudy>(program)
            .is_some(),
        "the refusal must not have unpinned it"
    );

    game.abandon_research().unwrap();
    game.unpin_subject(program)
        .expect("abandoning the project frees the subject to be unpinned");
}

// ---------------------------------------------------------------------
// C1 (final whole-branch review): `research_nodes` must apply the same
// subject term the readout flags on. Task 7's own tests compared
// `select_research`'s refusal (a flag since 2026-10-17) against a live
// `research_block` call — the same door twice — which is why a subject
// gate could reach `select_research` while `research_nodes` (the screen)
// still read `research_block_memo` directly and never asked the subject
// question at all. These tests drive `research_nodes` itself.
// ---------------------------------------------------------------------

/// The screen's own row must carry the same block reason the readout
/// flags with, and pinning a subject must clear it there too — not only
/// in the corner.
#[test]
fn research_nodes_flags_a_subject_gated_node_the_same_way_the_readout_does() {
    let mut game = Game::new(4407, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    discover_all_research(&mut game);

    let def = game
        .world
        .resource::<ResearchDb>()
        .get("paging")
        .cloned()
        .expect("paging ships and is gated at zone 2 with no other prereqs");
    assert!(
        def.requires_subject,
        "the fixture is vacuous unless paging is actually gated"
    );

    let want = game
        .research_block(&def)
        .expect("nobody is pinned yet, so the gate must be live");

    let row = research_node(&game, "paging");
    assert_eq!(
        row.blocked_by,
        Some(want),
        "research_nodes's row must carry the same block reason the readout flags with"
    );

    // Reachability: pinning a subject must clear the SCREEN's block line,
    // not only make `select_research` succeed.
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    let row = research_node(&game, "paging");
    assert_eq!(
        row.blocked_by, None,
        "pinning a subject must clear research_nodes's block line too"
    );
}

/// The ungated nodes must still show no block line from the screen
/// with nobody pinned — companion census to the test above, so a subject
/// term applied unconditionally would be caught here.
#[test]
fn research_nodes_reports_no_block_for_every_ungated_node_with_nobody_pinned() {
    let ungated: Vec<String> = {
        let game = Game::new(4410, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world
            .resource::<ResearchDb>()
            .all()
            .filter(|d| d.tree == ResearchTree::Base && !d.requires_subject)
            .map(|d| d.id.clone())
            .collect()
    };
    assert!(
        !ungated.is_empty(),
        "the fixture is vacuous with nothing ungated"
    );
    for id in ungated {
        let mut game = Game::new(4411, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        base_with_a_research_node(&mut game);
        research_prereqs_of(&mut game, &id);
        let zone = game
            .world
            .resource::<ResearchDb>()
            .get(&id)
            .expect("walked out of the same db")
            .min_zone;
        set_zone(&mut game, zone);
        let row = research_node(&game, &id);
        assert_eq!(
            row.blocked_by, None,
            "{id} should show no block on the screen with nobody pinned"
        );
    }
}

// ---------------------------------------------------------------------
// M1 (final whole-branch review): `Game::release_study_station` must not
// over-abandon, and must not silently swallow `abandon_research`'s refusal.
// ---------------------------------------------------------------------

/// Destroying a Station that happens to hold a subject must not deselect an
/// *unrelated, ungated* project — `release_study_station` used to call
/// `abandon_research` whenever the destroyed structure held any subject at
/// all, regardless of whether the active project needed one.
#[test]
fn releasing_a_stations_subject_does_not_abandon_an_ungated_project() {
    let mut game = Game::new(4700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    game.select_research("automation")
        .expect("automation is ungated and should be selectable immediately");
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    assert_eq!(
        active_research(&game),
        Some("automation".to_string()),
        "precondition"
    );

    game.release_study_station(node);

    assert_eq!(
        active_research(&game),
        Some("automation".to_string()),
        "an ungated project must survive the destruction of a station that merely \
         happened to be holding an unrelated pinned subject"
    );
}

/// `abandon_research` refuses during an active battle — its own first
/// rung — and `release_study_station` used to discard that refusal with a
/// bare `let _`, leaving a subject-gated project active with no subject and
/// no word to the player anywhere in the log.
#[test]
fn releasing_a_subjects_station_during_a_battle_does_not_swallow_the_refusal() {
    let mut game = Game::new(4701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    game.discover_research("paging");
    game.select_research("paging")
        .expect("the pinned subject clears the gate");

    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .expect("at least one species");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 0,
                mitigation: 1,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    assert!(game.has_active_battle(), "precondition");

    let before = game.message_log(50).len();
    game.release_study_station(node);

    assert_eq!(
        active_research(&game),
        Some("paging".to_string()),
        "abandon_research must have refused during the battle, leaving the project active"
    );
    let after = game.message_log(50);
    // The pen-release line alone always fires — `release_study_station`'s
    // first, unconditional log — so the refusal must add a *second* line on
    // top of it, not merely leave that one line standing.
    assert_eq!(
        after.len(),
        before + 2,
        "the refusal must not be silently discarded — a second line must say so, \
         beside the pen-release line: {after:?}"
    );
}

// ---------------------------------------------------------------------
// Completion spends the subject (Task 8)
// ---------------------------------------------------------------------

/// A minimal filler row for `components::DownedPrograms`, used only to
/// saturate the store — nothing about its content matters to the tests
/// that spawn it.
fn filler_downed_program() -> DownedProgram {
    DownedProgram {
        species: "test_generic".to_string(),
        level: 1,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 50,
        carried: None,
    }
}

/// Decision 5's short-circuit, the subject half: full progress and a full
/// bill do not complete a subject-gated project once its subject is gone —
/// `a_full_bill_alone_does_not_complete_a_project`'s failure, with a worse
/// loss, since a program is not refundable the way a shelf material is.
///
/// The subject leaves by a direct removal rather than through
/// `unpin_subject`, which now refuses this exact case — the door under test
/// here is `settle_research`'s own gate.
#[test]
fn a_subject_gated_project_does_not_complete_without_a_pinned_subject() {
    let mut game = Game::new(4500, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    game.discover_research("paging");
    game.select_research("paging").unwrap();
    game.world
        .entity_mut(program)
        .remove::<crate::components::UnderStudy>();
    let shelf = shelve_research_bill(&mut game, "paging", 8, 8);
    let stocked = node_output(&game, shelf, ids::BYTECODE_BLOCK);
    fill_research_progress(&mut game, "paging");

    game.tick();

    assert!(!game.is_researched("paging"));
    assert_eq!(
        node_output(&game, shelf, ids::BYTECODE_BLOCK),
        stocked,
        "a project missing its subject must not spend its materials either"
    );
}

/// M2 (final whole-branch review): once progress is full and the bill is
/// paid, a subject-gated project with no pinned subject is exactly as
/// stalled as one short a material or with nowhere to put the program —
/// `settle_research`'s own ordering already treats it that way — but
/// `research_material_shortfall` never said so, so the HUD read "Earning
/// n/cost" forever with no stall line once the subject was gone. The
/// subject leaves by direct removal, matching
/// `a_subject_gated_project_does_not_complete_without_a_pinned_subject`
/// above — reachable in play via M1's battle case, a second Station
/// resolving first, or the subject simply being walked off the pen.
#[test]
fn a_subject_gated_project_with_no_pinned_subject_reads_as_blocked() {
    let mut game = Game::new(4502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    game.discover_research("paging");
    game.select_research("paging").unwrap();
    game.world
        .entity_mut(program)
        .remove::<crate::components::UnderStudy>();
    // The bill is fully paid, so a material line cannot be the reason this
    // reads as a stall — isolating the subject as the only thing missing.
    shelve_research_bill(&mut game, "paging", 8, 8);
    fill_research_progress(&mut game, "paging");
    assert_eq!(
        game.pinned_subject(),
        None,
        "precondition: nobody is in the pen"
    );

    let shortfall = game.research_material_shortfall().expect(
        "a full bill and full progress with no pinned subject is exactly as stalled \
         as one short a material",
    );
    assert!(!shortfall.is_empty(), "the stall must name something");

    // The corner reads the subject gate as the research screen's own flag,
    // ahead of the stall: `research_block`'s sentence says what to do
    // where the shortfall only names what is short.
    let def = game
        .world
        .resource::<ResearchDb>()
        .get("paging")
        .cloned()
        .unwrap();
    let want = game
        .research_block(&def)
        .expect("nobody pinned, so the block is live");
    match game.research_readout() {
        Some(ResearchReadout::Blocked { reason, .. }) => {
            assert_eq!(
                reason, want,
                "the readout must report the same block the row is flagged with"
            );
        }
        other => panic!("expected a Blocked readout, got {other:?}"),
    }
}

/// The conversion: `downed_program_for_with_overkill(subject, 0.0)` →
/// `hold_at_station` → despawn. The pack is never involved: it is full here
/// and the completion goes through anyway. The level comes off the subject's real
/// `Experience`, not `ZoneLevel` — `ability_user_level`'s distinction from a
/// wild kill — and a routine installed on the subject comes back as
/// `carried`, since nothing about the test species' (declared-nothing) kit
/// claims it.
#[test]
fn completing_a_subject_gated_project_spends_the_pinned_subject() {
    let mut game = Game::new(4501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    game.world.get_mut::<Experience>(program).unwrap().level = 4;
    let species = game.world.get::<Creature>(program).unwrap().species.clone();
    spawn_machine_at(&mut game, "quarantine_rack", 6, 2);
    {
        let player = game.player_entity();
        game.world.get_mut::<DownedPrograms>(player).unwrap().0 = (0
            ..crate::tuning::MAX_DOWNED_PROGRAMS)
            .map(|_| filler_downed_program())
            .collect();
    }
    let prize = game
        .world
        .resource::<AbilityDb>()
        .wild_pool()
        .into_iter()
        .map(|(def, _)| def.id.clone())
        .next()
        .expect("some shipped ability is wild-poolable");
    game.world.get_mut::<Routines>(program).unwrap().0 = vec![prize.clone()];
    pin_subject_at_pen(&mut game, program, node);
    game.discover_research("paging");
    game.select_research("paging").unwrap();
    shelve_research_bill(&mut game, "paging", 8, 8);
    fill_research_progress(&mut game, "paging");

    game.tick();

    assert!(game.is_researched("paging"));
    assert!(
        game.world.get_entity(program).is_err(),
        "the subject leaves the world when it is spent"
    );
    assert_eq!(
        game.world
            .get::<DownedPrograms>(game.player_entity())
            .unwrap()
            .0
            .len(),
        crate::tuning::MAX_DOWNED_PROGRAMS,
        "research never touches the pack"
    );
    let held = game
        .world
        .get::<crate::components::StudyYield>(node)
        .and_then(|y| y.0.clone())
        .expect("the program is held at the station whose pen held the subject");
    assert_eq!(held.species, species);
    assert_eq!(
        held.level, 4,
        "level must come from the subject's real Experience, not ZoneLevel"
    );
    assert_eq!(
        held.carried,
        Some(prize),
        "a routine installed on the subject comes back as carried"
    );
    assert!(
        game.world.get::<Racked>(node).is_none(),
        "the station must never read as a rack"
    );
}

/// A subject spent in study is gone for good, and so are the routines it
/// built (ADR 0029). The downed record it becomes carries no built routine
/// either: the id would name nothing.
#[test]
fn completing_a_subject_gated_project_destroys_the_subjects_built_routines() {
    let mut game = Game::new(4503, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    spawn_machine_at(&mut game, "quarantine_rack", 6, 2);
    let built = "crafted/7".to_string();
    game.world
        .resource_mut::<crate::resources::CraftedRoutines>()
        .built
        .push(crate::routine_craft::CraftedRoutine {
            id: built.clone(),
            name: "Mender".into(),
            parts: vec![crate::routine_craft::Subroutine::Core(
                crate::routine_craft::Core::Heal,
                crate::routine_craft::Version::V1,
            )],
        });
    game.sync_crafted();
    game.world.get_mut::<Routines>(program).unwrap().0 = vec![built.clone()];
    pin_subject_at_pen(&mut game, program, node);
    game.discover_research("paging");
    game.select_research("paging").unwrap();
    shelve_research_bill(&mut game, "paging", 8, 8);
    fill_research_progress(&mut game, "paging");

    game.tick();

    assert!(game.is_researched("paging"));
    assert!(
        game.world.get_entity(program).is_err(),
        "the subject is spent"
    );
    assert!(
        game.world
            .resource::<crate::resources::CraftedRoutines>()
            .built
            .iter()
            .all(|r| r.id != built),
        "its built routine went with it"
    );
    assert!(game.world.resource::<AbilityDb>().get(&built).is_none());
    let held = game
        .world
        .get::<crate::components::StudyYield>(node)
        .and_then(|y| y.0.clone())
        .expect("the program is held at the station");
    assert_eq!(
        held.carried, None,
        "a built routine is never a carried prize"
    );
}

/// With no rack at all a subject-gated completion still goes through: the
/// spent program waits at the station and is set down beside it.
#[test]
fn no_rack_does_not_block_a_subject_gated_completion() {
    let mut game = Game::new(4502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    set_zone(&mut game, 2);
    let program = spawn_tamed(&mut game, 10, 3);
    pin_subject_at_pen(&mut game, program, node);
    game.discover_research("paging");
    game.select_research("paging").unwrap();
    shelve_research_bill(&mut game, "paging", 8, 8);
    fill_research_progress(&mut game, "paging");
    assert_eq!(game.research_material_shortfall(), None);

    game.tick();

    assert!(game.is_researched("paging"));
    assert!(
        game.world.get_entity(program).is_err(),
        "the subject is spent"
    );
}
