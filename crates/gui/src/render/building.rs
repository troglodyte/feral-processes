//! The build, staffing, demolition, upgrade and symlink pickers.

use super::manifest::base_job_label;
use super::popup::*;
use super::*;
use feral_processes_app_core::{BaseStaffRow, WorkOrderRow};
use feral_processes_engine::WorkProfile;

/// One buildable structure as the build menu needs it: everything that
/// required a `Game` to work out, already worked out.
pub(super) struct BuildEntry {
    pub label: String,
    pub description: String,
    pub category: StructureCategory,
}

/// The heading a group opens with, or `None` for Home — a single structure
/// under a "Shelter" banner is a heading longer than the thing it labels.
fn category_heading(category: StructureCategory) -> Option<&'static str> {
    match category {
        StructureCategory::Home => None,
        StructureCategory::Extractor => Some("── Extractors — produce on their own ──"),
        StructureCategory::Assembler => Some("── Assemblers — fed by what they touch ──"),
        StructureCategory::Utility => Some("── Utility ──"),
        StructureCategory::Trade => Some("── Trade ──"),
        StructureCategory::Defence => Some("── Defence ──"),
    }
}

/// The build menu's rows, pure so the layout invariant below can be tested
/// without a `Game` or a `Painter`.
///
/// **Every row after the first is a `Row::Item`, including the headings and
/// the descriptions.** `popup_layout` ends the scrollable body at the *last*
/// `Row::Item` and pins whatever follows as a footer — so a description
/// emitted as `Row::Text` would slice the final structure's description off
/// the bottom of the list and strand it under the scroll indicator, detached
/// from the row it describes. `draw_structures` carries the same fix for the
/// same reason.
///
/// `entries` is assumed to arrive grouped by category, which
/// `StructureDb::all` guarantees; a heading is emitted whenever the category
/// changes, so an ungrouped list would simply repeat headings rather than
/// mislabel anything.
pub(super) fn build_menu_rows(entries: &[BuildEntry], selected: usize) -> Vec<Row> {
    let mut rows = vec![text_row("Esc to cancel; Up/Down + Enter also work")];
    let mut current: Option<StructureCategory> = None;
    for (i, entry) in entries.iter().enumerate() {
        if current != Some(entry.category) {
            current = Some(entry.category);
            if let Some(heading) = category_heading(entry.category) {
                rows.push(colored_item_row("", false, TEXT_DIM));
                rows.push(colored_item_row(heading, false, TEXT_DIM));
            }
        }
        rows.push(item_row(
            format!("[{}] {}", menu_shortcut(i), entry.label),
            i == selected,
        ));
        rows.push(colored_item_row(
            format!("    {}", entry.description),
            false,
            TEXT_DIM,
        ));
    }
    rows
}

pub(super) fn draw_build_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let status = game.player_status();
    let defs = game.buildable_structure_defs();
    let entries: Vec<BuildEntry> = defs
        .iter()
        .map(|def| {
            let raw_cost = game.structure_build_cost(def);
            let cost = cost_display(game, &raw_cost, &status.inventory);
            BuildEntry {
                label: format!("{} - {}", def.name, cost.join(", ")),
                description: def.description.clone(),
                category: def.category(),
            }
        })
        .collect();
    let rows = build_menu_rows(&entries, selected);
    draw_popup("Deploy", PopupSize::Large, &rows, painter, m);
}

/// The deploy prompt's rows: what is about to be placed, before the compass
/// that places it.
///
/// All `Row::Text` — nothing here is pickable, the direction keys are — so
/// `popup_layout` pins the lot and none of it scrolls, which is what a prompt
/// this short wants.
pub(super) fn build_direction_rows(name: &str, description: &str, cost: &[String]) -> Vec<Row> {
    vec![
        Row::TextColored(name.to_string(), YELLOW),
        Row::TextColored(description.to_string(), TEXT_DIM),
        text_row(""),
        text_row(format!("Costs {}", cost.join(", "))),
        text_row(""),
        text_row(DIRECTION_PROMPT),
    ]
}

const DIRECTION_PROMPT: &str = "Choose a direction to deploy (arrows/hjkl), Esc to cancel";

