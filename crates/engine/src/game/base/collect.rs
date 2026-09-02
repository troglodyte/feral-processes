//! Taking adjacent structures' output buffers into the player's cargo — the
//! take half of `transfer.rs`, which owns the guards, the log line and the
//! turn.

use crate::game::base::hauling;
use crate::*;

/// The four tiles a machine feeds and the player collects from. Named once
/// so the two rules cannot drift: the moment a collect could reach a tile a
/// machine could not, the base would stop reading as a physical line.
pub(crate) const ORTHOGONAL: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

/// Plans a take of up to `want` units off the output buffers of the four
/// tiles orthogonally touching `tile`, walked in `ORTHOGONAL`'s own order and
/// stopping the moment the want is met. `available` answers how many units of
/// the one item in question a given neighbour is holding.
///
/// **The one machine-to-machine reach rule**, and it exists as a function for
/// the reason `ORTHOGONAL` itself does: `assembler_system` pulls a recipe's
/// ingredients out of its neighbours and `systems::power_grid_system` pulls a
/// supplier's Power Cell out of the same four tiles, and the moment those two
/// walks could differ the base stops reading as a physical line. Neither
/// caller can use `Game::take_from_adjacent` — that one is the *player's*
/// collect, keyed on where the party stands and needing `&mut Game`, which a
/// bevy system does not have.
///
/// Planning rather than moving, because both callers read a neighbour's
/// `output` and write their own buffer through the same `Query<&mut Stock>`
/// and cannot hold the two borrows at once. Units still leave a buffer
/// through `hauling::take_from` alone, once the plan is applied.
pub(crate) fn plan_adjacent_take(
    tile: (i32, i32),
    want: u32,
    by_tile: &std::collections::HashMap<(i32, i32), Entity>,
    available: impl Fn(Entity) -> u32,
) -> Vec<(Entity, u32)> {
    let (x, y) = tile;
    let mut outstanding = want;
    let mut plan = Vec::new();
    for (dx, dy) in ORTHOGONAL {
        if outstanding == 0 {
            break;
        }
        let Some(&feeder) = by_tile.get(&(x + dx, y + dy)) else {
            continue;
        };
        let take = outstanding.min(available(feeder));
        if take == 0 {
            continue;
        }
        plan.push((feeder, take));
        outstanding -= take;
    }
    plan
}

impl Game {
    /// The structures a collect can reach, in `(x, y)` order.
    ///
    /// The sort is `assembler_system`'s reason in a second place: bevy's
    /// query iteration order is not stable, and a *partial* take across two
    /// neighbours holding the same item has to drain them in the same order
    /// every run, or an identical save answers the same keypress differently
    /// between two runs. Taking everything could not see this, which is why
    /// the code went so long without one.
    ///
    /// `pub(crate)` so `game/base/transfer.rs` can build the offer from it
    /// and `tests/transfer.rs` can pin the order directly; nothing outside
    /// the crate calls it.
    pub(crate) fn adjacent_stock(&self) -> Vec<Entity> {
        // The party's base cell rather than their `Position`: every machine
        // with a `Stock` stands in base space, and `Position` is pinned to
        // the anchor tile on the zone surface while the party is in here.
        let Some((px, py)) = self.base_pos() else {
            return Vec::new();
        };
        // `iter_entities` rather than a query, for `stock::output_buffers`'
        // reason: `World::query_filtered` needs `&mut World` to build, and
        // the offer has to be readable from a screen.
        let mut found: Vec<(i32, i32, Entity)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let p = e.get::<Position>()?;
                e.contains::<Stock>().then_some((p.x, p.y, e.id()))
            })
            .filter(|(x, y, _)| {
                ORTHOGONAL
                    .iter()
                    .any(|(dx, dy)| (*x, *y) == (px + dx, py + dy))
            })
            .collect();
        found.sort();
        found.into_iter().map(|(_, _, e)| e).collect()
    }

    /// Moves an exact basket off the adjacent structures and reports what
    /// actually landed, keyed and ordered by `ItemId`.
    ///
    /// **It holds no guards of its own and it neither ticks nor logs.** Every
    /// caller must have checked game over, an active battle and
    /// `require_base` already, and owns the announcement and the turn. That
    /// is what lets one screen take and give inside a single action.
    ///
    /// An over-ask is clamped rather than refused, and units leave a buffer
    /// through `hauling::take_from` alone.
    pub(crate) fn take_from_adjacent(&mut self, want: &[(ItemId, u32)]) -> Vec<(ItemId, u32)> {
        let player = self.player_entity();
        let neighbours = self.adjacent_stock();

        let mut taken: std::collections::BTreeMap<ItemId, u32> = std::collections::BTreeMap::new();
        for (item, qty) in want {
            let mut outstanding = *qty;
            for structure in &neighbours {
                if outstanding == 0 {
                    break;
                }
                let mut stock = self.world.get_mut::<Stock>(*structure).unwrap();
                let got = hauling::take_from(&mut stock, item, outstanding);
                if got == 0 {
                    continue;
                }
                outstanding -= got;
                // Not `grant_loot`: collecting also has to clear the source
                // structure's own stock, which a plain inventory grant
                // doesn't touch.
                self.world
                    .get_mut::<Inventory>(player)
                    .unwrap()
                    .add(item.clone(), got);
                *taken.entry(item.clone()).or_default() += got;
            }
        }

        taken.into_iter().collect()
    }
}
