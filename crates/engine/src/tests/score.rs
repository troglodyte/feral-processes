//! The run score's tally sites, banking and history line
//! (`docs/superpowers/specs/2026-10-09-run-score-design.md`).

use super::support::*;
use crate::resources::{DifficultyMode, RunTally};
use crate::*;

/// A real save and load, never RON alone —
/// `ron-round-trip-cannot-catch-a-skipped-field`.
#[test]
fn a_nonzero_run_tally_survives_a_save_and_load() {
    let dir = scratch_assets_dir("run_tally_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = Game::new(20261009, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tally = RunTally {
        foe_levels: 41,
        bosses: 2,
        compiled: 3,
        achievements: 4,
        deepest_depth: 5,
        banked: 600,
    };
    *game.world.resource_mut::<RunTally>() = tally.clone();
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(*loaded.world.resource::<RunTally>(), tally);
}
