# Generated flavour prose for the Stack — design

**Date:** 2026-08-10
**Status:** approved, not implemented
**Scope:** `crates/engine`, `crates/app-core`, `crates/gui`, plus a new asset
schema — so this earns the full spec-and-plan pipeline per `CLAUDE.md`'s
process-weight rule.
**Save format:** unchanged, and that is a design property rather than luck —
see "Stability is derived, not stored".

## The gap this exists to fix

The Stack is the game's most atmospheric-by-premise screen and its least
atmospheric in practice. The engine hands the renderer exactly one string for
the first-person view — `StackView::standing_on` (`game/stack_view.rs:268-293`),
a one-line key prompt like `"A link leads down  [>] descend"` — plus short
one-line `log` calls on arrival and on spending a feature. There is no prose
layer at all. A corridor with a phase-shifting door in it reads as a coloured
trapezoid and four words.

The target is environment and lore description that **alters no gameplay**:

> "You see a door to your right. The bottom-right corner is phase-shifting,
> leaking bits and sparking. Who knows what's on the other side?"

Two properties make it worth building rather than hardcoding: a given thing in
a given stack must read the same way every time you look at it, and a different
stack must read differently.

## What exists today

| Instrument | What it holds | Why it does not cover this |
|---|---|---|
| `StackView::standing_on` | One line, the cell underfoot | Doubles as a key prompt; centred and **unwrapped** |
| `Game::log` calls | One-line events | Scattered literals, no pool, no variety |
| `CrashLogDb` (`crash_logs.rs`) | 8 authored lines for rot | One flat unkeyed pool; no subject, no composition |
| `assets/*/README.md` schemas | Authored `description` fields | Per-item, not per-place |

`crash_logs.rs` is the near miss and the precedent. It is *pure flavour* loaded
from `assets/crash_logs/*.ron`, read by the hidden `Z` listen key, and its doc
comment already states the exact invariant this design needs: *"Which line a
given patch of rot reads is a property of the place — derived from the zone, the
depth and the cell, never from `resources::GameRng`."* This work generalises it
and absorbs it.

## Decisions

| Question | Answer |
|---|---|
| Runtime model | **None.** An LLM authors fragments at dev time; they ship as `.ron` and a deterministic index composes them. Settled — do not revisit. |
| Surfaces | Three *lengths* of one content: the `standing_on` row, log lines, an examine paragraph. |
| Scope | Stack only, behind a domain-agnostic subject key so surface/items are a file drop later. |
| What a description may read | The place only. The frame-arrival mood line is the one exception and may read depth and Trace band. |
| `crash_logs` | **Folded in and deleted.** `Z` draws the new composed prose. |
| Underground `x` | Describes a cell. The creature scan stops firing underground — a bug fix, below. |
| Bank authoring | Written as part of the work. |

### Why no runtime LLM

Engine and app-core are 100% synchronous, single-threaded, zero-network,
zero-async today; the one `std::thread` use in the workspace is the offline CEM
trainer. `frame()` in gui runs input, tick and draw in a single Bevy system, so
a 100 ms call anywhere in that chain stalls a frame. A live model would need a
worker thread, a channel, a poll each frame, non-deterministic text cached in
the save (a `SAVE_FORMAT_VERSION` bump), a hard dependency on the player running
a model server, and a first look that shows nothing until it arrives — all to
produce text that a bank of authored fragments produces instantly and offline.
The LLM's value here is *authoring*, and authoring happens once.

## Stability is derived, not stored

The requirement — "remember the description for that thing in that stack, and
only that stack" — is met by **derivation**, so nothing is cached, the save is
untouched, and a reload cannot disagree with what you read a minute ago.

`FrameSpec::rng_seed()` (`stack.rs:294`) already folds world seed + entrance
tile + depth through FNV-1a, and `world_seed` changes on every breach
(`game/zone.rs:491-496`). So two links in a sector are two different stacks, two
depths are two different frames, and a new zone is new text — all for free.

### Selection is a fold, not an `StdRng`

