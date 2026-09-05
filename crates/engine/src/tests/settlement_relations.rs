//! Standing with a town — Phase 4's one door and its movers.
//!
//! Every test here goes through `Game::adjust_standing` or a mover that
//! calls it. Nothing writes `resources::Standings` by hand except the
//! fixtures that need a town already at a band, which is exactly the shape
//! the "one door" rule is meant to leave: setting the state is a test
//! convenience, moving it is the feature.

use super::support::*;
use crate::components::Durability;
use crate::items::{GearCopy, ids};
use crate::resources::Standings;
use crate::settlements::SettlementKey;
use crate::settlements::relations::Standing;
use crate::tuning::*;
use crate::*;

fn game() -> Game {
    Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn settlement_east_of_player(game: &mut Game) -> SettlementKey {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let key = SettlementKey { rx: 0, ry: 0 };
    place_settlement(game, key, pos.x + 1, pos.y);
    key
}

/// Puts a town at `standing` without going through the door — a fixture
/// convenience, and the only place in this file that writes the resource.
fn set_standing(game: &mut Game, key: SettlementKey, standing: i32) {
    game.world
        .resource_mut::<Standings>()
        .0
        .entry(key)
        .or_default()
        .standing = standing;
}

// ---------------------------------------------------------------------------
// The door
// ---------------------------------------------------------------------------

#[test]
fn a_town_starts_neutral_and_at_zero() {
    let game = game();
    let key = SettlementKey { rx: 7, ry: -3 };
    assert_eq!(game.standing(key), 0);
    assert_eq!(game.standing_band(key), Standing::Neutral);
}

#[test]
fn the_door_clamps_at_both_ends() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);

    game.adjust_standing(key, SETTLEMENT_MAX_STANDING * 10);
    assert_eq!(game.standing(key), SETTLEMENT_MAX_STANDING);

    game.adjust_standing(key, -SETTLEMENT_MAX_STANDING * 20);
    assert_eq!(game.standing(key), SETTLEMENT_MIN_STANDING);
}

/// `set_machine_status`' rule, one subsystem over: entering a band is news
/// and staying in it is not.
#[test]
fn a_band_crossing_is_announced_once() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);
    let name = game.settlement_report(key).name;

    // Counted over the whole history, summing `repeats`: `message_history`
    // folds identical lines, so a second, identical announcement would
    // otherwise hide inside the first entry rather than showing as a row.
    let spoken = |game: &Game| -> usize {
        game.message_history(500)
            .iter()
            .filter(|entry| entry.text.contains(&name))
            .map(|entry| entry.repeats)
            .sum()
    };

    game.adjust_standing(key, SETTLEMENT_WARM_STANDING);
    assert_eq!(spoken(&game), 1, "a crossing must speak exactly once");
    assert!(
        game.message_history(500)
            .iter()
            .any(|entry| entry.text.contains("Warm")),
        "the line must name the band it crossed into"
    );

    game.adjust_standing(key, 1);
    assert_eq!(spoken(&game), 1, "staying in a band is not news");
}

#[test]
fn a_move_that_changes_nothing_says_nothing() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);
    game.adjust_standing(key, SETTLEMENT_MAX_STANDING);

    let before = game.message_history(500).len();
    game.adjust_standing(key, 5);
    assert_eq!(game.standing(key), SETTLEMENT_MAX_STANDING);
    assert_eq!(
        game.message_history(500).len(),
        before,
        "a clamped no-op must not announce a crossing"
    );
}

// ---------------------------------------------------------------------------
// Movers
// ---------------------------------------------------------------------------

#[test]
fn trading_at_a_towns_counter_earns_standing_with_it() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);
    // Enough cargo that one basket clears the volume threshold on its own.
    let scrap = ItemId::from(ids::CORE_FRAGMENT);
    let unit = game
        .settlement_sell_price(&scrap, crate::settlements::Temperament::Open)
        .max(1);
    let qty = (SETTLEMENT_TRADE_CREDITS_PER_POINT / unit) + 1;
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(scrap.clone(), qty);

    let copy = GearCopy::plain(scrap);
    game.commit_settlement_basket(key, vec![(copy, qty)], Vec::new())
        .expect("the basket commits");

    assert!(
        game.standing(key) > 0,
        "volume across the counter must move standing, got {}",
        game.standing(key)
    );
}

