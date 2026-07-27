# Manifest screen — full stat sheet for the player and any program

Date: 2026-07-27

## Problem

Nothing in the game shows a complete stat sheet for anything.

- `Mode::InspectDetail` (`i` + direction) draws a ~12-line popup for a wild
  creature: HP, ATK, DEF, Power, decompile odds, habitats, moves. It omits XP,
  routines, potential rolls, growth, and speed, and it can only target a
  creature standing next to you on the map.
- The party and fuse menus pack a program onto one line: `Lv14 - HP 128/170
  ATK 41 DEF 27 PWR 238 [Excellent (94%)] (fused 1/3) (in party)`.
- The player has no detail screen at all. The map sidebar shows integrity,
  power, fatigue, level, zone, position, decompiler and equipment — a glance,
  not a reference — and several player-facing numbers (perk levels, an
  equipped item's actual scaled bonus, individual potential) appear nowhere.

Several stats the engine tracks are therefore invisible: all four `Potential`
rolls, `SpeciesDef::growth_multiplier`, `SpeciesDef::base_speed`, a program's
XP-to-next, and each `Perk`'s purchased level.

## Solution

One read-only full-window screen — the **Manifest** — that serves three
subjects: the player, an owned program, and a wild program. It replaces the
old inspect popup rather than sitting beside it.

### Naming

User-facing title: `MANIFEST`. In-fiction a manifest describes a program,
which fits the game's vocabulary (programs, routines, decompile, integrity,
Buffer) better than "character sheet" or "dossier". Code follows: `Mode::Manifest`,
`ManifestView`, `render/manifest.rs`.

### Why it replaces the inspect popup

The manifest is a strict superset of what `draw_inspect_detail` shows. Shipping
both would leave two inspection screens whose creature sections must be kept in
sync by hand — the exact drift CLAUDE.md's "a doc comment cannot hold two copies
in sync" rule exists to prevent. `InspectView`, `Game::inspect` and
`render/inspection.rs` are deleted, and `i` + direction lands on the manifest.

## Layout

```
┌ MANIFEST ────────────────────────────────────────────┐
│ ▓  Hexed                          (Scrapper 2)       │
│ ▓  Lv 14   Excellent (94%)   fused 1/3   in party    │
├──────────────────────────────────────────────────────┤
│ INTEGRITY ████████████░░░░░░  128/170                │
│ EXPERIENCE ██████░░░░░░░░░░░  240/620                │
├─ COMBAT ──────────────┬─ POTENTIAL ──────────────────┤
│ Attack         41     │ HP roll       1.14  ++       │
│ Defense        27     │ Attack roll   1.09  ++       │
│ Power         238     │ Defense roll  0.97   =       │
│                       │ Growth roll   1.18  +++      │
├─ ROUTINES ────────────┼─ SPECIES ────────────────────┤
│ 1 Overclock           │ Wastes, Ruins                │
│ 2 Firewall            │ Mines Core Fragment          │
│ 3 (empty)             │ Decompile difficulty  62%    │
├─ MOVES ───────────────┴──────────────────────────────┤
│ Slice (pow 12)      Static Burst (pow 9, Stun)       │
└ ←/→ other programs      Esc back ────────────────────┘
```

A header band, full-width meters, titled section boxes in two columns, and a
full-width band at the bottom. The player's sheet keeps the same frame and
swaps the right-hand column and the bottom band for player-only content.

## Engine

### `Game::manifest(&self, entity: Entity) -> Option<ManifestView>`

One accessor covering the player and any creature. Returns `None` for anything
that is neither (a structure, a nest, a despawned entity), or for a creature
whose species fails to resolve — same contract `Game::inspect` had.

### `ManifestView` (new, in `views.rs`)

Shared header fields, plus a `subject` enum carrying the half that differs.
The enum is the point: "the player has no `Potential` roll" and "a program has
no equipment" become type-level facts rather than `Option`s a renderer can
forget to check.

