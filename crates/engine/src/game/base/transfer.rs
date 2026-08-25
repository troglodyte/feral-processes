//! One screen's worth of moving cargo: what the adjacent shelves hold and
//! what the pack could put back, offered together and committed in one
//! action.
//!
//! The union of `collect.rs` and `deposit.rs`. It reimplements neither —
//! `take_from_adjacent` and `give_to_adjacent` are the two movers, and this
//! module is the offer, the two refusals and the one commit door.

use crate::*;

impl Game {
    /// Every item the party could move in either direction, in `ItemId`
    /// order.
    ///
    /// `&self`: no tick, no log, no RNG. The guards are
    /// `collectable_adjacent`'s, in the same order, each answering with an
    /// empty offer — game over, an active battle, `require_base`. No
    /// `require_surface`: `require_base` is the stronger statement.
    ///
    /// `in_pack` is 0 unless there is a Depot beside the party to put the
    /// item into and the item is not `banked` — a bank is not cargo. A
    /// banked item may still have `on_shelves`, since a Research Node
    /// produces one.
    pub fn transfer_offer(&self) -> Vec<TransferRow> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        if self.require_base().is_err() {
            return Vec::new();
        }
        // A `BTreeMap` so the `ItemId` order is the map's rather than a
        // second explicit sort that could drift from the log line's.
        let mut rows: std::collections::BTreeMap<ItemId, TransferRow> =
            std::collections::BTreeMap::new();
        for structure in self.adjacent_stock() {
            let stock = self.world.get::<Stock>(structure).unwrap();
            for (item, qty) in stock.output.iter() {
                if *qty == 0 {
                    continue;
                }
                rows.entry(item.clone())
                    .or_insert_with(|| TransferRow {
                        item: item.clone(),
                        on_shelves: 0,
                        in_pack: 0,
                    })
                    .on_shelves += qty;
            }
        }
        if !self.adjacent_depots().is_empty() {
            let player = self.player_entity();
            if let Some(inv) = self.world.get::<Inventory>(player) {
                for (item, qty) in inv.items.iter() {
                    if *qty == 0 || self.is_banked(item) {
                        continue;
                    }
                    rows.entry(item.clone())
                        .or_insert_with(|| TransferRow {
                            item: item.clone(),
                            on_shelves: 0,
                            in_pack: 0,
                        })
                        .in_pack += qty;
                }
            }
        }
        rows.into_values().collect()
    }

    /// Room left across the adjacent Depots, or `None` when there is no
    /// Depot beside the party at all.
    ///
    /// The one call that keeps "no Depot here" distinguishable from "a Depot
    /// with nothing left": a screen inferring the first from a zero draws a
    /// room line reading 0 beside a Mining Node, which claims the base is
    /// full when it has no shelf at all.
    pub fn transfer_room(&self) -> Option<u32> {
        if self.adjacent_depots().is_empty() {
            return None;
        }
        Some(self.deposit_room())
    }

    /// Moves a basket in both directions as one action, reporting what was
    /// taken and then what was given.
    ///
    /// **Take before give.** A rebalance that empties a full Depot and
    /// refills it from the pack only lands both halves in this order; the
    /// other way the give clamps to zero for want of room and the failure is
    /// silent.
    ///
    /// One `Loot` line for what came and one base line for what went, in
    /// that order, each skipped when its half is empty. Then one `tick()`,
    /// and only if anything moved — an empty or all-zero basket is a silent
    /// no-op costing no turn.
    pub fn transfer_items(
        &mut self,
        take: &[(ItemId, u32)],
        give: &[(ItemId, u32)],
    ) -> (Vec<(ItemId, u32)>, Vec<(ItemId, u32)>) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return (Vec::new(), Vec::new());
        }
        if self.require_base().is_err() {
            return (Vec::new(), Vec::new());
        }
        let taken = self.take_from_adjacent(take);
        let given = self.give_to_adjacent(give);

        if !taken.is_empty() {
            let summary = self.moved_summary(&taken);
            self.log_base_kind(MessageKind::Loot, format!("You collect {summary}."));
        }
        if !given.is_empty() {
            let summary = self.moved_summary(&given);
            self.log_base(format!("You put away {summary}."));
        }
        if !taken.is_empty() || !given.is_empty() {
            self.tick();
        }
        (taken, given)
    }

    /// "N item, M other item" — the one join both halves of a transfer log
    /// line are built from.
    fn moved_summary(&self, moved: &[(ItemId, u32)]) -> String {
        moved
            .iter()
            .map(|(item, n)| format!("{n} {}", self.item_name(item)))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Says why there is nothing to move, when the screen finds an empty
    /// offer.
    ///
    /// Two sentences, because they leave the player different errands: no
    /// adjacent `Stock` at all, or one with nothing on either side. The
    /// guards come first and refuse *silently*, as the doors this replaces
    /// always have — an action taken during a battle or from the surface is
    /// not the base telling you its shelves are bare.
    pub fn refuse_transfer(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        if self.require_base().is_err() {
            return;
        }
        if self.adjacent_stock().is_empty() {
            self.log_base("There is nothing here to take from or put into.");
        } else {
            self.log_base("There is nothing to move here.");
        }
    }
}
