use super::*;

/// `Game::fire_turrets` (Task 15).
mod turrets_fire {
    use super::*;
    use crate::structures::TurretDef;
    use crate::tactical::TacticalBattle;
    use crate::tactical::map::{BattleCell, BattleSpec, Board};
    use crate::world::Biome;

    fn open_fight(game: &mut Game, side: i32) {
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

    fn spawn_turret(game: &mut Game) -> Entity {
        let turret = game
            .world
            .spawn(Structure {
                kind: "turret".to_string(),
            })
            .id();
        charge_defences(game);
        turret
    }

    fn spawn_hostile(game: &mut Game, hp: i32) -> Entity {
        game.world
            .spawn((
                Hostile,
                Stats {
                    hp,
                    max_hp: hp,
                    atk: 0,
                    mitigation: 0,
                },
            ))
            .id()
    }

    fn turret_def(game: &Game) -> TurretDef {
        game.structure_defs()
            .into_iter()
            .find(|d| d.id == "turret")
            .expect("turret.ron should load as a structure")
            .turret
            .expect("turret.ron should declare a turret")
    }

    /// A turret in range with line of sight damages the nearest hostile at
    /// the round's start, and the initiative order's length does not
    /// change — a turret holds no slot to begin with.
    #[test]
    fn a_turret_in_range_damages_the_nearest_hostile() {
        let mut game = Game::new(996, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_fight(&mut game, 20);
        let def = turret_def(&game);
        let turret = spawn_turret(&mut game);
        let hostile = spawn_hostile(&mut game, 10_000);
        let player = game.player_entity();
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(turret, (0, 0));
            battle.place(hostile, (def.range as i32, 0));
            battle.place(player, (10, 10));
            battle.set_initiative(vec![player, hostile]);
        }
        let before: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .initiative()
            .to_vec();

        game.fire_turrets();

        let hp = game.world.get::<Stats>(hostile).unwrap().hp;
        assert!(hp < 10_000, "a turret in range must damage its target");
        let after: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .initiative()
            .to_vec();
        assert_eq!(
            before, after,
            "a turret holds no initiative slot and must not add one"
        );
    }

    /// A turret with no line of sight to the only hostile fires at nothing.
    #[test]
    fn a_turret_with_no_line_of_sight_fires_at_nothing() {
        let mut game = Game::new(997, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_fight(&mut game, 20);
        let turret = spawn_turret(&mut game);
        let hostile = spawn_hostile(&mut game, 10_000);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(turret, (0, 0));
            battle.place(hostile, (2, 0));
            // `Cover` is the only `BattleCell` kind that blocks sight —
            // `Blocked` is a chasm you can see over and cannot cross.
            battle.board.set(1, 0, BattleCell::Cover);
            battle.set_initiative(vec![hostile]);
        }

        game.fire_turrets();

        let hp = game.world.get::<Stats>(hostile).unwrap().hp;
        assert_eq!(
            hp, 10_000,
            "a turret with no line of sight must not hit blind"
        );
    }

    /// A turret out of range fires at nothing.
    #[test]
    fn a_turret_out_of_range_fires_at_nothing() {
        let mut game = Game::new(998, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_fight(&mut game, 20);
        let def = turret_def(&game);
        let turret = spawn_turret(&mut game);
        let hostile = spawn_hostile(&mut game, 10_000);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(turret, (0, 0));
            battle.place(hostile, (def.range as i32 + 1, 0));
            battle.set_initiative(vec![hostile]);
        }

        game.fire_turrets();

        let hp = game.world.get::<Stats>(hostile).unwrap().hp;
        assert_eq!(
            hp, 10_000,
            "a turret out of range must not reach its target"
        );
    }

    /// Two hostiles at different distances: the nearer is hit.
    #[test]
    fn the_nearer_of_two_hostiles_is_hit() {
        let mut game = Game::new(999, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_fight(&mut game, 20);
        let turret = spawn_turret(&mut game);
        let near = spawn_hostile(&mut game, 10_000);
        let far = spawn_hostile(&mut game, 10_000);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(turret, (0, 0));
            battle.place(near, (1, 0));
            battle.place(far, (3, 0));
            battle.set_initiative(vec![near, far]);
        }

        game.fire_turrets();

        assert!(
            game.world.get::<Stats>(near).unwrap().hp < 10_000,
            "the nearer hostile must be hit"
        );
        assert_eq!(
            game.world.get::<Stats>(far).unwrap().hp,
            10_000,
            "the farther hostile must be untouched"
        );
    }

    /// A destroyed turret stops firing.
    #[test]
    fn a_destroyed_turret_stops_firing() {
        let mut game = Game::new(1000, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_fight(&mut game, 20);
        let turret = spawn_turret(&mut game);
        let hostile = spawn_hostile(&mut game, 10_000);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(turret, (0, 0));
            battle.place(hostile, (1, 0));
            battle.set_initiative(vec![hostile]);
            // Destroyed: removed from the board and despawned, the way
            // `damage_structure`'s own teardown leaves one.
            battle.remove(turret);
        }
        game.world.despawn(turret);

        game.fire_turrets();

        assert_eq!(
            game.world.get::<Stats>(hostile).unwrap().hp,
            10_000,
            "a destroyed turret must not fire"
        );
    }
}

/// `save::SiegeSave` (Task 16) — a siege in progress persists across a real
/// save/load round trip, assembled from the board and the bodies rather
/// than by serialising `TacticalBattle` itself.
mod persist {
    use super::*;
    use crate::components::{Besieger, Carrying, Glyph, GlyphColor, StolenFrom, Structure};
    use crate::game::siege::board;
    use crate::items::ids;
    use crate::tactical::TacticalBattle;
    use crate::tactical::map::{BattleSpec, Board};
    use crate::world::Biome;