```rust
pub struct ManifestView {
    pub entity: Entity,
    /// Display name — the player's "You", a program's CustomName, or its
    /// zone-tagged species name.
    pub name: String,
    pub glyph: char,
    pub color: GlyphColor,
    pub level: Option<u32>,
    /// (xp, xp_to_next). `None` for a wild program, which carries no
    /// `Experience` until it is compiled.
    pub xp: Option<(u32, u32)>,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub power: i32,
    /// Active battle status condition, e.g. "Bleeding (2)" — see
    /// `Game::status_label`. Always `None` outside an intrusion.
    pub status_effect: Option<String>,
    /// Every routine slot, filled or empty — reuses `RoutineSlotView` rather
    /// than a parallel type, so the manifest and the routines menu cannot
    /// disagree about what is installed.
    pub routines: Vec<RoutineSlotView>,
    pub subject: ManifestSubject,
}

pub enum ManifestSubject {
    Player(PlayerManifest),
    Program(ProgramManifest),
}
```

`PlayerManifest`:

| Field | Source |
| --- | --- |
| `hunger`, `fatigue` | `Needs` |
| `decompiler` | `Decompiler::skill` |
| `equipment: Vec<ManifestEquipSlot>` | `Equipment`, one entry per occupied slot |
| `perk_points` | `Perks::points` |
| `perks: Vec<(String, u32)>` | every `Perk::all()` with `Perks::level > 0`, as (display name, level) |
| `position` | `Position` |
| `zone` | `ZoneLevel` |
| `pet_count`, `pet_capacity` | `Game::pet_count` / `pet_capacity` |
| `cargo_used` | `Inventory::cargo_used` |
| `party: Vec<CompanionInfo>` | `Game::party_info` |

`ManifestEquipSlot` carries the slot label, item display name, the bonus
**as currently applied** (`EquipmentStats::scaled_for_level(item.level)
.fused_for_tier(item.fusion_tier)`, matching `EquippedItem`'s captured values,
not a fresh preview at today's zone) and the fusion tier.

`ProgramManifest`:

| Field | Source |
| --- | --- |
| `species_name` | `SpeciesDef::name`; `None` unless a `CustomName` overrides it, in which case the header shows both |
| `is_hostile`, `is_tamed`, `is_companion`, `is_boss` | `Hostile` / `Tamed` / `Party` / `SpeciesDef::is_boss` |
| `activity` | `Game::program_activity`; `None` for a wild program |
| `potential: Option<ManifestPotential>` | all four `Potential` rolls plus `quality_percent` and `quality_label`; `None` for a legacy save with no component |
| `fusions`, `max_fusions` | `Game::fusion_count`, `tuning::MAX_FUSIONS` |
| `habitats` | `SpeciesDef::habitats` |
| `moves` | `SpeciesDef::moves` — full `MoveDef`, so the renderer can tag ranged and status effects |
| `work_resource` | `SpeciesDef::work_resource` |
| `taming_difficulty` | `SpeciesDef::taming_difficulty` |
| `decompile_chance` | same call `InspectView` made — `taming::capture_chance` against the held catalyst; `None` when no catalyst is held |
| `growth_multiplier`, `base_speed` | `SpeciesDef` |

### Stat sourcing

The player's `atk`/`def` come from `Game::effective_atk`/`effective_def` (gear
included) and `power` from `max_hp + atk + def`, identical to
`Game::player_status` — the manifest calls the same methods rather than
recomputing, so the sidebar and the sheet cannot show different numbers. A
program's come straight from its `Stats`, with `power` from `Stats::power()`.

### Deleted

`InspectView`, `Game::inspect`, and their re-exports.

## Renderer — `crates/gui/src/render/manifest.rs`

Drawn against `Painter` only; no backend calls, per the drawing-seam rule.
Reuses `bars::draw_bar` for the meters.

Sections are declarative — the draw function builds a `Vec<Section>` of
(title, rows) and hands it to the layout, so a section with no data is simply
absent and the ones below close up. A program with no `Potential` (legacy save)
or an empty routine list drops that box rather than drawing an empty one.

