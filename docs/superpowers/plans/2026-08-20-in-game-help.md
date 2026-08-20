# In-game help — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` or
> `superpowers:executing-plans` to implement this task-by-task. Steps use
> checkbox (`- [ ]`) syntax.

**Goal:** A player-facing manual read inside the game — an intro page and a
set of topics — authored as markdown files the user edits directly.

**Architecture:** An engine content db mirroring `achievements.rs`, held on
`App` so it reads with no run in progress; app-core owns navigation and row
counts; one gui screen draws both modes through `draw_popup`.

**Tech Stack:** Rust, 4-crate workspace, `bevy_ecs` (engine), `bevy` +
`bevy_egui` (gui). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-08-20-in-game-help-design.md` — read
it first; this plan argues from it and does not repeat its reasoning.

## Global constraints

- CLAUDE.md governs. Read it. In particular: TDD with the failing test
  first, `cargo fmt` and `cargo clippy --workspace` after every change,
  comments explain *why* and never *what*, and no backwards-compat shims.
- **Do not write the code into this plan's steps.** Each task names the
  files, the interface it must produce, the intent of each test and the gate
  to run. Deriving the implementation is the implementer's job.
- **No `git push`.** Commit freely on the branch `in-game-help`; releasing
  is the user's call.
- Malformed asset files are **skipped with a logged warning**, never a
  panic — follow `SpeciesDb::load_dir`.
- `assets/*/README.md` is the schema reference and is updated in the same
  change as the schema.
- Do not touch `docs/manual.md`, `README.md`, or `TODO.md`.

---

### Task 1: `text::wrap` in the engine

The wrap has to be engine-side because app-core owns a read-only screen's
row count, and a second implementation is the copy CLAUDE.md has been bitten
by four times.

**Files:**
- Create: `crates/engine/src/text.rs`
- Modify: `crates/engine/src/lib.rs` (declare the module)
- Modify: `crates/gui/src/render/popup.rs:584` — `wrap_text` becomes a call

**Interfaces:**
- Produces: `pub fn feral_processes_engine::text::wrap(text: &str, columns: usize) -> Vec<String>`

- [ ] **Step 1** — Move `render/popup.rs::wrap_text`'s body into
      `text::wrap` verbatim, along with its tests. Behaviour must not
      change: this is a move, not a rewrite.
- [ ] **Step 2** — Leave `popup.rs::wrap_text` in place as a one-line
      delegation, so its callers in `crafting.rs`, `building.rs` and
      `inventory.rs` are untouched. Do **not** repoint them; that is a
      wider diff than this task earns.
- [ ] **Step 3** — Gate: `cargo test -p feral-processes-engine text` and
      `cargo test -p feral-processes-gui`. Both green.
- [ ] **Step 4** — `cargo fmt && cargo clippy --workspace`, then commit.

---

### Task 2: the parser and the content db

**Files:**
- Create: `crates/engine/src/help.rs`
- Modify: `crates/engine/src/lib.rs` (declare and re-export)

**Interfaces:**
- Consumes: `text::wrap` from Task 1.
- Produces:
  ```rust
  pub const WRAP_COLUMNS: usize = 100;

  pub enum HelpBlock { Paragraph(String), Bullet(String), Blank }
  pub struct HelpLink { pub label: String, pub target: String }
  pub struct HelpPage {
      pub id: String,
      pub title: String,
      pub order: u32,
      pub blocks: Vec<HelpBlock>,
      pub links: Vec<HelpLink>,
  }
  pub struct HelpDb { /* private, sorted by (order, id) */ }

  impl HelpDb {
      pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)>;
      pub fn pages(&self) -> &[HelpPage];
      pub fn page(&self, id: &str) -> Option<&HelpPage>;
  }

  pub fn parse_page(id: &str, order: u32, source: &str) -> Result<HelpPage, String>;
  pub fn page_rows(page: &HelpPage, columns: usize) -> Vec<String>;
  ```

**Design points the implementer must not rediscover:**

- `HelpBlock::Paragraph`/`Bullet` hold **display text with links already
  flattened to their labels**. `[label](topic-id)` contributes `label` to
  the text and a `HelpLink` to `links`, in first-appearance order, deduped
  by target. `page_rows` therefore never sees link syntax.
- **Link resolution is a second pass in `load_dir`**, after every page has
  parsed — a link's target cannot be checked until the whole directory is
  known. An unresolvable target is dropped from `links` and reported as a
  warning. A dead link must never render as a row that refuses when picked.
- Filename is the identity: `10-start-here.md` → `order: 10`, `id:
  "start-here"`. A file without the `NN-` prefix is **skipped with a
  warning** rather than defaulted to order 0, so ordering is never
  ambiguous. Non-`.md` files are ignored silently.
- `page_rows` returns **prose only**. The further-reading rows are a menu,
  and menus are app-core's business (Task 4).

- [ ] **Step 1** — Write the failing tests first. Intent, one test each:
      title comes only from the first non-blank line, so a later `#` is
      an ordinary paragraph; bullets and blank lines survive the round
      trip; a link appearing twice yields one entry and renders as its
      label both times; a page with no title is an `Err`; `page_rows`
      wraps at the column count and emits no row wider than it; a bullet's
      wrapped continuation lines are indented under the text, not under the
      dash.
