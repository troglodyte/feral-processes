use super::*;

/// Seating a forked program on a battle map: beside the invoker, behind the
/// cursor, and gone whichever way the fight ends.
mod summons {
    use super::*;
    use crate::components::Summoned;

    /// A fight with `count` hostiles and one fork already seated beside the
    /// player.
    fn fight_with_a_fork(game: &mut Game, count: usize) -> Entity {
        tactical_fight(game, count, 40);
        let player = game.player_entity();
        let body = game.fork_programs(player, 1, 0)[0];
        assert!(
            game.seat_summon_on_board(player, body),
            "there is room beside the player"
        );
        body
    }

    #[test]
    fn a_fork_lands_on_a_free_walkable_cell_beside_its_invoker() {
        let mut game = game();
        let body = fight_with_a_fork(&mut game, 2);
        let battle = game.world.resource::<TacticalBattle>();
        let player = battle
            .cell_of(game.player_entity())
            .expect("the player stands somewhere");
        let at = battle.cell_of(body).expect("the fork was placed");
        assert!(battle.board.walkable(at.0, at.1));
        assert_eq!(
            battle.bodies().filter(|&(_, cell)| cell == at).count(),
            1,
            "and on nobody else's cell"
        );
        assert!(
            (at.0 - player.0).abs() <= 1 && (at.1 - player.1).abs() <= 1,
            "beside the invoker: {at:?} against {player:?}"
        );
    }

