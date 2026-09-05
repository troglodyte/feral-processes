# Settlement Aid Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the top of the standing ladder teeth — a passive garrison that
softens raids, a gifted program you ask a town for, and relay fast-travel
between an Allied town and your anchor.

**Architecture:** Three new named queries on `Standing`, exactly the shape the
four shipped ones use. The garrison is one clamped term added to
`Game::total_raid_defense`. The gift and the travel are engine doors reached by
two new uppercase keys on screens that already exist — **no new `Mode`
variant**. One additive save field.

**Tech Stack:** Rust, `bevy_ecs` 0.19, three of the four workspace crates
(`engine`, `app-core`, `gui`).

**Spec:** `docs/superpowers/specs/2026-09-05-settlement-aid-design.md` — read it
first; this plan argues from it and does not restate its decisions.

**Branch:** `feat/settlement-aid`, already created, with the spec committed.

## Global Constraints

Copied from `CLAUDE.md` and the spec. Every task's requirements include these.

- **TDD.** Failing test first, minimal implementation, green, commit. A commit
  per green step.
- **Gates after every change:** `cargo fmt`, `cargo clippy --workspace` (fix
  warnings, never silence them). `cargo test --workspace` before any task is
  called done. Iterate with `cargo test -p feral-processes-engine <name>`.
- **`cargo`'s exit code is lost through a pipe.** Never `| tail` or `| grep` a
  test run — redirect to a file and read it, or a failing suite reports success.
- **No `SAVE_FORMAT_VERSION` bump.** The one new field is additive behind
  `#[serde(default)]`. If you find yourself wanting a bump, stop and say so.
- **Named constants only** — every number goes in `crates/engine/src/tuning.rs`
  with a doc comment, never inline in a formula.
- **No new `Mode` variant.** `ALL_MODES` is hand-written and the draw match ends
  in `_ => {}`, so a new variant ships as a blank screen, and its length is a
  semantic merge conflict. Both verbs hang off existing screens.
- **Uppercase keys only.** Lowercase letters are row selectors everywhere in
  this game.
- **Every refusal lands before anything is spent, asserted _per refusal_.** One
  test over one path passes against every other path that never spends anyway.
- **The gift must not draw `resources::GameRng`.** It seeds a local `StdRng`,
  `pick_lair_species`' rule, so a reload cannot reroll it and it cannot shift
  the seeded stream.
- **Do not run the game.** There is no display in this environment; `cargo run`
  refuses. Every task ships on a green suite and zero screen time.

---

## File Structure

**Engine**

- `crates/engine/src/settlements/relations.rs` — the three new queries and
  their censuses. Everything about *what a band means* lives here; nothing
  about where it is read.
- `crates/engine/src/tuning.rs` — the eight new constants, in the existing
  `SETTLEMENT_*` block near `SETTLEMENT_BOARD_SALT` (~line 3443).
- `crates/engine/src/game/base/upkeep.rs` — `total_raid_defense` gains its
  second term (~line 230).
- `crates/engine/src/game/settlement_relations.rs` — the gift door and the two
  travel doors. This module already holds "the verbs" while `relations.rs`
  holds "the shape"; the split is stated in its own module doc and must hold.
- `crates/engine/src/game/spawning.rs` — widen `standable_near` (line 1013)
  from `fn` to `pub(crate) fn`. No other change.
- `crates/engine/src/views.rs` — `SettlementView` (line 1881) gains the aid
  fields.
- `crates/engine/src/game/inspection.rs` — `settlement_report` (line 809) fills
  them.
- `crates/engine/src/save.rs` — nothing to add; `Relation` is serialized whole
  inside `resources::Standings`. **Verify this rather than assuming it.**
- `crates/engine/src/tests/settlement_relations.rs` — every engine test in this
  plan. The four shipped settlement test files split by subsystem
  (`settlements.rs` placement, `settlement_market.rs`, `settlement_boards.rs`,
  `settlement_relations.rs` standing); aid is a consequence of standing, so all
  three features test here, garrison included.

**app-core**

- `crates/app-core/src/app/inspection.rs` — `handle_settlement_key` (line 141)
  gains `[G]` and `[T]`.
