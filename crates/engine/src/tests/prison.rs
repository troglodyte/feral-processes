//! The Holding Cell, pin half: a downed program revived as a prisoner in a
//! pen, the `Jailed` role it reads as, and the save field that keeps it.

use super::support::*;
use crate::components::Jailed;
use crate::components::Rarity;
use crate::game::base::prison::JailBlock;
use crate::items::DownedProgram;
use crate::resources::{GameOver, Party};
use crate::structures::StructureDb;
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
    let cell = spawn_structure_at(&mut game, CELL, ANCHOR.0, ANCHOR.1);
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
    let jailed = *loaded.world.get::<Jailed>(back).unwrap();
    assert_eq!((jailed.attempts, jailed.progress), (3, 17));
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
fn a_prisoner_whose_cell_is_gone_loads_as_staff() {
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

    let back = loaded
        .owned_pets()
        .into_iter()
        .find(|p| p.name.contains("Inmate"))
        .expect("the program is back on the roster")
        .entity;
    assert_eq!(loaded.program_role(back), Some(ProgramRole::Staff));
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
