use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::FieldBuffKind;
use crate::items::{EquipmentSlot, EquipmentStats, ItemCategory, ItemId};
use crate::species::SpeciesId;
use crate::structures::StructureId;

/// A singleton economy anchor. The game has exactly one item per role;
/// engine logic queries "the item with role X" instead of naming an id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EconomyRole {
    Currency,
    ResearchCurrency,
    CraftCurrency,
    /// What a trader pays and charges. Deliberately *not* `Currency`: the
    /// build economy runs on salvage a trader has no reason to hand out, and
    /// this is the only currency that survives a zone breach (see
    /// `Game::breach_portal`), which is why no trader may deal in the
    /// `Currency` or `CraftCurrency` item — see
    /// `StructureDb::strip_reserved_trade_goods`.
    TradeCurrency,
}

/// What `Game::use_item` does out of battle. All fields optional so one item
/// can restore several resources and/or arm a pre-battle buff.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ConsumeDef {
    #[serde(default)]
    pub power: f32,
    #[serde(default)]
    pub heal: i32,
    #[serde(default)]
    pub prebattle_buff: Option<PrebattleBuff>,
}

/// Arms a `FieldBuff` that survives on the map, through any battle that
/// follows, and through a save (see `Game::arm_field_buff`) — unlike
/// `CombatBuff`, which is wiped the moment a battle ends.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PrebattleBuff {
    pub kind: FieldBuffKind,
    pub power: i32,
    /// Game ticks, not battle rounds — see `ActiveFieldBuff::remaining`.
    pub ticks: u32,
}

/// What `Game::refactor_companion` does to a tamed program. Magnitudes live
/// here rather than in `tuning.rs` so a new upgrade item is a `.ron` file and
/// never a code change — only the per-companion slot cap is tuning.
///
/// Percentages rather than flat amounts because a companion's stats keep
/// growing across breaches, and because `×1.05` commutes with the `zone_bump`'s
/// `×ZONE_STAT_GROWTH` — so there is no ordering a player can exploit.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct CompanionUpgradeDef {
    #[serde(default)]
    pub hp_percent: f32,
    #[serde(default)]
    pub atk_percent: f32,
    #[serde(default)]
    pub def_percent: f32,
    /// Raises the program one zone tier, never above the player's own. Costs
    /// no upgrade slot — see `tuning::MAX_COMPANION_REFACTORS`.
    #[serde(default)]
    pub zone_bump: bool,
}

impl CompanionUpgradeDef {
    /// Whether this upgrade spends one of the companion's bounded slots. A
    /// bump bounds itself against the player's zone, so only the percentages
    /// need the cap.
    pub fn spends_a_slot(&self) -> bool {
        self.hp_percent != 0.0 || self.atk_percent != 0.0 || self.def_percent != 0.0
    }
}

