# Player Sprite Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every player can open the Sprite Forge from a Game Menu rail page, redraw any sprite, and have it saved to their data folder over the shipped art, with a revert.

**Architecture:** app-core loses the dev gate and gains a `Mode::SpriteOptions` hub page whose rail Enter opens the existing picker. A new pure, bevy-free gui module `sprite_layers.rs` owns the two folders: the merged scan, the whole-name shadowing, and every write, which goes into the user folder only. `gui::sprites`'s bevy systems become glue over it. The launcher derives `<data_dir>/sprites` and hands it to `App`.

**Tech Stack:** Rust, bevy (gui only), `png` codec already in `gui::sprites`.

**Spec:** `docs/superpowers/specs/2026-10-26-player-sprite-editor-design.md` (ADR 0030, Agreed).

## Global Constraints

- `assets/sprites/` is never written, renamed or deleted at runtime. Every write targets `<data_dir>/sprites/`.
- Whole-name rule: if the user folder holds any file for a name (`<name>.png`, `<name>.colour.png`, either `.png.off`, `<name>.anim.ron`), only the user folder decides that name.
- One behaviour for everyone: no env flag and no checkout check anywhere on this path.
- app-core does no file I/O. Only `gui` and `launcher` touch the disk.
- `crates/launcher/src/paths.rs` is the only place a runtime path is decided.
- Refusals and failures go through `App::refuse` (seams-screens: one door).
- Hub behaviour lives in the `GameMenu` `HubKind` impl, never in a `match hub` (seams-screens).
- Lists are highlight + Enter. No row letters.
- `cargo fmt`, then `cargo clippy --workspace --all-targets` clean, then `cargo test --workspace` green at the end of every task. Unset `FERAL_DEV_NO_SIEGES` first.
- Subagents: never push, never `git checkout`/`git stash` someone's uncommitted work, and stage explicit paths only.

## Review Focus

1. The user folder does not exist yet on first launch. The scan yields an empty user layer with no warning, and the first write creates the folder (Task 2, `missing_user_dir_is_an_empty_layer`, `save_creates_the_user_dir`).
2. The data folder is unwritable. The player sees "Couldn't save sprite `<name>`: …" on the status line and the map is unchanged (Task 2 `Failed` carries the message; Task 3 `failed_write_reaches_the_status_line`).
3. Save, revert, then save again leaves no stale `.anim.ron` or `.off` behind, so the pace and on/off state of the second save are exactly what was drawn (Task 2, `revert_then_save_leaves_no_leftovers`).
4. Turning a shipped sprite off, then on, leaves the name owned by the user, and revert restores the shipped art (Task 2, `disable_then_enable_a_shipped_sprite_then_revert`).
5. An undecodable PNG in the user folder still owns its name, by the whole-name rule. It draws the glyph with a warning and never the shipped art, and Revert clears it (Task 2, `a_corrupt_user_file_still_shadows_shipped`).

---

### Task 1: The Sprites rail page, for everyone

Remove the dev gate and add the rail entry. Writes still land wherever `drain_writes` sends them today; Task 4 redirects them.

