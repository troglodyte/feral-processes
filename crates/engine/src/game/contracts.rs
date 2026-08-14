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

use rand::prelude::*;

use crate::Game;
use crate::components::Inventory;
use crate::components::{Position, Structure};
use crate::contracts::{ContractId, Objective, Reward};
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
            Objective::Terminate { species, count: _ } => feats
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
        let paid = self.reward_line(&contract.def.reward);
        self.log_kind(
            MessageKind::Loot,
            format!("{} paid {paid}.", contract.def.name),
        );
    }

    /// How a contract's whole payout reads. The completion line and both
    /// screens go through this rather than each wording a `Reward` itself —
    /// `views::ContractRow`'s argument, and the reason item ids are resolved
    /// to display names here and nowhere downstream.
    fn reward_line(&self, reward: &[Reward]) -> String {
        reward
            .iter()
            .map(|r| match r {
                Reward::Credits(n) => format!("{n} {}", self.item_name(&self.trade_currency())),
                Reward::Item(item, n) => format!("{n} {}", self.item_name(item)),
                Reward::Xp(n) => format!("{n} XP"),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

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

/// Salts the board's seed so what a sector is offering does not correlate
/// with anything else derived from the same world seed. Its own named
/// constant, per `FrameSpec::salted`'s rule — one scheme, not a second seed
/// source that could collide with the Stack's.
const CONTRACT_BOARD_SALT: u64 = 0xC0A7_7AC7_5EED_0001;

impl Game {
    /// What a Broker within `CONTRACT_BOARD_RANGE_TILES` is offering, or
    /// `None` if there is no Broker in reach.
    ///
    /// One call answers both "is there a board" and "what is on it", so no
    /// screen asks those separately and then disagrees — `Game::stack_market`'s
    /// contract.
    ///
    /// **Offers are derived, never stored.** A local `StdRng` seeded from the
    /// world seed, the sector and the epoch, exactly as `market_offers` is
    /// seeded off `FrameSpec` and for the same forced reason: the player is
    /// shown an offer before they accept it, so the answer has to survive a
    /// save and load, and `GameRng`'s stream position is not persisted. Four
    /// properties come free — the board survives a reload with no save field,
    /// reading it spends no `GameRng` draw and so shifts nobody's stream, it
    /// cannot be rerolled by save-scumming, and it rotates on its own.
    ///
    /// `None` underground, and that is not an oversight: the player's
    /// `Position` is pinned to the surface entrance tile the whole time they
    /// are down there, so a range check made from it would put the party at a
    /// Broker they are four frames below. `active_contracts` is what reads
    /// anywhere.
    pub fn contract_board(&mut self) -> Option<Vec<crate::views::ContractRow>> {
        if self.is_underground() || !self.broker_in_range() {
            return None;
        }
        let mut pool = self.offerable_contracts();
        let mut rng = StdRng::seed_from_u64(self.board_seed());
        let mut rows = Vec::new();
        for _ in 0..crate::tuning::CONTRACT_BOARD_SLOTS.min(pool.len()) {
            let id = pool.swap_remove(rng.random_range(0..pool.len()));
            if let Some(def) = self
                .world
                .resource::<crate::contracts::ContractDb>()
                .get(&id)
            {
                rows.push(self.contract_row(def, 0));
            }
        }
        Some(rows)
    }

    /// Every contract the run currently holds. Always available, board or not:
    /// what you have taken is readable anywhere, including four frames down.
    pub fn active_contracts(&self) -> Vec<crate::views::ContractRow> {
        self.world
            .resource::<ActiveContracts>()
            .active
            .iter()
            .map(|held| self.contract_row(&held.def, held.progress))
            .collect()
    }

    /// Every authored contract, worded, at zero progress — the whole
    /// catalogue rather than one board's three.
    ///
    /// Exists for the renderer's width census, which has to measure the
    /// widest row the shipped assets can *ever* build rather than whichever
    /// three a seed happened to roll. Engine-side for the reason every other
    /// wording is: the row a census measures has to be the row the screen
    /// draws, or it is measuring a copy.
    pub fn contract_catalogue(&self) -> Vec<crate::views::ContractRow> {
        self.world
            .resource::<crate::contracts::ContractDb>()
            .iter()
            .map(|def| self.contract_row(def, 0))
            .collect()
    }

    /// Which authored contracts this run could be offered right now, in the
    /// db's stable id order — the pool the board draws its slots from.
    ///
    /// Named rather than inlined so a test can ask what is offerable without
    /// depending on which three the roll happened to pick, and so the filter
    /// itself is stated once: this sector's level, minus anything already in
    /// hand, minus anything finished that does not repeat.
    pub(crate) fn offerable_contracts(&self) -> Vec<crate::contracts::ContractId> {
        let zone = self.world.resource::<ZoneLevel>().0;
        let held = self.world.resource::<ActiveContracts>();
        self.world
            .resource::<crate::contracts::ContractDb>()
            .iter()
            .filter(|def| def.min_zone <= zone)
            .filter(|def| !held.active.iter().any(|c| c.def.id == def.id))
            .filter(|def| def.repeatable || !held.done.contains(&def.id))
            .map(|def| def.id.clone())
            .collect()
    }

    /// Whether a Contract Broker is close enough to deal with.
    fn broker_in_range(&mut self) -> bool {
        let Some(here) = self.world.get::<Position>(self.player_entity()).copied() else {
            return false;
        };
        let reach = crate::tuning::CONTRACT_BOARD_RANGE_TILES;
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Structure>>();
        let near: Vec<Entity> = query
            .iter(&self.world)
            .filter(|(_, pos)| (pos.x - here.x).abs() <= reach && (pos.y - here.y).abs() <= reach)
            .map(|(entity, _)| entity)
            .collect();
        near.into_iter().any(|entity| self.issues_contracts(entity))
    }

    /// The board's seed: the world seed, the sector and the epoch, folded
    /// FNV-1a a byte at a time.
    ///
    /// Byte-at-a-time rather than one XOR-and-multiply per word, for
    /// `FrameSpec::salted`'s measured reason: a whole-word XOR leaves low
    /// output bits a fixed function of the input, and consecutive epochs
    /// differ in exactly one low bit.
    fn board_seed(&self) -> u64 {
        let epoch = self.current_tick() / crate::tuning::CONTRACT_REFRESH_CYCLES as u64;
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for word in [
            self.world.resource::<crate::world::WorldMap>().seed() as u64,
            self.world.resource::<ZoneLevel>().0 as u64,
            epoch,
            CONTRACT_BOARD_SALT,
        ] {
            for byte in word.to_le_bytes() {
                h ^= byte as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        h
    }

    /// One contract, worded for a screen.
    fn contract_row(
        &self,
        def: &crate::contracts::ContractDef,
        progress: u32,
    ) -> crate::views::ContractRow {
        crate::views::ContractRow {
            id: def.id.clone(),
            name: def.name.clone(),
            description: def.description.clone(),
            objective_line: self.objective_line(&def.objective),
            reward_line: self.reward_line(&def.reward),
            progress,
            target: def.objective.target(),
        }
    }

    /// What an objective asks, in the player's words. Item and species ids
    /// are resolved to their display names here rather than shown raw — the
    /// same reason `Game::copy_name` exists.
    fn objective_line(&self, objective: &Objective) -> String {
        match objective {
            Objective::Terminate {
                species: Some(id),
                count,
            } => {
                let name = self
                    .world
                    .resource::<crate::species::SpeciesDb>()
                    .get(id)
                    .map(|def| def.name.clone())
                    .unwrap_or_else(|| id.clone());
                format!("Terminate {count} {name}")
            }
            Objective::Terminate {
                species: None,
                count,
            } => format!("Terminate {count} wild programs"),
            Objective::Deliver { item, count } => {
                format!("Deliver {count} {}", self.item_name(item))
            }
            Objective::Descend { depth } => format!("Stand {depth} frames down a Stack"),
            Objective::Breach { zone } => format!("Reach sector {zone}"),
            Objective::Build { structure } => {
                let name = self
                    .world
                    .resource::<crate::structures::StructureDb>()
                    .get(structure)
                    .map(|def| def.name.clone())
                    .unwrap_or_else(|| structure.clone());
                format!("Build a {name}")
            }
        }
    }
}

/// Why a contract action was refused. Typed rather than a `String` because
/// each of these leaves the player a different errand, and app-core words
/// them for the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractRefusal {
    /// `MAX_ACTIVE_CONTRACTS` are already held. Refused rather than silently
    /// capped — the "no silent caps" rule.
    TooMany,
    AlreadyActive,
    /// Finished this run, and not `repeatable`.
    AlreadyDone,
    /// No Broker in reach, or this contract is not on that Broker's board —
    /// and, for a delivery, not one the run is holding.
    NotOffered,
    /// Nothing in cargo that the contract asks for.
    NothingToDeliver,
}

impl Game {
    /// Takes a contract off the board in front of the player.
    ///
    /// Every refusal is checked before anything is written, the ordering
    /// `use_symlink` and `install_routine` follow — a refused acceptance must
    /// leave the run exactly as it found it.
    pub fn accept_contract(&mut self, id: &ContractId) -> Result<(), ContractRefusal> {
        let Some(board) = self.contract_board() else {
            return Err(ContractRefusal::NotOffered);
        };
        {
            let held = self.world.resource::<ActiveContracts>();
            if held.active.iter().any(|c| c.def.id == *id) {
                return Err(ContractRefusal::AlreadyActive);
            }
            if held.done.contains(id)
                && !self
                    .world
                    .resource::<crate::contracts::ContractDb>()
                    .get(id)
                    .is_some_and(|def| def.repeatable)
            {
                return Err(ContractRefusal::AlreadyDone);
            }
            if held.active.len() >= crate::tuning::MAX_ACTIVE_CONTRACTS {
                return Err(ContractRefusal::TooMany);
            }
        }
        if !board.iter().any(|row| row.id == *id) {
            return Err(ContractRefusal::NotOffered);
        }
        let Some(def) = self
            .world
            .resource::<crate::contracts::ContractDb>()
            .get(id)
            .cloned()
        else {
            return Err(ContractRefusal::NotOffered);
        };

        let accepted_tick = self.current_tick();
        let name = def.name.clone();
        self.world.resource_mut::<ActiveContracts>().active.push(
            crate::resources::ActiveContract {
                def,
                progress: 0,
                accepted_tick,
            },
        );
        self.log_kind(MessageKind::Outcome, format!("Contract taken: {name}."));
        Ok(())
    }

    /// Gives a contract back. Returns whether anything was abandoned.
    ///
    /// Progress is lost rather than banked, and the id is **not** filed under
    /// `done`: giving up is not finishing, and a contract handed back has to
    /// be takeable again.
    pub fn abandon_contract(&mut self, id: &ContractId) -> bool {
        let mut held = self.world.resource_mut::<ActiveContracts>();
        let Some(idx) = held.active.iter().position(|c| c.def.id == *id) else {
            return false;
        };
        let name = held.active.remove(idx).def.name;
        self.log_kind(MessageKind::Outcome, format!("Contract abandoned: {name}."));
        true
    }

    /// Hands over as many of a `Deliver` objective's items as it still needs,
    /// and returns how many were taken.
    ///
    /// The one place a `Deliver` objective's progress moves — `contract_system`
    /// deliberately does not poll cargo for it. It takes **only up to what the
    /// contract still needs**, or the player would lose cargo to a contract
    /// that was already satisfied, and it completes through
    /// `complete_contract` when that fills it so delivery and the polled
    /// objectives share one completion path.
    ///
    /// Every refusal lands before any item leaves cargo.
    pub fn deliver_to_contract(&mut self, id: &ContractId) -> Result<u32, ContractRefusal> {
        if self.contract_board().is_none() {
            return Err(ContractRefusal::NotOffered);
        }
        let Some((idx, item, wanted)) = self
            .world
            .resource::<ActiveContracts>()
            .active
            .iter()
            .enumerate()
            .find_map(|(idx, held)| match &held.def.objective {
                Objective::Deliver { item, count } if held.def.id == *id => {
                    Some((idx, item.clone(), count.saturating_sub(held.progress)))
                }
                _ => None,
            })
        else {
            return Err(ContractRefusal::NotOffered);
        };

        let player = self.player_entity();
        let carrying = self
            .world
            .get::<Inventory>(player)
            .map(|inv| inv.count(&item))
            .unwrap_or(0);
        let taken = carrying.min(wanted);
        if taken == 0 {
            return Err(ContractRefusal::NothingToDeliver);
        }

        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(item.clone(), taken);
        let name = self.item_name(&item).to_string();
        {
            let mut held = self.world.resource_mut::<ActiveContracts>();
            held.active[idx].progress += taken;
        }
        self.log_kind(
            MessageKind::Outcome,
            format!("You hand over {taken} {name}."),
        );
        let filled = {
            let held = self.world.resource::<ActiveContracts>();
            held.active[idx].progress >= held.active[idx].def.objective.target()
        };
        if filled {
            self.complete_contract(idx);
        }
        Ok(taken)
    }
}