/// The compass the build menu hands off to. Drawn `Large` rather than `Small`
/// like the other direction prompts because it carries a structure's
/// description, which is the same text the build menu needed the wider box
/// for.
pub(super) fn draw_build_direction(
    game: &mut Game,
    pending: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    // Picked out of the same list `handle_build_key` indexed, so the screen
    // cannot describe a structure the handler wouldn't deploy.
    let def = pending.and_then(|id| {
        game.buildable_structure_defs()
            .into_iter()
            .find(|d| d.id == id)
    });
    let rows = match def {
        Some(def) => {
            let status = game.player_status();
            let cost = cost_display(game, &game.structure_build_cost(&def), &status.inventory);
            build_direction_rows(&def.name, &def.description, &cost)
        }
        None => vec![text_row(DIRECTION_PROMPT)],
    };
    draw_popup("Deploy Direction", PopupSize::Large, &rows, painter, m);
}

/// The roster's standing-instruction toggles for the structure highlighted
/// there: keep it running, keep it guarded, or work it yourself right now.
///
/// Which rows exist is `App::staffing`'s decision — it asks the same two
/// questions `Game::set_standing_job` and `Game::work_structure` refuse on —
/// and this draws the list it is handed rather than filtering again, so the
/// row the handler acts on is the row under the highlight.
pub(super) fn draw_staffing_menu(
    staffing: &Staffing,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(format!(
        "Standing orders for the {} (Esc to close; Up/Down + Enter also work)",
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
        rows.push(item_row(
            format!("[{}] {mark}{}", menu_shortcut(i), row.label),
            i == selected,
        ));
    }
    rows.push(text_row(
        "A standing job is filled only by a program no work order needs.",
    ));
    draw_popup("Standing Orders", PopupSize::Large, &rows, painter, m);
}

/// The work order queue and its status: what the base has been told to
/// hold, how close it is, and which machine each order is waiting on.
///
/// The machine lines under an order are `Game::work_order_report`'s, which
/// is the scheduler's own `wants` walk — so what is on screen is what the
/// base believes by construction rather than by a comment claiming the two
/// agree.
pub(super) fn draw_work_orders(
    rows_in: &[WorkOrderRow],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(
        "Enter to queue an order, Backspace to drop one, Esc to close",
    )];
    if rows_in.is_empty() {
        rows.push(text_row("(nothing the base can make yet)"));
    }
    for (i, row) in rows_in.iter().enumerate() {
        let mut lines = work_order_lines(row, i, i == selected).into_iter();
        if let Some(head) = lines.next() {
            rows.push(item_row(head, i == selected));
        }
        rows.extend(lines.map(text_row));
    }
    draw_popup("Work Orders", PopupSize::Large, &rows, painter, m);
}

/// One work order's lines: the order itself, then a line per machine in the
/// chain — or the sentence naming why it is stalled.
///
/// A pure function of the row rather than pushed straight into the popup,
/// so a headless test can measure how wide the widest of them runs.
/// `draw_row` clamps a row vertically and never horizontally, so nothing
/// else would catch a line that runs off the body.
fn work_order_lines(row: &WorkOrderRow, index: usize, _selected: bool) -> Vec<String> {
    let Some(order) = &row.order else {
        return vec![format!("[{}] New work order...", menu_shortcut(index))];
    };
    let state = if order.stalled { "  STALLED" } else { "" };
    let mut lines = vec![format!(
        "[{}] {}  {}/{}{state}",
        menu_shortcut(index),
        order.label,
        order.have,
        order.target
    )];
    // Through `continuation_lines` rather than an inline `format!`: a
    // stalled order's reason is a whole sentence and a machine line carries
    // three names, and either can outrun the popup body.
    if let Some(why) = &order.blocked_by {
        lines.extend(continuation_lines(why));
        return lines;
    }
    if order.machines.is_empty() {
        lines.extend(continuation_lines("waiting — nothing to do here yet"));
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
        lines.extend(continuation_lines(&format!(
            "{} — {who}{short}",
            machine.label
        )));
    }
    lines
}

/// Picking what to order — `Game::orderable_items`, which asks the same
/// chain question the queue refuses on, so nothing here can be rejected.
pub(super) fn draw_work_order_pick(
    items: &[(ItemId, String)],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(
        "What should the base hold? (Esc to cancel; Up/Down + Enter also work)",
    )];
    if items.is_empty() {
        rows.push(text_row("(nothing the base can make — deploy a machine)"));
    }
    for (i, (_, name)) in items.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {name}", menu_shortcut(i)),
            i == selected,
        ));
    }
    draw_popup("New Work Order", PopupSize::Large, &rows, painter, m);
}

