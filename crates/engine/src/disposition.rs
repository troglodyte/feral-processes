//! Who a program *is*, as two axes and five points on them.
//!
//! A disposition is hidden. Nothing names it to the player and no screen
//! draws it — it is felt, through a body that runs itself down faster than
//! its neighbours or one that will not let a bad posting go. That is the
//! whole of the design, and it is what decides the shape of this module.
//!
//! **This is code and not `assets/`, deliberately.** Every other per-program
//! catalogue in the game — species, needs, memories — is a directory of
//! `.ron`, and the moddability rule says content is a file. A disposition is
//! not content: it ships no name, no blurb, no glyph and no mechanic, only
//! multipliers on numbers the sim already computes. That puts it on
//! `tuning.rs`'s side of the line that file draws — *how hard the game is, is
//! not moddable* — and the magnitudes live there accordingly.
//!
//! The second reason is stronger than the first. Every axis below is an
//! exhaustive `match`, so **adding a disposition fails to compile until every
//! axis answers for it** — `cell_mark`'s rule. A `.ron` def behind
//! `#[serde(default)]` can ship inert and unauthored instead, which this repo
//! has a scar from: `AbilityDef::spread` shipped documented, defaulted, and
//! used by none of the 77 files that could have set it.
//!
//! There is deliberately **no morale dial**. Morale *is*
//! `memories::sum_intensity`, so the two memory axes already move it; a third
//! constant aimed at the same number would be a second spelling of one
//! effect, and the two would drift.

use crate::tuning::{DISPOSITION_DRAIN_SWING, DISPOSITION_MEMORY_SWING};
use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};

/// A program's standing temperament — minted once from its `ProgramId` and
/// stored from then on.
///
/// **Two axes, and each disposition moves exactly one of them.** How fast the
/// body runs its reserves down, and how hard what it remembers lands. The
/// poles are symmetric by construction (`Languid`/`Dogged`,
/// `Amiable`/`Abrasive`), which is what keeps the whole table two constants
/// rather than a dozen, and what makes `Steady` mean *neutral on both* rather
/// than *unset*.
///
/// `Default` is `Steady`, so anything with no disposition at all — the
/// player, a wild program, a hand-built test fixture — reads as neutral
/// without a branch at any call site. That is `Memories`' rule: absence is a
/// meaning, not a missing value.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Disposition {
    /// Neutral on both axes. Not a fallback — a real draw, one of five.
    #[default]
    Steady,
    /// Bonds easily and lets things go: what it likes lands harder, what it
    /// resents lands softer.
    Amiable,
    /// Rough around the edges. Slow to settle anywhere, quick to hold a
    /// grudge against a tile or a machine.
    Abrasive,
    /// Runs itself down fast. Reads in play as lazy — not because it works
    /// any slower per tick, but because it is off at the Sandbox more of the
    /// time. See `need_drain`.
    Languid,
    /// Grinds on. Stays on a post long after its neighbours have wandered
    /// off to be serviced.
    Dogged,
}

impl Disposition {
    /// Every variant, for the seeding draw and for a census to walk.
    ///
    /// `GlyphColor::ALL`'s reason: a hand-written list in a test can miss a
    /// new variant where a walk of this one cannot.
    pub const ALL: [Disposition; 5] = [
        Disposition::Steady,
        Disposition::Amiable,
        Disposition::Abrasive,
        Disposition::Languid,
        Disposition::Dogged,
    ];

    /// Multiplies `NeedDef::drain_per_tick` in `needs_drain_system`.
    ///
    /// This is the whole of how "lazy" is expressed, and it is deliberately
    /// **not** a work-rate multiplier. `Task::progress` steps by an integer
    /// one tick at a time, so a rate multiplier there would need a new
    /// mechanic — a deterministic tick-skip — to mean anything at all. It
    /// does not need one: a body that runs its reserves down faster crosses
    /// its need's `critical` sooner, goes off shift sooner, and spends more
    /// of the run walking to an amenity. That is "doesn't want to finish
    /// tasks", reached entirely through machinery that already exists.
    pub fn need_drain(self) -> f32 {
        match self {
            Disposition::Languid => 1.0 + DISPOSITION_DRAIN_SWING,
            Disposition::Dogged => 1.0 - DISPOSITION_DRAIN_SWING,
            Disposition::Steady | Disposition::Amiable | Disposition::Abrasive => 1.0,
        }
    }

    /// Scales one memory's **signed** intensity, the pole chosen by the sign.
    ///
    /// Applied at the read rather than at the write, because that is where
    /// intensity exists: `Game::remember` stores a strike count and
    /// `Memory::intensity` derives the figure on every read. Scaling here
    /// means `morale`, `opinion_of` and the memories page cannot disagree
    /// about what a program feels, and it needs no save field of its own.
    ///
    /// **A scale and never a sign flip**, `Memory::intensity`'s rule one
    /// level down: a multiplier that could cross zero would turn an abrasive
    /// program's grudge into a fondness, so both poles stay strictly
    /// positive and `DISPOSITION_MEMORY_SWING` is bounded below 1 by the
    /// census in `tests/`.
    pub fn felt(self, intensity: f32) -> f32 {
        let (good, bad) = match self {
            Disposition::Amiable => (
                1.0 + DISPOSITION_MEMORY_SWING,
                1.0 - DISPOSITION_MEMORY_SWING,
            ),
            Disposition::Abrasive => (
                1.0 - DISPOSITION_MEMORY_SWING,
                1.0 + DISPOSITION_MEMORY_SWING,
            ),
            Disposition::Steady | Disposition::Languid | Disposition::Dogged => (1.0, 1.0),
        };
        intensity * if intensity < 0.0 { bad } else { good }
    }