/// A craft recipe declared by the item itself, replacing the two
/// formerly-hardcoded starter recipes. With no `requires_structure` it is
/// always available; naming a bench gates it on that structure standing,
/// the same way a researched recipe is gated (see `Game::craft_recipes`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CraftableDef {
    pub cost: Vec<(ItemId, u32)>,
    #[serde(default)]
    pub requires_structure: Option<StructureId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: ItemId,
    pub name: String,
    /// One line on what this item is for, shown wherever the item is listed.
    /// Authored here rather than derived from the other fields so a modder
    /// controls exactly how their item reads. `#[serde(default)]` so an
    /// existing mod file without it still parses — as an empty line, which
    /// the shipped-assets test refuses for anything in this repo.
    #[serde(default)]
    pub description: String,
    /// Marks this item as banked currency rather than ordinary cargo — see
    /// `Inventory::cargo_used`. `#[serde(default)]` so an existing mod file
    /// without the field still parses, as ordinary cargo.
    #[serde(default)]
    pub banked: bool,
    /// What one unit is worth in trade currency, before any trader's own
    /// `TradeDef::sell_rate` multiplier. Read through `Game::item_value`,
    /// never directly, so selling and the buyback shelf cannot disagree
    /// about a price. `#[serde(default)]` so an older mod file still parses
    /// — as `None`, which resolves to `tuning::DEFAULT_ITEM_VALUE`.
    #[serde(default)]
    pub value: Option<u32>,
    #[serde(default)]
    pub role: Option<EconomyRole>,
    #[serde(default)]
    pub equipment: Option<(EquipmentSlot, EquipmentStats)>,
    #[serde(default)]
    pub taming_potency: Option<f32>,
    #[serde(default)]
    pub consume: Option<ConsumeDef>,
    #[serde(default)]
    pub craftable: Option<CraftableDef>,
    /// Species that drop this item, each with its own 0.0-1.0 chance. The
    /// inverse of `SpeciesDef::equipment_drop`: an item names its sources
    /// instead of every species naming the item. Both are honoured and
    /// merged per kill (see `Game::equipment_drops_for`).
    #[serde(default)]
    pub droppable: Option<Vec<(SpeciesId, f32)>>,
    /// Chance, 0.0-1.0, that a Stack cache holds this item — see
    /// `Game::open_cache`. Rolled once per cache per declaring item, so the
    /// expected haul is the sum across the item set rather than a pick from
    /// a list, and a mod adding items adds to what caches can contain
    /// without touching engine code.
    #[serde(default)]
    pub cache_drop: Option<f32>,
    /// What this item does to a tamed program through
    /// `Game::refactor_companion`. `#[serde(default)]` like every other
    /// optional field, so an existing mod's items keep parsing as ordinary
    /// cargo.
    #[serde(default)]
    pub upgrade: Option<CompanionUpgradeDef>,
    /// A passive routine this item grants while it is worn, by ability id.
    ///
    /// Never written into the wearer's `Routines` — `Game::ready_passives`
    /// reads `Equipment` as a second source at fire time, so taking the item
    /// off ends the passive by omission and nothing about it reaches the
    /// save. Refused at load if the id names no ability or names one that
    /// could never fire; see `ItemDb::load_dir`.
    #[serde(default)]
    pub grants: Option<crate::abilities::AbilityId>,
}

impl ItemDef {
    /// Which group this item lists under. Checked in this order because the
    /// first match wins and the one overlap has a right answer: something
    /// both wearable and drinkable belongs in the gear list a player is
    /// scanning for gear.
    ///
    /// Total by construction — an item declaring none of these fields is
    /// salvage, which is what most loot is.
    pub fn category(&self) -> ItemCategory {
        if let Some((slot, _)) = self.equipment {
            return match slot {
                EquipmentSlot::Weapon => ItemCategory::Weapon,
                EquipmentSlot::Armor => ItemCategory::Armor,
                EquipmentSlot::Module => ItemCategory::Module,
            };
        }
        if self.consume.is_some() {
            return ItemCategory::Consumable;
        }
        if self.role.is_some() {
            return ItemCategory::Currency;
        }
        ItemCategory::Material
    }