/// How many of it. The same two-page shape the compile flow uses.
pub(super) fn draw_work_order_quantity(
    game: &Game,
    item: Option<ItemId>,
    typed: &str,
    painter: &Painter,
    m: &Metrics,
) {
    let name = item
        .as_ref()
        .map(|i| game.item_name(i).to_string())
        .unwrap_or_default();
    let shown = if typed.is_empty() { "1" } else { typed };
    let rows = vec![
        text_row(format!("How many {name} should the base hold?")),
        text_row(format!("  {shown}")),
        text_row("Digits then Enter. Esc to go back."),
        text_row("An order is a target, not a batch — what you already hold counts."),
    ];
    draw_popup("Order Quantity", PopupSize::Small, &rows, painter, m);
}

/// Putting programs on the base staff and standing them down. One key,
/// because a row is on the staff or it isn't — but what it says is the wider
/// question of what the program is *doing*, since standing one down leaves it
/// idle rather than in your party (the Companions screen is where a party is
/// picked), and the two used to read identically.
pub(super) fn draw_base_staff(
    game: &mut Game,
    staff_rows: &[BaseStaffRow],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let pets = game.owned_pets();
    let rows = base_staff_menu_rows(staff_rows, &pets, selected);
    draw_popup("Base Staff", PopupSize::Large, &rows, painter, m);
}

/// What a program is worth at a post, as the staff row says it.
///
/// The class goes through `manifest::base_job_label` rather than a second
/// mapping of its own: that one is exhaustive on purpose so a sixth class
/// cannot ship without deciding what it does at a post, and a copy here
/// would be a way to dodge that.
fn work_summary(work: Option<WorkProfile>) -> String {
    let Some(work) = work else {
        // The species is not in the db, so there are no numbers to quote —
        // saying so beats printing the roster's defaults as if authored.
        return "species not loaded".to_string();
    };
    let job = work
        .class
        .map(base_job_label)
        .unwrap_or_else(|| "no base job".to_string());
    format!("Spd {} · Ana {} · {job}", work.speed, work.analysis)
}

/// The Base Staff popup's rows: a shortcut line naming the program and what
/// it brings to a post, and an indented line under it for what it is doing
/// now.
///
/// Two lines because one busts the row budget. The widest realistic row — a
/// Gold fused program of the longest species name, with a zone tag, the
/// work summary and the longest activity — is 106 characters against
/// `ROW_WRAP_COLUMNS`' 100. Measured against the real font that row does
/// still *fit* the reference 1440x900 geometry, but by about two characters
/// (1203px of 1243px including the draw prefix), and `draw_row` clamps a row
/// vertically and never horizontally, so nothing catches the row that
/// finally doesn't. The activity is the half that moved, because it is the
/// half you read second.
///
/// Split out of `draw_base_staff` so the layout is reachable without a
/// `Game`, which is what `every_base_staff_activity_stays_inside_the_scrollable_body`
/// and `the_widest_base_staff_row_stays_inside_the_popup` need.
pub(super) fn base_staff_menu_rows(
    staff_rows: &[BaseStaffRow],
    pets: &[PetInfo],
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![text_row(
        "Enter puts a program to work in the base, or stands it down. Esc to close.",
    )];
    if staff_rows.is_empty() {
        rows.push(text_row("(no compiled programs — beat one first)"));
    }
    for (i, row) in staff_rows.iter().enumerate() {
        let pet = pets.iter().find(|p| p.entity == row.program.entity);
        rows.push(with_icon(
            tier_row(
                format!(
                    "[{}] {} — {}",
                    menu_shortcut(i),
                    row.program.label,
                    work_summary(row.work)
                ),
                i == selected,
                pet.map(|p| p.fusions).unwrap_or(0),
                pet.map(|p| p.rarity).unwrap_or_default(),
            ),
            row.program.glyph,
            glyph_color(row.program.color),
        ));
        let side = if row.on_staff {
            format!("base, {}", row.doing)
        } else {
            row.doing.clone()
        };
        // Dim `Item`s that can never be selected, not `Row::Text`:
        // `popup_layout` ends the scrollable body at the *last* `Row::Item`,
        // so text sub-lines under the last program would be pinned into the
        // footer alongside this screen's legend and drawn detached at the
        // bottom. That is the bug `routines::description_row` exists for, in
        // the same shape. `continuation_lines` rather than one `format!` so
        // the indent is the shared one and a long activity wraps.
        rows.extend(
            continuation_lines(&side)
                .into_iter()
                .map(|line| colored_item_row(line, false, TEXT_DIM)),
        );
    }
    rows.push(text_row(
        "Base staff are posted automatically by your work orders.",
    ));
    rows
}

