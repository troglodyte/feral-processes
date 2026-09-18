//! Where a body's kit comes from: the one derivation every reader of a kit
//! figure asks.
//!
//! A kit is what a body fights *with* — its basic attacks, its reach, its
//! affinities. Those used to be decided separately at each reader, each
//! asking "is this the player, who has no species?" on its own terms. Each
//! reader is now an exhaustive `match` on `Kit` instead, with no `_` arm, so
//! a new source of a kit is a new variant and every reader fails to compile
//! until it answers it. Emulation (todo #100) is the next source.
//!
//! What a kit is *not* is who the body is: `routine_slots`, the perks and
//! the talents stay keyed on player or companion, because they belong to the
//! body whatever it is fighting with.

use crate::*;

/// Where `Game::kit_of` found a body's kit.
pub(crate) enum Kit<'a> {
    /// No species to borrow from: the player, or a body whose species no
    /// longer resolves. The player's arms-length data strike is this arm.
    Unarmed,
    /// The body's own species, read through `SpeciesDb`.
    Innate(&'a SpeciesDef),
}

impl Game {
    pub(crate) fn kit_of(&self, entity: Entity) -> Kit<'_> {
        self.world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map_or(Kit::Unarmed, Kit::Innate)
    }
}
