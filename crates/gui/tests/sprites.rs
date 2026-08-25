//! Guards the shipped sprite assets against the one-cell contract.
//!
//! `text::map_cell` draws a map glyph at `16 x zoom` px with zoom clamped
//! to 1..4, so a 16x16 sprite lands on exactly 16/32/48/64 px — integer
//! multiples of its authored size, which is what keeps nearest-neighbour
//! sampling crisp. `font_rasterization.rs` holds unscii to the same ladder
//! for the same reason.
//!
//! A sprite authored at any other size still draws; it just blurs at some
//! zoom, silently and only on screen. Nothing else would catch that, which
//! is why it is a census over the real directory rather than a unit test.

use std::path::{Path, PathBuf};

/// The authored edge every sprite must have, in pixels.
const SPRITE_NATIVE: u32 = 16;

fn sprites_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/sprites")
}

/// Width, height, bit depth and colour type from a PNG's IHDR.
///
/// Hand-read rather than decoded: the header is at a fixed offset and this
/// crate has no image dependency outside bevy's own graph, which a test
/// cannot reach without booting an app.
fn png_header(path: &Path) -> (u32, u32, u8, u8) {
    let bytes = std::fs::read(path).expect("sprite must be readable");
    assert_eq!(
        &bytes[..8],
        b"\x89PNG\r\n\x1a\n",
        "{} is not a PNG",
        path.display()
    );
    let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    (w, h, bytes[24], bytes[25])
}

fn shipped_sprites() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(sprites_dir())
        .expect("assets/sprites must exist")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    found.sort();
    found
}

#[test]
fn the_shipped_sprites_are_one_cell() {
    let sprites = shipped_sprites();
    assert!(
        !sprites.is_empty(),
        "no sprites shipped, so this census proved nothing"
    );
    for path in sprites {
        let (w, h, depth, colour) = png_header(&path);
        let name = path.display();
        assert_eq!(
            (w, h),
            (SPRITE_NATIVE, SPRITE_NATIVE),
            "{name} is {w}x{h}; a sprite that is not {SPRITE_NATIVE}x{SPRITE_NATIVE} \
             is scaled by a non-integer factor at some zoom and blurs"
        );
        assert_eq!(depth, 8, "{name} must be 8 bits per channel");
        // Colour type 6 is RGBA. A sprite without an alpha channel draws an
        // opaque square over the tile's background and biome pattern.
        assert_eq!(colour, 6, "{name} must be RGBA, so the cell shows through");
    }
}

/// The ladder a sprite is drawn at, asserted against the same source
/// `font_rasterization.rs` reads, so the two cannot drift apart.
#[test]
fn the_sprite_ladder_is_integer_multiples_of_the_authored_size() {
    for zoom in feral_processes_app_core::MIN_ZOOM..=feral_processes_app_core::MAX_ZOOM {
        let drawn = SPRITE_NATIVE * zoom as u32;
        assert_eq!(
            drawn % SPRITE_NATIVE,
            0,
            "zoom {zoom} draws a sprite at {drawn}px, which is not a whole \
             multiple of its {SPRITE_NATIVE}px source"
        );
    }
}
