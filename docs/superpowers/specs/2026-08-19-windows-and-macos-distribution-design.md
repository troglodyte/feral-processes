# Windows and macOS distribution

**Status:** Approved 2026-08-19. Not implemented. (Status headers in this
directory are written at approval time and go stale — see `INDEX.md`; answer
"did this ship" from `CHANGELOG.md`, never from here.)

## The problem

Nothing in this codebase is Linux-only. Measured 2026-08-19:

- Exactly one `cfg(target_os)` in the whole tree — `graphics_available()` in
  `crates/launcher/src/main.rs:102`, whose `not(linux)` arm returns `true`.
  No `std::os::` anywhere, no `Command`, no `HOME`.
- The dependency graph resolves for both targets: 355 crates for
  `x86_64-pc-windows-msvc`, 366 for `aarch64-apple-darwin`. Neither pulls a
  `-sys` crate needing a pkg-config system library. **Linux is the fussy
  one** — `alsa-sys` wants `libasound2-dev`, plus `wayland-sys`. The `x11`
  and `wayland` features in `crates/gui/Cargo.toml` are inert off Linux;
  neither crate appears in those graphs.
- `cargo check -p feral-processes-app-core --target aarch64-apple-darwin`
  finishes clean in 9.9s — the engine and app-core are portable today.
- The same check on `feral-processes-gui` reaches `blake3`'s build script and
  fails only because the host `cc` cannot take `-arch arm64
  -mmacosx-version-min=11.0`. That is a cross-compilation artifact, not a
  portability defect.
