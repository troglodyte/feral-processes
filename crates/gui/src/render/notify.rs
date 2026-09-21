//! The notification: one moment, in a panel centred over the map, dismissed
//! by Esc.
//!
//! Not a `draw_popup`: that panel sizes itself to a row list and pages it,
//! and this one is a fixed fraction of the window holding a centred block of
//! prose. It draws through `Painter` alone — the drawing seam is not widened
//! here and no new operation was needed for it.

use feral_processes_engine::notifications::Notification;
use feral_processes_engine::text;

use super::{BORDER, Metrics, PANEL_BG, glyph_color};
use crate::paint::{Color, Painter, Rect};

/// Rows of the window's height the art is given, measured from the top of
/// the block. The whole block is then centred vertically, so this is a
/// proportion of the block and not of the window.
const ART_CELLS: f32 = 4.0;

/// How much of the window the panel takes, each way. Fixed rather than
/// fitted to the text, so every notice opens the same box and the map
/// stays visible around all of them.
const PANEL_FRACTION: f32 = 0.75;

/// How wide the prose is allowed to run, as a fraction of the panel. Long
/// measure is what makes a paragraph hard to read, and the panel is wider
/// than a comfortable paragraph.
const BODY_WIDTH_FRACTION: f32 = 0.84;

/// Drawn behind the panel. Light enough that the run shows through around
/// the notice, dark enough that the map does not compete with it.
const SCRIM: Color = Color::new(0.02, 0.02, 0.03, 0.55);

const HINT: &str = "Press Esc to continue";

/// The panel for a `w` x `h` window, centred.
fn panel_rect(w: f32, h: f32) -> Rect {
    let (pw, ph) = (w * PANEL_FRACTION, h * PANEL_FRACTION);
    Rect::new((w - pw) / 2.0, (h - ph) / 2.0, pw, ph)
}

/// How many UI cells the prose wraps at inside `panel`. Measured in UI cells
/// because the body is UI text — the map face is only ever used here for
/// the one glyph.
fn body_columns(painter: &Painter, panel: Rect, body_size: u16) -> usize {
    let columns = (panel.w * BODY_WIDTH_FRACTION) / painter.measure_ui_advance("M", body_size);
    (columns.floor() as usize).max(20)
}

/// `body` and `detail` wrapped for `panel`. A detail wraps too: it is drawn
/// from live game state, and an unlock list is as long as the node makes it.
fn wrapped(note_body: &str, detail: Option<&str>, columns: usize) -> (Vec<String>, Vec<String>) {
    let detail = detail.map_or_else(Vec::new, |d| text::wrap(d, columns));
    (wrapped_body(note_body, columns), detail)
}

/// The height of the centred block, art to hint. **The one sum** — the
/// renderer centres on it and every height census below measures it, so a
/// census cannot pass against a layout the screen no longer draws.
fn block_height(
    painter: &Painter,
    m: &Metrics,
    title: &str,
    body_lines: usize,
    detail_lines: usize,
) -> f32 {
    let title_h = painter.measure_ui(title, m.title() + 6).height;
    // Zero for the common case of no detail, so the block does not grow for
    // a notification that has nothing to report.
    let detail_h = if detail_lines == 0 {
        0.0
    } else {
        m.gap + detail_lines as f32 * m.line_height
    };
    let hint_h = painter.measure_ui(HINT, m.small()).height;
    m.line_height * ART_CELLS
        + m.gap
        + title_h
        + m.gap
        + body_lines as f32 * m.line_height
        + detail_h
        + m.gap * 2.0
        + hint_h
}

