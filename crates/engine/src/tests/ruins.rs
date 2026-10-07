//! Wrecks: what records one, when it is filed as a rebuild, what a rebuild
//! that needs a program waits for, and that all of it survives a save.

use super::support::*;
use crate::resources::{Ruin, Ruins};
use crate::*;

fn base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    stand_in_base(&mut game);
    game
}

/// A structure of `kind` one cell east of the party, with the `Durability`
/// a siege or a sweep needs to hurt it.
fn raidable(game: &mut Game, kind: &str) -> (Entity, i32, i32) {
    let (px, py) = game.base_pos().unwrap();
    let e = spawn_machine_at(game, kind, px + 1, py);
    game.world
        .entity_mut(e)
        .insert(Durability { hp: 1, max_hp: 1 });
    (e, px + 1, py)
}

fn ruin(kind: &str, x: i32, y: i32) -> Ruin {
    Ruin {
        kind: kind.to_string(),
        x,
        y,
    }
}

fn sites(game: &mut Game) -> Vec<(Entity, bool)> {
    let mut q = game.world.query::<(Entity, &BuildSite)>();
    q.iter(&game.world)
        .map(|(e, b)| (e, b.awaiting_program))
        .collect()
}

/// A fight seated as a siege, so `siege_running` reads true.
fn start_fake_siege(game: &mut Game) {
    let spec = crate::tactical::map::BattleSpec {
        world_seed: 1,
        site: (0, 0),
        tick: 0,
        zone: 1,
        biome: Biome::OpenGrid,
        bodies: 1,
    };
    let mut battle =
        crate::tactical::TacticalBattle::open(spec, crate::tactical::map::Board::solid(4));
    battle.siege_pack = 1;
    game.world.insert_resource(battle);
}

#[test]
fn a_destroyed_structure_records_a_ruin_and_a_demolish_does_not() {
    let mut game = base(2201);
    let (shield, x, y) = raidable(&mut game, "shield");
    game.damage_structure(shield, 10, "Shield", "a siege");
    assert_eq!(
        game.world.resource::<Ruins>().0,
        vec![ruin("shield", x, y)],
        "destruction leaves a wreck where it stood"
    );

    let mut game = base(2202);
    let (shield, ..) = raidable(&mut game, "shield");
    game.remove_structure(shield).unwrap();
    assert!(
        game.world.resource::<Ruins>().0.is_empty(),
        "a demolish is the player's choice, not a loss"
    );
}

#[test]
fn ruins_file_nothing_while_a_siege_runs_and_file_once_it_ends() {
    let mut game = base(2203);
    let (px, py) = game.base_pos().unwrap();
    game.world
        .resource_mut::<Ruins>()
        .0
        .push(ruin("shield", px + 1, py));
    start_fake_siege(&mut game);

    game.file_ruins();
    assert!(sites(&mut game).is_empty(), "nothing is filed mid-siege");
    assert_eq!(
        game.world.resource::<Ruins>().0.len(),
        1,
        "and the wreck waits"
    );

    game.world
        .remove_resource::<crate::tactical::TacticalBattle>();
    game.file_ruins();
    assert_eq!(sites(&mut game).len(), 1, "it files once the siege is over");
    assert!(game.world.resource::<Ruins>().0.is_empty());
}

#[test]
fn a_raids_ruin_files_on_the_next_tick() {
    let mut game = base(2204);
    let (shield, ..) = raidable(&mut game, "shield");
    game.damage_structure(shield, 10, "Shield", "a GC Entropy Sweep");
    assert!(sites(&mut game).is_empty(), "not filed in the blow itself");
    game.tick();
    assert_eq!(sites(&mut game).len(), 1, "a raid is not a siege");
}

