# Manifest Screen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One read-only full-window stat sheet — the Manifest — showing every stat the engine tracks for the player, an owned program, or a wild program.

**Architecture:** A single engine accessor `Game::manifest(entity) -> Option<ManifestView>` returns a shared header plus a `ManifestSubject` enum (`Player` | `Program`) carrying the half that differs. A new GUI screen draws it against `Painter` using a pure, headlessly-testable `manifest_layout` for geometry — the same split `popup_layout` already uses. It replaces `Mode::InspectDetail`'s popup entirely; `InspectView`, `Game::inspect` and `render/inspection.rs` are deleted.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine only), `bevy` + `bevy_egui` (gui only, behind `paint.rs`).

**Spec:** `docs/superpowers/specs/2026-07-27-manifest-screen-design.md`

## Global Constraints

- **The renderer never touches the ECS `World`.** All data reaches the GUI through `Game`'s public methods and the plain-data views in `crates/engine/src/views.rs`. `Game::world` is private; do not add an accessor.
- **`crates/gui/src/paint.rs` is the only file allowed to name a graphics library.** Everything in `crates/gui/src/render/` draws through `Painter` (`rect`, `rect_lines`, `line`, `ui`, `ui_bold`, `map`, `measure_ui`, `measure_map`, `clear`, `screen_w`, `screen_h`, `delta`) and the local `Color`/`Rect`/`TextDims`. No `bevy`/`egui` imports in `render/`.
- **No backwards-compat cruft.** When this plan says delete something, delete it — no shims, no `// removed` comments, no unused-variable renames.
- **Comments explain *why*, never *what*.**
- **Run `cargo fmt` and `cargo clippy --workspace` after every task**; fix warnings rather than silencing them.
- **`cargo test --workspace` is the final gate** (599 tests before this work). Per-task test commands are given, but the workspace suite must pass before the last task is called done.
- No `tuning.rs` or `.ron` asset changes anywhere in this plan, so `balance_sim`'s curves must not move. If a `balance_sim` test fails, something is wrong with the change, not the test.
- Existing helper names used verbatim by this plan: `Game::effective_atk`, `Game::effective_def`, `Game::program_activity`, `Game::pet_count`, `Game::pet_capacity`, `Game::party_info`, `Game::potential_quality_label`, `Game::routine_view`, `Game::status_label`, `Game::fusion_count`, `Game::taming_catalyst`, `Game::player_decompiler_skill`, `Game::zone_tagged_name`, `Game::is_boss_creature`, `Game::equipment_of`, `Game::item_name`, `taming::capture_chance`, `tuning::MAX_FUSIONS`, `bars::draw_bar`, `bars::bar_row_height`, `text::ui_metrics`.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/engine/src/views.rs` | **Modify.** Add `ManifestView`, `ManifestSubject`, `PlayerManifest`, `ProgramManifest`, `ManifestEquipSlot`, `ManifestPotential`. Delete `InspectView`. |
| `crates/engine/src/game/inspection.rs` | **Modify.** Add `Game::manifest` plus two private builders. Delete `Game::inspect`. |
| `crates/engine/src/tests/inspection.rs` | **Modify.** Port the two `inspect` tests to `manifest`, add the new coverage. |
| `crates/gui/src/render/manifest_layout.rs` | **Create.** Pure geometry — `Section`, `SectionRow`, `ManifestLayout`, `manifest_layout`. No `Painter` use, fully headless-testable. |
| `crates/gui/src/render/manifest.rs` | **Create.** `draw_manifest` and `draw_manifest_pick` — turns a `ManifestView` into sections and paints them. |
| `crates/gui/src/render/inspection.rs` | **Delete.** |
| `crates/gui/src/render/mod.rs` | **Modify.** Swap the module wiring and the `Mode` arms. |
| `crates/app-core/src/lib.rs` | **Modify.** `Mode::InspectDetail` → `Mode::Manifest`; add `Mode::ManifestPick`; `pending_inspect` → `pending_manifest`. |
| `crates/app-core/src/app/inspection.rs` | **Modify.** Rename the handlers, add the picker and ←/→ cycling. |
| `crates/app-core/src/app/input.rs` | **Modify.** Dispatch the two modes. |
| `crates/app-core/src/app/playing.rs` | **Modify.** Bind `d`. |
| `crates/app-core/src/app/lifecycle.rs` | **Modify.** Rename the field initialiser. |
| `crates/app-core/src/tests/menus.rs` | **Modify.** Add the app-core coverage. |
| `docs/manual.md`, `CHANGELOG.md` | **Modify.** Document the screen and the `d` key. |

---

### Task 1: Engine — `ManifestView` and `Game::manifest`

Builds the whole data layer. `Game::inspect` stays alive through this task so the GUI keeps compiling; Task 3 deletes it.

**Files:**
- Modify: `crates/engine/src/views.rs` (append after `InspectView`)
- Modify: `crates/engine/src/game/inspection.rs` (append inside the existing `impl Game`)
- Test: `crates/engine/src/tests/inspection.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `pub fn Game::manifest(&self, entity: Entity) -> Option<ManifestView>`
  - `pub struct ManifestView { entity: Entity, name: String, glyph: char, color: GlyphColor, level: Option<u32>, xp: Option<(u32, u32)>, hp: i32, max_hp: i32, atk: i32, def: i32, power: i32, status_effect: Option<String>, routines: Vec<RoutineSlotView>, subject: ManifestSubject }`
  - `pub enum ManifestSubject { Player(PlayerManifest), Program(ProgramManifest) }`
  - `pub struct PlayerManifest { hunger: f32, fatigue: f32, decompiler: i32, equipment: Vec<ManifestEquipSlot>, perk_points: u32, perks: Vec<(String, u32)>, position: (i32, i32), zone: u32, pet_count: usize, pet_capacity: usize, cargo_used: u32, party: Vec<CompanionInfo> }`
  - `pub struct ManifestEquipSlot { slot: String, item_name: String, gear_level: u32, fusion_tier: u32, atk: i32, def: i32, decompiler: i32 }`
  - `pub struct ProgramManifest { species_name: Option<String>, is_hostile: bool, is_tamed: bool, is_companion: bool, is_boss: bool, activity: Option<String>, potential: Option<ManifestPotential>, fusions: u32, max_fusions: u32, habitats: Vec<Biome>, moves: Vec<MoveDef>, work_resource: Option<ItemId>, taming_difficulty: f32, decompile_chance: Option<f32>, growth_multiplier: f32, base_speed: i32 }`
  - `pub struct ManifestPotential { hp_roll: f32, atk_roll: f32, def_roll: f32, growth_roll: f32, percent: u32, label: String }`
  - All re-exported automatically by the existing `pub use views::*;` in `crates/engine/src/lib.rs`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/inspection.rs`:

```rust
#[test]
fn manifest_reports_the_player_with_equipment_folded_into_their_stats() {
    let game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let status = game.player_status();

    let view = game.manifest(player).expect("the player has a manifest");
    assert_eq!(view.name, "You");
    assert_eq!(view.hp, status.hp);
    assert_eq!(view.max_hp, status.max_hp);
    assert_eq!(
        (view.atk, view.def, view.power),
        (status.atk, status.def, status.power),
        "the manifest must quote the same effective stats the sidebar does"
    );
    assert_eq!(view.level, Some(status.level));
    assert_eq!(view.xp, Some((status.xp, status.xp_to_next)));

    let ManifestSubject::Player(p) = view.subject else {
        panic!("the player is a Player subject");
    };
    assert_eq!(p.hunger, status.hunger);
    assert_eq!(p.fatigue, status.fatigue);
    assert_eq!(p.decompiler, status.decompiler);
    assert_eq!(p.zone, status.zone);
    assert_eq!(p.position, status.position);
    assert_eq!(p.pet_capacity, status.pet_capacity);
}

#[test]
fn manifest_lists_every_equipped_item_with_the_bonus_it_is_actually_granting() {
    let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let equippable = game
        .item_defs()
        .into_iter()
        .find(|d| d.equipment.is_some())
        .expect("the shipped item set has equippable gear");
    let item = ItemId::from(equippable.id.as_str());
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(item.clone(), 1);
    game.equip(&item).expect("equipping a held item works");

    let view = game.manifest(player).unwrap();
    let ManifestSubject::Player(p) = view.subject else {
        panic!("the player is a Player subject");
    };
    let slot = p
        .equipment
        .iter()
        .find(|s| s.item_name == equippable.name)
        .expect("the item just equipped is listed");
    let (_, base) = game.equipment_of(&item).unwrap();
    let expected = base
        .scaled_for_level(slot.gear_level)
        .fused_for_tier(slot.fusion_tier);
    assert_eq!(
        (slot.atk, slot.def, slot.decompiler),
        (expected.atk, expected.def, expected.decompiler),
        "the listed bonus must be the one captured at equip time, not a fresh preview"
    );
}

