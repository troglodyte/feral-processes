# Field routines

## Problem

Every ability in the game resolves inside a fight. All five `AbilityTarget`
shapes name allies or enemy *groups* (`abilities.rs:142`), and every
`AbilityEffect` variant is applied in `combat_round.rs:662-770`. There is no
way to spend a routine on the world.

The one mechanic that does reach from the map into a fight is
`ConsumeDef::prebattle_buff` (`items_db.rs:40`), armed by `Game::use_item`
(`turn.rs:342`) and deliberately spared by `start_battle` (`combat.rs:140-143`).
It is wired, tested, and used by **zero shipped items** — and it is also
broken twice over (below).

**Field routines** are abilities whose effect is a long-lived buff armed
outside combat, which keeps running once a fight starts. They give routines a
second job, give the base-and-expedition loop a resource to spend Power on,
and turn a tested-but-dead carry-over path into the foundation for ten pieces
of content.

## Two bugs to fix first

Neither is optional: with either in place the feature cannot work.

1. **A map-armed buff is destroyed at the end of every battle.**
   `clear_battle_status_effects` (`game/combat_status.rs`) nulls the player's
   `CombatBuff` unconditionally when a fight ends, however it ends. A 5-round
   stim that survives a 2-round fight is wiped with 3 rounds left. The
   clearing itself is correct for what it was written for — a Rally or a
   brace left armed is a permanent free stat, and the doc comment says so —
   so the fix is not to stop clearing, it is to give map-armed buffs a home
   that clearing does not reach.
2. **A map-armed buff does not survive a save.** `PlayerSave`
   (`save.rs:13-43`) has no buff field at all.

## What exists

Verified against source, not remembered. Re-check before relying on any of
it.

- `CombatBuff` (`components.rs:445`) holds `Option<ActiveBuff>` — **one slot**,
  and `arm_buff` (`game/combat_status.rs:502`) overwrites. The single slot is
  deliberate: a companion that braces gives up the Rally it was carrying.
- `ActiveBuff::remaining` (`components.rs:424`) counts **battle rounds**,
  ticked by `tick_combat_buff` (`game/combat_status.rs:448`).
- `BuffKind` is `Atk | Def` only (`components.rs:415`). A third variant is a
  hook in a formula, not a data file — the same seam as `Perk`.
- **Every battle round already calls `self.tick()`** (`combat_round.rs:91`).
  A duration counted in ticks therefore ages during battle with no second
  clock and no new call site.
- `is_defending` (`game/combat_status.rs:86`) identifies a brace by sniffing
  the buff slot for `kind == Def && power == DEFEND_DEF_BONUS`. A new Def
  buff that happened to land on exactly that power would be read as bracing.
- `effective_atk` (`combat_round.rs:786`) and `effective_def`
  (`combat_round.rs:811`) are the only readers of a buff's magnitude.
- `tick_inner` (`turn.rs:91`) takes an `age_temporary: bool`;
  `age_temporary_structures` (`turn.rs:111`) is the one thing gated on it.
- `rest` (`turn.rs:386`) loops `tick_inner(false)` for `REST_TICKS`
  (`tuning.rs:577` = **40**), then full-heals the player and every tamed
  program.
- `AbilityDef` (`abilities.rs:277`) already carries a per-ability cost —
  `fatigue_cost` (`abilities.rs:294`), defaulted from `tuning.rs`. Precedent
  for authoring a cost on the ability.
- `AbilityEffect::affinity_kind` (`abilities.rs:264`) returns `None` for the
  two variants with no magnitude. `AffinityKind::perk`/`perk_bonus_per_level`
  are the precedent for a taxonomy method on an enum rather than a match at
  the call site.
- `abilities::scaled_power(power, level, affinity)` is the one place an
  authored magnitude is scaled.
- `Game::message_history` and `Game::structure_report` are the precedent for
  a read-only screen: the engine owns the per-row transform because app-core
  derives the row count and gui draws the rows, and folding in the renderer
  opens the screen on a row that isn't drawn.
- Save is bincode with **no migration** — `load` refuses any file whose
  version is not `SAVE_FORMAT_VERSION` (`save.rs:211-235`, currently 14).

