# Delete the TUI

## Problem

The workspace ships two renderers, but only one of them is a game anyone
plays. `crates/launcher/src/main.rs:4` says so in its own module doc: the
text frontend "is no longer user-selectable; it's kept solely as the
fallback renderer for the no-display/GUI-crash cases below."

So `crates/tui` — 2,388 lines, `lib.rs` 60 and `ui.rs` 2,328 — exists to
serve two situations:

1. No `DISPLAY`/`WAYLAND_DISPLAY` is set (`graphics_available()` returns
   false).
2. `feral_processes_gui::run` panicked and `catch_unwind` caught it.

It has zero tests (`grep -c '#\[test\]'` returns 0 for both files), so
nothing verifies that those 2,388 lines still render correctly. What keeps
them alive is discipline: every battle-screen change so far has been made
twice, with paired comments in both files warning that "the two renderers
cannot drift."

That cost is now being paid for a screen no player sees. The immediate
trigger is a Bard's Tale-style redesign of the battle screen (see
[Next](#next)) which would otherwise be built twice, but the reasoning holds
independently of that work.

## Approach

Delete the crate and replace its two callers with the simplest thing that
serves each case honestly.

Rejected along the way:

- **Keep the crate, stop maintaining it.** Leaves 2,328 untested lines in
  the tree that read as a supported peer renderer — including comments in
  both files instructing the next change to keep them in sync. Rot with a
  maintenance contract still attached to it is worse than either extreme.
- **Delete `graphics_available()` too.** Its docstring records why it
  exists: macroquad's platform layer *aborts the process* rather than
  returning a catchable error when there is no windowing system. Removing
  the check trades a one-line diagnostic for a raw abort. The check's
  justification survives the TUI's removal untouched; only what it does on
  failure changes.
- **Fold the deletion into the battle redesign.** One diff mixing a crate
  removal with a layout rewrite is harder to review and harder to revert
  independently. Two commits, this one first.

Nothing here touches the engine, `app-core`, or the save format.

## Changes

### 1. Remove the crate and its wiring

Delete `crates/tui/` outright.

`Cargo.toml`: drop `"crates/tui"` from `[workspace] members` and the
`feral-processes-tui` line from `[workspace.dependencies]`.

`crates/launcher/Cargo.toml`: drop the `feral-processes-tui` dependency.

No other crate is coupled to it. `crates/app-core/Cargo.toml` depends only
on `feral-processes-engine`, and `crossterm`/`ratatui` appear nowhere
outside `crates/tui` — the one remaining mention anywhere else is a
comparative aside in `crates/gui/src/render.rs:4` ("instead of ratatui"),
which is now a reference to something that does not exist and gets
rewritten.

### 2. No-display becomes an error, not a fallback

`graphics_available()` keeps its body and its docstring rationale. Its
caller changes from selecting a renderer to failing cleanly:

```rust
if !graphics_available() {
    return Err(io::Error::other(
        "No display detected; feral-processes needs a graphical display.",
    ));
}
```

That prints `Error: No display detected; feral-processes needs a graphical
display.` to stderr and exits non-zero. `main` keeps its `io::Result<()>`
return type regardless — `std::fs::create_dir_all(&saves_dir)?` needs it.

### 3. The GUI-crash fallback goes away entirely

`panic::catch_unwind`, the `AssertUnwindSafe` wrapper, the
`std::panic::{self, AssertUnwindSafe}` import, and the branch's own
`App::new` construction all go. `feral_processes_gui::run(app)` is called
directly.

Rationale: a panic in the renderer is a bug to fix, not a runtime condition
to handle. Keeping six lines of unwind machinery to print one reassuring
sentence is cruft, and the reassurance is already true and already
documented — autosaves mean a crash costs at most a few ticks, recoverable
from the load-game menu.

This also removes the reason `assets_dir`, `saves_dir`, and `history_path`
are cloned at the `App::new` call site; they can be moved.

### 4. Documentation

Two README passages become false and get rewritten. The surrounding claim
that the simulation stays decoupled from presentation is still true and
stays — it is now warranted by the engine/`app-core` boundary rather than by
there being two renderers.

`README.md:11–13`, from:

> A terminal (TUI) frontend still exists internally as a fallback for
> headless environments, but it's no longer user-selectable.

to:

> A graphical display is required; there is no text mode.

`README.md:965–966`, from:

> Both are GUI-only — the TUI fallback is silent, and sound is a frontend
> concern the simulation knows nothing about.

to:

> Sound is a frontend concern the simulation knows nothing about.

Also:

- A new `### Frontend` subsection under `## Unreleased` in `CHANGELOG.md`
  records the removal, including that headless play is gone. (`Unreleased`
  currently carries `### Combat flow`, `### Structures`, and `### Balance`.)
  `CHANGELOG.md:434` mentions the text UI but is a historical release note;
  release history is not rewritten.
- `crates/gui/src/render.rs:4` loses its ratatui comparison.
- `CLAUDE.md`'s five-crate table and its `crates/tui` line go stale. That
  file is gitignored, so the edit will not ship with the branch — it is
  worth making locally, but it is not part of the diff.

## Consequences

**Headless play is gone.** Playing over SSH or on a box with no compositor
was possible and will not be. This is the only capability lost, it was
never advertised as a feature (the launcher doc calls it a fallback), and
it is the price of not maintaining a second renderer.

**The peer-renderer discipline dissolves.** CLAUDE.md's "two peer
renderers" framing and the paired don't-drift comments in both battle
screens no longer describe anything. The architectural rule they were
protecting — that the engine's `Game` struct is the entire public API and no
renderer touches the ECS `World` — stands on its own and stays. Worth
stating plainly: with one renderer, that rule is now upheld by convention
rather than enforced by a second consumer that would break.

**Every future renderer change is made once.** This is the point.

## Testing

The TUI has no tests, so there are none to port or delete.

- `cargo test --workspace` — must stay green with the count unchanged. Any
  movement in the count means something was coupled to the TUI that this
  spec claims was not.
- `cargo clippy --workspace` — clean. The launcher edits remove the only
  uses of two imports and three clones; clippy is what catches a missed one.
- `cargo fmt`.
- `cargo build -p feral-processes` — the binary still builds with one
  renderer dependency.
- `DISPLAY= WAYLAND_DISPLAY= cargo run -p feral-processes` — prints the new
  message and exits non-zero. This exercises the only changed behaviour and
  needs no display, so it is verifiable headlessly rather than by launching
  the GUI.

## Not doing

- Any change to the GUI beyond the one stale comment. The battle-screen
  redesign is the next change, not this one.
- Any change to `app-core`. Its `Mode`/`GameKey` surface was shared by both
  renderers and is entirely reusable by the one that remains; nothing in it
  is TUI-shaped.
- Rewriting CHANGELOG history.

## Next

The work that prompted this is a Bard's Tale-style redesign of the battle
screen, to be specced separately once this lands. Decisions already settled
for it, recorded here so they survive the gap:

- **Scope is screen chrome and layout only** — not prompt wording, not
  combat-log voice, not the interaction model. No engine changes.
- **Layout is a "headed ledger":** keep today's four stacked blocks
  (hostiles / log / party / action bar). Add a column-header row inside each
  roster frame, right-align the numerics into real columns, and give
  engaged-vs-back and status effects their own upper-case columns
  (`ENGAGED`/`BACK`, `OK` when no condition). Keep the HP bars.
- **The party panel stays sized to the roster you fielded**, not padded to
  `MAX_PARTY_SIZE + 1`. Bard's Tale's fixed six-slot table earned its blank
  rows by never reflowing, which this screen already does not: the roster is
  sized once at battle start (`slots = party.len() + 1`,
  `crates/engine/src/lib.rs:2605`) and a downed companion keeps its row at
  0 HP. Padding would cost the Intrusion log up to five lines for decoration.
- **Enemy groups stay lettered** (`A`, `B`, …) rather than numbered as Bard's
  Tale had them. `EnemyGroupView::letter` is engine-owned and the target
  picker keys off it; renumbering is an engine change, which is out of scope.
- **Still open:** where the GUI's column widths and header strings live, and
  how a header row sits above rows whose HP is a graphical bar rather than
  text. The GUI's UI font is unscii-16, monospace
  (`crates/gui/src/text.rs:16`), so real columns are achievable there.
