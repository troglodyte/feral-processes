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
        /// The category column on a row that names an item — see `ItemTag`.
        /// Set by `with_tag`, for `icon`'s reason: most rows are not an item.
        ///
        /// A *third* axis from `color` and `icon` both, and the one the
        /// column was lifted out of the row text for: it says how well this
        /// copy was compiled, which nothing else on the row can say.
        tag: Option<ItemTag>,
    },
}

/// The category column on a row that names an item — the `WEP` / `ARM` /
/// `MOD` tag `ItemCategory::short_label` hands out, held as its own piece of
/// the row rather than formatted into the middle of `text`.
///
/// Six screens print it, and each used to `format!` it into a row string, so
/// there was no span for a renderer to colour without re-parsing a string it
/// had just built. Held here, the colour and weight are `quality_tag_style`'s
/// and are decided once.
pub(super) struct ItemTag {
    /// The row's text *up to* the column: its shortcut, and the quantity
    /// where the screen prints one.
    ///
    /// It lives on the tag rather than in `text` because the tag sits inside
    /// the label — what precedes it is what places it. Recording an offset
    /// into a joined string instead would leave two representations of one
    /// row to keep in step; these are the pieces, and `item_text` is the
    /// join.
    pub(super) lead: String,
    pub(super) text: String,
    pub(super) color: Color,
    pub(super) bold: bool,
    /// The combat-rating column, between this tag and the row's name — see
    /// `PowerCell`. On the tag rather than beside it because it shares the
    /// tag's whole reason for existing: it is a fixed-width column inside
    /// the label, and what precedes it is what places it.
    pub(super) power: PowerCell,
}

/// The combat-rating column on a row that names an item — `Game::copy_power`
/// rendered into a fixed-width cell.
///
/// **Three cells, three meanings, and they are not interchangeable.** A
/// figure is a rating; an em dash is *no answer* — the item has no combat
/// axis at all, a Decompiler module or a consumable — and a blank is a row
/// that is not an item, the wagon's Routine and Program offers. A dash on
/// one of those would claim the disk had been rated and found wanting.
///
/// Fixed width so the figures form a straight edge down the list, which is
/// the entire feature. It sits *inside* the label and never in
/// `Row::Item::suffix`: `suffix_x` places a suffix one inset past each row's
/// own right edge, so the numbers would stagger with the name lengths above
/// them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PowerCell {
    Blank,
    Unrated,
    Rated(i32),
}

/// Cells wide enough for the shipped range — the strongest armour in the set
/// rates in the low thousands, and a weapon worse than bare fists rates
/// negative. A figure past this grows its own row rather than losing a
/// digit, `qty_column`'s call.
const POWER_COLUMN_WIDTH: usize = 4;

impl PowerCell {
    /// What one carried copy rates. `Unrated` where the engine has no
    /// answer, which is `Game::copy_power`'s own `None`.
    pub(super) fn of_copy(game: &Game, copy: &feral_processes_engine::items::GearCopy) -> Self {
        match game.copy_power(copy) {
            Some(power) => PowerCell::Rated(power.total),
            None => PowerCell::Unrated,
        }
    }

    /// What a *plain* copy of `item` would rate — for the rows that name
    /// something no copy exists of yet: a recipe's result, a trader's shelf.
    pub(super) fn of_item(game: &Game, item: &feral_processes_engine::items::ItemId) -> Self {
        PowerCell::of_copy(
            game,
            &feral_processes_engine::items::GearCopy::plain(item.clone()),
        )
    }

    /// Right-aligned, so the digits line up rather than the signs.
    fn text(self) -> String {
        match self {
            PowerCell::Blank => " ".repeat(POWER_COLUMN_WIDTH),
            PowerCell::Unrated => format!("{:>POWER_COLUMN_WIDTH$}", "\u{2014}"),
            PowerCell::Rated(n) => format!("{n:>POWER_COLUMN_WIDTH$}"),
        }
    }
}

/// What holds a tag apart from the name after it: the two spaces every one of
/// these rows already carried between the two columns.
const TAG_GAP: &str = "  ";

/// Gives an `Item` row its category column. A combinator rather than a
/// parameter on the row constructors, exactly as `with_icon` is, and for the
/// same reason.
///
/// `quality` is the copy's, or `None` where the row names something no copy
/// exists of yet — a recipe's result, a trader's stock. The band it lands in
/// is `quality_tag_style`'s call, so no caller carries a palette.
pub(super) fn with_tag(
    row: Row,
    lead: impl Into<String>,
    text: impl Into<String>,
    quality: Option<u8>,
    power: PowerCell,
) -> Row {
    let (color, bold) = quality_tag_style(quality);
    match row {
        Row::Item {
            text: row_text,
            selected,
            bold: row_bold,
            color: row_color,
            suffix,
            icon,
            ..
        } => Row::Item {
            text: row_text,
            selected,
            bold: row_bold,
            color: row_color,
            suffix,
            icon,
            tag: Some(ItemTag {
                lead: lead.into(),
                text: text.into(),
                color,
                bold,
                power,
            }),
        },
        // Nothing calls this on a Text row — `with_icon`'s call, for its
        // reason.
        other => other,
    }
}