Pick by modulo of an FNV-1a fold, matching `crash_logs.rs:84-90`'s reasoning:
the answer then stays stable **across builds** as well as across a reload,
whereas `StdRng`'s output sequence is not guaranteed stable across a `rand`
upgrade. For flavour text, a silent reshuffle of every description on a
dependency bump is exactly the failure worth designing out.

One helper is added beside `rng_seed`:

```rust
/// Continues `rng_seed`'s FNV-1a fold with further words, so anything that
/// must be a stable property of a *cell* of a stack salts off the one scheme
/// rather than inventing a second that could collide with it.
pub(crate) fn salted(self, words: &[u64]) -> u64
```

Descriptions fold `[DESCRIPTION_SALT, x, y, length_tag, slot_tag]`. Each word is
multiplied through the FNV prime rather than XORed once, so two cells — and the
three lengths of one cell, and the three slots of one paragraph — diverge
robustly rather than by luck. `DESCRIPTION_SALT` must differ from the three
existing salts: `LAIR_SALT` `0x1A19_B055`, `ORPHAN_SALT` `0xDEAD_C0DE`
(`game/stack_features.rs:196, 422`) and `FALL_SALT` `0xFA11_1E15`
(`stack.rs:1165`). Those three are **not** migrated: each answers one question
per frame, a single XOR is sufficient there, and they are tested.

**No caller ever constructs a seed.** Entry points take a place and a length and
own the mixing internally, the same shape as `Game::orphan_species`
(`game/stack_features.rs:412`). A caller-supplied seed parameter is how two call
sites drift on *how* they salt, and how a third copy-pastes `LAIR_SALT`.

## Architecture

### `crates/engine/src/descriptions.rs` (new)

`CrashLogDb` generalised: pools keyed by a **subject string** instead of one
flat unkeyed pool, plus composition.

```rust
pub struct DescriptionDef {
    pub subject: String,                 // "stack.door", "stack.cache"
    #[serde(default)] pub variants: Vec<DescriptionVariant>,
}

pub struct DescriptionVariant {
    /// `None` is the fallback, used when no other variant on this subject
    /// matches. At most one per subject; a second is a load warning.
    #[serde(default)] pub when: Option<String>,   // "opened", "spent", "cleared"
    #[serde(default)] pub underfoot: Vec<String>, // standing_on; no {bearing}
    #[serde(default)] pub sighted:   Vec<String>, // one log line; may use {bearing}
    #[serde(default)] pub openers:   Vec<String>, // paragraph, sentence 1
    #[serde(default)] pub details:   Vec<String>, // sentence 2, "" allowed
    #[serde(default)] pub codas:     Vec<String>, // sentence 3, "" allowed
}
```

The subject is a `String` following the `ItemId` precedent (`items.rs`), and
that is the whole expansion seam: `"biome.forest"` or `"species.drone"` later is
a file drop with no code change. Nothing else in the design needs to be general,
and per `CLAUDE.md`'s DRY-but-not-prematurely rule nothing else should be.

**Three lengths, three contracts**, because they land in three places with
different room:

- **`underfoot`** replaces the descriptive clause of `standing_on`. Bounded in
  length: that row is centred, pixel-measured and **unwrapped**
  (`render/stack.rs:198-207`). No `{bearing}` — you are standing on it.
- **`sighted`** is one log line. The pane draws exactly one row per line with no
  wrapping (`render/base.rs:420`), so this is one sentence by construction
  rather than a truncated paragraph.
- **`openers` / `details` / `codas`** are the paragraph. Each fragment is a
  complete sentence, so the author controls the prose and the engine only joins
  non-empty parts with a space. Empty strings in `details`/`codas` let a draw
  legitimately produce something shorter. Fragments are grouped per subject and
  per variant so a door's details all make sense for a door — this is
  slot-composition, not free-for-all mad-libs.

`{bearing}` is the **only** substitution token, filled at the call site. The
fragments are a function of the place; the bearing is a function of live view
geometry and is recomputed every draw. They are composed, never stored together.

