use super::*;

/// Walking into a hostile is a swing, not a refusal.
///
/// The board's own bump, `move_player`'s ladder one space over: an occupied
/// cell used to answer `Refused`, so an arrow key pressed at the body the
/// whole turn was spent closing on did nothing at all and the player had to
/// find `a` to finish the approach.
///
/// Every stream must spend the action; a landing blow is *searched for*
/// rather than owed, on `bracing_reduces_what_the_next_swing_lands`' rule —
/// `force_the_next_attack_to_land` cannot pin a tactical swing, because the
/// move is rolled first and eats the forced roll, and every matchup has a
/// miss chance by design.
#[test]
fn walking_into_a_hostile_swings_at_it() {
    // The fight built fresh and bumped on `stream`: what the step answered,
    // what the blow cost, and whether the action went with it.
    let bump = |stream: u64| -> (StepOutcome, i32, bool) {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 40);
        let player = game.player_entity();
        assert!(wait_for_turn(&mut game, player), "the fight ended early");
        let at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(player)
            .expect("the player stands on the board");
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(pack[0], (at.0 + 1, at.1)),
            "the cell beside the player is taken"
        );
        let before = game
            .world
            .get::<Stats>(pack[0])
            .expect("the hostile is alive")
            .hp;

        crate::tests::support::reseed_rng(&mut game, stream);
        let outcome = game.tactical_step((1, 0));
        // Read as "the turn is no longer the player's" rather than off
        // `acted`: the action *ends* the turn, so `tactical_attack` hands it
        // on and `acted` is then answering about whoever came next. A fight
        // that ended inside the blow took the resource with it, which spent
        // the turn as surely.
        let spent = game.tactical_actor() != Some(player);
        let after = game.world.get::<Stats>(pack[0]).map(|s| s.hp).unwrap_or(0);
        (outcome, before - after, spent)
    };

    let (outcome, _, spent) = bump(0);
    assert_eq!(
        outcome,
        StepOutcome::Struck,
        "a step into a hostile was not a swing"
    );
    assert!(spent, "the bump spent no action");

    let cost = (0..512u64)
        .map(bump)
        .find(|&(_, cost, _)| cost > 0)
        .map(|(_, cost, _)| cost)
        .expect("no stream in 0..512 landed the bump");
    assert!(cost > 0, "the bump landed no blow");
}

/// ...and the player does not move onto the cell it swung at.
///
/// A bump that both swung and stepped would put two bodies on one cell,
/// which `reach::movement_field`'s occupancy rule has no way to express.
#[test]
fn a_bump_spends_the_action_and_not_the_step() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player stands on the board");
    game.world
        .resource_mut::<TacticalBattle>()
        .move_to(pack[0], (at.0 + 1, at.1));

    assert_eq!(game.tactical_step((1, 0)), StepOutcome::Struck);
    assert_eq!(
        game.world.resource::<TacticalBattle>().cell_of(player),
        Some(at),
        "the player walked onto the body it swung at"
    );
}

/// Walking into one of your own is still refused.
///
/// Friendly fire is full and legal through the aim cursor — that is what a
/// shape is worth aiming for — but an arrow key is not an aim, and a bump
/// that attacked whatever was in the way would make crossing your own line
/// a coin flip. The gate is `Hostile` and nothing else.
#[test]
fn walking_into_a_companion_is_refused_rather_than_a_swing() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player stands on the board");

    // A body of the player's own, stood next to them.
    let friend = body(&mut game, "nothing-in-particular");
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .place(friend, (at.0 + 1, at.1)),
        "the cell beside the player is taken"
    );
    let before = game
        .world
        .get::<Stats>(friend)
        .expect("the companion is alive")
        .hp;

    assert_eq!(game.tactical_step((1, 0)), StepOutcome::Refused);
    assert_eq!(
        game.world.get::<Stats>(friend).map(|s| s.hp),
        Some(before),
        "a bump into one of your own landed a blow"
    );
    assert!(
        game.world.resource::<TacticalBattle>().actions_left() > 0,
        "a refused bump spent the turn's action"
    );
}

