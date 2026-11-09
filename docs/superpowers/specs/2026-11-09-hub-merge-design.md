# Hub merge, named tabs and mouse — design

**Date:** 2026-11-09 · **Branch:** `worktree-hub-merge-design`
**ADRs:** [0045](../../adr/0045-two-hubs-split-by-subject-and-tab-switches-tabs.md) (hubs and nav), [0046](../../adr/0046-pointer-targets-are-registered-while-drawing.md) (pointer layer). Both Agreed (user, 2026-11-09).
**Mockup:** https://claude.ai/artifact/C55zQrJ26S9kwVMaMGoDiX

## Intent

The user could not tell what `b` → Trade → Contracts was, or why it sat under Trade.
The cause is structural. Pages ended up in hubs by history, `b` keeps the old base menu's verb lists as launcher pages, and one subject is spread over several rail entries and hubs.
The user wants **one key for things that are not about a specific character or entity (`b`) and one for things that are (`i`)**, done cleanly with good UI practice, and **mouse support with hover** built as a reusable layer.

## Goals

- `b` and `i` split by subject, with the Sites hub folded into `b` and the structure hub folded into `i`.
- The rule for both: **the rail lists nouns, tabs are aspects of the highlighted noun, and actions are keys on a tab.** No launcher pages.
- One nav rule for every hub: arrows move the cursor (rail and page), and `Tab`/`Shift+Tab` switch tabs (user, 2026-11-09).
- Tabs are named on screen.
- Click and hover on rail entries, tabs, rows, keycaps and scroll markers, through a shared pointer layer. The sprite editor and map move onto that layer too (user, 2026-11-09).

## Non-goals

- No new remote gameplay. Reading moves; every action keeps its own reach check and refusal.
- No change to screen *content* beyond what a move forces. Colours, fonts and row styles stay.
- No mouse on in-page widgets that need their own targets (Duties grid cells, level sliders, Transfer basket `+/-`) beyond what the generic row click gives. The keyboard covers them. They come later.
- No mouse for the battle screen or popups that are not hub tabs.
- The Esc game menu keeps its own hub. It is about the game, not the world.

## Anti-goals

- Hiding a page because of where the party stands. Rail positions must stay stable for muscle memory.
- A second spelling of any action. A click is the key it stands for.
- Text that clips. Every new string (tab strip, rail titles) is under the width censuses, per the rule that gui text never clips or wraps.

## The two hubs

### `b` — the operation

| Section | Page | Tabs (in order) | From today |
|---|---|---|---|
| BASE | Overview | — | Overview |
| | Build | Deploy · Build orders · Dig · Floor | Build launcher's *Deploy*; Build orders page |
| | Production | Output · Machines · Power · Recipes · Compile | Production; Recipes page; Compile launcher |
| | Stock | Stock · Work orders | Stock; Work orders |
| | Staff | Wellbeing · Coverage · Discontent · Duties · Staffing | Staff; Duties; Staffing |
| | Research | Projects · Routines · Affixes · Weapons · Study | Research launcher's rows |
| | Breed | — | Breed (still only once a bay stands) |
| | Alerts | — | Alerts |
| JOBS | Contracts | Held · Board | Trade → Contracts popup |
| TOWNS | one per known town/city | Status · Goods · Routes · Jobs · Log | Sites hub |
| OUTPOSTS | one per outpost | Status · Crew · Stock · Log | Sites hub |
| TRAFFIC | Caravans | Schedule · In flight · Trader | Sites hub's Caravans; Trade → Caravan |
| | Sorties | — | Trade → Dispatch (sorties); its own entry (user, 2026-11-09) |

- **Contracts.** *Held* lists every held contract. Each row names its issuer ("Broker", or "for Ashford · hand in there") from `ContractRow::issuer_name`, which the popup never drew. *Board* is the Broker's offers, with today's header variants (off base, onboarding). The entry is present once a Broker stands or a contract is held, as the Trade row was.
- **The remaining launcher verbs** go to the entity they act on, in `i` (below): *Work a structure yourself*, *Upgrade*, *Demolish*, *Hold a program*.
- **`b` opens anywhere**, including the Stack, as now. Site pages are readable anywhere. Their actions refuse through `App::refuse` in the doors' own words (`WALK_TO_TRADE`, `WALK_TO_BOARD`, …). This reverses the Sites spec's "opens only at base" (user confirmed, 2026-11-09).
- **`w`** opens `b` on the site beside the party, else on the first TOWNS entry. `SITES_OPEN_REFUSAL` is deleted.

