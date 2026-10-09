//! The Points screen's drawing, shared: the creation wizard's Points step
//! and `Mode::AllocateStats` both draw the same `CreationRow::Attribute`
//! rows through `attribute_lines` and word their footer through `footer`.
//! Creation is a step of a popup with its own title and numbering;
//! `draw_allocate_stats` is the whole popup for the level-up spend.

use feral_processes_app_core::{AllocationFor, CreationRow};
use feral_processes_engine::StatOwner;
use feral_processes_engine::attributes::DerivedStat;

use super::level_up::{duel_heading, duel_lines};
use super::popup::{PopupSize, ROW_WRAP_COLUMNS, Row, draw_popup, item_row, text_row};
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
        DerivedStat::Perception => format!("{label} {before:.1} \u{2192} {after:.1} tiles"),
        DerivedStat::StatusResist => format!("{label} {before:.0}% \u{2192} {after:.0}%"),
        _ => format!("{label} {before:.0} \u{2192} {after:.0}"),
    }
}

/// The widest an attribute row's text runs, in characters, before its
/// remaining effects drop to a continuation line. `ROW_WRAP_COLUMNS` less
/// the selection prefix; `no_points_row_overflows_its_popup` and the
/// creation width census hold it against real glyph widths.
const ATTRIBUTE_LINE_COLUMNS: usize = ROW_WRAP_COLUMNS - 8;

/// An attribute row as text lines: its name and legacy word, the value it
/// will read, the points bought on it, and each stat it feeds as
/// before -> after. Effects that would run past `ATTRIBUTE_LINE_COLUMNS`
/// carry onto further lines indented to where the effects start; a row that
/// fits is one line. `None` for any other row kind.
///
/// `bar_pool` draws the points bought as a `[###---]` bar, creation's: its
/// width is the most this row could hold if the *whole* pool went to it —
/// not what the other rows have left, which changes row to row and would
/// make the bar's length a second, unlabelled figure to read. The level-up
/// spend passes `None`: its pool is a few points, too small for a bar to
/// say anything the count does not.
pub(super) fn attribute_lines(row: &CreationRow, bar_pool: Option<u32>) -> Option<Vec<String>> {
    let CreationRow::Attribute {
        name,
        legacy,
        spent,
        value,
        effects,
        cost,
        ..
    } = row
    else {
        return None;
    };
    let bar = bar_pool
        .map(|pool| {
            let width = (pool / (*cost).max(1)).max(*spent);
            let cells: String = (0..width)
                .map(|u| if u < *spent { '#' } else { '-' })
                .collect();
            format!("[{cells}] ")
        })
        .unwrap_or_default();
    let head = format!(
        "{:<24} {value:>3}  {bar}+{spent:<2}  ",
        format!("{name} ({legacy})")
    );
    let indent = " ".repeat(head.chars().count());
    let mut lines = vec![head];
    let mut fresh = true;
    for &(stat, before, after) in effects {
        let text = effect_text(stat, before, after);
        let line = lines.last_mut().expect("lines is never empty");
        if !fresh && line.chars().count() + 2 + text.chars().count() > ATTRIBUTE_LINE_COLUMNS {
            lines.push(format!("{indent}{text}"));
        } else {
            if !fresh {
                line.push_str("  ");
            }
            line.push_str(&text);
        }
        fresh = false;
    }
    Some(lines)
}

/// The key line under the rows. `spends_pool` is creation: a pool that is
/// lost if not spent, so the wizard moves on only once it is. Otherwise the
/// points are banked and leaving keeps them.
pub(super) fn footer(pool: u32, left: u32, spends_pool: bool, owner: Option<&str>) -> String {
    let whose = owner.map(|name| format!(" for {name}")).unwrap_or_default();
    let keys = "Left/Right spends (Shift: all, Ctrl: half)";
    match spends_pool {
        true => format!(
            "{}/{pool} points spent, {left} left - {keys}; Enter: next once it is spent   Esc: back",
            pool - left
        ),
        false => format!(
            "{left} of {pool} points left{whose} - {keys}; Enter confirms, Esc keeps the rest banked"
        ),
    }
}