/// Draws `note` in a panel over the map.
///
/// Takes no refusal argument, unlike every popup: this screen has no verb
/// that can be refused. It is registered in `needs_status_banner` instead,
/// so a refusal raised underneath it still reaches the player in the strip
/// along the bottom — the same arrangement `Mode::FrameMap` uses.
pub(super) fn draw_notification(note: &Notification, painter: &Painter, m: &Metrics) {
    let (w, h) = (painter.screen_w(), painter.screen_h());
    painter.rect(0.0, 0.0, w, h, SCRIM);
    let panel = panel_rect(w, h);
    painter.rect(panel.x, panel.y, panel.w, panel.h, PANEL_BG);
    painter.rect_lines(panel.x, panel.y, panel.w, panel.h, 2.0, BORDER);

    let color = glyph_color(note.color);
    let art_size = m.line_height * ART_CELLS;
    let title_size = m.title() + 6;
    let body_size = m.font_size;
    let centre_x = |width: f32| panel.x + (panel.w - width) / 2.0;

    let columns = body_columns(painter, panel, body_size);
    let (lines, detail) = wrapped(&note.body, note.detail.as_deref(), columns);
    let block = block_height(painter, m, &note.title, lines.len(), detail.len());
    let mut y = panel.y + ((panel.h - block) / 2.0).max(m.pad);

    // A sprite fills its square from a **top-left**; a glyph is drawn from a
    // *baseline* and centred against measured ink. Reading the two as one
    // convention is a half-cell offset, so they are laid out separately here
    // rather than sharing a `y`.
    let art_x = centre_x(art_size);
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
            centre_x(dims.width),
            y + (art_size + dims.height) / 2.0,
            size,
            color,
        );
    }
    y += art_size + m.gap;

    let title = painter.measure_ui(&note.title, title_size);
    painter.ui(
        &note.title,
        centre_x(title.width),
        y + title.height,
        title_size,
        color,
    );
    y += title.height + m.gap;

    // Left-aligned inside a centred column, not centred per line: ragged
    // both edges is what a centred paragraph is, and it is unreadable at
    // this length.
    let left = centre_x(panel.w * BODY_WIDTH_FRACTION);
    for line in &lines {
        y += m.line_height;
        painter.ui(line, left, y, body_size, super::TEXT);
    }

    // The notification's own colour, not the body's `TEXT`: this is the
    // payout or the unlock list, meant to read as a figure rather than as
    // more prose — the reason it is a separate block under the body rather
    // than folded into it. Centred per line, since it is rarely more than one.
    if !detail.is_empty() {
        y += m.gap;
        for line in &detail {
            y += m.line_height;
            let width = painter.measure_ui(line, body_size).width;
            painter.ui(line, centre_x(width), y, body_size, color);
        }
    }
    let hint = painter.measure_ui(HINT, m.small());
    y += m.gap * 2.0 + hint.height;
    painter.ui(HINT, centre_x(hint.width), y, m.small(), super::TEXT_DIM);
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
    use feral_processes_engine::notifications::NotificationKind;
    use feral_processes_engine::{DifficultyMode, Game, ResearchTree};

    /// The longest payout a shipped contract can state, for the height
    /// census's worst case. Built from the real `assets/contracts/` through
    /// `Game::contract_catalogue`, which already words every def **and**
    /// every template at its widest pool — never a hand-picked string, since
    /// a `detail` is drawn from live game state and this crate has no wording
    /// of its own for one (`Game::reward_line`'s rule).
    fn widest_contract_payout() -> String {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let game = Game::new(41, DifficultyMode::Forgiving, &assets).expect("shipped assets");
        game.contract_catalogue()
            .into_iter()
            .map(|row| row.reward_line)
            .max_by_key(|line| line.chars().count())
            .expect("the shipped assets define contracts")
    }

    /// The panel at the smallest window the game is built for. `with_painter`
    /// opens at 1440x900; the size under test is 1280x720, the tighter case,
    /// and only the painter's text measurement is borrowed from it.
    fn smallest_panel() -> Rect {
        panel_rect(1280.0, 720.0)
    }

    /// Every notification the engine can raise, paired with its copy. The
    /// census walks `NotificationKind::all` rather than a directory now, so
    /// a new variant is measured here the moment it compiles.
    fn shipped() -> impl Iterator<
        Item = (
            NotificationKind,
            feral_processes_engine::notifications::NotificationDef,
        ),
    > {
        NotificationKind::all().into_iter().map(|k| (k, k.def()))
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
    ///
    /// A shipped `NotificationDef` carries no `detail` — it is a parameter a
    /// firing site supplies, not a `.ron` field — so this census is blind to
    /// it unless the worst case is added by hand. Every def is checked as
    /// though it were the one that got a detail, at the longest payout the
    /// shipped `assets/contracts/` can ever word, which is the honest bound:
    /// today only `milestone_contract` fires with one, but nothing stops a
    /// future site pairing a long body with a long detail, and the constraint
    /// this test exists to hold is on the *screen*, not on which id does it
    /// first.
    #[test]
    fn the_tallest_shipped_notification_fits_its_screen() {
        let m = crate::text::ui_metrics(720.0);
        let detail = widest_contract_payout();
        assert!(
            !detail.is_empty(),
            "the census measured no payout — the shipped contracts have to reach here"
        );
        crate::paint::with_painter(|p| {
            let panel = smallest_panel();
            let columns = body_columns(p, panel, m.font_size);
            for (kind, def) in shipped() {
                let (lines, detail) = wrapped(def.body, Some(&detail), columns);
                let block = block_height(p, &m, def.title, lines.len(), detail.len());
                assert!(
                    block + 2.0 * m.pad < panel.h,
                    "{} is {block}px of notification (with the widest shipped contract \
                     payout as its detail) in a {}px panel ({} lines) — this screen has \
                     no scroll, so give it one or cut the body",
                    kind,
                    panel.h,
                    lines.len()
                );
            }
        });
    }

    /// **The onboarding briefing is a template, and its real height is the
    /// filled one.** The census above measures `{description}` — seventeen
    /// characters — where the screen will draw a whole mission paragraph, so
    /// on its own it says nothing about the screen a player sees.
    ///
    /// The fix for a failure here is to shorten that mission's
    /// `description`, which shortens its contracts-screen row too. There is
    /// no scroll to give it.
    #[test]
    fn every_onboarding_briefing_fits_its_screen_once_filled() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = feral_processes_engine::Game::new(
            47,
            feral_processes_engine::DifficultyMode::Forgiving,
            assets,
        )
        .expect("shipped assets");
        let template = NotificationKind::OnboardingMission.def();

        let missions: Vec<_> = game
            .contract_catalogue()
            .into_iter()
            .filter(|row| row.tutorial)
            .collect();
        assert!(
            !missions.is_empty(),
            "the census must walk the real chain, or it passes vacuously"
        );

        let m = crate::text::ui_metrics(720.0);
        crate::paint::with_painter(|p| {
            let panel = smallest_panel();
            let columns = body_columns(p, panel, m.font_size);
            for mission in &missions {
                let body = template
                    .body
                    .replace("{name}", &mission.name)
                    .replace("{objective}", &mission.objective_line)
                    .replace("{description}", &mission.description);
                let (lines, _) = wrapped(&body, None, columns);
                let block = block_height(p, &m, &mission.name, lines.len(), 0);
                assert!(
                    block + 2.0 * m.pad < panel.h,
                    "{}'s briefing is {block}px in a {}px panel ({} lines) — this \
                     screen has no scroll, so cut the mission's description",
                    mission.id,
                    panel.h,
                    lines.len()
                );
                assert!(
                    p.measure_ui_advance(&mission.name, m.title() + 6) < panel.w - 2.0 * m.pad,
                    "{}'s name is too wide for a title, which does not wrap",
                    mission.id
                );
            }
        });
    }

    /// **Both completion screens are templates too, and both carry a
    /// `detail`.** The census two tests up measures `{objective}` — eleven
    /// characters — where the screen draws a whole objective line, and
    /// `{progress}` where it draws a sentence, so on its own it says nothing
    /// about what a player sees when a contract settles.
    ///
    /// Every shipped contract and every template at its widest is checked, on
    /// whichever of the two screens its own `tutorial` flag picks, at the
    /// longest payout the shipped assets can word. `{progress}` is measured at
    /// **every step of the real chain** through the engine's own
    /// `onboarding_progress_line` rather than a phrase written here — the
    /// sentence exists once, and a census measuring a second copy of it is
    /// measuring the copy.
    #[test]
    fn every_completion_screen_fits_its_screen_once_filled() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(53, DifficultyMode::Forgiving, assets).expect("shipped assets");
        let rows = game.contract_catalogue();
        assert!(
            rows.iter().any(|r| r.tutorial) && rows.iter().any(|r| !r.tutorial),
            "the census must reach both screens, or half of it passes vacuously"
        );
        let steps = rows.iter().filter(|r| r.tutorial).count();
        let progress = (0..=steps)
            .map(|done| feral_processes_engine::contracts::onboarding_progress_line(done, steps))
            .max_by_key(|line| line.chars().count())
            .expect("the range is never empty");

        let m = crate::text::ui_metrics(720.0);
        crate::paint::with_painter(|p| {
            let panel = smallest_panel();
            let columns = body_columns(p, panel, m.font_size);
            for row in &rows {
                let kind = if row.tutorial {
                    NotificationKind::OnboardingComplete
                } else {
                    NotificationKind::ContractClosed
                };
                let def = kind.def();
                let body = def
                    .body
                    .replace("{name}", &row.name)
                    .replace("{objective}", &row.objective_line)
                    .replace("{progress}", &progress);
                assert!(
                    !body.contains('{'),
                    "{kind} has a hole this census does not fill: {body:?}"
                );
                let (lines, detail) = wrapped(&body, Some(&row.reward_line), columns);
                let block = block_height(p, &m, def.title, lines.len(), detail.len());
                assert!(
                    block + 2.0 * m.pad < panel.h,
                    "{}'s completion screen is {block}px in a {}px panel ({} lines) — this \
                     screen has no scroll, so shorten the contract's name or its objective \
                     wording",
                    row.id,
                    panel.h,
                    lines.len()
                );
            }
        });
    }

    /// **The research alert is a template too**, filled from the finished
    /// project's own name and description, with its unlock list as the
    /// detail. Every shipped node is measured as filled, since the census
    /// above measures `{description}` where the screen draws a paragraph.
    ///
    /// The fix for a failure here is to shorten that node's `description`,
    /// which the research screen draws too. There is no scroll to give it.
    #[test]
    fn every_research_alert_fits_its_screen_once_filled() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let mut game = Game::new(59, DifficultyMode::Forgiving, assets).expect("shipped assets");
        // Every shipped node, not only the visible ones: this census
        // measures content against a screen with no scroll, and
        // `research_nodes` narrows to what has been discovered.
        for def in game.research_defs() {
            game.discover_research(&def.id);
        }
        let nodes = game.research_nodes(ResearchTree::Base);
        assert!(
            nodes.iter().any(|n| n.unlocks.is_some()),
            "the census must reach a detail, or its widest case passes vacuously"
        );
        let def = NotificationKind::ResearchComplete.def();

        let m = crate::text::ui_metrics(720.0);
        crate::paint::with_painter(|p| {
            let panel = smallest_panel();
            let columns = body_columns(p, panel, m.font_size);
            for node in &nodes {
                let body = def
                    .body
                    .replace("{name}", &node.name)
                    .replace("{description}", &node.description);
                assert!(
                    !body.contains('{'),
                    "the research alert has a hole this census does not fill: {body:?}"
                );
                let (lines, detail) = wrapped(&body, node.unlocks.as_deref(), columns);
                let block = block_height(p, &m, def.title, lines.len(), detail.len());
                assert!(
                    block + 2.0 * m.pad < panel.h,
                    "{}'s research alert is {block}px in a {}px panel ({} lines) — this \
                     screen has no scroll, so shorten the node's description",
                    node.id,
                    panel.h,
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
            for (kind, def) in shipped() {
                let width = p.measure_ui_advance(def.title, m.title() + 6);
                assert!(
                    width < smallest_panel().w - 2.0 * m.pad,
                    "{kind} has a {width}px title in a {}px panel",
                    smallest_panel().w
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
            detail: None,
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

    /// A contract's payout goes on the alert screen, not just the log — it
    /// is drawn in the notification's own colour rather than the body's
    /// `TEXT`, so it reads as a figure and not as more prose.
    #[test]
    fn a_detail_is_drawn_in_the_notifications_own_colour() {
        let mut note = note();
        note.detail = Some("40 Credits, 25 XP".into());
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_sprites(crate::paint::SpriteTable::default(), |p| {
            draw_notification(&note, p, &m)
        });

        let runs = crate::paint::painted_runs_in(&shapes, glyph_color(note.color), false);
        assert!(
            runs.iter().any(|r| r.contains("40 Credits, 25 XP")),
            "the payout must be drawn in the notification's own colour: {runs:?}"
        );
    }

    /// No detail draws no extra line at all — the common case, and every
    /// notification but a completed contract's.
    #[test]
    fn no_detail_draws_no_extra_line() {
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_sprites(crate::paint::SpriteTable::default(), |p| {
            draw_notification(&note(), p, &m)
        });

        let texts = crate::paint::painted_text(&shapes);
        // The glyph, the title, the body ("B") and the hint are the only
        // text this fixture draws with no detail set.
        assert_eq!(
            texts,
            vec![">", "T", "B", "Press Esc to continue"],
            "an absent detail must draw nothing beyond the glyph, title, body and hint"
        );
    }

    /// No notification may name a sprite the game does not ship — not a
    /// failure (the glyph covers it) but a silent one, and a typo in a name
    /// reads as the art never having been drawn.
    #[test]
    fn every_shipped_sprite_name_has_a_file() {
        let art = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/sprites");
        for (kind, def) in shipped() {
            let Some(name) = def.sprite else { continue };
            assert!(
                art.join(format!("{name}.png")).exists(),
                "{kind} names the sprite {name:?}, which is not in assets/sprites/"
            );
        }
    }

    /// The notice is a panel over the map, not the whole window: bordered,
    /// three quarters of the window each way, and centred — so a quarter of
    /// the map stays in view around it.
    #[test]
    fn the_notice_is_a_centred_panel_three_quarters_of_the_window() {
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_sprites(crate::paint::SpriteTable::default(), |p| {
            draw_notification(&note(), p, &m)
        });

        let boxes = crate::paint::painted_rect_stroke_boxes(&shapes, BORDER);
        assert_eq!(boxes.len(), 1, "one bordered panel: {boxes:?}");
        let b = boxes[0];
        let near = |a: f32, e: f32| (a - e).abs() <= 2.0;
        assert!(
            near(b.width(), 1440.0 * 0.75) && near(b.height(), 900.0 * 0.75),
            "the panel is 75% of the 1440x900 window each way: {b:?}"
        );
        assert!(
            near(b.min.x, 1440.0 * 0.125) && near(b.min.y, 900.0 * 0.125),
            "and centred: {b:?}"
        );
    }

    /// Everything the notice says is inside its panel — the glyph, title,
    /// body, detail and hint all centre on the panel and not on the window.
    #[test]
    fn every_line_of_the_notice_lands_inside_its_panel() {
        let mut note = note();
        note.body = "A body long enough to be worth wrapping. ".repeat(6);
        note.detail = Some("Unlocks: A Very Long Structure Name, ".repeat(8));
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_sprites(crate::paint::SpriteTable::default(), |p| {
            draw_notification(&note, p, &m)
        });

        let panel = panel_rect(1440.0, 900.0);
        let texts = crate::paint::painted_text_boxes(&shapes);
        assert!(texts.len() > 4, "the fixture must wrap: {texts:?}");
        for (_, text, r) in texts {
            assert!(
                r.x >= panel.x
                    && r.y >= panel.y
                    && r.x + r.w <= panel.x + panel.w
                    && r.y + r.h <= panel.y + panel.h,
                "{text:?} at {r:?} spills out of the panel {panel:?}"
            );
        }
    }

    #[test]
    fn the_shipped_colours_all_resolve() {
        let mut seen = 0;
        for (kind, def) in shipped() {
            let c = glyph_color(def.color);
            assert!(c.a > 0.0, "{kind} draws its art invisible");
            seen += 1;
        }
        assert_eq!(
            seen,
            NotificationKind::all().len(),
            "the census walked none"
        );
    }
}
