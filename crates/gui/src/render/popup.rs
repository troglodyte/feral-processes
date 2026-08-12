//! The bordered popup box every menu screen is drawn into: its rows,
//! its sizing, and the scroll window that keeps the selected row visible.

use super::*;

/// One line of a popup's body. `Item` rows are the numbered/lettered
/// options a menu key press resolves to (see `App::selected_index`);
/// `Text` rows are just informational.
pub(super) enum Row {
    Text(String),
    TextColored(String, Color),
    Item {
        text: String,
        selected: bool,
        /// Draws the row in the bold face when selected. Reserved for lists
        /// where the row is a *creature you are addressing* rather than a
        /// command you are picking — see `draw_battle_target_menu`.
        bold: bool,
        color: Color,
        /// A trailing annotation drawn dim, set apart from the row's own text
        /// — the history screen's repeat count. See `counted_item_row`.
        suffix: Option<String>,
        /// The map glyph of whatever this row stands for, in its own colour.
        /// Set by `with_icon` rather than by the seven row constructors, so
        /// only the lists that name an entity pay for it.
        ///
        /// Its colour is deliberately a *second* axis from the row's `color`:
        /// that one already means fusion tier, CRITICAL HP or idleness
        /// depending on the screen, and the icon has to keep meaning what it
        /// means on the map or it isn't the same icon.
        icon: Option<(char, Color)>,
    },
}

/// What an icon occupies in a row's label: the glyph itself plus the gap
/// before the text. Held as a string of spaces because `draw_row` reserves the
/// slot inside the label and paints the glyph over it — see there.
const ICON_SLOT: &str = "   ";

/// Gives an `Item` row the icon of the thing it stands for. A combinator
/// rather than a parameter on all seven row constructors: most rows are not
/// an entity, and threading `None` through every caller would be the cost of
/// the field paid by the screens that don't use it.
pub(super) fn with_icon(row: Row, glyph: char, color: Color) -> Row {
    match row {
        Row::Item {
            text,
            selected,
            bold,
            color: text_color,
            suffix,
            ..
        } => Row::Item {
            text,
            selected,
            bold,
            color: text_color,
            suffix,
            icon: Some((glyph, color)),
        },
        // Nothing calls this on a Text row; returning it unchanged is a
        // cheaper answer than a panic for a case the type can't rule out.
        other => other,
    }
}

pub(super) fn text_row(s: impl Into<String>) -> Row {
    Row::Text(s.into())
}

pub(super) fn item_row(s: impl Into<String>, selected: bool) -> Row {
    Row::Item {
        text: s.into(),
        selected,
        bold: false,
        color: TEXT,
        suffix: None,
        icon: None,
    }
}

/// `item_row` for something still listed but no longer worth picking — a
/// research node already unlocked. Stays selectable, since the list is
/// navigated past it, but reads as spent.
pub(super) fn spent_item_row(s: impl Into<String>, selected: bool) -> Row {
    Row::Item {
        text: s.into(),
        selected,
        bold: false,
        color: TEXT_DIM,
        suffix: None,
        icon: None,
    }
}

/// `item_row` for a program close enough to 0 HP that another fight could
/// delete it for good — see `hp_critical`. Callers pair it with a CRITICAL
/// tag in the row text, so the warning still reads without colour.
pub(super) fn critical_item_row(s: impl Into<String>, selected: bool) -> Row {
    Row::Item {
        text: s.into(),
        selected,
        bold: false,
        color: RED,
        suffix: None,
        icon: None,
    }
}

/// `item_row` in a caller-chosen colour, for a list whose rows carry their
/// own meaning rather than a menu's uniform one — a log line's
/// `MessageKind`, a structure standing idle.
pub(super) fn colored_item_row(s: impl Into<String>, selected: bool, color: Color) -> Row {
    Row::Item {
        text: s.into(),
        selected,
        bold: false,
        color,
        suffix: None,
        icon: None,
    }
}

/// `item_row` for a program or a piece of gear, coloured by how many times
/// it has been fused — see `fusion_color`. Every menu that lists either
/// goes through here, so none of them can grow its own idea of what the
/// colours mean. A caller with a louder rule of its own checks that first
/// and only falls through to this (see `draw_companion_menu`'s CRITICAL).
pub(super) fn fusion_row(s: impl Into<String>, selected: bool, fusions: u32) -> Row {
    match fusion_color(fusions) {
        Some(color) => colored_item_row(s, selected, color),
        None => item_row(s, selected),
    }
}

