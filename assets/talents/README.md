# Talent trees (mods)

Edit or add a `.ron` file in this directory and it's picked up automatically
the next time a game session starts — no recompiling required. A malformed or
ill-formed file is skipped with a warning logged in-game rather than crashing
startup, so a broken tree costs one class its ladder and nothing else.

**This is a real content directory.** Unlike `assets/perks/`, you can add a
tree by dropping in a file: every node here is one of five shapes the engine
already knows how to apply, so nothing about a new tree needs Rust.

## What a tree is for

Every level a companion earns **above** `tuning::TALENT_START_LEVEL` (6) pays
one **talent point**, spent on one of two choices in the next untaken tier of
its class's tree.

How far it may spend is what Privilege Rings buy — dropped by lair guardians
in the Stack, and by nothing else. Each **Kernel Ring** opened on a program
unlocks `LEVELS_PER_RING` (2) further tiers of that program's tree. A ring
does **not** raise a level ceiling: everyone in the party, the player
included, is capped at the same zone-derived number (`Game::level_cap`), so
what a ring buys is depth in one program's tree and not the right to be
bigger than its roster-mates.

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

## The five node kinds

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
ability. `AffinityKind` is blind to the distinction — a `FieldBuff(kind: Mitigation)`
reports `Buff` like any other buff while never appearing in the Special
picker, which is the one place a granted routine is spent. A census refuses
both mistakes.

### `RoutineSlot`

One more routine slot than the program's level would give it. Note that every
routine slot in the game starts full, so this is what makes a granted routine
land beside a kit rather than competing with it.

### `Accuracy(points: <int>)`

Adds flat Accuracy to every attack the program makes — its own swings and the
routines it runs alike, since Accuracy belongs to whoever is swinging.

**Read on demand, never baked**, unlike `Stat`. Accuracy is derived from
speed, level and flat sources and has no field on a program's stats, so there
is nothing to bake it into and nothing that could be re-applied on load. It is
the companion's half of the accuracy axis; `Perk::TargetLock` is the player's,
and the two never stack.

Worth most early and least late, which is a property of the curve rather than
of the node: a hostile's Evasion grows with the *zone* while a program's
Accuracy grows with its *level*.

Bounded by `tuning::MAX_TALENT_ACCURACY_POINTS` (6), asserted over this
directory by a census. Accuracy feeds a ratio, so an unbounded node would walk
a program to the hit-chance ceiling on its own and make every later tier in
its tree moot.

## Authoring guidance

**Weight a tree toward `Ability`, `Affinity` and `RoutineSlot`, and keep the
`Stat` percentages and `Accuracy` points small.** Two reasons, and the second
is the one that matters:

1. A developed companion already carries four multiplicative axes — bought
   Recompile Kernel tiers, five refactor slots, the levels a ring buys, and
   now talents. Options compound far less dangerously than numbers do.
2. `balance_sim`, the engine's balance regression gate, models **no
   abilities**. A `Stat` node moves its curves; an `Ability`, `Affinity`,
   `Accuracy` or `RoutineSlot` node is invisible to it and can only be
   measured in the arena. Weighting toward options keeps less of the tree
   ungated *and* less of it dangerous.

A census holds every shipped tree to spending at most half its nodes on a
number — `Stat` and `Accuracy` together. `Accuracy` counts even though it
never reaches the program's stats: what the rule is about is how much of a
tree is a figure going up rather than a decision.

**Make the tree read as its class.** `assets/species/README.md`'s "Kits"
section is the authority on what each class means: a Medic tree that reads
like a Striker's is a content bug even though nothing will fail. The shipped
trees follow the same rough shape — a small choice at tier 1, routines in the
middle, a slot and a capstone routine at the top.
