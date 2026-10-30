use super::*;

/// The roster's standing-instruction toggles for the structure highlighted
/// there: keep it running, keep it guarded, or work it yourself right now.
///
/// Which rows exist is `App::staffing`'s decision — it asks the same two
/// questions `Game::set_standing_job` and `Game::work_structure` refuse on —
/// and this draws the list it is handed rather than filtering again, so the
/// row the handler acts on is the row under the highlight.
pub(crate) fn draw_staffing_menu(
    staffing: &Staffing,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(format!(
        "Standing orders for the {}",
        staffing.target
    ))];
    if staffing.rows.is_empty() {
        rows.push(text_row("(nothing to say about this one)"));
    }
    for (i, row) in staffing.rows.iter().enumerate() {
        let mark = match row.on {
            Some(true) => "[x] ",
            Some(false) => "[ ] ",
            None => "",
        };
        rows.push(item_row(format!("{mark}{}", row.label), i == selected));
    }
    rows.push(text_row(
        "A standing job is filled only by a program no work order needs.",
    ));
    push_key_footer(&mut rows, &["Enter choose", "Esc back"]);
    draw_popup(
        "Standing orders",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// The list is the priority, so the screen says so: nothing else on it
/// tells a player that moving an order changes what the base works. The
/// keys themselves are the hub keybar's.
pub(super) const WORK_ORDERS_NOTE: &str = "The top order is worked first.";

/// The work order queue and its status: what the base has been told to
/// hold, how close it is, and which machine each order is waiting on.
///
/// The machine lines under an order are `Game::work_order_report`'s, which
/// is the scheduler's own `wants` walk — so what is on screen is what the
/// base believes by construction rather than by a comment claiming the two
/// agree.
pub(crate) fn work_order_screen_rows(
    rows_in: &[WorkOrderRow],
    demand: LabourDemand,
    selected: usize,
) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(header) = labour_header(demand) {
        rows.push(text_row(header));
    }
    rows.push(text_row(WORK_ORDERS_NOTE));
    if rows_in.is_empty() {
        rows.push(text_row("(nothing the base can make yet)"));
    }
    for (i, row) in rows_in.iter().enumerate() {
        let mut lines = work_order_lines(row).into_iter();
        if let Some(head) = lines.next() {
            rows.push(work_order_head_row(row, head, i == selected));
        }
        rows.extend(lines.map(text_row));
    }
    rows
}

/// A work order's head row. A stalled order wears a red `!` in the icon
/// column — its reason sits on the continuation rows beneath — so the one
/// order that will not run without the player is findable at a glance in a
/// long queue. `OFFLINE`, the red for "stopped and will not start again on
/// its own".
pub(super) fn work_order_head_row(row: &WorkOrderRow, head: String, selected: bool) -> Row {
    let stalled = row
        .order
        .as_ref()
        .is_some_and(|o| o.state == OrderState::Stalled);
    let item = item_row(head, selected);
    if stalled {
        with_icon(item, '!', hud::palette::OFFLINE)
    } else {
        item
    }
}

/// How many bodies short the base is, or `None` when it has enough.
///
/// **Silent when there is no shortfall.** The figure answers "why is
/// nothing happening" from the other direction to `state_tag`: the tag says
/// which order has the base's attention, this says whether the base has
/// anyone to give it. A line that shows on every visit is a line nobody
/// reads by the third one, and a base with bodies to spare has nothing to
/// explain.
///
/// A pure function of the demand for `work_order_lines`' reason — it is a
/// head line, which is to say an unwrapped one, so it is the row on this
/// screen that can actually run off the body.
pub(super) fn labour_header(demand: LabourDemand) -> Option<String> {
    let short = demand.shortfall();
    if short == 0 {
        return None;
    }
    // **"post", not "machine".** The want list carries standing guard jobs
    // and dig sites as well as machines, so a shortfall is not always a
    // machine standing idle and the header must not claim it is.
    Some(format!(
        "{} post{} wanted, {} program{} on the base — {short} unfilled",
        demand.wanted,
        if demand.wanted == 1 { "" } else { "s" },
        demand.staff,
        if demand.staff == 1 { "" } else { "s" },
    ))
}

