# Perk catalogue (mods)

Edit a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup.

**This directory is a catalogue, not a content directory.** Unlike species,
structures, items and abilities, you cannot add a perk by dropping in a new
file — the twelve that exist are fixed, and each file here only controls what
one of them is *called*, how it *reads*, and what it *costs*. A file naming
anything else fails to parse and is skipped.

The reason is that a perk has no shape to express as data. Every other
moddable thing in this game is numbers and ids the engine already knows how
to consume; a perk is a hook into a particular formula, and no two hook into
the same one. Keen Scavenger reaches into the scan roll, Low Power Mode into
the hunger-decay multiplier, Exploit Focus into the decompile chance's HP
term, Lean Compiler into recipe costs, Attacker/Defender/Buffer write
straight to your stats at purchase time, and the five `*_affinity` perks each
multiply one `AffinityKind` category's magnitude for your own casts only —
see below. There is no `effect:` field that could cover those without
becoming a programming language. So a thirteenth perk means a new `Perk`
variant in `crates/engine/src/perks.rs` plus a hook wherever its effect
belongs — see the repo's `CLAUDE.md`.

**The five `*_affinity` perks share two magnitudes, not five.** Payload
Tuning, Field Medic, Overclocker, Corruption Vector and Siphon Protocol each
raise your affinity for one `AffinityKind` category (Damage, Heal, Buff,
Debuff, Drain), in `crates/engine/src/tuning.rs` — but not all by the same
rate. Field Medic (`Heal`), Overclocker (`Buff`) and Corruption Vector
(`Debuff`) use `AFFINITY_PERK_BONUS_PER_LEVEL`; Payload Tuning (`Damage`) and
Siphon Protocol (`Drain`) use the higher `AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED`
instead, because those two categories skip the level-scaling every other
category gets from `abilities::ability_power_scale` (see that constant's
doc) — the higher rate is what keeps them from being a strictly worse
purchase than the `Attacker` perk. `AffinityKind::perk_bonus_per_level`
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
    // Which perk this file describes. One of exactly these twelve, written
    // as a bare identifier (not a quoted string):
    //
    //   KeenScavenger   raises the scan (g) success chance
    //   LowPowerMode    slows Power drain, down to not draining at all
    //   ExploitFocus    softens how much a decompile target's remaining
    //                   Integrity counts against the attempt
    //   LeanCompiler    cuts each item a compile recipe requires
    //   Attacker        permanent Attack, applied on purchase
    //   Defender        permanent Defense, applied on purchase
    //   Buffer          permanent max Integrity, and heals you on purchase
    //   DamageAffinity  raises your own Damage-category ability magnitude
    //   HealAffinity    raises your own Heal-category ability magnitude
    //   BuffAffinity    raises your own Buff-category ability magnitude
    //                   (saps included — they're negative-power buffs)
    //   DebuffAffinity  raises your own Debuff-category ability magnitude
    //   DrainAffinity   raises your own Drain-category ability damage
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
