//! The hub's chrome: the page rail, the title line with its screen dots, and
//! the keybar. Returns the rect the screen's rows go in.

use super::manifest::keybar::{draw_groups, group, keycap_targets};
use super::manifest::layout::{RailGeom, frame_rect, rail_window};
use super::manifest::{cut_at_end_to_fit, elided_to_fit};
use super::*;
use feral_processes_app_core::{PointerHit, Region};
use hud::log_frame::keycap;

/// The rail's width in cells: a caret, the longest page title and the `!`.
pub(in crate::render) const RAIL_CELLS: usize = 22;

/// Where the parts of the frame go. Pure over the metrics so a test can
/// check it without a window.
pub(super) struct HubLayout {
    pub(super) rail: RailGeom,
    pub(super) title: Rect,
    pub(super) content: Rect,
    pub(super) keybar: Rect,
}

pub(super) fn hub_layout(frame: Rect, m: &Metrics, char_w: f32) -> HubLayout {
    let inner = Rect::new(
        frame.x + m.pad,
        frame.y + m.pad,
        frame.w - m.pad * 2.0,
        frame.h - m.pad * 2.0,
    );
    let keybar = Rect::new(
        inner.x,
        inner.y + inner.h - m.line_height,
        inner.w,
        m.line_height,
    );
    let body_bottom = keybar.y - m.gap;
    let rail_rect = Rect::new(
        inner.x,
        inner.y,
        RAIL_CELLS as f32 * char_w,
        body_bottom - inner.y,
    );
    let title_row = Rect::new(rail_rect.x, rail_rect.y, rail_rect.w, m.line_height);
    let rail = RailGeom {
        full: true,
        rect: rail_rect,
        title: Some(title_row),
        rows: Rect::new(
            rail_rect.x,
            rail_rect.y + m.line_height,
            rail_rect.w,
            body_bottom - rail_rect.y - m.line_height,
        ),
        row_h: m.line_height,
    };
    let main_x = rail_rect.x + rail_rect.w + m.pad;
    let main_w = inner.x + inner.w - main_x;
    let title = Rect::new(main_x, inner.y, main_w, m.line_height);
    let content_y = title.y + title.h + m.gap;
    HubLayout {
        rail,
        title,
        content: Rect::new(main_x, content_y, main_w, body_bottom - content_y),
        keybar,
    }
}

/// What one hub's frame shows, as `HubFrame` took it off the `App`.
pub(super) struct FrameView<'a> {
    pub(super) title: (&'a str, &'a str),
    pub(super) pages: &'a [HubPage],
    pub(super) page: usize,
    pub(super) screen: usize,
    /// The hub's own keys, as `(key, verb)`.
    pub(super) extra_keys: &'a [(&'static str, &'static str)],
    pub(super) page_keys: &'a [(&'static str, &'static str)],
    /// What the pointer is over, from the app's own record of it.
    pub(super) hover: Option<PointerHit>,
}

/// Paints the frame (its keybar only when `keybar_mode` is given) and
/// returns the content rect, already shortened by a red refusal line when
/// one is up. `on_rail` is whether the rail holds the hub's one cursor; the
/// rows draw it otherwise.
pub(super) fn paint_frame(
    view: &FrameView,
    keybar_mode: Option<Mode>,
    on_rail: bool,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) -> Rect {
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let frame = frame_rect(painter.screen_w(), painter.screen_h());
    let l = hub_layout(frame, m, char_w);

    painter.rect(frame.x, frame.y, frame.w, frame.h, hud::palette::STATUS_BG);
    painter.rect_lines(
        frame.x,
        frame.y,
        frame.w,
        frame.h,
        2.0,
        hud::palette::PANE_BORDER,
    );
    // Only the hub's own page is a pointer surface: the frame drawn behind
    // a popup child is not, and nothing under the popup should take a click.
    let live = keybar_mode.is_some();
    if live {
        painter.target(l.rail.rect, PointerHit::Region(Region::Rail));
        painter.target(l.content, PointerHit::Region(Region::Page));
    }
    draw_rail(&l.rail, view, on_rail, live, char_w, painter, m);
    draw_title(&l.title, view, live, painter, m);
    if let Some(mode) = keybar_mode {
        let groups = keybar_groups(
            mode,
            view.pages,
            view.page,
            on_rail,
            view.extra_keys,
            view.page_keys,
        );
        for (rect, key) in keycap_targets(l.keybar, &groups, painter, m) {
            let hit = PointerHit::Key(key);
            tint_if_hovered(view, hit, rect, painter);
            painter.target(rect, hit);
        }
        draw_groups(l.keybar, &groups, painter, m);
    }

    let mut content = l.content;
    if let Some(text) = refusal {
        painter.ui(
            elided_to_fit(painter, text, m.font_size, content.w - m.pad * 2.0),
            content.x + m.pad,
            content.y + m.font_size as f32,
            m.font_size,
            RED,
        );
        content.y += m.line_height;
        content.h = (content.h - m.line_height).max(0.0);
    }
    content
}

/// Tints `rect` when the pointer is over `hit`, behind whatever is drawn
/// next.
fn tint_if_hovered(view: &FrameView, hit: PointerHit, rect: Rect, painter: &Painter) {
    if view.hover == Some(hit) {
        painter.rect(rect.x, rect.y, rect.w, rect.h, hud::palette::HOVER_BG);
    }
}

/// One line of the rail: a section header, which the cursor never lands on,
/// or a page by its index.
#[derive(Debug, PartialEq, Eq)]
enum RailLine<'a> {
    Section(&'a str),
    Page(usize),
}

