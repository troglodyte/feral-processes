use super::*;

/// `Game::tactical_body_is_engaged` and the hand-on's skip of a body with
/// nothing in reach (`crates/engine/src/tactical/turn.rs`).
///
/// A hand-built fight rather than `tactical_fight`'s generated board and
/// bearing-seated pack: these tests need bodies at *exact* distances —
/// adjacent for an engaged pair, far past `TACTICAL_MOVE_MAX +
/// TACTICAL_MELEE_RANGE` for a disengaged one — which a generated board's
/// seeded placement cannot promise. Bodies carry no `Stats` or `Creature`
/// at all, `tactical::mod`'s own fixture style: `movement_allowance` and
/// `swing_range` both derive sensible defaults for an entity with neither,
/// and only `Hostile`'s presence or absence is what this mechanism reads.
mod engagement {
    use super::*;
    use crate::tactical::map::{BattleCell, BattleSpec, Board};
    use crate::world::Biome;

    /// A battle map wide enough that `TACTICAL_MOVE_MAX + TACTICAL_MELEE_RANGE`
    /// cannot bridge one side of it to the other, open everywhere so nothing
    /// about terrain confounds a reach measurement.
    fn open_fight(game: &mut Game) {
        let side = 30;
        let mut board = Board::solid(side);
        for y in 0..side {
            for x in 0..side {
                board.set(x, y, BattleCell::Open);
            }
        }
        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 1,
        };
        game.world
            .insert_resource(TacticalBattle::open(spec, board));
    }

    /// A bare body — no `Creature`, so `movement_allowance`/`swing_range`
    /// both derive their defaults — `Stats` alone so `reap_tactical_dead`
    /// (which `tactical_round_upkeep` runs on every round boundary) does
    /// not read it as already dead and remove it the moment a round wraps.
    /// `Hostile` marks which side it fights for.
    fn plain(game: &mut Game, hostile: bool) -> Entity {
        let mut entity = game.world.spawn(Stats {
            hp: 10,
            max_hp: 10,
            atk: 0,
            mitigation: 0,
        });
        if hostile {
            entity.insert(Hostile);
        }
        entity.id()
    }

    /// Ends `body`'s turn as if it had just spent its one action — the
    /// `set_actions_left(0)` a real `tactical_attack`/`tactical_defend`
    /// leaves behind before calling `hand_on_turn` for real.
    fn end_turn_of(game: &mut Game, body: Entity, round_before: u32) {
        game.world
            .resource_mut::<TacticalBattle>()
            .set_actions_left(0);
        game.hand_on_turn(body, round_before);
    }

    /// A three-body fight: `defender` and `raider` adjacent and on opposing
    /// sides (engaged with each other), `distant` far from both and on
    /// nobody's opposing side in particular — the "staff across the base"
    /// shape, seated in initiative order `[defender, raider, distant]`.
    fn engaged_pair_and_a_distant_body(game: &mut Game) -> (Entity, Entity, Entity) {
        open_fight(game);
        let defender = plain(game, false);
        let raider = plain(game, true);
        let distant = plain(game, false);
        let player = game.player_entity();
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(defender, (0, 0));
            battle.place(raider, (1, 0));
            battle.place(distant, (25, 25));
            // Placed but not seated in the order: `settle_tactical` ends a
            // fight the instant the player is not on the board at all
            // (`cell_of(player).is_none()`), on every round's reap — so a
            // fixture with nobody standing in for the player would tear the
            // fight down the first time a round wraps, well before the
            // skip logic under test ever runs.
            battle.place(player, (29, 29));
            battle.set_initiative(vec![defender, raider, distant]);
        }
        (defender, raider, distant)
    }

    /// A staff body across the base from the fighting is passed over: ending
    /// the engaged pair's two turns skips `distant` without it ever
    /// becoming the actor, and the order wraps into a new round on the way
    /// past it — the concrete, unmissable sign a body was skipped rather
    /// than merely deferred.
    #[test]
    fn a_distant_body_is_skipped_and_the_round_still_advances() {
        let mut game = game();
        let (defender, raider, distant) = engaged_pair_and_a_distant_body(&mut game);

        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(defender)
        );
        end_turn_of(&mut game, defender, 1);
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(raider),
            "the engaged raider must not be skipped"
        );
        assert_eq!(game.world.resource::<TacticalBattle>().round, 1);

        end_turn_of(&mut game, raider, 1);
        let battle = game.world.resource::<TacticalBattle>();
        assert_ne!(
            battle.actor(),
            Some(distant),
            "a disengaged body must not become the actor"
        );
        assert_eq!(
            battle.actor(),
            Some(defender),
            "skipping the last body in the order wraps back to the front"
        );
        assert_eq!(battle.round, 2, "the round must still have advanced");
    }

    /// The same body acts on the round a raider comes into its reach —
    /// engagement is re-evaluated every time the cursor reaches it, never
    /// cached from the round it was skipped in.
    #[test]
    fn a_once_distant_body_acts_once_something_reaches_it() {
        let mut game = game();
        let (defender, raider, distant) = engaged_pair_and_a_distant_body(&mut game);
        end_turn_of(&mut game, defender, 1);
        end_turn_of(&mut game, raider, 1);
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(defender),
            "the fixture must have skipped `distant` once already"
        );

        // A second hostile closes on `distant`.
        let closing = plain(&mut game, true);
        game.world
            .resource_mut::<TacticalBattle>()
            .place(closing, (24, 25));

        end_turn_of(&mut game, defender, 2);
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(raider)
        );
        end_turn_of(&mut game, raider, 2);
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(distant),
            "now in reach of `closing`, `distant` must get its turn"
        );
    }

    /// A board on which nobody is engaged still advances its rounds rather
    /// than hanging — the guard the skip is built on, and the test that
    /// would catch it spinning forever if that guard were ever dropped.
    #[test]
    fn nobody_engaged_still_advances_the_rounds() {
        let mut game = game();
        open_fight(&mut game);
        let a = plain(&mut game, false);
        let b = plain(&mut game, false);
        let c = plain(&mut game, false);
        // A hostile, far from all three — present so `settle_tactical`'s
        // "hostiles > 0" keeps the fight open across a round boundary, but
        // out of everybody's reach so it never makes anyone engaged.
        let distant_hostile = plain(&mut game, true);
        let player = game.player_entity();
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(a, (0, 0));
            battle.place(b, (1, 1));
            battle.place(c, (2, 2));
            battle.place(distant_hostile, (29, 29));
            battle.place(player, (29, 0));
            battle.set_initiative(vec![a, b, c]);
        }

        end_turn_of(&mut game, a, 1);
        assert_eq!(game.world.resource::<TacticalBattle>().actor(), Some(b));
        end_turn_of(&mut game, b, 1);
        assert_eq!(game.world.resource::<TacticalBattle>().actor(), Some(c));
        end_turn_of(&mut game, c, 1);
        let battle = game.world.resource::<TacticalBattle>();
        assert_eq!(battle.actor(), Some(a));
        assert_eq!(
            battle.round, 2,
            "three ordinary hand-ons must reach round 2"
        );
    }

    /// The player is never skipped, even disengaged, with an engaged pair
    /// elsewhere on the board forcing the skip loop to actually run.
    #[test]
    fn the_player_is_never_skipped() {
        let mut game = game();
        open_fight(&mut game);
        let player = game.player_entity();
        let defender = plain(&mut game, false);
        let raider = plain(&mut game, true);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(defender, (0, 0));
            battle.place(player, (25, 25));
            battle.place(raider, (1, 0));
            battle.set_initiative(vec![defender, player, raider]);
        }

        end_turn_of(&mut game, defender, 1);
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(player),
            "the player must never be skipped, disengaged or not"
        );
    }

    /// Alone on the board with nothing in reach, the player still gets a
    /// turn rather than being skipped past.
    #[test]
    fn the_player_alone_on_the_board_is_never_skipped() {
        let mut game = game();
        open_fight(&mut game);
        let player = game.player_entity();
        // Present so `settle_tactical`'s "hostiles > 0" keeps the fight
        // open across the round boundary this test crosses, and far enough
        // that it never puts the player in reach.
        let distant_hostile = plain(&mut game, true);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(player, (0, 0));
            battle.place(distant_hostile, (29, 29));
            battle.set_initiative(vec![player]);
        }
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(player)
        );

        end_turn_of(&mut game, player, 1);
        let battle = game.world.resource::<TacticalBattle>();
        assert_eq!(
            battle.actor(),
            Some(player),
            "the only body on the board must still be handed the next turn"
        );
        assert_eq!(battle.round, 2, "a solitary player still advances rounds");
    }

    fn round_dividers(game: &Game) -> Vec<String> {
        game.message_log(200)
            .into_iter()
            .filter(|e| e.kind == crate::MessageKind::Round)
            .map(|e| e.text)
            .collect()
    }

    /// A battle map's wrap is marked in the log the way a group fight's
    /// round is — one divider naming the round that has just begun — and
    /// the first round, which the fight's opening already marks, is not.
    #[test]
    fn a_round_wrap_logs_one_divider_naming_the_new_round() {
        let mut game = game();
        open_fight(&mut game);
        let player = game.player_entity();
        let distant_hostile = plain(&mut game, true);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(player, (0, 0));
            battle.place(distant_hostile, (29, 29));
            battle.set_initiative(vec![player]);
        }
        assert!(
            round_dividers(&game).is_empty(),
            "round 1 must not log a divider"
        );

        end_turn_of(&mut game, player, 1);
        let dividers = round_dividers(&game);
        assert_eq!(dividers.len(), 1, "one wrap, one divider: {dividers:?}");
        assert!(dividers[0].contains("round 2"), "{dividers:?}");

        end_turn_of(&mut game, player, 2);
        let dividers = round_dividers(&game);
        assert_eq!(dividers.len(), 2, "{dividers:?}");
        assert!(dividers[1].contains("round 3"), "{dividers:?}");
    }
}

