//! A body nothing can address: the five doors that name one, the three that
//! break a cloak, and the omissions that are the whole feature.
//!
//! `components::Cloaked` moves no stat, so nothing here measures damage for
//! its own sake — every assertion is about *who a picker named*.

use super::support::*;
use crate::abilities::{AbilityDef, AbilityEffect, AbilityShape, AbilityTarget};
use crate::components::Cloaked;
use crate::policy;
use crate::resources::EnemyPolicy;
use crate::tactical::TacticalBattle;
use crate::tactical::reach;
use crate::tactical::turn::StepOutcome;
use crate::*;

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// The shipped routine this whole feature ships for.
fn detach(game: &Game) -> AbilityDef {
    ability(game, "detach")
}

/// A hostile standing on the player's tile, in a fight with them.
fn abstract_fight(game: &mut Game) -> Entity {
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    let wild = spawn_wild_without_routine(game, "scrapper", pos.x, pos.y);
    insert_battle(game, player, vec![wild]);
    wild
}

/// A single hostile group two members deep, with the fight opened around it.
///
/// **`insert_battle_with_groups`, never `insert_battle`.** `group_pack` reads
/// the local `max_group_size`, which at a zone-1 spawn point is one member —
/// so the rolled partition would drop the body this fixture exists to stand
/// behind the front.
fn two_deep_group(game: &mut Game, player: Entity, at: Position) -> (Entity, Entity) {
    let front = spawn_wild_without_routine(game, "scrapper", at.x, at.y);
    let behind = spawn_wild_without_routine(game, "scrapper", at.x, at.y);
    let species = game.world.get::<Creature>(front).unwrap().species.clone();
    insert_battle_with_groups(
        game,
        player,
        vec![crate::battle::EnemyGroup {
            species,
            members: vec![front, behind],
        }],
    );
    (front, behind)
}

fn is_cloaked(game: &Game, entity: Entity) -> bool {
    game.world.get::<Cloaked>(entity).is_some()
}

// ─────────────────────────────────────────────────────────────────────────
// The five doors that name a body.
// ─────────────────────────────────────────────────────────────────────────

/// The baseline roll. Driven over many draws rather than one, because
/// `roll_enemy_target` is weighted and a single miss proves nothing.
#[test]
fn a_cloaked_party_member_leaves_the_weighted_pool_and_comes_back_when_exposed() {
    let mut game = game(701);
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 100_000, 1);
    enlist(&mut game, pet);
    abstract_fight(&mut game);

    game.arm_cloak(pet, 3);
    let while_hidden = (0..2000)
        .filter(|_| game.roll_enemy_target(player) == pet)
        .count();
    assert_eq!(
        while_hidden, 0,
        "a cloaked companion was named {while_hidden} times in 2000 rolls"
    );

    game.break_cloak(pet);
    let once_exposed = (0..2000)
        .filter(|_| game.roll_enemy_target(player) == pet)
        .count();
    assert!(
        once_exposed > 0,
        "an exposed companion never came back into the pool"
    );
}

/// The trained policy's half of the same door. `living_targets` is private,
/// so this reads it the way the game does — through `choose_wild_action`,
/// with weights installed so the scored branch is the one taken.
#[test]
fn a_cloaked_party_member_leaves_the_trained_policys_target_list() {
    let mut game = game(702);
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 100_000, 1);
    enlist(&mut game, pet);
    let wild = abstract_fight(&mut game);

    let (weights, warnings) =
        policy::PolicyWeights::from_pairs(&[("target_hp_frac".into(), 1.0)]).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    game.world.insert_resource(EnemyPolicy(Some(weights)));

    game.arm_cloak(pet, 5);
    let while_hidden = (0..400)
        .filter_map(|_| game.choose_wild_action(wild, 0, player))
        .filter(|(_, target)| *target == pet)
        .count();
    assert_eq!(
        while_hidden, 0,
        "the policy aimed at a cloaked companion {while_hidden} times"
    );

    game.break_cloak(pet);
    let once_exposed = (0..400)
        .filter_map(|_| game.choose_wild_action(wild, 0, player))
        .filter(|(_, target)| *target == pet)
        .count();
    assert!(
        once_exposed > 0,
        "an exposed companion never came back into the policy's list"
    );
}

