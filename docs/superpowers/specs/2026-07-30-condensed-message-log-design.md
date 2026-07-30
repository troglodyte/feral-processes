# Condensed message log

The `L` history screen repeats itself. Automation is the reason: a base with
three cronjobs pushes a yield line per producer per cycle
(`systems.rs:219`), a levelled node pushes a failure line whenever a roll
misses (`systems.rs:173`), and a passive processor pushes one per conversion
(`systems.rs:276`). Reading the screen means scrolling past forty copies of
`Your subroutine extracted 2 Data Shard.` to find the raid that mattered.

Collapse identical lines into one row carrying a repeat count.

## Where the fold happens

In the engine, as a **view over the stored log** — storage is untouched.

`MessageLog` keeps every line verbatim. `MESSAGE_LOG_CAP`, the
`pushed - dropped == lines.len()` invariant, `battle_start`, `round_start`
and `retain_outcomes_since_battle` all stay exactly as they are. A
collapse in `push_kind` would have to avoid merging a new round's first
line backwards across `open_round` — where it would fall outside the
battle pane's range and vanish from the narration — and would move the
line counts a good number of existing tests assert on. None of that risk
buys anything the screen needs.

It cannot be gui-side either. **app-core owns the scroll and gui draws the
rows**: `menus.rs:117` and `input.rs:63` both derive the row count and the
opening row from `game.message_log(MESSAGE_LOG_CAP).len()`. A collapse
applied only while drawing would leave the highlight indexing rows that no
longer exist. The engine's public API is the one place both consumers
agree.

So: a pure fold in `resources.rs`, exposed as `Game::message_history(n)`.

```rust
pub struct LogEntry {
    pub kind: MessageKind,
    pub text: String,
    pub repeats: usize,
}
```

`repeats` is 1 for a line that stands alone, so consumers never special-case
the uncollapsed path.

## The matching rule

Walk the stored lines oldest-first. For each line, scan the **last four
emitted entries** for one with the same `kind` and the same text. Found:
bump its `repeats`. Not found: push a new entry.

Anchored at first occurrence, so the screen's order is the order things
first happened.

The window is counted in *emitted entries*, not raw lines, and that is what
makes the three cases come out right:

- An unbroken run collapses completely, however long — each new copy finds
  the anchor as the newest emitted entry.
- Two or three cronjobs interleaving their yields each cycle still collapse;
  their anchors stay inside a four-entry window.
- The same warning 300 ticks later, with unrelated lines between, falls
  outside the window and stays its own row. Two starvation events read as
  two events.

`kind` is part of the key, so the same sentence pushed as `Info` and as
`Outcome` never merge — they are styled differently and mean different
things.

The window constant lives beside the fold in `resources.rs`, not in
`tuning.rs`. `tuning.rs` is difficulty; this is presentation, the same
reasoning that keeps `REVEAL_LINES_PER_SECOND` in app-core.

## The dim suffix

`Row::Item` gains `suffix: Option<String>`, drawn after the row text in
`TEXT_DIM` through the existing `painter.ui_runs` seam that
`emphasize_numbers` already uses. The count reads as annotation rather than
as part of the sentence.

A field rather than a new `Row` variant, deliberately: `popup_layout` finds
its scroll window with `matches!(r, Row::Item { .. })` over the first and
last item rows, so a sibling variant would silently drop the history screen
out of that detection — the failure mode the manifest packer already taught
us to watch for. The five row helpers in `popup.rs` pass `None`.

`draw_history` sets `suffix: (repeats > 1).then(|| format!("×{repeats}"))`.
`U+00D7` is present in `assets/fonts/DejaVuSansMono.ttf`, verified, so the
glyph renders rather than boxing.

## Scope

The `L` screen only. The map's bottom pane and the battle pane keep every
line verbatim — the battle pane's reveal pacing counts rows, and there is no
reason to disturb narration timing for this.

## Testing

Engine, against the pure fold:

- an unbroken run of one line becomes one entry with `repeats == n`
- `A,B,A,B` inside the window becomes two entries, each `repeats == 2`
- duplicates separated by more than the window stay separate entries
- the same text under two `MessageKind`s stays two entries
- an empty log folds to no entries; a single line folds to `repeats == 1`
- `Game::message_history` reflects what was actually pushed

app-core: the history screen's opening row and scroll bounds follow the
condensed count, not the raw line count.

gui: a repeated line produces one row carrying the suffix, and a line that
stands alone carries none.

Gate: `cargo test --workspace`. No save-format, `.ron` schema or `tuning.rs`
change, so no balance-sim exposure.