    /// *A count of turns taken is conserved under a cursor shift and would
    /// pass against the bug*, so this compares the order **by identity**.
    #[test]
    fn splicing_a_fork_costs_nobody_else_a_turn() {
        let mut game = game();
        tactical_fight(&mut game, 3, 40);
        let before: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .initiative()
            .to_vec();
        // Somewhere in the middle of the order, so an insertion ahead of the
        // cursor would have somewhere to go wrong.
        game.tactical_end_turn();
        let cursor = game
            .world
            .resource::<TacticalBattle>()
            .actor()
            .expect("somebody is acting");

        let player = game.player_entity();
        let body = game.fork_programs(player, 1, 0)[0];
        assert!(game.seat_summon_on_board(player, body));

        let after: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .initiative()
            .to_vec();
        let spliced: Vec<Entity> = after.iter().copied().filter(|&e| e != body).collect();
        assert_eq!(spliced, before, "nobody else moved in the order");
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(cursor),
            "and the cursor still names the body that was acting"
        );
        let at = after.iter().position(|&e| e == body).expect("it is in");
        let on = after.iter().position(|&e| e == cursor).expect("so is it");
        assert_eq!(at, on + 1, "immediately behind the cursor: it acts next");
    }

    /// A fork drives itself, which is the first exception to "every party
    /// body is the player's to command".
    #[test]
    fn a_fork_takes_its_own_turn_without_waiting_for_input() {
        let mut game = game();
        let body = fight_with_a_fork(&mut game, 2);
        assert!(wait_for_turn(&mut game, body));
        assert!(
            !game.tactical_awaits_input(),
            "the fight would hang waiting for a key nobody may press"
        );
        assert!(game.tactical_ai_turn(), "and the AI drove it");
    }

    /// The two endings only a battle map can reach. Task 1's sweep covers
    /// the group model's teardown, but a jack-out and a walk off the edge
    /// have never run it.
    #[test]
    fn no_fork_survives_a_jack_out() {
        let mut game = game();
        let body = fight_with_a_fork(&mut game, 2);
        let player = game.player_entity();
        assert!(wait_for_turn(&mut game, player));
        let edge = western_edge(&game);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, edge)
        );

        assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Departed);
        assert!(game.world.get_resource::<TacticalBattle>().is_none());
        assert!(
            game.world.get::<Stats>(body).is_none(),
            "a jack-out is one of the five endings, and the sweep covers it"
        );
    }

    #[test]
    fn a_fork_that_walks_off_the_edge_is_still_swept_at_the_end() {
        let mut game = game();
        let body = fight_with_a_fork(&mut game, 1);
        assert!(wait_for_turn(&mut game, body));
        let edge = western_edge(&game);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(body, edge)
        );
        assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Departed);
        assert!(
            game.world.get::<Stats>(body).is_some(),
            "breaking off is not dying"
        );
        assert!(
            game.world.get::<Summoned>(body).is_some(),
            "and it is still a fork"
        );

        // Now the player walks out too, so the fight tears down around a
        // body that is no longer on the board at all.
        let player = game.player_entity();
        assert!(wait_for_turn(&mut game, player));
        let edge = western_edge(&game);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, edge)
        );
        assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Departed);
        assert!(
            game.world.get::<Stats>(body).is_none(),
            "the sweep is over every holder, not over the board"
        );
    }

    /// Every refusal before anything is spent — `commit_caravan_basket`'s
    /// rule, which `tactical_use_routine` already states for its six.
    #[test]
    fn a_fork_with_nowhere_to_stand_is_refused_and_spends_nothing() {
        let mut game = game();
        tactical_fight(&mut game, 2, 40);
        let player = game.player_entity();
        // Fill the board, so `nearest_free` has nothing to answer with.
        let cells: Vec<(i32, i32)> = {
            let battle = game.world.resource::<TacticalBattle>();
            (0..battle.board.side)
                .flat_map(|x| (0..battle.board.side).map(move |y| (x, y)))
                .filter(|&(x, y)| battle.board.walkable(x, y))
                .filter(|&at| battle.occupant(at).is_none())
                .collect()
        };
        for at in cells {
            let filler = game.world.spawn(()).id();
            game.world
                .resource_mut::<TacticalBattle>()
                .place(filler, at);
        }

        let body = game.fork_programs(player, 1, 0)[0];
        assert!(
            !game.seat_summon_on_board(player, body),
            "there is nowhere for it to stand"
        );
        assert!(
            game.world
                .resource::<TacticalBattle>()
                .cell_of(body)
                .is_none()
        );
    }

    /// **Reproducer.** A fork is never `Hostile` and never joins `Party`, so
    /// `reap_tactical_dead`'s base-staff branch — `body != player &&
    /// !Party.contains(body)` — is exactly what a dead fork also satisfies.
    /// A fork killed on a battle map must be swept silently, the same as
    /// `dissolve_summons`/`finish_fight` sweep it everywhere else, not
    /// logged and revived as if it were a downed companion.
    #[test]
    fn a_fork_killed_on_a_battle_map_is_swept_not_logged_or_benched() {
        let mut game = game();
        let body = fight_with_a_fork(&mut game, 2);
        game.world.get_mut::<Stats>(body).unwrap().hp = 0;

        game.tactical_round_upkeep();

        assert!(
            !log_texts(&game)
                .iter()
                .any(|t| t.contains("falls in the siege")),
            "a fork is not base staff: {:#?}",
            log_texts(&game)
        );
        assert_eq!(
            game.world.get::<Stats>(body).map(|s| s.hp),
            Some(0),
            "left dead where it stands for finish_fight's own sweep, not revived to 1 hp"
        );
        assert!(
            game.world.get::<crate::components::Downed>(body).is_none(),
            "never benched as a downed program"
        );
        let fallen = game.world.resource::<TacticalBattle>().fallen().to_vec();
        assert_eq!(fallen.len(), 1, "only the fork died: {fallen:#?}");
        assert!(
            fallen[0].raise.is_none(),
            "a fork is already Summoned; Respawn does not raise the temporary"
        );
    }
}

/// Respawn: raising fallen bodies onto the caster's own side (party-side
/// casts only — a hostile carrier is Phase 2's).
mod respawn {
    use super::*;
    use crate::components::{PowerReserve, Respawned, Routines, Summoned, WanderAi};

    fn give_respawn(game: &mut Game, who: Entity) {
        game.world
            .entity_mut(who)
            .insert(Routines(vec!["respawn".to_string()]));
    }

    /// Kills `target` with the player's own attacks — `a_body_killed_on_a_
    /// battle_map_leaves_a_mark_where_it_fell`'s loop — and returns the cell
    /// it fell on.
    fn kill_with_the_player(game: &mut Game, target: Entity) -> (i32, i32) {
        let player = game.player_entity();
        let mut last_seen = None;
        for _ in 0..64 {
            if game.world.get::<Stats>(target).is_none() {
                break;
            }
            if !wait_for_turn(game, player) {
                break;
            }
            let Some(at) = game.world.resource::<TacticalBattle>().cell_of(target) else {
                break;
            };
            last_seen = Some(at);
            if let Some(near) = beside(game, at) {
                game.world
                    .resource_mut::<TacticalBattle>()
                    .move_to(player, near);
            }
            game.tactical_attack(target);
        }
        last_seen.expect("the target stood somewhere")
    }

