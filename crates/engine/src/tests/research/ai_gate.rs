use super::*;
use crate::storyline::{BaseAi, FusedAi, TrainingTarget};

const GATE: &str = "Seems feasible, but we don't know where to start";

/// A shipped node with its prerequisites researched, gated on `level`.
fn gated_on_ai_level(game: &mut Game, level: u32) -> String {
    let mut def = cheapest_gated_node(game, 0);
    research_prereqs_of(game, &def.id);
    def.requires_ai_level = Some(level);
    let id = def.id.clone();
    game.world.resource_mut::<ResearchDb>().insert_for_test(def);
    id
}

fn fuse_at_level(game: &mut Game, level: u32) {
    game.storyline.base_ai = BaseAi::Fused(FusedAi {
        host: crate::components::ProgramId(1),
        station: (0, 0),
        level,
        analysis_at_fusion: 20,
        pace_at_fusion: 1.0,
        practice: 0,
        carry: 0.0,
        training: TrainingTarget::default(),
    });
}

#[test]
fn a_node_gated_on_ai_level_is_locked_with_the_gated_reason_before_and_after_fusion() {
    let mut game = Game::new(730, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    discover_all_research(&mut game);
    let id = gated_on_ai_level(&mut game, 1);
    let locked = ResearchState::Locked {
        missing: Vec::new(),
        min_zone: None,
        ai_level: Some(1),
    };
    assert_eq!(research_state(&game, &id), locked, "before the fusion");
    fuse_at_level(&mut game, 0);
    assert_eq!(research_state(&game, &id), locked, "fused at level 0");
}

#[test]
fn it_opens_when_the_ai_reaches_the_level() {
    let mut game = Game::new(731, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    discover_all_research(&mut game);
    let id = gated_on_ai_level(&mut game, 1);
    fuse_at_level(&mut game, 1);
    assert_eq!(research_state(&game, &id), ResearchState::Available);
}

#[test]
fn selecting_a_gated_node_is_refused() {
    let mut game = Game::new(732, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    discover_all_research(&mut game);
    let id = gated_on_ai_level(&mut game, 1);
    base_with_a_research_node(&mut game);
    let err = game.select_research(&id).unwrap_err();
    assert!(err.contains(GATE), "{err}");
    assert_eq!(active_research(&game), None, "and nothing was taken on");
    fuse_at_level(&mut game, 1);
    game.select_research(&id).unwrap();
}

#[test]
fn the_study_station_is_the_ais_when_it_is_not_the_lowest_anchor() {
    let mut game = Game::new(733, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ai_node = base_with_a_research_node(&mut game);
    let lower = spawn_machine_at(&mut game, "research_node", -6, -6);
    let (x, y) = {
        let p = game.world.get::<Position>(ai_node).unwrap();
        (p.x, p.y)
    };
    assert_eq!(
        game.study_station(),
        Some(lower),
        "the lowest before fusion"
    );
    game.storyline.base_ai = BaseAi::Fused(FusedAi {
        host: crate::components::ProgramId(1),
        station: (x, y),
        level: 0,
        analysis_at_fusion: 20,
        pace_at_fusion: 1.0,
        practice: 0,
        carry: 0.0,
        training: TrainingTarget::default(),
    });
    assert_eq!(game.study_station(), Some(ai_node));
}

#[test]
fn the_player_cannot_work_a_research_station_once_the_ai_has_it() {
    let mut game = Game::new(734, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    stand_player_at_post(&mut game, node);
    game.storyline.base_ai = BaseAi::Claiming {
        host: crate::components::ProgramId(1),
        station: (2, 2),
    };
    let err = game.work_structure(node).unwrap_err();
    assert!(err.contains("AI"), "{err}");
    assert!(
        game.world
            .get::<crate::components::Task>(game.player_entity())
            .is_none()
    );
}
