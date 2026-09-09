# Tactical surface battles — Phase 1: the options screen

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the game's first settings screen, carrying exactly one
profile-backed toggle — tactical surface battles, default off — so the later
phases have a switch to read.

**Architecture:** The toggle is an additive `#[serde(default)] bool` on
`achievements::Profile`, which is already installed into the engine world as
a resource at `Game::new` and `Game::load`, so the engine needs no new
plumbing. app-core owns a new `Mode::Options` reachable with `[O]` from the
main menu; it lists rows from `App::option_rows()` and writes `profile.ron`
the moment a row is toggled. gui draws it as a popup and joins the two
censuses (`ALL_MODES`, the refusal count).

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine), RON via serde (profile),
`bevy` + `bevy_egui` (gui, drawn through `Painter`).

**Spec:** `docs/superpowers/archive/specs/2026-09-09-tactical-surface-battles-design.md`
(§8 is this phase; §12 item 1).

## Global Constraints

- **No `SAVE_FORMAT_VERSION` movement.** The toggle lives on `Profile`, not
  `SaveData`. `Profile` is deliberately outside the save; every field on it
  is `#[serde(default)]` for exactly this reason.
- **`Profile::load` discards the entire profile — achievements included —
  when it cannot parse.** The new field must be a plain `bool`, never a type
  whose retirement or unknown value becomes a parse error.
- **This phase changes no load-bearing seam.** Nothing goes into
  `docs/seams.md`, the `seams` skill or `CLAUDE.md`'s seam list here; those
  three writes belong to Phases 2–5, which mint the tactical rules. Do not
  invent a seam entry for a settings screen.
- **`ALL_MODES` (`crates/gui/src/render/mod.rs:1402`) is hand-written and the
  draw dispatch ends in `_ => {}`.** A new `Mode` compiles clean and ships as
  a blank screen. Task 3 exists to close that, and its first step is
  deliberately the red one.
- **Lowercase letters are row selectors; every new screen action is
  uppercase.** The main menu's `[O]` is matched case-insensitively there, the
  way `n`/`l`/`a`/`r`/`d`/`q` already are.
- **No version bump on the branch.** The workspace version, the
  `CHANGELOG.md` section and the tag happen once, at the merge.
- **Gates:** `cargo fmt`, `cargo clippy --workspace`, and
  `cargo test --workspace` before the phase is called done. Per-task runs may
  be narrowed with `-p`.

---

### Task 1: The profile field

**Files:**
- Modify: `crates/engine/src/achievements.rs` (the `Profile` struct, ~line
  236, beside `player_icon`; its `#[cfg(test)] mod tests`, ~line 520)

**Interfaces:**
- Consumes: nothing.
- Produces: `achievements::Profile::tactical_battles: bool`, default
  `false`. Read in Task 2 as `app.profile.tactical_battles`, and in a later
  phase off `Game::profile()`.

**No accessor is added here.** `Game::profile()` is already public and
`Profile::default()` is inserted into the world by both constructors
(`game/lifecycle.rs:554` and `:1178`), so a `Game::tactical_battles_enabled()`
with no caller would be dead code. Phase 4 adds it at the site that reads it.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/achievements.rs`, inside the existing
`#[cfg(test)] mod tests`:

```rust
/// The whole reason this is a plain `bool` on `Profile` rather than
/// anything richer: `load` throws away the entire profile — every
/// achievement earned — when it cannot parse, so a `profile.ron` written
/// by a build that predates this field must keep loading untouched.
#[test]
fn a_profile_without_the_options_field_still_loads() {
    let dir = std::env::temp_dir().join("feral_profile_options_absent");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("profile.ron");
    std::fs::write(
        &path,
        "(earned:[(id:\"first_kill\",first_tick:12)],seen_notifications:[],player_icon:None)",
    )
    .unwrap();

    let (profile, warning) = Profile::load(&path);

    assert_eq!(warning, None, "an older profile must not warn: {warning:?}");
    assert_eq!(profile.earned.len(), 1, "the achievement was discarded");
    assert!(
        !profile.tactical_battles,
        "an absent toggle must read as off"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_tactical_toggle_survives_a_profile_round_trip() {
    let dir = std::env::temp_dir().join("feral_profile_options_round_trip");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("profile.ron");
    let mut profile = Profile::default();
    profile.tactical_battles = true;
    profile.save(&path).unwrap();

    let (read_back, warning) = Profile::load(&path);

    assert_eq!(warning, None, "a profile this build wrote must reload: {warning:?}");
    assert!(read_back.tactical_battles, "the toggle did not survive the write");
    let _ = std::fs::remove_file(&path);
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p feral-processes-engine achievements::tests::a_profile_without_the_options_field -- --exact --nocapture`

