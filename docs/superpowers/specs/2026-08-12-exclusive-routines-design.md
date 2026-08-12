# Disk-first routines and the exclusive pool

**Date:** 2026-08-12
**Status:** approved

## What this is

Two changes that only make sense together.

The first reworks how every routine gets installed. Today `Game::install_routine`
does it in one step: you know a routine, you spend a blank Routine Disk, and the
routine lands in a slot. That is replaced by two steps — you *etch* a routine you
know onto a blank disk, producing an **etched disk** in cargo, and you *install*
the etched disk into a slot.

The second adds six **exclusive** routines. An exclusive routine is one nobody can
etch, because nobody can learn it. Its disk exists only where the world puts one:
dropped by a boss, or sold on a Stack trader's shelf.

The second is the point; the first is what makes it cheap. Once a disk is the
only thing that fills a slot, "a routine you cannot get" needs no new machinery —
it is simply a routine with no write path.

## Why disk-first

Three things fall out of it.

A routine acquired from the world has somewhere to *be* before it is used. A boss
dying while the whole party's slots are full has to put its prize somewhere, and
a market can refuse a purchase where a boss cannot un-die. Cargo is that place,
and it already saves, already has capacity limits, and is already a screen.

A Stack trader stops needing to write slots. `Game::buy_market_offer` currently
reaches into the party and writes routines into free slots, which drags in
`routine_recipients`, `routine_slot_free`, a target picker for the narrowest
scope rung, and a refusal for the case where nobody has room. Selling disks
deletes all of it: a purchase becomes `grant_loot`.

And exclusivity gets one enforcement point instead of four. Knowledge is the only
thing that can be duplicated — you know a routine once and etch it forever — so
keeping exclusive routines out of `KnownRoutines` is the whole gate.

## Data model

`AbilityDef` gains three fields, each `#[serde(default)]` so every shipped and
modded `.ron` keeps parsing unchanged.

```rust
/// Marks this routine exclusive: it never enters `KnownRoutines`, no
/// research node or species may grant it, and no blank disk can be etched
/// with it. Its etched disk is reachable exactly two ways — a boss drop
/// (`boss_drop`) or a Stack trader's rare shelf row.
///
/// Opt-in exclusion, the same idiom `wild_weight` uses: the default is
/// ordinary, so the pool is defined by the files that ask to be in it
/// rather than by this module naming them.
#[serde(default)]
pub exclusive: bool,

/// Which boss species drop this routine's etched disk, each with its own
/// 0.0-1.0 chance. Becomes the synthesised disk item's `ItemDef::droppable`,
/// which is why the boss path needs no engine code of its own.
#[serde(default)]
pub boss_drop: Option<Vec<(SpeciesId, f32)>>,

/// Fires on an event rather than being chosen on a turn. `None` — the
/// default, and what every shipped ability is — means the routine is
/// offered as a Special and runs when picked.
#[serde(default)]
pub triggers: Option<PassiveTrigger>,
```

```rust
/// What makes a passive routine fire. Deliberately a small closed set
/// rather than a general event name: each variant is a point in
/// `combat_round` that has to call the passive, and a trigger nothing
/// fires is an authored routine that silently never runs.
pub enum PassiveTrigger {
    /// A member of the holder's own group is dropped.
    AllyDropped,
    /// A status condition lands on the holder.
    Afflicted,
}
```

### Why a field rather than an `AbilityEffect` variant

The axis is *when* a routine runs, and it is genuinely orthogonal to *what* the
routine does — a passive should be able to Damage, Heal or Cleanse. Putting it in
`AbilityEffect` would either need one variant per effect it can pair with, or a
recursive `Passive { trigger, effect: Box<AbilityEffect> }` that forces a
delegating arm into all 27 match sites in `abilities.rs`. A field beside `effect`
is how `AbilityDef` already carries every other orthogonal modifier — `cooldown`,
`fatigue_cost`, `wild_weight` — and it costs no match site anything.

### Load-time refusals

Two clauses join `decompile_target_mismatch` and `field_buff_target_mismatch` in
`AbilityDef`'s validator chain, refused at load for their reason: a contradiction
caught at load is one file to fix, where the same contradiction caught at use is
a routine that silently does nothing.

- `triggers` set on a `field_only` effect. A `Phase` cannot fire when an ally
  drops; there is no battle for it to happen in.
- `exclusive` together with `wild_weight > 0`. A routine cannot be both hunt-only
  and boss-only — the two claims name different sole sources.

## Etched disks

`ItemDb::synthesise_etched_disks(&mut self, abilities: &AbilityDb)` runs after
both databases load and derives one `ItemDef` per ability:

| field | value |
|---|---|
| `id` | `etched_<ability_id>` |
| `name` | `Etched Disk · <Ability Name>` |
| `description` | the ability's own |
| `droppable` | the ability's `boss_drop` |
| `value` | `ETCHED_DISK_VALUE`, or `ETCHED_DISK_EXCLUSIVE_VALUE` when `exclusive` |
| everything else | default |

Leaving `craftable`, `cache_drop` and `equipment` at their defaults is what keeps
the exclusivity honest without a single explicit exclusion: an etched disk cannot
be pressed at a bench, cannot turn up in a Stack cache, and cannot leak into
`surface_boss_loot`, which filters on `equipment.is_some()`.

Synthesised rather than authored as ~66 `.ron` files because the set is a
function of the ability set. Hand-authored files would be 66 chances for the two
to drift, and nothing in the suite would notice a disk whose ability had been
deleted.

**No save bump.** `Inventory` is `Vec<(ItemId, u32)>` and `ItemId` is a `String`
newtype, so a synthesised id saves and loads like any other. A disk whose ability
a mod later removes resolves to nothing, exactly as a removed mod item already
does.

## The install flow

`Game::install_routine` is deleted. Two verbs replace it.

**`etch_disk(&mut self, ability: &str) -> Result<(), String>`** — refuses unless
the player knows the routine and holds a blank Routine Disk. Consumes the blank,
grants `etched_<ability>` ×1. An exclusive routine is refused here, though the
`knows_routine` check already catches every one of them; the explicit refusal
exists so the message says why rather than "you don't know that routine", which
would be true and useless.

**`install_disk(&mut self, entity: Entity, ability: &str) -> Result<(), String>`**
— refuses unless the holder is owned, has a free slot, is not already running the
routine, and the etched disk is in cargo. Consumes the disk, calls
`write_routine`.

Both spend last, after every refusal has cleared — the ordering `install_routine`
and `buy_market_offer` already promise, and for the same reason: there is no
buyback for a disk consumed on a path that then failed.

`uninstall_routine` is unchanged. The disk was consumed at install and does not
come back.

`write_routine` stays the shared primitive, but its documented two-caller seam
collapses to one caller now that the market no longer writes slots. Its comment
gets rewritten to say what is actually true rather than left describing a
disagreement that no longer exists.

## Boss drops

No engine code. `Game::equipment_drops_for` already merges every `ItemDef::
droppable` naming the dead species and `award_loot` already rolls each one, so a
synthesised disk carrying its ability's `boss_drop` is dropped by the existing
path.

Two consequences worth stating rather than discovering:

- A `FieldBuffKind::DropBoost` buff multiplies these chances like any other drop,
  because `equipment_drops_for` applies it to the whole merged table.
- The roll happens on every kill of that species, boss or not — but only bosses
  are named in any `boss_drop`, so in practice it is a boss-kill roll.

Three disks per boss at ~0.35 each is roughly one disk per boss kill.

## Trader shelf

Two changes to `Game::market_offers` and `buy_market_offer`.

**The scope rungs become quantities.** `RoutineScope::One` / `Party` / `Everyone`
keep their names, prices and labels, and now deliver 1 / 3 / 6 etched disks
respectively. Those are **constants, not the live party and roster sizes**: a
quantity derived from the party would change between the player reading the shelf
and paying for it, which is the objection `market_program_price` already makes
about folding Trace into a quote.

**A rare exclusive row.** Rolled after the program row, from the exclusive pool
only, at most one per market, single copy, priced above the Everyone rung. Its
chance is `STACK_MARKET_EXCLUSIVE_CHANCE_BASE + STACK_MARKET_EXCLUSIVE_CHANCE_PER_DEPTH
× depth`, clamped to `0.0..=1.0` — so the deep Stack is where these are actually
bought.

Rolled from the same local `StdRng` as everything else on the shelf, after the
program roll, so adding it shifts no existing draw and the shelf stays a pure
function of `FrameSpec::rng_seed`.

`routine_recipients`, `routine_slot_free` and `MarketOfferKind::Routine`'s target
picker are deleted. `MarketOfferKind` gains `EtchedDisk { ability }`.

## Extraction

`extract_routine` branches on `exclusive`:

- **Ordinary** — learn it, as today, and refuse if already known.
- **Exclusive** — grant `etched_<ability>` ×1, teach nothing. There is still
  exactly one copy in the run; what the player bought is the ability to re-site
  it, at the cost of a whole program.

The "already known" refusal applies only to the ordinary branch, since an
exclusive routine is never known.