#[test]
fn manifest_reports_a_tamed_program_with_all_four_potential_rolls() {
    let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 20, 5);
    game.world.entity_mut(pet).insert(Potential {
        hp_roll: 1.10,
        atk_roll: 1.05,
        def_roll: 0.95,
        growth_roll: 1.15,
    });

    let view = game.manifest(pet).expect("a tamed program has a manifest");
    assert_eq!(view.max_hp, 20);
    assert!(
        !view.routines.is_empty(),
        "spawn_tamed installs the species' innate routines"
    );

    let ManifestSubject::Program(p) = view.subject else {
        panic!("a creature is a Program subject");
    };
    assert!(p.is_tamed);
    assert!(!p.is_hostile);
    assert_eq!(p.max_fusions, MAX_FUSIONS);
    assert_eq!(
        p.activity.as_deref(),
        Some("idle"),
        "an owned program always reports what it is doing"
    );
    let rolls = p.potential.expect("the rolls were just inserted");
    assert_eq!(
        (rolls.hp_roll, rolls.atk_roll, rolls.def_roll, rolls.growth_roll),
        (1.10, 1.05, 0.95, 1.15),
        "every roll is surfaced individually, not just the aggregate tier"
    );
    assert!(!rolls.label.is_empty());
}

#[test]
fn manifest_of_a_wild_program_has_no_experience_and_no_activity() {
    let mut game = Game::new(14, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);

    let view = game.manifest(wild).expect("a wild program has a manifest");
    assert_eq!(
        view.xp, None,
        "a wild program carries no Experience until it is compiled"
    );
    let ManifestSubject::Program(p) = view.subject else {
        panic!("a creature is a Program subject");
    };
    assert!(p.is_hostile);
    assert!(!p.is_tamed);
    assert_eq!(p.activity, None, "a program you don't own isn't doing a job");
    assert!(
        p.decompile_chance.is_some(),
        "the starting kit includes a taming catalyst"
    );
    assert!(!game.has_active_battle(), "a manifest never starts a fight");
}

#[test]
fn manifest_survives_a_creature_that_predates_the_potential_component() {
    let mut game = Game::new(15, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 12, 3);
    game.world.entity_mut(pet).remove::<Potential>();

    let view = game.manifest(pet).expect("still inspectable without a roll");
    let ManifestSubject::Program(p) = view.subject else {
        panic!("a creature is a Program subject");
    };
    assert_eq!(p.potential, None);
}

#[test]
fn manifest_returns_none_for_anything_that_is_neither_the_player_nor_a_creature() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game.world.spawn(Position { x: 0, y: 0 }).id();
    assert!(game.manifest(structure).is_none());
}
```

Add to that file's imports whatever is missing — it already has `use super::support::*;` and `use crate::*;`, which cover `Potential`, `Inventory`, `ItemId`, `MAX_FUSIONS` and `spawn_tamed`/`spawn_wild_on_player_tile`. `MAX_FUSIONS` comes from `crate::tuning::MAX_FUSIONS`; add `use crate::tuning::MAX_FUSIONS;` if `crate::*` does not already bring it in.

`ManifestPotential` must derive `PartialEq` and `Debug` for the `assert_eq!(p.potential, None)` above.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine inspection`
Expected: FAIL — `no method named 'manifest' found for struct 'Game'`.

- [ ] **Step 3: Add the view types**

Append to `crates/engine/src/views.rs`, after `InspectView`:

```rust
/// Everything the engine knows about one subject, for the manifest screen —
/// the player, a program you own, or a wild one. Shared header fields plus a
/// `subject` carrying the half that differs, so "the player has no Potential
/// roll" and "a program has no equipment" are type-level facts rather than
/// `Option`s a renderer can forget to check.
pub struct ManifestView {
    pub entity: Entity,
    /// "You" for the player; a program's `CustomName` if it has one, else its
    /// zone-tagged species name (see `Game::zone_tagged_name`).
    pub name: String,
    pub glyph: char,
    pub color: GlyphColor,
    /// `None` for a wild program, which carries no `Experience` until it is
    /// compiled.
    pub level: Option<u32>,
    /// `(xp, xp_to_next)`, `None` for the same reason `level` is.
    pub xp: Option<(u32, u32)>,
    pub hp: i32,
    pub max_hp: i32,
    /// The player's is `Game::effective_atk` (equipment folded in); a
    /// program's is its raw `Stats`.
    pub atk: i32,
    pub def: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    /// Active battle status condition, e.g. "Bleeding (2)" — see
    /// `Game::status_label`. Always `None` outside an intrusion.
    pub status_effect: Option<String>,
    /// Every routine slot, filled or empty. Reuses `RoutineSlotView` rather
    /// than a parallel type, so the manifest and the routines menu cannot
    /// disagree about what is installed.
    pub routines: Vec<RoutineSlotView>,
    pub subject: ManifestSubject,
}

pub enum ManifestSubject {
    Player(PlayerManifest),
    Program(ProgramManifest),
}

/// The player-only half of a manifest.
pub struct PlayerManifest {
    pub hunger: f32,
    pub fatigue: f32,
    pub decompiler: i32,
    /// One entry per *occupied* slot — an empty slot is absent rather than
    /// listed as "(none)", so the section shrinks to what is actually worn.
    pub equipment: Vec<ManifestEquipSlot>,
    pub perk_points: u32,
    /// Every perk bought at least once, as (display name, level).
    pub perks: Vec<(String, u32)>,
    pub position: (i32, i32),
    pub zone: u32,
    pub pet_count: usize,
    pub pet_capacity: usize,
    pub cargo_used: u32,
    pub party: Vec<CompanionInfo>,
}

/// One worn item and the bonus it is *currently* granting.
///
/// `gear_level`/`fusion_tier` are the values captured on the `EquippedItem`
/// at equip time, and the stat fields are `EquipmentStats::scaled_for_level`
/// then `fused_for_tier` applied with exactly those — not a fresh preview at
/// today's zone level, which is what the inventory screen shows instead.
pub struct ManifestEquipSlot {
    /// `EquipmentSlot::label()` — "Weapon", "Armor", "Module".
    pub slot: String,
    pub item_name: String,
    pub gear_level: u32,
    pub fusion_tier: u32,
    pub atk: i32,
    pub def: i32,
    pub decompiler: i32,
}

/// The creature-only half of a manifest — an owned program or a wild one.
pub struct ProgramManifest {
    /// The species name, present only when a `CustomName` is overriding it,
    /// so the header can show "Hexed (Scrapper 2)" without repeating itself
    /// for an unrenamed program.
    pub species_name: Option<String>,
    pub is_hostile: bool,
    pub is_tamed: bool,
    pub is_companion: bool,
    pub is_boss: bool,
    /// What this program is doing right now — see `Game::program_activity`.
    /// `None` for a program you don't own, which has no job to report.
    pub activity: Option<String>,
    /// `None` for a creature with no `Potential` component — an old save
    /// predating it, or a test helper that spawned one directly.
    pub potential: Option<ManifestPotential>,
    pub fusions: u32,
    /// `tuning::MAX_FUSIONS`, carried so the renderer prints "1/3" without
    /// importing a tuning constant of its own.
    pub max_fusions: u32,
    pub habitats: Vec<Biome>,
    pub moves: Vec<MoveDef>,
    pub work_resource: Option<ItemId>,
    pub taming_difficulty: f32,
    /// Estimated decompile chance if an intrusion started right now, using
    /// the creature's current HP fraction. `None` when the player holds no
    /// taming catalyst: there is no potency to quote odds for, and the action
    /// isn't available at all.
    pub decompile_chance: Option<f32>,
    pub growth_multiplier: f32,
    pub base_speed: i32,
}

/// An individual's four `Potential` rolls, surfaced separately rather than
/// only as the aggregate tier the party menu shows.
#[derive(Debug, PartialEq)]
pub struct ManifestPotential {
    pub hp_roll: f32,
    pub atk_roll: f32,
    pub def_roll: f32,
    pub growth_roll: f32,
    /// `Potential::quality_percent`.
    pub percent: u32,
    /// `Potential::quality_label`.
    pub label: String,
}
```

`views.rs` already imports `EquippedItem`, `GlyphColor`, `ItemId`, `Perk`, `MoveDef`, `Biome` and `Entity`. `EquippedItem` and `Perk` may become unused once `InspectView` goes in Task 3 — leave the imports alone here; Task 3 cleans them up if clippy flags them.

- [ ] **Step 4: Implement `Game::manifest`**

Append inside the existing `impl Game` block in `crates/engine/src/game/inspection.rs`, next to `inspect`:

```rust
    /// Everything known about one subject, for the manifest screen. Works on
    /// the player and on any creature — wild, owned, or in the party.
    /// Read-only: looking a program over never triggers an intrusion.
    ///
    /// `None` for anything that is neither (a structure, a nest, a despawned
    /// entity), or for a creature whose species failed to resolve.
    pub fn manifest(&self, entity: Entity) -> Option<ManifestView> {
        if self.world.get::<Player>(entity).is_some() {
            return self.player_manifest(entity);
        }
        self.program_manifest(entity)
    }

    fn player_manifest(&self, entity: Entity) -> Option<ManifestView> {
        let stats = self.world.get::<Stats>(entity)?;
        let needs = self.world.get::<Needs>(entity)?;
        let pos = self.world.get::<Position>(entity)?;
        let inv = self.world.get::<Inventory>(entity)?;
        let exp = self.world.get::<Experience>(entity)?;
        let glyph = self.world.get::<Glyph>(entity)?;
        // The same calls `player_status` makes, so the sidebar and the sheet
        // cannot show different numbers for the same player.
        let atk = self.effective_atk(entity);
        let def = self.effective_def(entity);
        let equipment = self.world.get::<Equipment>(entity).cloned().unwrap_or_default();
        let perks = self.world.get::<Perks>(entity);
        Some(ManifestView {
            entity,
            name: "You".to_string(),
            glyph: glyph.ch,
            color: glyph.color,
            level: Some(exp.level),
            xp: Some((exp.xp, exp.xp_to_next)),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk,
            def,
            power: stats.max_hp + atk + def,
            status_effect: self.status_label(entity),
            routines: self.routine_view(entity),
            subject: ManifestSubject::Player(PlayerManifest {
                hunger: needs.hunger,
                fatigue: needs.fatigue,
                decompiler: self.world.get::<Decompiler>(entity).map(|d| d.skill).unwrap_or(0),
                equipment: [
                    EquipmentSlot::Weapon,
                    EquipmentSlot::Armor,
                    EquipmentSlot::Module,
                ]
                .into_iter()
                .filter_map(|slot| self.manifest_equip_slot(slot, equipment.get(slot)?))
                .collect(),
                perk_points: perks.map(|p| p.points).unwrap_or(0),
                perks: perks
                    .map(|p| {
                        Perk::all()
                            .into_iter()
                            .map(|perk| (perk, p.level(perk)))
                            .filter(|(_, level)| *level > 0)
                            .map(|(perk, level)| (perk.display_name().to_string(), level))
                            .collect()
                    })
                    .unwrap_or_default(),
                position: (pos.x, pos.y),
                zone: self.world.resource::<ZoneLevel>().0,
                pet_count: self.pet_count(),
                pet_capacity: self.pet_capacity(),
                cargo_used: inv.cargo_used(self.world.resource::<ItemDb>()),
                party: self.party_info(),
            }),
        })
    }

    /// One worn item as the manifest lists it. `None` if the item's
    /// definition has gone missing (a mod removed since the save was
    /// written), which drops the row rather than failing the whole sheet.
    fn manifest_equip_slot(
        &self,
        slot: EquipmentSlot,
        worn: EquippedItem,
    ) -> Option<ManifestEquipSlot> {
        let (_, base) = self.equipment_of(&worn.item)?;
        let mods = base
            .scaled_for_level(worn.level)
            .fused_for_tier(worn.fusion_tier);
        Some(ManifestEquipSlot {
            slot: slot.label().to_string(),
            item_name: self.item_name(&worn.item).to_string(),
            gear_level: worn.level,
            fusion_tier: worn.fusion_tier,
            atk: mods.atk,
            def: mods.def,
            decompiler: mods.decompiler,
        })
    }

    fn program_manifest(&self, entity: Entity) -> Option<ManifestView> {
        let creature = self.world.get::<Creature>(entity)?;
        let species = self.world.resource::<SpeciesDb>().get(&creature.species)?;
        let stats = self.world.get::<Stats>(entity)?;
        let exp = self.world.get::<Experience>(entity);
        let is_tamed = self.world.get::<Tamed>(entity).is_some();
        let custom = self.world.get::<CustomName>(entity).map(|c| c.0.clone());
        let decompiler_skill = self.player_decompiler_skill();
        Some(ManifestView {
            entity,
            name: match &custom {
                Some(name) => name.clone(),
                None => self.zone_tagged_name(entity, species.name.clone()),
            },
            glyph: species.glyph,
            color: species.color,
            level: exp.map(|e| e.level),
            xp: exp.map(|e| (e.xp, e.xp_to_next)),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk: stats.atk,
            def: stats.def,
            power: stats.power(),
            status_effect: self.status_label(entity),
            routines: self.routine_view(entity),
            subject: ManifestSubject::Program(ProgramManifest {
                species_name: custom
                    .is_some()
                    .then(|| self.zone_tagged_name(entity, species.name.clone())),
                is_hostile: self.world.get::<Hostile>(entity).is_some(),
                is_tamed,
                is_companion: self.world.resource::<Party>().0.contains(&entity),
                is_boss: species.is_boss,
                activity: is_tamed.then(|| self.program_activity(entity)),
                potential: self.world.get::<Potential>(entity).map(|p| ManifestPotential {
                    hp_roll: p.hp_roll,
                    atk_roll: p.atk_roll,
                    def_roll: p.def_roll,
                    growth_roll: p.growth_roll,
                    percent: p.quality_percent(),
                    label: p.quality_label().to_string(),
                }),
                fusions: self.fusion_count(entity),
                max_fusions: MAX_FUSIONS,
                habitats: species.habitats.clone(),
                moves: species.moves.clone(),
                work_resource: species.work_resource.clone(),
                taming_difficulty: species.taming_difficulty,
                decompile_chance: self.taming_catalyst().map(|(_, potency)| {
                    taming::capture_chance(
                        stats.hp_fraction(),
                        potency,
                        species.taming_difficulty,
                        decompiler_skill,
                    )
                }),
                growth_multiplier: species.growth_multiplier,
                base_speed: species.base_speed,
            }),
        })
    }
```

Add `use crate::tuning::MAX_FUSIONS;` to the file's existing `use crate::tuning::{...}` line. `crate::*` already brings in `Player`, `Stats`, `Needs`, `Position`, `Inventory`, `Experience`, `Glyph`, `Equipment`, `EquippedItem`, `EquipmentSlot`, `Perks`, `Perk`, `Decompiler`, `ItemDb`, `ZoneLevel`, `Party`, `Creature`, `CustomName`, `Tamed`, `Hostile`, `Potential`, `SpeciesDb` and `taming`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine inspection`
Expected: PASS, including the pre-existing `inspect_*` tests, which are untouched.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src/views.rs crates/engine/src/game/inspection.rs crates/engine/src/tests/inspection.rs
git commit -m "feat: one manifest view covering the player and any program"
```

---

### Task 2: GUI — pure layout geometry

No drawing yet. This task exists on its own because the geometry is the part that can be tested headlessly, and `popup_layout` is the precedent: settle what fits in whole rows before any of it becomes pixels.

**Files:**
- Create: `crates/gui/src/render/manifest_layout.rs`
- Modify: `crates/gui/src/render/mod.rs` (add `mod manifest_layout;`)
- Test: inline `#[cfg(test)] mod tests` in `manifest_layout.rs`

**Interfaces:**
- Consumes: `Rect` and `Metrics` from `crate::paint` / `crate::text`, `bars::bar_row_height`.
- Produces:
  - `pub(super) struct Section { pub(super) title: &'static str, pub(super) rows: Vec<SectionRow>, pub(super) full_width: bool }`
  - `pub(super) enum SectionRow { Stat(String, String), Note(String) }`
  - `pub(super) struct ManifestLayout { pub(super) frame: Rect, pub(super) header: Rect, pub(super) meters: Vec<Rect>, pub(super) sections: Vec<Rect>, pub(super) footer: Rect }`
  - `pub(super) fn manifest_layout(screen_w: f32, screen_h: f32, meters: usize, sections: &[Section], m: &Metrics) -> ManifestLayout`
  - `pub(super) const MAX_SECTION_ROWS: usize = 8;`
  - `pub(super) fn section_rows(rows: Vec<SectionRow>) -> Vec<SectionRow>` — truncates to `MAX_SECTION_ROWS` with a trailing `Note("+N more")`.

`ManifestLayout::sections` is index-aligned with the `sections` slice passed in: `sections[i]` is where `Section` `i` goes. Column-filled sections come first in the slice; `full_width: true` sections are placed as full-width bands below all the columned ones, in slice order.

- [ ] **Step 1: Write the failing tests**

