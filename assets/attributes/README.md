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

**Nothing reads these numbers.** No formula, no roll, no gate. They are
flavour today; each one's eventual mechanic will be designed for the number
rather than around it. That is why there is no field saying what an attribute
*does* — see `meaning` below.

A body's value is **minted once, from the place and the body, and stored**.
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
)
```

| Field | Meaning |
|---|---|
| `id` | Unique across the directory. It is what a save records, so renaming one drops whatever a body had under the old name — and it mints again on the next load. |
| `name` | What the dossier row leads with, in this setting's vocabulary. |
| `legacy` | The old-school word shown in parentheses after it — `"Willpower"`, `"Luck"`. What lets a player who has never read a line of code know what they are looking at. |
| `short` | The one-line gloss beside the number. |
| `meaning` | The page's prose: what a high or low value says about this program. **Never what it does** — see below. |
| `base` | The value a body with nothing authored for it mints around. Flavour, read by no formula, so tuning it changes no outcome. |
| `spread` | How far either side of the base a body's own value may land. `0` is legal and means every body reads the same number. Keep it below `base`, or a body can mint a negative attribute. |

All seven are required. Any field added in a later version will carry a
default, so a file written today keeps parsing untouched — but none of these
seven may be omitted.

## `meaning` must not promise an effect

A gloss that says an attribute "increases" or "reduces" something is a claim
the player will test and find false, and that is worse than no claim at all.
Describe what the number says about the program, not what it does to the sim.
A census in `crates/engine/src/tests/assets.rs` holds the shipped five to
this.

## A sixth attribute is a file drop

Nothing in Rust enumerates the shipped five. Drop a sixth `.ron` in here and
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

Two files claiming the same `id` is not an error; the alphabetically last one
wins, which is deliberate (a mod's `zz_parity.ron` overrides the shipped def
without deleting it).
