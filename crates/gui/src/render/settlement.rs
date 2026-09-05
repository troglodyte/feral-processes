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
        text_row(""),
    ];
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
    rows.push(text_row("[M] Market  ·  Esc to go back"));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::render::popup::{REFUSAL_MAX_LINES, popup_max_rows};
    use crate::text::ui_metrics;
    use feral_processes_engine::settlements::SettlementDb;

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

    /// The widest row the real catalogue can build — name, kind, specialty,
    /// temperament and blurb combined, since a page with no scroll has to be
    /// measured at its worst case rather than at whatever ships as the
    /// average one.
    fn tallest_settlement_page() -> Vec<Row> {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (db, warnings) =
            SettlementDb::load_dir(&assets.join("settlements")).expect("the catalogue loads");
        assert!(warnings.is_empty(), "{warnings:?}");
        let widest = db
            .iter()
            .max_by_key(|def| {
                def.name.chars().count()
                    + def.kind.label().chars().count()
                    + def.specialty.label().chars().count()
                    + def.temperament.label().chars().count()
                    + def.blurb.chars().count()
            })
            .expect("the census must walk a real catalogue");
        settlement_page_rows(&SettlementView {
            name: widest.name.clone(),
            kind: widest.kind.label(),
            specialty: widest.specialty.label(),
            temperament: widest.temperament.label(),
            blurb: widest.blurb.clone(),
            // The longest label the band ladder can put on the page — the
            // census measures the worst case, not the common one.
            standing: "Neutral",
        })
    }

    /// A text-row popup page has no scroll, so height is a layout
    /// constraint — `memory_page_rows`' own gate, one screen over.
    #[test]
    fn the_tallest_shipped_settlement_fits_its_popup() {
        let rows = tallest_settlement_page().len();
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
        let rows = tallest_settlement_page();
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for row in &rows {
                let line = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t,
                    _ => continue,
                };
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
