//! What a Depot will and will not take in — the rule, its two writers, and
//! the rows the screen that edits it draws.
//!
//! `components::DepotFilter` holds the **denied** set, so the whole feature
//! is inert until the player denies something: an absent component and an
//! empty one both mean "takes anything", which is what every Depot had
//! before this shipped.
//!
//! **`depot_accepts` is the one door.** Both things that put cargo into a
//! Depot read it — `deposit::give_to_adjacent` for the player's own hand
//! and `hauling::haul_step_system` for the crew — because a rule only one
//! of them obeyed would let the transfer screen smuggle past what the
//! screen itself says the Depot refuses. Nothing on the *take* side asks:
//! a filter says what may come in, and stock already inside is drawn off
//! by whoever wants it.
//!
//! **`stock::return_to_depots` is the deliberate third door and does not
//! ask either.** A refund is the base handing back goods it already owned —
//! a cancelled build, a route's proceeds — and `Game::return_material`'s
//! fallback ladder ends in a log line about units left in the dust when the
//! player is not in base space to catch them. Honouring a filter there
//! would let a closed shelf destroy materials while the player was out in
//! the field, which is not what anyone means by sorting their storage.

use crate::*;

/// The rule itself, over a Depot's filter rather than the Depot: the crew's
/// haul and the intake system hold a `DepotFilter` from a query and have no
/// `Game` to ask. `None` is the absent component, which takes anything.
pub(crate) fn filter_accepts(filter: Option<&DepotFilter>, item: &ItemId) -> bool {
    filter.is_none_or(|f| !f.denied.contains(item))
}

/// What a producer of kind `def` puts into its own output buffer, or `None`
/// when it makes nothing or makes a banked product, which reaches no buffer.
pub(crate) fn buffered_item<'a>(def: &'a StructureDef, items: &ItemDb) -> Option<&'a ItemId> {
    let item = crate::systems::produced_item(def)?;
    let banked = items.get(item.as_str()).is_some_and(|d| d.banked);
    (!banked).then_some(item)
}

/// The one rule for "a Depot takes from this producer": the item a producer of
/// kind `def` would hand over, if the Depot's `filter` lets it in. The pull
/// (`depot_intake_system`) and the drawn join (`Game::depot_takes_from`) both
/// ask it, so the picture cannot show a line the pull would refuse.
pub(crate) fn item_depot_takes<'a>(
    def: &'a StructureDef,
    items: &ItemDb,
    filter: Option<&DepotFilter>,
) -> Option<&'a ItemId> {
    buffered_item(def, items).filter(|item| filter_accepts(filter, item))
}

impl Game {
    /// Whether `depot` will take `item` in.
    ///
    /// True for any entity with no `DepotFilter` at all, which is what
    /// makes this safe to ask of a Mining Node's buffer or of a structure
    /// spawned by a test fixture.
    pub(crate) fn depot_accepts(&self, depot: Entity, item: &ItemId) -> bool {
        filter_accepts(self.world.get::<DepotFilter>(depot), item)
    }

    /// Denies or allows one row's items at one Depot — a single item, or
    /// every member of a group row (`filter_group`).
    ///
    /// The component is inserted lazily on the first denial and **removed
    /// again** when the last one is lifted, so "this Depot takes anything"
    /// keeps exactly one representation — the same invariant
    /// `StandingJob`'s absence keeps, and what lets the save encode the
    /// common case as an empty list.
    pub fn set_depot_filter(&mut self, depot: Entity, items: &[ItemId], allowed: bool) {
        if !self.is_depot(depot) {
            return;
        }
        let mut denied = self
            .world
            .get::<DepotFilter>(depot)
            .cloned()
            .unwrap_or_default()
            .denied;
        for item in items {
            if allowed {
                denied.remove(item);
            } else {
                denied.insert(item.clone());
            }
        }
        self.write_depot_filter(depot, denied);
    }

    /// Allows or denies every item the screen lists, in one keypress.
    ///
    /// Denying all writes the catalogue in rather than setting a flag: the
    /// stored set is what every reader asks, so a second representation of
    /// "everything" would be a second thing `depot_accepts` had to check.
    /// An item added by a mod after the fact is then allowed at that Depot
    /// until the player denies it again, which is the same answer a brand
    /// new item gets anywhere else.
    pub fn set_all_depot_filters(&mut self, depot: Entity, allowed: bool) {
        if !self.is_depot(depot) {
            return;
        }
        let denied = if allowed {
            std::collections::BTreeSet::new()
        } else {
            self.filterable_items().into_iter().collect()
        };
        self.write_depot_filter(depot, denied);
    }