pub(super) fn draw_structure_menu(
    structures: &[EntityView],
    title: &str,
    prompt: &str,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(format!(
        "{prompt} (Esc to cancel; Up/Down + Enter also work)"
    ))];
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
                    "[{}] {} at ({}, {}){}{}",
                    menu_shortcut(i),
                    s.label,
                    s.pos.0,
                    s.pos.1,
                    durability,
                    assigned
                ),
                i == selected,
            ),
            s.glyph,
            glyph_color(s.color),
        ));
    }
    draw_popup(title, PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_remove_menu(
    structures: &[EntityView],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(
        "Demolish which structure? Removing Home destroys the whole base. (Esc to cancel; Up/Down + Enter also work)",
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
                    "[{}] {} at ({}, {}){}{}",
                    menu_shortcut(i),
                    s.label,
                    s.pos.0,
                    s.pos.1,
                    durability,
                    home_tag
                ),
                i == selected,
            ),
            s.glyph,
            glyph_color(s.color),
        ));
    }
    draw_popup("Demolish Structure", PopupSize::Large, &rows, painter, m);
}

/// The bracketed tier tag on an upgrade-menu row.
///
/// A structure sitting at its ceiling is still listed rather than filtered
/// out — see `App::upgradeable_structures` — so the row has to say why it
/// has stopped. Whether that is temporary is exactly the difference between
/// `ceiling` and `max_tier`: below `max_tier` the zone is what's holding it,
/// and one more breach frees the next tier. At `max_tier` it is simply
/// finished, and the plain tag it has always shown is right.
fn tier_tag(s: &EntityView) -> String {
    let tier = s.tier.unwrap_or(1);
    match (s.ceiling, s.max_tier) {
        (Some(ceiling), Some(max_tier)) if tier >= ceiling && ceiling < max_tier => {
            let next = tier + 1;
            format!("Mk{tier} — zone {next} unlocks Mk{next}")
        }
        _ => format!("Mk{tier}"),
    }
}

pub(super) fn draw_upgrade_menu(
    structures: &[EntityView],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(
        "Upgrade which structure? Each tier costs more and yields more. (Esc to cancel; Up/Down + Enter also work)",
    )];
    if structures.is_empty() {
        rows.push(text_row("(no upgradeable structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        rows.push(with_icon(
            item_row(
                format!(
                    "[{}] {} at ({}, {}) [{}]",
                    menu_shortcut(i),
                    s.label,
                    s.pos.0,
                    s.pos.1,
                    tier_tag(s),
                ),
                i == selected,
            ),
            s.glyph,
            glyph_color(s.color),
        ));
    }
    draw_popup("Upgrade Structure", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_remove_confirm(selected: usize, painter: &Painter, m: &Metrics) {
    let rows = vec![
        Row::TextColored(
            "Removing Home destroys every other structure in this base and refunds".to_string(),
            ORANGE,
        ),
        Row::TextColored(
            "30% of each one's materials. This can't be undone.".to_string(),
            ORANGE,
        ),
        text_row(""),
        item_row("[y] Yes, demolish everything", selected == 0),
        item_row("[n] No, cancel", selected == 1),
    ];
    draw_popup("Confirm Demolish Home", PopupSize::Small, &rows, painter, m);
}

pub(super) fn draw_symlink_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let status = game.player_status();
    let targets = game.symlink_targets();
    let mut rows = vec![text_row(
        "Use symlink to which structure? (Esc to cancel; Up/Down + Enter also work)",
    )];
    if targets.is_empty() {
        rows.push(text_row("(no symlink-capable structures deployed yet)"));
    }
    for (i, t) in targets.iter().enumerate() {
        let raw_cost = game.symlink_cost(t.entity).unwrap_or_default();
        let cost = cost_display(game, &raw_cost, &status.inventory);
        let durability = t
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        rows.push(with_icon(
            item_row(
                format!(
                    "[{}] {} at ({}, {}){} - {}",
                    menu_shortcut(i),
                    t.label,
                    t.pos.0,
                    t.pos.1,
                    durability,
                    cost.join(", ")
                ),
                i == selected,
            ),
            t.glyph,
            glyph_color(t.color),
        ));
    }
    draw_popup("Symlink", PopupSize::Large, &rows, painter, m);
}

