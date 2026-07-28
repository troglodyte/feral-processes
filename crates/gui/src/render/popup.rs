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
    },
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
    }
}

/// `item_row` for a list of creatures — see `Row::Item::bold`.
pub(super) fn creature_row(s: impl Into<String>, selected: bool) -> Row {
    Row::Item {
        text: s.into(),
        selected,
        bold: true,
        color: TEXT,
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
fn draw_row(row: &Row, x: f32, w: f32, cy: f32, max_y: f32, painter: &Painter, m: &Metrics) -> f32 {
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
            let label = format!("{prefix}{s}");
            if *selected && *bold {
                painter.ui_bold(label, x + m.pad, cy, m.font_size, *color);
            } else {
                painter.ui(label, x + m.pad, cy, m.font_size, *color);
            }
        }
    }
    cy + m.line_height
}

#[cfg(test)]
mod tests {
    use super::*;

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
