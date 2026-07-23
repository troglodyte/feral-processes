# Data-driven, moddable items (Phase 1) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn items from a hardcoded `ItemId` enum into data-driven content loaded from `assets/items/*.ron`, so a new item is a dropped-in file — with item behavior expressed as declared roles and effect fields, not Rust `match` arms.

**Architecture:** Build and test the new data layer (`ItemDef` + `ItemDb`, keyed by `String`) *while the `ItemId` enum still exists* — a pure addition that compiles and passes. Then flip `ItemId` to a string newtype against that already-proven DB in one atomic sweep (Rust won't compile a half-migrated tree). Finally layer the new consume/buff behavior on top of the migrated core.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone), RON assets, serde, bincode saves.

Spec: `docs/superpowers/specs/2026-07-22-moddable-items-design.md`

## Global Constraints

- **Never commit.** `Bash(git commit *)` is denied in `.claude/settings.local.json` and CLAUDE.md forbids committing unasked. Every task ends with a suite run; leave the tree dirty for the user.
- **No hardcoded item behavior.** After Task 3, no engine *logic* path may `match`/`==` a specific item id. Item semantics come from `ItemDef` fields and the three economy roles. Shipped item *names* live only in the `ids` module, used by test setup and data-defined recipes — never by logic.
- **Warn-and-skip, never panic** on a malformed item file, matching `StructureDb::load_dir`. A missing/duplicated economy role is the one exception — it aborts startup with a clear error, because trade/research/crafting can't run without one.
- **`#[serde(default)]`** on every new optional `ItemDef` field so mod files stay forward-compatible.
- **Update `assets/items/README.md`** as the modder schema reference (created in Task 2).
- **`cargo fmt` + `cargo clippy --workspace`** after every task; fix warnings, don't silence.
- **`cargo test --workspace` is the final gate** for every task. Baseline ~200 tests, ~1s.
- **The 11 item ids** (variant → id): `CoreFragment`→`core_fragment`, `PowerCell`→`power_cell`, `IceBreaker`→`ice_breaker`, `OverclockCore`→`overclock_core`, `FirewallPlating`→`firewall_plating`, `NeuralAmplifier`→`neural_amplifier`, `PortalFragment`→`portal_fragment`, `ResearchData`→`research_data`, `MonofilamentWhip`→`monofilament_whip`, `AblativePlating`→`ablative_plating`, `CortexHack`→`cortex_hack`.
- **Verbatim data values:** ICE Breaker taming potency `0.4`; Research Data bank limit `200`; starter craft costs — ICE Breaker `3` Core Fragments, Power Cell `2`; equipment stats — Overclock Core `atk 3`, Monofilament Whip `atk 4`, Firewall Plating `def 3`, Ablative Plating `def 4`, Neural Amplifier `decompiler 2`, Cortex Hack `decompiler 3`; Power Cell consume `power 25.0`. Roles — Core Fragment `Currency`, Research Data `ResearchCurrency`, Portal Fragment `CraftCurrency`.

---

### Task 1: Serde derives for embedded item stats

`ItemDef` will embed `EquipmentStats` and (via `PrebattleBuff`) `BuffKind`. Both need serde derives, and `EquipmentStats` needs per-field defaults so RON can omit zero stats. Tiny, isolated, keeps compiling.

**Files:**
- Modify: `crates/engine/src/items.rs` (`EquipmentStats`, line 130-135)
- Modify: `crates/engine/src/components.rs` (`BuffKind`, line 398-402)

**Interfaces:**
- Consumes: nothing.
- Produces: `EquipmentStats` and `BuffKind` are `Serialize + Deserialize`; `EquipmentStats` fields are `#[serde(default)]`.

- [ ] **Step 1: Write the failing round-trip test**

In `crates/engine/src/items.rs` `mod tests`, add:

```rust
    #[test]
    fn equipment_stats_round_trip_ron_with_omitted_zero_fields() {
        let full: EquipmentStats = ron::from_str("(atk: 3, def: 0, decompiler: 0)").unwrap();
        assert_eq!((full.atk, full.def, full.decompiler), (3, 0, 0));
        // Zero fields may be omitted thanks to per-field serde defaults.
        let partial: EquipmentStats = ron::from_str("(atk: 4)").unwrap();
        assert_eq!((partial.atk, partial.def, partial.decompiler), (4, 0, 0));
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p feral-processes-engine equipment_stats_round_trip`

Expected: FAIL to compile — `EquipmentStats` does not implement `Deserialize`.

- [ ] **Step 3: Add the derives and field defaults**

`crates/engine/src/items.rs`, replace the `EquipmentStats` definition (lines 130-135):

```rust
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct EquipmentStats {
    #[serde(default)]
    pub atk: i32,
    #[serde(default)]
    pub def: i32,
    #[serde(default)]
    pub decompiler: i32,
}
```

`crates/engine/src/components.rs`, replace the `BuffKind` derive line (line 398):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test -p feral-processes-engine equipment_stats_round_trip`
Expected: PASS.

- [ ] **Step 5: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`
Expected: no warnings; all pass.

- [ ] **Step 6: Do not commit.**

---

### Task 2: The data layer — `ItemDef`, `ItemDb`, the 11 item files

Add the whole new data model and loader **without touching the `ItemId` enum**. `ItemDef.id` and recipe/role references are `String` for now (tightened to `ItemId` in Task 3). This is a pure addition: it compiles alongside the enum and every existing test still passes. Loading is fully TDD'd here, so Task 3's sweep runs against a proven DB.

**Files:**
- Create: `crates/engine/src/items_db.rs`
- Modify: `crates/engine/src/lib.rs` (add `pub mod items_db;` near line 5)
- Create: `assets/items/*.ron` (11 files)
- Create: `assets/items/README.md`

**Interfaces:**
- Consumes: `EquipmentStats`, `EquipmentSlot` (items.rs), `BuffKind` (components.rs).
- Produces:
  - `items_db::ItemDef { id: String, name: String, bank_limit: Option<u32>, role: Option<EconomyRole>, equipment: Option<(EquipmentSlot, EquipmentStats)>, taming_potency: Option<f32>, consume: Option<ConsumeDef>, craftable: Option<CraftableDef> }`
  - `items_db::EconomyRole { Currency, ResearchCurrency, CraftCurrency }`
  - `items_db::ConsumeDef { power: f32, fatigue: f32, heal: i32, prebattle_buff: Option<PrebattleBuff> }`
  - `items_db::PrebattleBuff { kind: BuffKind, power: i32, rounds: u32 }`
  - `items_db::CraftableDef { cost: Vec<(String, u32)> }`
  - `items_db::ItemDb` with `load_dir(&Path) -> io::Result<(Self, Vec<String>)>`, `get(&str) -> Option<&ItemDef>`, `all() -> impl Iterator<Item=&ItemDef>` (sorted by id), `currency()/research_currency()/craft_currency() -> Option<&String>`, `missing_roles() -> Vec<&'static str>`.

- [ ] **Step 1: Write the new module with its types and loader**

Create `crates/engine/src/items_db.rs`:

```rust
use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::BuffKind;
use crate::items::{EquipmentSlot, EquipmentStats};

/// A singleton economy anchor. The game has exactly one item per role;
/// engine logic queries "the item with role X" instead of naming an id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EconomyRole {
    Currency,
    ResearchCurrency,
    CraftCurrency,
}

/// What `Game::use_item` does out of battle. All fields optional so one item
/// can restore several resources and/or arm a pre-battle buff.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ConsumeDef {
    #[serde(default)]
    pub power: f32,
    #[serde(default)]
    pub fatigue: f32,
    #[serde(default)]
    pub heal: i32,
    #[serde(default)]
    pub prebattle_buff: Option<PrebattleBuff>,
}

/// Arms a `PlayerBuff` that survives on the map and applies during the next
/// intrusion — buffs only tick in battle (see `Game::tick_player_buff`).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PrebattleBuff {
    pub kind: BuffKind,
    pub power: i32,
    pub rounds: u32,
}

/// An always-available ("starter") craft recipe declared by the item itself,
/// replacing the two formerly-hardcoded starter recipes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CraftableDef {
    pub cost: Vec<(String, u32)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub bank_limit: Option<u32>,
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
}

#[derive(Resource, Default)]
pub struct ItemDb {
    items: HashMap<String, ItemDef>,
    currency: Option<String>,
    research_currency: Option<String>,
    craft_currency: Option<String>,
}

impl ItemDb {
    /// Loads every `*.ron` item definition in `dir`. A malformed file is
    /// skipped with a returned warning rather than aborting the load, same
    /// as `StructureDb::load_dir`. A duplicated economy role also warns and
    /// keeps the first-seen holder.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
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
                    if let Some(role) = def.role {
                        let slot = match role {
                            EconomyRole::Currency => &mut db.currency,
                            EconomyRole::ResearchCurrency => &mut db.research_currency,
                            EconomyRole::CraftCurrency => &mut db.craft_currency,
                        };
                        if let Some(existing) = slot {
                            warnings.push(format!(
                                "item {:?} claims role {role:?} already held by {existing:?}; ignoring",
                                def.id
                            ));
                        } else {
                            *slot = Some(def.id.clone());
                        }
                    }
                    db.items.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid item file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&ItemDef> {
        self.items.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &ItemDef> {
        let mut defs: Vec<&ItemDef> = self.items.values().collect();
        defs.sort_by(|a, b| a.id.cmp(&b.id));
        defs.into_iter()
    }

    pub fn currency(&self) -> Option<&String> {
        self.currency.as_ref()
    }

    pub fn research_currency(&self) -> Option<&String> {
        self.research_currency.as_ref()
    }

    pub fn craft_currency(&self) -> Option<&String> {
        self.craft_currency.as_ref()
    }

    /// Human-readable names of any economy role with no holder — empty when
    /// the item set is complete. `Game::new`/`load` abort if this is
    /// non-empty (the economy can't run without all three).
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
        missing
    }
}
```

Add to `crates/engine/src/lib.rs` module list (after `pub mod items;`, line 5):

```rust
pub mod items_db;
```

- [ ] **Step 2: Author the 11 item files**

Create these under `assets/items/`. (Zero-stat fields omitted via serde defaults.)

`core_fragment.ron`:
```ron
(id: "core_fragment", name: "Core Fragment", role: Some(Currency))
```
`research_data.ron`:
```ron
(id: "research_data", name: "Research Data", role: Some(ResearchCurrency), bank_limit: Some(200))
```
`portal_fragment.ron`:
```ron
(id: "portal_fragment", name: "Portal Fragment", role: Some(CraftCurrency))
```
`power_cell.ron`:
```ron
(
    id: "power_cell",
    name: "Power Cell",
    craftable: Some((cost: [("core_fragment", 2)])),
    consume: Some((power: 25.0)),
)
```
`ice_breaker.ron`:
```ron
(
    id: "ice_breaker",
    name: "ICE Breaker",
    taming_potency: Some(0.4),
    craftable: Some((cost: [("core_fragment", 3)])),
)
```
`overclock_core.ron`:
```ron
(id: "overclock_core", name: "Overclock Core", equipment: Some((Weapon, (atk: 3))))
```
`monofilament_whip.ron`:
```ron
(id: "monofilament_whip", name: "Monofilament Whip", equipment: Some((Weapon, (atk: 4))))
```
`firewall_plating.ron`:
```ron
(id: "firewall_plating", name: "Firewall Plating", equipment: Some((Armor, (def: 3))))
```
`ablative_plating.ron`:
```ron
(id: "ablative_plating", name: "Ablative Plating", equipment: Some((Armor, (def: 4))))
```
`neural_amplifier.ron`:
```ron
(id: "neural_amplifier", name: "Neural Amplifier", equipment: Some((Module, (decompiler: 2))))
```
`cortex_hack.ron`:
```ron
(id: "cortex_hack", name: "Cortex Hack", equipment: Some((Module, (decompiler: 3))))
```

- [ ] **Step 3: Write the failing loader tests**

In `crates/engine/src/items_db.rs`, add:

```rust
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
        let out = ItemDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        out
    }

    #[test]
    fn the_shipped_items_load_cleanly_with_all_roles_and_fields() {
        let (db, warnings) = ItemDb::load_dir(&assets_items_dir()).unwrap();
        assert!(warnings.is_empty(), "shipped items should parse clean: {warnings:?}");
        assert!(db.missing_roles().is_empty(), "all three roles must be held");
        assert_eq!(db.currency().unwrap(), "core_fragment");
        assert_eq!(db.research_currency().unwrap(), "research_data");
        assert_eq!(db.craft_currency().unwrap(), "portal_fragment");
        assert_eq!(db.get("research_data").unwrap().bank_limit, Some(200));
        assert_eq!(db.get("ice_breaker").unwrap().taming_potency, Some(0.4));
        assert_eq!(db.get("power_cell").unwrap().consume.unwrap().power, 25.0);
        let (slot, stats) = db.get("monofilament_whip").unwrap().equipment.unwrap();
        assert_eq!(slot, EquipmentSlot::Weapon);
        assert_eq!(stats.atk, 4);
        assert_eq!(db.all().count(), 11);
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

    #[test]
    fn missing_roles_names_every_absent_anchor() {
        let (db, _) = load_fixture(&[("a.ron", r#"(id: "a", name: "A")"#)]);
        assert_eq!(db.missing_roles(), vec!["Currency", "ResearchCurrency", "CraftCurrency"]);
    }
}
```

- [ ] **Step 4: Run the loader tests**

Run: `cargo test -p feral-processes-engine items_db`
Expected: PASS, 4 tests. (If `the_shipped_items_load...` fails, a RON file has a typo — fix the file, not the test.)

- [ ] **Step 5: Write the modder schema doc**

Create `assets/items/README.md` documenting every `ItemDef` field, the three economy roles and their singleton rule, the `consume`/`craftable`/`equipment`/`taming_potency`/`bank_limit` fields, and the warn-and-skip behavior — matching the tone and structure of `assets/structures/README.md`. Include the note that exactly one item must hold each of `Currency`, `ResearchCurrency`, `CraftCurrency` or the game won't start.

- [ ] **Step 6: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`
Expected: no warnings; all pass (the enum and all existing behavior are untouched; this task only added code and data).

- [ ] **Step 7: Do not commit.**

---

### Task 3: Flip `ItemId` to a string newtype and wire everything to `ItemDb`

The atomic core. Replace the enum with `ItemId(String)`, delete its behavior methods, thread `ItemDb` through `Game`, replace every logic touchpoint with a role/DB lookup, migrate all asset RON to string ids, bump the save format, and update app-core + both renderers. The tree does not compile mid-task; the gate is a green full suite. New logic (role validation, accessors) is TDD'd; the call-site and asset sweeps are compiler- and grep-verified.

**Files:**
- Modify: `crates/engine/src/items.rs`, `taming.rs`, `balance.rs`, `species.rs`, `structures.rs`, `research.rs`, `save.rs`, `systems.rs`, `lib.rs`
- Modify: `crates/app-core/src/lib.rs`, `crates/tui/src/ui.rs`, `crates/gui/src/render.rs`
- Modify: every `assets/{species,structures,research}/*.ron` naming an item

**Interfaces:**
- Consumes: `ItemDb` and all its types (Task 2).
- Produces:
  - `items::ItemId(pub String)` with `From<&str>`, `From<String>`, `Display`, `fn as_str(&self) -> &str`; the `ids` module of `pub const &str` names.
  - `ItemDef.id`, `CraftableDef.cost`, `SpeciesDef::work_resource`, `SpeciesDef::equipment_drop`, `WorkDef::produces`, `ResourceNode::resource`, `ResearchRecipe::{result,cost}`, `EquippedItem::item`, `Inventory::items`, `ItemFusions::tiers` all keyed on the newtype `ItemId`.
  - `Game` accessors: `item_name(&ItemId) -> &str`, `is_equippable(&ItemId) -> bool`, `equipment_of(&ItemId) -> Option<(EquipmentSlot, EquipmentStats)>`, `is_consumable(&ItemId) -> bool`, `bank_limit_of(&ItemId) -> Option<u32>`, `currency() -> ItemId`, `research_currency() -> ItemId`, `craft_currency() -> ItemId`.
  - `app-core::inventory_item_actions(&Game, &ItemId) -> Vec<(char, String)>`.

- [ ] **Step 1: Rewrite the `ItemId` type and add the `ids` module**

In `crates/engine/src/items.rs`, replace the `ItemId` enum and its `impl` (lines 9-108) with:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(pub String);

impl ItemId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ItemId {
    fn from(s: &str) -> Self {
        ItemId(s.to_string())
    }
}

impl From<String> for ItemId {
    fn from(s: String) -> Self {
        ItemId(s)
    }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Canonical ids of the shipped items. Used by test setup and data-defined
/// recipes for readability — never by engine *logic*, which goes through
/// economy roles and `ItemDef` fields.
pub mod ids {
    pub const CORE_FRAGMENT: &str = "core_fragment";
    pub const POWER_CELL: &str = "power_cell";
    pub const ICE_BREAKER: &str = "ice_breaker";
    pub const OVERCLOCK_CORE: &str = "overclock_core";
    pub const FIREWALL_PLATING: &str = "firewall_plating";
    pub const NEURAL_AMPLIFIER: &str = "neural_amplifier";
    pub const PORTAL_FRAGMENT: &str = "portal_fragment";
    pub const RESEARCH_DATA: &str = "research_data";
    pub const MONOFILAMENT_WHIP: &str = "monofilament_whip";
    pub const ABLATIVE_PLATING: &str = "ablative_plating";
    pub const CORTEX_HACK: &str = "cortex_hack";
}
```

Delete `RESEARCH_DATA_BANK_LIMIT` (lines 3-7) — it moves to `research_data.ron`. Keep `EquipmentSlot`, `EquipmentStats`, `GEAR_LEVEL_GROWTH`, `ITEM_FUSION_*`, and the `EquipmentStats` impls. Delete the enum-specific tests in `items.rs mod tests` (the `display_name`/`equipment`/`bank_limit` ones, lines 185-286) — that behavior no longer lives on the type; keep `scaled_for_level_grows_100_percent_per_level_above_1`.

In `items_db.rs`, retype `ItemDef.id` and `CraftableDef.cost` from `String` to `ItemId`:

```rust
use crate::items::ItemId;
// ...
pub struct ItemDef { pub id: ItemId, /* ... */ }
pub struct CraftableDef { pub cost: Vec<(ItemId, u32)> }
```

and change the `ItemDb` role fields and accessors from `String` to `ItemId` (the HashMap key stays `String`, inserted via `def.id.0.clone()`; role slots store `def.id.clone()`). Update `currency()/research_currency()/craft_currency()` to return `Option<&ItemId>`. Update the loader-test assertions from `"core_fragment"` to `&ItemId::from("core_fragment")` accordingly.

- [ ] **Step 2: Retype the item-bearing fields across the engine**

Mechanical retype (the fields already hold `ItemId`; the type's shape changed, so these need no edit *except* where a literal enum value appears). Confirm compilation errors point only at literal `ItemId::Variant` sites, handled next. Fields in `components.rs` (`EquippedItem::item`, `Inventory::items`, `ItemFusions::tiers`), `species.rs` (`work_resource`, `equipment_drop`), `structures.rs` (`WorkDef::produces`), `research.rs` (`ResearchRecipe::result`, `cost`), `save.rs` (`inventory`, `weapon`, `armor`, `module`, `item_fusions`), and `lib.rs` (`ResourceNode::resource`) need no field-type edits — they already name `ItemId`.

**By-value signatures stay; clone at reuse sites.** `Inventory::add/count/take` and `ItemFusions::tier/increment` take `item: ItemId` by value. These signatures still *compile* unchanged once `ItemId` is a newtype (`*i == item` compares by reference; `push((item, qty))` moves the owned id). The only breakage is a call site that used an id *after* moving it into one of these — those relied on `Copy` and now need an explicit `.clone()`. The compiler flags each; add `.clone()` where an owned `ItemId` is used again after such a call. Do **not** change these methods to take `&ItemId` — that would balloon the sweep for no benefit.

- [ ] **Step 3: Delete `taming::item_potency`; potency comes from data**

In `crates/engine/src/taming.rs`, delete `item_potency` (lines 3-22) and its `use crate::items::ItemId;` if now unused. Its three callers in `lib.rs` (lines 2168, 2685, 4313) change from `taming::item_potency(ItemId::IceBreaker)` to reading the held catalyst's potency from the DB. At each site the relevant item is the ICE Breaker being consumed; replace with:

```rust
let potency = self
    .world
    .resource::<ItemDb>()
    .get(ids::ICE_BREAKER)
    .and_then(|d| d.taming_potency)
    .unwrap_or(0.0);
