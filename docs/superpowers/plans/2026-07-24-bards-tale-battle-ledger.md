# Bard's Tale Battle Ledger Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redraw the GUI battle screen's two rosters as a headed, hard-columned
stat table, keeping the existing full-width HP bars and effects.

**Architecture:** One renderer file. `draw_battle` gains a shared row/header
formatter over fixed-width cells; `draw_bar` stops appending `hp/max` so the
caller owns column order. No engine, `app-core`, or save-format change.

**Tech Stack:** Rust 2024, macroquad 0.4, DejaVu Sans Mono (monospace) for UI
text, `fontdue` in dev-dependencies for headless font assertions.

## Global Constraints

- **Spec:** `docs/superpowers/specs/2026-07-24-bards-tale-battle-ledger-design.md`.
- **Baseline is 440 tests.** New tests raise it; nothing may break.
- `cargo clippy --workspace --all-targets` warning-free, `cargo fmt` run.
- **Do not launch the GUI to verify.** Standing project rule: drawing changes
  are verified by unit tests and code reading. Everything asserted here is pure.
- **Scope is `crates/gui` only.** No engine or `app-core` edits.
- Column widths are constants in `render.rs`; do not lift them into `app-core`.

---

### Task 1: Prove the UI font is monospace

The whole ledger rests on it, and it is currently an unstated assumption.

**Files:** Modify `crates/gui/tests/font_rasterization.rs`

- [ ] **Step 1: Add the failing test**

```rust
const UI_FONT: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSansMono.ttf");

static UI: LazyLock<fontdue::Font> = LazyLock::new(|| {
    fontdue::Font::from_bytes(UI_FONT, fontdue::FontSettings::default())
        .expect("DejaVuSansMono.ttf must parse as a font")
});

/// The battle ledger builds its columns by padding strings to a cell width,
/// which only lines up if every glyph advances the same distance. Asserted
/// here rather than assumed, and headlessly for the same reason the unscii
/// test is: fontdue is the rasterizer macroquad uses.
#[test]
fn the_ui_font_advances_every_glyph_equally() {
    let size = 16.0;
    let reference = UI.metrics('M', size).advance_width;
    for ch in "iIlW1 .0/…ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
        .chars()
    {
        let advance = UI.metrics(ch, size).advance_width;
        assert!(
            (advance - reference).abs() < 0.01,
            "{ch:?} advances {advance}, not {reference} — the UI font is not monospace, \
             so padded columns cannot line up"
        );
    }
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p feral-processes-gui --test font_rasterization`
Expected: PASS. If the `…` assertion fails the font lacks that glyph — switch
the truncation marker in Task 2 to `~` and note it in the spec.

- [ ] **Step 3: Commit**

```bash
git add crates/gui/tests/font_rasterization.rs
git commit -m "test: pin the UI font's monospace advance"
```

---

### Task 2: The `cell` helper

**Files:** Modify `crates/gui/src/render.rs`

**Interfaces:**
- Produces: `fn cell(s: &str, width: usize) -> String` — always exactly
  `width` chars. Task 3 formats every column through it.

- [ ] **Step 1: Write the failing tests**

In `render.rs`'s existing `mod tests`:

```rust
#[test]
fn cell_pads_short_content_to_exactly_the_column_width() {
    assert_eq!(cell("You", 8), "You     ");
    assert_eq!(cell("", 3), "   ");
    assert_eq!(cell("exact", 5), "exact");
}

/// A name longer than its column has to lose its tail. Letting it through
/// would push every column after it right and defeat the whole point.
#[test]
fn cell_truncates_over_width_content_and_marks_it() {
    let out = cell("4 Corrupted Null Daemons", 12);
    assert_eq!(out.chars().count(), 12);
    assert!(out.ends_with('…'), "a clipped cell has to show it was clipped");
}

/// Counted in chars, not bytes: a multi-byte name must not slice mid-glyph
/// or blow the width out.
#[test]
fn cell_counts_characters_not_bytes() {
    assert_eq!(cell("Ünïcödé", 7).chars().count(), 7);
    assert_eq!(cell("Ünïcödé", 4).chars().count(), 4);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-gui cell_`
Expected: FAIL — `cannot find function \`cell\``.

- [ ] **Step 3: Implement**

Beside `draw_battle`:

```rust
/// Pads `s` to exactly `width` monospace cells, truncating with `…` when it
/// overruns. Exactness is the contract: the header and every row are built
/// from these, so a cell that comes out the wrong width shifts every column
/// after it. Chars, not bytes — a name can hold multi-byte glyphs.
fn cell(s: &str, width: usize) -> String {
    if s.chars().count() > width {
        s.chars()
            .take(width.saturating_sub(1))
            .chain(std::iter::once('…'))
            .collect()
    } else {
        format!("{s:<width$}")
    }
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p feral-processes-gui cell_`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/gui/src/render.rs
git commit -m "feat: add a fixed-width cell formatter for the battle ledger"
```

---

### Task 3: Row and header formatting

**Files:** Modify `crates/gui/src/render.rs`

**Interfaces:**
- Consumes: `cell` from Task 2.
- Produces: `ROSTER_HEADER_HOSTILE`/`ROSTER_HEADER_PARTY` (`String`-returning
  fns or consts) and `fn roster_row(mark, name, hp, atk, def, reach, tail) -> String`.
  Task 4 calls these from `draw_battle`.

- [ ] **Step 1: Write the failing tests**

```rust
/// Header and rows are built from the same widths, so every line in a
/// roster block is the same length and every column starts at the same
/// offset. That is the entire ledger effect; assert it rather than trusting
/// the format strings to stay in step.
#[test]
fn every_roster_line_shares_the_headers_column_offsets() {
    let header = party_header();
    let row = roster_row(">1 ", "You", "21/30", 11, 6, "FRONT", "Attack A");
    assert_eq!(
        row.chars().count().min(header.chars().count()),
        header.chars().count().min(row.chars().count())
    );
    // The tail column is ragged by design; everything before it is not.
    let fixed = MARK_W + NAME_W + 1 + HP_W + 1 + STAT_W + 1 + STAT_W + 1 + REACH_W + 1;
    assert_eq!(
        header.chars().take(fixed).count(),
        row.chars().take(fixed).count()
    );
    for (col, label) in [(MARK_W, "NAME"), (MARK_W + NAME_W + 1, "HP")] {
        assert!(
            header[col_byte(&header, col)..].starts_with(label),
            "{label} must start at column {col}"
        );
    }
    let _ = row;
}

/// An over-long name is clipped, not allowed to shove the stats right.
#[test]
fn a_long_name_does_not_shift_the_columns_after_it() {
    let short = roster_row("A  ", "Glitch", "8/8", 3, 1, "ENGAGED", "OK");
    let long = roster_row("A  ", "4 Corrupted Null Daemons of Yendor", "8/8", 3, 1, "ENGAGED", "OK");
    let upto_tail = MARK_W + NAME_W + 1 + HP_W + 1 + STAT_W + 1 + STAT_W + 1 + REACH_W;
    assert_eq!(
        short.chars().take(upto_tail).collect::<String>().len(),
        long.chars().take(upto_tail).collect::<String>().len()
    );
    assert!(long.contains('…'), "the long name has to be visibly clipped");
}

