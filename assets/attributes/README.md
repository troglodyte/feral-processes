# Attributes (mods)

Edit or add a `.ron` file in this directory and it's picked up automatically
the next time a game session starts — no recompiling required. A malformed
file is skipped with a warning logged in-game rather than crashing startup, so
a broken def costs the game that one attribute and nothing else.

**This directory may be deleted.** An empty catalogue is valid and inert:
nothing is minted, every body's store stays empty and the dossier page reports
no rows — exactly the pre-attribute game. That is the same supported way to
play that deleting `assets/needs/` or `assets/memories/` is.

## What an attribute is

An attribute is a **second, non-combat stat block**: what a program is like,
apart from what it can do in a fight. Every creature and the player carry one
value per attribute, and you read them on the dossier page — `[D]` from a
program's manifest.

**Numbers are read only through `effects`, and by one exception.** The
player's are the stat allocation, and a seated program's work the same way.
The exception is Analysis on a posted program: its own value (falling back to
its species' catalogue figure) drives the mining roll through
`species::analysis_of`; its Extraction effect is the player's alone, so a
program never counts Analysis twice. Each point of an attribute above its
catalogue `base` moves the derived stats its `effects` name, and
`progression::derive` is the one function that turns a player's attributes
into HP, attack, mitigation, decompiler skill, Max Power, status resist,
extraction, crit and fumble. The level-up and creation Points screens,
`balance_sim` and the game itself all call it. An attribute with no
`effects` is flavour only; every shipped attribute has some.

A creature's value is **minted once, from the place and the body, and stored**.
The mint is a deterministic fold — the world seed, the tile the body spawned
on, its species and the zone — and spends no RNG draw, so it survives a save
and a reload unchanged. It also means **editing a `base` does not change a
program that already exists**: it changes what the next body to be minted
gets.

Each file is one attribute:

```ron
(
    id: "persistence",
    name: "Persistence",
    legacy: "Willpower",
    short: "holds its own state under stress",
    meaning: "How cleanly this program keeps its own state when things go wrong around it. A program low in Persistence is the one that comes back from a bad fight not quite the same as it went in.",
    base: 50,
    spread: 12,
    effects: [(stat: StatusResist, per_point: 1.0)],
    does: "Each point above the baseline shortens harmful statuses by 1%.",
)
```

| Field | Meaning |
|---|---|
| `id` | Unique across the directory. It is what a save records, so renaming one drops whatever a body had under the old name — and it mints again on the next load. |
| `name` | What the dossier row leads with, in this setting's vocabulary. |
| `legacy` | The old-school word shown in parentheses after it — `"Willpower"`, `"Luck"`. What lets a player who has never read a line of code know what they are looking at. |
| `short` | The one-line gloss beside the number. |
| `meaning` | The page's prose: what a high or low value says about this program. **Never what it does** — see below. |
| `base` | The value a body with nothing authored for it mints around. **It is also the zero point of `effects`**: a player at `base` gets nothing from the attribute, and each point above it (or below, for a negative sum) moves the stats. Editing it therefore shifts every player's derived stats, not only new bodies. |
| `spread` | How far either side of the base a creature's own value may land. `0` is legal and means every creature reads the same number. Keep it below `base`, or a creature can mint a negative attribute. The player has no spread; see below. |
| `effects` | `#[serde(default)]` — a list of `(stat: <DerivedStat>, per_point: <number>)`. Each point of the attribute above `base` adds `per_point` to that stat. `stat` is one of the closed set `MaxHp`, `Atk`, `Mitigation`, `Decompiler`, `MaxPower`, `StatusResist`, `Extraction`, `Crit`, `Fumble`; an unknown name makes the file malformed, and it is skipped with a warning. |
| `does` | `#[serde(default)]` — the one sentence saying what the effects do, in the player's words, shown beside the attribute on the Points screens. |

The first seven fields are required. `effects` and `does` default to empty,
so a file written before them keeps parsing untouched — but any field added
in a later version must carry a default too.

### Effects, and the one formula

`progression::derive` computes each derived stat as `base + sum(per_point *
(value - the attribute's catalogue base))` over every attribute that names
it, rounds once per stat, then clamps to the stat's range:

| Stat | Floor / ceiling |
|---|---|
| `MaxHp` | at least 1 |
| `Atk` | at least 1 |
| `Mitigation` | at least 0 |
| `Decompiler` | at least 0 |
| `MaxPower` | at least `tuning::MIN_MAX_POWER` (20) |
| `StatusResist` | `tuning::STATUS_RESIST_MIN` (-50) to `STATUS_RESIST_MAX` (75), in percent; negative lengthens a status |
| `Extraction` | 0 to `tuning::MINING_EXTRACTION_CAP` (0.10) |
| `Crit` | 0 to `tuning::CRIT_CHANCE_MAX` (0.20); starts at `CRIT_CHANCE` (0.08) |
| `Fumble` | 0 to `tuning::FUMBLE_CHANCE_MAX` (0.11); starts at `FUMBLE_CHANCE` (0.05) |

The shipped effects:

| Attribute | Each point above `base` |
|---|---|
| Parity | +6 max HP |
| Footprint | +2 max HP, +1 mitigation |
| Analysis | +1 attack, +1 decompiler, +0.005 extraction |
| Bandwidth | +2 max Power |
| Persistence | +1% status resist |
| Entropy | +0.002 crit chance, +0.001 fumble chance |

`tuning::CANONICAL_PARITY_PER_LEVEL` (4) and `CANONICAL_ANALYSIS_PER_LEVEL`
(2) reproduce the old automatic per-level growth, and a compile-time assert
in `tuning.rs` ties them to these Parity and Analysis values, so retuning
either `per_point` without those constants fails the build.

### What is buyable

**An attribute is buyable exactly when `effects` is non-empty**
(`AttributeDef::buyable`). Only buyable attributes are listed on the
creation and level-up Points screens, and a spend naming one that is not
is refused with `NotBuyable`. There is no separate flag: give an
attribute an effect and it appears; remove them all and it is flavour again.
Every buyable attribute must also author `does`, or the Points screen would
have nothing to say beside the number.

## `meaning` must not promise an effect; `does` may

`short` and `meaning` describe what the number says about the program and
still may not say what it does to the sim: a gloss that says an attribute
"increases" or "reduces" something is a claim the player will test, and it
would sit beside a `does` that has to be exact. `no_shipped_attribute_claims_an_effect`
in `crates/engine/src/tests/assets.rs` scans `short` and `meaning` (not
`does`) for words like "increases", "raises", "bonus" and "per point".

The rule used to forbid the claim everywhere, because nothing implemented
one. It is relaxed only in `does`, which is the sentence authored beside
`effects` and so is a claim made where the mechanic lives: it should match
the `effects` exactly, and a test that it exists is
`every_shipped_attribute_is_buyable_and_says_what_it_does`.

## A seventh attribute is a file drop

Nothing in Rust enumerates the shipped six. Drop another `.ron` in here and
it mints for every body from the next session onward, appears on the dossier
in id order, and is saved like the rest. The dossier page has no scroll, so
the engine trims the catalogue to `tuning::MAX_ATTRIBUTE_ROWS` rows before the
page is built — a catalogue past that ceiling costs the last rows in id order
rather than pushing the header off the bottom.

## Where a body's own base comes from

A species (`assets/species/`) and a class (`assets/classes/`) may each author
an `attributes:` map keyed by attribute id. That value replaces this def's
`base`, and the spread still applies around it — which is what makes a
species' own numbers show through while two bodies of that species still
differ. An id neither authors falls through to the def's `base` here, so the
two directories are independent of each other.

**The player is the exception to the spread.** The player carries no species,
so their starting value for each attribute is their class's authored number
(or the def's `base` if the class names none) with **no spread**: it is
chosen, not rolled, so the creation Points screen opens on the numbers the
run then starts with. Creation points and each level's banked points are
added on top, one attribute point each. A class authoring an attribute away
from `base` therefore starts with the corresponding derived-stat offset;
see `assets/classes/README.md`.

Two files claiming the same `id` is not an error; the alphabetically last one
wins, which is deliberate (a mod's `zz_parity.ron` overrides the shipped def
without deleting it).
