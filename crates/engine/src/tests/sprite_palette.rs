//! `SPRITE_PALETTE` and the quantiser that reads an existing sprite PNG back
//! into palette indices — the dev-only sprite editor's colour layer. See
//! `icon.rs`'s doc comments for why this is a **separate** palette from
//! `ICON_PALETTE` rather than an extension of it.

use crate::icon::{ICON_PALETTE, SPRITE_ALPHA_THRESHOLD, SPRITE_PALETTE, quantise, sprite_rgba};

/// The dev sprite editor's own round trip: painting a palette colour and
/// reading a file back must land on the same swatch, or every file the
/// editor writes drifts from what the artist saw on screen.
#[test]
fn every_sprite_palette_entry_quantises_to_its_own_index() {
    for (i, &(r, g, b)) in SPRITE_PALETTE.iter().enumerate() {
        assert_eq!(
            quantise((r, g, b, 255)),
            i as u8 + 1,
            "palette entry {i} ({r:#04x}, {g:#04x}, {b:#04x})"
        );
    }
}

#[test]
fn an_alpha_below_the_threshold_quantises_to_transparent_whatever_the_colour() {
    let low = SPRITE_ALPHA_THRESHOLD - 1;
    assert_eq!(quantise((255, 255, 255, low)), 0);
    assert_eq!(quantise((0, 0, 0, low)), 0);
    assert_eq!(quantise((0xc0, 0x39, 0x2b, low)), 0);
}

#[test]
fn an_alpha_at_or_above_the_threshold_matches_a_colour_instead_of_transparent() {
    assert_ne!(quantise((255, 255, 255, SPRITE_ALPHA_THRESHOLD)), 0);
}

/// Two ramp steps or hues that quantised to the same index would silently
/// merge two swatches the picker still shows as different colours.
#[test]
fn all_nineteen_sprite_palette_entries_are_distinct() {
    let mut seen = std::collections::HashSet::new();
    for &colour in SPRITE_PALETTE.iter() {
        assert!(seen.insert(colour), "{colour:?} appears more than once");
    }
    assert_eq!(seen.len(), SPRITE_PALETTE.len());
}

#[test]
fn sprite_rgba_inverts_quantise_for_every_palette_entry() {
    for (i, &(r, g, b)) in SPRITE_PALETTE.iter().enumerate() {
        assert_eq!(sprite_rgba(i as u8 + 1), (r, g, b, 255));
    }
    assert_eq!(sprite_rgba(0), (0, 0, 0, 0), "index 0 is transparent");
}

/// `ICON_PALETTE` is the player icon's save format — one hex digit per
/// cell, 0 reserved for transparent — so a sixteenth entry would silently
/// decode back as transparent. Named so a future edit here fails loudly
/// rather than as a save-corruption bug report.
#[test]
fn icon_palette_stays_at_exactly_fifteen_entries_because_that_is_the_save_format() {
    assert_eq!(ICON_PALETTE.len(), 15);
}