Expected: FAIL — `no field 'tactical_battles' on type Profile`.

If the first test fails on the RON literal rather than the missing field,
fix the literal against the current `Profile` shape before going on: it is
hand-written on purpose, because a struct-built fixture would serialise the
new field and prove nothing.

- [ ] **Step 3: Add the field**

In `Profile`, directly after `player_icon`:

```rust
    /// Whether surface fights open on a tactical grid rather than the
    /// abstract group model — see
    /// `docs/superpowers/archive/specs/2026-09-09-tactical-surface-battles-design.md`.
    ///
    /// Cross-run rather than per-save for the reason every field here is:
    /// this is a statement about how the player wants to play, not about
    /// one run. A plain `bool` for `seen_notifications`' reason — `load`
    /// discards the *whole* profile on a parse failure, so nothing here may
    /// become unparseable by a later build.
    #[serde(default)]
    pub tactical_battles: bool,
```

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p feral-processes-engine achievements::tests`
Expected: PASS, whole module green.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/achievements.rs
git commit -m "feat: a profile remembers whether tactical battles are on"
```

---

### Task 2: `Mode::Options` and its rows

**Files:**
- Modify: `crates/app-core/src/lib.rs` (the `Mode` enum, ~line 1151; a new
  `OptionRow`/`OptionKey` beside the other row types)
- Modify: `crates/app-core/src/app/menus.rs` (`handle_main_menu_key`, line 8;
  a new `handle_options_key`)
- Modify: `crates/app-core/src/app/input.rs` (the mode dispatch, ~line 200)
- Create: `crates/app-core/src/tests/options.rs`
- Modify: `crates/app-core/src/tests/mod.rs` (add `mod options;`, alphabetical
  — between `mod notifications;` and `mod party;`)

**Interfaces:**
- Consumes: `Profile::tactical_battles` (Task 1).
- Produces:
  - `Mode::Options`
  - `pub struct OptionRow { pub key: OptionKey, pub label: String, pub value: String }`
  - `pub enum OptionKey { TacticalBattles }`
  - `App::option_rows(&self) -> Vec<OptionRow>`
  - `App::toggle_option(&mut self, key: OptionKey)`
  - `App::handle_options_key(&mut self, key: GameKey)` — `pub(crate)`
  - Task 3 reads `option_rows()` and nothing else to draw the list.

`OptionRow` follows `AchievementRow`'s established shape: **one selectable row
per option, built in app-core and never restated in the renderer**, so the
scroll bound and the drawn list cannot drift. `OptionKey` is an enum rather
than a row index so `toggle_option` is an exhaustive match — a second option
added without a toggle arm fails to compile.

**Reachable from the main menu only, deliberately.** The engine reads the
toggle off the `Profile` resource installed once per run at
`install_profile`, so an in-run change would not take effect without
re-installing it. Extending later is one line at that call site; adding the
key now would be a switch that appears to do nothing mid-run.

- [ ] **Step 1: Write the failing tests**

Create `crates/app-core/src/tests/options.rs`:

```rust
use super::support::test_app;
use crate::{GameKey, Mode, OptionKey};
use feral_processes_engine::achievements::Profile;

/// The screen exists and the main menu can reach it. `[O]` is uppercase in
/// the label and matched case-insensitively, exactly as the five rows
/// already on that menu are.
#[test]
fn the_main_menu_opens_the_options_screen() {
    let mut app = test_app(9001);
    app.mode = Mode::MainMenu;

    app.handle_key(GameKey::Char('o'));

    assert_eq!(app.mode, Mode::Options, "[O] did not open the options screen");
}

#[test]
fn escape_returns_to_the_main_menu() {
    let mut app = test_app(9002);
    app.mode = Mode::Options;

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::MainMenu);
}

/// One row, and its value reads the profile rather than any second copy of
/// the state.
#[test]
fn the_toggle_row_reports_the_profile() {
    let mut app = test_app(9003);

    let rows = app.option_rows();
    assert_eq!(rows.len(), 1, "one option only, per the spec's YAGNI clause");
    assert_eq!(rows[0].key, OptionKey::TacticalBattles);
    assert_eq!(rows[0].value, "Off");

    app.profile.tactical_battles = true;
    assert_eq!(app.option_rows()[0].value, "On");
}

/// Enter on the highlighted row flips it **and writes `profile.ron`
/// immediately** — the same rule the icon write follows, and for the same
/// reason: nothing else on this screen is going to save it later.
#[test]
fn toggling_writes_the_profile_to_disk() {
    let mut app = test_app(9004);
    app.mode = Mode::Options;
    app.menu_selected = 0;

    app.handle_key(GameKey::Enter);

    assert!(app.profile.tactical_battles, "the toggle did not flip");
    let (from_disk, warning) = Profile::load(&app.profile_path);
    assert_eq!(warning, None, "the profile did not reload: {warning:?}");
    assert!(
        from_disk.tactical_battles,
        "the toggle flipped in memory but was never written"
    );

    app.handle_key(GameKey::Enter);
    assert!(!app.profile.tactical_battles, "the toggle does not flip back");
    let (from_disk, _) = Profile::load(&app.profile_path);
    assert!(!from_disk.tactical_battles, "the second flip was never written");
}

/// The toggle rides `install_profile`, which is the one hand-off from this
/// screen to a running game. Asserted here rather than left to Phase 4: a
/// field that does not survive that copy makes every later phase read
/// `false` forever, and nothing else in the suite would say so.
#[test]
fn install_profile_carries_the_toggle() {
    let mut app = test_app(9005);
    app.mode = Mode::Options;
    app.handle_key(GameKey::Enter);
    assert!(app.profile.tactical_battles);

    let mut game = app.game.take().expect("the fixture has a game");
    game.install_profile(app.profile.clone());

    assert!(
        game.profile().tactical_battles,
        "install_profile dropped the toggle"
    );
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p feral-processes-app-core options::`
Expected: FAIL to compile — `no variant Options`, `no method option_rows`.

- [ ] **Step 3: Add the mode, the row types and the handler**

In `crates/app-core/src/lib.rs`, add to `Mode` after `Achievements` (keep the
doc-comment density of its neighbours):

```rust
    /// The settings screen, opened with `[O]` from the main menu. Rows come
    /// from `App::option_rows`; Enter toggles the highlighted one and writes
    /// `profile.ron` on the spot.
    ///
    /// **Main menu only, deliberately.** An option here is read off the
    /// `Profile` resource the engine is handed once per run by
    /// `install_profile`, so a mid-run change would not reach the running
    /// game — a switch that appears to do nothing is worse than one the
    /// player has to leave the run to reach.
    Options,
```

Beside the other row structs in the same file:

```rust
/// One line of the options screen. Built in app-core and never restated in
/// the renderer, `AchievementRow`'s reason: the same list bounds the scroll
/// and draws the rows, so the highlight cannot land on a row nothing paints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionRow {
    pub key: OptionKey,
    pub label: String,
    /// Already rendered — `"On"` / `"Off"` — so the renderer holds no
    /// opinion about how a setting reads.
    pub value: String,
}

/// Which setting a row is. An enum rather than the row's index so
/// `App::toggle_option` is an exhaustive match and a second option added
/// without a toggle arm fails to compile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionKey {
    TacticalBattles,
}
```

In `crates/app-core/src/app/menus.rs`, extend the main menu's key list —
`'o'` goes after `'a'` and before the two dev switches, matching the row
order Task 3 draws:

```rust
        options.push('a');
        options.push('o');
```

and its arm:

```rust
            Some('o') => {
                self.status_line = None;
                self.mode = Mode::Options;
            }
```

Then, beside `handle_achievements_key`:

```rust
    /// Esc goes back to the main menu rather than through `close_screen`,
    /// `handle_achievements_key`'s reason: this screen is reachable without
    /// a run, so there is no map behind it to return to.
    pub(crate) fn handle_options_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::MainMenu;
            return;
        }
        let rows = self.option_rows();
        if key == GameKey::Enter {
            if let Some(row) = rows.get(self.menu_selected) {
                self.toggle_option(row.key);
            }
            return;
        }
        self.scroll(key, rows.len());
    }

    pub fn option_rows(&self) -> Vec<OptionRow> {
        vec![OptionRow {
            key: OptionKey::TacticalBattles,
            label: "Tactical surface battles".to_string(),
            value: on_off(self.profile.tactical_battles),
        }]
    }

    /// Flips one setting and writes `profile.ron` at once — the icon write's
    /// rule (`app/lifecycle.rs`), for the same reason: nothing downstream of
    /// this screen is going to save it, and a setting that silently forgets
    /// itself on quit is indistinguishable from one that does not work.
    /// A failed write costs the setting and nothing else.
    pub fn toggle_option(&mut self, key: OptionKey) {
        match key {
            OptionKey::TacticalBattles => {
                self.profile.tactical_battles = !self.profile.tactical_battles;
            }
        }
        if let Err(e) = self.profile.save(&self.profile_path) {
            self.status_line = Some(format!("Could not write profile: {e}"));
        }
    }
```

