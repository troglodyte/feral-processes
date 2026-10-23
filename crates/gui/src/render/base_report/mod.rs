//! The hub screens: a page rail down the left and the current screen's rows
//! in the content rect beside it. The heading, pages, marks and extra keys
//! come from the hub's `app_core::HubKind`; this module only draws what it
//! says, so a page cannot be on the rail and missing from the nav, and no
//! hub is named here.
//!
//! Each absorbed screen is a row builder drawn by `draw_rows_in`, not a
//! popup: the frame owns the chrome, the screen owns its rows.

mod commands;
mod frame;
#[cfg(test)]
pub(in crate::render) use fit::{assert_rows_fit, content_rows_at_720};
pub(in crate::render) use frame::RAIL_CELLS;
#[cfg(test)]
pub(in crate::render) use frame::content_rect_at;
mod orders;
mod overview;
mod production;
mod staff;
mod stock;

use super::popup::*;
use super::*;
use feral_processes_app_core::{BASE_REPORT_PAGES, GroupMenuRow, HubPage, WorkOrderRow};
use feral_processes_engine::{PerkPreview, WorkTable};

/// What the screens that read `App` state rather than the `Game` are drawn
/// from, taken off the app before `app.game` is borrowed — the same
/// handler-and-renderer-share-one-list rule `render_screen` follows.
pub(super) struct AppScreens<'a> {
    pub(super) work_orders: &'a [WorkOrderRow],
    pub(super) base_staff: Option<&'a WorkTable>,
    pub(super) work_column: usize,
    /// `App::hub_on_rail`: the rail holds the cursor, so the screen's rows
    /// draw none.
    pub(super) on_rail: bool,
    /// The current screen's rows when it is a command page.
    pub(super) commands: Option<&'a [GroupMenuRow]>,
    /// `(page index, row count)` for each command page, for the Overview.
    pub(super) command_counts: &'a [(usize, usize)],
    /// `App::perk_previews`, one per perk, for the Perks page.
    pub(super) perk_previews: &'a [Option<PerkPreview>],
    /// What was typed to filter the current screen's list, or `""`.
    pub(super) list_filter: &'a str,
}

/// Where a mode sits in its hub and what the frame says about it, taken off
/// the `App` before `app.game` is borrowed. Everything here is the hub's
/// `HubKind` answering for itself, so no hub is named in this module.
pub(super) struct HubFrame {
    mode: Mode,
    /// The rail heading as `(head, tail)`: the tail is kept whole when the
    /// head is elided to fit.
    title: (String, String),
    pages: Vec<&'static HubPage>,
    page: usize,
    screen: usize,
    /// Which pages carry the rail's `!`.
    marks: Vec<bool>,
    /// The hub's own keys, as `(key, verb)`.
    extra_keys: Vec<(&'static str, &'static str)>,
}

impl HubFrame {
    /// `mode`'s place in whichever hub is showing it, `None` outside every hub.
    pub(super) fn of(app: &App, mode: Mode) -> Option<Self> {
        let (hub, page, screen) = app.hub_for(mode)?;
        let kind = hub.kind();
        Some(HubFrame {
            mode,
            title: kind.heading(app),
            pages: kind.pages(app),
            page,
            screen,
            marks: kind.marks(app),
            extra_keys: kind.extra_keys(app),
        })
    }

    fn paint(
        &self,
        keybar_mode: Option<Mode>,
        on_rail: bool,
        refusal: Option<&str>,
        painter: &Painter,
        m: &Metrics,
    ) -> Rect {
        frame::paint_frame(
            &frame::FrameView {
                title: (&self.title.0, &self.title.1),
                pages: &self.pages,
                page: self.page,
                screen: self.screen,
                marks: &self.marks,
                extra_keys: &self.extra_keys,
            },
            keybar_mode,
            on_rail,
            refusal,
            painter,
            m,
        )
    }
}

/// Draws a hub page whose rows the caller built: the frame, then the rows
/// in its content rect. These are screens whose row lists their popups draw
/// too, so only the chrome differs.
pub(super) fn draw_hub_page(
    frame: &HubFrame,
    mode: Mode,
    rows: &[Row],
    on_rail: bool,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let content = frame.paint(Some(mode), on_rail, refusal, painter, m);
    draw_rows_in(content, rows, !on_rail, painter, m);
}

/// Draws the shared hub at `mode`, which `HubFrame::of` has already placed
/// on a page. `selected` is the screen's own cursor (`App::menu_selected`).
pub(super) fn draw_base_report(
    game: &mut Game,
    frame: &HubFrame,
    selected: usize,
    app_screens: &AppScreens,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mode = frame.mode;
    // The Overview spells out the flags the rail marks with `!`.
    let flags = if mode == Mode::BaseOverview {
        game.base_report_flags()
    } else {
        Default::default()
    };
    let rows = screen_rows(game, mode, selected, app_screens, &flags);
    let content = frame.paint(Some(mode), app_screens.on_rail, refusal, painter, m);
    draw_rows_in(content, &rows, !app_screens.on_rail, painter, m);
}

