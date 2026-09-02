# In-game help — design

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

TODO #32. A player-facing manual read inside the game: an intro page and a
set of topics reachable from it, authored as files the user edits directly.

## Settled before design

Four decisions taken in the brainstorm, recorded so they are not relitigated:

1. **Markdown subset in `assets/help/*.md`**, not RON. A page is prose, not
   structured data — there is nothing to validate beyond "does it have a
   title", so RON buys only house consistency while costing an escape on
   every quote in every paragraph.
2. **`?` opens the manual.** Today's key-bindings card becomes one page of
   it rather than a second help surface the player has to know about.
3. **System, intro and a few pages ship**; the rest of the topic list is
   authored into the directory afterwards, which is the point of the format.
4. **No relationship to `docs/manual.md`.** That stays the exhaustive
   out-of-game reference and stays carved out of the doc-update obligation.
   No generator, no shared source, no sync gate.

## The format

The whole grammar, and deliberately no more:

| Line | Means |
|---|---|
| `# Title` on the first non-blank line | the page title |
| blank line | paragraph break |
| `- text` | a bullet |
| anything else | paragraph text, wrapped at read time |
| `[label](topic-id)` inline | renders as `label`, and registers a further-reading link |

No emphasis, no headings past the first, no nesting, no tables, no code
fences. A `#` heading anywhere but the first line is a paragraph like any
other, so a stray `#` cannot silently retitle a page.

Ordering and identity come from the filename: `10-start-here.md` is order
`10`, id `start-here`. Reordering is renaming; there is no front matter and
so no second parser. Ties in `order` break on id, so the index is stable.

A malformed page — no title, or an empty body — is **skipped with a
warning**, never a panic, matching `SpeciesDb::load_dir` and every other
content db.

### The link rule

`[label](topic-id)` does two things from one authoring gesture: the prose
reads as `label`, and `topic-id` joins that page's further-reading list,
deduped, in first-appearance order. A link whose target is not a real page
is dropped from the list and reported as a load warning — a dead link must
not render as a row that refuses when picked.

This is why there is no `see_also:` field. Writing the cross-reference where
it belongs in the sentence is the only place it is written.

## Where each piece lives

**`crates/engine/src/help.rs`** — `HelpPage { id, title, order, blocks,
links }`, `HelpDb::load_dir(&Path) -> (HelpDb, Vec<String>)`, and
`help::page_rows(&page, columns) -> Vec<String>`, which is where the wrap
happens. Engine-side because of the seam in CLAUDE.md: a read-only screen's
row count is owned by app-core, so a per-row transform done in the renderer
opens the screen on rows that are not drawn.

**`crates/engine/src/text.rs`** — `pub fn wrap(text, columns)`, lifted out
of `render/popup.rs::wrap_text`, which becomes a call. One definition, since
a second implementation here is exactly the copy CLAUDE.md has been bitten
by four times. gui already depends on the engine directly, so this adds no
dependency edge.

**`App`** — `help_db: HelpDb`, loaded in `App::new` beside `achievement_db`,
from `assets_dir.join("help")`. Held on `App` rather than on `Game` so the
manual reads with **no run in progress**; nothing in this change puts it on
the main menu, but a `Game`-owned db would make that a rewrite rather than a
menu row.

**`Mode::Help`** becomes the index. **`Mode::HelpPage`** is one page, with
`App::help_stack: Vec<String>` as the reading trail.

**`crates/gui/src/render/help.rs`** — both screens, through `draw_popup`.
`HELP_ROWS` and `draw_help` leave `render/meta.rs`.

## Navigation: the index is a menu, a page is a document

The load-bearing rule, because the two screens answer to different idioms:

- **The index** is an ordinary numbered menu — digits, Up/Down + Enter, Esc
  closes back to wherever `?` was pressed. Identical to every other menu, so
  more than nine topics is already solved by `menu_shortcut`.
- **A page** follows the *history* screen (`L`), not the menu idiom: Up/Down
  scrolls the prose and Enter does nothing. Further-reading rows are chosen
  by typing their label.

A page cannot be both. Selection-driven scrolling keeps the *selected* row
visible, so a menu-idiom page with its links at the bottom would open
scrolled to the end of the prose. Putting the links at the top instead makes
long prose unreachable. Scroll and select are the same key on this screen and
scroll is what a document needs, so links take a typed label.

Esc on a page pops `help_stack`; an empty stack returns to the index. The
trail is what makes "read three links deep and come back" work.

**`?` stops closing on any key.** It is navigable now. This is a real change
to an existing reflex and is called out in the changelog.

## Phases

Each is a green commit on its own.

### Phase 1 — engine: wrap, parse, load

Extract `text::wrap`; repoint `render/popup.rs::wrap_text` at it. Add
`help.rs`: the block/span types, the parser, `load_dir`, `page_rows`.

Tests: title taken only from the first non-blank line; bullets and blanks
survive the round trip; a link renders as its label and lands in `links`
once when it appears twice; a page with no title is skipped and warns; a
dead link is dropped and warns; `page_rows` wraps at the column count and
never emits a row wider than it. No UI in this phase.

### Phase 2 — content

`assets/help/20-controls.md` carrying today's `HELP_ROWS` text verbatim, plus
`10-start-here.md` (ICE and Power), `30-zones.md` (what a zone is and what
crossing one costs) and `40-getting-stronger.md` (how to progress, and what
to do when stuck), cross-linked so the further-reading list is exercised by
real content rather than by a fixture. `assets/help/README.md` documents the
grammar as the schema reference, as every other asset dir does.

Nothing is deleted from Rust in this phase — the old screen still draws.

Tests, in `crates/engine/src/tests/assets.rs` with the other censuses over real
assets: every shipped page parses; every link resolves; no page carries more
than nine links, since a typed label is how one is followed.

### Phase 3 — app-core: navigation

`help_db` on `App`; `Mode::HelpPage`; `help_stack`; `help_index_rows` and
`help_page_rows`; `handle_help_key` rewritten to the menu idiom and
`handle_help_page_key` to the document idiom.

Tests: `?` opens the index; picking a topic opens it; a typed link label
pushes the trail; Esc pops one level and then closes to the index; Up/Down
scrolls a long page without changing which page is open.

### Phase 4 — gui: the screen, and the census moves

`render/help.rs` draws both modes; the two arms in `render/mod.rs` point at
it. Delete `HELP_ROWS` and `draw_help` from `render/meta.rs`.

The easter-egg census moves with the content it guards: it now reads
`assets/help/*.md` and asserts no page names `W`, `T` or `Z` as a key, on
whitespace-delimited tokens exactly as today. It belongs in the engine's
`src/tests/assets.rs`, and it now protects against the *user* editing a page as
well as against a developer editing a const. The Excavation-key test moves
the same way.

Tests: no rendered row overflows the popup, measured through
`paint::with_painter` like the crafting and build screens; every page's rows
stay inside the scrollable body.

### Phase 5 — docs

`CHANGELOG.md` (noting `?` no longer closes on any key), a `docs/seams.md`
entry for the link rule and the menu/document split, and the matching short
rule in CLAUDE.md. `crates/engine/EASTER_EGGS.md` is repointed at the asset.
`docs/manual.md` is untouched, per decision 4.

## Out of scope

Search, main-menu entry, contextual help (`?` from inside a screen opening
that screen's topic), and images. The format is chosen so search is a later
addition over `HelpPage` rather than a rewrite: pages are flat titled text.
