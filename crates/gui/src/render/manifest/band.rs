//! The summary band: who this is, the faces on offer, the one thing to act
//! on, and the vitals line under it.

use super::*;
use crate::paint::Face as FontFace;
use feral_processes_engine::CallToAct;
use hud::log_frame::{INTEG_CELLS, POWER_CELLS, XP_CELLS, meter};
use hud::strip::{Piece, label, value};

/// How big the glyph is drawn, relative to the UI title size — enough to
/// read as a portrait rather than another line of text, and sized to span
/// the band's two text lines.
const GLYPH_SCALE: u16 = 2;

/// Everything line 1 and 2 of the band say. Strings and flags rather than a
/// `ManifestView`, so the width census can state the widest one it can build.
pub(super) struct BandHead<'a> {
    pub(super) glyph: char,
    pub(super) glyph_color: Color,
    pub(super) name: &'a str,
    /// The species or class, drawn in parentheses after the name.
    pub(super) kind: Option<&'a str>,
    pub(super) boss: bool,
    pub(super) rarity: Rarity,
    pub(super) tags: &'a [String],
    pub(super) tabs: Tabs<'a>,
    pub(super) call: Option<CallToAct>,
}

/// What holds the right end of line 1.
#[derive(Clone, Copy)]
pub(super) enum Tabs<'a> {
    /// A wild program, which has only STATS.
    None,
    /// The faces the subject has, and which is open.
    Faces(&'a [&'static str], usize),
}

impl Tabs<'_> {
    fn pieces(&self) -> Vec<Piece> {
        match self {
            Tabs::None => Vec::new(),
            Tabs::Faces(faces, open) => face_tab_pieces(faces, *open),
        }
    }
}

/// The call to act as the band words it: the count, then the key that acts.
///
/// `Idle` has no key because nothing on the sheet fixes it; the rail's `!`
/// and this line say the same thing as `Game::attention`'s row for it.
pub(super) fn call_text(call: CallToAct) -> String {
    match call {
        CallToAct::PerkPoints(n) => format!("▸ {n} perk pts  L level up"),
        CallToAct::DevPoints(n) => format!("▸ {n} dev points  S spend"),
        CallToAct::Idle => "▸ idle".to_string(),
    }
}

/// `Tab STATS · SCORE`: the open face in `hud-pane-title` bold, the rest in
/// `hud-field-label`.
pub(super) fn face_tab_pieces(faces: &[&'static str], open: usize) -> Vec<Piece> {
    let mut pieces = vec![label("Tab ")];
    for (i, face) in faces.iter().enumerate() {
        if i > 0 {
            pieces.push((" · ".to_string(), hud::palette::FAINT, false));
        }
        pieces.push(if i == open {
            (face.to_string(), hud::palette::PANE_TITLE, true)
        } else {
            (face.to_string(), hud::palette::FIELD_LABEL, false)
        });
    }
    pieces
}

fn runs(pieces: &[Piece]) -> Vec<TextRun<'_>> {
    pieces
        .iter()
        .map(|(text, color, bold)| TextRun {
            text,
            bold: *bold,
            color: *color,
        })
        .collect()
}

fn advance(painter: &Painter, pieces: &[Piece], size: u16) -> f32 {
    let text: String = pieces.iter().map(|(t, _, _)| t.as_str()).collect();
    painter.measure_ui_advance(text, size)
}

/// Draws lines 1 and 2 of the band into `rect` (two `line_height`s tall).
pub(super) fn draw_band(rect: Rect, head: &BandHead, painter: &Painter, m: &Metrics) {
    let glyph_size = m.title() * GLYPH_SCALE;
    let glyph = head.glyph.to_string();
    painter.map(
        &glyph,
        rect.x,
        rect.y + glyph_size as f32 * 0.85,
        glyph_size,
        head.glyph_color,
    );
    let text_x = rect.x + painter.measure_map(&glyph, glyph_size).width + m.pad;
    let right = rect.x + rect.w;
    let line1 = rect.y + m.title() as f32;
    let line2 = line1 + m.line_height;

    // The tabs hold the right end of line 1, so the name yields to them.
    let tabs = head.tabs.pieces();
    let tabs_w = if tabs.is_empty() {
        0.0
    } else {
        let w = advance(painter, &tabs, m.small());
        painter.ui_runs(&runs(&tabs), right - w, line1, m.small());
        w + m.pad
    };
    draw_name(head, text_x, right - tabs_w, line1, painter, m);

    // And the call to act holds the right end of line 2.
    let call = head.call.map(call_text);
    let call_w = call.as_ref().map_or(0.0, |text| {
        let w = painter.measure_ui_advance(text, m.font_size);
        painter.ui_bold(text, right - w, line2, m.font_size, hud::palette::ATTENTION);
        w + m.pad
    });
    let tags = head.tags.join(" · ");
    painter.ui(
        elided_to_fit(painter, &tags, m.font_size, right - call_w - text_x),
        text_x,
        line2,
        m.font_size,
        hud::palette::FAINT,
    );
}