/// `Game::routine_tactical_range`/`routine_tactical_shape`: decompile's
/// research reach, read by the door, the aim outline and the preview alike.
/// Catalysts spent are what is counted, since a capture caps at 95% and a
/// roll cannot be forced.
mod decompile_reach {
    use super::*;
    use crate::items::ids::ICE_BREAKER;
    use crate::research::DecompilerUpgrade;
    use crate::tactical::reach;
    use crate::tests::support::{held, research_decompiler, set_inventory, spawn_tamed};

    fn grant(game: &mut Game, range: Option<u32>, radius: Option<u32>) {
        research_decompiler(game, "test_reach", DecompilerUpgrade { range, radius });
    }

    /// A fight of `hostiles` bodies on a lot of hp, the player holding only
    /// decompile and `catalysts` of them, on the player's turn.
    fn ready(game: &mut Game, hostiles: usize, catalysts: u32) -> Vec<Entity> {
        let pack = tactical_fight(game, hostiles, 400);
        let player = game.player_entity();
        only_routine(game, player, "decompile");
        set_inventory(game, &[(ICE_BREAKER, catalysts)]);
        assert!(wait_for_turn(game, player));
        pack
    }

    fn catalysts(game: &Game) -> u32 {
        held(game, &crate::ItemId::from(ICE_BREAKER))
    }

