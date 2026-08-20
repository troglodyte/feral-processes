//! Entropy on the frontier: base space reclaiming what was cut and never
//! floored.
//!
//! The pressure the whole of slice 2 is drawn against. Cutting rock is cheap
//! and floors it are not, so without this a player would open the whole
//! pocket outward and floor it at leisure; with it, ground you dug and left
//! bare goes back to solid and has to be cut again. **Only the frontier** —
//! a laid `BaseCell::Floor` is permanent, or a base could not be left alone
//! while its owner went to a zone.

use bevy_ecs::prelude::*;

use crate::base_grid::{BaseCell, BaseGrid};
use crate::components::{Player, Position, Task};
use crate::resources::{GameClock, Locale};
use crate::tuning::BASE_ENTROPY_REFILL_TICKS;

/// Reverts every unoccupied `Open` cell older than
/// `tuning::BASE_ENTROPY_REFILL_TICKS` to solid rock — *absent* from
/// `BaseGrid`, not chipped, so re-opening it costs the swings it cost the
/// first time.
///
/// **Occupancy is what keeps "standing inside rock" unreachable by
/// construction**, and it comes from two places that must not be confused.
/// The party's cell is `Locale::Base`'s own coordinates, never the player's
/// `Position` — which is pinned to the anchor tile on the zone surface and
/// would be a read in the wrong coordinate space, the silent bug class
/// 0.13.0 shipped two fixes for. A posted program's cell *is* its
/// `Position`, because a posted program is the one non-`Structure` entity
/// that has always stood in base space. The player can hold a `Task` too
/// (`Game::work_structure` posts them), which is exactly why the query
/// excludes them rather than trusting `With<Task>` to mean "a body in base
/// space".
pub(crate) fn base_entropy_system(
    mut grid: ResMut<BaseGrid>,
    clock: Res<GameClock>,
    locale: Res<Locale>,
    occupants: Query<&Position, (With<Task>, Without<Player>)>,
) {
    let party = match *locale {
        Locale::Base { x, y } => Some((x, y)),
        _ => None,
    };
    // Collected before anything is written: removing from the map while
    // iterating it is not a thing to work out at the call site.
    let doomed: Vec<(i32, i32)> = grid
        .iter()
        .filter_map(|(&(x, y), cell)| match cell {
            BaseCell::Open { mined_at }
                if clock.tick.saturating_sub(*mined_at) > BASE_ENTROPY_REFILL_TICKS =>
            {
                Some((x, y))
            }
            _ => None,
        })
        .filter(|cell| Some(*cell) != party)
        .filter(|(x, y)| !occupants.iter().any(|p| p.x == *x && p.y == *y))
        .collect();
    for (x, y) in doomed {
        grid.revert(x, y);
    }
}
