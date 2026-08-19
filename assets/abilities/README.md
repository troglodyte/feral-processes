# Custom abilities (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup. That
includes a file whose numbers aren't finite: RON accepts bare `NaN` and
`inf` literals, and they'd otherwise slip past every clamp downstream, so a
non-finite `power_cost` or `effect.status.chance` disqualifies the whole
file.

An ability is what a party member spends its round on when commanded with
Special in battle. The two sides of the party get theirs differently: a
companion's come from its species file — see `../species/README.md` — while
the player's come from a research node naming them in `unlocks_abilities`,
see `../research/README.md`. `decompile` is the one exception: it's
pre-installed on a new game rather than researched, so the player always
starts with a way to capture programs even before touching the research
tree.

Neither path is how the ability *reaches* a party member, though — abilities
are installed **routines** occupying level-derived slots (one per two
companion levels, one per ten player levels, six at most either way). What a
research node hands over is **knowledge**, not an item: it teaches the id.

Getting a known routine into a slot is then **two steps**. Etching burns one
blank **Routine Disk**, which the base has to manufacture, and produces an
*etched disk* carrying that routine; installing spends the etched disk on a
slot. Popping the routine back out later returns nothing — the disk is gone
at install.

A species file instead names what a companion shows up already running,
which skips both steps.

You never write an etched disk's item file: one is derived for every ability
at load (`ItemDb::synthesise_etched_disks`), with the id `etched_<ability
id>`. So shipping a new ability still means writing a single file here — no
second file and no per-ability item — and, if you want it reachable through
normal play, referencing its `id` from a species, a research node, or a
boss's `boss_drop`.

**Two abilities are mandatory.** `priority_boost` is the fallback every
companion falls back on when its species declares no abilities of its own,
and `decompile` is pre-installed into the player's first routine slot on a
new game — capturing a program is reached through the Special menu like
anything else. Deleting either file makes the game refuse to start, the same
way deleting the Currency item does.

## Schema