## Design

### Components

Three buff lifetimes, two components.

**`CombatBuff` is unchanged.** In-battle Rally, Shield, Defend. Single slot,
displacing, cleared at battle end. Every invariant above survives, including
the `is_defending` power-sniff. Nothing in this feature writes to it.

**`FieldBuff { active: Vec<ActiveFieldBuff> }` is new.**

```rust
pub struct ActiveFieldBuff {
    pub kind: FieldBuffKind,
    pub name: String,      // the ability or item that armed it, captured at cast
    pub power: i32,        // already scaled; see Casting
    pub remaining: u32,    // ticks
    pub source: BuffSource,
}

pub enum BuffSource { Consumable, Routine }
```

`name` is stored rather than derived from `kind` because two different
routines can arm the same kind, and the buff list has to tell them apart.

- Duration in **ticks**, not rounds.
- Never touched by `clear_battle_status_effects`. That is the carry-over.
- Persisted (see Save).
- Lives on the player and on any companion a creature-scoped buff is cast on.

`effective_atk` and `effective_def` sum both components.

### Insert policy

One `Vec`, two rules, enforced in one `arm_field_buff` function:

- **`Consumable`: at most one entry total.** A new one displaces whatever was
  there, whatever its kind. This preserves the existing item behaviour
  exactly — item buffs go on displacing each other.
- **`Routine`: at most one entry per `FieldBuffKind`.** Re-casting a kind
  overwrites that kind's entry and nothing else.

So an item stim and a routine's Hardened Shell coexist; two item stims do
not; two Hardened Shells do not.

