//! The build, staffing, demolition, upgrade and symlink pickers.

use super::popup::*;
use super::*;

pub(super) fn draw_build_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let status = game.player_status();
    let defs = game.buildable_structure_defs();
    let descriptions: Vec<String> = defs.iter().map(|def| def.description.clone()).collect();
    let mut rows = vec![
        text_row("Esc to cancel; Up/Down + Enter also work"),
        text_row(""),
    ];
    for (i, def) in defs.iter().enumerate() {
        let raw_cost = game.structure_build_cost(def);
        let cost = cost_display(game, &raw_cost, &status.inventory);
        rows.push(item_row(
            format!("[{}] {} - {}", menu_shortcut(i), def.name, cost.join(", ")),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", descriptions[i])));
    }
    draw_popup("Deploy", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_worker_menu(
    game: &mut Game,
    title: &str,
    prompt: &str,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let workers: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_tamed)
        .collect();
    // `view_entities` doesn't carry a raw power number, only a level and
    // an HP fraction — cross-reference `owned_pets` for it, same as the
    // fuse menu does.
    let pets = game.owned_pets();
    let mut rows = vec![text_row(format!(
        "{prompt} (Esc to cancel; Up/Down + Enter also work)"
    ))];
    if workers.is_empty() {
        rows.push(text_row("(no compiled programs nearby)"));
    }
    for (i, w) in workers.iter().enumerate() {
        let pet = pets.iter().find(|p| p.entity == w.entity);
        let power = pet.map(|p| format!(" PWR {}", p.power)).unwrap_or_default();
        let activity = pet.map(|p| activity_tag(&p.activity)).unwrap_or_default();
        rows.push(item_row(
            format!(
                "[{}] {}{}{} at ({}, {}){}",
                menu_shortcut(i),
                w.label,
                w.level.map(|l| format!(" Lv{l}")).unwrap_or_default(),
                power,
                w.pos.0,
                w.pos.1,
                activity
            ),
            i == selected,
        ));
    }
    draw_popup(title, PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_structure_menu(
    game: &mut Game,
    title: &str,
    prompt: &str,
    workable_only: bool,
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| {
            if workable_only {
                e.can_work
            } else {
                e.is_structure
            }
        })
        .collect();
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
        rows.push(item_row(
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
        ));
    }
    draw_popup(title, PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_remove_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_structure)
        .collect();
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
        rows.push(item_row(
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
        ));
    }
    draw_popup("Demolish Structure", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_upgrade_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let structures: Vec<_> = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.is_structure && e.tier.is_some())
        .collect();
    let mut rows = vec![text_row(
        "Upgrade which structure? Each tier costs more and yields more. (Esc to cancel; Up/Down + Enter also work)",
    )];
    if structures.is_empty() {
        rows.push(text_row("(no upgradeable structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        rows.push(item_row(
            format!(
                "[{}] {} at ({}, {}) [Mk{}]",
                menu_shortcut(i),
                s.label,
                s.pos.0,
                s.pos.1,
                s.tier.unwrap_or(1),
            ),
            i == selected,
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
        rows.push(item_row(
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
        let tier = s.tier.map(|t| format!(" T{t}")).unwrap_or_default();
        let durability = s
            .durability
            .map(|(hp, max)| format!("  {hp}/{max} HP"))
            .unwrap_or_default();
        let is_idle = s.workable && s.assignees.is_empty();
        let color = if is_idle { YELLOW } else { TEXT };
        rows.push(colored_item_row(
            format!(
                "{}{tier}  ({}, {})  {}d{durability}",
                s.label, s.pos.0, s.pos.1, s.distance
            ),
            i == selected,
            color,
        ));
        if is_idle {
            rows.push(Row::TextColored(
                "    idle — nobody assigned".into(),
                YELLOW,
            ));
        }
        for a in &s.assignees {
            rows.push(text_row(format!("    {}", assignee_line(a))));
        }
    }
    rows.push(text_row(""));
    rows.push(text_row("Up/Down to scroll, Esc to close."));
    draw_popup("Structures", PopupSize::Large, &rows, painter, m);
}

/// One assignee row: who it is, what it is doing, and how far into a cycle it
/// is. A guard has no cycle to be partway through —
/// `systems::task_progress_system` ignores the kind entirely — so it gets no
/// progress figure rather than a permanent `0/0`.
fn assignee_line(a: &Assignee) -> String {
    match a.kind {
        TaskKind::GatherResource => {
            format!("{} — cronjob {}/{}", a.label, a.progress, a.required)
        }
        TaskKind::Guard => format!("{} — guarding", a.label),
    }
}