```

(These three sites all concern the decompile catalyst, which is ICE Breaker by id; that is a content reference via `ids`, not a behavior branch.)

- [ ] **Step 4: Add the `ItemDb` resource and role validation to `Game::new`/`load`**

In `crates/engine/src/lib.rs`, in both `Game::new` and `Game::load`, after the `StructureDb` is loaded, load and insert the `ItemDb` and abort on missing roles. Near the existing structure-db load:

```rust
let (item_db, item_warnings) = ItemDb::load_dir(&assets_dir.join("items"))?;
load_warnings.extend(item_warnings);
let missing = item_db.missing_roles();
if !missing.is_empty() {
    return Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("item set is missing required economy role(s): {}", missing.join(", ")),
    ));
}
world.insert_resource(item_db);
```

Add `use crate::items_db::ItemDb;` and `use crate::items::ids;` to the imports (not `EconomyRole` — `Game` never names it; only `ItemDb` does).

- [ ] **Step 5: Add the `Game` item-metadata accessors**

In `impl Game` (near `structure_description`), add:

```rust
pub fn item_name(&self, id: &ItemId) -> &str {
    self.world
        .resource::<ItemDb>()
        .get(id.as_str())
        .map(|d| d.name.as_str())
        .unwrap_or_else(|| id.as_str())
}

