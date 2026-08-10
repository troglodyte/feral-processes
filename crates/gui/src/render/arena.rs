//! The dev arena's screens: the scenario builder, its two pickers, the
//! filename prompt and the result of a fight.
//!
//! Every list here is drawn from the one app-core call that also decides
//! what the handler dispatches against. The builder's rows in particular
//! are hidden dynamically, so a second opinion about them here would draw a
//! different row from the one under the highlight.

use feral_processes_engine::arena::RepRecord;

use super::popup::*;
use super::*;

/// The scenario editor. Rows come from `App::arena_builder_rows`; the keys
/// below are the ones `App::handle_arena_builder_key` accepts.
pub(super) fn draw_arena_builder(
    rows: &[ArenaRow],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut body: Vec<Row> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| item_row(row.label.clone(), i == selected))
        .collect();
    body.push(text_row(""));
    body.push(text_row("Left/Right adjust  Enter pick  Backspace remove"));
    body.push(text_row("[F]ight  [L]oad  [S]ave  Esc back"));
    draw_popup("Arena", PopupSize::Large, &body, painter, m);
}

/// Species or items, whichever opened it — see `App::arena_pick_rows`.
pub(super) fn draw_arena_pick(rows: &[String], selected: usize, painter: &Painter, m: &Metrics) {
    draw_id_list("Pick", rows, selected, painter, m);
}

pub(super) fn draw_arena_load(rows: &[String], selected: usize, painter: &Painter, m: &Metrics) {
    draw_id_list("Load Scenario", rows, selected, painter, m);
}

/// The two id lists differ only in their title, so they are one function
/// rather than two copies of the same nine lines.
fn draw_id_list(title: &str, rows: &[String], selected: usize, painter: &Painter, m: &Metrics) {
    let mut body: Vec<Row> = Vec::new();
    if rows.is_empty() {
        body.push(text_row("Nothing to choose from."));
    }
    for (i, row) in rows.iter().enumerate() {
        body.push(item_row(
            format!("[{}] {row}", menu_shortcut(i)),
            i == selected,
        ));
    }
    body.push(text_row(""));
    body.push(text_row("Enter to choose, Esc to go back."));
    draw_popup(title, PopupSize::Large, &body, painter, m);
}

/// Drawn over the builder, the way the fuse-naming page is drawn over the
/// party screen — the scenario you are about to write stays visible behind
/// the name you are giving it.
pub(super) fn draw_arena_save(input: &str, painter: &Painter, m: &Metrics) {
    let body = vec![
        text_row("Write this scenario to dev-arenas/ as:"),
        text_row(""),
        item_row(format!("{input}_"), true),
        text_row(""),
        text_row("An existing file of that name is overwritten."),
        text_row("Enter to write, Esc to cancel."),
    ];
    draw_popup("Save Scenario", PopupSize::Small, &body, painter, m);
}

/// What the fight cost, above the blow-by-blow it cost it in.
pub(super) fn draw_arena_result(
    record: Option<&RepRecord>,
    warnings: &[String],
    seed: u64,
    transcript: &[String],
    selected: usize,
    painter: &Painter,
    m: &Metrics,
) {
    let mut body = vec![match record {
        Some(r) if r.won => Row::TextColored(summary_line(r), GREEN),
        Some(r) => Row::TextColored(summary_line(r), RED),
        None => text_row("No fight yet."),
    }];
    body.push(text_row(format!("seed {seed}")));
    // Under the seed, because the two together are what identifies the
    // fight: a rolled encounter rolls a fresh composition per seed.
    if let Some(line) = record.and_then(composition_line) {
        body.push(text_row(line));
    }
    // Shown rather than applied — `build_opponents` builds whatever was
    // asked for, and this line is what keeps that honest.
    for warning in warnings {
        body.push(Row::TextColored(format!("warning: {warning}"), YELLOW));
    }
    body.push(text_row(""));
    for (i, line) in transcript.iter().enumerate() {
        body.push(item_row(line.clone(), i == selected));
    }
    body.push(text_row(""));
    body.push(text_row(
        "[R]efight this seed  [N]ext seed  Up/Down scroll  Esc back",
    ));
    draw_popup("Arena Result", PopupSize::Large, &body, painter, m);
}