/// The rail as lines: a header line comes before the first page that
/// carries one.
fn rail_lines(pages: &[HubPage]) -> Vec<RailLine<'_>> {
    let mut lines = Vec::with_capacity(pages.len());
    for (i, page) in pages.iter().enumerate() {
        if let Some(section) = page.section {
            lines.push(RailLine::Section(section));
        }
        lines.push(RailLine::Page(i));
    }
    lines
}

/// The current page always carries the caret; the highlight bar is the
/// cursor, drawn only while the rail has it and in the rows' own
/// `SELECT_BG`, so it reads as the one cursor moving rather than a second.
fn draw_rail(
    rail: &RailGeom,
    view: &FrameView,
    focused: bool,
    live: bool,
    char_w: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let (hub_title, pages, selected) = (view.title, view.pages, view.page);
    painter.rect(
        rail.rect.x + rail.rect.w + m.pad / 2.0,
        rail.rect.y,
        1.0,
        rail.rect.h,
        hud::palette::DIVIDER,
    );
    if let Some(title) = rail.title {
        // The tail ("(2 of 3)") is the part that says where the cycle is,
        // so the head gives way to it, never the other way round.
        let (head, tail) = hub_title;
        let room = title.w - painter.measure_ui_advance(tail, m.small());
        let head = elided_to_fit(painter, head, m.small(), room);
        let base = title.y + title.h * 0.75;
        painter.ui_bold(&head, title.x, base, m.small(), hud::palette::PANE_TITLE);
        if !tail.is_empty() {
            let x = title.x + painter.measure_ui_advance(&head, m.small());
            painter.ui_bold(tail, x, base, m.small(), hud::palette::PANE_TITLE);
        }
    }
    let lines = rail_lines(pages);
    let selected_line = lines
        .iter()
        .position(|l| *l == RailLine::Page(selected))
        .unwrap_or(0);
    let w = rail_window(lines.len(), selected_line, rail.fits());
    let mut slot = 0;
    // A marker stands for the nearest page it hides, so a click on it is
    // the step to that page.
    let page_of = |line: &RailLine| match line {
        RailLine::Page(i) => Some(*i),
        RailLine::Section(_) => None,
    };
    let more = |slot: usize, arrow: char, n: usize, hides: Option<usize>| {
        painter.ui(
            format!("{arrow} {n} more"),
            rail.rows.x + char_w,
            rail.rows.y + slot as f32 * rail.row_h + rail.row_h * 0.85,
            m.small(),
            hud::palette::FAINT,
        );
        if let (true, Some(page)) = (live, hides) {
            let bar = Rect::new(
                rail.rows.x,
                rail.rows.y + slot as f32 * rail.row_h,
                rail.rows.w,
                rail.row_h,
            );
            let hit = PointerHit::Page(pages[page].key);
            tint_if_hovered(view, hit, bar, painter);
            painter.target(bar, hit);
        }
    };
    if w.above > 0 {
        more(
            slot,
            '↑',
            w.above,
            lines[..w.start].iter().rev().find_map(page_of),
        );
        slot += 1;
    }
    for line in &lines[w.start..w.end] {
        let top = rail.rows.y + slot as f32 * rail.row_h;
        let base = top + rail.row_h * 0.85;
        slot += 1;
        let page = match line {
            RailLine::Section(name) => {
                painter.ui(name, rail.rows.x, base, m.small(), hud::palette::FAINT);
                continue;
            }
            RailLine::Page(page) => *page,
        };
        let entry = &pages[page];
        let current = page == selected;
        let bar = Rect::new(rail.rows.x, top, rail.rows.w, rail.row_h);
        if live {
            let hit = PointerHit::Page(entry.key);
            if !(current && focused) {
                tint_if_hovered(view, hit, bar, painter);
            }
            painter.target(bar, hit);
        }
        if current && focused {
            painter.rect(rail.rows.x, top, rail.rows.w, rail.row_h, SELECT_BG);
        }
        if current {
            painter.ui(
                "▸",
                rail.rows.x,
                base,
                m.font_size,
                hud::palette::PANE_TITLE,
            );
        }
        let ink = if current {
            hud::palette::PANE_TITLE
        } else {
            hud::palette::EMPHASIS
        };
        let mut title_x = rail.rows.x + char_w * 2.0;
        if let Some(glyph) = entry.glyph {
            painter.ui(glyph.to_string(), title_x, base, m.font_size, ink);
            title_x += char_w * 2.0;
        }
        // The `!` keeps its two cells whether or not it is drawn, so a long
        // title gives way to it and never the other way round.
        let room = rail.rows.x + rail.rows.w - char_w * 2.0 - title_x;
        let name = entry.rail_title.as_deref().unwrap_or(&entry.title);
        let title = if entry.name_cut_at_end {
            cut_at_end_to_fit(painter, name, m.font_size, room)
        } else {
            elided_to_fit(painter, name, m.font_size, room)
        };
        painter.ui(title, title_x, base, m.font_size, ink);
        if entry.marked {
            painter.ui_bold(
                "!",
                rail.rows.x + rail.rows.w - char_w * 2.0,
                base,
                m.font_size,
                hud::palette::ATTENTION,
            );
        }
    }
    if w.below > 0 {
        more(slot, '↓', w.below, lines[w.end..].iter().find_map(page_of));
    }
}

/// Between two tabs of the strip.
const TAB_SEPARATOR: &str = " · ";

/// A tab as the strip draws it: the current one bracketed, so the mark does
/// not lean on colour alone.
fn tab_label(name: &str, current: bool) -> String {
    if current {
        format!("[{name}]")
    } else {
        name.to_string()
    }
}