/// The name, then the class or species, then `[BOSS]` and the tier tag, each
/// cut to what the one before left. Names come first: they are what the
/// player came to read.
fn draw_name(head: &BandHead, x: f32, limit: f32, base: f32, painter: &Painter, m: &Metrics) {
    let size = m.title();
    let mut x = x;
    let mut put = |text: String, bold: bool, color: Color| {
        let room = limit - x;
        if room <= 0.0 || text.is_empty() {
            return;
        }
        let face = if bold { FontFace::UiBold } else { FontFace::Ui };
        let fitted = if painter.measure(face, &text, size).width <= room {
            text
        } else {
            elided_to_fit(painter, &text, size, room)
        };
        if bold {
            painter.ui_bold(&fitted, x, base, size, color);
        } else {
            painter.ui(&fitted, x, base, size, color);
        }
        // The advance and not the ink: the leading spaces of `  [BOSS]` are
        // the gap before it, and ink measures none of them.
        x += painter.measure_ui_advance(&fitted, size);
    };
    put(head.name.to_string(), true, hud::palette::EMPHASIS);
    if let Some(kind) = head.kind {
        put(format!("  ({kind})"), false, hud::palette::BODY);
    }
    if head.boss {
        put("  [BOSS]".to_string(), true, hud::palette::THREAT);
    }
    if let Some(tier) = rarity_color(head.rarity) {
        put(rarity_tag(head.rarity), true, tier);
    }
}

/// The vitals as pieces in priority order: INTEG, PWR (the player alone),
/// then XP with what remains to the next level.
pub(super) fn vitals_segments(view: &ManifestView) -> Vec<Vec<Piece>> {
    let mut out = Vec::new();
    let mut integ = meter(
        "INTEG ",
        view.hp as f32,
        view.max_hp.max(1) as f32,
        INTEG_CELLS,
        hud::palette::HEALTHY,
    );
    integ.push(value(format!(" {}/{}", view.hp, view.max_hp)));
    out.push(integ);
    if let ManifestSubject::Player(p) = &view.subject {
        let mut pwr = meter(
            "PWR ",
            p.power,
            p.max_power,
            POWER_CELLS,
            hud::palette::ATTENTION,
        );
        pwr.push(value(format!(" {:.0}/{:.0}", p.power, p.max_power)));
        out.push(pwr);
    }
    if let (Some((xp, to_next)), Some(level)) = (view.xp, view.level) {
        let mut seg = meter(
            "XP ",
            xp as f32,
            to_next.max(1) as f32,
            XP_CELLS,
            hud::palette::CH_GAIN,
        );
        seg.push(value(format!(" {xp}/{to_next}")));
        seg.push((
            format!("  {} to Lv {}", to_next.saturating_sub(xp), level + 1),
            hud::palette::FAINT,
            false,
        ));
        out.push(seg);
    }
    out
}

/// The vitals line in `rect`, between two `hud-divider` rules that sit in
/// the gaps above and below it. Whole segments are dropped from the end
/// when the line does not fit (`strip::fitting`'s rule).
pub(super) fn draw_vitals(rect: Rect, view: &ManifestView, painter: &Painter, m: &Metrics) {
    for y in [rect.y - m.gap / 2.0, rect.y + rect.h + m.gap / 2.0] {
        painter.rect(rect.x, y, rect.w, 1.0, hud::palette::DIVIDER);
    }
    let pieces = fitting_gapped(&vitals_segments(view), rect.w, painter, m);
    painter.ui_runs(
        &runs(&pieces),
        rect.x,
        rect.y + rect.h / 2.0 + m.small() as f32 * 0.35,
        m.small(),
    );
}

/// `fitting` with wider spaces between the vitals than a strip's ` · `: the
/// meters are blocks and want air around them, not a separator glyph.
fn fitting_gapped(
    segments: &[Vec<Piece>],
    avail: f32,
    painter: &Painter,
    m: &Metrics,
) -> Vec<Piece> {
    hud::strip::fitting_joined(
        segments,
        &("    ".to_string(), hud::palette::FAINT, false),
        avail,
        painter,
        m,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_call_to_act_names_its_count_and_its_key() {
        assert_eq!(
            call_text(CallToAct::PerkPoints(5)),
            "▸ 5 perk pts  L level up"
        );
        assert_eq!(
            call_text(CallToAct::DevPoints(2)),
            "▸ 2 dev points  S spend"
        );
        assert_eq!(call_text(CallToAct::Idle), "▸ idle");
    }

    #[test]
    fn the_open_face_is_bold_and_the_rest_are_field_labels() {
        let pieces = face_tab_pieces(&["STATS", "SOCIAL", "TALK"], 1);
        let face = |name: &str| pieces.iter().find(|(t, _, _)| t == name).cloned();
        assert_eq!(
            face("SOCIAL"),
            Some(("SOCIAL".into(), hud::palette::PANE_TITLE, true))
        );
        assert_eq!(
            face("STATS"),
            Some(("STATS".into(), hud::palette::FIELD_LABEL, false))
        );
        let text: String = pieces.iter().map(|(t, _, _)| t.as_str()).collect();
        assert_eq!(text, "Tab STATS · SOCIAL · TALK");
    }
}
