//! Being under study — the Research Station's pen.
//!
//! The home for `Game::study_pen` because the pin and unpin doors that join
//! it here in a later task read and write the same corner, and
//! `building.rs` is already past 1,200 lines owning a different
//! responsibility: placement and demolition, not what a structure's
//! footprint is *for*.

use crate::*;

impl Game {
    /// The footprint cell diagonally opposite `structure`'s anchor — its
    /// pen — or `None` when the structure's def does not declare `studies`.
    ///
    /// **The one door.** The pin, the walk, the block check, the draw and
    /// the consumption all call this rather than each re-deriving a corner
    /// from a footprint and a side.
    ///
    /// `#[allow(dead_code)]` rather than `#[cfg(test)]`: unlike a helper
    /// with no production plan, this one has a scheduled production caller
    /// — `Game::pin_subject` — landing in the very next task of this same
    /// plan. `#[cfg(test)]` would state a falsehood about where this is
    /// going; the allow states the truth, that the wiring is not here yet.
    #[allow(dead_code)]
    pub(crate) fn study_pen(&self, structure: Entity) -> Option<(i32, i32)> {
        let kind = self.world.get::<Structure>(structure)?.kind.clone();
        let def = self.world.resource::<StructureDb>().get(&kind)?;
        if !def.studies {
            return None;
        }
        let pos = self.world.get::<Position>(structure)?;
        let side = i32::from(def.footprint);
        Some((pos.x + side - 1, pos.y + side - 1))
    }
}
