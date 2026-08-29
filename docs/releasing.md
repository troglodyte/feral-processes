# Cutting a release

The per-platform packaging checklists. `CLAUDE.md`'s **Guardrails** section
holds the versioning rule (one release per change landing on `main`); this
file holds what to actually run on each machine, and why each of these is
manual by choice.

**Cutting a Windows release.** Manual by choice — no CI, no
`cargo-dist`, and cross-compiling from Linux is deliberately not supported
(nothing is installed for it, the GNU ABI is the less-travelled path for
wgpu, and a Windows machine is needed for the manual checklist regardless).
On a Windows machine: `rustup` with the default `x86_64-pc-windows-msvc`
toolchain, and Visual Studio Build Tools with "Desktop development with
C++" — several build scripts in the graph compile C or assembly (`blake3`
among them) and the linker comes from there. Then `cargo build --release`,
and copy `target\release\feral-processes.exe`, the `assets\` directory
and `packaging/windows-readme.txt` (as `README.txt`) into a release folder
and zip it. The deliverable is an executable **plus a loose `assets/`
tree**, never a single file — fonts and the sound cues are `include_bytes!`d
but game content must stay droppable, which is the moddability rule. The zip
is unsigned, so SmartScreen warns on first run.

**Cutting a macOS release.** The same shape, and the same manual-by-choice
rule. On a Mac: `rustup`, Xcode Command Line Tools (`xcode-select
--install`, for the linker and the C compiler several build scripts need),
then `cargo build --release`. Copy `target/release/feral-processes`, the
`assets/` directory and `packaging/macos-readme.txt` (as `README.txt`) into
a release folder and zip it. **Each architecture needs its own build** —
`aarch64-apple-darwin` is the one worth cutting if only one is.

**Ship a plain binary, not a `.app` bundle**, until the game is handed to
someone who will not open a Terminal. `paths.rs` already probes
`../Resources/assets`, so a bundle costs no code — but it costs a plist, an
icon and a build step, and Gatekeeper's click-through is not obviously
better on a bundle than the `xattr -dr com.apple.quarantine .` a plain zip
documents. The wart a bundle *would* fix is that double-clicking a plain
binary in Finder opens a Terminal window behind the game, which is macOS's
version of the console `windows_subsystem` suppresses.

**Cross-compiling to macOS from Linux does not work here and is not
supposed to.** `cargo check -p feral-processes-app-core --target
aarch64-apple-darwin` passes — the engine and app-core are portable, and
`dirs` checks clean too — but the graph reaches `blake3`'s build script,
which hands the host `cc` `-arch arm64 -mmacosx-version-min=11.0`. That is
a missing macOS toolchain, not a portability defect, and a Mac is needed for
the manual checklist regardless.