/// A hostile's own approach is never turned into a swing by a step.
///
/// `Game::tactical_step` is the door the AI's walk goes through — the whole
/// point of that seam — so a bump that fired for anybody would let a hostile
/// spend its action part-way along a path it planned, and spend it on
/// whatever of its own side happened to be standing in the way.
/// `tactical_awaits_input` is the gate for that reason and not a new
/// predicate: it is false for exactly the bodies the beat loop drives.
///
/// Two hostiles, deliberately. A hostile stepping into the *player* is
/// refused by the `Hostile` gate alone, so a test written that way passes
/// with this one deleted.
#[test]
fn a_hostile_mid_walk_does_not_bump() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 2, 40);
    assert!(wait_for_turn(&mut game, pack[0]), "the fight ended early");
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(pack[0])
        .expect("the hostile stands on the board");
    let beside = (at.0 + 1, at.1);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(pack[1], beside),
        "the cell beside the hostile is taken"
    );
    let before = game
        .world
        .get::<Stats>(pack[1])
        .expect("the second hostile is alive")
        .hp;

    assert_eq!(game.tactical_step((1, 0)), StepOutcome::Refused);
    assert_eq!(
        game.world.get::<Stats>(pack[1]).map(|s| s.hp),
        Some(before),
        "a hostile swung at its own side by walking into it"
    );
}

/// The door auto-attack drives the player's own side through, and the one
/// thing it has to be that `tactical_ai_beat` is not: willing to spend the
/// turn of a body the player commands.
///
/// Both halves, because "the auto door drove something" passes against a
/// door that is merely the AI's under a second name.
#[test]
fn the_auto_door_drives_a_party_body_the_ai_door_declines() {
    use crate::tactical::ai::AiBeat;

    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player), "the fight ended early");

    assert_eq!(
        game.tactical_ai_beat(),
        AiBeat::Idle,
        "the AI door spent a turn that was the player's"
    );
    for _ in 0..=TACTICAL_MOVE_MAX {
        if game.tactical_auto_beat() == AiBeat::Acted {
            break;
        }
    }
    assert_ne!(
        game.tactical_actor(),
        Some(player),
        "the auto door spent the turn without handing it on"
    );
}

/// A fight nobody touches resolves, and it resolves by the hostile being
/// beaten rather than by a party body wandering off the edge.
///
/// **The departure is the failure this rules out.** A step off the board is a
/// jack-out, so a driven party body that ever chose an edge cell would end
/// the fight with the hostile still standing — and the player's run would
/// leave a fight it was winning because nobody pressed a key.
#[test]
fn an_auto_driven_fight_is_won_rather_than_walked_out_of() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 10);

    for _ in 0..4000 {
        if !game.has_active_battle() {
            break;
        }
        game.tactical_auto_beat();
    }

    assert!(!game.has_active_battle(), "the auto fight never resolved");
    assert!(
        game.world.get::<Stats>(pack[0]).is_none_or(|s| s.hp <= 0),
        "the fight ended with the hostile still up, so somebody walked out"
    );
}

/// todo #103: auto-attack now invokes a party body's routines exactly as a
/// hostile's own AI does, gated on `Game::ability_unavailable` rather than
/// bypassing it — the cooldown and the reserve are both how that is visible,
/// since `run_tactical_routine` arms the one and charges the other, and a
/// swing does neither.
#[test]
fn an_auto_driven_party_body_invokes_a_ready_affordable_routine() {
    use crate::components::{AbilityCooldowns, PowerReserve};
    use crate::tactical::ai::AiBeat;
    use crate::tests::support::HOSTILE_SWEEP;

    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    assert!(
        game.wild_routine_ready(player).is_some(),
        "the fixture left nothing to invoke, so nothing is being tested"
    );
    let power_before = game.world.get::<PowerReserve>(player).unwrap().get();

    for _ in 0..=TACTICAL_MOVE_MAX {
        if game.tactical_auto_beat() == AiBeat::Acted {
            break;
        }
    }

    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_some_and(|c| c.0.contains_key(HOSTILE_SWEEP)),
        "an auto-driven turn with a ready, affordable routine swung instead of invoking"
    );
    assert!(
        game.world.get::<PowerReserve>(player).unwrap().get() < power_before,
        "the invocation charged no Power"
    );
}

