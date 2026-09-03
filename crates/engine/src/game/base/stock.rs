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

use bevy_ecs::prelude::{Entity, With};

use crate::Game;
use crate::components::{Position, Stock, Structure};
use crate::items::{ItemCategory, ItemId};
use crate::items_db::ItemDb;
use crate::structures::StructureDb;
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

/// Every item a deployed structure is set up to make — its `work.produces`
/// or its `assembles.item`.
///
/// **What "unlocked" means to the strip.** A pile whose tag only exists
/// while the buffer behind it is non-empty makes the row reshuffle every
/// time a hauler clears a shelf, which is the same thing sorting by
/// quantity would do and is the one thing a glanceable readout cannot
/// afford. A machine standing in the base is the base saying it makes that
/// item, so the tag holds its place at 0.
///
/// **Both halves, because an assembler declares no `work` block at all** —
/// `AssembleDef::item` is where a crafting machine says what it makes, and
/// a rule reading `work.produces` alone leaves every one of them off the
/// strip until its first unit lands.
///
/// Deliberately narrower than "any structure": a Depot makes nothing, and
/// seeding off what a building *could hold* would put a row on the row for
/// every item in the game. It is equally not the researched recipe list —
/// a bench recipe is compiled into the *player's* pack, never into a base
/// buffer, so a row for one would be a zero that could never move.
pub(crate) fn producible(game: &Game) -> impl Iterator<Item = &ItemId> {
    let structures = game.world.resource::<StructureDb>();
    game.world
        .iter_entities()
        .filter_map(|e| structures.get(&e.get::<Structure>()?.kind))
        .flat_map(|def| {
            def.work
                .iter()
                .map(|w| &w.produces)
                .chain(def.assembles.iter().map(|a| &a.item))
        })
}

/// Takes up to `qty` of `item` out of the base's own stores, and reports how
/// much it got.
///
/// **The spending half of `output_buffers`**, and deliberately the same set:
/// `work_orders::base_holding` counts these buffers and the stock strip
/// lists them, so a cost the base pays out of a *narrower* set would make
/// the strip say the base can afford something it then refuses to buy. The
/// player's own pack is not in it — that is `Inventory`, and every cost the
/// *player* incurs is paid from there.
///
/// Drained in tile order for `assembler_system`'s reason: bevy's iteration
/// order is not stable, and two shelves holding the same item would be spent
/// in a different order between runs, which is a base that saves differently
/// each time it is played. `hauling::take_from` does the arithmetic rather
/// than a second copy of it, so a buffer emptied here is *removed* rather
/// than left holding a zero.
///
/// There is one caller today — the dig crew's tile, the first cost the base
/// incurs on its own initiative rather than at the player's keypress. It
/// takes an item and a quantity anyway because that is what the arithmetic
/// needs to be honest about a partial take; a caller that must have all of
/// it checks the returned figure.
pub(crate) fn spend_from_base(
    game: &mut Game,
    item: &ItemId,
    qty: u32,
    source: crate::base_ledger::ConsumeSource,
) -> u32 {
    let mut shelves: Vec<(i32, i32, Entity)> = {
        let mut query = game
            .world
            .query_filtered::<(Entity, &Position), (With<Structure>, With<Stock>)>();
        query
            .iter(&game.world)
            .map(|(e, p)| (p.x, p.y, e))
            .collect()
    };
    shelves.sort_unstable();
    let mut taken = 0;
    for (.., shelf) in shelves {
        if taken == qty {
            break;
        }
        let Some(mut stock) = game.world.get_mut::<Stock>(shelf) else {
            continue;
        };
        taken += crate::game::base::hauling::take_from(&mut stock, item, qty - taken);
    }
    // Reported here rather than at the three callers: this is where the
    // units actually leave the shelves, and a partial take is exactly the
    // case a caller-side figure would get wrong.
    if taken > 0 {
        let id = item.0.clone();
        game.report_base(
            crate::base_ledger::Event::Consume {
                item: item.clone(),
                qty: taken,
            },
            move |tick, zone, _| crate::telemetry::Record::Consume {
                tick,
                zone,
                item: id,
                qty: taken,
                source: source.as_str().to_string(),
            },
        );
    }
    taken
}

/// Puts up to `qty` of `item` back onto the base's Depot shelves, and
/// reports how much landed.
///
/// **Not the inverse of `spend_from_base`, and deliberately narrower.** A
/// cost is drawn from every output buffer in the base, because that is what
/// the stock strip counts and a strip that promised more than the base
/// would spend is a strip lying about the base. A *refund* goes to Depots
/// alone: a unit pushed into a Mining Node's output buffer is
/// indistinguishable from a unit that node produced, and would be hauled
/// away and counted as a cycle's yield. The dig crew's substrate draw
/// already carries this same asymmetry.
///
/// Filled in tile order, `spend_from_base`'s reason: bevy's iteration order
/// is not stable, and two Depots taking a refund in a different order
/// between runs is a base that saves differently each time it is played.
/// Clamped against each Depot's `output_room` as the fill walks them, so a
/// full base simply returns a smaller figure and the caller decides what to
/// do with the remainder — this never destroys a unit it could not place.
pub(crate) fn return_to_depots(game: &mut Game, item: &ItemId, qty: u32) -> u32 {
    let mut depots: Vec<(i32, i32, Entity)> = {
        let standing: Vec<(Entity, Position, crate::structures::StructureId)> = {
            let mut query = game
                .world
                .query_filtered::<(Entity, &Position, &Structure), With<Stock>>();
            query
                .iter(&game.world)
                .map(|(e, p, s)| (e, *p, s.kind.clone()))
                .collect()
        };
        let structures = game.world.resource::<StructureDb>();
        standing
            .into_iter()
            .filter(|(_, _, kind)| structures.get(kind).is_some_and(|d| d.stores))
            .map(|(e, p, _)| (p.x, p.y, e))
            .collect()
    };
    depots.sort_unstable();
    let mut landed = 0;
    for (.., depot) in depots {
        if landed == qty {
            break;
        }
        let Some(mut stock) = game.world.get_mut::<Stock>(depot) else {
            continue;
        };
        let room = stock.output_room().min(qty - landed);
        if room == 0 {
            continue;
        }
        *stock.output.entry(item.clone()).or_insert(0) += room;
        landed += room;
    }
    landed
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
        for item in producible(self) {
            totals.entry(item).or_default();
        }
        for (_, stock) in output_buffers(self) {
            for (item, qty) in &stock.output {
                if *qty == 0 {
                    continue;
                }
                *totals.entry(item).or_default() += qty;
            }
        }
        // Folded in by the flag and never by name: `deliver_payout` sends a
        // banked item straight past its node's own `output`, so this is the
        // one holding no buffer above could ever have reported. A pool the
        // player has none of is *not* seeded, or every run would open on a
        // row for a resource nothing in the base makes yet — the same rule
        // `producible` states, applied to the one item that has no buffer to
        // stand in for it.
        for def in db.all().filter(|d| d.banked) {
            let held = self.banked(&def.id);
            if held > 0 || totals.contains_key(&def.id) {
                *totals.entry(&def.id).or_default() += held;
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
