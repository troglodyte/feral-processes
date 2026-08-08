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

use feral_processes_engine::arena::{self, PlayerSource, RepRecord, Scenario, Watch};

use crate::*;

/// One visit to the arena: the scenario being edited, the fight it is
/// currently staging, and what that fight cost.
///
/// Held as `App::arena: Option<ArenaSession>`, and the presence of that
/// `Option` is what makes the session inert on disk — see `App::in_arena`.
pub(crate) struct ArenaSession {
    pub(crate) scenario: Scenario,
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

    /// Whether an arena session is live — the one predicate every "an arena
    /// fight must not do this" rule reads. A named predicate rather than
    /// three ad-hoc `arena.is_some()` checks, because what has to stay true
    /// is a list and only a name makes the list checkable.
    pub(crate) fn in_arena(&self) -> bool {
        self.arena.is_some()
    }

    /// Stages `session.scenario` at `session.seed` and opens `Mode::Battle`
    /// — the real battle UI, entire, which is how a companion Special ever
    /// fires in a measured fight. A staging error stays on the builder with
    /// the reason in the status line.
    pub(crate) fn start_arena_fight(&mut self) {
        let Some(session) = &self.arena else { return };
        let seed = session.seed;
        // A **clone**: a `Template` source is resolved to the save it
        // generates only for the fight. The session's own scenario keeps
        // saying `Template(name)`, or saving it back out would rewrite the
        // author's file into a path under `saves/`.
        let mut scenario = session.scenario.clone();
        if let PlayerSource::Template(name) = &scenario.player {
            let Some(templates) = &self.dev_templates else {
                self.status_line = Some(format!(
                    "template `{name}` needs the launcher's template library"
                ));
                return;
            };
            match (templates.resolve)(name) {
                Ok(path) => scenario.player = PlayerSource::Save(path),
                Err(e) => {
                    self.status_line = Some(e);
                    return;
                }
            }
        }

        match arena::stage(&scenario, &self.assets_dir, seed) {
            Ok(staged) => {
                if let Some(session) = &mut self.arena {
                    session.warnings = staged.warnings;
                    session.watch = Some(staged.watch);
                    session.outcome = None;
                }
                // Shown, never applied: nothing about the composition is
                // capped, and showing the ask is what makes that honest.
                self.status_line = self
                    .arena
                    .as_ref()
                    .filter(|s| !s.warnings.is_empty())
                    .map(|s| format!("warning: {}", s.warnings.join(" · ")));
                self.game = Some(staged.game);
                self.restart_reveal();
                self.mode = Mode::Battle;
            }
            Err(e) => self.status_line = Some(e),
        }
    }

    /// Feeds the round just resolved to the session's `Watch`. Called from
    /// `settle_after_round`, so every action that can end a battle reports
    /// through one hook rather than each remembering to.
    pub(crate) fn observe_arena_round(&mut self) {
        let App { arena, game, .. } = self;
        if let (Some(session), Some(game)) = (arena.as_mut(), game.as_ref())
            && let Some(watch) = &mut session.watch
        {
            watch.observe(game);
        }
    }

    /// Closes the fight and opens the result screen. Reached both from
    /// `settle_after_round` and from `check_game_over`, since a Permadeath
    /// player brought in on a `Save` source can end the run mid-round.
    pub(crate) fn finish_arena_fight(&mut self) {
        let App { arena, game, .. } = self;
        if let (Some(session), Some(game)) = (arena.as_mut(), game.as_ref())
            && let Some(watch) = &session.watch
        {
            session.outcome = Some(watch.finish(game));
        }
        // The result screen is not a log pane, and its transcript is
        // scrolled rather than paced — so release the narration rather than
        // spending the player's first key on skipping it.
        self.finish_reveal();
        self.mode = Mode::ArenaResult;
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
        match key {
            GameKey::Esc => self.leave_arena(),
            // Its own key rather than Enter, which the rows need for
            // "edit this one" — one key that meant both editing and
            // fighting would be ambiguous on every row.
            GameKey::Char('f') => self.start_arena_fight(),
            _ => {}
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

    /// What the fight cost, and the two ways to run it again.
    ///
    /// `[N]` is the manual version of `reps`: `arena::run` runs rep *n* at
    /// `scenario.seed + n`, so the same increment here is what lets a loss
    /// found by hand be replayed by the headless bin. It moves the
    /// session's seed and never `scenario.seed`, which is where the *next*
    /// visit starts and what a save writes out.
    ///
    /// Both refights go back through `start_arena_fight`, so there is one
    /// staging path and a refight starts from a whole party rather than
    /// from this fight's corpses.
    pub(crate) fn handle_arena_result_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.mode = Mode::ArenaBuilder;
                return;
            }
            GameKey::Char('r') => {
                self.start_arena_fight();
                return;
            }
            GameKey::Char('n') => {
                if let Some(session) = &mut self.arena {
                    session.seed = session.seed.wrapping_add(1);
                }
                self.start_arena_fight();
                return;
            }
            _ => {}
        }
        // Everything else scrolls the transcript, which is the screen.
        let rows = self.arena_transcript().len();
        self.scroll(key, rows);
    }

    /// The round-by-round narration the `Watch` collected, empty until a
    /// fight has finished. The one source of both the row count this
    /// scrolls against and the rows gui draws.
    pub fn arena_transcript(&self) -> &[String] {
        self.arena
            .as_ref()
            .and_then(|s| s.outcome.as_ref())
            .map(|r| r.transcript.as_slice())
            .unwrap_or_default()
    }
}
