# Sites hub — design

**Date:** 2026-11-06 · **Branch:** `worktree-sites-hub`

## Intent

One screen for managing every town, city and outpost the party knows
about: how each one is doing, what it has to trade, what is travelling to
or from it, when the next caravan is due, and what happened there
recently. It also gathers every interaction the rules allow from where
the player is standing. Today this is spread across the world map, the
Relay hub (Dispatch), Alerts, and the visit pages that open only when you
walk onto a site.

## Goals

- A new hub on the shared rail + tab-page UI (`app::hub`, `HubKind`, gui
  `render/base_report/` frame), opened by its own key.
- The rail has one entry per known site: towns and cities first, then
  outposts, then a Caravans entry. Tabs within a site's entry show its
  status, goods, routes, jobs and log.
- **Town goods are what you last saw**: a snapshot of the shelf taken on
  each visit, shown with how long ago it was seen. A research unlock
  replaces the snapshot with the live shelf for every known town.
- **A short log per site**: a capped list of recent events, saved with
  the game.
- **When caravans are coming**: the base's visiting trader (who, and when)
  and every route in flight to or from each site.
- Every interaction the rules already allow from the player's position,
  in one place.

## Non-goals

- **No new remote gameplay doors.** This screen gathers existing actions;
  it does not make buying, accepting jobs, gifts, posting or repairing
  work from a distance. Relay trading would be a separate design.
- No aborting a trip already in flight (no engine door for it exists).
- Unvisited towns do not appear. The compass already withholds their
  names.
- No retirement of the World map or Compass. Dispatch's route-destination
  section moves here (see Placement).

## Anti-goals

- Showing ticks. The game has never shown the player a tick; every time
  on this screen is banded into words (see Time).
- A second source of truth: every figure comes from an engine view, so
  the screen and the visit pages cannot disagree.
- Making visits pointless: without the research, a town's goods are only
  what you saw in person.

## Where it opens and what it allows

**User's decision:** the hub opens **only at base**, and offers
**more actions when the party is in range of a site**, using the range
the game already defines.

| Where the party is | Hub opens? | What it allows |
|---|---|---|
| On base floor (`dispatch_reach() == AtRelay`, or base floor with no Relay) | yes | Every remote door that exists: sever a standing route, abandon a town's contract, recall outpost crew. With a Relay: dispatch a cargo route or an outpost route, and fast-travel to an Allied town (`travel_to_settlement`). |
| In range of one site (`settlement_reach` / `standing_at_outpost`, Chebyshev 1) | yes, open on that site | Everything above, plus that site's at-site doors: town market basket, town job board (accept/deliver), program gift, relay home; outpost post, repair, take. |
| Anywhere else | no; refused through `App::refuse` with a line saying it opens at base or at a site | — |