/// Gives an `Item` row its trailing annotation — a combinator, exactly as
/// `with_tag` and `with_icon` are, for the rows whose annotation is decided
/// after the row is built. The caravan basket's per-row amount is that case:
/// what a row is holding is not known to the constructor that names it.
pub(super) fn with_suffix(row: Row, text: impl Into<String>) -> Row {
    match row {
        Row::Item {
            text: row_text,
            selected,
            bold,
            color,
            icon,
            tag,
            ..
        } => Row::Item {
            text: row_text,
            selected,
            bold,
            color,
            suffix: Some(text.into()),
            icon,
            tag,
        },
        other => other,
    }
}

/// What precedes a tag column on a row that names an item: the row's
/// shortcut, and the quantity where the screen prints one.
///
/// One definition rather than six, for `qty_column`'s reason — the six
/// screens that draw a tag have no reason to disagree about the columns in
/// front of it, and a lead that is one space out puts the tag half a
/// character off the column it is supposed to form.
pub(super) fn row_lead(shortcut: char, qty: Option<u32>) -> String {
    match qty {
        Some(qty) => format!("[{shortcut}] {} ", qty_column(qty)),
        None => format!("[{shortcut}] "),
    }
}

/// The whole of an `Item` row's text, its tag column included — what the row
/// reads as on screen, and what it read as before the column was lifted out
/// of it.
///
/// `draw_row` measures this and every width test measures this, so a row that
/// fits in a test is the row that fits on screen. Drawing takes the pieces
/// apart again (see `tag_pieces`); the two must join back to exactly this
/// string, which `a_tagged_rows_pieces_join_back_into_its_text` pins.
pub(super) fn item_text(text: &str, tag: Option<&ItemTag>) -> String {
    match tag {
        Some(t) => tagged_text(&t.lead, &t.text, t.power, text),
        None => text.to_string(),
    }
}

/// `item_text` from the pieces a row is about to be built out of, for a
/// caller that has to know how wide the row will be *before* there is a row —
/// the two screens that wrap a long row onto continuation lines. The wrap has
/// to see the column's width, or it budgets for a row narrower than the one
/// it will draw.
pub(super) fn tagged_text(lead: &str, tag: &str, power: PowerCell, text: &str) -> String {
    format!("{lead}{tag}{TAG_GAP}{}{TAG_GAP}{text}", power.text())
}

/// `wrapped_row_lines` for a row that carries a tag column: the wrap sees the
/// whole row, and the head line comes back *without* the column, ready for
/// `with_tag`.
///
/// The alternative — wrapping the joined string and handing that to the row —
/// would put the tag back inside `text`, which is the five-way `format!` this
/// column was lifted out of.
pub(super) fn wrapped_tagged_lines(
    lead: &str,
    tag: &str,
    power: PowerCell,
    head: &str,
    tags: &[String],
) -> Vec<String> {
    let prefix = tagged_text(lead, tag, power, "");
    let mut lines = wrapped_row_lines(tagged_text(lead, tag, power, head), tags);
    lines[0] = lines[0]
        .strip_prefix(&prefix)
        .expect("wrapped_row_lines only ever appends to the head line")
        .to_string();
    lines
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
            tag,
            ..
        } => Row::Item {
            text,
            selected,
            bold,
            color: text_color,
            suffix,
            icon: Some((glyph, color)),
            tag,
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
        tag: None,
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
        tag: None,
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
        tag: None,
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
        tag: None,
    }
}

/// `item_row` for anything carrying a permanent tier — a program or a piece
/// of gear, both of which now have two of them. `tier_color` decides which
/// one wins; this is only the row-building half.
///
/// Every menu that lists either goes through here, so none of them can grow
/// its own idea of what the colours mean. A caller with a louder rule of its
/// own checks that first and only falls through to this (see
/// `draw_companion_menu`'s CRITICAL).
///
/// It absorbed a `fusion_row` that took no rarity, back when gear had none.
/// Keeping both would have left the gear screens silently unable to show a
/// tier they now roll.
pub(super) fn tier_row(s: impl Into<String>, selected: bool, fusions: u32, rarity: Rarity) -> Row {
    match tier_color(fusions, rarity) {
        Some(color) => colored_item_row(s, selected, color),
        None => item_row(s, selected),
    }
}

/// `colored_item_row` carrying a trailing note about the row, drawn dim and
/// set apart from the row's own text so it reads as an annotation rather than
/// as part of the label. `None` carries nothing at all, which is what a row
/// with nothing to annotate wants — a `×0` on every line is noise.
pub(super) fn annotated_item_row(
    s: impl Into<String>,
    suffix: Option<String>,
    selected: bool,
    color: Color,
) -> Row {
    Row::Item {
        text: s.into(),
        selected,
        bold: false,
        color,
        suffix,
        icon: None,
        tag: None,
    }
}