`load_dir` follows `CrashLogDb::load_dir` (`crash_logs.rs:52`): `.ron` only,
malformed files skipped with a returned warning and never a panic, and **pools
sorted by subject then by file id**. That sort is load-bearing and has already
cost this repo a bug once — `read_dir` returns entries in no defined order, so
without it the same cell reads a different line between runs, destroying the
one property the system exists to provide.

### Condition variants read the predicates that already exist

A looted cache reads differently from a sealed one. The `when` axis resolves
through the five predicates both views already consult — `cache_unopened`,
`seal_open`, `breakpoint_spent`, `orphan_present`, `lair_cleared` (all
`game/stack_features.rs`). Never a new record; `CLAUDE.md` pins this as the
two-halves rule.

### `relative_bearing` is called, never copied

`game/listen.rs:126` already computes `"ahead" | "behind" | "to your left" |
"to your right"` by dotting the offset against `Dir::delta` and
`Dir::right_delta` — the same rotation `view_cone` uses, with a doc comment
noting that reading the bearing off compass north instead is "the same mistake
with a plausible-looking answer". Both the examine paragraph and the sighting
line need it. Widen to `pub(crate)` and call it. `CLAUDE.md` is explicit that a
second consumer calls rather than copies.

## The four surfaces

**1. `standing_on`** (`game/stack_view.rs:268-293`). Keep the match arm
structure **exactly** as it is: arms returning `Some` draw their descriptive
clause from the bank and keep their key-prompt suffix verbatim
(`"…  [>] descend"`, `"…  [o] adopt"`); arms returning `None` stay `None`. Two
tests already assert `None` for a spent orphan (`tests/stack.rs:3608, 3801`) and
must keep passing untouched. Falls back to today's literal when the bank has no
entry, so deleting the asset directory leaves the game working — a mod's
prerogative, the same argument `crash_logs` makes.

**2a. First sighting.** `Game::remember_view` (`stack_view.rs:79`) already walks
the view cone and extends `FrameMemory::seen`. Diffing against `seen` *before*
the `extend` on line 107 yields the newly-sighted set for free, with no new save
state. Log one `sighted` line for the **most notable** newly-seen cell, capped
at one per call — a corridor opening onto four features must not push four rows
into a pane that shows a handful. Notability ranks unspent features above
terrain; spent features are not notable.

The load path must not announce: `restore_locale` calls `remember_view`, and a
save reloading into a corridor would replay sightings the player already read.
Split into `remember_view` (announces — every movement and turn path keeps
calling it unchanged) and a silent variant called from the load path only. One
site, pinned by a test.

**2b. Frame-arrival mood line.** One line from `Game::enter_frame`
(`game/stack.rs:322`) — the one spine every descent, ascent and fall goes
through — after `remember_view`. Subject `"stack.frame.arrival"`, with `when`
variants on depth band and Trace band. This is the single place a description
reads run state rather than only the place, and it is a separate subject so that
exception stays visible.

**3. Examine paragraph.** `x` + direction underground, read in **view space**:
up = ahead, left/right = your left/right, down = the cell underfoot. Absolute
compass directions are wrong in a first-person view. Describes the nearest
notable cell along that ray within the view cone, falling back to the
`"stack.floor"` corridor subject when the ray holds nothing notable, so the key
always answers. Drawn as a popup through the existing `wrap_text`
(`render/popup.rs:479`) in the `draw_item_describe` shape
(`render/inventory.rs:229-264`) — already the repo's one prose-on-screen pattern
and its only wrap helper.

**4. `Z` listen.** `Game::listen`'s rot branch (`game/listen.rs:79`) returns the
composed paragraph for the cell instead of a single `CrashLogDb` line. The
bearing branch (`listen.rs:63-73`) is untouched, and `Z` still costs a turn and
Trace either way.

## The bug this fixes

`Game::find_target_in_direction` (`game/inspection.rs:48`) scans creatures by
world `Position`, and underground `Position` is pinned to the surface entrance
tile. So `x` + direction in a corridor scans the **surface** around where the
party descended, and can open a manifest for a wild program four frames
overhead, reported as lying "that way".