pub fn is_equippable(&self, id: &ItemId) -> bool {
    self.equipment_of(id).is_some()
}

pub fn equipment_of(&self, id: &ItemId) -> Option<(EquipmentSlot, EquipmentStats)> {
    self.world.resource::<ItemDb>().get(id.as_str())?.equipment
}

pub fn is_consumable(&self, id: &ItemId) -> bool {
    self.world
        .resource::<ItemDb>()
        .get(id.as_str())
        .is_some_and(|d| d.consume.is_some())
}

pub fn bank_limit_of(&self, id: &ItemId) -> Option<u32> {
    self.world.resource::<ItemDb>().get(id.as_str())?.bank_limit
}

pub fn currency(&self) -> ItemId {
    self.world.resource::<ItemDb>().currency().expect("validated at startup").clone()
}

pub fn research_currency(&self) -> ItemId {
    self.world.resource::<ItemDb>().research_currency().expect("validated at startup").clone()
}

pub fn craft_currency(&self) -> ItemId {
    self.world.resource::<ItemDb>().craft_currency().expect("validated at startup").clone()
}
```

- [ ] **Step 6: Replace every engine logic touchpoint**

Apply these transformations in `lib.rs` (and note `balance.rs`). Each is a behavior site identified in the spec:

- **`Inventory` bank-limit internals** (`components.rs`): `cargo_used` (line 257), `add_capped` (271), and `has_room` (288) all call `item.bank_limit()` internally, and `Inventory` can't see `ItemDb`. Add a `db: &ItemDb` parameter to all three — `cargo_used(&self, db: &ItemDb)`, `add_capped(&mut self, item: ItemId, qty: u32, capacity: u32, db: &ItemDb)`, `has_room(&self, item: ItemId, qty: u32, capacity: u32, db: &ItemDb)` — replace `item.bank_limit()` with `db.get(item.as_str()).and_then(|d| d.bank_limit)` and internal `self.cargo_used()` with `self.cargo_used(db)`, and add `use crate::items_db::ItemDb;`. Update callers by borrow shape:
  - **Immutable callers are fine** — a `&Inventory` and a `&ItemDb` are two shared borrows of the `World`, no conflict. In `inventory_used` (4569), `check_room` (4579), and the status view (3730), bind `let db = self.world.resource::<ItemDb>();` then call `inv.cargo_used(db)` / `inv.has_room(item.clone(), qty, capacity, db)`. In `check_room`, resolve the item's own limit via `db.get(item.as_str()).and_then(|d| d.bank_limit)` in place of `item.bank_limit()`.
  - **The one mutable-conflict site is `grant_loot`** (1230) — it holds `&mut Inventory` *and* needs the DB. Use `World::resource_scope` so both are available disjointly:
    ```rust
    let added = self.world.resource_scope(|world, db: bevy_ecs::prelude::Mut<ItemDb>| {
        world.get_mut::<Inventory>(player).unwrap().add_capped(item.clone(), qty, capacity, &db)
    });
    ```
    The `item.clone()` is needed because `grant_loot` uses `item` again for its log label (next bullet).
  - **The two systems** — `task_progress_system` (`add_capped`, systems.rs:167) and `passive_process_system` (`has_room`, systems.rs:246) — receive `Res<ItemDb>` in Step 7 and pass `&item_db`; bevy grants disjoint access to `&mut Inventory`/`Res<ItemDb>` as separate params, so no `resource_scope` there.
  - **`components.rs` unit tests** (lines 652-704) build an `ItemDb` once via `ItemDb::load_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/items"))` and pass `&db` to each `cargo_used`/`add_capped` call.
