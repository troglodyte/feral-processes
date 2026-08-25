//! Putting plain cargo out of `Inventory` and into an adjacent Depot's
//! `Stock::output` — the mirror of `collect.rs`, giving instead of taking,
//! and the give half of `transfer.rs`.

use crate::*;

impl Game {
    /// The adjacent structures a deposit can reach, in `(x, y)` order.
    ///
    /// `adjacent_stock()` filtered to `StructureDef::stores` — a Mining Node
    /// has a `Stock` too, and mirroring collect exactly would let the player
    /// push cargo into a machine's own output as though that machine had
    /// produced it. Filtered rather than re-sorted, so `adjacent_stock`'s
    /// `(x, y)` order survives untouched: a partial fill across two Depots
    /// has to drain — here, fill — them in the same order every run.
    ///
    /// `pub(crate)` so `game/base/transfer.rs` can ask whether there is a
    /// Depot at all, and so `tests/transfer.rs` can pin the order directly.
    pub(crate) fn adjacent_depots(&self) -> Vec<Entity> {
        self.adjacent_stock()
            .into_iter()
            .filter(|e| {
                let Some(structure) = self.world.get::<Structure>(*e) else {
                    return false;
                };
                self.world
                    .resource::<StructureDb>()
                    .get(&structure.kind)
                    .is_some_and(|d| d.stores)
            })
            .collect()
    }

    /// Room left across every adjacent Depot's `output` — the shared budget
    /// the picker has to enforce live.
    ///
    /// Same guards as the offer, and the same empty answer for each: it is a
    /// claim about what is beside the party, so like `transfer_offer` it
    /// needs no `require_surface` of its own. **Read through
    /// `Game::transfer_room`**, which is what tells a zero here apart from
    /// there being no Depot at all.
    pub fn deposit_room(&self) -> u32 {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return 0;
        }
        if self.require_base().is_err() {
            return 0;
        }
        self.adjacent_depots()
            .into_iter()
            .map(|e| self.world.get::<Stock>(e).unwrap().output_room())
            .sum()
    }

    /// Moves an exact basket out of the pack and into the adjacent Depots,
    /// reporting what actually landed, keyed and ordered by `ItemId`.
    ///
    /// **It holds no guards of its own and it neither ticks nor logs.** Every
    /// caller must have checked game over, an active battle and
    /// `require_base` already, and owns the announcement and the turn. That
    /// is what lets one screen take and give inside a single action.
    ///
    /// Both clamps live here: against what the pack holds, and against each
    /// Depot's `output_room()` as the fill walks them in `(x, y)` order.
    pub(crate) fn give_to_adjacent(&mut self, give: &[(ItemId, u32)]) -> Vec<(ItemId, u32)> {
        let player = self.player_entity();
        let depots = self.adjacent_depots();

        let mut given: std::collections::BTreeMap<ItemId, u32> = std::collections::BTreeMap::new();
        for (item, qty) in give {
            let mut outstanding = *qty;
            for depot in &depots {
                if outstanding == 0 {
                    break;
                }
                let room = self.world.get::<Stock>(*depot).unwrap().output_room();
                if room == 0 {
                    continue;
                }
                let moved = self
                    .world
                    .get_mut::<Inventory>(player)
                    .unwrap()
                    .take(item.clone(), outstanding.min(room));
                if moved == 0 {
                    continue;
                }
                outstanding -= moved;
                *self
                    .world
                    .get_mut::<Stock>(*depot)
                    .unwrap()
                    .output
                    .entry(item.clone())
                    .or_default() += moved;
                *given.entry(item.clone()).or_default() += moved;
            }
        }

        given.into_iter().collect()
    }
}
