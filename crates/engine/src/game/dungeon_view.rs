//! The two views of a dungeon level, and the record of what they showed.
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

use super::dungeon::DungeonPos;
use crate::dungeon::{CellKind, Dir};
use crate::*;

/// How far ahead the first-person view reaches, in cells. Four is enough
/// corridor to read a junction coming without the far wall shrinking to
/// nothing.
pub const DUNGEON_VIEW_DEPTH: usize = 4;

/// Cells visible either side of the party's line of sight. One gives the
/// three-wide cone a classic blobber shows — the corridor you are in, plus
/// whatever opens off it.
pub const DUNGEON_VIEW_HALF_WIDTH: usize = 1;

/// The world coordinates of the view cone from `(x, y)` facing `facing`,
/// indexed `[ahead][lateral]` — the same shape and order
/// `views::DungeonView::cells` carries.
///
/// The first-person view and the map's record of what has been seen are both
/// filled by walking this, so the map cannot mark a cell the view never
/// showed and the view cannot show one the map won't remember.
fn view_cone(x: i32, y: i32, facing: Dir) -> Vec<Vec<(i32, i32)>> {
    let (fx, fy) = facing.delta();
    let (rx, ry) = facing.right_delta();
    let span = DUNGEON_VIEW_HALF_WIDTH as i32;

    (0..DUNGEON_VIEW_DEPTH as i32)
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
        let Some(pos) = self.dungeon_pos() else {
            return;
        };
        let Some(level) = self.world.resource::<CurrentDungeon>().0.clone() else {
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
                    .get(DUNGEON_VIEW_HALF_WIDTH)
                    .is_some_and(|&(cx, cy)| level.cell(cx, cy).blocks_sight());
            seen.extend(row);
            if blocked {
                break;
            }
        }

        let memory = self.level_memory_mut(pos);
        memory.seen.extend(seen);
    }

    /// The party's map of the level they are in — see
    /// `views::DungeonMapView`. `None` on the surface.
    ///
    /// Drawn from `DungeonMemory` rather than from the level, so it shows
    /// what has been seen and not what is there. The level is consulted only
    /// to say what each *remembered* cell holds.
    pub fn dungeon_map(&self) -> Option<DungeonMapView> {
        let pos = self.dungeon_pos()?;
        let level = self.world.resource::<CurrentDungeon>().0.as_ref()?;
        let memory = self.world.resource::<DungeonMemory>();
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
                            return DungeonMapCell::Unknown;
                        }
                        match level.cell(x, y) {
                            CellKind::Rock => DungeonMapCell::Rock,
                            CellKind::Floor => DungeonMapCell::Floor,
                            CellKind::LinkUp => DungeonMapCell::LinkUp,
                            CellKind::LinkDown => DungeonMapCell::LinkDown,
                            CellKind::Cache if self.cache_unopened(pos, (x, y)) => {
                                DungeonMapCell::Cache
                            }
                            CellKind::Cache => DungeonMapCell::Floor,
                            CellKind::Lair if !self.lair_cleared(pos) => DungeonMapCell::Lair,
                            CellKind::Lair => DungeonMapCell::Floor,
                            CellKind::Door => DungeonMapCell::Door,
                            CellKind::SealedDoor if self.seal_open(pos, (x, y)) => {
                                DungeonMapCell::Door
                            }
                            CellKind::SealedDoor => DungeonMapCell::SealedDoor,
                        }
                    })
                    .collect()
            })
            .collect();

        let mut marks: Vec<((i32, i32), DungeonMapMark)> = memory
            .0
            .get(&(pos.entrance, pos.depth))
            .map(|m| {
                m.fights
                    .iter()
                    .map(|&c| (c, DungeonMapMark::Fight))
                    .collect()
            })
            .unwrap_or_default();
        // Last, so a fight marker on the party's own cell doesn't hide them.
        marks.push(((pos.x, pos.y), DungeonMapMark::Party));

        let walkable = (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .filter(|&(x, y)| level.walkable(x, y))
            .count();
        let walked = seen.iter().filter(|&&(x, y)| level.walkable(x, y)).count();

        Some(DungeonMapView {
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
    /// into view space — see `views::DungeonView`. `None` on the surface.
    pub fn dungeon_view(&self) -> Option<DungeonView> {
        let pos = self.dungeon_pos()?;
        let DungeonPos {
            depth,
            frames,
            x,
            y,
            facing,
            ..
        } = pos;
        let level = self.world.resource::<CurrentDungeon>().0.as_ref()?;

        let cells = view_cone(x, y, facing)
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|(cx, cy)| match level.cell(cx, cy) {
                        CellKind::Rock => DungeonCellView::Rock,
                        CellKind::Floor => DungeonCellView::Floor,
                        CellKind::LinkUp => DungeonCellView::LinkUp,
                        CellKind::LinkDown => DungeonCellView::LinkDown,
                        // An emptied cache is just an alcove. Still drawing
                        // one would send the player back down a dead end
                        // they have already walked.
                        CellKind::Cache if self.cache_unopened(pos, (cx, cy)) => {
                            DungeonCellView::Cache
                        }
                        CellKind::Cache => DungeonCellView::Floor,
                        CellKind::Lair if !self.lair_cleared(pos) => DungeonCellView::Lair,
                        CellKind::Lair => DungeonCellView::Floor,
                        CellKind::Door => DungeonCellView::Door,
                        CellKind::SealedDoor if self.seal_open(pos, (cx, cy)) => {
                            DungeonCellView::Door
                        }
                        CellKind::SealedDoor => DungeonCellView::SealedDoor,
                    })
                    .collect()
            })
            .collect();

        let standing_on = match level.cell(x, y) {
            CellKind::LinkDown => Some("A link leads down  [>] descend".to_string()),
            CellKind::LinkUp if depth == 1 => Some("The breach out  [<] surface".to_string()),
            CellKind::LinkUp => Some("A link leads up  [<] climb".to_string()),
            // Emptied on arrival rather than on a key, so this reports what
            // already happened rather than offering a choice.
            CellKind::Cache => Some("An empty casing".to_string()),
            CellKind::Lair => Some("The lair, and nothing left holding it".to_string()),
            CellKind::Door | CellKind::SealedDoor => Some("A doorway".to_string()),
            _ => None,
        };

        Some(DungeonView {
            depth,
            frames,
            facing: facing.label(),
            position: (x, y),
            cells,
            standing_on,
        })
    }
}