- `crates/app-core/src/app/dispatch.rs` — `handle_dispatch_key` (line 123)
  gains `[T]`.
- `crates/app-core/src/lib.rs` — re-export anything new the gui needs.
- `crates/app-core/src/tests/dispatch.rs` and the settlement test file — key
  tests.

**gui**

- `crates/gui/src/render/settlement.rs` — `settlement_page_rows` (line 42)
  gains the aid rows.

**Docs (final task)**

- `docs/seams.md`, `.claude/skills/seams/references/ground.md`, `CLAUDE.md`,
  `CHANGELOG.md`.

---

### Task 1: The three named queries

**Files:**
- Modify: `crates/engine/src/settlements/relations.rs`
- Modify: `crates/engine/src/tuning.rs`
- Test: `crates/engine/src/settlements/relations.rs` (its own `mod tests`, where
  the four existing censuses live)

**Interfaces:**
- Consumes: nothing.
- Produces:
  ```rust
  impl Standing {
      pub fn garrison_defense(self) -> u32;
      pub fn gifts_programs(self) -> bool;
      pub fn hosts_a_relay(self) -> bool;
  }
  ```

Constants to add to `tuning.rs`, each with a doc comment saying it is an
unmeasured guess and naming the play question behind it:

| Constant | Type | Meaning |
| --- | --- | --- |
| `SETTLEMENT_WARM_GARRISON` | `u32` | a Warm town's contribution to raid defense |
| `SETTLEMENT_ALLIED_GARRISON` | `u32` | an Allied town's, strictly greater |
| `SETTLEMENT_GARRISON_RADIUS` | `i32` | Chebyshev tiles from the anchor a town must be inside to garrison |
| `SETTLEMENT_GARRISON_MAX` | `u32` | ceiling on the **settlement half** of raid defense |
| `SETTLEMENT_GIFT_COOLDOWN_TICKS` | `u64` | between gifts from one town |
| `SETTLEMENT_GIFT_STAT_MULT` | `f32` | the gift's `stat_mult` — decision 5 says labour, not power |
| `SETTLEMENT_GIFT_SPECIALTY_MULT` | `f32` | multiplies the above when the town's `Specialty` is `Programs` |
| `SETTLEMENT_TRAVEL_TICKS_PER_TILE` | `u64` | ticks charged per Chebyshev tile travelled |

Pick `SETTLEMENT_GARRISON_MAX` against `tuning::RAID_DAMAGE` — read that
constant and choose a value that leaves a raid meaningfully damaging with every
neighbour Allied. Say in the doc comment what fraction of `RAID_DAMAGE` you
chose and why.

- [ ] **Step 1: Write the three failing censuses.** Model each on the existing
  `every_standing_band_answers_whether_it_preys_on_routes` — an exhaustive
  five-band walk, asserting the answer per band rather than only that it
  compiles. Specifically:
  - `every_standing_band_answers_what_garrison_it_sends`: zero through
    `Neutral`, non-zero at `Warm`, strictly greater at `Allied`, and the ladder
    never descends across the whole five (the `windows(2)` shape
    `every_standing_band_answers_how_many_jobs_it_posts` uses).
  - `every_standing_band_answers_whether_it_gifts_programs`: `Allied` alone, and
    a band that gifts never also `refuses_service`.
  - `every_standing_band_answers_whether_it_hosts_a_relay`: `Allied` alone, and
    the same non-contradiction assertion.
- [ ] **Step 2: Run them and watch them fail to compile** — the methods do not
  exist. `cargo test -p feral-processes-engine relations 2>&1 > /tmp/t.log`,
  then read the file.
- [ ] **Step 3: Add the constants** to `tuning.rs`'s `SETTLEMENT_*` block.
- [ ] **Step 4: Write the three methods.** Each an exhaustive `match` on
  `Standing` — `cell_mark`'s rule, so a sixth band fails to compile. Doc-comment
  each the way the four existing queries are: say what it means, say it is
  exhaustive and why, and for `garrison_defense` say why it is a magnitude
  rather than a third boolean.
- [ ] **Step 5: Green.** Same command; then `cargo fmt` and
  `cargo clippy --workspace`.