/// `annotated_item_row` for a row standing for `repeats` identical log lines —
/// the history screen's folded rows (see `Game::message_history`). A row
/// standing for one line carries nothing.
pub(super) fn counted_item_row(
    s: impl Into<String>,
    repeats: usize,
    selected: bool,
    color: Color,
) -> Row {
    annotated_item_row(
        s,
        (repeats > 1).then(|| format!("×{repeats}")),
        selected,
        color,
    )
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
        tag: None,
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
/// How wide and tall a popup of this size is, as fractions of the window.
///
/// One table rather than a match at each reader: `popup_max_rows` and
/// `draw_popup` have to agree about the height, or a height test passes
/// against a box the drawing code sizes differently.
fn popup_fractions(size: PopupSize) -> (f32, f32) {
    match size {
        PopupSize::Large => (0.88, 0.85),
        PopupSize::Small => (0.5, 0.85),
    }
}

/// How many rows a popup of `size` draws at `screen_h` before `draw_row`
/// starts clipping them off the bottom.
///
/// **`draw_popup` pages a `Row::Item` span and nothing else**, so a page
/// built entirely out of text rows — the gear inspect page — has no scroll
/// and simply loses its tail, in silence. This is what a height test
/// measures such a page against.
#[cfg(test)]
pub(super) fn popup_max_rows(screen_h: f32, size: PopupSize, m: &Metrics) -> usize {
    let chrome = m.line_height * 2.0 + m.inset;
    ((screen_h * popup_fractions(size).1 - chrome) / m.line_height)
        .floor()
        .max(0.0) as usize
}

/// `status` is how many lines a refusal drawn under the title takes up.
/// Counted here rather than prepended to `rows` because `Row` is not
/// `Clone` and `rows` is borrowed — and because it must not join the
/// `Row::Item` span the body pages through, or a refusal appearing would
/// move the rows `App::selected_index` resolves a keypress against.
fn popup_layout<'a>(
    screen_h: f32,
    pct_h: f32,
    rows: &'a [Row],
    status: usize,
    m: &Metrics,
) -> PopupLayout<'a> {
    // Two lines for the title and its divider, plus the bottom inset.
    let chrome = m.line_height * 2.0 + m.inset;
    let max_rows = ((screen_h * pct_h - chrome) / m.line_height)
        .floor()
        .max(0.0) as usize;
    let rows_shown = (rows.len() + status).min(max_rows).max(MIN_POPUP_ROWS);
    let h = rows_shown as f32 * m.line_height + chrome;

    let first_item = rows.iter().position(|r| matches!(r, Row::Item { .. }));
    let last_item = rows.iter().rposition(|r| matches!(r, Row::Item { .. }));
    let (header, body, footer): (&[Row], &[Row], &[Row]) = match (first_item, last_item) {
        (Some(first), Some(last)) => (&rows[..first], &rows[first..=last], &rows[last + 1..]),
        _ => (rows, &[], &[]),
    };

    let raw_capacity = rows_shown.saturating_sub(header.len() + footer.len() + status);
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

