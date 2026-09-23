# Alert board

**Status:** approved design, not yet implemented.

## Intent

The player needs one place to review what is blocking production or has
happened to their programs — a program downed, a depot full, a siege
incoming, a machine stalled — and to clear each item once it has been seen.
Today these events are scattered: some are one-shot popups
(`notifications.rs`, tutorials and milestones, Esc-dismissed, not a list),
most are lines in a 100-line unsaved log mixed with everything else, and
`Game::attention` is a live derivation with no history.

The alert board is a **capped, saved, player-dismissed list** of those
events.

### What the user asked for

- A notification/alert board listing entity and production blockers.
- A hard cap; a new alert pushes the oldest off.
- Open it, Up/Down to highlight an alert, `x` or `d` to dismiss it.

### Decisions taken in brainstorming

| Question | Decision |
|---|---|
| Sources in the first cut | Machine stalls, program downed, sweep/siege incoming and hit, dig/build site cut off, depots full |
| An alert whose problem resolves itself | **Stays unchanged** — only a dismiss removes it |
| The same subject alerts again | **Collapse**: one entry per subject, `×N` count, moved to the top, marked unread |
| Persistence | **Saved** in `SaveData`, additive, no version bump |
| HUD | Unread count badge on the status bar, hidden at zero |
| Key | `N` (free on the map; `n` is mining) |
| Cap | 50, `tuning::ALERT_BOARD_CAP` |
| Read state | Opening the board marks every alert read |

It is **not** a replacement for notifications (popups stay as they are) or
for the log (every alert source keeps its log line).

## Engine

### `crates/engine/src/alerts.rs` (new)

- `AlertKind` — what happened:
  - `MachineStalled(MachineStatus)` — one of the six stall states
    (`Starved`, `Clogged`, `Unstaffed`, `Stranded`, `Unpowered`, `Dry`).
  - `ProgramDowned`
  - `SweepIncoming`, `SweepHit`
  - `SiegeIncoming`, `SiegeBegun`
  - `SiteCutOff` — a dig or build site with no route
  - `DepotsFull`

  **Variant names are save format** (field-named RON): append, never rename
  or reorder-and-rename. A retired variant must stay, as notifications'
  `latch_key` rule does for the profile.

- `Alert { kind: AlertKind, subject: String, text: String, count: u32, unread: bool }`.
  - `subject` is the collapse key and is minted by the poster: a
    structure's kind and tile for a stall (`"lathe@12,4"`), the program's
    `ProgramId` for a downed program, the site tile for a cut-off, and the
    kind alone for zone-wide events (sweep, siege, depots full).
  - The collapse identity is `(kind discriminant, subject)` — a Lathe that
    goes Clogged then Unpowered is **two** alerts, since they need different
    fixes; the same Lathe clogging twice is one alert with `count: 2`.
    For `MachineStalled` the discriminant includes the status.
  - `text` is the sentence the log line already carries for that event, so
    the board and the log never word the same event differently. Where a
    poster builds a log line, it builds the text once and passes it to both.

- `AlertBoard(VecDeque<Alert>)`, newest first, a `Resource`.

- **`alerts::post(board: &mut AlertBoard, kind, subject, text)` is the one
  door.** A free function, not a `Game` method, because
  `set_machine_status` is reached from bevy systems that have no `Game` —
  the same reason as `needs::strain` and `memories::sum_intensity`.
  `Game::post_alert` is a one-line call into it. Behaviour:
  1. If an entry with the same identity exists: remove it, `count += 1`,
     replace `text`, set `unread`, push it to the front.
  2. Otherwise push a new entry (`count: 1`, `unread: true`) to the front.
  3. Truncate to `ALERT_BOARD_CAP` from the **back** (oldest).

  `post` draws no RNG, so an alert cannot shift the seeded stream.

### Sources

| Source | Where | When |
|---|---|---|
| Machine stall | `systems::set_machine_status` | Only on a transition *into* one of the six stall states — the branch that already logs. `Running`/`Idle` post nothing. |
| Program downed | Each caller of `Game::bench_or_dissolve` that writes the downed log line (sortie, off-screen siege, raid defender, combat teardown, tactical turn) | When the line is logged, both difficulty arms |
| Sweep incoming | `raid_check`, on the `RaidPressure.warned` latch | Once per warning, as the log does today |
| Sweep hit | `run_raid` | Once per sweep |
| Siege incoming | `siege/clock.rs`, on the `SiegePressure.warned` latch | Once per warning |
| Siege begun | `Game::open_siege`, and the off-screen resolution in `siege/offscreen.rs` | Once per siege |
| Site cut off | `announce_dig_cut_off`, `announce_cut_off` | On their existing `announced_stuck` latch |
| Depots full | `hauling.rs`, the branch where a load finds every depot full (~:998) | On a new latch, below |

