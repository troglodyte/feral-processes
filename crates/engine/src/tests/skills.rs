//! Who carries `Skills`, and what survives a save (ADR 0038, 0039).

use super::support::*;
use crate::components::ProgramId;
use crate::save;
use crate::skills::{Skill, Skills};
use crate::*;
use bevy_ecs::entity::Entity;

fn new_game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn seed_of(game: &Game, species: &str) -> Skills {
    game.species_defs()
        .into_iter()
        .find(|d| d.id == species)
        .unwrap()
        .seed_skills()
}

/// A program tamed through the real door, of a species that ships a head
/// start, so a seed that came out all zero would fail the comparison.
fn tame(game: &mut Game, species: &str) -> Entity {
    let seed = seed_of(game, species);
    assert_ne!(
        seed,
        Skills::default(),
        "{species} ships no starting skills"
    );
    game.adopt_program(species, 0, 0, 1.0).unwrap()
}

fn skills_of(game: &mut Game, program_id: u32) -> Option<Skills> {
    let mut q = game.world.query::<(&ProgramId, &Skills)>();
    q.iter(&game.world)
        .find(|(id, _)| id.0 == program_id)
        .map(|(_, s)| s.clone())
}

#[test]
fn a_fresh_tame_starts_at_its_species_seed() {
    let mut game = new_game(31001);
    let worm = tame(&mut game, "worm");
    assert_eq!(
        *game.world.get::<Skills>(worm).unwrap(),
        seed_of(&game, "worm")
    );
    assert_eq!(game.world.get::<Skills>(worm).unwrap().level(Skill::Dig), 4);
}

#[test]
fn wild_programs_have_no_skills() {
    let mut game = new_game(31002);
    let wild = game.spawn_wild_creature("worm", 1, 1).unwrap();
    assert!(game.world.get::<Skills>(wild).is_none());
}

#[test]
fn a_bred_child_starts_at_its_species_seed_and_inherits_nothing() {
    let (mut game, a, b, bay) = {
        let mut game = new_game(31003);
        stand_in_base(&mut game);
        // The visit's walk is the base drift's, which needs a Home.
        spawn_machine_at(&mut game, crate::HOME_STRUCTURE_ID, -6, 2);
        let bay = spawn_machine_at(&mut game, "breeding_bay", 3, 3);
        let a = game.adopt_program("worm", 0, 0, 1.0).unwrap();
        let b = game.adopt_program("virus", 0, 0, 1.0).unwrap();
        (game, a, b, bay)
    };
    let seed = crate::items::ItemId::from("breeding_seed");
    let player = game.player_entity();
    game.world
        .get_mut::<crate::components::Inventory>(player)
        .unwrap()
        .add(seed, 1);
    for parent in [a, b] {
        game.world
            .get_mut::<Skills>(parent)
            .unwrap()
            .track_mut(Skill::Dig)
            .level = 18;
    }
    game.breed(a, b, bay).unwrap();
    super::breeding::finish_visit(&mut game, bay);
    game.world.resource_mut::<GameClock>().tick += crate::tuning::INCUBATION_TICKS;
    game.hatch_incubations();

    let mut q = game
        .world
        .query::<(&Creature, &crate::components::Generation, &Skills)>();
    let (_, _, skills) = q
        .iter(&game.world)
        .find(|(c, g, _)| c.species == "botnet" && g.0 == 1)
        .expect("the child should have hatched");
    assert_eq!(*skills, seed_of(&game, "botnet"));
}

#[test]
fn level_and_practice_survive_a_real_save_and_load() {
    let dir = scratch_assets_dir("skills_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = new_game(31004);
    let worm = tame(&mut game, "worm");
    let id = game.world.get::<ProgramId>(worm).unwrap().0;
    {
        let mut skills = game.world.get_mut::<Skills>(worm).unwrap();
        for (i, s) in Skill::ALL.into_iter().enumerate() {
            skills.set(s, 3 + i as u32, 10 + i as u32);
        }
    }
    let expected = game.world.get::<Skills>(worm).unwrap().clone();
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(skills_of(&mut loaded, id), Some(expected));
}

#[test]
fn a_save_without_a_skills_field_loads_the_species_seed() {
    let dir = scratch_assets_dir("skills_legacy_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = new_game(31005);
    let worm = tame(&mut game, "worm");
    let id = game.world.get::<ProgramId>(worm).unwrap().0;
    game.world
        .get_mut::<Skills>(worm)
        .unwrap()
        .track_mut(Skill::Dig)
        .level = 15;
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.creatures
        .iter_mut()
        .find(|c| c.program_id == id)
        .unwrap()
        .skills = Vec::new();
    save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(skills_of(&mut loaded, id), Some(seed_of(&loaded, "worm")));
}

#[test]
fn an_unknown_skill_name_is_dropped_and_the_load_succeeds() {
    let dir = scratch_assets_dir("skills_unknown_name");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = new_game(31006);
    let worm = tame(&mut game, "worm");
    let id = game.world.get::<ProgramId>(worm).unwrap().0;
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let row = data
        .creatures
        .iter_mut()
        .find(|c| c.program_id == id)
        .unwrap();
    row.skills.retain(|s| s.skill != "farm");
    row.skills.push(save::SkillSave {
        skill: "juggling".to_string(),
        level: 9,
        practice: 1,
    });
    save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let skills = skills_of(&mut loaded, id).expect("the program should still load");
    assert_eq!(skills.level(Skill::Dig), 4);
    assert_eq!(skills.level(Skill::Farm), 2, "the dropped row is the seed");
}

#[test]
fn a_save_listing_only_work_skills_loads_combat_at_the_species_seed() {
    let dir = scratch_assets_dir("skills_0_21_16_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let assets = test_assets_dir();
    let text = std::fs::read_to_string(assets.join("species/worm.ron")).unwrap();
    assert!(text.contains("(Force, 3)"), "the shipped worm seeds Force");
    let path = dir.join("save.bin");

    let mut game = Game::new(31007, DifficultyMode::Forgiving, &assets).unwrap();
    let worm = tame(&mut game, "worm");
    let id = game.world.get::<ProgramId>(worm).unwrap().0;
    assert_eq!(
        game.world.get::<Skills>(worm).unwrap().level(Skill::Force),
        crate::tuning::SKILL_NEUTRAL_LEVEL + 3
    );
    game.world
        .get_mut::<Skills>(worm)
        .unwrap()
        .track_mut(Skill::Dig)
        .level = 9;
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.creatures
        .iter_mut()
        .find(|c| c.program_id == id)
        .unwrap()
        .skills
        .retain(|row| Skill::WORK.iter().any(|s| s.name() == row.skill));
    save::save_to_file(&path, &data).unwrap();

    let mut loaded = Game::load(&path, &assets).unwrap();
    let skills = skills_of(&mut loaded, id).unwrap();
    assert_eq!(skills.level(Skill::Dig), 9);
    assert_eq!(
        skills.level(Skill::Force),
        crate::tuning::SKILL_NEUTRAL_LEVEL + 3
    );
}