/// `OneEnemyGroupFront` reaches the front of a group, so a cloaked front
/// hands the pick to whoever is standing behind it.
#[test]
fn front_of_group_skips_a_cloaked_member_and_answers_the_one_behind_it() {
    let mut game = game(703);
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    let (front, behind) = two_deep_group(&mut game, player, pos);

    game.arm_cloak(front, 3);
    assert_eq!(game.front_of_group(0), Some(behind));

    game.break_cloak(front);
    assert_eq!(game.front_of_group(0), Some(front));
}

/// The never-empty rule at the door that would otherwise refuse the player
/// their single-target attacks outright.
#[test]
fn a_wholly_cloaked_group_still_answers_a_front() {
    let mut game = game(704);
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    let (front, behind) = two_deep_group(&mut game, player, pos);

    game.arm_cloak(front, 3);
    game.arm_cloak(behind, 3);
    assert_eq!(
        game.front_of_group(0),
        Some(front),
        "an all-cloaked group answered nothing, so a single-target attack has no target"
    );
}

/// The never-empty rule at the door whose fallback returns the player
/// specifically — without it an all-cloaked party puts every blow on them.
#[test]
fn a_wholly_cloaked_party_still_rolls_a_real_weighted_pick() {
    let mut game = game(705);
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 100_000, 1);
    enlist(&mut game, pet);
    abstract_fight(&mut game);

    game.arm_cloak(player, 3);
    game.arm_cloak(pet, 3);
    let on_the_pet = (0..2000)
        .filter(|_| game.roll_enemy_target(player) == pet)
        .count();
    assert!(
        on_the_pet > 0,
        "with everyone cloaked the roll fell through to the player every time, \
         which is the `total == 0` fallback rather than a weighted pick"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// The three callers of `break_cloak`.
// ─────────────────────────────────────────────────────────────────────────

/// The attacker's own cloak, through `resolve_and_apply_attack` — every
/// creature-versus-creature swing in both models and in a sortie.
///
/// **The swing itself, not a whole round.** Driven through `player_attacks`
/// this passes with the hook deleted: the wild side retaliates in the same
/// round, its blow lands on the player, and `apply_damage`'s hook — a
/// different one — takes the cloak. One call is what isolates the caller
/// under test, and here the damage lands on the *other* body, so nothing
/// but the attacker hook can reveal the swinger.
#[test]
fn swinging_breaks_the_swingers_cloak() {
    let mut game = game(706);
    let player = game.player_entity();
    let wild = abstract_fight(&mut game);
    let hp_before = game.world.get::<Stats>(wild).unwrap().hp;

    game.arm_cloak(player, 5);
    force_the_next_attack_to_land(&mut game);
    game.resolve_and_apply_attack(
        player,
        wild,
        battle::Swing::plain(battle::DamageRange::centred(6, 0)),
    );

    assert!(
        game.world.get::<Stats>(wild).unwrap().hp < hp_before,
        "the fixture must actually land, or this measures a miss"
    );
    assert!(
        !is_cloaked(&game, player),
        "committing to a swing is the aggressive act"
    );
}

/// The actor's, through `use_ability` — the hook that catches the two
/// effects which deal no damage and so reach no other one.
#[test]
fn running_a_debuff_breaks_the_invokers_cloak_though_it_deals_no_damage() {
    let mut game = game(707);
    let player = game.player_entity();
    let wild = abstract_fight(&mut game);
    let hp_before = game.world.get::<Stats>(wild).unwrap().hp;

    let debuff = AbilityDef {
        effect: AbilityEffect::Debuff {
            kind: StatusKind::Bleed,
            power: 1,
            duration: 2,
        },
        ..detach(&game)
    };
    game.arm_cloak(player, 5);
    game.use_ability(&debuff, player, "You", &[wild]);

    assert_eq!(
        game.world.get::<Stats>(wild).unwrap().hp,
        hp_before,
        "the fixture must deal no damage, or this proves the wrong hook"
    );
    assert!(!is_cloaked(&game, player));
}

/// The target's, through `apply_damage` — "an area attack connected", the
/// only hook that names the body on the receiving end.
#[test]
fn an_area_routine_that_lands_breaks_the_targets_cloak() {
    let mut game = game(708);
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    let a = spawn_wild_without_routine(&mut game, "scrapper", pos.x, pos.y);
    let b = spawn_wild_without_routine(&mut game, "scrapper", pos.x, pos.y);
    insert_battle(&mut game, player, vec![a, b]);

    let blast = AbilityDef {
        target: AbilityTarget::AllEnemies,
        effect: AbilityEffect::Damage {
            power: 400,
            spread: 0,
            status: None,
        },
        accuracy: 500,
        ..detach(&game)
    };
    game.arm_cloak(a, 5);
    game.arm_cloak(b, 5);
    let hp_before = (
        game.world.get::<Stats>(a).unwrap().hp,
        game.world.get::<Stats>(b).unwrap().hp,
    );
    game.use_ability(&blast, player, "You", &[a, b]);

    let hp_after = (
        game.world.get::<Stats>(a).unwrap().hp,
        game.world.get::<Stats>(b).unwrap().hp,
    );
    assert!(
        hp_after.0 < hp_before.0 && hp_after.1 < hp_before.1,
        "the fixture must land on both, or a standing cloak proves nothing"
    );
    assert!(
        !is_cloaked(&game, a),
        "the first body stayed hidden after taking a hit"
    );
    assert!(
        !is_cloaked(&game, b),
        "the second body stayed hidden after taking a hit"
    );
}

/// The one line, once. A `Damage` routine reaches the attacker hook inside
/// `resolve_and_apply_attack` *and* the actor hook at the tail of
/// `use_ability`; `break_cloak` logs on the transition, so the second is a
/// no-op rather than a second reveal.
#[test]
fn breaking_a_cloak_is_announced_once_and_only_on_the_transition() {
    let mut game = game(709);
    let player = game.player_entity();
    let wild = abstract_fight(&mut game);
    game.world
        .resource_mut::<MessageLog>()
        .keep_battle_narration = true;

    let swipe = AbilityDef {
        target: AbilityTarget::AllEnemies,
        effect: AbilityEffect::Damage {
            power: 6,
            spread: 1,
            status: None,
        },
        accuracy: 500,
        ..detach(&game)
    };
    game.arm_cloak(player, 5);
    force_the_next_attack_to_land(&mut game);
    game.use_ability(&swipe, player, "You", &[wild]);
    game.break_cloak(player);

    let reveals = game
        .world
        .resource::<MessageLog>()
        .lines
        .iter()
        .filter(|l| l.text.contains("is exposed"))
        .count();
    assert_eq!(reveals, 1, "the reveal was logged {reveals} times");
}

// ─────────────────────────────────────────────────────────────────────────
// The omissions, which are the feature.
// ─────────────────────────────────────────────────────────────────────────

/// Moving is not aggression, in the one model where moving is a thing a
/// body does.
#[test]
fn a_tactical_step_leaves_a_cloak_standing() {
    let mut game = game(710);
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player));

    game.arm_cloak(player, 5);
    let moved = [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .any(|dir| game.tactical_step(dir) == StepOutcome::Moved);
    assert!(moved, "the player could not take a single step");
    assert!(is_cloaked(&game, player), "walking revealed the walker");
}

