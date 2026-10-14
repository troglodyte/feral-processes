//! What mends the party in base: a rest buys Integrity with Patch Routines,
//! and a free trickle runs underneath it so nobody is ever stuck.
//!
//! Who gets mended is one set, `Game::rest_bodies` — the player plus the
//! programs standing with them — shared by the rest, the trickle and the
//! field rest's full repair so they cannot disagree.

use crate::base_ledger::ConsumeSource;
use crate::components::{Creature, Stats, Tamed};
use crate::game::base::stock::spend_from_base;
use crate::game::base::work_orders::base_holding;
use crate::tuning::{BASE_TRICKLE_PER_TICK, REST_INTEGRITY_PER_PATCH};
use crate::*;

/// What a paid rest did: the Integrity the set was missing and how much of
/// it the patches covered.
#[derive(Default)]
pub(crate) struct Mend {
    missing: i32,
    healed: i32,
}

impl Mend {
    /// Whether some Integrity went unbought.
    pub(crate) fn is_partial(&self) -> bool {
        self.healed < self.missing
    }
}

/// Whether a program in `role` is repaired by a rest.
///
/// Exhaustive rather than a `!=`, `cell_mark`'s rule: the question is whether
/// this program is standing *with* the player, and there is no safe side to
/// default a new role to. `UnderStudy`, `Jailed`, `Siphoned`, `Sortie`,
/// `Outpost` and `Staff` are all not beside you — a pinned program or an away
/// crew member is no more with the party than base staff, whom a Repair Bay
/// serves instead. `None` is unreachable (the caller has filtered to programs
/// the player owns) and is spelled out so it cannot become the arm a new role
/// quietly lands in.
fn rests_with_player(role: Option<ProgramRole>) -> bool {
    match role {
        Some(ProgramRole::InParty | ProgramRole::Wielded) => true,
        Some(
            ProgramRole::Sortie
            | ProgramRole::Outpost
            | ProgramRole::Staff
            | ProgramRole::UnderStudy
            | ProgramRole::Jailed
            | ProgramRole::Siphoned,
        ) => false,
        None => false,
    }
}

impl Game {
    /// The player and every owned program a rest repairs, in entity order
    /// after the player so a spend walks them the same way every run.
    pub(crate) fn rest_bodies(&self) -> Vec<Entity> {
        let player = self.player_entity();
        let mut programs: Vec<Entity> = self
            .world
            .iter_entities()
            .filter(|e| e.contains::<Creature>())
            .filter(|e| e.get::<Tamed>().is_some_and(|t| t.owner == player))
            .map(|e| e.id())
            .filter(|&e| rests_with_player(self.program_role(e)))
            .collect();
        programs.sort();
        std::iter::once(player).chain(programs).collect()
    }

    /// Buys back as much of `bodies`' missing Integrity as the base's patches
    /// cover: `ceil(missing / REST_INTEGRITY_PER_PATCH)` of them, capped by
    /// stock. Short of patches the worst-hurt by fraction mend first, ties by
    /// entity; a partial last patch heals in full, which is the rounding in
    /// the party's favour.
    ///
    /// Writes `Stats::hp` directly, as the rest always has, rather than going
    /// through `restore_hp`: a rest is not a heal that a `HealBlock` status
    /// should stop.
    pub(crate) fn mend_with_patches(&mut self, bodies: &[Entity]) -> Mend {
        let mut hurt: Vec<(Entity, i64, i64)> = bodies
            .iter()
            .filter_map(|&e| {
                let stats = self.world.get::<Stats>(e)?;
                (stats.hp < stats.max_hp).then_some((e, stats.hp as i64, stats.max_hp as i64))
            })
            .collect();
        let missing: i32 = hurt.iter().map(|&(_, hp, max)| (max - hp) as i32).sum();
        if missing == 0 {
            return Mend::default();
        }
        // Lowest hp/max first, compared by cross-multiplication so no float
        // decides a tie.
        hurt.sort_by(|a, b| (a.1 * b.2).cmp(&(b.1 * a.2)).then(a.0.cmp(&b.0)));

        let patch = ItemId::from(ids::PATCH_ROUTINE);
        let needed = (missing as u32).div_ceil(REST_INTEGRITY_PER_PATCH as u32);
        let spend = needed.min(base_holding(self, &patch));
        let spent = spend_from_base(self, &patch, spend, ConsumeSource::Heal);
        let mut budget = if spent == needed {
            missing
        } else {
            spent as i32 * REST_INTEGRITY_PER_PATCH
        };

        let mut healed = 0;
        for (entity, hp, max) in hurt {
            let give = ((max - hp) as i32).min(budget);
            if give == 0 {
                continue;
            }
            if let Some(mut stats) = self.world.get_mut::<Stats>(entity) {
                stats.hp += give;
            }
            budget -= give;
            healed += give;
        }
        Mend { missing, healed }
    }

    /// One tick of the free floor under paid rest: while the party stands in
    /// base space and nothing is fighting, everyone a rest would repair
    /// regains `BASE_TRICKLE_PER_TICK` Integrity. Costs nothing.
    ///
    /// A `Game` method beside `run_repair_bays` rather than a bevy system,
    /// because `rest_bodies` needs `program_role`, which needs `&Game`.
    pub(crate) fn run_base_trickle(&mut self) {
        if self.is_game_over().is_some() || !self.in_base() || self.has_active_battle() {
            return;
        }
        for body in self.rest_bodies() {
            // **Never the dead.** A body at zero is awaiting
            // `death_handling_system`, and a trickle that ran first would
            // lift it off the floor and cancel the death.
            if self.world.get::<Stats>(body).is_some_and(|s| s.hp > 0) {
                self.restore_hp(body, BASE_TRICKLE_PER_TICK);
            }
        }
    }
}