/// `bus_fault` reshaped and installed as `body`'s only routine: an
/// aggressive, affordable routine whose footprint the test chooses.
pub(super) fn shaped_sweep(
    game: &mut Game,
    body: Entity,
    shape: crate::abilities::AbilityShape,
    range: crate::abilities::AbilityRange,
) -> &'static str {
    use crate::abilities::AbilityDb;
    use crate::tests::support::HOSTILE_SWEEP;

    let mut def = game
        .world
        .resource::<AbilityDb>()
        .get(HOSTILE_SWEEP)
        .expect("the shipped routine is loaded")
        .clone();
    def.id = "shaped_sweep".to_string();
    def.shape = Some(shape);
    def.range = Some(range);
    game.world.resource_mut::<AbilityDb>().insert(def);
    only_routine(game, body, "shaped_sweep");
    "shaped_sweep"
}

/// An auto-driven party body never lands an aggressive routine on its own
/// side, however many hostiles the blast would also catch — and a routine it
/// may not fire is not a wasted turn: it swings instead.
///
/// Three hostiles against the invoker and one companion is the count the
/// old net scoring fired at (+3 −2). The blast covers the whole board, so
/// there is no aim that spares the party. Two actions and a pinned walk are
/// the instrument: a swing spends one and leaves the player acting
/// (`Stepped`), where a turn that found nothing to do hands it on.
#[test]
fn an_auto_driven_party_body_swings_rather_than_blast_a_companion() {
    use crate::abilities::{AbilityRange, AbilityShape};
    use crate::components::AbilityCooldowns;
    use crate::tactical::ai::AiBeat;

    let mut game = game();
    let companion = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, companion);
    let pack = tactical_fight(&mut game, 3, 10_000);
    let player = game.player_entity();
    let bodies = [pack[0], pack[1], pack[2], companion];
    open_ground(&mut game, &bodies, &[(3, 4), (5, 4), (4, 5), (4, 3)]);
    let routine = shaped_sweep(
        &mut game,
        player,
        AbilityShape::Radius { radius: 20 },
        AbilityRange { min: 0, max: 6 },
    );
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    assert!(
        game.wild_routine_ready(player).is_some(),
        "the fixture left nothing to invoke, so nothing is being tested"
    );
    let companion_hp = hp_of(&game, companion);
    {
        let mut battle = game.world.resource_mut::<TacticalBattle>();
        battle.set_actions_left(2);
        battle.commit_walk(Vec::new());
    }

    let beat = game.tactical_auto_beat();

    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_none_or(|c| !c.0.contains_key(routine)),
        "the auto-driven player blasted its own companion"
    );
    assert_eq!(hp_of(&game, companion), companion_hp);
    assert_eq!(
        beat,
        AiBeat::Stepped,
        "the refused routine wasted the turn instead of swinging"
    );
}

/// Refusing the aim from where it stands is not refusing the routine: a line
/// the companion blocks from here is clear from a cell a short walk away,
/// and the auto-driven body walks there and fires.
#[test]
fn an_auto_driven_party_body_walks_to_a_clear_line_of_fire() {
    use crate::abilities::{AbilityRange, AbilityShape};
    use crate::components::AbilityCooldowns;

    let mut game = game();
    let companion = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, companion);
    let pack = tactical_fight(&mut game, 1, 10_000);
    let player = game.player_entity();
    open_ground(&mut game, &[pack[0], companion], &[(4, 1), (4, 3)]);
    let routine = shaped_sweep(
        &mut game,
        player,
        AbilityShape::Line { length: 6 },
        AbilityRange { min: 0, max: 6 },
    );
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    let companion_hp = hp_of(&game, companion);

    for _ in 0..=TACTICAL_MOVE_MAX {
        if game.tactical_actor() != Some(player) {
            break;
        }
        game.tactical_auto_beat();
    }

    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_some_and(|c| c.0.contains_key(routine)),
        "the player never fired, though a clear line was a few steps away"
    );
    assert_eq!(hp_of(&game, companion), companion_hp);
}

