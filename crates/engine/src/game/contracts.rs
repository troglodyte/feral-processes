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
use crate::components::Structure;
use crate::contracts::{ContractId, Deed, Objective, Reward};
use crate::items::{ItemId, ids};
use crate::resources::{ActiveContracts, Locale, MessageKind, RunFeats, ZoneLevel};

/// How deep the party stands, for a `Descend` objective.
///
/// A free function so `contract_system` and `Game::objective_state` cannot
/// disagree about what base space answers — 0 is what "no Stack depth
/// reached" means here, and base space is neither the surface nor a frame.
///
/// Deliberately not `stack_market`'s `stack_depth`, which floors at 1
/// because a price needs a multiplier.
fn contract_depth(locale: &Locale) -> u32 {
    match locale {
        Locale::Stack { depth, .. } => *depth,
        Locale::Surface | Locale::Base { .. } => 0,
    }
}

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
    player: Query<&Inventory, With<crate::components::Player>>,
) {
    let state = crate::contracts::ObjectiveState {
        depth: contract_depth(&locale),
        zone: zone.0,
        standing: structures.iter().map(|s| s.kind.clone()).collect(),
        carried: player
            .iter()
            .next()
            .map(|inv| inv.items.clone())
            .unwrap_or_default(),
    };

    for contract in &mut held.active {
        let target = contract.def.objective.target();
        let advance = match &contract.def.objective {
            Objective::Terminate { species, count: _ } => feats
                .kills
                .iter()
                .filter(|killed| species.as_ref().is_none_or(|want| *want == **killed))
                .count() as u32,
            Objective::Perform { deed } => feats.deeds.iter().filter(|d| *d == deed).count() as u32,
            // Not here — see the module doc.
            Objective::Deliver { .. } => 0,
            // The state-shaped ones advance by exactly the predicate
            // `Game::offerable` refuses a board slot on, so a contract cannot
            // be offered in a state that would finish it and then fail to
            // finish once taken.
            polled => u32::from(polled.already_met(&state)),
        };
        contract.progress = contract.progress.saturating_add(advance).min(target);
    }

    // Unconditional, and this system is each field's only drainer: leaving a
    // kill or a deed in one would advance a contract accepted afterwards,
    // forever.
    feats.kills.clear();
    feats.deeds.clear();
}

impl Game {
    /// The onboarding mission the run is on, or `None` once every step is
    /// finished.
    ///
    /// **Derived, never stored**: the first mission in
    /// `ContractDb::tutorial_chain` whose id is not in
    /// `ActiveContracts::done`. There is no cursor and no index, so nothing
    /// can disagree with `done` about where the player is — the rule
    /// `views::BuildOrderRow` and `Game::morale` already follow.
    ///
    /// Cloned rather than borrowed because every caller goes on to touch
    /// `&mut self`.
    pub(crate) fn current_tutorial(&self) -> Option<crate::contracts::ContractDef> {
        let done = &self.world.resource::<ActiveContracts>().done;
        self.world
            .resource::<crate::contracts::ContractDb>()
            .tutorial_chain()
            .into_iter()
            .find(|def| !done.contains(&def.id))
            .cloned()
    }

    /// Whether onboarding is still running. The board's suppression, the
    /// forced first decompile and the renderer's green row all read this one
    /// call rather than each deciding for themselves.
    pub fn in_tutorial(&self) -> bool {
        self.current_tutorial().is_some()
    }

    /// Puts the run's current onboarding mission in hand if it is not there
    /// already. **The one writer of a tutorial contract into
    /// `ActiveContracts`**, called from `Game::new`, `Game::load` and
    /// `Game::settle_contracts`.
    ///
    /// It never goes through `accept_contract`, and three things follow as
    /// **omissions rather than checks**, which is the point of routing it
    /// this way: `MAX_ACTIVE_CONTRACTS` never sees it, so the cap keeps
    /// meaning what it meant; `broker_reach` never sees it, which is what
    /// lets the first five missions exist before a Contract Broker does; and
    /// `offerable` never sees it, so no `min_zone` or `already_met` can hold
    /// the chain up.
    pub(crate) fn ensure_tutorial_held(&mut self) {
        let Some(def) = self.current_tutorial() else {
            return;
        };
        if self
            .world
            .resource::<ActiveContracts>()
            .active
            .iter()
            .any(|c| c.def.id == def.id)
        {
            return;
        }
        let accepted_tick = self.current_tick();
        let name = def.name.clone();
        // The briefing carries the contract's own words, filled from the def
        // in hand — one templated def rather than a second copy of every
        // mission's name and description.
        let objective = self.objective_line(&def.objective);
        self.notify_filled(
            crate::notifications::NotificationKind::OnboardingMission,
            &[
                ("name", &name),
                ("objective", &objective),
                ("description", &def.description),
            ],
        );
        self.world.resource_mut::<ActiveContracts>().active.push(
            crate::resources::ActiveContract {
                def,
                progress: 0,
                accepted_tick,
            },
        );
        // `Outcome` rather than `Info`, `complete_contract`'s reason: a
        // mission can be handed out mid-fight, and the battle prune keeps
        // only four kinds.
        self.log_kind(MessageKind::Outcome, format!("ONBOARDING: {name}"));
    }