/// The level-up spend: `Mode::AllocateStats`.
pub(super) fn draw_allocate_stats(
    title: &str,
    rows: &[Row],
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(title, PopupSize::Large, rows, refusal, painter, m);
}

/// The name of the program whose points these are, `None` for the player's
/// own: the title and the footer both name it.
fn allocation_owner(app: &App) -> Option<String> {
    let program = match app.stat_allocation.as_ref().map(|a| a.purpose()) {
        Some(AllocationFor::Owned(StatOwner::Program(e))) => Some(e),
        _ => None,
    }?;
    app.game
        .as_ref()
        .and_then(|game| game.manifest(program))
        .map(|view| view.name)
}

/// The popup's title: names the program whose points these are, and says
/// nothing extra for the player's own.
pub(super) fn allocate_stats_title(app: &App) -> String {
    match allocation_owner(app) {
        Some(name) => format!("Spend Stat Points - {name}"),
        None => "Spend Stat Points".to_string(),
    }
}

/// The Neural Load line, in the warning colour once over the cap.
fn load_row((load, cap): (u32, u32)) -> Option<Row> {
    if load == 0 {
        return None;
    }
    let text = format!("Neural Load {load}/{cap}");
    Some(if load > cap {
        Row::TextColored(
            format!("{text} - over the cap, fights risk a rejection"),
            ORANGE,
        )
    } else {
        text_row(text)
    })
}

/// An attribute's lines as popup rows: the cursor highlight sits on the
/// first, and the continuations stay `Row::Item` so the scroll body does not
/// tear them off the row they belong to.
pub(super) fn attribute_item_rows(lines: Vec<String>, selected: bool) -> Vec<Row> {
    lines
        .into_iter()
        .enumerate()
        .map(|(n, line)| item_row(line, selected && n == 0))
        .collect()
}

