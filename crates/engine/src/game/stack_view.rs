//! The two views of a Stack frame, and the record of what they showed.
//!
//! `view_cone` is the single walk both are built from, which is why it and
//! both its consumers live here and it stays private to this file: the map
//! cannot mark a cell the first-person view never showed, and it cannot do
//! so by construction rather than by agreement.
//!
//! Sight stops at `CellKind::blocks_sight` — rock, and any door, since a
//! shut door is the point of a door. Never at `ahead == 0`: that row is the
//! cell the party is standing in, and a cell cannot hide the party from
//! their own surroundings. A door is both walkable and sight-blocking, so
//! standing inside an occluder is reachable and both consumers carry that
//! exception explicitly.

use super::stack::StackPos;
use crate::stack::{CellKind, Dir};
use crate::*;

/// How far ahead the first-person view reaches, in cells. Four is enough
/// corridor to read a junction coming without the far wall shrinking to
/// nothing.
pub const STACK_VIEW_DEPTH: usize = 4;

/// Cells visible either side of the party's line of sight. One gives the
/// three-wide cone a classic blobber shows — the corridor you are in, plus
/// whatever opens off it.
pub const STACK_VIEW_HALF_WIDTH: usize = 1;

/// Whether `FERAL_DEV_REVEAL` is set in the environment — a development
/// switch that draws the whole frame on the map instead of what the party
/// has walked, for finding a fault or an orphan without walking a maze
/// first.
///
/// Read once and never written, so it is configuration rather than global
/// state: it cannot change during a run and nothing can toggle it. It is
/// deliberately *not* a `Game` field threaded from the launcher — every
/// path that builds a `Game` (new run, load, dev template) would have to
/// remember to set it, and one that forgot would look like the switch was
/// broken.
///
/// The map only. The first-person view, the encounter rolls and everything
/// the party can *do* are untouched, so a session with this on is still the
/// real game with the lights on.
fn dev_reveal() -> bool {
    static ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        std::env::var_os("FERAL_DEV_REVEAL").is_some_and(|v| !v.is_empty() && v != "0")
    });
    *ON
}

/// The world coordinates of the view cone from `(x, y)` facing `facing`,
/// indexed `[ahead][lateral]` — the same shape and order
/// `views::StackView::cells` carries.
///
/// The first-person view and the map's record of what has been seen are both
/// filled by walking this, so the map cannot mark a cell the view never
/// showed and the view cannot show one the map won't remember.
fn view_cone(x: i32, y: i32, facing: Dir) -> Vec<Vec<(i32, i32)>> {
    let (fx, fy) = facing.delta();
    let (rx, ry) = facing.right_delta();
    let span = STACK_VIEW_HALF_WIDTH as i32;

    (0..STACK_VIEW_DEPTH as i32)
        .map(|ahead| {
            (-span..=span)
                .map(|lateral| (x + fx * ahead + rx * lateral, y + fy * ahead + ry * lateral))
                .collect()
        })
        .collect()
}

/// The key-prompt suffix `standing_on` appends after a cell's descriptive
/// clause, keyed by the same `(subject, condition)` pair
/// `Game::subject_of` resolves a cell to.
///
/// This is the single home for those strings: `Game::stack_view`'s match
/// below reads it to build the row, and
/// `tests::descriptions::every_shipped_underfoot_line_fits_the_standing_on_row`
/// reads it to size the per-subject budget it holds the shipped bank to. A
/// subject/condition pair absent from this table appends nothing, which is
/// every arm that reports rather than offers.
const UNDERFOOT_SUFFIXES: &[(&str, Option<&str>, &str)] = &[
    ("stack.link_down", None, "  [>] descend"),
    ("stack.link_up", Some("surface"), "  [<] surface"),
    ("stack.link_up", None, "  [<] climb"),
    ("stack.orphan", None, "  [o] adopt"),
    ("stack.corruption", None, "  — moving on costs"),
];

/// Looks up `UNDERFOOT_SUFFIXES`, or `""` for a subject/condition pair that
/// appends nothing.
pub(crate) fn underfoot_suffix(subject: &str, condition: Option<&str>) -> &'static str {
    UNDERFOOT_SUFFIXES
        .iter()
        .find(|&&(s, c, _)| s == subject && c == condition)
        .map_or("", |&(_, _, suffix)| suffix)
}