#[test]
fn a_blocked_tile_drops_the_ruin_and_says_so() {
    let mut game = base(2205);
    let (_, x, y) = raidable(&mut game, "shield");
    game.world
        .resource_mut::<Ruins>()
        .0
        .push(ruin("shield", x, y));
    game.file_ruins();
    assert!(sites(&mut game).is_empty(), "the ground is taken");
    assert!(
        game.world.resource::<Ruins>().0.is_empty(),
        "the ruin is dropped"
    );
    assert!(
        game.message_history(50)
            .iter()
            .any(|m| m.text.contains("could not be rebuilt")),
        "and the log says why"
    );
}

#[test]
fn a_rebuild_with_no_job_is_raised_by_the_crew_at_full_cost() {
    let mut game = base(2206);
    spawn_tamed(&mut game, 500, 3);
    let (px, py) = game.base_pos().unwrap();
    game.world
        .resource_mut::<Ruins>()
        .0
        .push(ruin("shield", px + 1, py));
    game.file_ruins();
    let (site, awaiting) = sites(&mut game)[0];
    assert!(!awaiting, "a shield needs no program");
    let cost = game.world.get::<BuildSite>(site).unwrap().cost.clone();
    let fresh = {
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "shield")
            .unwrap();
        game.structure_build_cost(&def)
    };
    assert_eq!(cost, fresh, "full fresh-deploy cost");
    let held = count_item(&game, ids::CORE_FRAGMENT);
    for _ in 0..400 {
        if game.world.get::<BuildSite>(site).is_none() {
            break;
        }
        game.tick();
    }
    assert!(
        game.world.get::<BuildSite>(site).is_none(),
        "the crew finished it"
    );
    let paid: u32 = cost.iter().map(|(_, q)| q).sum();
    assert!(paid > 0 && count_item(&game, ids::CORE_FRAGMENT) < held);
    let standing = {
        let mut q = game.world.query::<(&Structure, &Position)>();
        q.iter(&game.world)
            .find(|(s, p)| s.kind == "shield" && (p.x, p.y) == (px + 1, py))
            .map(|_| ())
    };
    assert!(
        standing.is_some(),
        "a shield stands on the wreck's tile again"
    );
}

#[test]
fn an_awaiting_site_is_never_worked_or_announced_dry_until_a_program_is_committed() {
    let mut game = base(2207);
    spawn_tamed(&mut game, 500, 3);
    let (px, py) = game.base_pos().unwrap();
    game.world
        .resource_mut::<Ruins>()
        .0
        .push(ruin("mining_node", px + 1, py));
    game.file_ruins();
    let (site, awaiting) = sites(&mut game)[0];
    assert!(awaiting, "a node runs a job, so it waits for a program");
    assert!(!game.build_is_workable(site), "not workable while it waits");

    // Empty the pack so a workable site would be announced dry.
    let held = count_item(&game, ids::CORE_FRAGMENT);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .take(ItemId::from(ids::CORE_FRAGMENT), held);
    for _ in 0..60 {
        game.tick();
    }
    let b = game.world.get::<BuildSite>(site).unwrap();
    assert!(!b.announced_dry, "waiting on a program is not being dry");
    assert_eq!(b.progress, 0);
    assert!(b.delivered.is_empty(), "and nobody carried anything to it");
    assert_eq!(game.awaiting_program_sites().len(), 1);

    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    let program = tame_at_zone(&mut game, 1);
    let spare = tame_at_zone(&mut game, 1);
    game.world.resource_mut::<Party>().0.push(spare);
    game.commit_rebuild_program(site, program).unwrap();
    let b = game.world.get::<BuildSite>(site).unwrap();
    assert!(!b.awaiting_program && b.program.is_some());
    assert!(game.awaiting_program_sites().is_empty());
    assert!(game.build_is_workable(site), "and now it is worked");
    assert!(
        game.commit_rebuild_program(site, spare).is_err(),
        "a second commit has nothing to answer"
    );
}