/// One work order's lines: the order itself, then a line per machine in the
/// chain — or the sentence naming why it is stalled.
///
/// A pure function of the row rather than pushed straight into the popup,
/// so a headless test can measure how wide the widest of them runs.
/// `draw_row` clamps a row vertically and never horizontally, so nothing
/// else would catch a line that runs off the body.
/// This screen's continuation lines: wrapped to the hub's content rect, not
/// a popup body.
pub(super) fn hub_lines(text: &str) -> Vec<String> {
    continuation_lines_within(text, HUB_WRAP_COLUMNS)
}

pub(super) fn work_order_lines(row: &WorkOrderRow) -> Vec<String> {
    let Some(order) = &row.order else {
        return vec!["New work order...".to_string()];
    };
    let state = state_tag(order.state);
    let mut lines = vec![format!(
        "{}  {}/{}{state}",
        order.label, order.have, order.target
    )];
    // Through `continuation_lines` rather than an inline `format!`: a
    // stalled order's reason is a whole sentence and a machine line carries
    // three names, and either can outrun the popup body.
    if let Some(why) = &order.blocked_by {
        lines.extend(hub_lines(why));
        return lines;
    }
    if order.machines.is_empty() {
        // Three ways to want nobody, and the tag alone does not say which:
        // the base is holding the level, the line broke (handled above), or
        // every machine in it is momentarily busy or clogged.
        lines.extend(hub_lines(match order.state {
            OrderState::Dormant => "holding — the base has this, so nothing is being made",
            _ => "waiting — nothing to do here yet",
        }));
    }
    for machine in &order.machines {
        let who = match &machine.worker {
            Some(name) => name.clone(),
            None => "no one".to_string(),
        };
        let short = machine
            .short_of
            .as_ref()
            .map(|s| format!(", short of {s}"))
            .unwrap_or_default();
        lines.extend(hub_lines(&format!("{} — {who}{short}", machine.label)));
    }
    lines
}

/// The token beside an order's row. Every state carries one, including the
/// healthy ones: the screen's job in a queue several orders deep is to say
/// which order has the base's attention, and a tag that appears only when
/// something is wrong cannot answer that.
/// `HOLDING` rather than the enum's own word, because the sentence that
/// filed the order already said "hold 5 x Core Fragment" — the player's
/// word for the state is the one they typed it in with.
pub(super) fn state_tag(state: OrderState) -> &'static str {
    match state {
        OrderState::Working => "  WORKING",
        OrderState::Queued => "  QUEUED",
        OrderState::Dormant => "  HOLDING",
        OrderState::Stalled => "  STALLED",
    }
}