- [ ] **Step 6: Commit.** `feat(settlements): the three aid queries on the band`

---

### Task 2: The garrison term

**Files:**
- Modify: `crates/engine/src/game/base/upkeep.rs` (`total_raid_defense`, ~230)
- Test: `crates/engine/src/tests/settlement_relations.rs`

**Interfaces:**
- Consumes: `Standing::garrison_defense`, `SETTLEMENT_GARRISON_RADIUS`,
  `SETTLEMENT_GARRISON_MAX` from Task 1.
- Produces: no new public signature. `total_raid_defense` keeps
  `pub(crate) fn total_raid_defense(&self) -> u32`, and `raid_defense_active`
  keeps working unchanged — it is `total_raid_defense() > 0`, so a garrison with
  no structures now reads as an active shield, which is correct and is worth a
  test of its own.

The term: for each entry in `resources::Settlements`, band its key through
`Game::standing_band`, keep those whose `tile` is within
`SETTLEMENT_GARRISON_RADIUS` (Chebyshev) of `Game::anchor_position()`, sum their
`garrison_defense()`, then **clamp that sum alone** with `.min(SETTLEMENT_GARRISON_MAX)`
before adding it to the structure sum.

The clamp order is the whole point and is easy to get backwards:

```rust
let structures: u32 = /* the existing sum, unchanged */;
let garrison: u32 = /* summed over towns */;
structures + garrison.min(SETTLEMENT_GARRISON_MAX)
```

Clamping the total instead would cap the player's own shield network, which is
a different feature and a regression.

`anchor_position()` (`game/stack.rs:348`) returns `Option`; no anchor means no
garrison, not a panic.

- [ ] **Step 1: Write the failing tests.**
  - an Allied town inside the radius raises `total_raid_defense` above the
    structures-only figure
  - the same town outside the radius does not
  - a `Warm` town contributes, and less than the Allied one
  - a `Neutral` town contributes nothing
  - many Allied towns cannot push the settlement half past
    `SETTLEMENT_GARRISON_MAX` — build enough towns that the uncapped sum would
    exceed it, and assert the exact capped figure, not `<=`
  - **the clamp does not touch the structure half**: with structures summing
    above `SETTLEMENT_GARRISON_MAX` and no towns at all, the answer is unchanged
    from today. This is the test that fails if the clamp is applied to the total.
  Use the existing settlement test fixtures (whatever `tests/dispatch.rs`'s
  `register_a_known_settlement` does in app-core has an engine-side equivalent —
  find it rather than writing a fourth way to plant a town).
- [ ] **Step 2: Run and watch them fail** with the current structures-only sum.
- [ ] **Step 3: Add the term** with the clamp in the order above. Doc-comment
  the clamp with the reason: `saturating_sub` in `run_raid` means an uncapped
  garrison deletes raids silently, since a raid landing for zero still logs.
- [ ] **Step 4: Green**, then `fmt` and `clippy`.
- [ ] **Step 5: Run the raid suite** — `cargo test -p feral-processes-engine
  raids > /tmp/t.log 2>&1` — and read it. Existing raid tests assume a defense
  figure; if any moved, understand why before touching it.
- [ ] **Step 6: Commit.** `feat(settlements): an allied town garrisons the base`

---

### Task 3: The gift

**Files:**
- Modify: `crates/engine/src/settlements/relations.rs` (`Relation` gains a field)
- Modify: `crates/engine/src/game/settlement_relations.rs` (the door)
- Test: `crates/engine/src/tests/settlement_relations.rs`

**Interfaces:**
- Consumes: `Standing::gifts_programs`, `SETTLEMENT_GIFT_COOLDOWN_TICKS`,
  `SETTLEMENT_GIFT_STAT_MULT`, `SETTLEMENT_GIFT_SPECIALTY_MULT` from Task 1.
