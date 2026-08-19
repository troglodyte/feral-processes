//! The two field routines that move the party through a Stack frame rather
//! than around it — `AbilityEffect::Phase` and `AbilityEffect::Jump`.
//!
//! Everything a frame offers the player otherwise either *reads* the maze
//! (the view cone, the map, a breakpoint) or is *inflicted* by it (a fault,
//! corrupted ground). These two act on it. The design argument is in
//! `docs/superpowers/archive/specs/2026-08-05-stack-movement-routines-design.md`.
//!
//! Both are cast through `Game::cast_field_routine`, which owns the ordering
//! rule they share with every other field routine: every refusal lands
//! before the first write, and the clock ticks only on success.

use std::collections::{HashSet, VecDeque};

use super::stack::StackPos;
use crate::resources::{CurrentStack, Locale};
use crate::stack::CellKind;
use crate::*;

impl Game {
    /// Where a `Phase` cast from `pos` would put the party, or why it is
    /// refused.
    ///
    /// Exactly one wall thick, and that is a rule of the mechanic rather
    /// than an authored number: two deep, a cast from the frame edge cuts a
    /// diagonal across the whole maze, where at one wall it opens the room
    /// next door and nothing further.
    ///
    /// Facing is not consulted for anything but the direction — phasing
    /// leaves the party facing the way they already were, so the wall they
    /// just crossed is behind them and the corridor they landed in is ahead.
    pub(crate) fn phase_landing(&self, pos: StackPos) -> Result<(i32, i32), String> {
        let level = self
            .world
            .resource::<CurrentStack>()
            .0
            .as_ref()
            .ok_or_else(|| "There's nothing here to phase through.".to_string())?;
        let (dx, dy) = pos.facing.delta();
        let wall = (pos.x + dx, pos.y + dy);
        let landing = (pos.x + dx * 2, pos.y + dy * 2);

        if level.cell(wall.0, wall.1).walkable() {
            return Err("Nothing solid ahead — just walk.".into());
        }
        // Checked before walkability rather than folded into it: an
        // out-of-bounds read comes back as `CellKind::Rock`, so without this
        // the frame edge would report itself as more rock behind the wall,
        // which is true of the void and useless to the player.
        if !in_bounds(level, landing) {
            return Err("That's the edge of the frame. There's nothing on the other side.".into());
        }
        if !level.cell(landing.0, landing.1).walkable() {
            return Err("The rock runs deeper than one wall here.".into());
        }
        self.refuse_sealed_landing(pos, landing)?;
        Ok(landing)
    }

    /// Refuses a landing the party has not earned their way to: one behind
    /// an unopened seal, or one **on** an unopened `SealedDoor`.
    ///
    /// The second is not redundant. A sealed door is `walkable()` — the
    /// generator has to see through it — and `Game::step` only calls
    /// `force_seal` when the cell being *stepped into* is sealed. Landing on
    /// the door itself would therefore put the party past a seal that was
    /// never forced, with the next ordinary step into the wing never
    /// touching it: landing *on* the door is the bypass, not landing past
    /// it.
    ///
    /// A refusal rather than a death. The rule exists to keep "the lair is
    /// entered through its door" intact, not to punish a misclick, and an
    /// accident that costs nothing beats one that costs a run.
    ///
    /// The unreachable set is a flood fill, so in principle it also catches
    /// a walkable cell sealed off by plain rock — but `stack::generate`
    /// produces a fully connected frame, so an unopened seal is the only
    /// thing that ever puts a cell outside it, and the refusal says so.
    fn refuse_sealed_landing(&self, pos: StackPos, cell: (i32, i32)) -> Result<(), String> {
        if self.cell_is_shut_seal(pos, cell) {
            return Err("That's the seal itself. It doesn't want you standing in it.".into());
        }
        if !self.reachable_without_forcing_a_seal(pos).contains(&cell) {
            return Err("Sealed off. Whatever is back there is reached through the door.".into());
        }
        Ok(())
    }

    /// Whether `cell` holds a `SealedDoor` that has not been forced open —
    /// the frame's own layout crossed with `FrameMemory::opened`, which is
    /// the same pairing `frame_map_revealed` draws a seal from.
    fn cell_is_shut_seal(&self, pos: StackPos, cell: (i32, i32)) -> bool {
        self.world
            .resource::<CurrentStack>()
            .0
            .as_ref()
            .is_some_and(|level| level.cell(cell.0, cell.1) == CellKind::SealedDoor)
            && !self.seal_open(pos, cell)
    }

