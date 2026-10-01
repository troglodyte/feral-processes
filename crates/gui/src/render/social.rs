//! The SOCIAL face of an owned program's manifest: what it makes of the
//! others, and what they make of it.
//!
//! Built as `Section`s and laid out by `manifest_layout` like the STATS face,
//! so the frame, header and footer are one implementation and the fit census
//! here measures the boxes the renderer really draws.

use super::manifest_layout::{Section, SectionRow, section_rows_capped};
use super::*;
use feral_processes_app_core::ManifestTab;
use feral_processes_engine::{RelationshipRow, SocialView};

/// The relationships box's cap, its last line spent on "+N more" when the
/// holder has opinions of more programs than this. The page has no meters
/// and no second column, so this is what the tightest window has room for
/// with the KNOWN FOR box under it — `the_tallest_social_page_fits_the_tightest_window`
/// is what measures it.
pub(super) const MAX_RELATIONSHIP_ROWS: usize = 8;

/// The empty-state lines, so the boxes are never drawn hollow.
const NO_OPINIONS: &str = "No opinions of the others yet.";
const SOCIABILITY_LABEL: &str = "Sociability";
const NOT_KNOWN_FOR_ANYTHING: &str = "The others have nothing to say of it yet.";

fn relationship_row(row: &RelationshipRow) -> SectionRow {
    let name = if row.gone {
        format!("{} (gone)", row.name)
    } else {
        row.name.clone()
    };
    SectionRow::Stat(name, format!("{}  {:+.0}", row.bond.label(), row.opinion))
}

pub(super) fn social_sections(view: &SocialView) -> Vec<Section> {
    let relationships = if view.relationships.is_empty() {
        vec![SectionRow::Note(NO_OPINIONS.to_string())]
    } else {
        section_rows_capped(
            view.relationships.iter().map(relationship_row).collect(),
            MAX_RELATIONSHIP_ROWS,
        )
    };
    let known_for = if view.known_for.is_empty() {
        NOT_KNOWN_FOR_ANYTHING.to_string()
    } else {
        view.known_for.join(", ")
    };
    vec![
        Section {
            title: "RELATIONSHIPS",
            rows: relationships,
            full_width: true,
            overflow: 0,
        },
        Section {
            title: "KNOWN FOR",
            rows: vec![
                SectionRow::Stat(SOCIABILITY_LABEL.to_string(), view.sociability.to_string()),
                SectionRow::Note(known_for),
            ],
            full_width: true,
            overflow: 0,
        },
    ]
}

const TAB_KEY: &str = "[Tab] ";
const TAB_SEPARATOR: &str = " · ";
const TAB_STATS: &str = "STATS";
const TAB_SOCIAL: &str = "SOCIAL";
const TAB_TALK: &str = "TALK";

