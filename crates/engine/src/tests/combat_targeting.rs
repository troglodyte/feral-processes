//! Who takes the hit: retaliation, the soft party ranks that bias it,
//! bracing, and how far back an enemy group can reach.

use super::support::*;
use crate::components::{ActiveFieldBuff, BuffSource, FieldBuffKind};
use crate::tuning::{DEFEND_DEF_BONUS, FRONT_SLOTS};
use crate::*;

/// `wild_retaliate` rolls per-call whether a companion soaks the hit, so
/// this drives it across many seeds and checks both outcomes occur —
/// proof the roll is live, not that any single call behaves one way.
#[test]
fn wild_retaliation_can_land_on_either_the_player_or_the_companion() {
    let species_id = {
        let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.species_defs()
            .into_iter()
            .next()
            .expect("at least one species")
            .id
            .clone()
    };

    let mut companion_hit = false;
    let mut player_hit = false;

    for seed in 0..60u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 1000, 1);
        game.add_companion(companion).unwrap();
        let player_hp_before = game.world.get::<Stats>(player).unwrap().hp;

        let wild = game
            .world
            .spawn((
                Creature {
                    species: species_id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 1000,
                    max_hp: 1000,
                    atk: 5,
                    def: 0,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);

        player_attacks(&mut game);

        let companion_hp = game.world.get::<Stats>(companion).unwrap().hp;
        let player_hp_after = game.world.get::<Stats>(player).unwrap().hp;
        if companion_hp < 1000 {
            companion_hit = true;
        }
        if player_hp_after < player_hp_before {
            player_hit = true;
        }
        if companion_hit && player_hit {
            break;
        }
    }

    assert!(
        companion_hit,
        "across 60 battles, the companion should have taken at least one hit"
    );
    assert!(
        player_hit,
        "across 60 battles, the player should have taken at least one hit"
    );
}

#[test]
fn effective_def_excludes_the_players_party_bonus_when_a_companion_is_the_target() {
    // `wild_retaliate` calls `effective_def` on whichever entity got
    // hit — the player, or (per the test above) a companion. The
    // player's passive party bonus (see `party_stat_bonus`) must only
    // ever land on the player, never get double-applied to a
    // companion's own defense just because it's a party member too.
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 30);
    game.world.get_mut::<Stats>(a).unwrap().def = 20;
    game.add_companion(a).unwrap();
    // A second party member gives the *player's* bonus a nonzero,
    // easy-to-notice value if it ever leaked onto `a`.
    let b = spawn_tamed(&mut game, 10, 200);
    game.add_companion(b).unwrap();

    let raw_def = game.world.get::<Stats>(a).unwrap().def;
    assert_eq!(
        game.effective_def(a),
        raw_def,
        "a companion's effective DEF as a retaliation target must be its own raw Stats, \
         not inflated by the player's party bonus"
    );
}

/// Soft ranks, not hard ones: a back-slot member is hit *less*, never
/// *not at all*. Both halves matter — a version that made back slots
/// untouchable would pass a front-heavy assertion just as well, and
/// would quietly turn the roster into a wall of invulnerable reserves.
#[test]
fn back_slot_party_members_draw_less_fire_but_are_still_reachable() {
    let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let mut slots = vec![player];
    for _ in 0..MAX_PARTY_SIZE {
        // Huge HP pools so nobody drops out of the pool mid-sample.
        let pet = spawn_tamed(&mut game, 100_000, 1);
        game.add_companion(pet).unwrap();
        slots.push(pet);
    }
    assert!(
        slots.len() > FRONT_SLOTS,
        "the sample needs at least one back slot to be meaningful"
    );

    let mut hits = vec![0u32; slots.len()];
    for _ in 0..4000 {
        let target = game.roll_enemy_target(player);
        let idx = slots.iter().position(|&e| e == target).unwrap();
        hits[idx] += 1;
    }

    let (front, back) = hits.split_at(FRONT_SLOTS);
    assert!(
        back.iter().all(|&h| h > 0),
        "every back slot must still be reachable, got {hits:?}"
    );
    let front_min = *front.iter().min().unwrap();
    let back_max = *back.iter().max().unwrap();
    assert!(
        front_min > back_max,
        "every front slot should outdraw every back slot, got {hits:?}"
    );
}

