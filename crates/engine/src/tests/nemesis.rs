//! `components::Nemesis` — the trigger only. `Game::mark_nemeses` is called
//! from exactly one place, the window in `Game::end_battle` between the
//! `StackSpawn` stray sweep and `BattleState`'s removal. Task 1 ships no
//! reader for the mark beyond the raw component, so these assert directly
//! on `world.get::<Nemesis>(e)` rather than through any public API.

use super::support::*;
use crate::tuning::MAX_NEMESES;
use crate::*;

/// A fight the player cannot win outright and cannot survive either: a
/// `construct` scaled so far past the player's own stats that one round
/// both fails to kill it and flatlines the player. Mirrors
/// `combat::a_round_that_kills_the_player_ends_the_battle`'s fixture, minus
/// the `Permadeath` gate that test needs and a nemesis loss doesn't — a
/// Forgiving reboot is what lets the same program be lost to twice in one
/// test.
fn spawn_overwhelming_wild(game: &mut Game) -> Entity {
    let wild = game
        .spawn_wild_creature("construct", 5, 5)
        .expect("construct ships with the game");
    let mut stats = game.world.get_mut::<Stats>(wild).unwrap();
    stats.hp = 100_000;
    stats.max_hp = 100_000;
    stats.atk = 100_000;
    wild
}

/// Every entity in `game` currently carrying `Nemesis` — the ledger the cap
/// counts against, and what "marks nobody" means when there's no single
/// surviving entity left to assert `None` on (a won fight despawns its only
/// hostile).
fn nemesis_holders(game: &mut Game) -> Vec<Entity> {
    game.world
        .query_filtered::<Entity, With<Nemesis>>()
        .iter(&game.world)
        .collect()
}

#[test]
fn a_won_fight_marks_nobody() {
    let mut game = Game::new(40, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    game.world.get_mut::<Stats>(wild).unwrap().hp = 1;
    game.start_battle(vec![wild]);

    player_attacks(&mut game);

    assert!(!game.has_active_battle(), "the kill should end the fight");
    assert!(
        nemesis_holders(&mut game).is_empty(),
        "an emptied group has nothing left to mark"
    );
}

/// Like `support::flee_until_clear`, but reports which of its two possible
/// endings actually happened, rather than leaving a caller to assume one.
/// `flee_until_clear` returns on *either* a landed escape or a failed
/// attempt's counter-volley ending the fight in defeat first — both leave a
/// surviving hostile marked at grudge 1, so asserting only that the battle
/// is over afterward can't tell a jack-out from a Forgiving loss. Reporting
/// which one happened lets the test below assert on it directly: if the
/// defeat path is ever what actually ends the fight, that assertion fails
/// loudly instead of the test passing for the wrong reason.
///
/// A failed attempt is not free of risk here — `battle::compute_damage`
/// floors every landed hit at `tuning::MIN_DAMAGE`, so even this fixture's
/// zero-`atk` wild program (`spawn_wild_on_player_tile`) still deals 1
/// damage per hit; "deals no damage" would be the wrong reason to trust
/// this loop. What actually keeps 200 straight failures vanishingly
/// unlikely is `battle::jack_out_chance`'s own floor, `JACK_OUT_CHANCE_MIN`
/// — the same bound `support::flee_until_clear`'s doc argues from.
fn flee_until_it_lands(game: &mut Game) -> bool {
    for _ in 0..200 {
        if game.battle_flee() {
            return true;
        }
        if !game.has_active_battle() {
            return false;
        }
    }
    panic!("200 jack-out attempts all failed — the escape roll is broken");
}

#[test]
fn a_successful_jack_out_marks_the_surviving_hostile_at_grudge_1() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);

    assert!(
        flee_until_it_lands(&mut game),
        "the escape roll never landed in 200 tries — jack_out_chance floors \
         well above zero, so this means the roll itself is broken, not that \
         a legitimate defeat occurred"
    );

    assert_eq!(
        game.world.get::<Nemesis>(wild).map(|n| n.0),
        Some(1),
        "the program the party bailed out on should be marked"
    );
}

#[test]
fn a_forgiving_defeat_marks_the_surviving_hostile_at_grudge_1() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_overwhelming_wild(&mut game);
    game.start_battle(vec![wild]);

    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();

    assert!(
        !game.has_active_battle(),
        "the round should have flatlined the player and ended the fight"
    );
    assert_eq!(
        game.world.get::<Nemesis>(wild).map(|n| n.0),
        Some(1),
        "the program that beat the player should be marked"
    );
}

