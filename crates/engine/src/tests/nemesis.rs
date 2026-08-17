//! `components::Nemesis` — the trigger only. `Game::mark_nemeses` is called
//! from exactly one place, the window in `Game::end_battle` between the
//! `StackSpawn` stray sweep and `BattleState`'s removal. Task 1 ships no
//! reader for the mark beyond the raw component, so these assert directly
//! on `world.get::<Nemesis>(e)` rather than through any public API.

use super::support::*;
use crate::nemesis::{self, NemesisDb};
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

// --- Task 4: the name bank and the derived name ---------------------------

/// The controller ruling this test follows instead of the task brief:
/// `GameRng` wraps `StdRng`, which is not `PartialEq`, so its state cannot
/// be snapshotted and compared directly. The differential proof is two
/// `Game`s built from the same seed — sharing a stream position by
/// construction, since neither `Game::new` nor `start_battle_with_a_wild_
/// program` draws from `GameRng` — with only one asked to mark. **Several**
/// draws afterward, not one: a single draw could coincidentally land on the
/// same value either way, where a run of five landing on the same sequence
/// is not a coincidence.
///
/// Verified by mutation: a `rng.0.random::<u64>()` draw was added inside
/// `name_new_nemesis` and this test failed (`marked` and `untouched`
/// diverged at the very first draw), then the draw was removed. See the
/// task report for the transcript.
#[test]
fn marking_a_nemesis_spends_no_gamerng_draw() {
    let build = || {
        let mut game = Game::new(60, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let wild = start_battle_with_a_wild_program(&mut game);
        (game, wild)
    };
    let (mut marked, _) = build();
    marked.mark_nemeses();
    let (mut untouched, _) = build();

    let draw_several = |game: &mut Game| -> Vec<u64> {
        (0..5)
            .map(|_| game.world.resource_mut::<GameRng>().0.random())
            .collect()
    };

    assert_eq!(
        draw_several(&mut marked),
        draw_several(&mut untouched),
        "marking a nemesis must not advance the shared GameRng stream — a \
         draw here would shift every later roll of this run, and end_battle \
         runs this on every arena fight, so it would shift every scenario \
         in dev-arenas/ too"
    );
}

/// The name is derived, not looked up by hand — `expected` is computed
/// through the same public functions `name_new_nemesis` calls
/// (`nemesis::name_seed` and `NemesisDb::name`), so this pins the write
/// happening at all rather than duplicating the fold's arithmetic.
#[test]
fn a_name_is_derived_and_stored_in_customname_on_the_first_mark() {
    let mut game = Game::new(60, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);
    let species = game.world.get::<Creature>(wild).unwrap().species.clone();
    let potential = game
        .world
        .get::<Potential>(wild)
        .copied()
        .unwrap_or(Potential::NEUTRAL);
    let seed = nemesis::name_seed(&species, &potential);
    let expected = game
        .world
        .resource::<NemesisDb>()
        .name(seed)
        .map(str::to_string);
    assert!(
        expected.is_some(),
        "the shipped names.ron must not be empty, or this test proves nothing"
    );

    flee_until_clear(&mut game);

    assert_eq!(
        game.world.get::<CustomName>(wild).map(|c| c.0.clone()),
        expected,
        "the first mark should have written the bank-derived name"
    );
}

/// Verified by mutation: the `fresh` guard in `mark_nemeses` was removed
/// (calling `name_new_nemesis` unconditionally on every mark) and this test
/// failed, because the second mark then re-derived and overwrote the name
/// from a seed the grudge count had shifted — it no longer matched `first`.
/// The guard was restored and this test passed again.
#[test]
fn a_second_grudge_does_not_rename() {
    let mut game = Game::new(60, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_overwhelming_wild(&mut game);
    game.start_battle(vec![wild]);
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();
    let first = game.world.get::<CustomName>(wild).map(|c| c.0.clone());
    assert!(
        first.is_some(),
        "the first loss should have named the program"
    );

    // `name_seed` folds only the species id and `Potential` — neither
    // changes across these two losses on its own, so an unconditional
    // re-derivation on the second mark would silently write back the exact
    // same string and a plain before/after comparison could not tell that
    // apart from the guard actually holding. Perturbing `Potential` between
    // the two marks makes a re-derivation observable: if the guard is gone,
    // `CustomName` moves to `would_be_second_name` below instead of staying
    // at `first`.
    let species = game.world.get::<Creature>(wild).unwrap().species.clone();
    let perturbed = Potential {
        hp_roll: 0.2,
        atk_roll: 0.3,
        def_roll: 0.25,
        growth_roll: 0.4,
    };
    let would_be_second_name = game
        .world
        .resource::<NemesisDb>()
        .name(nemesis::name_seed(&species, &perturbed))
        .map(str::to_string);
    assert_ne!(
        would_be_second_name, first,
        "the perturbed Potential must derive a different name than `first`, \
         or this fixture cannot tell a rename apart from an unconditional \
         rewrite that happens to land on the same string — change the rolls"
    );
    game.world.entity_mut(wild).insert(perturbed);

    game.start_battle(vec![wild]);
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();

    assert_eq!(
        game.world.get::<Nemesis>(wild).map(|n| n.0),
        Some(2),
        "the second loss should still have escalated the grudge"
    );
    assert_eq!(
        game.world.get::<CustomName>(wild).map(|c| c.0.clone()),
        first,
        "a second grudge must not rename an already-named nemesis"
    );
}

#[test]
fn two_nemeses_of_one_species_with_different_potential_get_different_names() {
    let potential_a = Potential {
        hp_roll: 0.8,
        atk_roll: 0.9,
        def_roll: 1.0,
        growth_roll: 1.1,
    };
    let potential_b = Potential {
        hp_roll: 1.2,
        atk_roll: 1.05,
        def_roll: 0.95,
        growth_roll: 0.85,
    };
    let name_for = |potential: Potential| {
        let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let wild = start_battle_with_a_wild_program(&mut game);
        game.world.entity_mut(wild).insert(potential);
        flee_until_clear(&mut game);
        game.world
            .get::<CustomName>(wild)
            .map(|c| c.0.clone())
            .expect("the mark should have named the program")
    };

    let a = name_for(potential_a);
    let b = name_for(potential_b);

    assert_ne!(
        a, b,
        "two nemeses of the same species with different Potential rolls \
         should not share a name"
    );
}

/// Malformed content is skipped with a warning rather than panicking,
/// matching `DescriptionDb::load_dir`'s discipline.
#[test]
fn a_malformed_bank_file_is_skipped_with_a_warning_and_does_not_panic() {
    let dir = std::env::temp_dir().join(format!(
        "feral_nemesis_malformed_bank_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("names.ron"), "not ( valid ron").unwrap();

    let (db, warnings) = NemesisDb::load_dir(&dir).expect("a malformed file must not error out");

    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        warnings.len(),
        1,
        "the malformed names.ron should have produced exactly one warning"
    );
    assert!(
        warnings[0].contains("names.ron"),
        "the warning should name the file that was skipped: {warnings:?}"
    );
    assert!(
        db.name(0).is_none(),
        "a bank that failed to load must leave the pool empty, not panic or \
         half-populate it"
    );
}

/// The absence this pins: an empty (or unloaded) bank is not a fault.
/// `mark_nemeses` still marks, promotes and recharges — it just writes no
/// `CustomName`, since `name_new_nemesis` returns early on `NemesisDb::name`
/// reporting `None`.
#[test]
fn with_an_empty_bank_a_nemesis_is_still_marked_promoted_and_recharged_just_unnamed() {
    let mut game = Game::new(60, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(NemesisDb::default());
    let wild = start_battle_with_a_wild_program(&mut game);
    game.world.entity_mut(wild).insert(Rarity::Ordinary);

    flee_until_clear(&mut game);

    assert_eq!(
        game.world.get::<Nemesis>(wild).map(|n| n.0),
        Some(1),
        "the mark itself does not depend on the name bank"
    );
    assert_eq!(
        game.world.get::<Rarity>(wild).copied(),
        Some(Rarity::Silver),
        "promotion does not depend on the name bank either"
    );
    let stats = *game.world.get::<Stats>(wild).unwrap();
    assert_eq!(stats.hp, stats.max_hp, "the recharge must still run");
    assert!(
        game.world.get::<CustomName>(wild).is_none(),
        "an empty bank must leave the mark unnamed rather than writing a \
         placeholder or panicking"
    );
}