/// The strip's pieces left to right, each with whether it is the open tab.
fn strip_pieces(active: ManifestTab) -> [(&'static str, bool); 6] {
    [
        (TAB_KEY, false),
        (TAB_STATS, active == ManifestTab::Stats),
        (TAB_SEPARATOR, false),
        (TAB_SOCIAL, active == ManifestTab::Social),
        (TAB_SEPARATOR, false),
        (TAB_TALK, active == ManifestTab::Talk),
    ]
}

pub(super) fn tab_strip_width(painter: &Painter, m: &Metrics) -> f32 {
    strip_pieces(ManifestTab::Stats)
        .iter()
        .map(|(text, _)| painter.measure_ui_advance(text, m.small()))
        .sum()
}

/// Right-aligned to `right`, the open tab lit and the other dim.
pub(super) fn draw_tab_strip(
    active: ManifestTab,
    right: f32,
    baseline: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let mut x = right - tab_strip_width(painter, m);
    for (text, open) in strip_pieces(active) {
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
mod tests {
    use super::super::manifest::fitted_stat_row;
    use super::super::manifest_layout::manifest_layout;
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

    fn census_game() -> Game {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        Game::new(
            11,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets load")
    }

    fn row(name: &str, bond: Bond, opinion: f32, gone: bool) -> RelationshipRow {
        RelationshipRow {
            name: name.to_string(),
            bond,
            opinion,
            gone,
        }
    }

    /// The widest name a row can carry: a full custom name, its rarity tier
    /// and the gone marker, against the widest bond label and a figure at
    /// the clamp.
    fn widest_row() -> RelationshipRow {
        let name = format!(
            "Overclocked {}",
            "M".repeat(feral_processes_engine::MAX_CUSTOM_NAME_LEN)
        );
        row(&name, Bond::Neutral, -99.0, true)
    }

    fn view_with(rows: usize, known_for: Vec<String>) -> SocialView {
        SocialView {
            relationships: (0..rows).map(|_| widest_row()).collect(),
            known_for,
            sociability: "Reserved",
        }
    }

    #[test]
    fn a_row_reads_name_band_and_signed_figure() {
        let SectionRow::Stat(name, value) =
            relationship_row(&row("Kestrel", Bond::Friend, 12.4, false))
        else {
            panic!("a relationship row is a stat row");
        };
        assert_eq!(name, "Kestrel");
        assert_eq!(value, "Friend  +12");
        let SectionRow::Stat(name, _) = relationship_row(&row("Kestrel", Bond::Enemy, -30.0, true))
        else {
            panic!();
        };
        assert_eq!(name, "Kestrel (gone)");
    }

    #[test]
    fn an_empty_page_says_so_instead_of_drawing_hollow_boxes() {
        let sections = social_sections(&SocialView {
            relationships: Vec::new(),
            known_for: Vec::new(),
            sociability: "Sociable",
        });
        for s in &sections {
            assert!(!s.rows.is_empty(), "{} is drawn empty", s.title);
        }
    }

    #[test]
    fn the_sociability_line_heads_the_known_for_box() {
        let mut view = view_with(0, vec!["good company".to_string()]);
        view.sociability = "Chatty";
        let sections = social_sections(&view);
        assert_eq!(
            sections[1].rows[0],
            SectionRow::Stat("Sociability".to_string(), "Chatty".to_string())
        );
        assert_eq!(
            sections[1].rows[1],
            SectionRow::Note("good company".to_string())
        );
    }

    #[test]
    fn more_relationships_than_the_cap_end_in_a_count() {
        let sections = social_sections(&view_with(MAX_RELATIONSHIP_ROWS + 3, Vec::new()));
        assert_eq!(sections[0].rows.len(), MAX_RELATIONSHIP_ROWS);
        assert_eq!(
            sections[0].rows.last(),
            Some(&SectionRow::Note("+4 more".to_string()))
        );
    }

    /// The tallest page: a full relationships box and a known-for line, at
    /// every window, inside the frame with clearance above the footer.
    #[test]
    fn the_tallest_social_page_fits_the_tightest_window() {
        let known = vec!["steady in a fight".to_string(), "good company".to_string()];
        let sections = social_sections(&view_with(MAX_RELATIONSHIP_ROWS + 5, known));
        for (w, h) in WINDOWS {
            let m = ui_metrics(h);
            let l = manifest_layout(w, h, 0, &sections, &m);
            let bottom = l.sections.iter().map(|r| r.y + r.h).fold(0.0_f32, f32::max);
            assert!(
                l.footer.y - bottom >= 10.0,
                "the tallest SOCIAL page at {w}x{h} leaves {:.1}px above the footer",
                l.footer.y - bottom
            );
            for r in &l.sections {
                assert!(
                    r.y + r.h <= l.frame.y + l.frame.h,
                    "escaped the frame at {w}x{h}"
                );
            }
        }
    }

    /// No relationship row is cut to fit: the name is the thing a player
    /// scans the box for.
    #[test]
    fn no_relationship_row_is_cut_to_fit_its_box() {
        let sections = social_sections(&view_with(2, Vec::new()));
        let SectionRow::Stat(label, value) = sections[0].rows[0].clone() else {
            panic!("a relationship row is a stat row");
        };
        with_painter(|p| {
            for (w, h) in WINDOWS {
                let m = ui_metrics(h);
                let l = manifest_layout(w, h, 0, &sections, &m);
                let fitted = fitted_stat_row(p, &label, &value, l.sections[0], &m);
                assert_eq!(
                    (fitted.label.as_str(), fitted.value.as_str()),
                    (label.as_str(), value.as_str()),
                    "a relationship row is cut at {w}x{h}"
                );
            }
        });
    }

    /// Every shipped phrase, both at once, on one line inside the KNOWN FOR
    /// box at every window.
    #[test]
    fn every_shipped_known_for_fits_its_box() {
        let game = census_game();
        let mut phrases: Vec<String> = game
            .memory_defs()
            .into_iter()
            .filter_map(|d| d.known_for.clone())
            .collect();
        phrases.sort_by_key(|p| std::cmp::Reverse(p.chars().count()));
        phrases.dedup();
        assert!(!phrases.is_empty(), "the shipped catalogue names phrases");
        // `Game::known_for` hands over at most two, so the two widest
        // shipped phrases are the worst line there is.
        let known: Vec<String> = phrases.into_iter().take(2).collect();
        let sections = social_sections(&view_with(1, known));
        let SectionRow::Note(text) = sections[1].rows[1].clone() else {
            panic!("the known-for line is a note");
        };
        with_painter(|p| {
            for (w, h) in WINDOWS {
                let m = ui_metrics(h);
                let l = manifest_layout(w, h, 0, &sections, &m);
                let room = l.sections[1].w - m.inset * 2.0;
                let drawn = p.measure_ui_advance(&text, m.font_size);
                assert!(
                    drawn <= room,
                    "KNOWN FOR is {drawn}px into {room}px at {w}x{h}"
                );
            }
        });
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
                    let l = manifest_layout(w, h, 0, &[], &m);
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
