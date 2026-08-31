//! The one reader of `environment::GroundCondition`, and the zone-1 gate it
//! holds.
//!
//! Kept apart from the catalogue itself so a later weather layer has an
//! obvious home beside it rather than inside the catalogue module.

use crate::Game;
use crate::derive;
use crate::environment::{EnvironmentEffect, GroundCondition, StaticEvent};
use crate::game::contracts::fold;
use crate::resources::ZoneLevel;
use crate::tuning::{STATIC_CLEAR_WEIGHT, STATIC_EPOCH_TICKS};
use crate::world::{Biome, WorldMap};

/// What the ground at `(x, y)` is, and what it does to whoever stands on it.
pub struct Terrain {
    pub biome: Biome,
    pub condition: Option<GroundCondition>,
    /// Folded and clamped — the one number the turn hook needs, so it never
    /// has to know how many sources contributed to it.
    pub effect: EnvironmentEffect,
}

impl Game {
    /// Resolves `(x, y)` to its `Terrain`.
    ///
    /// The one door: both the zone-1 rule and the `Platform` refusal live
    /// here rather than at the call site, so neither can lapse when a
    /// second caller appears. Zone 1 is where a run learns the game, and
    /// ground that bites there would be a tax on the tutorial rather than
    /// an exception to it. The biome's *name* is deliberately outside the
    /// zone-1 gate — a zone-1 player must still learn the world's
    /// vocabulary, so this returns the real biome and a neutral effect
    /// rather than an `Option` that would hide both.
    ///
    /// `&mut self` because `WorldMap::tile` generates its chunk on demand;
    /// nothing here writes anything the caller can observe.
    pub fn terrain_at(&mut self, x: i32, y: i32) -> Terrain {
        let biome = self.world.resource_mut::<WorldMap>().tile(x, y).biome;
        if self.world.resource::<ZoneLevel>().0 <= 1 {
            return Terrain {
                biome,
                condition: None,
                effect: EnvironmentEffect::NONE,
            };
        }
        // The one safe floor in the game should not depend on a condition
        // never claiming it — a base can be stamped over any ground at all.
        if biome == Biome::Platform {
            return Terrain {
                biome,
                condition: None,
                effect: EnvironmentEffect::NONE,
            };
        }
        let condition = GroundCondition::for_biome(biome);
        let ground = condition
            .map(|c| c.def().effect)
            .unwrap_or(EnvironmentEffect::NONE);
        // Folded onto the identity rather than just clamped: there is only
        // one source this task, but the fold is the shape a later weather
        // layer stacks onto, not a case-split that has to change shape when
        // a second source arrives.
        let effect = EnvironmentEffect::NONE.fold(ground).clamped();
        Terrain {
            biome,
            condition,
            effect,
        }
    }

    /// The weather epoch the clock is in right now.
    pub(crate) fn static_epoch(&self) -> u64 {
        self.current_tick() / STATIC_EPOCH_TICKS
    }

    /// Ungated derivation, for an epoch the caller names. The zone-1 and
    /// `Platform` gates live in `terrain_at`, not here.
    ///
    /// **Takes the epoch rather than reading the clock**, because Task 4
    /// has to ask what was live in the epoch that just ended. A version
    /// that reads `current_tick()` internally cannot answer that, and the
    /// turnover announcement would have no way to know what cleared.
    ///
    /// Draws **no** `GameRng` — a draw here would not survive a save/load
    /// and would shift every later roll in the run, `stack::generate`'s
    /// rule for worldgen. `static_seed` folds the world seed, the zone, the
    /// biome and the epoch a byte at a time and `derive::index` reduces the
    /// result, never `%`, which would read nothing but the low bits of
    /// whichever word was folded in last.
    pub(crate) fn static_in_epoch(&self, biome: Biome, epoch: u64) -> Option<StaticEvent> {
        let seed = self.world.resource::<WorldMap>().seed();
        let zone = self.world.resource::<ZoneLevel>().0;
        let pool: Vec<StaticEvent> = StaticEvent::all()
            .into_iter()
            .filter(|event| event.claims(biome))
            .collect();
        let total = STATIC_CLEAR_WEIGHT as usize
            + pool.iter().map(|e| e.def().weight as usize).sum::<usize>();
        let mut roll = derive::index(static_seed(seed, zone, biome, epoch), total);
        // Clear is walked first, so adding a fifth event only ever eats into
        // the *event* slots and never reshuffles which epochs are clear.
        if roll < STATIC_CLEAR_WEIGHT as usize {
            return None;
        }
        roll -= STATIC_CLEAR_WEIGHT as usize;
        for event in pool {
            let weight = event.def().weight as usize;
            if roll < weight {
                return Some(event);
            }
            roll -= weight;
        }
        unreachable!("the roll is bounded by `total`, which sums exactly these weights")
    }

    /// `static_in_epoch` at the current epoch. What `terrain_at` calls.
    pub(crate) fn static_at(&self, biome: Biome) -> Option<StaticEvent> {
        self.static_in_epoch(biome, self.static_epoch())
    }
}

/// Biome discriminant is save-adjacent — `Biome` derives
/// `Serialize`/`Deserialize` and carries a `serde(alias)` from the
/// `StaticField` → `Deadlock` rename — so folding `biome as u64` would
/// silently re-roll every existing world's weather the day a variant is
/// inserted or reordered. This is the stable integer `static_seed` folds
/// instead of the discriminant.
fn biome_ord(biome: Biome) -> u64 {
    match biome {
        Biome::DataVoid => 0,
        Biome::Deadlock => 1,
        Biome::NullSector => 2,
        Biome::Mainframe => 3,
        Biome::OpenGrid => 4,
        Biome::BlackIce => 5,
        Biome::Platform => 6,
        Biome::Excavated => 7,
        Biome::Entropy => 8,
    }
}

/// Folds the world seed, the zone, the biome and the epoch into the value
/// `derive::index` reduces — `sectors::sector_seed`'s pattern with one more
/// word. Every word goes in a byte at a time: one XOR-then-multiply round
/// only carries a difference about the prime's own width (~41 bits) upward,
/// so a word folded in whole would leave a following word's low output bits
/// a fixed function of it and never reach bit 63, which is the bit
/// `derive::index` actually reads.
fn static_seed(seed: u32, zone: u32, biome: Biome, epoch: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for word in [seed as u64, zone as u64, biome_ord(biome), epoch] {
        h = fold(h, &word.to_le_bytes());
    }
    h
}
