//! The player carries `Skills` (ADR 0040): seeded by class, saved, practised
//! by fighting, and blind to work.

use super::support::*;
use crate::classes::{ClassDb, PlayerClass};
use crate::components::{Experience, FusionCount};
use crate::save;
use crate::skills::{Skill, Skills};
use crate::*;

fn new_game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A scratch assets dir whose Striker starts with Force 7 and Aim 3.
fn striker_assets(name: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(name);
    std::fs::create_dir_all(&*dir).unwrap();
    copy_shipped_assets(&dir, &[]);
    std::fs::create_dir_all(dir.join("classes")).unwrap();
    let path = dir.join("classes/striker.ron");
    std::fs::copy(test_assets_dir().join("classes/striker.ron"), &path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.replacen(
        "    kit:",
        "    starting_skills: [(Force, 7), (Aim, 3)],\n    kit:",
        1,
    );
    assert!(text.contains("(Force, 7)"));
    std::fs::write(&path, text).unwrap();
    dir
}

fn striker(dir: &std::path::Path, seed: u32) -> Game {
    let choice = CharacterChoice {
        class: Some(PlayerClass::Striker),
        ..CharacterChoice::default()
    };
    Game::new_with(seed, DifficultyMode::Forgiving, dir, &choice).expect("new_with")
}

fn player_skills(game: &Game) -> &Skills {
    game.world.get::<Skills>(game.player_entity()).unwrap()
}

#[test]
fn the_player_starts_at_its_class_seed_and_a_classless_player_at_zero() {
    let dir = striker_assets("skills_player_seed");
    let game = striker(&dir, 31010);
    let skills = player_skills(&game);
    assert_eq!(skills.level(Skill::Force), 7);
    assert_eq!(skills.level(Skill::Aim), 3);
    assert_eq!(skills.level(Skill::Defence), 0);
    assert_eq!(skills.level(Skill::Dig), 0);

    let classless = new_game(31011);
    assert_eq!(*player_skills(&classless), Skills::default());
}

#[test]
fn player_skills_survive_a_real_save_and_load() {
    let dir = striker_assets("skills_player_save");
    let path = dir.join("save.bin");
    let mut game = striker(&dir, 31012);
    let player = game.player_entity();
    {
        let mut skills = game.world.get_mut::<Skills>(player).unwrap();
        skills.set(Skill::Defence, 5, 4);
        skills.set(Skill::Force, 9, 2);
    }
    let expected = player_skills(&game).clone();
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &dir).unwrap();
    assert_eq!(*player_skills(&loaded), expected);
}

#[test]
fn an_old_player_save_without_skills_loads_at_the_class_seed() {
    let dir = striker_assets("skills_player_old_save");
    let path = dir.join("save.bin");
    let mut game = striker(&dir, 31013);
    game.save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.player.skills = Vec::new();
    save::save_to_file(&path, &data).unwrap();

    let loaded = Game::load(&path, &dir).unwrap();
    let skills = player_skills(&loaded);
    assert_eq!(skills.level(Skill::Force), 7);
    assert_eq!(skills.level(Skill::Aim), 3);
}

#[test]
fn the_player_works_at_todays_pace_with_work_skills_at_zero() {
    let mut game = new_game(31014);
    let player = game.player_entity();
    assert_eq!(player_skills(&game).level(Skill::Dig), 0);
    assert_eq!(game.skill_scale_of(player, Skill::Dig), 1.0);
    let steps: u32 = (0..10).map(|_| game.work_steps(player, Skill::Dig)).sum();
    assert_eq!(steps, 10);
    for _ in 0..50 {
        game.practise_work(player, Skill::Dig);
    }
    let t = player_skills(&game).track(Skill::Dig);
    assert_eq!(
        (t.level, t.practice),
        (0, 0),
        "the player never trains work"
    );

    // A program at the same level is slowed, so the guard is what differs.
    let worm = game.adopt_program("worm", 0, 0, 1.0).unwrap();
    set_skill_level(&mut game, worm, Skill::Dig, 0);
    assert!(game.skill_scale_of(worm, Skill::Dig) < 1.0);
}

