//! The screens outside a run: main menu, save picker, difficulty, help,
//! and the game-over page.

use super::popup::*;
use super::*;

pub(super) fn draw_main_menu(app: &App, painter: &Painter, m: &Metrics) {
    let mut options = vec!["[N] New Game".to_string()];
    if !app.list_saves().is_empty() {
        options.push("[L] Load Game".to_string());
    }
    options.push("[A] Achievements".to_string());
    options.push("[Q] Quit".to_string());
    let mut rows = vec![
        Row::TextColored("feral-processes".to_string(), TEXT),
        Row::TextColored("// jack into the Grid".to_string(), CYAN),
        text_row(""),
    ];
    for (i, opt) in options.iter().enumerate() {
        rows.push(item_row(opt.clone(), i == app.menu_selected));
    }
    if let Some(s) = &app.status_line {
        rows.push(text_row(""));
        rows.push(Row::TextColored(s.clone(), RED));
    }
    draw_popup("Main Menu", PopupSize::Large, &rows, painter, m);
}

/// Every authored rung, earned or not — the point is showing what is left.
///
/// Rows come from `App::achievement_rows`, which is also what app-core counts
/// to bound the scroll. Rebuilding the list here instead would let the
/// highlight land on a row this never draws.
///
/// Two `Row::Item`s per rung would break that count, so each rung is one
/// selectable row with its description folded into the same line.
pub(super) fn draw_achievements(app: &App, painter: &Painter, m: &Metrics) {
    let entries = app.achievement_rows();
    let earned = entries.iter().filter(|r| r.earned.is_some()).count();
    let mut rows = vec![
        text_row(format!(
            "{earned} of {} earned. Rewards are paid at the start of your next run.",
            entries.len()
        )),
        text_row(""),
    ];
    for (i, entry) in entries.iter().enumerate() {
        let selected = i == app.menu_selected;
        match &entry.earned {
            Some(summary) => {
                let stat = summary
                    .rolled_stat
                    .as_deref()
                    .map(|s| format!(" -> {s}"))
                    .unwrap_or_default();
                let mode = if summary.permadeath {
                    " [permadeath]"
                } else {
                    ""
                };
                rows.push(colored_item_row(
                    format!(
                        "{} - {} - cycle {}{mode}{stat}",
                        entry.name, entry.reward, summary.tick
                    ),
                    selected,
                    GREEN,
                ));
            }
            None => rows.push(spent_item_row(
                format!("{} - {} - {}", entry.name, entry.reward, entry.description),
                selected,
            )),
        }
    }
    rows.push(text_row(""));
    rows.push(text_row("Up/Down to scroll, Esc to close."));
    draw_popup("Achievements", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_load_game(app: &App, painter: &Painter, m: &Metrics) {
    let saves = app.list_saves();
    let mut rows = vec![text_row(
        "Pick a save (Esc to cancel; Up/Down + Enter also work)",
    )];
    if saves.is_empty() {
        rows.push(text_row("(no saves found)"));
    }
    for (i, save) in saves.iter().enumerate() {
        let summary = save
            .summary
            .as_deref()
            .unwrap_or("(incompatible save - can still be deleted)");
        rows.push(item_row(
            format!("[{}] {} - {}", menu_shortcut(i), save.name, summary),
            i == app.menu_selected,
        ));
    }
    draw_popup("Load Game", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_save_action(app: &App, painter: &Painter, m: &Metrics) {
    let name = app
        .pending_save
        .as_ref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(unknown save)".to_string());
    let mut rows = vec![
        Row::TextColored(name, TEXT),
        text_row(""),
        item_row("[L]oad".to_string(), app.menu_selected == 0),
        item_row("[X] Delete".to_string(), app.menu_selected == 1),
        text_row(""),
        text_row("Esc to cancel; Up/Down + Enter also work"),
    ];
    if let Some(s) = &app.status_line {
        rows.push(text_row(""));
        rows.push(Row::TextColored(s.clone(), RED));
    }
    draw_popup("Save", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_difficulty_pick(selected: usize, painter: &Painter, m: &Metrics) {
    let rows = vec![
        item_row(
            "[P] Permadeath - flatlining is final; the session is archived to a log".to_string(),
            selected == 0,
        ),
        item_row(
            "[F] Forgiving - flatlining costs you, but you reboot and keep going".to_string(),
            selected == 1,
        ),
        text_row(""),
        text_row("Esc to go back; Up/Down + Enter also work"),
    ];
    draw_popup("New Game", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_game_over(app: &mut App, painter: &Painter, m: &Metrics) {
    let summary = app
        .game
        .as_mut()
        .and_then(|g| g.history_summary())
        .unwrap_or_else(|| "Connection lost.".to_string());
    let rows = vec![
        Row::TextColored("FLATLINE".to_string(), RED),
        text_row(""),
        text_row(summary),
        text_row(""),
        text_row("Press any key to return to the main menu"),
    ];
    draw_popup("Session Terminated", PopupSize::Large, &rows, painter, m);
}

/// Confirms abandoning the run. Spells out what leaving costs rather than
/// asking a bare "are you sure?" — the answer depends on how long ago the
/// last autosave was, which is not something the player can see.
pub(super) fn draw_quit_run_confirm(selected: usize, painter: &Painter, m: &Metrics) {
    let rows = vec![
        text_row("Leave this run?"),
        text_row(""),
        item_row("[S] Save and quit to the menu".to_string(), selected == 0),
        item_row("[Q] Quit without saving".to_string(), selected == 1),
        item_row("[N] Keep playing".to_string(), selected == 2),
        text_row(""),
        text_row("Quitting without saving drops progress since the last autosave."),
        text_row("Esc to cancel; Up/Down + Enter also work"),
    ];
    draw_popup("Quit to Menu", PopupSize::Large, &rows, painter, m);
}

/// Confirms ending the process from the main menu. No run is loaded, so
/// this guards a misaimed keypress rather than any progress.
pub(super) fn draw_quit_app_confirm(selected: usize, painter: &Painter, m: &Metrics) {
    let rows = vec![
        text_row("Close feral-processes?"),
        text_row(""),
        item_row("[Y] Yes, quit".to_string(), selected == 0),
        item_row("[N] No, stay".to_string(), selected == 1),
        text_row(""),
        text_row("Esc to cancel; Up/Down + Enter also work"),
    ];
    draw_popup("Quit", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_help(painter: &Painter, m: &Metrics) {
    let rows = vec![
        text_row("hjkl/arrows move   . wait   e drain   r recharge"),
        text_row(""),
        text_row("b base menu       deploy, compile, cronjobs, work it yourself,"),
        text_row("                  guard, upgrade, demolish, roster, research"),
        text_row("p party menu      companions, manifests, fuse, install and"),
        text_row("                  extract routines, perks"),
        text_row("i pack            your inventory"),
        text_row(""),
        text_row("A menu lists only what you can actually do from where you"),
        text_row("stand — a row you can't see leads to an empty screen."),
        text_row("Esc backs out one level; finishing a job returns to the map."),
        text_row(""),
        text_row("C collect from adjacent structures   t trade   a routine"),
        text_row("u symlink   x examine a direction"),
        text_row("L history   F filter the log (all/field/base)"),
        text_row("s save   q main menu (confirms first)"),
        text_row("+/- zoom   [/] volume   \\ visual effects"),
        text_row(""),
        text_row("Every numbered menu also takes Up/Down + Enter, on top of"),
        text_row("typing a row's own number/letter directly."),
        text_row(""),
        text_row("On the companions screen, < and > move the highlighted member"),
        text_row("along the battle line — the front slot draws the most fire."),
        text_row(""),
        text_row("Trading:          S sell one, B buy one from the highlighted row"),
        text_row("                  (no quantity page). S works in your pack too."),
        text_row(""),
        text_row("In the Stack:     hjkl/arrows  forward, back, turn left, turn right"),
        text_row("                  > descend   < climb / leave the link"),
        text_row("                  o adopt an orphaned process (costs a catalyst)"),
        text_row("                  g map — only what you have seen"),
        text_row("                  +/- zoom the corner map, whole frame to close in"),
        text_row(""),
        text_row("In an intrusion:  a attack   d defend   s special"),
        text_row("                  u use item   j jack out"),
        text_row("                  A all attack   D all defend (shift = the whole party)"),
        text_row(""),
        text_row("Press any key to close"),
    ];
    draw_popup("Help", PopupSize::Large, &rows, painter, m);
}
