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
use crate::components::{Player, Position, Tamed, Task};
use crate::game::party::{ProgramRole, role_of};
use crate::resources::{GameClock, Locale, Party, PlayerEntity, WieldedProgram};
use crate::tuning::BASE_ENTROPY_REFILL_TICKS;

/// "A body that might be standing in base space", as the coarse filter for
/// `base_entropy_system`: a posted program (`Task`) or any program on the
/// roster (`Tamed`), and never the player, whose `Position` is pinned to
/// the anchor tile on the zone surface.
///
/// Deliberately wider than the real rule — a party member is `Tamed` and is
/// not in base space at all. `role_of` narrows it below, because the
/// narrowing is the part that must not exist twice.
type BaseOccupant = (Or<(With<Task>, With<Tamed>)>, Without<Player>);

/// What `base_entropy_system` reads off each candidate: where it is, whether
/// it is posted, and who owns it — the three `role_of` and the `Task` arm
/// between them need.
type Occupancy<'a> = (Entity, &'a Position, Option<&'a Task>, Option<&'a Tamed>);

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
///
/// **A `Task` is not what makes a body occupy a cell — standing there is**,
/// which is why staff are the other arm. Staff between postings hold
/// no `Task`, and `drift_idle_staff` cannot always move one on: it declines
/// a candidate tile that is occupied or is not laid floor, and
/// `schedule_base_labour` early-returns without moving anyone at all on a
/// game over or during a battle, while this system keeps running. A cell reverted under an idle
/// staffer seals it inside solid rock, and `hauling::post_field` gates its
/// own start tile on `BaseGrid::walkable` — so the body is unpostable and
/// unreachable for the rest of the run.
pub(crate) fn base_entropy_system(
    mut grid: ResMut<BaseGrid>,
    clock: Res<GameClock>,
    locale: Res<Locale>,
    player: Res<PlayerEntity>,
    roster: Res<Party>,
    wielded: Res<WieldedProgram>,
    occupants: Query<Occupancy, BaseOccupant>,
) {
    // A body occupies a base cell if it is posted at one, or if it is staff
    // waiting to be — the two arms the doc comment above argues for, with
    // the staff half asked of `role_of` rather than restated.
    let standing: Vec<&Position> = occupants
        .iter()
        .filter(|(entity, _, task, tamed)| {
            task.is_some()
                || tamed.is_some_and(|t| {
                    role_of(*entity, t.owner, player.0, &roster, wielded.0)
                        == Some(ProgramRole::Staff)
                })
        })
        .map(|(_, pos, _, _)| pos)
        .collect();
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
        .filter(|(x, y)| !standing.iter().any(|p| p.x == *x && p.y == *y))
        .collect();
    for (x, y) in doomed {
        grid.revert(x, y);
    }
}