/// Bracing is not aggression either — and this is the case a `CombatBuff`
/// implementation would have got wrong, since a brace overwrites whatever
/// that component was holding.
#[test]
fn bracing_leaves_a_cloak_standing() {
    let mut game = game(711);
    let player = game.player_entity();
    abstract_fight(&mut game);

    game.arm_cloak(player, 5);
    game.begin_defend(player);
    assert!(is_cloaked(&game, player));
    assert!(
        game.is_defending(player),
        "the brace itself has to have taken, or this proves nothing"
    );
}

/// Tending your own side is not aggression.
#[test]
fn healing_an_ally_leaves_the_healers_cloak_standing() {
    let mut game = game(712);
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 60, 1);
    enlist(&mut game, pet);
    game.world.get_mut::<Stats>(pet).unwrap().hp = 10;
    abstract_fight(&mut game);

    let patch = ability(&game, "checksum_repair");
    assert!(
        !patch.effect.breaks_cloak(),
        "the fixture routine must be one of the peaceful ones"
    );
    game.arm_cloak(player, 5);
    game.use_ability(&patch, player, "You", &[pet]);
    assert!(is_cloaked(&game, player));
}

/// **The asymmetric case.** Only landed damage reaches `apply_damage`, so a
/// swing that missed leaves the defender's cloak standing — you cannot flush
/// one out by swinging at where you guess it is. The attacker's own breaks
/// all the same, which is the other half of the same rule.
#[test]
fn a_swing_that_missed_leaves_the_defenders_cloak_standing() {
    let mut game = game(713);
    let player = game.player_entity();
    let wild = abstract_fight(&mut game);

    game.arm_cloak(wild, 5);
    // The swinger's own, so the second assertion below is not vacuous: the
    // whole point of the pair is that a miss breaks one and not the other.
    game.arm_cloak(player, 5);
    force_the_next_attack_to_miss(&mut game);
    let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
    let outcome = game.resolve_and_apply_attack(
        player,
        wild,
        battle::Swing::plain(battle::DamageRange::centred(4, 0)),
    );
    assert_eq!(
        game.world.get::<Stats>(wild).unwrap().hp,
        hp_before,
        "the fixture must actually miss, or this proves nothing: {outcome:?}"
    );
    assert!(
        is_cloaked(&game, wild),
        "a swing that landed nothing revealed the body it was aimed at"
    );
    assert!(
        !is_cloaked(&game, player),
        "the swinger stayed hidden after committing to a swing"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// The round cap, in both models.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn the_cap_expires_a_cloak_with_no_action_taken_in_the_group_model() {
    let mut game = game(714);
    let player = game.player_entity();
    abstract_fight(&mut game);

    game.arm_cloak(player, 2);
    game.tick_combatant_upkeep(player);
    assert!(is_cloaked(&game, player), "one round ended it early");
    game.tick_combatant_upkeep(player);
    assert!(!is_cloaked(&game, player), "the cap never came due");
}

/// The half that proves `tick_one_combatant` is reached from a battle map's
/// own round rather than only from the group model's.
#[test]
fn the_cap_expires_a_cloak_from_a_battle_maps_own_round() {
    let mut game = game(715);
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();

    game.arm_cloak(player, 1);
    for _ in 0..40 {
        if !is_cloaked(&game, player) {
            return;
        }
        if game.tactical_actor().is_none() {
            break;
        }
        game.tactical_end_turn();
    }
    panic!("a cloak armed for one round outlived a battle map's whole fight");
}

// ─────────────────────────────────────────────────────────────────────────
// The battle map's own doors.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn tactical_attack_refuses_a_cloaked_target() {
    let mut game = game(716);
    let pack = tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    let target = pack[0];
    assert!(wait_for_turn(&mut game, player));
    stand_beside(&mut game, player, target);

    game.arm_cloak(target, 5);
    assert!(
        !game.tactical_attack(target),
        "a cloaked body was named by a swing"
    );
    game.break_cloak(target);
    assert!(
        game.tactical_attack(target),
        "the same swing was refused once the body was exposed, so the fixture \
         proves nothing about the cloak"
    );
}