This is the defect the structures half was already fixed for. The doc comment at
`inspection.rs:42-47` excludes structures because "an unguarded scan reports the
base four frames overhead as being off to your east", then keeps creatures on
the grounds that "finding one is already how the inspector behaves down there" —
which describes the behaviour rather than defending it. The test that pins it
(`the_inspector_offers_no_structure_while_the_party_is_underground`,
`tests/inspection.rs:1026`) is a test about structures.

It is the same class as the `find_target_in_direction` entry in `CLAUDE.md`: the
test for whether a `Position` reader needs the guard is not "does it act" but
"does it claim something about where the party is". A creature scan reporting a
surface drone as being to your left claims exactly that.

Fix: skip the creature scan underground, as the structure scan already is, and
rewrite the doc comment so both halves are excluded for one stated reason.
`CLAUDE.md`'s entry on that function is updated in the same change.

## Absorbing `crash_logs`

Migrate `assets/crash_logs/*.ron` (4 files, 8 lines) into the bank as subjects
`"stack.fault"` and `"stack.corruption"`; delete `crates/engine/src/crash_logs.rs`
and `CrashLogDb`; drop it from `AssetDbs` / `load_asset_dbs`
(`game/lifecycle.rs:1021, 1040`) and both constructors' `insert_resource`
blocks; repoint `Game::listen`. Existing lines keep their text and gain
composition.

`crates/engine/EASTER_EGGS.md` and
`docs/superpowers/archive/specs/2026-08-06-easter-eggs-design.md` are updated in the same
change: `Z` still works, still costs a turn and Trace, and still says the thing
the frame map cannot — only what it reads now comes from elsewhere.

## Files, in dependency order

1. `crates/engine/src/stack.rs` — `FrameSpec::salted`.
2. `crates/engine/src/descriptions.rs` — **new**. `DescriptionDb`, `load_dir`,
   variant matching, composition.
3. `crates/engine/src/lib.rs` — module declaration and re-exports.
4. `crates/engine/src/crash_logs.rs` — **deleted**.
5. `crates/engine/src/game/lifecycle.rs` — swap the db in `AssetDbs` /
   `load_asset_dbs` and both `insert_resource` blocks.
6. `crates/engine/src/game/listen.rs` — widen `relative_bearing`; repoint the rot
   branch.
7. `crates/engine/src/game/stack_view.rs` — `standing_on` draws from the bank;
   `remember_view` announce/silent split and the newly-seen diff.
8. `crates/engine/src/game/stack.rs` — mood line in `enter_frame`.
9. `crates/engine/src/game/inspection.rs` — skip the creature scan underground;
   rewrite the doc comment.
10. The `Game` entry points — describe underfoot, describe a view-space
    direction, describe a cell. Each takes a place, never a seed.
11. `crates/engine/src/views.rs` — whatever the examine popup needs (likely a
    `String`; no `StackView` change if `standing_on` keeps its type).
12. `crates/app-core/src/app/inspection.rs` — underground branch in
    `handle_inspect_direction_key`.
13. `crates/app-core/src/lib.rs` — the new `Mode` variant, doc-commented per
    repo convention.
14. `crates/gui/src/render/` — draw the popup; dispatch arm in `mod.rs`.
15. `assets/descriptions/*.ron` + `assets/descriptions/README.md` — the bank and
    its schema doc, required by `CLAUDE.md` whenever an asset schema is
    introduced. The README also carries the authoring prompt, so the bank is
    extensible by a person or a local model without reading this spec.
16. `assets/crash_logs/` — **deleted**.
17. `CLAUDE.md` (+ its `AGENTS.md` twin) — the `find_target_in_direction` entry,
    and a new load-bearing-seams entry for the description bank.
18. `CHANGELOG.md` and the workspace version bump, at the merge. Not
    `docs/manual.md` and not the root `README.md` — both are carved out.