/// `PartyTurns::SwingOnly` is what `an_auto_driven_party_body_invokes_a_
/// ready_affordable_routine` above changed away from — the arena's own
/// door, and the tests that hold its numbers, must still get the pre-#103
/// behaviour with the exact same fixture.
#[test]
fn a_swing_only_driven_party_body_never_invokes() {
    use crate::components::AbilityCooldowns;
    use crate::tests::support::HOSTILE_SWEEP;

    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    assert!(
        game.wild_routine_ready(player).is_some(),
        "the fixture left nothing to invoke, so nothing is being tested"
    );

    assert!(
        game.tactical_drive_turn(PartyTurns::SwingOnly),
        "the door drove nobody"
    );

    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_none_or(|c| !c.0.contains_key(HOSTILE_SWEEP)),
        "a SwingOnly-driven turn armed a routine's cooldown, so it invoked rather than swung"
    );
}

/// An unaffordable routine with no Power cell in the pack falls back to a
/// swing: neither the cooldown arms (no invocation) nor the reserve moves
/// (no cell drunk).
#[test]
fn an_unaffordable_routine_with_no_cell_falls_back_to_a_swing() {
    use crate::components::{AbilityCooldowns, PowerReserve};
    use crate::tests::support::{HOSTILE_SWEEP, set_inventory};

    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    game.world
        .entity_mut(player)
        .insert(PowerReserve::new(5.0, crate::components::POWER_MAX));
    set_inventory(&mut game, &[]);

    for _ in 0..TACTICAL_MOVE_MAX {
        if game.tactical_actor() != Some(player) {
            break;
        }
        game.tactical_auto_beat();
    }

    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_none_or(|c| !c.0.contains_key(HOSTILE_SWEEP)),
        "an unaffordable routine with no cell still invoked"
    );
    // Only ordinary tick drain moves this, not a cell (power_cell alone
    // restores 25.0) — a turn on a battle map spends a world tick once its
    // round wraps, which this fixture's two-body order does every player
    // turn.
    let power = game.world.get::<PowerReserve>(player).unwrap().get();
    assert!(
        power <= 5.0 && 5.0 - power < 1.0,
        "an unaffordable routine with no cell still drank something: {power}"
    );
}

/// The heart of todo #103's Power-cell half: refused for Power alone, and a
/// cell in the pack covers the shortfall, so the turn drinks it instead of
/// swinging — one action, one fewer cell, more Power, and no cooldown yet.
/// The *next* turn is what proves the drink actually bought the invocation:
/// once the party is due again, the routine is affordable and runs.
#[test]
fn a_power_only_refusal_with_a_cell_in_reach_drinks_it_and_invokes_next_turn() {
    use crate::components::{AbilityCooldowns, PowerReserve};
    use crate::items::ItemId;
    use crate::items::ids::POWER_CELL;
    use crate::tests::support::{HOSTILE_SWEEP, set_inventory};

    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    // bus_fault costs 18.0; 10.0 is short by 8.0, well inside power_cell's 25.
    game.world
        .entity_mut(player)
        .insert(PowerReserve::new(10.0, crate::components::POWER_MAX));
    set_inventory(&mut game, &[(POWER_CELL, 2)]);

    for _ in 0..TACTICAL_MOVE_MAX {
        if game.tactical_actor() != Some(player) {
            break;
        }
        game.tactical_auto_beat();
    }

    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_none_or(|c| !c.0.contains_key(HOSTILE_SWEEP)),
        "the first turn already invoked, so drinking was never exercised"
    );
    // 10.0 + power_cell's 25.0, minus the same ordinary tick drain the
    // no-cell test accounts for.
    let power = game.world.get::<PowerReserve>(player).unwrap().get();
    assert!(
        power > 30.0,
        "the cell's restore did not land on the reserve: {power}"
    );
    assert_eq!(
        game.world
            .get::<crate::components::Inventory>(player)
            .unwrap()
            .count(&ItemId::from(POWER_CELL)),
        1,
        "drinking a cell did not spend it from the pack"
    );
    assert_ne!(
        game.tactical_actor(),
        Some(player),
        "drinking a cell did not spend the turn"
    );

    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    for _ in 0..TACTICAL_MOVE_MAX {
        if game.tactical_actor() != Some(player) {
            break;
        }
        game.tactical_auto_beat();
    }
    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_some_and(|c| c.0.contains_key(HOSTILE_SWEEP)),
        "the topped-up reserve still did not afford the routine next turn"
    );
}

