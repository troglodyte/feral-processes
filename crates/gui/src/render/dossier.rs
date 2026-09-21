//! The dossier — `[D]` from a program's manifest.
//!
//! A second sheet about the body `Mode::Manifest` is showing: five
//! attributes with their old-school names beside them, over a derived
//! header and one sentence of where this program came from.
//!
//! **Every figure comes out of `Game::dossier_report`**,
//! `draw_companion_memories`' rule — a renderer that derived a header
//! itself would be the fifth screen in this repo to keep a private copy of
//! something the engine already owns.
//!
//! **Nothing on this page is read for a mechanic.** See `attributes.rs`.
//!
//! Its own file rather than a third page in `render/party.rs`, which
//! already holds the roster, the gear page, the fusion picker and the
//! memories page: the dossier is read from the manifest rather than from
//! the roster, and the two censuses below want a file to live in.

use super::popup::*;
use super::*;

/// The dossier for `subject`, or a refusal when the body is gone —
/// `draw_companion_memories`' shape, including the `PopupSize::Small`
/// fallback.
pub(super) fn draw_dossier(
    game: &Game,
    subject: Option<Entity>,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(report) = subject.and_then(|e| game.dossier_report(e)) else {
        draw_popup(
            "Dossier",
            PopupSize::Small,
            &[text_row("That program is gone.")],
            refusal,
            painter,
            m,
        );
        return;
    };
    let rows = dossier_page_rows(&report, DESCRIBE_WRAP_COLUMNS);
    draw_popup("Dossier", PopupSize::Large, &rows, refusal, painter, m);
}

