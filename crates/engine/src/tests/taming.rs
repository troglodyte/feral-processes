//! Decompiling a wild program into a companion, and the catalysts it consumes.

use super::support::*;
use crate::tuning::DECOMPILE_ATTEMPT_BONUS_CAP;
use crate::*;

/// A `GameRng` seed whose first `random_bool` draw comes up false, so the
/// tests below can be about what a *failed* decompile leaves behind rather
/// than about landing one. Each such test asserts the target survived, so a
/// stream shift that invalidates this fails loudly instead of quietly
/// testing a success path.
const SEED_THAT_FAILS: u64 = 1;

#[test]
fn successful_decompile_removes_wander_ai_so_the_tamed_creature_stops_roaming() {
    let mut game = Game::new(19, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");

    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            WanderAi::default(),
            Position { x: 3, y: 3 },
            Stats {
                hp: 1,
                max_hp: 10,
                atk: 1,
                mitigation: 1,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    // Near-dead target + maxed decompiler skill + plenty of breakers,
    // so the capture-chance clamp (95%) makes a handful of attempts
    // succeed for certain, without needing to control the RNG directly.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::ICE_BREAKER), 50);
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

    for _ in 0..50 {
        if game.world.get::<Tamed>(wild).is_some() {
            break;
        }
        player_decompiles(&mut game);
    }

    assert!(
        game.world.get::<Tamed>(wild).is_some(),
        "creature should have been tamed"
    );
    assert!(game.world.get::<Hostile>(wild).is_none());
    assert!(
        game.world.get::<WanderAi>(wild).is_none(),
        "a tamed creature must stop roaming like a wild one"
    );
}

#[test]
fn decompile_spends_the_highest_potency_catalyst_held_not_the_shipped_one() {
    // The mod case `taming_potency` exists for: a dropped-in catalyst
    // stronger than the shipped ICE Breaker must be the one resolved
    // and consumed, with no Rust change.
    let dir = modded_assets_dir(
        "strong_catalyst",
        &[],
        &[(
            "master_key.ron",
            r#"(id: "master_key", name: "Master Key", taming_potency: Some(0.9))"#,
        )],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(3100, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 1), ("master_key", 1)]);

    player_decompiles(&mut game);

    let inv = game.world.get::<Inventory>(game.player_entity()).unwrap();
    assert_eq!(
        inv.count(&ItemId::from("master_key")),
        0,
        "the strongest catalyst held should be the one spent"
    );
    assert_eq!(
        inv.count(&ItemId::from(ids::ICE_BREAKER)),
        1,
        "the weaker catalyst must be left untouched"
    );
}

#[test]
fn decompiling_with_no_catalyst_is_refused_without_naming_a_shipped_item() {
    let mut game = Game::new(3101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);

    // No catalyst greys the row, so `battle_set_action` refuses it before a
    // round can ever resolve — the refusal is this `Err`, not a logged line.
    let index = game
        .battle_special_options(0)
        .into_iter()
        .find(|o| o.name.to_lowercase().contains("decompile"))
        .expect("the player starts with decompile installed")
        .index;
    let err = game
        .battle_set_action(
            0,
            BattleAction::Special {
                ability: index,
                target: battle::SpecialTarget::EnemyGroup { group: 0 },
            },
        )
        .unwrap_err();

    assert!(
        game.world.get::<Tamed>(wild).is_none(),
        "a decompile with no catalyst must not tame anything"
    );
    let shipped_names: Vec<String> = game
        .world
        .resource::<ItemDb>()
        .all()
        .map(|d| d.name.clone())
        .collect();
    for name in shipped_names {
        assert!(
            !err.contains(&name),
            "the refusal must not name a specific item, got: {err}"
        );
    }
}

#[test]
fn two_catalysts_of_equal_potency_resolve_to_the_first_id_alphabetically() {
    let dir = modded_assets_dir(
        "tied_catalysts",
        &[],
        &[
            (
                "alpha_key.ron",
                r#"(id: "alpha_key", name: "Alpha Key", taming_potency: Some(0.5))"#,
            ),
            (
                "omega_key.ron",
                r#"(id: "omega_key", name: "Omega Key", taming_potency: Some(0.5))"#,
            ),
        ],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(3102, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    start_battle_with_a_wild_program(&mut game);
    // Stocked in reverse so the tie can't be won by inventory order.
    set_inventory(&mut game, &[("omega_key", 1), ("alpha_key", 1)]);

    player_decompiles(&mut game);

    let inv = game.world.get::<Inventory>(game.player_entity()).unwrap();
    assert_eq!(
        inv.count(&ItemId::from("alpha_key")),
        0,
        "a tie should resolve to the first item id alphabetically"
    );
    assert_eq!(inv.count(&ItemId::from("omega_key")), 1);
}

#[test]
fn the_decompile_preview_follows_the_catalyst_held_not_a_fixed_item() {
    let dir = modded_assets_dir(
        "preview_catalyst",
        &[],
        &[(
            "master_key.ron",
            r#"(id: "master_key", name: "Master Key", taming_potency: Some(0.9))"#,
        )],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(3104, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let wild = spawn_wild_on_player_tile(&mut game);

    set_inventory(&mut game, &[(ids::ICE_BREAKER, 1)]);
    let with_shipped = program_manifest(&game, wild)
        .decompile_chance
        .expect("holding a catalyst should quote odds");
    set_inventory(&mut game, &[("master_key", 1)]);
    let with_mod = program_manifest(&game, wild)
        .decompile_chance
        .expect("holding a catalyst should quote odds");
    assert!(
        with_mod > with_shipped,
        "a stronger catalyst must preview better odds: {with_mod} vs {with_shipped}"
    );

    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 1)]);
    assert!(
        program_manifest(&game, wild).decompile_chance.is_none(),
        "with no catalyst there are no odds to quote — the action is unavailable"
    );
}

#[test]
fn a_running_capture_boost_field_buff_raises_the_quoted_decompile_odds() {
    let mut game = Game::new(3106, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 1)]);

    let before = program_manifest(&game, wild)
        .decompile_chance
        .expect("holding a catalyst should quote odds");

    let player = game.player_entity();
    game.world.entity_mut(player).insert(FieldBuff {
        active: vec![ActiveFieldBuff {
            kind: FieldBuffKind::CaptureBoost,
            name: "Test Capture Boost".to_string(),
            power: 30,
            remaining: 10,
            interval: 1,
            source: BuffSource::Routine,
        }],
    });

    let after = program_manifest(&game, wild)
        .decompile_chance
        .expect("holding a catalyst should still quote odds");

    assert!(
        after > before,
        "a running CaptureBoost should raise the quoted decompile odds: {after} vs {before}"
    );
}