- **`bank_limit()` label check** in `grant_loot` (line 1233): `item.bank_limit().is_some()` → `self.bank_limit_of(&item).is_some()` (resolve before or after the `resource_scope` block, both are `&self`).
- **`equipment()` calls** (lines 1616, 1644, 1705, 1741): `item.equipment()` → `self.equipment_of(&item)`; `equipped.item.equipment()` → `self.equipment_of(&equipped.item)`. For the two `.unwrap()` sites, keep the `.unwrap()`.
- **`display_name()` calls** (all sites listed in the spec, e.g. 1240, 1554, 1676, 1973, 2517, 4488, 4494-4495, 4504, 4512, 4524, 4659, 4701): `item.display_name()` → `self.item_name(&item)`; `result.display_name()` → `self.item_name(&result)`; `work.produces.display_name()` → `self.item_name(&work.produces)`; etc. Where the call is inside a closure that borrows `self` immutably alongside another `self` borrow, hoist the name into a `let` before the borrow.
- **Currency identity** (line 4637): `if item == ItemId::CoreFragment` → `if item == self.currency()`.
- **Currency loot/payout** — Core Fragment as money (lines 766, 1268, 4653, 4656, 4688, 4696, and scan/forage grant sites): `ItemId::CoreFragment` → `self.currency()`; in `sell_item`, capture `let currency = self.currency();` once and reuse (avoids re-borrow), replacing the two `ItemId::CoreFragment` uses in the payout.
- **Portal Fragment as craft currency / boss loot** (lines 2540, 2552 and the equipment craft recipe cost): `ItemId::PortalFragment` → `self.craft_currency()`.
- **Research Data as research currency** (lines 1382, 1443, 1450, 4780): `ItemId::ResearchData` → `self.research_currency()`.
- **`craft_recipes` starter recipes** (lines 1466-1475): delete the two hardcoded `CraftRecipe` literals and the `ICE_BREAKER_CORE_COST`/`POWER_CELL_CORE_COST` consts (lines 49, 52); build starter recipes from item data instead:

```rust
let mut recipes: Vec<CraftRecipe> = self
    .world
    .resource::<ItemDb>()
    .all()
    .filter_map(|def| {
        def.craftable.as_ref().map(|c| CraftRecipe {
            result: def.id.clone(),
            cost: c.cost.clone(),
        })
    })
    .collect();
```

- **`balance.rs` best-case gear** (lines 55-56): `ItemId::MonofilamentWhip.equipment().unwrap()` and `ItemId::AblativePlating.equipment().unwrap()` no longer resolve on the type. Change `best_case_gear_bonus` to accept the two `EquipmentStats` as parameters, and have its `lib.rs` caller resolve them from the DB via `ids::MONOFILAMENT_WHIP` / `ids::ABLATIVE_PLATING` and pass them in. (These are the strongest shipped weapon/armor — a balance-model assumption, referenced by `ids`, not a logic branch.)

- **The transitional `eat`** (line 1124): keep it behaving identically for now — change `if item != ItemId::PowerCell` to `if item != ItemId::from(ids::POWER_CELL)` and leave the `+25.0`. **Flagged: Task 4 replaces this whole method with data-driven `use_item` and deletes the transitional literal.**

- [ ] **Step 7: Thread `ItemDb` into the systems**

In `crates/engine/src/systems.rs`, `passive_process_system` and `task_progress_system` log item names via `.display_name()`. Add a `item_db: Res<ItemDb>` parameter to each and replace `recipe.produces.display_name()` / `recipe.consumes.display_name()` with `item_db.get(recipe.produces.as_str()).map(|d| d.name.as_str()).unwrap_or(recipe.produces.as_str())`. Both systems are already registered via `Game::build_schedule`; bevy resolves the new `Res` param automatically. Update their unit-test worlds to `world.insert_resource(item_db)` (load from `assets/items` with `ItemDb::load_dir`).

