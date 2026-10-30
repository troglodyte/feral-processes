use super::*;

mod offscreen {
    use super::*;
    use crate::components::Downed;
    use crate::game::siege::offscreen::pack_size;
    use crate::tuning::{SIEGE_PACK_BASE, SIEGE_PACK_MAX, SIEGE_PACK_PER_ZONE};

    /// `pack_size` grows with the sector and stops at `SIEGE_PACK_MAX`.
    #[test]
    fn pack_size_grows_with_the_sector_and_caps() {
        assert_eq!(pack_size(1), SIEGE_PACK_BASE + SIEGE_PACK_PER_ZONE);
        assert_eq!(pack_size(2), SIEGE_PACK_BASE + SIEGE_PACK_PER_ZONE * 2);
        assert!(pack_size(2) > pack_size(1), "a deeper sector fields more");

        let uncapped = SIEGE_PACK_BASE + SIEGE_PACK_PER_ZONE * 50;
        assert!(
            uncapped > SIEGE_PACK_MAX,
            "the fixture must actually threaten the cap"
        );
        assert_eq!(pack_size(50), SIEGE_PACK_MAX);
    }

    /// A shortfall of zero costs nothing and logs nothing — an established,
    /// well-staffed base facing the smallest pack the game fields.
    #[test]
    fn a_covered_shortfall_costs_and_logs_nothing() {
        let mut game = Game::new(930, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut game, 1);
        for _ in 0..10 {
            spawn_tamed(&mut game, 10, 3);
        }
        let mut stock = Stock::new(100);
        stock.output.insert(ItemId::from(ids::CORE_FRAGMENT), 20);
        game.world.spawn((
            Structure {
                kind: "test_structure".to_string(),
            },
            Position { x: 1, y: 1 },
            Durability { hp: 30, max_hp: 30 },
            stock,
        ));
        let before_len = game.message_log(usize::MAX).len();

        let fired = game.resolve_siege_offscreen();

        assert!(fired, "resolving an off-screen siege always reports true");
        assert_eq!(
            game.message_log(usize::MAX).len(),
            before_len,
            "a fully-covered pack must log nothing"
        );
    }

