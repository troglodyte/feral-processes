//! Turning a cell of a Stack frame into a bank lookup.
//!
//! `descriptions.rs` at the crate root is content and knows nothing about a
//! run. This is the other half: which subject a `CellKind` is, which
//! condition the spent-state predicates put it in, what seed the place folds
//! down to, and where `{bearing}` comes from.
//!
//! **No caller ever constructs a seed.** Every entry point here takes a
//! place and owns the mixing internally, the same shape as
//! `Game::orphan_species`. A caller-supplied seed parameter is how two call
//! sites drift on *how* they salt, and how a third copy-pastes `LAIR_SALT`.

use super::listen::relative_bearing;
use super::stack::StackPos;
use crate::descriptions::DescriptionDb;
use crate::resources::{CurrentStack, TraceBand};
use crate::stack::CellKind;
use crate::*;

/// Keeps description folds clear of `LAIR_SALT` (`0x1A19_B055`),
/// `ORPHAN_SALT` (`0xDEAD_C0DE`) and `FALL_SALT` (`0xFA11_1E15`). Those three
/// answer one question per frame and stay on their single XOR; this one is
/// asked per cell, per length and per slot, so it rides `FrameSpec::salted`.
const DESCRIPTION_SALT: u64 = 0xDE5C_21B3;

/// The subject key for the frame-arrival mood line — the one description
/// that reads run state (depth band and Trace band) rather than only the
/// place. A separate subject so that exception stays visible.
const ARRIVAL_SUBJECT: &str = "stack.frame.arrival";

/// What a cell with no subject of its own is described as. `CellKind::Rock`
/// has no subject at all and never reaches here.
const FLOOR_SUBJECT: &str = "stack.floor";

impl Game {
    /// Which bank subject `cell` is, and under which condition — the one
    /// place a `CellKind` is translated into bank vocabulary.
    ///
    /// The condition axis reads the five predicates both Stack views already
    /// consult rather than recording anything new, so a looted cache reads
    /// as looted everywhere or nowhere.
    fn subject_of(
        &self,
        pos: StackPos,
        cell: (i32, i32),
    ) -> Option<(&'static str, Option<&'static str>)> {
        let level = self.world.resource::<CurrentStack>().0.as_ref()?;
        Some(match level.cell(cell.0, cell.1) {
            CellKind::Rock => return None,
            CellKind::Floor => (FLOOR_SUBJECT, None),
            CellKind::LinkUp if pos.depth == 1 => ("stack.link_up", Some("surface")),
            CellKind::LinkUp => ("stack.link_up", None),
            CellKind::LinkDown => ("stack.link_down", None),
            CellKind::Door => ("stack.door", None),
            CellKind::SealedDoor if self.seal_open(pos, cell) => {
                ("stack.sealed_door", Some("opened"))
            }
            CellKind::SealedDoor => ("stack.sealed_door", None),
            CellKind::Cache if self.cache_unopened(pos, cell) => ("stack.cache", None),
            CellKind::Cache => ("stack.cache", Some("spent")),
            CellKind::Lair if self.lair_cleared(pos) => ("stack.lair", Some("cleared")),
            CellKind::Lair => ("stack.lair", None),
            CellKind::Breakpoint if self.breakpoint_spent(pos, cell) => {
                ("stack.breakpoint", Some("spent"))
            }
            CellKind::Breakpoint => ("stack.breakpoint", None),
            CellKind::Orphan if self.orphan_present(pos, cell) => ("stack.orphan", None),
            CellKind::Orphan => ("stack.orphan", Some("spent")),
            CellKind::Fault => ("stack.fault", None),
            CellKind::Corruption => ("stack.corruption", None),
        })
    }

    /// The seed a cell's description folds down to.
    ///
    /// `FrameSpec::rng_seed` already folds world seed, entrance tile and
    /// depth — and `world_seed` changes on every breach — so two links in a
    /// sector are two different stacks, two depths are two different frames,
    /// and a new zone is new text, all for free.
    fn description_seed(&self, pos: StackPos, cell: (i32, i32)) -> u64 {
        self.frame_spec(pos.depth, pos.frames, pos.entrance)
            .salted(&[DESCRIPTION_SALT, cell.0 as u32 as u64, cell.1 as u32 as u64])
    }

    /// The frame's own seed, for the one description that is a property of
    /// the frame rather than of a cell in it.
    fn frame_description_seed(&self, pos: StackPos) -> u64 {
        self.frame_spec(pos.depth, pos.frames, pos.entrance)
            .salted(&[DESCRIPTION_SALT])
    }

