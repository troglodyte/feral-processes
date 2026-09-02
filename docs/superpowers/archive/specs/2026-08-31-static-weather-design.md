# Static: weather, and the environment comes home to Rust

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

Phase 2 of `2026-08-19-environment-effects-design.md`, which shipped phase 1
and sketched this one. Two things changed since that sketch, and both are
deliberate departures from it: weather is **code, not a content directory**,
and phase 1's ground layer comes back into Rust with it.

## The problem

Two problems, and the second is why this is worth doing now.

**Weather was never built.** Phase 1 named the cold biome Deadlock
specifically to free the word "static" for this layer, and then the layer
never arrived. The surface has three standing ground conditions and nothing
that changes.

**Phase 1 is invisible in play, which is the bug that started this.** The
mechanism works — `Game::ground_effect` resolves, `game/turn.rs:525` applies
it, and past zone 1 every sector draws one of three biome shapes, so a player
in an `arid` sector is taking attrition on nearly every step. But:

- The only log line is `"You cross into {biome}."` (`turn.rs:539`). Phase 1's
  spec called for that line to name the effect and what it does. It does not.
- `EnvironmentDef::name` and `description` — "Dangling Reads", *"Nothing here
  is addressed. What you touch answers with garbage."* — are authored on all
  three shipped files, documented in the README, and **read by nothing**
  outside the loader and its tests.
- `Attrition` goes through `apply_damage`, which logs nothing. Integrity just
  drops. `Drag` spends extra ticks with no line at all.

So the entire player-facing half of a shipped feature is inert. This spec
fixes that in the same change, because the readout weather needs and the
readout the ground never got are the same surface.

A third thing is worth recording: `lock_contention` (the `Drag` one) is
effectively unreachable outside a `cold_storage` sector. `classify` puts the
temperature field near 1.0 around the origin against a neutral floor of
`-0.3`, which `assets/sectors/cold_storage.ron`'s own comment states. In
`arid` and `fractured` sectors the only effects a player can meet are the two
attrition ones. Weather is what gives the other biomes something that varies.

## Decisions taken, and why

Recorded so they are not re-litigated.

### Weather is a table in Rust, and phase 1's ground layer joins it

`assets/environment/` and its README are deleted. `crates/engine/src/
environment.rs` becomes the whole catalogue, on the shape
`crates/engine/src/notifications.rs` took — and for a stronger version of
notifications' own reason.

Notifications came home because "a `.ron` file cannot say *when* it fires".
The same holds here once weather has an ambush term: a multiplier on
`maybe_ambush`'s roll is a hook into a particular formula, not a shape a file
can express. The loader's whole cost — an id newtype, an absent-directory
rule, a malformed-file rule, three load-time refusals, and a pairing census
in the test suite to catch a def nothing resolves — was being paid to make
seven strings editable.

Two things go away that are worth naming, because they are real losses:

- **The README's promise is revoked.** "Deleting this directory restores the
  pre-environment game exactly" stops being true, because there is no
  directory. That promise is cited as *the* example of the absent-is-silent
  rule by four doc comments elsewhere in the tree — `rock.rs:101`,
  `perks.rs:674`, `sprites.rs:13`, `render/base.rs:2165` — and every one of
  them becomes false. They are repointed at `assets/sectors/`, which keeps the
  rule and keeps an example of it.
- **This contradicts CLAUDE.md's moddability rule** as stated, and is the
  second content directory to come home after notifications. It is a
  deliberate exception taken with the same argument, not drift.

The two enums keep their prose in an **exhaustive `def()` match**, not a
lookup table with a fallback — `cell_mark`'s rule. A variant with no words
must fail to compile.

### Ground and weather stack, and `EnvironmentEffect` stops being an enum

Today `EnvironmentEffect` is one-of: `Attrition { hp_percent, min_damage }`
or `Drag { extra_ticks }`. That shape cannot express a Thread Storm that both
slows you and draws things toward you, and it forces a case-split every time
ground and weather have to fold into one answer.

It becomes a struct with an identity and four terms. Folding is arithmetic:
**attrition and drag add, the ambush multiplier multiplies.** One fold, one
clamp, one bite — the summed percent and summed floor go into a single
`max(max_hp * pct, min)` so `apply_damage`'s floor-at-1 applies once rather
than twice.

Stacking rather than replacing is what makes weather read as weather: the
ground you know is still doing what it does, and something is on top of it.

### The three load-time refusals become a census

`EnvironmentDef::fault` exists because a *mod* could author `hp_percent: 0.5`
or `extra_ticks: 10_000` or claim `Platform`. Nothing is authored by a
stranger any more, so the runtime check is dead weight — but the ceilings
still matter, because the *fold* can now exceed what either half authored.