- [ ] **Step 8: Bump the save format**

In `crates/engine/src/save.rs`, `SAVE_FORMAT_VERSION: u32 = 7` → `= 8`. The `ItemId` string encoding is wire-incompatible with the old enum, so old saves are rejected by the existing version check — no other save code changes.

- [ ] **Step 9: Migrate the asset RON files to string ids**

For every `assets/{species,structures,research}/*.ron`, replace each bare item variant with its quoted id. Run this exact sweep:

```bash
cd /home/trog/code/feral-processes
map='CoreFragment core_fragment PowerCell power_cell IceBreaker ice_breaker \
OverclockCore overclock_core FirewallPlating firewall_plating \
NeuralAmplifier neural_amplifier PortalFragment portal_fragment \
ResearchData research_data MonofilamentWhip monofilament_whip \
AblativePlating ablative_plating CortexHack cortex_hack'
set -- $map
while [ $# -gt 0 ]; do
  variant=$1; id=$2; shift 2
  grep -rl --include=*.ron "\b$variant\b" assets/species assets/structures assets/research \
    | xargs -r sed -i "s/\b$variant\b/\"$id\"/g"
done
```

Then hand-check a sample: `grep -rn '"core_fragment"\|"overclock_core"\|"research_data"' assets/structures/mining_node.ron assets/species/glitch.ron assets/research/overclock.ron`. Confirm tuples read like `build_cost: [("core_fragment", 12)]` and `equipment_drop: Some(("overclock_core", 0.1))`. **Do not** run the sweep over `assets/*/README.md` (docs are updated by hand in Task 5) or `assets/items/` (already string ids).

- [ ] **Step 10: Sweep the test literals across the workspace**

Replace `ItemId::Variant` with `ItemId::from(ids::VARIANT)` everywhere it remains (engine tests, app-core tests). Run:

```bash
cd /home/trog/code/feral-processes
declare -A M=( [CoreFragment]=CORE_FRAGMENT [PowerCell]=POWER_CELL [IceBreaker]=ICE_BREAKER \
[OverclockCore]=OVERCLOCK_CORE [FirewallPlating]=FIREWALL_PLATING [NeuralAmplifier]=NEURAL_AMPLIFIER \
[PortalFragment]=PORTAL_FRAGMENT [ResearchData]=RESEARCH_DATA [MonofilamentWhip]=MONOFILAMENT_WHIP \
[AblativePlating]=ABLATIVE_PLATING [CortexHack]=CORTEX_HACK )
for v in "${!M[@]}"; do
  grep -rl "ItemId::$v\b" crates --include=*.rs \
    | xargs -r sed -i "s/ItemId::$v\b/ItemId::from(ids::${M[$v]})/g"
done
```

Add `use crate::items::ids;` (engine) or `use feral_processes_engine::items::ids;` (app-core) to any test module that now references `ids`. Some equality checks (`*item != ItemId::from(ids::CORE_FRAGMENT)`) are fine as-is.

- [ ] **Step 11: Update app-core**

In `crates/app-core/src/lib.rs`:
- `inventory_item_actions(item: ItemId)` → `inventory_item_actions(game: &Game, item: &ItemId)`; use `game.is_equippable(item)` for the Equip/Fuse rows. (The Use row is added in Task 4 — leave a `// Task 4: Use action` marker so the follow-up has an anchor.) Update both renderer call sites to pass `game` and `&item`.
- Line 773: `game.eat(ItemId::PowerCell)` → `game.eat(ItemId::from(ids::POWER_CELL))` (still transitional; Task 4 swaps to `use_power_source`).
- Line 1361: `*item != ItemId::CoreFragment` → `*item != game.currency()`.
- Fix test literals (covered by Step 10) and add the `ids` import.

- [ ] **Step 12: Update the renderers**

In `crates/tui/src/ui.rs` and `crates/gui/src/render.rs`, replace `item.display_name()` / `result.display_name()` with `game.item_name(&item)` (the render fns already hold `game`), and `*item != ItemId::CoreFragment` with `*item != game.currency()`. For the GUI's hardcoded item showcase (render.rs ~2150-2157 listing the six gear items) and any getting-started panel, drive the list from `game`-exposed data (`game.item_name(&ItemId::from(ids::...))`) rather than enum literals. Add `ids` imports where used.

- [ ] **Step 13: Compile, then green the suite**

Run iteratively: `cargo build --workspace` → fix the next error → repeat until it builds. Then:

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no warnings; all tests pass. If a test asserted an old hardcoded message like `"You drain a power cell"`, keep it — `eat` is unchanged this task.

- [ ] **Step 14: Verify no behavior hardcoding remains**

Run: `grep -rn "ItemId::CoreFragment\|ItemId::PowerCell\|ItemId::PortalFragment\|ItemId::ResearchData\|ItemId::IceBreaker" crates/engine/src/lib.rs | grep -v "ids::"`

Expected: no lines (every economic/behavior reference now goes through a role or `ids`). The only `ids::POWER_CELL` in a *logic* path is the transitional `eat`, removed in Task 4.

- [ ] **Step 15: Do not commit.**

---

### Task 4: Data-driven consumption (`use_item`) and the Use action

Replace the transitional `eat` with `Game::use_item(&ItemId)` applying an item's `ConsumeDef`, add `use_power_source` for the `e` key, and add a "Use" row to the inventory item-action menu. Now new consumables (Phase 2) work with zero Rust.

**Files:**
- Modify: `crates/engine/src/lib.rs` (replace `eat`; add `use_item`, `use_power_source`)
- Modify: `crates/app-core/src/lib.rs` (`e` key, Use action + dispatch)