    fn cell(game: &Game, body: Entity) -> (i32, i32) {
        game.world
            .resource::<TacticalBattle>()
            .cell_of(body)
            .expect("seated")
    }

    /// The first free walkable cell `d` from `from` that `from` can see.
    fn free_cell_at(game: &Game, from: (i32, i32), d: u32) -> (i32, i32) {
        let battle = game.world.resource::<TacticalBattle>();
        battle
            .board
            .cells()
            .map(|(c, _)| c)
            .find(|&c| {
                reach::distance(from, c) == d
                    && battle.board.walkable(c.0, c.1)
                    && battle.occupant(c).is_none()
                    && reach::line_of_sight(&battle.board, from, c)
            })
            .expect("a free cell at that distance")
    }

    /// Seats `body` on a free cell `d` from `from`.
    fn put_at(game: &mut Game, body: Entity, from: (i32, i32), d: u32) -> (i32, i32) {
        let at = free_cell_at(game, from, d);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(body, at)
        );
        at
    }

    /// Hostiles `a` and `b` and a friendly, all beside `a`, with the player
    /// one cell from `a` and the rest of the pack parked out of the blast.
    fn blast_of_three(game: &mut Game, pack: &[Entity]) -> ((i32, i32), Entity) {
        let player = game.player_entity();
        let from = cell(game, player);
        let a = put_at(game, pack[0], from, 1);
        put_at(game, pack[1], a, 1);
        let friend = game
            .world
            .spawn((
                Creature {
                    species: generic_species().id,
                },
                Stats {
                    hp: 50,
                    max_hp: 50,
                    atk: 1,
                    mitigation: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        let friend_at = free_cell_at(game, a, 1);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .place(friend, friend_at)
        );
        for &far in &pack[2..] {
            let spot = game
                .world
                .resource::<TacticalBattle>()
                .board
                .cells()
                .map(|(c, _)| c)
                .find(|&c| {
                    reach::distance(a, c) >= 4
                        && game
                            .world
                            .resource::<TacticalBattle>()
                            .occupant(c)
                            .is_none()
                        && game
                            .world
                            .resource::<TacticalBattle>()
                            .board
                            .walkable(c.0, c.1)
                })
                .expect("room away from the blast");
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(far, spot);
        }
        (a, friend)
    }

    #[test]
    fn a_capture_aimed_four_cells_out_needs_range_research() {
        let mut game = game();
        let pack = ready(&mut game, 1, 5);
        let from = cell(&game, game.player_entity());
        let at = put_at(&mut game, pack[0], from, 4);

        assert!(
            !game.tactical_use_routine(0, at),
            "range 4 without research"
        );
        assert_eq!(catalysts(&game), 5, "a refusal spends nothing");

        grant(&mut game, Some(4), None);
        assert!(game.tactical_use_routine(0, at));
        assert_eq!(catalysts(&game), 4);
    }

    #[test]
    fn the_aim_outline_and_blast_preview_follow_the_research() {
        let mut game = game();
        ready(&mut game, 1, 5);
        let from = cell(&game, game.player_entity());
        let far = free_cell_at(&game, from, 4);
        assert!(
            game.tactical_placeable_cells(0).is_empty(),
            "a Single has no outline"
        );

        grant(&mut game, Some(4), Some(1));
        assert!(
            game.tactical_placeable_cells(0).contains(&far),
            "the outline stops short of the researched range"
        );
        assert!(
            game.tactical_shape_cells(0, far).len() > 1,
            "the preview is still a single cell"
        );
    }

    #[test]
    fn a_blast_rolls_each_hostile_once_and_leaves_the_friendly_alone() {
        let mut game = game();
        let pack = ready(&mut game, 3, 50);
        let (a, friend) = blast_of_three(&mut game, &pack);
        grant(&mut game, None, Some(1));

        assert!(game.tactical_use_routine(0, a));
        assert_eq!(catalysts(&game), 48, "two hostiles, one catalyst each");
        assert!(game.world.get::<crate::components::Tamed>(friend).is_none());
        assert!(
            game.world
                .get_resource::<TacticalBattle>()
                .is_none_or(|b| b.cell_of(friend).is_some()),
            "the friendly left the board"
        );
    }

    #[test]
    fn one_catalyst_is_one_roll() {
        let mut game = game();
        let pack = ready(&mut game, 3, 1);
        let (a, _) = blast_of_three(&mut game, &pack);
        grant(&mut game, None, Some(1));

        assert!(game.tactical_use_routine(0, a));
        assert_eq!(catalysts(&game), 0);
        assert!(
            !log_texts(&game).iter().any(|l| l.contains("fizzles")),
            "the loop rolled on with none left"
        );
    }

    #[test]
    fn a_roster_with_one_place_stops_after_a_capture() {
        let mut game = game();
        for _ in 1..crate::tuning::ROSTER_HARD_CAP {
            spawn_tamed(&mut game, 10, 3);
        }
        assert_eq!(game.roster_room(), 1);
        let pack = ready(&mut game, 3, 50);
        let (a, _) = blast_of_three(&mut game, &pack);
        for &h in &pack {
            game.world.get_mut::<Stats>(h).unwrap().hp = 1;
        }
        game.world
            .get_mut::<crate::components::Decompiler>(game.player_entity())
            .unwrap()
            .skill = 2000;
        grant(&mut game, None, Some(1));

        assert!(game.tactical_use_routine(0, a));
        assert_eq!(game.roster_room(), 0, "the first roll should have landed");
        assert_eq!(catalysts(&game), 49, "the loop rolled past a full roster");
    }

    #[test]
    fn an_empty_cell_beside_a_hostile_is_a_legal_aim_only_with_radius() {
        let mut game = game();
        let pack = ready(&mut game, 1, 5);
        let from = cell(&game, game.player_entity());
        let at = put_at(&mut game, pack[0], from, 1);
        let beside_it = free_cell_at(&game, at, 1);
        assert!(reach::distance(from, beside_it) <= 2);

        grant(&mut game, Some(2), None);
        assert!(
            !game.tactical_use_routine(0, beside_it),
            "Single, empty cell"
        );

        grant(&mut game, Some(2), Some(1));
        assert!(game.tactical_use_routine(0, beside_it));
        assert_eq!(catalysts(&game), 4);
    }

    #[test]
    fn a_squad_in_the_blast_is_one_roll() {
        let mut game = game();
        crate::tests::support::spawn_data_cache(&mut game, 1);
        let pack = tactical_pack(&mut game, 9, 40);
        game.open_tactical_battle(pack);
        let squad = {
            let battle = game.world.resource::<TacticalBattle>();
            battle
                .bodies()
                .map(|(e, _)| e)
                .find(|&e| game.world.get::<Squad>(e).is_some())
                .expect("9 of a kind must seat a squad")
        };
        let player = game.player_entity();
        only_routine(&mut game, player, "decompile");
        set_inventory(&mut game, &[(ICE_BREAKER, 50)]);
        assert!(wait_for_turn(&mut game, player));
        let at = cell(&game, squad);
        let spot = beside(&game, at).expect("room beside the squad");
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(player, spot);
        grant(&mut game, None, Some(2));
        let (bodies, covered) = {
            let battle = game.world.resource::<TacticalBattle>();
            let blast = reach::shape_cells(
                &battle.board,
                spot,
                at,
                crate::abilities::AbilityShape::Radius { radius: 2 },
            );
            let hit: Vec<Vec<(i32, i32)>> = battle
                .bodies()
                .filter(|&(e, _)| game.world.get::<Hostile>(e).is_some())
                .map(|(e, _)| battle.cells_of(e))
                .filter(|cells| cells.iter().any(|c| blast.contains(c)))
                .collect();
            (hit.len(), hit.iter().map(Vec::len).sum::<usize>())
        };
        assert!(
            covered > bodies,
            "fixture: the squad must cover several cells"
        );

        assert!(game.tactical_use_routine(0, at));
        assert_eq!(
            catalysts(&game),
            50 - bodies as u32,
            "a squad is one body, one roll"
        );
    }
}

/// An ordinary fight's ground is the biome its board was generated from —
/// what the renderer picks a ground texture by.
#[test]
fn a_field_fights_view_carries_its_spec_biome_as_ground() {
    use crate::tactical::map::{BattleCell, BattleSpec, Board};
    use crate::world::Biome;

    let mut game = game();
    let mut board = Board::solid(4);
    board.set(0, 0, BattleCell::Open);
    let spec = BattleSpec {
        world_seed: 1,
        site: (0, 0),
        tick: 0,
        zone: 1,
        biome: Biome::Backplane,
        bodies: 1,
    };
    game.world
        .insert_resource(TacticalBattle::open(spec, board));

    assert_eq!(game.tactical_view().unwrap().ground, Biome::Backplane);
}

/// A cell the acting body can reach that is at least two steps off, so a
/// path to it has more than one cell in it. Sorted first so the pick is
/// stable across runs.
fn reachable_two_away(game: &mut Game) -> (i32, i32) {
    let actor = game.tactical_actor().expect("somebody is acting");
    let from = cell_of(game, actor).expect("the actor stands on the board");
    let mut reachable = game.tactical_view().unwrap().reachable;
    reachable.sort_by_key(|&(x, y)| (y, x));
    reachable
        .into_iter()
        .find(|&cell| reach::distance(from, cell) >= 2)
        .expect("the actor can reach a cell two steps off")
}

#[test]
fn tactical_path_to_a_reachable_cell_ends_there_and_starts_beside_the_actor() {
    let mut game = game();
    tactical_fight(&mut game, 1, 10);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player));
    let from = cell_of(&game, player).unwrap();
    let to = reachable_two_away(&mut game);
    let allowance = game.tactical_view().unwrap().allowance;

    let path = game.tactical_path_to(to);
    assert_eq!(path.last(), Some(&to));
    let mut prev = from;
    for &cell in &path {
        assert_eq!(reach::distance(prev, cell), 1, "{prev:?} -> {cell:?}");
        prev = cell;
    }
    assert!(path.len() as u32 <= allowance);
}

