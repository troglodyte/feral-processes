# Windows and macOS Distribution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `feral-processes.exe` plus an `assets/` directory can be zipped up,
unzipped anywhere on a Windows machine, and played — with saves in
`%APPDATA%`, no console window, and mods still droppable into `assets/`.

**Architecture:** One new module, `crates/launcher/src/paths.rs`, becomes the
single answer to "where does the game find X". It picks an *installed* layout
when an `assets/` directory sits beside `current_exe()` and a *repo* layout
otherwise, and puts player data in the OS data directory in both. `main.rs`
reads nothing else. Every other crate already takes its paths as parameters,
so this is a launcher-only change to production code.

**Tech Stack:** Rust, `dirs` 6, `cargo test`. No engine, app-core or gui
production code is touched.

**Spec:** `docs/superpowers/specs/2026-08-19-windows-and-macos-distribution-design.md`

Read the spec first. It carries the audit that established the code is already
portable, and the reasoning for each decision — in particular why
installed-ness is *sniffed* rather than flagged, and why player data goes to
the OS directory in a repo build too. The "macOS, as an afterthought" section
is scoped into Task 2 only as the `Contents/Resources` probe; **do not build a
`.app` bundle.**

## Global Constraints

- **Launcher-only.** No file under `crates/engine`, `crates/app-core` or
  `crates/gui` may be modified. If a change seems to require one, stop — the
  design is being violated. In particular `App::new`'s six-parameter signature
  and `DevTemplates`' `resolve: fn(&str) -> Result<PathBuf, String>`
  function-pointer type must not change.
- **`dirs` goes in `crates/launcher/Cargo.toml` and nowhere else.**
- **`dev_template::repo_root()` keeps its signature and all four of its
  callers**, but its body moves into `paths.rs` (see File Structure) so that
  "one module decides every path" is true rather than nearly true. `savetool`,
  `arena`, `train` and `tuner` keep resolving out of the repo. This plan does
  not make any of them work on Windows.
- **No save-format change.** `SAVE_FORMAT_VERSION` must not move. Files move
  location; their bytes are untouched.
- **The one env override is `FERAL_ASSETS_DIR`.** Do not add a second
  per-path override. Treat an empty value as unset, matching
  `dev_console::dev_flag`'s existing rule for the `FERAL_DEV_*` flags.
- **Full suite is the gate:** `cargo test --workspace` (2388 tests), plus
  `cargo clippy --workspace` and `cargo fmt`, before the plan is done.

## Refinements to the spec, decided while planning

Two places where the spec's sketch does not survive contact with the code.
Both are deliberate; implement these, not the spec's literal signatures.

**`paths::resolve()` is infallible.** The spec sketched
`resolve() -> Result<Paths, PathError>`. But `dev_template::working_copy` must
also learn the saves directory, and `working_copy` is called from
`dev_template::resolve`, which is handed to app-core as a bare `fn` pointer in
`DevTemplates`. Making the path lookup fallible would ripple through that
signature and force an app-core change, which the constraints forbid. So
`data_dir()` falls back to `repo_root()` when `dirs::data_dir()` returns `None`
(no `HOME`), and `resolve()` falls back to the repo layout when
`current_exe()` fails. Whether the resolved `assets` directory actually
*exists* is then a separate check in `main`, which is where the error belongs
anyway.

**The migration moves `profile.ron` too, not just saves.** The spec's
decisions section names only `saves/`. `profile.ron` holds the achievement
ladder's earned rewards, and `docs/seams.md` records it as the one of the
three run-spanning files "that costs real money if it regresses" — leaving it
behind would silently reset a player's profile. `run_history.log` moves with
it for consistency.

## File Structure

- **Create `crates/launcher/src/paths.rs`** — the whole feature. Layout
  selection, the OS data directory, the migration, and their tests. It also
  takes ownership of `repo_root()`, moved out of `dev_template`; the
  dependency runs `dev_template -> paths` and never the other way, or the
  fallback in `data_dir()` would be a cycle.
- **Modify `crates/launcher/src/lib.rs`** — one `pub mod paths;` line.
- **Modify `crates/launcher/src/main.rs:37-70`** — the path block becomes a
  `paths::resolve()` call; adds the `windows_subsystem` attribute and the
  startup-error file.