/// The other side of the split: `reach::recipients` never learns the word,
/// so a shape that covers a cloaked cell still lands on it — and landing
/// breaks it.
#[test]
fn an_area_shape_still_covers_a_cloaked_cell_and_landing_breaks_it() {
    let mut game = game(717);
    let pack = tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    let target = pack[0];
    assert!(wait_for_turn(&mut game, player));
    stand_beside(&mut game, player, target);
    game.arm_cloak(target, 5);

    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(target)
        .unwrap();
    let covered = {
        let battle = game.world.resource::<TacticalBattle>();
        reach::recipients(battle, player, at, AbilityShape::Radius { radius: 1 })
    };
    assert!(
        covered.contains(&target),
        "a blast over a cloaked body's own cell missed it"
    );

    let blast = AbilityDef {
        target: AbilityTarget::AllEnemies,
        effect: AbilityEffect::Damage {
            power: 20,
            spread: 2,
            status: None,
        },
        accuracy: 500,
        ..detach(&game)
    };
    force_the_next_attack_to_land(&mut game);
    game.use_ability(&blast, player, "You", &covered);
    assert!(
        !is_cloaked(&game, target),
        "a blast that connected left the body hidden"
    );
}

/// A cloaked body is still a wall. `reach::movement_field` refuses a
/// pass-through/destination split by design, and a cell the player cannot
/// path into is the tell that says where the body is.
#[test]
fn a_cloaked_body_still_blocks_the_movement_field() {
    let mut game = game(718);
    let pack = tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    let target = pack[0];
    assert!(wait_for_turn(&mut game, player));
    stand_beside(&mut game, player, target);
    game.arm_cloak(target, 5);

    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(target)
        .unwrap();
    let allowance = game.movement_allowance(player);
    let field = {
        let battle = game.world.resource::<TacticalBattle>();
        reach::movement_field(battle, player, allowance)
    };
    assert!(
        !field.contains_key(&at),
        "a cloaked body stopped being a wall, so its cell reads as empty ground"
    );
}

