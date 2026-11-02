//! What a program's skills look like outside the sim: the views that carry
//! them to the renderer, and the log line announcing a level (ADR 0038).

use super::support::*;
use crate::resources::PendingSkillLevels;
use crate::skills::{Skill, practice_to_next};
use crate::*;

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

#[test]
fn a_tamed_manifest_lists_every_skill_with_progress_and_a_wild_one_has_none() {
    let mut game = game(41101);
    let tamed = spawn_tamed(&mut game, 10, 3);
    set_skill_level(&mut game, tamed, Skill::Build, 3);
    let skills = program_manifest(&game, tamed).skills.expect("tamed");
    assert_eq!(skills.len(), Skill::WORK.len());
    let build = skills.iter().find(|s| s.skill_label == "Build").unwrap();
    assert_eq!(build.level, 3);
    assert_eq!(build.practice_to_next, practice_to_next(3));

    let wild = spawn_wild_on_player_tile(&mut game);
    assert!(program_manifest(&game, wild).skills.is_none());
}

#[test]
fn a_staff_row_carries_its_programs_skill_levels() {
    let mut game = game(41102);
    let tamed = spawn_tamed(&mut game, 10, 3);
    set_skill_level(&mut game, tamed, Skill::Dig, 4);
    let table = game.work_table();
    let row = table
        .rows
        .iter()
        .find(|r| r.program.entity == tamed)
        .expect("a row for the program");
    assert_eq!(
        row.skills
            .iter()
            .find(|s| s.skill_label == "Dig")
            .unwrap()
            .level,
        4
    );
}

#[test]
fn a_level_gained_is_announced_once_and_the_queue_is_drained() {
    let mut game = game(41103);
    let tamed = spawn_tamed(&mut game, 10, 3);
    game.world
        .resource_mut::<PendingSkillLevels>()
        .0
        .push((tamed, Skill::Build, 4));
    game.tick();
    let name = game.creature_name(tamed).unwrap();
    let lines = game.message_history(400);
    let hits: Vec<_> = lines
        .iter()
        .filter(|m| m.text == format!("{name} is now a level 4 builder."))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "{:?}",
        lines.iter().map(|m| &m.text).collect::<Vec<_>>()
    );
    assert!(game.world.resource::<PendingSkillLevels>().0.is_empty());
}

/// `practise` is fed one point a tick, so an entry is one level; if a gain
/// ever covers several levels the site queues one entry per level and each is
/// announced, so the log never skips a number.
#[test]
fn each_queued_level_gets_its_own_line() {
    let mut game = game(41104);
    let tamed = spawn_tamed(&mut game, 10, 3);
    {
        let mut q = game.world.resource_mut::<PendingSkillLevels>();
        q.0.push((tamed, Skill::Farm, 2));
        q.0.push((tamed, Skill::Farm, 3));
    }
    game.tick();
    let name = game.creature_name(tamed).unwrap();
    let lines = game.message_history(400);
    for n in [2, 3] {
        let want = format!("{name} is now a level {n} farmer.");
        assert!(lines.iter().any(|m| m.text == want), "missing {want}");
    }
}

/// A program carries both families and a manifest splits them, so the box
/// that lists work never lists combat and the other way about.
#[test]
fn a_tamed_manifest_carries_the_combat_skills_in_their_own_list() {
    let mut game = game(41105);
    let tamed = spawn_tamed(&mut game, 10, 3);
    set_skill_level(&mut game, tamed, Skill::Force, 6);
    let manifest = program_manifest(&game, tamed);
    let combat = manifest.combat_skills.expect("tamed");
    let labels: Vec<_> = combat.iter().map(|s| s.skill_label).collect();
    assert_eq!(labels, ["Aim", "Force", "Defence", "Routines"]);
    assert_eq!(combat[1].level, 6);
    assert!(
        manifest
            .skills
            .unwrap()
            .iter()
            .all(|s| !labels.contains(&s.skill_label))
    );

    let wild = spawn_wild_on_player_tile(&mut game);
    assert!(program_manifest(&game, wild).combat_skills.is_none());
}

/// The player's own page lists the four combat skills and no work skill.
#[test]
fn the_players_manifest_lists_the_four_combat_skills() {
    let mut game = game(41106);
    let player = game.player_entity();
    set_skill_level(&mut game, player, Skill::Defence, 5);
    let view = game.manifest(player).expect("the player");
    let ManifestSubject::Player(p) = view.subject else {
        panic!("the player's own subject");
    };
    let labels: Vec<_> = p.skills.iter().map(|s| s.skill_label).collect();
    assert_eq!(labels, ["Aim", "Force", "Defence", "Routines"]);
    assert_eq!(p.skills[2].level, 5);
}

/// The player is "You", and each combat skill has one plain noun.
#[test]
fn a_combat_level_names_a_plain_role_and_the_player_is_you() {
    let mut game = game(41107);
    let player = game.player_entity();
    let tamed = spawn_tamed(&mut game, 10, 3);
    {
        let mut q = game.world.resource_mut::<PendingSkillLevels>();
        q.0.push((player, Skill::Aim, 4));
        q.0.push((tamed, Skill::Defence, 2));
        q.0.push((tamed, Skill::Force, 3));
        q.0.push((tamed, Skill::Routines, 5));
    }
    game.tick();
    let name = game.creature_name(tamed).unwrap();
    let lines: Vec<String> = game
        .message_history(400)
        .iter()
        .map(|m| m.text.clone())
        .collect();
    for want in [
        "You are now a level 4 marksman.".to_string(),
        format!("{name} is now a level 2 defender."),
        format!("{name} is now a level 3 bruiser."),
        format!("{name} is now a level 5 scripter."),
    ] {
        assert!(lines.contains(&want), "missing {want}: {lines:?}");
    }
}
