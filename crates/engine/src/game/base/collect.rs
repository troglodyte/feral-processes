//! Emptying adjacent structures' output buffers into the player's cargo.

use crate::*;

/// The four tiles a machine feeds and the player collects from. Named once
/// so the two rules cannot drift: the moment a collect could reach a tile a
/// machine could not, the base would stop reading as a physical line.
pub(crate) const ORTHOGONAL: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

impl Game {
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
        if self.require_surface().is_err() {
            return Vec::new();
        }
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();

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
