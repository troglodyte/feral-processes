//! The SOCIAL face of an owned program's manifest: what it makes of the
//! others, and what they make of it.
//!
//! RELATIONSHIPS takes the wide part of the body and is drawn row by row,
//! since a row is a glyph, a name, a bond and a ruler and not a label and a
//! value. KNOWN FOR and TEMPERAMENT sit in the narrow column beside it.

use super::manifest::layout::{ManifestLayout, section_row_h};
use super::manifest::{PIPS, draw_pairs, elided_to_fit, pip_cells, subhead, wrap_to_width};
use super::*;
use feral_processes_app_core::ManifestTab;
use feral_processes_engine::tuning::{BOND_CLOSE_AT, BOND_ENEMY_AT};
use feral_processes_engine::{RelationshipRow, RosterRow, SocialView};

/// The relationships list's cap, its last line spent on "+N more" when the
/// holder has opinions of more programs than this.
pub(super) const MAX_RELATIONSHIP_ROWS: usize = 8;

/// The empty-state lines, so the columns are never drawn hollow.
const NO_OPINIONS: &str = "No opinions of the others yet.";
const SOCIABILITY_LABEL: &str = "Sociability";
const NOT_KNOWN_FOR_ANYTHING: &str = "The others have nothing to say of it yet.";

/// Cells a ruler takes at a row's right end, and the cells before the name:
/// the glyph and a space.
const RULER_CELLS: usize = PIPS;
const NAME_COL: f32 = 2.0;

/// `Friend  +12`: the band, then the signed figure the band is derived from.
fn bond_text(row: &RelationshipRow) -> String {
    format!("{}  {:+.0}", row.bond.label(), row.opinion)
}

/// How many of the ruler's pips `opinion` lights: the span from enemy to
/// close, clamped, spread over `PIPS`. A neutral program sits mid-ruler.
fn opinion_pips(opinion: f32) -> usize {
    let t = (opinion.clamp(BOND_ENEMY_AT, BOND_CLOSE_AT) - BOND_ENEMY_AT)
        / (BOND_CLOSE_AT - BOND_ENEMY_AT);
    (t * PIPS as f32).round() as usize
}

/// The rows the list draws: the first `MAX_RELATIONSHIP_ROWS - 1` and a
/// count of the rest when there are more than the cap, as `section_rows`
/// says it everywhere else.
fn shown(rows: &[RelationshipRow]) -> (&[RelationshipRow], usize) {
    if rows.len() <= MAX_RELATIONSHIP_ROWS {
        return (rows, 0);
    }
    let keep = MAX_RELATIONSHIP_ROWS - 1;
    (&rows[..keep], rows.len() - keep)
}

/// Paints the face's body: `l`'s ledger columns, the first two as one.
pub(super) fn paint_social(
    view: &SocialView,
    roster: &[RosterRow],
    l: &ManifestLayout,
    painter: &Painter,
    m: &Metrics,
) {
    let [first, second, narrow] = l.ledger;
    let wide = Rect::new(first.x, first.y, second.x + second.w - first.x, first.h);
    paint_relationships(view, roster, wide, painter, m);
    paint_narrow(view, narrow, painter, m);
}

fn paint_relationships(
    view: &SocialView,
    roster: &[RosterRow],
    rect: Rect,
    painter: &Painter,
    m: &Metrics,
) {
    subhead("RELATIONSHIPS", "", rect, rect.y, painter, m);
    let row_h = section_row_h(m);
    let mut base = rect.y + m.line_height + row_h * 0.85;
    if view.relationships.is_empty() {
        painter.ui(NO_OPINIONS, rect.x, base, m.font_size, hud::palette::FAINT);
        return;
    }
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let ruler_x = rect.x + rect.w - RULER_CELLS as f32 * char_w;
    let name_x = rect.x + NAME_COL * char_w;
    let (rows, hidden) = shown(&view.relationships);
    for row in rows {
        // The roster names every program you own by the label a
        // relationship carries, so it is where the glyph comes from.
        let owned = roster.iter().find(|r| r.name == row.name);
        let ink = if row.gone {
            hud::palette::FAINT
        } else {
            hud::palette::BODY
        };
        match (row.gone, owned) {
            (false, Some(r)) => painter.map(
                r.glyph.to_string(),
                rect.x,
                base,
                m.font_size,
                glyph_color(r.color),
            ),
            (true, _) => painter.ui("x", rect.x, base, m.font_size, hud::palette::FAINT),
            (false, None) => painter.ui("·", rect.x, base, m.font_size, hud::palette::FAINT),
        }
        let bond = bond_text(row);
        let bond_w = painter.measure_ui_advance(&bond, m.font_size);
        let bond_x = ruler_x - char_w - bond_w;
        painter.ui(
            elided_to_fit(painter, &row.name, m.font_size, bond_x - char_w - name_x),
            name_x,
            base,
            m.font_size,
            ink,
        );
        painter.ui(&bond, bond_x, base, m.font_size, hud::palette::FIELD_LABEL);
        let (lit, unlit) = pip_cells(opinion_pips(row.opinion));
        painter.ui_runs(
            &[
                TextRun {
                    text: &lit,
                    bold: false,
                    color: hud::palette::BODY,
                },
                TextRun {
                    text: &unlit,
                    bold: false,
                    color: hud::palette::BAR_TROUGH,
                },
            ],
            ruler_x,
            base,
            m.font_size,
        );
        base += row_h;
    }
    if hidden > 0 {
        painter.ui(
            format!("+{hidden} more"),
            rect.x,
            base,
            m.font_size,
            hud::palette::FAINT,
        );
    }
}

