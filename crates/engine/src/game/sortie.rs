//! Sending a squad of base staff away from the base to fight for a while.
//!
//! The whole feature's behaviour: reach, the board, dispatch, the trip and
//! the return. `crate::sorties` is the catalogue and holds no game logic;
//! `resources::Sortie` is the in-flight record.

use bevy_ecs::prelude::*;

use crate::Game;
use crate::components::{Stats, Structure};
use crate::items::ItemId;

/// Whether the player can read the board, and whether they can sign for a
/// squad.
///
/// Three states rather than two booleans, for `NoPost::BoxedIn`'s reason:
/// "no Relay built" and "not standing in the base" leave the player
/// different errands, and a screen that cannot tell them apart says the
/// wrong sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortieReach {
    NoRelay,
    OffBase,
    AtRelay,
}

/// Why a dispatch was refused.
///
/// Typed rather than a `String`, `ContractRefusal`'s reason: each of these
/// leaves the player a different errand, and app-core words them for the
/// screen. `NotStaff` and `Downed` are distinct for that reason too — the
/// first wants the program unpartied, the second wants it repaired.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SortieRefusal {
    NotAtRelay,
    /// This site is not on the current board. Kept apart from `NotAtRelay`
    /// because one is a walk and the other is a wait.
    NotOffered,
    NoSquad,
    /// One program named twice. Refused rather than silently deduped: the
    /// squad size is what the provisioning is priced off, so a quiet dedupe
    /// would charge for a body that never went.
    Duplicate,
    NotStaff(String),
    Downed(String),
    Wounded(String),
    /// The base would be left with nobody in it.
    WouldEmptyTheBase,
    Unprovisioned {
        item: ItemId,
        need: u32,
        held: u32,
    },
}

impl Game {
    /// Where the player is standing, as far as sorties are concerned.
    ///
    /// **It measures the base, never the distance to the Relay** — this is
    /// `Game::broker_reach` one verb along, and for its argument:
    /// `place_structure` refuses everything but a Home until a Home is
    /// standing and every structure has to stand on laid floor, so a Relay
    /// is in the base by construction. "Is the player in the base" is
    /// therefore the whole question, which since the base moved out of
    /// phase reads as: the party is in base space, standing on
    /// `BaseCell::Floor`.
    ///
    /// Floor and not merely `walkable`, `broker_reach`'s rule: the mast is
    /// reachable from the base's laid ground, not from a corridor mined out
    /// past its edge.
    pub fn sortie_reach(&mut self) -> SortieReach {
        if !self.has_relay() {
            return SortieReach::NoRelay;
        }
        let Some((x, y)) = self.base_pos() else {
            return SortieReach::OffBase;
        };
        if self
            .world
            .resource::<crate::base_grid::BaseGrid>()
            .is_floor(x, y)
        {
            SortieReach::AtRelay
        } else {
            SortieReach::OffBase
        }
    }

    /// Whether the run has a Relay standing at all, wherever it is.
    fn has_relay(&mut self) -> bool {
        let mut query = self.world.query_filtered::<Entity, With<Structure>>();
        let standing: Vec<Entity> = query.iter(&self.world).collect();
        standing
            .into_iter()
            .any(|entity| self.dispatches_sorties(entity))
    }

    /// Whether `entity` is a structure a squad can be dispatched from —
    /// read off the def's flag and never off the shipped id, so a mod's
    /// second dispatch structure works without an engine change.
    fn dispatches_sorties(&self, entity: Entity) -> bool {
        let Some(kind) = self.world.get::<Structure>(entity).map(|s| &s.kind) else {
            return false;
        };
        self.world
            .resource::<crate::structures::StructureDb>()
            .get(kind)
            .is_some_and(|def| def.dispatches_sorties)
    }