So: the ceilings move to the fold (`MAX_ENVIRONMENT_ATTRITION`,
`MAX_ENVIRONMENT_DRAG_TICKS`, and a new `MAX_STATIC_AMBUSH_MULT` are applied
to the summed effect), and a test walks both `all()` arrays asserting every
authored magnitude is inside its ceiling on its own. Cheaper than the loader
and strictly stronger: it fails the build rather than warning at startup.

`Platform` stays refused, in the resolver, where phase 1 already put it.

### Which event is live is derived, and there is no save field

```
epoch = current_tick() / STATIC_EPOCH_TICKS
seed  = fnv1a(world_seed, zone, biome, epoch)
```

...then a weighted pick from that biome's pool through `derive::index`,
against an implicit `STATIC_CLEAR_WEIGHT` so most epochs in most biomes are
clear. This is the third instance of a derivation already established twice —
`game/sortie.rs:174` and `game/contracts.rs:764` — and it buys the same four
things: no save field, no `SAVE_FORMAT_VERSION` bump, no `GameRng` draw
(worldgen must not draw from it), and nothing to save-scum.

**A fixed epoch rather than a per-event duration.** A per-event length reads
better — a Thread Storm should be short and violent — but "when did this one
start" is not derivable and would force a saved field and a migration. The
derived version costs nothing to avoid that, and the cost it pays is that
every biome in a zone turns over at the same instant. That is invisible in
play: the player is standing in one biome.

**Zone 1 takes no weather**, the same gate and for the same reason as ground.

### Open Grid gets weather and no standing condition

Phase 1 left an open question: *is three ambient flavours enough for the
ground to feel authored, or does Open Grid need one too, at the cost of "does
something" becoming a tax rather than an exception?*

Weather answers it. `PacketFlood` claims Open Grid, so the biome most of the
map is made of stays free to cross **usually** and is occasionally the worst
place to be. That is the shape the question was looking for, and it is only
reachable once the layer has an on/off axis.

## The catalogue

### `GroundCondition` — three variants, transcribed from the deleted files

| variant | biome | effect |
| --- | --- | --- |
| `DanglingReads` | Null Sector | attrition 0.02, floor 1 |
| `ThermalLoad` | Mainframe | attrition 0.03, floor 2 |
| `LockContention` | Deadlock | drag 1 |

Names, descriptions and magnitudes carry over unchanged. This half is a
transcription, not a redesign — nothing about how the ground plays moves in
this change, so a balance question raised here is a phase-1 question.

### `StaticEvent` — four variants, named in phase 1's sketch

| variant | biomes | attrition | floor | drag | ambush |
| --- | --- | --- | --- | --- | --- |
| `LeakingMemory` | Null Sector | +0.015 | +1 | — | — |
| `ThreadStorm` | Mainframe | — | — | +1 | x1.5 |
| `PacketFlood` | Open Grid | — | — | +1 | x1.6 |
| `SignalNoise` | Deadlock, Null Sector | — | — | — | x2.0 |

Every magnitude is an opening guess, and every one of them lives in
`tuning.rs` rather than in a match arm. Two shapes to hold to:

**No event is attrition-only except on ground that is already attrition.**
Otherwise weather is merely "the number went up", which is the failure mode
that makes it indistinguishable from the ground being worse today.
`SignalNoise` carries no damage at all, so at least one event is felt
entirely through what it lets happen to you.

**`SignalNoise` claims two biomes, and that is not symmetry, it is reach.**
Every other event claims one. Deadlock is only the dominant biome in a
`cold_storage` sector — around the origin `classify` puts the temperature
field near 1.0 against a neutral floor of `-0.3` — so an event claiming
Deadlock alone would be as unreachable as `LockContention` already is, and
would ship as a variant nobody meets. Null Sector is the second claim
because it is common in two of the three sectors. The cost is that Null
Sector is the one biome with two events in its pool; it is also the one with
the harshest standing condition, so a clear epoch there means more than it
does elsewhere.

Opening pool weights: one per event against `STATIC_CLEAR_WEIGHT`, so a
single-event biome is clear on most epochs and Null Sector somewhat less
often. Weights are per-event in the table so an event can be made rare
without being removed.

## Engine changes

### `crates/engine/src/environment.rs` — the catalogue

Rewritten. Holds `EnvironmentEffect` (the struct, with its identity, its fold
and its clamp), `GroundCondition`, `StaticEvent`, each enum's `all()` array
and exhaustive `def()`, and `GroundCondition::for_biome`. Two conditions
claiming one biome stops being a load-time warning and becomes
unrepresentable, because `for_biome` is a match on the biome.

