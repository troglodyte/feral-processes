//! What a skill level is worth at a machine, and what working one practises
//! (ADR 0038). The dig, farm and build sites are tested beside their own
//! fixtures in `base_space`, `farms` and `construction`.

use super::support::*;
use crate::components::{Stock, Task, TaskKind};
use crate::skills::{Skill, Skills};
use crate::*;
use bevy_ecs::entity::Entity;

fn base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 500);
    stand_in_base(&mut game);
    game
}

/// A machine of `kind` doing `recipe`, posted through the real door to a
/// worker whose `skill` is at `level` and every other skill is at zero. The
/// cycle is stretched so that nothing pays out and `Task::progress` is the
/// pure measure of pace.
fn posted(game: &mut Game, kind: &str, recipe: &str, skill: Skill, level: u32) -> (Entity, Entity) {
    place_now(game, kind, 1, 0).unwrap();
    let machine = first_structure(game, kind);
    if kind != "research_node" {
        set_recipe_researched(game, machine, recipe).unwrap();
    }
    stand_ample_grid_supply(game);
    let worker = spawn_tamed(game, 10, 3);
    game.world.entity_mut(worker).insert(Skills::default());
    set_skill_level(game, worker, skill, level);
    stand_player_at_post(game, machine);
    game.assign_cronjob(worker, machine).unwrap();
    park_at_post(game, worker, machine);
    game.world.get_mut::<Task>(worker).unwrap().required = 10_000;
    stock_inputs(game, machine);
    (machine, worker)
}

fn stock_inputs(game: &mut Game, machine: Entity) {
    let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
    stock.input.insert(ItemId::from(ids::CORE_FRAGMENT), 100);
}

fn progress_after(game: &mut Game, worker: Entity, ticks: u32) -> u32 {
    let start = game.world.get::<Task>(worker).unwrap().progress;
    for _ in 0..ticks {
        game.tick();
    }
    game.world.get::<Task>(worker).unwrap().progress - start
}

fn only_practised(game: &Game, worker: Entity, skill: Skill) {
    let skills = game.world.get::<Skills>(worker).unwrap();
    for other in Skill::ALL.into_iter().filter(|s| *s != skill) {
        let t = skills.track(other);
        assert!(
            t.practice == 0 && t.level == 0,
            "{other:?} must stay untouched, got {t:?}"
        );
    }
}

fn check_machine(kind: &str, recipe: &str, skill: Skill, seed: u32) {
    let mut game = base(seed);
    let (_, novice) = posted(&mut game, kind, recipe, skill, 0);
    let slow = progress_after(&mut game, novice, 20);
    let practice = game
        .world
        .get::<Skills>(novice)
        .unwrap()
        .track(skill)
        .practice;
    assert!(
        practice >= 15,
        "practice is flat per working tick, got {practice}"
    );
    only_practised(&game, novice, skill);

    let mut game = base(seed);
    let (_, expert) = posted(&mut game, kind, recipe, skill, 10);
    let fast = progress_after(&mut game, expert, 20);
    assert!(
        fast + 1 >= slow * 2 && fast > slow + 5,
        "level 10 should work about twice as fast as level 0: {fast} against {slow}"
    );
}

#[test]
fn an_extractor_practises_extract_and_a_high_level_works_faster() {
    check_machine("extractor", ids::CORE_FRAGMENT, Skill::Extract, 41001);
}

#[test]
fn an_assembler_practises_assemble_and_a_high_level_works_faster() {
    check_machine("factory", "bytecode_block", Skill::Assemble, 41002);
}

#[test]
fn a_research_station_practises_research_not_extract() {
    check_machine("research_node", ids::RESEARCH_DATA, Skill::Research, 41003);
}

#[test]
fn a_worker_without_skills_runs_at_exactly_one_step_a_tick() {
    let mut game = base(41004);
    let (_, worker) = posted(
        &mut game,
        "extractor",
        ids::CORE_FRAGMENT,
        Skill::Extract,
        0,
    );
    game.world.entity_mut(worker).remove::<Skills>();
    assert_eq!(progress_after(&mut game, worker, 20), 20);
}

#[test]
fn a_level_gained_mid_posting_speeds_the_next_ticks_and_is_collected() {
    let mut game = base(41005);
    let (_, worker) = posted(
        &mut game,
        "extractor",
        ids::CORE_FRAGMENT,
        Skill::Extract,
        0,
    );
    let cost = crate::skills::practice_to_next(0).unwrap();
    game.world
        .get_mut::<Skills>(worker)
        .unwrap()
        .track_mut(Skill::Extract)
        .practice = cost - 1;
    let before = progress_after(&mut game, worker, 10);
    let after = progress_after(&mut game, worker, 10);
    assert!(
        after > before,
        "level 1 works faster: {after} against {before}"
    );
    let want = format!(
        "{} is now a level 1 extractor.",
        game.creature_name(worker).unwrap()
    );
    let said: u32 = game
        .message_history(400)
        .iter()
        .filter(|m| m.text == want)
        .map(|m| m.repeats as u32)
        .sum();
    assert_eq!(said, 1);
}

#[test]
fn at_the_cap_practice_stops_accruing() {
    let mut game = base(41006);
    let cap = crate::tuning::SKILL_LEVEL_CAP;
    let (_, worker) = posted(
        &mut game,
        "extractor",
        ids::CORE_FRAGMENT,
        Skill::Extract,
        cap,
    );
    progress_after(&mut game, worker, 10);
    let t = game
        .world
        .get::<Skills>(worker)
        .unwrap()
        .track(Skill::Extract);
    assert_eq!((t.level, t.practice), (cap, 0));
}

#[test]
fn the_players_own_work_rate_is_unchanged() {
    let mut game = base(41007);
    place_now(&mut game, "extractor", 1, 0).unwrap();
    let machine = first_structure(&mut game, "extractor");
    set_recipe_researched(&mut game, machine, ids::CORE_FRAGMENT).unwrap();
    stand_ample_grid_supply(&mut game);
    stand_player_at_post(&mut game, machine);
    let speed = game.species_base_speed(game.player_entity());
    let shipped = game.work_ticks_for(machine, speed);
    game.work_structure(machine).unwrap();
    let player = game.player_entity();
    let task = game.world.get::<Task>(player).unwrap();
    assert_eq!(
        (task.kind, task.required),
        (TaskKind::GatherResource, shipped)
    );
    assert!(game.world.get::<Skills>(player).is_none());
}

#[test]
fn a_worker_at_a_clogged_extractor_earns_no_practice() {
    let mut game = base(41010);
    let (machine, worker) = posted(
        &mut game,
        "extractor",
        ids::CORE_FRAGMENT,
        Skill::Extract,
        0,
    );
    {
        let mut task = game.world.get_mut::<Task>(worker).unwrap();
        task.required = 5;
        task.progress = 5;
        let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
        let capacity = stock.capacity;
        stock
            .output
            .insert(ItemId::from(ids::CORE_FRAGMENT), capacity);
        assert_eq!(stock.output_room(), 0);
    }
    let practice = |game: &Game| {
        game.world
            .get::<Skills>(worker)
            .unwrap()
            .track(Skill::Extract)
            .practice
    };
    let before = practice(&game);
    for _ in 0..10 {
        game.tick();
    }
    assert_eq!(
        practice(&game),
        before,
        "waiting on a full output is not practice"
    );
}