The item path (`use_item`'s `prebattle_buff`) is retargeted from `CombatBuff`
to `FieldBuff` with `source: Consumable`. That is what fixes both bugs for
items: it is no longer in the component that gets cleared, and it is in the
component that gets saved. `PrebattleBuff::rounds` becomes ticks; the field
is renamed accordingly and `assets/items/README.md` updated.

### The ability seam

A new variant, and **the variant itself is the marker** — there is no
`field_cast: bool` on `AbilityDef`:

```rust
AbilityEffect::FieldBuff {
    kind: FieldBuffKind,
    power: i32,
    duration: u32,      // ticks
    power_cost: f32,    // Needs::hunger spent to cast
}
```

An ability carrying this effect is field-only. `combat_round.rs`'s match
gains an `unreachable!` arm with a message naming why, exactly as
`AbilityEffect::Decompile` already has one (`combat_round.rs:770`), and the
battle ability picker filters it out.

`AbilityDef::cooldown` (battle rounds) and `fatigue_cost` are both ignored on
a field cast — Power is the only cost. A field-only ability authoring either
is not an error, but the values are dead; the loader logs a warning naming
the file.

**No new `AbilityTarget` variant.** Scope lives on the kind instead:

```rust
impl FieldBuffKind {
    pub fn scope(self) -> FieldScope   // Creature | Run
    pub fn affinity_kind(self) -> Option<AffinityKind>
    pub fn magnitude_label(self, power: i32) -> String   // "ATK+2", "HP+1/t"
}
```

- **Creature-scoped** kinds land on the targets `AbilityTarget` names, and may
  author `OneAlly` or `WholeParty`.
- **Run-scoped** kinds always land on the player — `Needs` is `With<Player>`,
  and XP, encounter rate, capture odds and drop rolls are all player-side.
  They **must** author `WholeParty`, so no picker opens. `AbilityDb::load_dir`
  rejects a Run-scoped field ability authoring anything else, with a logged
  warning and a skipped file, per the malformed-asset rule.

### Casting

The player opens a field-routine screen listing every `FieldBuff`-effect
routine installed on **themselves or on any party member**, in one flat list,
each row labelled with its holder. `Game::routine_holders`
(`game/routines.rs:57`) is the existing walk to build it from.

The **routine's holder is the caster**: magnitude is
`scaled_power(power, holder_level, holder_affinity)`, so a Hardened Shell run
off a level-20 Sentinel is stronger than the same routine on a fresh capture.
The scaled value is what is stored in `ActiveFieldBuff::power`, so later
level-ups do not retroactively change a running buff.

Casting charges `power_cost` against the player's `Needs::hunger`. Too little
Power refuses the cast with a log line and spends nothing — validate before
mutating, no partial application.

Casting is **not** gated on `require_surface`: it touches no zone-map state,
so it works underground. It is refused during a battle and after game over,
like every other map action.

### Aging, and rest

Field buffs decrement in `tick_inner`, **ungated by `age_temporary`**. That
places them one line from `age_temporary_structures`, which *is* gated, so
the contrast is deliberate and needs a test each or the next reader will
assume they match:

- A `Temporary` structure does not decay while you rest.
- A field buff does. Rest is time passing.

`REST_TICKS` is 40, so a rest bites 40 ticks out of everything running. That
is the intended tension with Power-only costing: a rest refills the Power to
re-cast, but charges most of a buff's life to do it. Durations are authored
against 40 as the natural unit.

An expiring buff logs a line naming what faded.

### Save

`PlayerSave` and `CompanionSave` each gain:

```rust
pub field_buffs: Vec<ActiveFieldBuff>,
```

The component's own type, not a parallel tuple — a tuple is a second shape to
keep in sync, and the copy that drifts is the one nobody runs.

Companions need it because creature-scoped buffs land on them.
`SAVE_FORMAT_VERSION` → **15**. Bincode has no migration, so existing v14
saves stop loading — accepted, and how v14 itself arrived.

`FieldBuffKind` and `BuffSource` are bincode-encoded positionally, like
`Perk`: **append variants, never reorder**, or bump the version again.

Two things deliberately need no wiring, recorded so nobody goes looking:

- A companion that is sold, extracted, fused away or killed takes its
  `FieldBuff` with it when the entity despawns. Neither
  `dissolve_tamed_program` nor `fuse_companions` needs a hook.
- Field buffs are player state, not zone-local. They survive a breach, and
  `enter_next_zone` must **not** clear them — the opposite of the
  `BuybackLedger` trap.

### Display

`Game::active_buffs() -> Vec<ActiveBuffView>` in `views.rs`, one pre-formatted
row per running buff:

```
Hardened Shell    DEF+2     12t
Repair Loop       HP+1/t     8t     (Sentinel)
Overclock         ATK+2      3t
```

Each row carries name, the magnitude label from
`FieldBuffKind::magnitude_label` (the *scaled* value, which gui cannot compute
and must not try), ticks remaining, and a holder label when the buff sits on a
companion.

**One accessor, both screens.** It reads `FieldBuff` *and* `CombatBuff`, so in
battle the list also shows a running Rally or brace, and on the map
`CombatBuff` is simply empty. No branching and no second accessor.
Item-armed buffs appear in the same list, since they are in the same
component.

gui draws it as a panel through `Painter` in `render/` — one on the map
screen, one on the battle screen. No backend calls in `render/`.

## The ten routines

Ten `.ron` files under `assets/abilities/`. Magnitudes and durations are
authored in the files, like every other ability magnitude; only genuinely
global knobs go in `tuning.rs`.

| File | Kind | Scope | Affinity | Effect |
|---|---|---|---|---|
| `repair_loop.ron` | `Regen` | Creature | Heal | Integrity per tick |
| `coolant_flush.ron` | `Coolant` | Run | Heal | Fatigue recovered per tick |
| `trickle_charge.ron` | `Trickle` | Run | Heal | Power sustained per tick |
| `hardened_shell.ron` | `Def` | Creature | Buff | flat DEF |
| `overclock.ron` | `Atk` | Creature | Buff | flat ATK |
| `ablative_layer.ron` | `Mitigation` | Creature | Buff | % incoming damage cut |
| `deep_scan.ron` | `CaptureBoost` | Run | — | % capture chance up |
| `trace_analysis.ron` | `XpBoost` | Run | — | % XP gain |
| `ghost_protocol.ron` | `EncounterDamp` | Run | — | % encounter rate down |
| `salvage_routine.ron` | `DropBoost` | Run | — | % drop roll up |

The four rate modifiers carry `affinity_kind() == None` for the same reason
`Cleanse` and `Decompile` do: a rate is not a magnitude in any of the five
affinity categories, and inventing a sixth category to scale them would
change what perks mean.

`power` means points for the five flat kinds (`Regen`, `Coolant`, `Trickle`,
`Def`, `Atk`) and percentage points for the five marked `%` — documented per
variant, matching how `ActiveBuff::power` is already read differently per
`BuffKind`.

Durations land in the 60–150 tick range so a rest is a real bite and not a
wipe. **Nothing here has been playtested**; these are arithmetic-plausible
starting numbers.

## Hook sites

Eight, all verified to exist. The three over-time kinds share one, so it is
eight sites for ten routines:

| Kind | Site |
|---|---|
| `Atk` | `effective_atk` (`combat_round.rs:786`) |
| `Def` | `effective_def` (`combat_round.rs:811`) |
| `Regen`, `Coolant`, `Trickle` | new `tick_field_buffs` in `tick_inner` |
| `Mitigation` | `apply_damage` (`game/combat_status.rs:316`) |
| `CaptureBoost` | `taming::capture_chance` (`taming.rs:33`) |
| `XpBoost` | `progression::add_xp` (`progression.rs:55`) |
| `EncounterDamp` | `maybe_spawn_wild_creature` (`game/spawning.rs:357`) |
| `DropBoost` | `equipment_drops_for` (`game/combat_rewards.rs:18`) |

`apply_damage` is the single choke point every HP loss passes through, which
is why `Mitigation` goes there and not at call sites.

`taming::capture_chance` and `progression::add_xp` are pure functions taking
their inputs — the buff term is passed in by the caller, not read from the
world inside them, so both stay testable without a `Game`.

## Testing

Reproducers first, both currently failing:

1. A buff armed on the map is still running after a battle ends.
2. A buff armed on the map survives a save/load round-trip.

Then:

- One test per hook site, asserting the buffed and unbuffed values differ in
  the right direction by the right amount.
- Insert policy: a second `Consumable` displaces the first; a second
  `Routine` of the same kind displaces only that kind; a `Routine` of a
  different kind coexists; an item and a routine coexist.
- Aging: a buff loses `REST_TICKS` over one `rest`, and a `Temporary`
  structure in the same test loses none. This pair is the whole point of the
  ungated placement.
- Expiry drops the entry and logs.
- `AbilityDb::load_dir` skips a Run-scoped field ability authoring `OneAlly`,
  with a warning, and keeps loading the rest of the directory.
- Casting with insufficient Power refuses and spends nothing.
- Casting a field routine held by a companion scales off *that companion's*
  level and affinity, not the player's.
- `Game::active_buffs` rows: holder label present for a companion-borne buff,
  absent for the player's; `CombatBuff` entries appear in battle.
- Save round-trip preserves kind, scaled power, remaining and source, for
  both the player and a companion.

Fixtures go in `crates/engine/src/tests/support.rs` — check what is already
there before adding one.

`cargo test -p feral-processes-engine balance_sim` is required: `Atk`, `Def`
and `Mitigation` move combat curves, and a moved curve is the signal.
`cargo test --workspace` is the final gate.

## Documentation

Same change, not a follow-up:

- `assets/abilities/README.md` — the `FieldBuff` effect variant, every
  `FieldBuffKind`, the scope rule and which `AbilityTarget` each scope may
  author. This is the schema reference for anyone modding.
- `assets/items/README.md` — `PrebattleBuff::rounds` is now ticks.
- Root `README.md` and `CHANGELOG.md` — grep both for claims this falsifies,
  in particular anything saying abilities are combat-only.

## Out of scope

- **Field-castable versions of existing combat abilities.** A routine is
  field-only or battle-only, decided by its effect variant. No ability does
  both.
- **Enemy-facing field casts.** The three enemy `AbilityTarget` shapes are
  unreachable outside a fight by construction.
- **A cooldown on field casting.** Power is the only limiter, chosen
  deliberately; `REST_TICKS` is what stops rest-and-rebuff being free.
- **New `BuffKind` variants.** `CombatBuff` is untouched by this work.
