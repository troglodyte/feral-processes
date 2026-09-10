//! One Teardown Rig's tool holder: which tool the base's extractor runs on.
//!
//! A rig strips with the tool fitted to it and not with anything in the
//! player's own slots, so this screen is where a run's extraction rate is
//! actually set. `render/depot_filter.rs`'s shape — a header naming the
//! machine, the `Tab` line only when there is somewhere to go, then one row
//! per thing the pack can put in it.

use super::popup::*;
use super::*;
use feral_processes_app_core::RigToolScreen;

/// The lead `draw_row` puts in front of every `Row::Item` label. A
/// `Row::Text` header gets none and carries this itself, or the heading
/// sits two cells left of the table it names — `render/depot_filter.rs`
/// documents the same hazard.
const HEADER_LEAD: &str = "  ";

const COLUMN_GAP: &str = "  ";

const HEADINGS: [&str; 4] = ["tool", "reaches", "tier", "ticks"];

pub(super) fn draw_rig_tool(
    screen: Option<&RigToolScreen>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let body = match screen {
        Some(screen) => body_rows(screen, selected),
        // Unreachable through `App`, which closes the screen the moment the
        // rig stops answering — drawn rather than asserted, because a
        // renderer that panics takes the window with it.
        None => vec![text_row("There is no rig here.")],
    };
    draw_popup("Rig tool", PopupSize::Large, &body, refusal, painter, m);
}

fn body_rows(screen: &RigToolScreen, selected: usize) -> Vec<Row> {
    let view = &screen.view;
    let (x, y) = view.tile;
    let mut body = vec![text_row(format!("{} at ({x}, {y})", view.name))];
    if screen.of > 1 {
        body.push(text_row(format!(
            "One of {} beside you; Tab for the next",
            screen.of
        )));
    }
    match &view.installed {
        Some(row) => body.push(text_row(format!(
            "Fitted: {} — {}, tier {}, {} ticks a program",
            row.name,
            row.category.as_str(),
            row.tier,
            row.ticks
        ))),
        None => body.push(text_row(
            "Nothing fitted — the rig runs nothing until it has a tool",
        )),
    }
    // Said only when there is something to lose: pulling the tool leaves
    // what a body has already fetched sitting in the hopper, and that is
    // worth knowing before the key is pressed rather than after.
    if view.queued > 0 {
        body.push(text_row(format!(
            "{} in the hopper — pulling the tool stops the rig, and they stay put",
            view.queued
        )));
    }
    body.extend([
        text_row(""),
        text_row("Pick a row to fit it; the tool it replaces comes back to your pack"),
        text_row("[R] pull the fitted tool  Esc to go back"),
        text_row(""),
    ]);
    if view.candidates.is_empty() {
        body.push(text_row(
            "You are carrying no tool this rig can run. Forge one, or take one out of a rig.",
        ));
        return body;
    }
    let cols = Columns::of(screen);
    body.push(text_row(cols.header()));
    for (i, row) in view.candidates.iter().enumerate() {
        body.push(item_row(
            cols.row(&row.name, row.category.as_str(), row.tier, row.ticks),
            i == selected,
        ));
    }
    body
}

/// The measured column widths. Every one is measured rather than fixed: a
/// mod's tool name, category label and tick figure are all unbounded, and
/// an over-wide row is drawn off the panel in silence.
struct Columns {
    name: usize,
    category: usize,
    tier: usize,
}

impl Columns {
    fn of(screen: &RigToolScreen) -> Self {
        let view = &screen.view;
        let width = |f: &dyn Fn(&feral_processes_engine::RigToolRow) -> String, head: &str| {
            view.candidates
                .iter()
                .map(|r| f(r).chars().count())
                .chain(std::iter::once(head.chars().count()))
                .max()
                .unwrap_or(0)
        };
        Columns {
            name: width(&|r| r.name.clone(), HEADINGS[0]),
            category: width(&|r| r.category.as_str().to_string(), HEADINGS[1]),
            tier: width(&|r| r.tier.to_string(), HEADINGS[2]),
        }
    }