/// The fifth door. `tactical_sides` is private, so this reads it through the
/// AI beat it feeds: a lone cloaked hostile is not swung at.
#[test]
fn the_wild_sides_scoring_does_not_name_a_cloaked_body() {
    let mut game = game(719);
    let pack = tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    let hostile = pack[0];
    assert!(wait_for_turn(&mut game, hostile));
    stand_beside(&mut game, hostile, player);

    game.arm_cloak(player, 5);
    let hp_before = game.world.get::<Stats>(player).unwrap().hp;
    game.tactical_drive_turn();
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        hp_before,
        "a hostile swung at a cloaked body standing next to it"
    );

    // The control, without which a hostile that simply never reaches anyone
    // would pass this: exposed, the same body in the same place is swung at.
    assert!(wait_for_turn(&mut game, hostile));
    stand_beside(&mut game, hostile, player);
    game.break_cloak(player);
    let mut landed = false;
    for _ in 0..20 {
        let before = game.world.get::<Stats>(player).unwrap().hp;
        game.tactical_drive_turn();
        if game.world.get::<Stats>(player).unwrap().hp < before {
            landed = true;
            break;
        }
        if !wait_for_turn(&mut game, hostile) {
            break;
        }
        stand_beside(&mut game, hostile, player);
    }
    assert!(
        landed,
        "an exposed body standing next to a hostile was never swung at"
    );
}

/// **The fifth door on its own.** `tactical_attack` already refuses a cloaked
/// body, so the test above passes with `tactical_sides` unfiltered — what
/// that one proves is the pair. This is what the filter buys by itself:
/// `swing_at_best_neighbour` sorts by Integrity, so an unfiltered list hands
/// the hostile the wounded cloaked body, the swing is refused, and the
/// cloaked companion soaks the whole turn while the exposed player standing
/// beside it is never swung at.
#[test]
fn a_cloaked_body_does_not_soak_the_swing_the_exposed_one_should_take() {
    let mut game = game(724);
    let pet = spawn_tamed(&mut game, 100_000, 1);
    enlist(&mut game, pet);
    let pack = tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    let hostile = pack[0];

    // The wounded body is the one the sort reaches for first.
    game.world.get_mut::<Stats>(pet).unwrap().hp = 1;
    game.arm_cloak(pet, 9);

    // Driven over several turns rather than one: a swing can miss, and a
    // hostile may spend a turn walking. What discriminates is that with the
    // cloaked body in the list it is picked **every** turn — lowest Integrity
    // — and refused every turn, so the player is never reached at all.
    let mut landed = false;
    for _ in 0..24 {
        if !wait_for_turn(&mut game, hostile) {
            break;
        }
        stand_both_beside(&mut game, hostile, pet, player);
        let player_hp = game.world.get::<Stats>(player).unwrap().hp;
        game.tactical_drive_turn();
        assert_eq!(
            game.world.get::<Stats>(pet).unwrap().hp,
            1,
            "the hostile swung at the cloaked body"
        );
        if game.world.get::<Stats>(player).unwrap().hp < player_hp {
            landed = true;
            break;
        }
    }
    assert!(
        landed,
        "the hostile spent every turn on the cloaked body instead of the exposed \
         one standing right beside it"
    );
}

