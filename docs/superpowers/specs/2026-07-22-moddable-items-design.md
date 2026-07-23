# Data-driven, moddable items (Phase 1)

## Problem

Items are the last hardcoded content type. Species, structures, and research
are all data-driven — drop a `.ron` file, it loads. Items are an `ItemId`
enum in `crates/engine/src/items.rs`, so adding one is a Rust change, and
CLAUDE.md documents this as a deliberate exception. That exception is now the
thing to remove: items must become droppable-file content like everything
else.

The obstacle isn't the catalog — it's that item *behavior* is wired to
specific enum variants across the engine:

- **Core Fragment** is the universal currency: trade payouts, the "you can't
  sell money" rule, scan/forage loot, the default resource-node yield.
- **Portal Fragment** is the equipment-crafting currency and boss loot.
- **Research Data** is the banked currency the research tree is priced in.
- **Power Cell** is the only thing `eat` accepts (+25 Power).
- **ICE Breaker** is the taming catalyst, with a hardcoded `item_potency`.

Plus the 6 equipment items carry slot + stats in a `match` arm, and
`display_name` / `bank_limit` / `equipment` are all methods on the enum.

This is Phase 1: the infrastructure. **Phase 2** — authoring 10 new items —
is a separate, content-only spec that depends on the effect schema defined
here.

## Design

### `ItemId` becomes a string newtype

