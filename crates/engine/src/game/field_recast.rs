//! Auto-recast: a holder's flag on a timed field routine, and the rerun of
//! that routine when the buff it armed runs out.

use crate::components::AutoRecast;
use crate::*;

impl Game {
    /// The routines `entity` has flagged to rerun, in flag order. Empty when
    /// the holder carries no `AutoRecast`.
    pub(crate) fn auto_recast_of(&self, entity: Entity) -> Vec<crate::abilities::AbilityId> {
        self.world
            .get::<AutoRecast>(entity)
            .map(|a| a.0.clone())
            .unwrap_or_default()
    }

    /// Flags or unflags `ability` on `entity`. The component is created on
    /// the first flag and kept when emptied: absent and empty read the same.
    pub(crate) fn set_auto_recast(&mut self, entity: Entity, ability: &str, on: bool) {
        let mut flags = self.auto_recast_of(entity);
        flags.retain(|id| id != ability);
        if on {
            flags.push(ability.to_string());
        }
        self.world.entity_mut(entity).insert(AutoRecast(flags));
    }
}
