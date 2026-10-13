//! The keybar along the sheet's bottom: only the keys valid now, in three
//! groups, dropped whole from the end when the line is too narrow.

use super::*;
use hud::log_frame::keycap;
use hud::strip::{Piece, fitting_joined, sep};

/// What decides which keys the bar offers.
#[derive(Clone, Copy, Debug)]
pub(super) struct KeybarState {
    /// ↑/↓ and the digit/letter jump work: the subject is on the rail and
    /// there is somewhere to go.
    pub(super) cyclable: bool,
    /// The subject has another face to Tab to.
    pub(super) faces: bool,
    pub(super) remembers: bool,
    pub(super) watchable: bool,
    /// `Some` on your own sheet: `Some(false)` greys the key rather than
    /// hiding it, since one that sometimes vanishes is one the player stops
    /// looking for.
    pub(super) level_up: Option<bool>,
    /// The program has banked points to hold or spend.
    pub(super) points: bool,
}

fn dim(key: &str, verb: &str) -> Vec<Piece> {
    vec![
        (key.to_string(), hud::palette::FAINT, true),
        (format!(" {verb}"), hud::palette::FAINT, false),
    ]
}

pub(in crate::render) fn group(chips: Vec<Vec<Piece>>) -> Vec<Piece> {
    let mut out = Vec::new();
    for chip in chips {
        if !out.is_empty() {
            out.push(sep());
        }
        out.extend(chip);
    }
    out
}

/// The groups in priority order: what is last goes first. Empty groups are
/// absent, so a wild program's bar is group 2 and `Esc back`.
pub(super) fn keybar_groups(k: &KeybarState) -> Vec<Vec<Piece>> {
    let mut nav = Vec::new();
    if k.cyclable {
        nav.push(keycap("↑/↓", "subject"));
        nav.push(keycap("1–9", "jump"));
    }
    if k.faces {
        nav.push(keycap("Tab", "face"));
    }
    let mut acts = vec![keycap("D", "dossier")];
    if k.remembers {
        acts.push(keycap("R", "memories"));
    }
    if k.watchable {
        acts.push(keycap("W", "watch"));
    }
    match k.level_up {
        Some(true) => acts.push(keycap("L", "level up")),
        Some(false) => acts.push(dim("L", "level up")),
        None => {}
    }
    if k.points {
        acts.push(keycap("H", "hold"));
        acts.push(keycap("S", "spend"));
    }
    [nav, acts, vec![keycap("Esc", "back")]]
        .into_iter()
        .filter(|g| !g.is_empty())
        .map(group)
        .collect()
}

/// The dossier's bar: paging only when there is somewhere to page to, then
/// the way back to the manifest it was opened from.
pub(super) fn dossier_groups(cyclable: bool) -> Vec<Vec<Piece>> {
    let mut groups = Vec::new();
    if cyclable {
        groups.push(group(vec![keycap("↑/↓", "subject"), keycap("1–9", "jump")]));
    }
    groups.push(group(vec![keycap("Esc", "back to manifest")]));
    groups
}

/// The pieces of `groups` that fit `avail`: whole groups, from the front.
fn fitted_groups(groups: &[Vec<Piece>], avail: f32, painter: &Painter, m: &Metrics) -> Vec<Piece> {
    let divider = (" │ ".to_string(), hud::palette::KEYBAR_DIVIDER, false);
    fitting_joined(groups, &divider, avail, painter, m)
}

/// Draws the bar in `rect`, with a rule a half-gap above it.
pub(super) fn draw_keybar(rect: Rect, k: &KeybarState, painter: &Painter, m: &Metrics) {
    draw_groups(rect, &keybar_groups(k), painter, m);
}

/// The dossier's bar, in the same place and drawn the same way.
pub(super) fn draw_dossier_keybar(rect: Rect, cyclable: bool, painter: &Painter, m: &Metrics) {
    draw_groups(rect, &dossier_groups(cyclable), painter, m);
}

