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

impl Game {
    /// Records everything the party can see from where they are standing.
    ///
    /// Called from every place that moves the party or turns them, plus the
    /// load path — anywhere the view changes, the map has to change with it,
    /// or the player is told they never looked down a corridor they are
    /// currently staring at.
    pub(crate) fn remember_view(&mut self) {
        let Some(pos) = self.stack_pos() else {
            return;
        };
        let Some(level) = self.world.resource::<CurrentStack>().0.clone() else {
            return;
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
        memory.seen.extend(seen);
    }

    /// The party's map of the frame they are in — see
    /// `views::FrameMapView`. `None` on the surface.
    ///
    /// Drawn from `StackMemory` rather than from the frame, so it shows
    /// what has been seen and not what is there. The frame is consulted only
    /// to say what each *remembered* cell holds.
    pub fn frame_map(&self) -> Option<FrameMapView> {
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
                        if !seen.contains(&(x, y)) {
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
                    })
                    .collect()
            })
            .collect();

        let standing_on = match level.cell(x, y) {
            CellKind::LinkDown => Some("A link leads down  [>] descend".to_string()),
            CellKind::LinkUp if depth == 1 => Some("The link out  [<] surface".to_string()),
            CellKind::LinkUp => Some("A link leads up  [<] climb".to_string()),
            // Emptied on arrival rather than on a key, so this reports what
            // already happened rather than offering a choice.
            CellKind::Cache => Some("An empty casing".to_string()),
            CellKind::Lair => Some("The lair, and nothing left holding it".to_string()),
            CellKind::Door | CellKind::SealedDoor => Some("A doorway".to_string()),
            _ => None,
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