    /// The one seeding formula: which disposition a program is born with,
    /// derived from its `ProgramId`.
    ///
    /// Derived rather than drawn, `descriptions.rs`' rule — a `GameRng` draw
    /// does not survive a save/load and shifts every later roll in the run,
    /// and an `StdRng` sequence is not stable across a `rand` upgrade. The
    /// answer is then **stored**, so that editing `ALL` later cannot silently
    /// reshuffle the personality of every program in an existing save. This
    /// runs once per program: at `roster_parts` for a new one, and on the
    /// load path for a file written before dispositions existed.
    ///
    /// **The id goes in a byte at a time**, `sectors::sector_seed`'s idiom
    /// and for its exact reason: one XOR-then-multiply round carries a
    /// difference only about the prime's own width upward, so folding a small
    /// id as a single word leaves programs 1, 2 and 3 differing nowhere near
    /// bit 63 — which is the bit `derive::index` reads. Every program on the
    /// roster would take the same disposition while each individual answer
    /// still looked arbitrary.
    pub fn seed(program_id: u32) -> Self {
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for byte in (program_id as u64).to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Disposition::ALL[crate::derive::index(h, Disposition::ALL.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two poles of each axis must actually differ from neutral, and
    /// from each other. Written against `ALL` rather than a hand-listed pair
    /// so a sixth variant that forgets to move anything is visible here.
    #[test]
    fn every_disposition_moves_exactly_one_axis() {
        for d in Disposition::ALL {
            let drain = d.need_drain() != 1.0;
            let memory = d.felt(1.0) != 1.0 || d.felt(-1.0) != -1.0;
            if d == Disposition::Steady {
                assert!(!drain && !memory, "Steady must be neutral on both axes");
            } else {
                assert!(
                    drain ^ memory,
                    "{d:?} must move exactly one axis, not none and not both"
                );
            }
        }
    }

    /// The pole pairs are what keep the table two constants wide. If someone
    /// gives one pole its own number this fails rather than drifting.
    #[test]
    fn the_poles_are_symmetric_about_neutral() {
        assert!(
            (Disposition::Languid.need_drain() + Disposition::Dogged.need_drain() - 2.0).abs()
                < f32::EPSILON
        );
        assert!(
            (Disposition::Amiable.felt(1.0) + Disposition::Abrasive.felt(1.0) - 2.0).abs()
                < f32::EPSILON
        );
    }

    /// The one correctness property on the memory axis: decay is a magnitude
    /// scale and never a sign flip, and a disposition must not become the
    /// thing that breaks it. A grudge that read as a fondness would be a
    /// program cheering up because it was hurt.
    #[test]
    fn felt_never_flips_a_memorys_sign() {
        for d in Disposition::ALL {
            assert!(d.felt(6.0) > 0.0, "{d:?} turned a fondness negative");
            assert!(d.felt(-6.0) < 0.0, "{d:?} turned a grudge positive");
            assert_eq!(d.felt(0.0), 0.0);
        }
    }

    /// Reserves must run *down*, whoever is doing it — a negative or zero
    /// multiplier would stall or refill a need, which is not a personality.
    #[test]
    fn every_disposition_still_drains() {
        for d in Disposition::ALL {
            assert!(d.need_drain() > 0.0, "{d:?} does not drain at all");
        }
    }

    /// The trap `sectors::sector_seed` documents, reached by the same route:
    /// fold a small id as one word and every program on a roster takes the
    /// same disposition while each answer still looks arbitrary. A real
    /// roster is tens of programs, so the first twenty ids must not collapse
    /// onto one or two variants.
    #[test]
    fn consecutive_program_ids_do_not_all_take_one_disposition() {
        let seen: std::collections::BTreeSet<_> = (1..=20u32)
            .map(Disposition::seed)
            .map(|d| d as u8)
            .collect();
        assert!(
            seen.len() >= 4,
            "20 consecutive ids reached only {} of 5 dispositions — the fold \
             is not reaching the high bits",
            seen.len()
        );
    }

    /// Seeding is a derivation, so it must answer the same thing every time
    /// — that is the whole reason it is not an RNG draw.
    #[test]
    fn seeding_is_stable() {
        for id in [1u32, 7, 42, 1000, u32::MAX] {
            assert_eq!(Disposition::seed(id), Disposition::seed(id));
        }
    }

    /// Absence reads as neutral without a branch at any call site.
    #[test]
    fn the_default_is_neutral_on_both_axes() {
        let d = Disposition::default();
        assert_eq!(d, Disposition::Steady);
        assert_eq!(d.need_drain(), 1.0);
        assert_eq!(d.felt(-4.0), -4.0);
    }
}
