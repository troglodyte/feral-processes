//! `Duty`, `duty_admits` and `components::StaffRank` — phase 1 of work
//! assignments (`docs/superpowers/specs/2026-09-27-work-assignments-design.md`).
//! No scheduler change lands here; these are the data model and the roster
//! doors that mint a rank.

use super::support::*;
use crate::components::{Duties, StaffRank};
use crate::duties::{Duty, PostDesc, duty_admits};
use crate::*;

/// `duty_admits`'s "some checked duty admits it" rule only works because
/// v1's four duties don't overlap — this is the census that keeps it true.
/// A fifth `TaskKind` admitted by zero or two duties would silently break
/// the "some" shortcut without failing `Duty::admits`'s own exhaustive
/// match.
#[test]
fn exactly_one_duty_admits_each_task_kind() {
    for kind in [
        TaskKind::GatherResource,
        TaskKind::Guard,
        TaskKind::Excavate,
        TaskKind::Construct,
    ] {
        let post = PostDesc {
            kind,
            structure: None,
        };
        let admitting: Vec<Duty> = Duty::ALL.into_iter().filter(|d| d.admits(&post)).collect();
        assert_eq!(
            admitting.len(),
            1,
            "{kind:?} should be admitted by exactly one duty, got {admitting:?}"
        );
    }
}

#[test]
fn duty_admits_with_no_duties_component_is_always_true() {
    for kind in [
        TaskKind::GatherResource,
        TaskKind::Guard,
        TaskKind::Excavate,
        TaskKind::Construct,
    ] {
        let post = PostDesc {
            kind,
            structure: None,
        };
        assert!(
            duty_admits(None, &post),
            "an absent Duties component must read as everything checked"
        );
    }
}

#[test]
fn unchecking_the_admitting_duty_refuses_the_post() {
    let post = PostDesc {
        kind: TaskKind::Excavate,
        structure: None,
    };
    let off = Duties {
        off: [Duty::Dig].into_iter().collect(),
    };
    assert!(!duty_admits(Some(&off), &post));
}

#[test]
fn unchecking_a_different_duty_leaves_the_post_admitted() {
    let post = PostDesc {
        kind: TaskKind::Excavate,
        structure: None,
    };
    let off = Duties {
        off: [Duty::Build].into_iter().collect(),
    };
    assert!(duty_admits(Some(&off), &post));
}

#[test]
fn duty_name_round_trips_through_from_name() {
    for d in Duty::ALL {
        assert_eq!(
            Duty::from_name(d.name()),
            Some(d),
            "{d:?}'s name should round-trip"
        );
    }
}

#[test]
fn duty_from_name_is_none_for_an_unknown_name() {
    assert_eq!(
        Duty::from_name("mine"),
        None,
        "a retired or unrecognised name must load inert, not panic or guess"
    );
}