    /// Names the first field whose value is not usable, if any. RON accepts
    /// bare `NaN`/`inf` literals, and they survive every clamp downstream —
    /// a NaN `taming_potency` outranks every real catalyst and then panics
    /// the RNG. Cheaper to refuse the file at load, like any other malformed
    /// one, than to defend every read.
    fn non_finite_field(&self) -> Option<&'static str> {
        if self.taming_potency.is_some_and(|p| !p.is_finite()) {
            return Some("taming_potency");
        }
        if let Some(sources) = &self.droppable
            && sources.iter().any(|(_, chance)| !chance.is_finite())
        {
            return Some("droppable chance");
        }
        if self.cache_drop.is_some_and(|chance| !chance.is_finite()) {
            return Some("cache_drop");
        }
        if let Some(u) = self.upgrade {
            // Negative is refused as well as non-finite, and that is not
            // symmetry for its own sake: `Game::refactor_companion` floors
            // every percentage at `+1`, so a `-10.0` would *raise* the stat
            // by a point while spending one of the five permanent upgrade
            // slots. A downgrade item is a coherent thing to want and this
            // is not it, so the file is refused rather than half-honoured.
            for (name, pct) in [
                ("upgrade.hp_percent", u.hp_percent),
                ("upgrade.atk_percent", u.atk_percent),
                ("upgrade.def_percent", u.def_percent),
            ] {
                if !pct.is_finite() || pct < 0.0 {
                    return Some(name);
                }
            }
            // An `upgrade` block that does nothing would be taken out of the
            // player's cargo, change no stat, spend no slot, and log a
            // success line naming the numbers it did not move.
            if !u.spends_a_slot() && !u.zone_bump {
                return Some("upgrade (declares no effect)");
            }
        }
        match self.consume {
            Some(c) if !c.power.is_finite() => Some("consume.power"),
            _ => None,
        }
    }

    /// Why this item's `grants` could never fire, if it couldn't. Skipping
    /// the item rather than dropping the field, because an item whose whole
    /// point is the routine it carries is worth less than nothing if it
    /// silently carries none — the same call `passive_field_mismatch` makes
    /// about an ability that can never reach its trigger.
    ///
    /// A field-only routine needs no arm of its own: `AbilityDb::load_dir`
    /// refuses a `triggers` on one, so nothing field-only is ever passive.
    fn ungrantable_ability(&self, abilities: &crate::abilities::AbilityDb) -> Option<String> {
        let id = self.grants.as_ref()?;
        match abilities.get(id) {
            None => Some(format!("grants: no ability {id:?}")),
            Some(def) if !def.is_passive() => Some(format!(
                "grants: {id:?} is chosen on a turn and has no trigger to fire on"
            )),
            Some(_) => None,
        }
    }
}

#[derive(Resource, Default)]
pub struct ItemDb {
    items: HashMap<String, ItemDef>,
    currency: Option<ItemId>,
    research_currency: Option<ItemId>,
    craft_currency: Option<ItemId>,
    trade_currency: Option<ItemId>,
}