/// Picking what to order — `Game::orderable_items`, which asks the same
/// chain question the queue refuses on, so nothing here can be rejected.
pub(crate) fn draw_work_order_pick(
    items: &[(ItemId, String)],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row("What should the base hold?")];
    if items.is_empty() {
        rows.push(text_row("(nothing the base can make — deploy a machine)"));
    }
    for (i, (_, name)) in items.iter().enumerate() {
        rows.push(item_row(name.clone(), i == selected));
    }
    push_key_footer(&mut rows, &["Enter choose", "Esc cancel"]);
    draw_popup(
        "New work order",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

pub(super) const WORK_ORDER_QUANTITY_KEYS: &[&str] = &[
    "Type digits",
    "[S] standing order",
    "Enter order",
    "Esc cancel",
];

/// How many of it. The same two-page shape the compile flow uses.
///
/// `PopupSize::Large`, as `draw_craft_quantity` is: the sentences on it are
/// prose rather than menu rows, and the widest already ran 8px past a small
/// box before this page gained a toggle to explain.
pub(crate) fn draw_work_order_quantity(
    game: &Game,
    item: Option<ItemId>,
    typed: &str,
    standing: bool,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let name = item
        .as_ref()
        .map(|i| game.item_name(i).to_string())
        .unwrap_or_default();
    let shown = if typed.is_empty() { "1" } else { typed };
    let rows: Vec<Row> = work_order_quantity_lines(&name, shown, standing)
        .into_iter()
        .map(text_row)
        .collect();
    draw_popup(
        "Order quantity",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// The quantity page's lines, pure for `work_order_lines`' reason: a page
/// built out of text rows has no scroll and `draw_row` never clips one
/// horizontally, so a sentence that outgrows the popup body is lost in
/// silence and only a headless measurement catches it.
pub(super) fn work_order_quantity_lines(name: &str, shown: &str, standing: bool) -> Vec<String> {
    vec![
        format!("How many {name} should the base hold?"),
        String::new(),
        format!("Quantity: {shown}"),
        String::new(),
        format!(
            "Standing order: {}",
            if standing {
                "on — the base tops this level back up as it drains"
            } else {
                "off — one batch, and the order is done with"
            }
        ),
        String::new(),
        // `base_holding` sums machine and depot buffers only, so a player
        // carrying forty of the thing still reads 0/20 on the queue screen.
        "The target is what the base holds. What you are carrying is yours.".to_string(),
        String::new(),
        key_hints(WORK_ORDER_QUANTITY_KEYS),
    ]
}

/// The header line's three counts — `WorkTable::on_shift`/`jobs`/
/// `unworked_total` — in `labour_header`'s plain-sentence shape one screen
/// over, since a restriction that leaves work undone is exactly what this
/// screen exists to make visible rather than silent.
pub(super) fn work_table_header(table: &WorkTable) -> String {
    format!(
        "{} on shift, {} job{} wanted, {} unworked",
        table.on_shift,
        table.jobs,
        if table.jobs == 1 { "" } else { "s" },
        table.unworked_total,
    )
}

/// A duty column's header text — the label, plus `N!` when the matching left
/// wants under it unworked, blank at zero.
pub(super) fn duty_heading(column: &WorkColumn) -> String {
    if column.unworked > 0 {
        format!("{} {}!", column.label, column.unworked)
    } else {
        column.label.to_string()
    }
}

/// `s` centred inside `width`, the extra space split with the shorter side
/// first — a checkbox cell under a wider header column.
pub(super) fn centered(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.to_string();
    }
    let pad = width - len;
    let left = pad / 2;
    format!("{}{s}{}", " ".repeat(left), " ".repeat(pad - left))
}

/// Two spaces between every column — `transfer.rs`'s `COLUMN_GAP`, the same
/// whole-monospace-cell reasoning: a heading has no `suffix_x` pixel inset to
/// match, so every boundary has to land on a character count both the header
/// and the rows agree on.
pub(super) const COLUMN_GAP: &str = "  ";

/// The lead every data row on this screen carries before its own text: the
/// selection prefix (`"  "`/`"> "`, both two cells) plus the icon slot every
/// row reserves through `with_icon`. `transfer.rs`'s `HEADER_LEAD`, widened
/// by the icon slot this screen's rows also carry — the column header and
/// the away divider are both `Row::Text`, which get no lead of their own,
/// so without this they sit five cells left of the table they name.
pub(super) const ROW_LEAD: &str = "     ";

/// Every duty cell is a fixed three characters wide — `[x]`, `[ ]`, or the
/// column cursor's `<x>`/`< >` — so a duty column is never narrower than its
/// own cell even when nothing left a `!` on the header.
pub(super) const DUTY_CELL_WIDTH: usize = 3;

/// The widest the DOING column grows. The table sits beside the hub's rail,
/// where the longest shipped activity ("guarding the Contract Broker") plus
/// five duty columns and a POD column ran past the content rect at 1280x720.
pub(super) const DOING_MAX_CHARS: usize = 22;

/// `text` cut to `DOING_MAX_CHARS` with an ellipsis, or whole if it fits.
pub(super) fn clipped_chars(text: &str) -> String {
    if text.chars().count() <= DOING_MAX_CHARS {
        return text.to_string();
    }
    let kept: String = text.chars().take(DOING_MAX_CHARS - 1).collect();
    format!("{kept}…")
}

/// The Base staff table's column widths, measured from the rows actually
/// listed rather than fixed — `transfer.rs`'s `Columns`, widened from three
/// columns to `#`/`PROGRAM`/`DOING` plus one per `Duty`.
pub(super) struct Columns {
    rank: usize,
    program: usize,
    doing: usize,
    duty: Vec<usize>,
}

impl Columns {
    fn of(table: &WorkTable) -> Self {
        let mut cols = Columns {
            rank: 1, // "#"
            program: "PROGRAM".chars().count(),
            doing: "DOING".chars().count(),
            duty: table
                .columns
                .iter()
                .map(|c| duty_heading(c).chars().count().max(DUTY_CELL_WIDTH))
                .collect(),
        };
        for (i, row) in table.rows.iter().enumerate() {
            cols.rank = cols.rank.max((i + 1).to_string().len());
            cols.program = cols.program.max(row.program.label.chars().count());
            cols.doing = cols
                .doing
                .max(row.doing.chars().count().min(DOING_MAX_CHARS));
        }
        cols
    }

    fn total_width(&self) -> usize {
        self.rank
            + self.program
            + self.doing
            + self.duty.iter().sum::<usize>()
            + COLUMN_GAP.len() * (2 + self.duty.len())
    }

    fn header(&self, table: &WorkTable) -> String {
        let mut out = String::from(ROW_LEAD);
        out.push_str(&format!("{:<1$}", "#", self.rank));
        out.push_str(COLUMN_GAP);
        out.push_str(&format!("{:<1$}", "PROGRAM", self.program));
        out.push_str(COLUMN_GAP);
        out.push_str(&format!("{:<1$}", "DOING", self.doing));
        for (column, width) in table.columns.iter().zip(&self.duty) {
            out.push_str(COLUMN_GAP);
            out.push_str(&centered(&duty_heading(column), *width));
        }
        out
    }

    /// `cursor_column` is `Some` only on the selected row — every other row
    /// draws a plain `[x]`/`[ ]`, since a column cursor with no row cursor
    /// is not a cell.
    fn row(&self, index: usize, row: &WorkRow, cursor_column: Option<usize>) -> String {
        let mut out = format!("{:>1$}", index + 1, self.rank);
        out.push_str(COLUMN_GAP);
        out.push_str(&format!("{:<1$}", row.program.label, self.program));
        out.push_str(COLUMN_GAP);
        out.push_str(&format!("{:<1$}", clipped_chars(&row.doing), self.doing));
        for (i, width) in self.duty.iter().enumerate() {
            out.push_str(COLUMN_GAP);
            let checked = row.cells.get(i).copied().unwrap_or(true);
            let cell = match (checked, cursor_column == Some(i)) {
                (true, true) => "<x>",
                (true, false) => "[x]",
                (false, true) => "< >",
                (false, false) => "[ ]",
            };
            out.push_str(&centered(cell, *width));
        }
        out
    }
}

/// The rule between the Staff and Away sections — `── away ──…` padded to
/// the table's own width so it reads as a divider rather than a stray line.
pub(super) fn away_divider(cols: &Columns) -> String {
    const LABEL: &str = "── away ";
    let width = cols.total_width();
    format!(
        "{ROW_LEAD}{LABEL}{}",
        "─".repeat(width.saturating_sub(LABEL.chars().count()))
    )
}

/// The Base staff table's rows: the header counts, the column header, and
/// one row per program (Staff first, then an **away** divider and section) —
/// every program the player owns, the role it is in, what it is doing, and
/// one checkbox column per `Duty`. `Game::work_table` is the one derivation
/// both this and `App::handle_base_staff_key` read.
///
/// Takes the table rather than a `Game` so the width and height censuses
/// below can state their worst case.
pub(crate) fn base_staff_menu_rows(
    table: &WorkTable,
    pets: &[PetInfo],
    selected: usize,
    column: Option<usize>,
) -> Vec<Row> {
    let cols = Columns::of(table);
    let mut rows = vec![
        text_row(work_table_header(table)),
        text_row(cols.header(table)),
    ];
    if table.rows.is_empty() {
        rows.push(text_row("(no compiled programs — beat one first)"));
    }
    let mut drew_away_divider = false;
    for (i, row) in table.rows.iter().enumerate() {
        if row.section == WorkSection::Away && !drew_away_divider {
            drew_away_divider = true;
            rows.push(text_row(away_divider(&cols)));
        }
        let cursor_column = column.filter(|_| i == selected);
        let text = cols.row(i, row, cursor_column);
        let pet = pets.iter().find(|p| p.entity == row.program.entity);
        let base_row = if row.section == WorkSection::Away {
            // Dimmed rather than tier-coloured: away is a status, and this
            // screen edits it exactly as it edits a staffed row (see
            // `App::handle_base_staff_key`), so the colour must not read as
            // "cannot be changed".
            colored_item_row(text, i == selected, TEXT_DIM)
        } else {
            tier_row(
                text,
                i == selected,
                pet.map(|p| p.fusions).unwrap_or(0),
                pet.map(|p| p.rarity).unwrap_or_default(),
            )
        };
        rows.push(with_icon(
            base_row,
            row.program.glyph,
            glyph_color(row.program.color),
        ));
    }
    rows
}

pub(crate) fn draw_structure_menu(
    structures: &[EntityView],
    title: &str,
    prompt: &str,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(prompt)];
    if structures.is_empty() {
        rows.push(text_row("(no structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        let assigned = s
            .structure_worker
            .as_ref()
            .map(|w| format!(" (assigned: {w})"))
            .unwrap_or_default();
        let durability = s
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        rows.push(with_icon(
            item_row(
                format!(
                    "{} at ({}, {}){}{}",
                    s.label, s.pos.0, s.pos.1, durability, assigned
                ),
                i == selected,
            ),
            s.glyph,
            glyph_color(s.color),
        ));
    }
    push_key_footer(&mut rows, &["Enter choose", "Esc cancel"]);
    draw_popup(title, PopupSize::Large, &rows, refusal, painter, m);
}

pub(crate) fn draw_remove_menu(
    structures: &[EntityView],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(
        "Demolish which structure? Removing Home destroys the whole base.",
    )];
    if structures.is_empty() {
        rows.push(text_row("(no structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        let durability = s
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        let home_tag = if s.is_home { " (Home)" } else { "" };
        rows.push(with_icon(
            item_row(
                format!(
                    "{} at ({}, {}){}{}",
                    s.label, s.pos.0, s.pos.1, durability, home_tag
                ),
                i == selected,
            ),
            s.glyph,
            glyph_color(s.color),
        ));
    }
    push_key_footer(&mut rows, &["Enter choose", "Esc cancel"]);
    draw_popup(
        "Demolish structure",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

pub(crate) fn draw_remove_confirm(
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = demolish_home_rows(selected);
    draw_popup(
        "Confirm demolish Home",
        PopupSize::Small,
        &rows,
        refusal,
        painter,
        m,
    );
}

pub(super) fn demolish_home_rows(selected: usize) -> Vec<Row> {
    let mut rows = vec![text_row("Demolish Home?")];
    rows.extend(wrapped_colored_rows(
        "Removing Home destroys every other structure in this base and refunds 30% of each one's materials. This can't be undone.",
        ORANGE,
        PopupSize::Small,
    ));
    rows.push(text_row(""));
    rows.push(item_row("[Y] demolish everything", selected == 0));
    rows.push(item_row("[N] keep the base", selected == 1));
    rows.push(text_row(""));
    rows.push(text_row(key_hints(&["[Y] demolish", "[N] or Esc cancel"])));
    rows
}
