//! The character-creation wizard's screen — one popup per
//! `CreationStep`, drawn from `App::creation_rows`.
//!
//! **A placeholder.** Every step draws its rows as plain text so the
//! wizard is walkable and the refusal census has something to land on; the
//! per-step layout (a swatch strip on the Look step, a two-column table on
//! Points, the class blurb) is a later task's. What is already correct is
//! the seam: this file reads `App::creation_rows` and nothing else, so the
//! row shape it draws cannot drift from the row shape app-core dispatches
//! keys against.

use super::popup::{PopupSize, Row, draw_popup, item_row, text_row};
use super::*;
use feral_processes_app_core::{CreationRow, CreationStep};

pub(super) fn draw_create_character(
    app: &App,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let step = app.creation_step();
    let selected = app.menu_selected;
    let rows = app.creation_rows();
    let mut drawn: Vec<Row> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let line = row_line(row);
        // The Points step's cursor is a highlight and never a row shortcut
        // — `Mode::Transfer`'s rule, where a digit is a quantity. Every
        // other step numbers its rows.
        match step {
            CreationStep::Points | CreationStep::Name | CreationStep::Summary => {
                drawn.push(item_row(line, selected == i))
            }
            _ => drawn.push(item_row(
                format!("[{}] {line}", feral_processes_app_core::menu_shortcut(i)),
                selected == i,
            )),
        }
    }
    if drawn.is_empty() {
        // A step with nothing to offer — an empty `assets/classes/`, say —
        // still draws a row, or the popup would be a blank box the player
        // cannot tell from a broken screen.
        drawn.push(text_row("Nothing to choose here."));
    }
    drawn.push(text_row(""));
    drawn.push(text_row(footer(step)));
    let title = format!(
        "New Game — {} ({}/{})",
        step.title(),
        step.index() + 1,
        CreationStep::ALL.len()
    );
    draw_popup(&title, PopupSize::Large, &drawn, refusal, painter, m);
}

/// One row as a line of text. Exhaustive on `CreationRow`, `cell_mark`'s
/// rule: a new row kind must be given words rather than falling into a
/// blank line.
fn row_line(row: &CreationRow) -> String {
    match row {
        CreationRow::Difficulty { label, detail, .. } => format!("{label} - {detail}"),
        CreationRow::Class(class) => format!("{} - {} [{}]", class.name, class.axes, class.kit),
        CreationRow::Icon { glyph, sprite } => format!("{glyph}  ({sprite})"),
        CreationRow::Colour { index } => format!("Colour {}", index + 1),
        CreationRow::Stat {
            stat,
            spent,
            value,
            cost,
        } => format!("{:<12} {value:>4}   {spent} bought @ {cost}", stat.label()),
        CreationRow::Routine(routine) => {
            format!(
                "{} - {} ({:.0} Power)",
                routine.name, routine.effect, routine.power_cost
            )
        }
        CreationRow::Name { typed } => format!("Name: {typed}_"),
        CreationRow::Summary { label, value } => format!("{label:<12} {value}"),
    }
}

