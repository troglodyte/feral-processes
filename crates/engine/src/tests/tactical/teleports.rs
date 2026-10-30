use super::*;

/// `AbilityEffect::Teleport` — two aims, one door, and every refusal above
/// the charge.
mod teleport {
    use super::*;
    use crate::abilities::{AbilityDb, teleport_reach};
    use crate::components::{AbilityCooldowns, PowerReserve};
    use crate::tactical::reach::distance;
    use crate::tests::support::built_core;

    /// A level the reach is worth measuring at. At level 1 the reach is the
    /// floor, which is also touching distance — so every "past the reach"
    /// assertion below would be indistinguishable from "out of arm's
    /// reach", and two separate rules would be tested as one.
    const LEVEL: u32 = 8;

    /// The player at `LEVEL`, holding Teleport and nothing else, with the
    /// turn.
    fn armed(game: &mut Game) -> Entity {
        let player = game.player_entity();
        game.world.get_mut::<Experience>(player).unwrap().level = LEVEL;
        let id = built_core(game, crate::routine_craft::Core::Teleport);
        only_routine(game, player, &id);
        assert!(wait_for_turn(game, player));
        player
    }

    fn power_of(game: &Game, who: Entity) -> Option<f32> {
        game.world.get::<PowerReserve>(who).map(|r| r.get())
    }

    /// What a refusal must leave untouched: the Power, the cooldown and the
    /// turn. Asserted per refusal rather than once, because a single test
    /// over one path passes against every other path that never spends
    /// anyway — `commit_caravan_basket`'s rule.
    fn spent_nothing(game: &Game, player: Entity, power: Option<f32>) {
        assert_eq!(
            power_of(game, player),
            power,
            "a refused relocation was charged anyway"
        );
        assert!(
            game.world
                .get::<AbilityCooldowns>(player)
                .is_none_or(|c| c.0.is_empty()),
            "a refused relocation armed its cooldown"
        );
        assert_eq!(
            game.tactical_actor(),
            Some(player),
            "a refused relocation spent the turn"
        );
    }

    fn cell_of(game: &Game, body: Entity) -> (i32, i32) {
        game.world
            .resource::<TacticalBattle>()
            .cell_of(body)
            .expect("the body was not seated")
    }

    /// A legal destination for the body standing at `from`: whatever the
    /// engine's own outline offers, minus the cell it is already on.
    fn destination(game: &Game, from: (i32, i32)) -> (i32, i32) {
        game.teleport_destinations(from)
            .into_iter()
            .find(|&cell| cell != from)
            .expect("the outline offered nowhere to go")
    }

    /// The reach is half the invoker's level, floored at 1 so a low-level
    /// holder owns a routine that can actually move somebody.
    #[test]
    fn the_reach_is_half_the_invokers_level_and_never_zero() {
        assert_eq!(teleport_reach(1), 1, "a level-1 holder could move nobody");
        assert_eq!(teleport_reach(2), 1);
        assert_eq!(teleport_reach(8), 4);
        assert_eq!(teleport_reach(23), 11);
    }