#[test]
fn battle_view_offers_no_decompile_odds_without_a_catalyst() {
    let mut game = Game::new(3105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    start_battle_with_a_wild_program(&mut game);
    assert!(
        game.battle_view().unwrap().groups[0]
            .decompile_chance
            .is_some(),
        "the starting kit holds a catalyst"
    );

    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);

    // This is also what gates the engine-emitted de[c]ompile option.
    assert!(
        game.battle_view().unwrap().groups[0]
            .decompile_chance
            .is_none()
    );
}

#[test]
fn the_shipped_ice_breaker_still_tames_for_a_player_holding_only_it() {
    let mut game = Game::new(3103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
    // High skill against a fully-weakened target, which is the best odds the
    // shipped catalyst can produce for any species. Skill alone no longer
    // pins the chance to its clamp (it multiplies the base rather than being
    // added to it), so the target is weakened too: that puts even the
    // hardest-to-tame species far enough above zero that 50 seeded attempts
    // land without the test depending on a particular roll.
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;
    {
        let mut stats = game.world.get_mut::<Stats>(wild).unwrap();
        stats.hp = 1;
    }
    // The player has to outlive the attempts, or this stops being a test
    // about the catalyst and becomes a race between the capture roll and
    // the wild program's damage — which is a race the RNG stream position
    // decides. It was lost the day a draw was added upstream: the player
    // died on the 12th attempt and the untamed target read as a taming
    // failure. Capture odds are a function of the *target's* HP fraction
    // (`taming::capture_chance`), so nothing here touches what is measured.
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 100_000;
        stats.hp = 100_000;
    }

    // Counted by what the inventory actually lost rather than by loop
    // iterations: once the roll lands, `Tamed` is not visible until the
    // battle resolves, so the calls in between are refused and rightly
    // charge nothing. Counting iterations called those attempts and made
    // the assertion below depend on how many turns the seed took to settle
    // — which moves whenever anything upstream shifts `GameRng`.
    let held = |game: &Game| {
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::ICE_BREAKER))
    };
    let mut attempts = 0;
    for _ in 0..50 {
        if game.world.get::<Tamed>(wild).is_some() {
            break;
        }
        let before = held(&game);
        player_decompiles(&mut game);
        let spent = before - held(&game);
        assert!(
            spent <= 1,
            "a single decompile attempt spent {spent} catalysts"
        );
        attempts += spent;
    }
    assert!(attempts > 0, "the tame cost no catalyst at all");

    assert!(
        game.world.get::<Tamed>(wild).is_some(),
        "the shipped catalyst must still tame exactly as before"
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::ICE_BREAKER)),
        50 - attempts,
        "one ICE Breaker per attempt, same as before"
    );
}

