# Perk catalogue (mods)

Edit a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup.

**This directory is a catalogue, not a content directory.** Unlike species,
structures, items and abilities, you cannot add a perk by dropping in a new
file — the seventeen that exist are fixed, and each file here only controls what
one of them is *called*, how it *reads*, and what it *costs*. A file naming
anything else fails to parse and is skipped.

The reason is that a perk has no shape to express as data. Every other
moddable thing in this game is numbers and ids the engine already knows how
to consume; a perk is a hook into a particular formula, and no two hook into
the same one. Keen Scavenger reaches into the mining roll, Low Power Mode into
the hunger-decay multiplier, Exploit Focus into the decompile chance's HP
term, Lean Compiler into recipe costs, Attacker/Defender/Buffer write
straight to your stats at purchase time, Obfuscation into what a Trace source
costs, Process Pool into the roster capacity, Teardown into a kill's salvage,
Failover into the base-wide repair rate, Tighten Tolerances into the floor a
compiled copy of gear rolls its quality off, and the five `*_affinity` perks
each multiply one `AffinityKind` category's magnitude for your own casts only
— see below. There is no `effect:` field that could cover those without
becoming a programming language. So an eighteenth perk means a new `Perk`
variant in `crates/engine/src/perks.rs` plus a hook wherever its effect
belongs — see the repo's `CLAUDE.md`.

**Five of the seventeen reach subsystems the rest never touch.** Obfuscation
reduces what every Trace source adds while you are in the Stack, floored so a
source always costs *something* — deliberately unlike Low Power Mode, which
is allowed to stop hunger draining entirely, because Trace is the Stack's
only escalation pressure. Process Pool adds roster slots through the same
`pet_capacity` a Data Cache feeds, so what it buys survives losing the
building. Teardown adds a flat amount to the work resource a defeated program
drops. Failover adds to the base-wide repair rate, which means a base with no
Patch Node standing stops taking permanent sweep damage. Tighten Tolerances
raises the floor a piece of gear you compile rolls its quality off, so it is
worth exactly what a tier of the bench is worth and is the only input to that
floor that is not a building — gear already carried keeps the quality it was
compiled at. Their magnitudes are `OBFUSCATION_REDUCTION_PER_LEVEL`,
`PROCESS_POOL_SLOTS_PER_LEVEL`, `TEARDOWN_SALVAGE_PER_LEVEL`,
`FAILOVER_REPAIR_PER_LEVEL` and `QUALITY_PERK_PER_LEVEL` in
`crates/engine/src/tuning.rs`.

**The five `*_affinity` perks share two magnitudes, not five.** Payload
Tuning, Field Medic, Overclocker, Corruption Vector and Siphon Protocol each
raise your affinity for one `AffinityKind` category (Damage, Heal, Buff,
Debuff, Drain), in `crates/engine/src/tuning.rs` — but not all by the same
rate. Field Medic (`Heal`), Overclocker (`Buff`) and Corruption Vector
(`Debuff`) use `AFFINITY_PERK_BONUS_PER_LEVEL`; Payload Tuning (`Damage`) and
Siphon Protocol (`Drain`) use the higher `AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED`
instead, because at the shared rate those two would be a strictly worse
purchase than the flat `Attacker` perk, which pays on every attack rather
than on one category (see that constant's doc for the arithmetic, including
where the two cross over as player level rises).
`AffinityKind::perk_bonus_per_level`
picks the right one, so a perk's category decides its rate rather than a
match at each of the five call sites. The result is clamped at
`AFFINITY_MAX`, the same ceiling a species file is clamped to at load — perk
levels are uncapped, so without the clamp a long enough game would let your
own casts exceed the bound every other affinity is held to. They scale
**only your own** ability casts, never a companion's: a companion's
affinity is its species' business (`SpeciesDef::affinities`), and a
party-wide perk would double-multiply against it. As with every perk here,
the `description` below is authored text — the engine never derives it from
the constant, so retuning either rate leaves the wording stale (still
quoting the old percentage) until someone edits the affected files to
match.

What *is* here is worth having: the wording players read, and the price they
pay. Both are things you'd want to change without a compiler.

