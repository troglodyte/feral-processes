# 0018 Only `paint.rs` names a graphics library

- Status: Accepted (standing)
- Decided by: user (CLAUDE.md)
- Date: in `CLAUDE.md` by 2026-09-08; recorded here 2026-10-21
- Pattern: Adapter (port)

## Context

The renderer was moved from macroquad to bevy and egui once already.
Drawing code that calls a backend directly makes the next such move, or a backend upgrade, touch every screen.

## Decision

`crates/gui/src/paint.rs` is the only file that names a graphics library.
Everything in `crates/gui/src/render/` draws through `paint::Painter`, a small set of operations over local `Color`, `Rect`, `TextDims` and `TextRun` types.
`Painter` is a struct with one implementation, not a trait.

## Alternatives considered

- Call egui directly from screens. Faster to write; a backend change touches all of `render/`.
- `Painter` as a trait. Rejected in [0011](0011-restructurings-considered-and-rejected.md): one backend, no second caller to shape it.

## Consequences

The macroquad-to-bevy swap touched five files and no drawing code.
A new drawing need is a new `Painter` operation, argued in `.claude/rules/drawing-seam.md`, not a backend call in a screen.
Layout can be made pure on top of this ([0009](0009-layout-separate-from-paint.md)).

## Evidence

- `crates/gui/src/paint.rs` — `Painter`.
- `.claude/rules/drawing-seam.md` — the operations and their traps.