/// A program decompiled while another group is still standing leaves the
/// fight in progress (`end_battle` never runs) and drops out of its group
/// the same round — so it is in neither `all_living_enemies()` nor `Party`.
/// Nothing ticks or clears its `CombatBuff`/`AbilityCooldowns` in that state,
/// which was harmless before this branch (no hostile could hold either) and
/// is a live bug now that a carrier can.
#[test]
fn decompiling_a_program_mid_fight_clears_its_battle_scoped_state() {
    let mut game = Game::new(9101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let (x, y) = multi_group_ground(&mut game);
    let front_a = game.spawn_wild_creature("glitch", x, y).unwrap();
    let front_b = game.spawn_wild_creature("scrapper", x, y + 1).unwrap();
    game.start_battle(vec![front_a, front_b]);
    {
        let mut stats = game.world.get_mut::<Stats>(front_a).unwrap();
        stats.hp = 1;
    }
    // As if `front_a` had already mirrored a buff onto itself and fired a
    // routine earlier this same fight, before the capture that follows.
    game.arm_buff(
        front_a,
        ActiveBuff {
            kind: BuffKind::Mitigation,
            remaining: 3,
            power: 9,
        },
    );
    game.world.entity_mut(front_a).insert(AbilityCooldowns(
        std::iter::once(("kernel_panic".to_string(), 3)).collect(),
    ));
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

    for _ in 0..50 {
        if game.world.get::<Tamed>(front_a).is_some() {
            break;
        }
        game.attempt_decompile(0, player);
    }

    assert!(
        game.world.get::<Tamed>(front_a).is_some(),
        "front_a should have been captured"
    );
    assert!(
        game.world.get_resource::<BattleState>().is_some(),
        "group B is still standing, so the fight must still be going"
    );
    assert!(
        game.world
            .get::<CombatBuff>(front_a)
            .is_none_or(|b| b.active.is_none()),
        "a program captured mid-fight must not keep a battle-scoped buff forever"
    );
    assert!(
        game.world
            .get::<AbilityCooldowns>(front_a)
            .is_none_or(|c| c.0.is_empty()),
        "nor a cooldown from the routine it fired before capture"
    );
}

/// A boss is an encounter, never a companion. The refusal has to live at
/// plan time rather than in `ability_unavailable`, which takes no target —
/// the player picks the ability before the group, so the row can't grey.
#[test]
fn a_boss_is_refused_as_a_decompile_target() {
    let mut game = Game::new(3106, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = spawn_boss_on_player_tile(&mut game);
    game.start_battle(vec![boss]);

    let ability = game
        .battle_special_options(0)
        .into_iter()
        .find(|o| o.name.to_lowercase().contains("decompile"))
        .expect("the player starts with decompile installed")
        .index;

    let err = game
        .battle_set_action(
            0,
            BattleAction::Special {
                ability,
                target: crate::battle::SpecialTarget::EnemyGroup { group: 0 },
            },
        )
        .expect_err("a boss must be refused as a decompile target");
    assert!(
        err.to_lowercase().contains("ice"),
        "the refusal should say why, got {err:?}"
    );
}

/// A lair's guardian is an encounter too, and for a second reason on top of
/// the one above: walking off with it would leave the stack standing with
/// nothing left to beat, since `FrameMemory::cleared` is written by
/// `award_loot` and taming spends none. The boss refusal already covers
/// every shipped guardian; this covers `pick_lair_species`'s fallback, the
/// ordinary program a biome with no boss fields.
#[test]
fn a_lairs_guardian_is_refused_as_a_decompile_target() {
    let mut game = Game::new(3111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    descend(&mut game);
    let guardian = rouse_a_tameable_guardian(&mut game);
    assert!(
        !game.is_boss_creature(guardian),
        "the fixture must field the fallback guardian — a boss is refused by the rule above"
    );
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);

    let ability = game
        .battle_special_options(0)
        .into_iter()
        .find(|o| o.name.to_lowercase().contains("decompile"))
        .expect("the player starts with decompile installed")
        .index;

    let err = game
        .battle_set_action(
            0,
            BattleAction::Special {
                ability,
                target: crate::battle::SpecialTarget::EnemyGroup { group: 0 },
            },
        )
        .expect_err("a lair guardian must be refused as a decompile target");
    assert!(
        err.to_lowercase().contains("ice"),
        "the refusal should say why, got {err:?}"
    );
}

#[test]
fn a_refused_decompile_costs_the_player_no_catalyst_and_never_tames_the_boss() {
    let mut game = Game::new(3107, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let boss = spawn_boss_on_player_tile(&mut game);
    game.start_battle(vec![boss]);

    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

    for _ in 0..10 {
        player_decompiles(&mut game);
    }

    assert!(
        game.world.get::<Tamed>(boss).is_none(),
        "a boss must never join the roster"
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::ICE_BREAKER)),
        50,
        "a decompile that is refused must not spend a catalyst"
    );
}

#[test]
fn the_battle_view_quotes_no_decompile_odds_against_a_boss() {
    let mut game = Game::new(3108, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = spawn_boss_on_player_tile(&mut game);
    game.start_battle(vec![boss]);

    let view = game.battle_view().unwrap();
    assert!(view.groups[0].is_boss, "the fixture should field a boss");
    assert!(
        view.groups[0].decompile_chance.is_none(),
        "a boss can't be decompiled, so the target list must not advertise odds"
    );
}

/// Every line the log currently holds, for the two tests below reading a
/// specific one out.
fn log_text(game: &Game) -> Vec<String> {
    game.world
        .resource::<MessageLog>()
        .lines
        .iter()
        .map(|e| e.text.clone())
        .collect()
}

/// Hands the battle a decompile counter directly, for the read-path tests
/// that are about what the odds *do* with a count rather than about how the
/// count got there — those would otherwise have to land a seeded roll per
/// attempt, which couples them to the RNG stream for nothing.
fn set_attempts(game: &mut Game, target: Entity, attempts: u32) {
    game.world
        .resource_mut::<BattleState>()
        .decompile_attempts
        .insert(target, attempts);
}

fn quoted_odds(game: &Game, group: usize) -> f32 {
    game.battle_view().unwrap().groups[group]
        .decompile_chance
        .expect("holding a catalyst should quote odds")
}

#[test]
fn a_failed_decompile_raises_the_odds_quoted_for_the_next_one() {
    let mut game = Game::new(3109, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
    // A seeded stream so the attempt below reliably *fails* — the whole
    // point of the test is what a failure leaves behind. The precondition
    // assert is what stops this going quiet if a formula change ever makes
    // the first draw land instead.
    game.world
        .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(SEED_THAT_FAILS)));

    let before = quoted_odds(&game, 0);
    game.attempt_decompile(0, player);
    assert!(
        game.world.get::<Hostile>(wild).is_some(),
        "the seeded roll must fail for this test to be about anything"
    );

    let after = quoted_odds(&game, 0);
    assert!(
        after > before,
        "a spent attempt should leave the next one better off: {after} vs {before}"
    );
}

/// A spent attempt is narration of the round that spent it — the pane shows
/// every one of them live — but the summary a finished fight leaves on the
/// map wants the verdict once. Six catalysts fed to one program used to put
/// six near-identical refusals on the results screen, and the last of them is
/// the only one that still says anything: it is the one carrying the cap.
#[test]
fn only_the_last_failed_decompile_reaches_the_battle_summary() {
    let mut game = Game::new(3115, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);

    let attempts = DECOMPILE_ATTEMPT_BONUS_CAP + 1;
    for _ in 0..attempts {
        // Reseeded per attempt rather than once: `SEED_THAT_FAILS` is a seed
        // whose *first* draw comes up false, so restarting the stream is what
        // makes every one of these a failure rather than most of them.
        game.world
            .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(SEED_THAT_FAILS)));
        game.attempt_decompile(0, player);
    }
    assert!(
        game.world.get::<Hostile>(wild).is_some(),
        "every seeded roll must fail for this test to be about anything"
    );
    assert_eq!(
        ice_holds_lines(&game).len(),
        attempts as usize,
        "the live pane shows every attempt — that half is not the bug"
    );

    game.world.get_mut::<Stats>(wild).unwrap().hp = 1;
    // Forced: the summary this test reads only exists once the fight ends,
    // and the killing blow can miss now.
    force_the_next_attack_to_land(&mut game);
    resolve_round_with(&mut game, BattleAction::Attack { group: 0 });
    assert!(
        game.world.get_resource::<BattleState>().is_none(),
        "the fight has to be over for this to be about the summary"
    );

    let summary = ice_holds_lines(&game);
    assert_eq!(
        summary.len(),
        1,
        "one verdict, not one line per catalyst: {summary:?}"
    );
    assert!(
        summary[0].contains("as frayed as they will get"),
        "and it has to be the *last* attempt's, which is the one naming the cap: {:?}",
        summary[0]
    );
}

