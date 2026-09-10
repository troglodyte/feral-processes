//! One screen's worth of moving cargo: what the adjacent shelves hold and
//! what the pack could put back, offered together and committed in one
//! action.
//!
//! The union of `collect.rs` and `deposit.rs`. It reimplements neither —
//! `take_from_adjacent` and `give_to_adjacent` are the two movers, and this
//! module is the offer, the two refusals and the one commit door.

use crate::items::DownedProgram;
use crate::*;

/// What one direction of a transfer actually moved, keyed and ordered by
/// `ItemId` — the shape both movers already return.
pub type Moved = Vec<(ItemId, u32)>;

/// One basket as the one commit door takes it: items in both directions and
/// whole carriers, committed together or not at all.
///
/// A parameter object rather than a fourth and fifth positional argument.
/// The two item lists are quantities of a fungible thing; `carriers` is a
/// list of **indices into `Game::rack_offer()`**, `load_teardown_rig`'s own
/// index idiom, because a carrier has no id to name it by and an `Entity`
/// is not stable across a save.
#[derive(Default, Clone, Debug)]
pub struct TransferBasket {
    pub take: Vec<(ItemId, u32)>,
    pub give: Vec<(ItemId, u32)>,
    pub carriers: Vec<usize>,
}

impl TransferBasket {
    /// An items-only basket — what every caller wanted before racks shipped.
    pub fn items(take: &[(ItemId, u32)], give: &[(ItemId, u32)]) -> Self {
        Self {
            take: take.to_vec(),
            give: give.to_vec(),
            carriers: Vec::new(),
        }
    }

    /// A carriers-only basket, by row index into `Game::rack_offer()`.
    pub fn carriers(rows: &[usize]) -> Self {
        Self {
            carriers: rows.to_vec(),
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.take.is_empty() && self.give.is_empty() && self.carriers.is_empty()
    }
}

impl Game {
    /// Every item the party could move in either direction, in `ItemId`
    /// order.
    ///
    /// `&self`: no tick, no log, no RNG. The guards are
    /// the base doors' own, in the same order, each answering with an
    /// empty offer — game over, an active battle, `require_base`. No
    /// `require_surface`: `require_base` is the stronger statement.
    ///
    /// **The trade currency is not cargo and gets no row at all**, on either
    /// side — the same exclusion `caravan_shelf` and the stack market's
    /// listing already make, for the same reason: a currency is what a
    /// transfer is priced in rather than a thing that moves. Its own filter
    /// and not `ItemDef::banked`, because Credits are carried, are spendable
    /// from the pack, and survive a breach.
    ///
    /// `carried` is what the pack holds, whatever may be done with it.
    /// `can_put` is 0 unless there is a Depot beside the party to put the
    /// item into, the item is not `banked` — a bank is not cargo — and some
    /// adjacent Depot has room for it that its filter has not closed, and
    /// **only a row with a `can_put` of its own is created from the pack
    /// side**, so an item that can go nowhere is listed only when it is also
    /// sitting on a shelf. A banked item may still have `on_shelves`, since a
    /// Research Node produces one.
    pub fn transfer_offer(&self) -> Vec<TransferRow> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        if self.require_base().is_err() {
            return Vec::new();
        }
        let currency = self.trade_currency();
        // A `BTreeMap` so the `ItemId` order is the map's rather than a
        // second explicit sort that could drift from the log line's.
        let mut rows: std::collections::BTreeMap<ItemId, TransferRow> =
            std::collections::BTreeMap::new();
        for structure in self.adjacent_stock() {
            let stock = self.world.get::<Stock>(structure).unwrap();
            for (item, qty) in stock.output.iter() {
                if *qty == 0 || *item == currency {
                    continue;
                }
                rows.entry(item.clone())
                    .or_insert_with(|| TransferRow {
                        item: item.clone(),
                        on_shelves: 0,
                        carried: 0,
                        can_put: 0,
                    })
                    .on_shelves += qty;
            }
        }
        let puttable = !self.adjacent_depots().is_empty();
        let player = self.player_entity();
        if let Some(inv) = self.world.get::<Inventory>(player) {
            for (item, qty) in inv.items.iter() {
                if *qty == 0 || *item == currency {
                    continue;
                }
                // The permission and the quantity are two questions, and
                // only the permission decides whether the row exists. A
                // Depot that is full — or one whose filter refuses this
                // item — still leaves the player something to look at and a
                // `[F]` to press; a pack full of cargo standing beside a
                // Mining Node, with no shelf to put anything on at all,
                // would otherwise open a screen of rows that move in
                // neither direction.
                let may_put = puttable && !self.is_banked(item);
                let can_put = if may_put {
                    (*qty).min(self.deposit_room_for(item))
                } else {
                    0
                };
                match rows.get_mut(item) {
                    Some(row) => {
                        row.carried = *qty;
                        row.can_put = can_put;
                    }
                    None if may_put => {
                        rows.insert(
                            item.clone(),
                            TransferRow {
                                item: item.clone(),
                                on_shelves: 0,
                                carried: *qty,
                                can_put,
                            },
                        );
                    }
                    None => {}
                }
            }
        }
        rows.into_values().collect()
    }