    fn core_fragment() -> ItemId {
        ItemId::from(ids::CORE_FRAGMENT)
    }

    /// The geometry a fixture's caller needs back, since a reload mints
    /// fresh entities and only the cells are still comparable.
    struct Fixture {
        board: Board,
        player_cell: (i32, i32),
        staff_cell: (i32, i32),
        structure_cell: (i32, i32),
        besieger_cell: (i32, i32),
    }

    /// Every kind of body a siege's `TacticalBattle` can hold — the player,
    /// one staff program, one real structure (a Depot, so it survives
    /// `restore_structures`' own `StructureDb` lookup rather than being
    /// dropped as an unrecognised kind), and one besieger carrying stolen
    /// goods from it — seated on the real flood-filled board rather than a
    /// hand-drawn one, so `SiegeSave::origin`/`door` have real geometry to
    /// round-trip. Leaves the fight one turn into round 5, on the
    /// besieger's own action budget, so `round`/`turn`/`actions_left` are
    /// none of them a coincidental default.
    fn open_siege_fixture(game: &mut Game) -> Fixture {
        game.lay_test_pocket();
        stand_in_base_at(game, 0, 0);
        let siege_board = board::build(game).unwrap();

        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "depot")
            .expect("the shipped catalogue has a Depot");
        let structure = game.spawn_structure(&def, 2, 0, None);
        game.world
            .get_mut::<Stock>(structure)
            .unwrap()
            .output
            .insert(core_fragment(), 2);

        let staff = spawn_tamed(game, 10, 3);
        {
            let mut pos = game.world.get_mut::<Position>(staff).unwrap();
            pos.x = 1;
            pos.y = 0;
        }

        let besieger = game
            .world
            .spawn((
                Creature {
                    species: GENERIC_SPECIES_ID.to_string(),
                },
                Position { x: 3, y: 0 },
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
                Carrying {
                    item: core_fragment(),
                    qty: 3,
                },
                StolenFrom(structure),
            ))
            .id();

        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 2,
            biome: Biome::OpenGrid,
            bodies: 4,
        };
        let mut battle = TacticalBattle::open(spec, siege_board.board.clone());
        battle.siege_origin = siege_board.origin;
        battle.siege_door = siege_board.spawns[0];

        // The player's own tile: a reload seats them from their `Position`.
        let player_cell = siege_board.to_board((0, 0)).unwrap();
        let staff_cell = siege_board.to_board((1, 0)).unwrap();
        let structure_cell = siege_board.to_board((2, 0)).unwrap();
        let besieger_cell = siege_board.to_board((3, 0)).unwrap();

        battle.place(structure, structure_cell);
        battle.place(staff, staff_cell);
        let player = game.player_entity();
        battle.place(player, player_cell);
        battle.place(besieger, besieger_cell);

        battle.siege_pack = 1;
        // Fastest to slowest: the player, then staff, then the besieger —
        // and one turn spent moves the actor onto staff, which is where
        // the round/turn/actions_left assertions below need it.
        battle.set_initiative(vec![player, staff, besieger]);
        battle.end_turn();
        battle.round = 5;
        battle.set_actions_left(2);