/// The breach line is pushed live and survives the prune where it stands, so
/// a refusal held over from an earlier attempt would be flushed *below* it and
/// read as the capture having come undone.
#[test]
fn a_capture_leaves_no_refusal_on_the_summary() {
    let mut game = Game::new(3116, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);

    game.world
        .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(SEED_THAT_FAILS)));
    game.attempt_decompile(0, player);
    assert_eq!(
        ice_holds_lines(&game).len(),
        1,
        "the capture below has to have a refusal to clear for this to be about anything"
    );

    // Left to the unseeded stream from here, at odds a wounded program with a
    // skilled decompiler behind it cannot hold out against for long.
    game.world.get_mut::<Stats>(wild).unwrap().hp = 1;
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;
    for _ in 0..50 {
        if game.world.get::<Tamed>(wild).is_some() {
            break;
        }
        game.attempt_decompile(0, player);
    }

    assert!(
        game.world.get::<Tamed>(wild).is_some(),
        "the program should have been captured"
    );
    assert!(
        game.world.get_resource::<BattleState>().is_none(),
        "it was the last group standing, so the fight is over"
    );
    // Every attempt above was made inside one round, which is what puts the
    // early refusal in the same `since_round` range as the capture — in play
    // each attempt is its own round and only the last one's lines are on the
    // results screen. The prune is what guarantees it either way, and it now
    // runs when the player leaves that screen rather than at `end_battle`.
    game.prune_battle_narration();
    let summary = ice_holds_lines(&game);
    assert!(
        summary.is_empty(),
        "a fight that ended in a capture has no refusal left to report: {summary:?}"
    );
}