/// Puts `a` and `b` on two free cells adjacent to `anchor`, so both are in
/// reach of a melee swing and which of them to swing at is the only thing
/// left to decide.
fn stand_both_beside(game: &mut Game, anchor: Entity, a: Entity, b: Entity) {
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(anchor)
        .expect("the anchor is on the board");
    for body in [a, b] {
        let cell = *free_ring(game, at, 1)
            .first()
            .expect("a free cell beside the anchor");
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(body, cell)
        );
    }
}

/// Up to `want` free walkable cells adjacent to `cell`, in the board's own
/// reading order so a seeded fight resolves the same way twice.
fn free_ring(game: &Game, cell: (i32, i32), want: usize) -> Vec<(i32, i32)> {
    let battle = game.world.resource::<TacticalBattle>();
    (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
        .filter(|&(dx, dy)| (dx, dy) != (0, 0))
        .map(|(dx, dy)| (cell.0 + dx, cell.1 + dy))
        .filter(|&at| battle.board.walkable(at.0, at.1) && battle.occupant(at).is_none())
        .take(want)
        .collect()
}

/// Stands `mover` on a free cell adjacent to `anchor`, so a melee swing is
/// legal without walking there.
fn stand_beside(game: &mut Game, mover: Entity, anchor: Entity) {
    let battle = game.world.resource::<TacticalBattle>();
    let at = battle.cell_of(anchor).expect("the anchor is on the board");
    let free = (-1..=1)
        .flat_map(|dx| (-1..=1).map(move |dy| (dx, dy)))
        .filter(|&(dx, dy)| (dx, dy) != (0, 0))
        .map(|(dx, dy)| (at.0 + dx, at.1 + dy))
        .find(|&cell| {
            battle.board.walkable(cell.0, cell.1)
                && battle.occupant(cell).is_none_or(|e| e == mover)
        })
        .expect("a free cell beside the anchor");
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(mover, free)
    );
}

/// `count` hostiles, in an open tactical fight with the player.
fn tactical_fight(game: &mut Game, count: usize, hp: i32) -> Vec<Entity> {
    let player = game.player_entity();
    let at = *game.world.get::<Position>(player).unwrap();
    let pack: Vec<Entity> = (0..count)
        .map(|i| {
            let e = spawn_wild_without_routine(game, "scrapper", at.x + 1 + i as i32, at.y);
            game.world.get_mut::<Stats>(e).unwrap().hp = hp;
            game.world.get_mut::<Stats>(e).unwrap().max_hp = hp;
            e
        })
        .collect();
    game.open_tactical_battle(pack.clone());
    pack
}

/// Hands turns on until it is `who`'s again, or the fight ends.
fn wait_for_turn(game: &mut Game, who: Entity) -> bool {
    for _ in 0..64 {
        match game.tactical_actor() {
            None => return false,
            Some(actor) if actor == who => return true,
            Some(_) => game.tactical_end_turn(),
        }
    }
    panic!("the turn never came back round");
}

// ─────────────────────────────────────────────────────────────────────────
// Teardown, and what the player reads.
// ─────────────────────────────────────────────────────────────────────────

/// `Cloaked` is battle-scoped exactly as `CombatBuff` and `AbilityCooldowns`
/// are, which is what keeps it out of `save.rs`. Left set, it would follow
/// the player out of one fight and into the next.
#[test]
fn a_cloak_does_not_outlive_the_fight_it_was_armed_in() {
    let mut game = game(720);
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 200, 1);
    enlist(&mut game, pet);
    let wild = abstract_fight(&mut game);

    game.arm_cloak(player, 9);
    game.arm_cloak(pet, 9);
    game.arm_cloak(wild, 9);
    game.clear_battle_status_effects(player, Some(wild));

    assert!(!is_cloaked(&game, player), "the player's cloak survived");
    assert!(!is_cloaked(&game, pet), "a companion's cloak survived");
    assert!(!is_cloaked(&game, wild), "a hostile's cloak survived");
}