**A trap this spec designed against turned out not to exist.** The draft
required a cargo-room check before `dissolve_tamed_program`, on the assumption
that `grant_loot` can land 0 when cargo is full. It cannot: the Buffer is
unbounded, `Inventory::cargo_used` is a display figure rather than a cap, and
`grant_loot` documents that it always lands `qty` in full. The check was
dropped from both `extract_routine` and `etch_disk`, and a comment in
`etch_disk` records the condition under which it would have to come back — if
a hold limit is ever added, both become paths that can destroy something and
hand back nothing.

## The six routines

| routine | effect | target | cd | boss | chance |
|---|---|---|---|---|---|
| Kernel Shear | `Damage 22` + Bleed 0.75 / 4 / 6 | WholeEnemyGroup | 4 | Wintermute | 0.35 |
| Null Cache | `Drain 12, heal_fraction 1.0` | WholeEnemyGroup | 3 | Wintermute | 0.35 |
| Deadman | `Damage 14`, `triggers: AllyDropped` | AllEnemies | 4 | Wintermute | 0.30 |
| Hard Fault | `Debuff Stun, duration 2` | AllEnemies | 5 | Overseer | 0.30 |
| Long Winter | `FieldBuff Mitigation 25, duration 300, cost 40.0` | WholeParty | — | Overseer | 0.35 |
| Watchdog | `Cleanse`, `triggers: Afflicted` | WholeParty | 4 | Overseer | 0.35 |

**These magnitudes were revised down during implementation.** The first draft
put Kernel Shear at 44 and Null Cache at 20, written before the shipped band
was measured: the top damage power in the whole set is `segfault_v3` at 17,
into a single target. 44 across a group was not a premium, it was a different
game. Kernel Shear now sits at 22 on a whole group — above `segfault_v3`
while reaching wider — and the rest were pulled into proportion with it.

Deadman's cooldown moved 2 → 4 for a different reason: it is `AllEnemies`
scope, and `every_everyone_scope_routine_pays_the_everyone_tier_price` sets a
four-round floor for that tier. The Fatigue half of that rule is skipped for
passives, since Fatigue is a cast cost and a passive is never cast; the
cooldown half is not, because a cooldown bounds how often the effect lands
regardless of who asked for it.

Two per flavour: two straight power-tier, two an unusual shape built from effects
that already exist, two passives.

Naming is computing vocabulary throughout — no occult words, per the standing
content rule. "Deadman" is a deadman's switch.

**These magnitudes are ungated.** `balance_sim` models no abilities at all, so
nothing in the suite will catch an exclusive routine being twice as strong as it
should be. They rest on argument, and the only real check is play. This spec does
not close that gap; it inherits it.

## Frontend

Small surface, all of it already enumerated:

- `app-core/src/app/routines.rs` — the install action becomes two actions, and
  the picker lists etched disks held rather than routines known.
- `gui/src/render/routines.rs` — the "Disks: N" row becomes blank disks plus the
  etched ones held.
- `app-core/src/app/stack_market.rs` and `lib.rs:737` — the `RoutineScope::One`
  target-picker mode is deleted along with the engine call that needed it.

## Testing

Beyond the ordinary coverage each change carries:

- An exclusive routine never appears in `etchable_routines`, in an ordinary
  market routine row, or in any research node's `unlocks_abilities`. Writing
  this one found a gap: `etchable_routines` had no filter of its own and
  relied entirely on nothing reaching `KnownRoutines`. It now filters
  explicitly, so a leak reads as a missing row rather than as a picker row
  that always refuses.
- `etch_disk` refuses an exclusive routine, with the message that says why.
- Extracting an exclusive grants its disk and leaves `KnownRoutines` untouched.
- Extracting an ordinary routine still teaches and still yields no disk —
  without this the exclusive branch could swallow both cases unnoticed.
- A passive never appears in `battle_special_options` and never in the field cast
  list.
- Each passive actually fires at its trigger point, and respects its cooldown.
  The ally has to drop **inside** the round for this to mean anything:
  `battle_resolve_round` snapshots the living party before anyone acts, so a
  fixture that sets HP to 0 beforehand tests nothing. A hostile carrying a
  whole-party routine is the deterministic way to do it.
- A market shelf is byte-identical across a save/load round trip, exclusive row
  included.
- Every synthesised disk resolves through `item_name` and `item_value`.

Each gets the mutation check this repo expects: delete the fix, watch the test
fail. A test that passes with the change reverted is not coverage.

## Out of scope

- Balancing the six magnitudes against anything. There is no gate to balance them
  against; see above.
- Any second exclusivity source. Boss drops and Stack traders are the two, and
  adding a third later means one `.ron` field, not a redesign.
- Surface traders stocking etched disks. They do not, and nothing here makes them.
