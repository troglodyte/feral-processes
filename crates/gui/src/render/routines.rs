//! The routine panel and the extraction flow.

use super::popup::*;
use super::*;

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

pub(super) fn draw_routines(
    game: &Game,
    holder: Option<Entity>,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(holder) = holder else { return };
    let slots = game.routine_view(holder);
    let mut rows = vec![text_row(
        "Pick a filled slot to clear it — the disk is already spent — or an empty one to install.",
    )];
    for (i, s) in slots.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {}", menu_shortcut(i), s.name),
            i == selected,
        ));
        if !s.description.is_empty() {
            rows.push(text_row(format!("    {}", s.description)));
        }
    }
    draw_popup("Routines", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_routine_install(game: &Game, selected: usize, painter: &Painter, m: &Metrics) {
    let known = game.installable_routines();
    let mut rows = vec![
        text_row("Install which routine? Writing one burns a blank Routine Disk."),
        text_row(format!("Disks: {}", game.routine_disks_held())),
    ];
    if known.is_empty() {
        rows.push(text_row(
            "(you know no routines — research one, or extract one from a program)",
        ));
    }
    for (i, r) in known.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {}", menu_shortcut(i), r.name),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", r.description)));
    }
    draw_popup("Install Routine", PopupSize::Large, &rows, painter, m);
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
            fusion_row(
                format!(
                    "[{}] {} Lv{}{}",
                    menu_shortcut(i),
                    p.name,
                    p.level,
                    fusion_tag(p.fusions)
                ),
                i == selected,
                p.fusions,
            ),
            p.glyph,
            glyph_color(p.color),
        ));
    }
    draw_popup("Extract", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_extract_pick(
    game: &Game,
    program: Option<Entity>,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(program) = program else { return };
    let offered = game.extractable_routines(program);
    let mut rows = vec![text_row("Learn which routine? The rest are lost with it.")];
    for (i, a) in offered.iter().enumerate() {
        let known = if a.known { " (already known)" } else { "" };
        rows.push(item_row(
            format!("[{}] {}{known}", menu_shortcut(i), a.name),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", a.description)));
    }
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
