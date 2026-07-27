//! Who takes the hit: retaliation, the soft party ranks that bias it,
//! bracing, and how far back an enemy group can reach.

use super::support::*;
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

/// The reach rule is the balance valve that makes a big multi-group
/// fight survivable. A back group with only melee moves can't connect
/// at all.
#[test]
fn a_back_group_with_only_melee_moves_cannot_reach_the_party() {
    let mut game = Game::new(86, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Scrapper, Sentinel and Construct are authored melee-only.
    let (x, y) = multi_group_ground(&game);
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
    let (x, y) = multi_group_ground(&game);
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