/// What the player can still read about their decompile attempts, in the
/// order the log holds it.
fn ice_holds_lines(game: &Game) -> Vec<String> {
    game.battle_log()
        .into_iter()
        .filter(|line| line.text.contains("ICE holds"))
        .map(|line| line.text)
        .collect()
}

/// The counter is per program, not per player — otherwise softening up one
/// group would quietly discount every other fight on screen.
#[test]
fn attempts_against_one_group_do_not_help_against_another() {
    let mut game = Game::new(3110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    // Two species rather than two programs: `group_pack` partitions by
    // species, so a second drone would join the first one's group and there
    // would be no other group to read.
    let species: Vec<_> = game
        .species_defs()
        .into_iter()
        .filter(|s| !s.is_boss)
        .take(2)
        .map(|s| s.id.to_string())
        .collect();
    let first = spawn_wild_without_routine(&mut game, &species[0], pos.x, pos.y);
    let second = spawn_wild_without_routine(&mut game, &species[1], pos.x, pos.y);
    // Hand-built rather than through `insert_battle`, which goes via
    // `group_pack` — that caps at `enemy_group_ceiling()`, one group at
    // zone 1, and would drop the second group this test is entirely about.
    let groups: Vec<battle::EnemyGroup> = [first, second]
        .iter()
        .map(|&e| battle::EnemyGroup {
            species: game.world.get::<Creature>(e).unwrap().species.clone(),
            members: vec![e],
        })
        .collect();
    let slots = game.world.resource::<Party>().0.len() + 1;
    game.world.insert_resource(BattleState {
        player,
        round_targets: groups.iter().map(|g| g.members.clone()).collect(),
        groups,
        round: 1,
        planned: vec![None; slots],
        finished: false,
        player_won: false,
        decompile_attempts: std::collections::HashMap::new(),
        rewards: BattleRewards::default(),
        lair: None,
        outmatched: false,
    });
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);

    // Each group is measured against its own baseline: they are different
    // species, so their odds start at different places.
    let worked_before = quoted_odds(&game, 0);
    let untouched_before = quoted_odds(&game, 1);
    set_attempts(&mut game, first, 3);

    assert!(
        quoted_odds(&game, 0) > worked_before,
        "the group that was worked on should read easier: {} vs {worked_before}",
        quoted_odds(&game, 0)
    );
    assert_eq!(
        quoted_odds(&game, 1),
        untouched_before,
        "the group nobody touched must read exactly as it did"
    );
}

