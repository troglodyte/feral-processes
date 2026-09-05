# Settlements Phase 2 — the UI half

The engine half landed in `feat(settlements): a town materializes and stays`.
A settlement is derived per region, materialized onto walkable ground as the
party covers it, recorded in `resources::Settlements` and restored on load.
What is missing is every way a player could know it is there.

Spec: `docs/superpowers/archive/specs/2026-09-04-settlements-design.md`, Phase 2.
Read that first, then this. Read `CLAUDE.md`, then invoke the `seams` skill
for **the HUD** and **saves, logs and screens** before touching either.

## What the survey already settled — do not re-derive

Verified against the source on 2026-09-04. Verify a line before relying on
it, but do not re-run an exploration pass.

**The glyph already draws.** `Game::view_entities_at`
(`crates/engine/src/game/inspection.rs:805`) is a *generic*
`(Entity, &Position, &Glyph)` query filtered by `stands_in_base_space`
(`inspection.rs:330`), which tests for `Caravan`/`Structure`/`BuildSite`/
`Tamed`. A settlement entity carries none of them, so it falls through to
the surface map and lands in `draw_surface_map`'s `actor` slot
(`crates/gui/src/render/base.rs:1113`) exactly like a nest. **There is no
draw code to write.** There is a test to write asserting it, because
nothing states it.

**The bump ladder is the door.** `Game::move_player`
(`crates/engine/src/game/turn.rs:496`) runs three arms before it consults
terrain at all:

```
find_wild_creature_at -> start_battle, tick, return
find_nest_at          -> attack_nest,  tick, return
find_surface_link_at  -> enter_stack,  tick, return
                      -> then walkable check, then the step
```

Each returns before the walkability check, so **a bump arm is also what
blocks movement** — no occupancy field, no terrain change. A Stack entrance
is already bump-to-enter; a settlement is the fourth arm of the same ladder.

**`GlyphColor` has no room for a new hue.** It is an 11-variant enum
exhaustively matched by `hud::palette::glyph`
(`crates/gui/src/render/hud/palette.rs:147`), and the palette is a *hue*
table, not a role table. The spec's "a hue in `hud::palette::glyph`" is
therefore a **choice among the existing eleven**, not an addition — adding a
twelfth breaks every exhaustive match and re-opens
`every_content_hue_is_separable_from_the_others` (`palette.rs:234`).

**The screen template is `Mode::CompanionMemories`** — the simplest
read-only no-scroll page in the codebase. Six parts, listed in Task 3.

## Decisions taken

Settled with the user before implementation. Recorded so they are not
relitigated mid-task.

| Question | Decision |
|---|---|
| How the hub opens | **Bump to enter.** The tile does not admit you; walking into it opens the screen and you stay put. |
| What the hub shows | **Identity only** — name, kind, specialty, temperament, blurb. No action rows, no stubs for later phases. |
| The settlement hue | **`GlyphColor::Orange`**, replacing the `Yellow` it spawns with today. |
| Examine | `x` gets an `InspectTarget::Settlement` arm and opens the same screen. |
| Seam documentation | In scope. The full three writes, for Phase 1 and Phase 2 both. |

**Why not `Yellow`.** `GlyphColor::Yellow` maps to `palette::WARN` — the hue
a `Starved` or `Unstaffed` machine wears — and worse, it is the authored
colour of the **Scrapper**, so a settlement and a scrapper nest are the same
hue on the same map today.

**Why `Orange`.** A nest takes its guardian species' colour
(`nest_components`, `spawning.rs:1493`), so every hue a species authors is
reachable on the surface map — which makes "unclaimed" mean *unclaimed by a
species*, not unclaimed outright. **No species is authored `Orange`.** Its
only uses are base space's `BuildSite` glyph and three base structures,
which stand in a different coordinate space and can never share a tile with
a town, plus the difficulty bar's "tough" rung — a bar drawn *under* a
hostile, not a glyph fill. So `Orange` is the one variant that cannot
collide with anything the zone map draws. `Blue` was the first choice and is
worse: `sentinel.ron` authors it, so wild sentinels and sentinel nests wear
it.

**Why `x` opens the whole screen rather than a line.** `InspectTarget`'s
four existing variants each exist because a manifest opened on that subject
would be blank, and each routes to the subject's own page instead — the
doc comments at `views.rs:1813` and `:1820` say so. A settlement is that
case exactly. Reading a town's identity from across the map is what examine
is *for*, and it stays correct when Phase 3 adds a market: `broker_reach`'s
rule already splits *reading* a board from *signing* one, so the market and
the job board get their own reach check without this decision moving.

## Task 1 — the door (engine)

**Files:** `crates/engine/src/game/turn.rs`, `crates/engine/src/game/spawning.rs`,
`crates/engine/src/resources.rs`, `crates/engine/src/tests/`.