/// Where the tab strip starts and how wide it may be, given the title's
/// drawn width: one `gap` after the title, in what the title row leaves.
pub(super) fn strip_span(title_x: f32, title_w: f32, gap: f32, avail: f32) -> (f32, f32) {
    (title_x + title_w + gap, avail - title_w - gap)
}

/// Whether every tab name (the current one bracketed) fits in `budget`
/// wide. Measures only, so the fallback to dots can be tested without a
/// painter; `advance` is the text's width at the size the strip draws it.
pub(super) fn tab_strip_fits(
    names: &[&str],
    current: usize,
    budget: f32,
    advance: impl Fn(&str) -> f32,
) -> bool {
    let tabs: f32 = names
        .iter()
        .enumerate()
        .map(|(i, name)| advance(&tab_label(name, i == current)))
        .sum();
    let seps = advance(TAB_SEPARATOR) * names.len().saturating_sub(1) as f32;
    tabs + seps <= budget
}

/// The page's title, then its tab names with the current one marked
/// (`Title   One · [Two] · Three`). A one-tab page draws nothing after the
/// title, and a strip that does not fit falls back to one dot per tab.
fn draw_title(title: &Rect, view: &FrameView, live: bool, painter: &Painter, m: &Metrics) {
    let (pages, page, tab) = (view.pages, view.page, view.screen);
    let base = title.y + title.h * 0.75;
    let name = pages[page].title.as_str();
    painter.ui_bold(name, title.x, base, m.title(), hud::palette::PANE_TITLE);
    let tabs = &pages[page].tabs;
    if tabs.len() < 2 {
        return;
    }
    let title_w = painter.measure_ui_bold_advance(name, m.title());
    let (x, budget) = strip_span(title.x, title_w, m.pad, title.w);
    let advance = |text: &str| painter.measure_ui_advance(text, m.font_size);
    let names: Vec<&str> = tabs.iter().map(|t| t.name).collect();
    if !tab_strip_fits(&names, tab, budget, advance) {
        let dots: Vec<&str> = (0..tabs.len())
            .map(|i| if i == tab { "●" } else { "○" })
            .collect();
        if live {
            // One target per dot: the dot and the space after it.
            let step = advance("● ");
            for i in 0..tabs.len() {
                let bar = Rect::new(x + step * i as f32, title.y, step, title.h);
                let hit = PointerHit::Tab(i);
                tint_if_hovered(view, hit, bar, painter);
                painter.target(bar, hit);
            }
        }
        painter.ui(dots.join(" "), x, base, m.font_size, hud::palette::FAINT);
        return;
    }
    let mut x = x;
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            painter.ui(TAB_SEPARATOR, x, base, m.font_size, hud::palette::FAINT);
            x += advance(TAB_SEPARATOR);
        }
        let label = tab_label(name, i == tab);
        let w = advance(&label);
        if live {
            let bar = Rect::new(x, title.y, w, title.h);
            let hit = PointerHit::Tab(i);
            tint_if_hovered(view, hit, bar, painter);
            painter.target(bar, hit);
        }
        if i == tab {
            painter.rect(x, title.y, w, title.h, SELECT_BG);
            painter.ui(&label, x, base, m.font_size, hud::palette::PANE_TITLE);
        } else {
            painter.ui(&label, x, base, m.font_size, hud::palette::FAINT);
        }
        x += w;
    }
}

/// The keys that work here, most important first (the bar drops groups from
/// the end when it is too narrow). The rail and the rows each list only
/// what the cursor does where it is.
pub(super) fn keybar_groups(
    mode: Mode,
    pages: &[HubPage],
    page: usize,
    on_rail: bool,
    extra_keys: &[(&str, &str)],
    page_keys: &[(&str, &str)],
) -> Vec<Vec<hud::strip::Piece>> {
    let cycle = (!extra_keys.is_empty()).then(|| {
        extra_keys
            .iter()
            .map(|(key, verb)| keycap(key, verb))
            .collect::<Vec<_>>()
    });
    // The hub rule's keys, in both halves: Tab only where the page has a
    // second tab to go to, and the braces only where the rail has sections.
    let mut hub_keys = Vec::new();
    if pages[page].tabs.len() > 1 {
        hub_keys.push(keycap("Tab", "tab"));
    }
    if pages.iter().filter(|p| p.section.is_some()).count() > 1 {
        hub_keys.push(keycap("{ }", "section"));
    }
    if on_rail {
        let nav = vec![keycap("↑/↓", "page"), keycap("→", "open")];
        return [
            Some(nav),
            Some(hub_keys),
            cycle,
            Some(vec![keycap("Esc", "close")]),
        ]
        .into_iter()
        .flatten()
        .filter(|g| !g.is_empty())
        .map(group)
        .collect();
    }
    let mut nav = vec![keycap("←", "list"), keycap("[ ]", "page")];
    nav.extend(hub_keys);
    let mut acts = screen_keys(mode);
    // An `Esc` among the page's keys replaces the rail's: it steps back
    // inside the page first.
    let esc = page_keys
        .iter()
        .find(|(key, _)| *key == "Esc")
        .map_or("list", |(_, verb)| verb);
    acts.extend(
        page_keys
            .iter()
            .filter(|(key, _)| *key != "Esc")
            .map(|(key, verb)| keycap(key, verb)),
    );
    [Some(acts), Some(nav), cycle, Some(vec![keycap("Esc", esc)])]
        .into_iter()
        .flatten()
        .filter(|g| !g.is_empty())
        .map(group)
        .collect()
}

