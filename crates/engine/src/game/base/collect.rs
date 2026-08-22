//! Emptying adjacent structures' output buffers into the player's cargo.

use crate::game::base::hauling;
use crate::*;

/// The four tiles a machine feeds and the player collects from. Named once
/// so the two rules cannot drift: the moment a collect could reach a tile a
/// machine could not, the base would stop reading as a physical line.
pub(crate) const ORTHOGONAL: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

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
    /// `pub(crate)` solely so `tests/collect.rs` can pin the order directly;
    /// nothing outside this module calls it.
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

    /// What the adjacent structures are offering, pooled across all of them
    /// and in `ItemId` order — the order `Stock`'s own `BTreeMap` yields and
    /// the order the collect log already prints in, so the screen and the
    /// log line cannot disagree about how a haul is ordered.
    ///
    /// `&self`: no tick, no log, no RNG. It holds the same guards
    /// `collect_adjacent` does and answers with an empty offer for each. It
    /// is a claim about what is beside the party, so like `base_stock` it
    /// needs no `require_surface` of its own — `require_base` is the
    /// stronger statement.
    pub fn collectable_adjacent(&self) -> Vec<(ItemId, u32)> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        if self.require_base().is_err() {
            return Vec::new();
        }
        let mut offer: std::collections::BTreeMap<ItemId, u32> = std::collections::BTreeMap::new();
        for structure in self.adjacent_stock() {
            let stock = self.world.get::<Stock>(structure).unwrap();
            for (item, qty) in stock.output.iter() {
                if *qty == 0 {
                    continue;
                }
                *offer.entry(item.clone()).or_default() += qty;
            }
        }
        offer.into_iter().collect()
    }

    /// Takes an exact basket off the adjacent structures and reports what
    /// actually landed.
    ///
    /// The one taking path: `collect_adjacent` is this with everything
    /// asked for, so take-all and take-some cannot drift on clamping,
    /// logging or the tick. Units leave a buffer through
    /// `hauling::take_from` alone — its doc comment already names itself the
    /// one way, and a second remove-or-decrement is how a buffer ends up
    /// holding a zero entry every reader then has to know to skip.
    ///
    /// An over-ask is **clamped, not refused**: the buffers can only shrink
    /// between a screen opening and the commit — a raid, a hauler, an
    /// assembler pull — and a basket that has gone briefly optimistic should
    /// hand over what is there. Reporting what came rather than what was
    /// asked for is `apply_damage`'s rule: a log line printing the requested
    /// figure claims goods the player never received.
    ///
    /// One tick and one `Loot` line for the whole basket, because one commit
    /// is one action. An empty or all-zero request is a refusal — nothing
    /// taken, nothing said, no turn spent. It does not speak the "nothing to
    /// collect here" sentence either; that belongs to `collect_adjacent`,
    /// stated once.
    pub fn collect_items(&mut self, want: &[(ItemId, u32)]) -> Vec<(ItemId, u32)> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        if self.require_base().is_err() {
            return Vec::new();
        }
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

        if taken.is_empty() {
            return Vec::new();
        }
        let summary = taken
            .iter()
            .map(|(item, n)| format!("{n} {}", self.item_name(item)))
            .collect::<Vec<_>>()
            .join(", ");
        self.log_base_kind(MessageKind::Loot, format!("You collect {summary}."));
        self.tick();
        taken.into_iter().collect()
    }

    /// Empties the `output` of every structure orthogonally adjacent to the
    /// player into their cargo, and reports what was taken.
    ///
    /// The player pulls by exactly the rule a machine does, and like a
    /// machine can never reach another's `input` — a chain's directionality
    /// is the same fact whoever is doing the taking. Structures block
    /// movement, so the player always stands *beside* one rather than on it,
    /// which is what makes the symmetry work: standing in the crook of an L
    /// empties three buildings, standing at the end of a sprawled line
    /// empties one.
    ///
    /// A collect that takes nothing is a refusal and costs no turn — the
    /// base ticks on, and a misfired keypress shouldn't spend one.
    pub fn collect_adjacent(&mut self) -> Vec<(ItemId, u32)> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        if self.require_base().is_err() {
            return Vec::new();
        }
        let player = self.player_entity();
        // The party's base cell rather than their `Position`: every machine
        // with a `Stock` stands in base space, and `Position` is pinned to
        // the anchor tile on the zone surface while the party is in here.
        let Some((px, py)) = self.base_pos() else {
            return Vec::new();
        };
        let pos = Position { x: px, y: py };

        let neighbours: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), With<Stock>>();
            query
                .iter(&self.world)
                .filter(|(_, p)| {
                    ORTHOGONAL
                        .iter()
                        .any(|(dx, dy)| (p.x, p.y) == (pos.x + dx, pos.y + dy))
                })
                .map(|(e, _)| e)
                .collect()
        };

        let mut taken: std::collections::BTreeMap<ItemId, u32> = std::collections::BTreeMap::new();
        for structure in neighbours {
            let offer: Vec<(ItemId, u32)> = {
                let stock = self.world.get::<Stock>(structure).unwrap();
                stock.output.iter().map(|(i, n)| (i.clone(), *n)).collect()
            };
            for (item, qty) in offer {
                if qty == 0 {
                    continue;
                }
                // Not `grant_loot`: collecting also has to clear the source
                // structure's own stock, which a plain inventory grant
                // doesn't touch.
                self.world
                    .get_mut::<Inventory>(player)
                    .unwrap()
                    .add(item.clone(), qty);
                self.world
                    .get_mut::<Stock>(structure)
                    .unwrap()
                    .output
                    .remove(&item);
                *taken.entry(item).or_default() += qty;
            }
        }

        if taken.is_empty() {
            self.log_base("There is nothing to collect here.");
            return Vec::new();
        }
        let summary = taken
            .iter()
            .map(|(item, n)| format!("{n} {}", self.item_name(item)))
            .collect::<Vec<_>>()
            .join(", ");
        self.log_base_kind(MessageKind::Loot, format!("You collect {summary}."));
        self.tick();
        taken.into_iter().collect()
    }
}
