//! What the `Sulking` rung does: the predicate every behaviour reads, and the
//! freeze-out half of it.

use super::respite::sulk;
use super::support::*;
use crate::components::{
    Disgruntled, Grievance, MemorySubject, Position, ProgramId, Task, TaskKind,
};
use crate::*;

#[test]
fn sulks_is_true_from_the_mild_rung_up_and_false_below_it() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let who = spawn_tamed(&mut game, 10, 3);
    assert!(!game.sulks(who), "no marker, no sulk");

    sulk(&mut game, who);
    assert!(game.sulks(who));
    for grievance in [Grievance::DownedTools, Grievance::LashingOut] {
        game.world.get_mut::<Disgruntled>(who).unwrap().grievance = grievance;
        assert!(game.sulks(who), "{grievance:?} still sulks");
    }
}

#[test]
fn is_beside_is_chebyshev_one() {
    use crate::situations::is_beside;
    let at = |x, y| Position { x, y };
    assert!(is_beside(at(0, 0), at(1, 1)));
    assert!(is_beside(at(0, 0), at(0, 0)));
    assert!(!is_beside(at(0, 0), at(2, 0)));
    assert!(!is_beside(at(0, 0), at(1, -2)));
}

/// A post, a sulker and a body it avoids standing beside the post.
fn a_rival_beside_a_post(game: &mut Game) -> (Entity, Entity, Entity) {
    let post = spawn_structure_at(game, "mining_node", 5, 5);
    let sulker = spawn_tamed(game, 10, 3);
    let rival = spawn_tamed(game, 10, 3);
    game.world.get_mut::<Position>(sulker).unwrap().x = -4;
    let mut at = game.world.get_mut::<Position>(rival).unwrap();
    (at.x, at.y) = (6, 5);
    let about = *game.world.get::<ProgramId>(rival).unwrap();
    game.remember(sulker, "turned_on_me", MemorySubject::Program(about));
    assert!(
        game.bond(sulker, about).avoids(),
        "the fixture needs a rival"
    );
    (post, sulker, rival)
}

fn mark_sulking(game: &mut Game, who: Entity) {
    game.world.entity_mut(who).insert(Disgruntled {
        grievance: Grievance::Sulking,
        stranded: false,
        told: false,
    });
}

#[test]
fn a_sulker_refuses_a_post_with_a_rival_beside_it_until_the_rival_moves() {
    let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (post, sulker, rival) = a_rival_beside_a_post(&mut game);
    mark_sulking(&mut game, sulker);
    assert!(game.refuses_post(sulker, post, TaskKind::GatherResource));

    game.world.get_mut::<Position>(rival).unwrap().x = 9;
    assert!(!game.refuses_post(sulker, post, TaskKind::GatherResource));
}

#[test]
fn a_content_program_takes_a_post_with_a_rival_beside_it() {
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (post, sulker, _) = a_rival_beside_a_post(&mut game);
    assert!(!game.refuses_post(sulker, post, TaskKind::GatherResource));
}

#[test]
fn an_excavate_want_is_never_refused_for_a_rival() {
    let mut game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (post, sulker, _) = a_rival_beside_a_post(&mut game);
    mark_sulking(&mut game, sulker);
    assert!(!game.refuses_post(sulker, post, TaskKind::Excavate));
}

/// A rival whose body is not walking the base — a guard at its station —
/// is not "beside": `situations::assess`'s neighbour filter, so the thought
/// and the refusal cannot disagree.
#[test]
fn a_rival_who_is_not_walking_the_base_does_not_cause_refusal() {
    let mut game = Game::new(85, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (post, sulker, rival) = a_rival_beside_a_post(&mut game);
    mark_sulking(&mut game, sulker);
    assert!(
        game.refuses_post(sulker, post, TaskKind::GatherResource),
        "the control"
    );

    game.world.entity_mut(rival).insert(Task {
        kind: TaskKind::Guard,
        target: post,
        progress: 0,
        required: 1,
    });
    assert!(!game.refuses_post(sulker, post, TaskKind::GatherResource));
}

/// RF5: an avoided program that is not base staff is not "beside" either.
#[test]
fn a_rival_off_the_staff_does_not_cause_refusal() {
    let mut game = Game::new(86, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (post, sulker, rival) = a_rival_beside_a_post(&mut game);
    mark_sulking(&mut game, sulker);
    assert!(
        game.refuses_post(sulker, post, TaskKind::GatherResource),
        "the control"
    );

    game.world
        .entity_mut(rival)
        .insert(crate::components::PostedAt((0, 0)));
    assert_ne!(
        game.program_role(rival),
        Some(crate::game::party::ProgramRole::Staff),
        "fixture: the rival is off the staff"
    );
    assert!(!game.refuses_post(sulker, post, TaskKind::GatherResource));
}