- **Modify `crates/launcher/src/dev_template.rs:36-50, 62-66`** — `repo_root`
  becomes a one-line delegation to `paths::repo_root()` (its four callers and
  its doc comment stay), and `working_copy` asks `paths` for the saves
  directory instead of `repo_root().join("saves")`.
- **Modify `crates/launcher/Cargo.toml`** — `dirs = "6"`.
- **Modify `CLAUDE.md`, `AGENTS.md`, `docs/seams.md`, `CHANGELOG.md`** — the
  new seam and the release sequence.
- **Create `packaging/windows-readme.txt`** — the text file that ships inside
  the zip.

---

### Task 1: the data directory

**Files:**
- Create: `crates/launcher/src/paths.rs`
- Modify: `crates/launcher/src/lib.rs`
- Modify: `crates/launcher/Cargo.toml`
- Test: inside `crates/launcher/src/paths.rs`, a `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `pub fn repo_root() -> PathBuf` (moved verbatim from
  `dev_template`, doc comment and all), `pub fn data_dir() -> PathBuf`, and
  `pub fn saves_dir() -> PathBuf` (`data_dir().join("saves")`). All
  infallible.
- `dev_template::repo_root()` becomes `paths::repo_root()` in one line, so its
  four callers do not move. `dev_template`'s own tests that assert on
  `repo_root()` stay where they are.

- [ ] **Step 1: Write the failing tests**

Two tests, both against a helper that takes the OS directory as a parameter so
they never read the real environment:

- `the_data_dir_is_the_os_dir_under_the_game_name` — given `Some(/x/y)`, the
  result is `/x/y/feral-processes`.
- `no_os_data_dir_falls_back_to_the_repo` — given `None`, the result is
  `repo_root()`, unchanged from today's behaviour.

So the module needs a private `fn data_dir_from(os: Option<PathBuf>) -> PathBuf`
that `data_dir()` calls with `dirs::data_dir()`. This split is the only reason
these are testable; do not inline it.

- [ ] **Step 2: Run the tests and watch them fail**

`cargo test -p feral-processes paths::` — expect a compile failure, module not
found.

- [ ] **Step 3: Add the dependency and write the module**

`dirs = "6"` in `crates/launcher/Cargo.toml`, `pub mod paths;` in `lib.rs`.
The game-name segment is `"feral-processes"` — a `const`, not a literal
repeated at two sites. Move `repo_root` across in the same step and leave
`dev_template::repo_root` delegating; `cargo test -p feral-processes` proves
the four callers still resolve to the same directory.

- [ ] **Step 4: Run the tests and watch them pass**

`cargo test -p feral-processes paths::`

- [ ] **Step 5: Commit**

`feat(paths): the OS data directory`

---

### Task 2: layout selection

**Files:**
- Modify: `crates/launcher/src/paths.rs`
- Test: same file

**Interfaces:**
- Consumes: `data_dir()` from Task 1.
- Produces:

```rust
pub struct Paths {
    pub assets: PathBuf,
    pub data: PathBuf,
    /// `None` in an installed build — no repo to find dev material in.
    pub dev: Option<DevPaths>,
}

pub struct DevPaths {
    pub arenas: PathBuf,
    pub battle_log: PathBuf,
}

