# Classes (mods)

Edit or add a `.ron` file in this directory and it's picked up automatically
the next time a game session starts — no recompiling required. A malformed
file is skipped with a warning logged in-game rather than crashing startup,
so a broken def costs the game that one class and nothing else.

**This directory may be deleted.** An empty catalogue is valid and inert:
the creation screen's class step has nothing to offer, every affinity axis
resolves neutral, and a player who somehow still holds a class picks up the
same hardcoded starting kit today's game hands everyone — exactly the
pre-class game. That is the same supported way to play that deleting
`assets/needs/` or `assets/memories/` is.

## What a class is

A class is chosen once, at character creation, and never rerolled. It grants
an **affinity spread** — a multiplier per ability category that scales the
authored magnitude of the player's own routines — and a **starting kit**
that replaces the default four-item inventory. Nothing else: no stat bonus,
no talent tree. (Talent trees in `assets/talents/` are a *companion's* axis;
the player's equivalent is Perk Points, spent on the affinity perks in
`assets/perks/`.)

The kit is the **default**, not a guarantee. The creation wizard's Kit step
sits directly after the class step and offers a `tuning::CREATION_CREDITS`
allowance to spend on `ItemDb::creation_shelf`; anything bought there
replaces this kit outright, and an empty basket keeps it. So price a kit
against that allowance — `the_creation_allowance_sits_inside_the_class_kit_band`
in `crates/engine/src/tests/assets.rs` is what holds the two in a band, and
a kit far outside it makes the Kit step either a free upgrade over this
class or a punishment for using it.

Each file is one class:

```ron
(
    class: Medic,
    name: "Medic",
    description: "Keeps the party standing. Repair routines land harder; \
                   damage routines land softer.",
    affinities: (heal: 1.3, damage: 0.8),
    kit: [("core_fragment", 5), ("power_cell", 5), ("outlet", 2)],
)
```

| Field | Meaning |
|---|---|
| `class` | Which of the five `AffinityClass` variants this file is for: `Striker`, `Bastion`, `Medic`, `Saboteur`, or `Leech`. The five are fixed in Rust — they are load-bearing across talent trees, base-post behaviour and species stat shapes — so a file cannot invent a sixth. Two files naming the same class is not an error; the alphabetically last filename wins. |
| `name` | What the creation screen leads with. |
| `description` | One or two sentences of flavour under it. |
| `affinities` | `#[serde(default)]` — every field of `Affinities` (`damage`, `heal`, `buff`, `debuff`, `drain`) defaults to neutral (`1.0`) individually, so a file may name only the categories it cares about, or omit the field entirely for a class with no spread at all. |
| `kit` | `#[serde(default)]` — a list of `(item id, quantity)` pairs stocked into the player's `Inventory` at creation, replacing the four-item default kit. Omitting it (or writing `[]`) starts the class with an empty pack. Superseded entirely if the player buys anything on the Kit step; see "What a class is" above. |

## The damped-axis convention

**Every shipped class trades something.** Each raises one affinity axis
above neutral and holds a different one below it — a class is a trade, not
a flat bonus. The shipped spread is `1.3` up, `0.8` down on every class, and
which axis is damped follows `ClassShape`'s own `damps` field
(`crates/engine/src/species.rs`, the same table that generates a *species*'
stat block for that class's role):

| Class | Raises | Damps |
|---|---|---|
| Striker | Damage | Heal |
| Saboteur | Debuff | Heal |
| Medic | Heal | Damage |
| Bastion | Buff | Damage |
| Leech | Drain | Buff |

Nothing in `ClassDb::load_dir` enforces this — a mod is free to author a
class with no downside — but a class file that doesn't damp anything reads
as a strict upgrade over every other class, which is the thing this
convention exists to avoid.

## What the spread reaches, and what it doesn't

An affinity multiplies the *authored magnitude* of an `AbilityEffect` in
that category (see `Game::ability_affinity`), clamped at
`tuning::AFFINITY_MAX` the same way a species' own affinities are. It does
**not** touch:

- **The player's ordinary attack.** That is `attack_range` into
  `resolve_attack`, which has no affinity term at all — a Striker's Damage
  affinity moves nothing about their basic swing, only their *routines'*
  authored power.
- **`balance_sim`**, for the same reason: it calls the same affinity-free
  `battle::expected_damage` the ordinary swing uses, so a class's spread is
  invisible to the balance regression gate. Retuning `affinities` here needs
  no `cargo test -p feral-processes-engine balance_sim` pass — the arena is
  the instrument for a class, since only a real fight runs abilities.

## The `iter` order

`ClassDb::iter()` walks `AffinityClass::ALL`'s declaration order (Striker,
Bastion, Medic, Saboteur, Leech), not file or insertion order — every caller
of it, including the creation screen, sees classes in the same order no
matter what order their files loaded in.
