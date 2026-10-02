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

/// The single-program case above only ever exercises one iteration of
/// `Game::load`'s `(max_rank_seen + 1..).zip(pending_ranks)`; several
/// rankless programs at once is what pins the *order* — each should come
/// back one apart and in the order the file itself lists them, not merely
/// each individually greater than the pre-feature roster's max.
#[test]
fn several_rankless_programs_are_ranked_in_file_order_after_load() {
    let dir = scratch_assets_dir("duties_legacy_save_many");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(20260932, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let workers: Vec<Entity> = (0..3).map(|_| spawn_tamed(&mut game, 10, 3)).collect();
    let ids: Vec<u32> = workers
        .iter()
        .map(|&w| game.world.get::<ProgramId>(w).unwrap().0)
        .collect();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    for c in data.creatures.iter_mut() {
        if ids.contains(&c.program_id) {
            c.staff_rank = None;
        }
    }
    // Scoped to just these three rows: an ambient wild creature the map
    // seeded carries no `StaffRank` of its own either — `roster_parts` is
    // the only door that mints one — so it would already read `None` and
    // dilute this test's signal into however many the fixture happens to
    // spawn. The file's own order for the three — bevy's archetype order,
    // not necessarily creation order (`Game::save`'s own doc) — is the
    // order `Game::load` must rank them in, so it is read back off the
    // same data rather than assumed.
    let file_order: Vec<u32> = data
        .creatures
        .iter()
        .filter(|c| ids.contains(&c.program_id))
        .map(|c| c.program_id)
        .collect();
    assert_eq!(
        file_order.len(),
        3,
        "precondition: all three rows are in the file"
    );
    save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let mut query = loaded.world.query::<(&ProgramId, &StaffRank)>();
    let rank_by_id: std::collections::HashMap<u32, u32> = query
        .iter(&loaded.world)
        .filter(|(id, _)| ids.contains(&id.0))
        .map(|(id, rank)| (id.0, rank.0))
        .collect();
    let ranks: Vec<u32> = file_order.iter().map(|id| rank_by_id[id]).collect();
    assert_eq!(
        ranks[1],
        ranks[0] + 1,
        "consecutive and in file order: {ranks:?}"
    );
    assert_eq!(
        ranks[2],
        ranks[1] + 1,
        "consecutive and in file order: {ranks:?}"
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
/// a burner short of fuel, an order three machines make, a standing job and
/// two dig marks (appended last). Eight wants. The three machines are Mining
/// Nodes that touch, which feeds nothing: a feed-connected chain is one
/// production line and one want, which would turn this gate into a test of
/// seven bodies against six wants.
struct MixedBase {
    game: Game,
    site: Entity,
    burner: Entity,
    mine: Entity,
    mine_b: Entity,
    mine_c: Entity,
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
    let mine_b = spawn_machine_at(&mut game, "mining_node", 3, 0);
    let mine_c = spawn_machine_at(&mut game, "mining_node", 4, 0);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 8);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 5_000))
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
        mine_b,
        mine_c,
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
        mine_b,
        mine_c,
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
            Some((mine_b, GatherResource)),
            Some((mine_c, GatherResource)),
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
            Some((mine_b, GatherResource)),
            Some((mine_c, GatherResource)),
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

fn restrict(game: &mut Game, worker: Entity, off: &[Duty]) {
    game.world.entity_mut(worker).insert(Duties {
        off: off.iter().copied().collect(),
    });
}

/// A base with a build request and one dig mark, both workable: the two
/// wants of the spec's stranding case, build first.
fn build_and_dig(seed: u32) -> (Game, Entity, Entity) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 500);
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 10);
    file_build(&mut game, "depot", -2, 2).unwrap();
    let site = game.build_site_at(-2, 2).expect("a request was filed");
    let r = crate::tuning::STARTING_POCKET_RADIUS;
    game.toggle_mark_box((0, r + 1), (0, r + 1), None);
    let dig = game
        .dig_site_at(0, r + 1)
        .expect("a marked cell has a site");
    (game, site, dig)
}

/// The spec's stranding case: A (every column, first in the table) and B
/// (Build only) against `[Build, Dig]`. A greedy hand-out gives A the build
/// and leaves the dig with nobody who may take it; the matching seats B on
/// the build and A on the dig.
#[test]
fn a_restricted_body_does_not_strand_the_want_behind_it() {
    let (mut game, site, dig) = build_and_dig(20261002);
    let staff = hire(&mut game, 2);
    let (a, b) = (staff[0], staff[1]);
    restrict(&mut game, b, &[Duty::Operate, Duty::Guard, Duty::Dig]);

    game.schedule_base_labour();

    assert_eq!(post_of(&game, b), Some((site, TaskKind::Construct)));
    assert_eq!(post_of(&game, a), Some((dig, TaskKind::Excavate)));
    assert!(game.labour_demand().unworked.is_empty());
}