/// The status column's row. `Cloaked` carries no invocation-time name and no
/// magnitude, so the label is fixed and the magnitude is the em dash —
/// `PowerCell::Unrated`'s convention for *no answer* rather than a bad one.
#[test]
fn a_cloak_shows_as_a_buff_row_with_no_magnitude() {
    let mut game = game(721);
    let player = game.player_entity();
    game.arm_cloak(player, 4);

    let row = game
        .active_buffs()
        .into_iter()
        .find(|b| b.name == "Cloaked")
        .expect("a cloak is something running on the player");
    assert_eq!(row.magnitude, "\u{2014}");
    assert_eq!(row.remaining, "4t");
    assert_eq!(
        row.holder_label, None,
        "the player's own row carries no tag"
    );
}

/// Nothing is hidden from the player's view — a cloaked body still reaches
/// the renderer, carrying the flag that fades it.
#[test]
fn a_cloaked_body_reaches_the_tactical_view_marked_rather_than_missing() {
    let mut game = game(722);
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    game.arm_cloak(hostile, 3);

    let view = game.tactical_view().expect("a fight is open");
    let body = view
        .bodies
        .iter()
        .find(|b| b.entity == hostile)
        .expect("a cloaked hostile was omitted from the view");
    assert!(body.cloaked);
    assert!(
        view.order.iter().any(|r| r.entity == hostile),
        "a cloaked body lost its rung in the turn strip"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Load validation.
// ─────────────────────────────────────────────────────────────────────────

/// Both refusals, and the rest of the directory still loading — the
/// "malformed file is skipped with a warning, never a panic" rule.
#[test]
fn a_badly_authored_cloak_is_skipped_and_its_neighbours_still_load() {
    use crate::abilities::AbilityDb;

    let dir = scratch_assets_dir("cloak_load");
    std::fs::create_dir_all(&*dir).unwrap();
    std::fs::write(
        dir.join("good.ron"),
        r#"(id: "cloak_good", name: "Test Single", description: "d",
            target: OneAlly, effect: Cloak(duration: 2), cooldown: 3, power_cost: 4.0)"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("aimed_at_an_enemy.ron"),
        r#"(id: "cloak_enemy", name: "Test Single", description: "d",
            target: OneEnemyGroupFront, effect: Cloak(duration: 2), cooldown: 3, power_cost: 4.0)"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("no_duration.ron"),
        r#"(id: "cloak_zero", name: "Test Single", description: "d",
            target: OneAlly, effect: Cloak(duration: 0), cooldown: 3, power_cost: 4.0)"#,
    )
    .unwrap();

    let (db, warnings) = AbilityDb::load_dir(&dir).unwrap();
    assert!(db.get("cloak_good").is_some(), "a valid neighbour was lost");
    assert!(
        db.get("cloak_enemy").is_none(),
        "an enemy-facing cloak loaded"
    );
    assert!(db.get("cloak_zero").is_none(), "a zero-round cloak loaded");
    assert_eq!(warnings.len(), 2, "{warnings:?}");
}

// ─────────────────────────────────────────────────────────────────────────
// The shipped content.
// ─────────────────────────────────────────────────────────────────────────

/// End to end through the doors a player actually goes through: the shipped
/// routine, run at a companion, makes it untargetable.
#[test]
fn the_shipped_routine_hides_the_ally_it_is_run_at() {
    let mut game = game(723);
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 100_000, 1);
    enlist(&mut game, pet);
    abstract_fight(&mut game);

    let def = detach(&game);
    let AbilityEffect::Cloak { duration } = def.effect else {
        panic!("detach.ron authors a Cloak");
    };
    assert!(duration > 0);
    game.use_ability(&def, player, "You", &[pet]);

    assert!(is_cloaked(&game, pet));
    let named = (0..1000)
        .filter(|_| game.roll_enemy_target(player) == pet)
        .count();
    assert_eq!(named, 0, "the shipped routine hid nobody");
}