#[test]
fn a_second_loss_to_the_same_program_escalates_grudge_rather_than_re_marking() {
    let mut game = Game::new(43, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_overwhelming_wild(&mut game);

    game.start_battle(vec![wild]);
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();
    assert_eq!(game.world.get::<Nemesis>(wild).map(|n| n.0), Some(1));

    game.start_battle(vec![wild]);
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();

    assert_eq!(
        game.world.get::<Nemesis>(wild).map(|n| n.0),
        Some(2),
        "a second loss should raise the grudge count, not insert a second mark"
    );
    assert_eq!(
        nemesis_holders(&mut game),
        vec![wild],
        "only the one program should ever be marked here"
    );
}

#[test]
fn a_stack_fight_marks_nobody_because_the_strays_are_swept_first() {
    let mut game = Game::new(44, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);
    // What `rouse_lair` tags a Stack pack's members with — see
    // `game/stack.rs` around its battle setup. Tagging a normally spawned
    // pack directly exercises the same despawn path without walking a maze
    // to reach a real lair fight, which the task brief allows as a
    // stand-in.
    game.world.entity_mut(wild).insert(StackSpawn);

    flee_until_clear(&mut game);

    assert!(
        game.world.get_entity(wild).is_err(),
        "a StackSpawn stray should have been despawned on the way out"
    );
    assert!(
        nemesis_holders(&mut game).is_empty(),
        "nothing should be marked once the only survivor was swept first"
    );
}

#[test]
fn an_eleventh_distinct_program_is_not_marked() {
    let mut game = Game::new(45, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for _ in 0..MAX_NEMESES {
        game.world.spawn((Nemesis(1),));
    }
    let wild = start_battle_with_a_wild_program(&mut game);

    flee_until_clear(&mut game);

    assert!(
        game.world.get::<Nemesis>(wild).is_none(),
        "the cap should refuse a fresh mark once it's full"
    );
    assert_eq!(
        nemesis_holders(&mut game).len(),
        MAX_NEMESES,
        "the cap should not have grown"
    );
}

#[test]
fn an_already_marked_nemesis_still_escalates_while_the_cap_is_full() {
    let mut game = Game::new(46, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);
    game.world.entity_mut(wild).insert(Nemesis(1));
    for _ in 1..MAX_NEMESES {
        game.world.spawn((Nemesis(1),));
    }
    assert_eq!(nemesis_holders(&mut game).len(), MAX_NEMESES);

    flee_until_clear(&mut game);

    assert_eq!(
        game.world.get::<Nemesis>(wild).map(|n| n.0),
        Some(2),
        "an existing nemesis should escalate even with no room for a new one"
    );
}

/// A bare stand-in for a marked hostile, carrying only what `promote_rarity`
/// touches — spawned directly rather than through `spawn_wild_creature`, so
/// the numbers are round and the ratio math has nothing to round against.
fn spawn_promotable(game: &mut Game, rarity: Rarity) -> Entity {
    game.world
        .spawn((
            Stats {
                hp: 100,
                max_hp: 100,
                atk: 10,
                def: 10,
            },
            Hostile,
            rarity,
        ))
        .id()
}

#[test]
fn an_ordinary_nemesis_promotes_to_silver_at_the_full_multiplier() {
    let mut game = Game::new(47, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_promotable(&mut game, Rarity::Ordinary);

    let landed = game.promote_rarity(wild);

    assert_eq!(landed, Rarity::Silver);
    assert_eq!(
        game.world.get::<Rarity>(wild).copied(),
        Some(Rarity::Silver)
    );
    let stats = *game.world.get::<Stats>(wild).unwrap();
    assert_eq!(stats.max_hp, 150, "100 * SILVER_STAT_MULT (1.5)");
    assert_eq!(stats.atk, 15, "10 * SILVER_STAT_MULT (1.5)");
    assert_eq!(stats.def, 15, "10 * SILVER_STAT_MULT (1.5)");
}

/// The trap this pins: a second promotion must multiply by the **ratio**
/// between tiers (1.8 / 1.5 = 1.2 here), not by `GOLD_STAT_MULT` (1.8)
/// applied fresh to the already-Silver stats. The two disagree —
/// 150 * 1.2 = 180 against 150 * 1.8 = 270 — so this fails loudly if
/// `promote_rarity` ever regresses to applying the absolute tier multiplier
/// instead of the step between tiers.
#[test]
fn a_second_promotion_to_gold_multiplies_by_the_ratio_not_the_absolute_tier() {
    let mut game = Game::new(48, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_promotable(&mut game, Rarity::Ordinary);
    game.promote_rarity(wild);

    let landed = game.promote_rarity(wild);

    assert_eq!(landed, Rarity::Gold);
    assert_eq!(game.world.get::<Rarity>(wild).copied(), Some(Rarity::Gold));
    let stats = *game.world.get::<Stats>(wild).unwrap();
    assert_eq!(
        stats.max_hp, 180,
        "150 * (GOLD 1.8 / SILVER 1.5) = 150 * 1.2"
    );
    assert_eq!(stats.atk, 18, "15 * 1.2");
    assert_eq!(stats.def, 18, "15 * 1.2");
}

/// A Prismatic nemesis has nowhere left to climb, so the stat multiply is a
/// no-op — but the recharge is not conditioned on the multiply doing
/// anything. `stats.hp` is deliberately left below `max_hp` before the call
/// (`spawn_promotable` starts full, which would let a heal-skipping early
/// return at the ceiling pass unnoticed): a `promote_rarity` that special-
/// cased `new == old` into an early return, skipping the `hp = max_hp` line
/// entirely, would leave `hp` at 40 here and fail the last assertion. That
/// exact mutation was applied and watched fail before this test was kept —
/// see the report for Finding 1's verification.
#[test]
fn a_prismatic_nemesis_does_not_promote_but_still_heals_on_a_mark() {
    let mut game = Game::new(49, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_promotable(&mut game, Rarity::Prismatic);
    game.world.get_mut::<Stats>(wild).unwrap().hp = 40;

    let landed = game.promote_rarity(wild);

    assert_eq!(
        landed,
        Rarity::Prismatic,
        "already at the top of the ladder"
    );
    let stats = *game.world.get::<Stats>(wild).unwrap();
    assert_eq!(stats.atk, 10, "a no-op multiplier leaves stats untouched");
    assert_eq!(stats.def, 10);
    assert_eq!(
        stats.hp, stats.max_hp,
        "the recharge must still run even when the promotion itself is a no-op"
    );
}

#[test]
fn promotion_fully_heals_to_the_new_max_hp() {
    let mut game = Game::new(50, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_promotable(&mut game, Rarity::Ordinary);
    game.world.get_mut::<Stats>(wild).unwrap().hp = 40;

    game.promote_rarity(wild);

    let stats = *game.world.get::<Stats>(wild).unwrap();
    assert_eq!(stats.max_hp, 150);
    assert_eq!(
        stats.hp, stats.max_hp,
        "recharge fills to the promoted max, not the old one"
    );
}

/// The "no stats operation may run while a gear bonus sits in `Stats`" rule
/// is unreachable for `promote_rarity` because a wild program is never
/// equipped — pin the invariant rather than leaving it to the reader, per
/// the design doc.
#[test]
fn a_wild_program_never_carries_equipment_so_the_gear_hazard_is_unreachable() {
    let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);

    assert!(
        game.world.get::<Equipment>(wild).is_none(),
        "a wild program's Stats must stay pure spawn+rarity, or promote_rarity's \
         multiply would weld a gear bonus permanently into the base"
    );
}

/// Drives five real losses through the real `mark_nemeses` path (not a
/// direct `promote_rarity` call), reaching and then passing `Rarity::ALL`'s
/// ceiling. `Rarity::ALL` has 5 rungs, so the 4th mark is the last rung
/// climbed (Ordinary -> Silver -> Gold -> Platinum -> Prismatic) and the
/// 5th is a mark at the ceiling with nowhere left to promote — exercising,
/// through the real end-of-battle path rather than a unit call, exactly the
/// "old == new == Prismatic" case Finding 1 is about. The grudge count
/// keeps counting every loss regardless: it is `mark_nemeses`'s own field,
/// not `promote_rarity`'s, so a program with no rung left to climb still
/// racks one up.
#[test]
fn grudge_count_and_rarity_receipt_agree_past_the_promotion_ceiling() {
    let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_overwhelming_wild(&mut game);
    // Pinned rather than left to the spawn roll — `roll_rarity` could hand
    // this program a head start, and the assertions below need to know
    // exactly how many rungs each loss climbs.
    game.world.entity_mut(wild).insert(Rarity::Ordinary);

    for _ in 0..3 {
        game.start_battle(vec![wild]);
        game.battle_set_action(0, BattleAction::Attack { group: 0 })
            .unwrap();
        game.battle_resolve_round();
    }
    assert_eq!(game.world.get::<Nemesis>(wild).map(|n| n.0), Some(3));
    assert_eq!(
        game.world.get::<Rarity>(wild).copied(),
        Some(Rarity::Platinum),
        "three marks is three promotions: Ordinary -> Silver -> Gold -> Platinum"
    );

    game.start_battle(vec![wild]);
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();
    assert_eq!(game.world.get::<Nemesis>(wild).map(|n| n.0), Some(4));
    assert_eq!(
        game.world.get::<Rarity>(wild).copied(),
        Some(Rarity::Prismatic),
        "the 4th mark climbs the last rung"
    );

    // The 5th mark lands with old == new == Prismatic: nothing left to
    // promote, but the grudge still rises and the recharge still runs.
    game.start_battle(vec![wild]);
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();

    assert_eq!(
        game.world.get::<Nemesis>(wild).map(|n| n.0),
        Some(5),
        "the grudge keeps counting past the ceiling"
    );
    assert_eq!(
        game.world.get::<Rarity>(wild).copied(),
        Some(Rarity::Prismatic),
        "the ceiling holds"
    );
    let stats = *game.world.get::<Stats>(wild).unwrap();
    assert_eq!(
        stats.hp, stats.max_hp,
        "the recharge must still run on a mark that promotes nothing"
    );
}

/// Saves `game` to a fresh temp file and loads it straight back, mirroring
/// `spawning::a_wild_carrier_survives_a_save_load_round_trip` and
/// `spawning::a_nest_survives_a_save_load_round_trip` — one file per test
/// (via `tag`) and per process, so parallel test runs don't collide.
fn round_trip(game: &mut Game, tag: &str) -> Game {
    let path = std::env::temp_dir().join(format!(
        "feral_nemesis_save_{tag}_{}.sav",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    loaded
}

// The three round-trip tests below all spawn their one creature of interest
// at coordinates well outside `tuning::INITIAL_SPAWN_SCATTER_TILES` (40) of
// the zone spawn point, so `Game::new`'s own habitat spawning can't roll a
// second creature onto the same tile and make the `find` below ambiguous
// about which entity is "the" nemesis — the same precaution
// `spawning::a_nest_survives_a_save_load_round_trip` takes by filtering on
// tile.

#[test]
fn a_nemesis_survives_a_save_load_round_trip() {
    let mut game = Game::new(53, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("construct", 300, 300).unwrap();
    game.world.entity_mut(wild).insert(Nemesis(3));

    let mut loaded = round_trip(&mut game, "grudge");

    let mut query = loaded.world.query::<(&Position, Option<&Nemesis>)>();
    let (_, nemesis) = query
        .iter(&loaded.world)
        .find(|(pos, _)| pos.x == 300 && pos.y == 300)
        .expect("the nemesis must survive the round trip");
    assert_eq!(
        nemesis.map(|n| n.0),
        Some(3),
        "the grudge count must round-trip"
    );
}

/// The compounding guard this task exists to pin: `promote_rarity`'s
/// multiplier is already baked into `Stats` before the save happens, and
/// `CreatureSave::rarity` is restored as a tag only (`lifecycle.rs`, beside
/// where `c.rarity` is inserted). If a reload ever re-applied the
/// multiplier on top of the saved numbers, `after` would differ from
/// `before` here — this was verified by temporarily reintroducing that
/// exact bug in the loader, watching this assertion fail, and reverting.
#[test]
fn a_promoted_nemesis_stats_are_byte_identical_across_a_save_load_round_trip() {
    let mut game = Game::new(54, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("construct", 301, 301).unwrap();
    game.world.entity_mut(wild).insert(Rarity::Ordinary);
    game.promote_rarity(wild);
    game.world.entity_mut(wild).insert(Nemesis(1));
    let before = *game.world.get::<Stats>(wild).unwrap();

    let mut loaded = round_trip(&mut game, "stats");

    let mut query = loaded.world.query::<(&Position, &Stats)>();
    let (_, after) = query
        .iter(&loaded.world)
        .find(|(pos, _)| pos.x == 301 && pos.y == 301)
        .expect("the promoted nemesis must survive the round trip");
    assert_eq!(after.hp, before.hp, "hp must round-trip exactly");
    assert_eq!(
        after.max_hp, before.max_hp,
        "a reload must not re-apply promote_rarity's multiplier on top of \
         the numbers already saved — this is the compounding trap"
    );
    assert_eq!(
        after.atk, before.atk,
        "same trap as max_hp — atk would compound too"
    );
    assert_eq!(
        after.def, before.def,
        "same trap as max_hp — def would compound too"
    );
}

/// A save written before `nemesis_grudges` existed has no such key in its
/// RON at all — `#[serde(default)]` is what makes that load rather than
/// error, and a stripped-down file is how a v29-shaped save without this
/// field is reproduced without hand-maintaining a second save fixture.
#[test]
fn a_save_written_without_the_nemesis_field_loads_to_an_unmarked_creature() {
    let mut game = Game::new(55, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.spawn_wild_creature("construct", 302, 302).unwrap();

    let path =
        std::env::temp_dir().join(format!("feral_nemesis_no_field_{}.sav", std::process::id()));
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("nemesis_grudges"),
        "a fresh save must carry the field, or stripping it below proves nothing"
    );
    let stripped: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("nemesis_grudges"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, stripped).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let mut query = loaded.world.query::<(&Position, Option<&Nemesis>)>();
    let (_, nemesis) = query
        .iter(&loaded.world)
        .find(|(pos, _)| pos.x == 302 && pos.y == 302)
        .expect("the creature must still load without the field");
    assert!(
        nemesis.is_none(),
        "a save written before this field existed must load an unmarked \
         creature, not error"
    );
}