    /// The player is the whole of who may relocate a body — `Emulate`'s
    /// gate at the same door, so the picker greys the row with a reason
    /// rather than hiding it.
    #[test]
    fn only_the_player_may_relocate_a_body() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 40);
        let id = built_core(&mut game, crate::routine_craft::Core::Teleport);
        let def = game
            .world
            .resource::<AbilityDb>()
            .get(&id)
            .cloned()
            .expect("teleport compiles");
        let player = game.player_entity();
        assert!(
            game.ability_unavailable(player, &def).is_none(),
            "the player was refused their own routine"
        );
        // A full reserve on the hostile is load-bearing, and the trap it
        // closes is the one `ability_unavailable`'s own doc records: a
        // hostile holds no `PowerReserve` by design, so *every* priced
        // routine is already refused it and an assertion made without this
        // line passes with the player-only gate deleted.
        game.world.entity_mut(pack[0]).insert(PowerReserve::new(
            crate::components::POWER_MAX,
            crate::components::POWER_MAX,
        ));
        assert_eq!(
            game.ability_unavailable(pack[0], &def)
                .map(|r| r.to_string()),
            Some("only you can relocate".to_string()),
            "a body that could afford Teleport was offered it"
        );
    }

    /// todo #103's own load-bearing property: `ability_unavailable` checks
    /// Power *last*, so `Some(RoutineRefusal::Power { .. })` already means
    /// every earlier refusal passed — the party arm's "is Power the sole
    /// reason" question is one call rather than a second, Power-skipping
    /// copy of this gate. A non-player body refused for ownership *and*
    /// short of Power must still read as ownership, never as Power — the
    /// mutation this guards against is Power moving back to its old spot,
    /// ahead of the Decompile/Emulate/Teleport arms.
    #[test]
    fn ability_unavailable_prefers_an_earlier_refusal_over_power_when_both_apply() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 40);
        let id = built_core(&mut game, crate::routine_craft::Core::Teleport);
        let def = game
            .world
            .resource::<AbilityDb>()
            .get(&id)
            .cloned()
            .expect("teleport compiles");
        // Refused twice over: not the player, and (unlike the test above) no
        // reserve at all to pay Teleport's cost with.
        game.world
            .entity_mut(pack[0])
            .insert(PowerReserve::new(0.0, crate::components::POWER_MAX));
        assert_eq!(
            game.ability_unavailable(pack[0], &def)
                .map(|r| r.to_string()),
            Some("only you can relocate".to_string()),
            "a doubly-refused routine read as short of Power instead of the earlier refusal"
        );
    }

    /// The one-aim door refuses it, `Emulate`'s arm and its reason: there
    /// are two cells to collect and that door validates one.
    #[test]
    fn the_single_aim_door_refuses_a_relocation() {
        let mut game = game();
        tactical_fight(&mut game, 1, 40);
        let player = armed(&mut game);
        let at = cell_of(&game, player);
        let power = power_of(&game, player);

        assert!(
            !game.tactical_use_routine(0, at),
            "the single-aim door ran a relocation with no destination"
        );
        spent_nothing(&game, player, power);
    }

    /// The player relocates themselves, which is the routine's common case.
    #[test]
    fn the_player_relocates_themselves_and_spends_the_turn() {
        let mut game = game();
        tactical_fight(&mut game, 1, 40);
        let player = armed(&mut game);
        let from = cell_of(&game, player);
        let to = destination(&game, from);

        assert!(
            game.tactical_teleport(0, from, to),
            "the relocation was refused"
        );
        assert_eq!(
            game.world.resource::<TacticalBattle>().cell_of(player),
            Some(to),
            "the player did not move"
        );
        assert_ne!(
            game.tactical_actor(),
            Some(player),
            "the relocation ran and the player kept the turn"
        );
    }

    /// A body at arm's length is a legal subject whichever side it is on —
    /// flinging an adjacent hostile back is the control half of the routine.
    #[test]
    fn an_adjacent_hostile_is_a_legal_subject() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 40);
        let player = armed(&mut game);
        let beside = free_neighbour(&game, cell_of(&game, player));
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(pack[0], beside)
        );
        let to = destination(&game, beside);

        assert!(
            game.tactical_teleport(0, beside, to),
            "the hostile was not thrown"
        );
        assert_eq!(
            game.world.resource::<TacticalBattle>().cell_of(pack[0]),
            Some(to),
            "the hostile did not move"
        );
    }

    /// Touching distance is the whole of what may be picked up, and it is
    /// not the same figure as the reach — at `LEVEL` the reach is four.
    #[test]
    fn a_subject_out_of_arms_reach_is_refused_before_anything_is_spent() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 40);
        let player = armed(&mut game);
        let at = cell_of(&game, player);
        let two_away = game
            .teleport_destinations(at)
            .into_iter()
            .find(|&cell| distance(at, cell) == 2)
            .expect("no cell two steps out");
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(pack[0], two_away)
        );
        let landing = destination(&game, two_away);
        assert_ne!(landing, at, "the fixture aimed at the player's own cell");
        let power = power_of(&game, player);

        assert!(
            !game.tactical_teleport(0, two_away, landing),
            "a body two cells away was picked up"
        );
        spent_nothing(&game, player, power);
    }

    /// Empty ground is not a subject.
    #[test]
    fn an_empty_subject_cell_is_refused_before_anything_is_spent() {
        let mut game = game();
        tactical_fight(&mut game, 1, 40);
        let player = armed(&mut game);
        let at = cell_of(&game, player);
        let empty = free_neighbour(&game, at);
        let power = power_of(&game, player);

        assert!(
            !game.tactical_teleport(0, empty, destination(&game, at)),
            "empty ground was relocated"
        );
        spent_nothing(&game, player, power);
    }

    /// The destination is measured from the *subject*, against the
    /// *invoker's* reach.
    #[test]
    fn a_destination_past_the_reach_is_refused_before_anything_is_spent() {
        let mut game = game();
        tactical_fight(&mut game, 1, 40);
        let player = armed(&mut game);
        let from = cell_of(&game, player);
        let reach = teleport_reach(LEVEL);
        let battle = game.world.resource::<TacticalBattle>();
        let too_far = battle
            .board
            .cells()
            .map(|(cell, _)| cell)
            .find(|&(x, y)| battle.board.walkable(x, y) && distance(from, (x, y)) == reach + 1)
            .expect("the board is wider than one reach");
        let power = power_of(&game, player);

        assert!(
            !game.tactical_teleport(0, from, too_far),
            "a destination past the reach was accepted"
        );
        spent_nothing(&game, player, power);
    }

    /// An occupied destination is refused — `TacticalBattle::move_to`'s own
    /// rule, asked before anything is spent rather than discovered after.
    #[test]
    fn an_occupied_destination_is_refused_before_anything_is_spent() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 40);
        let player = armed(&mut game);
        let at = cell_of(&game, player);
        let beside = free_neighbour(&game, at);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(pack[0], beside)
        );
        let power = power_of(&game, player);

        assert!(
            !game.tactical_teleport(0, at, beside),
            "the player was relocated onto an occupied cell"
        );
        spent_nothing(&game, player, power);
    }

    /// The first stage's outline offers exactly the bodies the door will
    /// accept — the acting body's own cells included, and nothing standing
    /// further out.
    #[test]
    fn the_subject_outline_offers_every_body_at_arms_length_and_no_other() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 40);
        let player = armed(&mut game);
        let at = cell_of(&game, player);
        let beside = free_neighbour(&game, at);
        place_one(&mut game, pack[0], beside);

        let offered = game.teleport_subjects();
        assert!(
            offered.contains(&at),
            "the outline did not offer the player their own cell"
        );
        assert!(
            offered.contains(&beside),
            "the outline did not offer the hostile standing beside them"
        );

        // Walk the same body out of reach and it must leave the outline.
        let two_away = game
            .teleport_destinations(at)
            .into_iter()
            .find(|&cell| distance(at, cell) == 2)
            .expect("no cell two steps out");
        place_one(&mut game, pack[0], two_away);
        assert!(
            !game.teleport_subjects().contains(&two_away),
            "the outline offered a body two cells away"
        );
    }

    /// A cell the invoker cannot see is refused, and dropped from the
    /// outline by the same call — `reach::aim_in_sight` with a `Single`
    /// shape, which is `line_of_sight` from the invoker's own cell.
    ///
    /// Asserted in both directions off one blocked cell: a refusal the
    /// outline still offered would be a cursor the player can move onto and
    /// not commit from.
    #[test]
    fn a_destination_out_of_sight_is_refused_and_never_offered() {
        let mut game = game();
        tactical_fight(&mut game, 1, 40);
        let player = armed(&mut game);
        let from = cell_of(&game, player);
        // Two cells out along a straight run, with the cell between them
        // walled: `line_of_sight` excludes its endpoints, so a neighbour
        // could never be hidden by anything.
        let (behind, wall) = game
            .teleport_destinations(from)
            .into_iter()
            .find_map(|cell| {
                let between = (
                    from.0 + (cell.0 - from.0) / 2,
                    from.1 + (cell.1 - from.1) / 2,
                );
                (distance(from, cell) == 2 && between != from && between != cell)
                    .then_some((cell, between))
            })
            .expect("no cell two steps out with one between");
        block_cell(&mut game, wall);
        let power = power_of(&game, player);

        assert!(
            !game.teleport_destinations(from).contains(&behind),
            "the outline offered a cell the invoker cannot see"
        );
        assert!(
            !game.tactical_teleport(0, from, behind),
            "a destination out of sight was accepted"
        );
        spent_nothing(&game, player, power);
    }

    /// The outline the player aims with and the refusal that enforces it are
    /// the same two predicates — every cell offered must actually be taken.
    #[test]
    fn every_cell_the_outline_offers_is_one_the_door_accepts() {
        let mut game = game();
        tactical_fight(&mut game, 1, 40);
        let player = armed(&mut game);
        let from = cell_of(&game, player);
        let offered = game.teleport_destinations(from);
        assert!(offered.len() > 1, "the outline offered nothing to check");

        for cell in offered {
            assert!(
                distance(from, cell) <= teleport_reach(LEVEL),
                "the outline offered {cell:?}, past the reach"
            );
            assert!(
                game.world
                    .resource::<TacticalBattle>()
                    .occupant(cell)
                    .is_none_or(|body| body == player),
                "the outline offered {cell:?}, which somebody is standing on"
            );
        }
    }
}