- [ ] **Step 2** — Run them, confirm they fail for the right reason (not
      "module not found" after the first).
- [ ] **Step 3** — Implement to green.
- [ ] **Step 4** — Write the `load_dir` tests against a `tempfile` dir:
      pages come back sorted by `(order, id)`; a file with no `NN-` prefix
      is skipped and warns; a titleless page is skipped and warns; a link to
      a nonexistent page is dropped from `links` and warns; a missing
      directory is an `Err` rather than a panic.
- [ ] **Step 5** — Green, then `cargo fmt && cargo clippy --workspace`.
- [ ] **Step 6** — Gate: `cargo test -p feral-processes-engine help`.
      Commit.

---

### Task 3: the shipped pages and their census

**Files:**
- Create: `assets/help/10-start-here.md`, `assets/help/20-controls.md`,
  `assets/help/30-zones.md`, `assets/help/40-getting-stronger.md`
- Create: `assets/help/README.md`
- Modify: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: `HelpDb::load_dir`, `parse_page` from Task 2.

**Content requirements:**

- `20-controls.md` carries the text of `crates/gui/src/render/meta.rs`'s
  `HELP_ROWS` **verbatim**, minus its trailing "Press any key to close" row,
  which is no longer true. Its layout is already hand-aligned in columns, so
  those rows are authored as bullets or short paragraphs that survive
  wrapping unchanged — check the widest line against `WRAP_COLUMNS`.
- `10-start-here.md` explains ICE and Power and what the player is doing at
  all. `30-zones.md`: what a zone is and what crossing one costs.
  `40-getting-stronger.md`: how to progress and what to do when stuck.
- The three prose pages cross-link each other with `[label](topic-id)`, so
  the further-reading list is exercised by real content and not a fixture.
- **No page may name `W`, `T` or `Z` as a key** — see
  `crates/engine/EASTER_EGGS.md`. Task 5 adds the test that enforces it.
- `README.md` documents the whole grammar: the five block rules, the
  filename ordering convention, the link rule, and what gets a page skipped.

- [ ] **Step 1** — Write the censuses in `tests/assets.rs`, beside the
      existing ones, and watch them fail against an empty `assets/help/`:
      every shipped page parses with no warnings; every link resolves; no
      page carries more than nine links (a typed label is how one is
      followed, and `menu_shortcut` runs out of digits at nine).
- [ ] **Step 2** — Author the four pages and the README.
- [ ] **Step 3** — Green. `cargo test -p feral-processes-engine assets`.
- [ ] **Step 4** — Nothing is deleted from Rust in this task; the old help
      screen still draws. Verify `cargo test --workspace` is green before
      committing, since `HELP_ROWS`' own tests must still pass.
- [ ] **Step 5** — Commit.

---

### Task 4: app-core navigation

**Files:**
- Modify: `crates/app-core/src/lib.rs` (`Mode`, the `App` struct, view types)
- Modify: `crates/app-core/src/app/lifecycle.rs:18-40` (`App::new`)
- Modify: `crates/app-core/src/app/menus.rs:119` (`handle_help_key`)
- Modify: `crates/app-core/src/app/input.rs:190` (the dispatch arm)
- Test: `crates/app-core/src/tests/menus.rs` (the index is a menu; fixtures
  live in `crates/app-core/src/tests/support.rs`)

**Interfaces:**
- Consumes: `HelpDb`, `HelpPage`, `help::page_rows`, `help::WRAP_COLUMNS`.
- Produces:
  ```rust
  // on App
  help_db: HelpDb,                 // private, loaded in App::new
  pub help_stack: Vec<String>,     // reading trail, page ids, top is current

  pub struct HelpIndexRow { pub title: String }
  pub struct HelpLinkRow { pub shortcut: char, pub label: String }
  pub struct HelpPageView {
      pub title: String,
      pub prose: Vec<String>,
      pub links: Vec<HelpLinkRow>,
  }

  impl App {
      pub fn help_index_rows(&self) -> Vec<HelpIndexRow>;
      pub fn help_page_view(&self) -> Option<HelpPageView>;
  }

  // Mode gains one variant
  Mode::HelpPage
  ```

**Design points:**

- Load in `App::new` beside `achievement_db`, from
  `assets_dir.join("help")`, with the same `.unwrap_or_default()`
  warn-and-carry-on contract. A missing directory leaves an empty index, not
  a failed start.
- `handle_help_key` **changes signature** to take a `GameKey` — the index is
  a menu now. Update the `input.rs` dispatch arm.