/// A standing Research Node job and one body on it — a base with no work
/// order at all, so the pass takes the empty-queue early return unless
/// something forces it through.
fn a_quiet_base_with_one_post(seed: u32) -> (Game, Entity, Entity) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let node = spawn_machine_at(&mut game, "research_node", 2, 0);
    game.set_standing_job(node, true, false).unwrap();
    let worker = hire(&mut game, 1)[0];
    game.schedule_base_labour();
    assert_eq!(
        post_of(&game, worker),
        Some((node, TaskKind::GatherResource)),
        "precondition: the one body is on the node"
    );
    (game, node, worker)
}

/// Unchecking a column takes the body off at the next pass — on the
/// empty-queue path too, which otherwise never reads the table at all — and
/// the want it leaves is counted under that column.
#[test]
fn unchecking_a_column_frees_the_body_even_on_an_empty_queue() {
    let (mut game, _node, worker) = a_quiet_base_with_one_post(20261003);
    assert!(
        game.work_orders().is_empty(),
        "precondition: nothing but the standing job"
    );

    restrict(&mut game, worker, &[Duty::Operate]);
    game.schedule_base_labour();

    assert_eq!(post_of(&game, worker), None);
    let demand = game.labour_demand();
    assert_eq!(demand.unworked.get(&Duty::Operate), Some(&1));
    assert_eq!(demand.unworked.len(), 1, "{:?}", demand.unworked);

    game.schedule_base_labour();
    assert_eq!(post_of(&game, worker), None, "and it is not posted back");
}

/// Checking a *different* column changes nothing: the pass still takes its
/// quiet early return and the body keeps its post and its progress.
#[test]
fn unchecking_an_unrelated_column_leaves_the_body_where_it_is() {
    let (mut game, node, worker) = a_quiet_base_with_one_post(20261004);
    game.world.get_mut::<Task>(worker).unwrap().progress = 3;

    restrict(&mut game, worker, &[Duty::Dig, Duty::Build]);
    game.schedule_base_labour();

    let task = game.world.get::<Task>(worker).unwrap();
    assert_eq!((task.target, task.progress), (node, 3));
}