Deleted with the loader: `EnvironmentDb`, `load_dir`, `EnvironmentDef`, its
id field, `fault()`, both resource inserts in `game/lifecycle.rs`, and the
warnings plumbing for them.

### `crates/engine/src/game/environment.rs` — the resolvers

Phase 1 kept this file apart from the db "so a later weather layer has an
obvious home beside it rather than inside the loader". This is that layer
arriving where it was expected.

- `Game::ground_condition(x, y) -> Option<GroundCondition>` — phase 1's
  `ground_effect`, renamed for what it now returns.
- `Game::static_at(biome) -> Option<StaticEvent>` — the derivation above.
- `Game::terrain_effect(x, y) -> EnvironmentEffect` — **the one door.** Folds
  the two and clamps once. The zone-1 gate and the `Platform` refusal live
  here, so neither can lapse at a second caller, which is exactly the rule
  phase 1 established and this change inherits rather than restates.

**The renames fail to compile; the shape change does not, and that is the
trap.** `ground_effect` -> `ground_condition` breaks every call site loudly.
`EnvironmentEffect` going from one-of to all-of does not: code that reads the
attrition terms and ignores the rest compiles clean and silently drops both
drag and the ambush multiplier. There is exactly one reader — the hook in
`game/turn.rs` — and the test that holds it is the stacking test, which must
assert all three terms and not just the bite.

### `game/turn.rs` — the hook

Already in the right place, after the `Position` write and before
`maybe_ambush`. It changes from a `match` over two variants to: one folded
effect, one bite, one drag count. The order phase 1 fixed is unchanged.

### `maybe_ambush` — the new term

It already reads the tile's biome to skip the slab, so the multiplier lands at
an existing read: `random_bool(RANDOM_ENCOUNTER_CHANCE * mult)`, clamped.
This is phase 3's first hook arriving early, and only that hook — biome does
not bias the roll on its own, only weather does.

### `tuning.rs`

New: `STATIC_EPOCH_TICKS`, `STATIC_CLEAR_WEIGHT`, `MAX_STATIC_AMBUSH_MULT`,
and one magnitude per `StaticEvent` term. `MAX_ENVIRONMENT_ATTRITION` and
`MAX_ENVIRONMENT_DRAG_TICKS` keep their values and lose the half of their doc
comments that argues about what a file might write.

## The readout

Three surfaces. This is the half that fixes the original bug.

### Log — four triggers

1. **Crossing.** The existing biome-change line gains the condition's
   **name**: `You cross into Null Sector — Dangling Reads.` Unclaimed ground
   names nothing extra, so Open Grid reads exactly as it does today.
2. **First meeting, once per session.** The condition's `description` — the
   prose that currently has no reader at all — the first time a run crosses
   into that condition. Latched in a session-only resource, **not saved**: a
   reload re-announces, and that is cheaper than a save field for flavour
   text. `resources::RunFeats` is the precedent for session-only state.
3. **Weather arrives.** On the tick that crosses an epoch boundary *while the
   player stands in that biome*, if the new epoch is live. Carries the
   event's description, since it fires rarely enough to afford prose.
4. **Weather clears.** The same boundary, the other way.

Triggers 3 and 4 fire only for the biome under the player, so the other four
biomes turning over is silent. Deriving them from the boundary crossing
rather than from stored state is what makes a reload mid-epoch not
re-announce.

### HUD — the map pane's top-left border strip

Today that strip reads the static label `SECTOR MAP` and carries no
information. It becomes the live ground readout, drawn through
`strip::fitting` as **two segments in priority order**: the weather first,
the ground second. `fitting` keeps the longest prefix that fits, so a narrow
window drops the ground detail and keeps the news.

`THREAT` keeps the top-right mount unchanged, and **the layout does not
move.** The map pane's bottom border reads as free — `map_frame.rs`'s own doc
says it "carries nothing" — but it is not: `pane_gap` is `m.gap.max(clearance)`,
sized for exactly the vitals strip reaching *up* from the log pane's top
border, so a second strip reaching *down* into that gap needs twice the
clearance and costs map height. Using a mount that is already paid for is
what keeps this change out of the layout census.

Colour: live weather takes `palette::ATTENTION`, the ground name takes
`palette::LABEL`. **Not `THREAT`** — that role is reserved for inbound harm
and is what `fx.rs` paints a raid's flash with; a second meaning on it makes
both unreadable.

