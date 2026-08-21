//! What the base is holding, and the one walk that decides which buffers
//! count as holding it.
//!
//! Three questions read the same set of buffers — how much of one item the
//! whole base has (`work_orders::base_holding`), how much of it a hauler
//! could fetch from a Depot (`work_orders::depot_holding`), and what the
//! stock strip lists across the top of every non-battle screen
//! (`Game::base_stock`). They ask different things of it, but "which
//! buffers are the base's" has to be one answer or the strip becomes a
//! second opinion about the base rather than a readout of it.
//!
//! An **output** buffer, never an input one: `Stock`'s asymmetry is the
//! whole of a chain's directionality (see `CLAUDE.md`), and an ingredient
//! already committed to a machine's hopper is spent from the base's point
//! of view even though it has not been consumed yet.

use crate::Game;
use crate::components::{Stock, Structure};
use crate::items::{ItemCategory, ItemId};
use crate::items_db::ItemDb;
use crate::views::StockRow;

/// Every deployed structure's output buffer, paired with the structure it
/// belongs to so a caller that cares which kind it is can ask.
///
/// The one statement of what "the base is holding" reads.
pub(crate) fn output_buffers(game: &Game) -> impl Iterator<Item = (&Structure, &Stock)> {
    game.world
        .iter_entities()
        .filter_map(|e| Some((e.get::<Structure>()?, e.get::<Stock>()?)))
}

impl Game {
    /// What the base's machines and depots are holding, one row per item,
    /// as the stock strip lists it.
    ///
    /// Keyed by `ItemId` in a `BTreeMap` for `Stock`'s own reason: the
    /// order has to be the same every tick. A strip that re-sorted as
    /// buffers filled and drained would move every tag under the eye of
    /// the player reading it, which is the one thing a glanceable readout
    /// cannot do.
    ///
    /// Listed down to `Material` and `Currency` through the existing
    /// `ItemDef::category` derivation rather than a second predicate. The
    /// strip is one row wide and a weapon in a buffer would cost a pile off
    /// the end of it to say something the gear screens say better.
    ///
    /// Makes no claim about where the party is standing — it is about the
    /// base, the way a Broker's board is — so it needs no
    /// `require_surface` and reads the same four frames down the Stack.
    pub fn base_stock(&self) -> Vec<StockRow> {
        let db = self.world.resource::<ItemDb>();
        let mut totals: std::collections::BTreeMap<&ItemId, u32> =
            std::collections::BTreeMap::new();
        for (_, stock) in output_buffers(self) {
            for (item, qty) in &stock.output {
                if *qty == 0 {
                    continue;
                }
                *totals.entry(item).or_default() += qty;
            }
        }

        totals
            .into_iter()
            .filter_map(|(item, qty)| {
                let def = db.get(item.as_str())?;
                matches!(
                    def.category(),
                    ItemCategory::Material | ItemCategory::Currency
                )
                .then(|| StockRow {
                    item: item.clone(),
                    tag: def.tag(),
                    name: def.name.clone(),
                    qty,
                })
            })
            .collect()
    }
}