#[test]
fn tactical_path_to_an_unreachable_cell_is_empty() {
    let mut game = game();
    tactical_fight(&mut game, 1, 10);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player));
    assert!(game.tactical_path_to((-1, -1)).is_empty());
}

#[test]
fn tactical_walk_to_commits_and_walk_beats_spend_it_one_cell_at_a_time() {
    let mut game = game();
    tactical_fight(&mut game, 1, 10);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player));
    let to = reachable_two_away(&mut game);
    let path = game.tactical_path_to(to);

    assert!(game.tactical_walk_to(to));
    assert!(game.tactical_walking());
    let mut steps = 0;
    while game.tactical_walking() {
        assert_eq!(game.tactical_walk_beat(), AiBeat::Stepped);
        steps += 1;
        assert!(steps <= path.len(), "the walk did not end at its last cell");
    }
    assert_eq!(steps, path.len());
    assert_eq!(cell_of(&game, player), Some(to));
    assert!(
        !game.tactical_view().unwrap().acted,
        "arriving spent the action"
    );
    assert_eq!(game.tactical_actor(), Some(player));
    assert_eq!(game.tactical_walk_beat(), AiBeat::Idle);
}

#[test]
fn tactical_walk_to_refuses_for_an_unreachable_cell_and_on_the_wild_sides_turn() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 10);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player));
    assert!(!game.tactical_walk_to((-1, -1)));
    assert!(!game.tactical_walking());

    assert!(wait_for_turn(&mut game, pack[0]));
    assert!(!game.tactical_awaits_input());
    let to = reachable_two_away(&mut game);
    assert!(!game.tactical_walk_to(to));
    assert!(!game.tactical_walking());
}