#[test]
fn bringing_down_a_nest_beside_a_town_is_noticed_and_one_far_away_is_not() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();

    let far = game.spawn_nest("scrapper", pos.x + SETTLEMENT_NOTICE_RADIUS + 5, pos.y);
    game.world.get_mut::<Durability>(far).unwrap().hp = 1;
    game.attack_nest(far);
    assert_eq!(
        game.standing(key),
        0,
        "a nest beyond the radius is not the town's news"
    );

    let near = game.spawn_nest("scrapper", pos.x + 2, pos.y + 2);
    game.world.get_mut::<Durability>(near).unwrap().hp = 1;
    game.attack_nest(near);
    assert_eq!(game.standing(key), SETTLEMENT_NEST_CLEARED_STANDING);
}

#[test]
fn collapsing_a_stack_beside_a_town_is_noticed() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let entrance = (pos.x + 3, pos.y + 3);
    game.spawn_entrance_at(entrance.0, entrance.1);

    game.collapse_stack(entrance);

    assert_eq!(game.standing(key), SETTLEMENT_STACK_COLLAPSED_STANDING);
}

// ---------------------------------------------------------------------------
// The consequence
// ---------------------------------------------------------------------------

#[test]
fn a_hostile_town_shuts_its_counter_rather_than_vanishing() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);
    set_standing(&mut game, key, SETTLEMENT_HOSTILE_STANDING);

    let view = game
        .settlement_view(key)
        .expect("the screen must still open — `None` closes it under the player");
    assert!(view.closed);
    assert!(view.offers.is_empty());
    assert!(view.sells.is_empty());
}

/// `commit_caravan_basket`'s rule: every refusal lands before anything is
/// spent.
#[test]
fn a_hostile_town_refuses_a_basket_and_spends_nothing() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);
    let scrap = ItemId::from(ids::CORE_FRAGMENT);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(scrap.clone(), 5);
    set_standing(&mut game, key, SETTLEMENT_HOSTILE_STANDING);

    let held = |game: &Game| {
        game.world
            .get::<Inventory>(game.player_entity())
            .unwrap()
            .count(&scrap)
    };
    let before = held(&game);
    let copy = GearCopy::plain(scrap.clone());
    let refusal = game
        .commit_settlement_basket(key, vec![(copy, 5)], Vec::new())
        .expect_err("a hostile town takes nothing");
    assert!(refusal.contains("won't trade"), "{refusal}");
    assert_eq!(held(&game), before, "a refusal must spend nothing");
}

#[test]
fn the_hub_page_names_the_band() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);
    game.adjust_standing(key, SETTLEMENT_ALLIED_STANDING);
    assert_eq!(game.settlement_report(key).standing, "Allied");
}

// ---------------------------------------------------------------------------
// The save
// ---------------------------------------------------------------------------

#[test]
fn standing_survives_a_save_and_load() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);
    game.adjust_standing(key, SETTLEMENT_WARM_STANDING + 2);
    game.credit_trade_volume(key, SETTLEMENT_TRADE_CREDITS_PER_POINT / 2);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_settlement_standing_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.standing(key), game.standing(key));
    assert_eq!(
        loaded
            .world
            .resource::<Standings>()
            .0
            .get(&key)
            .map(|r| r.trade_credits),
        Some(SETTLEMENT_TRADE_CREDITS_PER_POINT / 2),
        "the remainder is what makes trade a volume rate rather than a rounding rule"
    );
}

// ---------------------------------------------------------------------------
// The garrison — the aid ladder's passive half
// ---------------------------------------------------------------------------

/// Plants a town at `standing`, `dx`/`dy` tiles from the base anchor.
fn town_near_anchor(game: &mut Game, key: SettlementKey, dx: i32, dy: i32, standing: i32) {
    let (ax, ay) = game.anchor_position().expect("a new game has an anchor");
    place_settlement(game, key, ax + dx, ay + dy);
    set_standing(game, key, standing);
}