    /// Every free, unoccupied cell within `radius` of `aim`, nearest first —
    /// the same Chebyshev metric `Game::raisable_marks` sorts by, so a test
    /// can place fallen marks at known distances from an aim cell.
    fn free_cells_within(game: &Game, aim: (i32, i32), radius: i32) -> Vec<(i32, i32)> {
        let battle = game.world.resource::<TacticalBattle>();
        let mut cells: Vec<(i32, i32)> = (0..battle.board.side)
            .flat_map(|x| (0..battle.board.side).map(move |y| (x, y)))
            .filter(|&(x, y)| battle.board.walkable(x, y))
            .filter(|&c| battle.occupant(c).is_none())
            .filter(|&c| crate::tactical::reach::distance(c, aim) as i32 <= radius)
            .collect();
        cells.sort_by_key(|&c| crate::tactical::reach::distance(c, aim));
        cells
    }

    /// Places a plain hostile `Creature` on `cell` with `stats`, kills it,
    /// and reaps it into a `Fallen` mark — full control over where a
    /// raisable mark ends up, without going through a real fight's own
    /// deployment.
    fn drop_a_fallen_mark(game: &mut Game, cell: (i32, i32), stats: Stats) {
        let body = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                stats,
                StatusEffects::default(),
            ))
            .id();
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .place(body, cell),
            "the cell must be free to place a fallen-mark candidate on"
        );
        game.world.get_mut::<Stats>(body).unwrap().hp = 0;
        game.tactical_round_upkeep();
    }

    #[test]
    fn a_party_cast_raises_the_fallen_at_075x_and_removes_the_mark() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 2, 1);
        let target = pack[0];
        // `hp: 1` keeps the kill one-shot; the rest are set well above 1 so
        // the multiplier's own value (0.75, not merely "less than the
        // original") is what the assertions below actually pin down —
        // rounding a base stat of 1 cannot distinguish 0.75 from 0.9.
        *game.world.get_mut::<Stats>(target).unwrap() = Stats {
            hp: 1,
            max_hp: 40,
            atk: 20,
            mitigation: 12,
        };
        let stats_before = *game.world.get::<Stats>(target).unwrap();
        let species = game.world.get::<Creature>(target).unwrap().species.clone();
        let cell = kill_with_the_player(&mut game, target);

        let player = game.player_entity();
        give_respawn(&mut game, player);
        assert!(wait_for_turn(&mut game, player));
        assert!(
            game.tactical_use_routine(0, cell),
            "respawn must land on its own mark"
        );

        assert!(
            !game
                .world
                .resource::<TacticalBattle>()
                .fallen()
                .iter()
                .any(|f| f.cell == cell),
            "the mark must be removed, not merely left unraisable"
        );
        let raised = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Respawned>(e).is_some())
            .expect("a raised body must be seated");
        assert!(game.world.get::<Summoned>(raised).is_some());
        assert!(game.world.get::<Hostile>(raised).is_none());
        assert!(game.world.get::<WanderAi>(raised).is_none());
        assert_eq!(game.world.get::<Creature>(raised).unwrap().species, species);
        let stats = *game.world.get::<Stats>(raised).unwrap();
        let expect = |v: i32| (v as f32 * 0.75).round() as i32;
        assert_eq!(stats.max_hp, expect(stats_before.max_hp));
        assert_eq!(stats.atk, expect(stats_before.atk));
        assert_eq!(stats.mitigation, expect(stats_before.mitigation));
        assert_eq!(stats.hp, stats.max_hp, "a raised body comes up at full hp");
    }

    /// `Perk::SpawnPriority`'s bonus is added on top of `REANIMATE_STAT_MULT`
    /// for a party-side cast, the same per-level amount `fork_programs` adds
    /// to `SUMMON_STAT_MULT` — three levels clears 1.0x, which the spec
    /// states is intended rather than capped.
    #[test]
    fn spawn_priority_raises_a_party_cast_above_075x() {
        use crate::components::Perks;
        use crate::perks::Perk;

        let mut game = game();
        let pack = tactical_fight(&mut game, 2, 1);
        let target = pack[0];
        *game.world.get_mut::<Stats>(target).unwrap() = Stats {
            hp: 1,
            max_hp: 40,
            atk: 20,
            mitigation: 12,
        };
        let stats_before = *game.world.get::<Stats>(target).unwrap();
        let cell = kill_with_the_player(&mut game, target);

        let player = game.player_entity();
        give_respawn(&mut game, player);
        game.world.entity_mut(player).insert(Perks {
            points: 0,
            unlocked: vec![Perk::SpawnPriority; 3],
        });
        assert!(wait_for_turn(&mut game, player));
        assert!(game.tactical_use_routine(0, cell));

        let raised = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Respawned>(e).is_some())
            .expect("a raised body must be seated");
        let stats = *game.world.get::<Stats>(raised).unwrap();
        // 0.75 + 3 * 0.10 = 1.05.
        let expect = |v: i32| (v as f32 * 1.05).round() as i32;
        assert_eq!(stats.max_hp, expect(stats_before.max_hp));
        assert!(
            stats.max_hp as f32 > stats_before.max_hp as f32 * 0.75,
            "three levels of Spawn Priority must raise the multiplier past 0.75x"
        );
    }

    /// A hostile cast reads no perks at all — `summon_rarity_window`'s own
    /// rule, borrowed here — so a hostile carrier's raise is unaffected by
    /// Spawn Priority levels the *player* holds.
    #[test]
    fn a_hostile_raise_ignores_the_players_perk() {
        use crate::components::Perks;
        use crate::perks::Perk;

        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 400);
        let carrier = pack[0];
        only_routine(&mut game, carrier, "respawn");
        let carrier_at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(carrier)
            .expect("the carrier is seated");
        let mark_cell = beside(&game, carrier_at).expect("room beside the carrier");
        let mark_stats = Stats {
            hp: 20,
            max_hp: 40,
            atk: 20,
            mitigation: 12,
        };
        drop_a_fallen_mark(&mut game, mark_cell, mark_stats);

        let player = game.player_entity();
        game.world.entity_mut(player).insert(Perks {
            points: 0,
            unlocked: vec![Perk::SpawnPriority; 3],
        });

        assert!(wait_for_turn(&mut game, carrier));
        assert!(game.tactical_ai_turn(), "the carrier's turn was not run");

        let raised = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Respawned>(e).is_some())
            .expect("the carrier must have raised the mark");
        let stats = *game.world.get::<Stats>(raised).unwrap();
        let expect = |v: i32| (v as f32 * 0.75).round() as i32;
        assert_eq!(
            stats.max_hp,
            expect(mark_stats.max_hp),
            "the player's Spawn Priority levels must not reach a hostile's own raise"
        );
    }

    #[test]
    fn a_cast_raises_at_most_count_nearest_first() {
        let mut game = game();
        // One live hostile, hard to kill, so the fight stays open around
        // the manually seeded marks below.
        tactical_fight(&mut game, 1, 400);
        let player = game.player_entity();
        let player_at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(player)
            .expect("the player is seated");
        let aim = beside(&game, player_at).expect("room beside the player");
        let cells = free_cells_within(&game, aim, 2);
        assert!(
            cells.len() >= 4,
            "the board must offer at least 4 free cells near the aim: {cells:?}"
        );
        let candidates = &cells[..4];
        for &cell in candidates {
            drop_a_fallen_mark(
                &mut game,
                cell,
                Stats {
                    hp: 20,
                    max_hp: 20,
                    atk: 5,
                    mitigation: 0,
                },
            );
        }

        give_respawn(&mut game, player);
        assert!(wait_for_turn(&mut game, player));
        assert!(game.tactical_use_routine(0, aim));

        let still_marked: Vec<(i32, i32)> = game
            .world
            .resource::<TacticalBattle>()
            .fallen()
            .iter()
            .filter(|f| f.raise.is_some())
            .map(|f| f.cell)
            .collect();
        assert_eq!(
            still_marked,
            vec![candidates[3]],
            "the farthest of the four must be the one left over"
        );
        let raised = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .filter(|&(e, _)| game.world.get::<Respawned>(e).is_some())
            .count();
        assert_eq!(raised, 3, "respawn.ron authors count: 3");
    }

    #[test]
    fn a_cast_with_nothing_raisable_in_reach_is_refused_and_spends_no_power() {
        let mut game = game();
        tactical_fight(&mut game, 1, 400);
        let player = game.player_entity();
        give_respawn(&mut game, player);
        assert!(wait_for_turn(&mut game, player));
        let player_at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(player)
            .unwrap();
        let before = game.world.get::<PowerReserve>(player).unwrap().get();

        assert!(
            !game.tactical_use_routine(0, player_at),
            "nothing has fallen anywhere near the player yet"
        );

        assert_eq!(
            game.world.get::<PowerReserve>(player).unwrap().get(),
            before,
            "a refusal spends no Power"
        );
    }

    /// `respawn_refusal` and `best_aim`/`reanimate` must read "raisable"
    /// off one predicate. A mark whose own cell is occupied is still
    /// raisable when a neighbour is free — `Game::reanimate`'s own
    /// `nearest_free` fallback — so the player's refusal must not be
    /// stricter than what a cast would actually do.
    #[test]
    fn a_mark_behind_a_live_blocker_with_a_free_neighbour_is_still_raisable() {
        let mut game = game();
        tactical_fight(&mut game, 1, 400);
        let player = game.player_entity();
        let player_at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(player)
            .expect("the player is seated");
        let aim = beside(&game, player_at).expect("room beside the player");
        drop_a_fallen_mark(
            &mut game,
            aim,
            Stats {
                hp: 20,
                max_hp: 20,
                atk: 5,
                mitigation: 0,
            },
        );
        let blocker = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 0,
                    mitigation: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .place(blocker, aim),
            "the mark's own cell must be free to plant the blocker on it"
        );

        give_respawn(&mut game, player);
        assert!(wait_for_turn(&mut game, player));

        assert!(
            game.tactical_use_routine(0, aim),
            "a mark whose own cell is blocked but has a free neighbour is \
             still raisable, the same room reanimate's own nearest_free \
             fallback would seat it on"
        );
    }

    #[test]
    fn a_raised_body_that_falls_again_leaves_a_plain_mark() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 2, 1);
        let target = pack[0];
        let cell = kill_with_the_player(&mut game, target);
        let player = game.player_entity();
        give_respawn(&mut game, player);
        assert!(wait_for_turn(&mut game, player));
        assert!(game.tactical_use_routine(0, cell));
        let raised = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Respawned>(e).is_some())
            .expect("a raised body must be seated");

        let raised_cell = kill_with_the_player(&mut game, raised);

        let mark = game
            .world
            .resource::<TacticalBattle>()
            .fallen()
            .iter()
            .find(|f| f.cell == raised_cell)
            .cloned();
        assert!(
            mark.is_some_and(|m| m.raise.is_none()),
            "a raised body is Summoned; its own death leaves an unraisable mark"
        );
    }

    /// A squad shell (`game::squads::spawn_squad`) is a non-`Boss`,
    /// non-`Summoned` `Creature` with every member's stats summed into one —
    /// `raisable_snapshot`'s existing exclusions do not catch it, so without
    /// its own guard Respawn would raise the shell as one oversized body
    /// instead of leaving a plain mark the way a real squad death does
    /// (`Game::reap_squad` never reads `raise` at all).
    #[test]
    fn a_dead_squad_shell_leaves_an_unraisable_mark() {
        let mut game = game();
        tactical_fight(&mut game, 1, 400);
        let player = game.player_entity();
        let player_at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(player)
            .expect("the player is seated");
        let cell = free_cells_within(&game, player_at, 3)
            .into_iter()
            .next()
            .expect("the board offers a free cell near the player");
        let squad = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                Stats {
                    hp: 0,
                    max_hp: 80,
                    atk: 20,
                    mitigation: 0,
                },
                StatusEffects::default(),
                Squad {
                    members: Vec::new(),
                    formation: 0,
                },
            ))
            .id();
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .place(squad, cell),
            "the chosen cell must be free to place the shell on"
        );
        game.tactical_round_upkeep();

        let mark = game
            .world
            .resource::<TacticalBattle>()
            .fallen()
            .iter()
            .find(|f| f.cell == cell)
            .cloned();
        assert!(
            mark.is_some_and(|m| m.raise.is_none()),
            "a squad shell must leave a plain, unraisable mark rather than \
             one Respawn would raise as a single oversized body"
        );
    }

    #[test]
    fn recast_replaces_only_its_own_sides_set_while_a_fork_survives() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 3, 1);
        let first_target = pack[0];
        let cell_one = kill_with_the_player(&mut game, first_target);
        let player = game.player_entity();
        give_respawn(&mut game, player);
        assert!(wait_for_turn(&mut game, player));
        assert!(game.tactical_use_routine(0, cell_one));
        let first_raised = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Respawned>(e).is_some())
            .expect("the first cast must have raised a body");

        let fork = game.fork_programs(player, 1, 0)[0];
        assert!(game.seat_summon_on_board(player, fork));

        let second_target = pack[1];
        let cell_two = kill_with_the_player(&mut game, second_target);
        assert!(wait_for_turn(&mut game, player));
        // Off cooldown: this test is about the dissolve, not the wait.
        game.world
            .entity_mut(player)
            .remove::<crate::components::AbilityCooldowns>();
        assert!(game.tactical_use_routine(0, cell_two));

        assert!(
            !game.creature_alive(first_raised),
            "the first cast's own raised body must be replaced"
        );
        assert!(
            game.creature_alive(fork),
            "a fork is not Respawn's to dissolve"
        );
        let second_raised = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .filter(|&(e, _)| game.world.get::<Respawned>(e).is_some())
            .count();
        assert_eq!(second_raised, 1, "exactly the second cast's own body");
    }

    #[test]
    fn every_raised_body_is_gone_after_finish_fight() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 2, 1);
        let target = pack[0];
        let cell = kill_with_the_player(&mut game, target);
        let player = game.player_entity();
        give_respawn(&mut game, player);
        assert!(wait_for_turn(&mut game, player));
        assert!(game.tactical_use_routine(0, cell));
        let raised = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Respawned>(e).is_some())
            .expect("a raised body must be seated");
        assert!(game.world.get::<Stats>(raised).is_some());

        let last = pack[1];
        kill_with_the_player(&mut game, last);

        assert!(
            game.world.get_resource::<TacticalBattle>().is_none(),
            "the last hostile's death must have closed the fight"
        );
        assert!(
            game.world.get::<Stats>(raised).is_none(),
            "finish_fight sweeps every Summoned body, raised or forked"
        );
    }

    /// A hostile carrier's own raised body is `Hostile` *and* `Summoned`, so
    /// `reap_tactical_dead`'s `Hostile` branch — checked before the
    /// `Summoned` guard the fork reproducer added — must not be the one
    /// that sees it first: it pays a kill's full reward (XP, loot,
    /// `DownedProgram`), same as any other hostile.
    #[test]
    fn killing_a_hostile_raised_body_pays_nothing() {
        let mut game = game();
        // One real hostile kept alive on a lot of hp, so the fight stays
        // open around the hand-placed raised body below.
        tactical_fight(&mut game, 1, 400);
        let player = game.player_entity();
        let player_at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(player)
            .expect("the player is seated");
        let cell = beside(&game, player_at).expect("room beside the player");
        let body = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                Summoned,
                Respawned,
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    mitigation: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .place(body, cell),
            "the cell must be free to seat the raised body on"
        );
        let xp_before = game
            .world
            .get::<Experience>(player)
            .map(|e| e.xp)
            .unwrap_or(0);
        let downed_before = game
            .world
            .get::<crate::components::DownedPrograms>(player)
            .map(|d| d.0.len())
            .unwrap_or(0);

        game.world.get_mut::<Stats>(body).unwrap().hp = 0;
        game.tactical_round_upkeep();

        assert_eq!(
            game.world.get::<Experience>(player).map(|e| e.xp),
            Some(xp_before),
            "a hostile raised body pays no XP"
        );
        assert_eq!(
            game.world
                .get::<crate::components::DownedPrograms>(player)
                .map(|d| d.0.len())
                .unwrap_or(0),
            downed_before,
            "a hostile raised body drops no DownedProgram"
        );
        assert!(
            !log_texts(&game)
                .iter()
                .any(|t| t.contains("crashes and deletes itself")),
            "a hostile raised body's death is not a paid kill"
        );
        assert_eq!(
            game.world.get::<Stats>(body).map(|s| s.hp),
            Some(0),
            "left dead where it stands for finish_fight's own sweep, not despawned here"
        );
    }

    /// A raised body would otherwise be a free capture — it is already
    /// `Hostile`, so `Decompile`'s existing refusal (aimed at something
    /// that is not `Hostile`) does not catch it, and `roster_parts` never
    /// asks whether a body is `Summoned` before joining the roster.
    #[test]
    fn decompile_refuses_a_raised_body() {
        let mut game = game();
        tactical_fight(&mut game, 1, 400);
        let player = game.player_entity();
        only_routine(&mut game, player, "decompile");
        crate::tests::support::set_inventory(&mut game, &[(crate::items::ids::ICE_BREAKER, 50)]);
        let player_at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(player)
            .expect("the player is seated");
        let cell = beside(&game, player_at).expect("room beside the player");
        let body = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                Summoned,
                Respawned,
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    mitigation: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .place(body, cell),
            "the cell must be free to seat the raised body on"
        );
        assert!(wait_for_turn(&mut game, player));

        assert!(
            !game.tactical_use_routine(0, cell),
            "Decompile must refuse a raised body"
        );
        assert!(
            game.world.get::<crate::components::Tamed>(body).is_none(),
            "it must never join the roster"
        );
    }

    /// `dissolve_respawned`'s `Hostile` filter is what keeps a recast to
    /// `Summoned` set per side — without it, a party-side recast would also
    /// dissolve a hostile carrier's own raised body, and vice versa.
    #[test]
    fn a_recast_on_one_side_leaves_the_opposing_sides_raised_body_alive() {
        let mut game = game();
        // Two live hostiles: the carrier, and a second kept alive on a lot
        // of hp so the fight stays open once the carrier's raised body
        // stands beside it.
        let pack = tactical_fight(&mut game, 2, 400);
        let carrier = pack[0];
        only_routine(&mut game, carrier, "respawn");
        let carrier_at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(carrier)
            .expect("the carrier is seated");
        let hostile_mark = beside(&game, carrier_at).expect("room beside the carrier");
        drop_a_fallen_mark(
            &mut game,
            hostile_mark,
            Stats {
                hp: 20,
                max_hp: 20,
                atk: 5,
                mitigation: 0,
            },
        );
        assert!(wait_for_turn(&mut game, carrier));
        assert!(game.tactical_ai_turn(), "the carrier's turn was not run");
        let hostile_raised = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Respawned>(e).is_some())
            .expect("the carrier must have raised a hostile-side body");
        assert!(game.world.get::<Hostile>(hostile_raised).is_some());

        // Now the player raises its own, party-side body.
        let player = game.player_entity();
        give_respawn(&mut game, player);
        let player_at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(player)
            .expect("the player is seated");
        let party_mark = beside(&game, player_at).expect("room beside the player");
        drop_a_fallen_mark(
            &mut game,
            party_mark,
            Stats {
                hp: 20,
                max_hp: 20,
                atk: 5,
                mitigation: 0,
            },
        );
        assert!(wait_for_turn(&mut game, player));
        assert!(game.tactical_use_routine(0, party_mark));

        // Recast on the player's own side: off cooldown and against a fresh
        // mark, this test is about the per-side dissolve, not the wait.
        game.world
            .entity_mut(player)
            .remove::<crate::components::AbilityCooldowns>();
        let second_party_mark = beside(&game, player_at).expect("more room beside the player");
        drop_a_fallen_mark(
            &mut game,
            second_party_mark,
            Stats {
                hp: 20,
                max_hp: 20,
                atk: 5,
                mitigation: 0,
            },
        );
        assert!(wait_for_turn(&mut game, player));
        assert!(game.tactical_use_routine(0, second_party_mark));

        assert!(
            game.creature_alive(hostile_raised),
            "a party-side recast must not touch the hostile side's own raised body"
        );
    }

    /// The wild side's own cast: a hostile carrier chooses Respawn
    /// unconditionally (`tactical_intent`'s hostile branch reads only
    /// `wild_routine_ready`), so `best_aim` scoring a mark it covers is the
    /// only thing standing between that choice and a wasted turn.
    #[test]
    fn a_hostile_carrier_casts_respawn_and_its_raised_body_is_hostile_and_ai_driven() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 400);
        let hostile = pack[0];
        only_routine(&mut game, hostile, "respawn");
        let hostile_at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(hostile)
            .expect("the hostile is seated");
        let mark_cell = beside(&game, hostile_at).expect("room beside the hostile");
        drop_a_fallen_mark(
            &mut game,
            mark_cell,
            Stats {
                hp: 20,
                max_hp: 20,
                atk: 5,
                mitigation: 0,
            },
        );

        assert!(wait_for_turn(&mut game, hostile));
        assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

        let raised = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Respawned>(e).is_some())
            .expect("the hostile carrier must have raised the mark");
        assert!(
            game.world.get::<Hostile>(raised).is_some(),
            "raised onto the caster's own, hostile side"
        );
        assert!(game.world.get::<Summoned>(raised).is_some());
    }

    /// A carrier with nothing to raise in reach must not spend its turn on
    /// Respawn anyway — `best_aim` returning `None` is Reanimate's own
    /// refusal on the AI's door, where `tactical_use_routine`'s
    /// `NothingToRespawn` is the player's.
    ///
    /// **Not just "cooldown stays clear"** — `tactical_intent` choosing
    /// `Intent::Routine` for a Reanimate `best_aim` will reject anyway wastes
    /// the whole turn, since `run_tactical_intent`'s hostile branch does not
    /// fall back to a swing the way its non-hostile one does. The player
    /// stands adjacent (`tactical_fight`'s own placement), so a hostile that
    /// actually falls through to `Intent::Swing` logs a swing line
    /// regardless of whether it hits.
    #[test]
    fn a_hostile_carrier_with_nothing_to_raise_does_not_cast_respawn() {
        use crate::resources::MessageLog;

        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 400);
        let hostile = pack[0];
        only_routine(&mut game, hostile, "respawn");
        assert!(wait_for_turn(&mut game, hostile));
        let lines_before = game.world.resource::<MessageLog>().lines.len();

        assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

        assert!(
            game.world
                .get::<crate::components::AbilityCooldowns>(hostile)
                .is_none_or(|c| !c.0.contains_key("respawn")),
            "nothing was in reach to raise, so the routine must not have run"
        );
        assert!(
            game.world.resource::<MessageLog>().lines.len() > lines_before,
            "with nothing to raise the hostile must swing at the adjacent \
             player instead of wasting its turn"
        );
    }

    /// The party side's own auto-invoke reads the same `best_aim` scoring —
    /// `an_auto_driven_party_body_invokes_a_ready_affordable_routine`'s
    /// pattern, with Respawn standing in for `HOSTILE_SWEEP`.
    #[test]
    fn a_party_body_auto_invokes_respawn_when_something_is_raisable() {
        use crate::components::AbilityCooldowns;
        use crate::tactical::ai::AiBeat;

        let mut game = game();
        // A second live hostile, hard to kill, so the fight stays open once
        // the first is felled to leave a mark for the auto-invoke below.
        let pack = tactical_fight(&mut game, 2, 400);
        let target = pack[0];
        game.world.get_mut::<Stats>(target).unwrap().hp = 1;
        let player = game.player_entity();
        kill_with_the_player(&mut game, target);
        give_respawn(&mut game, player);
        assert!(wait_for_turn(&mut game, player));

        for _ in 0..=TACTICAL_MOVE_MAX {
            if game.tactical_auto_beat() == AiBeat::Acted {
                break;
            }
        }

        assert!(
            game.world
                .get::<AbilityCooldowns>(player)
                .is_some_and(|c| c.0.contains_key("respawn")),
            "an auto-driven turn with a raisable mark in reach did not invoke Respawn"
        );
    }

    /// Phase 3: `TacticalBody.respawned` is what the renderer reads to draw
    /// the Respawn tint — `components::Respawned`'s own presence, and
    /// nothing else, since a fork carries `Summoned` without it.
    #[test]
    fn tacticalbody_respawned_is_set_for_a_raised_body_and_unset_for_a_fork() {
        let mut game = game();
        // A second live hostile, hard to kill, stands in for a fork below.
        let pack = tactical_fight(&mut game, 2, 400);
        let target = pack[0];
        game.world.get_mut::<Stats>(target).unwrap().hp = 1;
        let cell = kill_with_the_player(&mut game, target);
        let player = game.player_entity();
        give_respawn(&mut game, player);
        assert!(wait_for_turn(&mut game, player));
        assert!(
            game.tactical_use_routine(0, cell),
            "respawn must land on its own mark"
        );

        let raised = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Respawned>(e).is_some())
            .expect("a raised body must be seated");
        // A fork stand-in: `Summoned` without `Respawned`, the shape
        // `fork_programs` actually produces — the view must not confuse the
        // two summon pools.
        let fork = pack[1];
        game.world.entity_mut(fork).insert(Summoned);

        let view = game.tactical_view().expect("a fight is open");
        let raised_body = view
            .bodies
            .iter()
            .find(|b| b.entity == raised)
            .expect("the raised body is drawn");
        assert!(
            raised_body.respawned,
            "a raised body's view must flag respawned"
        );
        let fork_body = view
            .bodies
            .iter()
            .find(|b| b.entity == fork)
            .expect("the fork stand-in is drawn");
        assert!(!fork_body.respawned, "a fork must not read as respawned");
    }
}