**User's decision:** when the hub is opened beside a site, the rail
still lists every known site, and the site you're standing at is
selected on open. At-site actions are enabled only on that site's entry;
the others show the base-level actions. The mockup
(https://claude.ai/artifact/EaLKiWom6so1hWdwa9Uym2) shows each case.

**My call (from the builder check):** at-site actions **launch the
existing screens** rather than living inside the hub as tabs. Enter (or
the action's key) on a hub tab opens `Mode::SettlementMarket`,
`Mode::SettlementBoard`, `Mode::OutpostPost` or `Mode::Transfer`, and
Esc returns to the hub tab it was opened from. Each of those modes gets
a "came from" so its Esc goes back to where it was opened. Their
hardwired exits today (the market's Esc to `Mode::Settlement`,
`RouteCargo`'s return to `Mode::Dispatch`) become the fallback for that
value, not a second code path. Putting them inside the hub as its
own tabs would make a site's tab `Mode` change with the party's
position, and `hub_for` would claim the market opened from the visit
page. The visit pages that open on walking into a site stay as they
are.

Actions that aren't available are still drawn and are refused when used,
with the engine's own words (`App::refuse`, the seams-screens rule).

## Rail and tabs

Rail: one entry per known site, as in the mockup (**user's decision**,
chosen over a fixed Towns/Outposts/Caravans rail and over the
structure hub's cycle-sites shape). The hub framework can't do this
today: rail titles are static, rows have no glyph, and the page is found
from the `Mode`, which every town shares. **[ADR 0043](../../adr/0043-a-hub-rail-is-runtime-data-and-its-page-is-held.md)
(Proposed) makes every hub's rail runtime data and holds the current
page in `App` by key. The user chose this over a two-hook extension for
long-term flexibility. The work is two steps: first move the four
existing hubs onto the new model with no visible change, then build the
Sites hub on it. Nothing is built until the ADR is Agreed.** Each rail row carries the
glyph, the site name, and a section header (Towns and cities /
Outposts / Traffic). Order: settlements by name, then outposts by
name, then **Caravans**. The `!` mark (`HubKind::marks`) flags a site that
needs attention: a dark or raided outpost, a stalled route, a gift ready,
or a town that has turned Hostile.

**Town or city** tabs (each tab a screen, Left/Right between them):

1. **Status**: kind, specialty, temperament, standing band and what it
   means (service, standing routes, raids/patrols, garrison), vitality
   for cities, growth outlook for towns, gift readiness, travel
   availability. Built from `settlement_report`, `standing_band`,
   `gift_available_in`, `has_relay`/`hosts_a_relay` and the band queries,
   *not* from `settlement_aid_lines`, which are reach-gated.
2. **Goods**: the shelf. Without the research: the last-seen snapshot and
   a "seen …" age line, or "Not seen yet" if never captured. With the
   research: the live shelf. At the site: the market basket, buy/sell
   enabled.
3. **Routes**: routes in flight to this town (leg, cargo, time left,
   repeating or not, stalled). Sever a standing route. At base with a
   Relay: dispatch a new route (the `RouteCargo` basket).
4. **Jobs**: active contracts issued by this town (from
   `active_contracts`, filtered by issuer). Abandon from anywhere the hub
   opens. At the site: the board (accept/deliver).
5. **Log**: this site's recent events, newest first.

**Outpost** tabs:

1. **Status**: `outpost_report`: tier, growth, trend and reason,
   integrity, stock against cap, yields, next-tier requirement, dark.
2. **Crew**: crew rows; recall from anywhere. At the site: post (the
   `OutpostPost` picker) and repair.
3. **Stock / Routes**: stock lines; outpost route in flight; dispatch an
   outpost route (Relay); sever. At the site: take (Transfer).
4. **Log**.

**Caravans** (one entry):

- The **next visiting trader**: which trader and when, banded. If one is
  docked now, a line saying so and how long it stays. Shelf preview is
  left to the builder (the shelf is deterministic, so a preview spoils
  nothing, but `caravan_shelf` draws gear and needs `&mut self`).
- Every route in flight, across all sites, in one list.

## Last-seen shelf

- Captured when the party reaches a town (on the `take_visit` /
  `PendingVisit` path that opens `Mode::Settlement`) and when the market
  screen is shown. Stored per `SettlementKey`: the offer rows (item, price
  as shown at capture, quantity) and the tick they were seen.
- Prices are frozen at capture. The tab says they are as seen, not
  current.
- New save state, additive behind `#[serde(default)]` (no
  `SAVE_FORMAT_VERSION` bump). Needs a save → load test (memory:
  a RON round trip cannot catch a skipped field).

## Live-shelf research

- A new `assets/research/<id>.ron` using **only existing fields** (no new
  schema field, so no ADR), checked through `Game::is_researched` against
  a named id constant in `tuning/` (the `WEAPON_TREE_OPENER` precedent).
- When researched, the Goods tab shows `settlement_shelf(key,
  settlement_epoch)` read-only for every known town; buying still needs
  the site.
- Name, description, cost, materials, `min_zone` and `requires` are left
  to the builder. Follow the plain-language and no-occult naming rules.
  Update `assets/research/README.md` if it lists entries.

## Per-site log

- A new engine store, keyed by site (`SettlementKey` for towns, tile for
  outposts), each a ring capped by a `tuning/` constant (suggested ~12).
  Saved, additive, `#[serde(default)]`.
- Entries carry a kind, the tick, and engine-composed text. The gui never
  composes event text, per the seams-screens rule.
- **Writers**, one call each at the door that already performs the
  event:
  - Towns: visit (first and later), trade committed (net Credits), band
    crossing (`adjust_standing`, crossings only), contract accepted /
    delivered / abandoned, gift received, grew into a city, route
    delivered (proceeds), route preyed on, raid on base by this town.
  - Outposts: founded, tier reached, trend change (the same transitions
    that post alerts), raided, went dark, repaired, crew posted /
    recalled, stock taken, route hauled stock home.
- Each entry shows a banded age ("just now", "recently", …).

## Time

All durations on this screen go through **one** engine banding helper
(the `memories::age_phrase` / `settlement_relations::wait_line` precedent),
for both ages ("seen recently") and waits ("arrives soon"). The builder
picks the band edges. Existing tick leaks (Dispatch, Caravan,
`outpost_route_line`) are out of scope, except that any string this
screen reuses must not carry ticks: compose a banded variant rather than
reuse `OutpostReport::route` as is.

## Placement and overlap

- **New `Hub` variant** (`Hub::Sites`), a `HubKind` impl in
  `app/sites_hub.rs` (one module per mode/hub), `hub_last` slot, opened
  from the zone map by a **new key**. Not `p`: it was retired when the
  party hub folded into the inventory, and
  `tests/party_hub.rs::p_opens_nothing_on_the_map` holds it unbound.
  `w` is free. Builder's call among free keys; the key-hint case must
  match the handler case.
- **Dispatch (Relay hub)**: its route-destination section is superseded
  by the per-site Routes tabs. Removing that section from Dispatch is in
  scope (no backwards-compat cruft); Dispatch keeps sorties. If the
  builder finds removal ripples further than one section, keep it and
  flag it.
- Base report's Trade page could gain a row that opens the Sites hub;
  builder's call.

## Constraints (from project rules)

- The renderer never touches `World`; every figure is an engine view
  (`views/`), and app-core owns row counts. Per-row transforms live in
  the engine (seams-screens).
- Lists are highlight + Enter, no row digits. Hub nav is
  `App::handle_hub_nav` only; any screen that keeps arrows/Esc registers
  in `App::screen_claims`.
- A read-only page never advances the game.
- New engine logic is plain Rust (structs plus pure functions); the log
  is a plain resource, not ECS components. Constants go in `tuning/`.
- Text never clips or wraps (gui-text rule): long names and log lines
  need a width census like the docked-panel census.
- Gates: `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo
  test --workspace`. Run `balance_sim` if `tuning/` balance constants
  move (they shouldn't).
- Spans engine, app-core and gui, plus a save-format addition → full
  spec-and-plan pipeline. The CHANGELOG is updated at release.

## Left to the builder

- Exact band edges and wording of the time helper.
- Tab names and their final order within the shape above.
- Research id, cost and prerequisites.
- Log cap and exact entry wording.
- Whether Caravans previews the next trader's shelf.
- The key, if `p` collides with something found during the build.