/// The structure roster: everything standing in the zone and every program
/// posted to it.
///
/// Read-only, and the one screen that shows the base as a whole rather than
/// what happens to be within `MENU_SCAN_RADIUS` — see
/// `Game::structure_report`, which is also where the row order is decided so
/// that this draws it rather than inventing one.
///
/// An idle workable structure is drawn in yellow and says so in words: it is
/// the only thing on this screen the player can act on, and the point of
/// looking is usually to find it.
pub(super) fn draw_structures(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let report = game.structure_report();
    let assigned: usize = report.iter().map(|s| s.assignees.len()).sum();
    let idle = report
        .iter()
        .filter(|s| s.workable && s.assignees.is_empty())
        .count();
    let mut rows = vec![
        text_row(format!(
            "{} structure{}, {assigned} program{} assigned, {idle} idle",
            report.len(),
            if report.len() == 1 { "" } else { "s" },
            if assigned == 1 { "" } else { "s" },
        )),
        text_row(""),
    ];
    if report.is_empty() {
        rows.push(text_row("You have deployed nothing yet."));
    }
    for (i, s) in report.iter().enumerate() {
        rows.push(colored_item_row(
            structure_headline(s),
            i == selected,
            if structure_is_idle(s) { YELLOW } else { TEXT },
        ));
        // A structure's sub-lines are `Row::Item` (never selected) rather than
        // `Row::Text` so they sit inside the popup's scrollable body:
        // `popup_layout` ends that body at the *last* Item and pins whatever
        // follows it as a footer, which would otherwise leave the final
        // structure's assignees stuck on screen while the list scrolled past
        // them.
        for (line, color) in structure_detail_lines(s) {
            rows.push(colored_item_row(line, false, color));
        }
    }
    rows.push(text_row(""));
    // Enter is surface-only, matching `App::handle_structures_key` — the
    // roster still reads underground, so the hint has to stop advertising a
    // key that would only be refused down there.
    rows.push(text_row(if game.is_underground() {
        "Up/Down to scroll, Esc to close."
    } else {
        "Up/Down to scroll, Enter to staff, Esc to close."
    }));
    draw_popup("Structures", PopupSize::Large, &rows, painter, m);
}

/// A workable structure with nobody posted to it — the one thing on either
/// structure screen the player can immediately act on, which is why both
/// colour it yellow.
pub(super) fn structure_is_idle(s: &StructureReport) -> bool {
    s.workable && s.assignees.is_empty()
}

/// The one-line summary of a structure: what it is, where, how far, and how
/// battered. Shared with the inspector's single-structure sheet.
pub(super) fn structure_headline(s: &StructureReport) -> String {
    let tier = s.tier.map(|t| format!(" T{t}")).unwrap_or_default();
    let durability = s
        .durability
        .map(|(hp, max)| format!("  {hp}/{max} HP"))
        .unwrap_or_default();
    format!(
        "{}{tier}  ({}, {})  {}d{durability}",
        s.label, s.pos.0, s.pos.1, s.distance
    )
}

/// Everything under the headline — idleness, assignees, a stall, the two
/// buffers — as `(text, colour)` pairs.
///
/// Extracted rather than written twice: the `B` roster and the inspector's
/// sheet describe the same machine, and a detail screen that disagreed with
/// the roster about whether something was starved is exactly the drift
/// `CLAUDE.md` means by a mirror having to be a call. The two differ only in
/// which `Row` kind they wrap these in — the roster needs `Row::Item` so its
/// lines scroll, the sheet does not scroll at all.
pub(super) fn structure_detail_lines(s: &StructureReport) -> Vec<(String, Color)> {
    let mut lines = Vec::new();
    if structure_is_idle(s) {
        lines.push(("  idle — nobody assigned".to_string(), YELLOW));
    }
    for a in &s.assignees {
        lines.push((format!("  {}", assignee_line(a)), TEXT_DIM));
    }
    // A stall is drawn in yellow for the same reason an idle structure is:
    // it is a thing the player can walk over and fix.
    if let Some(line) = stall_line(s) {
        lines.push((format!("  {line}"), YELLOW));
    }
    if let Some(line) = buffer_line("in", &s.input, None) {
        lines.push((line, TEXT_DIM));
    }
    if let Some(line) = buffer_line("out", &s.output, Some(s.output_capacity)) {
        lines.push((line, TEXT_DIM));
    }
    lines
}

/// Why a machine is stalled, or `None` when it is running or is not a
/// machine at all. `Idle` says nothing here — the "nobody assigned" line
/// already above it is the same fact in better words.
fn stall_line(s: &StructureReport) -> Option<&'static str> {
    match s.status? {
        MachineStatus::Starved => Some("starved — nothing is feeding it"),
        MachineStatus::Clogged => Some("clogged — collect from it with c"),
        MachineStatus::Unstaffed => Some("no one at it — its program is away"),
        MachineStatus::Stranded => Some("cut off — its program can't reach it"),
        MachineStatus::Running | MachineStatus::Idle => None,
    }
}

