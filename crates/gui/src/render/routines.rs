//! The routine panel and the extraction flow.

use super::popup::*;
use super::*;
use feral_processes_engine::views::{
    EtchedDiskView, ExtractableRoutineView, KnownRoutineView, RoutineSlotView,
};

pub(super) fn draw_routine_target(
    game: &mut Game,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let holders = game.routine_holders();
    let mut rows = vec![text_row("Whose routines?")];
    for (i, h) in holders.iter().enumerate() {
        rows.push(with_icon(
            item_row(
                format!(
                    "[{}] {} Lv{} - {}/{} slots",
                    menu_shortcut(i),
                    h.name,
                    h.level,
                    h.filled,
                    h.slots
                ),
                i == selected,
            ),
            h.glyph,
            glyph_color(h.color),
        ));
    }
    draw_popup("Routines", PopupSize::Large, &rows, painter, m);
}

/// A routine's own text, indented under the row it belongs to.
///
/// **A `Row::Item`, never a `Row::Text`**, and every picker in this file owes
/// its descriptions to this. `popup_layout` ends the scrollable body at the
/// *last* `Row::Item` and pins whatever follows as a footer — so the final
/// routine's description, emitted as text, was drawn detached at the bottom
/// of the popup under the scroll indicator while every other description sat
/// inline under its own row. It read as missing. `build_menu_rows`
/// (`render/building.rs`) carries the same fix for the same reason.
fn description_row(description: &str) -> Row {
    colored_item_row(format!("    {description}"), false, TEXT_DIM)
}

/// The slot panel's rows, pure so the layout invariant above can be tested
/// without a `Game` or a `Painter`.
pub(super) fn routine_slot_rows(slots: &[RoutineSlotView], selected: usize) -> Vec<Row> {
    let mut rows = vec![text_row(
        "Pick a filled slot to clear it — the disk is already spent — or an empty one to install.",
    )];
    for (i, s) in slots.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {}", menu_shortcut(i), s.name),
            i == selected,
        ));
        if !s.description.is_empty() {
            rows.push(description_row(&s.description));
        }
    }
    rows
}

pub(super) fn draw_routines(
    game: &Game,
    holder: Option<Entity>,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(holder) = holder else { return };
    let rows = routine_slot_rows(&game.routine_view(holder), selected);
    draw_popup("Routines", PopupSize::Large, &rows, painter, m);
}

/// The install picker's rows. The two trailing rows are a deliberate footer —
/// a legend pinned below the list rather than scrolling with it — which is
/// the one thing `popup_layout` is allowed to hold back here.
pub(super) fn routine_install_rows(
    disks: &[EtchedDiskView],
    blanks: u32,
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![text_row(
        "Install which disk? Installing spends it — popping the routine back out later returns nothing.",
    )];
    if disks.is_empty() {
        rows.push(text_row(
            "(you are carrying no etched disks — press [e] to burn one, or find one on a boss)",
        ));
    }
    for (i, d) in disks.iter().enumerate() {
        // Exclusive disks are marked because they are the ones a wrong
        // choice cannot be undone on: an ordinary routine can be etched
        // again for a blank, and this one cannot be etched at all.
        let tag = if d.exclusive { "  ★ exclusive" } else { "" };
        rows.push(item_row(
            format!("[{}] {} ×{}{}", menu_shortcut(i), d.name, d.qty, tag),
            i == selected,
        ));
        rows.push(description_row(&d.description));
    }
    rows.push(text_row(""));
    rows.push(text_row(format!(
        "[e] burn a blank with a routine you know    Blanks: {blanks}"
    )));
    rows
}

pub(super) fn draw_routine_install(game: &Game, selected: usize, painter: &Painter, m: &Metrics) {
    let rows = routine_install_rows(&game.etched_disks_held(), game.blank_disks_held(), selected);
    draw_popup("Install Routine", PopupSize::Large, &rows, painter, m);
}

/// The etch picker's rows.
///
/// A routine already in cargo carries a dim `×N held` note: this screen is
/// where a blank is spent for good, and the question it has to answer before
/// that is whether the player is starting a stock or adding to one they
/// forgot they had. Nothing at all on a routine held zero of — a `×0` on
/// every line is the noise the annotation exists to avoid.
pub(super) fn routine_etch_rows(
    known: &[KnownRoutineView],
    blanks: u32,
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![
        text_row("Burn which routine onto a blank? The blank is gone either way."),
        text_row(format!("Blanks: {blanks}")),
    ];
    if known.is_empty() {
        rows.push(text_row(
            "(you know no routines — research one, or extract one from a program)",
        ));
    }
    for (i, r) in known.iter().enumerate() {
        rows.push(annotated_item_row(
            format!("[{}] {}", menu_shortcut(i), r.name),
            (r.held > 0).then(|| format!("×{} held", r.held)),
            i == selected,
            TEXT,
        ));
        rows.push(description_row(&r.description));
    }
    rows
}

pub(super) fn draw_routine_etch(game: &Game, selected: usize, painter: &Painter, m: &Metrics) {
    let rows = routine_etch_rows(&game.etchable_routines(), game.blank_disks_held(), selected);
    draw_popup("Etch Disk", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_extract(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let programs = game.owned_pets();
    let mut rows = vec![text_row(
        "Break down which program? Extraction destroys it and teaches you one of its routines.",
    )];
    if !game.can_extract_routines() {
        rows.push(text_row("(you need a Compiler standing somewhere first)"));
    }
    for (i, p) in programs.iter().enumerate() {
        rows.push(with_icon(
            tier_row(
                format!(
                    "[{}] {} Lv{}{}",
                    menu_shortcut(i),
                    p.name,
                    p.level,
                    fusion_tag(p.fusions)
                ),
                i == selected,
                p.fusions,
                p.rarity,
            ),
            p.glyph,
            glyph_color(p.color),
        ));
    }
    draw_popup("Extract", PopupSize::Large, &rows, painter, m);
}

pub(super) fn extract_pick_rows(offered: &[ExtractableRoutineView], selected: usize) -> Vec<Row> {
    let mut rows = vec![text_row("Learn which routine? The rest are lost with it.")];
    for (i, a) in offered.iter().enumerate() {
        let known = if a.known { " (already known)" } else { "" };
        rows.push(item_row(
            format!("[{}] {}{known}", menu_shortcut(i), a.name),
            i == selected,
        ));
        rows.push(description_row(&a.description));
    }
    rows
}

pub(super) fn draw_extract_pick(
    game: &Game,
    program: Option<Entity>,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(program) = program else { return };
    let rows = extract_pick_rows(&game.extractable_routines(program), selected);
    draw_popup("Extract", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_extract_confirm(
    game: &Game,
    program: Option<Entity>,
    index: Option<usize>,
    painter: &Painter,
    m: &Metrics,
) {
    let (Some(program), Some(index)) = (program, index) else {
        return;
    };
    let offered = game.extractable_routines(program);
    let Some(kept) = offered.get(index) else {
        return;
    };
    let lost: Vec<&str> = offered
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, a)| a.name.as_str())
        .collect();
    let mut rows = vec![
        text_row(format!("Learn {} and destroy the program?", kept.name)),
        text_row(""),
    ];
    if !lost.is_empty() {
        rows.push(text_row(format!("This loses: {}.", lost.join(", "))));
    }
    rows.push(text_row("Enter to confirm, Esc to back out."));
    draw_popup("Extract", PopupSize::Large, &rows, painter, m);
}