/// `fusion_row` for a *program*, which unlike a piece of gear can also carry
/// a rare-spawn tier. `program_color` is what decides between the two when
/// a program has both; this is only the row-building half, and the same
/// "one function so no menu grows its own idea" rule applies.
pub(super) fn program_row(
    s: impl Into<String>,
    selected: bool,
    fusions: u32,
    rarity: Rarity,
) -> Row {
    match program_color(fusions, rarity) {
        Some(color) => colored_item_row(s, selected, color),
        None => item_row(s, selected),
    }
}

/// `colored_item_row` for a row standing for `repeats` identical log lines —
/// the history screen's folded rows (see `Game::message_history`). The count
/// is drawn dim and set apart, so it reads as an annotation rather than as
/// part of the line, and a row standing for one line carries nothing at all.
pub(super) fn counted_item_row(
    s: impl Into<String>,
    repeats: usize,
    selected: bool,
    color: Color,
) -> Row {
    Row::Item {
        text: s.into(),
        selected,
        bold: false,
        color,
        suffix: (repeats > 1).then(|| format!("×{repeats}")),
        icon: None,
    }
}

/// `item_row` for a list of creatures — see `Row::Item::bold`.
pub(super) fn creature_row(s: impl Into<String>, selected: bool) -> Row {
    Row::Item {
        text: s.into(),
        selected,
        bold: true,
        color: TEXT,
        suffix: None,
        icon: None,
    }
}

/// How much of the window a popup claims. Height always shrinks to fit
/// short content regardless of size (see `draw_popup`), so this really
/// only controls width in practice — `Small` exists for the handful of
/// one-line prompts that would otherwise be a lot of empty box around a
/// single sentence.
#[derive(Clone, Copy)]
pub(super) enum PopupSize {
    /// Every list/detail menu with real content — deploy/compile/trade/
    /// inventory/party/etc. Sized to leave long rows room rather than
    /// running off the popup's edge, and to give scrollable lists (see
    /// `draw_popup`) more rows on screen before they need to scroll at all.
    Large,
    /// A short, single-purpose prompt with nothing to clip: a direction
    /// picker, a "that program is gone" message.
    Small,
}

/// Centered popup, sized as a percentage of the window — same idea as
/// `ui.rs`'s `centered_rect`, just in pixels instead of terminal cells.
///
/// `rows` is split around its first/last `Row::Item`: everything before is
/// a pinned header (the prompt line), everything after is a pinned footer
/// (e.g. "Esc to cancel"), and the `Item` span in between is the
/// scrollable body. Long lists (more structures/pets/etc. than fit the
/// popup) auto-scroll to keep the highlighted row in view instead of
/// silently running off the bottom with no way to see or reach it.
/// The vertical layout of a popup: how tall the box is, how its rows split
/// into a pinned header, a scrollable body and a pinned footer, and which
/// slice of that body is on screen.
struct PopupLayout<'a> {
    h: f32,
    header: &'a [Row],
    body: &'a [Row],
    footer: &'a [Row],
    /// Index of the first body row on screen.
    offset: usize,
    /// How many body rows are on screen.
    capacity: usize,
    /// Whether a line above and below the body is reserved for the
    /// "N more above/below" indicators.
    scrolling: bool,
}

/// A popup never shrinks below this many content rows, so a one-line prompt
/// still reads as a box rather than a strip.
const MIN_POPUP_ROWS: usize = 2;