/// Table order is the pick order: one want, two idle bodies — the higher
/// row gets it, and swapping the two ranks swaps the answer.
#[test]
fn table_order_decides_which_idle_body_is_posted() {
    for swap in [false, true] {
        let mut game = Game::new(20261005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        stand_in_base(&mut game);
        place_home(&mut game);
        let node = spawn_machine_at(&mut game, "research_node", 2, 0);
        game.set_standing_job(node, true, false).unwrap();
        let staff = hire(&mut game, 2);
        if swap {
            let (r0, r1) = (
                *game.world.get::<StaffRank>(staff[0]).unwrap(),
                *game.world.get::<StaffRank>(staff[1]).unwrap(),
            );
            game.world.entity_mut(staff[0]).insert(r1);
            game.world.entity_mut(staff[1]).insert(r0);
        }
        let first = game.base_staff()[0];

        game.schedule_base_labour();

        assert_eq!(
            post_of(&game, first).map(|p| p.0),
            Some(node),
            "swap {swap}"
        );
        let other = if first == staff[0] {
            staff[1]
        } else {
            staff[0]
        };
        assert_eq!(post_of(&game, other), None, "swap {swap}");
        assert_eq!(first, if swap { staff[1] } else { staff[0] });
    }
}

/// Stability: the body at the **top** of the table, already holding the
/// lower-priority want, keeps it and the idle body below takes the higher
/// want. A matching that simply walked the table would hand the build to
/// the top row and move it off the dig — the same two wants worked, and a
/// cronjob restarted from zero for nothing.
#[test]
fn a_posted_body_is_not_moved_when_moving_gains_nothing() {
    let (mut game, site, dig) = build_and_dig(20261006);
    let staff = hire(&mut game, 2);
    restrict(
        &mut game,
        staff[0],
        &[Duty::Operate, Duty::Guard, Duty::Build],
    );
    game.schedule_base_labour();
    assert_eq!(post_of(&game, staff[0]), Some((dig, TaskKind::Excavate)));
    assert_eq!(post_of(&game, staff[1]), Some((site, TaskKind::Construct)));
    game.world.get_mut::<Task>(staff[0]).unwrap().progress = 2;

    // Every column back on for the digger, and the builder stood down by
    // hand: either body could now take either want.
    game.world.entity_mut(staff[0]).remove::<Duties>();
    game.world.entity_mut(staff[1]).remove::<Task>();
    game.schedule_base_labour();

    assert_eq!(post_of(&game, staff[0]), Some((dig, TaskKind::Excavate)));
    assert_eq!(game.world.get::<Task>(staff[0]).unwrap().progress, 2);
    assert_eq!(post_of(&game, staff[1]), Some((site, TaskKind::Construct)));
}

/// The one forced keep that does not yield to the table: a body holding a
/// load keeps its post with its column unchecked, because freeing it would
/// destroy the load — and is freed on the first pass after it sets it down.
#[test]
fn a_carrying_body_delivers_before_its_unchecked_column_frees_it() {
    let (mut game, node, worker) = a_quiet_base_with_one_post(20261007);
    game.world.entity_mut(worker).insert(Carrying {
        item: ItemId::from(ids::CORE_FRAGMENT),
        qty: 2,
    });
    restrict(&mut game, worker, &[Duty::Operate]);

    game.schedule_base_labour();
    assert_eq!(
        post_of(&game, worker),
        Some((node, TaskKind::GatherResource)),
        "a loaded body is never freed"
    );
    assert!(
        game.world.get::<Carrying>(worker).is_some(),
        "nor is its load"
    );

    game.world.entity_mut(worker).remove::<Carrying>();
    game.schedule_base_labour();
    assert_eq!(post_of(&game, worker), None, "delivered, it is freed");
}

/// The other forced keep — a body on a clogged machine while a Depot stands
/// — yields to the table: an unchecked Operate column frees it where an
/// all-checked body is kept.
#[test]
fn a_clogged_machines_keep_respects_the_table() {
    for restricted in [false, true] {
        let mut game = Game::new(20261008, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        stand_in_base(&mut game);
        place_home(&mut game);
        let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);
        spawn_machine_at(&mut game, "depot", -2, 2);
        game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 5000))
            .unwrap();
        let worker = hire(&mut game, 1)[0];
        game.schedule_base_labour();
        assert_eq!(
            post_of(&game, worker).map(|p| p.0),
            Some(mine),
            "precondition"
        );
        {
            let mut stock = game.world.get_mut::<Stock>(mine).unwrap();
            let room = stock.output_room();
            *stock
                .output
                .entry(ItemId::from(ids::CORE_FRAGMENT))
                .or_default() += room;
        }
        if restricted {
            restrict(&mut game, worker, &[Duty::Operate]);
        }

        game.schedule_base_labour();

        assert_eq!(
            post_of(&game, worker).is_some(),
            !restricted,
            "restricted {restricted}"
        );
    }
}

/// Duties narrow which jobs an *on-shift* body may take and nothing else: a
/// downed body is freed and left out of the count exactly as it is with
/// every column checked, and its restriction is never read as a want it
/// could not take.
#[test]
fn a_downed_body_is_untouched_by_its_duties() {
    for restricted in [false, true] {
        let (mut game, _node, worker) = a_quiet_base_with_one_post(20261009);
        if restricted {
            restrict(&mut game, worker, &Duty::ALL);
        }
        game.world
            .entity_mut(worker)
            .insert(crate::components::Downed);

        game.schedule_base_labour();

        assert_eq!(post_of(&game, worker), None, "restricted {restricted}");
        let demand = game.labour_demand();
        assert_eq!(demand.staff, 0, "restricted {restricted}");
        assert_eq!(demand.unworked.get(&Duty::Operate), Some(&1));
    }
}

// ---------------------------------------------------------------------
// Phase 3: the engine API — `work_table`, `set_duty`, `set_duty_column`
// and `move_staff_row`.
// ---------------------------------------------------------------------

#[test]
fn work_table_columns_are_duty_all_in_order_with_labels() {
    let (mut game, _node, _worker) = a_quiet_base_with_one_post(20261010);
    let table = game.work_table();
    let keys: Vec<crate::views::WorkColumnKey> = table.columns.iter().map(|c| c.key).collect();
    let duty_keys: Vec<crate::views::WorkColumnKey> =
        Duty::ALL.map(crate::views::WorkColumnKey::Duty).to_vec();
    assert_eq!(keys, duty_keys);
    let labels: Vec<&str> = table.columns.iter().map(|c| c.label).collect();
    assert_eq!(labels, ["OPERATE", "GUARD", "DIG", "BUILD"]);
}