        let board = battle.board.clone();
        game.world.insert_resource(battle);

        Fixture {
            board,
            player_cell,
            staff_cell,
            structure_cell,
            besieger_cell,
        }
    }

    /// Finds the one entity of a marker component a fresh load must have
    /// reseeded — the round trip mints new `Entity` ids, so a body from
    /// before the reload can never be compared against one from after it.
    fn the<T: bevy_ecs::prelude::Component>(game: &mut Game) -> Entity {
        game.world
            .query_filtered::<Entity, With<T>>()
            .iter(&game.world)
            .next()
            .unwrap_or_else(|| {
                panic!(
                    "exactly one {} must have reloaded",
                    std::any::type_name::<T>()
                )
            })
    }

    /// **The main test.** A siege's board, every body's own cell, the
    /// initiative order and the round/turn/actions_left it was interrupted
    /// on all survive a real save→load round trip, and the fight can be
    /// continued afterward.
    #[test]
    fn a_siege_in_progress_survives_a_real_save_load_round_trip() {
        let mut game = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let fixture = open_siege_fixture(&mut game);

        let scratch = scratch_assets_dir("siege_persist_roundtrip");
        std::fs::create_dir_all(&*scratch).unwrap();
        let path = scratch.join("save.bin");
        game.save(&path).unwrap();

        let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();

        assert!(
            loaded.in_tactical_battle(),
            "the siege must still be open after the load"
        );

        let player = loaded.player_entity();
        let staff = the::<Tamed>(&mut loaded);
        let structure = the::<Structure>(&mut loaded);
        let besieger = the::<Besieger>(&mut loaded);

        {
            let battle = loaded.world.resource::<TacticalBattle>();
            assert_eq!(battle.board, fixture.board, "the board's own cells");
            assert_eq!(battle.siege_pack, 1, "siege_pack");
            assert_eq!(battle.round, 5, "round");
            assert_eq!(battle.actions_left(), 2, "actions_left");

            assert_eq!(
                battle.cell_of(player),
                Some(fixture.player_cell),
                "player cell"
            );
            assert_eq!(
                battle.cell_of(staff),
                Some(fixture.staff_cell),
                "staff cell"
            );
            assert_eq!(
                battle.cell_of(structure),
                Some(fixture.structure_cell),
                "structure cell"
            );
            assert_eq!(
                battle.cell_of(besieger),
                Some(fixture.besieger_cell),
                "besieger cell"
            );

            assert_eq!(
                battle.initiative(),
                &[player, staff, besieger],
                "the initiative order, fastest to slowest"
            );
            assert_eq!(
                battle.actor(),
                Some(staff),
                "the acting body must survive along with everything else"
            );
        }

        assert_eq!(
            loaded.tactical_view().unwrap().ground,
            crate::world::Biome::Platform,
            "a restored siege is still a siege, so still the base's floor"
        );

        // The fight can be continued: ending the acting body's turn moves
        // the cursor on exactly as it would have before the reload.
        loaded.tactical_end_turn();
        assert_eq!(
            loaded.world.resource::<TacticalBattle>().actor(),
            Some(besieger),
            "the fight must still take real turns after the load"
        );
    }

    /// A save written before this feature existed carries no `siege` key at
    /// all, and must load with no siege in progress rather than refusing or
    /// panicking — `#[serde(default)]`'s whole compatibility story.
    #[test]
    fn a_pre_siege_save_loads_with_no_siege_in_progress() {
        let mut game = Game::new(9002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_siege_fixture(&mut game);

        let scratch = scratch_assets_dir("siege_persist_pre_siege");
        std::fs::create_dir_all(&*scratch).unwrap();
        let path = scratch.join("save.bin");
        game.save(&path).unwrap();

        // Stripped to what a save written before sieges existed looked
        // like — the real save's own `siege` key, removed, rather than a
        // hand-built RON fixture that only proves the parser accepts an
        // absent field. Whole *block*, not the one line a naive filter
        // drops: pretty RON breaks `Some((...))` across many lines, and a
        // line filter leaves the tail behind as a parse error —
        // `settlement_boards::a_save_from_before_town_jobs_loads_its_
        // contracts_as_the_brokers`'s own depth-tracked pattern.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(
            raw.contains("siege: Some("),
            "the fixture must open a real siege, or this proves nothing"
        );
        let mut older = String::new();
        let mut depth = 0usize;
        for line in raw.lines() {
            if depth == 0 && !line.trim_start().starts_with("siege:") {
                older.push_str(line);
                older.push('\n');
                continue;
            }
            depth += line.matches('(').count();
            depth -= line.matches(')').count().min(depth);
        }
        assert!(!older.contains("siege:"), "the key has to actually be gone");
        let old_path = scratch.join("old.bin");
        std::fs::write(&old_path, older).unwrap();

        let loaded = Game::load(&old_path, &test_assets_dir()).unwrap();
        assert!(
            !loaded.in_tactical_battle(),
            "a pre-siege save must load with no siege in progress"
        );
    }

    /// The invariant this whole feature is built to hold —
    /// `TacticalBattle` never gains `Serialize`, so an ordinary tactical
    /// fight (one `siege_pack` reads as "not a siege") is still not saved.
    /// This test must fail the moment that changes.
    #[test]
    fn an_ordinary_tactical_fight_is_still_not_saved() {
        let mut game = Game::new(9003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let hostile = game
            .world
            .spawn((
                Position { x: 1, y: 0 },
                Glyph {
                    ch: 'h',
                    color: GlyphColor::Red,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    mitigation: 0,
                },
                Hostile,
            ))
            .id();
        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 2,
        };
        let mut battle = TacticalBattle::open(spec, Board::from_rows(&["....."; 5]));
        let player = game.player_entity();
        battle.place(player, (0, 0));
        battle.place(hostile, (1, 0));
        battle.set_initiative(vec![player, hostile]);
        assert_eq!(
            battle.siege_pack, 0,
            "an ordinary fight opens no siege pack"
        );
        game.world.insert_resource(battle);

        let scratch = scratch_assets_dir("siege_persist_ordinary_fight");
        std::fs::create_dir_all(&*scratch).unwrap();
        let path = scratch.join("save.bin");
        game.save(&path).unwrap();

        let data = crate::save::load_from_file(&path).unwrap();
        assert!(
            data.siege.is_none(),
            "an ordinary tactical fight must not be assembled into a save"
        );

        let loaded = Game::load(&path, &test_assets_dir()).unwrap();
        assert!(
            loaded.world.get_resource::<TacticalBattle>().is_none(),
            "an ordinary tactical fight must not survive a save/load round trip"
        );
    }

    /// A besieger carrying cargo across a save still drops it — into the
    /// structure it came from, resolved fresh by tile after the reload —
    /// when it is killed after the load.
    #[test]
    fn a_besieger_carrying_cargo_across_a_save_still_drops_it_when_killed_after_the_load() {
        let mut game = Game::new(9004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_siege_fixture(&mut game);

        let scratch = scratch_assets_dir("siege_persist_drop_cargo");
        std::fs::create_dir_all(&*scratch).unwrap();
        let path = scratch.join("save.bin");
        game.save(&path).unwrap();

        let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let structure = the::<Structure>(&mut loaded);
        let besieger = the::<Besieger>(&mut loaded);

        assert_eq!(
            loaded.world.get::<StolenFrom>(besieger).map(|s| s.0),
            Some(structure),
            "StolenFrom must resolve to the reloaded structure, not a stale entity"
        );
        assert_eq!(
            loaded.world.get::<Carrying>(besieger).map(|c| c.qty),
            Some(3),
            "Carrying must round-trip through a wild creature too"
        );
        let before_on_shelf = loaded
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);

        // Two turns: player then staff, landing on the besieger's own turn
        // exactly as the fixture left it before the save.
        loaded.tactical_end_turn();
        assert_eq!(
            loaded.world.resource::<TacticalBattle>().actor(),
            Some(besieger)
        );
        loaded.world.get_mut::<Stats>(besieger).unwrap().hp = 0;
        loaded.tactical_end_turn();

        assert!(
            loaded.world.get_resource::<TacticalBattle>().is_none()
                || loaded
                    .world
                    .resource::<TacticalBattle>()
                    .cell_of(besieger)
                    .is_none(),
            "the reap must have cleared the dead besieger off the board"
        );
        let after_on_shelf = loaded
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);
        assert_eq!(
            after_on_shelf,
            before_on_shelf + 3,
            "the stolen 3 units must return to the shelf they came from"
        );
    }

    /// The siege fields themselves were additive; the version moves with
    /// other features (ruins, the field base), never because a siege
    /// field was added.
    #[test]
    fn save_format_version_is_unchanged() {
        assert_eq!(
            crate::save::SAVE_FORMAT_VERSION,
            40,
            "the save format version moves only with a deliberate bump; update \
             this pin and the changelog together"
        );
    }
}