impl ItemDb {
    /// Loads every `*.ron` item definition in `dir`. A malformed file is
    /// skipped with a returned warning rather than aborting the load, same
    /// as `StructureDb::load_dir`. A duplicated economy role also warns and
    /// keeps the first-seen holder.
    ///
    /// Takes the `AbilityDb` for the one check `ItemDef` cannot make on its
    /// own — whether a `grants` id names an ability that can actually fire.
    /// `SpeciesDb::load_dir` and `ResearchDb::load_dir` take their
    /// cross-database dependency the same way, and `game::lifecycle` already
    /// loads abilities before items, so nothing reorders for this.
    pub fn load_dir(
        dir: &Path,
        abilities: &crate::abilities::AbilityDb,
    ) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = ItemDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<ItemDef>(&text) {
                Ok(def) => {
                    if let Some(field) = def.non_finite_field() {
                        warnings.push(format!(
                            "skipped invalid item file {path:?}: {field} is not a finite number"
                        ));
                        continue;
                    }
                    if let Some(reason) = def.ungrantable_ability(abilities) {
                        warnings.push(format!("skipped invalid item file {path:?}: {reason}"));
                        continue;
                    }
                    if let Some(role) = def.role {
                        let slot = match role {
                            EconomyRole::Currency => &mut db.currency,
                            EconomyRole::ResearchCurrency => &mut db.research_currency,
                            EconomyRole::CraftCurrency => &mut db.craft_currency,
                            EconomyRole::TradeCurrency => &mut db.trade_currency,
                        };
                        if let Some(existing) = slot {
                            warnings.push(format!(
                                "item {} claims role {role:?} already held by {}; ignoring",
                                def.id.as_str(),
                                existing.as_str()
                            ));
                        } else {
                            *slot = Some(def.id.clone());
                        }
                    }
                    db.items.insert(def.id.0.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid item file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    /// Derives one etched Routine Disk item per loaded ability and returns
    /// a warning for every id a real `.ron` file had already claimed.
    ///
    /// **Derived rather than authored** because the disk set is a function
    /// of the ability set. Sixty-six hand-written files would be sixty-six
    /// chances for the two to drift, and nothing in the suite would notice
    /// a disk whose ability had been deleted — where this cannot produce
    /// one at all.
    ///
    /// What is left at its default is doing as much work as what is set.
    /// No `craftable`, so a disk press cannot make one; no `cache_drop`, so
    /// a Stack cache cannot hold one; no `equipment`, so it cannot leak into
    /// `Game::surface_boss_loot`, which filters on `equipment.is_some()`.
    /// That is the whole of what keeps an exclusive routine exclusive
    /// without a single explicit exclusion — the disk is reachable only
    /// where something reaches for it by id.
    ///
    /// `droppable` is carried straight across from `AbilityDef::boss_drop`,
    /// which is why the boss path needs no engine code:
    /// `Game::equipment_drops_for` already merges every item naming the dead
    /// species and `award_loot` already rolls them.
    ///
    /// A modder's own `etched_*.ron` wins and is warned about, the same call
    /// `load_dir` makes about a duplicated economy role: a file on disk is a
    /// deliberate act, and silently overwriting one would make the conflict
    /// invisible.
    pub fn synthesise_etched_disks(
        &mut self,
        abilities: &crate::abilities::AbilityDb,
    ) -> Vec<String> {
        let mut warnings = Vec::new();
        for ability in abilities.all() {
            let id = ItemId::etched(&ability.id);
            if self.items.contains_key(id.as_str()) {
                warnings.push(format!(
                    "item file {} shadows the etched disk derived for ability {}; \
                     the file wins and the routine cannot be installed from a derived disk",
                    id.as_str(),
                    ability.id
                ));
                continue;
            }
            let value = if ability.exclusive {
                crate::tuning::ETCHED_DISK_EXCLUSIVE_VALUE
            } else {
                crate::tuning::ETCHED_DISK_VALUE
            };
            // Every field spelled out rather than `..Default::default()`, so
            // a new `ItemDef` field is a compile error here and someone has
            // to decide what an etched disk does about it. The four `None`s
            // below are load-bearing and a silent default would hide that.
            self.items.insert(
                id.0.clone(),
                ItemDef {
                    id,
                    name: format!("Etched Disk · {}", ability.name),
                    // The ability's own line, so a disk in cargo reads as
                    // what it will do rather than as what it is. A player
                    // deciding which of three disks to burn a slot on is
                    // asking the routine's question, not the item's.
                    description: ability.description.clone(),
                    banked: false,
                    value: Some(value),
                    role: None,
                    equipment: None,
                    taming_potency: None,
                    consume: None,
                    craftable: None,
                    droppable: ability.boss_drop.clone(),
                    cache_drop: None,
                    // A disk *installs* its routine; it is not worn, so
                    // there is nothing for a worn grant to hang off.
                    grants: None,
                    upgrade: None,
                },
            );
        }
        warnings
    }

    pub fn get(&self, id: &str) -> Option<&ItemDef> {
        self.items.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &ItemDef> {
        let mut defs: Vec<&ItemDef> = self.items.values().collect();
        defs.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        defs.into_iter()
    }

    pub fn currency(&self) -> Option<&ItemId> {
        self.currency.as_ref()
    }

    pub fn research_currency(&self) -> Option<&ItemId> {
        self.research_currency.as_ref()
    }

    pub fn craft_currency(&self) -> Option<&ItemId> {
        self.craft_currency.as_ref()
    }

    pub fn trade_currency(&self) -> Option<&ItemId> {
        self.trade_currency.as_ref()
    }

    /// Human-readable names of any economy role with no holder — empty when
    /// the item set is complete. `Game::new`/`load` abort if this is
    /// non-empty (the economy can't run with an anchor missing).
    pub fn missing_roles(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.currency.is_none() {
            missing.push("Currency");
        }
        if self.research_currency.is_none() {
            missing.push("ResearchCurrency");
        }
        if self.craft_currency.is_none() {
            missing.push("CraftCurrency");
        }
        if self.trade_currency.is_none() {
            missing.push("TradeCurrency");
        }
        missing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn assets_items_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/items")
    }

    /// Writes `files` (filename, RON) to a unique scratch dir and loads them.
    fn load_fixture(files: &[(&str, &str)]) -> (ItemDb, Vec<String>) {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "feral_itemdb_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (n, b) in files {
            std::fs::write(dir.join(n), b).unwrap();
        }
        let (abilities, _) =
            crate::abilities::AbilityDb::load_dir(&assets_items_dir().with_file_name("abilities"))
                .unwrap();
        let out = ItemDb::load_dir(&dir, &abilities).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        out
    }

    #[test]
    fn an_item_with_a_non_finite_number_is_skipped_rather_than_loaded() {
        // NaN survives every clamp downstream and reaches `random_bool`,
        // which panics — and `total_cmp` ranks it above every real catalyst,
        // so it would win the taming roll it then crashes.
        let (db, warnings) = load_fixture(&[
            (
                "nan.ron",
                r#"(id: "nan", name: "NaN", taming_potency: Some(NaN))"#,
            ),
            (
                "inf.ron",
                r#"(id: "inf", name: "Inf", consume: Some((power: inf)))"#,
            ),
            (
                "ok.ron",
                r#"(id: "ok", name: "Ok", taming_potency: Some(0.5))"#,
            ),
        ]);

        assert!(db.get("nan").is_none(), "a NaN potency must not load");
        assert!(db.get("inf").is_none(), "an infinite restore must not load");
        assert!(db.get("ok").is_some(), "a valid neighbour still loads");
        assert_eq!(warnings.len(), 2, "each skip warns: {warnings:?}");
    }

    /// Category is derived from the fields an item already declares, so a
    /// modded item is grouped without its author adding anything.
    #[test]
    fn an_items_category_comes_off_the_fields_it_already_declares() {
        let (db, warnings) = load_fixture(&[
            (
                "w.ron",
                r#"(id: "w", name: "W", equipment: Some((Weapon, (atk: 1))))"#,
            ),
            (
                "a.ron",
                r#"(id: "a", name: "A", equipment: Some((Armor, (def: 1))))"#,
            ),
            (
                "m.ron",
                r#"(id: "m", name: "M", equipment: Some((Module, (decompiler: 1))))"#,
            ),
            (
                "c.ron",
                r#"(id: "c", name: "C", consume: Some((power: 5.0)))"#,
            ),
            (
                "cur.ron",
                r#"(id: "cur", name: "Cur", role: Some(TradeCurrency))"#,
            ),
            ("mat.ron", r#"(id: "mat", name: "Mat")"#),
        ]);
        assert!(warnings.is_empty(), "fixtures must load: {warnings:?}");

        let cat = |id: &str| db.get(id).unwrap().category();
        assert_eq!(cat("w"), ItemCategory::Weapon);
        assert_eq!(cat("a"), ItemCategory::Armor);
        assert_eq!(cat("m"), ItemCategory::Module);
        assert_eq!(cat("c"), ItemCategory::Consumable);
        assert_eq!(cat("cur"), ItemCategory::Currency);
        assert_eq!(
            cat("mat"),
            ItemCategory::Material,
            "an item declaring nothing is salvage, not a panic"
        );
    }

    /// The one ordering that isn't obvious: an item that is both wearable
    /// and drinkable belongs in its slot, because that is the list a player
    /// looking for gear will scan.
    #[test]
    fn an_equippable_consumable_is_filed_under_its_slot() {
        let (db, _) = load_fixture(&[(
            "both.ron",
            r#"(id: "both", name: "Both", equipment: Some((Armor, (def: 1))), consume: Some((power: 5.0)))"#,
        )]);
        assert_eq!(db.get("both").unwrap().category(), ItemCategory::Armor);
    }

    #[test]
    fn the_shipped_items_load_cleanly_with_all_roles_and_fields() {
        let (abilities, _) =
            crate::abilities::AbilityDb::load_dir(&assets_items_dir().with_file_name("abilities"))
                .unwrap();
        let (db, warnings) = ItemDb::load_dir(&assets_items_dir(), &abilities).unwrap();
        assert!(
            warnings.is_empty(),
            "shipped items should parse clean: {warnings:?}"
        );
        assert!(db.missing_roles().is_empty(), "every role must be held");
        assert_eq!(db.currency().unwrap(), &ItemId::from("core_fragment"));
        assert_eq!(
            db.research_currency().unwrap(),
            &ItemId::from("research_data")
        );
        assert_eq!(
            db.craft_currency().unwrap(),
            &ItemId::from("portal_fragment")
        );
        assert_eq!(db.trade_currency().unwrap(), &ItemId::from("credits"));
        assert!(db.get("research_data").unwrap().banked);
        assert_eq!(db.get("ice_breaker").unwrap().taming_potency, Some(0.4));
        assert_eq!(db.get("power_cell").unwrap().consume.unwrap().power, 25.0);

        // Banking is what exempts an item from the cargo cap (see
        // `Inventory::cargo_used`), so a second banked item would silently
        // widen the buffer the player is supposed to be squeezed by.
        let banked: Vec<&str> = db
            .all()
            .filter(|d| d.banked)
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(banked, ["research_data"], "only Research Data is banked");

        // (id, slot, atk, def, decompiler) for every equippable that ships.
        let equipment = [
            // Research-gated tier.
            ("monofilament_whip", EquipmentSlot::Weapon, 4, 0, 0),
            ("overclock_core", EquipmentSlot::Weapon, 3, 0, 0),
            ("firewall_plating", EquipmentSlot::Armor, 0, 3, 0),
            ("ablative_plating", EquipmentSlot::Armor, 0, 4, 0),
            ("neural_amplifier", EquipmentSlot::Module, 0, 0, 2),
            ("cortex_hack", EquipmentSlot::Module, 0, 0, 3),
            // Scavenged tier — no bench.
            ("shiv_routine", EquipmentSlot::Weapon, 1, 0, 0),
            ("kinetic_edge", EquipmentSlot::Weapon, 2, 0, 0),
            ("scrap_ward", EquipmentSlot::Armor, 0, 1, 0),
            ("packet_buffer", EquipmentSlot::Armor, 0, 2, 0),
            ("probe_service", EquipmentSlot::Module, 0, 0, 1),
            ("handshake_forge", EquipmentSlot::Module, 0, 0, 2),
            // Standard tier — bench, Core Fragments.
            ("arc_lance", EquipmentSlot::Weapon, 3, 0, 0),
            ("recursion_blade", EquipmentSlot::Weapon, 2, 1, 0),
            ("shim_blade", EquipmentSlot::Weapon, 2, 0, 1),
            ("hardened_shell", EquipmentSlot::Armor, 0, 3, 0),
            ("null_weave", EquipmentSlot::Armor, 1, 2, 0),
            ("static_mesh", EquipmentSlot::Armor, 0, 2, 1),
            ("trace_sniffer", EquipmentSlot::Module, 0, 0, 3),
            ("logic_probe", EquipmentSlot::Module, 1, 0, 2),
            ("entropy_damper", EquipmentSlot::Module, 0, 1, 2),
            ("sync_governor", EquipmentSlot::Module, 1, 1, 1),
            // Premium tier — bench, Portal Fragments.
            ("plasma_router", EquipmentSlot::Weapon, 4, 0, 0),
            ("black_ice_pick", EquipmentSlot::Weapon, 3, 0, 2),
            ("siege_compiler", EquipmentSlot::Weapon, 3, 2, 0),
            ("bastion_lattice", EquipmentSlot::Armor, 0, 4, 0),
            ("phase_carapace", EquipmentSlot::Armor, 2, 3, 0),
            ("nullsteel_plate", EquipmentSlot::Armor, 0, 3, 2),
            ("kernel_key", EquipmentSlot::Module, 0, 0, 4),
            ("oracle_core", EquipmentSlot::Module, 2, 0, 3),
            ("singularity_matrix", EquipmentSlot::Module, 3, 3, 3),
        ];
        for (id, want_slot, atk, def, decompiler) in equipment {
            let (slot, stats) = db.get(id).unwrap().equipment.unwrap();
            assert_eq!(slot, want_slot, "{id} slot");
            assert_eq!(
                (stats.atk, stats.def, stats.decompiler),
                (atk, def, decompiler),
                "{id} stats"
            );
        }
        assert_eq!(
            db.all().filter(|d| d.equipment.is_some()).count(),
            equipment.len(),
            "an equippable not in the table above is unpinned"
        );
        assert_eq!(db.all().count(), 54);
    }

    #[test]
    fn a_malformed_file_is_skipped_with_a_warning_not_a_panic() {
        let (db, warnings) = load_fixture(&[
            ("good.ron", r#"(id: "good", name: "Good")"#),
            ("bad.ron", "(id: \"bad\", name:"),
        ]);
        assert_eq!(db.all().count(), 1);
        assert!(warnings.iter().any(|w| w.contains("bad.ron")));
    }

    #[test]
    fn a_duplicated_role_warns_and_keeps_the_first_holder() {
        let (db, warnings) = load_fixture(&[
            ("a.ron", r#"(id: "a", name: "A", role: Some(Currency))"#),
            ("b.ron", r#"(id: "b", name: "B", role: Some(Currency))"#),
        ]);
        assert!(warnings.iter().any(|w| w.contains("role")));
        assert!(db.currency().is_some());
    }

    /// `requires_structure` and `droppable` were added after the item schema
    /// shipped. Both default, so a mod's item file written before them keeps
    /// loading — an always-available recipe and no drops.
    #[test]
    fn an_item_file_predating_the_newer_fields_still_loads() {
        let (db, warnings) = load_fixture(&[(
            "old.ron",
            r#"(id: "old", name: "Old", craftable: Some((cost: [("core_fragment", 2)])))"#,
        )]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let def = db.get("old").unwrap();
        assert!(def.craftable.as_ref().unwrap().requires_structure.is_none());
        assert!(def.droppable.is_none());
    }

    #[test]
    fn a_droppable_entry_can_name_its_bench_and_sources() {
        let (db, warnings) = load_fixture(&[(
            "gear.ron",
            r#"(
                id: "gear",
                name: "Gear",
                equipment: Some((Weapon, (atk: 2, def: 1))),
                craftable: Some((cost: [("core_fragment", 5)], requires_structure: Some("fabricator"))),
                droppable: Some([("scrapper", 0.15), ("worm", 0.05)]),
            )"#,
        )]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let def = db.get("gear").unwrap();
        assert_eq!(
            def.craftable
                .as_ref()
                .unwrap()
                .requires_structure
                .as_deref(),
            Some("fabricator")
        );
        assert_eq!(
            def.droppable.as_deref(),
            Some(&[("scrapper".to_string(), 0.15), ("worm".to_string(), 0.05)][..])
        );
        let (_, stats) = def.equipment.unwrap();
        assert_eq!((stats.atk, stats.def), (2, 1), "hybrids stack two stats");
    }

    /// The engine floors a percentage gain at `+1`, so a negative one would
    /// quietly become a *raise* that also costs a permanent upgrade slot —
    /// and an `upgrade` block declaring nothing at all would spend the item
    /// for no effect while logging a success. Both are refused at load, so
    /// the guarantee holds for a mod and not merely for the shipped assets
    /// that `every_shipped_upgrade_item_says_what_it_does` censuses.
    #[test]
    fn an_upgrade_that_would_do_nothing_or_the_wrong_thing_is_skipped() {
        for (field, ron) in [
            (
                "upgrade.hp_percent",
                r#"upgrade: Some((hp_percent: -10.0))"#,
            ),
            (
                "upgrade.atk_percent",
                r#"upgrade: Some((atk_percent: -1.0))"#,
            ),
            (
                "upgrade.def_percent",
                r#"upgrade: Some((def_percent: -0.5))"#,
            ),
            ("declares no effect", r#"upgrade: Some(())"#),
            (
                "declares no effect",
                r#"upgrade: Some((hp_percent: 0.0, zone_bump: false))"#,
            ),
        ] {
            let (db, warnings) =
                load_fixture(&[("bad.ron", &format!(r#"(id: "bad", name: "Bad", {ron})"#))]);
            assert_eq!(db.all().count(), 0, "{field}: the whole file is refused");
            assert!(
                warnings.iter().any(|w| w.contains(field)),
                "{field}: {warnings:?}"
            );
        }
    }

    /// A non-finite drop chance would reach `random_bool` and panic the RNG,
    /// the same hazard `taming_potency` already guards against.
    #[test]
    fn an_item_with_a_non_finite_drop_chance_is_skipped() {
        let (db, warnings) = load_fixture(&[(
            "bad.ron",
            r#"(id: "bad", name: "Bad", droppable: Some([("scrapper", NaN)]))"#,
        )]);
        assert_eq!(db.all().count(), 0, "the whole file is refused");
        assert!(
            warnings.iter().any(|w| w.contains("droppable")),
            "{warnings:?}"
        );
    }

    #[test]
    fn an_item_can_declare_a_companion_upgrade() {
        let (db, warnings) = load_fixture(&[
            (
                "kernel.ron",
                r#"(id: "kernel", name: "Kernel", upgrade: Some((zone_bump: true)))"#,
            ),
            (
                "buff.ron",
                r#"(id: "buff", name: "Buff", upgrade: Some((hp_percent: 5.0, atk_percent: 2.5)))"#,
            ),
        ]);
        assert!(warnings.is_empty(), "{warnings:?}");

        let kernel = db.get("kernel").unwrap().upgrade.unwrap();
        assert!(kernel.zone_bump);
        assert_eq!(
            (kernel.hp_percent, kernel.atk_percent, kernel.def_percent),
            (0.0, 0.0, 0.0),
            "an unnamed percent defaults to no change"
        );

        let buff = db.get("buff").unwrap().upgrade.unwrap();
        assert_eq!((buff.hp_percent, buff.atk_percent), (5.0, 2.5));
        assert!(!buff.zone_bump, "a percent buff is not a bump by default");
    }

    /// Same hazard as `taming_potency`: a NaN percent survives every clamp and
    /// then poisons the stat arithmetic it is multiplied into.
    #[test]
    fn an_item_with_a_non_finite_upgrade_percent_is_skipped() {
        for (field, ron) in [
            ("hp_percent", r#"upgrade: Some((hp_percent: NaN))"#),
            ("atk_percent", r#"upgrade: Some((atk_percent: inf))"#),
            ("def_percent", r#"upgrade: Some((def_percent: NaN))"#),
        ] {
            let (db, warnings) =
                load_fixture(&[("bad.ron", &format!(r#"(id: "bad", name: "Bad", {ron})"#))]);
            assert_eq!(db.all().count(), 0, "{field}: the whole file is refused");
            assert!(
                warnings.iter().any(|w| w.contains(field)),
                "{field}: {warnings:?}"
            );
        }
    }

    #[test]
    fn missing_roles_names_every_absent_anchor() {
        let (db, _) = load_fixture(&[("a.ron", r#"(id: "a", name: "A")"#)]);
        assert_eq!(
            db.missing_roles(),
            vec![
                "Currency",
                "ResearchCurrency",
                "CraftCurrency",
                "TradeCurrency"
            ]
        );
    }
}