/// One buffer as a line, or `None` when it is empty — a base of empty
/// buffers would otherwise double the length of this screen to say nothing.
fn buffer_line(label: &str, stock: &[(String, u32)], capacity: Option<u32>) -> Option<String> {
    if stock.is_empty() {
        return None;
    }
    let contents = stock
        .iter()
        .map(|(name, n)| format!("{n} {name}"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(match capacity {
        Some(cap) => {
            let used: u32 = stock.iter().map(|(_, n)| n).sum();
            format!("  {label}: {contents}  [{used}/{cap}]")
        }
        None => format!("  {label}: {contents}"),
    })
}

/// One assignee row: who it is, how it is holding up, what it is doing, and
/// how far into a cycle it is. A guard has no cycle to be partway through —
/// `systems::task_progress_system` ignores the kind entirely — so it gets no
/// progress figure rather than a permanent `0/0`.
///
/// The vitals are here because this is the only screen that can carry them
/// for a posted program: at its post it is not drawn on the map, and the
/// inspector names only what is drawn, so there is no way to open its own
/// manifest without first calling it off the job.
fn assignee_line(a: &Assignee) -> String {
    let who = format!("{}{}", a.label, assignee_vitals(a));
    match a.kind {
        TaskKind::GatherResource => format!("{who} — cronjob {}/{}", a.progress, a.required),
        TaskKind::Guard => format!("{who} — guarding"),
    }
}

/// `" Lv7 HP 18/22"`, or empty for anything missing the components. Matching
/// the party roster's `Lv{n} HP {a}/{b}` so one program reads the same on
/// both screens.
fn assignee_vitals(a: &Assignee) -> String {
    let level = a.level.map(|l| format!(" Lv{l}")).unwrap_or_default();
    let hp =
        a.hp.map(|(hp, max)| format!(" HP {hp}/{max}"))
            .unwrap_or_default();
    format!("{level}{hp}")
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn view(tier: u32, ceiling: u32, max_tier: u32) -> EntityView {
        EntityView {
            entity: Entity::PLACEHOLDER,
            pos: (0, 0),
            glyph: 'n',
            color: GlyphColor::White,
            label: "Mining Node".into(),
            is_player: false,
            is_tamed: false,
            is_companion: false,
            is_hostile: false,
            is_structure: true,
            is_home: false,
            tier: Some(tier),
            ceiling: Some(ceiling),
            max_tier: Some(max_tier),
            is_boss: false,
            can_work: false,
            can_trade: false,
            issues_contracts: false,
            structure_worker: None,
            worker_away_from_post: false,
            position_is_honest: true,
            structure_attended: false,
            output_stranded: false,
            hp_fraction: None,
            level: None,
            durability: None,
            fusions: 0,
            rarity: Rarity::Ordinary,
            machine_status: None,
            linked_edges: Vec::new(),
        }
    }

    fn row_text(row: &Row) -> &str {
        match row {
            Row::Text(t) | Row::TextColored(t, _) => t,
            Row::Item { text, .. } => text,
        }
    }

    /// The deploy prompt used to be a bare compass, which meant the one screen
    /// where a structure is actually placed was also the one that never said
    /// what was being placed. Both halves matter: the identity, and the keys
    /// that act on it.
    #[test]
    fn the_deploy_prompt_names_what_is_being_placed_and_still_says_how() {
        let rows = build_direction_rows(
            "Mining Node",
            "Extracts Core Fragments on a timer.",
            &["Core Fragment (5/12)".to_string()],
        );
        let text: Vec<&str> = rows.iter().map(row_text).collect();
        assert!(text.contains(&"Mining Node"), "{text:?}");
        assert!(
            text.contains(&"Extracts Core Fragments on a timer."),
            "{text:?}"
        );
        assert!(
            text.iter().any(|t| t.contains("Core Fragment (5/12)")),
            "the cost carries the have/need figures the refusal would otherwise be the first news of: {text:?}"
        );
        assert!(text.contains(&DIRECTION_PROMPT), "{text:?}");
    }

    #[test]
    fn a_tier_below_the_ceiling_reads_as_it_always_has() {
        assert_eq!(tier_tag(&view(2, 5, 5)), "Mk2");
    }

    #[test]
    fn a_tier_stopped_by_the_zone_says_which_zone_would_free_it() {
        assert_eq!(tier_tag(&view(1, 1, 5)), "Mk1 — zone 2 unlocks Mk2");
    }

    #[test]
    fn a_tier_stopped_by_the_defs_own_ceiling_says_nothing_about_zones() {
        assert_eq!(tier_tag(&view(5, 5, 5)), "Mk5");
    }

    fn assignee(kind: TaskKind) -> Assignee {
        Assignee {
            entity: Entity::PLACEHOLDER,
            label: "Sub-Process (Z9)".into(),
            kind,
            progress: 999,
            required: 999,
            level: Some(99),
            hp: Some((999, 999)),
        }
    }

    #[test]
    fn an_assignee_row_reads_the_way_the_party_roster_does() {
        assert_eq!(
            assignee_line(&assignee(TaskKind::GatherResource)),
            "Sub-Process (Z9) Lv99 HP 999/999 — cronjob 999/999"
        );
        assert_eq!(
            assignee_line(&assignee(TaskKind::Guard)),
            "Sub-Process (Z9) Lv99 HP 999/999 — guarding"
        );
    }

    /// A program without the components still gets a row rather than a line
    /// full of placeholder figures.
    #[test]
    fn an_assignee_with_no_stats_reads_as_it_did_before_the_vitals() {
        let bare = Assignee {
            level: None,
            hp: None,
            ..assignee(TaskKind::Guard)
        };
        assert_eq!(assignee_line(&bare), "Sub-Process (Z9) — guarding");
    }

    /// The structure sheet is a `PopupSize::Small`, which is half the window
    /// wide — and `draw_row` clamps rows vertically but never horizontally,
    /// so a line too long for the box runs off its edge rather than wrapping
    /// or being cut. Adding the vitals lengthened every assignee row, so the
    /// worst case is measured against the real box rather than eyeballed.
    #[test]
    fn the_longest_assignee_row_fits_the_structure_sheet() {
        let m = crate::text::ui_metrics(900.0);
        let line = format!("  {}", assignee_line(&assignee(TaskKind::GatherResource)));
        crate::paint::with_painter(|p| {
            let box_w = p.screen_w() * 0.5;
            let text_w = p.measure_ui_advance(&line, m.font_size);
            assert!(
                text_w + 2.0 * m.pad < box_w,
                "an assignee row is {text_w}px inside a {box_w}px sheet: {line:?}"
            );
        });
    }
}

#[cfg(test)]
mod work_order_tests {
    use super::*;
    use feral_processes_engine::items::ItemId;
    use feral_processes_engine::{WorkOrderMachine, WorkOrderReport};

    /// The widest row this screen can draw is a machine line carrying a
    /// machine name, a worker name and a shortfall — and the runner-up is a
    /// stalled order's whole sentence. `draw_row` clamps a row vertically
    /// and **never horizontally**, so a row past the popup body simply runs
    /// off it; two shipped screens already do that because nobody measured.
    #[test]
    fn no_work_order_row_runs_past_the_popup_body() {
        let report = WorkOrderReport {
            item: ItemId::from("routine_disk"),
            label: "Routine Disk".to_string(),
            have: 2,
            target: 30,
            stalled: false,
            blocked_by: None,
            machines: vec![WorkOrderMachine {
                entity: Entity::PLACEHOLDER,
                label: "Annealing Node".to_string(),
                worker: Some("Sub-Process Lv12".to_string()),
                short_of: Some("Blank Substrate".to_string()),
                depth: 2,
            }],
        };
        let stalled = WorkOrderReport {
            stalled: true,
            blocked_by: Some(
                "Nothing beside the Annealing Node is making Blank Substrate — a machine can \
                 only take what a neighbour has finished."
                    .to_string(),
            ),
            machines: Vec::new(),
            ..report.clone()
        };
        let rows = [
            WorkOrderRow {
                order: Some(report),
            },
            WorkOrderRow {
                order: Some(stalled),
            },
            WorkOrderRow { order: None },
        ];

        for row in &rows {
            for line in work_order_lines(row, 0, false) {
                assert!(
                    line.chars().count() <= ROW_WRAP_COLUMNS,
                    "a {} char row runs past the {ROW_WRAP_COLUMNS} column body: {line:?}",
                    line.chars().count()
                );
            }
        }
    }
}

#[cfg(test)]
mod base_staff_tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::species::AffinityClass;

    fn staff_row(
        label: &str,
        work: Option<WorkProfile>,
        doing: &str,
        on_staff: bool,
    ) -> BaseStaffRow {
        let mut program = super::tests::view(1, 1, 1);
        program.label = label.to_string();
        program.is_structure = false;
        program.is_tamed = true;
        BaseStaffRow {
            program,
            on_staff,
            doing: doing.to_string(),
            work,
        }
    }

    /// The widest row the shipped content can produce: a Gold, thrice-fused
    /// program of the longest species name carrying a zone tag, the widest
    /// work summary, and the longest activity — "guarding the Contract
    /// Broker" over the longest structure name in `assets/structures/`.
    fn widest_staff_row() -> BaseStaffRow {
        staff_row(
            "Gold Sub-Process Lv18 [z9]",
            Some(WorkProfile {
                speed: 14,
                analysis: 18,
                class: Some(AffinityClass::Leech),
            }),
            "guarding the Contract Broker",
            true,
        )
    }

    /// `popup_layout` ends the scrollable body at the *last* `Row::Item` and
    /// pins everything after it as a footer. This screen has a legend, so an
    /// activity emitted as `Row::Text` would put the last program's activity
    /// below the scroll indicator, detached from the program it describes —
    /// the bug the `every_*_stays_inside_the_scrollable_body` family in
    /// `popup.rs` guards for the routine and build pickers.
    ///
    /// Asserted on the row list rather than through `popup_layout` because
    /// the cut is a property of where the last item sits and nothing else:
    /// the footer is every row after it, at any window size.
    #[test]
    fn every_base_staff_activity_stays_inside_the_scrollable_body() {
        for n in 1..6 {
            for selected in [0, n - 1] {
                let staff: Vec<BaseStaffRow> = (0..n)
                    .map(|i| staff_row(&format!("Program {i}"), None, "idle", false))
                    .collect();
                let rows = base_staff_menu_rows(&staff, &[], selected);
                let last_item = rows
                    .iter()
                    .rposition(|r| matches!(r, Row::Item { .. }))
                    .expect("a program is an item row");
                assert_eq!(
                    rows.len() - last_item - 1,
                    1,
                    "with {n} programs the popup pinned {} rows below the list, \
                     not the single legend it is allowed — an activity is \
                     detached from the program it belongs to",
                    rows.len() - last_item - 1
                );
            }
        }
    }

    /// Nothing clamps a popup row horizontally, so a staff row wider than the
    /// Base Staff popup's body runs off its right edge and takes the work
    /// summary with it — which is the whole reason the row carries one.
    ///
    /// **Both budgets, because only one of them discriminates.** In pixels
    /// the widest row clears the reference geometry either way, so that half
    /// would pass just as happily with the activity folded back onto the
    /// shortcut line — it is here for `no_roster_row_overflows_its_popup`'s
    /// reason, to catch the day `ROW_WRAP_COLUMNS` stops being the right
    /// budget. The column count is the half that fails if the two lines are
    /// rejoined, and it is the budget the rest of the file is written
    /// against (see `no_work_order_row_runs_past_the_popup_body`).
    #[test]
    fn the_widest_base_staff_row_stays_inside_the_popup() {
        for row in base_staff_menu_rows(&[widest_staff_row()], &[], 0) {
            let text = match &row {
                Row::Text(t) | Row::TextColored(t, _) => t.clone(),
                Row::Item { text, .. } => text.clone(),
            };
            assert!(
                text.chars().count() <= ROW_WRAP_COLUMNS,
                "a {} char Base Staff row runs past the {ROW_WRAP_COLUMNS} \
                 column body: {text:?}",
                text.chars().count()
            );
        }
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            let rows = base_staff_menu_rows(&[widest_staff_row()], &[], 0);
            for row in &rows {
                let text = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t.clone(),
                    Row::Item { text, .. } => format!("     {text}"),
                };
                let drawn = p.measure_ui_advance(&text, m.font_size);
                assert!(
                    drawn <= room,
                    "the widest Base Staff row overflows its popup by {:.0}px \
                     ({drawn:.0} into {room:.0}):\n{text}",
                    drawn - room
                );
            }
        });
    }

    /// The three facts reach the row, and a species the db never loaded says
    /// so rather than quoting the roster's defaults as if someone authored
    /// them for it.
    #[test]
    fn a_staff_row_spells_out_the_work_profile() {
        let rows = base_staff_menu_rows(&[widest_staff_row()], &[], 0);
        let head = rows
            .iter()
            .find_map(|r| match r {
                Row::Item { text, .. } => Some(text.clone()),
                _ => None,
            })
            .expect("the program is an item row");
        assert!(head.contains("Spd 14"), "{head}");
        assert!(head.contains("Ana 18"), "{head}");
        assert!(
            head.contains("Leech"),
            "the class is the third fact that decides a posting: {head}"
        );

        let unknown = base_staff_menu_rows(&[staff_row("Modded", None, "idle", false)], &[], 0);
        let head = unknown
            .iter()
            .find_map(|r| match r {
                Row::Item { text, .. } => Some(text.clone()),
                _ => None,
            })
            .unwrap();
        assert!(head.contains("not loaded"), "{head}");
    }
}