Extend, never copy: `remember_view`, `enter_frame`, `relative_bearing`, and the
five spent-state predicates.

## Bank content

Roughly 10–14 subject files covering the describable cell kinds —
`stack.door`, `stack.sealed_door`, `stack.cache`, `stack.lair`, `stack.orphan`,
`stack.breakpoint`, `stack.link_up`, `stack.link_down`, `stack.fault`,
`stack.corruption`, `stack.floor`, `stack.frame.arrival` — each with condition
variants where a spent state reads differently, and enough fragments per slot
that two frames genuinely differ rather than differing in principle.

Voice matches the existing lines: dry, technical, slightly elegiac. `CellKind::
Rock` has no subject; it is the default reading of a blocked corridor and the
thing everything else is distinguished against.

Prose respects the standing no-occult-naming rule — no daemon, demon, ghost,
wraith or phantom.

## Tests

Failing test first, per `CLAUDE.md`. Nothing here reads `GameRng`, so nothing is
flaky by construction; confirm with a grep for `GameRng` in the new module.

- `a_malformed_description_file_is_skipped_with_a_warning` — mirrors the
  existing `ItemDb` / `CrashLogDb` tests.
- `every_describable_cell_kind_has_a_shipped_bank_entry` — a census over the real
  `assets/descriptions/`, asserting an explicit subject list resolves for the
  fallback variant and for every authored condition. Same shape as
  `every_biome_a_stack_link_can_open_in_fields_a_boss`, so a content edit that
  empties a pool fails honestly instead of shipping silence.
- `the_same_cell_reads_the_same_description_twice` — determinism.
- `two_different_frames_describe_the_same_cell_differently` — the core property.
  Must use a subject with several fragments, or the assertion is vacuous.
- `a_description_survives_a_save_and_load` — mirrors
  `the_species_a_frame_offers_survives_a_save_and_load`; proves derivation
  delivers the requirement with no new save state.
- `an_underfoot_line_fits_the_standing_on_row` — max length, and the key-prompt
  suffixes still present.
- `a_spent_orphan_still_offers_nothing_underfoot` — the `None` arms stay `None`.
- `a_newly_seen_notable_cell_logs_once_per_move` — and a step revealing nothing
  new logs nothing.
- `loading_a_save_announces_no_sightings` — pins the announce/silent split.
- `a_frame_arrival_logs_a_mood_line_and_a_step_does_not`.
- `the_inspector_scans_no_creature_while_the_party_is_underground` — the bug fix,
  beside the existing structures test.
- `listening_on_rot_reads_the_description_bank` — the fold.

`crash_logs.rs`'s tests are deleted with it; any still asserting something true
of the new bank are ported rather than dropped.

## Verification

```sh
cargo test -p feral-processes-engine descriptions   # iterate here (~3s)
cargo test --workspace                              # the gate
cargo clippy --workspace && cargo fmt
```

Then play it, which a green suite is not a substitute for:

```sh
FERAL_DEV_REVEAL=1 cargo run -- --template stack
```

Walk a frame and read all four surfaces on screen: the `standing_on` row at the
bottom of the corridor view (confirm it does not overflow — it is unwrapped),
sighting lines arriving in the log at a readable rate rather than one per step,
`x` in each of the four directions, and `Z` on a rotten cell. Descend and confirm
the mood line fires once per frame and not once per step. Then reload the save
and confirm the same door reads the same way.

`balance_sim` is irrelevant here — nothing in this touches a formula.

## What this design deliberately does not do

- **No runtime model, no network, no thread, no async.** Settled above.
- **No cache and no save bump.** Derivation makes both unnecessary; adding either
  later would be the signal that something has started reading run state it
  shouldn't.
- **No generic slot engine.** Fixed `opener` / `detail` / `coda` slots are enough
  for one domain with one caller. Widen when the surface subjects actually land.
- **No `standing_on` re-architecture.** The row stays one line and stays a key
  prompt; only its descriptive clause is drawn from the bank.
- **No prose for `CellKind::Rock`.** See "Bank content".
