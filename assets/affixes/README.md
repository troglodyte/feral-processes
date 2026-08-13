# Custom affixes (mods)

An affix is a rolled modifier on a dropped piece of gear: it gives the copy a
generated name — "Overclocked Arc Lance **of Static**" — and a small extra
stat bonus on top of the item's own.

Drop a `.ron` file in this directory and it's picked up automatically the next
time a game session starts — no recompiling required. A malformed or unusable
file is skipped with a warning logged in-game rather than crashing startup.

**Deleting this directory is supported.** An empty or absent `assets/affixes/`
leaves `Game::roll_affix` with no pool, so it spends no RNG draw and every
seeded run behaves exactly as it did before affixes existed. That is the same
property `assets/policies/enemy_battle.ron` has.

## Schema

```ron
(
    // Required, unique across all affix files. This is what a save stores,
    // so renaming an id orphans it on existing copies — see "Removing one".
    id: "of_static",

    // Exactly ONE of `prefix` and `suffix`. A prefix goes in front of the
    // item name, a suffix behind it (conventionally "of ...").
    //
    // Both set is refused at load: "Overclocked Honed Arc Lance of Static"
    // fits in no column on any screen. Neither set is refused too — the
    // stat bonus would have no visible source, and a player cannot make a
    // decision about a number they cannot attribute.
    suffix: Some("of Static"),
    // prefix: Some("Honed"),

    // Required, and at least one stat must be non-zero — an affix that only
    // renames an item is refused at load for the reason above.
    //
    // Added to the item's own bonus BEFORE any scaling, so it grows with
    // gear level, fusion tier and rare tier exactly as the item's own stats
    // do. Added afterwards it would dwindle across a run, which is the
    // opposite of what a rolled property is for.
    stats: (atk: 1, def: 1),

    // Optional; omit for "any slot". One or more of `Weapon`, `Armor`,
    // `Module`. Worth setting when an affix reads oddly somewhere: DECOMP
    // does nothing at all on a program, so an affix granting it is best kept
    // off the slot a player is most likely to hand to a companion.
    slots: Some([Weapon, Module]),

    // Optional; defaults to 1. Relative likelihood *within the eligible
    // pool* — weight 12 is twice as likely as weight 6 — and normalised at
    // roll time, so it is not a probability and adding an affix does not
    // make affixes more common overall.
    weight: 10,
)
```

## Two rolls, not one

Whether a drop carries an affix at all is `tuning::GEAR_AFFIX_CHANCE`, decided
before this pool is ever consulted. `weight` only decides *which* affix, given
that one is being granted.

That split is deliberate and is the same shape `WILD_ROUTINE_CHANCE` and an
ability's `wild_weight` have: folded into one number, adding an affix to the
game would change how often affixes appear, so a mod could not add content
without also retuning the game. Content is moddable; how often it shows up is
a difficulty knob and lives in `crates/engine/src/tuning.rs`.

## Affixes and rare tiers are independent

A copy rolls its rare tier (`components::Rarity`) and its affix separately, so
most affixed copies are *ordinary* ones. That is on purpose: a rare tier is the
chase, at roughly 3.5% across the whole ladder, and an affix is the variety, at
roughly one drop in five. Gate the affix behind the tier and the overwhelming
majority of drops stay exactly as interchangeable as they were, which is the
complaint the feature exists to answer.

Both are rolled by `Game::grant_gear_drop`, so both come only from **found**
gear. Crafting and buying never roll either — made gear is deliberately plain.

## Calibration

The shipped set runs 1 to 3 points on one or two stats, weighted so the
smallest is the commonest:

| Affix | Grants | Weight |
|---|---|---|
| `scavenged` | +1 DEF, any slot | 14 |
| `honed` / `reinforced` | +2 ATK / +2 DEF | 12 |
| `of_static` | +1 ATK +1 DEF, any slot | 10 |
| `of_the_ghost_protocol` | +2 DECOMP | 8 |
| `overdriven` / `hardened` | +3 ATK / +3 DEF | 6 |
| `of_recursion` | +2 ATK +1 DEF | 5 |

For scale: shipped gear grants 1–4 points a stat, and `GEAR_LEVEL_STEP` adds
100% of base per gear level. So +2 is about what one gear level is worth on a
scavenged weapon — a real find early, a rounding error late. Author well past
+4 and an affix stops being a bonus and starts being the item.

## Removing one

Safe. A save naming an affix this build no longer has reads as *unaffixed*
rather than failing to load: `Game::affix_of` looks the id up and finds
nothing, and every reader goes through it. The copy keeps its slot, its rare
tier and its fusion tier, and loses the affix's name and bonus.

The same is true of renaming an `id`, which is why renaming is the one edit to
think twice about — every existing copy carrying the old id silently becomes
unaffixed.
