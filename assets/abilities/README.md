# Custom abilities (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup. That
includes a file whose numbers aren't finite: RON accepts bare `NaN` and
`inf` literals, and they'd otherwise slip past every clamp downstream, so a
non-finite `fatigue_cost` or `effect.status.chance` disqualifies the whole
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
companion levels, one per ten player levels, six at most either way), and
every loaded ability automatically gets a `routine_<ability_id>` item minted
for it — see `../items/README.md`. A species or research node just names
which routine shows up pre-installed or lands in cargo; installing,
swapping, and popping one back out is a separate act. So shipping a new
ability means writing a single file here — the routine item exists with no
second file — and, if you want it reachable through normal play, referencing
its `id` from a species, a research node, or both.

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
    name: "Display Name",         // shown in the ability picker

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

    // What it does to each recipient. Exactly one of eight:
    //
    //   Damage(power: 6)
    //     Direct damage through the same formula a move uses
    //     (`power + attacker ATK - target DEF`, minimum 1), so it scales
    //     with the user's ATK. Optionally carries a status rider:
    //       Damage(power: 6, status: Some((
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
    //     `Atk` or `Def`.
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
    //
    //   Drain(power: 10, heal_fraction: 0.5)
    //     Damage through the same formula as `Damage`, then the *user* is
    //     healed for that fraction of the damage it actually dealt, capped
    //     at its own maximum Integrity. Healing off the dealt figure rather
    //     than the authored power means an armoured target returns less,
    //     which is the intended shape. `heal_fraction` is clamped to
    //     0.0-1.0 at load; a non-finite one disqualifies the file.
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
    //   FieldBuff(kind: Regen, power: 3, duration: 40, power_cost: 15.0)
    //     The field-only marker: an ability carrying this effect never
    //     appears in the in-battle Special picker and a wild carrier never
    //     retaliates with it — there is no separate `field_cast: bool` to
    //     set, this variant *is* the flag. Instead it arms a running field
    //     buff outside battle, through whatever cast path spends `power_cost`
    //     of the caster's Power to start it. `duration` is in turns, not
    //     battle rounds, and keeps ticking through any battle that follows
    //     until it runs out.
    //
    //     `kind` is one of ten, split into two scopes that gate `target`:
    //
    //       Creature-scoped (`target: OneAlly` or `WholeParty` only —
    //       anything enemy-facing is refused at load, since there is no
    //       mechanic to aim a field buff at a hostile):
    //         Regen        heals HP each turn
    //         Def          flat Defense bonus
    //         Atk          flat Attack bonus
    //         Mitigation   percent damage reduction
    //
    //       Run-scoped (`target: WholeParty` only — these always land on
    //       the player regardless of who casts them, so any other target is
    //       a lie about where the buff actually goes, and is refused at
    //       load):
    //         Coolant       restores Fatigue each turn
    //         Trickle       restores Power each turn
    //         CaptureBoost  percent bonus to capture odds
    //         XpBoost       percent bonus to XP earned
    //         EncounterDamp percent reduction to wild encounter odds
    //         DropBoost     percent bonus to drop rates
    //
    //     Five of the ten run `power` through the same level/affinity
    //     scaling every other ability effect gets (`abilities::scaled_power`)
    //     before delivering it: Regen, Coolant, Trickle, Def, Atk. The other
    //     five — Mitigation and the four rate kinds (CaptureBoost, XpBoost,
    //     EncounterDamp, DropBoost) — are percentage points, delivered at
    //     exactly the authored `power` regardless of who casts them. A
    //     percentage already carries its own ceiling; scaling one the way a
    //     flat amount scales would let it exceed that ceiling, which is what
    //     the cap on Mitigation exists to prevent. This split is orthogonal
    //     to the Creature/Run scope above — author `power` as points for the
    //     first five, percentage points for the rest, regardless of scope.
    //
    //     `cooldown` and `fatigue_cost` are both dead on this variant —
    //     neither battle-round throttling nor commanding a battle action
    //     applies outside one. A nonzero `cooldown` logs a warning naming the
    //     file, since its default is 0 and any other value is a deliberate
    //     (if pointless) choice. `fatigue_cost` does *not* warn, even though
    //     it's equally unused: its own default is nonzero, so there's no way
    //     to tell "left alone" from "set on purpose" — leave it out of the
    //     file, its value simply won't be read.
    effect: Damage(power: 6),

    // Optional; defaults to 0 (usable every round). Battle rounds that must
    // pass before the same combatant can spend this ability again. While it
    // is cooling the picker shows the row greyed with the rounds remaining,
    // and planning it is refused rather than silently wasting the round.
    //
    // Cooldowns are scoped to a single intrusion: they are cleared when the
    // battle ends and are never saved.
    cooldown: 2,

    // Optional; defaults to 5.0, the flat cost commanding a companion has
    // always charged. Player Fatigue spent to command this ability — note
    // it comes off *your* Fatigue even when a companion is the one acting,
    // because it models you issuing the command. An ability you can't afford
    // is greyed in the picker and refused, same as one on cooldown.
    //
    // This is Fatigue, not Power: Power is the other need, and abilities
    // don't touch it.
    fatigue_cost: 8.0,

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

## Magnitudes scale with level

`power` is an authored *baseline*, not the figure that lands. `Heal`, `Buff`
and `Debuff` magnitudes are multiplied by the level of whoever used the
ability, so a `Heal(power: 8)` restores 9 at level 1 and 32 at level 20. The
curve is `1 + level x ABILITY_POWER_SCALE_PER_LEVEL`, capped at
`ABILITY_POWER_SCALE_LEVEL_CAP` — both in `crates/engine/src/tuning.rs`.
Author powers as a level-1 baseline.

`duration` never scales. Neither does `Damage` power, nor `Drain`: both go
through `power + ATK - DEF`, so they already grow with the user's ATK, and
scaling the flat term as well would count the same growth twice.

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
        (id: "sandbox"),                    // level defaults to 1
        (id: "redundancy_sync", level: 6),
    ],
```

Ids are validated when the game starts. A species naming an ability that
doesn't exist keeps loading — the unknown entry is dropped with a warning,
because a species is still perfectly playable without one of its abilities.

Companions cap at level 12, so a `level` above that makes an ability
permanently unreachable.

## The hunt-only set

Twenty shipped abilities carry a non-zero `wild_weight` and are named by no
species file and no research node: `kernel_panic`, `stack_smash`,
`pipeline_stall`, `fork_bomb`, `packet_shred`, `bus_fault`, `hard_lock`,
`heap_corruption`, `race_condition`, `bit_rot`, `hyperthread`, `bastion`,
`throttle`, `etch`, `checksum_repair`, `mirror_restore`, `cold_boot`,
`siphon_cycles`, `leech_array`, `flush_cache`.

The only way to get one is to find a wild program carrying it and decompile
that program. Killing the carrier destroys the routine. Adding any of these
to a species or research file would defeat the point, and a test
(`assets::no_species_or_research_file_grants_a_wild_only_ability`) fails if
you do — a mod is of course free to.