/// KNOWN FOR's phrases wrapped to the column, then TEMPERAMENT under it.
fn paint_narrow(view: &SocialView, rect: Rect, painter: &Painter, m: &Metrics) {
    subhead("KNOWN FOR", "", rect, rect.y, painter, m);
    let known_for = if view.known_for.is_empty() {
        NOT_KNOWN_FOR_ANYTHING.to_string()
    } else {
        view.known_for.join(", ")
    };
    let row_h = section_row_h(m);
    let mut y = rect.y + m.line_height;
    for line in wrap_to_width(painter, &known_for, m.font_size, rect.w) {
        painter.ui(
            elided_to_fit(painter, &line, m.font_size, rect.w),
            rect.x,
            y + row_h * 0.85,
            m.font_size,
            hud::palette::BODY,
        );
        y += row_h;
    }
    y += m.gap;
    let below = Rect::new(rect.x, y, rect.w, (rect.y + rect.h - y).max(0.0));
    draw_pairs(
        "TEMPERAMENT",
        &[(SOCIABILITY_LABEL, view.sociability.to_string())],
        None,
        below,
        painter,
        m,
    );
}

const TAB_KEY: &str = "[Tab] ";
const TAB_SEPARATOR: &str = " · ";
const TAB_STATS: &str = "STATS";
const TAB_SOCIAL: &str = "SOCIAL";
const TAB_TALK: &str = "TALK";

const TAB_SCORE: &str = "SCORE";

/// The strip's pieces left to right, each with whether it is the open tab.
/// The player's sheet has two faces, a program's three.
fn strip_pieces(active: ManifestTab, player: bool) -> Vec<(&'static str, bool)> {
    let mut pieces = vec![
        (TAB_KEY, false),
        (TAB_STATS, active == ManifestTab::Stats),
        (TAB_SEPARATOR, false),
    ];
    if player {
        pieces.push((TAB_SCORE, active == ManifestTab::Score));
    } else {
        pieces.extend([
            (TAB_SOCIAL, active == ManifestTab::Social),
            (TAB_SEPARATOR, false),
            (TAB_TALK, active == ManifestTab::Talk),
        ]);
    }
    pieces
}

/// A program's strip, the wider of the two.
#[cfg(test)]
pub(super) fn tab_strip_width(painter: &Painter, m: &Metrics) -> f32 {
    strip_width(ManifestTab::Stats, false, painter, m)
}

fn strip_width(active: ManifestTab, player: bool, painter: &Painter, m: &Metrics) -> f32 {
    strip_pieces(active, player)
        .iter()
        .map(|(text, _)| painter.measure_ui_advance(text, m.small()))
        .sum()
}

