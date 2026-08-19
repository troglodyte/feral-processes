# Talent trees (mods)

Edit or add a `.ron` file in this directory and it's picked up automatically
the next time a game session starts — no recompiling required. A malformed or
ill-formed file is skipped with a warning logged in-game rather than crashing
startup, so a broken tree costs one class its ladder and nothing else.

**This is a real content directory.** Unlike `assets/perks/`, you can add a
tree by dropping in a file: every node here is one of four shapes the engine
already knows how to apply, so nothing about a new tree needs Rust.

## What a tree is for

A companion normally stops at level 6 (`tuning::CREATURE_MAX_LEVEL`). Spending
Privilege Rings — dropped by lair guardians in the Stack, and by nothing else
— opens **Kernel Rings** on one program, each raising *that program's* ceiling
by two levels. Every level a companion earns **above** the base cap pays one
**talent point**, spent on one of two choices in the next untaken tier of its
class's tree.

Points are derived from level, never stored: a program at level 8 has earned
two, and spent as many as it has nodes. The ring buys room; the fights still
buy the levels.

## Which tree a program gets

By its **class**, which is derived from its species' affinities — see
`assets/species/README.md`'s "The five classes". A file's `class:` field says
which class it is for:

```ron
class: Some(Striker),   // or Bastion, Medic, Saboteur, Leech
class: None,            // the generic tree
```

`class: None` is the fallback, and it is not an edge case: a species that
raises no affinity axis, or more than one, has **no base job** rather than a
default class — a boss carries no affinities at all — and every such program
spends its points in the generic tree. `generic.ron` is the shipped one. A
class with no file of its own also falls back to it.

Two files claiming the same class is not an error; the alphabetically last one
wins, which is deliberate (a mod's `zz_striker.ron` overrides the shipped
tree without deleting it).

## Shape

```ron
(
    class: Some(Medic),
    tiers: [
        [
            (
                id: "medic_bedside",
                name: "Steady Hands",
                description: "Every repair routine mends 15% more.",
                node: Affinity(kind: Heal, mult: 1.15),
            ),
            (
                id: "medic_reserve",
                name: "Deep Reserve",
                description: "Raises Integrity by 10%.",
                node: Stat(stat: Hp, percent: 10.0),
            ),
        ],
        // ...five more tiers
    ],
)
```

| Field | Meaning |
|---|---|
| `class` | `Some(<class>)` or `None` for the generic tree. Defaults to `None`. |
| `tiers` | One list per rung, in order. Tier 1 is taken first. |
| `id` | Unique across **every** tree in the directory. It is what a save records, so renaming one loses whatever was bought. |
| `name` | The menu row. |
| `description` | One line under it, in the player's vocabulary. |
| `node` | What taking it does — one of the four below. |

**Exactly six tiers, exactly two choices each.** Six is
`KERNEL_RING_MAX * LEVELS_PER_RING` — one tier per level a fully ringed
companion can earn, so the last point spends the last rung. A tree of any
other depth is skipped with a warning, as is a tier offering one choice or
three: a tier is a decision, and a third option makes the ladder a list of
everything with extra steps.

## The four node kinds

### `Stat(stat: Hp | Atk | Def, percent: <float>)`

Raises one stat, **baked into the program's stats at purchase** through the
same arithmetic a Recompile Kernel's percentage buffs use — including its
never-less-than-a-whole-point floor, so a +8% on a Drone's 3 ATK is still
worth something. Because it is baked in, the save records the *node* and the
already-raised numbers; nothing re-applies on load.

Bounded by `tuning::MAX_TALENT_STAT_PERCENT` (15%), asserted over this
directory by a census.

### `Affinity(kind: Damage | Heal | Buff | Debuff | Drain, mult: <float>)`

Multiplies this program's affinity for one ability category, clamped to
`tuning::AFFINITY_MAX`. Its own species value is the base, so a Striker
sharpening Damage compounds with what it already had.

### `Ability(id: "<ability id>")`

Grants a routine outright, installed exactly as a species-kit unlock is: same
path, same slot competition, and a routine the program was carrying when you
decompiled it keeps its slot. Taking a node for a routine the program already
knows does nothing rather than duplicating it.

The id must name a file in `assets/abilities/`, and it must be a **battle**
ability. `AffinityKind` is blind to the distinction — a `FieldBuff(kind: Def)`
reports `Buff` like any other buff while never appearing in the Special
picker, which is the one place a granted routine is spent. A census refuses
both mistakes.

### `RoutineSlot`

One more routine slot than the program's level would give it. Note that every
routine slot in the game starts full, so this is what makes a granted routine
land beside a kit rather than competing with it.

## Authoring guidance

**Weight a tree toward `Ability`, `Affinity` and `RoutineSlot`, and keep the
`Stat` percentages small.** Two reasons, and the second is the one that
matters:

1. A developed companion already carries four multiplicative axes — bought
   Recompile Kernel tiers, five refactor slots, the levels a ring buys, and
   now talents. Options compound far less dangerously than numbers do.
2. `balance_sim`, the engine's balance regression gate, models **no
   abilities**. A `Stat` node moves its curves; an `Ability`, `Affinity` or
   `RoutineSlot` node is invisible to it and can only be measured in the
   arena. Weighting toward options keeps less of the tree ungated *and* less
   of it dangerous.

A census holds every shipped tree to spending at most half its nodes on
`Stat`.

**Make the tree read as its class.** `assets/species/README.md`'s "Kits"
section is the authority on what each class means: a Medic tree that reads
like a Striker's is a content bug even though nothing will fail. The shipped
trees follow the same rough shape — a small choice at tier 1, routines in the
middle, a slot and a capstone routine at the top.
