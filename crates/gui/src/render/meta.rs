//! The screens outside a run: main menu, save picker, difficulty, help,
//! and the game-over page.

use super::popup::*;
use super::*;

pub(super) fn draw_main_menu(app: &App, refusal: Option<&str>, painter: &Painter, m: &Metrics) {
    let options = main_menu_options(
        !app.list_saves().is_empty(),
        app.arena_enabled(),
        app.sprite_forge_enabled(),
    );
    let mut rows = vec![
        Row::TextColored("feral-processes".to_string(), TEXT),
        Row::TextColored("// jack into the Grid".to_string(), CYAN),
        text_row(""),
    ];
    for (i, opt) in options.iter().enumerate() {
        rows.push(item_row(opt.clone(), i == app.menu_selected));
    }
    draw_popup("Main Menu", PopupSize::Large, &rows, refusal, painter, m);
}

/// The main menu's rows, in the order `App::handle_main_menu_key` builds
/// its key list. Both clauses are conditional there and both are here — a
/// row drawn that the handler does not offer opens the screen below it.
///
/// The Arena and Sprite Forge rows are both dev switches (`FERAL_DEV_ARENA`,
/// `FERAL_DEV_SPRITES`), so an ordinary player never sees either and a
/// release build costs nothing for them.
fn main_menu_options(has_saves: bool, arena: bool, sprite_forge: bool) -> Vec<String> {
    let mut options = vec!["[N] New Game".to_string()];
    if has_saves {
        options.push("[L] Load Game".to_string());
    }
    options.push("[A] Achievements".to_string());
    options.push("[O] Options".to_string());
    if arena {
        options.push("[R] Arena".to_string());
    }
    if sprite_forge {
        options.push("[D] Sprite Forge".to_string());
    }
    options.push("[Q] Quit".to_string());
    options
}

/// The settings screen. Rows come from `App::option_rows`, which is also
/// what app-core scrolls, so the highlight cannot land on a row this never
/// draws — `draw_achievements`' rule.
///
/// The description is **hand-wrapped into short lines**, not one long
/// sentence: `wrapped_row_lines` breaks tag segments and nothing else, so a
/// prose row wider than the body is simply drawn off the edge.
pub(super) fn draw_options(app: &App, refusal: Option<&str>, painter: &Painter, m: &Metrics) {
    let mut rows = vec![text_row("")];
    for (i, row) in app.option_rows().iter().enumerate() {
        rows.push(item_row(
            format!("{}: {}", row.label, row.value),
            i == app.menu_selected,
        ));
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "Surface packs and town patrols fight on a generated grid,",
    ));
    rows.push(text_row(
        "one body at a time. Nests, lairs, Entropy Sweeps and the",
    ));
    rows.push(text_row("Stack keep the abstract model."));
    rows.push(text_row(""));
    rows.push(text_row("Enter toggles, Esc to close."));
    draw_popup("Options", PopupSize::Large, &rows, refusal, painter, m);
}

/// Every authored rung, earned or not — the point is showing what is left.
///
/// Rows come from `App::achievement_rows`, which is also what app-core counts
/// to bound the scroll. Rebuilding the list here instead would let the
/// highlight land on a row this never draws.
///
/// Two `Row::Item`s per rung would break that count, so each rung is one
/// selectable row with its description folded into the same line.
pub(super) fn draw_achievements(app: &App, refusal: Option<&str>, painter: &Painter, m: &Metrics) {
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
    draw_popup("Achievements", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_load_game(app: &App, refusal: Option<&str>, painter: &Painter, m: &Metrics) {
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
    draw_popup("Load Game", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_save_action(app: &App, refusal: Option<&str>, painter: &Painter, m: &Metrics) {
    let name = app
        .pending_save
        .as_ref()
        .and_then(|p| p.file_stem())
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "(unknown save)".to_string());
    let rows = vec![
        Row::TextColored(name, TEXT),
        text_row(""),
        item_row("[L]oad".to_string(), app.menu_selected == 0),
        item_row("[X] Delete".to_string(), app.menu_selected == 1),
        text_row(""),
        text_row("Esc to cancel; Up/Down + Enter also work"),
    ];
    draw_popup("Save", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_game_over(app: &mut App, refusal: Option<&str>, painter: &Painter, m: &Metrics) {
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
    draw_popup(
        "Session Terminated",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// Confirms abandoning the run. Spells out what leaving costs rather than
/// asking a bare "are you sure?" — the answer depends on how long ago the
/// last autosave was, which is not something the player can see.
pub(super) fn draw_quit_run_confirm(
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
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
    draw_popup("Quit to Menu", PopupSize::Large, &rows, refusal, painter, m);
}

/// Confirms ending the process from the main menu. No run is loaded, so
/// this guards a misaimed keypress rather than any progress.
pub(super) fn draw_quit_app_confirm(
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = vec![
        text_row("Close feral-processes?"),
        text_row(""),
        item_row("[Y] Yes, quit".to_string(), selected == 0),
        item_row("[N] No, stay".to_string(), selected == 1),
        text_row(""),
        text_row("Esc to cancel; Up/Down + Enter also work"),
    ];
    draw_popup("Quit", PopupSize::Large, &rows, refusal, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_menu_shows_arena_only_when_enabled() {
        let named = |opts: &[String]| opts.iter().any(|o| o.contains("Arena"));
        assert!(!named(&main_menu_options(true, false, false)));
        assert!(!named(&main_menu_options(false, false, false)));
        assert!(named(&main_menu_options(false, true, false)));
        // And it sits between Achievements and Quit, which is where
        // `handle_main_menu_key` puts the `r` in its own list.
        let opts = main_menu_options(true, true, false);
        let at = |s: &str| opts.iter().position(|o| o.contains(s)).unwrap();
        assert!(at("Achievements") < at("Arena"));
        assert!(at("Arena") < at("Quit"));
    }

    #[test]
    fn the_main_menu_shows_sprite_forge_only_when_enabled() {
        let named = |opts: &[String]| opts.iter().any(|o| o.contains("Sprite Forge"));
        assert!(!named(&main_menu_options(true, false, false)));
        assert!(!named(&main_menu_options(false, false, false)));
        assert!(named(&main_menu_options(false, false, true)));
        // Beside Arena, both above Quit — `handle_main_menu_key` pushes `d`
        // right after `r` and before `q`.
        let opts = main_menu_options(true, true, true);
        let at = |s: &str| opts.iter().position(|o| o.contains(s)).unwrap();
        assert!(at("Arena") < at("Sprite Forge"));
        assert!(at("Sprite Forge") < at("Quit"));
    }

    /// The menu has to offer the row, not just the screen exist — a row the
    /// handler does not offer opens whatever screen is underneath it, and
    /// the note on `main_menu_options` says so. `[O]` sits between
    /// Achievements and the two dev switches, which is where
    /// `handle_main_menu_key` pushes its `o`.
    #[test]
    fn the_main_menu_offers_the_options_row() {
        let opts = main_menu_options(true, true, true);
        let at = |s: &str| opts.iter().position(|o| o.contains(s)).unwrap();
        assert!(
            opts.iter().any(|row| row.contains("[O]")),
            "the main menu does not offer the options row: {opts:#?}"
        );
        assert!(at("Achievements") < at("Options"));
        assert!(at("Options") < at("Arena"));
    }
}