- **The index is a menu, a page is a document** (spec's rule). The index
  uses `selected_index`; the page uses `scroll` for Up/Down and matches a
  typed `menu_shortcut` label against its links. Enter does nothing on a
  page.
- Esc on a page pops `help_stack`; empty stack returns to `Mode::Help`. Esc
  on the index clears `help_stack` and closes to `Mode::Playing`.
- `Mode::HelpPage` joins the non-battle arm of `Mode::is_battle`
  (`crates/app-core/src/lib.rs:1151`) beside `Mode::Help`. That match is
  exhaustive, so a missing arm fails to compile rather than misbehaving —
  which is the point.

- [ ] **Step 1** — Failing tests first. Intent, one each: `?` from
      `Mode::Playing` opens `Mode::Help`; picking a topic opens
      `Mode::HelpPage` with that id on the stack; typing a link's shortcut
      pushes the target and leaves the first page beneath it; Esc pops one
      level rather than closing; Esc on the last level returns to the index;
      Esc on the index returns to `Mode::Playing` with an empty stack;
      Up/Down on a long page moves `menu_selected` without changing which
      page is open.
- [ ] **Step 2** — Run, confirm they fail.
- [ ] **Step 3** — Implement to green.
- [ ] **Step 4** — Gate: `cargo test -p feral-processes-app-core`, then
      `cargo fmt && cargo clippy --workspace`. Commit.

---

### Task 5: the gui screen, and the census moves with the content

**Files:**
- Create: `crates/gui/src/render/help.rs`
- Modify: `crates/gui/src/render/mod.rs:443` and its module list
- Modify: `crates/gui/src/render/meta.rs` — delete `HELP_ROWS`, `draw_help`
  and their two tests
- Modify: `crates/engine/src/tests/assets.rs` — the two tests land here

**Interfaces:**
- Consumes: `App::help_index_rows`, `App::help_page_view`.
- Produces:
  ```rust
  pub(super) fn draw_help_index(app: &App, painter: &Painter, m: &Metrics);
  pub(super) fn draw_help_page(app: &App, painter: &Painter, m: &Metrics);
  ```

**Design points:**

- Both draw over `draw_playing_base` through `draw_popup` with
  `PopupSize::Large`, as `draw_help` does today.
- The page's footer says what the keys are: Up/Down to scroll, a label to
  read on, Esc to go back. The index's footer is the ordinary menu one.
- **The two tests move to the engine, because the content moved to the
  assets.** They now read `assets/help/*.md` rather than a Rust const:
  no page names `W`, `T` or `Z` as a whitespace-delimited token; some page
  binds `m` to the Excavation plan. They protect against the *user* editing
  a page now, not only against a developer editing a const — which is the
  reason for the move and belongs in the test's doc comment.

- [ ] **Step 1** — Move the two tests to `crates/engine/src/tests/assets.rs`
      first, retargeted at the assets, and confirm they pass against Task
      3's pages. Then delete the gui originals along with `HELP_ROWS` and
      `draw_help`.
- [ ] **Step 2** — Write the failing width tests in `render/help.rs`,
      measured through `paint::with_painter` as `crafting.rs` and
      `building.rs` do: no rendered row of any shipped page overflows the
      `PopupSize::Large` body, and every row stays inside the scrollable
      body rather than under the scroll indicator. This is also what pins
      `help::WRAP_COLUMNS` to a width the popup can actually draw — assert
      it here rather than duplicating the constant in gui.
- [ ] **Step 3** — Implement both draw functions and the two `render/mod.rs`
      arms to green.
- [ ] **Step 4** — Gate: `cargo test --workspace`, `cargo fmt`,
      `cargo clippy --workspace`. Commit.
- [ ] **Step 5** — Launch it: `cargo run -- --template extraction`, press
      `?`, walk the index, follow a link, Esc back out. A green suite is not
      evidence of play. Report what it actually felt like — especially
      whether losing "press any key to close" is annoying.

---

### Task 6: docs

**Files:**
- Modify: `CHANGELOG.md`, `docs/seams.md`, `CLAUDE.md`, `AGENTS.md`,
  `crates/engine/EASTER_EGGS.md`

- [ ] **Step 1** — `CHANGELOG.md`: a new `## X.Y.Z` section. This is a
      minor bump, not a patch — it is a feature, and no save format moved.
      Say plainly that `?` no longer closes on any key.
- [ ] **Step 2** — `docs/seams.md`: an entry carrying the *argument* — why
      the index is a menu and a page is a document, why the link rule
      replaces a `see_also:` field, and why the wrap is engine-side.
- [ ] **Step 3** — `CLAUDE.md`: the one- or two-line rule under a **Help
      and documentation** heading, and the matching moddability bullet
      ("New help pages → drop a `.md` file in `assets/help/`"). Then
      `cp CLAUDE.md AGENTS.md` — they are gitignored twins with no tracking
      to catch drift.
- [ ] **Step 4** — `crates/engine/EASTER_EGGS.md`: repoint it at
      `assets/help/` and at the test's new home.
- [ ] **Step 5** — Gate: `cargo test --workspace`. Commit.

---

## Not in this change

Search, a main-menu entry, contextual help, and images. The format is chosen
so search is a later addition over `HelpPage` — pages are flat titled text —
rather than a rewrite.