    /// Stores stolen: an undefended, stocked base loses units off its
    /// shelves.
    #[test]
    fn an_undefended_siege_steals_from_the_shelves() {
        let mut game = Game::new(931, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut game, 6);
        let item = ItemId::from(ids::CORE_FRAGMENT);
        let mut stock = Stock::new(1000);
        stock.output.insert(item.clone(), 500);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Position { x: 1, y: 1 },
                stock,
            ))
            .id();

        game.resolve_siege_offscreen();

        let remaining = game
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&item)
            .copied()
            .unwrap_or(0);
        assert!(remaining < 500, "an undefended shelf must lose units");
    }

    /// Structures damaged: a machine with more Durability than the pack can
    /// spend survives, wounded.
    #[test]
    fn an_undefended_siege_damages_a_sturdy_machine() {
        let mut game = Game::new(932, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut game, 2);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Position { x: 1, y: 1 },
                Durability {
                    hp: 10_000,
                    max_hp: 10_000,
                },
            ))
            .id();

        game.resolve_siege_offscreen();

        let hp = game.world.get::<Durability>(structure).unwrap().hp;
        assert!(hp < 10_000, "an undefended machine must take damage");
    }

    /// Structures damaged: a machine with less Durability than the pack can
    /// spend is destroyed outright, and the remainder spreads to a second
    /// machine.
    #[test]
    fn an_undefended_siege_destroys_a_frail_machine_and_spreads_the_rest() {
        let mut game = Game::new(933, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut game, 6);
        let frail = game
            .world
            .spawn((
                Structure {
                    kind: "test_structure_a".to_string(),
                },
                Position { x: 1, y: 1 },
                Durability { hp: 1, max_hp: 1 },
            ))
            .id();
        let sturdy = game
            .world
            .spawn((
                Structure {
                    kind: "test_structure_b".to_string(),
                },
                Position { x: 2, y: 1 },
                Durability {
                    hp: 10_000,
                    max_hp: 10_000,
                },
            ))
            .id();

        game.resolve_siege_offscreen();

        assert!(
            game.world.get::<Durability>(frail).is_none(),
            "a machine outmatched by the whole shortfall must be destroyed outright"
        );
        assert!(
            game.world.get::<Durability>(sturdy).unwrap().hp < 10_000,
            "the shortfall left over from destroying the frail machine must spread to the next one"
        );
    }

    /// Staff killed: a big enough shortfall benches a defender.
    #[test]
    fn an_undefended_siege_benches_a_staff_body() {
        let mut game = Game::new(934, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Zone 8: `pack_size` is already at `SIEGE_PACK_MAX` (12), and one
        // staff body's own defence (2) leaves a shortfall of 10 — exactly
        // `SIEGE_POINTS_PER_CASUALTY`, so one casualty is guaranteed without
        // outrunning the pack cap.
        set_zone(&mut game, 8);
        let staff = spawn_tamed(&mut game, 10, 3);

        game.resolve_siege_offscreen();

        assert!(
            game.world.get::<Downed>(staff).is_some(),
            "a large enough shortfall must bench a defending staff body"
        );
    }

    /// A defended base loses strictly less than an undefended one facing the
    /// same pack — asserted as a comparison between two runs, since a
    /// constant moves with a retune and the ordering must not.
    #[test]
    fn a_defended_base_loses_less_than_an_undefended_one() {
        let item = ItemId::from(ids::CORE_FRAGMENT);

        let mut undefended = Game::new(935, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut undefended, 6);
        let mut stock = Stock::new(1000);
        stock.output.insert(item.clone(), 500);
        let poor_shelf = undefended
            .world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Position { x: 1, y: 1 },
                stock.clone(),
            ))
            .id();
        undefended.resolve_siege_offscreen();
        let undefended_loss = 500
            - undefended
                .world
                .get::<Stock>(poor_shelf)
                .unwrap()
                .output
                .get(&item)
                .copied()
                .unwrap_or(0);

        let mut defended = Game::new(935, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut defended, 6);
        for _ in 0..2 {
            spawn_tamed(&mut defended, 10, 3);
        }
        let rich_shelf = defended
            .world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Position { x: 1, y: 1 },
                stock,
            ))
            .id();
        defended.resolve_siege_offscreen();
        let defended_loss = 500
            - defended
                .world
                .get::<Stock>(rich_shelf)
                .unwrap()
                .output
                .get(&item)
                .copied()
                .unwrap_or(0);

        assert!(
            defended_loss < undefended_loss,
            "a defended base ({defended_loss} lost) must lose strictly less than an \
             undefended one ({undefended_loss} lost) facing the same pack"
        );
    }

    /// The whole resolution spends no `GameRng` draw — a deterministic
    /// payout is what lets Task 8's on-screen fight be the only place a
    /// siege rolls.
    #[test]
    fn resolving_offscreen_spends_no_rng_draw() {
        let seed = 936;
        let mut touched = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let mut untouched = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut touched, 6);
        set_zone(&mut untouched, 6);
        for game in [&mut touched, &mut untouched] {
            let item = ItemId::from(ids::CORE_FRAGMENT);
            let mut stock = Stock::new(1000);
            stock.output.insert(item, 500);
            game.world.spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Position { x: 1, y: 1 },
                Durability {
                    hp: 10_000,
                    max_hp: 10_000,
                },
                stock,
            ));
        }
        spawn_tamed(&mut touched, 10, 3);
        spawn_tamed(&mut untouched, 10, 3);

        touched.resolve_siege_offscreen();

        let after_touched: u64 = touched.world.resource_mut::<GameRng>().0.random();
        let after_untouched: u64 = untouched.world.resource_mut::<GameRng>().0.random();
        assert_eq!(
            after_touched, after_untouched,
            "an off-screen siege must draw nothing from GameRng"
        );
    }
}

/// `components::Besieger`'s citizenship — the third kind of body carrying a
/// base-space `Position`, after a posted program and a `DigSite` (Task 11).
mod raiders_citizenship {
    use super::*;
    use crate::components::{Besieger, Glyph, GlyphColor};

    fn spawn_besieger_at(game: &mut Game, x: i32, y: i32) -> Entity {
        game.world
            .spawn((
                Position { x, y },
                Glyph {
                    ch: 'r',
                    color: GlyphColor::Red,
                },
                Stats {
                    hp: 5,
                    max_hp: 5,
                    atk: 1,
                    mitigation: 0,
                },
                Hostile,
                Besieger,
            ))
            .id()
    }

