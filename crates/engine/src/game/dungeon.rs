//! Walking a dungeon: getting in, moving around inside, and getting back
//! out.
//!
//! Everything here operates on `resources::Locale` and leaves the player's
//! `Position` component alone — see that resource's docs for why. The one
//! exception is `enter_dungeon`, which pins `Position` to the entrance tile
//! on the way in.

use crate::dungeon::{self, CellKind, Dir};
use crate::resources::{CurrentDungeon, Locale};
use crate::*;

/// How far ahead the first-person view reaches, in cells. Four is enough
/// corridor to read a junction coming without the far wall shrinking to
/// nothing.
pub const DUNGEON_VIEW_DEPTH: usize = 4;

/// Cells visible either side of the party's line of sight. One gives the
/// three-wide cone a classic blobber shows — the corridor you are in, plus
/// whatever opens off it.
pub const DUNGEON_VIEW_HALF_WIDTH: usize = 1;

impl Game {
    /// Finds a `DungeonEntrance` at `(x, y)`, if any — checked in
    /// `move_player` before the generic blocking-structure check, so walking
    /// onto one descends instead of just bumping into it.
    pub(crate) fn find_dungeon_entrance_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<DungeonEntrance>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    pub fn is_underground(&self) -> bool {
        matches!(self.world.resource::<Locale>(), Locale::Dungeon { .. })
    }

    /// `Err` if the party is underground — for actions that reach into the
    /// zone map through the player's `Position`.
    ///
    /// While underground that `Position` is pinned to the dungeon entrance
    /// (see `resources::Locale`), so these would otherwise operate on a tile
    /// out in the wild that the player is nowhere near: deploying a
    /// structure beside the breach, or — worst of all — `use_symlink`
    /// teleporting the pinned entrance somewhere else and changing where
    /// climbing back up puts you.
    ///
    /// Party and inventory management deliberately isn't on this list.
    /// Fusing programs, installing routines, crafting, equipping and
    /// spending perk points all work fine four levels down, and stopping to
    /// sort your gear in a dungeon is a thing the genre expects.
    pub(crate) fn require_surface(&self) -> Result<(), String> {
        if self.is_underground() {
            return Err("Not down here — that needs open grid.".into());
        }
        Ok(())
    }

    /// Descends into the dungeon reached through the entrance standing at
    /// `(x, y)`, which the player is stepping onto. The entrance itself
    /// survives — unlike a zone portal, it is a place you can come back to.
    pub(crate) fn enter_dungeon(&mut self, x: i32, y: i32) {
        let player = self.player_entity();
        if let Some(mut pos) = self.world.get_mut::<Position>(player) {
            pos.x = x;
            pos.y = y;
        }
        self.descend_to(1, (x, y));
        self.log("You drop through the breach. The signal above you thins to nothing.".to_string());
    }

    /// Generates the level for `depth` and puts the party on its entry cell
    /// facing north. Shared by the way in and every flight of stairs down.
    fn descend_to(&mut self, depth: u32, entrance: (i32, i32)) {
        let seed = self.world.resource::<WorldMap>().seed();
        let level = dungeon::generate(seed, depth);
        let entry = level.entry;
        self.world.insert_resource(CurrentDungeon(Some(level)));
        self.world.insert_resource(Locale::Dungeon {
            depth,
            x: entry.0,
            y: entry.1,
            facing: Dir::North,
            entrance,
        });
    }

    /// Climbs out to the zone map. The player's `Position` was pinned to the
    /// entrance tile the whole time, so there is nothing to restore.
    fn leave_dungeon(&mut self) {
        self.world.insert_resource(Locale::Surface);
        self.world.insert_resource(CurrentDungeon(None));
        self.log("You surface through the breach, back onto open grid.".to_string());
    }

    /// The party's current cell and facing, or `None` on the surface.
    fn dungeon_pos(&self) -> Option<(u32, i32, i32, Dir, (i32, i32))> {
        match *self.world.resource::<Locale>() {
            Locale::Surface => None,
            Locale::Dungeon {
                depth,
                x,
                y,
                facing,
                entrance,
            } => Some((depth, x, y, facing, entrance)),
        }
    }

    /// Whether a dungeon action can run at all: underground, alive, and not
    /// mid-intrusion. Mirrors the guard at the top of `move_player`.
    fn can_act_underground(&self) -> bool {
        self.is_underground() && self.is_game_over().is_none() && !self.has_active_battle()
    }

    fn set_facing(&mut self, dir: Dir) {
        if let Locale::Dungeon { facing, .. } = &mut *self.world.resource_mut::<Locale>() {
            *facing = dir;
        }
    }

    pub fn turn_left(&mut self) {
        if !self.can_act_underground() {
            return;
        }
        let Some((_, _, _, facing, _)) = self.dungeon_pos() else {
            return;
        };
        self.set_facing(facing.turn_left());
        self.tick();
    }

    pub fn turn_right(&mut self) {
        if !self.can_act_underground() {
            return;
        }
        let Some((_, _, _, facing, _)) = self.dungeon_pos() else {
            return;
        };
        self.set_facing(facing.turn_right());
        self.tick();
    }

