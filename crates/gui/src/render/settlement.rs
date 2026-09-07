//! The settlement hub — identity: name, kind, specialty, temperament, blurb,
//! and now (Phase 3) the door onto its market. `Mode::CompanionMemories`'s
//! shape one level over: opened two ways (a bump, or `x`) that both land on
//! the same page.

use super::popup::*;
use super::*;

pub(super) fn draw_settlement(
    game: &mut Game,
    key: Option<SettlementKey>,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    // `App::pending_settlement` is set from either door the instant the
    // screen opens — the bump's drain, or `Game::settlement_key` off an
    // examined entity — so `None` here is not a state a player can reach by
    // playing; it is the same "the subject is gone" shape every other
    // popup in this file falls back to rather than assuming its argument.
    let Some(key) = key else {
        draw_popup(
            "Settlement",
            PopupSize::Small,
            &[text_row("Nothing to report.")],
            refusal,
            painter,
            m,
        );
        return;
    };
    let view = game.settlement_report(key);
    let rows = settlement_page_rows(&view);
    draw_popup("Settlement", PopupSize::Large, &rows, refusal, painter, m);
}

/// The page's rows, out of a `SettlementView` alone rather than a `Game` —
/// `memory_page_rows`' split, and for its reason: the width and height
/// censuses have to measure the page at its worst case, and a view built by
/// hand is a state a fixture can state outright rather than one a `Game`
/// would have to be played into.
pub(super) fn settlement_page_rows(view: &SettlementView) -> Vec<Row> {
    let mut rows = vec![
        // A call to the same door the map glyph is drawn through
        // (`spawn_settlement_at`'s `Glyph { color: GlyphColor::Orange }`,
        // resolved by `glyph_color`), not a second, hand-copied orange —
        // that is what keeps "what am I looking at" answered the same way
        // on both surfaces. See CLAUDE.md: "A doc comment claiming to
        // mirror other code must be a call, not a copy."
        Row::TextColored(view.name.clone(), glyph_color(GlyphColor::Orange)),
        text_row(format!(
            "{}  ·  {}  ·  {}",
            view.kind, view.specialty, view.temperament
        )),
        // Its own row rather than a fourth token on the line above: that
        // line is identity — what this place *is* — and standing is the one
        // thing on the page that changes while the party stands there.
        text_row(format!("They regard you as {}.", view.standing)),
    ];
    // Its own row, and only for a city. A Server has no band, and the row
    // is dropped rather than drawn with a word that could never change —
    // `SettlementView::vitality`'s own reason.
    if let Some(vitality) = view.vitality {
        rows.push(text_row(format!("The place is {vitality}.")));
    }
    rows.push(text_row(""));
    // What this town is currently worth, one row per aid — built in the
    // engine (`Game::settlement_aid_lines`) rather than here, because a
    // read-only page's rows are the engine's and a phrase assembled in a
    // renderer is a transform in the wrong crate. An empty list draws
    // nothing at all rather than a row saying "no": the page is about what
    // this place is worth, not a checklist.
    if !view.aid.is_empty() {
        for line in &view.aid {
            rows.push(text_row(line.clone()));
        }
        rows.push(text_row(""));
    }
    // `wrap_text`'s pattern from `stack::cell_describe_rows` and every other
    // prose-on-screen page in this file: `draw_row` clips a row vertically
    // and never horizontally, so an author's blurb — free text, unbounded —
    // has to be wrapped rather than trusted to fit. `Kernel Reach`'s blurb
    // is 632px past this popup's body at 1440x900 unwrapped, which is what
    // `no_settlement_row_overflows_its_popup` caught.
    rows.extend(
        wrap_text(&view.blurb, DESCRIBE_WRAP_COLUMNS)
            .into_iter()
            .map(text_row),
    );
    rows.push(text_row(""));
    // Uppercase — `lowercase-letters-are-row-selectors`'s rule — even though
    // this page has no rows to select, since a modder's free-text blurb
    // could otherwise collide with a lowercase key.
    rows.push(text_row("[M] Market  ·  [J] Jobs  ·  Esc to go back"));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::render::popup::{REFUSAL_MAX_LINES, popup_max_rows};
    use crate::text::ui_metrics;
    use feral_processes_engine::settlements::{SettlementDb, SettlementKind, Vitality};

    /// The row's content, off a hand-built view — the census below is what
    /// proves it against the real catalogue; this is what proves every
    /// field actually reaches the page at all.
    #[test]
    fn a_row_names_the_settlement_its_kind_specialty_temperament_and_blurb() {
        let view = SettlementView {
            name: "Hollow Index".to_string(),
            kind: "Server",
            specialty: "Programs",
            temperament: "Open",
            blurb: "Programs come here when their owners do not come back for them.".to_string(),
            standing: "Neutral",
            // A Server, which has no band — `SettlementView::vitality`.
            vitality: None,
            aid: Vec::new(),
        };
        let rows = settlement_page_rows(&view);
        let joined = rows
            .iter()
            .filter_map(|r| match r {
                Row::Text(t) | Row::TextColored(t, _) => Some(t.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        for want in [
            view.name.as_str(),
            view.kind,
            view.specialty,
            view.temperament,
            view.blurb.as_str(),
            view.standing,
        ] {
            assert!(joined.contains(want), "the page never names {want:?}");
        }
    }

    /// The header's colour must be a *call* to the same door the map glyph
    /// is drawn through, not a second, independently-authored orange — the
    /// comment above `Row::TextColored(view.name.clone(), ORANGE)` claims
    /// the two agree, and per CLAUDE.md's rule ("A doc comment claiming to
    /// mirror other code must be a call, not a copy") that claim has to be
    /// checked against `hud::palette::glyph(GlyphColor::Orange)` —
    /// `spawn_settlement_at`'s own `Glyph { color: GlyphColor::Orange }` is
    /// what the map actually resolves through `glyph_color`.
    #[test]
    fn the_header_wears_the_map_glyphs_own_orange() {
        let view = SettlementView {
            name: "Hollow Index".to_string(),
            kind: "Server",
            specialty: "Programs",
            temperament: "Open",
            blurb: "Programs come here when their owners do not come back for them.".to_string(),
            standing: "Neutral",
            // A Server, which has no band — `SettlementView::vitality`.
            vitality: None,
            aid: Vec::new(),
        };
        let rows = settlement_page_rows(&view);
        let Row::TextColored(text, color) = &rows[0] else {
            panic!("the header row must be the first row and must carry a colour");
        };
        assert_eq!(text, &view.name);
        assert_eq!(
            *color,
            glyph_color(GlyphColor::Orange),
            "the header must draw the same orange the map glyph resolves through, not a \
             hand-copied constant"
        );
    }

    /// A city that has been starved has to say so, or the only signal is a
    /// shelf that used to be longer — which the player cannot compare
    /// against anything.
    #[test]
    fn a_citys_page_names_its_vitality_and_a_towns_does_not() {
        let city = SettlementView {
            name: "Tally Yard".to_string(),
            kind: "Mainframe",
            specialty: "Materials",
            temperament: "Mercantile",
            blurb: "Everything that passes through is counted twice.".to_string(),
            standing: "Neutral",
            vitality: Some("Starved"),
            aid: Vec::new(),
        };
        let text = |rows: Vec<Row>| {
            rows.iter()
                .filter_map(|r| match r {
                    Row::Text(t) | Row::TextColored(t, _) => Some(t.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert!(
            text(settlement_page_rows(&city)).contains("Starved"),
            "a starved city's page never says so"
        );

        let town = SettlementView {
            kind: "Server",
            vitality: None,
            ..city.clone()
        };
        let drawn = text(settlement_page_rows(&town));
        for band in ["Starved", "Steady", "Thriving"] {
            assert!(
                !drawn.contains(band),
                "a town's page reports a vitality band it cannot have"
            );
        }
    }

    /// **Every** page the shipped catalogue can put on screen: one per
    /// (def × effective kind × vitality band), each carrying every aid line
    /// at once.
    ///
    /// It walks the whole catalogue rather than picking the def that
    /// maximises the sum of its fields, and that is a correction rather
    /// than thoroughness for its own sake. A summed-field fold picks *one*
    /// page and measures *its* rows, so a def whose blurb is short but
    /// whose kind/specialty/temperament line is the longest on the map is
    /// never measured at all. Tally Yard is exactly that def.
    ///
    /// It also enumerates the **effective** kind, not the authored one.
    /// `Game::settlement_kind` folds the run's growth latch into the label,
    /// so a `Server` draws `Mainframe` — three characters wider — the
    /// moment it grows, on a page with no scroll and against a catalogue
    /// that authors the shorter word. Measuring `def.kind` measures a page
    /// the game stops drawing.
    fn every_settlement_page() -> Vec<Vec<Row>> {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (db, warnings) =
            SettlementDb::load_dir(&assets.join("settlements")).expect("the catalogue loads");
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut pages = Vec::new();
        for def in db.iter() {
            for kind in effective_kinds(def.kind) {
                for vitality in page_vitalities(kind) {
                    pages.push(settlement_page_rows(&SettlementView {
                        name: def.name.clone(),
                        kind: kind.label(),
                        specialty: def.specialty.label(),
                        temperament: def.temperament.label(),
                        blurb: def.blurb.clone(),
                        // The longest label the band ladder can put on the
                        // page — the census measures the worst case, not
                        // the common one.
                        standing: "Neutral",
                        vitality,
                        // Every aid line at once, which is what an Allied
                        // town with a Relay standing actually draws. The
                        // strings are the engine's, repeated here rather
                        // than derived because the census has to measure
                        // the page and not build a `Game` to do it — and
                        // `every_aid_line_the_engine_can_write_is_measured`
                        // is what holds the two lists together.
                        aid: aid_lines_worst_case(),
                    }));
                }
            }
        }
        assert!(!pages.is_empty(), "the census must walk a real catalogue");
        pages
    }

    /// Every label a def with this authored kind can wear once the run has
    /// had its say — `Game::settlement_kind`'s fold, restated as the set it
    /// can answer rather than the one value the catalogue wrote.
    ///
    /// Exhaustive on purpose: a third `SettlementKind` fails this match
    /// rather than quietly going unmeasured on a page with no scroll.
    fn effective_kinds(authored: SettlementKind) -> Vec<SettlementKind> {
        match authored {
            // Growth is one-way, so a city is only ever a city.
            SettlementKind::Mainframe => vec![SettlementKind::Mainframe],
            // `Relation::grown` latches, so this def's page is drawn under
            // both words in the same run — and `Mainframe` is the longer.
            SettlementKind::Server => vec![SettlementKind::Server, SettlementKind::Mainframe],
        }
    }

    /// Every `SettlementView::vitality` a page of this kind can carry. A
    /// Server has no band and draws no row; a city draws one and the row is
    /// as wide as the widest band label.
    ///
    /// Exhaustive on both enums for `effective_kinds`' reason.
    fn page_vitalities(kind: SettlementKind) -> Vec<Option<&'static str>> {
        match kind {
            SettlementKind::Server => vec![None],
            SettlementKind::Mainframe => {
                let bands = [Vitality::Starved, Vitality::Steady, Vitality::Thriving];
                for band in bands {
                    match band {
                        Vitality::Starved | Vitality::Steady | Vitality::Thriving => {}
                    }
                }
                bands.into_iter().map(|band| Some(band.label())).collect()
            }
        }
    }

    /// The aid rows at their worst case: **every** sentence the engine can
    /// write, all at once.
    ///
    /// Read off `settlement_relations::AID_LINES` rather than restated, so
    /// a reworded sentence cannot slip past the width gate below — a gui
    /// test cannot build a `Game` to ask what the live lines are, because
    /// `Game::world` is private, so a shared constant is what holds the two
    /// sides together.
    ///
    /// More rows than any single town draws (a town cannot both offer a
    /// gift and be waiting to), which is the right way to be wrong: the
    /// page is measured with room to spare rather than exactly.
    fn aid_lines_worst_case() -> Vec<String> {
        feral_processes_engine::AID_LINES
            .iter()
            .map(|line| line.to_string())
            .collect()
    }

    /// The censuses have to measure the page the **run** draws, not the one
    /// the catalogue authored — and the gap between the two is three
    /// characters wide on a page with nothing that clips horizontally.
    ///
    /// This guards `every_settlement_page` itself. Both gates below read
    /// their worst case out of it, so a census that quietly went back to
    /// `def.kind` would leave them green while measuring a `Server` label
    /// the game has stopped drawing, and a census that dropped the band
    /// would measure a page a row short of the tallest one.
    #[test]
    fn the_census_measures_a_grown_towns_wider_label_and_a_citys_band() {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (db, _) =
            SettlementDb::load_dir(&assets.join("settlements")).expect("the catalogue loads");
        let towns: Vec<_> = db
            .iter()
            .filter(|def| def.kind == SettlementKind::Server)
            .collect();
        assert!(
            !towns.is_empty(),
            "census premise: the shipped catalogue authors at least one Server, or there is \
             nothing for growth to widen"
        );
        let drawn: Vec<String> = every_settlement_page()
            .iter()
            .flatten()
            .filter_map(|row| match row {
                Row::Text(t) | Row::TextColored(t, _) => Some(t.clone()),
                _ => None,
            })
            .collect();
        for town in towns {
            let grown = format!(
                "{}  ·  {}  ·  {}",
                SettlementKind::Mainframe.label(),
                town.specialty.label(),
                town.temperament.label()
            );
            assert!(
                drawn.iter().any(|line| line == &grown),
                "the census never measures {} as the Mainframe it can grow into:\n{grown}",
                town.name
            );
        }
        for band in [Vitality::Starved, Vitality::Steady, Vitality::Thriving] {
            let row = format!("The place is {}.", band.label());
            assert!(
                drawn.contains(&row),
                "the census never measures a city at {}",
                band.label()
            );
        }
    }

    /// A text-row popup page has no scroll, so height is a layout
    /// constraint — `memory_page_rows`' own gate, one screen over.
    #[test]
    fn the_tallest_shipped_settlement_fits_its_popup() {
        let rows = every_settlement_page()
            .iter()
            .map(|page| page.len())
            .max()
            .expect("the census must walk a real catalogue");
        for h in (600..=2160).step_by(60) {
            let m = ui_metrics(h as f32);
            let cap = popup_max_rows(h as f32, PopupSize::Large, &m);
            assert!(
                rows + REFUSAL_MAX_LINES <= cap,
                "the tallest settlement builds a {rows}-row page into a {cap}-row popup at {h}px"
            );
        }
    }

    /// The other axis, and the one nothing clamps at all: `draw_row` clips a
    /// row vertically and never horizontally, so a line past the right edge
    /// is simply lost.
    #[test]
    fn no_settlement_row_overflows_its_popup() {
        let pages = every_settlement_page();
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for row in pages.iter().flatten() {
                let line = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t,
                    _ => continue,
                };
                // The UI font, which is what a popup row is drawn in —
                // `measure_ui_advance` is the same door `draw_row` goes
                // through. The map's font is a different one and measuring
                // a page in it would be measuring the wrong thing.
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "a settlement row overflows the page by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }
}