/// Every roster door mints a distinct, increasing rank — the barrier
/// `roster_parts` is for every other component it hands out. Exercised
/// across two different doors (the `spawn_tamed` fixture, which calls
/// `roster_parts` directly, and `adopt_program`) so this can't pass by
/// coincidence of one call site's own counter.
#[test]
fn roster_doors_mint_distinct_increasing_ranks() {
    let mut game = Game::new(20260927, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let first = spawn_tamed(&mut game, 10, 3);
    let rank_first = game.world.get::<StaffRank>(first).unwrap().0;

    let second = spawn_tamed(&mut game, 10, 3);
    let rank_second = game.world.get::<StaffRank>(second).unwrap().0;
    assert!(
        rank_second > rank_first,
        "a later door should mint a higher rank: {rank_first} then {rank_second}"
    );

    let species = game.species_defs().into_iter().next().unwrap().id.clone();
    let third = game.adopt_program(&species, 4, 4, 1.0).unwrap();
    let rank_third = game.world.get::<StaffRank>(third).unwrap().0;
    assert!(
        rank_third > rank_second,
        "adopt_program should mint a rank past every existing one: {rank_second} then {rank_third}"
    );
}

/// Fusion is the one door that does not mint a fresh rank for the result —
/// it overrides `roster_parts`' fresh one with the dominant parent's own
/// place in line and job restrictions, `KernelRing`/`Talents`' precedent
/// one paragraph up in `fuse_companions`.
#[test]
fn fusion_keeps_the_dominant_parents_duties_and_rank() {
    let mut game = Game::new(20260928, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "program_refactoring");
    let player = game.player_entity();
    let species = game.species_defs();
    let species_a = species[0].id.clone();
    let species_b = species[1 % species.len()].id.clone();

    // `a` is the dominant parent: higher level wins, and ties favor `a`.
    let a = game
        .world
        .spawn((
            Creature { species: species_a },
            Position { x: 3, y: 3 },
            Stats {
                hp: 20,
                max_hp: 20,
                atk: 10,
                mitigation: 4,
            },
            Potential {
                hp_roll: 1.0,
                atk_roll: 1.0,
                def_roll: 1.0,
                growth_roll: 1.0,
                assembly_roll: 1.0,
                extraction_roll: 1.0,
            },
            Tamed { owner: player },
            PowerReserve::default(),
            Experience {
                level: 5,
                xp: 0,
                xp_to_next: 100,
            },
            Duties {
                off: [Duty::Guard].into_iter().collect(),
            },
            StaffRank(3),
        ))
        .id();
    let b = game
        .world
        .spawn((
            Creature { species: species_b },
            Position { x: 4, y: 4 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 6,
                mitigation: 2,
            },
            Potential {
                hp_roll: 1.0,
                atk_roll: 1.0,
                def_roll: 1.0,
                growth_roll: 1.0,
                assembly_roll: 1.0,
                extraction_roll: 1.0,
            },
            Tamed { owner: player },
            PowerReserve::default(),
            Experience {
                level: 2,
                xp: 0,
                xp_to_next: 40,
            },
            Duties {
                off: [Duty::Build].into_iter().collect(),
            },
            StaffRank(9),
        ))
        .id();

    game.fuse_companions(a, b, None).unwrap();

    let mut query = game.world.query::<(&Tamed, &StaffRank, &Duties)>();
    let (_, rank, duties) = query
        .iter(&game.world)
        .find(|(t, ..)| t.owner == player)
        .expect("a fused creature should exist");
    assert_eq!(
        rank.0, 3,
        "the fused entity should keep a's rank, not b's or a fresh one"
    );
    assert_eq!(
        duties.off,
        [Duty::Guard].into_iter().collect(),
        "the fused entity should keep a's duties, not b's"
    );
}

/// A field-named RON round trip cannot catch a skipped field —
/// `ron-round-trip-cannot-catch-a-skipped-field` — so this goes through a
/// **real** save and load, `a_disposition_survives_a_save_and_load`'s
/// pattern.
#[test]
fn off_duties_and_staff_rank_survive_a_save_and_load() {
    let dir = scratch_assets_dir("duties_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(20260929, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    let program_id = game.world.get::<ProgramId>(worker).unwrap().0;
    game.world.entity_mut(worker).insert((
        Duties {
            off: [Duty::Guard, Duty::Build].into_iter().collect(),
        },
        StaffRank(77),
    ));
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let mut query = loaded.world.query::<(&ProgramId, &Duties, &StaffRank)>();
    let (_, duties, rank) = query
        .iter(&loaded.world)
        .find(|(id, ..)| id.0 == program_id)
        .expect("the worker should still exist after the round trip");
    assert_eq!(
        duties.off,
        [Duty::Guard, Duty::Build].into_iter().collect(),
        "off_duties should survive the round trip"
    );
    assert_eq!(rank.0, 77, "staff_rank should survive the round trip");
}

/// A duty retired since the file was written (or a hand-edited name) must
/// load inert rather than fail the parse — `NotificationKind::latch_key`'s
/// rule with a second implementor.
#[test]
fn an_unknown_duty_name_loads_inert() {
    let dir = scratch_assets_dir("duties_unknown_name");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(20260930, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    let program_id = game.world.get::<ProgramId>(worker).unwrap().0;
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let saved = data
        .creatures
        .iter_mut()
        .find(|c| c.program_id == program_id)
        .expect("the worker's row should be in the file");
    saved.off_duties = vec!["mine".to_string()];
    save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let mut query = loaded.world.query::<(&ProgramId, Option<&Duties>)>();
    let has_duties = query
        .iter(&loaded.world)
        .find(|(id, _)| id.0 == program_id)
        .map(|(_, d)| d.is_some())
        .expect("the worker should still exist");
    assert!(
        !has_duties,
        "an unrecognised name must not survive as a restriction"
    );
}

/// A save written before ranks existed (or an old one hand-edited to strip
/// them back out) must still come back with every owned program ranked,
/// never with a job screen that has nothing to sort by.
#[test]
fn a_save_with_neither_field_loads_all_on_and_ranked() {
    let dir = scratch_assets_dir("duties_legacy_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(20260931, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    let program_id = game.world.get::<ProgramId>(worker).unwrap().0;
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let existing_max = data
        .creatures
        .iter()
        .filter(|c| c.program_id != program_id)
        .filter_map(|c| c.staff_rank)
        .max()
        .unwrap_or(0);
    let saved = data
        .creatures
        .iter_mut()
        .find(|c| c.program_id == program_id)
        .expect("the worker's row should be in the file");
    saved.off_duties = Vec::new();
    saved.staff_rank = None;
    save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let mut query = loaded
        .world
        .query::<(&ProgramId, Option<&Duties>, Option<&StaffRank>)>();
    let (_, duties, rank) = query
        .iter(&loaded.world)
        .find(|(id, ..)| id.0 == program_id)
        .expect("the worker should still exist");
    assert!(
        duties.is_none(),
        "no Duties component reads as every column checked"
    );
    assert_eq!(
        rank.map(|r| r.0),
        Some(existing_max + 1),
        "an unranked program should be assigned one past the highest rank the file did carry"
    );
}

// ---------------------------------------------------------------------
// Phase 2: the scheduler obeys the table
// ---------------------------------------------------------------------

/// Where `worker` is posted, as `(target, kind)`, or `None` if it is idle.
fn post_of(game: &Game, worker: Entity) -> Option<(Entity, TaskKind)> {
    game.world.get::<Task>(worker).map(|t| (t.target, t.kind))
}

/// `n` programs on the base staff, in table order.
fn hire(game: &mut Game, n: usize) -> Vec<Entity> {
    (0..n).map(|_| spawn_tamed(game, 500, 3)).collect()
}

fn put_output(game: &mut Game, machine: Entity, item: &str, qty: u32) {
    let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
    *stock.output.entry(ItemId::from(item)).or_default() += qty;
}

/// Every want kind the scheduler has, at once: a build request (prepended),
/// a burner short of fuel, a three-deep order, a standing job and two dig
/// marks (appended last). Eight wants.
struct MixedBase {
    game: Game,
    site: Entity,
    burner: Entity,
    mine: Entity,
    lathe: Entity,
    press: Entity,
    node: Entity,
    digs: Vec<Entity>,
}

fn mixed_base(seed: u32) -> MixedBase {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 500);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 10);
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);
    let lathe = spawn_machine_at(&mut game, "lathe", 3, 0);
    let press = spawn_machine_at(&mut game, "disk_press", 4, 0);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    put_output(&mut game, lathe, ids::BLANK_SUBSTRATE, 6);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::ROUTINE_DISK), 30))
        .unwrap();
    let node = spawn_machine_at(&mut game, "research_node", 2, 3);
    game.set_standing_job(node, true, false).unwrap();
    let burner = spawn_machine_at(&mut game, "recharger_node", -2, -2);
    let store = spawn_machine_at(&mut game, "depot", -3, 2);
    put_output(&mut game, store, "power_cell", 5);
    file_build(&mut game, "depot", -2, 2).unwrap();
    let site = game.build_site_at(-2, 2).expect("a request was filed");
    let r = crate::tuning::STARTING_POCKET_RADIUS;
    game.toggle_mark_box((0, r + 1), (1, r + 1), None);
    let mut digs: Vec<Entity> = [(0, r + 1), (1, r + 1)]
        .into_iter()
        .map(|(x, y)| game.dig_site_at(x, y).expect("a marked cell has a site"))
        .collect();
    digs.sort_by_key(|&d| {
        let p = game.world.get::<Position>(d).unwrap();
        (p.x, p.y)
    });
    MixedBase {
        game,
        site,
        burner,
        mine,
        lathe,
        press,
        node,
        digs,
    }
}