#[test]
fn an_allied_town_beside_the_anchor_stations_a_detachment() {
    let mut game = game();
    let before = game.total_raid_defense();
    town_near_anchor(
        &mut game,
        SettlementKey { rx: 1, ry: 0 },
        2,
        0,
        SETTLEMENT_ALLIED_STANDING,
    );
    assert_eq!(
        game.total_raid_defense(),
        before + SETTLEMENT_ALLIED_GARRISON
    );
}

#[test]
fn a_town_beyond_the_garrison_radius_stations_nobody() {
    let mut game = game();
    let before = game.total_raid_defense();
    town_near_anchor(
        &mut game,
        SettlementKey { rx: 1, ry: 0 },
        SETTLEMENT_GARRISON_RADIUS + 1,
        0,
        SETTLEMENT_ALLIED_STANDING,
    );
    assert_eq!(game.total_raid_defense(), before);
}

#[test]
fn a_warm_town_garrisons_and_a_neutral_one_does_not() {
    let mut game = game();
    let before = game.total_raid_defense();
    let key = SettlementKey { rx: 1, ry: 0 };

    town_near_anchor(&mut game, key, 2, 0, 0);
    assert_eq!(
        game.total_raid_defense(),
        before,
        "a neutral town garrisons"
    );

    set_standing(&mut game, key, SETTLEMENT_WARM_STANDING);
    let warm = game.total_raid_defense();
    assert_eq!(warm, before + SETTLEMENT_WARM_GARRISON);

    set_standing(&mut game, key, SETTLEMENT_ALLIED_STANDING);
    assert!(
        game.total_raid_defense() > warm,
        "allied stations no more than warm"
    );
}

/// The clamp, and it is checked on the exact figure rather than with a
/// `<=`: an off-by-one in the cap would pass a bound check.
#[test]
fn the_settlement_half_of_raid_defense_is_capped() {
    let mut game = game();
    let before = game.total_raid_defense();
    // Enough Allied neighbours that the uncapped sum would clear the ceiling
    // several times over.
    let towns = (SETTLEMENT_GARRISON_MAX / SETTLEMENT_ALLIED_GARRISON) + 3;
    for i in 0..towns as i32 {
        town_near_anchor(
            &mut game,
            SettlementKey { rx: i + 1, ry: 0 },
            i + 2,
            0,
            SETTLEMENT_ALLIED_STANDING,
        );
    }
    assert_eq!(game.total_raid_defense(), before + SETTLEMENT_GARRISON_MAX);
}

/// The test that fails if the clamp is applied to the total instead of to
/// the settlement half: the player's own shield network is not capped by a
/// settlement constant, and two Shields already out-defend it.
#[test]
fn the_garrison_cap_does_not_touch_the_structure_half() {
    let mut game = game();
    let (ax, ay) = game.anchor_position().unwrap();
    let before = game.total_raid_defense();
    spawn_structure_at(&mut game, "shield", ax + 3, ay);
    spawn_structure_at(&mut game, "shield", ax + 4, ay);
    let with_shields = game.total_raid_defense();
    assert!(
        with_shields > before + SETTLEMENT_GARRISON_MAX,
        "two shields ({with_shields}) were clamped by a settlement constant"
    );
}

// ---------------------------------------------------------------------------
// The gift — the aid ladder's first verb
// ---------------------------------------------------------------------------

/// Every program the party owns, whatever it is currently doing.
fn owned_programs(game: &Game) -> usize {
    game.world
        .iter_entities()
        .filter(|e| e.contains::<crate::components::Tamed>())
        .count()
}

/// A town standing next to the player, Allied, with no gift taken yet.
fn allied_neighbour(game: &mut Game) -> SettlementKey {
    let key = settlement_east_of_player(game);
    set_standing(game, key, SETTLEMENT_ALLIED_STANDING);
    key
}

/// Asserts a refusal changed nothing a gift would have changed.
fn assert_gift_spent_nothing(game: &Game, key: SettlementKey, roster: usize, msg: &str) {
    assert_eq!(owned_programs(game), roster, "{msg}: the roster moved");
    let relation = game
        .world
        .resource::<Standings>()
        .0
        .get(&key)
        .copied()
        .unwrap_or_default();
    assert_eq!(
        relation.last_gift_tick, None,
        "{msg}: the cooldown was started"
    );
    assert_eq!(relation.gifts_taken, 0, "{msg}: the gift was counted");
}

