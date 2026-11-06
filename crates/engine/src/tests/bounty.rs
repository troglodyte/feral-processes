//! Bounty contracts on the map: the board, the accept, the quarry the
//! contract owns, the compass row that follows it, and the save that must
//! bring it back (ADR 0044).

use super::support::*;
use crate::components::{BountyMark, Hostile};
use crate::contracts::{ContractDef, ContractId, Objective};
use crate::resources::{ActiveContracts, CompassBearing};
use crate::settlements::CompassTarget;
use crate::*;

fn deploy(game: &mut Game, kind: &str, x: i32, y: i32) {
    game.world.spawn((
        Structure {
            kind: kind.to_string(),
        },
        Position { x, y },
        Glyph {
            ch: '!',
            color: GlyphColor::Yellow,
        },
    ));
}

fn broker_game(seed: u32, zone: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    skip_tutorial(&mut game);
    if zone > 1 {
        game.warp_to_zone(zone).unwrap();
    }
    game.lay_test_pocket();
    deploy(&mut game, "home", 0, 0);
    deploy(&mut game, "contract_broker", 1, 0);
    stand_in_base_at(&mut game, 1, 1);
    game
}

fn board_bounty(game: &mut Game) -> Option<ContractDef> {
    game.broker_board_defs_for_test()?
        .into_iter()
        .find(|d| matches!(d.objective, Objective::Bounty(_)))
}

/// The first seed whose zone-`zone` board carries a bounty. The odds are a
/// coin flip per epoch, so this is a search over the world seed rather than
/// a fixture pinned to one.
fn game_with_a_bounty_on_offer(zone: u32) -> (Game, ContractDef) {
    for seed in 1..200 {
        let mut game = broker_game(seed, zone);
        if let Some(def) = board_bounty(&mut game) {
            return (game, def);
        }
    }
    panic!("no seed in 1..200 offers a bounty at zone {zone}");
}

fn quarries(game: &mut Game) -> Vec<(Entity, BountyMark, Position)> {
    let mut q = game.world.query::<(Entity, &BountyMark, &Position)>();
    let mut found: Vec<_> = q
        .iter(&game.world)
        .map(|(e, m, p)| (e, m.clone(), *p))
        .collect();
    found.sort_by_key(|(e, ..)| *e);
    found
}

fn accepted() -> (Game, ContractId) {
    let (mut game, def) = game_with_a_bounty_on_offer(3);
    game.accept_contract(&def.id, None).unwrap();
    (game, def.id)
}

fn anchor_of(game: &Game) -> (i32, i32) {
    game.held_bounty().unwrap().1.anchor
}

