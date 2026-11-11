# Hub rough spots — design

**Date:** 2026-11-11 · **Follows:** `2026-11-09-hub-merge-design.md` (0.21.55–0.21.57), ADR 0045 (hub nav), ADR 0046 (pointer layer)

## Why

The user found the merged `b` / `i` reports awkward in play. A screenshot pass
(template `settlements`) found one defect and four rough spots. The worst: on
`b`, merely highlighting Research in the rail opens the research tree
full-screen over the hub, so scrolling the rail past it throws you out of the
report. Everything else here is polish on the same screens.

Success: every page of `b` and `i` lives inside the hub frame, the rail is
always visible, the key bar is the one place keys are explained, and nothing
on the rail is ambiguous.

## Goals (user's decisions)

1. **Research tree inside the page.** On `b` → Research (Projects, Routines,
   Affixes, Weapons tabs), the tree view draws in the hub's content rect, beside
   the rail and under the title/tab strip — never over the whole window. It
   keeps its existing scroll-to-cursor behaviour, now measured against the
   content rect. `G` still toggles list view in place. The rail cursor
   resting on Research shows the tree but takes no keys; `→`/click enters it;
   Left from the tree's left edge and Esc return to the rail, as ADR 0045
   already says for every page. The tree's own header ("No research project —
   pick one", `[G] list · Arrows move · Tab tree · Enter work`) and its
   detail panel fit inside the content rect; its key hints move to the hub
   key bar.
2. **b owns base staff; i's Roster is party + a compact bench.** Roster shows
   the party in today's full detail. Below it, one line per program you could
   bring into the party (today's "Base staff" group), with `P` still adding
   one. Wellbeing/coverage/duties/staffing detail is on `b` → Staff only. The
   other role groups (sortie, posted, under study, held, base AI, visiting
   the bay, carried as weapon) stay as they are today unless they are base
   staff in disguise — builder's call, stated in the plan.
3. **Towns are told apart by direction.** A town's rail label is its name
   plus its compass heading from the base, `"Tally Yard NE"`, using the same
   `game::stack::heading` that names outposts (`default_outpost_name`). Every
   town gets the suffix, not just duplicates, so a label does not change
   when a second same-named town is found. The `s`/`M` kind glyph leaves the
   rail; the town's size is on its Status tab (already there). Display only:
   no save field.
4. **One place for key hints.** A hub page draws no key-hint footer of its
   own. Row builders shared with the old popups (`push_build_menu_footer` and
   the scout's list: breeding, dispatch, caravan, routines, farm and machine
   recipe, contracts) drop the footer when drawn as a hub page; the key bar
   carries those keys via `screen_keys(mode)`. Popups reached outside a hub
   keep their footers.
5. **No echoed tab names.** When a page's first tab has the page's title
   (`Roster [Roster] · Errands`), the strip still shows every tab but the
   title is not repeated — builder picks between renaming the first tab
   (e.g. Roster → "Party") and a draw rule; prefer renaming, one place.
6. **Consistent key-bar words.** The rail is called one thing everywhere:
   today it is "← menu", "Esc menu" on a page and "Esc back" on the rail.
   Use "list" for the rail ("← list", "Esc list") and "Esc close" on the
   rail. `Tab`/`{ }` are already hidden where they do nothing — keep that.

## Non-goals

- No change to which keys exist or what they do (user kept all five nav
  kinds).
- No new hub, page, or section; no change to `b`/`i` page order.
- No town renaming in the engine and no save-format change.
- The Stack research overlay reached outside `b` (if any) is not restyled
  beyond what sharing the draw function forces.

## Anti-goals

- A page that covers the rail again, for any mode.
- A key that stops working because its hint moved (every footer key must
  appear in the key bar for that mode — census test).
- Staff detail duplicated back into `i`.

## Constraints

- `render/` draws only through `Painter`; the tree takes a `Rect` origin
  (drawing seam: panes take their origin from the caller, never `0.0`).
- Pointer targets for tree cells register in the draw that places them
  (ADR 0046); a click on a cell is the key that selects/works it.
- `App::screen_claims` stays the one place the tree claims arrows/Esc.
  Its research arm today keys off `research_tree_shown()` — keep it, and
  make sure the rail-resting case (`hub_on_rail`) claims nothing.
- Two crates (app-core, gui) plus a one-line engine label change → plan
  pipeline; TDD; `cargo test --workspace`, clippy `--all-targets`, fmt.
- CHANGELOG entry; seam docs updated where they describe the tree drawing
  over the hub (`render/mod.rs` comment near `research_page_rows`,
  `seams-screens` if it mentions it).

## Calls made after the builder check (author's, not the user's)

- **One draw function, given a `Rect`.** `draw_research_graph` and
  `geometry` take the content rect instead of `screen_w/h` and stop painting
  a full-window backdrop. If any caller outside the hub remains, it passes
  the window rect; if none does, there is no second path to keep.
- **Rail-resting: draw and claim are split.** On the rail the tree (or the
  list view, if `G` chose it) draws with its selection frozen; `screen_claims`
  claims nothing while `hub_on_rail`. `→`/click enters through the existing
  page-entry path. Today `research_tree_shown()` feeds both draw and claim;
  the claim arms gain `&& !self.hub_on_rail` (the Esc/brace arm already has
  it; the Left arm does not).
- **Footers:** the census covers every key a removed hub-page footer named:
  each must appear in `screen_keys(mode)` for that mode. A footer key missing
  from the bar is added to it. A key that only exists in some states shows
  in the bar the same way the footer showed it (the bar is already per-mode).
  The plan lists each footer builder and whether its mode is a hub page.
- **Town heading:** measured from `Game::anchor_position()` (one base). A
  town at `Heading::Here`, or with no anchor, gets no suffix.
- **Roster:** only the "Base staff" group becomes the compact bench. Every
  other role group keeps its current rows — none of them is base staff.
- **Roster's first tab is renamed "Party"** (title stays Roster); the
  plan greps tests, help markdown and keybar censuses for the old label.

## Left to the builder

- How the tree's detail panel splits the content rect (`PANEL_FRACTION`
  against a narrower width) — must stay readable at the minimum window.
- Whether the town heading lives in `site_rail` (engine) or `site_pages`
  (app-core); engine is preferred because the tile and anchor are there.
- The exact wording of the compact bench line (name, level, HP, role).
