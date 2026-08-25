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

    /// Gives an exact basket to the adjacent Depots and reports what
    /// actually landed.
    ///
    /// The one giving path: `deposit_adjacent` is this with everything
    /// offered, so put-all and put-some cannot drift on clamping, logging
    /// or the tick.
    ///
    /// Clamped **twice**, and neither clamp is a refusal: against what the
    /// player actually holds, and against `output_room()`, per Depot, as
    /// the fill walks them in `(x, y)` order. Room is checked **before**
    /// anything leaves the pack — taking from the pack first and clamping
    /// the write after would let a full base silently eat cargo the player
    /// never got back. Never past `capacity`, the same ceiling
    /// `hauling::deposit` writes under: an over-capacity write would make
    /// that field a suggestion, and a full Depot is a decided failure mode
    /// rather than an exception to one.
    ///
    /// Units leave the pack through `Inventory::take` alone, never a second
    /// decrement — it already clamps to what is held and drops a slot that
    /// reaches zero, so its return value *is* the pack-side clamp. Reporting
    /// what landed rather than what was asked for is `apply_damage`'s rule:
    /// a log line printing the requested figure claims goods the base never
    /// received.
    ///
    /// One tick and one log line for the whole basket, because one commit
    /// is one action. An empty or all-zero request is a no-op — nothing
    /// moved, nothing said, no turn spent. It does not speak either refusal
    /// sentence; those belong to `deposit_adjacent`, stated once.
    pub fn deposit_items(&mut self, give: &[(ItemId, u32)]) -> Vec<(ItemId, u32)> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        if self.require_base().is_err() {
            return Vec::new();
        }
        let given = self.give_to_adjacent(give);
        if given.is_empty() {
            return Vec::new();
        }
        let summary = given
            .iter()
            .map(|(item, n)| format!("{n} {}", self.item_name(item)))
            .collect::<Vec<_>>()
            .join(", ");
        self.log_base(format!("You put away {summary}."));
        self.tick();
        given
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

    /// Gives everything the pack is offering, and reports what was taken.
    ///
    /// The offer through `depositable`, the give through `deposit_items` —
    /// one giving path rather than two that could drift on clamping,
    /// logging or the tick.
    ///
    /// **The two refusals live here and nowhere else**, because they leave
    /// the player different errands: no adjacent Depot at all, or a Depot
    /// with nothing in the pack to put in it. A Depot with no *room* is not
    /// a refusal — `deposit_items` clamps to zero and its log line already
    /// says nothing landed. The guards come first and refuse *silently*, as
    /// they always have: an action taken during a battle or from the
    /// surface is not the base telling you its shelves are full.
    pub fn deposit_adjacent(&mut self) -> Vec<(ItemId, u32)> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        if self.require_base().is_err() {
            return Vec::new();
        }
        if self.adjacent_depots().is_empty() {
            self.log_base("There is nowhere here to put anything.");
            return Vec::new();
        }
        let offer = self.depositable();
        if offer.is_empty() {
            self.log_base("You have nothing to put away.");
            return Vec::new();
        }
        self.deposit_items(&offer)
    }
}