- Produces:
  ```rust
  // crates/engine/src/settlements/relations.rs
  pub struct Relation {
      pub standing: i32,
      #[serde(default)]
      pub trade_credits: u32,
      #[serde(default)]
      pub last_gift_tick: Option<u64>,   // new
  }

  // crates/engine/src/game/settlement_relations.rs
  impl Game {
      pub fn request_program_gift(&mut self, key: SettlementKey) -> Result<(), String>;
      /// The preview the screen draws: `None` when this town will never gift,
      /// `Some(0)` when it will gift now, `Some(n)` for ticks remaining.
      pub fn gift_available_in(&self, key: SettlementKey) -> Option<u64>;
  }
  ```

`gift_available_in` exists so the town screen and the door cannot disagree —
`BuildOrderRow`'s rule. Task 5 consumes it.

**The refusal ladder, in order, each landing before anything is spent:**

1. game over
2. a battle is running
3. no `Settlements` entry under `key`
4. out of reach — `Game::settlement_reach(key)`
   (`game/settlement_market.rs:69`), Chebyshev 1. That is the underlying
   predicate; the app-core screens ask it through `settlement_view` only
   because they need the view anyway, and an engine door that wants the
   question alone should ask the question alone.
5. `!self.standing_band(key).gifts_programs()`
6. the cooldown has not elapsed against `resources::GameClock`

**What arrives:** `Game::adopt_program(species, x, y, stat_mult)`
(`game/spawning.rs:407`) at `anchor_position()`. It is `pub(crate)` and this
door is in the same crate, so no widening.

**The species roll must not touch `GameRng`.** Build the pool from
`Game::habitat_pools(town_x, town_y, None, 0)` — the town's own region, so a
town gives you something local — and pick by index from a `StdRng` seeded from
`(world seed, the settlement key, the count of gifts already taken from this
town)`. `pick_lair_species` (`game/stack_features.rs:196`) is the pattern; note
its comment that the pool must be **sorted**, because the draw picks by index
and an unsorted pool changes the answer between runs on one seed. `habitat_pools`
returns `Option<(Vec<String>, Vec<String>)>` — ordinary candidates and boss
candidates. **Use the ordinary pool only**; a gifted apex species is decision 5
inverted.

Deriving the seed from a gift *count* means `Relation` needs that count. Either
add `gifts_taken: u32` beside `last_gift_tick` (both additive, still no version
bump) or derive it — adding the field is simpler and honest. Say which you did.

`stat_mult` is `SETTLEMENT_GIFT_STAT_MULT`, multiplied by
`SETTLEMENT_GIFT_SPECIALTY_MULT` when `def.specialty == Specialty::Programs`.

Log the arrival as **base news** — `MessageSource`'s base variant — because the
program appears at the anchor and the player may be nowhere near it.

- [ ] **Step 1: Write the failing tests.** One per refusal, each asserting
  **nothing was spent**: the roster is unchanged, `last_gift_tick` is unchanged,
  and no line was logged. Six refusal tests, not one.
- [ ] **Step 2: Write the failing success tests.**
  - a gift joins the roster through `roster_parts` — assert the components the
    roster barrier installs, not merely that an entity appeared
  - the gifted program lands as **staff**: `Game::program_role` answers the
    staff variant, since it is not in the party, not wielded and not away
  - the species is stable across a save→load round trip **and** across two
    `Game`s built on the same seed — the derived-not-`GameRng` property
  - a gift does not advance `GameRng`: take the RNG state before and after
    (follow whatever `..._is_deterministic_and_ignores_gamerng` in
    `crates/engine/src/tests/stack.rs` does)
  - the cooldown refuses the second request; ticking past
    `SETTLEMENT_GIFT_COOLDOWN_TICKS` releases it
  - a `Specialty::Programs` town's gift has strictly better stats than an
    otherwise identical town's
  - **save→load** preserves `last_gift_tick` and `gifts_taken`. Not a RON
    round-trip — that cannot catch a `#[serde(skip)]`.
- [ ] **Step 3: Run and watch them fail.**
- [ ] **Step 4: Add the fields, then the door.** Doc-comment `last_gift_tick`
  with why the limiter is time rather than a price, and the door with the
  refusals-before-spending rule.
- [ ] **Step 5: Green**, `fmt`, `clippy`.
- [ ] **Step 6: Commit.** `feat(settlements): an allied town gifts a program`

---