**Interfaces:**
- Consumes: `ItemDb`, `ConsumeDef`, `is_consumable` (Task 3); `PlayerBuff`/`ActiveBuff`/`BuffKind` (components).
- Produces: `Game::use_item(&mut self, id: &ItemId)`; `Game::use_power_source(&mut self)`.

- [ ] **Step 1: Write the failing consume tests**

In `crates/engine/src/lib.rs mod tests`, add (fixtures use shipped ids; `power_cell` restores 25 Power):

```rust
    #[test]
    fn use_item_applies_a_power_restore_and_consumes_one() {
        let mut game = Game::new(500, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
        game.world.get_mut::<Inventory>(player).unwrap().add(ItemId::from(ids::POWER_CELL), 2);

        game.use_item(&ItemId::from(ids::POWER_CELL));

        assert_eq!(game.world.get::<Needs>(player).unwrap().hunger, 75.0);
        assert_eq!(game.world.get::<Inventory>(player).unwrap().count(ItemId::from(ids::POWER_CELL)), 1);
    }

    #[test]
    fn use_item_clamps_power_at_full() {
        let mut game = Game::new(501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 90.0;
        game.world.get_mut::<Inventory>(player).unwrap().add(ItemId::from(ids::POWER_CELL), 1);

        game.use_item(&ItemId::from(ids::POWER_CELL));

        assert_eq!(game.world.get::<Needs>(player).unwrap().hunger, 100.0);
    }

    #[test]
    fn use_item_rejects_a_non_consumable() {
        let mut game = Game::new(502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Inventory>(player).unwrap().add(ItemId::from(ids::CORE_FRAGMENT), 3);

        game.use_item(&ItemId::from(ids::CORE_FRAGMENT));

        assert_eq!(
            game.world.get::<Inventory>(player).unwrap().count(ItemId::from(ids::CORE_FRAGMENT)),
            3,
            "a non-consumable must not be consumed"
        );
    }

    #[test]
    fn use_item_on_an_empty_stack_is_a_no_op() {
        let mut game = Game::new(503, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let before = game.world.get::<Needs>(player).unwrap().hunger;
        game.use_item(&ItemId::from(ids::POWER_CELL));
        assert_eq!(game.world.get::<Needs>(player).unwrap().hunger, before);
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine use_item`
Expected: FAIL — `use_item` doesn't exist.

- [ ] **Step 3: Replace `eat` with `use_item` + `use_power_source`**

In `crates/engine/src/lib.rs`, delete the transitional `eat` (lines ~1124-1148) and add:

```rust
/// Consume one unit of `id` out of battle, applying its `ConsumeDef`:
/// restore Power/Fatigue/Integrity (each clamped) and/or arm a pre-battle
/// combat buff (see `use_item`'s `prebattle_buff`, applied at the next
/// intrusion). A non-consumable or an empty stack is a logged no-op.
pub fn use_item(&mut self, id: &ItemId) {
    if self.is_game_over().is_some() || self.has_active_battle() {
        return;
    }
    let Some(effect) = self
        .world
        .resource::<ItemDb>()
        .get(id.as_str())
        .and_then(|d| d.consume)
    else {
        self.log("You can't use that.");
        return;
    };
    let player = self.player_entity();
    if self.world.get_mut::<Inventory>(player).unwrap().take(id.clone(), 1) == 0 {
        self.log(format!("You have no {}.", self.item_name(id)));
        return;
    }
    {
        let mut needs = self.world.get_mut::<Needs>(player).unwrap();
        needs.hunger = (needs.hunger + effect.power).min(100.0);
        needs.fatigue = (needs.fatigue + effect.fatigue).min(100.0);
    }
    if effect.heal != 0 {
        let mut stats = self.world.get_mut::<Stats>(player).unwrap();
        stats.hp = (stats.hp + effect.heal).min(stats.max_hp);
    }
    if let Some(buff) = effect.prebattle_buff {
        self.world.get_mut::<PlayerBuff>(player).unwrap().active = Some(ActiveBuff {
            kind: buff.kind,
            remaining: buff.rounds,
            power: buff.power,
        });
    }
    let name = self.item_name(id).to_string();
    self.log(format!("You use a {name}."));
    self.tick();
}

/// The `e` shortcut: use the first inventory item that restores Power.
pub fn use_power_source(&mut self) {
    let player = self.player_entity();
    // Scope the DB + Inventory borrows so both release before use_item's
    // &mut self: target is an owned Option<ItemId>.
    let target = {
        let db = self.world.resource::<ItemDb>();
        let inv = self.world.get::<Inventory>(player).unwrap();
        inv.items
            .iter()
            .map(|(id, _)| id.clone())
            .find(|id| db.get(id.as_str()).and_then(|d| d.consume).is_some_and(|c| c.power > 0.0))
    };
    match target {
        Some(id) => self.use_item(&id),
        None => self.log("You have nothing to recharge from."),
    }
}
```

The inner block drops the `ItemDb` and `Inventory` borrows (both shared borrows of the same `World`, so they coexist) before the `match` calls `use_item`, which needs `&mut self`.

- [ ] **Step 4: Run the consume tests**

Run: `cargo test -p feral-processes-engine use_item`
Expected: PASS, 4 tests.

- [ ] **Step 5: Wire the `e` key and the Use action in app-core**

In `crates/app-core/src/lib.rs`:
- Line 773: `game.eat(ItemId::from(ids::POWER_CELL))` → `game.use_power_source()`.
- In `inventory_item_actions(game, item)`, add (at the `// Task 4` marker) a Use row for consumables:

```rust
    if game.is_consumable(item) {
        actions.push(('c', "[C]onsume".to_string()));
    }
```

- In `handle_inventory_item_action_key`, dispatch `'c'` to `game.use_item(&item)` (mirror how `'e'`/`'x'` are handled), then return to `Mode::Playing` (or `Mode::Inventory`) as the other actions do.