/// The popup's rows, split out so the census can measure what is drawn.
pub(super) fn allocate_stats_rows(app: &App) -> Vec<Row> {
    let mut rows: Vec<Row> = app
        .allocation_rows()
        .iter()
        .enumerate()
        .filter_map(|(i, row)| {
            attribute_lines(row, None)
                .map(|lines| attribute_item_rows(lines, i == app.menu_selected))
        })
        .flatten()
        .collect();
    if rows.is_empty() {
        rows.push(text_row("Nothing to spend points on."));
    }
    // The player's own spend only: a program has no implants. Shown only
    // once there is Load, so a run that never splices sees no new row.
    if allocation_owner(app).is_none()
        && let Some(game) = app.game.as_ref()
        && let Some(row) = load_row(game.neural_load())
    {
        rows.push(row);
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
        allocation_owner(app).as_deref(),
    )));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_app_core::StatAllocation;
    use feral_processes_engine::{DifficultyMode, Game};

    #[test]
    fn perception_reads_to_one_decimal_in_tiles() {
        assert_eq!(
            effect_text(DerivedStat::Perception, 5.0, 5.5),
            "Perception 5.0 \u{2192} 5.5 tiles"
        );
    }

    fn shipped_game() -> Game {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        Game::new(42, DifficultyMode::Forgiving, &assets).expect("shipped assets")
    }

    fn allocation(game: &Game, pool: u32) -> StatAllocation {
        StatAllocation::new(
            AllocationFor::Owned(StatOwner::Player),
            pool,
            game.derived_base(game.player_entity()),
            game.attribute_db(),
            game.attributes_of(game.player_entity()),
        )
    }

    fn attribute_row(spent: u32, cost: u32) -> CreationRow {
        CreationRow::Attribute {
            id: "parity".into(),
            name: "Parity".into(),
            legacy: "Vitality".into(),
            spent,
            value: 5 + spent as i32,
            effects: Vec::new(),
            cost,
        }
    }

    /// Creation draws each row's share of the pool as a bar as wide as the
    /// most that row could ever hold, so a full row reads full; the level-up
    /// spend draws none.
    #[test]
    fn a_creation_row_draws_its_spend_as_a_bar() {
        let line = attribute_lines(&attribute_row(3, 1), Some(8))
            .unwrap()
            .remove(0);
        assert!(line.contains("[###-----]"), "{line}");
        let line = attribute_lines(&attribute_row(0, 2), Some(8))
            .unwrap()
            .remove(0);
        assert!(
            line.contains("[----]"),
            "a 2-point row holds 4 of 8: {line}"
        );
        let line = attribute_lines(&attribute_row(3, 1), None)
            .unwrap()
            .remove(0);
        assert!(!line.contains('['), "{line}");
    }

    #[test]
    fn the_load_row_is_absent_without_implants_and_warns_over_the_cap() {
        assert!(load_row((0, 4)).is_none());
        assert_eq!(
            super::popup::row_label_text(&load_row((3, 4)).unwrap()),
            "Neural Load 3/4"
        );
        assert!(matches!(load_row((5, 4)), Some(Row::TextColored(_, c)) if c == ORANGE));
    }

    /// An attribute whose effects overflow the line carries the rest onto an
    /// indented second line under where the effects start, keeping every
    /// label whole; one that fits stays a single line.
    #[test]
    fn analysis_wraps_its_effects_onto_an_aligned_continuation() {
        let game = shipped_game();
        let rows = allocation(&game, 999).rows(&Default::default());
        let lines_of = |id: &str| {
            rows.iter()
                .find_map(|r| match r {
                    CreationRow::Attribute { id: i, .. } if i.as_str() == id => {
                        attribute_lines(r, Some(8))
                    }
                    _ => None,
                })
                .unwrap()
        };
        let analysis = lines_of("analysis");
        assert!(analysis.len() >= 2, "{analysis:?}");
        let start = analysis[0].find("ATK").expect("first effect on line one");
        assert!(
            analysis[1].starts_with(&" ".repeat(start))
                && !analysis[1].trim_start().is_empty()
                && analysis[1].len() - analysis[1].trim_start().len() == start,
            "{analysis:?}"
        );
        assert!(analysis.join(" ").contains("tiles"), "{analysis:?}");
        assert_eq!(lines_of("parity").len(), 1);
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
                for line in attribute_lines(row, None).unwrap() {
                    let drawn = p.measure_ui_advance(format!("  {line}"), m.font_size);
                    assert!(
                        drawn <= room,
                        "a Points row overflows by {:.0}px:\n{line}",
                        drawn - room
                    );
                }
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
            let (_, shapes) =
                with_painter(|p| draw_allocate_stats("Spend Stat Points", &rows, None, p, &m));
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

    /// The footer names whose points are being spent, as the title does: a
    /// program by name, the player by nothing.
    #[test]
    fn the_footer_names_the_program_whose_points_these_are() {
        let mut game = super::super::test_support::game_with_a_single_program(7);
        let program = game.owned_pets()[0].entity;
        let name = game.manifest(program).unwrap().name;
        let allocation = StatAllocation::new(
            AllocationFor::Owned(StatOwner::Program(program)),
            4,
            game.derived_base(program),
            game.attribute_db(),
            game.attributes_of(program),
        );
        let mut app = shipped_app();
        app.game = Some(game);
        app.stat_allocation = Some(allocation);
        app.mode = Mode::AllocateStats;

        let text: Vec<String> = allocate_stats_rows(&app)
            .iter()
            .map(super::super::popup::row_label_text)
            .collect();
        let footer = text.iter().find(|t| t.contains("points left")).unwrap();
        assert!(footer.contains(&format!("for {name}")), "{footer}");
        assert!(allocate_stats_title(&app).ends_with(&name));
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
