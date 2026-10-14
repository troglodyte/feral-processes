//! Defences that spend a supply: a Shield or Turret counts only while its
//! input holds a Charge Coil, and spends one per event it takes part in.
//! The hauling of the coils is `tests::supply`.

use super::support::*;
use crate::tactical::TacticalBattle;
use crate::tactical::map::{BattleCell, BattleSpec, Board};
use crate::tuning::*;
use crate::world::Biome;
use crate::*;

fn coil() -> ItemId {
    ItemId::from(ids::CHARGE_COIL)
}

fn game() -> Game {
    Game::new(4301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A structure of `kind` carrying `coils` in its input.
fn defence(game: &mut Game, kind: &str, x: i32, coils: u32) -> Entity {
    let mut stock = Stock::new(10);
    if coils > 0 {
        stock.input.insert(coil(), coils);
    }
    let entity = spawn_structure_at(game, kind, x, 1);
    game.world.entity_mut(entity).insert(stock);
    entity
}

fn coils_in(game: &Game, entity: Entity) -> u32 {
    game.world
        .get::<Stock>(entity)
        .and_then(|s| s.input.get(&coil()).copied())
        .unwrap_or(0)
}

fn shield_defense(game: &Game) -> u32 {
    game.structure_defs()
        .into_iter()
        .find(|d| d.id == "shield")
        .unwrap()
        .raid_defense
}

fn log_has(game: &Game, needle: &str) -> usize {
    game.message_log(usize::MAX)
        .iter()
        .filter(|l| l.text.contains(needle))
        .count()
}

#[test]
fn an_uncharged_defence_adds_nothing() {
    let mut game = game();
    let base = game.total_raid_defense();
    defence(&mut game, "shield", 1, 0);
    defence(&mut game, "turret", 2, 0);
    assert_eq!(game.total_raid_defense(), base);
    assert_eq!(
        crate::game::siege::turrets::turret_defense(&game),
        0,
        "a turret without a coil adds no siege damage either"
    );
}

#[test]
fn a_charged_defence_adds_its_authored_numbers() {
    let mut game = game();
    let base = game.total_raid_defense();
    defence(&mut game, "shield", 1, 1);
    assert_eq!(game.total_raid_defense(), base + shield_defense(&game));
}

#[test]
fn a_sweep_spends_one_coil_from_each_contributing_defence() {
    let mut game = game();
    set_zone(&mut game, RAID_MIN_ZONE);
    spawn_min_raid_staff(&mut game);
    let shield = defence(&mut game, "shield", 1, 3);
    let turret = defence(&mut game, "turret", 2, 3);
    let dry = defence(&mut game, "shield", 3, 0);
    spawn_structure_at(&mut game, "mining_node", 5, 5);
    let target = game
        .world
        .query_filtered::<Entity, With<Structure>>()
        .iter(&game.world)
        .find(|&e| {
            game.world
                .get::<Position>(e)
                .is_some_and(|p| p.x == 5 && p.y == 5)
        })
        .unwrap();
    game.world
        .entity_mut(target)
        .insert(Durability { hp: 30, max_hp: 30 });

    sweep_now(&mut game);

    assert_eq!(coils_in(&game, shield), 2, "a shield spends one coil");
    assert_eq!(
        coils_in(&game, turret),
        2,
        "a turret spends one coil for a sweep, not its eight damage"
    );
    assert_eq!(coils_in(&game, dry), 0);
}

#[test]
fn an_off_screen_siege_spends_one_coil_per_contributing_defence() {
    let mut game = game();
    set_zone(&mut game, 1);
    let shield = defence(&mut game, "shield", 1, 3);
    let turret = defence(&mut game, "turret", 2, 3);

    game.resolve_siege_offscreen();

    assert_eq!(coils_in(&game, shield), 2);
    assert_eq!(coils_in(&game, turret), 2);
}

#[test]
fn a_siege_the_staff_already_covers_spends_no_coil() {
    let mut game = game();
    set_zone(&mut game, 1);
    for _ in 0..12 {
        spawn_tamed(&mut game, 10, 3);
    }
    let shield = defence(&mut game, "shield", 1, 3);

    game.resolve_siege_offscreen();

    assert_eq!(coils_in(&game, shield), 3, "nothing needed the shield");
}

#[test]
fn an_uncharged_defence_raises_the_production_flag() {
    let mut game = game();
    let shield = defence(&mut game, "shield", 1, 0);
    game.world.entity_mut(shield).insert(MachineStatus::Running);
    game.tick();
    assert_eq!(
        game.world.get::<MachineStatus>(shield),
        Some(&MachineStatus::Uncharged)
    );
    assert!(game.base_report_flags().production.is_some());

    game.world
        .get_mut::<Stock>(shield)
        .unwrap()
        .input
        .insert(coil(), 2);
    game.tick();
    assert_eq!(
        game.world.get::<MachineStatus>(shield),
        Some(&MachineStatus::Running),
        "a charged defence reports running again"
    );
}

mod tactical {
    use super::*;

    fn open_fight(game: &mut Game) {
        let mut board = Board::solid(20);
        for y in 0..20 {
            for x in 0..20 {
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

    fn fight_with_a_turret(game: &mut Game, coils: u32, gap: i32) -> (Entity, Entity) {
        open_fight(game);
        let turret = defence(game, "turret", 1, coils);
        let hostile = game
            .world
            .spawn((
                Hostile,
                Stats {
                    hp: 10_000,
                    max_hp: 10_000,
                    atk: 0,
                    mitigation: 0,
                },
            ))
            .id();
        let player = game.player_entity();
        let mut battle = game.world.resource_mut::<TacticalBattle>();
        battle.place(turret, (0, 0));
        battle.place(hostile, (gap, 0));
        battle.place(player, (10, 10));
        battle.set_initiative(vec![player, hostile]);
        (turret, hostile)
    }

    #[test]
    fn a_turret_shot_spends_a_coil() {
        let mut game = game();
        let (turret, hostile) = fight_with_a_turret(&mut game, 2, 1);

        game.fire_turrets();

        assert!(game.world.get::<Stats>(hostile).unwrap().hp < 10_000);
        assert_eq!(coils_in(&game, turret), 1);
    }

    #[test]
    fn a_dry_turret_skips_its_shot_and_says_so_once() {
        let mut game = game();
        let (turret, hostile) = fight_with_a_turret(&mut game, 0, 1);

        game.fire_turrets();
        game.fire_turrets();

        assert_eq!(
            game.world.get::<Stats>(hostile).unwrap().hp,
            10_000,
            "a turret with no coil does not fire"
        );
        assert_eq!(coils_in(&game, turret), 0);
        assert_eq!(
            log_has(&game, "dry"),
            1,
            "the dry line is logged once per battle, not every round"
        );
    }

    #[test]
    fn a_turret_with_no_target_spends_nothing() {
        let mut game = game();
        let (turret, _) = fight_with_a_turret(&mut game, 2, 19);

        game.fire_turrets();

        assert_eq!(coils_in(&game, turret), 2, "an empty round costs nothing");
    }
}