/// Bracing draws fire — that is what makes Defend a party-level play
/// rather than a selfish one.
#[test]
fn a_bracing_member_draws_more_fire_than_it_otherwise_would() {
    let sample = |brace: bool| {
        let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let pet = spawn_tamed(&mut game, 100_000, 1);
        game.add_companion(pet).unwrap();
        if brace {
            game.begin_defend(pet);
        }
        (0..4000)
            .filter(|_| game.roll_enemy_target(player) == pet)
            .count()
    };
    assert!(
        sample(true) > sample(false),
        "a bracing companion must take more of the incoming fire"
    );
}

/// Party order is mechanically meaningful under soft ranks — front
/// slots draw more fire — so it has to survive a save/load round trip.
/// The roster order here deliberately differs from spawn order, which
/// is what the party used to be rebuilt from.
#[test]
fn party_order_survives_a_save_load_round_trip() {
    let dir = std::env::temp_dir().join("feral_party_order_roundtrip");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Distinct max HP is the identity that has to come back in order.
    let a = spawn_tamed(&mut game, 31, 3);
    let b = spawn_tamed(&mut game, 47, 3);
    let c = spawn_tamed(&mut game, 53, 3);
    for pet in [c, a, b] {
        game.add_companion(pet).unwrap();
    }
    let path = dir.join("slot.sav");
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let order: Vec<i32> = loaded
        .world
        .resource::<Party>()
        .0
        .iter()
        .filter_map(|&e| loaded.world.get::<Stats>(e).map(|s| s.max_hp))
        .collect();
    assert_eq!(
        order,
        vec![53, 31, 47],
        "party order must round-trip exactly, not fall back to spawn order"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Defend has to actually reduce incoming damage, or it's a wasted turn
/// dressed up as a choice. Same seed both times: neither Defend nor the
/// player's flat strike draws from the RNG, so the two runs stay in
/// lockstep and the only difference is the DEF bonus.
#[test]
fn defending_reduces_the_damage_a_party_member_takes_this_round() {
    let damage_taken = |defend: bool| {
        let mut game = Game::new(89, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let wild = game.spawn_wild_creature("scrapper", 5, 5).unwrap();
        game.start_battle(vec![wild]);
        let player = game.player_entity();
        let before = game.world.get::<Stats>(player).unwrap().hp;
        game.battle_set_action(
            0,
            if defend {
                BattleAction::Defend
            } else {
                BattleAction::Attack { group: 0 }
            },
        )
        .unwrap();
        game.battle_resolve_round();
        before - game.world.get::<Stats>(player).unwrap().hp
    };
    assert!(
        damage_taken(true) < damage_taken(false),
        "a defended round must cost less HP than an undefended one"
    );
}

/// Defend is offered to companions, so a companion must be able to hold
/// the buff it grants. Only the player is spawned carrying a buff slot,
/// so without inserting one on demand a companion's Defend would log
/// its message and change nothing.
#[test]
fn a_companion_can_hold_the_buff_defend_grants() {
    let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    game.add_companion(pet).unwrap();
    let raw_def = game.world.get::<Stats>(pet).unwrap().def;

    game.begin_defend(pet);

    assert_eq!(
        game.effective_def(pet),
        raw_def + DEFEND_DEF_BONUS,
        "a bracing companion must actually gain the DEF, not silently no-op"
    );
}

fn test_field_buff(kind: FieldBuffKind, power: i32) -> ActiveFieldBuff {
    ActiveFieldBuff {
        kind,
        name: "Test Field Buff".to_string(),
        power,
        remaining: 5,
        source: BuffSource::Routine,
    }
}

/// A running `Def`/`Atk` field buff has to actually raise the stat it
/// names, or casting a routine for one is a no-op dressed up as a choice.
#[test]
fn a_field_buff_raises_the_effective_stat_it_names() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    let raw_def = game.world.get::<Stats>(pet).unwrap().def;
    let raw_atk = game.world.get::<Stats>(pet).unwrap().atk;

    game.arm_field_buff(pet, test_field_buff(FieldBuffKind::Def, 11));
    assert_eq!(game.effective_def(pet), raw_def + 11);

    game.arm_field_buff(pet, test_field_buff(FieldBuffKind::Atk, 13));
    assert_eq!(game.effective_atk(pet), raw_atk + 13);
}

/// A field `Def` buff and a `CombatBuff` Def bonus (e.g. from Defend) are
/// two separate sources and both apply — `arm_field_buff` was built as a
/// distinct component from `CombatBuff` specifically so the two can
/// coexist. Distinct, non-round-numbered powers so no other formula could
/// produce the same total by coincidence.
#[test]
fn a_field_buff_stacks_with_a_combat_buff_of_the_same_kind() {
    let mut game = Game::new(95, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    let raw_def = game.world.get::<Stats>(pet).unwrap().def;

    game.arm_buff(
        pet,
        ActiveBuff {
            kind: BuffKind::Def,
            remaining: 3,
            power: 17,
        },
    );
    game.arm_field_buff(pet, test_field_buff(FieldBuffKind::Def, 23));

    assert_eq!(
        game.effective_def(pet),
        raw_def + 17 + 23,
        "the CombatBuff and FieldBuff Def bonuses must both apply, summed"
    );
}

/// The landmine this is guarding against: `is_defending` identifies a
/// brace by sniffing `CombatBuff` for `Def` at exactly `DEFEND_DEF_BONUS`.
/// A field `Def` buff that happens to land on that same power must not be
/// mistaken for a brace — `FieldBuff` exists as a separate component from
/// `CombatBuff` precisely so `is_defending` never has to look at it.
#[test]
fn is_defending_ignores_a_field_def_buff_even_at_the_defend_power() {
    let mut game = Game::new(96, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);

    game.arm_field_buff(pet, test_field_buff(FieldBuffKind::Def, DEFEND_DEF_BONUS));

    assert!(
        !game.is_defending(pet),
        "a field Def buff, even at exactly the Defend power, must not read as bracing"
    );
}

/// Each companion's own field buff affects only that companion — proof
/// `FieldBuff` is per-entity rather than something that leaks across the
/// party the way the player's passive party bonus does not.
#[test]
fn a_companions_own_field_buff_affects_only_that_companions_stat() {
    let mut game = Game::new(97, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let buffed = spawn_tamed(&mut game, 30, 5);
    let other = spawn_tamed(&mut game, 30, 5);
    let raw_atk = game.world.get::<Stats>(buffed).unwrap().atk;
    let other_raw_atk = game.world.get::<Stats>(other).unwrap().atk;

    game.arm_field_buff(buffed, test_field_buff(FieldBuffKind::Atk, 9));

    assert_eq!(game.effective_atk(buffed), raw_atk + 9);
    assert_eq!(
        game.effective_atk(other),
        other_raw_atk,
        "an unbuffed companion must be untouched by another companion's field buff"
    );
}

/// The reach rule is the balance valve that makes a big multi-group
/// fight survivable. A back group with only melee moves can't connect
/// at all.
#[test]
fn a_back_group_with_only_melee_moves_cannot_reach_the_party() {
    let mut game = Game::new(86, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Scrapper, Sentinel and Construct are authored melee-only.
    let (x, y) = multi_group_ground(&mut game);
    let a = game.spawn_wild_creature("scrapper", x, y).unwrap();
    let b = game.spawn_wild_creature("sentinel", x, y + 1).unwrap();
    let c = game.spawn_wild_creature("construct", x, y + 2).unwrap();
    game.start_battle(vec![a, b, c]);
    let player = game.player_entity();
    let hp_before = game.world.get::<Stats>(player).unwrap().hp;

    // Group 2 (Construct) is behind the engaged pair and melee-only.
    let construct = game.front_of_group(2).unwrap();
    for _ in 0..20 {
        game.wild_retaliate(construct, 2, player);
    }

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        hp_before,
        "a melee-only back group must deal no damage"
    );
}

/// ...but a back group holding a ranged move connects normally. Without
/// this half, the test above would pass just as well against a bug that
/// makes back groups unconditionally inert.
#[test]
fn a_back_group_with_a_ranged_move_still_connects() {
    let mut game = Game::new(87, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (x, y) = multi_group_ground(&mut game);
    let a = game.spawn_wild_creature("scrapper", x, y).unwrap();
    let b = game.spawn_wild_creature("sentinel", x, y + 1).unwrap();
    // Glitch's "Static Burst" is authored ranged.
    let c = game.spawn_wild_creature("glitch", x, y + 2).unwrap();
    game.start_battle(vec![a, b, c]);
    let player = game.player_entity();
    let hp_before = game.world.get::<Stats>(player).unwrap().hp;

    let glitch = game.front_of_group(2).unwrap();
    game.wild_retaliate(glitch, 2, player);

    assert!(
        game.world.get::<Stats>(player).unwrap().hp < hp_before,
        "a ranged back group must be able to land a hit"
    );
}

/// An engaged group picks from its whole moveset, ranged or not —
/// the restriction is about distance, not about the moves themselves.
#[test]
fn an_engaged_group_still_uses_its_melee_moves() {
    let mut game = Game::new(88, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let construct = game.spawn_wild_creature("construct", 5, 5).unwrap();
    game.start_battle(vec![construct]);
    let player = game.player_entity();
    let hp_before = game.world.get::<Stats>(player).unwrap().hp;

    game.wild_retaliate(construct, 0, player);

    assert!(
        game.world.get::<Stats>(player).unwrap().hp < hp_before,
        "a melee-only species in the front rank must still hit"
    );
}

/// The mirror: "ally" means the user's own side, whichever side that is.
#[test]
fn a_hostile_ally_target_resolves_to_its_own_side() {
    let mut game = Game::new(6601, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 3, 100);

    let recipients = game.ability_recipients(
        enemies[0],
        crate::abilities::AbilityTarget::WholeParty,
        &battle::SpecialTarget::WholeParty,
    );
    assert_eq!(
        recipients.len(),
        3,
        "a hostile's 'whole party' is its own side"
    );
    assert!(
        !recipients.contains(&player),
        "and never reaches across to the player"
    );
    for e in &enemies {
        assert!(recipients.contains(e));
    }
}

#[test]
fn a_hostile_one_ally_target_picks_one_of_its_own() {
    let mut game = Game::new(6602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 3, 100);

    let recipients = game.ability_recipients(
        enemies[0],
        crate::abilities::AbilityTarget::OneAlly,
        &battle::SpecialTarget::WholeParty,
    );
    assert_eq!(recipients.len(), 1, "exactly one recipient");
    assert!(
        enemies.contains(&recipients[0]) && recipients[0] != player,
        "and it is one of the hostiles, not the player"
    );
}

/// `WholeEnemyGroup` and `AllEnemies` collapse for a hostile actor: the
/// player has one party where the hostiles have groups, and there is no
/// player-side subdivision to select.
#[test]
fn both_hostile_area_enemy_targets_resolve_to_the_whole_player_party() {
    let mut game = Game::new(6603, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 2, 100);

    let group = game.ability_recipients(
        enemies[0],
        crate::abilities::AbilityTarget::WholeEnemyGroup,
        &battle::SpecialTarget::EnemyGroup { group: 0 },
    );
    let all = game.ability_recipients(
        enemies[0],
        crate::abilities::AbilityTarget::AllEnemies,
        &battle::SpecialTarget::AllEnemies,
    );
    assert_eq!(group, all, "the two collapse for a hostile actor");
    assert!(group.contains(&player));
    for e in &enemies {
        assert!(
            !group.contains(e),
            "a hostile area attack never hits its own side"
        );
    }
}

#[test]
fn a_hostile_single_enemy_target_hits_exactly_one_party_member() {
    let mut game = Game::new(6604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 100);

    // `chosen` is `Ally { slot }` here, not `EnemyGroup` — `wild_retaliate` is
    // what resolves a hostile's single-target routine now (via
    // `roll_enemy_target`, aggro-weighted) and hands the slot down; this call
    // stands in for that resolved slot rather than re-deriving it.
    let recipients = game.ability_recipients(
        enemies[0],
        crate::abilities::AbilityTarget::OneEnemyGroupFront,
        &battle::SpecialTarget::Ally { slot: 0 },
    );
    assert_eq!(recipients.len(), 1);
    assert!(
        !enemies.contains(&recipients[0]),
        "it aims at the party, not at itself"
    );
    assert_eq!(
        recipients[0], player,
        "with only the player in the party, it is the player"
    );
}

/// Spec §4: a hostile's `OneEnemyGroupFront` routine resolves through
/// `roll_enemy_target`, the same aggro-weighted roll a wild *move* uses — so
/// slot order and bracing still matter. Before the fix, `wild_retaliate`
/// hardcoded `chosen` as `SpecialTarget::EnemyGroup { group }`, which made
/// the hostile branch of `ability_recipients` fall through to
/// `living_party().take(1)` — always slot 0, the player, no matter who was
/// bracing or how the party was arranged.
#[test]
fn a_hostile_single_target_routine_is_aggro_weighted_across_the_party() {
    let mut player_hit = false;
    let mut bracing_companion_hit = false;
    let mut other_companion_hit = false;

    for seed in 0..200u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let bracing = spawn_tamed(&mut game, 1000, 1);
        let other = spawn_tamed(&mut game, 1000, 1);
        game.add_companion(bracing).unwrap();
        game.add_companion(other).unwrap();
        let player_hp_before = game.world.get::<Stats>(player).unwrap().hp;

        let enemies = battle_with_a_pack_of(&mut game, 1, 200);
        // Packet Shred Single: OneEnemyGroupFront, flat Damage — HP loss identifies
        // who it landed on without depending on a `StatusEffects` component
        // the test's own companions don't carry.
        game.world
            .entity_mut(enemies[0])
            .insert(Routines(vec!["kernel_panic".to_string()]));
        // Bracing draws extra aggro weight — see `battle::slot_aggro_weight`.
        game.begin_defend(bracing);

        game.wild_retaliate(enemies[0], 0, player);

        let hp_dropped = |e: Entity, before: i32| game.world.get::<Stats>(e).unwrap().hp < before;
        player_hit |= hp_dropped(player, player_hp_before);
        bracing_companion_hit |= hp_dropped(bracing, 1000);
        other_companion_hit |= hp_dropped(other, 1000);
    }

    assert!(player_hit, "the player must still be a possible target");
    assert!(
        bracing_companion_hit || other_companion_hit,
        "a hostile's single-target routine must be able to land on a companion — before the fix \
         it always resolved to slot 0 (the player), ignoring bracing and slot order entirely"
    );
}

/// The player side is unchanged by any of this.
#[test]
fn the_players_side_targets_exactly_as_it_did() {
    let mut game = Game::new(6605, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let _ = battle_with_a_pack_of(&mut game, 3, 100);

    let recipients = game.ability_recipients(
        player,
        crate::abilities::AbilityTarget::WholeEnemyGroup,
        &battle::SpecialTarget::EnemyGroup { group: 0 },
    );
    assert_eq!(
        recipients.len(),
        3,
        "the player's group attack still hits the group"
    );
    assert!(!recipients.contains(&player));
}