fn save_and_load(game: &mut Game, tag: &str) -> Game {
    let path = std::env::temp_dir().join(format!("feral_bounty_{tag}_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    loaded
}

#[test]
fn zone_one_never_offers_a_bounty() {
    for seed in 1..40 {
        let mut game = broker_game(seed, 1);
        assert!(board_bounty(&mut game).is_none(), "seed {seed}");
    }
}

#[test]
fn a_sector_two_or_deeper_board_can_offer_a_bounty_and_reads_the_same_every_time() {
    let (mut game, def) = game_with_a_bounty_on_offer(3);
    assert_eq!(board_bounty(&mut game).unwrap().id, def.id);
    assert_eq!(board_bounty(&mut game).unwrap().id, def.id);
}

#[test]
fn the_board_is_the_same_bounty_wherever_the_party_stands() {
    let (mut game, def) = game_with_a_bounty_on_offer(3);
    stand_in_base_at(&mut game, 5, 5);
    assert_eq!(board_bounty(&mut game).unwrap().id, def.id);
}

#[test]
fn the_board_is_stable_across_save_and_load() {
    let (mut game, def) = game_with_a_bounty_on_offer(3);
    let mut loaded = save_and_load(&mut game, "board");
    assert_eq!(board_bounty(&mut loaded).map(|d| d.id), Some(def.id));
}

#[test]
fn accepting_spawns_exactly_one_marked_quarry_at_the_anchor() {
    let (mut game, id) = accepted();
    let found = quarries(&mut game);
    assert_eq!(found.len(), 1);
    let (_, mark, pos) = &found[0];
    assert_eq!(mark.contract, id);
    assert_eq!((pos.x, pos.y), anchor_of(&game));
    assert!(game.world.get::<Hostile>(found[0].0).is_some());
}

#[test]
fn the_quarry_carries_the_contracts_grade_and_rarity() {
    let (mut game, _) = accepted();
    let quarry = game.held_bounty().unwrap().1;
    let (entity, ..) = quarries(&mut game).remove(0);
    assert_eq!(*game.world.get::<Rarity>(entity).unwrap(), quarry.rarity);
    assert!(game.world.get::<Boss>(entity).is_some());
    // A plain boss of the same species, raised to the same zone, is the baseline.
    let plain = game
        .spawn_wild_creature_scaled(&quarry.species, 0, 0, 1.0, true)
        .unwrap();
    game.world.entity_mut(plain).insert(quarry.rarity);
    let graded = game.world.get::<Stats>(entity).unwrap().max_hp as f32;
    let base = game.world.get::<Stats>(plain).unwrap().max_hp as f32;
    // Potential rolls differ per body, so only the order of magnitude is stable.
    assert!(
        graded > base * 0.8 * quarry.grade() / 1.3,
        "{graded} vs {base} x {}",
        quarry.grade()
    );
}

#[test]
fn the_compass_lists_the_quarry_at_its_live_position() {
    let (mut game, id) = accepted();
    let (entity, ..) = quarries(&mut game).remove(0);
    let row = |game: &mut Game| {
        game.compass_targets()
            .into_iter()
            .find(|r| r.target == CompassTarget::Bounty(id.clone()))
            .expect("the compass lists the held bounty")
    };
    let first = row(&mut game);
    assert!(first.label.starts_with("Bounty: "), "{}", first.label);
    let here = *game.world.get::<Position>(game.player_entity()).unwrap();
    let q = *game.world.get::<Position>(entity).unwrap();
    assert_eq!(
        first.distance,
        (q.x - here.x).abs().max((q.y - here.y).abs())
    );
    // Move the quarry: the same row follows it.
    let (dx, dy) = (q.x - here.x, q.y - here.y);
    let moved = if dx.abs() >= dy.abs() {
        Position {
            x: q.x + 3 * dx.signum(),
            y: q.y,
        }
    } else {
        Position {
            x: q.x,
            y: q.y + 3 * dy.signum(),
        }
    };
    *game.world.get_mut::<Position>(entity).unwrap() = moved;
    assert_eq!(row(&mut game).distance, first.distance + 3);
}

#[test]
fn the_quarry_stays_within_its_tether_over_many_ticks() {
    let (mut game, _) = accepted();
    let quarry = game.held_bounty().unwrap().1;
    let (entity, ..) = quarries(&mut game).remove(0);
    let mut moved = false;
    for _ in 0..600 {
        game.idle_tick();
        let p = *game.world.get::<Position>(entity).unwrap();
        let d = (p.x - quarry.anchor.0)
            .abs()
            .max((p.y - quarry.anchor.1).abs());
        assert!(d <= quarry.tether, "wandered {d} from the anchor");
        moved |= (p.x, p.y) != quarry.anchor;
    }
    assert!(moved, "a tether that never lets it move proves nothing");
}

fn assert_one_quarry_at_grade(game: &mut Game, id: &ContractId) {
    let found = quarries(game);
    assert_eq!(found.len(), 1, "exactly one quarry");
    assert_eq!(found[0].1.contract, *id);
}

#[test]
fn a_forced_cull_leaves_the_quarry_standing() {
    let (mut game, id) = accepted();
    let e = quarries(&mut game)[0].0;
    let before = game.world.get::<Stats>(e).unwrap().max_hp;
    game.cull_to_cap(usize::MAX / 2);
    game.ensure_bounty_quarry();
    assert_one_quarry_at_grade(&mut game, &id);
    let e = quarries(&mut game)[0].0;
    let after = game.world.get::<Stats>(e).unwrap().max_hp;
    assert_eq!(before, after, "spared, not respawned");
}

#[test]
fn clear_local_wild_leaves_the_quarry_standing() {
    let (mut game, id) = accepted();
    // The quarry is the thing under test, so stand next to it.
    let anchor = anchor_of(&game);
    stand_in_base_at(&mut game, anchor.0, anchor.1 + 1);
    game.clear_local_wild();
    assert_eq!(quarries(&mut game).len(), 1, "spared without a respawn");
    game.ensure_bounty_quarry();
    assert_one_quarry_at_grade(&mut game, &id);
}

#[test]
fn a_breach_leaves_one_quarry_at_its_original_grade() {
    let (mut game, id) = accepted();
    let grade = game.held_bounty().unwrap().1.grade_pct;
    game.enter_next_zone();
    assert_one_quarry_at_grade(&mut game, &id);
    assert_eq!(game.held_bounty().unwrap().1.grade_pct, grade);
}

#[test]
fn a_missing_quarry_is_respawned_and_a_second_is_removed() {
    let (mut game, id) = accepted();
    let (entity, ..) = quarries(&mut game).remove(0);
    game.world.despawn(entity);
    game.ensure_bounty_quarry();
    assert_one_quarry_at_grade(&mut game, &id);
    // A stray second body carrying the mark (a double respawn) is removed.
    let (_, mark, _) = quarries(&mut game).remove(0);
    let stray = game
        .spawn_wild_creature_scaled("drone", 0, 0, 1.0, true)
        .unwrap();
    game.world.entity_mut(stray).insert(mark);
    assert_eq!(quarries(&mut game).len(), 2);
    game.ensure_bounty_quarry();
    game.ensure_bounty_quarry();
    assert_one_quarry_at_grade(&mut game, &id);
}

#[test]
fn a_bounty_survives_the_real_save_and_load() {
    let (mut game, id) = accepted();
    let held_before = game.held_bounty().unwrap();
    game.set_compass_bearing(Some(CompassTarget::Bounty(id.clone())));
    let mut loaded = save_and_load(&mut game, "live");

    assert_eq!(loaded.held_bounty().unwrap(), held_before);
    let found = quarries(&mut loaded);
    assert_eq!(found.len(), 1, "the quarry is respawned on load");
    assert_eq!((found[0].2.x, found[0].2.y), held_before.1.anchor);
    let bearing = loaded.compass_bearing().expect("a bearing resolves");
    assert_eq!(bearing.target, CompassTarget::Bounty(id));
}

#[test]
fn a_failed_bounty_id_survives_save_and_load() {
    let (mut game, def) = game_with_a_bounty_on_offer(3);
    game.world
        .resource_mut::<ActiveContracts>()
        .failed
        .push(def.id.clone());
    assert!(board_bounty(&mut game).is_none(), "a failed id is hidden");
    let mut loaded = save_and_load(&mut game, "failed");
    assert!(
        loaded
            .world
            .resource::<ActiveContracts>()
            .failed
            .contains(&def.id)
    );
    assert!(board_bounty(&mut loaded).is_none());
}

#[test]
fn abandoning_removes_the_quarry_and_the_compass_falls_back_to_home() {
    let (mut game, id) = accepted();
    game.set_compass_bearing(Some(CompassTarget::Bounty(id.clone())));
    assert_eq!(
        game.compass_bearing().unwrap().target,
        CompassTarget::Bounty(id.clone())
    );
    assert!(game.abandon_contract(&id));
    assert!(quarries(&mut game).is_empty());
    assert!(
        game.compass_targets()
            .iter()
            .all(|r| !matches!(r.target, CompassTarget::Bounty(_)))
    );
    assert_eq!(game.world.resource::<CompassBearing>().0, None);
    assert_eq!(game.compass_bearing().unwrap().target, CompassTarget::Home);
}

#[test]
fn an_ended_contract_clears_a_selection_that_names_it() {
    let (mut game, id) = accepted();
    game.set_compass_bearing(Some(CompassTarget::Bounty(id.clone())));
    // Ended by some route other than abandon: the tick must notice.
    game.world.resource_mut::<ActiveContracts>().active.clear();
    game.idle_tick();
    assert_eq!(game.world.resource::<CompassBearing>().0, None);
    assert!(quarries(&mut game).is_empty());
}

#[test]
fn a_held_bounty_hides_any_second_one_on_the_board() {
    let (mut game, _) = accepted();
    assert!(board_bounty(&mut game).is_none());
}

#[test]
fn a_bounty_does_not_move_which_other_contracts_the_board_draws() {
    let (mut game, def) = game_with_a_bounty_on_offer(3);
    let others: Vec<ContractId> = game
        .broker_board_defs_for_test()
        .unwrap()
        .into_iter()
        .filter(|d| d.id != def.id)
        .map(|d| d.id)
        .collect();
    // Hide the bounty by marking it failed: the rest of the draw must not move.
    game.world
        .resource_mut::<ActiveContracts>()
        .failed
        .push(def.id.clone());
    let without: Vec<ContractId> = game
        .broker_board_defs_for_test()
        .unwrap()
        .into_iter()
        .map(|d| d.id)
        .collect();
    assert!(
        others.iter().all(|id| without.contains(id)),
        "{others:?} vs {without:?}"
    );
}
