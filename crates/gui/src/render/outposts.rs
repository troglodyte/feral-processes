//! An outpost's own page (`Mode::OutpostVisit`), its staff picker
//! (`Mode::OutpostPost`), and the tile marks the surface map draws for it —
//! design spec §9. `Game::outpost_report`/`Game::outpost_marks` are the one
//! derivation each of these reads; nothing here builds a sentence the
//! engine did not already build.

use super::marks::{draw_progress_bar, outpost_pip_rects};
use super::popup::*;
use super::terrain::at_level;
use super::*;
use feral_processes_engine::outposts::Trend;
use feral_processes_engine::tuning::OUTPOST_CREW_CAP;

/// The outpost glyph's own hue. `GlyphColor::Orange` is a settlement's —
/// the one variant no species authors, `base.rs`'s own reason a patrol
/// borrows it — so an outpost, a different landmark, needs a hue of its
/// own rather than reading as a second kind of town. `Brown` is otherwise
/// unclaimed (`CLAUDE.md`'s "Brown and orange stand outside the handoff's
/// sixteen deliberately") and reads as ground-coloured, which is fitting
/// for a fixture built into the terrain. Unmeasured, like every other
/// figure this feature draws — a playtest may want something louder.
fn outpost_glyph_color() -> Color {
    hud::palette::glyph(GlyphColor::Brown)
}

/// The growth bar's colour — design spec §9: "its own hue / not animated /
/// `WARN` / `ATTENTION`", one term per `Trend` variant in the table's own
/// order. `Declining` reads as the more severe of the two warning colours
/// because it is the state that is actively getting worse, `machine_color`'s
/// own ladder (`Clogged`/`Stranded` outrank `Starved`/`Unstaffed` for the
/// same reason).
pub(super) fn outpost_trend_color(trend: Trend) -> Color {
    match trend {
        Trend::Growing => hud::palette::HEALTHY,
        Trend::Stable => hud::palette::FAINT,
        Trend::Stale => hud::palette::WARN,
        Trend::Declining => hud::palette::ATTENTION,
    }
}

/// How dim a dark outpost's glyph draws — design spec §9's "the glyph is
/// dimmed and there is no bar."
const DARK_GLYPH_LEVEL: f32 = 0.35;

/// Draws every outpost's glyph, growth bar and tier pips — `DigMark`'s own
/// pass shape (`base.rs::draw_excavation_plan`), a pass of its own after the
/// tile loop rather than three more branches inside it, since none of this
/// is a property of a *tile* the loop already owns.
///
/// `occupant` answers `Some(is_nemesis)` when the earlier entity pass
/// already drew something on `mark.tile` — the player's `@` or a wild
/// program — and `None` for bare ground (I4). The glyph never draws over an
/// entity's own; the growth bar holds the bottom edge, which every corner
/// mark that could share a tile is built to lift clear of
/// (`marks.rs`'s corner census), so it draws regardless. The tier pips sit
/// in the nemesis mark's own top-right corner, so they draw unless the
/// occupant is a nemesis.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_outpost_marks(
    painter: &Painter,
    marks: &[OutpostMark],
    at: impl Fn((i32, i32)) -> (f32, f32),
    tile_px: f32,
    glyph_px: u16,
    pane: Rect,
    occupant: impl Fn((i32, i32)) -> Option<bool>,
) {
    for mark in marks {
        let (px, py) = at(mark.tile);
        if px >= pane.x + pane.w
            || py >= pane.y + pane.h
            || px + tile_px <= pane.x
            || py + tile_px <= pane.y
        {
            continue;
        }
        let occupant_here = occupant(mark.tile);
        if occupant_here.is_none() {
            let color = if mark.dark {
                at_level(outpost_glyph_color(), DARK_GLYPH_LEVEL)
            } else {
                outpost_glyph_color()
            };
            let glyph = mark.glyph.to_string();
            let dims = painter.measure_map(&glyph, glyph_px);
            painter.map(
                &glyph,
                px + (tile_px - dims.width) / 2.0,
                py + (tile_px + dims.height) / 2.0,
                glyph_px,
                color,
            );
        }
        if mark.dark {
            continue;
        }
        draw_progress_bar(
            painter,
            Some(mark.fill),
            px,
            py,
            tile_px,
            outpost_trend_color(mark.trend),
            1.0,
        );
        if occupant_here != Some(true) {
            let pip_color = outpost_glyph_color();
            for rect in outpost_pip_rects(px, py, tile_px, mark.tier + 1) {
                painter.rect(rect.x, rect.y, rect.w, rect.h, pip_color);
            }
        }
    }
}