    fn header(&self) -> String {
        format!(
            "{HEADER_LEAD}{:<name$}{COLUMN_GAP}{:<cat$}{COLUMN_GAP}{:<tier$}{COLUMN_GAP}{}",
            HEADINGS[0],
            HEADINGS[1],
            HEADINGS[2],
            HEADINGS[3],
            name = self.name,
            cat = self.category,
            tier = self.tier,
        )
    }

    fn row(&self, name: &str, category: &str, tier: u32, ticks: u64) -> String {
        format!(
            "{:<name$}{COLUMN_GAP}{:<cat$}{COLUMN_GAP}{:<tier$}{COLUMN_GAP}{ticks}",
            name,
            category,
            tier,
            name = self.name,
            cat = self.category,
            tier = self.tier,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::{DifficultyMode, Game, RigToolRow, RigToolView};

    fn shipped_game() -> Game {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        Game::new(42, DifficultyMode::Forgiving, assets).expect("shipped assets")
    }

    /// `PopupSize::Large`'s body, matching `draw_popup`'s 0.88 width.
    fn body_room(m: &Metrics) -> f32 {
        1440.0 * 0.88 - m.pad * 2.0
    }

    /// Every shipped tool at the widest figure each column can print, so the
    /// census measures the catalogue rather than whatever a fixture happened
    /// to fit.
    fn widest_screen(game: &Game) -> RigToolScreen {
        let candidates: Vec<RigToolRow> = game
            .tool_defs()
            .into_iter()
            .map(|def| RigToolRow {
                id: def.id.clone(),
                name: def.name.clone(),
                category: def.category,
                tier: u32::MAX,
                ticks: u64::MAX,
                carriers_held: u32::MAX,
            })
            .collect();
        RigToolScreen {
            rig: Entity::PLACEHOLDER,
            of: 1,
            view: RigToolView {
                tile: (0, 0),
                name: "Teardown Rig".to_string(),
                installed: candidates.first().cloned(),
                candidates,
                queued: 0,
            },
        }
    }

    /// **The widest row the shipped catalogue can build still fits, and so
    /// does the header over it.**
    ///
    /// `draw_row` clips a row vertically and nothing clips it horizontally,
    /// so an over-wide row is drawn off the panel in silence — taking the
    /// tick figure the whole screen exists to compare with it.
    #[test]
    fn no_rig_tool_row_overflows_its_popup() {
        let game = shipped_game();
        let screen = widest_screen(&game);
        let cols = Columns::of(&screen);
        let widest = screen
            .view
            .candidates
            .iter()
            .max_by_key(|r| r.name.chars().count())
            .expect("the shipped assets define tools");
        let row = cols.row(&widest.name, widest.category.as_str(), u32::MAX, u64::MAX);
        let header = cols.header();

        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = body_room(&m);
            let drawn = p.measure_ui_advance(format!("  {row}"), m.font_size);
            assert!(
                drawn > 0.0,
                "the census measured nothing — the shipped set has to reach here"
            );
            assert!(
                drawn <= room,
                "the widest rig tool row overflows by {:.0}px ({drawn:.0} into {room:.0}):\n{row}",
                drawn - room
            );
            let head = p.measure_ui_advance(&header, m.font_size);
            assert!(
                head <= room,
                "the rig tool header overflows by {:.0}px",
                head - room
            );
        });
    }

    /// Every line of the page fits the popup's height, which has no scroll —
    /// the hopper warning and the "nothing you can fit" line included, since
    /// those are the two that only appear in particular states.
    #[test]
    fn the_page_says_what_it_has_to_in_every_state() {
        let game = shipped_game();
        let mut screen = widest_screen(&game);
        screen.view.queued = 6;
        let loud = body_rows(&screen, 0);
        assert!(
            loud.iter().any(|r| row_label_text(r).contains("hopper")),
            "a loaded rig warns that pulling the tool strands the queue"
        );

        screen.view.candidates.clear();
        screen.view.installed = None;
        let empty = body_rows(&screen, 0);
        assert!(
            empty
                .iter()
                .any(|r| row_label_text(r).contains("Nothing fitted")),
            "an untooled rig says so"
        );
        assert!(
            empty
                .iter()
                .any(|r| row_label_text(r).contains("Forge one")),
            "and a pack with nothing to fit says what to do about it"
        );
    }
}