    /// The offers standing at the Relay, or `None` with no Relay built.
    ///
    /// **Derived, never stored** — the Broker board's rule and for its
    /// reasons: recomputed on every read from the world seed, `ZoneLevel`
    /// and the clock epoch, so there is no save field, no roll to scum, and
    /// it rotates on its own as the epoch advances.
    ///
    /// Draws **no** `GameRng` at all. A draw here would not survive a reload
    /// and would shift every later roll in the run — `stack::generate`'s
    /// rule. Selection and each site's battle count both fold their own seed
    /// and reduce it through `derive::index`, never `%`: for a small pool `%`
    /// reads nothing but the seed's lowest bit and silently anti-correlates
    /// two draws taken off one fold.
    ///
    /// An empty catalogue gives an empty `Vec` and **not** `None`, which
    /// means "no Relay" — the two leave the player different errands, which
    /// is `SortieReach`'s own argument one level down.
    pub fn sortie_board(&mut self) -> Option<Vec<crate::views::SortieRow>> {
        if self.sortie_reach() == SortieReach::NoRelay {
            return None;
        }
        let seed = self.sortie_board_seed();
        let mut pool: Vec<crate::sorties::SortieDef> = self
            .world
            .resource::<crate::sorties::SortieDb>()
            .iter()
            .cloned()
            .collect();
        let mut rows = Vec::new();
        // Drawn without replacement, so one epoch's board never offers the
        // same site twice. `swap_remove` is what makes the walk O(slots)
        // and is safe for reproducibility because the pool it starts from
        // is id-sorted and every index is derived, not rolled.
        for slot in 0..crate::tuning::SORTIE_BOARD_SLOTS {
            if pool.is_empty() {
                break;
            }
            let pick = crate::derive::index(salt(seed, b"slot", slot as u64), pool.len());
            let def = pool.swap_remove(pick);
            let span = (def.battles_max - def.battles_min + 1) as usize;
            let battles = def.battles_min
                + crate::derive::index(salt(seed, def.id.as_str().as_bytes(), slot as u64), span)
                    as u32;
            rows.push(crate::views::SortieRow {
                id: def.id.clone(),
                name: def.name.clone(),
                description: def.description.clone(),
                risk: def.risk,
                battles,
                ticks: Self::sortie_duration(def.risk, battles),
            });
        }
        Some(rows)
    }

    /// The board's seed: the world seed, the sector and the epoch, folded
    /// FNV-1a a byte at a time.
    ///
    /// Byte-at-a-time rather than one XOR-and-multiply per word, for
    /// `FrameSpec::salted`'s measured reason and `Game::board_seed`'s: a
    /// whole-word XOR leaves low output bits a fixed function of the input,
    /// and consecutive epochs differ in exactly one low bit.
    fn sortie_board_seed(&self) -> u64 {
        let epoch = self.current_tick() / crate::tuning::SORTIE_BOARD_ROTATION_TICKS;
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for word in [
            self.world.resource::<crate::world::WorldMap>().seed() as u64,
            self.world.resource::<crate::resources::ZoneLevel>().0 as u64,
            epoch,
            crate::tuning::SORTIE_SALT,
        ] {
            h = crate::game::contracts::fold(h, &word.to_le_bytes());
        }
        h
    }

    /// What the provisioning for a squad of `squad` bodies running
    /// `battles` fights costs the base.
    ///
    /// Priced per battle *and* per body, because both are what provisions
    /// have to cover. Denominated in the **build currency**, which is
    /// role-derived rather than named in Rust and is what the base's shelves
    /// actually hold — the figure the stock strip is already showing.
    pub fn sortie_provision_cost(&self, battles: u32, squad: usize) -> Vec<(ItemId, u32)> {
        let units = crate::tuning::SORTIE_PROVISION_PER_BATTLE * battles * squad as u32;
        vec![(self.currency(), units)]
    }

