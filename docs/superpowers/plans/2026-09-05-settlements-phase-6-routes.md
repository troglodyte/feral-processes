# Settlements Phase 6 — Caravan routes

Spec: `docs/superpowers/specs/2026-09-04-settlements-design.md`, "Phase 6".
Branch: `feat/caravan-routes`. Scope confirmed with the user 2026-09-05:
**both halves in one landing** (one-off dispatch *and* standing routes,
including Hostile route predation), **plus the first dispatch screen, which
covers sorties as well as routes.**

Read `CLAUDE.md`, then the `seams` skill's `references/sorties.md`,
`references/items.md` and `references/screens.md` before touching code.

## Decisions taken

Made from the spec plus the 2026-09-05 source survey. Recorded so they are
not relitigated mid-build.

| Question | Decision |
|---|---|
| What a route is | One record, `routes::Route`, with a **`standing: bool`** flag — `WorkOrder`'s shape, and its reason: the record stores what was asked for, never how it will be done. A one-off is a standing route that does not go again. |
| Where you dispatch from | **The Relay.** Routes gate on `StructureDef::dispatches_sorties`, not a second flag — one building, one screen, and the seam that says a Relay is identified by its flag and never by its id is untouched. |
| Consequently | `SortieReach` → **`DispatchReach`**, `Game::sortie_reach` → `Game::dispatch_reach`. A compiler-checked rename; it now gates two features and the old name would lie. |
| What a route carries | Cargo out (`Vec<(ItemId, u32)>` spent from base stock at dispatch), **Credits back**. The sale happens at the destination on the outbound leg's completion, priced through the existing `Game::settlement_sell_price`, and pays standing through the existing `credit_trade_volume` door. |
| Legs | `RouteLeg::{Outbound, Inbound}`. Outbound completion sells and turns the trip around; inbound completion deposits proceeds into base stock. |
| Duration | Derived from Chebyshev distance base anchor → destination tile, `ROUTE_TICKS_BASE + ROUTE_TICKS_PER_TILE * d`. **One computation, quoted by the board and run by the countdown** — `sortie_duration`'s rule. No term for cargo size or party strength. |
| Standing route unlock | A **new named query**, `Standing::allows_standing_route`, true from `Warm` up. One-off dispatch needs only `!refuses_service`. |
| Predation | A **new named query**, `Standing::preys_on_routes`, true at `Hostile`. A known settlement preys on a trip when its tile lies within `ROUTE_PREDATION_RADIUS` of the **segment** base→destination. Each predator rolls `ROUTE_PREDATION_CHANCE` at a leg's completion; a hit takes `ROUTE_PREDATION_LOSS` of the cargo (outbound, before the sale) or the proceeds (inbound). |
| Severing | `Game::sever_route` clears `standing` and nothing else. The trip in flight still completes and still pays. No refund path, no cargo teleport. |
| Reload | A standing route reloads the same manifest from base stock on arrival home and departs again; short stock **stalls** it (`stalled: bool`, retried each tick) rather than severing it — a stalled work order's rule. |
| Save | `RouteSave` stores the **whole resolved record**, destination town def included, added as `PlayerSave::routes` behind `#[serde(default)]`. **No `SAVE_FORMAT_VERSION` bump** — an added field on field-named RON is free, `PlayerSave::sorties`' own precedent. Resist bumping to be safe. |
| Route identity | Keyed on `SettlementKey`, **not** on the `CreatureSave::sortie_index` scheme. Cargo has no entity whose id could be unstable, so that pattern solves a problem this does not have. |
| Screen | `Mode::Dispatch` is the Relay hub — sortie sites, route destinations and every trip in flight on one popup. `Mode::SortieSquad` picks a squad; `Mode::RouteCargo` picks a cargo basket. Three new modes, all popups, opened by a **row on the base menu**, not a new key. |
| Sorties' first UI | In scope by the user's decision. Nothing exists to extend — `views::SortieRow` and `views::SortieReport` are built and read by nothing — so this writes the first screen for both features at once. |
| No new catalogue | A route is a record keyed on an existing settlement, not a catalogue entry. **No `assets/routes/`.** |

## Tasks

Each task: branch is already cut, TDD with the failing test first, a commit per
green step, `cargo fmt` and `cargo clippy --workspace` after each. The gate for
the whole branch is `cargo test --workspace`.

### 1 — The record, the save, the tuning

Files: new `crates/engine/src/routes.rs`; `resources.rs`; `save.rs`;
`game/lifecycle.rs`; `tuning.rs`; `lib.rs` (module).