/// Sizes a popup and works out its visible body slice. Split out of
/// `draw_popup` and kept free of macroquad so it can be tested headlessly —
/// `screen_h` is the window height, which is all the sizing needs.
///
/// What fits is settled in whole rows before any of it becomes pixels, and
/// the height follows from that count. Sizing the box to its content and
/// then re-deriving the row budget from that height is the same arithmetic
/// run backwards, and it doesn't survive f32: at most window heights the
/// quotient landed a hair under a whole line, so a list the box had room for
/// lost a row, which turned scrolling on, which spent two more rows on the
/// indicators — three rows hidden below a list with blank space under it.
fn popup_layout<'a>(screen_h: f32, pct_h: f32, rows: &'a [Row], m: &Metrics) -> PopupLayout<'a> {
    // Two lines for the title and its divider, plus the bottom inset.
    let chrome = m.line_height * 2.0 + m.inset;
    let max_rows = ((screen_h * pct_h - chrome) / m.line_height)
        .floor()
        .max(0.0) as usize;
    let rows_shown = rows.len().min(max_rows).max(MIN_POPUP_ROWS);
    let h = rows_shown as f32 * m.line_height + chrome;

    let first_item = rows.iter().position(|r| matches!(r, Row::Item { .. }));
    let last_item = rows.iter().rposition(|r| matches!(r, Row::Item { .. }));
    let (header, body, footer): (&[Row], &[Row], &[Row]) = match (first_item, last_item) {
        (Some(first), Some(last)) => (&rows[..first], &rows[first..=last], &rows[last + 1..]),
        _ => (rows, &[], &[]),
    };

    let raw_capacity = rows_shown.saturating_sub(header.len() + footer.len());
    let scrolling = body.len() > raw_capacity;
    // Scrolling reserves one line above and below for "N more" indicators,
    // so the item rows themselves never get a partial cut-off line.
    let capacity = if scrolling {
        raw_capacity.saturating_sub(2).max(1)
    } else {
        raw_capacity
    };

    let selected_idx = body
        .iter()
        .position(|r| matches!(r, Row::Item { selected: true, .. }))
        .unwrap_or(0);
    let offset = if body.len() <= capacity {
        0
    } else {
        let max_offset = body.len() - capacity;
        selected_idx.saturating_sub(capacity / 2).min(max_offset)
    };

    PopupLayout {
        h,
        header,
        body,
        footer,
        offset,
        capacity,
        scrolling,
    }
}

pub(super) fn draw_popup(
    title: &str,
    size: PopupSize,
    rows: &[Row],
    painter: &Painter,
    m: &Metrics,
) {
    let (pct_w, pct_h) = match size {
        PopupSize::Large => (0.88, 0.85),
        PopupSize::Small => (0.5, 0.85),
    };
    let layout = popup_layout(painter.screen_h(), pct_h, rows, m);
    let w = painter.screen_w() * pct_w;
    let h = layout.h;
    let x = (painter.screen_w() - w) / 2.0;
    let y = (painter.screen_h() - h) / 2.0;

    painter.rect(x, y, w, h, PANEL_BG);
    painter.rect_lines(x, y, w, h, 2.0, BORDER);
    painter.ui(
        title,
        x + m.font_size as f32 / 2.0,
        y + m.font_size as f32,
        m.title(),
        CYAN,
    );
    // Sits below the title's own size rather than a fixed offset, so a
    // larger font pushes the rule down instead of striking through it.
    let divider_y = y + m.title() as f32 + m.gap;
    let divider_inset = m.pad / 2.0;
    painter.line(
        x + divider_inset,
        divider_y,
        x + w - divider_inset,
        divider_y,
        1.0,
        BORDER,
    );

    let mut cy = y + m.line_height * 2.0;
    let max_y = y + h - m.inset;
    for row in layout.header {
        cy = draw_row(row, x, w, cy, max_y, painter, m);
    }

    if !layout.body.is_empty() {
        if layout.scrolling {
            let text = if layout.offset > 0 {
                format!("↑ {} more above", layout.offset)
            } else {
                String::new()
            };
            painter.ui(&text, x + m.pad, cy, m.small(), TEXT_DIM);
            cy += m.line_height;
        }

        let visible_end = (layout.offset + layout.capacity).min(layout.body.len());
        for row in &layout.body[layout.offset..visible_end] {
            cy = draw_row(row, x, w, cy, max_y, painter, m);
        }

        if layout.scrolling {
            let below = layout.body.len() - visible_end;
            let text = if below > 0 {
                format!("↓ {below} more below")
            } else {
                String::new()
            };
            painter.ui(&text, x + m.pad, cy, m.small(), TEXT_DIM);
            cy += m.line_height;
        }
    }

    for row in layout.footer {
        cy = draw_row(row, x, w, cy, max_y, painter, m);
    }
}