The pane loses its title. That is a straight gain — the log pane's border
already carries vitals rather than a title, and `SECTOR MAP` names something
the player cannot mistake.

### Notification — one new variant

`NotificationKind::FirstStatic`, tutorial group, `Repeat::OnceEver`.

**Raised at one site**: the `game/turn.rs` hook, on the step where
`terrain_effect` resolves with a live event on the destination tile — the
same place the effect is applied, so "you were told" and "it happened to you"
cannot come apart. Not on the epoch boundary, which fires for a player who
may be standing three biomes away from anything.

The latch is `achievements::Profile::seen_notifications` in `profile.ron`,
not the save, so this costs no save change. `all()` goes from 7 to 8, which
the array length makes a compile error rather than an omission.

## Testing

Engine, fixtures from `crates/engine/src/tests/support.rs`:

- `for_biome` answers for the three claimed biomes and `None` for Open Grid,
  Data Void, Black Ice and Platform.
- Census over both `all()` arrays: every def has a non-empty name and
  description; every authored magnitude is inside its ceiling; nothing claims
  `Platform`. **This replaces the three load-time refusals.**
- `terrain_effect` is the identity at zone 1, on `Platform`, and on unclaimed
  ground with no weather.
- Ground and weather stack: the summed attrition lands as **one** bite, and
  the drag counts add.
- The fold is clamped: an authored pair that would exceed a ceiling is cut to
  it. Written against a forced pair, so it does not depend on shipped
  magnitudes staying where they are.
- Mitigation blunts the stacked bite — it goes through `apply_damage`.
- The party is untouched; the player alone takes it.
- Attrition that kills does not then start an ambush (phase 1's test, still
  the one lethal-edge interaction, now with weather in the sum).
- `static_at` is stable within an epoch and differs across epochs.
- `static_at` **draws no `GameRng`**: the stream is byte-identical across a
  call. This is the worldgen rule and the reason for the whole derivation.
- `static_at` gives the same answer either side of a save/load round trip at
  the same tick.
- Clear is reachable and live is reachable: over a run of epochs a claimed
  biome is both.
- The ambush multiplier reaches the roll: with a forced live event, a seeded
  run ambushes more often than the same seeded run with the event forced
  clear. Asserted as a difference, never as an absolute rate.
- The crossing line names the condition, and names nothing extra on unclaimed
  ground.
- The description fires once per session per condition, not per crossing.
- Arrival and clearing fire on the boundary, in the player's biome only, and
  a reload mid-epoch does not re-announce.
- `tutorials_latch_and_milestones_do_not` picks up `FirstStatic` without
  being edited.

GUI:

- Width census, through `paint::with_painter` so real text is measured: the
  widest shipped weather-plus-ground pair fits the map pane's top border
  beside the widest `THREAT` readout at 1280x720.
- `fitting` drops the ground segment before the weather segment.
- `the_tallest_shipped_notification_fits_its_screen` covers `FirstStatic`.

**`balance_sim` gates none of this**, the way it gates no walking today. The
suite proves the mechanism; every magnitude here is a play question.

## Documentation obligations

- Delete `assets/environment/README.md` with the directory.
- `CLAUDE.md` — the `ground_effect` seam line becomes `terrain_effect`, plus
  a line for the derivation and one for the fold. Rules only.
- `docs/seams.md` — the matching entries, carrying the argument.
- `.claude/skills/seams/` — the trap behind each new rule.
- The four doc comments citing `assets/environment/` as the absent-is-silent
  example, repointed at `assets/sectors/`.
- `CHANGELOG.md` — one section, and it must say a mod-facing content
  directory was removed.
- `docs/manual.md` and the root `README.md` stay carved out.

No `SAVE_FORMAT_VERSION` bump: everything here is derived or latched in
`profile.ron`.

## What this does not do

**Phase 3 — encounters** is only half-arrived. Weather biases the ambush
roll; biome on its own still does not.

**Phase 4 — combat reads its terrain** is untouched. `BattleState` still has
no notion of place.

Weather is biome-wide and either on or off. It is not a front that moves, it
does not vary in intensity, and two biomes never share an event's instance.

## Open questions

- **Is one epoch length right for every event?** The fixed epoch is what
  avoids a save field, and it is the first thing to revisit if a Thread Storm
  reads as overstaying.
- **Does the map pane want its title back?** Only play answers whether the
  strip reads as a readout or as a missing label.
- **Is the ambush multiplier felt, or just measured?** An ambush is already
  the one fight the player cannot route around; weather making it likelier
  may read as bad luck rather than as weather. The log line arriving first is
  what is supposed to make the difference, and that is the thing to watch.
