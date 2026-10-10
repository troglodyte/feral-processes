//! The Holding Cell, pin half: a downed program revived as a prisoner in a
//! pen, the `Jailed` role it reads as, and the save field that keeps it.

use super::support::*;
use crate::components::Jailed;
use crate::components::Rarity;
use crate::components::{Durability, MachineStatus, Task, TaskKind, Temporary};
use crate::game::base::prison::JailBlock;
use crate::items::DownedProgram;
use crate::resources::{GameOver, GameRng, Party, PowerGrid};
use crate::structures::StructureDb;
use crate::tuning::JAIL_BREAKDOWN_SCALE;
use crate::tuning::{JAIL_BASE_POTENCY, JAIL_MAX_ATTEMPTS, ROSTER_HARD_CAP};
use crate::*;

const CELL: &str = "holding_cell";
/// A shipped species whose `taming_difficulty` is dead centre.
const MID_SPECIES: &str = "crawler";
/// Anchor at (2, 2), footprint 2: the pen is (3, 3).
const ANCHOR: (i32, i32) = (2, 2);
const PEN: (i32, i32) = (3, 3);
/// Orthogonally beside the pen, off the footprint.
const BESIDE: (i32, i32) = (4, 3);

fn record(species: &str, rarity: Rarity) -> DownedProgram {
    DownedProgram {
        species: species.to_string(),
        level: 12,
        rarity,
        boss: false,
        condition: 70,
        carried: None,
    }
}

fn protocols(game: &Game) -> u32 {
    let player = game.player_entity();
    game.world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(crate::items::ids::REINITIALIZATION_PROTOCOL))
}

fn records(game: &Game) -> Vec<DownedProgram> {
    let player = game.player_entity();
    game.world.get::<DownedPrograms>(player).unwrap().0.clone()
}

/// A base with one Holding Cell, the party standing beside its pen, two
/// protocols in the pack and one downed `MID_SPECIES` record.
fn base_with_a_cell() -> (Game, Entity) {
    let mut game = Game::new(7301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, BESIDE.0, BESIDE.1);
    let cell = spawn_machine_at(&mut game, CELL, ANCHOR.0, ANCHOR.1);
    game.world
        .resource_mut::<crate::base_grid::BaseGrid>()
        .lay_floor(PEN.0, PEN.1);
    set_inventory(
        &mut game,
        &[(crate::items::ids::REINITIALIZATION_PROTOCOL, 2)],
    );
    let player = game.player_entity();
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 =
        vec![record(MID_SPECIES, Rarity::Ordinary)];
    (game, cell)
}

fn the_prisoner(game: &Game, cell: Entity) -> Entity {
    game.cell_prisoner(cell).expect("the cell holds a prisoner")
}

/// Asserts the pin was refused with `block` and wrote nothing: the pack,
/// the records, the roster and the log all as they were.
fn assert_refused_untouched(game: &mut Game, index: usize, block: JailBlock) {
    let packs = protocols(game);
    let held = records(game);
    let pets = game.pet_count();
    let log = game.message_log(200).len();

    assert_eq!(game.jail_blocker(index), Some(block));
    let err = game.jail_program(index).expect_err("must be refused");
    assert_eq!(err, block.refusal());

    assert_eq!(
        protocols(game),
        packs,
        "{block:?} must not spend a protocol"
    );
    assert_eq!(records(game), held, "{block:?} must not touch the records");
    assert_eq!(game.pet_count(), pets, "{block:?} must not add a body");
    assert_eq!(game.message_log(200).len(), log, "{block:?} logs nothing");
}

