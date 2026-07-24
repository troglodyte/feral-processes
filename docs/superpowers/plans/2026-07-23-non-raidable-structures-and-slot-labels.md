# Non-raidable structures and inventory slot labels — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Home immune to raids via a data-driven `raidable` flag, and show
which equipment slot (WEP/ARM/MOD) an inventory item would occupy.

**Architecture:** A non-raidable structure is spawned *without* a `Durability`
component; `Game::raid_check`'s existing `With<Durability>` target query then
excludes it with no new branching. Separately, the byte-identical
`equip_preview_tag` in both renderers is hoisted into `app-core` and then taught
to lead with the slot's new `short_label()`.

**Tech Stack:** Rust 2024, standalone `bevy_ecs`, `ron` for asset files, ratatui
(TUI) and macroquad (GUI) renderers.

**Spec:** `docs/superpowers/specs/2026-07-23-non-raidable-structures-and-slot-labels-design.md`

## Global Constraints

- New `StructureDef` fields MUST be `#[serde(default …)]` so existing `.ron`
  files, including third-party mods, keep parsing untouched.
- `assets/structures/README.md` MUST be updated in the same change as any
  schema field addition — it is the modder-facing schema reference.
- Run `cargo fmt` and `cargo clippy --workspace` after every change; fix
  warnings rather than silencing them.
- `cargo test --workspace` is the final gate before any task is called done
  (~200 tests, ~1s).
- Never commit unless the step says to. Never push.
- Comments explain *why*, never *what*.
- If many tests fail at once with `NotFound` on an assets path, that is stale
  build artifacts from the old `petmud` directory name, not real failures. Fix
  with `cargo clean -p feral-processes-engine -p feral-processes-app-core` —
  NOT a full `cargo clean` (`target/` is ~4 GB).

---

### Task 1: Non-raidable structures, with Home as the first one

**Files:**
- Modify: `crates/engine/src/structures.rs:124-155` (add field), `:157-159` (add default fn)
- Modify: `crates/engine/src/lib.rs:787-803` (save-load spawn site)
- Modify: `crates/engine/src/lib.rs:2004-2017` (deploy spawn site)
- Modify: `assets/structures/home.ron`
- Modify: `assets/structures/README.md`
- Test: `crates/engine/src/lib.rs` (the existing `mod tests` at the bottom)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `StructureDef::raidable: bool` (default `true`) and
  `structures::default_raidable() -> bool`. No later task depends on these.

Existing helpers your tests will use, all already in `mod tests`:
`test_assets_dir() -> std::path::PathBuf`, `place_home(game: &mut Game, dx: i32, dy: i32)`
(grants 5 Core Fragments then places Home). Already in scope in that module:
`Durability`, `Structure`, `Entity`, `With`, `Inventory`, `ItemId`, `ids`,
`DifficultyMode`, `HOME_STRUCTURE_ID`.

- [ ] **Step 1: Write the failing tests**

Append these five tests inside `mod tests` in `crates/engine/src/lib.rs` (put
them next to the other raid tests, near `raid_check_can_damage_an_undefended_structure`
around line 10286):