    /// The Quarantine Racks orthogonally beside the party, in `(x, y)`
    /// order.
    ///
    /// `adjacent_stock`'s shape and its reason for sorting: bevy's iteration
    /// order is not stable, and an identical save must answer one keypress
    /// the same way on every run. The order is not decoration — it is which
    /// rack a put lands in.
    pub fn adjacent_racks(&self) -> Vec<Entity> {
        let Some((px, py)) = self.base_pos() else {
            return Vec::new();
        };
        let db = self.world.resource::<StructureDb>();
        let mut found: Vec<(i32, i32, Entity)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let kind = &e.get::<Structure>()?.kind;
                let p = e.get::<Position>()?;
                db.get(kind)?.racks.as_ref()?;
                Some((p.x, p.y, e.id()))
            })
            .filter(|(x, y, _)| {
                crate::game::base::collect::ORTHOGONAL
                    .iter()
                    .any(|(dx, dy)| (*x, *y) == (px + dx, py + dy))
            })
            .collect();
        found.sort();
        found.into_iter().map(|(_, _, e)| e).collect()
    }

    /// Every carrier the party could move in either direction: the pack's
    /// first, then each adjacent rack's shelf in `(x, y)` order.
    ///
    /// **The index into this list is the handle**, and the ordering is the
    /// whole of what makes it one: the basket names rows, and this call and
    /// the commit both build the list the same way from the same world.
    ///
    /// Guarded exactly as `transfer_offer` is, and for the same reason.
    pub fn rack_offer(&self) -> Vec<crate::views::TransferCarrier> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        if self.require_base().is_err() {
            return Vec::new();
        }
        self.carrier_slots()
            .into_iter()
            .map(|(program, rack)| crate::views::TransferCarrier {
                label: self.downed_program_label(&program),
                racked: rack.is_some(),
            })
            .collect()
    }

    /// `rack_offer`'s rows with the world's own handles still attached —
    /// the program and, for a racked one, which rack is holding it. Private
    /// because an `Entity` never crosses into a view.
    fn carrier_slots(&self) -> Vec<(DownedProgram, Option<Entity>)> {
        let player = self.player_entity();
        let mut rows: Vec<(DownedProgram, Option<Entity>)> = self
            .world
            .get::<crate::components::DownedPrograms>(player)
            .map(|held| held.0.iter().cloned().map(|p| (p, None)).collect())
            .unwrap_or_default();
        for rack in self.adjacent_racks() {
            let Some(racked) = self.world.get::<crate::components::Racked>(rack) else {
                continue;
            };
            rows.extend(racked.0.iter().cloned().map(|p| (p, Some(rack))));
        }
        rows
    }

    /// How many carriers this Quarantine Rack holds, `slots * tier`.
    ///
    /// **Derived per read and never stored**, `BuildSite::required_ticks`'
    /// rule: a ceiling copied into the component at build goes stale the
    /// moment a tier lands. A structure with no `StructureTier` reads as
    /// tier 1, `extraction_ticks`' own reading of a never-upgraded machine;
    /// a def that racks nothing answers 0.
    pub fn rack_slots(&self, rack: Entity) -> u32 {
        let Some(kind) = self.world.get::<Structure>(rack).map(|s| s.kind.clone()) else {
            return 0;
        };
        let Some(slots) = self
            .world
            .resource::<StructureDb>()
            .get(&kind)
            .and_then(|def| def.racks.as_ref())
            .map(|r| r.slots)
        else {
            return 0;
        };
        let tier = self
            .world
            .get::<crate::components::StructureTier>(rack)
            .map_or(1, |t| t.0.max(1));
        slots * tier
    }

    /// Free slots on this rack — its ceiling less what is standing on it.
    pub fn rack_room(&self, rack: Entity) -> u32 {
        let held = self
            .world
            .get::<crate::components::Racked>(rack)
            .map_or(0, |r| r.0.len() as u32);
        self.rack_slots(rack).saturating_sub(held)
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
    pub fn transfer_items(&mut self, basket: &TransferBasket) -> (Moved, Moved) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return (Moved::new(), Moved::new());
        }
        if self.require_base().is_err() {
            return (Moved::new(), Moved::new());
        }
        // Both carrier refusals land here, above the item movers, which is
        // what makes one basket one commit: a carrier half that cannot go
        // through takes the item half with it rather than leaving the
        // player a half-moved basket to work out.
        if let Err(refusal) = self.check_carrier_basket(&basket.carriers) {
            self.note_refusal(refusal);
            return (Moved::new(), Moved::new());
        }
        let taken = self.take_from_adjacent(&basket.take);
        let given = self.give_to_adjacent(&basket.give);
        let carriers = self.move_carriers(&basket.carriers);

        if !taken.is_empty() {
            let summary = self.moved_summary(&taken);
            self.log_base_kind(MessageKind::Loot, format!("You collect {summary}."));
        }
        if !given.is_empty() {
            let summary = self.moved_summary(&given);
            self.log_base(format!("You put away {summary}."));
        }
        if carriers > 0 {
            let what = if carriers == 1 {
                "one downed program".to_string()
            } else {
                format!("{carriers} downed programs")
            };
            self.log_base(format!("You shift {what} on the rack."));
        }
        // The take side only. The mission teaches pulling stock *out* of a
        // machine; a player who only put something in has not done it.
        // Before the tick, so the deed is drained by the very
        // `contract_system` run this action pays for.
        if !taken.is_empty() {
            self.note_deed(crate::contracts::Deed::TookFromContainer);
        }
        if !taken.is_empty() || !given.is_empty() || carriers > 0 {
            self.tick();
        }
        (taken, given)
    }

    /// Both carrier refusals, answered before anything is spent.
    ///
    /// The pack ceiling is checked on the **net**: a basket that puts one
    /// carrier away and takes another back is legal at exactly the cap,
    /// which is what the picker's own clamps already offer.
    fn check_carrier_basket(&self, rows: &[usize]) -> Result<(), String> {
        if rows.is_empty() {
            return Ok(());
        }
        let slots = self.carrier_slots();
        let (mut taking, mut giving) = (0usize, 0usize);
        for row in rows
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
        {
            match slots.get(row) {
                Some((_, Some(_))) => taking += 1,
                Some((_, None)) => giving += 1,
                None => {}
            }
        }
        let player = self.player_entity();
        let held = self
            .world
            .get::<crate::components::DownedPrograms>(player)
            .map_or(0, |h| h.0.len());
        if held + taking - giving.min(held) > crate::tuning::MAX_DOWNED_PROGRAMS {
            return Err("You have no room to carry another downed program.".to_string());
        }
        let room: u32 = self
            .adjacent_racks()
            .into_iter()
            .map(|rack| self.rack_room(rack))
            .sum();
        if giving as u32 > room {
            return Err("The racks here are full.".to_string());
        }
        Ok(())
    }

    /// Moves the checked carriers and returns how many crossed.
    ///
    /// Taken before given, `transfer_items`' own order, so a basket that
    /// empties a rack and refills it from the pack lands both halves. Rows
    /// are resolved against one snapshot of `carrier_slots` and removed
    /// **back to front**, so an earlier removal cannot shift a later index.
    fn move_carriers(&mut self, rows: &[usize]) -> u32 {
        if rows.is_empty() {
            return 0;
        }
        let slots = self.carrier_slots();
        let wanted: Vec<usize> = rows
            .iter()
            .copied()
            .filter(|r| *r < slots.len())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .rev()
            .collect();
        let player = self.player_entity();
        let mut moved = 0;
        let mut to_rack: Vec<DownedProgram> = Vec::new();
        for row in wanted {
            let (program, holder) = slots[row].clone();
            match holder {
                Some(rack) => {
                    let Some(mut shelf) = self.world.get_mut::<crate::components::Racked>(rack)
                    else {
                        continue;
                    };
                    let Some(at) = shelf.0.iter().position(|p| *p == program) else {
                        continue;
                    };
                    let taken = shelf.0.remove(at);
                    if let Some(mut held) = self
                        .world
                        .get_mut::<crate::components::DownedPrograms>(player)
                    {
                        held.0.push(taken);
                    }
                    moved += 1;
                }
                None => {
                    if let Some(mut held) = self
                        .world
                        .get_mut::<crate::components::DownedPrograms>(player)
                        && let Some(at) = held.0.iter().position(|p| *p == program)
                    {
                        to_rack.push(held.0.remove(at));
                    }
                }
            }
        }
        // Put back in the order the player is looking at, and into the
        // first rack with room in `(x, y)` order.
        for program in to_rack.into_iter().rev() {
            let landed = self
                .adjacent_racks()
                .into_iter()
                .find(|rack| self.rack_room(*rack) > 0);
            let Some(rack) = landed else {
                // Unreachable behind `check_carrier_basket`, and the carrier
                // goes back in the pack rather than being dropped if it ever
                // is not.
                if let Some(mut held) = self
                    .world
                    .get_mut::<crate::components::DownedPrograms>(player)
                {
                    held.0.push(program);
                }
                continue;
            };
            if let Some(mut shelf) = self.world.get_mut::<crate::components::Racked>(rack) {
                shelf.0.push(program);
                moved += 1;
            }
        }
        moved
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
