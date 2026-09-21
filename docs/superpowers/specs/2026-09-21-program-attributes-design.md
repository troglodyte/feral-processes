# Program attributes and the dossier page

**Status:** design approved, not implemented.

A second, non-combat stat block carried by every creature and by the player,
authored as a data catalogue, minted at spawn, stored, and read on a new
page. Nothing consumes the numbers yet: the feature's deliverable is that
the player can read who a program is, in plain language.

---

## 1. What it is

Every creature — the player, a companion, a wild program, a boss — carries a
**`Profile`**: a small map of attribute id to integer. Five attributes ship.
They are deliberately the axes the game does *not* already model, and each
is named twice: once in this setting's vocabulary and once in the old-school
one, so a player who has never read a line of code still knows what they are
looking at.

| Attribute | Legacy | What it means |
|---|---|---|
| Coherence | Willpower | keeps its own state when things go wrong around it |
| Entropy | Luck | accumulated noise — the one that rises over a run |
| Bandwidth | Stamina | how much it can push at once |
| Footprint | Size | how much heap it occupies |
| Parity | Vitality | how well it corrects its own errors |

Alongside them, a derived **dossier header**: a build revision, a checksum,
and one sentence of provenance.

Both are read on `Mode::Dossier`, opened with `D` from the manifest.

## 2. Decisions taken

These were settled in design and are not open in implementation.

1. **The numbers do nothing.** No mechanic reads a `Profile` in this change.
   That is the request, not an unfinished edge — and it is why decision 3
   exists.
2. **The page never claims an effect that does not exist.** Each attribute
   authors what it *means* about the program, which is true today. The
   sentence saying what it *does* lands with the mechanic that does it, in a
   later change, as a second authored field. A gloss promising a mechanic is
   a claim the player will test and find false.
3. **Each attribute has exactly one obvious hook it is waiting on** —
   Coherence to status-condition duration, Entropy to fumble rate, Bandwidth
   to multi-target reach, Footprint to roster or squad slot cost, Parity to
   passive repair. Recorded here so the eventual mechanic is designed *for*
   the number rather than *around* it.
4. **The legacy names avoid the classic four**, because the game already has
   them under setting names: Constitution is `max_hp` (shown as Integrity),
   Strength is `atk`, Dexterity is `base_speed`'s accuracy and evasion, and
   Intelligence is literally `SpeciesDef::base_int`. Reusing those words
   would double-book a stat the manifest already draws.
5. **Not on `components::Stats`.** See section 4.
6. **No `resources::GameRng` draw.** See section 5.
7. **An absent `assets/attributes/` is the pre-feature game**, held at both
   ends: the mint resolves a def before storing anything, and every reader
   skips what it cannot resolve. `needs::NeedDb`'s property.
8. **A sixth attribute is a file drop.** Nothing in Rust enumerates the
   shipped five. `Rapport (Charisma)` was scoped and deferred on that basis.

## 3. Why the catalogue is data and `Disposition` is not

`crates/engine/src/disposition.rs` argues at length that a per-program
catalogue belongs in Rust rather than `assets/`, and this feature is the
other side of that line. A disposition "ships no name, no blurb, no glyph
and no mechanic, only multipliers on numbers the sim already computes" —
which puts it with `tuning.rs`, on the not-moddable side.

An attribute is the inverse. It ships a **name, a legacy name and two lines
of player-facing prose**, and — today — no multiplier at all. That is
content by exactly the test that put species, needs, memories and the perk
catalogue in `assets/`, so it goes in `assets/attributes/`.

The cost of being data is the one `disposition.rs` names: a `.ron` def
behind `#[serde(default)]` can ship inert and unauthored, as
`AbilityDef::spread` did. Section 8's censuses are what pay it.

### The def schema

```ron
( id: "coherence",
  name: "Coherence",
  legacy: "Willpower",
  short: "holds together under stress",
  meaning: "How cleanly this program keeps its own state when things go \
            wrong around it. A low-coherence program is the one that comes \
            back from a bad fight not quite right.",
  base: 50,
  spread: 12 )
```

