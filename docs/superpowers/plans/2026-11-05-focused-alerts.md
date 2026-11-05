# Focused alerts — plan

Spec: `docs/superpowers/specs/2026-11-05-focused-alerts-design.md`. Read
it first; its decisions are settled. Mockup (option C):
<https://claude.ai/artifact/8a4q8LNUc3xqy6wbtXbg8J>.

Worktree `.claude/worktrees/focused-alerts`, branch `worktree-focused-alerts`.

Load these before touching code:
- `.claude/rules/seams-notifications.md`, plus the `seams` skill's
  notifications section
- `.claude/rules/drawing-seam.md`
- `seams-hud.md` / `seams-screens.md` if a gui file pulls them

The `design-patterns` skill applies to the door change in 1.2.

How to work:
- Five phases, run in order by one implementer. TDD.
- Commit at each green step. Never push, never `git checkout` / `stash` /
  `reset` uncommitted work, stage explicit paths.
- Each phase gate: `cargo test -p <crate>` for the crates touched, plus
  `cargo clippy --workspace --all-targets` and `cargo fmt`. Phase 5 runs
  `cargo test --workspace`.
- No tuning or RNG change is expected. If one happens, run
  `cargo test -p feral-processes-engine balance_sim`.

## Phase 1: engine. Subject, door and opt-out

Files:
- `crates/engine/src/notifications.rs`
- `crates/engine/src/game/notify.rs`
- `crates/engine/src/achievements.rs` (`Profile`)
- the profile save/load path
- `crates/engine/src/tests/` (a new `focused_alerts.rs`, or the existing
  notification test file)

### 1.1 Types in `notifications.rs`

- `pub enum NotificationSubject { Entity(Entity), Tile((i32, i32)) }`
- `Notification` gains two fields:
  - `subject: Option<NotificationSubject>`
  - `silence_key: Option<&'static str>`. This is the kind's slug, `Some`
    only when the kind repeats.
- `NotificationKind::repeats(self) -> bool` is an **exhaustive match**, per
  the seam. It is false for `BaseAiAwakening`, `BaseAiFused`, and any
  other kind that fires at most once per run (onboarding complete,
  `SweepsBegin`), and true for the rest.
- Census test: every kind is classified, and a repeating kind's slug is
  unique.

### 1.2 The door, in `game/notify.rs`

- Thread an optional subject through `queue_notification`.
- `notify_filled`'s doc already warns against a wrapper per combination.
  Pick one shape: either one new door carrying the subject, or fold the
  extras into a small args struct. Don't add a wrapper per combination.
  Note the reason in a comment.
- `queue_notification` returns `false`, queueing nothing, when the kind's
  slug is in the profile's milestone "never" list.

### 1.3 The profile opt-out

- Add `#[serde(default)] notifications_never: Vec<String>` to `Profile`,
  or nest it under an existing sub-struct if `ftue` sets the precedent.
- Add `Game::silence_notification(slug)`. It goes through `Game` and then
  `flush_profile_writes`, the `ftue_never` precedent. It is **not** in
  `keep_settings_from`; the seam rule explains why.

### 1.4 Where the subject is on screen

Add `Game::subject_position(&NotificationSubject) -> Option<(i32, i32)>`:

- an entity goes through `watch_position`
- a tile returns `Some` only when the party is on the surface that holds
  it. Add a same-surface check: there is none today, and outpost keys carry
  no zone id, so find how the zone surface vs base space is told apart
  (`field_pos`, `stack.rs:383`).
- `None` everywhere else

### 1.5 Tests

- a silenced kind doesn't queue
- a non-repeating kind has no `silence_key`
- a subject on another surface resolves to `None`
- **profile save→load keeps `notifications_never`.** This needs a real
  save and load (memory `ron-round-trip-cannot-catch-a-skipped-field`).

## Phase 2: engine. Firing sites

Each site fires through the Phase 1 door with its subject. Each new kind
gets a `def` arm and a fired-by-site census entry.

### 2.1 The base AI storyline (`game/storyline.rs`)

- `announce_awakening`: `Entity` subject (the host's entity is already
  in hand).
- `announce_fusion`: `Entity(base_ai_host())`. The host is seated on the
  station, so it marks the station.

### 2.2 Program decompiled (`game/base/prison.rs::settle_jail_attempt`)

`Entity(prisoner)`.

### 2.3 Guidance "Your Base"

- The anchor tile is the subject.
- `FtueTrigger::Event` / the ftue queue path gains an optional subject,
  passed from `game/base/building.rs:323`.
- Guidance keeps its own D/T and gets no `silence_key`.

### 2.4 New kind `BaseStaffDowned`, templated `{name}`

- It fires only when a base staff member is downed at the base.
- First find every such path. Start at `game/base/tantrum.rs` (brawls),
  then `rg 'insert\(Downed\)|hp <= 0'` under `game/base/`.
- **Excluded:**
  - `bench_or_dissolve` callers (battle, sortie, off-screen siege)
  - `repair.rs::admit_the_badly_hurt`
- The subject is `Entity`.
- Test: a seeded brawl that downs a staff member queues it with that
  subject. Mutation-check it.

### 2.5 Outposts