impl Game {
    /// Records everything the party can see from where they are standing,
    /// and announces the most notable thing that just came into view.
    ///
    /// Called from every place that moves the party or turns them — anywhere
    /// the view changes, the map has to change with it, or the player is
    /// told they never looked down a corridor they are currently staring at.
    ///
    /// **The load path calls `remember_view_silent` instead.**
    /// `restore_locale` runs the same walk, and a save reloading into a
    /// corridor would replay sightings the player already read a session
    /// ago. One site, pinned by `tests::descriptions::loading_a_save_announces_no_sightings`.
    pub(crate) fn remember_view(&mut self) {
        let newly_seen = self.remember_view_silent();
        self.announce_sighting(&newly_seen);
    }

    /// The view walk itself, returning the cells that were not on the map
    /// before this call.
    ///
    /// The diff is free: `FrameMemory::seen` is consulted before the
    /// `extend`, so nothing new is stored to support it and the save format
    /// does not move.
    pub(crate) fn remember_view_silent(&mut self) -> Vec<(i32, i32)> {
        let Some(pos) = self.stack_pos() else {
            return Vec::new();
        };
        let Some(level) = self.world.resource::<CurrentStack>().0.clone() else {
            return Vec::new();
        };

        let mut seen = Vec::new();
        for (ahead, row) in view_cone(pos.x, pos.y, pos.facing).into_iter().enumerate() {
            // The party's own cell can never stop their view out of it. That
            // is not hypothetical: a door both blocks sight and is walkable —
            // the only cell that is both — so standing in a doorway would
            // otherwise blind the party to the corridor they are standing in.
            //
            // The wall that stops the view is itself in plain sight, so the
            // row is recorded before the break, not after the check.
            let blocked = ahead > 0
                && row
                    .get(STACK_VIEW_HALF_WIDTH)
                    .is_some_and(|&(cx, cy)| level.cell(cx, cy).blocks_sight());
            seen.extend(row);
            if blocked {
                break;
            }
        }

        let memory = self.frame_memory_mut(pos);
        let newly_seen: Vec<(i32, i32)> = seen
            .iter()
            .copied()
            .filter(|cell| !memory.seen.contains(cell))
            .collect();
        memory.seen.extend(seen);
        newly_seen
    }

    /// Logs one line for the most notable cell that just came into view, or
    /// nothing when none of them was worth a line.
    ///
    /// **Capped at one.** A corridor opening onto four features must not
    /// push four rows into a pane that shows a handful — and the one row it
    /// does push should be the thing the player would actually walk to.
    ///
    /// `notability`'s ranks are not a total order — an unspent cache and an
    /// unspent breakpoint both rank 3, `LinkDown` and a sealed door both rank
    /// 2. `newly_seen` is already a `Vec` walked off `view_cone` in a fixed
    /// order, so `max_by_key` on rank alone would still be deterministic —
    /// but it resolves ties by picking whichever tied cell happens to come
    /// *last* in that scan order, which is an accident of the view cone's
    /// layout, not the cell nearest the party. Breaking ties on Manhattan
    /// distance (nearest first) makes the winner the tied cell closest to
    /// where the player is standing — the one they would actually walk to —
    /// and the final `Reverse(cell)` only matters for the rarer case of two
    /// notable cells tied on both rank and distance, where it keeps the pick
    /// a pure function of coordinates rather than of scan order.
    ///
    /// Falls through to the next-best candidate if the winner's own line is
    /// missing from the bank (an empty variant pool, or a deleted asset
    /// directory) rather than saying nothing: a lower-ranked cell with a
    /// line to offer is still more useful than silence, and the cap still
    /// holds because at most one candidate's line is ever logged.
    fn announce_sighting(&mut self, newly_seen: &[(i32, i32)]) {
        let Some(pos) = self.stack_pos() else {
            return;
        };
        let mut candidates: Vec<(u8, (i32, i32))> = newly_seen
            .iter()
            .filter(|&&cell| cell != (pos.x, pos.y))
            .filter_map(|&cell| self.notability(pos, cell).map(|rank| (rank, cell)))
            .collect();
        candidates.sort_by_key(|&(rank, cell)| {
            let steps = (cell.0 - pos.x).abs() + (cell.1 - pos.y).abs();
            std::cmp::Reverse((rank, std::cmp::Reverse(steps), std::cmp::Reverse(cell)))
        });
        if let Some(line) = candidates
            .into_iter()
            .find_map(|(_, cell)| self.sighted_description(pos, cell))
        {
            self.log(line);
        }
    }

