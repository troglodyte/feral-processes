//! Guards the one empirical assumption the map font rests on.
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