pub(in crate::render) fn draw_groups(
    rect: Rect,
    groups: &[Vec<Piece>],
    painter: &Painter,
    m: &Metrics,
) {
    painter.rect(
        rect.x,
        rect.y - m.gap / 2.0,
        rect.w,
        1.0,
        hud::palette::DIVIDER,
    );
    let pieces = fitted_groups(groups, rect.w, painter, m);
    let runs: Vec<TextRun> = pieces
        .iter()
        .map(|(text, color, bold)| TextRun {
            text,
            bold: *bold,
            color: *color,
        })
        .collect();
    painter.ui_runs(
        &runs,
        rect.x,
        rect.y + rect.h / 2.0 + m.small() as f32 * 0.35,
        m.small(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;

    const FULL: KeybarState = KeybarState {
        cyclable: true,
        faces: true,
        remembers: true,
        watchable: true,
        level_up: Some(true),
        points: true,
    };

    fn text(pieces: &[Piece]) -> String {
        pieces.iter().map(|(t, _, _)| t.as_str()).collect()
    }

    #[test]
    fn the_bar_offers_only_the_keys_valid_now_in_the_specs_order() {
        let groups: Vec<String> = keybar_groups(&FULL).iter().map(|g| text(g)).collect();
        assert_eq!(
            groups,
            [
                "↑/↓ subject · 1–9 jump · Tab face",
                "D dossier · R memories · W watch · L level up · H hold · S spend",
                "Esc back"
            ]
        );
        let wild = KeybarState {
            cyclable: false,
            faces: false,
            remembers: false,
            watchable: false,
            level_up: None,
            points: false,
        };
        let groups: Vec<String> = keybar_groups(&wild).iter().map(|g| text(g)).collect();
        assert_eq!(
            groups,
            ["D dossier", "Esc back"],
            "a wild program has no paging group"
        );
    }

    #[test]
    fn the_dossier_bar_pages_only_where_there_is_somewhere_to_go() {
        let text_of = |cyclable| -> Vec<String> {
            dossier_groups(cyclable).iter().map(|g| text(g)).collect()
        };
        assert_eq!(
            text_of(true),
            ["↑/↓ subject · 1–9 jump", "Esc back to manifest"]
        );
        assert_eq!(text_of(false), ["Esc back to manifest"]);
    }

    #[test]
    fn a_reopenable_level_up_is_lit_and_a_spent_one_is_faint() {
        let colour_of = |open| {
            let k = KeybarState {
                level_up: Some(open),
                ..FULL
            };
            keybar_groups(&k)
                .concat()
                .into_iter()
                .find(|(t, _, _)| t == "L")
                .map(|(_, c, _)| c)
        };
        assert_eq!(colour_of(true), Some(hud::palette::EMPHASIS));
        assert_eq!(colour_of(false), Some(hud::palette::FAINT));
    }

    /// The bar drops whole groups from the end and never draws past the
    /// width it was given, at every window.
    #[test]
    fn the_bar_fits_by_dropping_groups_at_every_width() {
        let m = ui_metrics(720.0);
        let (_, _) = with_painter(|p| {
            let full = fitted_groups(&keybar_groups(&FULL), f32::MAX, p, &m);
            let all = p.measure_ui_advance(text(&full), m.small());
            let mut previous = usize::MAX;
            for step in (0..=40).rev() {
                let avail = all * step as f32 / 40.0;
                let bar = fitted_groups(&keybar_groups(&FULL), avail, p, &m);
                assert!(p.measure_ui_advance(text(&bar), m.small()) <= avail);
                let groups = text(&bar).matches(" │ ").count() + usize::from(!bar.is_empty());
                assert!(groups <= previous, "narrowing never adds a group back");
                previous = groups;
            }
            assert_eq!(
                previous, 0,
                "no room draws nothing rather than a clipped key"
            );
        });
    }
}