    /// The wild-population systems must not count or cull a besieger —
    /// `local_hostile_count` (read by both `maybe_spawn_wild_creature` and
    /// `populate_chunk`/`ensure_local_population`) and `cull_to_cap`.
    #[test]
    fn a_besieger_does_not_count_toward_local_wild_population() {
        let mut game = Game::new(971, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let (px, py) = {
            let pos = game.world.get::<Position>(game.player_entity()).unwrap();
            (pos.x, pos.y)
        };
        // The ambient wild spawn around a fresh game is not this test's
        // concern — only the *delta* a siege's pack makes to it is.
        game.clear_local_wild();
        let before = game.local_hostile_count(px, py);
        for _ in 0..3 {
            spawn_besieger_at(&mut game, px, py);
        }
        assert_eq!(
            game.local_hostile_count(px, py),
            before,
            "a besieger must not be counted as local wild population"
        );

        game.cull_to_cap(0);
        assert_eq!(
            game.world
                .query_filtered::<Entity, With<Besieger>>()
                .iter(&game.world)
                .count(),
            3,
            "cull_to_cap must never evict a besieger"
        );
    }

    /// `position_is_honest` reads `true` for anything that isn't `Tamed`,
    /// which a besieger never is — still passing is the whole assertion.
    #[test]
    fn a_besiegers_position_is_still_honest() {
        let mut game = Game::new(972, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let besieger = spawn_besieger_at(&mut game, 5, 5);
        assert!(game.position_is_honest(besieger));
    }
}

/// `Game::besieger_turn`'s steal arm (Task 12).
mod raiders_steal {
    use super::*;
    use crate::components::{Besieger, Carrying, Glyph, GlyphColor, StolenFrom, Structure};
    use crate::game::siege::board;
    use crate::tactical::TacticalBattle;
    use crate::tactical::map::BattleSpec;
    use crate::tuning::HAUL_CARRY_CAPACITY;
    use crate::world::Biome;

    /// A real base pocket, opened as a `TacticalBattle` through the exact
    /// board a siege would build. `besieger_turn` re-derives the door the
    /// same way (`board::build`), so this fixture and the code under test
    /// always agree about where it is. Returns the door's board cell.
    fn open_pocket_battle(game: &mut Game) -> (i32, i32) {
        lay_siege_pocket(game);
        let siege_board = board::build(game).unwrap();
        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 1,
        };
        let mut battle = TacticalBattle::open(spec, siege_board.board.clone());
        // `Game::besieger_turn` reads `battle.siege_door` rather than
        // rebuilding the board every beat (a dig finishing mid-siege must
        // not move where the door is read as standing) — this fixture has
        // to carry the same value `Game::open_siege` would have written, or
        // every besieger under test reads a phantom door at `(0, 0)`.
        battle.siege_door = siege_board.spawns[0];
        // Seated at the door so `Game::settle_tactical` does not read the
        // fight as a jack-out the moment a besieger's own turn wraps the
        // round (`reap_tactical_dead`'s unconditional `settle_tactical`
        // call) — a fixture that never seats the player at all reads
        // `battle.cell_of(player)` as `None`, which is the same "gone" a
        // real jack-out leaves.
        battle.place(game.player_entity(), siege_board.spawns[0]);
        game.world.insert_resource(battle);
        siege_board.spawns[0]
    }

    fn spawn_besieger(game: &mut Game) -> Entity {
        game.world
            .spawn((
                Glyph {
                    ch: 'r',
                    color: GlyphColor::Red,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    mitigation: 0,
                },
                Hostile,
                Besieger,
            ))
            .id()
    }

    fn spawn_stocked_structure(game: &mut Game, qty: u32) -> Entity {
        let mut stock = Stock::new(1000);
        stock.output.insert(ItemId::from(ids::CORE_FRAGMENT), qty);
        game.world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                stock,
            ))
            .id()
    }

    fn core_fragment() -> ItemId {
        ItemId::from(ids::CORE_FRAGMENT)
    }

    /// Adjacent to a stocked structure, a besieger steals and makes for the
    /// door, over as many beats as the whole trip takes — and once through
    /// it, the roster is empty and the fight reads as won, the same
    /// `settle_tactical` answer a wipe gives.
    #[test]
    fn a_raider_beside_a_stocked_structure_steals_and_departs() {
        let mut game = Game::new(980, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let structure = spawn_stocked_structure(&mut game, 3);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (3, 1));
            battle.place(besieger, (4, 1));
            battle.set_initiative(vec![besieger]);
        }

        assert!(
            game.besieger_turn(besieger),
            "adjacent to a stocked structure, a besieger must steal"
        );
        assert!(
            game.world.get::<Carrying>(besieger).is_some(),
            "it must now be carrying"
        );
        let after_steal = game
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);
        assert!(after_steal < 3, "stealing must take units off the shelf");

        // The door is more than one turn's walk away, so a beat that does
        // nothing — the turn's allowance spent — hands the turn on as a real
        // round would, or the spent walk is never cleared.
        let beats = 4 * TEST_POCKET_HALF_WIDTH + 4;
        for _ in 0..beats {
            if game.world.get_resource::<TacticalBattle>().is_none() {
                break;
            }
            if !game.besieger_turn(besieger)
                && let Some(mut battle) = game.world.get_resource_mut::<TacticalBattle>()
            {
                battle.end_turn();
            }
        }

        assert!(
            game.world.get_resource::<TacticalBattle>().is_none(),
            "a lone besieger that reached the door and left empties the \
             roster, which the fight reads as won"
        );
    }

    /// A raider killed while carrying drops what it held back into the
    /// structure it came from, if that still stands — the base's total
    /// stock unchanged across the whole episode.
    #[test]
    fn a_raider_killed_one_cell_short_of_the_door_drops_what_it_held() {
        let mut game = Game::new(981, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let door = open_pocket_battle(&mut game);
        let structure = spawn_stocked_structure(&mut game, 2);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (3, 1));
            // One cell short of the door, already carrying — the steal
            // itself is Task 12's other test's concern.
            battle.place(besieger, (door.0 + 1, door.1));
            battle.set_initiative(vec![besieger]);
        }
        game.world.entity_mut(besieger).insert((
            Carrying {
                item: core_fragment(),
                qty: 3,
            },
            StolenFrom(structure),
        ));
        let total_before = 2 /* on the shelf */ + 3 /* carried */;

        game.world.get_mut::<Stats>(besieger).unwrap().hp = 0;
        let round_before = game.world.resource::<TacticalBattle>().round;
        game.world
            .resource_mut::<TacticalBattle>()
            .set_actions_left(0);
        game.hand_on_turn(besieger, round_before);

        let total_after = game
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);
        assert_eq!(
            total_after, total_before,
            "the base's total stock must be unchanged across the whole episode"
        );
    }

    /// A raider with nothing in reach to take does not stand still — it
    /// falls through to the generic AI.
    #[test]
    fn a_raider_with_nothing_to_take_falls_through_to_the_generic_ai() {
        let mut game = Game::new(982, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(besieger, (2, 2));
            battle.set_initiative(vec![besieger]);
        }

        assert!(
            !game.besieger_turn(besieger),
            "nothing adjacent to take must fall through rather than act"
        );
    }

    /// The hook actually reaches real gameplay's pacing door,
    /// `Game::tactical_ai_beat` — not just `Game::besieger_turn` called
    /// directly, which every other test in this module does. Without the
    /// `run_tactical_beat` hook a besieger would fight like an ordinary
    /// hostile in real play and only steal under a test that drives its
    /// turn by hand.
    #[test]
    fn the_pacing_driver_reaches_the_steal_hook() {
        let mut game = Game::new(984, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let structure = spawn_stocked_structure(&mut game, 3);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (3, 1));
            battle.place(besieger, (4, 1));
            battle.set_initiative(vec![besieger]);
        }

        use crate::tactical::ai::AiBeat;
        // `Stepped`, not `Acted`: this besieger is the fight's only
        // initiative slot, so spending its one action wraps the round
        // straight back onto itself (`TacticalBattle::end_turn`'s `wrap`)
        // rather than handing off to anybody else — `battle.actor()` still
        // names it once the beat returns.
        assert_eq!(game.tactical_ai_beat(), AiBeat::Stepped);
        assert!(
            game.world.get::<Carrying>(besieger).is_some(),
            "the real per-frame pacing door must reach the steal hook too"
        );
    }

    /// A besieger mid-wind-up spends its turn on the charge: the besieger
    /// hook runs after the charger's, or it would steal, wreck or leave with
    /// the charge still standing.
    #[test]
    fn a_charging_raider_does_not_steal() {
        let mut game = Game::new(985, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let def = crate::tests::charge::charge_def(40, 3, 5, 0);
        game.world
            .resource_mut::<crate::abilities::AbilityDb>()
            .insert(def.clone());
        let structure = spawn_stocked_structure(&mut game, 3);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (3, 1));
            battle.place(besieger, (4, 1));
            battle.set_initiative(vec![besieger]);
        }
        game.start_charge(
            besieger,
            &def,
            crate::components::ChargeAim::Cells(vec![(4, 2)]),
        );

        game.tactical_ai_beat();

        assert!(
            game.world.get::<Carrying>(besieger).is_none(),
            "a charger's turn is the charge's, not the siege's"
        );
    }

    /// The carry cap bounds one raider's haul, against the shared constant
    /// rather than a literal.
    #[test]
    fn the_carry_cap_bounds_one_raiders_haul() {
        let mut game = Game::new(983, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let plenty = HAUL_CARRY_CAPACITY + 5;
        let structure = spawn_stocked_structure(&mut game, plenty);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (3, 1));
            battle.place(besieger, (4, 1));
            battle.set_initiative(vec![besieger]);
        }

        assert!(game.besieger_turn(besieger));

        let carrying = game.world.get::<Carrying>(besieger).unwrap();
        assert_eq!(carrying.qty, HAUL_CARRY_CAPACITY);
        let remaining = game
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);
        assert_eq!(remaining, plenty - HAUL_CARRY_CAPACITY);
    }
}