```rust
    /// Finds the deployed Home, if any. Home is the only structure of its
    /// kind, so the first match is the only match.
    fn find_home(game: &mut Game) -> Option<Entity> {
        let mut query = game.world.query::<(Entity, &Structure)>();
        query
            .iter(&game.world)
            .find(|(_, s)| s.kind == HOME_STRUCTURE_ID)
            .map(|(e, _)| e)
    }

    #[test]
    fn home_loads_as_non_raidable_and_other_structures_default_to_raidable() {
        let game = Game::new(700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let defs = game.structure_defs();

        let home = defs.iter().find(|d| d.id == "home").expect("home should load");
        assert!(!home.raidable, "home.ron must set raidable: false");

        let mining = defs
            .iter()
            .find(|d| d.id == "mining_node")
            .expect("mining_node should load");
        assert!(
            mining.raidable,
            "a structure file that omits `raidable` must default to raidable"
        );
    }

    #[test]
    fn deploying_home_gives_it_no_durability_pool() {
        let mut game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game, -1, 0);
        let home = find_home(&mut game).expect("place_home should have spawned a Home");

        assert!(
            game.world.get::<Durability>(home).is_none(),
            "a non-raidable structure must not carry a Durability pool at all"
        );
    }

    #[test]
    fn deploying_a_raidable_structure_still_gives_it_a_durability_pool() {
        // Seed 300 is known to have walkable terrain at both offsets — it's
        // the seed `place_structure_rejects_anything_but_home_until_a_home_exists`
        // already places two structures on.
        let mut game = Game::new(300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game, -1, 0);
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 20);
        game.place_structure("mining_node", 1, 0).unwrap();

        let node = {
            let mut query = game.world.query::<(Entity, &Structure)>();
            query
                .iter(&game.world)
                .find(|(_, s)| s.kind == "mining_node")
                .map(|(e, _)| e)
                .expect("the mining node should have been deployed")
        };

        let durability = game
            .world
            .get::<Durability>(node)
            .expect("a raidable structure must still get its Durability pool");
        assert_eq!(durability.hp, durability.max_hp);
        assert!(durability.max_hp > 0);
    }

    #[test]
    fn raid_check_never_targets_home_even_as_the_only_structure() {
        let mut game = Game::new(702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Strip every pre-existing Durability holder (habitat nests and
        // anything else the world seeded) so a raid has no legal target left
        // at all if Home genuinely isn't one.
        let existing: Vec<Entity> = {
            let mut query = game.world.query_filtered::<Entity, With<Durability>>();
            query.iter(&game.world).collect()
        };
        for e in existing {
            game.world.despawn(e);
        }
        place_home(&mut game, -1, 0);

        for _ in 0..500 {
            game.raid_check();
        }

        let home_still_standing = {
            let mut query = game.world.query::<&Structure>();
            query.iter(&game.world).any(|s| s.kind == HOME_STRUCTURE_ID)
        };
        assert!(
            home_still_standing,
            "Home must survive every raid roll — it can't be a raid target at all"
        );
        let home = find_home(&mut game).expect("checked above: Home is standing");
        assert!(
            game.world.get::<Durability>(home).is_none(),
            "Home must still have no Durability pool after the raid rolls"
        );
    }

    #[test]
    fn home_survives_save_and_load_without_gaining_a_durability_pool() {
        let assets = test_assets_dir();
        let mut game = Game::new(703, DifficultyMode::Forgiving, &assets).unwrap();
        place_home(&mut game, -1, 0);

        let path = std::env::temp_dir().join(format!(
            "feral_processes_home_raidable_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let home = find_home(&mut loaded).expect("Home should survive a save/load round trip");
        assert!(
            loaded.world.get::<Durability>(home).is_none(),
            "the load path must not re-attach Durability to a non-raidable structure"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:
```sh
cargo test -p feral-processes-engine raidable
```
Expected: compile error — `no field 'raidable' on type 'StructureDef'`. That
counts as the failing state; the field does not exist yet.

- [ ] **Step 3: Add the `raidable` field to the schema**

In `crates/engine/src/structures.rs`, add this field to `StructureDef`
immediately after the `durability` field (which ends at line 125):

```rust
    /// Whether raids can target this structure. A non-raidable structure is
    /// spawned with no `Durability` component at all, which is what keeps
    /// `Game::raid_check` — whose target query is `With<Durability>` — from
    /// ever selecting it, and leaves `durability` above inert.
    /// `#[serde(default = "default_raidable")]` so existing structure files
    /// (including mods) stay raidable, exactly as before this field existed.
    #[serde(default = "default_raidable")]
    pub raidable: bool,
```

Then add the default function next to `default_durability` (line 157):

```rust
fn default_raidable() -> bool {
    true
}
```

- [ ] **Step 4: Make the deploy path honour it**

In `crates/engine/src/lib.rs`, replace the spawn at lines 2004-2017 (inside
`place_structure`):

```rust
        let mut entity = self.world.spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x, y },
            Glyph {
                ch: def.glyph,
                color: def.color,
            },
            Durability {
                hp: def.durability,
                max_hp: def.durability,
            },
        ));
```

with:

```rust
        let mut entity = self.world.spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x, y },
            Glyph {
                ch: def.glyph,
                color: def.color,
            },
        ));
        if def.raidable {
            entity.insert(Durability {
                hp: def.durability,
                max_hp: def.durability,
            });
        }
```

- [ ] **Step 5: Make the save-load path honour it**

In `crates/engine/src/lib.rs`, replace the spawn at lines 787-803 (inside the
save-loading loop over `data.structures`):

```rust
            let mut entity = game.world.spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position {
                    x: s.position.0,
                    y: s.position.1,
                },
                Glyph {
                    ch: def.glyph,
                    color: def.color,
                },
                Durability {
                    hp: s.durability.unwrap_or(def.durability).min(def.durability),
                    max_hp: def.durability,
                },
            ));