with, as a free function in the same file:

```rust
fn on_off(value: bool) -> String {
    if value { "On" } else { "Off" }.to_string()
}
```

In `crates/app-core/src/app/input.rs`, beside the `Achievements` arm:

```rust
            Mode::Options => self.handle_options_key(key),
```

Export the two new types wherever `Mode` and the other row types are
re-exported from `crates/app-core/src/lib.rs`.

- [ ] **Step 4: Register the test module and run**

Add `mod options;` to `crates/app-core/src/tests/mod.rs` in alphabetical
position.

Run: `cargo test -p feral-processes-app-core options::`
Expected: PASS, five tests.

- [ ] **Step 5: Run the app-core suite and the two format gates**

Run: `cargo fmt && cargo clippy -p feral-processes-app-core && cargo test -p feral-processes-app-core`
Expected: PASS, no warnings. `tests/menus.rs` and `tests/quitting.rs` walk
the main menu's rows — if either asserts a row count or a key list, update it
to include `[O]` rather than working around it.

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/lib.rs crates/app-core/src/app/menus.rs \
        crates/app-core/src/app/input.rs crates/app-core/src/tests/mod.rs \
        crates/app-core/src/tests/options.rs
git commit -m "feat: an options screen with the tactical-battles toggle"
```

---

### Task 3: Drawing it, and the two censuses

**Files:**
- Modify: `crates/gui/src/render/meta.rs` (`main_menu_options`, line 31; a new
  `draw_options`)
- Modify: `crates/gui/src/render/mod.rs` (the draw dispatch, ~line 588
  beside the `Achievements` arm; `ALL_MODES`, line 1402; its `#[cfg(test)]
  mod tests`, which is where both new tests go)

**Interfaces:**
- Consumes: `App::option_rows() -> Vec<OptionRow>`, `App::menu_selected`
  (Task 2).
- Produces: nothing later phases consume. Phase 7's tactical screens are
  separate modes.

**Take the steps in this order.** Adding the `ALL_MODES` entry *before* the
draw arm is what proves the census actually catches a blank screen — this
repo's known trap, where a new `Mode` compiles clean and ships undrawn.

- [ ] **Step 1: Add the mode to the census and watch it fail**

In `crates/gui/src/render/mod.rs`, change `const ALL_MODES: [Mode; 103]` to
`[Mode; 104]` and add `Mode::Options,` directly after the existing
`Mode::Achievements` entry (around line 1492 — the array is not in enum
order, so find that line rather than counting).

Run: `cargo test -p feral-processes-gui every_screen_draws_a_refusal_exactly_once`
Expected: FAIL — `Options painted the refusal 0 times, not once`. That is the
blank screen, caught.

- [ ] **Step 2: Write the row test too, and watch it fail**

In `crates/gui/src/render/mod.rs`'s existing `#[cfg(test)] mod tests` —
**not** in `meta.rs`. `census_app()`, `popup_body_width`, `with_painter` and
`painted_text` are all already in scope there, and `meta.rs`'s items reach it
through the module's `use super::*`, so this needs no second fixture:

```rust
    /// The row draws its setting's current value, and the main menu offers
    /// the row that opens it. Both halves, because a menu row the handler
    /// does not offer opens whatever screen is underneath it — the note on
    /// `main_menu_options` says so.
    #[test]
    fn the_options_screen_draws_its_toggle() {
        let mut app = census_app();
        app.mode = Mode::Options;
        let m = ui_metrics(900.0);
        let (_, shapes) = with_painter(|p| draw_options(&app, None, p, &m));
        let drawn = painted_text(&shapes);
        assert!(
            drawn.iter().any(|t| t.contains("Tactical surface battles") && t.contains("Off")),
            "the toggle row did not draw its value: {drawn:#?}"
        );

        let menu = main_menu_options(false, false, false);
        assert!(
            menu.iter().any(|row| row.contains("[O]")),
            "the main menu does not offer the options row: {menu:#?}"
        );
    }

    /// The width census. `wrapped_row_lines` wraps only tag segments, so a
    /// prose line too long for the body draws off the popup's edge in
    /// silence — the failure this screen's hand-wrapped description exists
    /// to avoid.
    #[test]
    fn no_options_row_overflows_the_popup_body_at_1280x720() {
        let mut app = census_app();
        app.mode = Mode::Options;
        let m = ui_metrics(720.0);
        let body = popup_body_width(1280.0, PopupSize::Large, &m);
        let (_, shapes) = with_painter(|p| draw_options(&app, None, p, &m));
        with_painter(|p| {
            for line in painted_text(&shapes) {
                let width = p.measure_ui_advance(format!("  {line}"), m.font_size);
                assert!(
                    width <= body,
                    "an options row draws {width}px into a {body}px body: {line:?}"
                );
            }
        });
    }
```