An enum cannot be extended by a dropped-in file, so:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(pub String);
```

with `From<&str>`, `From<String>`, `Display`, `as_str`, `Borrow<str>`. Serde
treats a newtype struct transparently, so it serializes as a bare string:
`"core_fragment"` in RON, a length-prefixed string in a save.

It loses `Copy`. This is the dominant source of mechanical churn: every
`item: ItemId` read-parameter becomes `&ItemId`, and the few places that
relied on bitwise copy now borrow or `.clone()`. The three behavior methods
(`display_name`, `bank_limit`, `equipment`) are **deleted** — that data moves
to `ItemDef`.

A **hybrid** `enum { CoreFragment, …, Custom(String) }` was rejected: it
privileges the 11 originals, keeps their behavior in Rust while mods live in
data, and is exactly the two-sources-of-truth cruft CLAUDE.md forbids. The
originals must become ordinary `.ron` files — they are the proof the system
works.

### `ItemDb` — a registry mirroring `StructureDb`

New `assets/items/` directory and an `ItemDb` resource loaded from it,
following `StructureDb::load_dir` exactly: iterate `*.ron`, skip a malformed
file with a logged warning (never panic), return `(ItemDb, Vec<String>)`.

```rust
#[derive(Resource, Default)]
pub struct ItemDb {
    items: HashMap<String, ItemDef>,
    currency: Option<ItemId>,
    research_currency: Option<ItemId>,
    craft_currency: Option<ItemId>,
}
```

Role fields are resolved during load by scanning each def's `role`. A missing
or duplicated role pushes a warning. Accessors:

```rust
pub fn get(&self, id: &str) -> Option<&ItemDef>;
pub fn all(&self) -> impl Iterator<Item = &ItemDef>;   // sorted by id
pub fn currency(&self) -> &ItemId;          // .expect, validated at startup
pub fn research_currency(&self) -> &ItemId;
pub fn craft_currency(&self) -> &ItemId;
```

The three economy roles are singletons — the game has one currency, one
research currency, one craft currency. `Game::new`/`Game::load` validate that
all three resolved after loading; a set missing one is a fatal config error
that aborts startup with a clear message (per CLAUDE.md, startup config that
should abort anyway). After that gate the `currency()` accessors are
infallible.

### An `ids` module for shipped-item names

`items.rs` gains:

```rust
pub mod ids {
    pub const CORE_FRAGMENT: &str = "core_fragment";
    pub const POWER_CELL: &str = "power_cell";
    // … all 11
}
```

Engine **logic** never names a shipped item — it goes through roles and
`ItemDef` fields. But test setup and the two data-defined starter recipes
legitimately need to name specific shipped items; they use `ids::*` for
readability (`ItemId::from(ids::CORE_FRAGMENT)`) rather than bare string
literals scattered around. This is naming, not behavior.

### `ItemDef` schema (`assets/items/*.ron`)

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: ItemId,
    pub name: String,
    #[serde(default)] pub bank_limit: Option<u32>,
    #[serde(default)] pub role: Option<EconomyRole>,
    #[serde(default)] pub equipment: Option<(EquipmentSlot, EquipmentStats)>,
    #[serde(default)] pub taming_potency: Option<f32>,
    #[serde(default)] pub consume: Option<ConsumeDef>,
    #[serde(default)] pub craftable: Option<CraftableDef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EconomyRole { Currency, ResearchCurrency, CraftCurrency }

/// What `Game::use_item` does out of battle. A struct of optional effects,
/// so one item can restore several resources and/or arm a pre-battle buff.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ConsumeDef {
    #[serde(default)] pub power: f32,     // Power (Needs::hunger) restored
    #[serde(default)] pub fatigue: f32,   // Fatigue restored
    #[serde(default)] pub heal: i32,      // Integrity (Stats::hp) restored
    #[serde(default)] pub prebattle_buff: Option<PrebattleBuff>,
}

/// Arms a `PlayerBuff` that survives on the map and applies during the next
/// intrusion (buffs only tick in battle — see `Game::tick_player_buff`).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PrebattleBuff { pub kind: BuffKind, pub power: i32, pub rounds: u32 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CraftableDef { pub cost: Vec<(ItemId, u32)> }
```

`EquipmentStats` and `BuffKind` gain `Serialize, Deserialize` derives (both
are otherwise unchanged). `EquipmentSlot` already has them.

Example — the migrated Power Cell:

```ron
(
    id: "power_cell",
    name: "Power Cell",
    craftable: Some((cost: [("core_fragment", 4)])),
    consume: Some((power: 25.0)),
)
```

### Behavior migration: roles replace variant checks

Every `match`/`==` on a specific `ItemId` in engine *logic* is replaced by a
role lookup or an `ItemDef` field. No shipped item id appears in a logic path.

| Today (hardcoded) | Becomes |
|---|---|
| `item == ItemId::CoreFragment` (can't-sell rule) | `item == db.currency()` |
| trade payout, scan, forage, default node yield → Core Fragment | `db.currency().clone()` |
| boss loot, portal-fragment rolls → Portal Fragment | `db.craft_currency().clone()` |
| research node cost paid in `ItemId::ResearchData` | `db.research_currency()` |
| `bank_limit()` (Research Data = 200) | `db.get(id).bank_limit` |
| `eat` accepts only Power Cell, +25 | `use_item` applies the `consume` def |
| `taming::item_potency(ItemId::IceBreaker)` | `db.get(id).taming_potency` |
| `equipment()` slot/stats match arm | `db.get(id).equipment` |
| 2 hardcoded starter recipes (`craft_recipes`) | items with a `craftable` field |

`taming::item_potency(item: ItemId) -> f32` is deleted; callers read
`db.get(id).and_then(|d| d.taming_potency)` and treat `None`/absent as
"not a catalyst". The `RESEARCH_DATA_BANK_LIMIT` constant moves into
`research_data.ron` as `bank_limit: Some(200)`; the constant is deleted.

### Consumption UX (out of battle only)

`Game::eat` is replaced by:

```rust
pub fn use_item(&mut self, id: &ItemId)   // applies the item's ConsumeDef
```

Guards (game-over / in-battle) then: look up `consume` (none → "You can't
use that."); take 1 from inventory (0 → "You have none."); apply
`hunger = (hunger + power).min(100)`, `fatigue = (fatigue + fatigue).min(100)`,
`hp = (hp + heal).min(max_hp)`; if `prebattle_buff`, set
`PlayerBuff.active = Some(ActiveBuff { kind, remaining: rounds, power })`;
log; `tick()`.

`start_battle` must **not** clear a pre-existing `PlayerBuff` (today it's
never set outside battle, so this is a new invariant to protect); `end_battle`
still clears it as before. A buff armed on the map thus persists un-ticked
until the next intrusion, then ticks down per round.

The `e` key (app-core) stops calling `game.eat(ItemId::PowerCell)` and calls a
new `Game::use_power_source(&mut self)` that uses the first inventory item
whose `consume.power > 0` (logs if none). A **"Use"** action is added to the
existing `Mode::InventoryItemAction` menu for any consumable item — no new
mode; mirrors the equip/fuse/erase pattern already there.

### Decoupling app-core and the renderers

app-core and both renderers currently call methods on `ItemId`
(`display_name`, `equipment`, `bank_limit`) and name variants directly
(`ItemId::CoreFragment`, `ItemId::PowerCell`, the 6 gear items). With behavior
gone from the type, that metadata is exposed through `Game` accessors so the
"renderers talk to `Game`, never the ECS/DB" rule holds:

```rust
pub fn item_name(&self, id: &ItemId) -> &str;          // falls back to id.0
pub fn is_equippable(&self, id: &ItemId) -> bool;
pub fn equipment_of(&self, id: &ItemId) -> Option<(EquipmentSlot, EquipmentStats)>;
pub fn is_consumable(&self, id: &ItemId) -> bool;
pub fn bank_limit_of(&self, id: &ItemId) -> Option<u32>;
pub fn currency(&self) -> &ItemId;   // + research_currency, craft_currency
```

`item_name` falling back to the raw id string keeps a save that references a
since-removed mod item renderable instead of crashing.

`inventory_item_actions(item: ItemId)` (app-core) gains a `&Game` parameter so
it can offer Equip/Fuse (equippable) and Use (consumable) from live data; both
renderers already hold a `Game` handle at their call sites.

## Migration

- **Save format:** bump `SAVE_FORMAT_VERSION`. `Inventory`, `Equipment`, and
  `ItemFusions` all serialize `ItemId`; the string representation is
  wire-incompatible with the old enum, so old saves are rejected cleanly by
  the existing version-prefix check. Consistent with prior breaking bumps.
- **Shipped assets:** every `assets/{species,structures,research}/*.ron`
  that names an item by variant migrates to the string id — `CoreFragment` →
  `"core_fragment"`, `(OverclockCore, 0.1)` → `("overclock_core", 0.1)`,
  recipe `result: IceBreaker` → `result: "ice_breaker"`, and so on. Roughly
  20 species + 14 structures + 6 research files, all mechanical.
  `SpeciesDef::work_resource` and `SpeciesDef::equipment_drop` change from
  `ItemId` (enum) to the string-backed `ItemId` with no field-shape change.
- **The 11 originals** are authored as `assets/items/*.ron`, each exercising
  the schema: currencies with `role`, Research Data with `bank_limit`, the 6
  gear items with `equipment`, ICE Breaker with `taming_potency`, Power Cell
  with `consume` + `craftable`.
- **Docs:** new `assets/items/README.md` (schema reference for modders,
  matching the species/structures READMEs); flip the CLAUDE.md rule that says
  items require a code change; `README.md` and `CHANGELOG.md` updated.

## Consequences

- **Old saves don't load.** A one-line version bump; the game already treats
  this as "start a new game", same as the fusion (v6) change.
- **All existing mods break once.** Any third-party species/structure/research
  file naming an item by enum-variant must switch to the string id. This is
  the unavoidable one-time cost of making items data; documented in the
  changelog and the new items README.
- **`ItemId` is no longer `Copy`.** Engine code threads `&ItemId` and clones
  at the few ownership boundaries (loot grants, recipe results). Localized to
  read paths; no algorithmic change.
- **A malformed item file is skipped, not fatal** — but a set missing a
  required economy role *is* fatal at startup, because trade/research/crafting
  cannot function without one. The asymmetry is deliberate: one bad mod file
  shouldn't crash the game, but a fundamentally broken economy should fail
  loud and early.
- **Crafting gains a data starter-recipe path.** Folding the two hardcoded
  starter recipes into `craftable` means a mod can add an always-craftable
  item with no Rust change — a small capability gain that falls out of the
  refactor.

## Implementation surface

- `crates/engine/src/items.rs` — rewrite: `ItemId` newtype, `ItemDef`,
  `EconomyRole`, `ConsumeDef`, `PrebattleBuff`, `CraftableDef`, `ItemDb`,
  `ids` module. Delete `display_name`/`bank_limit`/`equipment`/
  `item_potency`/`RESEARCH_DATA_BANK_LIMIT`.
- `crates/engine/src/taming.rs` — delete `item_potency`; potency read from
  `ItemDb`.
- `crates/engine/src/components.rs` — `EquipmentStats`, `BuffKind` gain
  serde derives. `Inventory`/`Equipment`/`ItemFusions` unchanged in shape.
- `crates/engine/src/species.rs`, `structures.rs`, `research.rs` — item
  fields retype to the string-backed `ItemId` (no field renames).
- `crates/engine/src/lib.rs` — insert `ItemDb` resource in `new`/`load` +
  role validation; replace every variant check with a role/`ItemDb` lookup;
  `use_item`, `use_power_source`, the new `Game` accessors;
  `craft_recipes` sourced from `craftable` defs; `start_battle` preserves a
  map-armed `PlayerBuff`.
- `crates/engine/src/systems.rs` — `passive_process_system` /
  `task_progress_system` take `Res<ItemDb>` for item-name logging.
- `crates/engine/src/save.rs` — `SAVE_FORMAT_VERSION` bump.
- `crates/app-core/src/lib.rs` — `inventory_item_actions(&Game, &ItemId)`;
  `e` → `use_power_source`; Use action dispatch; currency filter via accessor.
- `crates/tui/src/ui.rs`, `crates/gui/src/render.rs` — replace
  `.display_name()`/`.equipment()`/`ItemId::X` with `Game` accessors.
- `assets/items/*.ron` (11 new) + `assets/items/README.md`.
- `assets/{species,structures,research}/*.ron` — item ids to strings.
- `README.md`, `CHANGELOG.md`, `CLAUDE.md`.

## Testing

- `ItemDb::load_dir` skips a malformed file with a warning; loads the 11.
- Role resolution: exactly one of each; a fixture with a duplicate or missing
  role warns; startup aborts when a required role is absent.
- Each migrated behavior via role/field: trade pays and refuses in the
  currency; scan/forage/boss grant the right role item; research priced in the
  research currency; `bank_limit` from data; taming potency from data;
  equipment slot/stats from data.
- `use_item` applies each `consume` primitive and clamps (Power/Fatigue at
  100, HP at max_hp); a non-consumable and an empty stack are no-ops with a
  log.
- A pre-battle buff armed on the map is present at the next intrusion, ticks
  down per round, and is not wiped by `start_battle`.
- `craft_recipes` returns the two starter recipes sourced from `craftable`
  data; a modded starter-craftable item appears too.
- Every shipped asset still loads with zero warnings after id migration.
- Save round-trips under the new version; an old-version save is rejected.
- Full `cargo test --workspace` is the gate.

## Phase 2 (separate spec)

Once this lands, `docs/superpowers/specs/…-ten-new-items-design.md` adds 10
`.ron` files spanning the four chosen categories (restore consumables,
pre-battle buffs, tiered taming catalysts, new equipment) — pure content, no
Rust. Sketch: Repair Kit, Coolant Flush, Surge Cell, Full Reboot, Adrenaline
Spike, Hardened Shell, Overclock Serum, Cheap Exploit Kit, Master Key, and one
new gear piece.
