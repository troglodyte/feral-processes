//! The full-screen notification: one moment, centred, dismissed by any key.
//!
//! The only screen in the game that is neither a popup over the map nor a
//! pane of it. It draws through `Painter` alone — the drawing seam is not
//! widened here and no new operation was needed for it.

use feral_processes_engine::notifications::Notification;
use feral_processes_engine::text;

use super::{Metrics, glyph_color};
use crate::paint::{Color, Painter};

/// Rows of the window's height the art is given, measured from the top of
/// the block. The whole block is then centred vertically, so this is a
/// proportion of the block and not of the window.
const ART_CELLS: f32 = 4.0;

/// How wide the prose is allowed to run, as a fraction of the window. Long
/// measure is what makes a paragraph hard to read; the popup screens get
/// this for free from `draw_popup`'s panel, and this screen has no panel.
const BODY_WIDTH_FRACTION: f32 = 0.62;

/// Drawn behind everything. Not `Painter::clear`'s black: the map is already
/// painted underneath by the caller, and letting it show through faintly is
/// what says the run is still there behind the notice.
const SCRIM: Color = Color::new(0.02, 0.02, 0.03, 0.92);

/// Draws `note` over the whole window.
///
/// Takes no refusal argument, unlike every popup: this screen has no verb
/// that can be refused. It is registered in `needs_status_banner` instead,
/// so a refusal raised underneath it still reaches the player in the strip
/// along the bottom — the same arrangement `Mode::FrameMap` uses.
pub(super) fn draw_notification(note: &Notification, painter: &Painter, m: &Metrics) {
    let (w, h) = (painter.screen_w(), painter.screen_h());
    painter.rect(0.0, 0.0, w, h, SCRIM);

    let color = glyph_color(note.color);
    let art_size = m.line_height * ART_CELLS;
    let title_size = m.title() + 6;
    let body_size = m.font_size;

    // Measured in UI cells, because the body is UI text — the map face is
    // only ever used here for the one glyph.
    let columns =
        ((w * BODY_WIDTH_FRACTION) / painter.measure_ui_advance("M", body_size)).floor() as usize;
    let lines = wrapped_body(&note.body, columns.max(20));

    let title_h = painter.measure_ui(&note.title, title_size).height;
    let body_h = lines.len() as f32 * m.line_height;
    let hint = "Press any key to continue";
    let hint_h = painter.measure_ui(hint, m.small()).height;

    let block = art_size + m.gap + title_h + m.gap + body_h + m.gap * 2.0 + hint_h;
    let mut y = ((h - block) / 2.0).max(m.pad);

    // A sprite fills its square from a **top-left**; a glyph is drawn from a
    // *baseline* and centred against measured ink. Reading the two as one
    // convention is a half-cell offset, so they are laid out separately here
    // rather than sharing a `y`.
    let art_x = (w - art_size) / 2.0;
    let drew_sprite = note
        .sprite
        .as_deref()
        .is_some_and(|name| painter.sprite(name, art_x, y, art_size, color));
    if !drew_sprite {
        let glyph = note.glyph.to_string();
        let size = art_size as u16;
        let dims = painter.measure_map(&glyph, size);
        painter.map(
            &glyph,
            (w - dims.width) / 2.0,
            y + (art_size + dims.height) / 2.0,
            size,
            color,
        );
    }
    y += art_size + m.gap;

    let title_w = painter.measure_ui(&note.title, title_size).width;
    painter.ui(
        &note.title,
        (w - title_w) / 2.0,
        y + title_h,
        title_size,
        color,
    );
    y += title_h + m.gap;

    // Left-aligned inside a centred column, not centred per line: ragged
    // both edges is what a centred paragraph is, and it is unreadable at
    // this length.
    let left = (w - w * BODY_WIDTH_FRACTION) / 2.0;
    for line in &lines {
        y += m.line_height;
        painter.ui(line, left, y, body_size, super::TEXT);
    }
    y += m.gap * 2.0 + hint_h;

    let hint_w = painter.measure_ui(hint, m.small()).width;
    painter.ui(hint, (w - hint_w) / 2.0, y, m.small(), super::TEXT_DIM);
}

