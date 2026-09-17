//! Opportunity attacks: what standing next to an enemy costs.
//!
//! The budget, the two triggers and the one door they both go through.

use crate::Game;
use crate::components::{Cloaked, Stats, StatusEffects};
use crate::resources::DifficultyMode;
use crate::tactical::TacticalBattle;
use crate::tactical::turn::StepOutcome;
use crate::tests::support::test_assets_dir;
use crate::tests::tactical::{only_routine, tactical_fight, wait_for_turn};
use crate::tuning::TACTICAL_MELEE_RANGE;
use bevy_ecs::prelude::Entity;

fn game() -> Game {
    Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// The player and one hostile standing shoulder to shoulder, with an empty
/// cell on the player's far side to step out to and one above to step
/// sideways to.
///
/// Seated by hand, and the strip is *searched for* rather than hardcoded:
/// every board is generated from its own seed, so a fixed cell is walkable
/// in some fights and rock in others.
fn face_off(game: &mut Game) -> (Entity, Entity) {
    let pack = tactical_fight(game, 1, 4000);
    let player = game.player_entity();
    let hostile = pack[0];
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, hostile]);
    let at = clear_strip(game).expect("no board in this fixture had a clear strip");
    let mut battle = game.world.resource_mut::<TacticalBattle>();
    // Off the board first: `move_to` refuses an occupied cell, and the two
    // are seated where the deployment put them.
    battle.move_to(hostile, (at.0 + 3, at.1 + 3));
    assert!(battle.move_to(player, at), "the player could not be seated");
    assert!(
        battle.move_to(hostile, (at.0 + 1, at.1)),
        "the hostile could not be seated beside them"
    );
    assert!(wait_for_turn(game, player));
    (player, hostile)
}

/// A cell whose east, west and north neighbours are all walkable and
/// unoccupied — the shape every test here stands its two bodies on.
fn clear_strip(game: &Game) -> Option<(i32, i32)> {
    let battle = game.world.resource::<TacticalBattle>();
    let taken: Vec<(i32, i32)> = battle.bodies().map(|(_, cell)| cell).collect();
    let side = battle.board.side;
    (1..side - 1).find_map(|y| {
        (2..side - 2).find_map(|x| {
            let cells = [(x, y), (x + 1, y), (x - 1, y), (x, y + 1), (x - 1, y + 1)];
            cells
                .iter()
                .all(|&(cx, cy)| battle.board.walkable(cx, cy) && !taken.contains(&(cx, cy)))
                .then_some((x, y))
        })
    })
}

/// Where the player stands, as the fight sees it.
fn cell(game: &Game, body: Entity) -> (i32, i32) {
    game.world
        .resource::<TacticalBattle>()
        .cell_of(body)
        .expect("the body is on the board")
}

#[test]
fn stepping_out_of_reach_provokes() {
    let mut game = game();
    let (player, hostile) = face_off(&mut game);
    let before = game.world.get::<Stats>(player).unwrap().hp;

    // East is away from the hostile, which stands to the west of the cell
    // this lands on.
    assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Moved);

    assert!(
        game.world.get::<Stats>(player).unwrap().hp < before,
        "walking out of a hostile's reach cost nothing"
    );
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .reaction_spent(hostile),
        "the reaction was taken and not charged"
    );
}

#[test]
fn a_step_that_stays_in_reach_provokes_nobody() {
    let mut game = game();
    let (player, hostile) = face_off(&mut game);
    let before = game.world.get::<Stats>(player).unwrap().hp;

    // North: still diagonally adjacent to the hostile, so still inside
    // `TACTICAL_MELEE_RANGE` of it.
    assert_eq!(game.tactical_step((0, 1)), StepOutcome::Moved);
    let at = cell(&game, player);
    let theirs = cell(&game, hostile);
    assert!(
        crate::tactical::reach::distance(at, theirs) <= TACTICAL_MELEE_RANGE,
        "this fixture meant to keep the player inside reach"
    );

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        before,
        "a step that stayed in reach was swung at anyway"
    );
    assert!(
        !game
            .world
            .resource::<TacticalBattle>()
            .reaction_spent(hostile),
        "a reaction was spent on a step that provoked nobody"
    );
}