### `i` — one thing

| Section | Page | Tabs |
|---|---|---|
| YOU | You | Items · Tools · Phase keys · Routines · Perks · Manifests |
| PARTY | one per party member (user, 2026-11-09) | that member's views from today's Companions page, split into tabs (builder's call on the split) |
| PROGRAMS | Roster | as today's Programs: base staff stays one entry, never one per program (user, 2026-11-09) |
| | Downed | as today's Downed programs |
| BESIDE YOU | one per adjacent structure (`Game::adjacent_structures`) | that structure's structure-hub pages (Inventory · Recipe · … · Info) |

- Structure verbs from `b`'s launchers become keys on the structure's tabs: *Work it*, *Upgrade* and *Demolish* on Info, and *Hold* on a Holding Cell's Hold tab, which already exists. Builder's call on the key letters, under the key-hint census.
- The structure hub's "(n of m)" title and `Tab` cycling are deleted.
- **`c`** opens `i` on the first BESIDE YOU entry, or refuses as `c` does today when nothing is adjacent.
- `i` opens on the structure beside the party if there is one, else on its last page.

## Nav rule (ADR 0045)

| Key | On the rail | In the page |
|---|---|---|
| `↑/↓` | pick an entry | rows |
| `→` / Enter | enter the page | the page's own (column, level), else nothing / act |
| `←` | — | the page's own; from its leftmost position, back to the rail |
| `Tab` / `Shift+Tab` | next / previous tab of the highlighted entry | same |
| `[ ]` | step the rail | step the rail without leaving the page |
| `{ }` | jump rail section | same |
| Esc | close the hub | back to the rail |

- `GameKey::ShiftTab` is new. The gui maps Shift+Tab to it.
- `screen_claims` keeps its claims (Duties columns, level rows, downed drill-down, filters, confirms). `Left` now falls through to "back to the rail" rather than "previous tab".
- Keybar wording: `[ ] page`, `Tab tab`, `{ } section`. Code renames "screen" to "tab" where it means a page's sub-view (`HubPage::screens` → `tabs`), done in the phase that touches it.

## Named tabs

`frame::draw_title` draws `Title   One · [Two] · Three`, with the current tab marked in `SELECT_BG`/`PANE_TITLE`, as the mockup shows.
If the strip does not fit the title row, it falls back to the dots. A census measures every page's strip at 1280×720 and records which pages fall back.

## Pointer layer (ADR 0046)

- **`Painter::target(rect, PointerHit)`** records a target for this frame. The hub frame registers rail entries, tabs, rows (`Row::Item` ordinal), keybar keycaps and "↑/↓ n more" markers.
- **`crates/gui/src/pointer.rs`** is the one reader of egui pointer input. It resolves against the previous frame's targets and emits one `PointerEvent { hover, press/drag/up/double, button, wheel }` to `App::handle_pointer`. It owns "armed" (a button held when a screen opens does nothing) and click-vs-drag.
- **`PointerHit` moves to `app/pointer.rs`** and gains `Page(HubPageKey)`, `Tab(usize)`, `Row(usize)`, `Key(GameKey)`, `More(Direction)` and `MapTile(i32, i32)`, beside the existing `Cell`, `Swatch`, `Button`, `Subject`.
- **`App::pointer_hover`** replaces `App::map_hover`. `App::hovered(&PointerHit) -> bool` is how a renderer draws hover. Hover draws the row or rail entry in `SELECT_BG` at reduced strength, distinct from the cursor, as a builder's call within the palette (`hud::palette`).
- **Hits are keys:** `Page` focuses the rail and turns to it, `Tab` switches, `Row` selects, `Row` again or a double-click is Enter, `Key` calls `handle_key`, `More` scrolls, and the wheel is `↑/↓` on the hovered region.
- **Migration:** the sprite editor (strokes, swatches, buttons, picker) and the map (hover label, click-to-walk/aim) register targets and route through the reader. `SpritePointer`, `MapPointer`, `handle_sprite_pointer`, `handle_map_pointer`, `sprite_hits` and `MapClickLayout`/`Fx::set_map_click` are deleted. Their existing tests are the regression gate. They are moved, not rewritten. The map's camera ease is why it stashed the layout, and registering during the draw keeps that property.

## Phases (one release each)

1. **Nav rule and named tabs**, across every existing hub with no rail changes. Add `ShiftTab`, the tab strip, its census and keybar wording. Until phase 3 the structure hub's next-structure cycle moves from `Tab` to `{ }`.
2. **`b` regrouped:** sections, merged pages with tabs, launcher pages removed, Contracts page with issuer, Sites hub folded in, `w` as shortcut. The Trade/Caravan/Dispatch verbs land per the table.
3. **`i` regrouped:** sections, BESIDE YOU from `adjacent_structures`, structure verbs as keys, structure hub folded in, `c` as shortcut.
4. **Pointer layer and hub mouse with hover.**
5. **Sprite editor and map onto the pointer layer.**

## Constraints

- app-core owns row counts and behaviour. gui draws, and has no `match app.mode` for what a click means.
- Every hub walks `App::handle_hub_nav`. The rail is `HubKind::pages` and nothing else (seams-screens).
- `Row::Item` ordinal == app-core selection index on every hub tab. A census asserts it per tab mode.
- Text never clips. Width censuses cover the tab strip, rail titles and new rows.
- `Painter` stays the only graphics-naming file. `target` takes local `Rect`.
- Update `.claude/rules/seams-screens.md`, `.claude/rules/drawing-seam.md`, the `seams` skill and the `seam:` graph entries in the phase that changes each.
- `CHANGELOG.md` per release. `assets/help` / manual are not edited (standing rule).

## Tests (intent)

- `hub_census` and `shared_screens_are_pinned` updated per phase. A rail census asserts each hub's sections and order.
- Nav: `Tab`/`ShiftTab` cycle tabs from rail and page, `←` from the leftmost position returns to the rail on every tab, and the Duties columns never switch tabs.
- Contracts held row shows issuer. A town job on the Contracts page and the town's Jobs tab is the same contract (abandon from either).
- Every launcher verb is reachable from its new home (a census over the old `group_menu` tables, deleted with them).
- Pointer: headless egui frames (the `handle_map_pointer` test precedent). Hover follows the previous frame's targets, a press while armed-false does nothing, double-click is Enter, a click on a keycap equals its key, and the wheel equals `↑/↓`.
- Migrated sprite editor and map tests pass unchanged in intent.

## Answered questions (user, 2026-11-09)

1. `b` opens anywhere, like today, including the merged site pages.
2. PARTY has one entry per party member. Base staff stays a single Roster entry.
3. Sorties is its own TRAFFIC entry.

## Calls made after the builder check (Claude's, for the user to check)

- **A tab's own `Tab` use moves to `←/→`.** Inside a hub, `Tab` is always the hub's. The Manifests tab's `ManifestTab` toggle (`toggle_manifest_tab`, `inspection.rs`) becomes the page's own `←/→`. `Tab` on screens outside the hubs (map log pane, world map, canvas editor, menus) is unchanged.
- **The tab model stays ADR 0043's:** one `Mode` per tab, with the page key held in `App`. Entries in one section share their tab list. Moving to another entry keeps the current tab when that entry has it, otherwise its first tab. No per-entry tab memory.
- **When the held page leaves the rail while open** (structure demolished, contract delivered, bay gone), the hold moves to the next surviving entry in the same section, else the section's previous one, else the hub's first page. Focus stays where it was (rail or page). On reopen a vanished key is forgotten, as 0043 already says.
- **Phase 1 leaves the structure hub's `Tab` cycle alone.** `c` keeps `Tab` until phase 3 folds it into `i`, so its tests are edited once. Phase 1's nav rule applies to every other hub.
- **`{ }`, like `[ ]`, yield to a typing screen.** A tab with an applied or active text filter claims Char input through `screen_claims` first.
- **Pointer input is egui's**, including the wheel (`smooth_scroll_delta`, as `Frontend::wheel_acc` already reads). Keys stay `bevy::KeyCode`.
- **Rows register only `Row::Item`s.** A tab with no items (Duties grid, prose pages) has no row targets, and wheel scrolls it as `↑/↓` would.
- **Contracts from the Stack is `b` → Contracts**, since `b` opens there. The page's presence reads `Game::broker_reach` and `active_contracts` inside `HubKind::pages(&App)`, which already has the game.
- **A popup-style tab** (if the builder keeps one opening over the hub) owns Esc and `Tab` until it closes, through `screen_claims`, like today's confirms.
- **Text that names old paths** (`b → Trade`, `w` as its own hub) in `assets/` or key hints is swept in the phase that moves the page. The manual is not edited.

## Left to the builder

Key letters for structure verbs on Info, hover tint value, exact tab names where a mode's title is long, and whether a popup-only screen (Research picker, Compile) draws as a tab in the content rect or opens over the hub. The census decides fit.