Run: `cargo test -p feral-processes-gui options`
Expected: FAIL to compile — `draw_options` does not exist.

- [ ] **Step 3: Draw it**

In `crates/gui/src/render/meta.rs`, add the row to the menu list, between
Achievements and the two dev switches so it matches the key order
`handle_main_menu_key` builds:

```rust
    options.push("[A] Achievements".to_string());
    options.push("[O] Options".to_string());
```

and the screen itself:

```rust
/// The settings screen. Rows come from `App::option_rows`, which is also
/// what app-core scrolls, so the highlight cannot land on a row this never
/// draws — `draw_achievements`' rule.
///
/// The description is **hand-wrapped into short lines**, not one long
/// sentence: `wrapped_row_lines` breaks tag segments and nothing else, so a
/// prose row wider than the body is simply drawn off the edge.
pub(super) fn draw_options(app: &App, refusal: Option<&str>, painter: &Painter, m: &Metrics) {
    let mut rows = vec![text_row("")];
    for (i, row) in app.option_rows().iter().enumerate() {
        rows.push(item_row(
            format!("{}: {}", row.label, row.value),
            i == app.menu_selected,
        ));
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "Surface packs and town patrols fight on a generated grid,",
    ));
    rows.push(text_row(
        "one body at a time. Nests, lairs, Entropy Sweeps and the",
    ));
    rows.push(text_row("Stack keep the abstract model."));
    rows.push(text_row(""));
    rows.push(text_row("Enter toggles, Esc to close."));
    draw_popup("Options", PopupSize::Large, &rows, refusal, painter, m);
}
```

In `crates/gui/src/render/mod.rs`'s `draw`, beside the `Achievements` arm:

```rust
        Mode::Options => draw_options(app, refusal, painter, &m),
```

`needs_status_banner` is **not** touched: this screen draws a popup, and
`draw_popup` is already handed the refusal.

- [ ] **Step 4: Run both tests and the gui suite**

Run: `cargo test -p feral-processes-gui options && cargo test -p feral-processes-gui every_screen_draws_a_refusal_exactly_once`
Expected: PASS both.

Run: `cargo fmt && cargo clippy -p feral-processes-gui && cargo test -p feral-processes-gui`
Expected: PASS, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/gui/src/render/meta.rs crates/gui/src/render/mod.rs
git commit -m "feat: draw the options screen and join both censuses"
```

---

### Task 4: The full-suite gate

**Files:**
- None. This task runs gates and hands a line to the merge.

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces: nothing.

`docs/manual.md` and the root `README.md` are both carved out of the doc
obligation. No `assets/*/README.md` changes: nothing here is asset schema.

- [ ] **Step 1: Run the full suite**

Run: `cargo test --workspace`
Expected: PASS. The count moves up from 5061 by the tests this phase added;
report the real number rather than predicting it.

If anything unrelated fails, check it against the known flakes before
touching it: app-core's `an_unspent_allowance_holds_its_step` fails on an
unmodified binary, and a bevy-query-order flake can show in `--workspace` and
not in `-p`.

- [ ] **Step 2: Note the changelog line for the merge**

**Do not add a `CHANGELOG.md` section on the branch.** A release is cut per
change that lands on `main`: the version bump, the `## X.Y.Z` section and the
tag all happen once, at the merge, so a section written here is a conflict
waiting for a rebase. The `deploy` skill writes it. The entry it should carry
— one bold sentence, the convention from `0.13.58` on:

```markdown
- **An Options screen, reachable with `[O]` from the main menu, carrying one
  cross-run setting: tactical surface battles, default off.** The setting is
  stored in `profile.ron` and read by nothing yet.
```

- [ ] **Step 3: Report**

Say what actually ran and what it said — the suite's real test count, and any
failure and whether it was one of the known flakes. A green suite is not
evidence this screen has been seen; nothing in this phase has been on a
display, and the toggle is unplayable until Phase 7 draws a tactical fight.

---

## What Phase 2 inherits

- `Profile::tactical_battles`, written and persisted, read nowhere.
- `Mode::Options` with one row; a second setting is `option_rows()` plus a
  `toggle_option` arm and nothing else.
- The open question Phase 2 must answer first, from spec §7: what replaces
  `BattleTimeline` / `RosterFrame` / `SwingOutcome` for a fight that resolves
  one body at a time. It is explicitly deferred to the implementation plan
  and no code here constrains it.
