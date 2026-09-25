//! The player's walk: a queued step or a route toward a click, spent one
//! cell a clock tick by `App::update_realtime`'s `spend_walk_tick`
//! (`app/playing.rs`) — never by `handle_key`, which is what stops holding
//! an arrow from fast-forwarding the world past the speed setting
//! (`travel-on-the-clock`'s whole point).

use feral_processes_engine::views::drawn_on_surface_map;

use crate::*;

/// What the clock is about to spend its next tick on.
///
/// `Step` never builds a route — an arrow is a direction, never a
/// destination — so it carries nothing past the delta. `Travel` re-asks
/// `Game::travel_step` every tick rather than caching a path, which is what
/// lets it *follow* a moving hostile instead of walking to wherever it
/// stood when the travel was set; `in_base` is the space the travel was set
/// in, so a crossing into the other one ends it (`update_realtime`'s own
/// check) rather than asking `travel_step` to route through a locale its
/// `Tile`/`Creature` goal was never resolved against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Walk {
    Step(i32, i32),
    Travel { goal: TravelGoal, in_base: bool },
}

impl App {
    /// Sets a travel toward `(x, y)` — a click on the map. `Mode::Playing`
    /// only, and refused underground: the Stack's first-person arrows have
    /// no notion of a clicked tile, and a `TravelStep` search never runs
    /// down there anyway (`Game::travel_step` answers `NoRoute`
    /// unconditionally), so refusing here says the same thing before a
    /// tick is spent finding it out.
    ///
    /// The goal is `Creature` when a hostile stands on the tile, found
    /// through the same rule the map draws by —
    /// `views::drawn_on_surface_map` — so a wild program mid-errand or a
    /// posted program whose `Position` has gone stale does not become a
    /// chase target just because a query still turns it up there.
    pub fn travel_to(&mut self, x: i32, y: i32) {
        if self.mode != Mode::Playing {
            return;
        }
        let Some(game) = &mut self.game else { return };
        if game.is_underground() {
            return;
        }
        let in_base = game.in_base();
        let goal = game
            .view_entities_at((x, y), 0, 0)
            .into_iter()
            .find(|e| e.is_hostile && drawn_on_surface_map(e.is_tamed, e.position_is_honest))
            .map_or(TravelGoal::Tile(x, y), |e| TravelGoal::Creature(e.entity));
        self.walk = Some(Walk::Travel { goal, in_base });
    }
}