```

with:

```rust
            let mut entity = game.world.spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position {
                    x: s.position.0,
                    y: s.position.1,
                },
                Glyph {
                    ch: def.glyph,
                    color: def.color,
                },
            ));
            // A save written before `raidable` existed still records a
            // durability for what is now a non-raidable structure; the def
            // wins, so that stored value is simply dropped.
            if def.raidable {
                entity.insert(Durability {
                    hp: s.durability.unwrap_or(def.durability).min(def.durability),
                    max_hp: def.durability,
                });
            }
```

- [ ] **Step 6: Mark Home non-raidable**

Replace `assets/structures/home.ron` entirely with:

```ron
(
    id: "home",
    name: "Home",
    glyph: 'H',
    color: Green,
    build_cost: [("core_fragment", 5)],
    work: None,
    teleport_cost: Some([("power_cell", 4)]),
    raidable: false,
)
```

- [ ] **Step 7: Run the tests to verify they pass**

Run:
```sh
cargo test -p feral-processes-engine raidable
cargo test -p feral-processes-engine home_survives_save_and_load
cargo test -p feral-processes-engine raid_check
```
Expected: PASS for all, including the pre-existing `raid_check_*` tests (they
spawn their own `Durability` holders directly, so they are unaffected).

- [ ] **Step 8: Document the field for modders**

In `assets/structures/README.md`, add this block to the schema comment
immediately after the existing `durability: 30,` entry:

```ron
    // Optional; can be left out entirely (defaults to true). Set to false
    // to make the structure impossible to raid: it's deployed with no
    // durability pool at all, so `Game::raid_check` can never select it,
    // it never takes damage, and no [HP x/y] is shown for it anywhere.
    // `durability` above is inert when this is false. This is how Home
    // works — losing the structure that gates every other build, anchors
    // symlinks, and can only exist once would strand the player rather
    // than cost them something.
    raidable: false,
```

Then update the existing `durability` entry's comment to note the
interaction — change its last line from:

```
    // Damaged structures slowly regenerate over time regardless.
```

to:

```
    // Damaged structures slowly regenerate over time regardless. Ignored
    // entirely when `raidable: false` (see below).
```

- [ ] **Step 9: Format, lint, and run the full suite**

Run:
```sh
cargo fmt
cargo clippy --workspace
cargo test --workspace
```
Expected: no warnings, all tests pass.

- [ ] **Step 10: Commit**

```bash
git add crates/engine/src/structures.rs crates/engine/src/lib.rs \
        assets/structures/home.ron assets/structures/README.md
git commit -m "Add a raidable structure flag and make Home non-raidable

