# Full-screen notifications

**Status:** approved, not implemented
**Date:** 2026-08-29

Every big moment in a run currently lands as a line in the log pane. A zone
breach, a first descent, an achievement earned and a machine finishing a
cycle all scroll past at the same weight, in a pane the player is often not
looking at. This gives the handful of moments that are genuinely
*irreversible* — or that the player has never seen before — a screen of
their own: full-bleed, one at a time, any key to dismiss.

The feature is small in mechanism and almost entirely content. That split is
the design: **the queue, the door and the screen are Rust; every word the
player reads is a `.ron` file.** A fifth tutorial must cost one asset file
and one line at a call site, with no enum arm, no match, no renderer change
and no test fixture. That is the acceptance criterion the implementation is
held to, and it is stated here because it is the thing that decays first.

## What this is not

Not a toast, not a corner popup, not a history screen. There is exactly one
notification on screen at a time and it owns the whole window; the log line
at each site stays, so nothing here is the *only* record of anything.

## The catalogue

`assets/notifications/*.ron`, loaded by `NotificationDb`, which follows
`NeedDb::load_dir` line for line — which in turn follows `MemoryDb`'s. That
is not a stylistic note: the three properties it inherits are load-bearing.

- An **absent directory loads silently empty**. Deleting
  `assets/notifications/` restores the pre-notification game exactly, the
  same supported way deleting `assets/needs/` or `assets/memories/` does.
- A **malformed file is skipped with a warning**, costing the game that one
  notification and nothing else, rather than stopping a player reaching the
  main menu over somebody else's mod.
- `paths.sort()` before parsing, so two files claiming one id resolve the
  same way every run, and `iter()` is **sorted by id** because every caller
  walks it.

**Never gate a system, a trigger or the screen on the db being non-empty.**
That is how a property like this holds by accident at one site and lapses at
another.

```ron
(
    id: "tutorial_first_descent",
    title: "The Stack",
    body: "Below the on-ramp the ground stops being a map and starts \
           being a corridor...",
    sprite: "stack_entrance",
    glyph: '>',
    color: Cyan,
    repeat: OnceEver,
)
```

`NotificationId` is a string newtype following `NeedId` — a private `String`
with an `as_str()`, never an enum, or a mod cannot add one.

`sprite` is optional and every field but `id`, `title` and `body` is
`#[serde(default)]`, so an existing mod file keeps parsing when a field is
added later.

### The art is optional, and that is the sprite seam's own rule

`sprite` and `glyph` are **not** two things drawn together. The
`Painter::sprite` operation returns `false` for a name the table has nothing
under, and the caller draws its glyph instead — a sprite **substitutes** for
a glyph and never draws beside it. This screen inherits that rule unchanged.

`assets/sprites/` ships exactly one file today (`player.png`), so every
shipped notification will in fact draw its glyph. That is the design
working, not a gap to close before shipping: the day art exists, dropping a
`.png` in with a matching name is the whole change.

`color` is a `GlyphColor`, which already derives `Serialize`/`Deserialize`,
and is resolved through `hud::palette::glyph` — the one table a content hue
is drawn from. The renderer does not invent a colour for this screen.

## The queue and the one door

`resources::Notifications` holds a `VecDeque<Notification>` and is
**not saved**, `resources::RunFeats`' precedent. Nothing is lost by that: a
notification is news about a moment, and a player reloading has already left
the moment.

`Game::notify(&NotificationId)` is the **one door**. It:

1. resolves the def — an id no file defines is a **returned refusal**, since
   the engine has no runtime warning channel (`Game::remember`'s rule);
2. checks the latch (below);
3. pushes a **resolved** `Notification` value onto the queue.

**Resolved, not an id.** The queued value carries the finished title, body,
glyph, colour and sprite name. This is `ActiveContract`'s rule and
`Sortie`'s: the whole resolved def travels, so a `.ron` edited or deleted
between the push and the draw cannot strand or silently rewrite a
notification already queued.

`notify` draws **no RNG** and writes **no log line** — the site that fires it
already logs, and a second line would double every announcement in the
history screen.

`Game::take_notification() -> Option<Notification>` pops one.
`Game::take_effects`' counterpart, and like it, a frontend that draws none
must still call it or the queue grows for the life of the run.

## Two repeat policies, and why not three

```rust
enum Repeat { Always, OnceEver }
```

The obvious third — once per run — was **considered and rejected**. Nothing
in the shipped content wants it, and its latch would have to live on the
session resource, which is not saved: "once per run" would quietly mean
"once per session" and re-fire on every reload. A name that lies is worse
than a missing policy. It is an additive enum variant the day something
actually needs it, and adding it then costs one arm.

`OnceEver`'s latch is `achievements::Profile::seen_notifications`, a
`#[serde(default)] Vec<NotificationId>`, written through one method,
`Profile::see(&NotificationId) -> bool` — `true` on a first sighting, `false`
and no change on a repeat, deliberately the same shape as `Profile::record`.

Three consequences fall out of putting it there, all of them wanted:

- **The profile is not the save file**, so this touches no
  `SAVE_FORMAT_VERSION`. A tutorial seen in one run stays seen in the next,
  which is what "tutorial" means.
- It is written through the **existing** `take_pending_profile_writes`
  channel, which `App::flush_profile_writes` drains. That sits *below*
  `after_tick`'s `in_arena()` early return, so **"an arena session touches
  no disk" holds by construction** rather than needing a new check here.
  Do not add one; the omission is the enforcement.
- `Profile::record`'s existing shape — a first write returns `true`, a
  re-write returns `false` — is the same shape the latch needs, so the
  "has this been seen" question has one answer in one place.

## The sources

Six sites, each one line. All six are `Game` methods that are already the
one door to the thing they announce, which is why no new seam is
introduced.

| Site | id | repeat |
|---|---|---|
| `place_structure`, the `founding` branch | `tutorial_base_founding` | `OnceEver` |
| `Game::descend_to` | `tutorial_first_descent` | `OnceEver` |
| `raid_check`, where the sweep opens | `tutorial_first_raid` | `OnceEver` |
| `queue_work_order`, on `Ok` | `tutorial_first_work_order` | `OnceEver` |
| `Game::enter_next_zone` | `milestone_breach` | `Always` |
| `Game::complete_contract` | `milestone_contract` | `Always` |

`complete_contract` and not `contract_system`: both the polled path and the
delivery path already route through it, so it is the one door and a third
completion path added later gets its notification for free.

The four tutorials fire unconditionally at their site and are made
once-only by `Repeat::OnceEver` alone. There is deliberately **no `if
first_time` check in Rust** — that would put the policy in two places and
let the two disagree.

### Achievements are a second source, not a second door

`achievement_system` already distinguishes a first earn from a re-earn:
`Profile::record` returns the bool. On a first earn it builds a
`Notification` from **the achievement def's own `name` and `description`**
and pushes it through the same queue.

A call, not a copy. Authoring a second `.ron` file per achievement carrying
the same prose is exactly the pattern that drifts — the copy that drifts is
the one nobody runs. Achievements therefore need no rows in the catalogue at
all, and a new achievement gets a notification with no notification work.

### Missions

TODO 53 (mission tracking) does not exist yet. When it lands it calls
`notify` at its own completion door, exactly as contracts do. Nothing here
anticipates it.

## The screen

### app-core

`Mode::Notification`, with `App::pending_notification` as the subject — one
writer, following `GearInspect`'s rule that the subject field has exactly
one writer or three distinct failures are inherited.

**Drained in `after_tick`, and only when `mode == Mode::Playing`.** That
single condition is the whole of "next safe moment": a fight, any picker,
text entry and the excavation plan are all untouched, and anything queued
while one of them is open waits until the player is back on the map. A
notification therefore never eats a keypress in the middle of an unrelated
flow, which is the failure the timing rule exists to prevent.

Any key dismisses. If the queue is non-empty the next one takes the screen
immediately; otherwise the mode returns to `Mode::Playing`. Esc is not
special — this is `Mode::CellDescribe`'s idiom, a page with nothing to page
through.

`Mode::is_battle`'s exhaustive match makes the new variant a **compile
error** until it is classified. That is the gate working; do not add a `_ =>`
arm to quiet it.

### gui

`crates/gui/src/render/notify.rs`, drawing through `Painter` alone — the
drawing seam is not widened and no new `Painter` operation is needed.

Layout, top to bottom on a full-bleed dimming rect: the sprite (or its glyph
fallback) drawn large and centred, the title, the body wrapped through
`text::wrap` in the engine, and a footer line.

Two consequences of it drawing **no popup**:

- It joins `Battle`, `BattleResult`, `FrameMap` and `FieldCastCell` in
  `needs_status_banner`, so a refusal reaches a surface.
- The refusal census `every_screen_draws_a_refusal_exactly_once` drives
  every `Mode` through `draw` and counts what was painted, so forgetting it
  fails the build rather than shipping a screen that can swallow a refusal.

**The screen has no scroll**, which makes its height a layout constraint —
the memories page's rule. `the_tallest_notification_fits_its_screen` is what
says the shipped catalogue fits at 1280x720, and raising the body length
past what fits means giving the screen a scroll rather than trimming the
test.

Wrapping in the engine and not the renderer: a read-only screen's row count
is owned by app-core, and a per-row transform in gui opens the screen on
rows that are not drawn.

## Testing

TDD throughout, failing test first, per step. The load-bearing ones:

- **An empty and an absent catalogue are both supported installs.** No
  trigger, no system and no screen is gated on the db. Asserted at both
  ends, `MemoryDb`'s property.
- **A notification queued mid-battle is still there when the player reaches
  the map.** Asserts the queue, not just the drain — a test that fires one
  from `Mode::Playing` passes against a design with no queue at all.
- **`OnceEver` survives a reload; `Always` does not latch.** The first half
  needs a real profile round trip, not just a same-session second call.
- **An arena session writes no profile**, asserted on the *file*, matching
  the three omission tests that already do. This is the one that costs real
  money if it regresses.
- **The tallest shipped notification fits at 1280x720**, measured through a
  real draw with `paint::with_painter`.
- **A pairing census in `tests/assets.rs`**, `MEMORY_TRIGGERS`' rule: every
  shipped def id is paired with the Rust site that fires it, and a def
  shipped with nothing firing it fails the build. There is no `trigger:`
  field to derive this from — the catalogue is data and the triggers are
  Rust, so the census is the whole rule.
- **The achievement path quotes the achievement's own prose**, so a test
  that changes an achievement's `description` and reads it back off the
  notification is what stops someone "tidying" it into a duplicate file.

## Deliberately out of scope

- **A `trigger:` field in the def.** Fully data-driven firing was
  considered. A tutorial's natural triggers are app-core state the engine
  cannot observe ("the first time you open the base menu"), so the
  vocabulary would grow without end. `Game::notify(id)` lets a Rust site
  name any trigger point with no new variant, and a `trigger:` field can be
  added later behind `#[serde(default)]` without touching a single shipped
  file.
- **A sound cue.** `SwingOutcome`'s cue machinery is battle-only and this is
  not a swing.
- **A notification history screen.** The log line at each site stays and
  `Mode::History` already shows it.
- **A saved queue.** Session-only, stated above.
- **Any change to the log lines that already fire at the six sites.** The
  notification is additive; nothing is moved out of the log to pay for it.

## Files

| Crate | File | Change |
|---|---|---|
| engine | `src/notifications.rs` | new — `NotificationId`, `NotificationDef`, `Repeat`, `NotificationDb`, `Notification` |
| engine | `src/resources.rs` | new `Notifications` resource |
| engine | `src/game/lifecycle.rs` | load the db beside `NeedDb`/`SortieDb` (~2046) |
| engine | `src/game/notify.rs` | new — `Game::notify`, `Game::take_notification` |
| engine | `src/achievements.rs` | `Profile::seen_notifications` |
| engine | `src/game/achievements.rs` | push on a first earn |
| engine | 6 call sites | one line each, per the table above |
| engine | `tests/assets.rs` | the pairing census |
| app-core | `src/lib.rs` | `Mode::Notification`, `is_battle` arm |
| app-core | `src/app/lifecycle.rs` | drain in `after_tick` |
| app-core | `src/app/input.rs` | any-key dismiss |
| gui | `src/render/notify.rs` | new — the screen |
| gui | `src/render/mod.rs` | `draw` arm, `needs_status_banner` |
| assets | `assets/notifications/` | 6 defs + `README.md` schema reference |