/// `Game::besieger_turn`'s wreck arm, below the steal arm (Task 13).
mod raiders_wreck {
    use super::*;
    use crate::components::{Besieger, Carrying, Durability, Glyph, GlyphColor, Structure};
    use crate::game::siege::board;
    use crate::tactical::TacticalBattle;
    use crate::tactical::map::BattleSpec;
    use crate::world::Biome;

    fn open_pocket_battle(game: &mut Game) -> (i32, i32) {
        lay_siege_pocket(game);
        let siege_board = board::build(game).unwrap();
        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 1,
        };
        let mut battle = TacticalBattle::open(spec, siege_board.board.clone());
        // `Game::besieger_turn` reads `battle.siege_door` rather than
        // rebuilding the board every beat (a dig finishing mid-siege must
        // not move where the door is read as standing) — this fixture has
        // to carry the same value `Game::open_siege` would have written, or
        // every besieger under test reads a phantom door at `(0, 0)`.
        battle.siege_door = siege_board.spawns[0];
        // Seated at the door so `Game::settle_tactical` does not read the
        // fight as a jack-out the moment a besieger's own turn wraps the
        // round (`reap_tactical_dead`'s unconditional `settle_tactical`
        // call) — a fixture that never seats the player at all reads
        // `battle.cell_of(player)` as `None`, which is the same "gone" a
        // real jack-out leaves.
        battle.place(game.player_entity(), siege_board.spawns[0]);
        game.world.insert_resource(battle);
        siege_board.spawns[0]
    }

    fn spawn_besieger(game: &mut Game) -> Entity {
        game.world
            .spawn((
                Glyph {
                    ch: 'r',
                    color: GlyphColor::Red,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 5,
                    mitigation: 0,
                },
                Hostile,
                Besieger,
            ))
            .id()
    }

    fn spawn_structure(game: &mut Game, hp: u32, stocked: u32) -> Entity {
        let mut stock = Stock::new(1000);
        if stocked > 0 {
            stock
                .output
                .insert(ItemId::from(ids::CORE_FRAGMENT), stocked);
        }
        game.world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Durability { hp, max_hp: hp },
                stock,
            ))
            .id()
    }

    /// A raider beside an empty machine attacks it.
    #[test]
    fn a_raider_beside_an_empty_machine_wrecks_it() {
        let mut game = Game::new(990, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let structure = spawn_structure(&mut game, 10_000, 0);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (3, 1));
            battle.place(besieger, (4, 1));
            battle.set_initiative(vec![besieger]);
        }

        assert!(
            game.besieger_turn(besieger),
            "beside an empty machine, a besieger must wreck it"
        );
        let hp = game.world.get::<Durability>(structure).unwrap().hp;
        assert!(hp < 10_000, "wrecking must lower the machine's Durability");
        assert!(
            game.world.get::<Carrying>(besieger).is_none(),
            "wrecking is not stealing"
        );
    }

    /// A raider beside a stocked one steals instead — steal before wreck.
    #[test]
    fn a_raider_beside_a_stocked_machine_steals_instead() {
        let mut game = Game::new(991, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let structure = spawn_structure(&mut game, 10_000, 3);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (3, 1));
            battle.place(besieger, (4, 1));
            battle.set_initiative(vec![besieger]);
        }

        assert!(game.besieger_turn(besieger));
        assert!(
            game.world.get::<Carrying>(besieger).is_some(),
            "a stocked machine must be stolen from rather than wrecked"
        );
        assert_eq!(
            game.world.get::<Durability>(structure).unwrap().hp,
            10_000,
            "a machine that was stolen from rather than wrecked keeps its Durability"
        );
    }

    /// A base with nothing worth taking anywhere is still wrecked rather
    /// than stood in.
    #[test]
    fn a_base_with_nothing_worth_taking_is_still_wrecked_rather_than_stood_in() {
        let mut game = Game::new(992, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let structure = spawn_structure(&mut game, 10_000, 0);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (3, 1));
            battle.place(besieger, (4, 1));
            battle.set_initiative(vec![besieger]);
        }

        let acted = game.besieger_turn(besieger);
        assert!(
            acted,
            "with nothing to take anywhere in reach, a besieger must still act"
        );
    }
}