/// What was fielded, or `None` for a fight that was never staged. Split out
/// alongside `summary_line` so a test can read it without a window.
fn composition_line(r: &RepRecord) -> Option<String> {
    if r.composition.is_empty() {
        return None;
    }
    Some(format!(
        "fought {}",
        r.composition
            .iter()
            .map(|(species, count)| format!("{species} x{count}"))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Won or lost, and what it took. Split out so a test can read it without a
/// window.
fn summary_line(r: &RepRecord) -> String {
    format!(
        "{} in {} rounds, {:.0}% HP left, {} companions down",
        if r.won { "WON" } else { "LOST" },
        r.rounds,
        r.player_hp_fraction * 100.0,
        r.companions_downed
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_app_core::ArenaRowKind;
    use feral_processes_engine::DifficultyMode;

    fn record(won: bool) -> RepRecord {
        RepRecord {
            seed: 7,
            won,
            rounds: 12,
            player_hp_fraction: 0.4,
            companions_downed: 1,
            composition: Vec::new(),
            transcript: vec!["── round 1 ──".to_string()],
        }
    }

    #[test]
    fn a_lost_fight_says_so_rather_than_reporting_its_hp() {
        assert!(summary_line(&record(true)).starts_with("WON"));
        let lost = summary_line(&record(false));
        assert!(lost.starts_with("LOST"), "{lost}");
        assert!(lost.contains("12 rounds"), "{lost}");
    }

    #[test]
    fn the_result_screen_shows_the_staging_warnings() {
        // Nothing about a composition is ever capped, so the ask being on
        // screen is the only thing that makes that honest.
        with_painter(|p| {
            let m = ui_metrics(900.0);
            draw_arena_result(
                Some(&record(false)),
                &["9 asked for; zone 1 would field at most 1".to_string()],
                3,
                &record(false).transcript,
                0,
                p,
                &m,
            );
        });
    }

    /// A rolled encounter fields something different every rep, so what was
    /// fought has to be on the screen that reports the fight.
    #[test]
    fn the_result_screen_names_the_composition() {
        let mut r = record(true);
        r.composition = vec![("glitch".to_string(), 3)];
        with_painter(|p| {
            let m = ui_metrics(900.0);
            draw_arena_result(Some(&r), &[], 7, &r.transcript, 0, p, &m);
        });
        let line = composition_line(&r).expect("a fought composition draws a line");
        assert!(line.contains("glitch"), "{line}");
        assert!(line.contains('3'), "{line}");
        assert!(
            composition_line(&record(true)).is_none(),
            "an empty composition draws no line at all"
        );
    }

    #[test]
    fn every_arena_screen_draws() {
        let rows = vec![
            ArenaRow {
                label: "Player: Fresh".to_string(),
                kind: ArenaRowKind::PlayerSource,
            },
            ArenaRow {
                label: "Against: glitch x3".to_string(),
                kind: ArenaRowKind::Opponent(0),
            },
        ];
        let ids = vec!["glitch".to_string(), "sprite".to_string()];
        with_painter(|p| {
            let m = ui_metrics(900.0);
            draw_arena_builder(&rows, 0, p, &m);
            draw_arena_pick(&ids, 1, p, &m);
            draw_arena_load(&ids, 0, p, &m);
            draw_arena_save("my-fight", p, &m);
            draw_arena_result(
                Some(&record(true)),
                &[],
                7,
                &["a line".to_string()],
                0,
                p,
                &m,
            );
            // The empty states are reachable: an unopened session, and a
            // checkout with no `dev-arenas/` at all.
            draw_arena_builder(&[], 0, p, &m);
            draw_arena_load(&[], 0, p, &m);
            draw_arena_result(None, &[], 0, &[], 0, p, &m);
        });
    }

    /// `draw_row` clamps a row vertically and nothing clamps it
    /// horizontally, so an over-long row runs off the panel in silence.
    /// The builder's widest row is a species id beside a count.
    #[test]
    fn a_builder_row_fits_its_popup() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(3, DifficultyMode::Forgiving, assets).expect("shipped assets load");
        let widest = game
            .species_defs()
            .iter()
            .map(|d| format!("  Against: {} x100", d.id))
            .max_by_key(|line| line.chars().count())
            .expect("the shipped roster is not empty");

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            let drawn = p.measure_ui_advance(&widest, m.font_size);
            assert!(
                drawn <= room,
                "the widest builder row overflows its popup by {:.0}px:\n{widest}",
                drawn - room
            );
        });
    }
}