/// What the cursor does on `mode`'s own rows, for the keybar's first group.
fn screen_keys(mode: Mode) -> Vec<Vec<hud::strip::Piece>> {
    match mode {
        Mode::BaseOverview => vec![keycap("↑/↓", "select"), keycap("Enter", "open")],
        Mode::Alerts => vec![keycap("↑/↓", "select"), keycap("x/d", "dismiss")],
        Mode::StaffWellbeing
        | Mode::StaffCoverage
        | Mode::StaffDiscontent
        | Mode::BaseMachines
        | Mode::BasePower
        | Mode::BaseStock
        | Mode::BuildOrders
        | Mode::DigOrders
        | Mode::FloorOrders
        | Mode::Recipes => vec![keycap("↑/↓", "scroll")],
        Mode::WorkOrders => vec![
            keycap("↑/↓", "select"),
            keycap("Enter", "queue"),
            keycap("Bksp", "drop"),
            keycap("< >", "move"),
        ],
        Mode::BaseStaff => vec![
            keycap("↑/↓ ←/→", "cell"),
            keycap("Space", "toggle"),
            keycap("A", "column"),
            keycap("< >", "move"),
        ],
        Mode::Structures => vec![keycap("↑/↓", "select"), keycap("Enter", "staff")],
        Mode::ContractsHeld => vec![
            keycap("↑/↓", "select"),
            keycap("Enter", "hand over"),
            keycap("A", "give back"),
        ],
        Mode::Contracts => vec![keycap("↑/↓", "select"), keycap("Enter", "take")],
        Mode::Inventory => vec![
            keycap("↑/↓", "select"),
            keycap("a-z", "filter"),
            keycap("S", "sell"),
            keycap("U", "fuse all"),
            keycap("A", "autofit"),
            keycap("I", "inspect"),
            keycap("D", "downed"),
        ],
        Mode::Companion | Mode::PartyMember => party::ROSTER_KEYS
            .iter()
            .map(|(key, verb)| keycap(key, verb))
            .collect(),
        Mode::SiteStatus
        | Mode::SiteGoods
        | Mode::SiteRoutes
        | Mode::SiteJobs
        | Mode::SiteLog
        | Mode::OutpostStatus
        | Mode::OutpostCrew
        | Mode::OutpostStock
        | Mode::CaravanSchedule
        | Mode::RoutesInFlight => feral_processes_app_core::sites_screen_keys(mode)
            .iter()
            .map(|(key, verb)| keycap(key, verb))
            .collect(),
        Mode::Research | Mode::RoutineResearch | Mode::AffixResearch | Mode::WeaponResearch => {
            vec![
                keycap("↑/↓ ←/→", "move"),
                keycap("Enter", "work"),
                keycap("G", "list"),
                keycap("A", "abandon"),
            ]
        }
        Mode::Dispatch => vec![keycap("↑/↓", "select"), keycap("S", "send a squad")],
        Mode::Perks => vec![keycap("a-z", "buy a level")],
        Mode::PhaseKeys => vec![keycap("↑/↓", "read")],
        Mode::DownedPrograms => vec![keycap("↑/↓", "select"), keycap("Enter", "extract")],
        Mode::Breed => vec![keycap("↑/↓", "select"), keycap("Enter", "choose")],
        Mode::PinPrisoner => vec![keycap("↑/↓", "select"), keycap("Enter", "hold")],
        Mode::Tools => vec![
            keycap("F", "forge"),
            keycap("I", "install"),
            keycap("X", "pull"),
        ],
        Mode::Transfer => vec![
            keycap("↑/↓", "select"),
            keycap("-/+", "take/put"),
            keycap("0-9", "amount"),
            keycap("Bksp", "erase"),
            keycap("Shift+←/→", "to the end"),
            keycap("Ctrl+←/→", "halve the gap"),
            keycap("A", "take all"),
            keycap("N", "clear"),
            keycap("Enter", "transfer"),
        ],
        Mode::Caravan => vec![
            keycap("↑/↓", "select"),
            keycap("←/→", "set a row"),
            keycap("Shift+←/→", "to the end"),
            keycap("Ctrl+←/→", "halve the gap"),
            keycap("A", "all your cargo"),
            keycap("N", "clear"),
            keycap("I", "inspect"),
            keycap("Enter", "trade"),
        ],
        Mode::DepotFilter => vec![
            keycap("↑/↓", "select"),
            keycap("+/-", "allow/deny"),
            keycap("A", "allow all"),
            keycap("D", "deny all"),
        ],
        Mode::Build | Mode::BuildProgram | Mode::MachineRecipe | Mode::FarmRecipe => {
            vec![keycap("↑/↓", "select"), keycap("Enter", "choose")]
        }
        Mode::Craft => vec![
            keycap("↑/↓", "select"),
            keycap("a-z", "filter"),
            keycap("Enter", "compile"),
        ],
        Mode::RigTool => vec![
            keycap("↑/↓", "select"),
            keycap("R", "pull the tool"),
            keycap("Enter", "fit"),
        ],
        Mode::SpliceRig => vec![
            keycap("↑/↓", "select"),
            keycap("I", "install"),
            keycap("R", "remove"),
        ],
        Mode::GameplayOptions | Mode::GraphicsOptions => {
            vec![keycap("↑/↓", "select"), keycap("Enter", "toggle")]
        }
        Mode::AudioOptions => vec![keycap("↑/↓", "select"), keycap("←/→", "volume")],
        _ => Vec::new(),
    }
}