### Task 4: The travel

**Files:**
- Modify: `crates/engine/src/game/spawning.rs` — `standable_near` (1013) to
  `pub(crate) fn`. Nothing else in that file.
- Modify: `crates/engine/src/game/settlement_relations.rs` (the two doors)
- Test: `crates/engine/src/tests/settlement_relations.rs`

**Interfaces:**
- Consumes: `Standing::hosts_a_relay`, `SETTLEMENT_TRAVEL_TICKS_PER_TILE`.
- Produces:
  ```rust
  impl Game {
      pub fn travel_to_settlement(&mut self, key: SettlementKey) -> Result<(), String>;
      pub fn travel_to_anchor(&mut self) -> Result<(), String>;
      /// What a trip would cost, for the screen. `None` when travel is refused
      /// for any reason — the screen shows no figure it cannot honour.
      pub fn travel_cost_ticks(&self, key: SettlementKey) -> Option<u64>;
  }
  ```

**The gate is one rule for both directions:** the town is Allied
(`hosts_a_relay`) **and** the base has a Relay — ask `Game::dispatch_reach`
(`game/sortie.rs:76`), the door a squad and a route already leave through, and
refuse on anything but its at-the-relay variant. Read `DispatchReach`'s variants
before writing the match; do not invent a fourth.

**Refusals, per-refusal tests again:** game over; a battle is running;
underground (`require_surface`, `game/stack.rs:374`); no Relay; the town does
not host a relay; no `Settlements` entry; **no standable neighbour at the far
end**.

**Where you land.** Never on the settlement tile — a settlement admits nobody
and the bump is the fourth arm of `move_player`'s ladder. Use
`standable_near(town_tile)`; `None` is the seventh refusal above, not a panic
and not a landing inside rock. Travelling home lands on `anchor_position()`.

**What it costs.** Chebyshev distance between origin and landing tile times
`SETTLEMENT_TRAVEL_TICKS_PER_TILE`, spent as `for _ in 0..n { self.tick() }` —
`game/base_space.rs:830` and `game/crafting.rs:1001` are the shape. Charge the
ticks **after** the move lands, so a refusal has spent nothing. `after_tick()`
is app-core's and `handle_key`'s tail already pays it for this keypress; travel
adds no fourth tick-spending path to that list.

**On arrival at a town**, queue `resources::PendingVisit` with the key, so the
town screen opens exactly as walking into the tile does. One arrival behaviour,
not two. Note `PendingVisit` is deliberately not serialized.

- [ ] **Step 1: Write the failing refusal tests** — seven, each asserting the
  player's `Position` is unchanged and no ticks were spent (compare
  `GameClock` before and after).
- [ ] **Step 2: Write the failing success tests.**
  - travelling to a town lands the player on a walkable tile adjacent to it and
    **not on the town's own tile**
  - arriving queues `PendingVisit`, and `Game::take_settlement_visit` drains it
  - travelling home lands on `anchor_position()`
  - the `GameClock` advanced by exactly `travel_cost_ticks`' quoted figure —
    the quote and the charge cannot differ
  - a town whose neighbours are all unwalkable refuses and moves nobody
- [ ] **Step 3: Run and watch them fail.**
- [ ] **Step 4: Widen `standable_near`, then write the doors.**
- [ ] **Step 5: Green**, `fmt`, `clippy`.
- [ ] **Step 6: Commit.** `feat(settlements): relay travel between an ally and home`

---

### Task 5: The town screen says what aid is available

**Files:**
- Modify: `crates/engine/src/views.rs` (`SettlementView`, 1881)
- Modify: `crates/engine/src/game/inspection.rs` (`settlement_report`, 809)
- Modify: `crates/gui/src/render/settlement.rs` (`settlement_page_rows`, 42)
- Test: engine test for the view; a gui width/height census

**Interfaces:**
- Consumes: `Game::gift_available_in`, `Game::travel_cost_ticks`,
  `Standing::garrison_defense`, and `Game::dispatch_reach` (a town hosts a
  relay, but travel also needs the Relay at your end — the sentence must not
  promise a trip the door would refuse).
