//! Putting plain cargo out of `Inventory` and into an adjacent Depot's
//! `Stock::output` — the mirror of `collect.rs`, giving instead of taking,
//! and the give half of `transfer.rs`.

use crate::game::base::transfer::Shelves;
use crate::*;

/// A structure hand-loaded in one give: which, with what, and how many.
pub(crate) type SinkLoad = (Entity, ItemId, u32);

impl Game {
    /// The Depots a transfer puts into: `reachable_stock` under
    /// the Depot filter, so beside an Index Terminal that is every Depot in
    /// the base.
    pub(crate) fn reachable_depots(&self) -> Vec<Entity> {
        self.only_depots(self.reachable_stock())
    }

    /// The structures orthogonally beside the party that spend a `supply`
    /// item, author a `supply_capacity` and are not shelves, each with the
    /// item they take and the room left in their input, in `(x, y)` order.
    ///
    /// **Only an authored capacity can be hand-loaded**: a Turret or a Shield
    /// is stocked by its haulers alone, so a capacity-less structure is not a
    /// put target however much supply it spends.
    ///
    /// **Beside, never `reachable`**: an Index Terminal widens what a shelf
    /// can be reached from and a machine's buffer stays walk-up. Loading is
    /// the one hand-delivery to a machine, so it asks for the machine to be
    /// the thing the party is standing at.
    pub(crate) fn supply_sinks(&self) -> Vec<(Entity, ItemId, u32)> {
        self.adjacent_stock()
            .into_iter()
            .filter_map(|e| self.supply_sink_of(e))
            .collect()
    }

    /// This structure as a put target for a supply item, with the room left
    /// in its input — `None` for a shelf, a structure with no authored
    /// capacity or one that spends nothing.
    pub(crate) fn supply_sink_of(&self, e: Entity) -> Option<(Entity, ItemId, u32)> {
        let def = self.structure_def_of(e)?;
        if def.stores || def.supply_capacity.is_none() {
            return None;
        }
        let item = def.spends_supply()?.clone();
        let held = self.world.get::<Stock>(e)?.input.get(&item).copied();
        let room = def
            .supply_input_capacity()
            .saturating_sub(held.unwrap_or(0));
        Some((e, item, room))
    }

    /// Whether anything beside the party takes a put: a Depot, or a
    /// structure that spends a supply.
    pub(crate) fn has_put_target_over(&self, shelves: &Shelves) -> bool {
        !shelves.depots.is_empty() || !shelves.sinks.is_empty()
    }

    fn only_depots(&self, cells: Vec<Entity>) -> Vec<Entity> {
        cells.into_iter().filter(|e| self.is_depot(*e)).collect()
    }

    /// Room left across every adjacent Depot's `output` — the shared budget
    /// the picker has to enforce live.
    ///
    /// Same guards as the offer, and the same empty answer for each: it is a
    /// claim about what is beside the party, so like `transfer_offer` it
    /// needs no `require_surface` of its own. **Read through
    /// `Game::transfer_room`**, which is what tells a zero here apart from
    /// there being no Depot at all.
    ///
    /// **A Depot that will take nothing the party is carrying is not room**
    /// — see `depots_for_put`. Summed blind, a shelf filtering out the whole
    /// pack promises the picker twice the space that exists, and the missing
    /// half is discovered as a silent short fill.
    pub fn deposit_room(&self) -> u32 {
        self.deposit_room_over(&self.pooled_shelves())
    }