    /// Sends `members` to the site `id` names on the current board.
    ///
    /// Every refusal lands **before anything is spent**,
    /// `commit_caravan_basket`'s rule. Only once every one of them has
    /// passed does the provisioning leave the shelves, through
    /// `stock::spend_from_base` — a teleport off the shelf is right here:
    /// this is a base cost paid at the Relay, not a build a body walks to.
    ///
    /// The record stores the **whole resolved site**, never the id or a
    /// board index. A board that rotates while the squad is out, or an
    /// `assets/sorties/` file edited between sessions, must not be able to
    /// rewrite or strand a trip already in flight — `ActiveContract` stores
    /// a whole `ContractDef` for exactly that reason.
    pub fn dispatch_sortie(
        &mut self,
        id: &crate::sorties::SortieId,
        members: &[Entity],
    ) -> Result<(), SortieRefusal> {
        if self.sortie_reach() != SortieReach::AtRelay {
            return Err(SortieRefusal::NotAtRelay);
        }
        let Some(row) = self
            .sortie_board()
            .unwrap_or_default()
            .into_iter()
            .find(|r| &r.id == id)
        else {
            return Err(SortieRefusal::NotOffered);
        };
        if members.is_empty() {
            return Err(SortieRefusal::NoSquad);
        }
        let mut seen: Vec<Entity> = members.to_vec();
        seen.sort();
        seen.dedup();
        if seen.len() != members.len() {
            return Err(SortieRefusal::Duplicate);
        }
        for &member in members {
            if self.program_role(member) != Some(crate::game::party::ProgramRole::Staff) {
                return Err(SortieRefusal::NotStaff(self.creature_label(member)));
            }
            if self
                .world
                .get::<crate::components::Downed>(member)
                .is_some()
            {
                return Err(SortieRefusal::Downed(self.creature_label(member)));
            }
            let Some(stats) = self.world.get::<Stats>(member) else {
                return Err(SortieRefusal::NotStaff(self.creature_label(member)));
            };
            if (stats.hp as f32) < stats.max_hp as f32 * crate::tuning::SORTIE_MIN_HP_FRACTION {
                return Err(SortieRefusal::Wounded(self.creature_label(member)));
            }
        }
        // The base is never emptied. Production stops dead and a sweep lands
        // on an empty base — the same category of guard as `max_deployed`.
        if self.base_staff().len() <= members.len() {
            return Err(SortieRefusal::WouldEmptyTheBase);
        }
        let cost = self.sortie_provision_cost(row.battles, members.len());
        for (item, qty) in &cost {
            if crate::game::base::work_orders::base_holding(self, item) < *qty {
                return Err(SortieRefusal::Unprovisioned {
                    item: item.clone(),
                    need: *qty,
                    held: crate::game::base::work_orders::base_holding(self, item),
                });
            }
        }

        for (item, qty) in &cost {
            crate::game::base::stock::spend_from_base(self, item, *qty);
        }
        let site = self
            .world
            .resource::<crate::sorties::SortieDb>()
            .get(id)
            .cloned()
            .expect("a board row names a site the catalogue holds");
        let names: Vec<String> = members.iter().map(|&e| self.creature_label(e)).collect();
        self.world
            .resource_mut::<crate::resources::Sorties>()
            .0
            .push(crate::resources::Sortie {
                risk: site.risk,
                site,
                members: members.to_vec(),
                ticks_total: row.ticks,
                ticks_elapsed: 0,
                battles_total: row.battles,
                battles_done: 0,
                aborted: false,
                loot: Vec::new(),
                xp: 0,
                kills: 0,
                casualties: Vec::new(),
            });
        self.log_base(format!(
            "{} {} out for {}.",
            names.join(", "),
            if names.len() == 1 { "ships" } else { "ship" },
            row.name
        ));
        Ok(())
    }

    /// How long a trip to a site of this risk offset, running this many
    /// battles, takes.
    ///
    /// **The one place the figure is computed.** The board quotes it and the
    /// countdown runs it, `views::BuildOrderRow`'s rule that every figure on
    /// a screen is a call rather than a copy — a screen quoting one number
    /// while the countdown runs another is precisely the failure that rule
    /// exists for.
    ///
    /// It reads the site's **risk offset** and never the absolute danger
    /// band, or every trip late in a run would take enormously longer for no
    /// reason the player could name. And there is no term for squad size,
    /// level or power: a stronger squad shows up as better outcomes and
    /// never as a faster cycle.
    pub fn sortie_duration(risk: u32, battles: u32) -> u64 {
        crate::tuning::SORTIE_TRAVEL_BASE_TICKS
            + crate::tuning::SORTIE_TRAVEL_PER_RISK_TICKS * risk as u64
            + crate::tuning::SORTIE_TICKS_PER_BATTLE * battles as u64
    }
}

/// One draw's own seed, folded off the board's.
///
/// A separate fold per draw rather than one stream, `FrameSpec::salted`'s
/// rule: a site added to or removed from the catalogue must not reshuffle
/// which battle count the sites around it were offered at. Folded a byte at
/// a time and ending on the counter, because `derive::index` reads bit 63
/// and a value folded in as one whole word never reaches it.
fn salt(seed: u64, tag: &[u8], n: u64) -> u64 {
    let h = crate::game::contracts::fold(seed, tag);
    crate::game::contracts::fold(h, &n.to_le_bytes())
}