`short` is the one-line gloss beside the number; `meaning` is the page's
prose. `legacy` is the parenthesised old-school name. All seven fields are
required — an attribute with no prose is the thing this feature exists to
avoid.

## 4. Why `Profile` is its own component

`components::Stats` is level-scaled by `progression::stats_after_levels`,
zone-multiplied by `ZoneLevel::stat_multiplier`, folded into by
`Game::apply_equipment_delta`, and summed by `Stats::power()` — which
`Game::difficulty_color`, `progression::kill_xp` and trade valuation all
read. A sixth field there enters every one of those whether it is wanted or
not, and `balance_sim`'s hardcoded empirical curves move.

A separate component is also what keeps decision 1 honest: nothing that
reads `Stats` can accidentally start reading an attribute.

`Profile` is a `BTreeMap<AttributeId, i32>`, not five named fields —
`BTreeMap` for `Stock`'s reason, that a `HashMap` makes the save encoding
differ run to run. `AttributeId` is a string newtype following `ItemId` and
`NeedId`.

## 5. Why minting spends no RNG draw

`Game::spawn_wild_creature_scaled` already draws three times from
`resources::GameRng` — `roll_potential`, `roll_wild_routine`,
`roll_rarity`. A fourth draw shifts every later roll in the run, which moves
seed-luck tests, arena baselines and `balance_sim`'s reference numbers, for
a cosmetic value.

So a `Profile` is **minted from a fold and stored from then on** —
`Disposition::seed(id)`'s precedent exactly, and `descriptions.rs`'s
argument in full: a draw from the shared stream does not survive a
save/load, so the same program would read differently after a reload.

The fold's inputs:

- through `Game::roster_parts()`: the `ProgramId` just minted.
- a wild creature: `(world seed, spawn x, spawn y, species id, zone)`.
- the player: the world seed.

Reduction is `derive::index`'s high-bit reducer, **never `%`** — the
anti-correlation trap `descriptions.rs` documents.

Two creatures of one species spawned on one tile in one zone get identical
profiles. Accepted: it is invisible flavour, and the alternative — a spawn
counter as a new `Resource` — reshuffles bevy's query iteration order.

## 6. Where a base value comes from

- A creature: `SpeciesDef::attributes`, a `BTreeMap<String, i32>` behind
  `#[serde(default)]`.
- The player: `ClassDef::attributes`, identically shaped. The player carries
  no `Creature` and no species, so its class is the only authored source it
  has.
- Neither authored: the catalogue's own `base`.

Variance is `base ± spread`, `spread` authored per attribute.

Both defaults mean no existing `.ron` — including a mod's — needs editing.
Both are censused, per section 3.

## 7. The dossier header

Three rows, none of them an attribute and none of them stored.

- **Build revision** and **checksum**: derived in Rust from the same seed as
  the profile, on `handles::of`'s model — a pure function, no pool, no
  storage, no content.
- **Provenance**: one sentence drawn from a `"program.dossier"` subject in
  `assets/descriptions/`. That module's doc comment already names this as
  its expansion seam ("`biome.forest` later is a file drop with no code
  change"), so this needs no new asset subsystem and is moddable by
  construction.

## 8. The page

`Mode::Dossier`, opened with `D` from `Mode::Manifest`, `Esc` back to it.
Uppercase because lowercase letters are row selectors.

**A new `Mode` variant does not fail to compile.** It must be hand-added to
`ALL_MODES`, to the renderer's draw match (which ends in `_ => {}`) and to
`needs_status_banner`, or it ships as a blank screen.

One derivation, `Game::dossier_report`, read by app-core for its row count
and by gui for drawing — a read-only screen's row count is app-core's and
its drawing is gui's, so any per-row transform lives in the engine or the
screen opens on a row that is not drawn.

The page has **no scroll**, so both budgets are censuses rather than
runtime behaviour:

- rows: `MAX_ATTRIBUTE_ROWS`, trimming the catalogue *before* the page's own
  cap, `MAX_NEED_ROWS`'s rule, so a modded catalogue cannot push the dossier
  header off the end. In `tuning.rs` on `MAX_TOOL_ROWS`'s precedent (a
  no-scroll ceiling the engine truncates against), not in gui, because the
  trim is a per-row transform and those live in the engine.
