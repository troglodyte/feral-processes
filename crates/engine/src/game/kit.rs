//! Where a body's kit comes from: the one derivation every reader of a kit
//! figure asks.
//!
//! A kit is what a body fights *with* — its basic attacks, its reach, its
//! affinities. Those used to be decided separately at each reader, each
//! asking "is this the player, who has no species?" on its own terms. Each
//! reader is now an exhaustive `match` on `Kit` instead, with no `_` arm, so
//! a new source of a kit is a new variant and every reader fails to compile
//! until it answers it. Emulation (todo #100) is the third source.
//!
//! What a kit is *not* is who the body is: `routine_slots`, the perks and
//! the talents stay keyed on player or companion, because they belong to the
//! body whatever it is fighting with. `attacks_for`, `combat_speed`/
//! `species_base_speed` (`game/combat.rs`) and `movement_allowance`
//! (`tactical/reach.rs`) join that list for the same reason (todo #100 Task
//! 3): swing count, initiative and battle-map footwork are the fighter's own
//! training and body, not the borrowed form — an emulating player keeps
//! their own class's extra swing and their own pace, exactly as they keep
//! their own HP.

use crate::abilities::AbilityId;
use crate::perks::emulation_fidelity_level;
use crate::progression::{EmulatedStats, emulated_stats};
use crate::*;

/// Where `Game::kit_of` found a body's kit.
pub(crate) enum Kit<'a> {
    /// No species to borrow from: the player, or a body whose species no
    /// longer resolves. The player's arms-length data strike is this arm.
    Unarmed,
    /// The body's own species, read through `SpeciesDb`.
    Innate(&'a SpeciesDef),
    /// The player is emulating `def`'s species — `components::Emulation`.
    /// `stats` is `progression::emulated_stats` at the player's own level
    /// and `Perk::EmulationFidelity`, computed once here so every reader
    /// this round reads the same figure.
    Emulated {
        def: &'a SpeciesDef,
        stats: EmulatedStats,
    },
}

/// Every ability `def` grants at or below `level`, filtered to what
/// `abilities` can actually resolve — the filter `install_innate_routines`
/// (a companion's kit) and `actor_abilities`'s `Kit::Emulated` arm (an
/// emulation's) both need, extracted so the two cannot drift apart.
pub(crate) fn innate_routine_ids(
    def: &SpeciesDef,
    level: u32,
    abilities: &AbilityDb,
) -> Vec<AbilityId> {
    def.abilities
        .iter()
        .filter(|a| a.level <= level)
        .map(|a| a.id.clone())
        .filter(|id| abilities.get(id).is_some())
        .collect()
}

impl Game {
    pub(crate) fn kit_of(&self, entity: Entity) -> Kit<'_> {
        if let Some(emulation) = self.world.get::<Emulation>(entity)
            && let Some(def) = self.world.resource::<SpeciesDb>().get(&emulation.species)
        {
            let level = self.ability_user_level(entity);
            let fidelity = emulation_fidelity_level(self.world.get::<Perks>(entity));
            let stats = emulated_stats(def, level, fidelity);
            return Kit::Emulated { def, stats };
        }
        self.world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map_or(Kit::Unarmed, Kit::Innate)
    }

    /// Removes `entity`'s emulation and logs `line` — the lapse's door and
    /// Revert's (todo #100 Task 4). No-op when nothing is emulating, so
    /// either caller can reach for it unconditionally without checking
    /// first; both real callers only ever fire on an actual drop, so this
    /// always speaks when it does anything.
    ///
    /// **Not** what `finish_fight` uses — that teardown drops the component
    /// silently, `Cloaked`'s own pattern, because a fight ending is not a
    /// deliberate change of form. See `Game::clear_battle_status_effects`.
    pub(crate) fn drop_emulation(&mut self, entity: Entity, line: &str) {
        if self.world.get::<Emulation>(entity).is_none() {
            return;
        }
        self.world.entity_mut(entity).remove::<Emulation>();
        self.log(line);
    }
}
