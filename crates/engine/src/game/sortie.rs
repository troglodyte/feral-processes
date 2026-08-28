//! Sending a squad of base staff away from the base to fight for a while.
//!
//! The whole feature's behaviour: reach, the board, dispatch, the trip and
//! the return. `crate::sorties` is the catalogue and holds no game logic;
//! `resources::Sortie` is the in-flight record.

use bevy_ecs::prelude::*;

use crate::Game;
use crate::components::Structure;

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