#[test]
fn the_shipped_holding_cell_loads_with_a_pen_and_no_study() {
    let game = Game::new(7300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let def = game
        .world
        .resource::<StructureDb>()
        .get(CELL)
        .expect("the shipped cell loads");
    assert_eq!(def.footprint, 2);
    assert!(!def.studies);
    assert_eq!(
        def.holds_prisoner.as_ref().map(|p| p.attempt_ticks),
        Some(40)
    );
}

#[test]
fn a_cell_is_a_prison_pen_and_never_a_study_pen() {
    let (mut game, cell) = base_with_a_cell();
    let station = spawn_structure_at(&mut game, "research_node", 8, 8);
    assert_eq!(game.prison_pen(cell), Some(PEN));
    assert_eq!(game.study_pen(cell), None);
    assert_eq!(game.study_station(), Some(station));
    assert_eq!(game.prison_pen(station), None);
    // One rule for the corner: a station of the same footprint agrees.
    assert_eq!(game.study_pen(station), Some((9, 9)));
}

#[test]
fn a_holding_cell_file_without_a_pen_cell_is_skipped() {
    let dir = std::env::temp_dir().join(format!("feral_processes_cell_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, footprint: u8, ticks: u32| {
        std::fs::write(
            dir.join(format!("{name}.ron")),
            format!(
                "(id: \"{name}\", name: \"X\", description: \"x\", glyph: 'J', color: Red, \
                 build_cost: [], footprint: {footprint}, \
                 holds_prisoner: Some((attempt_ticks: {ticks})))"
            ),
        )
        .unwrap();
    };
    write("flat", 1, 40);
    write("idle", 2, 0);
    write("fine", 2, 40);
    let (db, warnings) = StructureDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(db.get("flat").is_none() && db.get("idle").is_none());
    assert!(db.get("fine").is_some());
    assert_eq!(warnings.len(), 2, "{warnings:?}");
}

#[test]
fn a_pin_spends_one_protocol_and_one_record_and_boots_the_body_in_the_pen() {
    let (mut game, cell) = base_with_a_cell();
    let player = game.player_entity();
    let carried = game
        .world
        .resource::<AbilityDb>()
        .wild_pool()
        .first()
        .map(|(def, _)| def.id.clone())
        .expect("some shipped routine is in the wild pool");
    let kept = record("scrapper", Rarity::Ordinary);
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = vec![
        DownedProgram {
            carried: Some(carried.clone()),
            ..record(MID_SPECIES, Rarity::Platinum)
        },
        kept.clone(),
    ];
    let pets = game.pet_count();

    game.jail_program(0)
        .expect("a free cell is beside the party");

    assert_eq!(protocols(&game), 1, "exactly one protocol is spent");
    assert_eq!(records(&game), vec![kept], "exactly the pinned record goes");
    assert_eq!(game.pet_count(), pets + 1, "the prisoner counts as roster");
    let body = the_prisoner(&game, cell);
    assert_eq!(game.program_role(body), Some(ProgramRole::Jailed));
    assert_eq!(
        *game.world.get::<Position>(body).unwrap(),
        Position { x: PEN.0, y: PEN.1 }
    );
    assert_eq!(game.world.get::<Experience>(body).unwrap().level, 1);
    assert_eq!(game.world.get::<Rarity>(body), Some(&Rarity::Platinum));
    assert_eq!(
        game.world.get::<Routines>(body).map(|r| r.0.clone()),
        Some(vec![carried]),
        "the carried routine is kept, not re-rolled"
    );
    let jailed = game.world.get::<Jailed>(body).unwrap();
    assert_eq!((jailed.attempts, jailed.progress), (0, 0));
    assert_eq!(game.jail_blocker(0), Some(JailBlock::NoFreeCell));
}

#[test]
fn every_refusal_leaves_the_pack_the_records_and_the_roster_alone() {
    // Finished run.
    let (mut game, _) = base_with_a_cell();
    game.world.resource_mut::<GameOver>().reason = Some("done".into());
    assert_refused_untouched(&mut game, 0, JailBlock::NotNow);

    // In a fight.
    let (mut game, _) = base_with_a_cell();
    let enemy = spawn_wild_without_routine(&mut game, "scrapper", 20, 20);
    let player = game.player_entity();
    insert_battle(&mut game, player, vec![enemy]);
    assert_refused_untouched(&mut game, 0, JailBlock::NotNow);

    // No such record.
    let (mut game, _) = base_with_a_cell();
    assert_refused_untouched(&mut game, 5, JailBlock::NoSuchRecord);

    // A boss.
    let (mut game, _) = base_with_a_cell();
    let player = game.player_entity();
    game.world.get_mut::<DownedPrograms>(player).unwrap().0[0].boss = true;
    assert_refused_untouched(&mut game, 0, JailBlock::Boss);

    // No protocol.
    let (mut game, _) = base_with_a_cell();
    set_inventory(&mut game, &[]);
    assert_refused_untouched(&mut game, 0, JailBlock::NoProtocol);

    // A full roster.
    let (mut game, _) = base_with_a_cell();
    while game.roster_room() > 0 {
        let extra = spawn_tamed(&mut game, 10, 3);
        game.world.get_mut::<Position>(extra).unwrap().x = 30;
    }
    assert_refused_untouched(&mut game, 0, JailBlock::RosterFull);

    // A species a mod removed.
    let (mut game, _) = base_with_a_cell();
    let player = game.player_entity();
    game.world.get_mut::<DownedPrograms>(player).unwrap().0[0].species = "gone".to_string();
    assert_refused_untouched(&mut game, 0, JailBlock::UnknownSpecies);

    // Not beside the cell.
    let (mut game, _) = base_with_a_cell();
    stand_in_base_at(&mut game, 12, 12);
    assert_refused_untouched(&mut game, 0, JailBlock::NoFreeCell);

    // Out of base space.
    let (mut game, _) = base_with_a_cell();
    game.world.insert_resource(Locale::Surface);
    assert_refused_untouched(&mut game, 0, JailBlock::NoFreeCell);

    // The party standing on the pen itself.
    let (mut game, _) = base_with_a_cell();
    stand_in_base_at(&mut game, PEN.0, PEN.1);
    assert_refused_untouched(&mut game, 0, JailBlock::NoFreeCell);

    // A staff body already in the pen.
    let (mut game, _) = base_with_a_cell();
    let squatter = spawn_tamed(&mut game, 10, 3);
    assert_eq!(
        *game.world.get::<Position>(squatter).unwrap(),
        Position { x: 3, y: 3 }
    );
    assert_refused_untouched(&mut game, 0, JailBlock::NoFreeCell);

    // No floor under the pen.
    let (mut game, _) = base_with_a_cell();
    game.world
        .resource_mut::<crate::base_grid::BaseGrid>()
        .open(PEN.0, PEN.1, 0);
    assert_refused_untouched(&mut game, 0, JailBlock::NoFreeCell);

    // A structure that is not a cell.
    let mut game = Game::new(7302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, BESIDE.0, BESIDE.1);
    spawn_structure_at(&mut game, "research_node", ANCHOR.0, ANCHOR.1);
    set_inventory(
        &mut game,
        &[(crate::items::ids::REINITIALIZATION_PROTOCOL, 1)],
    );
    let player = game.player_entity();
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 =
        vec![record(MID_SPECIES, Rarity::Ordinary)];
    assert_refused_untouched(&mut game, 0, JailBlock::NoFreeCell);
}

#[test]
fn an_occupied_cell_refuses_a_second_prisoner() {
    let (mut game, cell) = base_with_a_cell();
    let player = game.player_entity();
    game.world
        .get_mut::<DownedPrograms>(player)
        .unwrap()
        .0
        .push(record("scrapper", Rarity::Ordinary));
    game.jail_program(0).unwrap();
    let first = the_prisoner(&game, cell);
    assert_refused_untouched(&mut game, 0, JailBlock::NoFreeCell);
    assert_eq!(game.cell_prisoner(cell), Some(first));
}

#[test]
fn the_last_place_on_the_roster_is_the_prisoners() {
    let (mut game, _) = base_with_a_cell();
    while game.roster_room() > 1 {
        let extra = spawn_tamed(&mut game, 10, 3);
        game.world.get_mut::<Position>(extra).unwrap().x = 30;
    }
    assert!(game.pet_count() < ROSTER_HARD_CAP);
    game.jail_program(0).expect("one place is left");
    assert_eq!(game.roster_room(), 0, "the prisoner holds its place");
}

#[test]
fn a_prisoner_occupies_ground_but_is_not_staff() {
    let (mut game, cell) = base_with_a_cell();
    game.jail_program(0).unwrap();
    let body = the_prisoner(&game, cell);
    assert!(game.base_bodies().iter().any(|(e, _)| *e == body));
    assert!(!game.base_staff().contains(&body));
    assert!(game.position_is_honest(body));
}

#[test]
fn a_prisoner_is_never_handed_a_task_and_never_moves() {
    let (mut game, cell) = base_with_a_cell();
    place_home(&mut game);
    spawn_machine_at(&mut game, "mining_node", 2, 0);
    spawn_machine_at(&mut game, "lathe", 3, 0);
    spawn_machine_at(&mut game, "disk_press", 4, 0);
    let crew = [
        spawn_tamed(&mut game, 100, 5),
        spawn_tamed(&mut game, 100, 5),
    ];
    for (i, c) in crew.iter().enumerate() {
        *game.world.get_mut::<Position>(*c).unwrap() = Position { x: i as i32, y: 1 };
    }
    game.jail_program(0).unwrap();
    let body = the_prisoner(&game, cell);
    let start = *game.world.get::<Position>(body).unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();

    let mut crew_worked = false;
    for _ in 0..60 {
        game.tick();
        crew_worked |= crew.iter().any(|c| game.world.get::<Task>(*c).is_some());
        assert!(game.world.get::<Task>(body).is_none());
        assert_eq!(*game.world.get::<Position>(body).unwrap(), start);
        assert_eq!(game.program_role(body), Some(ProgramRole::Jailed));
    }
    assert!(crew_worked, "the work must have been there to hand out");
}

#[test]
fn a_prisoner_joins_no_party_sortie_or_sale() {
    let (mut game, cell) = base_with_a_cell();
    unlock_research_chain(&mut game, "program_refactoring");
    let player = game.player_entity();
    let ring = ItemId::from(crate::items::ids::PRIVILEGE_RING);
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ring.clone(), 4);
    spawn_structure_at(&mut game, "compiler", 30, 30);
    let market = spawn_market(&mut game);
    game.jail_program(0).unwrap();
    let body = the_prisoner(&game, cell);
    let prize = game
        .world
        .resource::<AbilityDb>()
        .wild_pool()
        .into_iter()
        .map(|(def, _)| def.id.clone())
        .next()
        .expect("some shipped ability is wild-poolable");
    game.world.get_mut::<Routines>(body).unwrap().0 = vec![prize];
    let other = spawn_tamed(&mut game, 10, 3);
    game.world.get_mut::<Position>(other).unwrap().x = 30;

    let in_cell = |err: String| assert!(err.contains("Holding Cell"), "unexpected: {err}");
    in_cell(game.add_companion(body).unwrap_err());
    in_cell(game.wield_program(body).unwrap_err());
    in_cell(game.fuse_companions(body, other, None).unwrap_err());
    in_cell(game.fuse_companions(other, body, None).unwrap_err());
    in_cell(game.extract_routine(body, 0).unwrap_err());
    in_cell(game.open_kernel_ring(body).unwrap_err());
    in_cell(game.sell_companion(market, body).unwrap_err());

    assert_eq!(game.world.get::<Inventory>(player).unwrap().count(&ring), 4);
    assert!(
        game.world.get::<Creature>(body).is_some(),
        "nothing consumed it"
    );
    assert!(game.world.resource::<Party>().0.is_empty());
    assert_eq!(game.wielded_program(), None);
    assert_eq!(game.program_role(body), Some(ProgramRole::Jailed));
    assert!(
        game.programs_for_build(1).iter().all(|p| p.entity != body),
        "a prisoner is not offered to a build"
    );
    assert!(game.commit_program(body).is_none());
}

#[test]
fn a_prisoner_is_not_healed_by_a_rest_and_files_between_study_and_siphon() {
    let (mut game, cell) = base_with_a_cell();
    game.jail_program(0).unwrap();
    let body = the_prisoner(&game, cell);
    game.world.get_mut::<Stats>(body).unwrap().hp = 1;

    game.rest().unwrap();

    assert_eq!(game.world.get::<Stats>(body).unwrap().hp, 1);
    assert!(ProgramRole::UnderStudy.roster_rank() < ProgramRole::Jailed.roster_rank());
    assert!(ProgramRole::Jailed.roster_rank() < ProgramRole::Siphoned.roster_rank());
}

#[test]
fn a_prisoners_attempts_and_progress_survive_a_save_and_load() {
    let (mut game, cell) = base_with_a_cell();
    game.jail_program(0).unwrap();
    let body = the_prisoner(&game, cell);
    game.rename_companion(body, Some("Inmate".to_string()))
        .expect("named");
    {
        let mut jailed = game.world.get_mut::<Jailed>(body).unwrap();
        jailed.attempts = 3;
        jailed.progress = 17;
    }

    let path = std::env::temp_dir().join(format!(
        "feral_processes_jailed_reload_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).expect("load");
    let _ = std::fs::remove_file(&path);

    let back = loaded
        .owned_pets()
        .into_iter()
        .find(|p| p.name.contains("Inmate"))
        .expect("the prisoner is back on the roster")
        .entity;
    assert_eq!(loaded.program_role(back), Some(ProgramRole::Jailed));
    let jailed = loaded.world.get::<Jailed>(back).unwrap().clone();
    assert_eq!((jailed.attempts, jailed.progress), (3, 17));
    assert_eq!(
        jailed.record,
        Some(record(MID_SPECIES, Rarity::Ordinary)),
        "the booted-from record survives the round trip whole"
    );
    let reloaded_cell = loaded
        .find_blocking_structure_at(ANCHOR.0, ANCHOR.1)
        .expect("the cell reloads standing");
    assert_eq!(jailed.cell, reloaded_cell, "tethered to the reloaded cell");
    assert_eq!(loaded.cell_prisoner(reloaded_cell), Some(back));
    assert_eq!(
        *loaded.world.get::<Position>(back).unwrap(),
        Position { x: PEN.0, y: PEN.1 }
    );
}

#[test]
fn a_prisoner_whose_cell_is_gone_returns_its_record_on_load() {
    let (mut game, cell) = base_with_a_cell();
    game.jail_program(0).unwrap();
    let body = the_prisoner(&game, cell);
    game.rename_companion(body, Some("Inmate".to_string()))
        .expect("named");
    // A reload that finds a different structure on the tile.
    game.world.get_mut::<Structure>(cell).unwrap().kind = "data_cache".to_string();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_jailed_no_cell_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).expect("load");
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded
            .owned_pets()
            .into_iter()
            .all(|p| !p.name.contains("Inmate")),
        "the body is gone, not freed as staff"
    );
    assert_eq!(
        records(&loaded),
        vec![record(MID_SPECIES, Rarity::Ordinary)]
    );
}

#[test]
fn a_standing_empty_cell_draws_nothing_from_the_rng() {
    use rand::RngExt;
    let next = |with_cell: bool| -> u64 {
        let mut game = Game::new(7304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        stand_in_base_at(&mut game, BESIDE.0, BESIDE.1);
        if with_cell {
            spawn_structure_at(&mut game, CELL, ANCHOR.0, ANCHOR.1);
        }
        for _ in 0..30 {
            game.tick();
        }
        game.world.resource_mut::<GameRng>().0.random()
    };
    assert_eq!(next(true), next(false));
}

#[test]
fn a_warded_empty_cell_draws_nothing_from_the_rng() {
    use rand::RngExt;
    let next = |with_cell: bool| -> u64 {
        let mut game = Game::new(7304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        stand_in_base_at(&mut game, BESIDE.0, BESIDE.1);
        if with_cell {
            let cell = spawn_machine_at(&mut game, CELL, ANCHOR.0, ANCHOR.1);
            stand_ample_grid_supply(&mut game);
            let warden = spawn_tamed(&mut game, 10, 3);
            game.world.entity_mut(warden).insert(Task {
                kind: TaskKind::GatherResource,
                target: cell,
                progress: 0,
                required: 1,
            });
        }
        for _ in 0..30 {
            game.tick();
        }
        game.world.resource_mut::<GameRng>().0.random()
    };
    assert_eq!(next(true), next(false));
}

#[test]
fn the_odds_rise_with_every_failed_attempt_and_are_worth_the_wait() {
    let (mut game, cell) = base_with_a_cell();
    game.jail_program(0).unwrap();
    let body = the_prisoner(&game, cell);

    let mut odds = Vec::new();
    for attempts in 0..JAIL_MAX_ATTEMPTS {
        game.world.get_mut::<Jailed>(body).unwrap().attempts = attempts;
        odds.push(game.jail_odds(body).expect("a prisoner has odds"));
    }

    assert!(odds[0] < 0.5, "the first roll is a long shot: {odds:?}");
    assert!(
        odds.windows(2).all(|w| w[1] > w[0]),
        "each failure must raise the next roll: {odds:?}"
    );
    let escapes: f32 = odds.iter().map(|p| 1.0 - p).product();
    assert!(
        1.0 - escapes > 0.6,
        "five rolls must take most prisoners: {odds:?}"
    );

    let items = game.world.resource::<ItemDb>();
    let breaker = items
        .get("ice_breaker")
        .and_then(|d| d.taming_potency)
        .expect("the shipped ICE Breaker has a potency");
    assert!(
        JAIL_BASE_POTENCY >= breaker,
        "a cell is never worse than the catalyst it saves"
    );
    assert_eq!(game.jail_odds(game.player_entity()), None);
}

// ---------------------------------------------------------------------
// Phase B: the process.
// ---------------------------------------------------------------------

const ATTEMPT_TICKS: u32 = 40;

/// A cell with a prisoner inside, a lit grid, and a worker holding the
/// warden's `Task` — the `a_staffed_rig_*` fixtures' shape.
fn a_warded_cell(seed: u64) -> (Game, Entity, Entity) {
    let (mut game, cell) = base_with_a_cell_seeded(seed);
    game.jail_program(0).unwrap();
    stand_ample_grid_supply(&mut game);
    let warden = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(warden).insert(Task {
        kind: TaskKind::GatherResource,
        target: cell,
        progress: 0,
        required: 1,
    });
    let prisoner = the_prisoner(&game, cell);
    (game, cell, prisoner)
}

fn base_with_a_cell_seeded(seed: u64) -> (Game, Entity) {
    let (mut game, cell) = base_with_a_cell();
    if seed != 7301 {
        let rng = rand::SeedableRng::seed_from_u64(seed);
        game.world.resource_mut::<GameRng>().0 = rng;
    }
    (game, cell)
}

fn jailed(game: &Game, body: Entity) -> Jailed {
    game.world.get::<Jailed>(body).unwrap().clone()
}

#[test]
fn a_cell_takes_a_warden_through_the_ordinary_cronjob_door() {
    let (mut game, cell) = base_with_a_cell();
    let worker = spawn_tamed(&mut game, 10, 3);
    stand_in_base_at(&mut game, BESIDE.0, BESIDE.1);
    assert!(game.accepts_a_program(cell));
    game.assign_cronjob(worker, cell)
        .expect("a cell is staffable");
    assert_eq!(game.world.get::<Task>(worker).map(|t| t.target), Some(cell));
    assert!(game.world.get::<MachineStatus>(cell).is_some());
}

#[test]
fn no_warden_means_no_progress() {
    let (mut game, cell) = base_with_a_cell();
    game.jail_program(0).unwrap();
    stand_ample_grid_supply(&mut game);
    let body = the_prisoner(&game, cell);
    for _ in 0..(ATTEMPT_TICKS * 2) {
        game.tick();
    }
    let j = jailed(&game, body);
    assert_eq!((j.attempts, j.progress), (0, 0));
}

#[test]
fn a_warden_advances_progress_one_per_beat() {
    let (mut game, _cell, body) = a_warded_cell(7301);
    for _ in 0..5 {
        game.tick();
    }
    assert_eq!(jailed(&game, body).progress, 5);
}

#[test]
fn a_dark_cell_advances_nothing() {
    let (mut game, cell) = base_with_a_cell();
    game.jail_program(0).unwrap();
    let warden = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(warden).insert(Task {
        kind: TaskKind::GatherResource,
        target: cell,
        progress: 0,
        required: 1,
    });
    let body = the_prisoner(&game, cell);
    // No supply stood: the cell draws power and the grid has none to give it.
    for _ in 0..5 {
        game.tick();
    }
    assert!(
        game.world.resource::<PowerGrid>().is_dark(cell),
        "precondition"
    );
    assert_eq!(jailed(&game, body).progress, 0);
}

#[test]
fn an_attempt_fires_when_progress_reaches_attempt_ticks_and_counts() {
    // The first roll is a long shot, so the first seed that leaves the
    // prisoner jailed after it is the one that shows the counter moving.
    let mut found = false;
    for seed in 1..60u64 {
        let (mut game, _cell, body) = a_warded_cell(seed);
        game.world.get_mut::<Jailed>(body).unwrap().progress = ATTEMPT_TICKS - 1;
        game.tick();
        if game.world.get::<Jailed>(body).is_none() {
            continue;
        }
        let j = jailed(&game, body);
        assert_eq!((j.attempts, j.progress), (1, 0), "seed {seed}");
        found = true;
        break;
    }
    assert!(found, "no seed in range failed its first roll");
}

#[test]
fn a_landed_decompile_joins_the_roster_and_is_a_deed() {
    let (mut game, cell, body) = a_warded_cell(7301);
    let compiled = game.world.resource::<crate::resources::RunTally>().compiled;
    let queued = game.notifications_pending();
    let pets = game.pet_count();

    game.settle_jail_attempt(cell, body, true);

    assert!(game.world.get::<Jailed>(body).is_none());
    assert_eq!(game.program_role(body), Some(ProgramRole::Staff));
    assert_eq!(game.pet_count(), pets, "already counted from the pin");
    assert!(game.cell_prisoner(cell).is_none());
    assert_eq!(
        game.world.resource::<crate::resources::RunTally>().compiled,
        compiled + 1
    );
    assert_eq!(game.notifications_pending(), queued + 1);
    let label = game.creature_label(body);
    assert!(
        game.message_log(20)
            .iter()
            .any(|l| l.text.contains(&label) && l.text.contains("joins your roster"))
    );
}

#[test]
fn the_last_failure_breaks_the_prisoner_down_into_the_cells_output() {
    let (mut game, cell, body) = a_warded_cell(7301);
    let pets = game.pet_count();
    for expected in 1..JAIL_MAX_ATTEMPTS {
        game.settle_jail_attempt(cell, body, false);
        assert_eq!(jailed(&game, body).attempts, expected);
    }
    assert!(game.world.get_entity(body).is_ok(), "still held");

    game.settle_jail_attempt(cell, body, false);

    assert!(game.world.get_entity(body).is_err(), "the body is gone");
    assert!(game.cell_prisoner(cell).is_none());
    assert_eq!(game.pet_count(), pets - 1, "nothing joins the roster");
    let stock = game.world.get::<Stock>(cell).unwrap();
    assert!(
        stock.output.values().sum::<u32>() > 0,
        "a breakdown salvages something: {:?}",
        stock.output
    );
}

#[test]
fn a_breakdown_pays_the_scaled_extraction_yield() {
    let (game, _cell) = base_with_a_cell();
    let program = record(MID_SPECIES, Rarity::Ordinary);
    let tool = game
        .world
        .resource::<crate::tools::ToolDb>()
        .get("salvage_clamp")
        .cloned()
        .expect("the shipped salvage clamp");
    let total = |rows: Vec<(ItemId, u32)>| rows.iter().map(|(_, q)| *q).sum::<u32>();
    let full = total(game.extraction_yield(&program, &tool, 20));
    let broken = total(game.breakdown_yield(&program, &tool, 20));
    assert_eq!(
        broken,
        total(game.extraction_yield(&program, &tool, 8)),
        "{JAIL_BREAKDOWN_SCALE} of the roll, through extraction_yield"
    );
    assert!(broken < full);
}

#[test]
fn a_whole_unlucky_run_ends_in_the_roster_or_in_the_output() {
    let (mut game, cell, body) = a_warded_cell(7301);
    for _ in 0..(ATTEMPT_TICKS * JAIL_MAX_ATTEMPTS + 5) {
        game.tick();
    }
    assert!(game.cell_prisoner(cell).is_none(), "the cell is settled");
    let rostered = game.world.get_entity(body).is_ok();
    let salvage: u32 = game.world.get::<Stock>(cell).unwrap().output.values().sum();
    assert!(
        rostered ^ (salvage > 0),
        "either it joined the roster ({rostered}) or it paid out ({salvage})"
    );
}

/// The three destruction doors, each with a prisoner inside.
fn assert_door_returns_the_record(door: impl FnOnce(&mut Game, Entity)) {
    let (mut game, cell) = base_with_a_cell();
    let player = game.player_entity();
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 =
        vec![record(MID_SPECIES, Rarity::Platinum)];
    game.jail_program(0).unwrap();
    let body = the_prisoner(&game, cell);
    assert!(records(&game).is_empty());
    let packs = protocols(&game);

    door(&mut game, cell);

    assert!(game.world.get_entity(body).is_err(), "the body despawns");
    assert_eq!(
        records(&game),
        vec![record(MID_SPECIES, Rarity::Platinum)],
        "the record returns whole, level and condition included"
    );
    assert_eq!(protocols(&game), packs, "the protocol is lost");
}

#[test]
fn a_demolished_cell_returns_the_record() {
    assert_door_returns_the_record(|game, cell| {
        game.remove_structure(cell).unwrap();
    });
}

#[test]
fn a_destroyed_cell_returns_the_record() {
    assert_door_returns_the_record(|game, cell| {
        game.world
            .entity_mut(cell)
            .insert(Durability { hp: 1, max_hp: 1 });
        game.damage_structure(cell, 5, "The Holding Cell", "a GC Entropy Sweep");
    });
}

#[test]
fn an_expired_cell_returns_the_record() {
    assert_door_returns_the_record(|game, cell| {
        game.world
            .entity_mut(cell)
            .insert(Temporary { ticks_remaining: 1 });
        game.tick();
    });
}

/// A mod that pulled an ability out from under an old kill: the record's
/// `carried` still names it, but `AbilityDb` no longer resolves it, and the
/// pin must not hand the body a `Routines` entry nothing expects to fail.
#[test]
fn a_pin_drops_a_carried_routine_ability_db_no_longer_resolves() {
    let (mut game, cell) = base_with_a_cell();
    let player = game.player_entity();
    let stale = "not_a_shipped_ability".to_string();
    assert!(game.world.resource::<AbilityDb>().get(&stale).is_none());
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = vec![DownedProgram {
        carried: Some(stale.clone()),
        ..record(MID_SPECIES, Rarity::Ordinary)
    }];

    game.jail_program(0).unwrap();

    let body = the_prisoner(&game, cell);
    assert!(!game.world.get::<Routines>(body).unwrap().0.contains(&stale));
}

/// A record with no carried routine pins an **empty** list rather than
/// falling through to a roll, which would spend a `GameRng` draw. Checked on
/// the stream: the pin draws only `roll_potential`.
#[test]
fn a_pin_with_no_carried_routine_draws_nothing_for_routines() {
    use rand::RngExt;
    let (mut baseline, _) = base_with_a_cell();
    let _ = baseline.roll_potential();
    let after_baseline: u64 = baseline.world.resource_mut::<GameRng>().0.random();

    let (mut game, _) = base_with_a_cell();
    game.jail_program(0).unwrap();
    let after_pin: u64 = game.world.resource_mut::<GameRng>().0.random();

    assert_eq!(after_baseline, after_pin);
}

#[test]
fn a_prisoner_rattles_only_while_its_cell_is_working() {
    let (mut game, cell, _body) = a_warded_cell(7301);
    let mark = |game: &Game| game.view_pinned_at(PEN, 2, 2)[2][2];
    assert_eq!(mark(&game), crate::views::PinMark::Strained);
    let warden = game
        .world
        .iter_entities()
        .find(|e| e.get::<Task>().is_some_and(|t| t.target == cell))
        .map(|e| e.id())
        .unwrap();
    game.world.entity_mut(warden).remove::<Task>();
    assert_eq!(mark(&game), crate::views::PinMark::Settled);
}

#[test]
fn the_holding_cell_is_buildable_only_once_containment_is_researched() {
    let mut game = Game::new(7390, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.structure_unlocked(CELL));
    game.world
        .resource_mut::<crate::resources::Research>()
        .0
        .insert("containment".into());
    assert!(game.structure_unlocked(CELL));
}

fn output_total(game: &Game, cell: Entity) -> u32 {
    game.world.get::<Stock>(cell).unwrap().output.values().sum()
}

#[test]
fn a_breakdown_is_priced_from_the_carried_record_not_the_level_one_body() {
    let paid = |level: u32| -> u32 {
        let (mut game, cell, body) = a_warded_cell(7301);
        game.world.get_mut::<Jailed>(body).unwrap().record = Some(DownedProgram {
            level,
            ..record(MID_SPECIES, Rarity::Ordinary)
        });
        game.world.get_mut::<Jailed>(body).unwrap().attempts = JAIL_MAX_ATTEMPTS - 1;
        game.settle_jail_attempt(cell, body, false);
        output_total(&game, cell)
    };
    assert!(paid(60) > paid(1), "a higher-level kill pays more");
}

#[test]
fn a_breakdown_is_priced_without_the_bench_bonus() {
    let paid = |bench_tier: Option<u32>| -> Vec<(ItemId, u32)> {
        let (mut game, cell, body) = a_warded_cell(7301);
        if let Some(t) = bench_tier {
            let bench = spawn_structure_at(&mut game, "compiler", 9, 9);
            game.world.entity_mut(bench).insert(StructureTier(t));
            assert!(game.extraction_bench_tier() >= t, "precondition");
        }
        {
            let mut j = game.world.get_mut::<Jailed>(body).unwrap();
            j.record = Some(DownedProgram {
                level: 60,
                ..record(MID_SPECIES, Rarity::Ordinary)
            });
            j.attempts = JAIL_MAX_ATTEMPTS - 1;
        }
        game.settle_jail_attempt(cell, body, false);
        let mut rows: Vec<_> = game
            .world
            .get::<Stock>(cell)
            .unwrap()
            .output
            .clone()
            .into_iter()
            .collect();
        rows.sort();
        rows
    };
    assert_eq!(paid(None), paid(Some(6)), "a cell is not a bench's work");
}

#[test]
fn a_breakdown_with_no_room_waits_and_draws_nothing() {
    use rand::RngExt;
    let (mut game, cell, body) = a_warded_cell(7301);
    {
        let mut stock = game.world.get_mut::<Stock>(cell).unwrap();
        let room = stock.output_room();
        stock.output.insert(ItemId::from("core_fragment"), room);
        assert_eq!(stock.output_room(), 0);
    }
    game.world.get_mut::<Jailed>(body).unwrap().attempts = JAIL_MAX_ATTEMPTS - 1;
    game.world.resource_mut::<GameRng>().0 = rand::SeedableRng::seed_from_u64(99);
    let mut twin: rand::rngs::StdRng = rand::SeedableRng::seed_from_u64(99);

    game.settle_jail_attempt(cell, body, false);
    for _ in 0..5 {
        game.run_holding_cells();
    }

    assert_eq!(game.cell_prisoner(cell), Some(body), "the prisoner waits");
    assert_eq!(
        game.world.resource_mut::<GameRng>().0.random::<u64>(),
        twin.random::<u64>(),
        "a deferred breakdown draws nothing"
    );

    game.world.get_mut::<Stock>(cell).unwrap().output.clear();
    game.run_holding_cells();
    assert!(game.cell_prisoner(cell).is_none(), "room lets it through");
    assert!(output_total(&game, cell) > 0);
}

#[test]
fn a_worst_case_above_capacity_still_proceeds_once_the_stock_is_empty() {
    let (mut game, cell, body) = a_warded_cell(7301);
    {
        let mut j = game.world.get_mut::<Jailed>(body).unwrap();
        j.record = Some(DownedProgram {
            level: 60,
            ..record(MID_SPECIES, Rarity::Ordinary)
        });
        j.attempts = JAIL_MAX_ATTEMPTS - 1;
    }
    game.world.get_mut::<Stock>(cell).unwrap().capacity = 1;
    game.settle_jail_attempt(cell, body, false);
    for _ in 0..3 {
        game.run_holding_cells();
    }
    assert!(
        game.cell_prisoner(cell).is_none(),
        "an empty output must never be Clogged by a payout larger than it"
    );
}