/// Draws a bordered popup: `title`, a `status` line if the player's last
/// action was refused, then `rows`.
///
/// **Every caller passes a `status`, and the topmost popup on screen is the
/// one that passes `Some`.** A refusal is drawn where the player is already
/// looking — inside the panel they typed into — rather than on the strip
/// `draw_status_banner` paints along the bottom edge, which is what the
/// message used to have to compete with a centred popup from. That strip
/// survives only for the modes that draw no popup at all; see
/// `needs_status_banner`.
///
/// The refusal sits between the title and the rows as its own lines rather
/// than as a `Row`, so it never lands in the `Row::Item` span the body
/// pages through: a refusal appearing must not renumber the options a
/// keypress resolves against.
pub(super) fn draw_popup(
    title: &str,
    size: PopupSize,
    rows: &[Row],
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let (pct_w, pct_h) = popup_fractions(size);
    let status_lines: Vec<Row> = refusal
        .map(|s| wrap_text(s, status_wrap_columns(size)))
        .unwrap_or_default()
        .into_iter()
        .map(|line| Row::TextColored(line, RED))
        .collect();
    let layout = popup_layout(painter.screen_h(), pct_h, rows, status_lines.len(), m);
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
    for row in &status_lines {
        cy = draw_row(row, x, w, cy, max_y, painter, m);
    }
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

/// A tagged row's label in the two pieces that sit either side of its
/// category column, so `draw_row` can hand the painter three runs and let the
/// tag carry its own colour and weight.
///
/// Split out, like `row_label` itself, so a test can hold the pieces to
/// joining back into exactly the string `draw_row` measures. A tag drawn from
/// one set of pieces and measured from another is a row whose suffix lands on
/// top of its own tail.
fn tag_pieces(
    prefix: &str,
    icon: Option<(char, Color)>,
    text: &str,
    tag: &ItemTag,
) -> (String, String, String) {
    (
        row_label(prefix, icon, &tag.lead),
        format!("{TAG_GAP}{}", tag.power.text()),
        format!("{TAG_GAP}{text}"),
    )
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
            tag,
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
            let label = row_label(prefix, *icon, &item_text(s, tag.as_ref()));
            let heavy = *selected && *bold;
            match tag {
                // Three runs rather than three draw calls: `ui_runs` lays the
                // line out in one galley, so the tag's weight cannot shift
                // the name after it (see
                // `emphasising_part_of_a_line_does_not_shift_the_rest_of_it`).
                Some(t) => {
                    let (head, power, tail) = tag_pieces(prefix, *icon, s, t);
                    painter.ui_runs(
                        &[
                            TextRun {
                                text: &head,
                                bold: heavy,
                                color: *color,
                            },
                            TextRun {
                                text: &t.text,
                                bold: t.bold,
                                color: t.color,
                            },
                            // Dim, like the suffix column: the rating is an
                            // annotation on the row, and painting it in the
                            // row's own colour would make it compete with
                            // the name it is rating.
                            TextRun {
                                text: &power,
                                bold: false,
                                color: TEXT_DIM,
                            },
                            TextRun {
                                text: &tail,
                                bold: heavy,
                                color: *color,
                            },
                        ],
                        x + m.pad,
                        cy,
                        m.font_size,
                    );
                }
                None if heavy => painter.ui_bold(&label, x + m.pad, cy, m.font_size, *color),
                None => painter.ui(&label, x + m.pad, cy, m.font_size, *color),
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

/// How wide a *menu row* may run before its trailing detail moves onto a
/// continuation line. Separate from `DESCRIBE_WRAP_COLUMNS` because that one
/// is for prose, which reads better narrow than wide; a menu row is a record
/// the eye scans, and splitting one that would have fit costs a line of the
/// popup and a glance to reassemble.
///
/// So this sits close to the real edge instead. Measured at 1440x900, a
/// `PopupSize::Large` body has room for about 114 monospace cells including
/// the two `draw_row` prefixes with, so 100 leaves margin for the narrower
/// windows the popup's percentage width produces without splitting rows that
/// would have been fine.
pub(super) const ROW_WRAP_COLUMNS: usize = 100;

/// How wide a refusal may run inside a popup of `size` before it wraps.
///
/// Derived from `ROW_WRAP_COLUMNS` and the popup's own width fraction
/// rather than authored per size: the two fractions are what actually
/// decide how many cells there are, so a `PopupSize::Small` panel — half a
/// `Large` one — gets half the budget without anyone maintaining a second
/// number. Nothing clamps a row horizontally (see `wrapped_row_lines`), so
/// an over-wide refusal would be drawn off the panel in silence.
/// How many lines to leave for a refusal when measuring whether a
/// scroll-less page fits its popup.
///
/// `draw_popup` pages a `Row::Item` span and nothing else, so a page built
/// entirely out of text rows — the gear inspect page, the memories page —
/// has no scroll and loses its tail in silence. A refusal makes such a page
/// one or two rows taller for four seconds, which is exactly long enough to
/// eat the last row of one and exactly short enough that nobody would catch
/// it by eye. Two lines is `status_wrap_columns` twice over, longer than any
/// sentence the engine builds for a refusal.
#[cfg(test)]
pub(super) const REFUSAL_MAX_LINES: usize = 2;

fn status_wrap_columns(size: PopupSize) -> usize {
    let (pct_w, _) = popup_fractions(size);
    let (large_w, _) = popup_fractions(PopupSize::Large);
    ((ROW_WRAP_COLUMNS as f32) * pct_w / large_w) as usize
}

/// What a continuation line is indented by: the glyph slot `with_icon`
/// reserves inside the row above, plus the width of its `[x] ` shortcut, so
/// the detail sits under the row's name rather than under its icon.
const ROW_CONTINUATION_INDENT: &str = "       ";

/// A menu row's trailing detail, wrapped and indented onto its own lines
/// under the row it belongs to — the shape `draw_craft_menu` gives a recipe
/// and its cost, and the fuse and extract pickers a program and its
/// routines.
///
/// One definition rather than one per screen because the indent is a fact
/// about `with_icon`'s glyph slot and `menu_shortcut`'s bracket, which every
/// caller shares, and because the wrap is what keeps a row inside the popup:
/// `draw_row` clamps a row vertically and nothing clamps it horizontally.
///
/// Empty text yields no lines at all, so a caller with nothing to add gets
/// no blank continuation.
pub(super) fn continuation_lines(text: &str) -> Vec<String> {
    wrap_text(text, ROW_WRAP_COLUMNS - ROW_CONTINUATION_INDENT.len())
        .into_iter()
        .map(|line| format!("{ROW_CONTINUATION_INDENT}{line}"))
        .collect()
}

/// What an authored description is indented by under the entry it belongs
/// to. Four columns rather than `ROW_CONTINUATION_INDENT`'s seven: that
/// indent is a fact about `with_icon`'s glyph slot, and none of the pickers
/// that print a description carry an icon — four is `menu_shortcut`'s
/// `[x] ` alone, which is what puts the prose under the entry's name.
pub(super) const DESCRIPTION_INDENT: &str = "    ";

/// One entry's authored description, wrapped to the popup and indented under
/// the row it belongs to — the perk and research pickers' shape, and the
/// deploy menu's.
///
/// One definition rather than one per picker because the shipped assets
/// carry up to about 300 characters of prose against a `PopupSize::Large`
/// body of roughly 114: printed raw it runs off the right edge in silence,
/// since `draw_row` clamps a row vertically and nothing clamps it
/// horizontally. The deploy menu shipped doing exactly that while the perk
/// picker beside it was already wrapping.
///
/// `DESCRIBE_WRAP_COLUMNS` rather than `ROW_WRAP_COLUMNS` because this is
/// prose, which reads better narrow than wide — the same width the Recipes
/// screen wraps a product's description to, so no two screens can disagree
/// about how wide the game's prose runs.
///
/// The lines stay `Row::Item`, which `perks_menu_rows` and `build_menu_rows`
/// both document as load-bearing: `popup_layout` cuts the scrollable body at
/// the last `Row::Item`, so a description made of `Row::Text` is torn off the
/// entry it describes and pinned to the foot of the box.
pub(super) fn description_rows(description: &str) -> impl Iterator<Item = Row> + '_ {
    wrap_text(
        description,
        DESCRIBE_WRAP_COLUMNS - DESCRIPTION_INDENT.chars().count(),
    )
    .into_iter()
    .map(|line| colored_item_row(format!("{DESCRIPTION_INDENT}{line}"), false, TEXT_DIM))
}

/// A menu row wrapped onto indented continuation lines at its own segment
/// boundaries: a `head` that always leads, then trailing `tags` packed on
/// after it while they fit and shed onto a fresh indented line when they
/// don't. An empty tag contributes nothing, so a row carrying none is
/// returned as the single line it already was.
///
/// The counterpart of `continuation_lines` rather than a second copy of it,
/// and the difference is what a segment is. That one takes a row's trailing
/// *detail* — separate text, and prose, so word wrap is right for it. A row
/// built out of optional tags is neither: the tags are the units that come
/// and go, so breaking inside one splits a fact across two lines, and the
/// double spaces holding a row's columns apart are exactly what
/// `wrap_text`'s `split_whitespace` would collapse. So this touches nothing
/// inside a segment and never rewrites what it packs.
///
/// A single tag wider than the budget is emitted whole on its own line, the
/// same call `wrap_text` makes about an over-long word and for the same
/// reason: one row running wide beats losing text.
pub(super) fn wrapped_row_lines(head: String, tags: &[String]) -> Vec<String> {
    let mut lines = vec![head];
    for tag in tags.iter().filter(|t| !t.is_empty()) {
        let line = lines.last_mut().expect("the head is always present");
        if line.chars().count() + tag.chars().count() <= ROW_WRAP_COLUMNS {
            line.push_str(tag);
        } else {
            // Trimmed because a tag carries the space that joined it to the
            // row it was following, and it is no longer following anything.
            lines.push(format!("{ROW_CONTINUATION_INDENT}{}", tag.trim_start()));
        }
    }
    lines
}

/// Greedy word wrap to `columns`, for prose too long to sit on one popup
/// row — an item's authored description, chiefly.
///
/// The wrap itself is the engine's, so the manual's row count and the
/// renderer's cannot drift; this is the name the drawing code already uses.
pub(super) fn wrap_text(text: &str, columns: usize) -> Vec<String> {
    feral_processes_engine::text::wrap(text, columns)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three properties `wrapped_row_lines` is relied on for, none of
    /// which the roster census above it can distinguish from luck: a row
    /// under budget is left exactly as it was, an empty tag is not a
    /// segment, and packing continues across as many lines as the tags need
    /// rather than dumping the whole tail onto a second one.
    #[test]
    fn a_row_packs_its_tags_and_sheds_only_what_will_not_fit() {
        let short = wrapped_row_lines("[a] Kestrel".into(), &[" (in party)".into(), String::new()]);
        assert_eq!(short, vec!["[a] Kestrel (in party)"]);

        // Five 43-cell tags behind an 8-cell head: two fit on the head line,
        // two more on the first continuation, one on the second. A packer
        // that shed the whole tail at the first tag that did not fit would
        // give four lines here rather than three.
        let tag = |n: usize| format!(" ({})", "x".repeat(n));
        let packed = wrapped_row_lines("[a] head".into(), &vec![tag(40); 5]);
        assert_eq!(
            packed.len(),
            3,
            "each line takes what it can hold: {packed:#?}"
        );
        assert!(packed[1].starts_with(ROW_CONTINUATION_INDENT));
        assert!(
            packed.iter().all(|l| l.chars().count() <= ROW_WRAP_COLUMNS),
            "{packed:#?}"
        );

        // Wider than the budget on its own. Emitted whole rather than split,
        // the same call `wrap_text` makes about an over-long word: one row
        // running wide beats losing text.
        let huge = wrapped_row_lines("[a] head".into(), &[tag(ROW_WRAP_COLUMNS + 10)]);
        assert_eq!(huge.len(), 2);
        assert!(huge[1].contains(&"x".repeat(ROW_WRAP_COLUMNS + 10)));
    }

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
    fn a_tier_row_colours_by_depth_and_leaves_a_plain_row_plain() {
        let plain = Rarity::Ordinary;
        assert_eq!(row_color(&tier_row("x", false, 0, plain)), Some(TEXT));
        assert_eq!(row_color(&tier_row("x", false, 1, plain)), Some(CYAN));
        assert_eq!(
            row_color(&tier_row("x", false, MAX_FUSIONS, plain)),
            Some(MAGENTA)
        );
        // A rare copy that has never been fused takes the rarity colour, and
        // a fused one gives it up — the precedence `tier_color` states. Gear
        // reaches both arms now, which is why this row builder took a rarity
        // in the first place.
        assert_eq!(
            row_color(&tier_row("x", false, 0, Rarity::Gold)),
            rarity_color(Rarity::Gold)
        );
        assert_eq!(
            row_color(&tier_row("x", false, 1, Rarity::Gold)),
            Some(CYAN)
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

    /// The columns in front of the tag, pinned against the hand-formatted
    /// rows they were lifted out of. Six screens draw this column, and it is
    /// a column only while every lead is built the same way — one space out
    /// and a shelf ragged-lefts.
    ///
    /// The literals are what the six `format!` calls printed before the lift,
    /// so this is the assertion that says the screens did not move.
    #[test]
    fn a_lifted_tag_row_reads_exactly_as_the_hand_formatted_one_did() {
        // `"[{}] {} {}  Sell {} ..."` — a trader's shelf, the inventory list
        // and the Stack market.
        assert_eq!(
            tagged_text(
                &row_lead('a', Some(3)),
                "WEP",
                PowerCell::Rated(54),
                "Sell Arc Lance"
            ),
            format!(
                "[{}] {} {}  {:>4}  {}",
                'a',
                qty_column(3),
                "WEP",
                54,
                "Sell Arc Lance"
            ),
        );
        // `"[{}] {}  Buy {} ..."` — a trader's stock, and the Compile screen,
        // neither of which prints a count.
        assert_eq!(
            tagged_text(
                &row_lead('a', None),
                "MOD",
                PowerCell::Rated(397),
                "Buy Watchdog Tap"
            ),
            format!("[{}] {}  {:>4}  {}", 'a', "MOD", 397, "Buy Watchdog Tap"),
        );
        // A four-digit stack grows the row rather than losing a digit, which
        // is `qty_column`'s call, not this one's.
        assert!(row_lead('a', Some(1234)).len() > row_lead('a', Some(3)).len());
    }

    /// The pieces `draw_row` paints and the string it measures are the same
    /// row. They are built by different code — three runs for the painter,
    /// one joined label for `suffix_x` — and a gap between the two is a
    /// suffix landing on top of the row's own tail, which is exactly the bug
    /// `suffix_x` was split out to hold off.
    #[test]
    fn a_tagged_rows_pieces_join_back_into_its_text() {
        let Row::Item { text, tag, .. } = with_tag(
            item_row("Arc Lance (115%)", false),
            "[a]   3x  ",
            "WEP",
            Some(115),
            PowerCell::Rated(54),
        ) else {
            panic!("with_tag returns the Item row it was given")
        };
        let tag = tag.expect("with_tag sets the column");
        assert_eq!(
            item_text(&text, Some(&tag)),
            "[a]   3x  WEP    54  Arc Lance (115%)",
            "the joined row reads exactly as the hand-formatted one did"
        );
        for icon in [None, Some(('o', TEXT))] {
            let (head, power, tail) = tag_pieces("  ", icon, &text, &tag);
            assert_eq!(
                format!("{head}{}{power}{tail}", tag.text),
                row_label("  ", icon, &item_text(&text, Some(&tag))),
                "the drawn pieces and the measured label must be one string"
            );
        }
    }

    /// The three cells are three *meanings*, and each has to be
    /// distinguishable on the row's own pieces — never on a substring of the
    /// joined text, which passes just as well against a renderer that
    /// formatted the column into the middle of a string and left no span to
    /// paint.
    #[test]
    fn each_power_cell_draws_its_own_mark() {
        let cell = |power| {
            let Row::Item { text, tag, .. } = with_tag(
                item_row("Arc Lance", false),
                row_lead('a', None),
                "WEP",
                None,
                power,
            ) else {
                panic!("with_tag returns the Item row it was given")
            };
            let tag = tag.expect("with_tag sets the column");
            let (_, power, _) = tag_pieces("  ", None, &text, &tag);
            power
        };
        assert_eq!(cell(PowerCell::Rated(54)).trim(), "54");
        assert_eq!(
            cell(PowerCell::Unrated).trim(),
            "\u{2014}",
            "no combat axis is an em dash — there is no answer, not a bad answer"
        );
        assert_eq!(
            cell(PowerCell::Blank).trim(),
            "",
            "a row that is not an item draws nothing here: a dash would claim it \
             had been rated and found wanting"
        );
    }

    /// The straight edge down the list is the entire feature, and the UI face
    /// is monospace — so equal character offsets are equal pixels, which is
    /// the same reasoning `draw_row` reserves the icon's slot on.
    ///
    /// Asserted across *both* axes that could break it: a longer name after
    /// the cell, and a wider figure inside it.
    #[test]
    fn every_power_cell_is_the_same_width() {
        let widths: Vec<usize> = [
            PowerCell::Blank,
            PowerCell::Unrated,
            PowerCell::Rated(5),
            PowerCell::Rated(-38),
            PowerCell::Rated(1421),
        ]
        .into_iter()
        .map(|c| c.text().chars().count())
        .collect();
        assert_eq!(widths, vec![POWER_COLUMN_WIDTH; 5]);

        let offset = |name: &str, power| {
            let row = tagged_text(&row_lead('a', Some(3)), "WEP", power, name);
            row.find(name).expect("the name is in the row it names")
        };
        assert_eq!(
            offset("Shim Blade", PowerCell::Rated(36)),
            offset("A Very Much Longer Name Indeed", PowerCell::Rated(1421)),
            "the name — and so the column in front of it — must start at one x \
             on every row"
        );
    }

    /// The column is a *third* axis, beside the row's colour and its icon:
    /// how well the copy was compiled is not something either of those can
    /// say. So neither may be disturbed by it, and an untagged row must be
    /// left byte-identical to what it was before the column existed.
    #[test]
    fn a_tag_leaves_the_rows_own_colour_and_text_alone() {
        let row = with_tag(
            tier_row("Arc Lance", true, 1, Rarity::Ordinary),
            "[a] ",
            "WEP",
            Some(130),
            PowerCell::Rated(54),
        );
        assert_eq!(
            row_color(&row),
            Some(CYAN),
            "the row keeps its fusion colour"
        );
        let Row::Item { text, selected, .. } = &row else {
            panic!("still an Item row")
        };
        assert_eq!(
            text, "Arc Lance",
            "the tag is not folded back into the text"
        );
        assert!(selected, "the selection survives the combinator");
        assert_eq!(
            item_text("Arc Lance", None),
            "Arc Lance",
            "an untagged row is exactly the string it always was"
        );
    }

    /// What the whole phase is for: the tag carries its band's colour and
    /// weight while the rest of the row keeps its own. Drawn through the real
    /// painter, because the runs are only three separate styles once egui has
    /// laid the job out — `painted_text` would flatten them back to one line
    /// and report nothing.
    #[test]
    fn a_drawn_tag_carries_its_band_and_the_row_keeps_its_own_colour() {
        let m = Metrics {
            font_size: 16,
            line_height: 20.0,
            pad: 8.0,
            inset: 4.0,
            gap: 6.0,
        };
        let row = with_tag(
            colored_item_row("Arc Lance (130%)", false, CYAN),
            "[a] ",
            "WEP",
            Some(130),
            PowerCell::Rated(54),
        );
        let (_, shapes) = crate::paint::with_painter(|p| {
            draw_row(&row, 0.0, 400.0, 40.0, 400.0, p, &m);
        });
        let (gold, bold) = quality_tag_style(Some(130));
        assert_eq!(
            crate::paint::painted_runs_in(&shapes, gold, bold),
            vec!["WEP"],
            "the exceptional band paints the tag and nothing else"
        );
        assert_eq!(
            crate::paint::painted_runs_in(&shapes, CYAN, false),
            vec!["  [a] ", "  Arc Lance (130%)"],
            "the row's own colour still carries everything either side of it"
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
                    let l = popup_layout(window_h, 0.85, &rows, 0, &m);
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
                    let l = popup_layout(window_h, 0.85, &rows, 0, &m);
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

    /// The four routine pickers' real rows for `n` entries with row
    /// `selected` highlighted, each paired with what `popup_layout` is
    /// allowed to pin below the list. Built through the shipping row
    /// builders rather than restated here, for `build_menu_rows_fixture`'s
    /// reason: a hand-written mirror would keep passing after the real one
    /// drifted.
    fn routine_rows_fixtures(n: usize, selected: usize) -> Vec<(&'static str, Vec<Row>, usize)> {
        use super::super::routines::{
            extract_pick_rows, routine_etch_rows, routine_install_rows, routine_slot_rows,
        };
        use feral_processes_engine::views::{
            EtchedDiskView, ExtractableRoutineView, KnownRoutineView, RoutineSlotView,
        };
        let id = |i: usize| format!("routine_{i}");
        let name = |i: usize| format!("Routine {i}");
        let text = |i: usize| format!("What routine {i} does, at some length.");
        vec![
            (
                "slots",
                routine_slot_rows(
                    &(0..n)
                        .map(|i| RoutineSlotView {
                            index: i,
                            ability: Some(id(i)),
                            name: name(i),
                            description: text(i),
                        })
                        .collect::<Vec<_>>(),
                    selected,
                ),
                0,
            ),
            (
                "install",
                routine_install_rows(
                    &(0..n)
                        .map(|i| EtchedDiskView {
                            ability: id(i),
                            name: name(i),
                            description: text(i),
                            exclusive: i % 3 == 0,
                            qty: 1 + i as u32,
                        })
                        .collect::<Vec<_>>(),
                    4,
                    selected,
                ),
                // The blank line and the `[e]` legend are pinned on purpose.
                2,
            ),
            (
                "etch",
                routine_etch_rows(
                    &(0..n)
                        .map(|i| KnownRoutineView {
                            ability: id(i),
                            name: name(i),
                            description: text(i),
                            held: i as u32 % 3,
                        })
                        .collect::<Vec<_>>(),
                    4,
                    selected,
                ),
                0,
            ),
            (
                "extract",
                extract_pick_rows(
                    &(0..n)
                        .map(|i| ExtractableRoutineView {
                            ability: id(i),
                            name: name(i),
                            description: text(i),
                            known: i % 2 == 0,
                        })
                        .collect::<Vec<_>>(),
                    selected,
                ),
                0,
            ),
        ]
    }

    /// The reported bug: the last routine on the etch screen didn't show what
    /// it does. The body is cut at the *last* `Row::Item` and everything after
    /// it is pinned as a footer, so a description emitted as `Row::Text` fell
    /// past that cut — drawn detached at the bottom of the popup, under the
    /// scroll indicator, while every other description sat inline under its
    /// own routine.
    ///
    /// Nothing may follow the last item row except a screen's own pinned
    /// legend, which only the install picker has.
    #[test]
    fn every_routine_description_stays_inside_the_scrollable_body() {
        for window_h in WINDOW_HEIGHTS {
            let m = ui_metrics(window_h);
            for n in 1..14 {
                for selected in [0, n - 1] {
                    for (screen, rows, legend) in routine_rows_fixtures(n, selected) {
                        let l = popup_layout(window_h, 0.85, &rows, 0, &m);
                        assert_eq!(
                            l.footer.len(),
                            legend,
                            "at {window_h}px the {screen} picker with {n} rows pinned {} \
                             row(s) below the list, not the {legend} it is allowed — a \
                             description is detached from the routine it describes",
                            l.footer.len()
                        );
                    }
                }
            }
        }
    }

    /// The two progression pickers' real rows for `n` entries with row
    /// `selected` highlighted, built through the shipping row builders for
    /// `build_menu_rows_fixture`'s reason.
    ///
    /// The descriptions are as long as the shipped assets carry rather than
    /// a token phrase, because `description_rows` wraps them: a short one
    /// costs a single row here and the two tests below would then measure a
    /// body half the height of the real one — which is exactly the case
    /// `the_selected_progression_row_is_inside_the_visible_window` exists
    /// to catch.
    fn progression_rows_fixtures(n: usize, selected: usize) -> Vec<(&'static str, Vec<Row>)> {
        use super::super::progression::{perks_menu_rows, research_menu_rows};
        use feral_processes_engine::ResearchStatus;
        use feral_processes_engine::perks::{Perk, PerkDef};
        vec![
            (
                "perks",
                perks_menu_rows(
                    3,
                    // Split across two headed sections, because the headings
                    // and the blank line between them are rows too: a
                    // fixture of one flat section would measure a body
                    // shorter than the one the screen draws.
                    &[("Combat", 0..n / 2), ("Workshop", n / 2..n)]
                        .into_iter()
                        .map(|(name, range)| {
                            (
                                name.to_string(),
                                range
                                    .map(|i| PerkDef {
                                        // Every shipped perk is a distinct
                                        // variant, but the row shape doesn't
                                        // read the id beyond counting levels,
                                        // so one variant repeated is enough.
                                        id: Perk::Attacker,
                                        name: format!("Perk {i}"),
                                        description: format!(
                                            "What perk {i} does, at the length the shipped \
                                             perks run to: a sentence naming the fantasy, \
                                             then another one naming the number it moves \
                                             and roughly how far each level moves it."
                                        ),
                                        cost: 1 + i as u32,
                                    })
                                    .collect::<Vec<_>>(),
                            )
                        })
                        // `PerkDb::grouped` never hands back an empty
                        // section, so neither may the fixture.
                        .filter(|(_, defs): &(String, Vec<PerkDef>)| !defs.is_empty())
                        .collect::<Vec<_>>(),
                    &[Perk::Attacker],
                    selected,
                ),
            ),
            (
                "research",
                research_menu_rows(
                    40,
                    &(0..n)
                        .map(|i| ResearchStatus {
                            id: format!("node_{i}"),
                            name: format!("Node {i}"),
                            description: format!(
                                "What node {i} unlocks, at the length the \
                                 shipped nodes run to: a sentence naming what \
                                 it teaches, then another one naming the \
                                 structures and recipes it hands over and what \
                                 they cost to build once you have it."
                            ),
                            cost: 10 + i as u32,
                            state: if i % 3 == 0 {
                                ResearchState::Unlocked
                            } else {
                                ResearchState::Available
                            },
                            affordable: true,
                            recommended: i == 1,
                        })
                        .collect::<Vec<_>>(),
                    selected,
                ),
            ),
        ]
    }

    /// The reported bug: the last perk on the perk screen didn't show what it
    /// does. Same cut as the build menu and the routine pickers above — the
    /// body ends at the last `Row::Item`, so a trailing `Row::Text`
    /// description was pinned as a footer and drawn at the foot of the box,
    /// under a blank scroll indicator, never scrolling with the list it
    /// belonged to.
    ///
    /// Nothing may follow the last item row on either picker.
    #[test]
    fn every_progression_description_stays_inside_the_scrollable_body() {
        for window_h in WINDOW_HEIGHTS {
            let m = ui_metrics(window_h);
            for n in 1..20 {
                for selected in [0, n - 1] {
                    for (screen, rows) in progression_rows_fixtures(n, selected) {
                        let l = popup_layout(window_h, 0.85, &rows, 0, &m);
                        assert!(
                            l.footer.is_empty(),
                            "at {window_h}px the {screen} picker with {n} rows pinned {} \
                             row(s) below the list — the last description is detached from \
                             the row it describes",
                            l.footer.len()
                        );
                    }
                }
            }
        }
    }

    /// And the selected row stays reachable: descriptions double the body's
    /// length, so a capacity that only counted the perks themselves would
    /// leave the last one unreachable.
    #[test]
    fn the_selected_progression_row_is_inside_the_visible_window() {
        for window_h in WINDOW_HEIGHTS {
            let m = ui_metrics(window_h);
            for n in 1..20 {
                for selected in 0..n {
                    for (screen, rows) in progression_rows_fixtures(n, selected) {
                        let l = popup_layout(window_h, 0.85, &rows, 0, &m);
                        let idx = l
                            .body
                            .iter()
                            .position(|r| matches!(r, Row::Item { selected: true, .. }))
                            .expect("the selected row is in the body");
                        assert!(
                            idx >= l.offset && idx < l.offset + l.capacity,
                            "at {window_h}px, {screen} row {selected} of {n} sits at body \
                             row {idx}, outside the window [{}, {})",
                            l.offset,
                            l.offset + l.capacity
                        );
                    }
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
                let l = popup_layout(window_h, 0.85, &rows, 0, &m);
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
                let l = popup_layout(window_h, 0.85, &rows, 0, &m);
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
            let l = popup_layout(window_h, 0.85, &rows, 0, &m);
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