/// A consumable that restores Power but also arms a `prebattle_buff` (a
/// sustain/backfeed-style item) is never drunk for a Power-only refusal —
/// `Game::power_cell_for`'s own filter — so the same shortfall that would
/// have drunk a plain cell instead falls back to a swing.
#[test]
fn a_prebattle_buff_cell_is_never_drunk_for_a_power_refusal() {
    use crate::components::{AbilityCooldowns, PowerReserve};
    use crate::tests::support::{HOSTILE_SWEEP, set_inventory};

    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    game.world
        .entity_mut(player)
        .insert(PowerReserve::new(10.0, crate::components::POWER_MAX));
    // `backfeed_cell` restores Power and arms a `Trickle` buff — the
    // sustain/backfeed shape `power_cell_for`'s filter exists for. Any
    // shipped `.ron` with both `consume.power > 0` and a `prebattle_buff`
    // would exercise the same filter; this is the fixture, not content the
    // assertion below trusts blindly.
    let backfeed = "backfeed_cell";
    assert!(
        game.world
            .resource::<crate::items_db::ItemDb>()
            .get(backfeed)
            .is_some_and(|d| d
                .consume
                .is_some_and(|c| c.power > 0.0 && c.prebattle_buff.is_some())),
        "fixture: {backfeed} must restore Power and carry a prebattle_buff"
    );
    set_inventory(&mut game, &[(backfeed, 2)]);

    for _ in 0..TACTICAL_MOVE_MAX {
        if game.tactical_actor() != Some(player) {
            break;
        }
        game.tactical_auto_beat();
    }

    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_none_or(|c| !c.0.contains_key(HOSTILE_SWEEP)),
        "a prebattle-buff cell was drunk and then invoked"
    );
    let power = game.world.get::<PowerReserve>(player).unwrap().get();
    assert!(
        power <= 10.0 && 10.0 - power < 1.0,
        "a prebattle-buff cell was drunk for a Power-only refusal: {power}"
    );
}

/// `PartyTurns::SwingOnly` drinks no cell either, even when a Power-only
/// refusal and a covering cell are both present — the arm that would drink
/// one is nested under the same `Invoke` gate as the routine arm.
#[test]
fn swing_only_never_drinks_a_cell_even_when_one_would_cover_the_shortfall() {
    use crate::components::PowerReserve;
    use crate::items::ItemId;
    use crate::items::ids::POWER_CELL;
    use crate::tests::support::{HOSTILE_SWEEP, set_inventory};

    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    game.world
        .entity_mut(player)
        .insert(PowerReserve::new(10.0, crate::components::POWER_MAX));
    set_inventory(&mut game, &[(POWER_CELL, 2)]);

    assert!(
        game.tactical_drive_turn(PartyTurns::SwingOnly),
        "the door drove nobody"
    );

    let power = game.world.get::<PowerReserve>(player).unwrap().get();
    assert!(
        power <= 10.0 && 10.0 - power < 1.0,
        "SwingOnly drank a cell: {power}"
    );
    assert_eq!(
        game.world
            .get::<crate::components::Inventory>(player)
            .unwrap()
            .count(&ItemId::from(POWER_CELL)),
        2,
        "SwingOnly spent a cell from the pack"
    );
}