    /// The one door a `Deed` is written through. The six triggers are
    /// **callers of this, not writers beside it** — `Game::remember`'s rule,
    /// and what keeps "which deeds exist" answerable by reading one file.
    pub(crate) fn note_deed(&mut self, deed: Deed) {
        self.world.resource_mut::<RunFeats>().deeds.push(deed);
    }

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
        // Below the reward loop rather than above it, so the alert screen
        // can quote the same figure the `Loot` line just logged. Moving it
        // does not reopen double-payment: the contract already left
        // `ActiveContracts` at the top of this function, which is the half
        // of "drop before pay" that makes a second settle unexpressible —
        // this notify never touches `ActiveContracts` at all.
        self.notify_with_detail(
            crate::notifications::NotificationKind::ContractClosed,
            Some(paid),
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

/// Where the player is standing, as far as contracts are concerned. See
/// `Game::broker_reach`, which is the only thing that builds one.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BrokerReach {
    /// Nothing standing that `issues_contracts`, so there is no board to
    /// read and nothing to be away from.
    NoBroker,
    /// A Broker exists, but the player is not in the base — out on the grid, or
    /// underground. The board is readable; nothing on it can be acted on.
    OffBase,
    /// In base space, standing on laid floor, with a Broker standing.
    /// Everything is available.
    AtBroker,
}

/// Salts the board's seed so what a sector is offering does not correlate
/// with anything else derived from the same world seed. Its own named
/// constant, per `FrameSpec::salted`'s rule — one scheme, not a second seed
/// source that could collide with the Stack's.
const CONTRACT_BOARD_SALT: u64 = 0xC0A7_7AC7_5EED_0001;

/// FNV-1a, a byte at a time — the one folding scheme this feature salts with,
/// shared by the board's own seed and each template's.
///
/// Byte-at-a-time rather than one XOR-and-multiply per word, for
/// `FrameSpec::salted`'s measured reason: a whole-word XOR leaves low output
/// bits a fixed function of the input, and consecutive epochs differ in
/// exactly one low bit.
pub(crate) fn fold(mut h: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

impl Game {
    /// What the run's Broker is offering, or `None` if the run has no Broker
    /// standing.
    ///
    /// One call answers both "is there a board" and "what is on it", so no
    /// screen asks those separately and then disagrees — `Game::stack_market`'s
    /// contract. Whether the player may *take* any of it is the third
    /// question and `Game::broker_reach`'s.
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
    /// Readable from anywhere, underground included, and that follows from
    /// the seed above rather than being a concession: the board is a property
    /// of the sector and makes no claim about where the party is standing, so
    /// there is nothing for distance to invalidate. It used to be `None`
    /// underground because reach *was* measured from the player's `Position`,
    /// which is pinned to the surface entrance tile the whole time they are
    /// down there — that reading is what `broker_reach` retired.
    pub fn contract_board(&mut self) -> Option<Vec<crate::views::ContractRow>> {
        let defs = self.board_defs()?;
        Some(defs.iter().map(|def| self.contract_row(def, 0)).collect())
    }

    /// The board as **definitions** rather than worded rows — what
    /// `contract_board` renders and what `accept_contract` takes its copy from.
    ///
    /// Carrying the def rather than an id is the whole reason a rolled
    /// contract works at all. Every step of the accept path used to re-resolve
    /// the def out of `ContractDb` by id, which a rolled contract has no entry
    /// in — so it would be refused as `NotOffered` while sitting visibly on the
    /// board. `resources::ActiveContract` already stores the whole resolved
    /// def, for a different reason (a file edited mid-run must not strand a
    /// contract already accepted), and that is exactly the shape a rolled one
    /// needs.
    fn board_defs(&mut self) -> Option<Vec<crate::contracts::ContractDef>> {
        if self.broker_reach() == BrokerReach::NoBroker {
            return None;
        }
        // Onboarding owns the board while it runs. A new player choosing
        // between three offers they have no way to evaluate is what the
        // chain exists to replace, and the starter queue below was the
        // weaker first attempt at the same thing.
        //
        // `Some(vec![])` rather than `None`: the Broker is standing and
        // reachable, and `None` is the claim that it is not — a claim two
        // other readers act on.
        //
        // It also keeps the chain's *later* steps off the board. They are
        // ordinary unfinished contracts, so `offerable` would happily list
        // step 3 beside step 1's held copy; once the chain is over they are
        // all in `done` and refused there, so this is the only guard needed.
        if self.in_tutorial() {
            return Some(Vec::new());
        }
        let mut pool = self.offerable_contracts();
        pool.extend(self.rolled_contracts());
        // Starters first, and only then the rest of the pool. Three slots
        // drawn uniformly out of everything eligible left a new run's first
        // contract to chance, which is not what an onboarding job is for.
        // Both halves draw the same way, so a board with no starters left
        // spends its draws exactly as it did before they existed.
        //
        // The queue is the **first sector's** only: a starter is still
        // offerable past it, just no longer ahead of everything else. Breaching
        // is where onboarding ends — one of the shipped starters is that very
        // breach — and a board that kept pushing them would hand a zone-4 run's
        // Broker a job to go and kill three drones.
        let onboarding = self.world.resource::<ZoneLevel>().0 <= 1;
        let (mut starters, mut rest): (Vec<_>, Vec<_>) =
            pool.into_iter().partition(|def| onboarding && def.starter);
        let mut rng = StdRng::seed_from_u64(self.board_seed());
        let mut defs = Vec::new();
        for tier in [&mut starters, &mut rest] {
            while defs.len() < crate::tuning::CONTRACT_BOARD_SLOTS && !tier.is_empty() {
                defs.push(tier.swap_remove(rng.random_range(0..tier.len())));
            }
        }
        Some(defs)
    }

    /// Every template rolled once against this sector, keeping the ones that
    /// came out finishable and offerable.
    ///
    /// Each template rolls from its **own** `StdRng`, salted off the board seed
    /// with the template's id, rather than all of them sharing the board's
    /// stream. That is `FrameSpec::salted`'s rule and it buys something
    /// concrete here: a template that rolls nothing spends no draws, so adding
    /// or deleting a template file cannot reshuffle what the others offered.
    fn rolled_contracts(&mut self) -> Vec<crate::contracts::ContractDef> {
        let pools = self.template_pools();
        let zone = self.world.resource::<ZoneLevel>().0;
        let seed = self.board_seed();
        let templates: Vec<crate::contracts::ContractTemplate> = self
            .world
            .resource::<crate::contracts::ContractDb>()
            .templates()
            .filter(|t| t.min_zone <= zone)
            .cloned()
            .collect();
        templates
            .iter()
            .filter_map(|t| {
                let mut rng = StdRng::seed_from_u64(fold(seed, t.id.as_str().as_bytes()));
                t.roll(&mut rng, &pools)
            })
            .filter(|def| self.offerable(def))
            .collect()
    }

    /// What this sector can supply a rolled contract with.
    ///
    /// The species half is read from the Chebyshev ring **around the base's
    /// anchor**, at the band the base's own footprint used to end at. It is a
    /// reading of what lives on the run's doorstep, and the door is the one
    /// thing the base still has on the zone surface. `spawn_surface_links`
    /// draws its on-ramp from the same kind of band, through the same
    /// `stack::ring_offset`.
    ///
    /// Sampled at `CONTRACT_HABITAT_SAMPLES` evenly-spaced points rather than
    /// walked whole, because `contract_board` sits on a per-frame path — both
    /// the contracts screen and the base menu's row test call it.
    pub(crate) fn template_pools(&mut self) -> crate::contracts::TemplatePools {
        let zone = self.world.resource::<ZoneLevel>().0;
        let mut pools = crate::contracts::TemplatePools {
            species: Vec::new(),
            items: Vec::new(),
            structures: Vec::new(),
            zone,
        };
        // Only the species half needs a tile to read from, and it is the
        // anchor's: the base itself is out of phase now and has no ground on
        // the zone surface at all, so the door it is reached through is the
        // whole of its presence in the sector. A run with no anchor has no
        // doorstep to read and is offered no Hunt — but it can still be asked
        // to deliver or to build, neither of which is a question about the
        // ground.
        if let Some((cx, cy)) = self.anchor_position() {
            let band = crate::tuning::STARTING_POCKET_RADIUS + 1;
            let perimeter = 8 * band;
            let samples = crate::tuning::CONTRACT_HABITAT_SAMPLES.min(perimeter);
            let mut species: Vec<String> = Vec::new();
            for i in 0..samples {
                let (dx, dy) = crate::game::stack::ring_offset(band, i * perimeter / samples);
                if let Some((ordinary, _bosses)) = self.habitat_pools(cx + dx, cy + dy, None, 0) {
                    species.extend(ordinary);
                }
            }
            species.sort();
            species.dedup();

            let species_db = self.world.resource::<crate::species::SpeciesDb>();
            pools.species = species
                .into_iter()
                .filter_map(|id| {
                    species_db
                        .get(&id)
                        .map(|def| (id.clone(), def.name.clone()))
                })
                .collect();
        }

        pools.items = self.deliverable_items(&pools.species);
        pools.structures = self.commissionable_structures();
        pools
    }

    /// Items a rolled `Deliver` may ask for: cheap bulk stock the player can
    /// make, that a machine they could build prints, or that a program on
    /// their doorstep drops.
    ///
    /// A delivery is asked for **by the score**, and that is what both filters
    /// are about. `ItemCategory::Material` is the "what you hoard" bucket, so
    /// it rules out anything worn, drunk or spent as currency — and a `Deliver`
    /// reads plain `Inventory`, which is by definition the plain-copy store, so
    /// asking for gear would be asking for the one thing that may not be
    /// sitting in it. The value ceiling is the rest: `ItemDef::value`'s ladder
    /// runs printable 1 → scavenged 3-8 → standard 12-16 → researched 20-60 →
    /// premium 80-120, and only the bottom rungs are things a base accumulates
    /// twenty of. Without it a Requisition asked for twenty etched Routine
    /// Disks — a run's worth of research, stated as an errand.
    ///
    /// Portal Fragments fall out of the first filter, since `role` makes them
    /// `Currency`. That matters more than it looks: they are the breaching
    /// currency, their only source is a boss underground, and a contract
    /// eating a stack's worth is a run that cannot breach out.
    /// `a_rolled_delivery_never_asks_for_the_breaching_currency` asserts the
    /// outcome rather than the mechanism, so a later retune of `role` cannot
    /// quietly reopen it.
    fn deliverable_items(&self, species: &[(String, String)]) -> Vec<(ItemId, String)> {
        let structures = self.world.resource::<crate::structures::StructureDb>();
        let printed: Vec<ItemId> = structures
            .all()
            .filter(|def| self.structure_unlocked(&def.id))
            .flat_map(|def| {
                def.work
                    .iter()
                    .map(|w| w.produces.clone())
                    .chain(def.assembles.iter().map(|a| a.item.clone()))
            })
            .collect();
        self.world
            .resource::<crate::items_db::ItemDb>()
            .all()
            .filter(|def| def.category() == crate::items::ItemCategory::Material)
            .filter(|def| self.item_value(&def.id) <= crate::tuning::CONTRACT_MAX_DELIVER_VALUE)
            .filter(|def| {
                def.craftable.is_some()
                    || printed.contains(&def.id)
                    || def
                        .droppable
                        .iter()
                        .flatten()
                        .any(|(from, _)| species.iter().any(|(id, _)| id == from.as_str()))
            })
            .map(|def| (def.id.clone(), def.name.clone()))
            .collect()
    }

    /// Structures a rolled `Build` may ask for: unlocked, and **not already
    /// standing**. The second half is the validity rule — `contract_system`
    /// finishes a `Build` the moment one is deployed, so naming something the
    /// player already owns pays out on acceptance.
    fn commissionable_structures(&self) -> Vec<(crate::structures::StructureId, String)> {
        let standing = self.standing_structures();
        self.buildable_structure_defs()
            .into_iter()
            .filter(|def| !standing.contains(&def.id))
            .map(|def| (def.id.clone(), def.name.clone()))
            .collect()
    }

    /// Whether the run could be offered `def` right now: this sector's level,
    /// not already in hand, and not finished-and-not-repeatable.
    ///
    /// One predicate rather than two copies, because an authored contract and
    /// a rolled one have to be filtered by exactly the same rule — a rolled
    /// contract that survived a filter the authored ones don't would reappear
    /// on the board after it had been finished.
    #[cfg(test)]
    pub(crate) fn offerable_contracts_for_test(&self, def: &crate::contracts::ContractDef) -> bool {
        self.offerable(def)
    }

    fn offerable(&self, def: &crate::contracts::ContractDef) -> bool {
        let zone = self.world.resource::<ZoneLevel>().0;
        let held = self.world.resource::<ActiveContracts>();
        if def.min_zone > zone {
            return false;
        }
        if held.active.iter().any(|c| c.def.id == def.id) {
            return false;
        }
        if held.done.contains(&def.id) && !def.repeatable {
            return false;
        }
        // Never offer something the run has already done — asked at depth 0
        // and against an empty pack, deliberately.
        //
        // The board is **the sector's**, and it is readable underground and
        // off the base. Answered from the party's live `ObjectiveState` a
        // `Descend(1)` would drop out of the pool the moment the party stood
        // one frame down, and `board_defs` draws with `swap_remove`, so a
        // pool one entry shorter reshuffles *every* slot — a board that
        // changed as you walked, against the seam that says it is derived
        // from the seed and nothing else. `carried` is the same trap one
        // objective over, waiting for the first non-tutorial `Hold`.
        !def.objective
            .already_met(&crate::contracts::ObjectiveState {
                depth: 0,
                zone,
                standing: self.standing_structures(),
                carried: Vec::new(),
            })
    }

    /// Every deployed structure's kind. Collected rather than queried lazily
    /// because both readers want to ask about several contracts against one
    /// snapshot.
    fn standing_structures(&self) -> Vec<crate::structures::StructureId> {
        self.world
            .iter_entities()
            .filter_map(|e| e.get::<Structure>().map(|s| s.kind.clone()))
            .collect()
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
        let db = self.world.resource::<crate::contracts::ContractDb>();
        let widest = self.widest_pools();
        db.iter()
            .map(|def| self.contract_row(def, 0))
            .chain(
                db.templates()
                    .filter_map(|t| t.widest(&widest))
                    .map(|def| self.contract_row(&def, 0)),
            )
            .collect()
    }

    /// The pools at their widest: every species, item and structure the assets
    /// define rather than what one sector supplies, and sector 0 so a rolled
    /// `Breach` is not floored out of its range.
    ///
    /// Only `contract_catalogue` wants this, and only because the width census
    /// has to measure the widest row the shipped assets can *ever* build. It
    /// is an upper bound rather than a reachable board — a row it flags as
    /// overflowing is one to shorten, which is right whether or not that exact
    /// roll can happen.
    fn widest_pools(&self) -> crate::contracts::TemplatePools {
        let species: Vec<(String, String)> = self
            .world
            .resource::<crate::species::SpeciesDb>()
            .all()
            .map(|def| (def.id.clone(), def.name.clone()))
            .collect();
        crate::contracts::TemplatePools {
            items: self.deliverable_items(&species),
            structures: self
                .world
                .resource::<crate::structures::StructureDb>()
                .all()
                .map(|def| (def.id.clone(), def.name.clone()))
                .collect(),
            species,
            zone: 0,
        }
    }

    /// Which authored contracts this run could be offered right now, in the
    /// db's stable id order — the pool the board draws its slots from.
    ///
    /// Named rather than inlined so a test can ask what is offerable without
    /// depending on which three the roll happened to pick, and so the filter
    /// itself is stated once: this sector's level, minus anything already in
    /// hand, minus anything finished that does not repeat.
    pub(crate) fn offerable_contracts(&self) -> Vec<crate::contracts::ContractDef> {
        self.world
            .resource::<crate::contracts::ContractDb>()
            .iter()
            .filter(|def| self.offerable(def))
            .cloned()
            .collect()
    }

    /// Where the player stands in relation to the run's Contract Broker —
    /// the one derivation behind both "is there a board" and "may I act on
    /// it".
    ///
    /// Three states out of one call rather than two predicates, for
    /// `NoPost::BoxedIn`'s reason for sitting beside `NoPost::NoRoute`: five
    /// things ask this (the board, accepting, delivering, the base menu's row
    /// and the screen's own header) and two independent booleans would let
    /// them disagree about whether a board that is drawn can be taken from.
    ///
    /// The Broker's own tile does not enter into it, and never did — this
    /// measures the **base**. `place_structure` refuses everything but a Home
    /// until a Home is standing and every structure has to stand on laid
    /// floor, so a Broker is in the base by construction; "is the player in
    /// the base" is therefore the whole question. Since the base moved out of
    /// phase that reads as: the party is in base space, standing on
    /// `BaseCell::Floor`.
    ///
    /// Floor and not merely `walkable`, so it keeps saying what it said: the
    /// desk is reachable from the base's laid ground, not from a corridor
    /// mined out past its edge. Nothing else is asked — no `is_underground`
    /// check, because `base_pos` is already `None` in every locale but this
    /// one, and no locale-dependent `Position` read, because `Position` is
    /// pinned to the anchor tile the whole time the party is in here.
    pub fn broker_reach(&mut self) -> BrokerReach {
        if !self.has_broker() {
            return BrokerReach::NoBroker;
        }
        let Some((x, y)) = self.base_pos() else {
            return BrokerReach::OffBase;
        };
        if self
            .world
            .resource::<crate::base_grid::BaseGrid>()
            .is_floor(x, y)
        {
            BrokerReach::AtBroker
        } else {
            BrokerReach::OffBase
        }
    }

    /// Whether the run has a Broker standing at all, wherever it is.
    fn has_broker(&mut self) -> bool {
        let mut query = self.world.query_filtered::<Entity, With<Structure>>();
        let standing: Vec<Entity> = query.iter(&self.world).collect();
        standing
            .into_iter()
            .any(|entity| self.issues_contracts(entity))
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
            h = fold(h, &word.to_le_bytes());
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
            tutorial: def.tutorial.is_some(),
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
            Objective::Hold { item, count } => {
                format!("Hold {count} {}", self.item_name(item))
            }
            // Exhaustive on purpose: a new `Deed` fails to compile here
            // rather than shipping a row with no words on it.
            Objective::Perform { deed } => match deed {
                Deed::Examined => "Examine something with [x]".to_string(),
                Deed::Tamed => "Decompile a wild program".to_string(),
                Deed::TookFromContainer => "Take stock out of a machine with [c]".to_string(),
                Deed::QueuedStandingOrder => "Place a standing work order".to_string(),
                Deed::UnlockedPerk => "Spend a Perk Point".to_string(),
                Deed::PostedStaff => "Set a machine to be kept staffed".to_string(),
            },
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
    /// No Broker standing at all, or this contract is not on the board —
    /// and, for a delivery, not one the run is holding.
    NotOffered,
    /// A Broker is standing and the contract is on its board, but the player
    /// is not on the base. Distinct from `NotOffered` because the two leave
    /// the player different errands: one is a walk home, the other is a
    /// contract that was never on offer.
    NotAtBroker,
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
        let Some(board) = self.board_defs() else {
            return Err(ContractRefusal::NotOffered);
        };
        if self.broker_reach() != BrokerReach::AtBroker {
            return Err(ContractRefusal::NotAtBroker);
        }
        {
            let repeatable = self
                .world
                .resource::<crate::contracts::ContractDb>()
                .repeatable(id);
            let held = self.world.resource::<ActiveContracts>();
            if held.active.iter().any(|c| c.def.id == *id) {
                return Err(ContractRefusal::AlreadyActive);
            }
            if held.done.contains(id) && !repeatable {
                return Err(ContractRefusal::AlreadyDone);
            }
            if held.active.len() >= crate::tuning::MAX_ACTIVE_CONTRACTS {
                return Err(ContractRefusal::TooMany);
            }
        }
        // The def comes off the board that was just built, never out of the db
        // again — a rolled contract has no db entry, and looking it up a
        // second time is what made one refusable while visibly on offer.
        let Some(def) = board.into_iter().find(|def| def.id == *id) else {
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
        // An onboarding mission cannot be given back. This is the invariant,
        // so it does not depend on a caller remembering to ask; the sentence
        // the player reads is app-core's, through `App::refuse`, because a
        // bare `false` cannot reach the log.
        if held.active[idx].def.tutorial.is_some() {
            return false;
        }
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
        match self.broker_reach() {
            BrokerReach::NoBroker => return Err(ContractRefusal::NotOffered),
            BrokerReach::OffBase => return Err(ContractRefusal::NotAtBroker),
            BrokerReach::AtBroker => {}
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
