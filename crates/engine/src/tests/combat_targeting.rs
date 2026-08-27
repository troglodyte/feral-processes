//! Who takes the hit: retaliation, the soft party ranks that bias it,
//! bracing, and how far back an enemy group can reach.

use super::support::*;
use crate::components::{ActiveFieldBuff, BuffSource, FieldBuffKind};
use crate::species::MoveDef;
use crate::tuning::{DEFEND_MITIGATION_BONUS, FRONT_SLOTS, MAX_PARTY_SIZE};
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
        enlist(&mut game, companion);
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
                    mitigation: 0,
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

/// Companions act in their own right — they take a turn, they swing, they
/// soak hits. They used to *also* lend the player a passive tenth of their
/// ATK and DEF on top of that, which double-counted the same body: recruiting
/// paid twice, once in actions and once in the player's own stat line.
/// Removed 2026-08-19. What the player's sheet says is now what the player
/// has, whatever the roster looks like.
///
/// Both stats and both ends of the roster in one test: the ATK half alone
/// passes against a change that only dropped the mitigation term.
#[test]
fn a_roster_no_longer_inflates_the_players_own_attack_or_defense() {
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let alone = (
        game.effective_atk(player),
        game.effective_mitigation(player),
    );

    // Deliberately beefy: under the old tenth-share these were worth a
    // visible double-digit bump, so a lingering term cannot hide in rounding.
    for _ in 0..MAX_PARTY_SIZE {
        let mate = spawn_tamed(&mut game, 10, 200);
        game.world.get_mut::<Stats>(mate).unwrap().mitigation = 40;
        enlist(&mut game, mate);
    }

    assert_eq!(
        game.world.resource::<Party>().0.len(),
        MAX_PARTY_SIZE,
        "the fixture has to actually fill the roster, or it asserts nothing"
    );
    assert_eq!(
        (
            game.effective_atk(player),
            game.effective_mitigation(player)
        ),
        alone,
        "a full roster must leave the player's own effective stats exactly where \
         an empty one does"
    );
}

/// The companion side of the same rule, kept from when the party bonus
/// existed: `wild_retaliate` calls `effective_mitigation` on whichever entity
/// got hit, so a companion's own defense is its raw `Stats` and nothing else.
#[test]
fn a_companions_effective_defense_as_a_retaliation_target_is_its_own_raw_stats() {
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 30);
    game.world.get_mut::<Stats>(a).unwrap().mitigation = 20;
    enlist(&mut game, a);
    let b = spawn_tamed(&mut game, 10, 200);
    enlist(&mut game, b);

    let raw_def = game.world.get::<Stats>(a).unwrap().mitigation;
    assert_eq!(game.effective_mitigation(a), raw_def);
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
        enlist(&mut game, pet);
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
        enlist(&mut game, pet);
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
        enlist(&mut game, pet);
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
    enlist(&mut game, pet);
    let raw_def = game.world.get::<Stats>(pet).unwrap().mitigation;

    game.begin_defend(pet);

    assert_eq!(
        game.effective_mitigation(pet),
        raw_def + DEFEND_MITIGATION_BONUS,
        "a bracing companion must actually gain the DEF, not silently no-op"
    );
}

fn test_field_buff(kind: FieldBuffKind, power: i32) -> ActiveFieldBuff {
    ActiveFieldBuff {
        kind,
        name: "Test Field Buff".to_string(),
        power,
        remaining: 5,
        interval: 1,
        source: BuffSource::Routine,
    }
}

/// A running `Def`/`Atk` field buff has to actually raise the stat it
/// names, or running a routine for one is a no-op dressed up as a choice.
#[test]
fn a_field_buff_raises_the_effective_stat_it_names() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    let raw_def = game.world.get::<Stats>(pet).unwrap().mitigation;
    let raw_atk = game.world.get::<Stats>(pet).unwrap().atk;

    game.arm_field_buff(pet, test_field_buff(FieldBuffKind::Mitigation, 11));
    assert_eq!(game.effective_mitigation(pet), raw_def + 11);

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
    let raw_def = game.world.get::<Stats>(pet).unwrap().mitigation;

    game.arm_buff(
        pet,
        ActiveBuff {
            kind: BuffKind::Mitigation,
            remaining: 3,
            power: 17,
        },
    );
    game.arm_field_buff(pet, test_field_buff(FieldBuffKind::Mitigation, 23));

    assert_eq!(
        game.effective_mitigation(pet),
        raw_def + 17 + 23,
        "the CombatBuff and FieldBuff Def bonuses must both apply, summed"
    );
}