#[test]
fn a_gift_refuses_after_game_over_and_spends_nothing() {
    let mut game = game();
    let key = allied_neighbour(&mut game);
    let roster = owned_programs(&game);
    game.world.resource_mut::<GameOver>().reason = Some("done".to_string());

    assert!(game.request_program_gift(key).is_err());
    assert_gift_spent_nothing(&game, key, roster, "game over");
}

#[test]
fn a_gift_refuses_during_an_active_battle_and_spends_nothing() {
    let mut game = game();
    let key = allied_neighbour(&mut game);
    let roster = owned_programs(&game);
    game.world
        .insert_resource(super::extraction::minimal_active_battle(&game));

    assert!(game.request_program_gift(key).is_err());
    assert_gift_spent_nothing(&game, key, roster, "active battle");
}

#[test]
fn a_gift_refuses_an_unknown_town_and_spends_nothing() {
    let mut game = game();
    let roster = owned_programs(&game);
    let key = SettlementKey { rx: 99, ry: 99 };

    assert!(game.request_program_gift(key).is_err());
    assert_gift_spent_nothing(&game, key, roster, "unknown town");
}

#[test]
fn a_gift_refuses_from_out_of_reach_and_spends_nothing() {
    let mut game = game();
    let key = SettlementKey { rx: 3, ry: 3 };
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    place_settlement(&mut game, key, pos.x + 30, pos.y + 30);
    set_standing(&mut game, key, SETTLEMENT_ALLIED_STANDING);
    let roster = owned_programs(&game);

    assert!(game.request_program_gift(key).is_err());
    assert_gift_spent_nothing(&game, key, roster, "out of reach");
}

#[test]
fn a_gift_refuses_below_allied_and_spends_nothing() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);
    set_standing(&mut game, key, SETTLEMENT_WARM_STANDING);
    let roster = owned_programs(&game);

    assert!(game.request_program_gift(key).is_err());
    assert_gift_spent_nothing(&game, key, roster, "warm town");
}

#[test]
fn a_gift_refuses_inside_the_cooldown_and_spends_nothing() {
    let mut game = game();
    let key = allied_neighbour(&mut game);
    game.request_program_gift(key).expect("the first gift");
    let roster = owned_programs(&game);
    let after_first = *game.world.resource::<Standings>().0.get(&key).unwrap();

    assert!(game.request_program_gift(key).is_err());
    assert_eq!(owned_programs(&game), roster, "a second gift was granted");
    assert_eq!(
        *game.world.resource::<Standings>().0.get(&key).unwrap(),
        after_first,
        "the refused request moved the relation"
    );
}

#[test]
fn the_cooldown_releases_and_the_town_gifts_again() {
    let mut game = game();
    let key = allied_neighbour(&mut game);
    game.request_program_gift(key).expect("the first gift");
    let roster = owned_programs(&game);

    game.world.resource_mut::<GameClock>().tick += SETTLEMENT_GIFT_COOLDOWN_TICKS;

    game.request_program_gift(key).expect("the second gift");
    assert_eq!(owned_programs(&game), roster + 1);
    assert_eq!(
        game.world
            .resource::<Standings>()
            .0
            .get(&key)
            .unwrap()
            .gifts_taken,
        2
    );
}

#[test]
fn a_gifted_program_joins_the_roster_as_staff() {
    let mut game = game();
    let key = allied_neighbour(&mut game);
    let before: Vec<Entity> = game
        .world
        .iter_entities()
        .filter(|e| e.contains::<crate::components::Tamed>())
        .map(|e| e.id())
        .collect();

    game.request_program_gift(key).expect("the gift");

    let after: Vec<Entity> = game
        .world
        .iter_entities()
        .filter(|e| e.contains::<crate::components::Tamed>())
        .map(|e| e.id())
        .collect();
    assert_eq!(after.len(), before.len() + 1);
    let gifted = *after.iter().find(|e| !before.contains(e)).unwrap();
    assert_eq!(
        game.program_role(gifted),
        Some(crate::game::party::ProgramRole::Staff),
        "a gift is labour, not a party member"
    );
    // The roster barrier's own parts, not merely "an entity appeared".
    assert!(game.world.get::<crate::components::Tamed>(gifted).is_some());
    assert!(game.world.get::<crate::components::Stats>(gifted).is_some());
    assert!(
        game.world
            .get::<crate::components::Hostile>(gifted)
            .is_none()
    );
}