    fn write_depot_filter(&mut self, depot: Entity, denied: std::collections::BTreeSet<ItemId>) {
        let mut entity = self.world.entity_mut(depot);
        if denied.is_empty() {
            entity.remove::<DepotFilter>();
        } else {
            entity.insert(DepotFilter { denied });
        }
    }

    /// Every item a Depot could ever be handed, in `ItemId` order —
    /// `ItemDb::all`'s own order, not a second sort.
    ///
    /// The screen's row list and `set_all_depot_filters`' catalogue are the
    /// same call, so "deny all" cannot deny something the player was never
    /// shown. The two exclusions are `transfer_offer`'s own: the trade
    /// currency is what cargo is priced in rather than cargo, and a banked
    /// item never reaches a shelf to be refused from one.
    pub(crate) fn filterable_items(&self) -> Vec<ItemId> {
        let currency = self.trade_currency();
        self.world
            .resource::<ItemDb>()
            .all()
            .filter(|def| !def.banked && def.id != currency)
            .map(|def| def.id.clone())
            .collect()
    }

    /// Lets the producers beside a Depot's output into it, once, when the
    /// Depot is placed — called from `spawn_structure`, the one place a
    /// structure comes into being, so a filter the player edits afterwards is
    /// never re-derived. A producer built later beside an existing Depot
    /// leaves it alone: the player has had the chance to set it.
    ///
    /// A Depot with no filter becomes allow-only for each item; one that
    /// already has a filter only gains them, so the player's denials are never
    /// widened or narrowed beyond that. A banked product reaches no buffer, so
    /// it is skipped rather than written as a filter nothing could use.
    pub(crate) fn auto_filter_depots(&mut self, built: Entity) {
        if !self.is_depot(built) {
            return;
        }
        let Some(&Position { x, y }) = self.world.get::<Position>(built) else {
            return;
        };
        let items: Vec<ItemId> = crate::game::base::collect::ORTHOGONAL
            .iter()
            .filter_map(|(dx, dy)| self.buffered_output(self.structure_at_tile((x + dx, y + dy))?))
            .collect();
        for item in items {
            if self.world.get::<DepotFilter>(built).is_none() {
                self.set_all_depot_filters(built, false);
            }
            self.set_depot_filter(built, &[item], true);
        }
    }

    /// Whether `depot` would pull `producer`'s output: it is a Depot, the
    /// producer makes something that reaches a buffer, and the filter lets it
    /// in. The worked and lit gates are the pull's alone — the drawn join is
    /// about the layout, as the machine-to-machine one is.
    pub(crate) fn depot_takes_from(&self, depot: Entity, producer: Entity) -> bool {
        self.is_depot(depot)
            && self
                .structure_def(producer)
                .and_then(|def| {
                    item_depot_takes(
                        def,
                        self.world.resource::<ItemDb>(),
                        self.world.get::<DepotFilter>(depot),
                    )
                })
                .is_some()
    }

    /// Whether `depot` could hand `consumer` an ingredient: it is a Depot, the
    /// consumer assembles something, and the filter lets at least one of that
    /// recipe's ingredients in. `assembler_system` pulls any recipe item out
    /// of any neighbour's output buffer, so a Depot beside a machine feeds it
    /// whatever the Depot holds of the recipe, and the filter is what decides
    /// whether it can ever hold any. Not gated on stock, worker or power, for
    /// `depot_takes_from`'s reason.
    pub(crate) fn depot_supplies(&self, depot: Entity, consumer: Entity) -> bool {
        let items = self.world.resource::<ItemDb>();
        self.is_depot(depot)
            && self
                .structure_def(consumer)
                .and_then(|def| crate::systems::assembly_recipe(def, items))
                .is_some_and(|recipe| {
                    let filter = self.world.get::<DepotFilter>(depot);
                    recipe.iter().any(|(item, _)| filter_accepts(filter, item))
                })
    }

    fn structure_def(&self, structure: Entity) -> Option<&StructureDef> {
        let kind = &self.world.get::<Structure>(structure)?.kind;
        self.world.resource::<StructureDb>().get(kind)
    }

    /// What `structure` puts into its own output buffer, or `None` when it
    /// makes nothing or makes a banked product.
    fn buffered_output(&self, structure: Entity) -> Option<ItemId> {
        buffered_item(
            self.structure_def(structure)?,
            self.world.resource::<ItemDb>(),
        )
        .cloned()
    }