/// The landmine this is guarding against: `is_defending` identifies a
/// brace by sniffing `CombatBuff` for `Def` at exactly `DEFEND_MITIGATION_BONUS`.
/// A field `Def` buff that happens to land on that same power must not be
/// mistaken for a brace — `FieldBuff` exists as a separate component from
/// `CombatBuff` precisely so `is_defending` never has to look at it.
#[test]
fn is_defending_ignores_a_field_def_buff_even_at_the_defend_power() {
    let mut game = Game::new(96, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);

    game.arm_field_buff(
        pet,
        test_field_buff(FieldBuffKind::Mitigation, DEFEND_MITIGATION_BONUS),
    );

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
    // Forced: the point is that a back group with a ranged move gets to
    // swing at all, not that its swing beats the player's evasion.
    force_the_next_attack_to_land(&mut game);
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
    // A carrier spends its round on the routine rather than a move (see
    // `wild_retaliate`), and whether this one rolled a routine is a property
    // of where the shared `GameRng` stream happened to be. The claim here is
    // about *moves*, so the roll is taken out of it.
    game.world.entity_mut(construct).insert(Routines(vec![]));
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
        enlist(&mut game, bracing);
        enlist(&mut game, other);
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

/// Installs a hostile species cloned from a shipped one, carrying at most
/// one move and that one named — the log prints a move's name on every
/// outcome, hit or miss, so a named move is how a swing is attributed to
/// the program that threw it.
///
/// `base_speed` is far outside the shipped roster's 6..14 on purpose: the
/// gap has to exceed `INITIATIVE_DIE` for the acting order to be a fact
/// rather than a seed's opinion. `ranged` throughout, so reach is never the
/// variable under test.
fn install_test_species(
    game: &mut Game,
    id: &str,
    base_speed: i32,
    move_name: Option<&str>,
) -> SpeciesId {
    let template = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    let def = SpeciesDef {
        id: id.to_string(),
        base_speed,
        moves: move_name
            .map(|name| MoveDef {
                name: name.to_string(),
                power: 2,
                spread: 0,
                // No rider: a status line is a second log line from one
                // swing, and the count below is of swings.
                effect: None,
                ranged: true,
            })
            .into_iter()
            .collect(),
        ..template
    };
    let id = def.id.clone();
    game.world.resource_mut::<SpeciesDb>().insert(def);
    id
}

fn spawn_hostile(game: &mut Game, species: &SpeciesId, hp: i32) -> Entity {
    game.world
        .spawn((
            Creature {
                species: species.clone(),
            },
            Hostile,
            Position { x: 0, y: 0 },
            Stats {
                hp,
                max_hp: hp,
                atk: 0,
                mitigation: 0,
            },
            StatusEffects::default(),
        ))
        .id()
}

/// Initiative is rolled once at the top of a round, and `battle::Actor`
/// used to name a hostile by *position* — an index into `BattleState::
/// groups` and into that group's members. A kill mid-round drops the dead
/// member, and an emptied group drops out of `groups`, so every index
/// behind it shifted down one and a stale actor resolved to whoever had
/// moved into its place.
///
/// Both halves of that are here, and a count of swings cannot see either:
/// the shift *conserves* the number of actors, since the last index is the
/// one left resolving to nothing. What it does not conserve is who. Group C
/// swings on its own turn, the player's strike then empties group A, and C
/// slides into B's index — so the attack bleeds from the fallen group into
/// the one behind it and C swings twice, while B, whose own index has moved
/// off the end, loses its round in silence. So each program is counted by
/// the name of the move it threw.
///
/// Swept across seeds rather than pinned to one: the player's strike is
/// capped at `HIT_CHANCE_MAX`, so on the odd seed it misses and group A
/// survives. A surviving A has no move at all, so one swing each is the
/// answer either way and the sweep needs no branch.
#[test]
fn a_group_that_falls_mid_round_neither_lends_nor_steals_a_turn() {
    for seed in 1..12u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // A is the punching bag the player empties; C is faster than the
        // player and B is slower, so A falls after C has swung and before B
        // has.
        let a = install_test_species(&mut game, "test_group_a", 1, None);
        let b = install_test_species(&mut game, "test_group_b", 1, Some("beta lance"));
        let c = install_test_species(&mut game, "test_group_c", 100, Some("gamma lance"));
        let members = [
            spawn_hostile(&mut game, &a, 1),
            spawn_hostile(&mut game, &b, 999),
            spawn_hostile(&mut game, &c, 999),
        ];
        let groups = [a, b, c]
            .into_iter()
            .zip(members)
            .map(|(species, member)| crate::battle::EnemyGroup {
                species,
                members: vec![member],
            })
            .collect();
        insert_battle_with_groups(&mut game, player, groups);

        game.battle_set_action(0, BattleAction::Attack { group: 0 })
            .expect("the player can swing at group A");
        game.battle_resolve_round();

        let lines: Vec<String> = game.message_log(200).into_iter().map(|e| e.text).collect();
        for move_name in ["beta lance", "gamma lance"] {
            let swings = lines.iter().filter(|l| l.contains(move_name)).count();
            assert_eq!(
                swings, 1,
                "seed {seed}: {move_name} was thrown {swings} times in one \
                 round — a group falling mid-round must neither hand its \
                 initiative turn to the group behind it nor take that \
                 group's turn away.\nlog: {lines:#?}"
            );
        }
    }
}