/// **The equivalence gate**: with every column checked, the matching hands
/// out exactly what the truncate-and-fill it replaced handed out — seven
/// bodies against eight wants, from idle and then again from a base where
/// most bodies already hold a post. Written against the old scheduler first.
#[test]
fn an_all_checked_mixed_base_schedules_as_it_did_before_the_matching() {
    let MixedBase {
        mut game,
        site,
        burner,
        mine,
        lathe,
        press,
        node,
        digs,
    } = mixed_base(20261001);
    let staff = hire(&mut game, 7);
    assert_eq!(
        game.base_staff(),
        staff,
        "precondition: table order is hire order"
    );

    game.schedule_base_labour();

    use TaskKind::*;
    let posts: Vec<_> = staff.iter().map(|&w| post_of(&game, w)).collect();
    assert_eq!(
        posts,
        vec![
            Some((site, Construct)),
            Some((burner, GatherResource)),
            Some((mine, GatherResource)),
            Some((lathe, GatherResource)),
            Some((press, GatherResource)),
            Some((node, GatherResource)),
            Some((digs[0], Excavate)),
        ],
        "wants in priority order, bodies in table order; the second dig mark is cut"
    );

    // The build is withdrawn — the cancel reschedules on the spot, so the
    // builder takes the second dig mark — and then the Lathe's body walks
    // off by hand: one body free, one want open, six that must not move.
    game.cancel_build_request(site).unwrap();
    game.world.entity_mut(staff[3]).remove::<Task>();
    game.schedule_base_labour();

    let posts: Vec<_> = staff.iter().map(|&w| post_of(&game, w)).collect();
    assert_eq!(
        posts,
        vec![
            Some((digs[1], Excavate)),
            Some((burner, GatherResource)),
            Some((mine, GatherResource)),
            Some((lathe, GatherResource)),
            Some((press, GatherResource)),
            Some((node, GatherResource)),
            Some((digs[0], Excavate)),
        ],
        "every holder keeps its post; the freed body fills the open want"
    );

    // Two bodies stood down by hand: the higher want (the burner) goes to
    // the higher row (the dig mark's old body), whoever held it last.
    // Both set down in open floor, so each can walk to either post and the
    // question is table order alone.
    for (w, x) in [(staff[0], -1), (staff[1], 1)] {
        game.world.entity_mut(w).remove::<Task>();
        *game.world.get_mut::<Position>(w).unwrap() = Position { x, y: 1 };
    }
    game.schedule_base_labour();

    assert_eq!(post_of(&game, staff[0]), Some((burner, GatherResource)));
    assert_eq!(post_of(&game, staff[1]), Some((digs[1], Excavate)));
    for (i, &w) in staff.iter().enumerate().skip(2) {
        assert_eq!(post_of(&game, w), posts[i], "row {i} must not move");
    }
}