/// Right-aligned to `right`, the open tab lit and the other dim.
pub(super) fn draw_tab_strip(
    active: ManifestTab,
    player: bool,
    right: f32,
    baseline: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let mut x = right - strip_width(active, player, painter, m);
    for (text, open) in strip_pieces(active, player) {
        painter.ui(
            text,
            x,
            baseline,
            m.small(),
            if open { CYAN } else { TEXT_DIM },
        );
        x += painter.measure_ui_advance(text, m.small());
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::manifest::layout::manifest_layout;
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::bonds::Bond;

    const WINDOWS: [(f32, f32); 8] = [
        (1280.0, 720.0),
        (1366.0, 768.0),
        (1440.0, 900.0),
        (1600.0, 900.0),
        (1680.0, 1050.0),
        (1920.0, 1080.0),
        (1920.0, 1200.0),
        (2560.0, 1440.0),
    ];

    fn row(name: &str, bond: Bond, opinion: f32, gone: bool) -> RelationshipRow {
        RelationshipRow {
            name: name.to_string(),
            bond,
            opinion,
            gone,
        }
    }

    /// The widest name a row can carry: a full custom name and its rarity
    /// tier, against the widest bond label and a figure at the clamp.
    pub(in crate::render) fn widest_row() -> RelationshipRow {
        let name = format!(
            "Overclocked {}",
            "M".repeat(feral_processes_engine::MAX_CUSTOM_NAME_LEN)
        );
        row(&name, Bond::Neutral, -99.0, false)
    }

    pub(in crate::render) fn view_with(rows: usize, known_for: Vec<String>) -> SocialView {
        SocialView {
            relationships: (0..rows).map(|_| widest_row()).collect(),
            known_for,
            sociability: "Reserved",
        }
    }

    #[test]
    fn a_row_reads_the_band_then_the_signed_figure() {
        assert_eq!(
            bond_text(&row("Kestrel", Bond::Friend, 12.4, false)),
            "Friend  +12"
        );
        assert_eq!(
            bond_text(&row("Kestrel", Bond::Enemy, -30.0, true)),
            "Enemy  -30"
        );
    }

    /// The ruler spans enemy to close and is clamped at both ends.
    #[test]
    fn the_ruler_lights_pips_across_the_bond_span() {
        assert_eq!(opinion_pips(BOND_ENEMY_AT - 50.0), 0);
        assert_eq!(opinion_pips(BOND_ENEMY_AT), 0);
        assert_eq!(opinion_pips(BOND_CLOSE_AT), PIPS);
        assert_eq!(opinion_pips(BOND_CLOSE_AT + 50.0), PIPS);
        let mid = (BOND_ENEMY_AT + BOND_CLOSE_AT) / 2.0;
        assert!(
            (1..PIPS).contains(&opinion_pips(mid)),
            "a neutral bond reads mid-ruler"
        );
        assert!(opinion_pips(-5.0) < opinion_pips(10.0));
    }

    #[test]
    fn more_relationships_than_the_cap_end_in_a_count() {
        let view = view_with(MAX_RELATIONSHIP_ROWS + 3, Vec::new());
        let (rows, hidden) = shown(&view.relationships);
        assert_eq!(rows.len(), MAX_RELATIONSHIP_ROWS - 1);
        assert_eq!(hidden, 4);
        let exact = view_with(MAX_RELATIONSHIP_ROWS, Vec::new());
        let (rows, hidden) = shown(&exact.relationships);
        assert_eq!((rows.len(), hidden), (MAX_RELATIONSHIP_ROWS, 0));
    }

    /// The two widest shipped phrases, which is all `Game::known_for` hands
    /// over at most.
    pub(in crate::render) fn widest_known_for() -> Vec<String> {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load");
        let mut phrases: Vec<String> = game
            .memory_defs()
            .into_iter()
            .filter_map(|d| d.known_for.clone())
            .collect();
        phrases.sort_by_key(|p| std::cmp::Reverse(p.chars().count()));
        phrases.dedup();
        assert!(!phrases.is_empty(), "the shipped catalogue names phrases");
        phrases.into_iter().take(2).collect()
    }

    /// The strip shares the header's title line, so the widest title the
    /// sheet can draw plus the strip has to fit what is left of the header
    /// after the glyph, at the narrowest window.
    #[test]
    fn the_widest_title_and_the_strip_fit_the_header() {
        use super::super::manifest::{header_rarity, header_title};
        use super::super::test_support::game_with_tweaked_programs;
        use feral_processes_engine::components::Rarity;

        // The two ways a title gets long: a full custom name, or a handle
        // that has the species appended to it. Either can be a boss of the
        // highest tier, in a deep zone.
        let custom = "M".repeat(feral_processes_engine::MAX_CUSTOM_NAME_LEN);
        // `subject_title` appends the species, so the fixture's first
        // species would hide the longest-name worst case.
        let longest_species = feral_processes_engine::Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            &super::super::test_support::test_assets_dir(),
        )
        .expect("the shipped asset tree builds a fresh game")
        .species_defs()
        .into_iter()
        .max_by_key(|d| d.name.chars().count())
        .expect("the shipped catalogue has species")
        .id;
        for (fixture, name) in [
            ("title_custom", Some(custom.as_str())),
            ("title_handle", None),
        ] {
            let mut game = game_with_tweaked_programs(fixture, 11, &[(false, "")], |c| {
                c.custom_name = name.map(str::to_string);
                c.species = longest_species.clone();
                c.rarity = Rarity::Gold;
                c.boss = true;
                c.zone = 10;
            });
            let subject = game.manifest_subjects()[1];
            let view = game.manifest(subject).expect("an owned program has a page");
            let head = header_title(&view);
            let tag = rarity_tag(header_rarity(&view));
            assert!(head.contains("[BOSS]") && !tag.is_empty(), "{head:?}{tag}");
            with_painter(|p| {
                for (w, h) in WINDOWS {
                    let m = ui_metrics(h);
                    let l = manifest_layout(w, h, &[], &m);
                    let glyph = p.measure_map("x", m.title() * 2).width + m.pad;
                    let title = p
                        .measure(crate::paint::Face::UiBold, &head, m.title())
                        .width
                        + p.measure(crate::paint::Face::UiBold, &tag, m.title()).width;
                    let strip = tab_strip_width(p, &m);
                    assert!(
                        glyph + title + m.pad + strip <= l.header.w,
                        "title {head:?}{tag} and tab strip collide at {w}x{h}: {} into {}",
                        glyph + title + m.pad + strip,
                        l.header.w
                    );
                }
            });
        }
    }
}
