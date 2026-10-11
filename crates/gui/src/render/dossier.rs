//! The dossier — `[D]` from a program's manifest.
//!
//! A second page about the body `Mode::Manifest` is showing, drawn in the
//! manifest's frame under its band: where this program came from, then its
//! attributes with their old-school names beside them. The figures are
//! `Game::dossier_report`'s, and the entry point that fetches them is
//! `manifest::draw_dossier`.
//!
//! **Nothing on this page is read for a mechanic.** See `attributes.rs`.
//!
//! The page has no scroll, so `MAX_ATTRIBUTE_ROWS` is what bounds it, and
//! the fit census in `manifest` paints its tallest case at every window.

use super::manifest::layout::section_row_h;
use super::manifest::{elided_to_fit, subhead, wrap_to_width};
use super::popup::*;
use super::*;
use feral_processes_engine::views::{AttributeRow, DossierReport};

/// Where there is nothing to list. What an install with `assets/attributes/`
/// deleted draws, which is the supported way to play without this feature.
const NOTHING_ON_FILE: &str = "Nothing on file about this one.";

/// Cells between the attribute table's columns.
const COLUMN_GAP_CELLS: f32 = 2.0;

/// The attribute table's column widths, in pixels, measured from its own
/// rows so the columns hug what is in them: name, `WAS legacy`, value.
struct Columns {
    name: f32,
    legacy: f32,
    value: f32,
}

impl Columns {
    fn of(rows: &[AttributeRow], painter: &Painter, m: &Metrics) -> Columns {
        let widest = |text: fn(&AttributeRow) -> String| {
            rows.iter()
                .map(|r| painter.measure_ui_advance(text(r), m.font_size))
                .fold(0.0_f32, f32::max)
        };
        Columns {
            name: widest(|r| r.name.clone()),
            legacy: widest(legacy_cell),
            value: widest(|r| r.value.to_string()),
        }
    }
}

fn legacy_cell(row: &AttributeRow) -> String {
    format!("WAS {}", row.legacy)
}

/// PROVENANCE, with the revision and checksum in its subhead, then
/// ATTRIBUTES as a table of name, legacy name, value and short gloss.
///
/// **One row per attribute, carrying the `short` gloss and not the
/// `meaning` prose**: with a table a gloss wraps under its own column and the
/// census, not an eye, decided that the longer prose would not fit.
/// `AttributeRow::meaning` stays in the report for the inspect page a later
/// change adds.
pub(super) fn paint_dossier(report: &DossierReport, body: Rect, painter: &Painter, m: &Metrics) {
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let row_h = section_row_h(m);
    let hint = format!("{}    {}", report.revision, report.checksum);
    subhead("PROVENANCE", &hint, body, body.y, painter, m);
    let mut y = body.y + m.line_height;
    if let Some(provenance) = &report.provenance {
        for line in wrap_text(provenance, DESCRIBE_WRAP_COLUMNS) {
            painter.ui(
                elided_to_fit(painter, &line, m.font_size, body.w),
                body.x,
                y + row_h * 0.85,
                m.font_size,
                hud::palette::BODY,
            );
            y += row_h;
        }
    }
    y += m.gap;
    let below = Rect::new(body.x, y, body.w, (body.y + body.h - y).max(0.0));
    subhead("ATTRIBUTES", "", below, y, painter, m);
    if report.rows.is_empty() {
        painter.ui(
            NOTHING_ON_FILE,
            below.x,
            y + m.line_height + row_h * 0.85,
            m.font_size,
            hud::palette::FAINT,
        );
        return;
    }
    let cols = Columns::of(&report.rows, painter, m);
    let gap = COLUMN_GAP_CELLS * char_w;
    let legacy_x = below.x + cols.name + gap;
    let value_x = legacy_x + cols.legacy + gap;
    let short_x = value_x + cols.value + gap;
    let short_w = below.x + below.w - short_x;
    let mut y = y + m.line_height;
    for row in &report.rows {
        let base = y + row_h * 0.85;
        painter.ui(&row.name, below.x, base, m.font_size, hud::palette::BODY);
        painter.ui(
            legacy_cell(row),
            legacy_x,
            base,
            m.font_size,
            hud::palette::FAINT,
        );
        let value = row.value.to_string();
        let value_w = painter.measure_ui_advance(&value, m.font_size);
        painter.ui_bold(
            &value,
            value_x + cols.value - value_w,
            base,
            m.font_size,
            hud::palette::EMPHASIS,
        );
        // The gloss wraps under its own column, so a long one is more rows
        // and never a cut word.
        let lines = wrap_to_width(painter, &row.short, m.font_size, short_w);
        for (i, line) in lines.iter().enumerate() {
            painter.ui(
                elided_to_fit(painter, line, m.font_size, short_w),
                short_x,
                base + row_h * i as f32,
                m.font_size,
                hud::palette::FIELD_LABEL,
            );
        }
        y += row_h * lines.len().max(1) as f32;
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
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
    pub(in crate::render) fn tallest_dossier() -> DossierReport {
        let widest = |pick: fn(&AttributeDef) -> &str| {
            shipped_attribute_defs()
                .iter()
                .map(|d| pick(d).to_string())
                .max_by_key(|s| s.chars().count())
                .expect("the shipped catalogue is not empty")
        };
        let row = AttributeRow {
            name: widest(|d| &d.name),
            legacy: widest(|d| &d.legacy),
            // The widest value the mint can produce from any shipped def,
            // and negative-signed nowhere — but measured at three digits,
            // because a modded `base` may reach them.
            value: 100,
            short: widest(|d| &d.short),
            meaning: widest(|d| &d.meaning),
        };
        DossierReport {
            name: longest_shipped_program_name(),
            revision: "rev 9.99".to_string(),
            checksum: "0xFFFFFF".to_string(),
            provenance: Some(longest_shipped_provenance()),
            rows: vec![row; feral_processes_engine::tuning::MAX_ATTRIBUTE_ROWS],
        }
    }
}