/// The whole reason the species is derived: it must not move when the
/// seeded stream does, so a reload — which replays a different number of
/// draws — cannot reroll what a town hands over.
#[test]
fn a_gifts_species_ignores_game_rng() {
    let species_of = |burn: u32| {
        let mut game = game();
        let key = allied_neighbour(&mut game);
        {
            let mut rng = game.world.resource_mut::<GameRng>();
            for _ in 0..burn {
                rng.0.random_range(0..1_000_000);
            }
        }
        let before: Vec<Entity> = game
            .world
            .iter_entities()
            .filter(|e| e.contains::<crate::components::Tamed>())
            .map(|e| e.id())
            .collect();
        game.request_program_gift(key).expect("the gift");
        let gifted = game
            .world
            .iter_entities()
            .filter(|e| e.contains::<crate::components::Tamed>())
            .map(|e| e.id())
            .find(|e| !before.contains(e))
            .unwrap();
        game.world
            .get::<crate::components::Creature>(gifted)
            .unwrap()
            .species
            .clone()
    };
    assert_eq!(species_of(0), species_of(50));
}

/// And the other half, stated exactly: **choosing** the species spends no
/// draw. Spawning one does — `adopt_program` rolls rarity and stats, and
/// every other adoption in the game pays that same cost — so the claim is
/// about the selection, not the door. The control adopts the same species
/// by hand and must leave the stream in the same place.
#[test]
fn choosing_a_gifts_species_spends_no_draw() {
    let mut gifted_game = game();
    let key = allied_neighbour(&mut gifted_game);
    let before: Vec<Entity> = gifted_game
        .world
        .iter_entities()
        .filter(|e| e.contains::<crate::components::Tamed>())
        .map(|e| e.id())
        .collect();
    gifted_game.request_program_gift(key).expect("the gift");
    let gifted = gifted_game
        .world
        .iter_entities()
        .filter(|e| e.contains::<crate::components::Tamed>())
        .map(|e| e.id())
        .find(|e| !before.contains(e))
        .unwrap();
    let species = gifted_game
        .world
        .get::<crate::components::Creature>(gifted)
        .unwrap()
        .species
        .clone();
    let after_gift = gifted_game
        .world
        .resource_mut::<GameRng>()
        .0
        .random_range(0..1_000_000);

    let mut control = game();
    let key = allied_neighbour(&mut control);
    let _ = key;
    let (ax, ay) = control.anchor_position().unwrap();
    control
        .adopt_program(&species, ax, ay, SETTLEMENT_GIFT_STAT_MULT)
        .expect("the control adoption");
    let after_control = control
        .world
        .resource_mut::<GameRng>()
        .0
        .random_range(0..1_000_000);

    assert_eq!(
        after_gift, after_control,
        "picking the species cost a draw the plain adoption did not"
    );
}

#[test]
fn a_gift_is_available_only_at_allied_and_the_preview_matches_the_door() {
    let mut game = game();
    let key = settlement_east_of_player(&mut game);
    assert_eq!(game.gift_available_in(key), None, "a neutral town gifts");

    set_standing(&mut game, key, SETTLEMENT_ALLIED_STANDING);
    assert_eq!(game.gift_available_in(key), Some(0), "allied gifts now");

    game.request_program_gift(key).expect("the gift");
    assert_eq!(
        game.gift_available_in(key),
        Some(SETTLEMENT_GIFT_COOLDOWN_TICKS),
        "the preview must quote the cooldown the door will enforce"
    );
}

#[test]
fn a_gift_and_its_cooldown_survive_a_save_and_load() {
    let mut game = game();
    let key = allied_neighbour(&mut game);
    game.request_program_gift(key).expect("the gift");
    let before = *game.world.resource::<Standings>().0.get(&key).unwrap();
    assert!(before.last_gift_tick.is_some());

    let path = std::env::temp_dir().join(format!(
        "feral_processes_settlement_gift_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        *loaded.world.resource::<Standings>().0.get(&key).unwrap(),
        before,
        "last_gift_tick and gifts_taken must survive the round trip"
    );
}
