//! The Points screen's drawing, shared: the creation wizard's Points step
//! and `Mode::AllocateStats` both draw the same `CreationRow::Attribute`
//! rows through `attribute_line` and word their footer through `footer`.
//! Creation is a step of a popup with its own title and numbering;
//! `draw_allocate_stats` is the whole popup for the level-up spend.

use feral_processes_app_core::CreationRow;
use feral_processes_engine::attributes::DerivedStat;

use super::level_up::{duel_heading, duel_lines};
use super::popup::{PopupSize, Row, draw_popup, item_row, text_row};
use super::*;

/// One stat's before -> after, in the units the player reads it in.
/// Extraction, Crit and Fumble are chances, so they read as percentages.
fn effect_text(stat: DerivedStat, before: f32, after: f32) -> String {
    let label = stat.label();
    match stat {
        DerivedStat::Extraction | DerivedStat::Crit | DerivedStat::Fumble => {
            format!(
                "{label} {:.1}% \u{2192} {:.1}%",
                before * 100.0,
                after * 100.0
            )
        }
        DerivedStat::StatusResist => format!("{label} {before:.0}% \u{2192} {after:.0}%"),
        _ => format!("{label} {before:.0} \u{2192} {after:.0}"),
    }
}

/// An attribute row as a line of text: its name and legacy word, the value
/// it will read, the points bought on it, and each stat it feeds as
/// before -> after. `None` for any other row kind.
pub(super) fn attribute_line(row: &CreationRow) -> Option<String> {
    let CreationRow::Attribute {
        name,
        legacy,
        spent,
        value,
        effects,
        ..
    } = row
    else {
        return None;
    };
    let effects: Vec<String> = effects
        .iter()
        .map(|&(stat, before, after)| effect_text(stat, before, after))
        .collect();
    Some(format!(
        "{:<24} {value:>3}  +{spent:<2}  {}",
        format!("{name} ({legacy})"),
        effects.join("  ")
    ))
}

/// The key line under the rows. `spends_pool` is creation: a pool that is
/// lost if not spent, so the wizard moves on only once it is. Otherwise the
/// points are banked and leaving keeps them.
pub(super) fn footer(pool: u32, left: u32, spends_pool: bool) -> String {
    let keys = "Left/Right spends (Shift: all, Ctrl: half)";
    match spends_pool {
        true => format!(
            "{}/{pool} points spent, {left} left - {keys}; Enter moves on once it is spent",
            pool - left
        ),
        false => format!(
            "{left} of {pool} points left - {keys}; Enter confirms, Esc keeps the rest banked"
        ),
    }
}

/// The level-up spend: `Mode::AllocateStats`.
pub(super) fn draw_allocate_stats(
    rows: &[Row],
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        "Spend Stat Points",
        PopupSize::Large,
        rows,
        refusal,
        painter,
        m,
    );
}

