# Non-raidable structures and inventory slot labels

Two independent changes, grouped because both are small and neither touches
the other's code.

## 1. Home cannot be raided

### Problem

`Game::raid_check` picks its target at random from every entity carrying
`Durability`, excluding nests:

```rust
query_filtered::<Entity, (With<Durability>, Without<Nest>)>()
```

Home is deployed like any other structure and gets `Durability { hp: 30,
max_hp: 30 }` from `StructureDef::durability`, so it sits in that pool. A
raid can destroy it.

That is wrong for what home *is*. It gates every other structure (`lib.rs:1960`
refuses to build anything else without one), it is the symlink anchor
(`lib.rs:3248`), and only one may exist at a time (`lib.rs:1963`). Losing it
to a random roll strands the player rather than costing them something.

### Design

A structure that cannot be raided is one that has no durability pool at all
— not one with an immunity flag checked at damage time. The absence of the
component *is* the immunity, which means `raid_check` needs no new branch:
its existing `With<Durability>` filter already excludes it, and
`structure_regen`'s `&mut Durability` query skips it for the same reason.

#### Schema

One new field on `StructureDef` (`crates/engine/src/structures.rs`):

```rust
/// Whether raids can target this structure. A non-raidable structure is
/// spawned without a `Durability` component at all, which is what keeps
/// `Game::raid_check` from ever selecting it.
#[serde(default = "default_raidable")]
pub raidable: bool,
```

Named `raidable`, not `attackable`, to match the vocabulary already in use
throughout: `raid_check`, `raid_defense`, `RAID_DAMAGE`, `MessageKind::Raid`.

`#[serde(default = "default_raidable")]` returning `true` keeps every
existing `.ron` file — shipped and third-party — parsing untouched and
raidable, per the schema rule in CLAUDE.md.

#### Spawn sites

Both places that build a structure entity make the `Durability` insert
conditional on `def.raidable`:

- `Game::deploy_structure` (`lib.rs:2004`) — the build path.
- Save loading (`lib.rs:787`) — the restore path.

Nothing else needs to change. Every consumer of structure durability already
takes an `Option`: the map view (`lib.rs:4445`), the inspect list
(`lib.rs:4561`), and the save writer (`lib.rs:942`, `durability.map(|d| d.hp)`).
The GUI's damage wash (`gui/src/fx.rs:29`) reads the same optional fraction.

#### Data

`assets/structures/home.ron` gains `raidable: false`. No other structure file
changes.

#### Consequences

A raid that rolls while home is the only deployed structure finds an empty
target set and returns without logging anything. That is correct: there is
nothing raidable to raid, and inventing a message for it would be noise.

Home shows no `[HP x/y]` in the inventory, map, or inspect panels, because
it has no durability to show.

`raid_defense` remains independent of this. `total_raid_defense` sums over
`Structure` kind via `StructureDb`, not over `Durability`, so a non-raidable
structure could still contribute base-wide raid defense to others. Home does
not set `raid_defense`, but the combination is coherent if a mod wants it.

#### Save compatibility

Saves written before this change record `durability: Some(30)` for home. The
load path skips the component when the def says non-raidable, so the stored
value is simply ignored. No migration, no version bump.

### Tests

In `crates/engine/src/lib.rs`:

- Deploying home produces no `Durability` component; deploying a mining node
  still does.
- `raid_check` leaves home standing when it is the only structure — following
  the existing `raid_check_never_targets_a_nest_even_as_the_only_durability_holder`
  pattern of stripping every other durability holder first.
- `home.ron` loads with `raidable == false`, and a def that omits the field
  parses as `true`.
- A save/load round-trip leaves home without `Durability`.

### Docs

`assets/structures/README.md` gains a `raidable` entry, and its `durability`
entry notes that durability is inert when `raidable: false`.

## 2. Inventory rows show the slot an item would take

### Problem

The equip screen lists an equippable item's stat bonus but not which slot it
competes for:

```
[a] Monofilament Whip x1 (+4 ATK)
[b] Ablative Plating x2 (+3 DEF)
```

The slot is knowable — `Game::equipment_of` returns `(EquipmentSlot,
EquipmentStats)` — but both renderers discard it in the same line:

```rust
let Some((_, base_mods)) = game.equipment_of(item) else {
```

So the player has to infer from the stat which slot an item lands in, and
that inference is only reliable while every weapon happens to be pure ATK.

### Design

#### The abbreviation belongs to the slot type

`EquipmentSlot` (`crates/engine/src/items.rs:59`) already has `label()`
returning `"Weapon"` / `"Armor"` / `"Module"`. Add:

```rust
/// Compact form for space-constrained rows (see the inventory list's
/// equip tag). `label` stays the name for headers and prose.
pub fn short_label(self) -> &'static str  // "WEP" / "ARM" / "MOD"
```

Defining it beside the variants keeps a new slot from having to remember to
update renderer code. Uppercase matches the existing stat vocabulary sharing
the same parentheses (`+4 ATK`, `+3 DEF`, `+2 DECOMP`); a lowercase `wep`
next to `ATK` reads as a different class of token.

#### One formatter, in app-core

`tui/src/ui.rs:1653` and `gui/src/render.rs:1249` hold byte-identical copies
of `equip_preview_tag`, doc comment included. Since this change edits that
function, it moves to `crates/app-core/src/lib.rs` as a `pub fn` and both
renderers import it. app-core already hosts exactly this class of shared
renderer helper — `menu_shortcut`, `inventory_item_actions` — and already
depends on the engine, so nothing new is introduced.

The slot leads the existing parenthesised group:

```
Inventory — Buffer 7/30 (row key to equip/fuse/erase):
  [a] Monofilament Whip x1 (WEP +4 ATK)
  [b] Ablative Plating x2 (ARM +3 DEF)
  [c] Cortex Hack x1 (MOD +2 DECOMP fusion T1)
  [d] Core Fragment x12
  [e] Power Cell x3
```

No new punctuation and no width beyond the word itself. A non-equippable item
still returns an empty string, so `Core Fragment` and `Power Cell` are
unchanged.

The item-action popup (`ui.rs:1700`, `render.rs:1341`) calls the same
function, so its header gains the slot too — the screen where the player
confirms an equip is where the slot matters most.

#### Unchanged

The `Equipped:` header rows keep full `Weapon` / `Armor` / `Module` labels
(`ui.rs:1561-1563`, `render.rs` equivalent). They have the room, and
abbreviating a section header buys nothing.

### Tests

In `crates/app-core`, where the function now lives:

- A weapon's tag starts with `(WEP`, an armor's with `(ARM`, a module's with
  `(MOD`.
- A non-equippable item still yields an empty string.
- Level scaling and fusion tier still appear in the tag alongside the slot,
  so the hoist did not drop behaviour.