```ron
(
    id: "unique_snake_case_id",   // must be unique across all ability files
    // Shown in the ability picker. The shipped set follows
    // `<Family> <Scope>` — see "Naming" below — and two tests hold it,
    // but nothing in the loader enforces it, so a mod may name an
    // ability whatever it likes.
    name: "Packet Shred Group v2.0",

    // The one-line detail shown under the name in the picker. Authored
    // rather than computed from `effect`, so you control exactly how your
    // ability reads — the engine never rewrites this text.
    description: "Damage 6 to every member of one group",

    // Who the ability lands on. One of:
    //
    //   OneAlly             one party member the player picks
    //   WholeParty          every living party member, no picker
    //   OneEnemyGroupFront  the front member of one group the player picks
    //   WholeEnemyGroup     every living member of one group the player picks
    //   AllEnemies          every living enemy in every group, no picker
    //
    // `WholeParty` and `AllEnemies` open no second picker at all — there is
    // nothing left to choose, so the action commits the moment it's picked.
    // The two party-facing shapes skip members who are already down; a heal
    // spent on a downed member would be wasted.
    target: WholeEnemyGroup,

    // What it does to each recipient. Exactly one of ten:
    //
    //   Damage(power: 6, spread: 2)
    //     Direct damage through the same resolution a move uses: an attack
    //     roll against the target's Evasion, then a uniform roll from the
    //     damage band, then the user's ATK added flat, then the target's
    //     Mitigation taken off as a percentage. So it scales with the user's
    //     ATK, and it can miss.
    //
    //     `power` is the *centre* of the band and `spread` its half-width,
    //     so `power: 6, spread: 2` rolls 4..=8 inclusive. The low end is
    //     floored at 0, so a wide spread on a weak ability cannot roll
    //     negative. Both ends scale with the caster's level and affinity, so
    //     the band widens as the numbers grow rather than collapsing to a
    //     point.
    //
    //     `spread` is optional and defaults to 0 — a degenerate band, which
    //     is exactly the single deterministic number every ability dealt
    //     before ranges existed. No shipped file needed editing and a mod
    //     that never mentions it behaves as it always did.
    //
    //     Optionally carries a status rider, which lands only on a hit:
    //       Damage(power: 6, spread: 2, status: Some((
    //           kind: Bleed, chance: 0.5, duration: 2, power: 2,
    //       )))
    //     `chance` is 0.0-1.0. `kind` is `Bleed` or `Stun`; `power` is the
    //     per-round bleed damage and is unused (but still required — use 0)
    //     for `Stun`.
    //
    //   Heal(power: 8)
    //     Restores that much Integrity, capped at the recipient's maximum.
    //
    //   Buff(kind: Atk, power: 3, duration: 3)
    //     Temporary stat boost for `duration` battle rounds. `kind` is
    //     `Atk` (flat attack points) or `Mitigation` (**percentage
    //     points** of damage reduction, capped together with every other
    //     source at `tuning::MAX_MITIGATION_PERCENT`). There is no `Def`:
    //     defence is a percentage now, so `Mitigation` is the only name
    //     for that axis.
    //     A *negative* power is how you write a sap: `Buff(kind: Atk,
    //     power: -4, duration: 3)` with `target: WholeEnemyGroup` weakens
    //     a group rather than strengthening it, because the buff bonus is
    //     added unconditionally wherever it lands. There is no separate
    //     `Sap` effect, and adding one would be a second spelling of this.
    //     One caveat: a combatant holds a single buff at a time, and the
    //     Defend stance is itself a buff — so a sap landing on a bracing
    //     member overwrites its stance and it stops defending.
    //
    //   Debuff(kind: Stun, power: 0, duration: 1)
    //     Inflicts a status condition. Same `kind`/`power` rules as the
    //     rider above. A combatant carries at most one status at a time; a
    //     fresh application overwrites whatever was active.
    //     `duration` counts the rounds *after* the one the condition landed
    //     in, for both a `Debuff` and a `Damage` rider: `duration: 1` stuns
    //     the victim for the following round, and a `duration: 3` bleed
    //     ticks at the end of the next three. The round it lands in is not
    //     charged to it — otherwise a stun would expire before a victim
    //     that had already acted ever felt it.
    //
    //   Drain(power: 10, spread: 3, heal_fraction: 0.5)
    //     Damage through the same resolution as `Damage`, `spread` and all,
    //     then the *user* is healed for that fraction of the damage it
    //     actually dealt, capped at its own maximum Integrity. Healing off
    //     the dealt figure rather than the authored power means an armoured
    //     target returns less, and a miss returns nothing — which is the
    //     intended shape. `heal_fraction` is clamped to 0.0-1.0 at load; a
    //     non-finite one disqualifies the file.
    //
    //   Cleanse
    //     Clears each recipient's active status condition. No fields.
    //     Silent on a recipient that had nothing to clear.
    //
    //   Decompile
    //     Spends a taming catalyst and rolls a capture against the target
    //     group's front program — the formula lives in `taming`, not here.
    //     Requires `target: OneEnemyGroupFront` or `WholeEnemyGroup` —
    //     anything else is enforced at load time (a file pairing
    //     `Decompile` with `OneAlly`, `WholeParty` or `AllEnemies` is
    //     skipped with a warning, the same as a non-finite number), because
    //     it is resolved by group index and any other target would silently
    //     waste the round instead. Carries no fields of its own, and greys
    //     out in the picker with a reason — "no taming catalyst" or "roster
    //     is full" — instead of being refused silently. This is what
    //     `decompile.ron` uses; there is no reason to declare a second
    //     ability with this effect.
    //
    //   FieldBuff(kind: Regen, power: 3, duration: 40)
    //   FieldBuff(kind: Atk, power: 4)
    //     The field-only marker: an ability carrying this effect never
    //     appears in the in-battle Special picker and a wild carrier never
    //     retaliates with it — there is no separate `field_cast: bool` to
    //     set, this variant *is* the flag. Instead it arms a running field
    //     buff outside battle, through whatever cast path spends `power_cost`
    //     of the caster's Power to start it.
    //
    //     **Whether you write a `duration` at all is decided by `kind`, and
    //     both mistakes are refused at load rather than resolved quietly.**
    //     The two over-time kinds (Regen and Trickle) count turns down and
    //     *require* a `duration`; every other kind runs until the party
    //     rests and must *omit* it. A duration on one of those states a
    //     lifetime the game will never read — your 90-turn shield would be
    //     permanent and nothing would say so — and a missing one on an
    //     over-time kind arms at zero and expires on the turn it was cast.
    //
    //     `duration` is in turns, not battle rounds, and keeps ticking
    //     through any battle that follows until it runs out. An until-rest
    //     buff is ended by a rest, by a Forgiving reboot, or by another
    //     routine of the same kind displacing it — and by nothing else. Note
    //     that a *consumable*'s buff (`ItemEffect::prebattle_buff`) always
    //     keeps its own `ticks` count whatever kind it arms: the rule above
    //     is about routines.
    //
    //     `interval` (optional, defaults to 1) is how many turns pass between
    //     firings — `interval: 4` on a `duration: 300` Regen heals 75 times
    //     over 300 turns rather than 300 times. It only means anything to the
    //     two over-time kinds below (Regen and Trickle); the rest are
    //     read on demand and have no per-tick effect to space out. Make the
    //     duration a multiple of the interval: the cadence is phased off the
    //     turns remaining, so a duration that divides evenly fires on the
    //     first turn and every interval-th turn after, and one that doesn't
    //     simply starts its cadence later.
    //
    //     `kind` is one of eight, split into two scopes that gate `target`:
    //
    //       Creature-scoped (`target: OneAlly` or `WholeParty` only —
    //       anything enemy-facing is refused at load, since there is no
    //       mechanic to aim a field buff at a hostile):
    //         Regen        heals HP each turn
    //         Atk          flat Attack bonus
    //         Mitigation   percentage points of damage reduction
    //
    //       Run-scoped (`target: WholeParty` only — these always land on
    //       the player regardless of who casts them, so any other target is
    //       a lie about where the buff actually goes, and is refused at
    //       load):
    //         Trickle       restores Power each turn
    //         CaptureBoost  percent bonus to capture odds
    //         XpBoost       percent bonus to XP earned
    //         EncounterDamp percent reduction to wild encounter odds
    //         DropBoost     percent bonus to drop rates
    //
    //     Two of the eight run `power` through the stat-point
    //     level/affinity scaling (`abilities::scaled_stat_power` — see
    //     "Magnitudes scale with level") before delivering it: Regen and
    //     Atk. The other six are delivered at exactly the authored `power`
    //     regardless of who casts them, because each already carries its own
    //     ceiling and scaling one the way a flat amount scales would let it
    //     exceed that ceiling — which is what the cap on Mitigation exists to
    //     prevent. Mitigation and the four rate kinds (CaptureBoost, XpBoost,
    //     EncounterDamp, DropBoost) are percentage points. Trickle is the
    //     odd one: it is a flat point amount, but the pool it fills tops out
    //     at a fixed 100 at every level, unlike Regen's `max_hp`, so scaling
    //     it would let the level term swamp whatever the file authors. This
    //     split is orthogonal to the Creature/Run scope above.
    //
    //     `cooldown` is dead on this variant — a field buff runs outside
    //     battle, so battle-round throttling doesn't apply — and a nonzero
    //     one logs a warning naming the file, since its default is 0 and any
    //     other value is a deliberate (if pointless) choice.
    //
    //     What casting costs is the top-level `power_cost` below, the same
    //     field every other effect is priced in. This variant carried a
    //     `power_cost` of its own until the two cost fields were folded into
    //     one; a file still authoring it inside the effect will not parse.
    //
    //   Phase
    //     Steps the party through exactly one solid cell along their current
    //     facing, landing on the open cell beyond. Field-only and
    //     **Stack-only**: it reads and writes the party's frame
    //     coordinates, so it is greyed with a reason on open grid. Carries
    //     no fields — one wall is a rule of the mechanic, not an authored
    //     magnitude, because two deep turns it from "open the room next
    //     door" into a diagonal shortcut across the whole maze. Refused,
    //     spending nothing, when the rock runs deeper than one cell, when
    //     the far side is off the frame, or when there is nothing solid
    //     ahead in the first place. This is what `buffer_overrun.ron` uses.
    //
    //   Jump
    //     Moves the party to any cell of the frame they are standing in —
    //     the player aims a cursor over the frame map — and kills them if
    //     that cell turns out to be solid. Field-only and Stack-only like
    //     `Phase`, and carries no fields: the unvalidated landing is the
    //     whole mechanic. This is what `wild_jump.ron` uses.
    //
    //     Both movement effects require `target: WholeParty`; anything else
    //     is refused at load with a warning, since they move the party as a
    //     body and there is no mechanic to phase one companion through a
    //     wall. Both charge `power_cost` like everything else, both raise
    //     Trace on success, neither ever appears in
    //     the battle Special picker, and neither will be run by a wild
    //     carrier. Both refuse a landing behind an unopened sealed door, and
    //     refuse landing *on* one — a sealed door is walkable, so landing on
    //     it would otherwise bypass the lock entirely.
    effect: Damage(power: 6),

    // Optional; defaults to 0 (usable every round). Battle rounds that must
    // pass before the same combatant can spend this ability again. While it
    // is cooling the picker shows the row greyed with the rounds remaining,
    // and planning it is refused rather than silently wasting the round.
    // When it's ready the picker prices the row with it — "2 rd" — because
    // this is the *whole* cost of a Special: running one charges no need,
    // for a companion, for the player, or for a wild carrier.
    //
    // So a battle ability leaving this at 0 is completely unthrottled, and
    // the shipped set gives every one of them a cooldown between 1 and 5.
    // The single exception is `decompile`, which is deliberately free — it
    // already spends an ICE Breaker per attempt.
    //
    // Cooldowns are scoped to a single intrusion: they are cleared when the
    // battle ends and are never saved.
    cooldown: 2,

    // Optional; **defaults to 0.0**. Power spent to run this routine, by
    // whoever runs it — the caster pays, so a companion's Special draws on
    // the companion's own reserve rather than the player's.
    //
    // Free by default on purpose. This field reached only `Phase` and `Jump`
    // until 2026-08-17 and defaulted to a flat 5.0; now that every routine
    // in the game is priced in it, a nonzero default would silently charge
    // for every ability a mod ships. An ability that means to cost says so.
    //
    // The whole curve is scaled by `tuning::ROUTINE_POWER_COST_MULTIPLIER`,
    // which is code rather than data — how hard the game is, is not moddable.
    // Editing a number here and restarting the game needs no rebuild, which
    // is the faster loop for tuning one ability.
    //
    // `Game::proc_wielded_routine` is the one thing that does not charge it:
    // the 25% proc rate is that feature's whole price.
    power_cost: 8.0,

    // Optional; defaults to 0. How likely this ability is to be found
    // already installed on a wild program you meet in the field — a
    // "carrier". 0 means it never spawns wild, which is why every ability
    // reachable through a species or a research node leaves this alone.
    //
    // Weights are relative within the pool, not probabilities: an ability
    // at 12 turns up twice as often as one at 6. Whether a given wild
    // program carries anything at all is a separate roll the engine makes
    // (`WILD_ROUTINE_CHANCE` in `tuning.rs`); this only decides *which*
    // routine it gets once that roll has already succeeded.
    //
    // A carrier uses its routine against you in battle, and hands it over
    // installed if you decompile it. Killing it destroys the routine.
    wild_weight: 8,
)
```

## Naming

The shipped abilities are named `<Family> <Scope>`, with an optional
`vN.N` on the end. The scope word is the point of the scheme — the picker
shows the name before anything else, so how wide an ability reaches should
be readable without stopping to read the description:

| `target` | scope word |
| --- | --- |
| `OneAlly`, `OneEnemyGroupFront` | `Single` |
| `WholeParty` | `Party` |
| `WholeEnemyGroup` | `Group` |
| `AllEnemies` | `Everyone` |

The **family** is everything before the scope, and it names an *effect*,
not a file: `Packet Shred` is plain damage at any scope, `Fork Bomb` is
damage carrying a Bleed rider, `Pipeline Stall` carries a Stun, `Patch`
heals, `Hard Lock` stuns outright. So `kernel_panic.ron` displays as
"Packet Shred Single" — **an ability's id and its display name are
deliberately allowed to diverge**, because the id is frozen (see below)
while the name is free to move.

The **version tag** separates two abilities in one family that share a
scope and differ only in magnitude — `Patch Single v1.0` / `v2.0` / `v3.0`
restore 8, 25 and 50. A major bump is a real step up, a minor one is the
same thing slightly bigger (`Patch Party v1.0` and `v1.1` heal 8 and 10).

One consequence worth knowing before authoring a set of them: a family is a
**shared namespace between the hunt-only pool and everything else**, and the
hunt-only set got there first. Most family × scope slots are already held by
an ability with a `wild_weight`, which a species file may not name (see "The
hunt-only set" below), so a ladder built by adding rungs to an existing family
usually ends up with a hole no species can fill. `Segfault`, `Rollback` and
`Skim` are new families for exactly that reason rather than for flavour —
`Packet Shred Single`, `Patch Single v2.0` and `Leech Single` were all taken
by hunt-only files. Adding a version tag to an existing name is fine and
costs nothing (`sandbox` and `memory_leak` both picked one up when their
families became ladders); **renaming an `id` is not**, per the rule above.

Two tests in `crates/engine/src/tests/assets.rs` hold this over the
shipped set — `every_shipped_ability_name_ends_in_the_scope_it_targets`
and `no_two_shipped_abilities_share_a_display_name`. Neither looks at
files outside this repo, so **a mod is free to ignore the scheme**; the
loader has no opinion about `name` beyond it being a string.

**Do not rename an `id` to match a new display name.** Ids are save
format: `PlayerSave.known_routines` stores them directly, and each one
mints a `routine_<id>` item that may be sitting in a player's cargo.
Renaming one orphans both.

## Magnitudes scale with level

`power` is an authored *baseline*, not the figure that lands. Every effect's
magnitude is multiplied by the level of whoever used the ability. Author
powers as a level-1 baseline.

There are two curves, and which one applies depends on what the magnitude is
measured in:

- **HP** — `Damage`, `Drain`, `Heal`, `Debuff`. These are weighed against a
  target's Integrity, which grows fast (12 per level, and doubles again per
  zone), so they scale on the steeper
  `1 + level x ABILITY_HP_SCALE_PER_LEVEL`.
- **Stat points** — `Buff` and `FieldBuff`. These are added to ATK or DEF, or
  read as percentage points, and ATK/DEF grow at 1 per level — so they scale
  on the gentler `1 + level x ABILITY_STAT_SCALE_PER_LEVEL`. A `Buff` on the
  HP curve would turn a +3 attack routine into a tripling.

Both are capped at `ABILITY_SCALE_LEVEL_CAP`; all three constants are in
`crates/engine/src/tuning.rs`.

`duration` never scales, and neither does `Drain`'s `heal_fraction` — it
rides damage that has already been scaled.

A wild program has no level — it scales by zone and distance — so a carrier
scales its routine from the current zone instead.

`power` also carries a caster-side multiplier that has nothing to do with
level: every effect above belongs to an `AffinityKind` category (`Damage`,
`Heal`, `Buff`, `Debuff`, `Drain`), and the caster's affinity for that
category — from its species' `affinities` field, or from the player's
matching perk — multiplies the magnitude on top of everything in this
section. See `assets/species/README.md`'s `affinities` entry for the
schema and the categories.

## Referencing an ability from a species

In a species file, `abilities` is a list of entries naming an `id` and the
companion level that unlocks it:

```ron
    abilities: [
        (id: "overclock_array", level: 2),  // level defaults to 1 if omitted
        (id: "bastion_shield_v3", level: 6),
    ],