/// Opus review of todo #103 (2026-09-26): the party arm's candidates came
/// from `ready_routines`, which reads a body's raw `Routines` component
/// directly rather than `Game::kit_of` — "the one answer to where a body's
/// kit comes from" (CLAUDE.md). An emulating player's `Routines` is still
/// whatever is installed underneath the emulation, so auto-attack invoked
/// that instead of what the emulated species actually knows.
#[test]
fn an_emulating_player_ignores_an_installed_routine_missing_from_the_emulated_kit() {
    use crate::components::{AbilityCooldowns, Emulation};
    use crate::species::SpeciesDb;
    use crate::tactical::ai::AiBeat;
    use crate::tests::support::HOSTILE_SWEEP;

    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(wait_for_turn(&mut game, player), "the fight ended early");

    // A species with an empty `abilities` list, so `actor_abilities`'s
    // `Kit::Emulated` arm offers nothing — the emulated kit HOSTILE_SWEEP is
    // missing from, where the buggy raw-`Routines` read would still find it.
    let species = generic_species();
    let species_id = species.id.clone();
    game.world.resource_mut::<SpeciesDb>().insert(species);
    game.world.entity_mut(player).insert(Emulation {
        species: species_id,
        rounds_left: 3,
    });

    for _ in 0..=TACTICAL_MOVE_MAX {
        if game.tactical_auto_beat() == AiBeat::Acted {
            break;
        }
    }

    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_none_or(|c| !c.0.contains_key(HOSTILE_SWEEP)),
        "an emulating player auto-invoked a routine absent from the \
         emulated kit"
    );
}

/// Opus review of todo #103 (2026-09-26): `run_tactical_intent` always
/// floors an invoked routine's cooldown at `ENEMY_ROUTINE_MIN_COOLDOWN`,
/// the hostile AI's own floor. A party-arm invocation stands in for the
/// player's own hand, so it must cool at `tactical_use_routine`'s floor of
/// 0 instead — the `cooldown_floor` distinction CLAUDE.md's combat seam
/// draws between the two doors.
///
/// **Two hostiles, player first in initiative, is what keeps the reading
/// honest.** With only the player and one hostile, the player's own beat
/// can be the round's last — its own `hand_on_turn` would then tick the
/// very cooldown it just armed before this test ever reads it. Two
/// hostiles left to act after the player's beat rules that out, so the
/// value read back is the armed value itself, not a post-tick remainder.
#[test]
fn an_auto_invoked_party_routine_cools_at_the_hand_floor_not_the_enemy_one() {
    use crate::abilities::AbilityDb;
    use crate::components::AbilityCooldowns;
    use crate::tactical::ai::AiBeat;
    use crate::tests::support::HOSTILE_SWEEP;

    const ZERO_COOLDOWN: &str = "zero_cooldown_sweep";

    let mut game = game();
    let pack = tactical_fight(&mut game, 2, 200);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0], pack[1]]);

    let mut def = game
        .world
        .resource::<AbilityDb>()
        .get(HOSTILE_SWEEP)
        .expect("the fixture routine ships")
        .clone();
    def.id = ZERO_COOLDOWN.to_string();
    def.cooldown = 0;
    game.world.resource_mut::<AbilityDb>().insert(def);
    only_routine(&mut game, player, ZERO_COOLDOWN);

    for _ in 0..=TACTICAL_MOVE_MAX {
        if game.tactical_auto_beat() == AiBeat::Acted {
            break;
        }
    }

    assert_eq!(
        game.world
            .get::<AbilityCooldowns>(player)
            .and_then(|c| c.0.get(ZERO_COOLDOWN).copied()),
        // At the hand floor (0), `Game::arm_cooldown`'s own early return
        // arms nothing for an authored `cooldown: 0` — "spammable by
        // design", `decompile.ron`'s own doc for the one shipped routine
        // that authors it. At the enemy floor (`ENEMY_ROUTINE_MIN_COOLDOWN`,
        // 1), `abilities::armed_cooldown(0, 1) + 1 == 2` arms one anyway.
        None,
        "an auto-invoked party routine armed the enemy AI's cooldown floor \
         instead of the hand's own"
    );
}

mod tactical_use_item {
    //! `Game::tactical_use_item` — `[U]`'s door in phase 3, and the door
    //! todo #103's party arm already drives. Every refusal below is asserted
    //! to spend nothing, `tactical_defend`'s own rule.

    use super::{game, tactical_fight, wait_for_turn};
    use crate::components::{Inventory, PowerReserve};
    use crate::items::ItemId;
    use crate::items::ids::POWER_CELL;
    use crate::tactical::TacticalBattle;
    use crate::tests::support::set_inventory;