#[test]
fn ruins_and_an_awaiting_site_survive_a_real_save_and_load() {
    let mut game = base(2208);
    spawn_tamed(&mut game, 500, 3);
    let (px, py) = game.base_pos().unwrap();
    game.world
        .resource_mut::<Ruins>()
        .0
        .push(ruin("mining_node", px + 1, py));
    game.file_ruins();
    game.world
        .resource_mut::<Ruins>()
        .0
        .push(ruin("shield", px + 2, py));
    assert_eq!(game.awaiting_program_sites().len(), 1);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_ruins_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.world.resource::<Ruins>().0,
        vec![ruin("shield", px + 2, py)],
        "the unfiled wreck came back"
    );
    let awaiting = loaded.awaiting_program_sites();
    assert_eq!(
        awaiting.len(),
        1,
        "and so did the site waiting for a program"
    );
    assert_eq!(
        (awaiting[0].structure.as_str(), awaiting[0].x, awaiting[0].y),
        ("mining_node", px + 1, py)
    );
}

/// `adjacent_interactions`: what `[c]` can do beside the party.
mod adjacent {
    use super::*;

    fn file_node_east(game: &mut Game) -> Entity {
        let (px, py) = game.base_pos().unwrap();
        game.world
            .resource_mut::<Ruins>()
            .0
            .push(ruin("mining_node", px + 1, py));
        game.file_ruins();
        sites(game)[0].0
    }

    #[test]
    fn nothing_beside_the_party_offers_nothing() {
        let game = base(2301);
        assert!(game.adjacent_interactions().is_empty());
    }

    #[test]
    fn an_awaiting_site_beside_the_party_offers_its_rebuild() {
        let mut game = base(2302);
        let site = file_node_east(&mut game);
        assert_eq!(
            game.adjacent_interactions(),
            vec![Interaction {
                dir: (1, 0),
                kind: InteractionKind::RebuildProgram(site)
            }]
        );
    }

    #[test]
    fn a_site_two_tiles_away_or_already_committed_offers_nothing() {
        let mut game = base(2303);
        let (px, py) = game.base_pos().unwrap();
        game.world
            .resource_mut::<Ruins>()
            .0
            .push(ruin("mining_node", px + 3, py));
        game.file_ruins();
        assert!(game.adjacent_interactions().is_empty());

        let mut game = base(2304);
        let site = file_node_east(&mut game);
        spawn_tamed(&mut game, 500, 3);
        let program = tame_at_zone(&mut game, 1);
        let spare = tame_at_zone(&mut game, 1);
        game.world.resource_mut::<Party>().0.push(spare);
        game.commit_rebuild_program(site, program).unwrap();
        assert!(game.adjacent_interactions().is_empty());
    }

    #[test]
    fn a_depot_beside_the_party_offers_a_transfer_in_its_direction() {
        let mut game = base(2305);
        let (px, py) = game.base_pos().unwrap();
        spawn_machine_at(&mut game, "depot", px - 1, py);
        assert_eq!(
            game.adjacent_interactions(),
            vec![Interaction {
                dir: (-1, 0),
                kind: InteractionKind::Transfer
            }]
        );
    }

    #[test]
    fn a_depot_and_a_site_offer_both() {
        let mut game = base(2306);
        let (px, py) = game.base_pos().unwrap();
        spawn_machine_at(&mut game, "depot", px - 1, py);
        let site = file_node_east(&mut game);
        let found = game.adjacent_interactions();
        assert_eq!(found.len(), 2);
        assert!(found.contains(&Interaction {
            dir: (1, 0),
            kind: InteractionKind::RebuildProgram(site)
        }));
        assert!(found.iter().any(|i| i.kind == InteractionKind::Transfer));
    }
}

#[test]
fn an_awaiting_sites_examine_line_says_it_needs_a_program() {
    let mut game = base(2310);
    let (px, py) = game.base_pos().unwrap();
    game.world
        .resource_mut::<Ruins>()
        .0
        .push(ruin("mining_node", px + 1, py));
    game.file_ruins();
    let (site, _) = sites(&mut game)[0];
    let blurb = game.build_site_blurb(site).unwrap();
    assert!(blurb.contains("waiting for a program"), "{blurb}");
    assert!(game.build_order_report()[0].awaiting_program);
}