- Produces: `SettlementView` gains **one** field:
  ```rust
  /// One sentence per aid this town currently offers, already in the
  /// player's words. Empty when the town offers none.
  pub aid: Vec<String>,
  ```

One field of finished sentences rather than three raw figures, for two reasons
that both bind. **A read-only screen's row count is owned by app-core and drawn
by gui, so any per-row transform must live in the engine** — a phrase built in
the renderer is a transform in the wrong crate. And **there is no player-facing
tick vocabulary in this game**: a cooldown quoted as a tick count is a number
the player has no way to read. `game/memories.rs:565`'s `age_phrase` is the
precedent — it bands elapsed time against the def's own half-life and returns
words, never a figure. Band the gift's remaining cooldown against
`SETTLEMENT_GIFT_COOLDOWN_TICKS` the same way.

The travel line quotes **no figure at all** — that the relay will carry you is
the whole of what the player needs, and the tick charge has no in-fiction unit
to be quoted in. `travel_cost_ticks` stays, because the door needs it and
because the quote-equals-charge test in Task 4 asserts against it; it simply is
not rendered.

Every sentence is built from a **call** to the door that will honour it, never
a second derivation — `BuildOrderRow`'s rule, and `settlement_page_rows`
already carries a comment explaining why the name's colour is a call rather
than a copy.

- [ ] **Step 1: Write the failing engine test** — `settlement_report` on an
  Allied town returns three aid sentences; on a Warm one, only the garrison's;
  on a Neutral one, an empty `aid`. Assert on the count and on each sentence
  naming its aid, and assert **no sentence contains a bare tick count**.
- [ ] **Step 2: Write the failing gui census.** This page is a popup with **no
  scroll**, so height is a layout constraint. Follow the existing width/height
  census in `render/settlement.rs` (`settlement_page_rows` was built to be
  measurable out of a view alone for exactly this). Measure the **worst case**:
  longest town name, longest blurb, all three aid rows present. Verify the test
  by mutation — make a row too wide on purpose and confirm it fails.
- [ ] **Step 3: Run and watch both fail.**
- [ ] **Step 4: Add the field, fill it, render one row per sentence.** An
  absent aid contributes **no sentence** rather than one saying "no" — the page
  is about what this town is worth, not a checklist. An empty `aid` draws no
  rows and no header.
- [ ] **Step 5: Green**, `fmt`, `clippy`.
- [ ] **Step 6: Commit.** `feat(settlements): the town page says what aid it offers`

---

### Task 6: The keys

**Files:**
- Modify: `crates/app-core/src/app/inspection.rs` (`handle_settlement_key`, 141)
- Modify: `crates/app-core/src/app/dispatch.rs` (`handle_dispatch_key`, 123)
- Modify: `crates/app-core/src/lib.rs` — the `Mode::Settlement` and
  `Mode::Dispatch` doc comments both enumerate their keys; both are now wrong.
- Test: `crates/app-core/src/tests/dispatch.rs`,
  `crates/app-core/src/tests/settlement.rs`

**Interfaces:**
- Consumes: all three engine doors from Tasks 3 and 4.
- Produces: no new public app-core API unless a screen needs a view accessor.

**`[G]` on `Mode::Settlement`** — the reach check first, the same shape `[M]`
and `[J]` use and for the same reason (a town read from across the map via `x`
would otherwise call a door that answers `None`), then
`Game::request_program_gift`. A refusal goes through `App::refuse`
(`app/input.rs:386`), which is the one door for a refusal on both surfaces.

**`[T]` on `Mode::Settlement`** — `Game::travel_to_anchor`. Same reach check.
On success the screen must close to `Mode::Playing` and clear
`pending_settlement`, since the player is no longer standing there. **This is
the trap in this task**: leaving the screen open leaves it describing a town
that is now far away.