**How much a perk gives per level is not here.** Those magnitudes live in
`crates/engine/src/tuning.rs` alongside every other difficulty knob, on the
principle that content is moddable but how hard the game is, is not. Cost
sits on this side of that line and magnitude on the other, so a `.ron` edit
can make a perk cheaper or dearer but not stronger. If you change a price,
say so in the `description` too — nothing keeps the two in sync for you.

## Schema

```ron
(
    // Which perk this file describes. One of exactly these seventeen, written
    // as a bare identifier (not a quoted string):
    //
    //   KeenScavenger   raises a mining node's per-cycle success chance
    //   LowPowerMode    slows Power drain, down to not draining at all
    //   ExploitFocus    softens how much a decompile target's remaining
    //                   Integrity counts against the attempt
    //   LeanCompiler    cuts each item a compile recipe requires
    //   Attacker        permanent Attack, applied on purchase
    //   Defender        permanent Mitigation, applied on purchase
    //   Buffer          permanent max Integrity, and heals you on purchase
    //   DamageAffinity  raises your own Damage-category ability magnitude
    //   HealAffinity    raises your own Heal-category ability magnitude
    //   BuffAffinity    raises your own Buff-category ability magnitude
    //                   (saps included — they're negative-power buffs)
    //   DebuffAffinity  raises your own Debuff-category ability magnitude
    //   DrainAffinity   raises your own Drain-category ability damage
    //   Obfuscation     cuts what every Trace source costs in the Stack,
    //                   floored so a source always costs something
    //   ProcessPool     raises how many tamed programs you may own
    //   Teardown        raises the work resource a defeated program drops
    //   Failover        repairs your structures with no Patch Node standing
    //   TightenTolerances raises the quality gear you compile rolls at
    //
    // Two files naming the same perk is not an error; the last one loaded
    // wins, and which that is depends on directory order — so don't.
    id: ExploitFocus,

    // Shown in the perk picker (x menu), the purchase message, and the
    // manifest's list of what you've bought.
    name: "Exploit Focus",

    // The line under the name in the picker. Write what a level actually
    // does — this is the only place a player is told.
    description: "You strike at the seams, not the surface. A target's remaining Integrity counts 3% less against your decompile odds per level.",

    // Perk Points per level, charged the same every time however many
    // levels you already have. You earn 1 Perk Point per player level.
    // Must be at least 1 — a free perk could be bought without limit, so a
    // file with `cost: 0` is skipped with a warning.
    cost: 3,
)
```

All four fields are required.

## Deleting a file

A perk with no file here simply stops being offered: it vanishes from the
picker and can't be bought. It is not an error, and it is not retroactive —
a save that already holds levels of that perk keeps them, and keeps the
bonus, because every effect reads what you've bought rather than this
catalogue.

## `groups.ron` — the picker's layout

One file here is not a perk. `groups.ron` is a list of the picker's
sections, in the order they appear on screen, each naming its heading and
which perks sit under it:

```ron
[
    (
        name: "Combat",
        perks: [Attacker, Defender, Buffer],
    ),
    (
        name: "Workshop",
        perks: [KeenScavenger, LeanCompiler, TightenTolerances, Failover],
    ),
]
```

It is read by name and skipped by the loader that reads the catalogue
entries, so it never has to parse as a perk. Both fields are required.

**It is the one statement of a section's heading, its contents and its
position**, which is why the grouping is not a `group:` field on each of the
seventeen catalogue files. Membership alone orders nothing — a per-perk
label would need a second rule for which heading comes first, and two
authored halves of one layout drift apart the first time someone edits only
one of them.

The order here is also, deliberately, not the order the perks are declared
in `crates/engine/src/perks.rs`. That order is save format — bincode encodes
an enum variant positionally — so it cannot be reshuffled to read better.
This file is where the reading order lives instead, and reordering it is
free.

Three things it does *not* do:

- **A perk no section names is still offered.** It lands in a trailing
  section with no heading, after everything else. A typo below costs a
  heading, never a row — the row is what a player spends points at.
- **A malformed file costs the headings and nothing else.** It's skipped
  with a warning, the same as a malformed perk, and the picker falls back to
  one flat unheaded list.
- **Deleting it restores the flat list** the screen drew before headings
  existed. That is a supported thing to do.

Naming a perk twice is not an error either; the first section that claims it
wins, so it appears once.