pub fn resolve() -> Paths;
```

- [ ] **Step 1: Write the failing tests**

All of these drive a private
`fn layout(exe_dir: Option<&Path>, assets_override: Option<PathBuf>) -> Paths`,
built over scratch directories — follow whatever `dev_template.rs`'s existing
tests already do for scratch space rather than adding a crate.

- `assets_beside_the_exe_is_an_installed_build` — an `assets/` directory
  exists beside the given exe dir: `assets` points at it and `dev` is `None`.
- `no_assets_beside_the_exe_is_a_repo_build` — nothing beside the exe:
  `assets` is `repo_root()/assets` and `dev` is `Some`, with `arenas` at
  `repo_root()/dev-arenas` and `battle_log` at
  `repo_root()/dev-logs/battles.jsonl`.
- `a_mac_bundle_finds_its_assets_in_resources` — `assets/` sits at
  `<exe_dir>/../Resources/assets`, nothing beside the exe: that is an
  installed build and `assets` points into `Resources`. This is the entire
  macOS `.app` provision; it costs three lines and is checked *after* the
  beside-the-exe probe.
- `an_unknown_exe_location_falls_back_to_the_repo` — `exe_dir` is `None`
  (i.e. `current_exe()` failed): repo layout.
- `the_assets_override_wins_in_both_layouts` — with an override set, `assets`
  is the override whether or not there is an `assets/` beside the exe. Assert
  both cases in one test; the installed half alone passes against an
  implementation that ignores the layout entirely.
- `an_empty_assets_override_reads_as_unset` — `Some("")` behaves as `None`.
- `data_is_the_os_directory_in_both_layouts` — one test, both layouts.

`resolve()` itself is then three lines over `current_exe()` and
`std::env::var_os("FERAL_ASSETS_DIR")` and is not directly tested.

- [ ] **Step 2: Run the tests and watch them fail**

`cargo test -p feral-processes paths::`

- [ ] **Step 3: Implement**

Note the probe order, which is the only subtle part: beside-the-exe, then
`../Resources/assets`, then repo. `dev` is `Some` **iff** the repo layout was
chosen — the two are one decision, not two.

- [ ] **Step 4: Run the tests and watch them pass**

- [ ] **Step 5: Commit**

`feat(paths): installed and repo layouts`

---

### Task 3: the migration

**Files:**
- Modify: `crates/launcher/src/paths.rs`
- Test: same file

**Interfaces:**
- Produces: `pub fn migrate_from_repo(repo_root: &Path, data: &Path)`.
  Returns nothing — a failed move must never stop the game starting.

What it does, in this order:

1. If `data/saves` already contains any `*.bin`, do nothing at all and return.
   This is what makes the whole thing one-shot; there is no marker file.
2. Move every `*.bin` in `repo_root/saves` into `data/saves`.
3. Move `repo_root/save.bin` (the pre-`saves/` legacy file that `main.rs:46`
   handles today) into `data/saves/save.bin`, keeping its name.
4. Move `repo_root/profile.ron` and `repo_root/run_history.log` into `data/`,
   each only if the destination does not already exist.

- [ ] **Step 1: Write the failing tests**

Each builds two scratch directories and asserts on the *files*, not on a
return value:

- `saves_move_into_an_empty_data_dir` — two `.bin` in the repo, none in data:
  both arrive, and the source no longer has them.
- `a_populated_data_dir_is_left_alone` — one `.bin` already in data: the
  repo's `.bin` files stay where they are. **This is the test that stops a
  second run eating a newer save.**
- `the_legacy_root_save_moves_too` — `repo_root/save.bin` with no `saves/`
  directory at all: it lands at `data/saves/save.bin`.
- `the_profile_and_history_move` — both present in the repo, absent in data:
  both arrive.
- `an_existing_profile_is_not_overwritten` — a `profile.ron` in data with
  known contents, another in the repo: the data one is unchanged.
- `nothing_to_migrate_is_not_an_error` — an empty repo directory: the call
  returns and creates no files.

- [ ] **Step 2: Run the tests and watch them fail**

- [ ] **Step 3: Implement**

`std::fs::rename` across directories can fail with `EXDEV` if the data
directory is on a different filesystem from the repo, which is entirely
possible on Linux. So each move is rename-then-fallback-to-copy-and-delete,
and a failure of both is swallowed — the game starts either way. Do not
propagate.

- [ ] **Step 4: Run the tests and watch them pass**

- [ ] **Step 5: Commit**

`feat(paths): one-time move of player data into the OS directory`

---

### Task 4: wire the launcher

**Files:**
- Modify: `crates/launcher/src/main.rs`
- Modify: `crates/launcher/src/dev_template.rs:62-66`
- Test: `crates/launcher/src/dev_template.rs`'s existing tests
  (`a_working_copy_lands_in_saves_under_the_dev_prefix`,
  `resolving_a_template_generates_its_working_copy`) must be updated, not
  deleted — they assert the working copy lands beside real saves, which is
  still the rule, just at a new address.

**Interfaces:**
- Consumes: `paths::resolve()`, `paths::saves_dir()`, `paths::migrate_from_repo()`.

- [ ] **Step 1: Update the two `dev_template` tests to the new address**

`working_copy(name)` must equal `paths::saves_dir().join("dev_<name>.bin")`.
Change the assertion, run it, watch it fail against the old implementation.

- [ ] **Step 2: Point `working_copy` at `paths::saves_dir()`**

Its doc comment currently explains that `savetool template` and the game's
`--template` flag must land on the same file; that reason is unchanged and the
comment should say the address now comes from `paths`. Run the two tests.

- [ ] **Step 3: Replace the path block in `main.rs`**

Lines 37-54 become a `paths::resolve()` call, `create_dir_all` on the saves
directory, and one `paths::migrate_from_repo` call. The old inline legacy
`save.bin` migration at 46-49 is *deleted* — Task 3 subsumed it, and leaving
both means two things move the same file. The `App::new` call takes
`paths.data.join(...)` for the three player files, and for the two dev paths
takes `paths.dev`'s values when present, falling back to paths under
`paths.data` when it is `None`.

The comment at 51-53 (why the history and profile sit beside `saves/` rather
than in it) is still true and must survive the edit.

- [ ] **Step 4: Add the Windows subsystem attribute and the error file**

At the top of `main.rs`:

```rust
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
```

Then one helper that both fatal startup conditions go through: write the
message to `startup-error.txt` beside `current_exe()` *and* `eprintln!` it,
then `exit(1)`. Both, unconditionally — the file is useless to a developer
watching a terminal and stderr is invisible to a player, and branching on
which is which is a `cfg` nobody will maintain. The two conditions are:

- `paths.assets` is not a directory (a shipped build missing its assets, or a
  bad `FERAL_ASSETS_DIR`) — message must name the path it looked in.
- `create_dir_all` on the saves directory failed — message must name the path
  and the io error.

`graphics_available()` stays exactly as it is, including its Linux `cfg` and
its doc comment.

- [ ] **Step 5: Run the launcher's tests and the full suite**

`cargo test -p feral-processes` then `cargo test --workspace`. The workspace
suite must be green: nothing outside the launcher was touched, and every test
in it builds `App` with explicit paths, so nothing can reach `resolve()`.

- [ ] **Step 6: Play it once on Linux**

```sh
cargo run
```

Start a run, save, quit, relaunch, load it. Confirm
`~/.local/share/feral-processes/` now holds `saves/`, `profile.ron` and
`run_history.log`, and that the repo's `saves/` is empty. **Back up `saves/`
and `profile.ron` before the first run** — this is the migration firing on
real data for the first time.

Then `cargo run -- --template extraction` and confirm the working copy lands in
the new saves directory and appears in the load menu.

- [ ] **Step 7: Commit**

`feat(launcher): resolve every runtime path through paths.rs`

---

### Task 5: the release layout and its documentation

**Files:**
- Create: `packaging/windows-readme.txt`
- Modify: `CLAUDE.md`, `AGENTS.md`, `docs/seams.md`, `CHANGELOG.md`

- [ ] **Step 1: Write the seam entry**

`docs/seams.md` gets the argument, `CLAUDE.md` the one-or-two-line rule under
**Load-bearing seams**, both under the same title: **"There is one place a
runtime path is decided."** The trap to name is a second site resolving
against `CARGO_MANIFEST_DIR` because it is convenient in a dev build — it
works on the build machine, works nowhere else, and nothing fails to compile.
Note that the four dev bins keep `repo_root()` on purpose, so "one place" is
about the *game's* paths.

- [ ] **Step 2: Add the release sequence to `CLAUDE.md`'s Build & test section**

The four steps from the spec: MSVC toolchain, VS Build Tools with "Desktop
development with C++", `cargo build --release`, copy exe + `assets/` and zip.
Plus the note that cross-compiling from Linux is deliberately not supported.

- [ ] **Step 3: `cp CLAUDE.md AGENTS.md`**

They are gitignored twins with no tracking to catch drift.

- [ ] **Step 4: Write `packaging/windows-readme.txt`**

Ships inside the zip. Four things and nothing else: what the game is, that
saves live in `%APPDATA%\feral-processes`, that a mod is a `.ron` dropped into
`assets\`, and that SmartScreen will warn because the build is unsigned.

- [ ] **Step 5: Add the `CHANGELOG.md` section and bump the version**

Per the release-per-change rule, at merge — read `CHANGELOG.md`'s preamble for
which digit moves rather than guessing. No save format change, so this is not
a minor bump on that ground.

- [ ] **Step 6: Commit**

`docs: the runtime-path seam and the Windows release sequence`

---

## After the last task

- `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt --check`.
- **`README.md:52` is now false** — it tells players the clone must stay put.
  The root README is carved out of this repo's documentation obligation, so
  flag it for the user and do not edit it.
- Everything about the Windows runtime is unverified until someone runs the
  ten-step manual checklist in the spec on a Windows machine. Say so plainly
  when reporting the work done; a green Linux suite is not evidence that any
  of this works on Windows.
