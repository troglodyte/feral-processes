//! The dev arena: authoring a battle scenario and playing it in the real
//! battle UI.
//!
//! Every screen in the family mutates one `ArenaSession` and they are
//! meaningless apart, so they share a module the way `app/trade.rs`'s do.
//!
//! What is being edited is a `feral_processes_engine::arena::Scenario` —
//! the same struct the `.ron` holds and the `arena` bin runs. There is no
//! parallel builder type, so a knob added to the schema cannot exist in one
//! tool and not the other.

use feral_processes_engine::arena::{RepRecord, Scenario, Watch};

use crate::*;

/// One visit to the arena: the scenario being edited, the fight it is
/// currently staging, and what that fight cost.
///
/// Held as `App::arena: Option<ArenaSession>`, and the presence of that
/// `Option` is what makes the session inert on disk — see `App::in_arena`.
pub(crate) struct ArenaSession {
    pub(crate) scenario: Scenario,
    /// The `.ron` it was loaded from or last saved to. `None` for a
    /// scenario that has only ever existed on this screen.
    pub(crate) path: Option<PathBuf>,
    /// The seed of the current or next fight. Separate from
    /// `scenario.seed`, which is where the *next visit* would start:
    /// reseeding on the result screen is the manual version of a rep, and
    /// must not rewrite the file the author is building.
    pub(crate) seed: u64,
    pub(crate) watch: Option<Watch>,
    pub(crate) outcome: Option<RepRecord>,
    /// What staging said the composition asks for past the zone's ceilings.
    /// Kept for the result screen — nothing is ever capped, and showing the
    /// ask is the only thing that makes that honest.
    pub(crate) warnings: Vec<String>,
}

impl ArenaSession {
    fn new() -> Self {
        let scenario = Scenario::default();
        Self {
            seed: scenario.seed,
            scenario,
            path: None,
            watch: None,
            outcome: None,
            warnings: Vec::new(),
        }
    }
}

/// How the game reaches the launcher's template library.
///
/// `dev_template` lives in the launcher crate — the `arena` bin lives there
/// for the same reason — and app-core cannot see it, so the launcher injects
/// what the builder needs. A plain `fn` rather than a boxed closure because
/// `dev_template::resolve` captures nothing. Absent, the `Template` player
/// source is simply not offered.
pub struct DevTemplates {
    pub names: Vec<String>,
    pub resolve: fn(&str) -> Result<PathBuf, String>,
}

/// Whether `FERAL_DEV_ARENA` is set — the switch that puts the arena row on
/// the main menu. Same predicate as the engine's `FERAL_DEV_REVEAL`
/// (`game/stack_view.rs`): present, non-empty and not `"0"`. Two answers to
/// "is a dev flag set" is exactly the drift this repo keeps catching.
///
/// Read once, in `App::new`, into a field — so a test can open the gate
/// without touching an environment the parallel suite shares.
pub(crate) fn dev_arena_enabled() -> bool {
    std::env::var_os("FERAL_DEV_ARENA").is_some_and(|v| !v.is_empty() && v != "0")
}

impl App {
    /// Installed by the launcher right after `App::new`, unconditionally:
    /// the gate decides visibility, and installing only when gated would
    /// make one flag mean two things.
    pub fn install_dev_templates(&mut self, templates: DevTemplates) {
        self.dev_templates = Some(templates);
    }

    /// Whether the main menu offers the arena at all.
    pub fn arena_enabled(&self) -> bool {
        self.arena_enabled
    }

    pub(crate) fn open_arena(&mut self) {
        self.status_line = None;
        self.arena = Some(ArenaSession::new());
        self.mode = Mode::ArenaBuilder;
    }

    /// Esc drops the session rather than parking it: a scenario outliving
    /// its screen would be the one the next visit silently fought.
    fn leave_arena(&mut self) {
        self.arena = None;
        self.status_line = None;
        self.mode = Mode::MainMenu;
    }

    pub(crate) fn handle_arena_builder_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.leave_arena();
        }
    }

    pub(crate) fn handle_arena_pick_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::ArenaBuilder;
        }
    }

    pub(crate) fn handle_arena_load_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::ArenaBuilder;
        }
    }

    pub(crate) fn handle_arena_save_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::ArenaBuilder;
        }
    }

    pub(crate) fn handle_arena_result_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::ArenaBuilder;
        }
    }
}