#[test]
fn work_table_unworked_counts_match_labour_demand() {
    let (mut game, _node, worker) = a_quiet_base_with_one_post(20261011);
    restrict(&mut game, worker, &[Duty::Operate]);
    game.schedule_base_labour();

    let table = game.work_table();
    let demand = game.labour_demand();
    assert_eq!(table.on_shift, demand.staff);
    assert_eq!(table.jobs, demand.wanted);
    assert_eq!(
        table.unworked_total,
        demand.unworked.values().sum::<usize>()
    );
    let operate = table
        .columns
        .iter()
        .find(|c| c.key == crate::views::WorkColumnKey::Duty(Duty::Operate))
        .unwrap();
    assert_eq!(operate.unworked, 1);
    for other in [Duty::Guard, Duty::Dig, Duty::Build] {
        let column = table
            .columns
            .iter()
            .find(|c| c.key == crate::views::WorkColumnKey::Duty(other))
            .unwrap();
        assert_eq!(column.unworked, 0, "{other:?}");
    }
}

/// Staff first, then away, each in rank order — the table's whole row
/// order, across the one split that matters (`ProgramRole::Staff` against
/// everything else).
#[test]
fn work_table_rows_are_staff_then_away_in_rank_order() {
    let mut game = Game::new(20261012, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let staff = hire(&mut game, 2);
    let away = hire(&mut game, 1)[0];
    game.world.resource_mut::<Party>().0.push(away);

    let table = game.work_table();
    let entities: Vec<Entity> = table.rows.iter().map(|r| r.program.entity).collect();
    assert_eq!(entities, vec![staff[0], staff[1], away]);
    let sections: Vec<views::WorkSection> = table.rows.iter().map(|r| r.section).collect();
    assert_eq!(
        sections,
        vec![
            views::WorkSection::Staff,
            views::WorkSection::Staff,
            views::WorkSection::Away
        ]
    );
    assert_eq!(table.rows[2].role, Some(ProgramRole::InParty));
    // Rank order is preserved inside each half, not just membership.
    assert!(table.rows[0].rank < table.rows[1].rank);
}

/// `Mode::BaseStaff` spends no tick of its own, so without a reschedule
/// inside `set_duty` the table's `N!` counts would read the previous
/// pass's figures until the next real tick — stale the moment the player
/// presses `Space`.
#[test]
fn set_duty_reschedules_so_the_table_updates_without_a_tick() {
    let (mut game, _node, worker) = a_quiet_base_with_one_post(20261024);
    assert_eq!(
        game.work_table().unworked_total,
        0,
        "precondition: everything is worked"
    );

    game.set_duty(worker, Duty::Operate, false).unwrap();

    assert_eq!(
        game.work_table().unworked_total,
        1,
        "set_duty reschedules on its own"
    );
}

/// The `[A]` whole-column toggle carries the same obligation, and reassigns
/// once for the whole column rather than once per row.
#[test]
fn set_duty_column_reschedules_so_the_table_updates_without_a_tick() {
    let (mut game, _node, worker) = a_quiet_base_with_one_post(20261025);
    let _ = worker;
    assert_eq!(game.work_table().unworked_total, 0);

    game.set_duty_column(Duty::Operate, false).unwrap();

    assert_eq!(
        game.work_table().unworked_total,
        1,
        "set_duty_column reschedules on its own"
    );
}

/// A second body on the tantrum rung beside `a_quiet_base_with_one_post`'s
/// worker, so a toggle that ran the beat's clock stages would roll for it.
/// Soured rather than only marked, or the beat's `update_disgruntled` clears
/// the marker before `run_tantrums` sees it and the test goes vacuous —
/// `disposition.rs`'s `sour_to`, which that file keeps private.
fn a_quiet_base_with_a_body_lashing_out(seed: u32) -> (Game, Entity, Entity) {
    let (mut game, _node, worker) = a_quiet_base_with_one_post(seed);
    // `run_tantrums` does nothing on a young base, so it is made an
    // established one: fillers with every column off, so none of them can
    // cover the post the worker leaves, and Depots for the structure count.
    let fillers = hire(&mut game, crate::tuning::BASE_ESTABLISHED_STAFF);
    for &filler in &fillers {
        restrict(&mut game, filler, &Duty::ALL);
    }
    let depot = game
        .world
        .resource::<crate::structures::StructureDb>()
        .get(&crate::structures::StructureId::from("depot"))
        .expect("a Depot ships")
        .clone();
    for i in 0..crate::tuning::BASE_ESTABLISHED_STRUCTURES {
        game.spawn_structure(&depot, -20 - i as i32, 20, None);
    }
    assert!(game.base_is_established(), "precondition: tantrums can run");
    let angry = fillers[0];
    let now = game.current_tick();
    let mut n = 0;
    while game.morale(angry) > crate::tuning::MORALE_LASHES_OUT_AT {
        game.world
            .get_mut::<crate::components::Memories>(angry)
            .expect("a roster program holds a store")
            .0
            .push(crate::components::Memory {
                def: crate::memories::MemoryId::from("frayed_here"),
                subject: crate::components::MemorySubject::BaseTile { x: n, y: 900 },
                subject_name: None,
                reinforced: now,
                strikes: 1,
            });
        n += 1;
        assert!(n < 400, "morale never reached the tantrum rung");
    }
    game.world
        .entity_mut(angry)
        .insert(crate::components::Disgruntled {
            grievance: crate::components::Grievance::LashingOut,
            stranded: false,
            told: false,
        });
    (game, worker, angry)
}

fn peek_rng(game: &mut Game) -> u64 {
    use rand::RngExt;
    game.world
        .resource_mut::<crate::resources::GameRng>()
        .0
        .random()
}

/// A duty toggle is a keypress with the clock stopped, so it must run none
/// of the beat's clock-driven stages: a tantrum rolled per press would let
/// a player re-roll the base by spamming `Space`. It still refreshes the
/// table.
#[test]
fn set_duty_draws_no_rng_and_rolls_no_tantrum() {
    let (mut game, worker, _angry) = a_quiet_base_with_a_body_lashing_out(20261027);
    reseed_rng(&mut game, 55);
    let untouched = peek_rng(&mut game);

    reseed_rng(&mut game, 55);
    for _ in 0..50 {
        game.set_duty(worker, Duty::Operate, false).unwrap();
        game.set_duty(worker, Duty::Operate, true).unwrap();
    }
    game.set_duty(worker, Duty::Operate, false).unwrap();
    let toggled = peek_rng(&mut game);

    assert_eq!(untouched, toggled, "a toggle must not draw GameRng");
    assert!(
        game.world
            .resource::<crate::resources::Brawls>()
            .open
            .is_empty()
    );
    assert_eq!(
        game.work_table().unworked_total,
        1,
        "the toggle still refreshes the table"
    );
}

/// The `[A]` column toggle carries the same obligation.
#[test]
fn set_duty_column_draws_no_rng_and_rolls_no_tantrum() {
    let (mut game, _worker, _angry) = a_quiet_base_with_a_body_lashing_out(20261028);
    reseed_rng(&mut game, 55);
    let untouched = peek_rng(&mut game);

    reseed_rng(&mut game, 55);
    for _ in 0..50 {
        game.set_duty_column(Duty::Operate, false).unwrap();
        game.set_duty_column(Duty::Operate, true).unwrap();
    }
    game.set_duty_column(Duty::Operate, false).unwrap();
    let toggled = peek_rng(&mut game);

    assert_eq!(untouched, toggled, "a column toggle must not draw GameRng");
    assert!(
        game.world
            .resource::<crate::resources::Brawls>()
            .open
            .is_empty()
    );
    assert_eq!(game.work_table().unworked_total, 1);
}

/// Right after a real `Game::load` — before any tick, and before the player
/// has pressed anything — the Base staff screen used to read
/// `resources::LabourDemand`'s untouched `Default`: 0 on shift, 0 jobs, 0
/// unworked, whatever the save actually holds. `on_shift` is now derived at
/// load, so a save with a posted staff program reports it immediately.
#[test]
fn a_freshly_loaded_game_reports_nonzero_on_shift_with_no_tick() {
    let (mut game, _node, _worker) = a_quiet_base_with_one_post(20261026);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_duties_on_shift_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.work_table().on_shift,
        1,
        "on-shift is derived at load rather than left at the scheduler's \
         Default zero until the first real tick"
    );
}