/// A popup child's parent page, drawn as the frame behind it: rail and title,
/// but not the screen's rows or its keybar (the child carries its own hints). The popup panel is not quite
/// opaque and lets whatever lies under it through, which over the map is
/// the point and over a screen of text is unreadable, so the rows wait
/// until the popup is gone. The child owns the cursor and the refusal line.
pub(super) fn draw_popup_parent(parent: Option<&HubFrame>, painter: &Painter, m: &Metrics) {
    if let Some(parent) = parent {
        parent.paint(None, false, None, painter, m);
    }
}

/// The rows of one hub screen. Every figure comes from the engine; this only
/// formats.
fn screen_rows(
    game: &mut Game,
    mode: Mode,
    selected: usize,
    app_screens: &AppScreens,
    flags: &feral_processes_engine::BaseReportFlags,
) -> Vec<Row> {
    match mode {
        Mode::BaseOverview => overview::overview_rows(flags, app_screens.command_counts, selected),
        Mode::BaseBuild
        | Mode::BaseResearch
        | Mode::BaseTrade
        | Mode::BaseCompile
        | Mode::PartyPrograms
        | Mode::PartyRoutines => {
            commands::command_rows(app_screens.commands.unwrap_or_default(), selected)
        }
        Mode::StaffWellbeing => staff::wellbeing_rows(&game.staff_wellbeing(), selected),
        Mode::StaffCoverage => staff::coverage_rows(&game.need_coverage(), selected),
        Mode::StaffDiscontent => staff::discontent_rows(&game.discontent(), selected),
        Mode::BaseOutput => building::base_output_rows(&game.base_output_report()),
        Mode::BaseMachines => production::machine_rows(&game.machine_report(), selected),
        Mode::BasePower => production::power_rows(&game.power_report(), selected),
        Mode::BaseStock => stock::stock_rows(&game.stock_report(), selected),
        Mode::BuildOrders => orders::build_rows(&game.build_order_report(), selected),
        Mode::DigOrders => orders::dig_rows(&game.dig_marks(), selected),
        Mode::FloorOrders => orders::floor_rows(&game.floor_marks(), selected),
        Mode::WorkOrders => building::work_order_screen_rows(
            app_screens.work_orders,
            game.labour_demand(),
            selected,
        ),
        Mode::BaseStaff => match app_screens.base_staff {
            Some(table) => building::base_staff_menu_rows(
                table,
                &game.owned_pets(),
                selected,
                (!app_screens.on_rail).then_some(app_screens.work_column),
            ),
            None => Vec::new(),
        },
        Mode::Structures => building::structures_rows(game, selected),
        Mode::Alerts => alerts::alert_rows(&game.alerts(), selected),
        Mode::Inventory => inventory::items_rows(game, selected, app_screens.list_filter),
        Mode::PhaseKeys => inventory::phase_key_rows(&game.phase_keys(), selected),
        Mode::DownedPrograms => extraction::downed_program_list_rows(game, selected),
        Mode::Tools => tools::tools_rows(game, selected),
        Mode::Companion => party::roster_rows(game, selected),
        Mode::Perks => progression::perks_rows(game, app_screens.perk_previews, selected),
        other => vec![text_row(format!("No report for {other:?}."))],
    }
}

/// Each page whose first screen is a command page, with its index.
pub(super) fn command_pages() -> impl Iterator<Item = (usize, Mode)> {
    BASE_REPORT_PAGES
        .iter()
        .enumerate()
        .map(|(page, entry)| (page, entry.screens[0]))
}

/// What every screen's width census shares.
#[cfg(test)]
pub(in crate::render) mod fit {
    use super::*;

    /// How many rows the content rect holds at 1280x720, the smallest window.
    pub(in crate::render) fn content_rows_at_720() -> usize {
        crate::paint::with_painter(|p| {
            let m = crate::text::ui_metrics(720.0);
            let char_w = p.measure_ui_advance("M", m.font_size);
            let content = crate::render::base_report::content_rect_at(1280.0, 720.0, char_w, &m);
            (content.h / m.line_height).floor() as usize
        })
        .0
    }

    pub(super) fn text_of(row: &Row) -> &str {
        match row {
            Row::Text(t) | Row::TextColored(t, _) => t,
            Row::Item { text, .. } => text,
        }
    }

    /// `draw_row` clips a row vertically and never horizontally, so a line
    /// past the right edge is lost. Held at the smallest supported window,
    /// where the content rect is narrowest in characters.
    pub(in crate::render) fn assert_rows_fit(rows: &[Row], what: &str) {
        crate::paint::with_painter(|p| {
            for (w, h) in [(1280.0, 720.0), (1440.0, 900.0), (1920.0, 1080.0)] {
                let m = crate::text::ui_metrics(h);
                let char_w = p.measure_ui_advance("M", m.font_size);
                let content = crate::render::base_report::content_rect_at(w, h, char_w, &m);
                let room = content.w - m.pad * 2.0;
                for row in rows {
                    let line = row_label_text(row);
                    let drawn = p.measure_ui_advance(&line, m.font_size);
                    assert!(
                        drawn <= room,
                        "a {what} row overflows by {:.0}px at {w}x{h} \
                         ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                        drawn - room
                    );
                }
            }
        });
    }
}
