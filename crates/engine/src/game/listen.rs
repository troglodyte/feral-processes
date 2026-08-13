//! Standing still in a Stack frame and listening to it.
//!
//! Bound to a key no screen in the game names — see
//! `crates/engine/EASTER_EGGS.md`, and the design argument in
//! `docs/superpowers/archive/specs/2026-08-06-easter-eggs-design.md`.
//!
//! What the reading says is deliberately the one thing the frame map
//! cannot tell you. The map gives absolute positions for everything the
//! party has walked past; a bearing off your own facing is what a
//! first-person sense returns, and turning in place moves the answer.

use super::stack::StackPos;
use crate::resources::CurrentStack;
use crate::stack::CellKind;
use crate::tuning::TRACE_PER_LISTEN;
use crate::*;

impl Game {
    /// Reads the frame the party is standing in: the bearing and distance
    /// of the nearest thing they have not spent yet, or silence.
    ///
    /// The refusal is `stack_pos` coming back `None`, **not**
    /// `require_surface`. This reads `Locale::Stack`'s own coordinates and
    /// facing and never touches `Position`, so it is the `Phase`/`Jump`
    /// case rather than the surface-action case — and by construction it
    /// cannot misreport where the party is, since everything it names is a
    /// cell of the frame they are in.
    ///
    /// **The turn and the Trace are charged either way.** Listening is loud
    /// in itself, and a swept-clean frame reporting silence is the
    /// information the turn bought. Free-when-empty would turn this into a
    /// zero-risk sweep to run on every tile.
    pub fn listen(&mut self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let Some(pos) = self.stack_pos() else {
            return Err("Nothing out here but open grid and your own fans.".into());
        };

        let line = self.reading(pos);
        // Ordered like `open_cache`: the line saying what was heard, then
        // the charge, so a band crossing reads as the consequence of having
        // listened rather than as something that happened first.
        self.log_kind(MessageKind::Outcome, line);
        self.raise_trace(TRACE_PER_LISTEN);
        self.tick();
        Ok(())
    }

    /// What listening from `pos` says.
    ///
    /// Rotten ground gets the first word. Standing on a fault or on
    /// corruption reads *that place's* own description rather than pointing
    /// at anything, which is why neither cell appears in `nearest_unspent`:
    /// they are not features to be spent, they are the ones with something
    /// to say.
    fn reading(&self, pos: StackPos) -> String {
        if let Some(line) = self.rot_reading(pos) {
            return line;
        }
        let Some(target) = self.nearest_unspent(pos) else {
            return "You go still and listen. Nothing left in this frame but the hum.".into();
        };
        let steps = (target.0 - pos.x).abs() + (target.1 - pos.y).abs();
        if steps == 0 {
            return "You go still and listen. Whatever's left down here is right under you.".into();
        }
        let bearing = relative_bearing(pos, target);
        let plural = if steps == 1 { "" } else { "s" };
        format!("You go still and listen. Something unspent {bearing}, {steps} step{plural} off.")
    }

    /// What the rot underfoot has to say — the description bank's paragraph
    /// for this exact cell, or `None` when the ground is not rotten, or when
    /// the bank has nothing for it, which is a mod's prerogative and falls
    /// back to the bearing rather than to silence.
    fn rot_reading(&self, pos: StackPos) -> Option<String> {
        if !matches!(
            self.cell_underfoot(),
            Some(CellKind::Fault | CellKind::Corruption)
        ) {
            return None;
        }
        self.cell_paragraph(pos, (pos.x, pos.y))
    }

    /// The nearest cell of the frame holding something the party has not
    /// used up, by Manhattan distance — the frame moves four ways, so that
    /// is the distance in steps rather than an approximation of one.
    ///
    /// Exactly the four features with a `FrameMemory` record.
    /// `CellKind::Fault` and `CellKind::Corruption` are absent on purpose:
    /// neither is used up, so neither has a record and "unspent" is not a
    /// question that can be asked about them — and listing every fault in
    /// the frame would drown the reading in terrain.
    fn nearest_unspent(&self, pos: StackPos) -> Option<(i32, i32)> {
        let level = self.world.resource::<CurrentStack>().0.as_ref()?;
        (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .filter(|&cell| match level.cell(cell.0, cell.1) {
                CellKind::Cache => self.cache_unopened(pos, cell),
                CellKind::SealedDoor => !self.seal_open(pos, cell),
                CellKind::Orphan => self.orphan_present(pos, cell),
                CellKind::Lair => !self.lair_cleared(pos),
                _ => false,
            })
            .min_by_key(|&(x, y)| (x - pos.x).abs() + (y - pos.y).abs())
    }
}

/// Where `target` lies relative to the way the party is looking, named by
/// whichever of the two relative axes dominates — ties going to
/// forward/back, which is the axis the party can act on without turning.
///
/// The rotation is the one place here a sign error would be invisible, so
/// it is done by dotting the offset against the party's own two axes:
/// `Dir::delta` is the way they are looking and `Dir::right_delta` is their
/// right hand. Reading a bearing off compass north instead would be the
/// same mistake with a plausible-looking answer.
///
/// `pub(crate)` because the description bank's `sighted` lines and examine
/// paragraphs need the same answer. A second copy of this rotation is
/// exactly the drift the doc comment above warns about.
pub(crate) fn relative_bearing(pos: StackPos, target: (i32, i32)) -> &'static str {
    let (dx, dy) = (target.0 - pos.x, target.1 - pos.y);
    let (fx, fy) = pos.facing.delta();
    let (rx, ry) = pos.facing.right_delta();
    let forward = dx * fx + dy * fy;
    let right = dx * rx + dy * ry;
    if forward.abs() >= right.abs() {
        if forward > 0 { "ahead" } else { "behind" }
    } else if right > 0 {
        "to your right"
    } else {
        "to your left"
    }
}