Create `crates/gui/src/render/manifest_layout.rs` containing *only* this test module for now (the code above it comes in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The same window heights `popup_layout`'s tests sweep — the layout's
    /// row budget is font-dependent, so a bug that misses by one line can
    /// hide at one height and bite at the next.
    const WINDOW_HEIGHTS: [f32; 9] = [
        720.0, 768.0, 800.0, 900.0, 1000.0, 1050.0, 1080.0, 1200.0, 1440.0,
    ];
    const WINDOW_WIDTHS: [f32; 3] = [1280.0, 1600.0, 1920.0];

    fn section(title: &'static str, rows: usize, full_width: bool) -> Section {
        Section {
            title,
            rows: (0..rows)
                .map(|i| SectionRow::Stat(format!("label {i}"), format!("{i}")))
                .collect(),
            full_width,
        }
    }

    /// The fullest page a program can produce: four columned boxes at their
    /// real row counts, plus the Moves band at the cap. Row counts are the
    /// ones `sections_for` actually builds — 3 combat stats, 5 potential
    /// lines, 6 species facts, and `COMPANION_ROUTINE_SLOT_CAP` routines.
    fn worst_case_program() -> Vec<Section> {
        vec![
            section("COMBAT", 3, false),
            section("POTENTIAL", 5, false),
            section("SPECIES", 6, false),
            section("ROUTINES", 6, false),
            section("MOVES", MAX_SECTION_ROWS, true),
        ]
    }

    /// The fullest page the player can produce — six columned boxes, no
    /// band. Perks caps at `MAX_SECTION_ROWS` (there are 7 perk types), party
    /// at `MAX_PARTY_SIZE`.
    fn worst_case_player() -> Vec<Section> {
        vec![
            section("COMBAT", 3, false),
            section("PROGRESSION", 4, false),
            section("EQUIPMENT", 3, false),
            section("ROUTINES", 6, false),
            section("PERKS", MAX_SECTION_ROWS, false),
            section("PARTY", 5, false),
        ]
    }

    /// Meter counts: a program shows Integrity and Experience, the player
    /// adds Power and Fatigue.
    const PROGRAM_METERS: usize = 2;
    const PLAYER_METERS: usize = 4;

    fn overlaps(a: &Rect, b: &Rect) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }

    fn contains(outer: &Rect, inner: &Rect) -> bool {
        inner.x >= outer.x - 0.5
            && inner.y >= outer.y - 0.5
            && inner.x + inner.w <= outer.x + outer.w + 0.5
            && inner.y + inner.h <= outer.y + outer.h + 0.5
    }

    /// The gate this whole module exists for: the fullest page either subject
    /// can produce has to fit the tightest window, at every window size, with
    /// nothing overlapping and nothing escaping the frame.
    ///
    /// 720px is the binding case — the UI font is 19px there, and the header,
    /// four meters and the footer eat most of the box before a single stat
    /// row is drawn. If this fails, the fix is content, not the assertion:
    /// lower `MAX_SECTION_ROWS`, then merge two of the player's boxes.
    #[test]
    fn the_real_worst_case_pages_fit_the_tightest_window() {
        for window_h in WINDOW_HEIGHTS {
            for window_w in WINDOW_WIDTHS {
                let m = ui_metrics(window_h);
                for (who, sections, meters) in [
                    ("program", worst_case_program(), PROGRAM_METERS),
                    ("player", worst_case_player(), PLAYER_METERS),
                ] {
                    let l = manifest_layout(window_w, window_h, meters, &sections, &m);

                    let mut boxes = vec![l.header];
                    boxes.extend(l.meters.iter().copied());
                    boxes.extend(l.sections.iter().copied());
                    boxes.push(l.footer);

                    for (i, a) in boxes.iter().enumerate() {
                        assert!(
                            contains(&l.frame, a),
                            "the fullest {who} page at {window_w}x{window_h}: box {i} ({a:?}) \
                             escaped the frame ({:?})",
                            l.frame
                        );
                        for (j, b) in boxes.iter().enumerate().skip(i + 1) {
                            assert!(
                                !overlaps(a, b),
                                "the fullest {who} page at {window_w}x{window_h}: boxes {i} \
                                 ({a:?}) and {j} ({b:?}) overlap"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_frame_always_fits_the_window() {
        for window_h in WINDOW_HEIGHTS {
            for window_w in WINDOW_WIDTHS {
                let m = ui_metrics(window_h);
                let sections = worst_case_player();
                let l = manifest_layout(window_w, window_h, PLAYER_METERS, &sections, &m);
                let window = Rect::new(0.0, 0.0, window_w, window_h);
                assert!(
                    contains(&window, &l.frame),
                    "at {window_w}x{window_h} the frame {:?} runs off the window",
                    l.frame
                );
            }
        }
    }

    /// A subject missing a section (a legacy program with no Potential roll)
    /// must not leave a hole — the boxes after it move up into the space.
    #[test]
    fn a_missing_section_closes_the_gap_rather_than_leaving_a_hole() {
        let m = ui_metrics(1080.0);
        let full = vec![
            section("COMBAT", 3, false),
            section("POTENTIAL", 4, false),
            section("ROUTINES", 3, false),
            section("SPECIES", 6, false),
        ];
        let without_potential = vec![
            section("COMBAT", 3, false),
            section("ROUTINES", 3, false),
            section("SPECIES", 6, false),
        ];
        let a = manifest_layout(1600.0, 1080.0, 2, &full, &m);
        let b = manifest_layout(1600.0, 1080.0, 2, &without_potential, &m);

        assert_eq!(a.sections.len(), 4);
        assert_eq!(b.sections.len(), 3);
        assert_eq!(
            b.sections[1].y, a.sections[1].y,
            "dropping a section must not push the survivors down"
        );
        let a_bottom = a.sections.iter().map(|r| r.y + r.h).fold(0.0_f32, f32::max);
        let b_bottom = b.sections.iter().map(|r| r.y + r.h).fold(0.0_f32, f32::max);
        assert!(
            b_bottom <= a_bottom + 0.5,
            "three sections must not occupy more vertical space than four"
        );
    }

    /// Two columns, so the second section sits beside the first rather than
    /// under it.
    #[test]
    fn columned_sections_fill_left_then_right() {
        let m = ui_metrics(1080.0);
        let sections = vec![section("A", 3, false), section("B", 3, false)];
        let l = manifest_layout(1600.0, 1080.0, 2, &sections, &m);
        assert_eq!(l.sections[0].y, l.sections[1].y, "equal boxes share a row");
        assert!(
            l.sections[1].x > l.sections[0].x,
            "the second box goes to the right column"
        );
    }

    /// A full-width band spans both columns and sits below every columned box.
    #[test]
    fn a_full_width_section_spans_both_columns_below_the_grid() {
        let m = ui_metrics(1080.0);
        let sections = vec![
            section("A", 3, false),
            section("B", 3, false),
            section("MOVES", 2, true),
        ];
        let l = manifest_layout(1600.0, 1080.0, 2, &sections, &m);
        let band = l.sections[2];
        assert!(
            band.y >= l.sections[0].y + l.sections[0].h,
            "the band sits below the grid"
        );
        assert!(
            band.w > l.sections[0].w * 1.5,
            "the band spans both columns, not one"
        );
    }

    /// A section longer than the cap is trimmed with a counted note, so a
    /// modded species with twenty moves can't blow the layout out.
    #[test]
    fn section_rows_trims_past_the_cap_and_says_how_many_it_hid() {
        let rows: Vec<SectionRow> = (0..MAX_SECTION_ROWS + 5)
            .map(|i| SectionRow::Note(format!("row {i}")))
            .collect();
        let trimmed = section_rows(rows);
        assert_eq!(trimmed.len(), MAX_SECTION_ROWS);
        let SectionRow::Note(last) = &trimmed[MAX_SECTION_ROWS - 1] else {
            panic!("the trailing row is a note");
        };
        assert_eq!(last, "+6 more");
    }

    #[test]
    fn section_rows_leaves_a_short_list_alone() {
        let rows: Vec<SectionRow> = (0..3).map(|i| SectionRow::Note(format!("row {i}"))).collect();
        assert_eq!(section_rows(rows).len(), 3);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Add `mod manifest_layout;` to the module list in `crates/gui/src/render/mod.rs` (alphabetically, after `mod inventory;`).

Run: `cargo test -p feral-processes-gui manifest_layout`
Expected: FAIL to compile — `cannot find function 'manifest_layout' in this scope`.

- [ ] **Step 3: Implement the layout**

Prepend to `crates/gui/src/render/manifest_layout.rs`, above the test module:

```rust
//! Where every box on the manifest screen goes.
//!
//! Kept free of `Painter` for the same reason `popup_layout` is: what fits is
//! settled in whole rows before any of it becomes pixels, and that arithmetic
//! is then testable at every window size without a window.

use super::bars::bar_row_height;
use crate::paint::Rect;
use crate::text::Metrics;

/// One titled box. Built by the draw function and handed here, so a section
/// with no data is simply absent from the slice and the boxes after it move
/// up — rather than the layout having to know which subject it is drawing.
pub(super) struct Section {
    pub(super) title: &'static str,
    pub(super) rows: Vec<SectionRow>,
    /// Spans both columns, below the grid. The Moves and Party bands.
    pub(super) full_width: bool,
}

pub(super) enum SectionRow {
    /// A label on the left, its value right-aligned against the box's inner
    /// edge.
    Stat(String, String),
    /// One run of free text across the box.
    Note(String),
}

/// The most rows any one box draws. A section is bounded so the worst-case
/// page height is bounded — otherwise a modded species with twenty moves
/// would silently push the footer off the bottom.
///
/// 6 is what the tightest supported window (720px, where the UI font is 19px)
/// has room for once the header, four meters and the footer are paid for —
/// see `the_real_worst_case_pages_fit_the_tightest_window`. It is also the
/// routine-slot cap, so a full kit is never trimmed.
pub(super) const MAX_SECTION_ROWS: usize = 6;

/// Trims `rows` to `MAX_SECTION_ROWS`, spending the last line on a count of
/// what was dropped. A silent truncation would read as "that's all of them".
pub(super) fn section_rows(mut rows: Vec<SectionRow>) -> Vec<SectionRow> {
    if rows.len() <= MAX_SECTION_ROWS {
        return rows;
    }
    let hidden = rows.len() - (MAX_SECTION_ROWS - 1);
    rows.truncate(MAX_SECTION_ROWS - 1);
    rows.push(SectionRow::Note(format!("+{hidden} more")));
    rows
}

/// The manifest's boxes. `sections` is index-aligned with the slice passed to
/// `manifest_layout`.
pub(super) struct ManifestLayout {
    pub(super) frame: Rect,
    pub(super) header: Rect,
    pub(super) meters: Vec<Rect>,
    pub(super) sections: Vec<Rect>,
    pub(super) footer: Rect,
}

/// How much of the window the sheet claims. Not the full window: the status
/// banner (see `draw_status_banner`) lives in the bottom strip, and a refusal
/// drawn over the footer would be unreadable.
const FRAME_W_PCT: f32 = 0.92;
const FRAME_H_PCT: f32 = 0.90;

/// Header rows: a name line and a subtitle line. The glyph is drawn to their
/// left at three times the title size and spans both, so it costs no rows of
/// its own — which the 720px budget needs it not to.
const HEADER_ROWS: f32 = 2.0;

pub(super) fn manifest_layout(
    screen_w: f32,
    screen_h: f32,
    meters: usize,
    sections: &[Section],
    m: &Metrics,
) -> ManifestLayout {
    let w = screen_w * FRAME_W_PCT;
    let h = screen_h * FRAME_H_PCT;
    let frame = Rect::new((screen_w - w) / 2.0, (screen_h - h) / 2.0, w, h);

    let inner_x = frame.x + m.pad;
    let inner_w = frame.w - m.pad * 2.0;
    let mut y = frame.y + m.pad;

    let header = Rect::new(inner_x, y, inner_w, m.line_height * HEADER_ROWS);
    y += header.h + m.gap;

    let meter_rects: Vec<Rect> = (0..meters)
        .map(|i| {
            Rect::new(
                inner_x,
                y + i as f32 * bar_row_height(m),
                inner_w,
                bar_row_height(m),
            )
        })
        .collect();
    y += meters as f32 * bar_row_height(m) + m.gap;

    let footer = Rect::new(
        inner_x,
        frame.y + frame.h - m.pad - m.line_height,
        inner_w,
        m.line_height,
    );

    let col_gap = m.pad;
    let col_w = (inner_w - col_gap) / 2.0;
    // Running bottom edge of each column. A box lands under whichever side is
    // currently shorter, so an uneven set of boxes still fills evenly instead
    // of leaving one column short.
    let mut col_y = [y, y];
    // `None` for a full-width band, which can't be placed until every
    // columned box is down and the grid's true bottom is known.
    let mut placed: Vec<Option<Rect>> = Vec::with_capacity(sections.len());

    for section in sections {
        if section.full_width {
            placed.push(None);
            continue;
        }
        let side = usize::from(col_y[0] > col_y[1]);
        let box_h = section_height(section, m);
        placed.push(Some(Rect::new(
            inner_x + side as f32 * (col_w + col_gap),
            col_y[side],
            col_w,
            box_h,
        )));
        col_y[side] += box_h + m.gap;
    }

    let mut band_y = col_y[0].max(col_y[1]);
    for (slot, section) in placed.iter_mut().zip(sections) {
        if slot.is_some() {
            continue;
        }
        let box_h = section_height(section, m);
        *slot = Some(Rect::new(inner_x, band_y, inner_w, box_h));
        band_y += box_h + m.gap;
    }

    ManifestLayout {
        frame,
        header,
        meters: meter_rects,
        sections: placed.into_iter().flatten().collect(),
        footer,
    }
}

/// A stat row's height inside a box — tighter than `m.line_height`, which is
/// tuned for prose the eye reads left to right rather than a column of
/// label/value pairs it scans down.
///
/// Derived from `font_size` rather than from `m.small()`: `small()` is
/// `font_size - 4`, so it closes on the body size as the font grows, and a
/// box would be *relatively taller* at 1440px than at 720px. That inverts
/// which window is the tight one, which is exactly the kind of thing the
/// height sweep exists to catch.
pub(super) fn section_row_h(m: &Metrics) -> f32 {
    m.font_size as f32
}

/// A box's height: its title line, one `section_row_h` per row, and a gap
/// above and below the rows.
fn section_height(section: &Section, m: &Metrics) -> f32 {
    m.line_height + section_row_h(m) * section.rows.len() as f32 + m.gap * 2.0
}
```

Add `use crate::text::ui_metrics;` and `use crate::paint::Rect;` to the test module's `use super::*;` reach — `ui_metrics` is not imported by the main module, so the test module needs its own `use crate::text::ui_metrics;` line.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-gui manifest_layout`
Expected: PASS — all seven tests.

If `no_two_boxes_ever_overlap_and_every_box_stays_inside_the_frame` fails at 720px because the worst case genuinely doesn't fit, lower `MAX_SECTION_ROWS` until it does and leave a comment saying 720px is what pins it. Do not weaken the assertion.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/gui/src/render/manifest_layout.rs crates/gui/src/render/mod.rs
git commit -m "feat: headless geometry for the manifest screen"
```

---

### Task 3: GUI — draw the manifest, retire the inspect popup

Swaps the screen. `Mode::InspectDetail` is renamed here (it is the same rename that makes the swap coherent), and the old popup, `InspectView` and `Game::inspect` all go.

**Files:**
- Create: `crates/gui/src/render/manifest.rs`
- Delete: `crates/gui/src/render/inspection.rs`
- Modify: `crates/gui/src/render/mod.rs`
- Modify: `crates/engine/src/views.rs` (delete `InspectView`)
- Modify: `crates/engine/src/game/inspection.rs` (delete `Game::inspect`)
- Modify: `crates/engine/src/tests/inspection.rs` (delete the two `inspect_*` tests — Task 1's manifest tests already cover both behaviours)
- Modify: `crates/app-core/src/lib.rs`, `app/inspection.rs`, `app/input.rs`, `app/lifecycle.rs` (rename only)

**Interfaces:**
- Consumes: `Game::manifest`, `ManifestView`, `ManifestSubject`, `PlayerManifest`, `ProgramManifest`, `ManifestEquipSlot`, `ManifestPotential` (Task 1); `Section`, `SectionRow`, `section_rows`, `manifest_layout`, `ManifestLayout`, `MAX_SECTION_ROWS` (Task 2).
- Produces:
  - `pub(super) fn draw_manifest(game: &mut Game, entity: Option<Entity>, cyclable: bool, painter: &Painter, m: &Metrics)`
  - `pub(super) fn glyph_color(c: GlyphColor) -> Color` — moved up from `base.rs`, unchanged
  - `pub fn App::manifest_subjects(&mut self) -> Vec<Entity>` — the player followed by every owned program, in `owned_pets` order
  - `Mode::Manifest`, `App::pending_manifest`

- [ ] **Step 1: Rename the mode and the field**

Pure mechanical rename, no behaviour change:

```bash
# Mode::InspectDetail -> Mode::Manifest, pending_inspect -> pending_manifest
grep -rl 'InspectDetail\|pending_inspect' crates/ --include=*.rs \
  | xargs sed -i 's/Mode::InspectDetail/Mode::Manifest/g; s/pending_inspect/pending_manifest/g'
```

Then rename the two handlers in `crates/app-core/src/app/inspection.rs`: `handle_inspect_detail_key` → `handle_manifest_key`, and update its call site in `crates/app-core/src/app/input.rs`. Update the doc comment on `App::pending_manifest` in `crates/app-core/src/lib.rs` to say it is the subject the manifest screen is showing. Rename the `InspectDetail` variant's position in `Mode::is_battle`'s exhaustive match (the rename above already did it) and in `Mode`'s declaration list; add a doc comment on the variant:

```rust
    /// The manifest — a full read-only stat sheet for the player, a program
    /// you own, or a wild one. `App::pending_manifest` is the subject.
    Manifest,
```

- [ ] **Step 2: Verify the rename compiles and the suite still passes**

Run: `cargo test --workspace`
Expected: PASS, same count as before (599). The screen still draws the old popup at this point.

- [ ] **Step 3: Move `glyph_color` up so the manifest can share it**

`glyph_color` is currently a private `fn` at the top of
`crates/gui/src/render/base.rs`. The manifest needs the same
`GlyphColor` → `Color` mapping for its header portrait. Move the function
verbatim into `crates/gui/src/render/mod.rs` (beside `desaturate`) and mark it
`pub(super)`. Do not copy it — one mapping, one home. `base.rs` picks it up
through its existing `use super::*;` with no other change.

Run: `cargo test -p feral-processes-gui`
Expected: PASS, unchanged.

- [ ] **Step 4: Write the manifest renderer**

Create `crates/gui/src/render/manifest.rs`:

```rust
//! The manifest — one read-only stat sheet for the player, a program you own,
//! or a wild one.

use super::bars::*;
use super::manifest_layout::*;
use super::popup::*;
use super::*;
use feral_processes_engine::species::MoveDef;
use feral_processes_engine::{
    ManifestEquipSlot, ManifestSubject, ManifestView, PlayerManifest, ProgramManifest,
};

/// How big the header glyph is drawn, relative to the UI title size — enough
/// to read as a portrait rather than another line of text, and sized to span
/// the header's two text lines without spilling into the meters below (the
/// header is `HEADER_ROWS` × `line_height` tall, which is a hair over twice
/// `m.title()`).
const HEADER_GLYPH_SCALE: u16 = 2;

pub(super) fn draw_manifest(
    game: &mut Game,
    entity: Option<Entity>,
    cyclable: bool,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(view) = entity.and_then(|e| game.manifest(e)) else {
        draw_popup(
            "Manifest",
            PopupSize::Small,
            &[text_row("That program is gone. Esc to go back.")],
            painter,
            m,
        );
        return;
    };

    let meters = meter_rows(&view);
    let sections = sections_for(game, &view);
    let l = manifest_layout(
        painter.screen_w(),
        painter.screen_h(),
        meters.len(),
        &sections,
        m,
    );

    painter.rect(l.frame.x, l.frame.y, l.frame.w, l.frame.h, PANEL_BG);
    painter.rect_lines(l.frame.x, l.frame.y, l.frame.w, l.frame.h, 2.0, BORDER);

    draw_header(&view, l.header, painter, m);
    for (rect, meter) in l.meters.iter().zip(&meters) {
        let g = BarGeometry {
            x: rect.x,
            y: rect.y + m.label() as f32,
            w: rect.w,
        };
        draw_bar(
            g,
            &format!("{}  {}", meter.label, meter.readout),
            meter.value,
            meter.max,
            BarStyle::plain(meter.color),
            painter,
            m,
        );
    }
    for (rect, section) in l.sections.iter().zip(&sections) {
        draw_section(section, *rect, painter, m);
    }

    let footer = if cyclable {
        "←/→ other programs      Esc back"
    } else {
        "Esc back"
    };
    painter.ui(footer, l.footer.x, l.footer.y + m.font_size as f32, m.small(), TEXT_DIM);
}

/// One meter on the sheet.
struct Meter {
    label: &'static str,
    readout: String,
    value: f32,
    max: f32,
    color: Color,
}

fn meter_rows(view: &ManifestView) -> Vec<Meter> {
    let mut meters = vec![Meter {
        label: "INTEGRITY",
        readout: format!("{}/{}", view.hp, view.max_hp),
        value: view.hp as f32,
        max: view.max_hp.max(1) as f32,
        color: GREEN,
    }];
    if let Some((xp, to_next)) = view.xp {
        meters.push(Meter {
            label: "EXPERIENCE",
            readout: format!("{xp}/{to_next}"),
            value: xp as f32,
            max: to_next.max(1) as f32,
            color: CYAN,
        });
    }
    // Needs are player-only — no creature in the sim carries `Needs`.
    if let ManifestSubject::Player(p) = &view.subject {
        meters.push(Meter {
            label: "POWER",
            readout: format!("{:.0}/100", p.hunger),
            value: p.hunger,
            max: 100.0,
            color: YELLOW,
        });
        meters.push(Meter {
            label: "FATIGUE",
            readout: format!("{:.0}/100", p.fatigue),
            value: p.fatigue,
            max: 100.0,
            color: BLUE,
        });
    }
    meters
}

fn draw_header(view: &ManifestView, rect: Rect, painter: &Painter, m: &Metrics) {
    let glyph_size = m.title() * HEADER_GLYPH_SCALE;
    painter.map(
        view.glyph.to_string(),
        rect.x,
        rect.y + glyph_size as f32 * 0.85,
        glyph_size,
        glyph_color(view.color),
    );
    let text_x = rect.x + painter.measure_map(view.glyph.to_string(), glyph_size).width + m.pad;

    let boss = matches!(&view.subject, ManifestSubject::Program(p) if p.is_boss);
    let species = match &view.subject {
        ManifestSubject::Program(p) => p.species_name.clone(),
        ManifestSubject::Player(_) => None,
    };
    let title = match species {
        Some(s) => format!("{}  ({s})", view.name),
        None => view.name.clone(),
    };
    painter.ui_bold(
        format!("{title}{}", if boss { "  [BOSS]" } else { "" }),
        text_x,
        rect.y + m.title() as f32,
        m.title(),
        if boss { RED } else { WHITE },
    );

    let mut tags: Vec<String> = Vec::new();
    if let Some(level) = view.level {
        tags.push(format!("Lv {level}"));
    }
    match &view.subject {
        ManifestSubject::Program(p) => {
            if let Some(q) = &p.potential {
                tags.push(format!("{} ({}%)", q.label, q.percent));
            }
            if p.fusions > 0 {
                tags.push(format!("fused {}/{}", p.fusions, p.max_fusions));
            }
            if p.is_companion {
                tags.push("in party".to_string());
            } else if let Some(activity) = &p.activity {
                tags.push(activity.clone());
            } else if p.is_hostile {
                tags.push("rogue".to_string());
            }
        }
        ManifestSubject::Player(p) => {
            tags.push(format!("Zone {}", p.zone));
            tags.push(format!("Pets {}/{}", p.pet_count, p.pet_capacity));
        }
    }
    if let Some(status) = &view.status_effect {
        tags.push(status.clone());
    }
    painter.ui(
        tags.join("   "),
        text_x,
        rect.y + m.title() as f32 + m.line_height,
        m.font_size,
        TEXT_DIM,
    );
}

fn draw_section(section: &Section, rect: Rect, painter: &Painter, m: &Metrics) {
    painter.rect_lines(rect.x, rect.y, rect.w, rect.h, 1.0, BORDER);
    painter.ui(
        section.title,
        rect.x + m.inset,
        rect.y + m.line_height,
        m.small(),
        CYAN,
    );
    let mut cy = rect.y + m.line_height + m.gap;
    for row in &section.rows {
        cy += section_row_h(m);
        match row {
            SectionRow::Stat(label, value) => {
                painter.ui(label, rect.x + m.inset, cy, m.font_size, TEXT_DIM);
                let dims = painter.measure_ui(value, m.font_size);
                painter.ui(
                    value,
                    rect.x + rect.w - m.inset - dims.width,
                    cy,
                    m.font_size,
                    TEXT,
                );
            }
            SectionRow::Note(text) => {
                painter.ui(text, rect.x + m.inset, cy, m.font_size, TEXT);
            }
        }
    }
}

fn stat(label: impl Into<String>, value: impl Into<String>) -> SectionRow {
    SectionRow::Stat(label.into(), value.into())
}

fn sections_for(game: &Game, view: &ManifestView) -> Vec<Section> {
    let mut sections = vec![Section {
        title: "COMBAT",
        rows: section_rows(vec![
            stat("Attack", view.atk.to_string()),
            stat("Defense", view.def.to_string()),
            stat("Power", view.power.to_string()),
        ]),
        full_width: false,
    }];
    match &view.subject {
        ManifestSubject::Player(p) => player_sections(&mut sections, p),
        ManifestSubject::Program(p) => program_sections(&mut sections, game, p),
    }
    if !view.routines.is_empty() {
        sections.push(Section {
            title: "ROUTINES",
            rows: section_rows(
                view.routines
                    .iter()
                    .map(|r| stat(format!("{}", r.index + 1), r.name.clone()))
                    .collect(),
            ),
            full_width: false,
        });
    }
    sections
}

/// Every player box is columned — no full-width bands. Six boxes across two
/// columns is what 720px has room for; two of them promoted to bands would
/// cost about 180px the budget doesn't have (a band is as tall as a columned
/// box but consumes a whole row of the grid).
///
/// XP is deliberately not a row here: the Experience meter above already
/// reads `xp/to_next`. Position stays, because unlike the sidebar's copy this
/// screen is the one that claims to hold every stat.
fn player_sections(sections: &mut Vec<Section>, p: &PlayerManifest) {
    sections.push(Section {
        title: "PROGRESSION",
        rows: section_rows(vec![
            stat("Decompiler", p.decompiler.to_string()),
            stat("Perk points", p.perk_points.to_string()),
            stat("Cargo carried", p.cargo_used.to_string()),
            stat("Position", format!("{}, {}", p.position.0, p.position.1)),
        ]),
        full_width: false,
    });

    if !p.equipment.is_empty() {
        sections.push(Section {
            title: "EQUIPMENT",
            rows: section_rows(p.equipment.iter().map(equip_row).collect()),
            full_width: false,
        });
    }
    if !p.perks.is_empty() {
        sections.push(Section {
            title: "PERKS",
            rows: section_rows(
                p.perks
                    .iter()
                    .map(|(name, level)| stat(name.clone(), format!("Lv {level}")))
                    .collect(),
            ),
            full_width: false,
        });
    }
    if !p.party.is_empty() {
        sections.push(Section {
            title: "PARTY",
            rows: section_rows(
                p.party
                    .iter()
                    .map(|c| {
                        stat(
                            c.name.clone(),
                            format!("HP {}/{}  ATK {}  DEF {}", c.hp, c.max_hp, c.atk, c.def),
                        )
                    })
                    .collect(),
            ),
            full_width: false,
        });
    }
}

fn equip_row(slot: &ManifestEquipSlot) -> SectionRow {
    let mut bonus: Vec<String> = Vec::new();
    if slot.atk != 0 {
        bonus.push(format!("+{} ATK", slot.atk));
    }
    if slot.def != 0 {
        bonus.push(format!("+{} DEF", slot.def));
    }
    if slot.decompiler != 0 {
        bonus.push(format!("+{} DECOMP", slot.decompiler));
    }
    if slot.fusion_tier > 0 {
        bonus.push(format!("T{}", slot.fusion_tier));
    }
    SectionRow::Stat(
        format!("{}: {}", slot.slot, slot.item_name),
        bonus.join(" "),
    )
}

fn program_sections(sections: &mut Vec<Section>, game: &Game, p: &ProgramManifest) {
    if let Some(q) = &p.potential {
        sections.push(Section {
            title: "POTENTIAL",
            rows: section_rows(vec![
                stat("HP roll", roll_readout(q.hp_roll)),
                stat("Attack roll", roll_readout(q.atk_roll)),
                stat("Defense roll", roll_readout(q.def_roll)),
                stat("Growth roll", roll_readout(q.growth_roll)),
                stat("Overall", format!("{} ({}%)", q.label, q.percent)),
            ]),
            full_width: false,
        });
    }

    let mut species = vec![stat(
        "Habitats",
        if p.habitats.is_empty() {
            "unknown".to_string()
        } else {
            p.habitats
                .iter()
                .map(|b| format!("{b:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        },
    )];
    if let Some(res) = &p.work_resource {
        species.push(stat("Work aptitude", game.item_name(res).to_string()));
    }
    species.push(stat(
        "Decompile difficulty",
        format!("{:.0}%", p.taming_difficulty * 100.0),
    ));
    species.push(stat(
        "Decompile chance now",
        match p.decompile_chance {
            Some(c) => format!("{:.0}%", c * 100.0),
            // Which item is a catalyst is item data, not something a renderer
            // gets to name.
            None => "needs a taming catalyst".to_string(),
        },
    ));
    species.push(stat("Growth", format!("{:.2}x", p.growth_multiplier)));
    species.push(stat("Speed", p.base_speed.to_string()));
    sections.push(Section {
        title: "SPECIES",
        rows: section_rows(species),
        full_width: false,
    });

    if !p.moves.is_empty() {
        sections.push(Section {
            title: "MOVES",
            rows: section_rows(p.moves.iter().map(move_row).collect()),
            full_width: true,
        });
    }
}

fn move_row(mv: &MoveDef) -> SectionRow {
    let mut tags = vec![format!("pow {}", mv.power)];
    if mv.ranged {
        tags.push("ranged".to_string());
    }
    if let Some(effect) = &mv.effect {
        tags.push(format!(
            "{:?} {:.0}% for {}",
            effect.kind,
            effect.chance * 100.0,
            effect.duration
        ));
    }
    SectionRow::Stat(mv.name.clone(), tags.join(", "))
}

/// A potential roll as a number plus a coarse glance-readable tier. 1.0 is
/// neutral; the roll range is `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`.
fn roll_readout(roll: f32) -> String {
    let tier = if roll >= 1.15 {
        "+++"
    } else if roll >= 1.05 {
        "++"
    } else if roll > 0.95 {
        "="
    } else if roll > 0.85 {
        "-"
    } else {
        "--"
    };
    format!("{roll:.2}  {tier}")
}
```

- [ ] **Step 5: Add `App::manifest_subjects`**

The footer only advertises `←`/`→` when there is somewhere to page to, so the
renderer needs the subject list now. Add to `crates/app-core/src/app/inspection.rs`:

```rust
    /// You, then every program you own — everyone the manifest can page
    /// through with ←/→. A wild program reached via `i` is deliberately not
    /// in here: it is not yours to page to, and paging away from it would be
    /// a one-way trip.
    pub fn manifest_subjects(&mut self) -> Vec<Entity> {
        let Some(game) = &mut self.game else {
            return Vec::new();
        };
        let mut subjects = vec![game.player_entity()];
        subjects.extend(game.owned_pets().into_iter().map(|p| p.entity));
        subjects
    }
```

- [ ] **Step 6: Wire it up and delete the old screen**

In `crates/gui/src/render/mod.rs`:
- `mod inspection;` → `mod manifest;` (keep `mod manifest_layout;` from Task 2)
- `use inspection::draw_inspect_detail;` → `use manifest::draw_manifest;`
- `draw_mode_overlay` takes `&mut App` but then borrows `app.game` mutably, so
  the subject list has to be computed before that borrow. Add it beside the
  existing `selected`:

```rust
fn draw_mode_overlay(app: &mut App, painter: &Painter, m: &Metrics) {
    let selected = app.menu_selected;
    let pending_manifest = app.pending_manifest;
    // Computed before `app.game` is borrowed below — `manifest_subjects`
    // needs `&mut self` for `owned_pets`, which that borrow would block.
    let manifest_subjects = matches!(app.mode, Mode::Manifest)
        .then(|| app.manifest_subjects())
        .unwrap_or_default();
    let Some(game) = &mut app.game else { return };
```

- Replace the `Mode::InspectDetail => draw_inspect_detail(...)` arm (now
  `Mode::Manifest` after Step 1) with:

```rust
        Mode::Manifest => {
            // Only advertise ←/→ when they actually do something. A wild
            // program reached via `i` is not in the owned list, so cycling
            // from it is a no-op and the footer must not claim otherwise.
            let cyclable = manifest_subjects.len() > 1
                && pending_manifest.is_some_and(|e| manifest_subjects.contains(&e));
            draw_manifest(game, pending_manifest, cyclable, painter, m)
        }
```

Then:

```bash
git rm crates/gui/src/render/inspection.rs
```

In `crates/engine/src/game/inspection.rs` delete `pub fn inspect`. In `crates/engine/src/views.rs` delete `pub struct InspectView`. In `crates/engine/src/tests/inspection.rs` delete `inspect_reports_species_detail_without_starting_a_battle` and `inspect_returns_none_for_non_creature_entities` — Task 1's `manifest_of_a_wild_program_has_no_experience_and_no_activity` and `manifest_returns_none_for_anything_that_is_neither_the_player_nor_a_creature` cover the same ground.

Fix whatever `use` lines clippy then reports as unused in `views.rs` and `mod.rs`.

- [ ] **Step 7: Run the suite**

Run: `cargo test --workspace`
Expected: PASS. The count drops by 2 (the deleted `inspect_*` tests) and rises by 6 (Task 1) plus 7 (Task 2).

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace
git add -A
git commit -m "feat: draw the manifest, retire the inspect popup"
```

---

### Task 4: app-core — the `d` key, the subject picker, and ←/→ cycling

**Files:**
- Modify: `crates/app-core/src/lib.rs` (add `Mode::ManifestPick`)
- Modify: `crates/app-core/src/app/playing.rs` (bind `d`)
- Modify: `crates/app-core/src/app/input.rs` (dispatch)
- Modify: `crates/app-core/src/app/inspection.rs` (picker + cycling)
- Modify: `crates/gui/src/render/manifest.rs` (add `draw_manifest_pick`), `crates/gui/src/render/mod.rs`
- Test: `crates/app-core/src/tests/menus.rs`

**Interfaces:**
- Consumes: `Mode::Manifest`, `App::pending_manifest`, `App::manifest_subjects`, `draw_manifest`, `glyph_color` (Task 3); `Game::manifest` (Task 1); `menu_shortcut`, `App::selected_index`, `popup::creature_row`.
- Produces: `Mode::ManifestPick`, `pub(super) fn draw_manifest_pick(game: &mut Game, subjects: &[Entity], selected: usize, painter: &Painter, m: &Metrics)`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/app-core/src/tests/menus.rs`:

```rust
#[test]
fn d_opens_the_manifest_picker_and_esc_backs_out() {
    let mut app = test_app(70);
    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::ManifestPick);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn the_picker_always_offers_you_as_its_first_row() {
    let mut app = test_app(71);
    let player = app.game.as_ref().unwrap().player_entity();
    assert_eq!(
        app.manifest_subjects().first().copied(),
        Some(player),
        "you are always inspectable, even owning nothing"
    );

    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::Manifest);
    assert_eq!(app.pending_manifest, Some(player));
}

#[test]
fn the_picker_lists_every_owned_program_after_you() {
    let mut app = app_owning_distant_programs(72, 2);
    let subjects = app.manifest_subjects();
    assert_eq!(subjects.len(), 3, "you plus two programs");

    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.mode, Mode::Manifest);
    assert_eq!(app.pending_manifest, Some(subjects[1]));
}

#[test]
fn left_and_right_cycle_the_owned_subjects_and_wrap_at_both_ends() {
    let mut app = app_owning_distant_programs(73, 2);
    let subjects = app.manifest_subjects();
    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.pending_manifest, Some(subjects[0]));

    app.handle_key(GameKey::Right);
    assert_eq!(app.pending_manifest, Some(subjects[1]));
    app.handle_key(GameKey::Right);
    assert_eq!(app.pending_manifest, Some(subjects[2]));
    app.handle_key(GameKey::Right);
    assert_eq!(
        app.pending_manifest,
        Some(subjects[0]),
        "past the last subject wraps to the first"
    );
    app.handle_key(GameKey::Left);
    assert_eq!(
        app.pending_manifest,
        Some(subjects[2]),
        "before the first subject wraps to the last"
    );
    assert_eq!(app.mode, Mode::Manifest, "cycling never leaves the screen");
}

/// A wild program near the player, and the cardinal direction it lies in.
/// Found by scanning `view_entities` rather than guessing a direction, so the
/// test doesn't depend on where a seed happened to put one.
fn a_wild_program_and_its_direction(app: &mut App) -> (Entity, GameKey) {
    let game = app.game.as_mut().unwrap();
    let player = game.player_status().position;
    let wild = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .find(|e| e.is_hostile)
        .expect("a fresh map has a wild program within scan radius");
    let (dx, dy) = (wild.pos.0 - player.0, wild.pos.1 - player.1);
    let key = if dx.abs() >= dy.abs() {
        if dx > 0 { GameKey::Right } else { GameKey::Left }
    } else if dy > 0 {
        GameKey::Down
    } else {
        GameKey::Up
    };
    (wild.entity, key)
}

#[test]
fn cycling_does_nothing_when_the_subject_is_a_program_you_do_not_own() {
    let mut app = test_app(74);
    // A wild program reached through `i` + direction is not in the owned
    // list, so there is nothing to cycle to.
    let (wild, _) = a_wild_program_and_its_direction(&mut app);

    app.pending_manifest = Some(wild);
    app.mode = Mode::Manifest;
    app.handle_key(GameKey::Right);
    assert_eq!(app.pending_manifest, Some(wild));
    assert_eq!(app.mode, Mode::Manifest);
}

#[test]
fn esc_leaves_the_manifest_for_the_map_rather_than_the_picker() {
    let mut app = app_owning_distant_programs(75, 1);
    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::Manifest);
    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::Playing,
        "the picker is a way in, not a place to be"
    );
    assert_eq!(app.pending_manifest, None);
}

#[test]
fn inspecting_a_direction_still_lands_on_the_manifest() {
    let mut app = test_app(76);
    let (_, direction) = a_wild_program_and_its_direction(&mut app);

    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::InspectDirection);
    app.handle_key(direction);
    assert_eq!(app.mode, Mode::Manifest);
    assert!(
        app.pending_manifest.is_some(),
        "the inspected program is the manifest's subject"
    );
}
```

`crates/app-core/src/tests/menus.rs` already has `use super::support::*;` and `use crate::*;`, which cover `test_app`, `app_owning_distant_programs`, `GameKey`, `Mode` and `MENU_SCAN_RADIUS`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-app-core menus`
Expected: FAIL to compile — `no variant named 'ManifestPick'`.

- [ ] **Step 3: Add the mode, the key and the handlers**

In `crates/app-core/src/lib.rs`, add next to `Mode::Manifest`:

```rust
    /// Picking whose manifest to read — you, or any program you own.
    /// Reached with `d` from `Mode::Playing`.
    ManifestPick,
```

Add `Mode::ManifestPick` to `Mode::is_battle`'s `false` arm (the exhaustive match makes this a compile error until you do, which is the point).

In `crates/app-core/src/app/playing.rs`, add beside the other mode keys:

```rust
            GameKey::Char('d') => {
                self.mode = Mode::ManifestPick;
                return;
            }
```

In `crates/app-core/src/app/input.rs`, add beside `Mode::Manifest`:

```rust
            Mode::ManifestPick => self.handle_manifest_pick_key(key),
```

In `crates/app-core/src/app/inspection.rs`, replace the body of `handle_manifest_key` (Task 3 renamed it; it currently still closes on any key) and add the picker handler beside `manifest_subjects`:

```rust
    pub(crate) fn handle_manifest_pick_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let subjects = self.manifest_subjects();
        if let Some(idx) = self.selected_index(key, subjects.len()) {
            self.pending_manifest = Some(subjects[idx]);
            self.status_line = None;
            self.mode = Mode::Manifest;
        }
    }

    pub(crate) fn handle_manifest_key(&mut self, key: GameKey) {
        let step = match key {
            GameKey::Left => -1,
            GameKey::Right => 1,
            GameKey::Esc => {
                self.pending_manifest = None;
                self.mode = Mode::Playing;
                return;
            }
            _ => return,
        };
        let subjects = self.manifest_subjects();
        let Some(current) = self
            .pending_manifest
            .and_then(|e| subjects.iter().position(|&s| s == e))
        else {
            // A wild program isn't in the list, so there is nothing to
            // cycle to — the footer doesn't offer the keys either.
            return;
        };
        let next = (current as isize + step).rem_euclid(subjects.len() as isize) as usize;
        self.pending_manifest = Some(subjects[next]);
    }
```

Update the module's `//!` doc comment to say it owns aiming the inspector and the manifest screen.

Note the behaviour change this makes deliberate: the manifest no longer closes on *any* key the way the old popup did. Only Esc leaves.

- [ ] **Step 4: Run the app-core tests**

Run: `cargo test -p feral-processes-app-core menus`
Expected: PASS.

- [ ] **Step 5: Draw the picker**

Append to `crates/gui/src/render/manifest.rs`:

```rust
pub(super) fn draw_manifest_pick(
    game: &mut Game,
    subjects: &[Entity],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row("Read whose manifest?")];
    for (i, &entity) in subjects.iter().enumerate() {
        let label = match game.manifest(entity) {
            Some(v) => match &v.subject {
                ManifestSubject::Player(_) => format!("You - Lv{}", v.level.unwrap_or(1)),
                ManifestSubject::Program(p) => format!(
                    "{} Lv{} - HP {}/{}  PWR {}{}",
                    v.name,
                    v.level.unwrap_or(1),
                    v.hp,
                    v.max_hp,
                    v.power,
                    p.activity
                        .as_ref()
                        .map(|a| activity_tag(a))
                        .unwrap_or_default()
                ),
            },
            None => "(gone)".to_string(),
        };
        rows.push(creature_row(
            format!("[{}] {label}", menu_shortcut(i)),
            i == selected,
        ));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to cancel"));
    draw_popup("Manifest", PopupSize::Large, &rows, painter, m);
}
```

In `crates/gui/src/render/mod.rs`, import `draw_manifest_pick` alongside `draw_manifest`, widen Task 3's guard so the picker gets the list too, and add the picker arm:

```rust
    let manifest_subjects = matches!(app.mode, Mode::Manifest | Mode::ManifestPick)
        .then(|| app.manifest_subjects())
        .unwrap_or_default();
```

```rust
        Mode::ManifestPick => draw_manifest_pick(game, &manifest_subjects, selected, painter, m),
```

- [ ] **Step 6: Run the suite**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace
git add -A
git commit -m "feat: d opens the manifest picker, arrows page between subjects"
```

---

### Task 5: Documentation

The repo's docs make claims this change falsifies — the manual's key table and its inspect-panel prose both describe the deleted popup.

**Files:**
- Modify: `docs/manual.md`
- Modify: `CHANGELOG.md`
- Modify: `crates/gui/src/render/meta.rs` (the in-game help)

- [ ] **Step 1: Update the in-game help**

In `draw_help` in `crates/gui/src/render/meta.rs`, change:

```rust
        text_row("u symlink   i inspect   v inventory   p companions"),
```

to:

```rust
        text_row("u symlink   i inspect   d manifest   v inventory   p companions"),
```

- [ ] **Step 2: Update the manual**

In `docs/manual.md`:

- Line ~120, change the `i` row to say it opens the manifest for the first program that way, and add a `d` row directly after it:

```markdown
| `d` | Manifest: a full read-only stat sheet for you or any program you own — integrity, XP, combat stats, potential rolls, routines, species detail. `←`/`→` page between subjects, `Esc` closes |
```

- Line ~449 (**Decompile chance**): replace "on the inspect panel" with "on the manifest".
- Line ~708: replace "in the pets screen (`p`) and the inspect screen" with "in the pets screen (`p`) and the manifest (`d`), which also breaks the tier down into the four individual rolls behind it".
- Line ~817: replace "on the inspect/battle screens" with "on the manifest and battle screens".

Grep for any other survivor: `grep -n 'inspect panel\|inspect screen' docs/manual.md README.md`.

- [ ] **Step 3: Update the changelog**

Add to `CHANGELOG.md` under the current unreleased heading, matching the file's existing entry style:

```markdown
- **Manifest screen (`d`).** A full read-only stat sheet for you or any program
  — integrity and XP meters, combat stats, all four potential rolls, installed
  routines, equipment with the bonus each piece is actually granting, perks,
  and a species panel with habitats, moves, work aptitude, growth and speed.
  `←`/`→` page between you and everything you own. Replaces the old inspect
  popup, which `i` now opens the manifest for instead.
```

- [ ] **Step 4: Final gate**

```bash
cargo fmt
cargo clippy --workspace
cargo test --workspace
```

Expected: PASS. Confirm `cargo test -p feral-processes-engine balance_sim` is green — it should be untouched, since nothing in this plan edits `tuning.rs` or an asset.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs: document the manifest screen and the d key"
```

---

## Manual verification

The repo's standing policy is to verify drawing changes through unit tests rather than launching the GUI, with a final visual sign-off by the user. After Task 5, hand over for that sign-off:

```sh
cargo run -p feral-processes
```

Check by eye: `d` opens the picker; row 1 is You; the sheet's boxes don't overlap or run off the edge; `←`/`→` pages between subjects; `i` + direction lands on a wild program's sheet with the footer showing only `Esc back`; `Esc` returns to the map from both.