    fn structure_at_tile(&self, tile: (i32, i32)) -> Option<Entity> {
        self.world
            .iter_entities()
            .find(|e| {
                e.contains::<Structure>() && e.get::<Position>().is_some_and(|p| (p.x, p.y) == tile)
            })
            .map(|e| e.id())
    }

    pub(crate) fn is_depot(&self, entity: Entity) -> bool {
        let Some(structure) = self.world.get::<Structure>(entity) else {
            return false;
        };
        self.world
            .resource::<StructureDb>()
            .get(&structure.kind)
            .is_some_and(|d| d.stores)
    }

    /// The adjacent Depots the filter screen can be opened on, in the same
    /// `(x, y)` order the transfer picker fills them in.
    ///
    /// `pub` because the screen holds the entity it is editing and needs
    /// the list to cycle through; a re-read on every open is what makes a
    /// Depot demolished while the screen was shut simply not be there.
    pub fn adjacent_depot_entities(&self) -> Vec<Entity> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Vec::new();
        }
        if self.require_base().is_err() {
            return Vec::new();
        }
        self.adjacent_depots()
    }

    /// One Depot's filter as the screen draws it — the header line, then a
    /// row per `filter_group` family, then a row per ungrouped item.
    ///
    /// A group row reads allowed only when every member is, so a family
    /// with any member refused shows as refused and one Right lifts it.
    ///
    /// `None` when the entity is not a standing Depot, which is what the
    /// screen reads to close itself after the building it was editing has
    /// been demolished or fallen to a raid.
    pub fn depot_filter_view(&self, depot: Entity) -> Option<DepotFilterView> {
        if !self.is_depot(depot) {
            return None;
        }
        let stock = self.world.get::<Stock>(depot);
        let held = |item: &ItemId| stock.and_then(|s| s.output.get(item).copied()).unwrap_or(0);
        let db = self.world.resource::<ItemDb>();
        let mut groups: Vec<(&'static str, Vec<ItemId>)> =
            filter_groups().map(|name| (name, Vec::new())).into();
        let mut singles = Vec::new();
        for item in self.filterable_items() {
            match db.get(item.as_str()).and_then(filter_group) {
                Some(name) => groups
                    .iter_mut()
                    .find(|(g, _)| *g == name)
                    .expect("filter_group names only filter_groups")
                    .1
                    .push(item),
                None => singles.push((self.item_name(&item).to_string(), vec![item])),
            }
        }
        let rows = groups
            .into_iter()
            .filter(|(_, items)| !items.is_empty())
            .map(|(name, items)| (name.to_string(), items))
            .chain(singles)
            .map(|(name, items)| DepotFilterRow {
                name,
                held: items.iter().map(held).sum(),
                allowed: items.iter().all(|i| self.depot_accepts(depot, i)),
                items,
            })
            .collect();
        let pos = self.world.get::<Position>(depot)?;
        Some(DepotFilterView {
            tile: (pos.x, pos.y),
            room: stock.map(|s| s.output_room()).unwrap_or(0),
            rows,
        })
    }
}

/// The family the synthesised `etched_*` disks fold into.
const ETCHED_DISKS: &str = "Etched Disks";

/// The families the filter screen folds into one row each, in the order
/// they head the list. A player sorting a shelf means "gear" or "disks",
/// never one weapon of forty, and the etched family alone is a disk per
/// routine in the catalogue. The gear rows take `ItemCategory::heading`,
/// the wagon's own words for the same runs.
fn filter_groups() -> [&'static str; 4] {
    [
        ItemCategory::Weapon.heading(),
        ItemCategory::Armor.heading(),
        ItemCategory::Module.heading(),
        ETCHED_DISKS,
    ]
}

/// Which `filter_groups` row an item folds into, if any. Derived, never
/// authored, so a modded weapon or a routine's synthesised disk joins its
/// row without a field saying so. The disk test comes first: it is the id
/// that marks the family, whatever the def's category reads.
fn filter_group(def: &ItemDef) -> Option<&'static str> {
    if def.id.etched_ability().is_some() {
        return Some(ETCHED_DISKS);
    }
    match def.category() {
        c @ (ItemCategory::Weapon | ItemCategory::Armor | ItemCategory::Module) => {
            Some(c.heading())
        }
        _ => None,
    }
}