#[test]
fn a_reaction_is_spent_once_and_refunded_at_that_body_s_own_turn() {
    let mut game = game();
    let (_player, hostile) = face_off(&mut game);

    assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Moved);
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .reaction_spent(hostile),
        "the step did not provoke"
    );

    // Still the player's turn: the budget is not refunded by the step that
    // spent it, nor by anything else the mover does.
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .reaction_spent(hostile),
        "the reaction came back inside the turn that spent it"
    );

    game.tactical_end_turn();
    assert_eq!(
        game.tactical_actor(),
        Some(hostile),
        "this fixture meant the hostile to act next"
    );
    assert!(
        !game
            .world
            .resource::<TacticalBattle>()
            .reaction_spent(hostile),
        "the reaction was not refunded at the start of its own turn"
    );
}

#[test]
fn a_cloaked_mover_provokes_nobody() {
    let mut game = game();
    let (player, hostile) = face_off(&mut game);
    game.world
        .entity_mut(player)
        .insert(Cloaked { remaining: 3 });
    let before = game.world.get::<Stats>(player).unwrap().hp;

    assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Moved);

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        before,
        "a cloaked body was swung at on its way out"
    );
    assert!(
        !game
            .world
            .resource::<TacticalBattle>()
            .reaction_spent(hostile),
        "a reaction was spent on a body nobody could name"
    );
}

/// The Opening rung's rule, and the reason for it: a fumbled reaction would
/// riposte, and a riposte is another swing that could provoke again.
///
/// **Probed on the reactor and not on the log**, because no fumble line
/// contains the word: every rung of the ladder lands on the *swinger* — the
/// two middle ones as damage, the two outer ones as a condition — so a
/// reactor that comes through two hundred reactions unmarked is a reactor
/// that never fumbled. Fails with `Swing::reaction`'s `free` flipped off.
#[test]
fn a_reaction_never_fumbles() {
    let mut game = game();
    let (player, hostile) = face_off(&mut game);
    let reactor_hp = game.world.get::<Stats>(hostile).unwrap().hp;
    let mut reactions = 0;

    for _ in 0..200 {
        // Kept standing: the mover dying would end the sweep early, and what
        // is being measured is the reactor's own swings.
        game.world.get_mut::<Stats>(player).unwrap().hp = 9_000;
        assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Moved);
        assert!(
            game.world
                .resource::<TacticalBattle>()
                .reaction_spent(hostile),
            "the step provoked nobody — this sweep would measure nothing"
        );
        reactions += 1;
        assert_eq!(
            game.world.get::<Stats>(hostile).unwrap().hp,
            reactor_hp,
            "a reaction cost its own swinger Integrity: a Recoil or an Opening rung"
        );
        assert!(
            game.world
                .get::<StatusEffects>(hostile)
                .is_none_or(|effects| effects.active.is_none()),
            "a reaction left its own swinger marked: an Exposed or a Crash rung"
        );
        assert_eq!(game.tactical_step((1, 0)), StepOutcome::Moved);
        game.tactical_end_turn();
        assert!(wait_for_turn(&mut game, player));
    }
    assert_eq!(reactions, 200, "the sweep did not run");
}

/// A reaction that kills the mover stops the step where it stood: the move
/// is never written, so the body falls on the cell it tried to leave.
///
/// The kill is *waited for* rather than forced. `force_the_next_attack_to_land`
/// cannot pin this one — `swing_move_at` rolls the reactor's move first and
/// eats the forced roll, which is `tactical_defend`'s fixture trap one door
/// over — so the player steps out, steps back and hands the turn on until a
/// reaction lands, which it does inside a round or two against 500 Attack.
#[test]
fn a_fatal_reaction_stops_the_step() {
    let mut game = game();
    let (player, hostile) = face_off(&mut game);
    game.world.get_mut::<Stats>(hostile).unwrap().atk = 500;

    for _ in 0..24 {
        game.world.get_mut::<Stats>(player).unwrap().hp = 1;
        let at = cell(&game, player);
        let outcome = game.tactical_step((-1, 0));
        if !game.creature_alive(player)
            || game
                .world
                .get_resource::<TacticalBattle>()
                .is_none_or(|battle| battle.cell_of(player).is_none())
        {
            assert_ne!(
                outcome,
                StepOutcome::Moved,
                "a reaction put the mover down and the step was written anyway"
            );
            return;
        }
        assert_eq!(outcome, StepOutcome::Moved);
        assert_eq!(cell(&game, player), (at.0 - 1, at.1));
        // Back into reach — stepping *toward* a body provokes nothing — and
        // on round the order goes, which refunds the reaction.
        assert_eq!(game.tactical_step((1, 0)), StepOutcome::Moved);
        game.tactical_end_turn();
        assert!(wait_for_turn(&mut game, player));
    }
    panic!("no reaction landed in 24 rounds against 500 Attack");
}

