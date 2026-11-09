# 0046 Pointer targets are registered while drawing, and a hit is a key

- Status: Agreed
- Decided by: user, on Claude's proposal (approved 2026-11-09; the shared layer and migrating both existing users were the user's picks)
- Date: 2026-11-09
- Pattern: Adapter (port), extending 0018; immediate-mode hit list
- Depends on: [0018](0018-only-paint-names-a-graphics-library.md), [0045](0045-two-hubs-split-by-subject-and-tab-switches-tabs.md)

## Context

Two screens take the mouse, and each does it its own way:

- **Sprite editor and picker.** `render::sprite_forge::sprite_hits` *recomputes* the layout to get `HitRects`. `lib.rs::handle_sprite_pointer` keeps a `SpritePointer` tracker for press/drag/up, and resolves to `app_core::PointerHit`. `EditorButton::key` makes a button press its key.
- **Map.** The draw pass *stashes* `MapClickLayout` through `Fx::set_map_click`. `lib.rs::handle_map_pointer` keeps a `MapPointer`, resolves to a cell, and calls `App::set_map_hover` / `App::map_click` (`app/pointer.rs`).

The hubs would be a third copy. They need hover as well as click, over rail entries, tabs, rows, keycaps and "↓ n more" markers.
`egui`'s widgets cannot help: `render/` draws through `Painter` (ADR 0018), so to egui the screen is shapes with no identity.

## Decision

**A screen registers each pointer target as a rect plus a semantic id while it draws. One per-frame reader turns egui's pointer into a single event against the previous frame's targets. app-core holds the hover and turns every hit into a call the keyboard already makes.**

- **Registration is a `Painter` operation** (`Painter::target(rect, id)`), so the draw that places a thing is the only place its rect is known. It is never recomputed. It names no graphics type.
- **The id is an app-core enum** (`PointerHit`, moved from `sprite_editor.rs` to `app/pointer.rs` and widened: rail page, tab, row, keycap, more-marker, plus the existing cell, swatch, button, subject and map tile). gui never matches on `app.mode` to decide what a hit means.
- **One reader in gui** (`crates/gui/src/pointer.rs`) owns frame-to-frame state: press, drag, release, double-click, wheel, armed-after-open. It is the only code that reads `egui` pointer input. It emits a `PointerEvent` to `App::handle_pointer`.
- **Hover is app state.** `App::pointer_hover: Option<PointerHit>` generalises `App::map_hover`. A renderer asks `app.hovered(&id)` to draw a highlight, so hover and the click it previews agree.
- **A hit is a key.** `App::handle_pointer` maps each hit to the key, or the sequence of keys, it stands for, through the existing handlers (`EditorButton::key`'s rule, made general). A row click selects the row; a second click or a double-click is Enter.
- **Hover runs one frame behind the draw**, because targets come from the frame just drawn.
- **Both existing users move onto it.** The sprite editor's strokes and the map's camera-aware tile become target kinds, `SpritePointer` and `MapPointer` fold into the reader, and `sprite_hits` and `MapClickLayout` are deleted.

## Alternatives considered

- **A hub-specific copy.** Least code now. It is the third divergent copy, and the two existing ones already disagree on recompute vs stash.
- **Draw calls return `Response { hovered, clicked }` (egui-style).** Puts behaviour in the renderer. app-core owns behaviour (ADR 0014; `app/pointer.rs`'s rule that gui has no `match app.mode`).
- **A target registry beside `Painter` rather than in it.** Every draw function would take a second parameter, for no gain over a recording op on the object they already hold.
- **Migrate later.** The user chose to migrate both now (2026-11-09).

## Consequences

- Any screen gains mouse support by registering targets and adding a hit kind. Popups and the battle screen are the obvious next users (not proposed).
- The drawing seam's operation count grows by one. Update `.claude/rules/drawing-seam.md`, the `seams` skill and `seam:` graph entry.
- Mouse and keyboard cannot drift, because a click is a key.
- Rows need `Row::Item` ordinal == app-core's selection index on every hub screen. A census enforces it.
- Recomputing a layout for hit-testing is retired as a pattern.

## Evidence

`paint::Painter`, `app_core::{PointerHit, PointerButton, PointerPhase, EditorButton}`, `App::{handle_pointer, set_map_hover, map_click, map_hover}`, `crates/gui/src/lib.rs` (`SpritePointer`, `MapPointer`, `handle_sprite_pointer`, `handle_map_pointer`), `render::sprite_forge::{sprite_hits, HitRects}`, `render::MapClickLayout`, `Fx::set_map_click`.