/// Numbers right-align so a column of them can be compared by scanning.
#[test]
fn stat_columns_are_right_aligned() {
    let row = roster_row("A  ", "Glitch", "8/8", 3, 1, "ENGAGED", "OK");
    assert!(row.contains("  3   1 "), "single digits pad left, got {row:?}");
}
```

`col_byte` is a test helper converting a char offset to a byte offset:

```rust
fn col_byte(s: &str, chars: usize) -> usize {
    s.char_indices().nth(chars).map(|(i, _)| i).unwrap_or(s.len())
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-gui roster`
Expected: FAIL — the functions and width constants do not exist.

- [ ] **Step 3: Implement**

```rust
/// Column widths for both battle rosters. They live here rather than in
/// `app-core` because there is one renderer to serve; see the spec.
const MARK_W: usize = 3;
const NAME_W: usize = 18;
/// `hp/max` as one cell, so the header can sit over the pair.
const HP_W: usize = 11;
const STAT_W: usize = 3;
/// Widest value is `ENGAGED`.
const REACH_W: usize = 7;

fn party_header() -> String {
    roster_row_raw("   ", "NAME", "HP", "ATK", "DEF", "POS", "ACTION")
}

fn hostile_header() -> String {
    roster_row_raw("   ", "GROUP", "HP", "ATK", "DEF", "RANGE", "STATUS")
}

/// The one row shape both rosters and both headers are built from — so a
/// column cannot move in a row without moving in its header too.
fn roster_row_raw(
    mark: &str,
    name: &str,
    hp: &str,
    atk: &str,
    def: &str,
    reach: &str,
    tail: &str,
) -> String {
    format!(
        "{}{} {} {:>STAT_W$} {:>STAT_W$} {} {}",
        cell(mark, MARK_W),
        cell(name, NAME_W),
        cell(hp, HP_W),
        atk,
        def,
        cell(reach, REACH_W),
        tail,
    )
}

fn roster_row(
    mark: &str,
    name: &str,
    hp: &str,
    atk: i32,
    def: i32,
    reach: &str,
    tail: &str,
) -> String {
    roster_row_raw(
        mark,
        name,
        hp,
        &atk.to_string(),
        &def.to_string(),
        reach,
        tail,
    )
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p feral-processes-gui roster`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/gui/src/render.rs
git commit -m "feat: format battle roster rows and headers from shared widths"
```

---

### Task 4: Wire the rosters up, and stop `draw_bar` appending HP

**Files:** Modify `crates/gui/src/render.rs`

**Interfaces:**
- Consumes: `roster_row`, `hostile_header`, `party_header` from Task 3.

- [ ] **Step 1: Make `draw_bar` draw the label verbatim**

In `draw_bar`, replace

```rust
    let text = format!("{label} {value:.0}/{max:.0}");
```

with

```rust
    // The label is drawn as given. The battle ledger needs HP in a column of
    // its own rather than tacked onto the end, which an append here would
    // make impossible.
    let text = label;
```

and adjust the two `fonts.ui*` calls to take `text` (a `&str`).

- [ ] **Step 2: Fix the three status-panel callers**

`render.rs:543`/`556`/`569` currently pass `"Integrity"`, `"Power"`,
`"Fatigue"` and rely on the append. Each becomes its own format, e.g.

```rust
        &format!("Integrity {:.0}/{:.0}", status.hp, status.max_hp.max(1)),
```

with `Power`/`Fatigue` using `status.hunger`/`status.fatigue` over `100.0`,
matching what the append produced.

- [ ] **Step 3: Draw the hostile header and rows**

After the `"Hostile programs — round {}"` title line, draw
`hostile_header()` with `fonts.ui(..., m.label(), TEXT)` and advance `y` by
`m.line_height`. Replace the per-group `format!` with:

```rust
            &roster_row(
                &format!("{}  ", g.letter),
                &format!("{}{}", name, if g.is_boss { " [BOSS]" } else { "" }),
                &format!("{}/{}", g.front_hp, g.front_max_hp),
                g.atk,
                g.def,
                if g.engaged { "ENGAGED" } else { "BACK" },
                &g.status_effect
                    .as_deref()
                    .map(|s| s.to_uppercase())
                    .unwrap_or_else(|| "OK".to_string()),
            ),
```

- [ ] **Step 4: Draw the party header and rows**

Same shape after the `"Your party — DECOMP {}"` title:

```rust
            &roster_row(
                &format!("{}{} ", if active { ">" } else { " " }, p.slot + 1),
                &p.name,
                &format!("{}/{}", p.hp, p.max_hp),
                p.atk,
                p.def,
                if p.front { "FRONT" } else { "BACK" },
                &p.planned.clone().unwrap_or_else(|| "—".to_string()),
            ),
```

The `status_tag` suffix leaves the party row: a companion's condition now
belongs in a column, appended to the action tail as `— BLEEDING (2)` only if
it has one. Keep `status_tag` if other screens still call it; delete it if
this was its last caller.

- [ ] **Step 5: Account for the header rows in the party block's height**

`bar_row_height` feeds `party_height`, which bottom-anchors the party block.
The party header is one extra `m.line_height`, so `party_height` becomes

```rust
    let party_height =
        m.line_height * 2.0 + view.party.len() as f32 * bar_row_height(m) + m.inset;
```

Getting this wrong shifts the whole block; it is the one line in this task
that is not local to a single row.

- [ ] **Step 6: Build, test, lint**

Run: `cargo build --workspace` — expected: clean.
Run: `cargo test --workspace` — expected: 440 plus the new tests, none failing.
Run: `cargo clippy --workspace --all-targets` — expected: no warnings.
Run: `cargo fmt`

- [ ] **Step 7: Commit**

```bash
git add crates/gui/src/render.rs
git commit -m "feat: draw the battle rosters as a headed ledger"
```

---

## Self-review against the spec

| Spec section | Covered by |
|---|---|
| Layout: four blocks, two lines per row, header per roster | Task 4 Steps 3-5 |
| Columns table (mark/name/hp/atk/def/reach/tail) | Task 3 Step 3 |
| Right-aligned numerics | Task 3 Step 3, asserted Step 1 |
| `ENGAGED`/`BACK`, `FRONT`/`BACK`, `OK` | Task 4 Steps 3-4 |
| `[BOSS]` marker kept on the name | Task 4 Step 3 |
| Truncation with `…` | Task 2 |
| Widths as `render.rs` constants | Task 3 Step 3 |
| `draw_bar` stops appending | Task 4 Steps 1-2 |
| Verification: monospace font | Task 1 |
| Verification: row/header offsets, padding, truncation | Tasks 2-3 |
| Not doing: group numbering, roster padding, pickers, wording | no task — deliberate |

One gap accepted: the spec notes the party block can still overrun the action
bar on a short window, and the two header rows make that marginally more
likely. No task addresses it, by design — it needs a layout decision about
what gives way.