/// Draws one popup row and returns the y coordinate for the next one.
/// `max_y` is a last-resort safety clamp — normal layout keeps every row
/// within bounds via `draw_popup`'s capacity accounting, so this only ever
/// bites if that accounting is off by a line.
///
/// `pub(super)` rather than private: the active-buff panel (`render/field.rs`)
/// draws the same `Row::Item` shape outside a popup's box — an ambient list,
/// not a modal menu — and reuses this rather than a second copy of the
/// suffix-placement arithmetic.
/// Where a row's suffix starts: one inset past the right edge of the row's
/// own drawn text, given the row's left edge `row_x`.
///
/// Split out of `draw_row` so the advance-versus-ink distinction is held by
/// a test rather than by a comment — measuring `label`'s ink box here reads
/// as harmless and silently drops the suffix on top of the row's tail,
/// because every `Row::Item` label opens with a two-space prefix that has no
/// ink of its own. See `a_rows_suffix_clears_the_text_it_follows`.
fn suffix_x(label: &str, row_x: f32, painter: &Painter, m: &Metrics) -> f32 {
    row_x + m.pad + painter.measure_ui_advance(label, m.font_size) + m.inset
}

/// The whole of what `draw_row` hands to the painter for an `Item`: the
/// selection caret, the icon's reserved slot, then the row's own text.
///
/// Split out for the same reason `suffix_x` was — the slot is the one thing
/// keeping `suffix_x` honest on an icon row, and a test can say so where a
/// comment can only claim it. A row with no icon reserves nothing, so the
/// screens that never had an icon are drawn exactly where they always were.
fn row_label(prefix: &str, icon: Option<(char, Color)>, text: &str) -> String {
    match icon {
        Some(_) => format!("{prefix}{ICON_SLOT}{text}"),
        None => format!("{prefix}{text}"),
    }
}

pub(super) fn draw_row(
    row: &Row,
    x: f32,
    w: f32,
    cy: f32,
    max_y: f32,
    painter: &Painter,
    m: &Metrics,
) -> f32 {
    if cy > max_y {
        return cy;
    }
    match row {
        Row::Text(s) => {
            painter.ui(s, x + m.pad, cy, m.font_size, TEXT_DIM);
        }
        Row::TextColored(s, color) => {
            painter.ui(s, x + m.pad, cy, m.font_size, *color);
        }
        Row::Item {
            text: s,
            selected,
            bold,
            color,
            suffix,
            icon,
        } => {
            if *selected {
                // Anchored to the same `m.pad` the row text uses, so the
                // highlight keeps leading its text by one inset at every
                // font size instead of drifting left as the text grows.
                let bleed = m.pad - m.inset;
                painter.rect(
                    x + bleed,
                    cy - m.font_size as f32,
                    w - bleed * 2.0,
                    m.line_height,
                    SELECT_BG,
                );
            }
            let prefix = if *selected { "> " } else { "  " };
            // The icon's slot is spaces inside the label, with the glyph drawn
            // over it afterwards in its own colour. Reserving it this way
            // rather than measuring segments and summing them keeps every
            // row's text starting at the same x — the UI face is monospace, so
            // a space and a glyph take the same advance — and leaves
            // `suffix_x` measuring one string, as it already documents.
            let label = row_label(prefix, *icon, s);
            if *selected && *bold {
                painter.ui_bold(&label, x + m.pad, cy, m.font_size, *color);
            } else {
                painter.ui(&label, x + m.pad, cy, m.font_size, *color);
            }
            // Placed by measuring the row's own text rather than padded into
            // it, so the gap is one inset at every font size instead of a
            // count of spaces that drifts with the glyph width. Never bold, so
            // measuring the regular face is right for every row that has one:
            // `bold` is a creature list's, and those carry no suffix.
            //
            // Measured by *advance*, not by `measure_ui`'s ink box: `label`
            // always opens with a two-space prefix, which has no ink, so the
            // ink box would report a width that starts two characters into
            // the row and drop the suffix on top of the row's own tail.
            if let Some((glyph, glyph_color)) = icon {
                painter.ui(
                    glyph.to_string(),
                    x + m.pad + painter.measure_ui_advance(prefix, m.font_size),
                    cy,
                    m.font_size,
                    *glyph_color,
                );
            }
            if let Some(suffix) = suffix {
                painter.ui(
                    suffix,
                    suffix_x(&label, x, painter, m),
                    cy,
                    m.font_size,
                    TEXT_DIM,
                );
            }
        }
    }
    cy + m.line_height
}