/// The second trigger: a body that invokes beside an enemy is swung at,
/// whatever it was invoking and wherever it aimed.
#[test]
fn invoking_beside_an_enemy_provokes() {
    let mut game = game();
    let (player, _hostile) = face_off(&mut game);
    only_routine(&mut game, player, "mirror_restore");
    let at = cell(&game, player);

    let before = game.message_history(usize::MAX).len();

    assert!(game.tactical_use_routine(0, at));

    // Read off the log and not off the budget: a routine **ends the turn**,
    // so by the time this looks, the hand-on has reached the reactor's own
    // turn and refunded it. The interrupt line is the durable record.
    assert!(
        game.message_history(usize::MAX)
            .into_iter()
            .skip(before)
            .any(|line| line.text.contains("(interrupt)")),
        "invoking under a hostile's nose cost nothing"
    );
}

/// A capture is the exemption, and it is the only one: its whole cost is
/// already the catalyst it spends.
#[test]
fn a_capture_provokes_nobody() {
    let mut game = game();
    let (player, hostile) = face_off(&mut game);
    only_routine(&mut game, player, "decompile");
    let theirs = cell(&game, hostile);

    let before = game.message_history(usize::MAX).len();

    assert!(game.tactical_use_routine(0, theirs));

    assert!(
        !game
            .message_history(usize::MAX)
            .into_iter()
            .skip(before)
            .any(|line| line.text.contains("(interrupt)")),
        "a capture provoked the body it was aimed at"
    );
}

/// A routine cut off by a reaction has still paid for itself: the fizzle is
/// the rest interrupt's shape, not a refusal, so nothing is handed back and
/// the effect never lands.
///
/// **The effect is probed on the reactor**, which is the one body the
/// player's own death cannot explain away: `mirror_restore` is full
/// friendly fire, so a heal that resolved would mend the hostile standing
/// beside the invoker, and a Forgiving reboot cannot be mistaken for it.
#[test]
fn a_fizzled_routine_keeps_its_price_and_lands_nothing() {
    let mut game = game();
    let (player, hostile) = face_off(&mut game);
    only_routine(&mut game, player, "mirror_restore");
    game.world.get_mut::<Stats>(hostile).unwrap().atk = 500;
    let at = cell(&game, player);

    for _ in 0..24 {
        game.world.get_mut::<Stats>(player).unwrap().hp = 1;
        let power_before = game
            .world
            .get::<crate::components::PowerReserve>(player)
            .unwrap()
            .get();
        let before = game.message_history(usize::MAX).len();

        assert!(game.tactical_use_routine(0, at));

        let cut_off = game
            .message_history(usize::MAX)
            .into_iter()
            .skip(before)
            .any(|line| line.text.contains("is cut off"));
        if cut_off {
            // **Probed on the log and not on a body.** A fizzle in a
            // one-on-one fight is the player going down, which closes the
            // fight and hands every hostile back to the zone at full
            // Integrity — so no body on the board can still answer whether
            // the heal landed, and the narration can.
            assert!(
                !game
                    .message_history(usize::MAX)
                    .into_iter()
                    .skip(before)
                    .any(|line| line.text.contains("patches")),
                "the routine was cut off and its effect landed anyway"
            );
            assert!(
                game.world
                    .get::<crate::components::PowerReserve>(player)
                    .unwrap()
                    .get()
                    < power_before,
                "the fizzle handed back the Power the invocation had spent"
            );
            return;
        }
        assert!(
            game.world.get_resource::<TacticalBattle>().is_some(),
            "the fight closed without the invocation ever being cut off"
        );
        game.tactical_end_turn();
        assert!(wait_for_turn(&mut game, player));
        game.world
            .entity_mut(player)
            .insert(crate::components::AbilityCooldowns::default());
    }
    panic!("no reaction cut the invocation off in 24 rounds against 500 Attack");
}