/// Wraps the body at `columns`, keeping blank lines between paragraphs.
///
/// `text::wrap` is the engine's, and calling it is the rule: a read-only
/// screen's row count is owned by app-core, so a per-row transform living in
/// the renderer opens a screen on rows that are not drawn. It has no notion
/// of a paragraph, so the split is here — a wrap of the whole body would
/// swallow every `\n\n` into a single block.
pub(super) fn wrapped_body(body: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for (i, para) in body.split("\n\n").enumerate() {
        if i > 0 {
            lines.push(String::new());
        }
        lines.extend(text::wrap(para, columns));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::components::GlyphColor;
    use feral_processes_engine::notifications::{NotificationDb, NotificationId};

    fn shipped() -> NotificationDb {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/notifications");
        let (db, warnings) = NotificationDb::load_dir(&dir).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        db
    }

    /// A blank line between paragraphs and nowhere else. Wrapping the whole
    /// body in one call swallows every `\n\n`, which reads as the writer
    /// having forgotten the break rather than as the renderer eating it.
    #[test]
    fn paragraph_breaks_survive_the_wrap() {
        let lines = wrapped_body("one two three\n\nfour five six", 8);
        let blanks = lines.iter().filter(|l| l.is_empty()).count();
        assert_eq!(blanks, 1, "{lines:?}");
        assert!(lines.iter().all(|l| l.chars().count() <= 8), "{lines:?}");
        assert!(lines.first().is_some_and(|l| !l.is_empty()));
        assert!(lines.last().is_some_and(|l| !l.is_empty()));
    }

    /// **The screen has no scroll.** A row past the bottom edge is dropped in
    /// silence, so what says the shipped catalogue fits is this, at the
    /// smallest window the game is built for.
    #[test]
    fn the_tallest_shipped_notification_fits_its_screen() {
        let m = crate::text::ui_metrics(720.0);
        crate::paint::with_painter(|p| {
            // `with_painter` opens at 1440x900; the height under test is the
            // 720 the metrics were taken at, which is the tighter case.
            let h = 720.0;
            let w = 1280.0;
            let columns = ((w * BODY_WIDTH_FRACTION) / p.measure_ui_advance("M", m.font_size))
                .floor() as usize;
            for def in shipped().iter() {
                let lines = wrapped_body(&def.body, columns.max(20));
                let title_h = p.measure_ui(&def.title, m.title() + 6).height;
                let hint_h = p.measure_ui("Press any key to continue", m.small()).height;
                let block = m.line_height * ART_CELLS
                    + m.gap
                    + title_h
                    + m.gap
                    + lines.len() as f32 * m.line_height
                    + m.gap * 2.0
                    + hint_h;
                assert!(
                    block + 2.0 * m.pad < h,
                    "{} is {block}px of notification in a {h}px window ({} lines) — this \
                     screen has no scroll, so give it one or cut the body",
                    def.id,
                    lines.len()
                );
            }
        });
    }

    /// A title is drawn large and **does not wrap**, so an over-long one runs
    /// off both edges rather than being clipped anywhere it could be seen.
    #[test]
    fn every_shipped_title_fits_on_one_line() {
        let m = crate::text::ui_metrics(720.0);
        crate::paint::with_painter(|p| {
            for def in shipped().iter() {
                let width = p.measure_ui_advance(&def.title, m.title() + 6);
                assert!(
                    width < 1280.0 - 2.0 * m.pad,
                    "{} has a {width}px title in a 1280px window",
                    def.id
                );
            }
        });
    }

    /// Draws one notification with `sprites` loaded and reports what landed:
    /// how many textured meshes, and every map glyph painted.
    fn drawn(note: &Notification, sprites: crate::paint::SpriteTable) -> (usize, Vec<String>) {
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_sprites(sprites, |p| draw_notification(note, p, &m));
        (
            crate::paint::painted_images(&shapes).len(),
            crate::paint::painted_map_glyphs(&shapes)
                .into_iter()
                .map(|(g, _)| g)
                .collect(),
        )
    }

    fn note() -> Notification {
        Notification {
            title: "T".into(),
            body: "B".into(),
            sprite: Some("notify_art".into()),
            glyph: '>',
            color: GlyphColor::Cyan,
        }
    }

    /// A sprite **stands in for** the glyph and never draws beside it —
    /// `Painter::sprite`'s own rule, and both halves are asserted in one
    /// test for its reason: the sprite half alone passes against a renderer
    /// that paints the texture over a glyph still sitting underneath, which
    /// looks exactly right on opaque art and is wrong the moment the art has
    /// any transparency.
    #[test]
    fn a_loaded_sprite_stands_in_for_the_glyph() {
        let mut table = crate::paint::SpriteTable::default();
        table.insert("notify_art", bevy_egui::egui::TextureId::User(1));

        let (images, glyphs) = drawn(&note(), table);

        assert_eq!(images, 1, "exactly one sprite, the notification's");
        assert!(
            !glyphs.iter().any(|g| g == ">"),
            "the glyph must give way to the sprite, not sit under it: {glyphs:?}"
        );
    }

    /// ...and a name nothing is loaded under falls back to the glyph. This
    /// is what makes `sprite:` optional, and it is the state every shipped
    /// notification is in today.
    #[test]
    fn an_unloaded_sprite_name_falls_back_to_the_glyph() {
        let (images, glyphs) = drawn(&note(), crate::paint::SpriteTable::default());

        assert_eq!(images, 0, "nothing loaded must paint no texture at all");
        assert!(
            glyphs.iter().any(|g| g == ">"),
            "the glyph is what a missing sprite falls back to: {glyphs:?}"
        );
    }

    /// Nothing in the catalogue may name a sprite the game does not ship —
    /// not a failure (the glyph covers it) but a silent one, and a typo in a
    /// name reads as the art never having been drawn.
    #[test]
    fn every_shipped_sprite_name_has_a_file() {
        let art = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/sprites");
        for def in shipped().iter() {
            let Some(name) = &def.sprite else { continue };
            assert!(
                art.join(format!("{name}.png")).exists(),
                "{} names the sprite {name:?}, which is not in assets/sprites/",
                def.id
            );
        }
    }

    #[test]
    fn the_shipped_colours_all_resolve() {
        for def in shipped().iter() {
            let c = glyph_color(def.color);
            assert!(c.a > 0.0, "{} draws its art invisible", def.id);
        }
        assert!(
            shipped()
                .get(&NotificationId::from("milestone_breach"))
                .is_some()
        );
    }
}