/// The growth bar's text rendering for the popup page — a bracketed ASCII
/// gauge, since `draw_popup` is text rows and has no bar primitive of its
/// own. `width` is in characters.
fn text_growth_bar(fill: f32, width: usize) -> String {
    let filled = ((fill.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    format!("[{}{}]", "#".repeat(filled), "-".repeat(width - filled))
}

const GROWTH_BAR_WIDTH: usize = 20;

/// `Mode::OutpostVisit`'s page — every figure comes out of
/// `Game::outpost_report` alone, `draw_companion_memories`'s own rule: a
/// renderer that formatted a trend or a growth figure itself would be a
/// fifth screen keeping a private copy of a formula the engine already
/// owns.
pub(super) fn draw_outpost_visit(
    game: &mut Game,
    tile: Option<(i32, i32)>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let report = tile.and_then(|t| game.outpost_report(t));
    let Some(report) = report else {
        draw_popup(
            "Outpost",
            PopupSize::Small,
            &[text_row("Nothing to report.")],
            refusal,
            painter,
            m,
        );
        return;
    };
    let rows = outpost_page_rows(&report, selected);
    draw_popup("Outpost", PopupSize::Large, &rows, refusal, painter, m);
}

/// The page's rows, out of an `OutpostReport` alone rather than a `Game` —
/// `memory_page_rows`'s split: the height and width censuses have to
/// measure the page at its worst case, and a report a fixture builds by
/// hand is a state a test can state outright rather than one a `Game`
/// would have to be played into.
pub(super) fn outpost_page_rows(report: &OutpostReport, selected: usize) -> Vec<Row> {
    let mut rows = vec![
        Row::TextColored(
            format!(
                "{} · {:?} — Tier {} · {}",
                report.name,
                report.biome,
                report.tier + 1,
                report.tier_label
            ),
            CYAN,
        ),
        text_row(format!(
            "Growth  {}",
            text_growth_bar(report.growth_fill, GROWTH_BAR_WIDTH)
        )),
        text_row(report.reason.clone()),
        text_row(format!(
            "Integrity {}/{}   Stock {}/{}{}",
            report.integrity,
            report.max_integrity,
            report.stock,
            report.stock_cap,
            match &report.route {
                Some(line) => format!("   Route: {line}"),
                None => String::new(),
            }
        )),
        text_row(""),
        Row::TextColored(
            format!("CREW ({}/{OUTPOST_CREW_CAP})", report.crew.len()),
            TEXT,
        ),
    ];
    if report.crew.is_empty() {
        rows.push(text_row("  Nobody is posted here."));
    }
    for (i, member) in report.crew.iter().enumerate() {
        let letter = (b'a' + i as u8) as char;
        rows.push(item_row(
            format!(
                "{letter}  {}  {}  lv {}",
                member.name, member.species, member.level
            ),
            i == selected,
        ));
    }
    rows.push(text_row(""));
    rows.push(Row::TextColored("YIELDS NOW".to_string(), TEXT));
    if report.yields.is_empty() {
        rows.push(text_row("  Nothing yet."));
    }
    for y in &report.yields {
        rows.push(text_row(format!("  {}   tier {}", y.name, y.tier)));
    }
    if let Some(next) = &report.next_tier_requirement {
        rows.push(text_row(format!("  {next}")));
    }
    rows.push(text_row(""));
    rows.push(text_row("[P] Post  [U] Recall  [R] Repair  [C] Take stock"));
    rows
}

/// `Mode::OutpostPost`'s staff picker — every base-staff program, a letter
/// posts one. `App::outpost_post_candidates`'s rows are already resolved
/// display names, `SortieSquadRow`'s own shape one field narrower.
pub(super) fn draw_outpost_post(
    candidates: &[feral_processes_app_core::OutpostPostRow],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![Row::TextColored("Post to the outpost".to_string(), CYAN)];
    if candidates.is_empty() {
        rows.push(text_row("Nobody on staff is free to post."));
    }
    for (i, row) in candidates.iter().enumerate() {
        let letter = (b'a' + i as u8) as char;
        rows.push(item_row(format!("{letter}  {}", row.name), i == selected));
    }
    draw_popup("Post", PopupSize::Large, &rows, refusal, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{painted_rect_fill_count, painted_text, with_painter};
    use crate::text::ui_metrics;
    use feral_processes_engine::items::ItemId;
    use feral_processes_engine::outposts::Trend;
    use feral_processes_engine::world::Biome;
    use feral_processes_engine::{OutpostCrewRow, OutpostYieldRow};

    fn tallest_report() -> OutpostReport {
        OutpostReport {
            tile: (0, 0),
            name: "Outpost".to_string(),
            biome: Biome::Deadlock,
            tier: 2,
            tier_label: "Complex",
            growth_fill: 1.0,
            trend: Trend::Stable,
            reason: "Stable: max tier reached".to_string(),
            integrity: 100,
            max_integrity: 100,
            stock: 60,
            stock_cap: 60,
            route: Some("running toward Anchorpoint Station".to_string()),
            crew: (0..OUTPOST_CREW_CAP)
                .map(|i| OutpostCrewRow {
                    name: format!("Overclocked 0x{i:04x} Scrapper"),
                    species: "Scrapper".to_string(),
                    level: 99,
                })
                .collect(),
            yields: vec![
                OutpostYieldRow {
                    item: ItemId("salvage_coil".to_string()),
                    name: "Salvage Coil".to_string(),
                    tier: 1,
                },
                OutpostYieldRow {
                    item: ItemId("cache_grain".to_string()),
                    name: "Cache Grain".to_string(),
                    tier: 2,
                },
                OutpostYieldRow {
                    item: ItemId("core_fragment".to_string()),
                    name: "Core Fragment".to_string(),
                    tier: 3,
                },
            ],
            next_tier_requirement: None,
            dark: false,
        }
    }

    /// **The page has no scroll.** `draw_popup` pages a `Row::Item` span and
    /// this page has a fixed one — `OUTPOST_CREW_CAP` rows — so a page built
    /// at the cap must still fit inside the popup at every window height the
    /// UI supports, `the_tallest_memory_page_fits_its_popup`'s own trap.
    #[test]
    fn the_tallest_outpost_page_fits_its_popup() {
        let rows = outpost_page_rows(&tallest_report(), 0).len();
        for h in (600..=2160).step_by(60) {
            let m = ui_metrics(h as f32);
            let cap = popup_max_rows(h as f32, PopupSize::Large, &m);
            assert!(
                rows + REFUSAL_MAX_LINES <= cap,
                "a full outpost builds a {rows}-row page into a {cap}-row popup at {h}px"
            );
        }
    }

    /// The other axis: `draw_row` clips a row vertically and never
    /// horizontally, so a row past the right edge is simply lost —
    /// `no_memory_row_overflows_its_popup`'s own census.
    #[test]
    fn no_outpost_row_overflows_its_popup() {
        let rows = outpost_page_rows(&tallest_report(), 0);
        with_painter(|p| {
            let m = ui_metrics(900.0);
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for row in &rows {
                let line = match &row {
                    Row::Text(t) | Row::TextColored(t, _) => t,
                    Row::Item { text, .. } => text,
                };
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "an outpost row overflows the page by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// A dark outpost's glyph is genuinely darker than a healthy one's —
    /// design spec §9's "the glyph is dimmed". `outpost_glyph_color` is
    /// never pure black (`Brown`'s own def), so a dimming factor that did
    /// nothing (or one bugged to 1.0) would leave a dark outpost reading
    /// identically to a standing one on the map.
    #[test]
    fn a_dark_outposts_glyph_is_dimmer_than_a_healthy_ones() {
        let bright = outpost_glyph_color();
        let dark = at_level(bright, DARK_GLYPH_LEVEL);
        assert!(dark.r < bright.r);
        assert!(dark.g < bright.g);
        assert!(dark.b < bright.b);
    }

    fn a_mark() -> OutpostMark {
        OutpostMark {
            tile: (0, 0),
            glyph: 'O',
            tier: 1,
            fill: 0.5,
            trend: Trend::Growing,
            dark: false,
        }
    }

    fn draw_one_mark(
        mark: &OutpostMark,
        occupant: impl Fn((i32, i32)) -> Option<bool>,
    ) -> Vec<bevy_egui::egui::epaint::ClippedShape> {
        let (_, shapes) = with_painter(|p| {
            draw_outpost_marks(
                p,
                std::slice::from_ref(mark),
                |_| (0.0, 0.0),
                CELL,
                CELL_GLYPH_PX,
                Rect::new(0.0, 0.0, CELL, CELL),
                occupant,
            )
        });
        shapes
    }

    const CELL: f32 = 20.0;
    const CELL_GLYPH_PX: u16 = 16;

    /// I4: the outpost's own glyph never draws over an entity's — the
    /// player's `@` or a wild program standing on the tile.
    #[test]
    fn glyph_draws_when_the_tile_is_unoccupied() {
        let mark = a_mark();
        let shapes = draw_one_mark(&mark, |_| None);
        assert!(painted_text(&shapes).contains(&"O".to_string()));
    }

    #[test]
    fn glyph_is_suppressed_when_an_entity_stands_on_the_tile() {
        let mark = a_mark();
        let shapes = draw_one_mark(&mark, |_| Some(false));
        assert!(
            !painted_text(&shapes).contains(&"O".to_string()),
            "the outpost glyph must not paint over the entity's own"
        );
    }

    /// The growth bar holds the bottom edge, which every corner mark that
    /// could share a tile is built to lift clear of — `marks.rs`'s corner
    /// census — so it keeps drawing whether or not the tile is occupied.
    #[test]
    fn the_growth_bar_still_draws_over_a_non_nemesis_occupant() {
        let mark = a_mark();
        let shapes = draw_one_mark(&mark, |_| Some(false));
        assert!(
            painted_rect_fill_count(&shapes, outpost_trend_color(mark.trend)) > 0,
            "the growth bar's fill must still be drawn"
        );
    }

    /// The tier pips share the nemesis mark's top-right corner
    /// (`marks.rs`'s corner census), so a nemesis standing on the tile wins
    /// that corner and the pips must not draw over it.
    #[test]
    fn pips_are_suppressed_when_a_nemesis_occupies_the_tile() {
        let mark = a_mark();
        let shapes = draw_one_mark(&mark, |_| Some(true));
        assert_eq!(
            painted_rect_fill_count(&shapes, outpost_glyph_color()),
            0,
            "the pips must not collide with the nemesis mark in the same corner"
        );
    }

    /// A non-nemesis occupant (the player, an ordinary wild program) does not
    /// contest that corner, so the pips still draw.
    #[test]
    fn pips_still_draw_over_a_non_nemesis_occupant() {
        let mark = a_mark();
        let shapes = draw_one_mark(&mark, |_| Some(false));
        assert!(
            painted_rect_fill_count(&shapes, outpost_glyph_color()) > 0,
            "the pips must still draw when nothing contests their corner"
        );
    }
}
