//! The screens outside a run: main menu, save picker, difficulty, help,
//! and the game-over page.

use super::popup::*;
use super::*;

pub(super) fn draw_main_menu(app: &App, painter: &Painter, m: &Metrics) {
    let options = main_menu_options(!app.list_saves().is_empty(), app.arena_enabled());
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

/// The main menu's rows, in the order `App::handle_main_menu_key` builds
/// its key list. Both clauses are conditional there and both are here — a
/// row drawn that the handler does not offer opens the screen below it.
///
/// The Arena row is a dev switch (`FERAL_DEV_ARENA`), so an ordinary player
/// never sees it and a release build costs nothing for it.
fn main_menu_options(has_saves: bool, arena: bool) -> Vec<String> {
    let mut options = vec!["[N] New Game".to_string()];
    if has_saves {
        options.push("[L] Load Game".to_string());
    }
    options.push("[A] Achievements".to_string());
    if arena {
        options.push("[R] Arena".to_string());
    }
    options.push("[Q] Quit".to_string());
    options
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

/// Every row of the help screen, in order.
///
/// Split out from `draw_help` so the easter-egg census below can read what
/// this screen actually says. `draw_help` is the *only* screen in the game
/// that lists key bindings — everything else a player reads about battle
/// keys comes from `Game::battle_action_options` and
/// `Game::battle_party_commands`, which are engine data with their own
/// guard.
pub(super) const HELP_ROWS: &[&str] = &[
    "hjkl/arrows move   . wait   e drain   r recharge",
    "",
    "b base menu       deploy, compile, cronjobs, work it yourself,",
    "                  guard, upgrade, demolish, roster, research",
    "p party menu      companions, manifests, fuse, install and",
    "                  extract routines, perks",
    "i pack            your inventory",
    "",
    "A menu lists only what you can actually do from where you",
    "stand — a row you can't see leads to an empty screen.",
    "Esc backs out one level; finishing a job returns to the map.",
    "",
    "c collect from adjacent structures   t trade   a routine",
    "u symlink   x examine a direction   d demolish a direction",
    "v lay a VectorStasis Tile on the cell you're standing on",
    "m Excavation plan   space anchors a box, space again marks it,",
    "                  Esc backs out. Marked rock is cut and floored;",
    "                  drawing a plan costs no time.",
    "> phase into the base, on the anchor   < phase back out, at the exit",
    "L history   f filter the log (all/field/base)",
    "s save   q main menu (confirms first)",
    "+/- zoom   [/] volume   \\ visual effects",
    "",
    "Every numbered menu also takes Up/Down + Enter, on top of",
    "typing a row's own number/letter directly.",
    "",
    "On the companions screen, < and > move the highlighted member",
    "along the battle line — the front slot draws the most fire.",
    "",
    "Routines:         a slot takes an etched disk, never a routine you",
    "                  merely know. On the install page, e burns a blank",
    "                  with something you know to make one.",
    "",
    "Trading:          S sell one, B buy one from the highlighted row",
    "                  (no quantity page). S works in your pack too.",
    "",
    "In the Stack:     hjkl/arrows  forward, back, turn left, turn right",
    "                  > descend   < climb / leave the link — the same",
    "                  pair the anchor uses, on the surface and the base",
    "                  o adopt an orphaned process (costs a catalyst)",
    "                  t trade, if somebody is selling on this cell —",
    "                  they keep no buyback, and the shelf does not refill",
    "                  g map — only what you have seen",
    "                  +/- zoom the corner map, whole frame to close in",
    "",
    "In an intrusion:  a attack   d defend   s special",
    "                  u use item   j jack out",
    "                  A all attack   D all defend (shift = the whole party)",
    "                  Up/Down scroll the narration, on the results page too",
    "",
    "Press any key to close",
];

pub(super) fn draw_help(painter: &Painter, m: &Metrics) {
    let rows: Vec<Row> = HELP_ROWS.iter().map(|line| text_row(*line)).collect();
    draw_popup("Help", PopupSize::Large, &rows, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The easter-egg census, gui half. `draw_help` is the one screen in
    /// the game that lists key bindings, so it is the one place a
    /// well-meaning edit would give the hidden keys away — and the person
    /// making that edit has no way to know they are breaking a feature.
    /// `crates/engine/EASTER_EGGS.md` is the note that tells them; this is
    /// what catches them if they never read it.
    ///
    /// Asserted on whitespace-delimited **tokens**, never on substrings.
    /// `key name` is the binding idiom this screen uses (`s save`,
    /// `L history`, `A all attack`), so a token match catches a real
    /// documentation of a key while staying satisfiable: the rows are full
    /// of these letters inside ordinary words, and of the lowercase `t`
    /// that legitimately binds trade.
    #[test]
    fn the_main_menu_shows_arena_only_when_enabled() {
        let named = |opts: &[String]| opts.iter().any(|o| o.contains("Arena"));
        assert!(!named(&main_menu_options(true, false)));
        assert!(!named(&main_menu_options(false, false)));
        assert!(named(&main_menu_options(false, true)));
        // And it sits between Achievements and Quit, which is where
        // `handle_main_menu_key` puts the `r` in its own list.
        let opts = main_menu_options(true, true);
        let at = |s: &str| opts.iter().position(|o| o.contains(s)).unwrap();
        assert!(at("Achievements") < at("Arena"));
        assert!(at("Arena") < at("Quit"));
    }

    /// The Excavation plan is the one verb in the game with no engine-side
    /// refusal to teach it: `m` opens a mode, and a mode nobody can find is
    /// a feature nobody has. So the help screen is where it is learned.
    #[test]
    fn the_help_screen_names_the_excavation_plan_key() {
        let says = |what: &str| HELP_ROWS.iter().any(|row| row.contains(what));
        assert!(says("m "), "the help screen never names the m key");
        assert!(
            says("Excavation"),
            "the help screen names no Excavation plan"
        );
    }

    #[test]
    fn the_help_screen_never_names_a_hidden_key() {
        for row in HELP_ROWS {
            for token in row.split_whitespace() {
                assert!(
                    !matches!(token, "W" | "T" | "Z"),
                    "the help screen names a hidden key: {row:?}"
                );
            }
        }
    }
}