    pub fn step_forward(&mut self) {
        self.step(1);
    }

    /// Backs up without turning round — the party keeps facing the way it
    /// was, which is how you retreat down a corridor you have already read.
    pub fn step_back(&mut self) {
        self.step(-1);
    }

    /// `sign` is +1 to walk forward along the facing, -1 to back up along it.
    /// Ticks the surface sim either way, and ticks it even when the step is
    /// blocked by rock: shoving at a wall still passes time, exactly as a
    /// blocked step does on the surface.
    fn step(&mut self, sign: i32) {
        if !self.can_act_underground() {
            return;
        }
        let Some((_, x, y, facing, _)) = self.dungeon_pos() else {
            return;
        };
        let (dx, dy) = facing.delta();
        let (nx, ny) = (x + dx * sign, y + dy * sign);

        let walkable = self
            .world
            .resource::<CurrentDungeon>()
            .0
            .as_ref()
            .is_some_and(|level| level.walkable(nx, ny));

        if walkable && let Locale::Dungeon { x, y, .. } = &mut *self.world.resource_mut::<Locale>()
        {
            *x = nx;
            *y = ny;
        }
        self.tick();
    }

    /// Takes the stairs under the party, in whichever direction they lead.
    /// Climbing from depth 1 surfaces. Does nothing on a cell without
    /// stairs, so a renderer can bind this unconditionally.
    pub fn take_stairs(&mut self) {
        if !self.can_act_underground() {
            return;
        }
        let Some((depth, x, y, _, entrance)) = self.dungeon_pos() else {
            return;
        };
        let Some(cell) = self
            .world
            .resource::<CurrentDungeon>()
            .0
            .as_ref()
            .map(|level| level.cell(x, y))
        else {
            return;
        };

        match cell {
            CellKind::StairsDown => {
                self.descend_to(depth + 1, entrance);
                self.log(format!("You descend to dungeon level {}.", depth + 1));
                self.tick();
            }
            CellKind::StairsUp if depth == 1 => {
                self.leave_dungeon();
                self.tick();
            }
            CellKind::StairsUp => {
                // Climbing lands on the level above's stairs *down*, not its
                // entry — otherwise every ascent would teleport the party
                // back to that level's entrance and undo the walk they just
                // made.
                self.ascend_to(depth - 1, entrance);
                self.log(format!("You climb back to dungeon level {}.", depth - 1));
                self.tick();
            }
            _ => self.log("There are no stairs here.".to_string()),
        }
    }

    fn ascend_to(&mut self, depth: u32, entrance: (i32, i32)) {
        let seed = self.world.resource::<WorldMap>().seed();
        let level = dungeon::generate(seed, depth);
        let landing = level.stairs_down;
        self.world.insert_resource(CurrentDungeon(Some(level)));
        self.world.insert_resource(Locale::Dungeon {
            depth,
            x: landing.0,
            y: landing.1,
            facing: Dir::North,
            entrance,
        });
    }

    /// Restores a saved dungeon position, regenerating the level from the
    /// world seed and `depth` rather than reading it off disk — see
    /// `resources::CurrentDungeon`.
    pub(crate) fn restore_locale(&mut self, locale: Locale) {
        if let Locale::Dungeon { depth, .. } = locale {
            let seed = self.world.resource::<WorldMap>().seed();
            let level = dungeon::generate(seed, depth);
            self.world.insert_resource(CurrentDungeon(Some(level)));
        }
        self.world.insert_resource(locale);
    }

    pub(crate) fn locale(&self) -> Locale {
        *self.world.resource::<Locale>()
    }

    /// The first-person view of the cells around the party, already rotated
    /// into view space — see `views::DungeonView`. `None` on the surface.
    pub fn dungeon_view(&self) -> Option<DungeonView> {
        let (depth, x, y, facing, _) = self.dungeon_pos()?;
        let level = self.world.resource::<CurrentDungeon>().0.as_ref()?;

        let (fx, fy) = facing.delta();
        let (rx, ry) = facing.right_delta();
        let span = DUNGEON_VIEW_HALF_WIDTH as i32;

        let cells = (0..DUNGEON_VIEW_DEPTH as i32)
            .map(|ahead| {
                (-span..=span)
                    .map(|lateral| {
                        let cx = x + fx * ahead + rx * lateral;
                        let cy = y + fy * ahead + ry * lateral;
                        match level.cell(cx, cy) {
                            CellKind::Rock => DungeonCellView::Rock,
                            CellKind::Floor => DungeonCellView::Floor,
                            CellKind::StairsUp => DungeonCellView::StairsUp,
                            CellKind::StairsDown => DungeonCellView::StairsDown,
                        }
                    })
                    .collect()
            })
            .collect();

        let standing_on = match level.cell(x, y) {
            CellKind::StairsDown => Some("Stairs lead down".to_string()),
            CellKind::StairsUp if depth == 1 => {
                Some("A breach leads back to the surface".to_string())
            }
            CellKind::StairsUp => Some("Stairs lead up".to_string()),
            _ => None,
        };

        Some(DungeonView {
            depth,
            facing: facing.label(),
            position: (x, y),
            cells,
            standing_on,
        })
    }
}