- **Header band:** glyph in species colour at title size, name, species name in
  parentheses when a custom name overrides it, then a subtitle line of level,
  quality tier, fusion depth, and activity/status.
- **Meters:** Integrity and Experience full-width. The player additionally gets
  Power (hunger) and Fatigue, matching the sidebar's labels.
- **Columns:** program — Combat / Potential / Routines / Species. Player —
  Combat / Progression (XP, decompiler, perk points) / Routines / Equipment.
  The Species box carries habitats, work aptitude, taming difficulty, the live
  decompile chance, growth multiplier and base speed — every `SpeciesDef` field
  the sim actually reads, so nothing in `ProgramManifest` goes undrawn.
- **Bottom band:** program — Moves, each with power and any ranged/status tag.
  Player — Perks (each purchased perk and its level), then Party.

### `manifest_layout(screen_w, screen_h, sections, m) -> ManifestLayout`

Pure geometry, free of the backend, exactly like `popup_layout`. Returns the
header rect, meter rects, a rect per section, and the footer rect. Splitting it
out is what makes the layout testable headlessly, and it is the same reason
`popup_layout` exists.

## app-core

- `Mode::InspectDetail` is renamed `Mode::Manifest`; `App::pending_inspect`
  becomes `App::pending_manifest`.
- New `Mode::ManifestPick` — the subject list: "You" followed by every program
  from `Game::owned_pets`, one row each.
- `d` on `Mode::Playing` opens `Mode::ManifestPick`. `d` is unbound today.
- Picking a row sets `pending_manifest` and enters `Mode::Manifest`.
- `i` + direction sets `pending_manifest` from `find_creature_in_direction` and
  enters `Mode::Manifest` directly, as it did for `Mode::InspectDetail`.
- Left/Right in `Mode::Manifest` cycles the *owned* subject list (You + owned
  programs) with wraparound. A wild program is not in that list, so Left/Right
  is a no-op there; the footer only advertises the keys when there is more than
  one subject to move between.
- Esc leaves `Mode::Manifest` for `Mode::Playing` — including when it was
  reached through `Mode::ManifestPick`, since the picker is a way in, not a
  place to be. Esc from `Mode::ManifestPick` itself also returns to `Playing`.
- Both variants are classified in `Mode::is_battle`'s exhaustive match (both
  `false`).

## Read-only

No mutation from this screen. Equipping, installing routines, party changes and
renaming stay in the menus that own them. The screen is a reference, and keeping
it one means it has nothing to hold in sync with those flows.

## Testing

**Engine**

- `manifest` on the player returns `ManifestSubject::Player` with the equipped
  bonuses reflected in `atk`/`def`, and its numbers equal `player_status`'s.
- `manifest` on a tamed creature returns `ManifestSubject::Program` with all
  four potential rolls, its routines, and `activity`.
- `manifest` on a wild creature reports `xp: None` and `activity: None`.
- `manifest` on a structure returns `None`.
- A creature with no `Potential` component yields `potential: None` rather than
  panicking.

**GUI**

- `manifest_layout` across the nine window heights the popup tests already
  sweep: no section rect overlaps another, and every rect stays inside the
  window.
- A section list with a hole in it (no Potential) leaves no gap — the following
  section moves up.

**app-core**

- `d` from `Playing` opens `ManifestPick`; Esc returns to `Playing`.
- Picking a row enters `Manifest` with `pending_manifest` set to that entity.
- Left/Right cycles the owned list and wraps at both ends.
- Left/Right on a wild subject changes nothing.
- `i` + direction enters `Manifest` targeting the creature found.

**Gate**

`cargo test --workspace`. No `tuning.rs` or asset change, so `balance_sim` is
untouched — but it runs as part of the workspace suite regardless.

## Out of scope

- Acting on the subject from this screen (see Read-only above).
- A manifest for structures or nests — `manifest` returns `None` for them.
  Structures already surface durability and worker on the map and in the base
  panel.
- Sprites or portraits. The header glyph is the existing `Glyph`, drawn large.