- No reserved Windows filenames in the tree, no `:*?"<>|` in any path, no
  case-only collisions, longest tracked path 91 chars. `Path::file_name()`
  (the arena's escape guard, `crates/app-core/src/app/arena.rs:201`) handles
  `\` on Windows, and `str::lines()` strips `\r`, so a CRLF checkout of the
  `.ron` assets parses.

What is Linux-only is the **distribution model**, and it is Linux-only by
accident rather than by design. Every runtime path derives from
`dev_template::repo_root()`, which is `env!("CARGO_MANIFEST_DIR")` — the
absolute path of the machine that compiled the binary. `README.md:52` already
tells players the clone has to stay put. So the game has no distributable
build on *any* platform; Windows is what makes that unavoidable rather than
merely untidy.

Two smaller things follow from wanting a release binary at all:

- There is no `windows_subsystem` attribute, so a release `.exe` opens a
  console window behind the game.
- Fonts and the six sound cues are `include_bytes!`d, but species, items,
  structures, abilities, talents, perks, achievements, affixes, descriptions,
  sectors and policies are read from disk at runtime and **must stay loose
  files** — that is the moddability rule. So the deliverable is an executable
  plus an `assets/` tree, never a single file.

## What this does not do

- **No CI.** Verification is manual, by choice. The "Testing" section says
  plainly what stays unverified as a result.
- **No dev tooling on Windows.** `savetool`, `arena`, `train` and `tuner`,
  and the `dev-saves/`, `dev-arenas/`, `dev-training/`, `dev-logs/`
  directories, stay repo-bound and Linux-only. They keep `repo_root()`
  deliberately — a tool that only ever runs out of the repo should resolve
  out of the repo. `arena.sh` and `saves-roundtrip.sh` stay Linux-only too.
- **No test suite on Windows.** `cargo test --workspace` remains a Linux gate.
- **No cross-compilation.** See "Building on Windows".
- **No packaging automation.** A documented three-step sequence, not a script
  and not `cargo-dist`. One release has been cut this way zero times; a build
  tool ahead of that is speculative.

## Decisions taken, and why

Recorded so they are not re-litigated.

**One module decides every runtime path.** A new `crates/launcher/src/paths.rs`
is the single answer to "where does the game find X", and `main.rs` reads
nothing else. This is the seam the whole change exists to create, and the trap
it closes is a second site resolving a path against `CARGO_MANIFEST_DIR`
because it happens to be convenient in a dev build — that site works on the
build machine and nowhere else, and nothing fails to compile.

This is a **launcher-only** change to production code. Every other crate
already takes its paths as parameters: `App::new` is handed six of them, and
`Game::new`/`Game::load` take `&Path` to the assets. That existing discipline
is what keeps the blast radius to one crate. The `CARGO_MANIFEST_DIR` uses
elsewhere in the tree are all inside `#[cfg(test)]` modules — including
`crates/gui/src/lib.rs:337` — and stay exactly as they are.

**Installed-ness is sniffed, not flagged.** `paths::resolve()` asks whether an
`assets/` directory sits beside `current_exe()`. If yes, this is an installed
build. If no, fall back to the repo root as today. A shipped build cannot run
without `assets/`, so the probe tests something that is required to be true.

A cargo feature (`--features bundled`) was rejected: forgetting the flag
produces a zip that works only on the build machine, and that failure is
invisible until a stranger unzips it. Chosen verification here is manual, so a
footgun that only a stranger can trip is the worst available shape. A
`build.rs` that copies `assets/` into `target/debug/` was rejected outright —
it duplicates roughly two hundred files on every build and puts a stale copy
between the developer and an asset edit, which is actively hostile in a game
whose content lives in those files.

**Player data goes to the OS data directory in every layout, including a repo
build.** `%APPDATA%\feral-processes\` on Windows,
`~/Library/Application Support/feral-processes/` on macOS,
`$XDG_DATA_HOME/feral-processes/` or `~/.local/share/feral-processes/` on
Linux. Saves, `profile.ron` and `run_history.log` all live under it.

Writing beside the executable ("portable") was rejected because a build
unzipped under `Program Files` cannot write there, and the failure mode is a
game that appears to save and silently doesn't. A split rule — data directory
when installed, repo when developing — was rejected for two reasons: it is two
code paths where one will do, and it means a dev build cannot reproduce a
player's report about save location. The cost of uniformity is that
`cargo run` and a shipped build share one save directory, which for a
single-developer game is a feature: the same runs are visible to both.

**A one-time migration moves the repo's `saves/` into the data directory.**
Same shape as the legacy `save.bin` migration already in `main.rs:46`, and for
the same reason — a save that disappears from the load menu reads as data
loss. It is a **move, not a copy**: two save directories that drift is worse
than one that moved. It runs only when the destination holds no `.bin`, so it
cannot fire twice or clobber newer saves.

**`dirs` 6 in the launcher, and nowhere else.** Reading `%APPDATA%` by hand
misses folder redirection; `dirs` calls `SHGetKnownFolderPath`, which does
not. `directories` (the mid-level crate, `ProjectDirs` with a qualifier and
organisation) was rejected as a larger surface for the same answer — this game
needs one directory, not a qualified triple. On Windows the crate's transitive
`windows-sys` is already in the graph, so it costs nothing there. It must not
appear in `crates/engine` or `crates/app-core`: those crates are handed paths,
and that is the rule that keeps this change small.

**`FERAL_ASSETS_DIR` overrides the assets directory, and nothing else
overrides anything.** A modder can point a build at an alternative asset tree
without disturbing the install, and it is the natural switch for testing a
shipped build against repo assets. The spec explicitly refuses a second
environment variable per path — a per-path override matrix is how "one module
decides every path" stops being true.

**The console window goes away in release builds only.**
`#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]`
at the top of `main.rs`. A debug build keeps stderr, which is what a developer
on Windows would want. The consequence is that the `eprintln!`-then-exit paths
go silent in a release build: three of them are dev/CLI paths a player cannot
reach (bad arguments, a bare `--template`, a template that fails to generate),
and the fourth — the data directory failing to be created — is real. That one,
and a missing `assets/` directory, write a `startup-error.txt` next to the
executable and exit. A message box was rejected as a dependency bought for two
error strings.

## The module

```rust
/// Every runtime path the game reads or writes. Built once in `main`.
pub struct Paths {
    /// Loose asset tree: species, items, structures, abilities, …
    pub assets: PathBuf,
    /// Player data: `saves/`, `profile.ron`, `run_history.log`.
    pub data: PathBuf,
    /// Repo-only directories. `None` in an installed build.
    pub dev: Option<DevPaths>,
}

pub struct DevPaths {
    pub arenas: PathBuf,      // dev-arenas/
    pub battle_log: PathBuf,  // dev-logs/battles.jsonl
}

pub fn resolve() -> Result<Paths, PathError>
```

Resolution order, and it is the whole rule:

| Path | Installed build | Repo build |
|---|---|---|
| `assets` | `FERAL_ASSETS_DIR`, else beside `current_exe()` | `FERAL_ASSETS_DIR`, else `<repo>/assets` |
| `data` | OS data directory | OS data directory |
| `dev` | `None` | `Some` — `<repo>/dev-arenas`, `<repo>/dev-logs/battles.jsonl` |

`main.rs` then builds `App::new` from `paths.data.join("saves")`,
`paths.data.join("run_history.log")`, `paths.data.join("profile.ron")`, and
the two dev paths — which need a value even when `dev` is `None`, since
`App::new` takes them unconditionally. An installed build passes paths under
`data` that will never be created: the arena and template rows are gated
behind `FERAL_DEV_ARENA`, and both `dev_template::list()` and the arena
catalog already handle a missing directory with
`let Ok(entries) = read_dir(…) else`. That graceful degradation exists today
and needs no work — it is why an installed build with no `dev-arenas/` is not
a special case.

`dev_template::repo_root()` stays exactly as it is, used by `savetool`,
`arena`, `train`, `tuner` and the `--template` flag. The `--template` flag in
an installed build fails the way it does today when a name is unknown: an
error and a non-zero exit, which in a release Windows build means
`startup-error.txt`. That is acceptable for a developer-only flag.

## Release layout

```
feral-processes-0.11.9-windows-x64/
  feral-processes.exe
  assets/
  README.txt        # what this is, where saves go, how to add a mod
```

Data directory by platform:

| OS | Location |
|---|---|
| Windows | `%APPDATA%\feral-processes\` |
| macOS | `~/Library/Application Support/feral-processes/` |
| Linux | `$XDG_DATA_HOME/feral-processes/`, else `~/.local/share/feral-processes/` |

The zip is unsigned, so Windows SmartScreen will warn on first run and the
user has to click through "More info → Run anyway". Signing is a certificate
purchase and is out of scope; the `README.txt` should say so rather than
letting it read as a malware warning.

## Building on Windows

1. `rustup` with the default `x86_64-pc-windows-msvc` toolchain.
2. Visual Studio Build Tools with "Desktop development with C++" — several
   build scripts in the graph compile C or assembly (`blake3` among them) and
   the linker comes from there.
3. `cargo build --release`.
4. Copy `target\release\feral-processes.exe` and the `assets\` directory into
   the release folder, zip it.

**Cross-compiling from Linux is not recommended and is not part of this
spec.** It is possible — `x86_64-pc-windows-gnu` with mingw-w64, or
`-msvc` with `cargo-xwin` — but none of that is installed on the development
machine (no `x86_64-w64-mingw32-gcc`, no `cargo-xwin`, no wine), the GNU ABI
is the less-travelled path for wgpu, and a Windows machine is needed for the
manual verification regardless. Revisit only if producing artifacts without
booting Windows becomes the bottleneck.

## Testing

**What can be tested on Linux, and must be.** `paths.rs` is a pure function of
its inputs once `current_exe()` and the environment are parameters rather than
calls, so the module splits into a testable core and a thin `resolve()` that
supplies the real values:

- An `assets/` beside the exe selects the installed layout, and `dev` is
  `None`.
- No `assets/` beside the exe selects the repo layout, and `dev` is `Some`.
- `FERAL_ASSETS_DIR` wins over both, in each layout.
- An empty `FERAL_ASSETS_DIR` is treated as unset, matching
  `dev_console::dev_flag`'s existing rule for the three `FERAL_DEV_*` flags.
- The migration moves a repo `saves/` into an empty data directory, and does
  **not** move it when the data directory already holds a `.bin`.
- The migration is a no-op when there is no repo `saves/`.

The existing suite is unaffected: every test builds `App` with explicit
paths, so nothing in it can reach `paths::resolve()` and no test can trip the
migration. That is the protection, and the rule that keeps it true is that
`resolve()` has exactly one caller — `main`.

**What cannot be tested here.** Everything about the Windows runtime — window
creation, DX12 through wgpu, WASAPI audio, keyboard input, the console
suppression, SmartScreen, and whether `%APPDATA%` resolves as expected. With
manual-only verification, that list is the honest statement of what ships
unverified until someone runs the checklist.

**Manual checklist, Windows**, in order:

1. `cargo build --release` completes on a clean Windows machine.
2. The game window opens, and **no console window appears behind it**.
3. Keyboard input moves the player; the map draws; the map font renders
   (unscii-16 is embedded, so a missing glyph here means a font-loading
   problem, not a missing file).
4. Audio plays — a step and a battle-start cue.
5. Starting a run creates `%APPDATA%\feral-processes\saves\`, and the file
   appears in the load menu after a relaunch.
6. `profile.ron` and `run_history.log` land beside it, not in the game folder.
7. Unzip to a path containing a space and a non-ASCII character; it still
   runs.
8. Unzip under `Program Files`; it still runs and still saves.
9. Delete `assets/` and launch: `startup-error.txt` appears with a readable
   message, and the process exits rather than hanging or crashing.
10. Drop a new `.ron` into `assets\species\` beside the exe and confirm it
    loads — this is the moddability rule surviving the move.

## macOS, as an afterthought

The same module covers it, and the engine plus app-core already cross-check
clean for `aarch64-apple-darwin`. Two facts are macOS-specific:

- **A `.app` bundle moves the goalposts.** Inside a bundle the executable is
  at `Contents/MacOS/`, and resources belong at `Contents/Resources/`. So the
  "beside the exe" probe needs a second candidate — `../Resources/assets` —
  checked before giving up. It is three lines and belongs in `paths.rs` from
  the start rather than as a later special case, but it is only *exercised*
  if a bundle is ever built.
- **Gatekeeper blocks unsigned binaries**, and the workaround (right-click →
  Open, once) is worse on a bundle than on a plain binary.

Recommendation: ship a plain `feral-processes` binary plus `assets/` in a zip
first, exactly like the Windows layout, and build a `.app` only if the game is
ever handed to a non-technical macOS user. Both architectures need their own
build; `aarch64-apple-darwin` is the one worth cutting if only one is.

## Documentation obligations

- `CHANGELOG.md` gets a section at merge, per the release-per-change rule.
- `CLAUDE.md` and `docs/seams.md` get a matching entry: **there is one place a
  runtime path is decided, `crates/launcher/src/paths.rs`**, with the trap
  being a second site resolving against `CARGO_MANIFEST_DIR` and working only
  on the build machine. `AGENTS.md` is the gitignored twin — `cp` after
  editing.
- `CLAUDE.md`'s **Build & test** section gains the Windows release sequence.
- `README.md:52` — "the clone needs to stay put" — becomes false. The root
  README is carved out of the documentation obligation (2026-08-05), so this
  spec flags it rather than editing it. It is worth the user's decision,
  because it is the file a new player reads.

## Open questions

- Does the release `README.txt` need writing as part of this work, or is a
  three-line placeholder enough until there is somewhere to publish the zip?
- Should the version be visible in-game or in the zip name only? The zip name
  is assumed here.
- Is a `package.ps1` wanted after the first manual release, or do the four
  documented steps stay the process?
