//! The one place a contract's progress moves.
//!
//! Four of the five objectives are state-shaped, so this system polls them and
//! needs no call sites at all — the argument `achievement_system` makes about
//! being the one place that decides what has been earned. The fifth is
//! event-shaped: `Game::award_loot` *records* a kill into `RunFeats::kills`
//! and this system still decides what that advanced, so the kill site cannot
//! drift from the rules.
//!
//! `Objective::Deliver` is the deliberate exception and is not evaluated here
//! at all. Its progress is an act rather than a state — the player hands items
//! over at the Broker — and polling cargo for it would advance a contract the
//! player never took anything to.

use bevy_ecs::prelude::*;

use crate::components::Structure;
use crate::contracts::Objective;
use crate::resources::{ActiveContracts, Locale, RunFeats, ZoneLevel};

/// Raises `ActiveContract::progress` and nothing else. Completion is
/// `Game::settle_contracts`' — a payout writes to the player's inventory and
/// grants XP, which is `&mut Game` work rather than anything a system can
/// reach.
pub fn contract_system(
    mut held: ResMut<ActiveContracts>,
    mut feats: ResMut<RunFeats>,
    zone: Res<ZoneLevel>,
    // `Locale`, never `Position`: `Position` is pinned to the surface entrance
    // tile while the party is underground, so a depth taken from it would be a
    // surface coordinate.
    locale: Res<Locale>,
    structures: Query<&Structure>,
) {
    let depth = match *locale {
        Locale::Stack { depth, .. } => depth,
        Locale::Surface => 0,
    };

    for contract in &mut held.active {
        let target = contract.def.objective.target();
        let advance = match &contract.def.objective {
            Objective::Kill { species, count: _ } => feats
                .kills
                .iter()
                .filter(|killed| species.as_ref().is_none_or(|want| *want == **killed))
                .count() as u32,
            Objective::Descend { depth: want } => u32::from(depth >= *want),
            Objective::Breach { zone: want } => u32::from(zone.0 >= *want),
            Objective::Build { structure } => {
                u32::from(structures.iter().any(|s| s.kind == *structure))
            }
            // Not here — see the module doc.
            Objective::Deliver { .. } => 0,
        };
        contract.progress = contract.progress.saturating_add(advance).min(target);
    }

    // Unconditional, and this system is the field's only drainer: leaving a
    // kill in it would advance a contract accepted afterwards, forever.
    feats.kills.clear();
}