/// The popup's rows, split out so the census can measure what is drawn.
pub(super) fn allocate_stats_rows(app: &App) -> Vec<Row> {
    let mut rows: Vec<Row> = app
        .allocation_rows()
        .iter()
        .enumerate()
        .filter_map(|(i, row)| {
            attribute_line(row).map(|line| item_row(line, i == app.menu_selected))
        })
        .collect();
    if rows.is_empty() {
        rows.push(text_row("Nothing to spend points on."));
    }
    // What the pending spend does in a fight, after the attribute rows and
    // pinned with the footer: the screen keeps its whole attribute list in
    // view and this block costs the body no row.
    if let Some(duel) = &app.allocation_duel {
        rows.push(text_row(""));
        rows.push(text_row(duel_heading(duel)));
        rows.extend(duel_lines(duel).into_iter().map(text_row));
    }
    rows.push(text_row(""));
    rows.push(text_row(footer(
        app.allocation_pool(),
        app.allocation_points_left(),
        false,
    )));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_app_core::{AllocationFor, StatAllocation};
    use feral_processes_engine::{DifficultyMode, Game, StatOwner};

    fn shipped_game() -> Game {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        Game::new(42, DifficultyMode::Forgiving, &assets).expect("shipped assets")
    }

    fn allocation(game: &Game, pool: u32) -> StatAllocation {
        StatAllocation::new(
            AllocationFor::Owned(StatOwner::Player),
            pool,
            game.attribute_db(),
            game.attributes_of(game.player_entity()),
        )
    }

    /// `draw_row` clips vertically only, so a row wider than the popup body
    /// is drawn off the panel in silence. Measured at the widest figures a
    /// row can print, against every shipped attribute.
    #[test]
    fn no_points_row_overflows_its_popup() {
        let game = shipped_game();
        let allocation = allocation(&game, 999);
        let spent = allocation
            .rows(&Default::default())
            .into_iter()
            .filter_map(|row| match row {
                CreationRow::Attribute { id, .. } => Some((id, 999)),
                _ => None,
            })
            .collect();
        let rows = allocation.rows(&spent);
        assert!(
            !rows.is_empty(),
            "the shipped catalogue has buyable attributes"
        );
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for row in &rows {
                let line = attribute_line(row).unwrap();
                let drawn = p.measure_ui_advance(format!("  {line}"), m.font_size);
                assert!(
                    drawn <= room,
                    "a Points row overflows by {:.0}px:\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// The level-up spend draws each attribute with its name, legacy word
    /// and before -> after, follows the pending spend, and words its footer
    /// as banked points.
    #[test]
    fn the_level_up_points_screen_shows_the_spend_and_the_footer() {
        let game = shipped_game();
        let mut app = shipped_app();
        app.stat_allocation = Some(allocation(&game, 6));
        app.mode = Mode::AllocateStats;

        let text = |app: &App| {
            let rows = allocate_stats_rows(app);
            let m = ui_metrics(900.0);
            let (_, shapes) = with_painter(|p| draw_allocate_stats(&rows, None, p, &m));
            crate::paint::painted_text(&shapes).join(" | ")
        };
        let fresh = text(&app);
        assert!(fresh.contains("Parity (Vitality)"), "{fresh}");
        assert!(fresh.contains("Max HP 90 \u{2192} 90"), "{fresh}");
        assert!(fresh.contains("6 of 6 points left"), "{fresh}");

        app.allocation_spent.insert("parity".into(), 2);
        let spent = text(&app);
        assert!(spent.contains("Max HP 90 \u{2192} 102"), "{spent}");
        assert!(spent.contains("4 of 6 points left"), "{spent}");
    }

    /// The block sits in the pinned footer, so the widest Points screen, at
    /// every shipped attribute with the widest duel and a refusal showing,
    /// must still fit the smallest window without scrolling: a scroll here
    /// would hide attribute rows behind the block.
    #[test]
    fn the_points_screen_with_its_duel_block_fits_without_scrolling() {
        use feral_processes_engine::DuelComparison;
        let game = shipped_game();
        let mut app = shipped_app();
        app.stat_allocation = Some(allocation(&game, 999));
        app.allocation_duel = Some(DuelComparison {
            zone: 99,
            hit_chance: (0.05, 0.99),
            per_swing: (1234.9, 9999.9),
            swings_to_win: (999, 999),
            swings_to_down_you: (999, 999),
        });
        app.mode = Mode::AllocateStats;
        let rows = allocate_stats_rows(&app);
        let m = ui_metrics(720.0);
        assert!(
            !super::super::popup::popup_scrolls(
                720.0,
                PopupSize::Large,
                &rows,
                Some("Requires Zone 3 first."),
                &m
            ),
            "the Points screen scrolls at 1280x720 with {} rows drawn",
            rows.len()
        );
        with_painter(|p| {
            let room = super::super::popup::popup_body_width(1280.0, PopupSize::Large, &m);
            for row in &rows {
                let text = super::super::popup::row_label_text(row);
                assert!(
                    p.measure_ui_advance(&text, m.font_size) <= room,
                    "a Points row overflows: {text}"
                );
            }
        });
        let text: Vec<String> = rows
            .iter()
            .map(super::super::popup::row_label_text)
            .collect();
        assert!(text.iter().any(|t| t.contains("AGAINST A TYPICAL ZONE 99")));
    }

    fn shipped_app() -> App {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tmp = std::env::temp_dir().join(format!("fp_gui_points_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        App::new(
            root.join("assets"),
            tmp.join("saves"),
            tmp.join("history.log"),
            tmp.join("profile.ron"),
            root.join("dev-arenas"),
            tmp.join("telemetry.jsonl"),
        )
    }
}
