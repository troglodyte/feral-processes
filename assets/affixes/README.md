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

    // Required, and at least one stat must be POSITIVE. An affix that only
    // renames an item is refused at load for the reason above; so is one
    // that only charges, since nothing weighs its cost and no player would
    // ever equip the copy that rolled it.
    //
    // A negative stat alongside a positive one is fine, and three shipped
    // affixes use it — see "Drawbacks" below.
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

| Affix | Grants | Slots | Weight |
|---|---|---|---|
| `scavenged` | +1 DEF | any | 14 |
| `tempered` | +1 ATK | any | 13 |
| `honed` / `reinforced` | +2 ATK / +2 DEF | Weapon / Armor | 12 |
| `patched` | +1 DECOMP | Module, Armor | 11 |
| `of_static` | +1 ATK +1 DEF | any | 10 |
| `shimmed` | +2 DEF | Module | 10 |
| `rigged` | +2 ATK | Module | 9 |
| `of_the_ghost_protocol` | +2 DECOMP | Module, Armor | 8 |
| `of_deep_cache` | +1 DEF +1 DECOMP | Module, Armor | 7 |
| `overdriven` / `hardened` | +3 ATK / +3 DEF | Weapon / Armor | 6 |
| `of_sidechannel` | +1 ATK +1 DECOMP | Module | 6 |
| `volatile` | +2 ATK **-1 DEF** | Armor | 6 |
| `of_deadlock` | +2 DEF **-1 ATK** | Weapon | 6 |
| `of_recursion` | +2 ATK +1 DEF | Weapon, Module | 5 |
| `of_cold_boot` | +1 ATK +2 DEF | Armor, Module | 5 |
| `of_hot_swap` | +2 ATK +2 DEF **-1 DECOMP** | any | 4 |

For scale: shipped gear grants 1–4 points a stat, and `GEAR_LEVEL_STEP` adds
100% of base per gear level. So +2 is about what one gear level is worth on a
scavenged weapon — a real find early, a rounding error late. Author well past
+4 and an affix stops being a bonus and starts being the item.

**No single stat passes +3**, and `every_shipped_affix_pays_and_none_pays_
past_the_calibration` (`crates/engine/src/tests/affixes.rs`) is what holds
that against a retune. The ceiling is on the *stat*, not on the affix: a
drawback pays for a fourth point across two of them without reaching for a
fifth on either.

Slots are the other half of the calibration and are easy to skew by
accident. Every slot must have something to roll — an empty pool leaves that
slot's drops as interchangeable as they were before affixes existed, which
is the complaint the feature answers — and `every_slot_has_something_to_roll`
asserts it. Module is the slot that goes thin without anyone noticing,
because it is the one no affix is *obviously* about.

## Drawbacks

Three shipped affixes charge for what they grant. The rule that makes one
worth rolling is not the size of the bonus — with a +3 ceiling, an affix
that merely undercut `hardened` would be strictly worse than `hardened` and
nobody would want it however common it was. It is the **slot**: each of the
three puts a stat somewhere no clean affix will, and bills the slot's own
axis for it. ATK on armour, DEF on a weapon, and a fourth point across two
stats paid for in DECOMP.

Three consequences follow from where the penalty is applied, and all three
are deliberate:

- **A drawback grows with the run.** The affix is folded into the base
  before `Game::copy_bonus`'s three scaling axes, so the cost scales with
  gear level exactly as the bonus does. Applied afterwards it would quietly
  stop costing anything after a breach or two, which reads as a free upgrade
  rather than as a choice.
- **Neither fusion nor a rare tier deepens it.** Those are what a player
  *spends* to improve a copy, and spending to make your own gear worse on
  one axis is a trade nobody would take. `EquipmentStats::fused_for_tier`
  and `for_rarity` both leave a value at or below zero exactly where it is.
- **A DECOMP penalty is inert on a companion.** A program never attempts a
  capture, so `of_hot_swap` costs the player nothing on gear they hand
  across — which is the one place in the set where owning two copies of
  something is worth more than owning one.

## Removing one

Safe. A save naming an affix this build no longer has reads as *unaffixed*
rather than failing to load: `Game::affix_of` looks the id up and finds
nothing, and every reader goes through it. The copy keeps its slot, its rare
tier and its fusion tier, and loses the affix's name and bonus.

The same is true of renaming an `id`, which is why renaming is the one edit to
think twice about — every existing copy carrying the old id silently becomes
unaffixed.