    /// The party's map of the frame they are in — see
    /// `views::FrameMapView`. `None` on the surface.
    ///
    /// Drawn from `StackMemory` rather than from the frame, so it shows
    /// what has been seen and not what is there. The frame is consulted only
    /// to say what each *remembered* cell holds.
    pub fn frame_map(&self) -> Option<FrameMapView> {
        self.frame_map_revealed(dev_reveal())
    }

    /// The map the party would have if they had seen `revealed` of the
    /// frame — everything, or only what they walked.
    ///
    /// Split from `frame_map` so the reveal is testable without touching
    /// the environment: `std::env::set_var` is process-wide and unsafe, and
    /// this suite runs in parallel, so a test that set the variable would
    /// reach every other test in the binary.
    pub(crate) fn frame_map_revealed(&self, revealed: bool) -> Option<FrameMapView> {
        let pos = self.stack_pos()?;
        let level = self.world.resource::<CurrentStack>().0.as_ref()?;
        let memory = self.world.resource::<StackMemory>();
        let seen = memory
            .0
            .get(&(pos.entrance, pos.depth))
            .map(|m| &m.seen)
            .cloned()
            .unwrap_or_default();

        let cells = (0..level.height)
            .map(|y| {
                (0..level.width)
                    .map(|x| {
                        if !revealed && !seen.contains(&(x, y)) {
                            return FrameMapCell::Unknown;
                        }
                        match level.cell(x, y) {
                            CellKind::Rock => FrameMapCell::Rock,
                            CellKind::Floor => FrameMapCell::Floor,
                            CellKind::LinkUp => FrameMapCell::LinkUp,
                            CellKind::LinkDown => FrameMapCell::LinkDown,
                            CellKind::Cache if self.cache_unopened(pos, (x, y)) => {
                                FrameMapCell::Cache
                            }
                            CellKind::Cache => FrameMapCell::Floor,
                            CellKind::Lair if !self.lair_cleared(pos) => FrameMapCell::Lair,
                            CellKind::Lair => FrameMapCell::Floor,
                            CellKind::Door => FrameMapCell::Door,
                            CellKind::SealedDoor if self.seal_open(pos, (x, y)) => {
                                FrameMapCell::Door
                            }
                            CellKind::SealedDoor => FrameMapCell::SealedDoor,
                            CellKind::Breakpoint if !self.breakpoint_spent(pos, (x, y)) => {
                                FrameMapCell::Breakpoint
                            }
                            CellKind::Breakpoint => FrameMapCell::Floor,
                            CellKind::Fault => FrameMapCell::Fault,
                            CellKind::Corruption => FrameMapCell::Corruption,
                            CellKind::Orphan if self.orphan_present(pos, (x, y)) => {
                                FrameMapCell::Orphan
                            }
                            CellKind::Orphan => FrameMapCell::Floor,
                        }
                    })
                    .collect()
            })
            .collect();

        let mut marks: Vec<((i32, i32), FrameMapMark)> = memory
            .0
            .get(&(pos.entrance, pos.depth))
            .map(|m| m.fights.iter().map(|&c| (c, FrameMapMark::Fight)).collect())
            .unwrap_or_default();
        // Last, so a fight marker on the party's own cell doesn't hide them.
        marks.push(((pos.x, pos.y), FrameMapMark::Party));

        let walkable = (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .filter(|&(x, y)| level.walkable(x, y))
            .count();
        let walked = seen.iter().filter(|&&(x, y)| level.walkable(x, y)).count();

        Some(FrameMapView {
            depth: pos.depth,
            frames: pos.frames,
            width: level.width,
            height: level.height,
            cells,
            marks,
            facing: pos.facing.label(),
            entrance: pos.entrance,
            explored: if walkable == 0 {
                0.0
            } else {
                walked as f32 / walkable as f32
            },
            revealed,
        })
    }

    /// The first-person view of the cells around the party, already rotated
    /// into view space — see `views::StackView`. `None` on the surface.
    pub fn stack_view(&self) -> Option<StackView> {
        let pos = self.stack_pos()?;
        let StackPos {
            depth,
            frames,
            x,
            y,
            facing,
            ..
        } = pos;
        let level = self.world.resource::<CurrentStack>().0.as_ref()?;

        let cells = view_cone(x, y, facing)
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|(cx, cy)| match level.cell(cx, cy) {
                        CellKind::Rock => StackCellView::Rock,
                        CellKind::Floor => StackCellView::Floor,
                        CellKind::LinkUp => StackCellView::LinkUp,
                        CellKind::LinkDown => StackCellView::LinkDown,
                        // An emptied cache is just an alcove. Still drawing
                        // one would send the player back down a dead end
                        // they have already walked.
                        CellKind::Cache if self.cache_unopened(pos, (cx, cy)) => {
                            StackCellView::Cache
                        }
                        CellKind::Cache => StackCellView::Floor,
                        CellKind::Lair if !self.lair_cleared(pos) => StackCellView::Lair,
                        CellKind::Lair => StackCellView::Floor,
                        CellKind::Door => StackCellView::Door,
                        CellKind::SealedDoor if self.seal_open(pos, (cx, cy)) => {
                            StackCellView::Door
                        }
                        CellKind::SealedDoor => StackCellView::SealedDoor,
                        // A spent port is dead hardware. Still drawing it
                        // would send the party back to a cell with nothing
                        // left in it, exactly as an emptied cache would.
                        CellKind::Breakpoint if !self.breakpoint_spent(pos, (cx, cy)) => {
                            StackCellView::Breakpoint
                        }
                        CellKind::Breakpoint => StackCellView::Floor,
                        CellKind::Fault => StackCellView::Fault,
                        CellKind::Corruption => StackCellView::Corruption,
                        // An adopted orphan is an empty dead end. Still
                        // drawing it would send the party back down a
                        // corridor with nothing at the end of it, exactly
                        // as an emptied cache would.
                        CellKind::Orphan if self.orphan_present(pos, (cx, cy)) => {
                            StackCellView::Orphan
                        }
                        CellKind::Orphan => StackCellView::Floor,
                    })
                    .collect()
            })
            .collect();

        // Each arm keeps its key-prompt suffix verbatim (via
        // `underfoot_suffix`) and draws only its *descriptive clause* from
        // the bank, falling back to the literal this row shipped with. The
        // `None` arms stay `None`: those are cells with nothing to offer,
        // not cells with nothing to say, and two tests in `tests/stack.rs`
        // pin the difference.
        let described = |fallback: &str| {
            self.underfoot_description(pos)
                .unwrap_or_else(|| fallback.to_string())
        };
        let standing_on = match level.cell(x, y) {
            CellKind::LinkDown => Some(format!(
                "{}{}",
                described("A link leads down"),
                underfoot_suffix("stack.link_down", None)
            )),
            CellKind::LinkUp if depth == 1 => Some(format!(
                "{}{}",
                described("The link out"),
                underfoot_suffix("stack.link_up", Some("surface"))
            )),
            CellKind::LinkUp => Some(format!(
                "{}{}",
                described("A link leads up"),
                underfoot_suffix("stack.link_up", None)
            )),
            // Emptied on arrival rather than on a key, so this reports what
            // already happened rather than offering a choice.
            CellKind::Cache => Some(described("An empty casing")),
            CellKind::Lair => Some(described("The lair, and nothing left holding it")),
            CellKind::Door | CellKind::SealedDoor => Some(described("A doorway")),
            // Like the cache above, these report rather than offer: all three
            // fire on arrival, so by the time this line is read the port is
            // spent and the substrate has already bitten. A fault never
            // appears here at all — the party is in the frame below before
            // the view is next built.
            CellKind::Breakpoint => Some(described("A burnt-out debug port")),
            CellKind::Corruption => Some(format!(
                "{}{}",
                described("Rotten substrate"),
                underfoot_suffix("stack.corruption", None)
            )),
            // The one line here that offers rather than reports. Everything
            // else underfoot has already happened by the time this is read;
            // an orphan costs a catalyst, so it waits for the key — and
            // stops offering once it has been taken.
            CellKind::Orphan if self.orphan_present(pos, (x, y)) => Some(format!(
                "{}{}",
                described("An orphaned process"),
                underfoot_suffix("stack.orphan", None)
            )),
            CellKind::Orphan => None,
            CellKind::Rock | CellKind::Floor | CellKind::Fault => None,
        };

        Some(StackView {
            depth,
            frames,
            facing: facing.label(),
            trace: self.trace_band().label(),
            position: (x, y),
            cells,
            standing_on,
        })
    }
}
