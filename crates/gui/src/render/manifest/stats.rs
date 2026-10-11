//! The STATS face: the roster rail, the summary band with its vitals, the
//! three-column ledger, the band under it and the keybar, in the HUD palette.

use super::band::{Tabs, draw_vitals};
use super::chrome::{Head, paint_chrome};
use super::keybar::{KeybarState, draw_keybar};
use super::layout::{Face, frame_rect};
use super::ledger::{draw_bands, draw_ledger};
use super::sections::{Band, band_for};
use super::*;
use feral_processes_engine::{CallToAct, RosterRow, SubjectRanks};

/// Everything the face draws, gathered from the `Game` before any of it is
/// laid out. Owned so the painting is a pure function of it, and a census
/// can paint the widest page it can build at any frame.
pub(super) struct StatsFace {
    pub(super) view: ManifestView,
    pub(super) roster: Vec<RosterRow>,
    /// `None` for a wild subject, which is not on the roster to rank.
    pub(super) ranks: Option<SubjectRanks>,
    pub(super) call: Option<CallToAct>,
    pub(super) sections: Vec<Section>,
    pub(super) band: Option<Band>,
    pub(super) tags: Vec<String>,
    /// The faces Tab reaches, STATS first; empty for a wild program.
    pub(super) faces: Vec<&'static str>,
    pub(super) keys: KeybarState,
}

/// The faces a subject has: the player has SCORE, an owned program SOCIAL
/// and TALK, and a wild program only STATS.
pub(super) fn faces_of(own: bool, owned_program: bool) -> Vec<&'static str> {
    if own {
        vec!["STATS", "SCORE"]
    } else if owned_program {
        vec!["STATS", "SOCIAL", "TALK"]
    } else {
        Vec::new()
    }
}

/// The keys the sheet offers, off the subject and how the screen was opened.
/// The same on every face: the keys are the sheet's, not the face's.
pub(super) fn keybar_state(
    view: &ManifestView,
    nav: &ManifestNav,
    faces: &[&'static str],
) -> KeybarState {
    KeybarState {
        cyclable: nav.cyclable,
        faces: !faces.is_empty(),
        remembers: view.remembers(),
        watchable: nav.watchable,
        level_up: nav.level_up,
        points: matches!(&view.subject, ManifestSubject::Program(p) if p.points.is_some()),
    }
}

impl StatsFace {
    pub(super) fn gather(
        game: &mut Game,
        view: ManifestView,
        nav: &ManifestNav,
        owned_program: bool,
    ) -> StatsFace {
        let own = matches!(view.subject, ManifestSubject::Player(_));
        let faces = faces_of(own, owned_program);
        let roster = game.manifest_roster();
        let escaped = game.phase_keys().story_complete;
        StatsFace {
            ranks: game.subject_ranks(view.entity),
            call: game.call_to_act(view.entity),
            sections: sections_for(game, &view),
            band: band_for(&view, &roster),
            tags: header_tags(&view, escaped),
            keys: keybar_state(&view, nav, &faces),
            faces,
            roster,
            view,
        }
    }
}

/// Paints the face into `frame`.
pub(super) fn paint_stats(frame: Rect, f: &StatsFace, painter: &Painter, m: &Metrics) {
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let band_rows = f.band.as_ref().map_or(0, Band::rows);
    let l = paint_chrome(
        frame,
        Face::Stats,
        band_rows,
        &Head {
            view: &f.view,
            roster: &f.roster,
            tags: &f.tags,
            tabs: if f.faces.is_empty() {
                Tabs::None
            } else {
                Tabs::Faces(&f.faces, 0)
            },
            call: f.call,
            level_up: f.keys.level_up,
        },
        painter,
        m,
    );
    if let Some(vitals) = l.vitals {
        draw_vitals(vitals, &f.view, painter, m);
    }
    draw_ledger(&l.ledger, &f.sections, f.ranks.as_ref(), painter, m);
    if let Some(band) = &f.band {
        draw_bands(l.bands, band, char_w, painter, m);
    }
    draw_keybar(l.keybar, &f.keys, painter, m);
}

/// The STATS face in the frame the sheet always takes.
pub(super) fn draw_stats(f: &StatsFace, painter: &Painter, m: &Metrics) {
    paint_stats(
        frame_rect(painter.screen_w(), painter.screen_h()),
        f,
        painter,
        m,
    );
}
