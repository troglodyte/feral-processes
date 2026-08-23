//! Putting plain cargo out of `Inventory` and into an adjacent Depot's
//! `Stock::output` — the mirror of `collect.rs`, giving instead of taking.

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
    /// `pub(crate)` solely so `tests/deposit.rs` can pin the order directly;
    /// nothing outside this module calls it.
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

    /// What the player may put into an adjacent Depot: the `Inventory` rows
    /// that are not banked, sorted into `ItemId` order — the order `Stock`'s
    /// own `BTreeMap` yields and the order a deposit log line will print in.
    ///
    /// The sort is explicit here, unlike `collectable_adjacent`'s: that
    /// function pools into a `BTreeMap` and gets the order for free, while
    /// `Inventory::items` is a `Vec` in insertion order — an unsorted list
    /// here would put the rows in whatever order the player happened to
    /// pick things up.
    ///
    /// `&self`: no tick, no log, no RNG. It holds the same guards
    /// `deposit_items` does and answers with an empty offer for each — game
    /// over, an active battle, `require_base` failing, or no adjacent
    /// Depot. It is a claim about what is beside the party, so like
    /// `collectable_adjacent` it needs no `require_surface` of its own;
    /// `require_base` is the stronger statement.
    pub fn depositable(&self) -> Vec<(ItemId, u32)> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        if self.require_base().is_err() {
            return Vec::new();
        }
        if self.adjacent_depots().is_empty() {
            return Vec::new();
        }
        let player = self.player_entity();
        let mut offer: Vec<(ItemId, u32)> = self
            .world
            .get::<Inventory>(player)
            .map(|inv| {
                inv.items
                    .iter()
                    .filter(|(item, _)| !self.is_banked(item))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        offer.sort();
        offer
    }

    /// Room left across every adjacent Depot's `output` — the shared budget
    /// the picker has to enforce live.
    ///
    /// Same guards, same empty answer, for `depositable`'s reason.
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
}