Produce:

- `Game::find_settlement_at(&mut self, x, y) -> Option<SettlementKey>` —
  mirror `Game::find_surface_link_at` (`game/stack.rs:137`) exactly:
  `query_filtered::<(&Position, &Settlement), ()>` shape, same borrow
  discipline.
- `resources::PendingVisit(Option<SettlementKey>)` — **not serialized**, for
  `resources::CurrentStack`'s reason: it is a cue about this instant, and a
  save that restored one would open a screen on load. Follow `CurrentStack`'s
  doc comment shape.
- `Game::take_settlement_visit(&mut self) -> Option<SettlementKey>` — a
  **drain**, `take_effects`/`take_transits`' shape
  (`game/base/upkeep.rs:193`). It must return `Some` once and `None`
  thereafter, or the screen reopens on the next keypress and the player
  cannot walk away.
- The **fourth arm** in `move_player`, placed after the surface-link arm and
  before the biome/walkable read. It writes the cue, ticks, and returns —
  the same three lines the other three arms end with.

The arm ticks like its three neighbours. That is the consistency call: a
bump is an action whatever it bumps into.

**Tests** (`crates/engine/src/tests/`, new file `settlements.rs` or the
existing zone/turn file — follow what is already there):

- Walking into a settlement leaves the player's `Position` unchanged. This
  is the blocking assertion and it is the one the feature is about.
- The bump queues exactly one visit naming that settlement's key.
- A second `take_settlement_visit` answers `None`. **Write this one first
  and watch it fail against a non-draining read** — a getter passes every
  other test in this list.
- The cue is absent from a save/load round trip.
- Walking onto ordinary ground beside a settlement still moves you, and
  queues nothing.

## Task 2 — examine, label and the derivation (engine)

**Files:** `crates/engine/src/game/inspection.rs`, `crates/engine/src/views.rs`,
`crates/engine/src/game/spawning.rs`, tests.

Produce:

- An `entity_label` arm (`inspection.rs:686`, the `SurfaceLink` arm at
  `:715` is the model) answering the settlement's **authored name**, off
  `resources::Settlements` by key — never the def id and never a
  hand-built string in a renderer, which is `Game::copy_name`'s rule.
- `InspectTarget::Settlement(Entity)` (`views.rs:1810`), with a doc comment
  in the house style of the two beside it saying why it is not `Structure`.
- `Game::find_target_in_direction` (`inspection.rs:407`) gains the
  settlement query. Note the comment already standing at `:477` that names
  nests and surface links as the *remaining* gap — settlements were about to
  join that list; this closes the settlement half and the comment needs
  updating rather than leaving to read as though it still applies.
- `Game::settlement_report(key) -> views::SettlementView` — **the one
  derivation**, engine-side. Name, kind label, specialty label, temperament
  label, blurb. Per the screens seam, a read-only screen's rows are owned by
  app-core and drawn by gui, so **every per-row transform lives here**, not
  in the renderer.
- Change `spawn_settlement_at`'s `GlyphColor::Yellow` to
  `GlyphColor::Blue` (`spawning.rs:938`).

**Tests:**

- `entity_label` on a materialized settlement is its authored name — the
  `assert_eq!(game.entity_label(link), "Stack Entrance")` shape at
  `tests/inspection.rs:2157`.
- `find_target_in_direction` finds a settlement it previously looked
  through. Assert the *negative* first against the current code.
- `settlement_report` names all five fields off the resolved def, and a
  catalogue entry edited after materialization does not change it — the
  whole reason `KnownSettlement` stores the def.

## Task 3 — the mode (app-core)

**Files:** `crates/app-core/src/lib.rs`, `crates/app-core/src/app/input.rs`,
`crates/app-core/src/app/playing.rs`, `crates/app-core/src/app/lifecycle.rs`,
`crates/app-core/src/tests/`.

Copy `Mode::CompanionMemories`'s six parts:

1. `Mode::Settlement` on the enum (`lib.rs:1144`-`1602`), doc comment saying
   how it is reached.
2. `App::pending_settlement: Option<SettlementKey>` (`lib.rs:2003` is where
   `pending_memory_program` sits), initialized `None` in
   `lifecycle.rs:91`.
3. Opened from `App::after_world_action` (`playing.rs:550`), **beside the
   `has_active_battle` check and not instead of it**, by draining
   `take_settlement_visit`. This is the one place a tile arrival changes the
   mode today and it is why the engine hands over a cue rather than setting
   a mode it cannot see.
4. `handle_settlement_key` — Esc only, clearing `pending_settlement` and
   `status_line`, returning to `Mode::Playing`.
5. The dispatch line in `input.rs:239`'s match.
6. The Esc-adjacency pair at `input.rs:72`.