/// The page's rows, out of the engine's report rather than out of a `Game`
/// — the split `memory_page_rows` makes, and for its reason: the height and
/// width censuses have to measure the page at its **worst** case, and
/// `MAX_ATTRIBUTE_ROWS` rows of the widest shipped def is a state a fixture
/// can state and a `Game` would have to be played into.
///
/// **One row per attribute, carrying the `short` gloss and not the
/// `meaning` prose**, and the census is what decided that rather than an
/// eye. The page has no scroll, and a `PopupSize::Large` popup holds 23
/// rows at the narrowest supported window; the page as written builds 16
/// at its worst case. `MAX_ATTRIBUTE_ROWS` (10) meanings wrapped at
/// `DESCRIBE_WRAP_COLUMNS` is about thirty rows on top of that, and even
/// the shipped five would overrun. `AttributeRow::meaning` stays in the
/// report for the inspect page a later change adds.
pub(super) fn dossier_page_rows(
    report: &feral_processes_engine::views::DossierReport,
    cols: usize,
) -> Vec<Row> {
    let mut rows = vec![
        Row::TextColored(format!("{} — dossier", report.name), CYAN),
        text_row(format!("{}    {}", report.revision, report.checksum)),
    ];
    if let Some(provenance) = &report.provenance {
        rows.extend(wrap_text(provenance, cols).into_iter().map(text_row));
    }
    rows.push(text_row(""));

    if report.rows.is_empty() {
        // Also what an install with `assets/attributes/` deleted draws,
        // which is the supported way to play without this feature at all.
        rows.push(text_row("Nothing on file about this one."));
    }
    for row in &report.rows {
        rows.push(text_row(format!(
            "{} ({})  {}   {}",
            row.name, row.legacy, row.value, row.short
        )));
    }

    rows.push(text_row(""));
    rows.push(text_row("Esc to go back"));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use feral_processes_engine::attributes::{AttributeDb, AttributeDef};

    fn assets_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    fn shipped_attribute_defs() -> Vec<AttributeDef> {
        let (db, warnings) = AttributeDb::load_dir(&assets_dir().join("attributes")).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        db.iter().cloned().collect()
    }

    /// The widest string `Game::creature_label` can build, which is what
    /// `Game::dossier_report` puts in the title — stated from its parts
    /// rather than played into, `tallest_dossier`'s reason.
    ///
    /// The long form, not `creature_short_label`'s: the widest rare tier's
    /// prefix, an 8-character handle, the longest shipped species name
    /// after it (a handle names an owned program, not what it *is*, so the
    /// species rides along) and a two-digit zone tag. A `CustomName` is
    /// capped at `MAX_CUSTOM_NAME_LEN` (12) and drops the species suffix
    /// entirely, so it is the narrower of the two candidates here and the
    /// handle form is the one measured.
    fn longest_shipped_program_name() -> String {
        let (abilities, _) =
            feral_processes_engine::abilities::AbilityDb::load_dir(&assets_dir().join("abilities"))
                .expect("the abilities load");
        let (species, warnings) = feral_processes_engine::species::SpeciesDb::load_dir(
            &assets_dir().join("species"),
            &abilities,
        )
        .expect("the species load");
        assert!(warnings.is_empty(), "{warnings:?}");
        let longest_species = species
            .all()
            .map(|def| def.name.clone())
            .max_by_key(|n| n.chars().count())
            .expect("the census must walk a real roster");
        let longest_tier = feral_processes_engine::components::Rarity::ALL
            .iter()
            .filter_map(|r| r.label())
            .max_by_key(|l| l.chars().count())
            .expect("at least one tier carries a label");
        format!("{longest_tier} 0x435eaD {longest_species} 99")
    }

    /// The longest sentence the shipped `"program.dossier"` pool can draw.
    /// Read off the file rather than played into, for `tallest_dossier`'s
    /// reason.
    fn longest_shipped_provenance() -> String {
        let text = std::fs::read_to_string(assets_dir().join("descriptions/program_dossier.ron"))
            .expect("the shipped provenance pool");
        text.lines()
            .filter_map(|line| {
                let line = line.trim();
                line.strip_prefix('"')
                    .and_then(|l| l.rsplit_once('"'))
                    .map(|(s, _)| s.to_string())
            })
            .max_by_key(|s| s.chars().count())
            .expect("the pool is not empty")
    }

    /// Built by hand, not played into: the census has to measure the page
    /// at a worst case the shipped game can reach but no save is likely to
    /// be in. `MAX_ATTRIBUTE_ROWS` rows, each carrying the longest name,
    /// legacy word and gloss any shipped def uses, over a full header with
    /// the longest shipped provenance sentence.
    fn tallest_dossier() -> Vec<Row> {
        let widest = |pick: fn(&AttributeDef) -> &str| {
            shipped_attribute_defs()
                .iter()
                .map(|d| pick(d).to_string())
                .max_by_key(|s| s.chars().count())
                .expect("the shipped catalogue is not empty")
        };
        let row = feral_processes_engine::views::AttributeRow {
            name: widest(|d| &d.name),
            legacy: widest(|d| &d.legacy),
            // The widest value the mint can produce from any shipped def,
            // and negative-signed nowhere — but measured at three digits,
            // because a modded `base` may reach them.
            value: 100,
            short: widest(|d| &d.short),
            meaning: widest(|d| &d.meaning),
        };
        let report = feral_processes_engine::views::DossierReport {
            name: longest_shipped_program_name(),
            revision: "rev 9.99".to_string(),
            checksum: "0xFFFFFF".to_string(),
            provenance: Some(longest_shipped_provenance()),
            rows: vec![row; feral_processes_engine::tuning::MAX_ATTRIBUTE_ROWS],
        };
        dossier_page_rows(&report, DESCRIBE_WRAP_COLUMNS)
    }

    /// **The page has no scroll.** `draw_popup` pages a `Row::Item` span
    /// and this page has none, so a row past the bottom is dropped in
    /// silence — `the_tallest_memory_page_fits_its_popup`'s trap exactly.
    /// Raising `MAX_ATTRIBUTE_ROWS` past what fits means giving the page a
    /// scroll first.
    ///
    /// Swept rather than measured at one window: `ui_metrics` clamps the
    /// font at both ends, so below the clamp the box keeps shrinking while
    /// the line height stops and the tightest window is the smallest one.
    #[test]
    fn the_tallest_dossier_fits_its_popup() {
        let rows = tallest_dossier().len();
        for h in (600..=2160).step_by(60) {
            let m = ui_metrics(h as f32);
            let cap = popup_max_rows(h as f32, PopupSize::Large, &m);
            assert!(
                rows + REFUSAL_MAX_LINES <= cap,
                "the dossier builds a {rows}-row page into a {cap}-row popup at {h}px"
            );
        }
    }

    /// The other axis, and the one nothing clamps at all: `draw_row` clips
    /// a row vertically and never horizontally, so a line past the right
    /// edge is simply lost. On this page the tail of a row is the number
    /// and the gloss — which is most of what the row is read for.
    #[test]
    fn no_dossier_row_overflows_its_popup() {
        let rows = tallest_dossier();
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
                    "a dossier row overflows the page by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }
}