/// The morale break: `SIEGE_MORALE_BREAK_PERCENT` of the pack down sends
/// the rest to the door unconditionally (Task 14).
mod raiders_withdraw {
    use super::*;
    use crate::components::{Besieger, Carrying, Glyph, GlyphColor, StolenFrom, Structure};
    use crate::game::siege::board;
    use crate::tactical::TacticalBattle;
    use crate::tactical::map::BattleSpec;
    use crate::world::Biome;

    /// A real base pocket, opened as a `TacticalBattle` through the exact
    /// board a siege would build, with `TacticalBattle::siege_pack` set to
    /// `pack` — `Game::open_siege`'s own write, done by hand here so a
    /// fixture can pick a pack size independent of how many bodies it
    /// actually seats.
    fn open_pocket_battle(game: &mut Game, pack: u32) -> (i32, i32) {
        lay_siege_pocket(game);
        let siege_board = board::build(game).unwrap();
        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 1,
        };
        let mut battle = TacticalBattle::open(spec, siege_board.board.clone());
        battle.siege_pack = pack;
        // See the other `open_pocket_battle`s: `Game::besieger_turn` reads
        // `battle.siege_door` rather than rebuilding the board every beat.
        battle.siege_door = siege_board.spawns[0];
        // Seated at the door so `Game::settle_tactical` does not read the
        // fight as a jack-out the moment a besieger's own turn wraps the
        // round (`reap_tactical_dead`'s unconditional `settle_tactical`
        // call) — a fixture that never seats the player at all reads
        // `battle.cell_of(player)` as `None`, which is the same "gone" a
        // real jack-out leaves.
        battle.place(game.player_entity(), siege_board.spawns[0]);
        game.world.insert_resource(battle);
        siege_board.spawns[0]
    }

    fn spawn_besieger(game: &mut Game) -> Entity {
        game.world
            .spawn((
                Glyph {
                    ch: 'r',
                    color: GlyphColor::Red,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    mitigation: 0,
                },
                Hostile,
                Besieger,
            ))
            .id()
    }

    fn chebyshev(a: (i32, i32), b: (i32, i32)) -> i32 {
        (a.0 - b.0).abs().max((a.1 - b.1).abs())
    }

    /// Downing half the pack sends the rest to the door — asserted on a
    /// survivor's cell moving doorward.
    #[test]
    fn downing_half_the_pack_sends_survivors_doorward() {
        let mut game = Game::new(993, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pack: u32 = 5;
        let door = open_pocket_battle(&mut game, pack);
        let besiegers: Vec<Entity> = (0..pack).map(|_| spawn_besieger(&mut game)).collect();
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            for (i, &b) in besiegers.iter().enumerate() {
                // `(2, i)`, not `(3, i)`: the survivor this test reads
                // (index `down`) must start more than `TACTICAL_MELEE_RANGE`
                // from the door, or the fixed adjacency-based leave check
                // (C1) reads it as already having reached the door and
                // despawns it on this very call — which is correct behaviour
                // for that case, but leaves nothing here to assert the
                // "moves doorward" claim against.
                battle.place(b, (2, i as i32));
            }
            battle.set_initiative(besiegers.clone());
        }
        let down = pack.div_ceil(2);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            for &b in besiegers.iter().take(down as usize) {
                battle.remove(b);
            }
        }
        let survivor = besiegers[down as usize];
        let before = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(survivor)
            .unwrap();
        assert!(
            chebyshev(before, door) > 1,
            "the fixture must place the survivor away from the door"
        );

        assert!(
            game.besieger_turn(survivor),
            "a broken pack's door arm must be unconditional"
        );

        let after = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(survivor)
            .unwrap();
        assert!(
            chebyshev(after, door) < chebyshev(before, door),
            "a withdrawing survivor must move doorward: {before:?} -> {after:?}, door {door:?}"
        );
    }

    /// Below the threshold, nobody withdraws — a survivor with nothing in
    /// reach to take or wreck falls through instead of heading for the
    /// door.
    #[test]
    fn below_the_threshold_nobody_withdraws() {
        let mut game = Game::new(994, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pack: u32 = 5;
        let door = open_pocket_battle(&mut game, pack);
        let besiegers: Vec<Entity> = (0..pack).map(|_| spawn_besieger(&mut game)).collect();
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            for (i, &b) in besiegers.iter().enumerate() {
                // Spread along a line inside the starting pocket, away from
                // the door and with nothing else adjacent to any of them.
                battle.place(b, (2, i as i32));
            }
            battle.set_initiative(besiegers.clone());
        }
        // Down exactly one — under half of any pack of at least 4.
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.remove(besiegers[0]);
        }
        let survivor = besiegers[1];
        let before = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(survivor)
            .unwrap();
        assert!(
            chebyshev(before, door) > 1,
            "the fixture must place the survivor away from the door"
        );

        assert!(
            !game.besieger_turn(survivor),
            "below the morale-break threshold a besieger with nothing to \
             take must fall through rather than make for the door"
        );
    }

    /// A pack that fully withdraws ends the fight as a win, through the
    /// same verdict a wipe produces — and the withdrawn raiders' cargo is
    /// gone from the base.
    #[test]
    fn a_fully_withdrawn_pack_ends_the_fight_as_a_win_and_keeps_its_plunder() {
        let mut game = Game::new(995, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = {
            let mut stock = Stock::new(1000);
            // Represents what is left on the shelf after the carrier's own
            // steal already happened — this fixture attaches `Carrying`
            // directly rather than replaying `besieger_steal`, so the 5
            // units it holds are not double-counted here.
            stock.output.insert(ItemId::from(ids::CORE_FRAGMENT), 5);
            game.world
                .spawn((
                    Structure {
                        kind: "test_structure".to_string(),
                    },
                    stock,
                ))
                .id()
        };
        // A pack of 5 with only two seated on this board — `siege_pack` is
        // set by hand precisely so a fixture can pick a size independent of
        // what it seats, and 2 is already under half of 5, so both
        // survivors read the pack as broken from their very first beat.
        let door = open_pocket_battle(&mut game, 5);
        let carrier = spawn_besieger(&mut game);
        let bare = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (5, 5));
            battle.place(carrier, (door.0 + 1, door.1));
            battle.place(bare, (door.0 + 2, door.1));
            battle.set_initiative(vec![carrier, bare]);
        }
        game.world.entity_mut(carrier).insert((
            Carrying {
                item: ItemId::from(ids::CORE_FRAGMENT),
                qty: 5,
            },
            StolenFrom(structure),
        ));

        for _ in 0..20 {
            if game.world.get_resource::<TacticalBattle>().is_none() {
                break;
            }
            game.besieger_turn(carrier);
            if game.world.get_resource::<TacticalBattle>().is_some() {
                game.besieger_turn(bare);
            }
        }

        assert!(
            game.world.get_resource::<TacticalBattle>().is_none(),
            "a fully withdrawn pack empties the roster, the same verdict a wipe gives"
        );
        let remaining = game
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied()
            .unwrap_or(0);
        assert_eq!(
            remaining, 5,
            "the carrier's plunder must stay gone — it escaped, it was not killed"
        );
    }
}