Also route `InspectTarget::Settlement` to the same mode in
`crates/app-core/src/app/inspection.rs:44`.

Note what the two neighbouring arms do there: `Caravan` and `BuildSite` both
route to `Mode::CellDescribe` with a one-line blurb rather than a screen of
their own, because neither has a page. A settlement **does** have a page, so
it routes to `Mode::Settlement` — the same screen the bump opens, which is
what keeps the two doors from drifting into two derivations.

**Traps.** Lowercase letters are row selectors, so **no new lowercase key**
is bound anywhere for this — the door is the bump and `x`, both of which
already exist. Nothing is added to `handle_playing_key`'s match.

**Tests** (`crates/app-core/src/tests/`, following `party.rs:581`):

- Bumping a settlement puts the app in `Mode::Settlement` with
  `pending_settlement` set.
- Esc returns to `Mode::Playing` and clears it.
- Walking away and taking one more step does **not** reopen it — the drain,
  asserted from the app's side as well as the engine's.
- `x` toward a settlement opens the same mode.

## Task 4 — the screen (gui)

**Files:** `crates/gui/src/render/mod.rs`, a draw function beside the
existing read-only pages, tests.

- `draw_settlement` + a `settlement_page_rows` row builder taking **view
  data, not a `Game`** — `memory_page_rows` (`render/party.rs:148`) is the
  shape, and taking view data is what makes the width and height censuses
  writable at all.
- The dispatch arm in the popup match (`render/mod.rs:898`-`1184`), which
  ends in `_ => {}` — **a new `Mode` does not fail to compile and ships as a
  blank screen**.
- `ALL_MODES` (`render/mod.rs:1248`) `[Mode; 92]` → `[Mode; 93]`. Note this
  length is a **semantic merge conflict** with any other branch adding a
  mode: the entries merge cleanly and the count does not.

**Tests:**

- A row names the settlement, its kind, its specialty and its blurb.
- `the_tallest_shipped_settlement_fits_its_popup` — built from the **real
  catalogue**, both dimensions. A text-row popup page has no scroll, so
  height is a layout constraint; and `draw_row` clips vertically only, so
  **width is a real constraint that fails silently**. Measure the joined
  row, head and tail together.
- `every_screen_draws_a_refusal_exactly_once` (`render/mod.rs:1468`) must
  still pass with the new mode in `ALL_MODES`.
- The map draws a settlement's glyph on the surface — the assertion nothing
  currently makes, per "the glyph already draws" above.

## Task 5 — the three writes (docs)

Owed for **Phase 1 and Phase 2 both**. `docs/seams.md` has Phase 1's
argument only (`### A breach raises a tier now, and does not rebuild the
world`); the traps and the rules were never written, and `CLAUDE.md` still
describes the retired sector system as live.

In the order the `seams` skill mandates:

1. **`docs/seams.md`** — a `###` section for *Where a settlement stands is
   derived, never stored*, and one for *A settlement is entered by walking
   into it*. The measurement, what was tried, what was rejected.
2. **`.claude/skills/seams/references/`** — the trap beside each rule, house
   style. The persistent-world entries belong in whichever file owns the
   zone; the screen entries in `hud.md` and `screens.md`.
3. **`CLAUDE.md`** — one sentence per rule, no more. It is loaded every turn
   and reached 151 KB by letting traps creep back in beside rules. **Also
   correct what Phase 1 falsified**: the file still describes `assets/
   sectors/` and per-sector palette rotation as live.

`CLAUDE.md` is gitignored and cannot ride the branch — it must be edited in
the primary checkout, and `AGENTS.md` is its untracked twin and must be
copied over from it in the same pass.

`assets/settlements/README.md` gains the glyph and hue in its `kind` row.

## Gates

Per task: `cargo fmt`, `cargo clippy --workspace` (fix, never silence), and
the crate's own tests. Before the phase is called done:
`cargo test --workspace`.

`balance_sim` is **not** a gate here — nothing in this phase touches
`tuning.rs`, a species file or an item file.

**Two known hazards when reading a suite result.** Piping `cargo test`
through `grep` or `tail` reports the *pipeline's* exit code, so a failing
suite announces success — redirect to a file. And this branch produced one
intermittent engine failure on 2026-09-04 that a second full run did not
reproduce; a single red test that passes on re-run is a known flake, not
this work, and must not be brute-forced with repeated runs.

## Out of scope

A `dev-saves/` template capturing a party standing beside a town is worth
having and the spec asks for one "once settlements are placeable" — but
capturing it needs a run, and agents cannot run the game here. Flag it for
the user rather than attempting it.

No help page. `assets/help/` is a real content directory and a settlements
page is cheap, but the feature is one screen with no verbs yet; the page is
worth writing when there is something to *do* at a town.
