//! SCORE, SOCIAL, TALK and the dossier: the faces that are a body in the
//! sheet's frame, with the rail and band of every face and none of STATS's
//! ledger.

use super::band::Tabs;
use super::chrome::{Head, paint_chrome};
use super::keybar::{KeybarState, draw_dossier_keybar, draw_keybar};
use super::layout::{Face, frame_rect};
use super::ledger::draw_bands;
use super::sections::Band;
use super::stats::{faces_of, keybar_state};
use super::*;
use crate::render::dossier::paint_dossier;
use crate::render::score::{ScorePage, paint_score};
use crate::render::social::paint_social;
use crate::render::talk::paint_talk;
use feral_processes_engine::views::DossierReport;
use feral_processes_engine::{ExchangeView, RosterRow, SocialView};

/// What the body of the page is. A closed set, like `ManifestTab`.
pub(super) enum Body {
    Score(ScorePage),
    Social(SocialView),
    Talk(Vec<ExchangeView>),
    Dossier(DossierReport),
}

impl Body {
    /// What holds the band's right end: the open one of `faces_of`'s tabs,
    /// or the dossier's tag.
    fn tabs<'a>(&self, faces: &'a [&'static str]) -> Tabs<'a> {
        match self {
            Body::Score(_) | Body::Social(_) => Tabs::Faces(faces, 1),
            Body::Talk(_) => Tabs::Faces(faces, 2),
            Body::Dossier(_) => Tabs::Dossier,
        }
    }

    /// The band under the body, if the face has one.
    fn band(&self) -> Option<Band> {
        match self {
            Body::Score(page) => Some(Band::Total(page.total.clone())),
            Body::Social(_) | Body::Talk(_) | Body::Dossier(_) => None,
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
            tabs: f.body.tabs(&f.faces),
            call: None,
            level_up: None,
        },
        painter,
        m,
    );
    match &f.body {
        Body::Score(page) => paint_score(page, l.body(), painter, m),
        Body::Social(view) => paint_social(view, &f.roster, &l, painter, m),
        Body::Dossier(report) => paint_dossier(report, l.body(), painter, m),
        Body::Talk(exchanges) => {
            // `who` on a line is the roster's label for the subject.
            let own = f
                .roster
                .iter()
                .find(|r| r.entity == f.view.entity)
                .map_or("", |r| r.name.as_str());
            paint_talk(exchanges, own, &l, painter, m);
        }
    }
    if let Some(band) = &band {
        draw_bands(l.bands, band, char_w, painter, m);
    }
    match f.body {
        Body::Dossier(_) => draw_dossier_keybar(l.keybar, f.keys.cyclable, painter, m),
        _ => draw_keybar(l.keybar, &f.keys, painter, m),
    }
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