/// `run_tactical_beat` skips planning whenever a walk is planned, so a
/// click-walk that left `Some(vec![])` behind would have `[A]` act from
/// where the body stands instead of closing in. The player's walk is not
/// an AI turn: once spent or stopped it leaves nothing planned.
#[test]
fn a_spent_or_stopped_walk_leaves_nothing_planned_for_the_rest_of_the_turn() {
    let mut arrived = game();
    tactical_fight(&mut arrived, 1, 10);
    let player = arrived.player_entity();
    assert!(wait_for_turn(&mut arrived, player));
    let to = reachable_two_away(&mut arrived);
    assert!(arrived.tactical_walk_to(to));
    while arrived.tactical_walking() {
        arrived.tactical_walk_beat();
    }
    assert!(
        !arrived.world.resource::<TacticalBattle>().walk_planned(),
        "arriving left a walk planned"
    );

    let mut stopped = game();
    tactical_fight(&mut stopped, 1, 10);
    let player = stopped.player_entity();
    assert!(wait_for_turn(&mut stopped, player));
    let to = reachable_two_away(&mut stopped);
    assert!(stopped.tactical_walk_to(to));
    stopped.tactical_walk_beat();
    stopped.tactical_stop_walk();
    assert!(
        !stopped.world.resource::<TacticalBattle>().walk_planned(),
        "stopping left a walk planned"
    );
}

#[test]
fn tactical_stop_walk_leaves_the_body_where_it_stands() {
    let mut game = game();
    tactical_fight(&mut game, 1, 10);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player));
    let to = reachable_two_away(&mut game);
    let path = game.tactical_path_to(to);

    assert!(game.tactical_walk_to(to));
    assert_eq!(game.tactical_walk_beat(), AiBeat::Stepped);
    game.tactical_stop_walk();
    assert!(!game.tactical_walking());
    assert_eq!(cell_of(&game, player), Some(path[0]));
    assert_eq!(game.tactical_walk_beat(), AiBeat::Idle);
    assert_eq!(cell_of(&game, player), Some(path[0]));
}