/// The counter lives on `BattleState`, so walking away from a fight throws
/// it out — this is "you are wearing its ICE down", not a run-wide pity
/// meter that a player could bank against a later target.
#[test]
fn the_attempt_counter_does_not_survive_the_battle() {
    let mut game = Game::new(3111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);

    let fresh = quoted_odds(&game, 0);
    set_attempts(&mut game, wild, DECOMPILE_ATTEMPT_BONUS_CAP);
    assert!(quoted_odds(&game, 0) > fresh, "the fixture should take");

    flee_until_clear(&mut game);
    game.start_battle(vec![wild]);

    assert_eq!(
        quoted_odds(&game, 0),
        fresh,
        "re-engaging the same program must meet it with its ICE intact"
    );
}

#[test]
fn a_failed_decompile_says_the_ice_is_fraying() {
    let mut game = Game::new(3112, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
    game.world
        .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(SEED_THAT_FAILS)));

    game.attempt_decompile(0, player);
    assert!(
        game.world.get::<Hostile>(wild).is_some(),
        "the seeded roll must fail for this test to be about anything"
    );
    assert!(
        log_text(&game).iter().any(|l| l.contains("fray a little")),
        "a failure below the cap should say the attempt bought something: {:?}",
        log_text(&game)
    );
}

/// Reaching the cap has to be *said*, not just silently stop helping —
/// otherwise a player reads a number that stopped moving and keeps paying
/// catalysts into a wall.
#[test]
fn a_failed_decompile_at_the_cap_says_persistence_has_run_out() {
    let mut game = Game::new(3113, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
    set_attempts(&mut game, wild, DECOMPILE_ATTEMPT_BONUS_CAP - 1);
    game.world
        .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(SEED_THAT_FAILS)));

    game.attempt_decompile(0, player);
    assert!(
        game.world.get::<Hostile>(wild).is_some(),
        "the seeded roll must fail for this test to be about anything"
    );
    let lines = log_text(&game);
    assert!(
        lines
            .iter()
            .any(|l| l.contains("as frayed as they will get")),
        "the attempt that reaches the cap should say so: {lines:?}"
    );
}

/// Rewrites a creature to a chosen `Stats::power` at full Integrity. How the
/// total splits across the three stats is arbitrary — only the sum reaches
/// `inspection::power_ratio`.
fn set_power_at_full_integrity(game: &mut Game, entity: Entity, power: i32) {
    let mut stats = game.world.get_mut::<Stats>(entity).unwrap();
    stats.atk = 1;
    stats.mitigation = 1;
    stats.max_hp = power - 2;
    stats.hp = stats.max_hp;
}