A non-raidable structure is spawned with no Durability component, so
raid_check's existing With<Durability> target query excludes it without
any new branching. Defaults to true so every existing structure file,
mods included, keeps parsing unchanged."
```

Note: `crates/engine/src/lib.rs` may carry an unrelated pre-existing
modification (a `battle_view_integrity_matches_the_map_status_panel` test) that
was in the working tree before this plan started. Do not commit it — if
`git diff --cached` shows it, unstage with `git restore --staged` and stage only
your hunks via `git add -p`.

---

### Task 2: Hoist `equip_preview_tag` into app-core (no behaviour change)

**Files:**
- Modify: `crates/app-core/src/lib.rs` (add the function, near `inventory_item_actions` at line 58)
- Modify: `crates/tui/src/ui.rs:7-9` (import), delete `:1648-1674` (the function)
- Modify: `crates/gui/src/render.rs:11-13` (import), delete `:1244-1270` (the function)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `pub fn equip_preview_tag(game: &Game, item: &ItemId, zone_level: u32, fusion_tier: u32) -> String`
  in `feral_processes_app_core`. Task 3 modifies this exact function.

This task is a pure move. The two renderer copies are byte-identical, doc
comment included — verify with `diff` in Step 1 before deleting either.

- [ ] **Step 1: Confirm the two copies are identical**

Run:
```sh
diff <(sed -n '1648,1674p' crates/tui/src/ui.rs) <(sed -n '1244,1270p' crates/gui/src/render.rs)
```
Expected: no output (identical). If they differ, stop and report the
difference rather than guessing which copy is canonical.

- [ ] **Step 2: Add the function to app-core**

In `crates/app-core/src/lib.rs`, insert after `inventory_item_actions` (which
ends at line 75):

```rust
/// Formats an equippable item's stat bonus as it would be *if equipped right
/// now* — gear scales with the current zone level at the moment you equip it
/// (see `Game::equip`), so this previews that same number rather than a flat,
/// unscaled base value. Empty string for a non-equippable item.
///
/// Lives here rather than in either renderer because both draw the identical
/// tag, on both the inventory list and the item-action page.
pub fn equip_preview_tag(game: &Game, item: &ItemId, zone_level: u32, fusion_tier: u32) -> String {
    let Some((_, base_mods)) = game.equipment_of(item) else {
        return String::new();
    };
    let mods = base_mods
        .scaled_for_level(zone_level)
        .fused_for_tier(fusion_tier);
    let mut parts = Vec::new();
    if mods.atk != 0 {
        parts.push(format!("+{} ATK", mods.atk));
    }
    if mods.def != 0 {
        parts.push(format!("+{} DEF", mods.def));
    }
    if mods.decompiler != 0 {
        parts.push(format!("+{} DECOMP", mods.decompiler));
    }
    if fusion_tier > 0 {
        parts.push(format!("fusion T{fusion_tier}"));
    }
    format!(" ({})", parts.join(" "))
}
```

- [ ] **Step 3: Point the TUI at it**

In `crates/tui/src/ui.rs`, change the app-core import (lines 7-9) to:

```rust
use feral_processes_app_core::{
    App, MENU_SCAN_RADIUS, Mode, TradeChoice, equip_preview_tag, inventory_item_actions,
    menu_shortcut,
};
```

Then delete the entire local `equip_preview_tag` function — the doc comment at
line 1648 through the closing `}` at line 1674. Both call sites (line 1578 and
line 1700) stay exactly as they are; they now resolve to the imported function.

- [ ] **Step 4: Point the GUI at it**

In `crates/gui/src/render.rs`, change the app-core import (lines 11-13) to:

```rust
use feral_processes_app_core::{
    App, MENU_SCAN_RADIUS, Mode, TradeChoice, equip_preview_tag, inventory_item_actions,
    menu_shortcut,
};
```

Then delete the entire local `equip_preview_tag` function — the doc comment at
line 1244 through the closing `}` at line 1270. Both call sites (line 1227 and
line 1341) stay exactly as they are.

- [ ] **Step 5: Verify the workspace still builds and behaves identically**

Run:
```sh
cargo fmt
cargo clippy --workspace
cargo test --workspace
```
Expected: no warnings (in particular no unused-import warning in either
renderer), all tests pass. No test output should change — this task adds no
behaviour.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/lib.rs crates/tui/src/ui.rs crates/gui/src/render.rs
git commit -m "Hoist equip_preview_tag into app-core

Both renderers held byte-identical copies, doc comment included. app-core
already owns this class of shared renderer helper (menu_shortcut,
inventory_item_actions), so there is one definition to change now."
```

---

### Task 3: Show the equipment slot on inventory rows

**Files:**
- Modify: `crates/engine/src/items.rs:59-67` (add `short_label` to `EquipmentSlot`)
- Modify: `crates/app-core/src/lib.rs` (the `equip_preview_tag` added in Task 2)
- Test: `crates/app-core/src/lib.rs` (the existing `mod tests` at line 1679)

**Interfaces:**
- Consumes: `feral_processes_app_core::equip_preview_tag` from Task 2 —
  `pub fn equip_preview_tag(game: &Game, item: &ItemId, zone_level: u32, fusion_tier: u32) -> String`.
- Produces: `EquipmentSlot::short_label(self) -> &'static str`, returning
  `"WEP"` / `"ARM"` / `"MOD"`.

Shipped items you will use, all reachable from `items::ids` (already imported
under `#[cfg(test)]` in app-core at line 14):
- `ids::MONOFILAMENT_WHIP` — Weapon, `atk: 4`
- `ids::ABLATIVE_PLATING` — Armor, `def: 4`
- `ids::CORTEX_HACK` — Module, `decompiler: 3`
- `ids::CORE_FRAGMENT` — not equipment at all

At zone level 1, `scaled_for_level(1)` multiplies by `GEAR_LEVEL_GROWTH^0` = 1,
and `fused_for_tier(0)` multiplies by 1.0, so the bonuses above appear
unscaled. The tag has a **leading space** — `" (WEP +4 ATK)"` — because callers
concatenate it straight onto the row text.

The existing `test_app(seed) -> App` helper in app-core's `mod tests` builds an
`App` with `app.game: Option<Game>` already populated.

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `crates/app-core/src/lib.rs`:

```rust
    #[test]
    fn equip_preview_tag_leads_with_the_slot_the_item_would_take() {
        let app = test_app(900);
        let game = app.game.as_ref().expect("test_app builds a game");

        assert_eq!(
            equip_preview_tag(game, &ItemId::from(ids::MONOFILAMENT_WHIP), 1, 0),
            " (WEP +4 ATK)"
        );
        assert_eq!(
            equip_preview_tag(game, &ItemId::from(ids::ABLATIVE_PLATING), 1, 0),
            " (ARM +4 DEF)"
        );
        assert_eq!(
            equip_preview_tag(game, &ItemId::from(ids::CORTEX_HACK), 1, 0),
            " (MOD +3 DECOMP)"
        );
    }

    #[test]
    fn equip_preview_tag_stays_empty_for_a_non_equippable_item() {
        let app = test_app(901);
        let game = app.game.as_ref().expect("test_app builds a game");

        assert_eq!(
            equip_preview_tag(game, &ItemId::from(ids::CORE_FRAGMENT), 1, 0),
            "",
            "a non-equippable item must contribute no tag at all, not a bare slot"
        );
    }

    #[test]
    fn equip_preview_tag_keeps_showing_level_scaling_and_fusion_beside_the_slot() {
        let app = test_app(902);
        let game = app.game.as_ref().expect("test_app builds a game");

        // Zone 2 doubles the base bonus (GEAR_LEVEL_GROWTH), and one fusion
        // tier adds ITEM_FUSION_BONUS_PER_TIER on top: 4 -> 8 -> 9.
        assert_eq!(
            equip_preview_tag(game, &ItemId::from(ids::MONOFILAMENT_WHIP), 2, 1),
            " (WEP +9 ATK fusion T1)"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:
```sh
cargo test -p feral-processes-app-core equip_preview_tag
```
Expected: FAIL — `equip_preview_tag_leads_with_the_slot_the_item_would_take`
and the scaling test both fail on the assertion, showing the current
slot-less output (`" (+4 ATK)"`, `" (+9 ATK fusion T1)"`).
`equip_preview_tag_stays_empty_for_a_non_equippable_item` should already pass.

- [ ] **Step 3: Add `short_label` to the slot type**

In `crates/engine/src/items.rs`, add this method to the `impl EquipmentSlot`
block, right after `label` (which ends at line 66):

```rust
    /// Compact form for space-constrained rows — see the inventory list's
    /// equip tag, where it sits beside `ATK`/`DEF`/`DECOMP` and so matches
    /// their case. `label` stays the name for headers and prose.
    pub fn short_label(self) -> &'static str {
        match self {
            EquipmentSlot::Weapon => "WEP",
            EquipmentSlot::Armor => "ARM",
            EquipmentSlot::Module => "MOD",
        }
    }
```

- [ ] **Step 4: Lead the tag with the slot**

In `crates/app-core/src/lib.rs`, in `equip_preview_tag`, bind the slot instead
of discarding it and seed `parts` with it. Change:

```rust
    let Some((_, base_mods)) = game.equipment_of(item) else {
        return String::new();
    };
    let mods = base_mods
        .scaled_for_level(zone_level)
        .fused_for_tier(fusion_tier);
    let mut parts = Vec::new();
```

to:

```rust
    let Some((slot, base_mods)) = game.equipment_of(item) else {
        return String::new();
    };
    let mods = base_mods
        .scaled_for_level(zone_level)
        .fused_for_tier(fusion_tier);
    let mut parts = vec![slot.short_label().to_string()];
```

Also update the function's doc comment first line to say what it now shows —
replace:

```rust
/// Formats an equippable item's stat bonus as it would be *if equipped right
```

with:

```rust
/// Formats the slot an equippable item would occupy plus its stat bonus as it
/// would be *if equipped right
```

- [ ] **Step 5: Run the tests to verify they pass**

Run:
```sh
cargo test -p feral-processes-app-core equip_preview_tag
```
Expected: PASS, all three.

- [ ] **Step 6: Format, lint, and run the full suite**

Run:
```sh
cargo fmt
cargo clippy --workspace
cargo test --workspace
```
Expected: no warnings, all tests pass. Both renderers pick the slot up with no
edit of their own — the inventory list and the item-action page share this
function.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/items.rs crates/app-core/src/lib.rs
git commit -m "Show which slot an inventory item would equip into

Rows now read '(WEP +4 ATK)' instead of '(+4 ATK)'. The abbreviation lives
on EquipmentSlot beside label(), so a new slot can't forget to define one.
Both renderers share the formatter, so both pick it up."
```

---

## Verification

After all three tasks:

```sh
cargo test --workspace     # ~200 tests, ~1s
cargo clippy --workspace
cargo fmt --check
```

The Home change has no visual verification step — its effect is the *absence*
of an `[HP x/y]` readout and the absence of raid messages naming Home. The slot
change is visible on the inventory screen (`v`) in either renderer, but per
standing project policy it is verified by the app-core unit tests above rather
than by launching the GUI.