**Files:**
- Modify: `crates/app-core/src/lib.rs` (`Mode::SpriteOptions` after `SpriteEditor`, with a doc line; update `SpritePicker`'s doc; delete fields `sprite_forge_flag`, `sprite_dir_installed`; add `user_sprite_dir: Option<PathBuf>`)
- Modify: `crates/app-core/src/app/lifecycle.rs` (field inits)
- Modify: `crates/app-core/src/app/sprite_forge.rs` (delete `dev_sprite_forge_flag`, `install_sprite_dir`, `sprite_forge_enabled`; rework `open_sprite_forge`; add `set_user_sprite_dir`/`user_sprite_dir`)
- Modify: `crates/app-core/src/app/game_menu.rs` (`SPRITES` page; `GameMenu::key`; `extra_keys`)
- Modify: `crates/app-core/src/app/menus.rs` (drop `'d'`), `crates/app-core/src/app/dev_console.rs` (drop the Sprite Forge row and its arm)
- Modify: `crates/app-core/src/app/input.rs` (route `Mode::SpriteOptions`, which has no keys of its own beyond the hub's)
- Modify: `crates/gui/src/render/meta.rs` (`main_menu_options` loses `sprite_forge`; `draw_game_menu_page` draws the Sprites page rows), the render dispatch / `render::ALL_MODES` census for the new mode, and `crates/gui/src/sprites.rs` `install_library` (ungated)
- Modify: `crates/launcher/src/paths.rs` (delete `DevPaths::sprites` and its test assertion; add `pub fn user_sprites_dir() -> PathBuf { data_dir().join("sprites") }`), `crates/launcher/src/main.rs` (ungated: `app.set_user_sprite_dir(paths::user_sprites_dir())`, then `install_sprite_library`)
- Test: `crates/app-core/src/tests/sprite_forge.rs`

**Interfaces:**
- Produces: `Mode::SpriteOptions`; `App::set_user_sprite_dir(&mut self, dir: PathBuf)`; `App::user_sprite_dir(&self) -> Option<&Path>` (`None` in tests and the arena: user layer absent); `pub fn user_sprites_dir() -> PathBuf` in `launcher::paths`.
- `open_sprite_forge` sets `menu_origin = Some(Mode::SpriteOptions)` and `menu_selected = 0`, so the picker's existing `close_screen` Esc lands back on the Sprites page. `hub_on_rail` is still `true` from the rail, which gives "Esc from the picker returns to the rail on the Sprites page".
- `GameMenu::key`: when `app.mode == Mode::SpriteOptions && app.hub_on_rail` and the key is `Enter` or `Right`, call `open_sprite_forge` and return `true`. `extra_keys` for that state: `("Enter", "open")`.
- Page order: Gameplay, Audio, Graphics, **Sprites**, then Save & quit (in a run only).
- Sprites page body: two text rows. Copy: "Redraw any sprite in the game." and "Your drawings are saved in your data folder and replace the built-in art. Revert brings the original back."

- [ ] **Step 1: Rewrite the gate tests as failing entry tests.** Delete the four `sprite_forge_enabled` tests (lines ~45–92) and the `sprite_forge_flag`/`install_sprite_dir` setup in the fixture and at ~654. Add:

```rust
#[test]
fn the_game_menu_rail_offers_sprites_before_save_and_quit() {
    let mut app = /* fixture with a run */;
    app.open_game_menu(true);
    let titles: Vec<_> = Hub::GameMenu.kind().pages(&app).iter().map(|p| p.title).collect();
    assert_eq!(titles, ["Gameplay", "Audio", "Graphics", "Sprites", "Save & quit"]);
}

#[test]
fn enter_on_the_sprites_rail_entry_opens_the_picker_and_esc_returns_to_the_rail() {
    let mut app = /* fixture, no flag set */;
    app.open_game_menu(false);
    for _ in 0..3 { app.handle_key(GameKey::Down); }
    assert_eq!(app.mode, Mode::SpriteOptions);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::SpritePicker);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::SpriteOptions);
    assert!(app.hub_on_rail);
}

#[test]
fn the_main_menu_has_no_sprite_forge_row() { /* 'd' on MainMenu leaves mode == MainMenu */ }
```

- [ ] **Step 2: Run them.** `cargo test -p feral-processes-app-core sprite_forge`. Expected: compile FAIL (`Mode::SpriteOptions` not defined).
- [ ] **Step 3: Implement the app-core side** per Files/Interfaces. Update the doc comments that cite the gate; delete them, don't leave `// removed` notes.
- [ ] **Step 4: Implement the gui and launcher side.** Run `rg -n "FERAL_DEV_SPRITES|sprite_forge_enabled|install_sprite_dir|DevPaths::sprites|\.sprites\b" crates CLAUDE.md` afterwards; expected: no hits.
- [ ] **Step 5: Gate.** `cargo fmt && cargo clippy --workspace --all-targets && cargo test --workspace`. Expected: clean, all pass.
- [ ] **Step 6: Commit** ("Sprites rail page: the Sprite Forge for every player").

---

### Task 2: `sprite_layers` — the two folders as one library (pure)

**Files:**
- Create: `crates/gui/src/sprite_layers.rs` (declare in `crates/gui/src/lib.rs`)
- Modify: `crates/app-core/src/app/sprite_forge.rs` (`SpriteOp::Revert`; delete `SpriteOp::full_colour()`)
- Modify: `crates/gui/src/sprites.rs`: move `scan_library`'s per-dir decode, `apply_sprite_write`, `sprite_path`, `sprite_stem`, `write_anim`, `anim_path` and their tests into `sprite_layers.rs`. `sprites.rs` keeps the codec (`frames_to_png`, `png_to_frames`), `read_frame_ms` and the bevy systems.

**Interfaces:**
- Consumes: `SpriteOp`, `InstalledSprite`, `png_to_frames`, `frames_to_png`, `read_frame_ms`.
- Produces:

```rust
pub enum SpriteOp { Save { frames, frame_ms, full_colour }, Enable { full_colour }, Disable { full_colour }, Revert }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer { Shipped, User }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadKey { pub layer: Layer, pub stem: String }   // stem: "x" or "x.colour"

pub struct SpriteLayers { pub shipped: PathBuf, pub user: Option<PathBuf> }
pub struct Library {
    pub enabled: HashMap<String, InstalledSprite>,
    pub disabled: HashMap<String, InstalledSprite>,
    pub edited: HashSet<String>,          // names the user layer owns
}
pub enum WriteOutcome {
    Load(LoadKey),                                  // Save / Enable landed
    Remove { name: String, then_load: Vec<LoadKey> }, // Disable / Revert landed
    Failed(String),                                 // player-facing sentence
}

impl SpriteLayers {
    pub fn dir(&self, layer: Layer) -> Option<&Path>;
    pub fn user_names(&self) -> HashSet<String>;          // base names, colour suffix stripped, '@' stems skipped
    pub fn load_list(&self) -> Vec<LoadKey>;              // every enabled stem the map should load, shadowing applied
    pub fn library(&self) -> Library;                     // decoded, shadowing applied; pace read from the owning layer
    pub fn apply(&self, name: &str, op: SpriteOp) -> WriteOutcome;
}
```

- `apply` semantics, all writes into `user` (`user == None` → `Failed`):
  - **Save:** create the user dir; write `<stem>.png`; `write_anim`; retire a user `<stem>.png.off`. Returns `Load(User, stem)`.
  - **Enable:** rename user `<stem>.png.off` → `<stem>.png`. The existing refuse-if-occupied rule is kept. Returns `Load(User, stem)`.
  - **Disable:** if a user `<stem>.png` exists, rename it to `.off` (existing rule). Otherwise copy shipped `<stem>.png` to user `<stem>.png.off` and copy shipped `<name>.anim.ron` if present. Returns `Remove { then_load }`, where `then_load` holds the user plain stem when a colour variant was disabled and a user plain `<name>.png` remains (today's `plain_remains`, now within the user layer only).
  - **Revert:** delete every user file for the name (`<name>.png`, `<name>.colour.png`, both `.off`, `<name>.anim.ron`). A missing file is fine. Returns `Remove { then_load: shipped load keys for that name }`.
  - Any I/O error → `Failed(format!("Couldn't save sprite `{name}`: {e}"))`, with a `warn!` as today.

- [ ] **Step 1: Write the failing tests** in `sprite_layers.rs`, using a `codec_test_dir`-style temp-dir helper with unique names (memory: shared temp paths between tests). Each test asserts files on disk and the `library()` it implies:
  - `missing_user_dir_is_an_empty_layer`: `user` points at a non-existent dir; `library()` equals the shipped-only scan; `edited` is empty.
  - `a_user_file_shadows_both_shipped_variants`: shipped `x.colour.png` + user `x.png` → `enabled["x"].full_colour == false`; `load_list()` has `LoadKey{User,"x"}` and no `Shipped` key for `x`.
  - `a_user_off_file_alone_hides_shipped_art`: shipped `x.png`, user `x.png.off` → `x` in `disabled`, not `enabled`; `load_list()` has nothing for `x`.
  - `pace_comes_from_the_owning_layer`: shipped `x.anim.ron` 300 ms, user two-frame `x.png` with no anim → `enabled["x"].frame_ms == DEFAULT_SPRITE_FRAME_MS`.
  - `save_creates_the_user_dir_and_never_touches_shipped`: hash the shipped dir's bytes before and after; equal.
  - `disable_then_enable_a_shipped_sprite_then_revert`: after Disable the user `x.png.off` matches the shipped bytes; after Enable `x` is in `edited` and `enabled`; after Revert `edited` is empty and `apply` returned `Remove{then_load: [Shipped "x"]}`.
  - `revert_then_save_leaves_no_leftovers`: save 2 frames, revert, save 1 frame → user dir holds exactly `x.png`.
  - `a_corrupt_user_file_still_shadows_shipped`: user `x.png` is garbage bytes → `x` in `edited`, not in `enabled`; no shipped key for `x` in `load_list()`.
  - `no_user_dir_fails_a_write`: `user: None` → `apply(Save)` is `Failed(_)`, and no file is written anywhere.
  - Ported: the moved `apply_sprite_write` invariant tests (never both `.png` and `.png.off` per variant), now pointed at the user layer.
- [ ] **Step 2: Run them.** `cargo test -p feral-processes-gui sprite_layers`. Expected: compile FAIL.
- [ ] **Step 3: Implement** `sprite_layers.rs` and `SpriteOp::Revert`. app-core's picker does not emit `Revert` yet. `drain_writes` calls `SpriteLayers { shipped: assets/sprites, user: app.user_sprite_dir() }.apply(..)` and maps the outcomes onto its existing table/queue code. Loading user-layer keys arrives in Task 4; until then a `User` key goes to the same `queue_load` path (a known gap, closed in Task 4).
- [ ] **Step 4: Gate**, as in Task 1.
- [ ] **Step 5: Commit** ("sprite_layers: user folder over shipped art, per name").

---

### Task 3: Picker: edited marker, revert, failure message

**Files:**
- Modify: `crates/app-core/src/app/sprite_forge.rs`, `crates/app-core/src/lib.rs` (fields `sprite_edited: HashSet<String>`, `pending_sprite_revert: Option<String>`)
- Modify: `crates/gui/src/render/sprite_forge.rs` (marker, y/n prompt, key line)
- Modify: `crates/gui/src/sprites.rs` (`install_library` and `drain_writes` pass `edited`; `Failed(msg)` → `app.report_sprite_write_failure(msg)`)
- Test: `crates/app-core/src/tests/sprite_forge.rs`

**Interfaces:**
- Consumes: `Library` (Task 2), `SpriteOp::Revert`.
- Produces: `App::install_sprite_library(enabled, disabled, edited: HashSet<String>)`; `SpriteSubject::edited: bool`; `pub fn report_sprite_write_failure(&mut self, message: String)`, which calls `self.refuse(message)`; `pub fn sprite_revert_pending(&self) -> Option<&str>` for the prompt.
- Keys on the picker: `r` on an edited row sets `pending_sprite_revert` and the screen asks "Revert `<label>` to the original art? (y/n)". `y` queues `SpriteWrite { name, op: Revert }` and clears it. `n` or `Esc` clears it without queueing. While it is pending, every other key is ignored, and Esc does not leave the picker. `r` on an unedited row → `refuse("`<label>` has no edits to revert.")`.
- Copy: the keybar/key line gains `r: revert`. The edited marker is a short suffix such as `(edited)`; check the width rule (`gui-text-never-clips-or-wraps`) against the longest label.

- [ ] **Step 1: Write the failing tests:**
  - `edited_subjects_are_marked`: install with `edited = {"anchor"}` → `sprite_subjects()` row `anchor` has `edited == true`, and every other row `false`.
  - `r_then_y_queues_a_revert`: select `anchor` (edited), press `r`, assert `sprite_revert_pending() == Some("anchor")` and no writes yet; press `y` → `take_sprite_writes() == [SpriteWrite{name:"anchor", op: SpriteOp::Revert}]`.
  - `r_then_n_or_esc_queues_nothing_and_stays_on_the_picker`: covers both keys; `mode == SpritePicker`.
  - `r_on_an_unedited_row_is_refused`: `status_line` contains "no edits to revert"; no writes.
  - `failed_write_reaches_the_status_line`: `report_sprite_write_failure("Couldn't save sprite `x`: denied".into())` → `status_line` equals it.
- [ ] **Step 2: Run them.** Expected: compile FAIL.
- [ ] **Step 3: Implement.** Update existing `install_sprite_library` call sites in tests to pass `HashSet::new()`.
- [ ] **Step 4: Gate.**
- [ ] **Step 5: Commit** ("Sprite picker: mark edited art, revert it, report failed saves").

---

### Task 4: Load the user layer into bevy

**Files:**
- Modify: `crates/gui/src/sprites.rs` (`load`, `register`, `queue_load`, `drain_writes`, `install_library` all build a `SpriteLayers` from `app.assets_dir()` and `app.user_sprite_dir()`; `Sprites::pending` holds `(LoadKey, Handle<Image>)`; `register` reads the pace from `layers.dir(key.layer)`)
- Modify: `crates/gui/src/lib.rs` (wherever `AssetPlugin` is built: register the user layer's source before it)
- Modify: `assets/sprites/README.md` (replace the "dev-only sprite editor" paragraph with the player folder rule and the whole-name rule)

**Interfaces:**
- Consumes: `SpriteLayers::load_list`, `LoadKey`, `WriteOutcome`.
- The route is the builder's choice (spec). The recommended one is a named asset source, `"user-sprites"`, registered on the bevy `App` before `DefaultPlugins` when `user_sprite_dir()` is `Some`, so a `User` key loads `AssetPath::from(format!("{stem}.png")).with_source("user-sprites")` through the same `load_builder` and nearest sampler. A `Shipped` key keeps loading `sprites/{stem}.png`. Whatever reload mechanism `queue_load` relies on today must hold for the user source too: a second save of the same name must redraw.

- [ ] **Step 1: Write the failing test.** The bevy half has no unit seam. Pin the one pure piece: `pending_keys_route_by_layer` asserts the asset path string built for `LoadKey{User,"x"}` is in the user source and for `Shipped` is `sprites/x.png` (extract `fn asset_path(key: &LoadKey) -> AssetPath<'static>`).
- [ ] **Step 2: Run it.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Gate.**
- [ ] **Step 5: See it on screen.** With a throwaway `XDG_DATA_HOME` (no other environment changes), run `cargo run -- --template stack --keys "<Esc, Down×3, Enter, …select a species, Enter, draw, s, Esc, Esc, Esc>" --screenshot out.png`, Read the PNG, and confirm the redrawn sprite is on the map. Then confirm `$XDG_DATA_HOME/feral-processes/sprites/<name>.png` exists and `git status assets/` is clean.
- [ ] **Step 6: Commit** ("Sprites load from the player's folder over the shipped art").

---

## Final gate (controller)

- Whole-branch review on opus, given the spec, the ADR and the diff as a file.
- `cargo test -p feral-processes-engine balance_sim` is not affected (no tuning, species or item changes); run it only if the diff touches `engine`.
- The ADR flips to Accepted, and CHANGELOG.md gets a line, at deploy, not on the branch.