/// The power gap has to reach the *screen*, not just the roll: a player who
/// cannot see that a trivial program is now worth a catalyst will not spend
/// one. Both readings here go through `Game::target_resistance`, which is
/// what makes the quote and the roll the same number by construction.
///
/// Integrity stays full on both sides deliberately. That is the whole case
/// this term exists for — a program you delete in one strike can never be
/// presented at low HP, so the old formula priced every attempt on it as
/// though the player had simply declined to soften it.
#[test]
fn a_program_far_beneath_you_previews_better_odds_than_an_even_match() {
    let mut game = Game::new(3108, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 1)]);
    let player_power = game.player_power();

    set_power_at_full_integrity(&mut game, wild, player_power);
    let even = program_manifest(&game, wild)
        .decompile_chance
        .expect("holding a catalyst should quote odds");

    set_power_at_full_integrity(&mut game, wild, player_power / 4);
    let outclassed = program_manifest(&game, wild)
        .decompile_chance
        .expect("holding a catalyst should quote odds");

    assert!(
        outclassed > even * 1.5,
        "a program at a quarter of the player's power should preview \
         markedly better odds at the same full Integrity: {outclassed} vs {even}"
    );
}

// ---------------------------------------------------------------------------
// The onboarding chain's decompile mission
// ---------------------------------------------------------------------------

/// A fight the seeded stream would lose: `SEED_THAT_FAILS`' first draw comes
/// up false, and the target is untouched with no decompiler investment.
fn hopeless_decompile(seed: u32, dir: &std::path::Path) -> (Game, Entity) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, dir).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
    game.world
        .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(SEED_THAT_FAILS)));
    (game, wild)
}

fn catalysts_held(game: &Game) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .items
        .iter()
        .find(|(id, _)| id.as_str() == ids::ICE_BREAKER)
        .map(|(_, n)| *n)
        .unwrap_or(0)
}

/// The chain's decompile mission cannot be failed — a run of bad rolls would
/// end onboarding permanently, which is the one thing an unbreakable chain
/// must not do. The catalyst is still spent, so the lesson that decompiling
/// is priced in catalysts still lands.
#[test]
fn the_chains_decompile_mission_cannot_be_failed() {
    let dir = assets_with_decompile_mission("forced_decompile");
    let (mut game, wild) = hopeless_decompile(3201, &dir);
    let player = game.player_entity();
    let before = catalysts_held(&game);

    game.attempt_decompile(0, player);

    assert!(
        game.world.get::<Tamed>(wild).is_some(),
        "the roll is forced while the mission is live"
    );
    assert_eq!(
        catalysts_held(&game),
        before - 1,
        "and the catalyst is still spent — that is the half of the lesson that stays"
    );
}

/// Off the mission the formula is untouched. Without this the forced roll is
/// indistinguishable from having broken `capture_chance`.
#[test]
fn a_decompile_outside_the_mission_still_rolls() {
    let dir = assets_with_decompile_mission("honest_decompile");
    let (mut game, wild) = hopeless_decompile(3202, &dir);
    skip_tutorial(&mut game);
    // The stream is spent by whatever `skip_tutorial` did not do; reseed so
    // the failing draw is the one this attempt takes.
    game.world
        .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(SEED_THAT_FAILS)));
    let player = game.player_entity();

    game.attempt_decompile(0, player);

    assert!(
        game.world.get::<Hostile>(wild).is_some(),
        "hopeless odds still fail when nothing is forcing them"
    );
}

/// A successful decompile writes its deed either way, which is what finishes
/// the mission.
#[test]
fn a_successful_decompile_writes_its_deed() {
    let dir = assets_with_decompile_mission("deed_decompile");
    let (mut game, wild) = hopeless_decompile(3203, &dir);
    let player = game.player_entity();

    game.attempt_decompile(0, player);

    assert!(
        game.world.get::<Tamed>(wild).is_some(),
        "the fixture has to land one"
    );
    assert!(
        game.world
            .resource::<crate::resources::RunFeats>()
            .deeds
            .contains(&crate::contracts::Deed::Tamed)
    );
}