#[test]
fn the_player_practises_every_combat_skill_in_a_fight() {
    let mut game = new_game(31015);
    let player = game.player_entity();
    let wild = game.spawn_wild_creature("worm", 1, 1).unwrap();
    {
        let mut stats = game.world.get_mut::<Stats>(wild).unwrap();
        stats.hp = 1_000_000;
        stats.max_hp = 1_000_000;
    }

    let mut swing = battle::Swing::plain(game.natural_range_of(player));
    swing.accuracy = 1_000_000;
    game.resolve_and_apply_attack(player, wild, swing);
    assert!(player_skills(&game).track(Skill::Aim).practice > 0);
    assert!(player_skills(&game).track(Skill::Force).practice > 0);

    let wild_swing = battle::Swing::plain(game.natural_range_of(wild));
    game.resolve_and_apply_attack(wild, player, wild_swing);
    assert!(player_skills(&game).track(Skill::Defence).practice > 0);

    *game.world.get_mut::<PowerReserve>(player).unwrap() = PowerReserve::new(50.0, POWER_MAX);
    game.spend_power(player, 2.0);
    assert!(player_skills(&game).track(Skill::Routines).practice > 0);
}

#[test]
fn a_fused_child_keeps_its_dominant_parents_skills() {
    let mut game = new_game(31016);
    unlock_research_chain(&mut game, "program_refactoring");
    let a = game.adopt_program("worm", 0, 0, 1.0).unwrap();
    let b = game.adopt_program("virus", 0, 0, 1.0).unwrap();
    game.world.get_mut::<Experience>(a).unwrap().level = 9;
    game.world.get_mut::<Experience>(b).unwrap().level = 2;
    {
        let mut skills = game.world.get_mut::<Skills>(a).unwrap();
        skills.set(Skill::Force, 13, 5);
        skills.set(Skill::Dig, 17, 1);
    }
    let expected = game.world.get::<Skills>(a).unwrap().clone();

    game.fuse_companions(a, b, None).unwrap();

    let mut q = game.world.query::<(&FusionCount, &Skills)>();
    let (_, skills) = q.iter(&game.world).next().expect("the fused child");
    assert_eq!(*skills, expected);
}

#[test]
fn class_starting_skills_keep_combat_rows_and_drop_the_rest_with_warnings() {
    let dir = scratch_assets_dir("classes_starting_skills");
    std::fs::create_dir_all(&*dir).unwrap();
    let text = std::fs::read_to_string(test_assets_dir().join("classes/striker.ron")).unwrap();
    let cap = crate::tuning::SKILL_LEVEL_CAP;
    let text = text.replacen(
        "    kit:",
        &format!(
            "    starting_skills: [(Force, 4), (Dig, 3), (Aim, {})],\n    kit:",
            cap + 5
        ),
        1,
    );
    std::fs::write(dir.join("striker.ron"), text).unwrap();
    let (db, warnings) = ClassDb::load_dir(&dir).unwrap();
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    let seed = db.get(PlayerClass::Striker).unwrap().seed_skills();
    assert_eq!(seed.level(Skill::Force), 4);
    assert_eq!(seed.level(Skill::Aim), cap);
    assert_eq!(seed.level(Skill::Dig), 0);
}

/// Parsed raw, since `load_dir` repairs what this census forbids.
#[test]
fn every_shipped_class_starts_on_combat_skills_within_the_cap() {
    for entry in std::fs::read_dir(test_assets_dir().join("classes")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let def: crate::classes::ClassDef =
            ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for (skill, level) in &def.starting_skills {
            assert!(
                Skill::COMBAT.contains(skill),
                "{:?} starts on {}, which is not a combat skill",
                def.class,
                skill.name()
            );
            assert!(*level <= crate::tuning::SKILL_LEVEL_CAP);
        }
    }
}