    /// Every cell the party could have walked to without forcing a seal: a
    /// flood fill from the frame's entry over walkable cells, with an
    /// unopened `SealedDoor` treated as blocking.
    ///
    /// Reads `FrameMemory::opened` through `seal_open`, so a seal the player
    /// has already shouldered open stops excluding anything behind it — the wing
    /// is theirs from then on, exactly as it is on foot.
    fn reachable_without_forcing_a_seal(&self, pos: StackPos) -> HashSet<(i32, i32)> {
        let mut seen = HashSet::new();
        let Some(level) = self.world.resource::<CurrentStack>().0.as_ref() else {
            return seen;
        };
        let mut queue = VecDeque::from([level.entry]);
        seen.insert(level.entry);
        while let Some((x, y)) = queue.pop_front() {
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let next = (x + dx, y + dy);
                if !in_bounds(level, next) || seen.contains(&next) {
                    continue;
                }
                if !level.cell(next.0, next.1).walkable() || self.cell_is_shut_seal(pos, next) {
                    continue;
                }
                seen.insert(next);
                queue.push_back(next);
            }
        }
        seen
    }

    /// Refuses a `Jump` at `cell` for everything except the one thing that
    /// is supposed to kill you.
    ///
    /// Solid ground is deliberately **not** refused here — a landing nobody
    /// validated is the whole mechanic, and cells the party has already seen
    /// are safe *because* they have been seen. What is refused is the frame's
    /// bounds (unreachable in practice, since the cursor is clamped to the
    /// grid, but the engine does not take the picker's word for it) and the
    /// sealed wing, and the sealed check runs only against a landing that is
    /// walkable at all: rock inside a sealed wing is rock, and rock kills.
    pub(crate) fn jump_refusal(&self, pos: StackPos, cell: (i32, i32)) -> Result<(), String> {
        let level = self
            .world
            .resource::<CurrentStack>()
            .0
            .as_ref()
            .ok_or_else(|| "There's nowhere to jump to.".to_string())?;
        if !in_bounds(level, cell) {
            return Err("That's outside the frame entirely.".into());
        }
        if level.cell(cell.0, cell.1).walkable() {
            self.refuse_sealed_landing(pos, cell)?;
        }
        Ok(())
    }

    /// Whether a `Jump` at `cell` would put the party inside solid substrate
    /// — which kills them. Callers have already cleared `jump_refusal`.
    pub(crate) fn jump_is_lethal(&self, cell: (i32, i32)) -> bool {
        !self
            .world
            .resource::<CurrentStack>()
            .0
            .as_ref()
            .is_some_and(|level| level.cell(cell.0, cell.1).walkable())
    }

    /// Kills the player for materialising inside rock.
    ///
    /// Goes through `Game::apply_damage` — the one path in the game that
    /// lowers a creature's HP — so anything that must see all damage sees
    /// this too, and what happens next is already built: on Forgiving,
    /// `difficulty::death_handling_system` warps the party out through
    /// `stack::surfaced`; on permadeath the run ends.
    ///
    /// **`Locale` is never written.** The party does not briefly stand in
    /// the rock and get rescued; they never move at all. That matters beyond
    /// tidiness: rock is the one `CellKind` that is both unwalkable and
    /// sight-blocking, so a party inside one is exactly the occluder trap
    /// doors sprang — a first-person view full of flat wall and a map
    /// truncated to the party's own row. Not writing `Locale` means no state
    /// exists in which that is reachable, so neither `view_cone` consumer
    /// needs a new exception.
    pub(crate) fn die_in_the_rock(&mut self) {
        let player = self.player_entity();
        self.log_kind(
            MessageKind::Outcome,
            "You resolve inside solid substrate. Nothing about that is survivable.",
        );
        // `kill_outright`, not `apply_damage`: mitigation would leave the
        // player standing on a point of HP, and the line above has already
        // promised otherwise.
        self.kill_outright(player);
    }

    /// Puts the party on `cell` of the frame they are already in, and
    /// remembers what that cell can see.
    ///
    /// The only place `Locale::Stack`'s coordinates are written outside
    /// `Game::step` (a walk) and `Game::enter_frame` (a change of frame).
    /// `remember_view` before the caller's `arrive`, for the reason `step`
    /// puts it there: a corridor the party moved into belongs on their map
    /// even if something jumps them the moment they enter it.
    ///
    /// Facing is left alone by both routines — you come out of a wall
    /// pointing the way you went in, and a jump is a change of address
    /// rather than of heading.
    pub(crate) fn relocate_within_frame(&mut self, cell: (i32, i32)) {
        if let Locale::Stack { x, y, .. } = &mut *self.world.resource_mut::<Locale>() {
            *x = cell.0;
            *y = cell.1;
        }
        self.remember_view();
    }
}

fn in_bounds(level: &crate::stack::Frame, (x, y): (i32, i32)) -> bool {
    x >= 0 && y >= 0 && x < level.width && y < level.height
}
