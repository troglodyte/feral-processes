//! What the player can see on the surface tile map: the one engine answer
//! every player-facing reader asks. See `views::Sight` and `views::shown_at`.

use crate::components::Position;
use crate::resources::{Locale, SeenTiles};
use crate::views::Sight;
use crate::*;

/// Whether `tile` is inside a circle of `radius` tiles about `from`. One
/// rule for sight and for what memory marks, so the two cannot disagree.
fn within(from: (i32, i32), tile: (i32, i32), radius: f32) -> bool {
    let (dx, dy) = ((tile.0 - from.0) as f32, (tile.1 - from.1) as f32);
    dx * dx + dy * dy <= radius * radius
}

impl Game {
    /// Whether fog applies at all: only on the surface, and not under the
    /// dev reveal switch.
    fn fog_applies(&self) -> bool {
        matches!(*self.world.resource::<Locale>(), Locale::Surface)
            && !super::stack_view::dev_reveal()
    }

    fn player_tile(&self) -> (i32, i32) {
        let pos = self.world.get::<Position>(self.player_entity());
        pos.map_or((0, 0), |p| (p.x, p.y))
    }

    /// Records every tile inside the player's current radius. Called from the
    /// turn beside `mark_explored_chunks`, and once when a game starts or
    /// loads so the first frame is not black outside a hole.
    pub(crate) fn mark_seen_tiles(&mut self) {
        if !matches!(*self.world.resource::<Locale>(), Locale::Surface) {
            return;
        }
        let from = self.player_tile();
        let radius = self.perception_radius();
        let reach = radius.ceil() as i32;
        let mut seen = self.world.resource_mut::<SeenTiles>();
        for y in (from.1 - reach)..=(from.1 + reach) {
            for x in (from.0 - reach)..=(from.0 + reach) {
                if within(from, (x, y), radius) {
                    seen.mark((x, y));
                }
            }
        }
    }

    /// What the player sees of `pos`. **Off the surface, and under
    /// `FERAL_DEV_REVEAL`, everything is `InSight`**, so no caller carries a
    /// locale check of its own.
    ///
    /// `InSight` is computed live from the player's current position, never
    /// read from memory, so a teleport leaves no gap.
    pub fn sight_at(&self, pos: (i32, i32)) -> Sight {
        if !self.fog_applies() {
            return Sight::InSight;
        }
        if within(self.player_tile(), pos, self.perception_radius()) {
            Sight::InSight
        } else if self.world.resource::<SeenTiles>().contains(pos) {
            Sight::Remembered
        } else {
            Sight::Unseen
        }
    }

    /// `views::shown_in` for a raw entity, for the readers that query
    /// positions themselves rather than going through an `EntityView`.
    pub(crate) fn is_shown(&self, entity: Entity, is_landmark: bool) -> bool {
        let Some(pos) = self.world.get::<Position>(entity) else {
            return false;
        };
        let in_party = self.world.resource::<Party>().0.contains(&entity);
        crate::views::shown_in(self.sight_at((pos.x, pos.y)), in_party, is_landmark)
    }

    /// [`Game::sight_at`] for every tile `view_tiles_at` would return for the
    /// same arguments: same indexing, same centre.
    pub fn sight_view_at(&self, center: (i32, i32), half_w: i32, half_h: i32) -> Vec<Vec<Sight>> {
        (-half_h..=half_h)
            .map(|ty| {
                (-half_w..=half_w)
                    .map(|tx| self.sight_at((center.0 + tx, center.1 + ty)))
                    .collect()
            })
            .collect()
    }
}