/// What each step's keys are, in one line under its rows.
fn footer(step: CreationStep) -> &'static str {
    match step {
        CreationStep::Difficulty => "Esc backs out to the menu",
        CreationStep::Class => "Up/Down + Enter; [R] rolls the rest; Esc goes back",
        CreationStep::Look => "Up/Down + Enter picks; [n] moves on; [R] rolls; Esc goes back",
        CreationStep::Points => {
            "Left/Right spends (Shift: all, Ctrl: half); Enter moves on; [R] rolls"
        }
        CreationStep::Routine => "Up/Down + Enter; [n] takes none; [R] rolls; Esc goes back",
        CreationStep::Name => "Type a name; Enter moves on; Esc goes back",
        CreationStep::Summary => "Enter starts the run; Esc goes back",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_app_core::{CREATION_COLOURS, GameKey};

    /// `CREATION_COLOURS` is app-core's count of a table only this crate
    /// holds, so nothing but this can hold the two in step. A wizard
    /// offering more swatches than the palette has draws nothing for the
    /// last of them, and one offering fewer makes a shipped colour
    /// unreachable.
    #[test]
    fn the_wizard_offers_every_shipped_swatch() {
        assert_eq!(
            CREATION_COLOURS as usize,
            hud::palette::PLAYER_CHOICES.len(),
            "the Look step and the palette disagree about how many colours there are"
        );
    }

    /// A fresh app on the main menu with a scratch profile — the wizard
    /// needs no run, which is the whole reason it holds its own catalogue.
    fn wizard_app() -> App {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tmp = std::env::temp_dir().join(format!("fp_gui_wizard_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let mut app = App::new(
            root.join("assets"),
            tmp.join("saves"),
            tmp.join("history.log"),
            tmp.join("profile.ron"),
            root.join("dev-arenas"),
            tmp.join("telemetry.jsonl"),
        );
        app.handle_key(GameKey::Char('n'));
        app
    }

    /// **The refusal census, turned ninety degrees.** `ALL_MODES` walks the
    /// wizard as one mode, so it only ever exercises whichever step the
    /// census app happens to have left the cursor on. Walking
    /// `CreationStep::ALL` is what says the other six can say why they
    /// refused something — a step drawing no popup at all would swallow its
    /// own refusal in silence.
    #[test]
    fn every_creation_step_draws_a_refusal_exactly_once() {
        const REFUSAL: &str = "Requires Zone 3 first.";
        let mut app = wizard_app();
        // The keys that walk one step forward from each of the first six.
        let forward = [
            GameKey::Char('f'),
            GameKey::Char('1'),
            GameKey::Char('n'),
            GameKey::Enter,
            GameKey::Char('n'),
            GameKey::Enter,
        ];
        let mut steps = Vec::new();
        for (i, step) in CreationStep::ALL.iter().enumerate() {
            assert_eq!(
                app.creation_step(),
                *step,
                "the walk fell out of step at {step:?}"
            );
            app.status_line = Some(REFUSAL.to_string());
            let m = ui_metrics(900.0);
            let (_, shapes) =
                crate::paint::with_painter(|p| draw_create_character(&app, Some(REFUSAL), p, &m));
            let drawn = crate::paint::painted_text(&shapes)
                .iter()
                .filter(|t| t.contains(REFUSAL))
                .count();
            assert_eq!(
                drawn, 1,
                "{step:?} painted the refusal {drawn} times, not once"
            );
            steps.push(*step);
            if let Some(key) = forward.get(i) {
                app.handle_key(*key);
            }
        }
        assert_eq!(steps.len(), CreationStep::ALL.len());
    }

    /// Every step draws at least one row of its own. A blank popup is
    /// indistinguishable from a broken screen, and against the real
    /// `assets/` an empty step would mean the class or routine catalogue
    /// silently failed to load.
    #[test]
    fn every_creation_step_draws_its_rows() {
        let mut app = wizard_app();
        let forward = [
            GameKey::Char('f'),
            GameKey::Char('1'),
            GameKey::Char('n'),
            GameKey::Enter,
            GameKey::Char('n'),
            GameKey::Enter,
        ];
        for (i, step) in CreationStep::ALL.iter().enumerate() {
            let m = ui_metrics(900.0);
            let (_, shapes) =
                crate::paint::with_painter(|p| draw_create_character(&app, None, p, &m));
            let drawn = crate::paint::painted_text(&shapes);
            assert!(
                drawn.iter().any(|t| t.contains(step.title())),
                "{step:?} did not draw its own heading: {drawn:?}"
            );
            assert!(
                drawn.len() > 2,
                "{step:?} drew nothing but chrome: {drawn:?}"
            );
            if let Some(key) = forward.get(i) {
                app.handle_key(*key);
            }
        }
    }
}
