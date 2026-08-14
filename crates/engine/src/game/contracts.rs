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

use crate::Game;
use crate::components::Structure;
use crate::contracts::{Objective, Reward};
use crate::items::{ItemId, ids};
use crate::resources::{ActiveContracts, Locale, MessageKind, RunFeats, ZoneLevel};

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

impl Game {
    /// Finishes every held contract that has reached its target.
    ///
    /// Separate from `contract_system` because a payout writes the player's
    /// inventory and grants XP, which is `&mut Game` work no bevy system can
    /// reach — so the split is the same one `tick_inner` already makes for
    /// `structure_regen` and `raid_check`. The system raises the number; this
    /// reads it and settles.
    pub(crate) fn settle_contracts(&mut self) {
        loop {
            let finished = self
                .world
                .resource::<ActiveContracts>()
                .active
                .iter()
                .position(|c| c.progress >= c.def.objective.target());
            match finished {
                Some(idx) => self.complete_contract(idx),
                None => return,
            }
        }
    }

    /// The single door out of an active contract: announces it, files the id
    /// under `ActiveContracts::done`, drops the `ActiveContract`, and grants
    /// every `Reward`.
    ///
    /// Dropping it *before* paying is what makes double payment
    /// unexpressible: a reward that itself ticked the game could not find the
    /// contract to settle a second time.
    pub(crate) fn complete_contract(&mut self, idx: usize) {
        let contract = {
            let mut held = self.world.resource_mut::<ActiveContracts>();
            if idx >= held.active.len() {
                return;
            }
            let contract = held.active.remove(idx);
            held.done.push(contract.def.id.clone());
            contract
        };

        // `Outcome` rather than `Info`, for `achievement_system`'s reason: a
        // contract can finish mid-fight, and
        // `MessageLog::retain_outcomes_since_battle` deletes everything but
        // four kinds when the battle ends — so an `Info` line would vanish at
        // exactly the moment the player looked up from the fight.
        self.log_kind(
            MessageKind::Outcome,
            format!("CONTRACT COMPLETE: {}", contract.def.name),
        );

        for reward in &contract.def.reward {
            match *reward {
                Reward::Credits(n) => {
                    self.grant_loot(ItemId::from(ids::CREDITS), n);
                }
                // Plain copies, deliberately not through `grant_gear_drop` —
                // that is the one door a copy above `Ordinary` enters the game
                // by, and crafting and buying are already not callers. Found
                // gear is categorically better than made gear, and a contract
                // payout is closer to made.
                Reward::Item(ref item, n) => {
                    self.grant_loot(item.clone(), n);
                }
                // Through `award_player_xp` so a level-up full-heals exactly
                // as it does from a kill.
                Reward::Xp(n) => {
                    let player = self.player_entity();
                    self.award_player_xp(player, n);
                }
            }
        }
        let paid = contract
            .def
            .reward
            .iter()
            .map(reward_label)
            .collect::<Vec<_>>()
            .join(", ");
        self.log_kind(
            MessageKind::Loot,
            format!("Contract {} paid: {paid}", contract.def.id),
        );
    }
}

/// How a reward reads in the completion line. Engine-side for the reason
/// `views::ContractRow` composes its own wording: two screens must not word
/// one contract differently.
pub(crate) fn reward_label(reward: &Reward) -> String {
    match reward {
        Reward::Credits(n) => format!("{n} Credits"),
        Reward::Item(item, n) => format!("{n}x {item}"),
        Reward::Xp(n) => format!("{n} XP"),
    }
}

impl Game {
    /// Whether `entity` is a deployed Contract Broker. The one predicate for
    /// it, so the `EntityView` flag a frontend scans for and the board's own
    /// range check cannot disagree about what counts as a Broker.
    pub(crate) fn issues_contracts(&self, entity: Entity) -> bool {
        let Some(kind) = self.world.get::<Structure>(entity).map(|s| &s.kind) else {
            return false;
        };
        self.world
            .resource::<crate::structures::StructureDb>()
            .get(kind)
            .is_some_and(|def| def.issues_contracts)
    }
}