    #[test]
    fn refused_with_no_fight_open_and_spends_nothing() {
        let mut game = game();
        set_inventory(&mut game, &[(POWER_CELL, 3)]);
        let player = game.player_entity();

        assert!(!game.tactical_use_item(&ItemId::from(POWER_CELL)));

        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(POWER_CELL)),
            3,
            "a refusal with no fight open still spent the item"
        );
    }

    #[test]
    fn refused_for_a_body_that_is_not_the_player_or_in_party() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 200);
        let wild = pack[0];
        set_inventory(&mut game, &[(POWER_CELL, 3)]);
        assert!(wait_for_turn(&mut game, wild), "the hostile never acted");

        assert!(!game.tactical_use_item(&ItemId::from(POWER_CELL)));

        let player = game.player_entity();
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(POWER_CELL)),
            3,
            "a refusal for a non-party actor still spent the pack"
        );
        assert_eq!(
            game.tactical_actor(),
            Some(wild),
            "a refusal for a non-party actor still spent its turn"
        );
    }

    #[test]
    fn refused_with_no_actions_left_and_spends_nothing() {
        let mut game = game();
        tactical_fight(&mut game, 1, 200);
        let player = game.player_entity();
        set_inventory(&mut game, &[(POWER_CELL, 3)]);
        assert!(wait_for_turn(&mut game, player), "the fight ended early");
        // Spend the turn's own action first, through an ordinary door.
        assert!(game.tactical_defend(), "the fixture could not brace");
        assert_ne!(
            game.tactical_actor(),
            Some(player),
            "fixture: bracing did not end the turn, so this test proves nothing"
        );

        assert!(!game.tactical_use_item(&ItemId::from(POWER_CELL)));

        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(POWER_CELL)),
            3,
            "a refusal with no actions left still spent the pack"
        );
    }

    #[test]
    fn refused_for_an_item_the_pack_has_none_of_and_spends_nothing() {
        let mut game = game();
        tactical_fight(&mut game, 1, 200);
        let player = game.player_entity();
        set_inventory(&mut game, &[]);
        assert!(wait_for_turn(&mut game, player), "the fight ended early");

        assert!(!game.tactical_use_item(&ItemId::from(POWER_CELL)));

        assert_eq!(
            game.tactical_actor(),
            Some(player),
            "a refusal for an absent item still spent the turn"
        );
    }

    #[test]
    fn refused_for_an_item_with_no_consume_effect_and_spends_nothing() {
        let mut game = game();
        tactical_fight(&mut game, 1, 200);
        let player = game.player_entity();
        // Any shipped item with no `consume:` field does — a weapon, say —
        // and this fixture does not depend on which one so long as one
        // ships. `ice_breaker` is a taming catalyst, not a consumable.
        set_inventory(&mut game, &[(crate::items::ids::ICE_BREAKER, 3)]);
        assert!(wait_for_turn(&mut game, player), "the fight ended early");

        assert!(!game.tactical_use_item(&ItemId::from(crate::items::ids::ICE_BREAKER)));

        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(crate::items::ids::ICE_BREAKER)),
            3,
            "a refusal for an unusable item still spent it"
        );
        assert_eq!(
            game.tactical_actor(),
            Some(player),
            "a refusal for an unusable item still spent the turn"
        );
    }

    #[test]
    fn a_usable_item_spends_one_action_one_unit_and_restores_power() {
        let mut game = game();
        tactical_fight(&mut game, 1, 200);
        let player = game.player_entity();
        game.world
            .entity_mut(player)
            .insert(PowerReserve::new(10.0, crate::components::POWER_MAX));
        set_inventory(&mut game, &[(POWER_CELL, 2)]);
        assert!(wait_for_turn(&mut game, player), "the fight ended early");

        assert!(game.tactical_use_item(&ItemId::from(POWER_CELL)));

        assert!(
            game.world.get::<PowerReserve>(player).unwrap().get() > 30.0,
            "a used cell did not restore Power"
        );
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(POWER_CELL)),
            1,
            "a used cell was not spent from the pack"
        );
        assert_ne!(
            game.tactical_actor(),
            Some(player),
            "a used item did not spend the turn"
        );
        assert!(
            game.world.get_resource::<TacticalBattle>().is_some(),
            "fixture: the fight should still be open"
        );
    }
}
