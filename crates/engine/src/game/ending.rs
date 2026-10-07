//! The Basin Exit: the structure that ends the story, the confirmation the
//! step onto it raises, and the escape itself.
//!
//! Escaping sets `PhaseKeys::story_complete` and nothing else changes: the
//! keys stay, the run goes on, and `achievement_system` notices the flag on
//! the next tick. The prompt is engine state (`PendingBasinExit`) rather than
//! a return value because the step that raises it is `move_in_base`, deep
//! under `move_player`, which only returns whether a turn passed.

use crate::components::PhaseKeys;
use crate::phase_keys::PhaseKeyDb;
use crate::resources::PendingBasinExit;
use crate::story::EndingScreen;
use crate::*;

impl Game {
    /// Whether the Basin Exit may be built: every key held, zone
    /// `PHASE_KEY_COUNT` or deeper, and the story not already finished. One
    /// rule for the build menu, `place_structure` and the step onto it.
    pub(crate) fn basin_exit_gate(&self) -> Result<(), String> {
        let keys = self
            .world
            .get::<PhaseKeys>(self.player_entity())
            .copied()
            .unwrap_or_default();
        if keys.story_complete {
            return Err("You have already left the Basin.".into());
        }
        if keys.count() < crate::tuning::PHASE_KEY_COUNT {
            return Err(format!(
                "Needs all {} Phase Keys ({} recovered).",
                crate::tuning::PHASE_KEY_COUNT,
                keys.count()
            ));
        }
        if self.world.resource::<ZoneLevel>().0 < crate::tuning::PHASE_KEY_COUNT {
            return Err(format!(
                "The exit only opens from zone {} or deeper.",
                crate::tuning::PHASE_KEY_COUNT
            ));
        }
        Ok(())
    }

    /// Raised by the step onto a standing Basin Exit; the frontend asks the
    /// player and answers with `escape_basin`. Free: no tick passes.
    pub(crate) fn raise_basin_exit_prompt(&mut self) {
        self.world.insert_resource(PendingBasinExit);
    }

    /// Whether the player just stepped onto the exit, clearing the flag.
    pub fn take_basin_exit_prompt(&mut self) -> bool {
        self.world.remove_resource::<PendingBasinExit>().is_some()
    }

    /// Leaves the Basin: sets the story-complete flag, removes the exit and
    /// lets a tick pass, which is when the achievement lands. The keys are
    /// kept. Refused unless the gate is open and an exit is standing, so a
    /// frontend that skips the prompt still cannot escape without one.
    pub fn escape_basin(&mut self) -> Result<(), String> {
        self.basin_exit_gate()?;
        let exit = self
            .find_basin_exit()
            .ok_or_else(|| "There is no Basin Exit here.".to_string())?;
        let player = self.player_entity();
        if let Some(mut keys) = self.world.get_mut::<PhaseKeys>(player) {
            keys.story_complete = true;
        }
        self.world.despawn(exit);
        self.log_kind(
            MessageKind::Outcome,
            "You step through the exit and leave the Basin.",
        );
        self.tick();
        Ok(())
    }

    /// The ending screens, in order; never empty.
    pub fn ending_screens(&self) -> Vec<EndingScreen> {
        self.world
            .resource::<PhaseKeyDb>()
            .ending
            .screens()
            .to_vec()
    }

    fn find_basin_exit(&mut self) -> Option<Entity> {
        let mut query = self.world.query::<(Entity, &Structure)>();
        let db = self.world.resource::<StructureDb>();
        query
            .iter(&self.world)
            .find(|(_, s)| db.get(s.kind.as_str()).is_some_and(|d| d.basin_exit))
            .map(|(e, _)| e)
    }
}
