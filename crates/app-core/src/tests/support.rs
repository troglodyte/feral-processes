//! Fixtures shared by the app-core tests.

use crate::*;

pub(crate) fn test_app(seed: u32) -> App {
    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let saves_dir = std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}_saves"));
    let history_path =
        std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}.log"));
    let mut app = App::new(assets_dir.clone(), saves_dir, history_path);
    app.game = Game::new(seed, DifficultyMode::Forgiving, &assets_dir).ok();
    app.mode = Mode::Playing;
    app
}
