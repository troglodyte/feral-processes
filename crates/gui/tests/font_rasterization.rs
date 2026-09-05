//! Guards the empirical assumptions the two fonts rest on.
//!
//! unscii ships as vectorized outlines of a bitmap rather than a real
//! bitmap — HEX and PCF are its only true bitmap formats, and macroquad's
//! loader needs outlines — so it is pixel-crisp only if the rasterizer
//! lands on the pixel grid. macroquad rasterizes with fontdue, so testing
//! fontdue directly exercises the same path without needing a GL context
//! or a window.

use std::sync::LazyLock;

use feral_processes_app_core::MAX_ZOOM;

const UNSCII: &[u8] = include_bytes!("../../../assets/fonts/unscii-16.ttf");
const UI_FONT: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSansMono.ttf");

/// The sizes `text::map_cell` draws map glyphs at: 1x-4x unscii-16's
/// native 16px cell.
const LADDER: [f32; 4] = [16.0, 32.0, 48.0, 64.0];

/// How far a coverage byte may sit from fully-off or fully-on before it
/// counts as antialiasing rather than a hard pixel edge.
const BLUR_TOLERANCE: u8 = 24;

/// Parse the font once and reuse it across all test invocations, rather than
/// re-parsing the same 280 KB file 380 times (4 sizes × 95 ASCII chars).
static FONT: LazyLock<fontdue::Font> = LazyLock::new(|| {
    fontdue::Font::from_bytes(UNSCII, fontdue::FontSettings::default())
        .expect("unscii-16.ttf must parse as a font")
});

static UI: LazyLock<fontdue::Font> = LazyLock::new(|| {
    fontdue::Font::from_bytes(UI_FONT, fontdue::FontSettings::default())
        .expect("DejaVuSansMono.ttf must parse as a font")
});

fn blurry_pixels(font: &fontdue::Font, size: f32, ch: char) -> Vec<u8> {
    font.rasterize(ch, size)
        .1
        .into_iter()
        .filter(|&c| c > BLUR_TOLERANCE && c < 255 - BLUR_TOLERANCE)
        .collect()
}

#[test]
fn unscii_rasterizes_crisp_at_every_map_zoom_step() {
    // Guards against the ladder silently falling out of sync if the
    // supported zoom range ever changes.
    assert_eq!(LADDER.len(), MAX_ZOOM as usize);

    // Every species and structure glyph under assets/ is printable ASCII,
    // so this sweep covers everything the map can ever draw.
    for size in LADDER {
        for ch in ' '..='~' {
            let blurry = blurry_pixels(&FONT, size, ch);
            assert!(
                blurry.is_empty(),
                "{ch:?} at {size}px has {} antialiased pixels (coverages {:?}) \
                 — the glyph is not landing on the pixel grid",
                blurry.len(),
                &blurry[..blurry.len().min(8)]
            );
        }
    }
}

/// The battle ledger builds its columns by padding strings to a cell width,
/// which lines up only if every glyph advances the same distance. The UI
/// font is DejaVu Sans *Mono*, so this should hold — but the ledger is
/// unreadable if it ever stops holding, and "the font is monospace" was an
/// unstated assumption until this test existed.
///
/// Tested through fontdue for the same reason the unscii sweep is: it is the
/// rasterizer macroquad uses, so no window or GL context is needed.
#[test]
fn the_ui_font_advances_every_glyph_equally() {
    // Every character the roster rows can contain: stat digits, the HP
    // slash, the truncation marker, and the letters of a species or
    // companion name.
    let sample: Vec<char> = (' '..='~').chain(['…']).collect();
    let reference = UI.metrics('M', 16.0).advance_width;
    for &ch in &sample {
        let advance = UI.metrics(ch, 16.0).advance_width;
        assert!(
            (advance - reference).abs() < 0.01,
            "{ch:?} advances {advance}px against {reference}px for 'M' — the UI \
             font is not monospace, so the ledger's padded columns cannot line up"
        );
    }
}

/// The HUD's border strips draw block glyphs, box-drawing rules and a
/// handful of symbols that are not ASCII — meters are `█` runs, the vitals
/// strip's perk row leads with `▸`, and the build queue marks a finished
/// site with `✓`.
///
/// The design handoff says to verify these exist before starting and lists
/// ASCII fallbacks in case any are missing. They are all present in DejaVu
/// Sans Mono, so what this test is actually for is the *next* font: a swap
/// that drops one of them turns a meter into a row of tofu boxes, which
/// reads as the HUD being broken rather than as a font being wrong.
#[test]
fn the_ui_font_has_every_glyph_the_hud_draws() {
    for (ch, name) in [
        ('\u{2588}', "FULL BLOCK — meter fill and trough"),
        ('\u{2589}', "LEFT SEVEN EIGHTHS BLOCK — crew strip"),
        ('\u{2591}', "LIGHT SHADE — strict-16 trough fallback"),
        ('\u{25B8}', "BLACK RIGHT-POINTING SMALL TRIANGLE — pointer"),
        ('\u{2713}', "CHECK MARK — a finished build site"),
        ('\u{2192}', "RIGHTWARDS ARROW — program to target"),
        ('\u{00B7}', "MIDDLE DOT — field separator"),
        ('\u{2500}', "BOX DRAWINGS LIGHT HORIZONTAL"),
        ('\u{2502}', "BOX DRAWINGS LIGHT VERTICAL"),
    ] {
        let (metrics, coverage) = UI.rasterize(ch, 16.0);
        assert!(
            metrics.width > 0 && metrics.height > 0,
            "{ch:?} ({name}) has no raster box in the UI font"
        );
        assert!(
            coverage.iter().any(|&c| c > 0),
            "{ch:?} ({name}) rasterizes to an empty bitmap — the font has no \
             glyph for it and it will draw as tofu"
        );
    }
}

/// The compass block draws `stack::Heading::arrow`'s answer, and a missing
/// glyph in either face is a box or a blank rather than a compile error —
/// so the nine are held here, where the fonts' other empirical assumptions
/// already live.
///
/// Both faces, because the block is UI text today and a future caller
/// drawing one on the map grid would reach for unscii without thinking to
/// check.
#[test]
fn both_fonts_carry_every_compass_arrow() {
    for ch in ['↑', '↗', '→', '↘', '↓', '↙', '←', '↖', '●'] {
        for (name, font) in [("unscii", &*FONT), ("DejaVu Sans Mono", &*UI)] {
            assert_ne!(
                font.lookup_glyph_index(ch),
                0,
                "{name} has no glyph for {ch:?} — the compass would draw a box"
            );
            let (metrics, _) = font.rasterize(ch, 16.0);
            assert!(
                metrics.width > 0 && metrics.height > 0,
                "{name} rasterizes {ch:?} to nothing at 16px"
            );
        }
    }
}
