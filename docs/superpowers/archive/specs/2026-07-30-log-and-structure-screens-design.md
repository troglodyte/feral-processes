# Two read-only screens: message history and the structure roster

The map's log pane is ~28% of the window and holds a handful of lines, so
anything that scrolls past is gone. And nothing anywhere lists the base as a
whole: what has been built, where, and which program is on it. Both gaps are
answered by a screen you open, read, and close — no actions on either.

## Scope

Read-only. Neither screen assigns, demolishes, upgrades, or clears anything;
both are reached from `Mode::Playing` and leave with Esc.

Explicitly **not** in scope: a longer or persisted message archive. The engine
keeps the last `MESSAGE_LOG_CAP` (100) lines and
`MessageLog::retain_outcomes_since_battle` drops a finished battle's
blow-by-blow, so the history screen shows what the run still holds and says so
on screen rather than implying a full transcript. Extending that is a separate
change with a save-format decision attached.

## Shared shape

Both are `Mode`s drawn as `PopupSize::Large` popups through the existing
`draw_popup`, so they inherit its sizing, its header/body/footer split and its
scroll window. Rows are emitted as `Row::Item`, because `popup_layout` derives
the scrollable body from the first and last `Row::Item` and follows the
selected row — that is the scrolling machinery, and a screen built from
`Row::Text` alone would have none of it.

`App::selected_index` already moves `menu_selected` on Up/Down with wrapping,
so the highlighted row doubles as the scroll cursor and no new scroll state is
needed. Both handlers discard the index it returns: nothing is selectable, and
a digit or letter press therefore does nothing.

## Screen 1 — `Mode::History`, opened with `L`

Shows `Game::message_log(MESSAGE_LOG_CAP)` oldest-first, matching the log
pane's order, with the cursor starting on the newest line.

- `MESSAGE_LOG_CAP` becomes `pub` so the frontend can ask for exactly what is
  retained instead of guessing a number.
- Lines keep their `MessageKind` colour by passing `message_color(kind)` into
  `Row::Item`'s existing `color` field. `message_color` is the function the log
  pane's `draw_message_line` already calls — one source, not a copy.
- A footer states both limits plainly: the last 100 lines, and that a finished
  intrusion keeps only its results.
- Long lines clip rather than wrap; `draw_popup` does not wrap, and adding
  wrapping for this screen alone is out of scope.

## Screen 2 — `Mode::Structures`, opened with `B`

New engine query in `game/inspection.rs`:

```rust
pub fn structure_report(&mut self) -> Vec<StructureReport>
```

`StructureReport` joins `views.rs` beside `EntityView`. Per structure: label,
`tier`, tile, distance from the player, `durability` as `Option<(u32, u32)>`,
and **every** assignee as `(label, TaskKind, progress, required)`.

Two constraints drive that shape:

- **Zone-wide, no radius.** `MENU_SCAN_RADIUS` would hide a cronjob worker
  parked at a far-flung node — the case `tests/support.rs`'s
  `app_owning_distant_programs` exists to cover.
- **`view_entities` cannot be reused.** Its `worker_by_structure` is a
  `HashMap` keyed by the task's target, so a guard and a cronjob worker on one
  structure collapse into one entry. The roster's whole point is showing both.

Rows group by structure kind with the Home first, then nearest-first within a
kind. A structure whose def declares `work` but has nobody on it reads as
idle — that is the actionable half of the screen. A header line counts
structures, assignees and idle workable structures.

Underground, the distance is measured from the dungeon entrance, because that
is the tile `Position` is pinned to while the party is below (see the
load-bearing seam in CLAUDE.md). Tile coordinates are absolute and correct
either way. The screen is read-only and writes no `Position`, so it needs no
`require_surface` guard.

## Tests

Engine (`crates/engine/src/tests/`):

- two assignees on one structure — a guard and a cronjob worker — both reported
- a structure past `MENU_SCAN_RADIUS` is still reported
- tier and durability are reported, and are `None` for a structure whose def
  declares neither
- a workable structure with no assignee reports an empty assignee list

app-core (`crates/app-core/src/tests/`):

- `L` opens `Mode::History`, Esc returns to `Mode::Playing`
- Up/Down move `menu_selected` within the history without leaving the screen
- `B` opens `Mode::Structures`, Esc returns to `Mode::Playing`
- neither screen mutates the game: the tick count is unchanged across opening,
  scrolling and closing

## Documentation obligations

- `Mode::is_battle`'s exhaustive match gains both variants — it is exhaustive
  precisely so a new mode cannot be added without being classified.
- The help screen's keybind list gains both keys.
- Root `README.md` and `CHANGELOG.md`.