Produce: `routes::Route` (destination `SettlementKey`, the resolved
`SettlementDef`, destination tile, `cargo: Vec<(ItemId,u32)>`, `standing: bool`,
`stalled: bool`, `leg: RouteLeg`, `ticks_total`, `ticks_elapsed`,
`proceeds: u32`, `losses: Vec<String>`), `routes::RouteLeg`,
`resources::Routes(pub Vec<Route>)` (a `Resource`, **not** `Serialize` —
`Sorties`' shape), `save::RouteSave` mirroring it, drained and reassembled in
`lifecycle.rs` beside the sortie code. Tuning constants go in a new
`// Caravan routes` section beside `// Sorties`: `ROUTE_TICKS_BASE`,
`ROUTE_TICKS_PER_TILE`, `ROUTE_PREDATION_RADIUS`, `ROUTE_PREDATION_CHANCE`,
`ROUTE_PREDATION_LOSS`, `ROUTE_MAX_ACTIVE`.

Tests: a route in flight survives a **real save round trip** packed back into a
save file (not a bare RON round trip — that leaves a `#[serde(skip)]` green);
`SAVE_FORMAT_VERSION` is unchanged; a pre-routes save loads with no routes.

### 2 — The two standing queries and the predation geometry

Files: `crates/engine/src/settlements/relations.rs`; `routes.rs`.

Produce: `Standing::preys_on_routes` and `Standing::allows_standing_route`,
both exhaustive matches in the module's existing style, each with its own
census test in the shape of
`every_standing_band_answers_whether_it_refuses_service`. Plus a pure free
function in `routes.rs` answering which known settlements prey on a given trip
— point-to-segment distance against `ROUTE_PREDATION_RADIUS`, no `Game`, no
RNG.

Tests: the two censuses; the geometry against a town beside the line, a town
past either end, and a town at the midpoint; a Warm town does not prey and a
Hostile one does not host a standing route.

### 3 — The dispatch doors

Files: `crates/engine/src/game/sortie.rs` (the rename), new
`crates/engine/src/game/route.rs`, `game/mod.rs`.

Produce: `DispatchReach` shared by both features, one private helper both
`dispatch_reach` callers use. Then `Game::route_destinations` (every **known**
settlement, its band, its quoted duration and quoted proceeds — three-state
like `board_defs`: `None` for no Relay, `Some(vec![])` for a Relay with
nothing reachable), `Game::route_quote` (the one derivation of what a manifest
is worth at a town, shared by the preview and the sale — a quoted figure and a
granted figure may not differ), `Game::dispatch_route`, `Game::sever_route`,
`Game::route_reports`.

`RouteRefusal` variants at minimum: `NotAtRelay`, `UnknownDestination`,
`Refused` (the town is Hostile), `NoStandingRoutes` (asked for standing below
Warm), `EmptyManifest`, `Understocked { item, need, held }`, `Duplicate` (a
route to that town is already in flight), `TooMany`.

Tests: **every refusal asserted individually** to leave base stock exactly as
it was — `every_refusal_leaves_credits_and_cargo_exactly_as_they_were`'s shape,
and one test over one refusal passes against every path that never spends
anyway. Plus: the quote equals what the sale later grants.

### 4 — The tick

Files: `crates/engine/src/game/route.rs`; `game/turn.rs`.

Produce: `Game::run_routes`, called from `turn.rs` beside `run_sorties`,
carrying the same `is_game_over() || has_active_battle()` guard. Outbound
completion: roll predation against the cargo, sell the remainder through the
settlement's own pricing, `credit_trade_volume`, turn the trip around. Inbound
completion: roll predation against the proceeds, deposit the rest into base
stock, then either reload (standing, stock allowing), stall, or drop the
record. Both dispatch and arrival queue a `TransitCue` — extract the glyph-and-
tile half of `queue_squad_walk` so cargo can use it without an entity.

Tests: a full out-and-back pays the quoted proceeds and raises standing; a
Hostile town beside the line takes its cut and says so in the log; a standing
route departs again on arrival; short stock stalls it and restocking releases
it; a severed route completes its trip and does not go again; predation spends
`GameRng` and nothing else does.

### 5 — views and app-core

Files: `crates/engine/src/views.rs`; `crates/app-core/src/lib.rs`;
`app/inspection.rs` or a new `app/dispatch.rs`; `app/group_menu.rs`;
`app/input.rs`; `crates/app-core/src/tests/dispatch.rs`.

Produce: `views::RouteRow`, `views::RouteReport`, and whatever the hub needs to
draw sortie rows beside route rows. `Mode::Dispatch`, `Mode::SortieSquad`,
`Mode::RouteCargo`, each added to `Mode::is_battle`'s exhaustive match and to
`input.rs`'s dispatch tables (the compiler forces both). A `"Dispatch"` row on
`BASE_ROWS` in the `"Caravan"` row's shape, `available` on
`dispatch_reach() != NoRelay`. Uppercase keys only for actions —
`[S]`/`[C]`/`[X]` on the hub — since lowercase is a row selector.

Tests: the base-menu row appears only with a Relay; the row opens the hub;
`[C]` reaches the cargo picker and Enter dispatches; a refusal shows on
`App::refuse`'s one door.

### 6 — gui

Files: new `crates/gui/src/render/dispatch.rs`; `crates/gui/src/render/mod.rs`.

Draw all three modes through `draw_popup`, `render/settlement_market.rs` and
`render/settlement_board.rs` as the pattern. Wire the row-source match, the
basket-build match and the draw dispatch. Add all three modes to `ALL_MODES`
(and to its documented exception list only if the census app genuinely cannot
stand a Relay up). They are popups, so `needs_status_banner` is **not** touched.

Tests: the census `every_screen_draws_a_refusal_exactly_once` stays green; a
row-width check for the widest shipped route row, since the popup body clips
vertically only and never horizontally.

### 7 — Documentation

`CHANGELOG.md` section and the workspace version bump at the merge, not on the
branch. The three seam writes, in order: the argument into `docs/seams.md`, the
trap into `.claude/skills/seams/references/sorties.md`, the one-sentence rule
into `CLAUDE.md` (and `cp` it to `AGENTS.md` — they are gitignored twins).
`docs/superpowers/INDEX.md` gets the phase. No `assets/*/README.md` changes:
no schema field moves.

## Out of scope

Standing-route *severing by the town* (a Hostile town cutting a route rather
than taxing it), town-sourced raids, hostile patrols, and settlement aid
rewards. All deferred by the spec.
