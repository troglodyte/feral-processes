use super::*;

#[test]
fn the_data_cache_is_buildable_without_any_research() {
    let game = Game::new(710, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(
        game.buildable_structure_defs()
            .iter()
            .any(|d| d.id == "data_cache"),
        "buffer expansion must not be gated behind research the player \
         can't afford while the cap is at its tightest"
    );
}

#[test]
fn no_research_node_is_left_unlocking_nothing() {
    let game = Game::new(711, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for node in game.research_nodes(ResearchTree::Base) {
        let def = game
            .world
            .resource::<ResearchDb>()
            .get(&node.id)
            .expect("a listed node should exist in the db");
        assert!(
            !def.unlocks_structures.is_empty()
                || !def.unlocks_recipes.is_empty()
                || game.structure_defs().iter().any(|s| {
                    s.productions()
                        .any(|p| p.recipe.research.as_deref() == Some(node.id.as_str()))
                })
                || !def.unlocks_tools.is_empty()
                || def.decompiler.is_some(),
            "{} unlocks nothing and is dead weight in the tree",
            node.id
        );
    }
}

/// The cheapest node in a band, found by asking the db rather than named,
/// so retuning which node sits in which band leaves the tests below still
/// measuring what they mean to. `ResearchDb::all` is already cheapest-first.
fn cheapest_gated_node(game: &Game, zone: u32) -> ResearchDef {
    game.world
        .resource::<ResearchDb>()
        .all()
        // Base tree only: these tests are about the base tree's own zone
        // gating, and `ResearchDb::all()` now also carries one synthesised
        // routine node per eligible ability, many of them roots of their
        // own family with no prerequisite — exactly what
        // `a_node_can_report_both_a_missing_prereq_and_its_zone` needs to
        // *not* pick.
        .find(|d| d.min_zone == zone && d.tree == crate::research::ResearchTree::Base)
        .unwrap_or_else(|| panic!("the shipped base tree should band something at zone {zone}"))
        .clone()
}

/// Unlocks everything `id` transitively requires — and deliberately *not*
/// `id` itself, which is the node under test.
///
/// Not `support::unlock_research_chain` on `id` itself, which would leave
/// nothing to refuse; it is what lands each *prerequisite*. That helper also
/// raises `ZoneLevel` to clear the chain's own bands, which is the very thing
/// being tested, so it is called per prereq — every prereq of a gated node
/// sits at or below its band — rather than on the node under test.
///
/// Every prereq of a gated node sits in a band at or below its own — that is
/// `no_research_node_is_gated_below_its_own_prerequisite` — so at the zone
/// the caller is testing, all of them are buyable.
pub(super) fn research_prereqs_of(game: &mut Game, id: &str) {
    let requires = game
        .world
        .resource::<ResearchDb>()
        .get(id)
        .expect("a shipped node")
        .requires
        .clone();
    for prereq in requires {
        if game.is_researched(&prereq) {
            continue;
        }
        research_prereqs_of(game, &prereq);
        unlock_research_chain(game, &prereq);
    }
}

fn research_state(game: &Game, id: &str) -> ResearchState {
    game.research_nodes(ResearchTree::Base)
        .into_iter()
        .find(|n| n.id == id)
        .map(|n| n.state)
        .expect("a shipped node should be listed")
}

#[test]
fn a_node_above_the_players_zone_reports_its_zone() {
    let mut game = Game::new(715, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 2);
    discover_all_research(&mut game);
    match research_state(&game, &gated.id) {
        ResearchState::Locked { min_zone, .. } => assert_eq!(
            min_zone,
            Some(2),
            "{} is a zone-2 node and the party is in zone 1",
            gated.id
        ),
        other => panic!("expected {} to be Locked, got {other:?}", gated.id),
    }
}

/// A gated node is not filtered out of the menu, for the reason
/// `Game::upgrade_ceiling` records about a structure stalled at its zone
/// ceiling: hiding the stalled rows means a player who never breached never
/// learns the tier exists. The visible zone-3 band *is* the reason to breach.
#[test]
fn a_zone_gated_node_is_still_listed() {
    let mut game = Game::new(716, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 3);
    discover_all_research(&mut game);
    assert!(
        game.research_nodes(ResearchTree::Base)
            .iter()
            .any(|n| n.id == gated.id),
        "{} must stay on the menu at zone 1 — it is what tells the player \
         there is a reason to breach",
        gated.id
    );
}

#[test]
fn select_research_refuses_a_node_above_the_players_zone() {
    let mut game = Game::new(717, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 2);
    discover_all_research(&mut game);
    research_prereqs_of(&mut game, &gated.id);
    base_with_a_research_node(&mut game);

    let err = game.select_research(&gated.id).unwrap_err();

    assert!(err.contains("Zone 2"), "got: {err}");
    assert!(!game.is_researched(&gated.id));
    // The half that fails if the refusal is ever moved below the filing —
    // without it this passes against a build that files and then refuses.
    assert_eq!(active_research(&game), None);
    assert!(game.work_orders().is_empty(), "and files nothing");
}

#[test]
fn breaching_makes_a_zone_gated_node_available() {
    let mut game = Game::new(718, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 2);
    discover_all_research(&mut game);
    research_prereqs_of(&mut game, &gated.id);
    assert!(
        matches!(
            research_state(&game, &gated.id),
            ResearchState::Locked {
                min_zone: Some(2),
                ..
            }
        ),
        "the fixture is vacuous unless the node starts gated"
    );

    game.enter_next_zone();

    assert_eq!(
        research_state(&game, &gated.id),
        ResearchState::Available,
        "with its prereqs met, reaching zone 2 is the whole of what {} was waiting on",
        gated.id
    );
}

#[test]
fn a_node_can_report_both_a_missing_prereq_and_its_zone() {
    let mut game = Game::new(719, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let deep = cheapest_gated_node(&game, 3);
    discover_all_research(&mut game);
    match research_state(&game, &deep.id) {
        ResearchState::Locked { missing, min_zone } => {
            assert!(
                !missing.is_empty(),
                "{} sits on an unresearched chain, so it owes a prereq too",
                deep.id
            );
            assert_eq!(
                min_zone,
                Some(3),
                "and the zone is a second, separate reason"
            );
        }
        other => panic!("expected {} to be Locked, got {other:?}", deep.id),
    }
}

/// `upgrade_structure` checks its ceilings before the materials check "so the
/// player is never sent to find fragments they couldn't have spent". Same
/// argument: a broke player at zone 1 must hear about the zone.
#[test]
fn the_zone_gate_is_refused_before_the_machines() {
    let mut game = Game::new(720, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let gated = cheapest_gated_node(&game, 2);
    discover_all_research(&mut game);
    research_prereqs_of(&mut game, &gated.id);
    // Deliberately no Research Node: the zone must still be the reason, or a
    // player at zone 1 is sent to build machinery for a node they could not
    // have taken.
    stand_in_base(&mut game);

    let err = game.select_research(&gated.id).unwrap_err();

    assert!(err.contains("Zone 2"), "got: {err}");
    assert!(
        !err.contains("Research Station"),
        "the zone is the reason, not the base's plant: {err}"
    );
}

/// A node gated below its own prerequisite is a gate that can never fire:
/// the prereq lock outlives it, so the zone is never the reason the node is
/// unbuyable, and the menu shows a reason that disappears without the node
/// becoming available. Catches a band edit that makes a gate unreachable.
#[test]
fn no_research_node_is_gated_below_its_own_prerequisite() {
    let game = Game::new(713, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<ResearchDb>();
    for def in db.all() {
        for prereq_id in &def.requires {
            let prereq = db
                .get(prereq_id)
                .expect("a dangling prereq is dropped at load, so this must resolve");
            assert!(
                prereq.min_zone <= def.min_zone,
                "{} is zone {} but requires {}, which is zone {} — the gate could never fire",
                def.id,
                def.min_zone,
                prereq.id,
                prereq.min_zone
            );
        }
    }
}

/// The one way this feature could softlock a run: gate a node that unlocks
/// the structure the player needs in order to *reach* the zone that ungates
/// it. Researching the Zone Portal is fine and a mod is free to do it —
/// gating it is what breaks.
///
/// Vacuously true today, since no shipped node names the portal at all, and
/// that is exactly the point: the property is currently safe by accident,
/// and one content edit could remove it silently.
///
/// Asserted against the loaded `ResearchDb` rather than by reading the
/// files, so a node dropped at load time cannot make it pass for the wrong
/// reason.
#[test]
fn nothing_needed_to_breach_is_locked_behind_research() {
    let game = Game::new(714, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.world.resource::<ResearchDb>().all() {
        if def.unlocks_structures.iter().any(|s| s == "portal") {
            assert_eq!(
                def.min_zone, 0,
                "{} gates the Zone Portal behind zone {} — the portal is how you \
                 reach that zone, so the run cannot get there",
                def.id, def.min_zone
            );
        }
    }
}

#[test]
fn the_research_node_is_a_cronjob_worked_research_data_source() {
    let game = Game::new(60, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "research_node")
        .expect("research_node.ron should load");
    let work = def.work.expect("the Research Node must be workable");
    assert_eq!(work.produces, ItemId::from(ids::RESEARCH_DATA));
}

// --- Material bills ------------------------------------------------------
//
// `ResearchDef::materials` — the goods a node consumes beside its Research
// Data. These use a scratch node rather than a shipped one so what is being
// asserted is the *mechanism*, and a retune of the shipped bills cannot make
// them fail for a reason that isn't a bug.