    /// `deposit_room` across an explicit shelf set.
    pub(crate) fn deposit_room_over(&self, shelves: &Shelves) -> u32 {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return 0;
        }
        if self.require_base().is_err() {
            return 0;
        }
        let carried = self.puttable_carried();
        let sinks: u32 = shelves
            .sinks
            .iter()
            .filter(|(_, item, _)| carried.contains(item))
            .map(|(.., room)| room)
            .sum();
        sinks
            + self
                .depots_for_put(shelves)
                .into_iter()
                .map(|e| self.world.get::<Stock>(e).unwrap().output_room())
                .sum::<u32>()
    }

    /// The adjacent Depots the shared put budget is measured across: those
    /// that accept at least one thing the pack is holding.
    ///
    /// **Falls back to all of them when the pack has nothing puttable**, so
    /// a player who opened the screen only to take still reads the real
    /// room beside them rather than a zero that means "you are carrying
    /// nothing".
    ///
    /// The budget is one number across every row, so it cannot express a
    /// per-item answer; this is the coarse half, and `TransferRow::can_put`
    /// is the exact one.
    fn depots_for_put(&self, shelves: &Shelves) -> Vec<Entity> {
        let depots = shelves.depots.clone();
        let carried = self.puttable_carried();
        if carried.is_empty() {
            return depots;
        }
        depots
            .into_iter()
            .filter(|e| carried.iter().any(|item| self.depot_accepts(*e, item)))
            .collect()
    }

    /// What the pack holds that a Depot could ever take: `transfer_offer`'s
    /// two exclusions, asked of the inventory rather than of the catalogue.
    fn puttable_carried(&self) -> Vec<ItemId> {
        let currency = self.trade_currency();
        let player = self.player_entity();
        self.world
            .get::<Inventory>(player)
            .map(|inv| {
                inv.items
                    .iter()
                    .filter(|(item, qty)| *qty > 0 && *item != currency && !self.is_banked(item))
                    .map(|(item, _)| item.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Room for **one item** across the adjacent Depots that will take it.
    ///
    /// The exact figure the coarse budget above cannot give, and what
    /// `TransferRow::can_put` is clamped to: a Depot refusing this item is
    /// not room for it however empty it stands.
    pub(crate) fn deposit_room_for(&self, shelves: &Shelves, item: &ItemId) -> u32 {
        let sinks: u32 = shelves
            .sinks
            .iter()
            .filter(|(_, supply, _)| supply == item)
            .map(|(.., room)| room)
            .sum();
        sinks
            + shelves
                .depots
                .iter()
                .filter(|e| self.depot_accepts(**e, item))
                .map(|e| self.world.get::<Stock>(*e).unwrap().output_room())
                .sum::<u32>()
    }

    /// Moves an exact basket out of the pack into the structures beside the
    /// party, reporting what actually landed, keyed and ordered by `ItemId`,
    /// and separately each load a supply sink took. An item that is an
    /// adjacent sink's supply fills the sink before any Depot, or a shelf
    /// beside the bench would swallow what was meant to load it; everything
    /// else goes to `reachable_depots` — the adjacent ones, or every one
    /// beside a terminal.
    ///
    /// **It holds no guards of its own and it neither ticks nor logs.** Every
    /// caller must have checked game over, an active battle and
    /// `require_base` already, and owns the announcement and the turn. That
    /// is what lets one screen take and give inside a single action.
    ///
    /// Every clamp lives here: against what the pack holds, against each
    /// sink's `supply_input_capacity`, and against each Depot's
    /// `output_room()` as the fill walks them in `(x, y)` order — and each
    /// Depot's own filter, `depot_accepts`.
    pub(crate) fn give_to_shelves(
        &mut self,
        shelves: &Shelves,
        give: &[(ItemId, u32)],
    ) -> (Vec<(ItemId, u32)>, Vec<SinkLoad>) {
        let depots = shelves.depots.clone();
        let mut sinks = shelves.sinks.clone();
        let mut given: std::collections::BTreeMap<ItemId, u32> = std::collections::BTreeMap::new();
        let mut loads = Vec::new();
        for (item, qty) in give {
            // A bench first when the item is its supply: it can take nothing
            // else, while a Depot beside it would swallow every core and
            // leave the bench the one thing the player came to load. Room is
            // spent as it is filled and each row keeps its own remainder, so
            // a basket naming one item twice can neither overfill a bench nor
            // be short-changed by what an earlier row already gave.
            let loaded = self.fill_sinks(&mut sinks, item, *qty);
            let into_sinks: u32 = loaded.iter().map(|(_, n)| n).sum();
            loads.extend(loaded.into_iter().map(|(sink, n)| (sink, item.clone(), n)));
            let into_depots = self.fill_depots(&depots, item, *qty - into_sinks);
            let moved = into_sinks + into_depots;
            if moved > 0 {
                *given.entry(item.clone()).or_default() += moved;
            }
        }
        (given.into_iter().collect(), loads)
    }

    /// Moves up to `want` of `item` from the pack into the Depots that accept
    /// it, in order, and returns how many landed.
    fn fill_depots(&mut self, depots: &[Entity], item: &ItemId, want: u32) -> u32 {
        let player = self.player_entity();
        let mut outstanding = want;
        for depot in depots {
            if outstanding == 0 {
                break;
            }
            // A refusal reads exactly as a full shelf here, which is
            // what lets one filtered Depot in a row of them cost the
            // fill nothing but a step to the next.
            if !self.depot_accepts(*depot, item) {
                continue;
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
        }
        want - outstanding
    }

    /// Moves up to `want` of `item` from the pack into the supply sinks that
    /// spend it, spending each sink's remaining `room` as it goes, and
    /// returns how many landed in each.
    fn fill_sinks(
        &mut self,
        sinks: &mut [(Entity, ItemId, u32)],
        item: &ItemId,
        want: u32,
    ) -> Vec<(Entity, u32)> {
        let player = self.player_entity();
        let mut outstanding = want;
        let mut loaded = Vec::new();
        for (sink, supply, room) in sinks {
            if outstanding == 0 {
                break;
            }
            if supply != item {
                continue;
            }
            let moved = self
                .world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take(item.clone(), outstanding.min(*room));
            if moved == 0 {
                continue;
            }
            outstanding -= moved;
            *room -= moved;
            *self
                .world
                .get_mut::<Stock>(*sink)
                .unwrap()
                .input
                .entry(item.clone())
                .or_default() += moved;
            loaded.push((*sink, moved));
        }
        loaded
    }
}