**`[T]` on `Mode::Dispatch`** — resolve the highlighted row through
`dispatch_row` exactly as `[C]` and `[X]` do; a `Site` row refuses with a
sentence in the same voice as the existing two ("Highlight a destination to
travel to."). On success, close the hub to `Mode::Playing`.

- [ ] **Step 1: Write the failing app-core tests.**
  - `[G]` on a town read from out of reach refuses and never reaches the engine
  - `[G]` in reach at Allied adds a program to the roster
  - `[T]` on the town screen moves the player and **closes the screen**, with
    `pending_settlement` cleared
  - `[T]` on a hub *site* row refuses; on a *destination* row it travels and
    closes the hub
  - a refusal appears on `App::status_line` **and** in the log, once each
  Use `tests/dispatch.rs`'s existing fixtures (`app_at_a_relay`,
  `register_a_known_settlement`, `a_dispatch_ready_app`) rather than new ones.
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Add the three key arms**, and correct both `Mode` doc comments
  in `lib.rs` to list the keys each screen now has.
- [ ] **Step 4: Green**, `fmt`, `clippy`.
- [ ] **Step 5: Full gate.** `cargo test --workspace > /tmp/t.log 2>&1`, then
  read the file. Do not pipe it.
- [ ] **Step 6: Commit.** `feat(settlements): [G] asks for aid and [T] travels`

---

### Task 7: The seam writes and the changelog

**Files:**
- Modify: `docs/seams.md` (the argument)
- Modify: `.claude/skills/seams/references/ground.md` (the trap) — the shipped
  settlement seams live under "The ground"; check there and follow suit.
- Modify: `CLAUDE.md` (the rule, one sentence each)
- Modify: `CHANGELOG.md`

**A new seam is three writes**, and the order is: argument to `docs/seams.md`,
trap to the skill, rule to `CLAUDE.md`. **`CLAUDE.md` is gitignored** and has a
twin at `AGENTS.md` with no tracking to catch drift — edit `CLAUDE.md`, then
`cp CLAUDE.md AGENTS.md`.

**One sentence is a budget, not a style** for the `CLAUDE.md` lines. That file
is loaded every turn and reached 151 KB by letting each trap creep back in
beside its rule.

The seams worth writing, one each:

1. **The garrison clamp is on the settlement half alone.** The trap: clamping
   the total caps the player's own shield network, and an uncapped garrison
   deletes raids silently because a raid landing for zero still logs.
2. **A gift's species is derived, never drawn.** The trap: `GameRng` would let
   a reload reroll it and would shift the seeded stream.
3. **Travel never lands on the settlement tile.** The trap: a settlement admits
   nobody, so the obvious implementation puts the party somewhere `move_player`
   would have refused, and no test on the tile itself sees it.
4. **Aid is a named query on the band, never a table of effects** — this one
   already exists in `relations.rs`'s module doc; extend the existing seam entry
   rather than writing a fourth.

**Changelog:** a new `## X.Y.Z` section written in the voice of the surrounding
entries. **Do not bump the workspace version and do not tag** — the bump happens
once, at the merge, so a rebase or squash cannot invalidate a version already
tagged. Which digit moves is decided by `CHANGELOG.md`'s own preamble; "breaking"
means a save stops loading, which this does not.

**Do not update `docs/manual.md` or the root `README.md`** — both are carved out
of the documentation obligation. `assets/*/README.md` does not apply either;
this change adds no asset schema.

- [ ] **Step 1: Write the three arguments** into `docs/seams.md`, under headings
  matching the `CLAUDE.md` rules exactly.
- [ ] **Step 2: Write the traps** into the seams skill reference.
- [ ] **Step 3: Write the rules** into `CLAUDE.md`'s settlement section, then
  `cp CLAUDE.md AGENTS.md`.
- [ ] **Step 4: Write the changelog section.**
- [ ] **Step 5: Final gate.** `cargo fmt`, `cargo clippy --workspace`,
  `cargo test --workspace > /tmp/t.log 2>&1` — read the file.
- [ ] **Step 6: Commit.** `docs(settlements): the aid seams and the changelog`

---

## After the last task

A whole-branch review on **opus**, seeing the full diff as a file rather than
pasted into the prompt, and told the exact rules to check rather than "the
conventions". Per-task gates were deliberately skipped for this branch, so the
final review is the only gate — read its evidence, not just its verdict, and
re-run anything it quotes.

Then: this feature ships with a green suite and **zero screen time**. Say so
once, plainly, and do not ask for a playtest.