- width: measured through `paint::with_painter` against real text. The
  meaning lines wrap through `text::wrap`; the name row does not.

## 9. Save

`CreatureSave::profile` and `PlayerSave::profile`, additive behind
`#[serde(default)]`. The save is field-named RON, so this costs **no
`save::SAVE_FORMAT_VERSION` bump**.

An existing save loads with empty profiles and mints them on first read, so
a run in progress gets attributes without a migration.

## 10. Files

| File | Change |
|---|---|
| `assets/attributes/*.ron` | new; five defs |
| `assets/attributes/README.md` | new; the schema, per the moddability rule |
| `assets/descriptions/program_dossier.ron` | new; the provenance pool |
| `assets/species/*.ron` | optional `attributes:` on each of the 17 |
| `assets/classes/*.ron` | optional `attributes:` |
| `crates/engine/src/attributes.rs` | new; `AttributeId`, `AttributeDef`, `AttributeDb` |
| `crates/engine/src/components.rs` | `Profile` |
| `crates/engine/src/species.rs` | `SpeciesDef::attributes` |
| `crates/engine/src/classes.rs` | `ClassDef::attributes` |
| `crates/engine/src/game/spawning.rs` | mint at the wild door and in `roster_parts` |
| `crates/engine/src/game/creation.rs` | mint for the player |
| `crates/engine/src/game/lifecycle.rs` | restore on load, mint when absent |
| `crates/engine/src/views.rs` | `DossierReport` and its rows |
| `crates/engine/src/game/inspection.rs` | `Game::dossier_report` |
| `crates/engine/src/save.rs` | two `profile` fields |
| `crates/engine/src/tuning.rs` | `MAX_ATTRIBUTE_ROWS` |
| `crates/engine/src/tests/assets.rs` | the censuses |
| `crates/app-core/src/lib.rs` | `Mode::Dossier`, `ALL_MODES`, `needs_status_banner` |
| `crates/app-core/src/app/inspection.rs` | `D` on the manifest, `Esc` back |
| `crates/gui/src/render/` | the page, plus its draw-match arm |
| `assets/help/` | a manual page |
| `CHANGELOG.md` | the release section |

## 11. Tests

Intent, not implementation. TDD per step; `cargo test --workspace` is the
final gate, and `cargo test -p feral-processes-engine balance_sim` is the
balance gate.

1. **A spawn spends no `GameRng` draw** — the stream is identical across a
   spawn with and without the catalogue loaded. This is the test that
   protects every seeded baseline in the repo.
2. **An absent `assets/attributes/` is the pre-feature game** — no panic,
   empty profiles, the page reports nothing.
3. **A profile survives a save and a load** — a real save→load round trip,
   not a RON round trip, because `#[serde(skip)]` leaves a RON test green.
4. **An old save mints on load** rather than staying blank.
5. **Minting is deterministic** — the same seed and the same spawn produce
   the same profile, twice, and across a reload.
6. **Two censuses over the shipped assets**: every shipped species and class
   authors every shipped attribute, and every shipped attribute def has all
   its required fields. Without these the `attributes:` field is authored
   nowhere and nobody finds out.
7. **The page fits** — a row census and a width census, the width measured
   against real text through `paint::with_painter`.
8. **`Mode::Dossier` is in `ALL_MODES`** and draws something.
9. **`balance_sim` is unchanged**, asserted by the existing curves staying
   green with no edit.

## 12. Deliberately not in scope

- Any mechanic reading a `Profile`. Decision 1.
- The "what it does" prose line. Decision 2.
- `Rapport (Charisma)`. Decision 8.
- Showing `Disposition`, which is hidden by its own design and stays hidden.
- Attributes on structures. The request was entities and the player.