**Depots full is the one new detection.** Today it has no state: the load
goes back and the machine re-clogs. The haul branch runs many times a tick,
so posting on every hit would climb `count` without bound. A latch,
`resources::DepotsFullLatch(bool)`, is set when the branch fires and posts only on the
false→true edge; the next successful deposit into any depot clears it. The
latch is not saved: at worst a reload re-posts once, collapsed into the
existing entry.

### `Game` doors

- `alerts(&self) -> Vec<views::AlertView>`, newest first, where
  `AlertView { kind: AlertKind, text: String, count: u32, unread: bool }`.
  gui reads the view and never `Alert`: the collapse `subject` is engine
  bookkeeping.
- `unread_alerts(&self) -> usize`
- `mark_alerts_read(&mut self)`
- `dismiss_alert(&mut self, index: usize)` — out of range is a no-op.

### Save

`SaveData::alerts: Vec<Alert>` behind `#[serde(default)]`, written by
`Game::save` and restored in `Game::load` as `work_orders` is
(`lifecycle.rs`). Additive in field-named RON, so **no
`SAVE_FORMAT_VERSION` bump**. Load re-applies the cap, so a hand-edited
save cannot exceed it.

## App-core

- `Mode::Alerts`.
- `N` in `handle_playing_key`'s **top match**, before the hand-off to
  `handle_stack_key`, so the board opens on the surface, in base space and
  in the Stack alike. Opening sets `menu_selected = 0` and calls
  `mark_alerts_read`.
- `handle_alerts_key`:
  - `x` / `d` dismiss the highlighted alert, **handled before
    `selected_index`** so they never act as row shortcuts (lowercase letters
    are row selectors on every other list). Selection is clamped to the new
    length.
  - Up/Down move `menu_selected` through `selected_index`. Digit and
    other-letter shortcuts may select rows as usual.
  - Esc returns to `Mode::Playing`.
- The board does not gate `show_next_notification`: that already fires only
  from `Mode::Playing`, so a popup waits until the board closes.

## GUI

- `crates/gui/src/render/alerts.rs`: a popup list.
  - Row: unread marker, the alert text, `×N` when `count > 1`.
  - Highlighted row for `menu_selected`.
  - Empty board: "No alerts."
  - Footer: `[↑↓] select  [x/d] dismiss  [Esc] close`.
  - **The list scrolls.** 50 rows do not fit a popup, so it draws the
    window of rows around the selection. This is the first scrolled list
    screen; the window arithmetic is a pure function with its own tests.
- `Mode::Alerts` added to the draw match **and to `ALL_MODES`** by hand —
  a missing entry ships a blank screen and does not fail to compile.
- Status bar: an unread badge beside the attention badge, e.g. `N 3`,
  drawn only when `unread_alerts() > 0`.
- Stall alerts reuse the stall palette roles (`ATTENTION` for
  Clogged/Stranded/Unpowered, `WARN` for Starved/Unstaffed); sweep and siege
  take `THREAT`.
- Row text must fit the popup: a width census over the longest text each
  source can produce, measured through `paint::with_painter`.

## Testing

Engine:
- `post` collapses the same identity, bumps `count`, marks it unread and
  moves it to the front.
- The same subject with a different stall status is a separate entry.
- The cap drops the **oldest**.
- `dismiss_alert` removes exactly the indexed entry; out of range is a
  no-op.
- One test per source row in the table above, each proving the source
  posts — and for the latched sources, that it posts once and not twice.
- Depots full: posts once across repeated failed hauls, and posts again
  only after a successful deposit clears the latch.
- **save → load** round trip preserving order, counts and unread. A RON
  round-trip test alone passes against a `#[serde(skip)]`.
- Each test deleted-fix checked: the fix removed, the test fails.

App-core:
- `N` opens the board from the surface and from the Stack, and marks all
  read.
- `x` and `d` each dismiss and do not select a row.
- The selection clamps after dismissing the last row.
- Esc closes.

GUI:
- The scroll-window function keeps the selection visible at both ends.
- The row-width census.
- The `ALL_MODES` census picks up the new mode.

## Seams and docs

- New seam, three writes (the `seams` skill's order): the argument to the
  memory graph, the trap to the skill, the rule to CLAUDE.md —
  *"`alerts::post` is the one door onto the board, a free function because
  `set_machine_status` has no `Game`, and collapse identity is kind plus
  subject."*
- `CHANGELOG.md` at the merge. The in-game key help lists `N`. The manual
  and root README are carved out and stay untouched.

## Out of scope

- Resolving alerts automatically, or marking them resolved.
- Per-source filtering or muting.
- "Dismiss all". Easy to add later as an uppercase key.
- Jumping to the alert's subject from the board.