- New kinds: `OutpostTierReached` (`{tier}`), `OutpostStale`,
  `OutpostDeclining`, `OutpostRaided` and `OutpostDark`. All are
  `Tile(key)`.
- Fire them beside the existing `post_alert` calls:
  - `game/outposts.rs::announce_outpost_trend` for stale and declining
  - `game/base/upkeep.rs:649` and `:658` for raided and dark
- The board entries stay.
- **Tier latch:**
  - Add `Outpost::announced_tier: Option<u32>`, unsaved, next to
    `announced`.
  - Seed it in `reseed_outpost_announcements`.
  - Fire only on a rise above the latch; founding seeds the latch without
    firing.
  - Find the tick that grows `growth`, and check `outposts::tier` after it.
- Tests:
  - a tier rise fires once
  - a reload doesn't re-fire
  - founding doesn't fire
  - a drop doesn't fire

### 2.6 Copy

- Plain language, no tick vocabulary, no occult naming.
- Every new body must pass the existing fits-on-screen census.
- Run `rg` over `assets/` and the copy for any stale "once" wording about
  program down.

## Phase 3: app-core. Keys, camera, watch

Files:
- `crates/app-core/src/app/lifecycle.rs` (`handle_notification_key`
  ~556)
- `app/inspection.rs` (`watch_center`, `start_watching`)
- `lib.rs`, only if a field is unavoidable. No new mode logic goes in
  `lib.rs`.
- the app-core tests: `tests/alerts.rs` or a new `tests/focused_alerts.rs`

### 3.1 Keys

- **W:** when the pending note's subject is an `Entity` and
  `watch_position` is `Some`, set `watching`, then advance to the next
  notification. Otherwise W does nothing.
- **D:** when `silence_key` is `Some`, call `game.silence_notification`,
  then advance. The existing guidance D path (`ftue_key`) stays first and
  unchanged.
- Key-hint case matches the handler case (memory
  `key-hint-case-is-the-handler-case`).

### 3.2 Camera

Add `App::notification_focus() -> Option<(i32, i32)>`. While
`Mode::Notification` has a subject, it returns
`game.subject_position(..)`. The gui centres on it instead of
`watch.unwrap_or(position)`. It is resolved every frame, as `watch_center`
is.

### 3.3 Tests

- W on an entity subject starts the watch and dismisses.
- W with no subject is inert.
- D silences, and the next fire of that kind doesn't queue.
- D on a guidance note still goes to the ftue path.

## Phase 4: gui. Docked panel and marker

Files:
- `crates/gui/src/render/notify.rs`
- `render/mod.rs` (the `draw_notification` call ~683)
- `render/base/playing.rs` (centre ~79 and ~282)
- `render/base/click_layout.rs` (`tile_origin_px`)

### 4.1 Layout and side

- `notify.rs` gets a `Dock { Centre, Left, Right }` layout input and
  `docked_panel_rect(w, h, side)`, about 38% wide and 88% tall, with a 2%
  margin.
- `fn dock_side(subject_px_x, screen_w) -> Side` is pure: the side
  opposite the subject's pre-centring screen x, and `Right` near the
  centre. The camera then centres the subject in the clear half, by
  offsetting the centre passed to `draw_surface_map`.

### 4.2 Marker

- Corner brackets around the subject's cell, plus a leader line from the
  panel's inner edge to the brackets (mockup C), in the note's own
  colour.
- The pulse comes from the existing fx clock.
- All drawing goes through `Painter`. If a needed operation is missing,
  stop and raise it rather than reaching past the seam.

### 4.3 Hints and fallback

- The hint line builds only the keys that apply: `[Enter] Continue`,
  `[W] Watch`, `[D] Don't show again`.
- The FTUE hint stays as it is.
- When `notification_focus()` is `None`, draw the centred panel exactly as
  today.

### 4.4 Tests

- **Census.** Extend
  `the_tallest_shipped_notification_fits_its_screen` (and the
  onboarding, completion and research siblings, where a kind can be
  focused) to measure every kind that can carry a subject against the
  docked panel at `smallest_panel()`'s window size. Mutation-check it by
  shrinking the dock fraction.
- `dock_side` unit tests.
- A headless "every line lands inside its panel" for the docked rect.

### 4.5 Screenshot check

- `cargo run -- --template stack --screenshot` can't trigger a
  notification by itself.
- If no dev save reaches one, a `--keys` path or a temporary dev template
  is acceptable. `capture` it if it's worth keeping.
- Read the PNG once.

## Phase 5: docs, gate, review

### 5.1 Seam update

- The "fixed 75% panel" rule gains its docked variant, and the
  `DownedProgram` line names `BaseStaffDowned` as its at-base twin.
- Update all three places, in the `seams` skill's order:
  1. the graph `seam:` entity
  2. the skill
  3. `.claude/rules/seams-notifications.md`

### 5.2 Docs

- `CHANGELOG.md`: an unversioned entry. The version is bumped at deploy.
- `docs/superpowers/INDEX.md`: a row pointing at the spec and this plan.
- Don't touch the README, the manual or TODO.

### 5.3 Gate

- `cargo test --workspace`, with `FERAL_DEV_NO_SIEGES` unset
- clippy and fmt clean

### 5.4 Final review

- A whole-branch review on opus.
- Give it the diff as a file.
- Point it at the spec and at the seams rule.