- [ ] **Step 6: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no warnings; all pass. Any old test asserting the literal `"You drain a power cell"` log becomes `"You use a Power Cell."` — update those assertions to match the new message.

- [ ] **Step 7: Do not commit.**

---

### Task 5: Pre-battle buff persistence

A buff armed on the map by a consumable must survive until the next intrusion and apply there. Today `PlayerBuff` is only ever set inside battle, so `start_battle` has no reason to preserve it — this task establishes and protects that invariant.

**Files:**
- Modify: `crates/engine/src/lib.rs` (`start_battle`, ~line 2110)

**Interfaces:**
- Consumes: `use_item`'s `prebattle_buff` path (Task 4); `PlayerBuff` (components).
- Produces: the invariant that a map-armed `PlayerBuff` is live at the first battle round.

- [ ] **Step 1: Write the failing test**

Add a fixture consumable that arms a buff, then assert it survives into battle. In `crates/engine/src/lib.rs mod tests`:

```rust
    #[test]
    fn a_prebattle_buff_armed_on_the_map_is_live_at_the_next_intrusion() {
        let mut game = Game::new(504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Arm an Atk buff directly (models what a prebattle_buff consumable does).
        game.world.get_mut::<PlayerBuff>(player).unwrap().active = Some(ActiveBuff {
            kind: BuffKind::Atk,
            remaining: 3,
            power: 5,
        });

        let wild = spawn_wild_in_front_of_player(&mut game);
        game.start_battle(vec![wild]);

        let buff = game.world.get::<PlayerBuff>(player).unwrap().active;
        assert!(
            matches!(buff, Some(ActiveBuff { kind: BuffKind::Atk, power: 5, .. })),
            "a buff armed before the fight must still be active when it starts"
        );
    }
```

If no `spawn_wild_in_front_of_player` helper exists, spawn a minimal wild `Creature` at the player's tile using the existing test spawn pattern (see `spawn_tamed`) and pass its entity to `start_battle`.

- [ ] **Step 2: Run it**

Run: `cargo test -p feral-processes-engine prebattle_buff`
Expected: PASS if `start_battle` already leaves `PlayerBuff` alone, FAIL if it clears it. Inspect `start_battle` (line 2110):

- [ ] **Step 3: Protect the invariant**

Read `start_battle`. If it resets `PlayerBuff` (directly or via a shared reset), remove that clear so a pre-existing buff carries in; if it already leaves it untouched, add a one-line comment at `start_battle` documenting that a map-armed `PlayerBuff` is intentionally preserved (the test guards it). Do **not** touch `end_battle`'s clear.

- [ ] **Step 4: Full gate**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`
Expected: no warnings; all pass.

- [ ] **Step 5: Do not commit.**

---

### Task 6: Documentation

Flip the moddability docs so items read as data-driven, and record the breaking change.

**Files:**
- Modify: `CLAUDE.md`, `README.md`, `CHANGELOG.md`
- Verify: `assets/items/README.md` (from Task 2)

- [ ] **Step 1: Update CLAUDE.md**

In the Moddability section, replace the **New items** bullet (currently "items aren't data-driven from files … adding one still requires a code change") with a bullet stating items are now data — add a `.ron` to `assets/items/`, schema in `assets/items/README.md` — and note the `ItemId` newtype + `ids` module for shipped-item references. Add items to the `#[serde(default)]` / warn-and-skip rules list alongside species and structures. Remove the now-false claim that a new item needs a code change.

- [ ] **Step 2: Update README.md**

Where the README describes items/equipment/currencies, note that items live in `assets/items/`. If there's a Modding section listing species/structures as data-driven, add items. No player-facing mechanic changed (the 11 items behave identically), so keep gameplay prose intact.

- [ ] **Step 3: Add the CHANGELOG entry**

In `CHANGELOG.md`, after `Release notes for …`, add a `## 2026-07-22` entry (or append to the existing one if the recharger change landed first) covering: items are now data-driven RON files in `assets/items/`; the save format bumped to **v8** (old saves need a new game); **breaking for mods** — any species/structure/research file naming an item as `CoreFragment` must switch to `"core_fragment"` (list the mapping or point at `assets/items/README.md`); crafting gained a data `craftable` starter-recipe path.

- [ ] **Step 4: Verify the items README**

Confirm `assets/items/README.md` documents every field and the singleton-role rule. Fix any drift from the final schema.

- [ ] **Step 5: Full gate**

Run: `cargo test --workspace`
Expected: all pass (docs-only, but run it to be sure nothing references a renamed symbol).

- [ ] **Step 6: Do not commit.**

---

## Final verification

- [ ] `cargo fmt --check` clean; `cargo clippy --workspace` no warnings; `cargo test --workspace` all pass.
- [ ] `grep -rn "\.display_name()\|\.bank_limit()\|\.equipment()" crates/engine/src crates/app-core/src crates/tui/src crates/gui/src` returns nothing on `ItemId` values (only `EquipmentSlot::label`, `Perk::display_name`, etc. may remain).
- [ ] `grep -rn "ItemId::[A-Z]" crates` returns nothing except `ItemId::from(...)`.
- [ ] Every `assets/{species,structures,research}/*.ron` loads with zero warnings (asserted by an engine test that constructs `Game::new` and checks the message log has no "skipped invalid").
- [ ] Deleting an item file that holds a role makes `Game::new` return a clear error (spot-check once by hand, then restore the file).
- [ ] Report to the user what was run and the actual output.

## Deferred to Phase 2

The 10 new items are a separate content-only spec/plan (`…-ten-new-items-design.md`): Repair Kit, Coolant Flush, Surge Cell, Full Reboot (restore consumables); Adrenaline Spike, Hardened Shell, Overclock Serum (pre-battle buffs); Cheap Exploit Kit, Master Key (tiered taming catalysts); one new gear piece. All are `assets/items/*.ron` drops — no Rust — exercising `consume`, `prebattle_buff`, `taming_potency`, and `equipment` respectively.
