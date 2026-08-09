//! The enemy battle policy: loading the weight asset, and the behaviour the
//! loaded weights buy.
//!
//! The arithmetic itself is tested in `policy.rs`'s own `mod tests` — this
//! module is about the parts that need a `Game`.

use super::support::*;
use crate::policy;
use crate::resources::EnemyPolicy;
use crate::*;

#[test]
fn an_absent_policy_file_loads_as_none_without_warning() {
    let dir = scratch_assets_dir("policy_absent");
    let (weights, warnings) = policy::load_file(&dir.join("enemy_battle.ron")).unwrap();
    assert!(weights.is_none());
    assert!(warnings.is_empty(), "an absent file is a valid state");
}

#[test]
fn a_malformed_policy_file_is_skipped_with_a_warning() {
    let dir = scratch_assets_dir("policy_malformed");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("enemy_battle.ron");
    std::fs::write(&path, "(features: [this is not ron").unwrap();

    let (weights, warnings) = policy::load_file(&path).unwrap();
    assert!(weights.is_none());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

#[test]
fn a_game_starts_with_no_policy_file() {
    let game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.world.resource::<EnemyPolicy>().0.is_none());
}