/// How wide a popup lets prose run before wrapping. Deliberately
/// conservative rather than derived from the popup's pixel width, which is
/// a percentage of the window and so varies per machine — the longest
/// description any shipped item carries is about 165 characters, which lands
/// in three rows here with room to spare on the narrowest supported window.
///
/// One constant beside `wrap_text` rather than one per screen: the three
/// screens that print prose have no reason to disagree about the width, and
/// the pair that preceded this were kept in step by a doc comment.
pub(super) const DESCRIBE_WRAP_COLUMNS: usize = 72;

/// Greedy word wrap to `columns`, for prose too long to sit on one popup
/// row — an item's authored description, chiefly.
///
/// A word longer than `columns` is emitted whole on its own line rather
/// than split or dropped: one row running wide is a smaller problem than
/// losing text, and the alternative is hyphenating identifiers.
pub(super) fn wrap_text(text: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.is_empty() {
            line.push_str(word);
        } else if line.chars().count() + 1 + word.chars().count() <= columns {
            line.push(' ');
            line.push_str(word);
        } else {
            lines.push(std::mem::take(&mut line));
            line.push_str(word);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_color(row: &Row) -> Option<Color> {
        match row {
            Row::Item { color, .. } => Some(*color),
            _ => None,
        }
    }

    /// Eleven menus list a program or a piece of gear, and all of them pick
    /// their row this way rather than repeating the match — a screen that
    /// grew its own copy is how one list ends up disagreeing with the rest
    /// about what magenta means.
    #[test]
    fn fusion_row_colours_by_depth_and_leaves_an_unfused_row_plain() {
        assert_eq!(row_color(&fusion_row("x", false, 0)), Some(TEXT));
        assert_eq!(row_color(&fusion_row("x", false, 1)), Some(CYAN));
        assert_eq!(
            row_color(&fusion_row("x", false, MAX_FUSIONS)),
            Some(MAGENTA)
        );
    }

    /// The bug this guards: `measure_ui` reports the *ink* box, which begins
    /// at the first visible glyph. Every `Row::Item` label opens with a
    /// two-space prefix, so measuring ink here put the suffix roughly two
    /// characters back, printing a buff's tick countdown on top of its own
    /// magnitude in the map's Routines panel.
    #[test]
    fn a_rows_suffix_clears_the_text_it_follows() {
        let m = Metrics {
            font_size: 16,
            line_height: 20.0,
            pad: 8.0,
            inset: 4.0,
            gap: 6.0,
        };
        crate::paint::with_painter(|p| {
            let label = "  Coolant Flush       FTG+4";
            let row_x = 100.0;
            let text_right = row_x + m.pad + p.measure_ui_advance(label, m.font_size);
            assert!(
                suffix_x(label, row_x, p, &m) >= text_right,
                "the suffix must start past the row text, not inside it"
            );
        });
    }

    /// The icon is painted *over* a slot reserved inside the label rather than
    /// pushing the text along, so everything downstream that measures the row
    /// — `suffix_x`, chiefly — sees the space the glyph occupies. Measuring
    /// the un-slotted text and drawing the glyph before it would have put the
    /// icon on top of the selection caret and the suffix two characters early.
    #[test]
    fn an_icon_reserves_its_slot_inside_the_row_label() {
        let plain = row_label("  ", None, "Drone Lv3");
        let iconed = row_label("  ", Some(('o', TEXT)), "Drone Lv3");
        assert_eq!(plain, "  Drone Lv3");
        assert_eq!(
            iconed.len(),
            plain.len() + ICON_SLOT.len(),
            "the slot is reserved, not merely implied: {iconed:?}"
        );
        assert!(
            iconed.ends_with("Drone Lv3") && iconed.starts_with("  "),
            "the caret still leads and the text still trails: {iconed:?}"
        );
    }

    /// The icon's colour is a second axis. A party row's own colour already
    /// means fusion tier or CRITICAL HP, and the icon has to keep meaning what
    /// it means on the map, so neither may overwrite the other.
    #[test]
    fn an_icon_leaves_the_rows_own_colour_and_selection_alone() {
        let row = with_icon(critical_item_row("Glitch Lv2", true), 'o', GREEN);
        let Row::Item {
            selected,
            color,
            icon,
            ..
        } = row
        else {
            panic!("with_icon must return the Item it was given");
        };
        assert!(selected);
        assert_eq!(color, RED, "CRITICAL still owns the row text");
        assert_eq!(icon, Some(('o', GREEN)), "the glyph still owns its colour");
    }

    #[test]
    fn wrapping_breaks_on_spaces_and_never_exceeds_the_column_budget() {
        let text = "A fragment of stolen authorization, cut for locks the Stack \
                    no longer uses.";
        let lines = wrap_text(text, 30);
        assert!(
            lines.iter().all(|l| l.chars().count() <= 30),
            "no wrapped line may run past the budget: {lines:?}"
        );
        assert_eq!(
            lines.join(" "),
            text,
            "wrapping must preserve the words and their order exactly"
        );
    }

    #[test]
    fn a_short_description_stays_on_one_line() {
        assert_eq!(
            wrap_text("Standard armor. Solid protection.", 72),
            vec!["Standard armor. Solid protection."]
        );
    }

    /// A single word longer than the budget has nowhere to break, so it gets
    /// its own overlong line rather than being silently truncated or
    /// dropped. Losing text is worse than one row running wide.
    #[test]
    fn an_unbreakable_word_gets_its_own_line_rather_than_being_lost() {
        let lines = wrap_text("tiny supercalifragilisticexpialidocious end", 10);
        assert!(lines.contains(&"supercalifragilisticexpialidocious".to_string()));
        assert_eq!(
            lines.join(" "),
            "tiny supercalifragilisticexpialidocious end"
        );
    }

    /// Window heights worth sizing a popup against: the default window, the
    /// common desktop resolutions, and both ends of the UI font's clamp. The
    /// popup's row budget is font-dependent, so a bug that misses by one line
    /// can hide at one height and bite at the next.
    const WINDOW_HEIGHTS: [f32; 9] = [
        720.0, 768.0, 800.0, 900.0, 1000.0, 1050.0, 1080.0, 1200.0, 1440.0,
    ];

    /// `draw_inventory`'s row shape for an `items`-long inventory with row
    /// `selected` highlighted: three pinned header rows, a body of the three
    /// equipment slots plus two label rows plus one row per item, and two
    /// pinned footer rows.
    fn inventory_rows(items: usize, selected: usize) -> Vec<Row> {
        let mut rows = vec![text_row(""), text_row(""), text_row("")];
        rows.extend((0..3).map(|slot| item_row("", selected == slot)));
        rows.push(text_row(""));
        rows.push(text_row(""));
        rows.extend((0..items).map(|i| item_row("", selected == i + 3)));
        rows.push(text_row(""));
        rows.push(text_row(""));
        rows
    }

    /// The real build-menu rows, for `structures` entries spread across two
    /// categories so the heading rows are exercised too. Built through
    /// `build_menu_rows` rather than restated here — a hand-written mirror
    /// of the row shape would keep passing after the real one drifted.
    fn build_menu_rows_fixture(structures: usize, selected: usize) -> Vec<Row> {
        use super::super::building::{BuildEntry, build_menu_rows};
        use feral_processes_engine::structures::StructureCategory;
        let entries: Vec<BuildEntry> = (0..structures)
            .map(|i| BuildEntry {
                label: format!("Structure {i} - 12 Core Fragments"),
                description: format!("What structure {i} is for."),
                category: if i == 0 {
                    StructureCategory::Home
                } else if i < structures.div_ceil(2) {
                    StructureCategory::Extractor
                } else {
                    StructureCategory::Assembler
                },
            })
            .collect();
        build_menu_rows(&entries, selected)
    }

    /// The body is cut at the *last* `Row::Item` and everything after it is
    /// pinned as a footer. When the build menu's descriptions were
    /// `Row::Text`, the final structure's description fell past that cut: it
    /// was drawn detached at the bottom of the popup, under the scroll
    /// indicator, while every other description sat inline under its own
    /// structure. It read as missing.
    ///
    /// Nothing may follow the last item row.
    #[test]
    fn every_build_menu_description_stays_inside_the_scrollable_body() {
        for window_h in WINDOW_HEIGHTS {
            let m = ui_metrics(window_h);
            for structures in 1..14 {
                for selected in [0, structures - 1] {
                    let rows = build_menu_rows_fixture(structures, selected);
                    let l = popup_layout(window_h, 0.85, &rows, &m);
                    assert!(
                        l.footer.is_empty(),
                        "at {window_h}px a {structures}-structure build menu pinned {} \
                         row(s) as a footer — the last description is detached from \
                         the structure it describes",
                        l.footer.len()
                    );
                }
            }
        }
    }

    /// The selected structure has to be reachable by scrolling. Headings and
    /// descriptions triple the body's length, so a capacity that only
    /// counted structures would leave the last one unreachable.
    #[test]
    fn the_selected_build_menu_row_is_inside_the_visible_window() {
        for window_h in WINDOW_HEIGHTS {
            let m = ui_metrics(window_h);
            for structures in 1..14 {
                for selected in 0..structures {
                    let rows = build_menu_rows_fixture(structures, selected);
                    let l = popup_layout(window_h, 0.85, &rows, &m);
                    let idx = l
                        .body
                        .iter()
                        .position(|r| matches!(r, Row::Item { selected: true, .. }))
                        .expect("the selected row is in the body");
                    assert!(
                        idx >= l.offset && idx < l.offset + l.capacity,
                        "at {window_h}px, structure {selected} of {structures} sits at \
                         body row {idx}, outside the window [{}, {})",
                        l.offset,
                        l.offset + l.capacity
                    );
                }
            }
        }
    }

    /// A popup shrinks to fit its content, so a list the box has room for
    /// must never be reported as scrolling.
    ///
    /// The height and the row budget used to be derived from each other
    /// through f32 pixel arithmetic. The two cancel algebraically but not in
    /// f32: at most window heights the quotient landed a hair under a whole
    /// line, which flipped scrolling on, which then spent two more lines on
    /// the indicators — hiding three rows of a list that fit with room to
    /// spare, and leaving the space they should have occupied blank.
    #[test]
    fn a_list_the_box_has_room_for_is_shown_whole() {
        for window_h in WINDOW_HEIGHTS {
            let m = ui_metrics(window_h);
            for items in 0..10 {
                let rows = inventory_rows(items, 0);
                let l = popup_layout(window_h, 0.85, &rows, &m);
                let needed = rows.len() as f32 * m.line_height + m.line_height * 2.0 + m.inset;
                assert!(
                    l.h >= needed - 0.5,
                    "at {window_h}px a {items}-item inventory got a {}px box for {needed}px of rows",
                    l.h
                );
                assert!(
                    !l.scrolling && l.capacity >= l.body.len(),
                    "at {window_h}px a {items}-item inventory fits its box but hid {} of its \
                     {} body rows behind a scroll indicator",
                    l.body.len().saturating_sub(l.capacity),
                    l.body.len()
                );
            }
        }
    }

    /// And when the list genuinely doesn't fit, every row stays reachable:
    /// the window follows the highlight down the list rather than letting it
    /// run off the bottom.
    #[test]
    fn a_list_too_long_for_the_box_keeps_the_highlighted_row_on_screen() {
        for window_h in WINDOW_HEIGHTS {
            let m = ui_metrics(window_h);
            let items = 60;
            for selected in 0..3 + items {
                let rows = inventory_rows(items, selected);
                let l = popup_layout(window_h, 0.85, &rows, &m);
                assert!(
                    l.scrolling,
                    "a {items}-item inventory has to scroll at {window_h}px"
                );
                let sel = l
                    .body
                    .iter()
                    .position(|r| matches!(r, Row::Item { selected: true, .. }))
                    .expect("the highlighted row is always in the body");
                assert!(
                    sel >= l.offset && sel < l.offset + l.capacity,
                    "at {window_h}px row {selected} sits at body index {sel}, outside the \
                     visible window [{}, {})",
                    l.offset,
                    l.offset + l.capacity
                );
            }
        }
    }

    /// A scrolling popup draws two indicator lines the non-scrolling one
    /// doesn't. They come out of the same box, so the budget has to include
    /// them or the footer gets pushed through the bottom edge.
    #[test]
    fn a_scrolling_popup_fits_its_indicator_lines_inside_the_box() {
        for window_h in WINDOW_HEIGHTS {
            let m = ui_metrics(window_h);
            let rows = inventory_rows(60, 0);
            let l = popup_layout(window_h, 0.85, &rows, &m);
            let drawn = l.header.len() + 1 + l.capacity + 1 + l.footer.len();
            let needed = drawn as f32 * m.line_height + m.line_height * 2.0 + m.inset;
            assert!(
                needed <= l.h + 0.5,
                "at {window_h}px a scrolling popup draws {drawn} rows needing {needed}px \
                 into a {}px box",
                l.h
            );
        }
    }
}