    /// The descriptive clause for the row under the first-person view, or
    /// `None` when the bank has nothing — which leaves `stack_view` free to
    /// fall back to its own literal, so deleting the asset directory leaves
    /// the game working. A mod's prerogative, the same argument
    /// `crash_logs` made.
    pub(crate) fn underfoot_description(&self, pos: StackPos) -> Option<String> {
        let (subject, condition) = self.subject_of(pos, (pos.x, pos.y))?;
        let seed = self.description_seed(pos, (pos.x, pos.y));
        self.world
            .resource::<DescriptionDb>()
            .underfoot(subject, condition, seed)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
    }

    /// The one log line for `cell` coming into view, with `{bearing}`
    /// filled from the party's current facing.
    ///
    /// The fragments are a function of the place; the bearing is a function
    /// of live view geometry and is recomputed every call. They are
    /// composed, never stored together.
    pub(crate) fn sighted_description(&self, pos: StackPos, cell: (i32, i32)) -> Option<String> {
        let (subject, condition) = self.subject_of(pos, cell)?;
        let seed = self.description_seed(pos, cell);
        let line = self
            .world
            .resource::<DescriptionDb>()
            .sighted(subject, condition, seed)?;
        Some(fill_bearing(line, pos, cell))
    }

    /// The examine paragraph for `cell`, with `{bearing}` filled.
    pub(crate) fn cell_paragraph(&self, pos: StackPos, cell: (i32, i32)) -> Option<String> {
        let (subject, condition) = self.subject_of(pos, cell)?;
        let seed = self.description_seed(pos, cell);
        let text = self
            .world
            .resource::<DescriptionDb>()
            .paragraph(subject, condition, seed)?;
        Some(fill_bearing(&text, pos, cell))
    }

    /// The mood line fired once on entering a frame.
    ///
    /// The Trace band takes precedence over the depth band once the party is
    /// loud enough to be worth remarking on: being hunted is the more
    /// interesting fact about where you have just arrived, and it is the one
    /// the player has agency over.
    pub(crate) fn arrival_line(&self, pos: StackPos) -> Option<String> {
        let condition = match self.trace_band() {
            TraceBand::Hunted => Some("hunted"),
            TraceBand::Traced => Some("traced"),
            _ if pos.depth >= pos.frames => Some("bottom"),
            _ if pos.depth == 1 => Some("shallow"),
            _ => None,
        };
        self.world
            .resource::<DescriptionDb>()
            .sighted(ARRIVAL_SUBJECT, condition, self.frame_description_seed(pos))
            .map(str::to_string)
    }

    /// How much `cell` is worth a line when it first comes into view, or
    /// `None` when it is not worth one at all.
    ///
    /// Ranks unspent features above terrain and drops spent ones out
    /// entirely, so a corridor opening onto four features announces the one
    /// the player would actually walk to. Also the notability test the
    /// examine ray uses to decide which cell along a direction it is
    /// describing — one definition, so the thing `x` describes is the thing
    /// the log announced.
    ///
    /// `CellKind::LinkUp` is deliberately absent: it is the way the party
    /// came in, and announcing it would fire on arrival in every frame.
    /// Doors are absent for being the most common non-floor cell there is.
    pub(crate) fn notability(&self, pos: StackPos, cell: (i32, i32)) -> Option<u8> {
        let level = self.world.resource::<CurrentStack>().0.as_ref()?;
        Some(match level.cell(cell.0, cell.1) {
            CellKind::Lair if !self.lair_cleared(pos) => 5,
            CellKind::Orphan if self.orphan_present(pos, cell) => 4,
            CellKind::Cache if self.cache_unopened(pos, cell) => 3,
            CellKind::Breakpoint if !self.breakpoint_spent(pos, cell) => 3,
            CellKind::SealedDoor if !self.seal_open(pos, cell) => 2,
            CellKind::LinkDown => 2,
            CellKind::Fault | CellKind::Corruption => 1,
            _ => return None,
        })
    }
}

/// Expands the one substitution token the bank carries.
///
/// A cell the party is standing on has no bearing from itself, and
/// `relative_bearing` would answer "behind" for it, so that case is spelled
/// out rather than left to the dot product.
fn fill_bearing(text: &str, pos: StackPos, cell: (i32, i32)) -> String {
    if !text.contains("{bearing}") {
        return text.to_string();
    }
    let bearing = if cell == (pos.x, pos.y) {
        "right under you"
    } else {
        relative_bearing(pos, cell)
    };
    text.replace("{bearing}", bearing)
}