/// The forecast and the fight have to agree: what `provocation` says a step
/// will cost is what taking that step actually spends.
///
/// Both directions, because a prediction that answered "yes" to everything
/// would pass a test that only asked about the provoking step.
#[test]
fn provocation_predicts_what_a_step_really_does() {
    let mut game = game();
    let (player, hostile) = face_off(&mut game);
    let at = cell(&game, player);
    let out = (at.0 - 1, at.1);
    let alongside = (at.0, at.1 + 1);

    assert!(
        game.provocation(player, at, out) > 0.0,
        "stepping out of reach was predicted free"
    );
    assert_eq!(
        game.provocation(player, at, alongside),
        0.0,
        "a step that stays in reach was predicted to cost something"
    );

    assert_eq!(game.tactical_step((0, 1)), StepOutcome::Moved);
    assert!(
        !game
            .world
            .resource::<TacticalBattle>()
            .reaction_spent(hostile),
        "the step the prediction called free provoked anyway"
    );
    assert_eq!(game.tactical_step((0, -1)), StepOutcome::Moved);
    assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Moved);
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .reaction_spent(hostile),
        "the step the prediction priced provoked nobody"
    );
}

/// The term the walk scoring reads: a destination whose path leaves a
/// reactor's reach is worth less than the same destination when nothing can
/// react — and a reaction already spent is nothing that can react.
#[test]
fn a_walk_that_provokes_is_worth_less_than_one_that_does_not() {
    let mut game = game();
    let (player, hostile) = face_off(&mut game);
    let at = cell(&game, player);
    let out = (at.0 - 1, at.1);

    let risked = game.walk_risk(player, out);
    assert!(
        risked > 0.0,
        "walking out of a hostile's reach was scored as free"
    );

    game.world
        .resource_mut::<TacticalBattle>()
        .spend_reaction(hostile);
    assert_eq!(
        game.walk_risk(player, out),
        0.0,
        "a body with no reaction left was still priced as a threat"
    );
}

/// The telegraph and the act have to agree, on both halves: the rows say
/// what invoking here will provoke, and a capture's row says nothing.
#[test]
fn the_routine_rows_say_what_invoking_here_provokes() {
    let mut game = game();
    let (player, _hostile) = face_off(&mut game);
    game.world
        .entity_mut(player)
        .insert(crate::components::Routines(vec![
            "mirror_restore".to_string(),
            "decompile".to_string(),
        ]));

    let rows = game.tactical_routine_options();
    let heal = rows
        .iter()
        .find(|row| row.name.contains("Patch"))
        .expect("the heal is offered");
    assert_eq!(heal.provokes, 1, "the row did not price the hostile beside");
    let capture = rows
        .iter()
        .find(|row| row.name.to_lowercase().contains("decompile"))
        .expect("the capture is offered");
    assert_eq!(
        capture.provokes, 0,
        "a capture's row promised a cost the act does not charge"
    );
}

/// The movement telegraph is a call into the term the AI reads, so the
/// cells the player is warned about are the cells a hostile would price.
#[test]
fn the_movement_overlay_marks_the_cells_that_provoke() {
    let mut game = game();
    let (player, _hostile) = face_off(&mut game);
    let at = cell(&game, player);
    let view = game.tactical_view().expect("a fight is open");

    assert!(
        view.provoking.iter().all(|c| view.reachable.contains(c)),
        "a cell was marked as provoking that could not be walked to"
    );
    assert!(
        view.provoking.contains(&(at.0 - 1, at.1)),
        "stepping out of reach was not marked"
    );
    assert!(
        !view.provoking.contains(&(at.0, at.1 + 1)),
        "a step that stays in reach was marked as costing something"
    );
}