```

That is the shipped Sentinel's, and it is the shape every shipped non-boss
species uses: a class utility at level 2 and a tier rung at level 6. See
`../species/README.md`'s "The five classes" for the table and for why nothing
unlocks at level 1. A mod is under no such obligation — the loader has no
opinion about how many abilities a species grants or when.

Ids are validated when the game starts. A species naming an ability that
doesn't exist keeps loading — the unknown entry is dropped with a warning,
because a species is still perfectly playable without one of its abilities.

Companions cap at level 12, so a `level` above that makes an ability
permanently unreachable.

## `ranged`

`ranged: true` marks an attack that reaches past the front line. It is read
by **one path only** — the basic-attack path a wild program falls back on
when it has no Special to cast. A group standing behind
`tuning::ENGAGED_GROUPS` may use only its ranged attacks, and idles if it
has none.

A Special has never been gated on reach and still is not, so setting this on
an ordinary routine changes nothing today. Defaults to `false`, matching the
`ranged` field on a species' `moves:`, which is where every shipped value of
it comes from: a species' basic attacks are converted into abilities at
load (`species::basic_attack_ability`), and this is the field that survives
the conversion.

## The exclusive set

`exclusive: true` marks a routine **nobody can learn**. It never enters
`KnownRoutines`, no research node or species file may name it, and no blank
can be etched with it. The only thing that exists is its already-etched
disk, and there are exactly two places one comes from:

- **A boss drop.** `boss_drop: Some([("wintermute", 0.35)])` names the
  species that drop it and how often. This becomes the derived disk's
  `ItemDef::droppable`, so the ordinary loot roll pays it out — there is no
  separate boss-drop mechanic. Nothing here *requires* the named species to
  be a boss; the shipped set is held to it by
  `a_boss_drops_the_disks_its_abilities_claim`.
- **A Stack trader's rare shelf row**, drawn from the whole exclusive pool
  at a chance that climbs with depth.

Six ship: `kernel_shear`, `null_cache` and `deadman` off Wintermute;
`hard_fault`, `long_winter` and `watchdog` off the Overseer. Extraction at a
Compiler pops an exclusive routine's *disk* back out rather than teaching
it, which is the only way to move one — and it still costs the whole
program.

`exclusive` and a non-zero `wild_weight` together are refused at load: both
claim to name the routine's only source, and they name different ones.

## Passives

`triggers: Some(RoundStart)` — or `AllyWounded`, `Afflicted`, `AllyDropped`
— makes a routine fire on an event instead of being chosen on a turn. A passive occupies a slot
like anything else and appears in no menu: not the Special picker, not the
field cast list, and not a wild carrier's options. It costs no turn, so its
`power_cost` is never charged; `cooldown` is its whole price and is honoured
exactly as a chosen routine's is.

The trigger set is closed and small on purpose — each variant is a point in
`battle_resolve_round` that calls `Game::fire_passives`, so a trigger with
no call site would be a routine that silently never runs. `triggers` on a
field-only effect is refused at load for that reason: there is no battle
moment for it to fire in.

The four triggers, and who each one fires for:

| Trigger | Fires when | Holders |
|---|---|---|
| `RoundStart` | the round opens | every living party member |
| `AllyWounded` | a living member crosses below a third Integrity **this round** | the wounded member alone |
| `Afflicted` | a status condition lands on a combatant | the afflicted member alone |
| `AllyDropped` | a party member goes down that round | every living party member |

`AllyWounded` is a *crossing*, not a state: a member held low for six rounds
is one event, not six. Someone who died is `AllyDropped`'s and is never also
reported as wounded.

**Think twice before authoring on `AllyDropped`.** A dropped companion is
dissolved and despawned when the battle ends, with no revive at any
difficulty — so a routine paying out there only ever pays a player who has
already lost far more than the payout is worth, and the only way to use it
is to let a program die. It suits an exclusive last-stand routine like
`deadman`, which is meant to be the thing you never want to see fire. For
anything a player *chooses* to carry — an item's `grants` especially —
`AllyWounded` is the recoverable crisis you probably meant.

## The hunt-only set

Twenty-eight shipped abilities carry a non-zero `wild_weight` and are named by
no species file and no research node: `kernel_panic`, `stack_smash`,
`pipeline_stall`, `branch_hazard`, `fork_bomb`, `pid_exhaustion`,
`packet_shred`, `bus_fault`, `hard_lock`, `heap_corruption`,
`race_condition`, `bit_rot`, `hyperthread`, `bastion`, `clock_gate`,
`throttle`, `brownout`, `acid_wash`, `etch`, `oxide_strip`,
`checksum_repair`, `mirror_restore`, `cold_boot`, `siphon_cycles`,
`leech_array`, `cycle_harvest`, `invalidate_line`, `flush_cache`.

The only way to get one is to find a wild program carrying it and decompile
that program. Killing the carrier destroys the routine. Adding any of these
to a species or research file would defeat the point, and a test
(`assets::no_species_or_research_file_grants_a_wild_only_ability`) fails if
you do — a mod is of course free to.
