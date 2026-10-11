//! SCORE, SOCIAL, TALK and the dossier: the faces that are a body in the
//! sheet's frame, with the rail and band of every face and none of STATS's
//! ledger.

use super::band::Tabs;
use super::chrome::{Head, paint_chrome};
use super::keybar::{KeybarState, draw_keybar};
use super::layout::{Face, frame_rect};
use super::ledger::draw_bands;
use super::sections::Band;
use super::stats::{faces_of, keybar_state};
use super::*;
use crate::render::score::{ScorePage, paint_score};
use feral_processes_engine::RosterRow;

/// What the body of the page is. A closed set, like `ManifestTab`.
pub(super) enum Body {
    Score(ScorePage),
}

impl Body {
    /// Which of `faces_of`'s tabs this body is.
    fn open_face(&self) -> usize {
        match self {
            Body::Score(_) => 1,
        }
    }

    /// The band under the body, if the face has one.
    fn band(&self) -> Option<Band> {
        match self {
            Body::Score(page) => Some(Band::Total(page.total.clone())),
        }
    }
}

/// Everything the page draws, gathered before any of it is laid out, so the
/// painting is a pure function of it.
pub(super) struct OtherFace {
    pub(super) view: ManifestView,
    pub(super) roster: Vec<RosterRow>,
    pub(super) tags: Vec<String>,
    pub(super) faces: Vec<&'static str>,
    pub(super) keys: KeybarState,
    pub(super) body: Body,
}

impl OtherFace {
    pub(super) fn gather(
        game: &mut Game,
        view: ManifestView,
        nav: &ManifestNav,
        owned_program: bool,
        body: Body,
    ) -> OtherFace {
        let own = matches!(view.subject, ManifestSubject::Player(_));
        let faces = faces_of(own, owned_program);
        OtherFace {
            roster: game.manifest_roster(),
            tags: header_tags(&view, game.phase_keys().story_complete),
            keys: keybar_state(&view, nav, &faces),
            faces,
            view,
            body,
        }
    }
}

/// Paints the page into `frame`.
pub(super) fn paint_other(frame: Rect, f: &OtherFace, painter: &Painter, m: &Metrics) {
    let char_w = painter.measure_ui_advance("M", m.font_size);
    let band = f.body.band();
    let l = paint_chrome(
        frame,
        Face::Other,
        band.as_ref().map_or(0, Band::rows),
        &Head {
            view: &f.view,
            roster: &f.roster,
            tags: &f.tags,
            tabs: Tabs::Faces(&f.faces, f.body.open_face()),
            call: None,
        },
        painter,
        m,
    );
    match &f.body {
        Body::Score(page) => paint_score(page, l.body(), painter, m),
    }
    if let Some(band) = &band {
        draw_bands(l.bands, band, char_w, painter, m);
    }
    draw_keybar(l.keybar, &f.keys, painter, m);
}

/// The page in the frame the sheet always takes.
pub(super) fn draw_other(f: &OtherFace, painter: &Painter, m: &Metrics) {
    paint_other(
        frame_rect(painter.screen_w(), painter.screen_h()),
        f,
        painter,
        m,
    );
}