/// The content rect at a window size, for the censuses that hold a screen's
/// rows against the room the hub gives them.
#[cfg(test)]
pub(in crate::render) fn content_rect_at(
    screen_w: f32,
    screen_h: f32,
    char_w: f32,
    m: &Metrics,
) -> Rect {
    hub_layout(frame_rect(screen_w, screen_h), m, char_w).content
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_app_core::GameKey;

    /// Rail, title, content and keybar tile the frame without overlapping or
    /// escaping it, at every window height the game supports.
    #[test]
    fn the_hub_layout_stays_inside_its_frame_and_overlaps_nothing() {
        with_painter(|p| {
            for h in (600..=2160).step_by(60) {
                let h = h as f32;
                let w = h * 16.0 / 9.0;
                let m = ui_metrics(h);
                let char_w = p.measure_ui_advance("M", m.font_size);
                let frame = frame_rect(w, h);
                let l = hub_layout(frame, &m, char_w);
                let inside = |r: &Rect| {
                    r.x >= frame.x
                        && r.y >= frame.y
                        && r.x + r.w <= frame.x + frame.w + 0.5
                        && r.y + r.h <= frame.y + frame.h + 0.5
                };
                for r in [&l.rail.rect, &l.title, &l.content, &l.keybar] {
                    assert!(inside(r), "a hub box escapes its frame at {h}px");
                }
                assert!(
                    l.content.w > 0.0 && l.content.h > 0.0,
                    "no content at {h}px"
                );
                assert!(
                    l.rail.rect.x + l.rail.rect.w < l.content.x,
                    "rail overlaps content at {h}px"
                );
                assert!(
                    l.content.y + l.content.h <= l.keybar.y,
                    "content overlaps keybar at {h}px"
                );
                assert!(
                    l.title.y + l.title.h <= l.content.y,
                    "title overlaps content at {h}px"
                );
            }
        });
    }

    #[test]
    fn the_keybar_names_the_way_out_of_each_half_on_every_screen() {
        for mode in [
            Mode::BaseOverview,
            Mode::StaffWellbeing,
            Mode::StaffCoverage,
            Mode::StaffDiscontent,
            Mode::BaseOutput,
            Mode::BaseMachines,
            Mode::BasePower,
            Mode::BaseStock,
            Mode::BuildOrders,
            Mode::DigOrders,
            Mode::FloorOrders,
            Mode::Recipes,
            Mode::WorkOrders,
            Mode::BaseStaff,
            Mode::Structures,
            Mode::Alerts,
        ] {
            let pages: Vec<HubPage> = BASE_REPORT_PAGES.iter().map(|p| p.page()).collect();
            let page = pages.iter().position(|p| p.offers(mode)).unwrap();
            let text = |on_rail| -> String {
                keybar_groups(mode, &pages, page, on_rail, &[], &[])
                    .iter()
                    .flatten()
                    .map(|(t, _, _)| t.as_str())
                    .collect()
            };
            let rail = text(true);
            assert!(rail.contains("page"), "{mode:?}: {rail}");
            assert!(rail.contains("open"), "{mode:?}: {rail}");
            assert!(rail.contains("close"), "{mode:?}: {rail}");
            let rows = text(false);
            assert!(rows.contains("list"), "{mode:?}: {rows}");
        }
    }

    fn bar(pages: &[HubPage], page: usize, on_rail: bool) -> Vec<String> {
        keybar_groups(Mode::BaseStock, pages, page, on_rail, &[], &[])
            .iter()
            .map(|g| g.iter().map(|(t, _, _)| t.as_str()).collect())
            .collect()
    }

    /// The keybar teaches the hub rule: `[ ] page` always, `Tab tab` only on
    /// a page with a second tab, `{ } section` only on a rail with
    /// sections, and never the retired `←/→ screen`.
    #[test]
    fn the_keybar_names_the_hub_nav_keys() {
        let pages: Vec<HubPage> = BASE_REPORT_PAGES.iter().map(|p| p.page()).collect();
        let staff = pages.iter().position(|p| p.title == "Staff").unwrap();
        let alerts = pages.iter().position(|p| p.title == "Alerts").unwrap();
        for on_rail in [false, true] {
            let multi = bar(&pages, staff, on_rail).join("|");
            assert!(multi.contains("Tab tab"), "{multi}");
            let single = bar(&pages, alerts, on_rail).join("|");
            assert!(!single.contains("Tab"), "{single}");
            assert!(!multi.contains("{ }"), "no sections here: {multi}");
            assert!(!multi.contains("screen"), "{multi}");
        }
        let page_half = bar(&pages, staff, false).join("|");
        assert!(page_half.contains("[ ] page") && page_half.contains("← list"));

        let sectioned = [
            page("Aa", Some("Towns and cities"), false),
            page("Bb", Some("Outposts"), false),
        ];
        for on_rail in [false, true] {
            let text = bar(&sectioned, 0, on_rail).join("|");
            assert!(text.contains("{ } section"), "{text}");
        }
    }

    const TAB: &[(&str, &str)] = &[("W", "work it")];

    fn bar_text(extra_keys: &[(&str, &str)], on_rail: bool) -> String {
        let pages = [feral_processes_app_core::structure_page(
            feral_processes_engine::StructurePage::Inventory,
        )
        .page()];
        keybar_groups(Mode::Transfer, &pages, 0, on_rail, extra_keys, &[])
            .iter()
            .flatten()
            .map(|(t, _, _)| t.as_str())
            .collect()
    }

    /// A hub's own keys reach the bar from both halves, and only when the
    /// hub offers them.
    #[test]
    fn a_hubs_own_keys_are_on_the_bar_from_both_halves() {
        for on_rail in [false, true] {
            assert!(bar_text(TAB, on_rail).contains("work it"));
            assert!(!bar_text(&[], on_rail).contains("work it"));
        }
    }

    /// A key a page has only in some states (`T` train, `U` unlock) is on the
    /// bar beside the page's fixed keys, and only with the cursor in the page.
    #[test]
    fn a_state_dependent_page_key_is_on_the_bar_off_the_rail_only() {
        let pages = [feral_processes_app_core::structure_page(
            feral_processes_engine::StructurePage::Inventory,
        )
        .page()];
        let text = |keys: &[(&str, &str)], on_rail| -> String {
            keybar_groups(Mode::Research, &pages, 0, on_rail, &[], keys)
                .iter()
                .flatten()
                .map(|(t, _, _)| t.as_str())
                .collect()
        };
        let offered = [("T", "train your AI"), ("U", "unlock")];
        let rows = text(&offered, false);
        assert!(
            rows.contains("train your AI") && rows.contains("unlock"),
            "{rows}"
        );
        assert!(!text(&offered, true).contains("train your AI"));
        assert!(!text(&[], false).contains("train your AI"));
    }

    /// With a Deploy category open Esc steps back to the categories, and the
    /// bar says `back`, not `list`.
    #[test]
    fn esc_is_labelled_back_where_it_steps_back_inside_the_page() {
        let pages: Vec<HubPage> = BASE_REPORT_PAGES.iter().map(|p| p.page()).collect();
        let page = pages.iter().position(|p| p.offers(Mode::Build)).unwrap();
        let text = |keys: &[(&str, &str)]| -> String {
            keybar_groups(Mode::Build, &pages, page, false, &[], keys)
                .iter()
                .flatten()
                .map(|(t, _, _)| t.as_str())
                .collect::<Vec<_>>()
                .join("")
        };
        assert!(text(&[]).contains("Esc list"), "{}", text(&[]));
        let open = text(&[("Esc", "back")]);
        assert!(
            open.contains("Esc back") && !open.contains("Esc list"),
            "{open}"
        );
    }

    /// A structure's page reads like the base report's: its own keys, then
    /// the way back to the rail.
    #[test]
    fn a_structure_page_names_its_keys_and_the_way_back_like_any_hub_page() {
        let rows = bar_text(TAB, false);
        for want in ["take/put", "←", "list", "page", "work it"] {
            assert!(rows.contains(want), "{want:?} missing: {rows}");
        }
    }

    /// A modded structure's name is cut to make room; the position in the
    /// cycle is the part that must survive.
    #[test]
    fn a_long_structure_name_is_elided_and_the_cycle_position_survives() {
        let m = ui_metrics(720.0);
        let name = "W".repeat(80);
        let page = feral_processes_app_core::structure_page(
            feral_processes_engine::StructurePage::Inventory,
        )
        .page();
        let (_, shapes) = with_painter(|p| {
            paint_frame(
                &FrameView {
                    title: (&name, " — north (2 of 3)"),
                    pages: std::slice::from_ref(&page),
                    page: 0,
                    screen: 0,
                    extra_keys: TAB,
                    page_keys: &[],
                    hover: None,
                },
                None,
                false,
                None,
                p,
                &m,
            );
        });
        let drawn = crate::paint::painted_text(&shapes);
        assert!(
            drawn.iter().any(|t| t.contains("(2 of 3)")),
            "the position was cut: {drawn:?}"
        );
        assert!(
            !drawn.iter().any(|t| t.contains(&name)),
            "the name was not elided: {drawn:?}"
        );
    }

    fn page(title: &str, section: Option<&'static str>, marked: bool) -> HubPage {
        HubPage {
            key: feral_processes_app_core::HubPageKey::Named("test"),
            title: title.to_string(),
            glyph: None,
            section,
            tabs: vec![feral_processes_app_core::HubTab {
                name: "Alerts",
                mode: Mode::Alerts,
            }],
            marked,
            name_cut_at_end: false,
            rail_title: None,
        }
    }

    fn rail_text(pages: &[HubPage], selected: usize, h: f32) -> Vec<String> {
        let m = ui_metrics(h);
        let (_, shapes) = with_painter(|p| {
            paint_frame(
                &FrameView {
                    title: ("HUB", ""),
                    pages,
                    page: selected,
                    screen: 0,
                    extra_keys: &[],
                    page_keys: &[],
                    hover: None,
                },
                None,
                true,
                None,
                p,
                &m,
            );
        });
        crate::paint::painted_text(&shapes)
    }

    /// The tab strip's width census at 1280x720, the smallest window the
    /// game supports (`popup_row_width`'s precedent: measure headlessly, pin
    /// the exceptions as a literal). Every multi-tab page of every hub, with
    /// each of its tabs current, since the bracketed tab is the widest.
    /// Site pages carry the town's name as their title, so they are measured
    /// under a worst-case 20-cell name.
    #[test]
    fn every_tab_strip_fits_at_the_smallest_window_or_is_listed() {
        use feral_processes_app_core::{
            BASE_REPORT_PAGES, CARAVAN_TABS, HubTab, INVENTORY_PAGES, MEMBER_TABS, OUTPOST_TABS,
            TOWN_TABS,
        };
        use feral_processes_engine::StructurePage;
        /// Pages whose strip falls back to dots at 1280x720: a Workbench
        /// with the weapon pages researched has seven tabs.
        const FALLBACK: &[&str] = &["Workbench"];
        let long_name = "W".repeat(20);
        let mut pages: Vec<(String, &[HubTab])> = BASE_REPORT_PAGES
            .iter()
            .chain(INVENTORY_PAGES)
            .map(|p| (p.title.to_string(), p.tabs))
            .collect();
        // The fullest structure: a Workbench with the weapon pages researched.
        let bench: Vec<HubTab> = [
            StructurePage::Inventory,
            StructurePage::Build,
            StructurePage::Built,
            StructurePage::Weapon,
            StructurePage::Salvage,
            StructurePage::Compile,
            StructurePage::Info,
        ]
        .map(|p| feral_processes_app_core::structure_page(p).tabs[0])
        .to_vec();
        pages.push(("Workbench".to_string(), &bench));
        pages.push((long_name.clone(), MEMBER_TABS));
        pages.push((long_name.clone(), TOWN_TABS));
        pages.push((long_name, OUTPOST_TABS));
        pages.push(("Caravans".to_string(), CARAVAN_TABS));
        let m = ui_metrics(720.0);
        let mut fell_back = Vec::new();
        let mut multi_tab_pages = 0;
        with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let title_row = hub_layout(frame_rect(1280.0, 720.0), &m, char_w).title;
            for (title, tabs) in &pages {
                if tabs.len() < 2 {
                    continue;
                }
                multi_tab_pages += 1;
                let names: Vec<&str> = tabs.iter().map(|t| t.name).collect();
                let title_w = p.measure_ui_bold_advance(title, m.title());
                let (_, budget) = strip_span(title_row.x, title_w, m.pad, title_row.w);
                let advance = |t: &str| p.measure_ui_advance(t, m.font_size);
                let fits = (0..tabs.len()).all(|i| tab_strip_fits(&names, i, budget, advance));
                if !fits {
                    fell_back.push(title.as_str());
                }
            }
            let expected: Vec<&str> = FALLBACK.to_vec();
            assert_eq!(fell_back, expected);
            assert!(multi_tab_pages > 0, "the census measured no strip");
        });
    }

    #[test]
    fn a_strip_that_does_not_fit_is_measured_as_not_fitting() {
        let advance = |t: &str| t.chars().count() as f32;
        // "One" 3 + "[Two]" 5 + one separator of 3.
        let names = ["One", "Two"];
        assert!(tab_strip_fits(&names, 1, 11.0, advance));
        assert!(!tab_strip_fits(&names, 1, 10.9, advance));
    }

    #[test]
    fn the_strip_starts_one_gap_after_the_title_and_gets_what_is_left() {
        assert_eq!(strip_span(10.0, 50.0, 4.0, 200.0), (64.0, 146.0));
    }

    #[test]
    fn the_strip_draws_names_with_the_current_one_bracketed() {
        let m = ui_metrics(720.0);
        let staff = BASE_REPORT_PAGES.iter().find(|p| p.title == "Staff");
        let pages = [staff.unwrap().page()];
        let (_, shapes) = with_painter(|p| {
            paint_frame(
                &FrameView {
                    title: ("BASE REPORT", ""),
                    pages: &pages,
                    page: 0,
                    screen: 1,
                    extra_keys: &[],
                    page_keys: &[],
                    hover: None,
                },
                None,
                true,
                None,
                p,
                &m,
            );
        });
        let drawn = crate::paint::painted_text(&shapes);
        for want in ["Wellbeing", "[Coverage]", "Discontent"] {
            assert!(
                drawn.iter().any(|t| t == want),
                "{want:?} missing: {drawn:?}"
            );
        }
        assert!(!drawn.iter().any(|t| t == "●"), "{drawn:?}");
    }

    #[test]
    fn a_section_header_draws_above_its_first_page() {
        let pages = [
            page("Aa", Some("Towns and cities"), false),
            page("Bb", None, false),
            page("Cc", Some("Outposts"), false),
        ];
        let drawn = rail_text(&pages, 0, 720.0);
        let at = |t: &str| drawn.iter().position(|d| d.trim() == t).unwrap();
        assert!(at("Towns and cities") < at("Aa"), "{drawn:?}");
        assert!(at("Outposts") < at("Cc"), "{drawn:?}");
        assert!(at("Bb") < at("Outposts"), "{drawn:?}");
    }

    #[test]
    fn a_long_rail_title_is_elided_and_keeps_its_mark() {
        let name = "W".repeat(80);
        let mut long = page(&name, None, true);
        long.glyph = Some('M');
        let pages = [long, page("Short", None, false)];
        let drawn = rail_text(&pages, 1, 720.0);
        assert!(!drawn.iter().any(|t| t.contains(&name)), "{drawn:?}");
        assert!(drawn.iter().any(|t| t == "!"), "{drawn:?}");
        assert!(drawn.iter().any(|t| t == "M"), "{drawn:?}");
        let m = ui_metrics(720.0);
        with_painter(|p| {
            let char_w = p.measure_ui_advance("M", m.font_size);
            let title = drawn.iter().find(|t| t.contains('W')).unwrap();
            // Caret, glyph and `!` take four cells of the rail.
            assert!(
                p.measure_ui_advance(title, m.font_size) + char_w * 4.0
                    <= RAIL_CELLS as f32 * char_w + 0.5,
                "{title:?} runs past the rail"
            );
        });
    }

    #[test]
    fn the_selected_page_stays_in_view_when_sections_take_rows() {
        let sections = ["One", "Two", "Three"];
        let pages: Vec<HubPage> = (0..30)
            .map(|i| {
                page(
                    &format!("Site {i}"),
                    (i % 10 == 0).then(|| sections[i / 10]),
                    false,
                )
            })
            .collect();
        let drawn = rail_text(&pages, 29, 720.0);
        assert!(drawn.iter().any(|t| t == "▸"), "{drawn:?}");
        assert!(drawn.iter().any(|t| t == "Site 29"), "{drawn:?}");
    }

    #[test]
    fn a_fixed_rail_draws_exactly_as_before() {
        let pages: Vec<HubPage> = BASE_REPORT_PAGES.iter().map(|p| p.page()).collect();
        let lines = rail_lines(&pages);
        assert_eq!(lines.len(), pages.len());
        assert!(lines.iter().all(|l| matches!(l, RailLine::Page(_))));
    }

    fn base_pages() -> Vec<HubPage> {
        feral_processes_app_core::BASE_REPORT_PAGES
            .iter()
            .map(|p| p.page())
            .collect()
    }

    /// What `paint_frame` registers for the base report's Staff page (four
    /// tabs), keybar on, at 1440x900.
    fn staff_targets(hover: Option<PointerHit>, live: bool) -> Vec<(Rect, PointerHit)> {
        let pages = base_pages();
        let staff = pages.iter().position(|p| p.title == "Staff").unwrap();
        let m = ui_metrics(900.0);
        crate::paint::with_targets(|p| {
            paint_frame(
                &FrameView {
                    title: ("BASE", ""),
                    pages: &pages,
                    page: staff,
                    screen: 0,
                    extra_keys: &[],
                    page_keys: &[],
                    hover,
                },
                live.then_some(Mode::StaffWellbeing),
                true,
                None,
                p,
                &m,
            );
        })
    }

    fn hits(targets: &[(Rect, PointerHit)]) -> Vec<PointerHit> {
        targets.iter().map(|(_, h)| *h).collect()
    }

    /// Every rail entry, every tab name and the keybar's keys are targets,
    /// under a background region for each half.
    #[test]
    fn the_frame_registers_its_rail_tabs_and_keys() {
        let pages = base_pages();
        let hit = hits(&staff_targets(None, true));
        assert_eq!(hit[0], PointerHit::Region(Region::Rail));
        assert_eq!(hit[1], PointerHit::Region(Region::Page));
        for p in &pages {
            assert!(hit.contains(&PointerHit::Page(p.key)), "{}", p.title);
        }
        let staff = pages.iter().find(|p| p.title == "Staff").unwrap();
        for i in 0..staff.tabs.len() {
            assert!(hit.contains(&PointerHit::Tab(i)), "tab {i}");
        }
        for key in [GameKey::Tab, GameKey::Up, GameKey::Down, GameKey::Esc] {
            assert!(hit.contains(&PointerHit::Key(key)), "{key:?}");
        }
    }

    /// The frame behind a popup child registers nothing: a click there is
    /// the popup's, and nothing under it may take one.
    #[test]
    fn a_popup_parents_frame_registers_nothing() {
        assert!(staff_targets(None, false).is_empty());
    }

    /// Hover tints the target under the pointer and only it, and not the
    /// cursor's own row, which already wears the stronger fill.
    #[test]
    fn hover_tints_one_target_and_not_the_held_one() {
        let count = |hover| {
            let pages = base_pages();
            let staff = pages.iter().position(|p| p.title == "Staff").unwrap();
            let m = ui_metrics(900.0);
            let (_, shapes) = with_painter(|p| {
                paint_frame(
                    &FrameView {
                        title: ("BASE", ""),
                        pages: &pages,
                        page: staff,
                        screen: 0,
                        extra_keys: &[],
                        page_keys: &[],
                        hover,
                    },
                    Some(Mode::StaffWellbeing),
                    true,
                    None,
                    p,
                    &m,
                );
            });
            crate::paint::painted_rect_fill_count(&shapes, hud::palette::HOVER_BG)
        };
        let pages = base_pages();
        let other = pages.iter().find(|p| p.title == "Build").unwrap().key;
        let held = pages.iter().find(|p| p.title == "Staff").unwrap().key;
        assert_eq!(count(None), 0);
        assert_eq!(count(Some(PointerHit::Page(other))), 1);
        assert_eq!(count(Some(PointerHit::Tab(2))), 1);
        assert_eq!(count(Some(PointerHit::Key(GameKey::Esc))), 1);
        assert_eq!(count(Some(PointerHit::Page(held))), 0, "the cursor's own");
    }

    /// A keycap's label is the key it presses, `↑/↓` and `[ ]` one key per
    /// half, and a range like `1–9` is no key at all.
    #[test]
    fn keycaps_press_the_keys_they_spell() {
        let m = ui_metrics(900.0);
        let bar = Rect::new(0.0, 0.0, 1400.0, m.line_height);
        let caps = |chips: Vec<Vec<hud::strip::Piece>>| {
            crate::paint::with_painter(|p| keycap_targets(bar, &[group(chips)], p, &m)).0
        };
        let keys = |chips| caps(chips).into_iter().map(|(_, k)| k).collect::<Vec<_>>();
        assert_eq!(
            keys(vec![keycap("↑/↓", "page"), keycap("[ ]", "turn")]),
            [
                GameKey::Up,
                GameKey::Down,
                GameKey::Char('['),
                GameKey::Char(']')
            ]
        );
        assert_eq!(
            keys(vec![keycap("Enter", "open"), keycap("x/d", "dismiss")]),
            [GameKey::Enter, GameKey::Char('x'), GameKey::Char('d')]
        );
        assert!(keys(vec![keycap("1–9", "jump")]).is_empty());
        // The halves of a two-key cap tile its own text, left then right.
        let two = caps(vec![keycap("↑/↓", "page")]);
        assert!(two[0].0.x + two[0].0.w <= two[1].0.x + 0.01);
    }

    /// The hover tint is the cursor's colour at 40% of its strength.
    #[test]
    fn the_hover_tint_is_the_cursor_at_two_fifths_alpha() {
        let (h, s) = (hud::palette::HOVER_BG, crate::render::SELECT_BG);
        assert_eq!((h.r, h.g, h.b), (s.r, s.g, s.b));
        assert!((h.a - s.a * 0.4).abs() < 1e-6);
    }
}