#[test]
fn set_duty_toggles_one_cell_and_leaves_the_others() {
    let mut game = Game::new(20261013, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = hire(&mut game, 1)[0];

    game.set_duty(worker, Duty::Dig, false).unwrap();
    let table = game.work_table();
    let row = table
        .rows
        .iter()
        .find(|r| r.program.entity == worker)
        .unwrap();
    let cell = |d: Duty| {
        row.cells[table
            .columns
            .iter()
            .position(|c| c.key == crate::views::WorkColumnKey::Duty(d))
            .unwrap()]
    };
    assert!(!cell(Duty::Dig));
    for other in [Duty::Operate, Duty::Guard, Duty::Build] {
        assert!(cell(other), "{other:?}");
    }
}

#[test]
fn set_duty_removes_the_component_once_every_column_is_back_on() {
    let mut game = Game::new(20261014, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = hire(&mut game, 1)[0];

    game.set_duty(worker, Duty::Dig, false).unwrap();
    assert!(game.world.get::<Duties>(worker).is_some());

    game.set_duty(worker, Duty::Dig, true).unwrap();
    assert!(
        game.world.get::<Duties>(worker).is_none(),
        "every column back on must drop the component, not leave it empty"
    );
}

#[test]
fn set_duty_refuses_an_entity_the_player_does_not_own() {
    let mut game = Game::new(20261015, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let stranger = game.world.spawn(()).id();
    assert!(game.set_duty(stranger, Duty::Dig, false).is_err());
}

#[test]
fn set_duty_column_toggles_every_owned_program_including_away_ones() {
    let mut game = Game::new(20261016, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let staff = hire(&mut game, 2);
    let away = hire(&mut game, 1)[0];
    game.world.resource_mut::<Party>().0.push(away);

    game.set_duty_column(Duty::Guard, false).unwrap();
    for &e in staff.iter().chain(std::iter::once(&away)) {
        assert!(
            game.world
                .get::<Duties>(e)
                .is_some_and(|d| d.off.contains(&Duty::Guard)),
            "entity should have Guard off"
        );
    }

    game.set_duty_column(Duty::Guard, true).unwrap();
    for &e in staff.iter().chain(std::iter::once(&away)) {
        assert!(game.world.get::<Duties>(e).is_none());
    }
}

#[test]
fn move_staff_row_swaps_with_its_neighbour_and_selection_follows_the_math() {
    let mut game = Game::new(20261017, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = hire(&mut game, 3);
    assert_eq!(game.base_staff(), staff, "precondition: hire order");

    game.move_staff_row(staff[2], -1).unwrap();
    assert_eq!(game.base_staff(), vec![staff[0], staff[2], staff[1]]);
}

#[test]
fn move_staff_row_at_the_top_is_a_no_op_rather_than_a_refusal() {
    let mut game = Game::new(20261018, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = hire(&mut game, 2);

    assert!(game.move_staff_row(staff[0], -1).is_ok());
    assert_eq!(game.base_staff(), staff, "clamped at the top: unchanged");

    assert!(game.move_staff_row(staff[1], 1).is_ok());
    assert_eq!(game.base_staff(), staff, "clamped at the bottom: unchanged");
}

#[test]
fn move_staff_row_refuses_an_entity_the_player_does_not_own() {
    let mut game = Game::new(20261019, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let stranger = game.world.spawn(()).id();
    assert!(game.move_staff_row(stranger, 1).is_err());
}

/// The spec's interleaved case: two staff rows with an away program's rank
/// sitting between them. `work_table` displays Staff first, then Away — the
/// away row is not the row `<`/`>` should ever land on. Moving the raw
/// *rank* neighbour (the away program) would land here with nothing to see
/// move; moving the displayed neighbour (the other staff row) is the fix.
#[test]
fn move_staff_row_moves_within_the_displayed_order_not_raw_rank_order() {
    let mut game = Game::new(20261020, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let s1 = hire(&mut game, 1)[0];
    let away = hire(&mut game, 1)[0];
    game.world.resource_mut::<Party>().0.push(away);
    let s2 = hire(&mut game, 1)[0];
    // Precondition: raw rank order interleaves `away` between the two staff
    // rows, but the screen's own order does not.
    assert_eq!(game.base_staff(), vec![s1, s2], "precondition: hire order");
    let displayed = |game: &mut Game| -> Vec<Entity> {
        game.work_table()
            .rows
            .iter()
            .map(|r| r.program.entity)
            .collect()
    };
    assert_eq!(
        displayed(&mut game),
        vec![s1, s2, away],
        "precondition: Staff, Staff, then Away"
    );

    game.move_staff_row(s1, 1).unwrap();

    assert_eq!(
        displayed(&mut game),
        vec![s2, s1, away],
        "`>` on the top staff row swaps with the other staff row, not the \
         away row sitting between them in raw rank order"
    );
}
