//! The player's 16x16 map avatar and its save-string codec.
//!
//! A `PlayerIcon` is 256 palette indices, one per pixel, row-major from the
//! top-left. Index 0 is transparent rather than a fifteenth palette entry —
//! an icon the player never opens the editor for is legitimately blank, and
//! a blank icon has to encode and decode like any other rather than needing
//! a special case at the edges of the feature.

/// Opaque RGB triples, palette order. Index 0 is deliberately absent here:
/// it means transparent and is handled by `PlayerIcon::rgba`, the one place
/// an index becomes a colour, so the gui and any test agree about it.
///
/// Five steps of value, then ten hues. At 16x16 the value ramp is the half
/// that reads — see `assets/sprites/README.md` on shading by value rather
/// than hue. Brown is on the list on purpose: without it a figure this size
/// has no skin, leather or wood to draw with.
pub const ICON_PALETTE: [(u8, u8, u8); 15] = [
    (0x1c, 0x1c, 0x1c),
    (0x4a, 0x4a, 0x4a),
    (0x7d, 0x7d, 0x7d),
    (0xb4, 0xb4, 0xb4),
    (0xf2, 0xf2, 0xf2),
    (0xc0, 0x39, 0x2b),
    (0xd9, 0x7b, 0x2b),
    (0xe8, 0xc5, 0x47),
    (0x4f, 0x9d, 0x4f),
    (0x3f, 0xa9, 0xa0),
    (0x4b, 0xb3, 0xd9),
    (0x3b, 0x6f, 0xd4),
    (0x7a, 0x55, 0xc4),
    (0xc0, 0x4f, 0x9e),
    (0x8a, 0x5a, 0x3c),
];

/// The icon's edge, in pixels, on both axes. Public because app-core's
/// editor clamps its cursor to this grid and the gui draws that many
/// cells — a second `16` in either would be a copy of the format, not a
/// coincidence.
pub const ICON_SIZE: usize = 16;

/// Total pixel count, and the length of the encoded payload — one hex digit
/// per pixel.
const ICON_PIXELS: usize = ICON_SIZE * ICON_SIZE;

/// `encode`'s format tag. `decode` refuses anything else outright rather
/// than guessing at an older or foreign format.
const ENCODING_PREFIX: &str = "v1:";

/// The player's 16x16 map avatar.
///
/// One palette index per pixel, row-major from the top-left. There is no
/// separate alpha channel — index 0 stands for transparent, `1..=15` index
/// `ICON_PALETTE` at value `index - 1`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlayerIcon {
    pixels: [u8; ICON_PIXELS],
}

impl Default for PlayerIcon {
    /// All-transparent — a 256-element array is past what `#[derive(Default)]`
    /// can build, so this states the same thing by hand.
    fn default() -> Self {
        PlayerIcon {
            pixels: [0; ICON_PIXELS],
        }
    }
}

impl PlayerIcon {
    /// The pixel at `(x, y)`, or 0 (transparent) if either coordinate is
    /// off the 16x16 grid.
    pub fn get(&self, x: usize, y: usize) -> u8 {
        match Self::index_of(x, y) {
            Some(i) => self.pixels[i],
            None => 0,
        }
    }

    /// Paints `(x, y)` with `index`. An out-of-range coordinate or an
    /// `index` past the palette is dropped rather than panicking — the
    /// editor is the only caller and cannot produce either, and a loaded
    /// save cannot reach this path at all, since `decode` refuses first.
    pub fn set(&mut self, x: usize, y: usize, index: u8) {
        if index as usize > ICON_PALETTE.len() {
            return;
        }
        if let Some(i) = Self::index_of(x, y) {
            self.pixels[i] = index;
        }
    }

    /// Resets every pixel to transparent.
    pub fn clear(&mut self) {
        self.pixels = [0; ICON_PIXELS];
    }

    /// Whether every pixel is transparent — a player who never opened the
    /// editor, or who cleared it.
    pub fn is_blank(&self) -> bool {
        self.pixels.iter().all(|&p| p == 0)
    }

    /// The pixel at `(x, y)` as RGBA. The one place index 0 becomes a
    /// transparent pixel rather than a colour, so the gui's texture upload
    /// and any test agree about it.
    pub fn rgba(&self, x: usize, y: usize) -> (u8, u8, u8, u8) {
        match self.get(x, y) {
            0 => (0, 0, 0, 0),
            index => {
                let (r, g, b) = ICON_PALETTE[index as usize - 1];
                (r, g, b, 255)
            }
        }
    }

    /// `"v1:"` followed by 256 lowercase hex digits, row-major — one per
    /// pixel, `decode`'s exact inverse.
    pub fn encode(&self) -> String {
        let mut s = String::with_capacity(ENCODING_PREFIX.len() + ICON_PIXELS);
        s.push_str(ENCODING_PREFIX);
        for &p in &self.pixels {
            s.push(std::char::from_digit(p as u32, 16).expect("palette index fits one hex digit"));
        }
        s
    }

    /// The inverse of `encode`. Strict and total: a wrong prefix, any
    /// length but `ENCODING_PREFIX.len() + ICON_PIXELS`, or a non-hex digit
    /// all return `None` rather than recovering partially — the caller
    /// falls back to the glyph, which is already correct on its own.
    pub fn decode(s: &str) -> Option<PlayerIcon> {
        let digits = s.strip_prefix(ENCODING_PREFIX)?;
        if digits.len() != ICON_PIXELS {
            return None;
        }
        let mut pixels = [0u8; ICON_PIXELS];
        for (i, c) in digits.chars().enumerate() {
            pixels[i] = c.to_digit(16)? as u8;
        }
        Some(PlayerIcon { pixels })
    }

    fn index_of(x: usize, y: usize) -> Option<usize> {
        if x < ICON_SIZE && y < ICON_SIZE {
            Some(y * ICON_SIZE + x)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drawn_icon_round_trips_through_encode_and_decode_unchanged() {
        let mut icon = PlayerIcon::default();
        icon.set(0, 0, 1);
        icon.set(15, 15, 15);
        icon.set(3, 7, 6);
        let decoded = PlayerIcon::decode(&icon.encode()).expect("a well-formed encoding decodes");
        assert_eq!(icon, decoded);
    }

    #[test]
    fn a_default_icon_encodes_to_the_prefix_plus_256_zeros() {
        let icon = PlayerIcon::default();
        let expected = format!("v1:{}", "0".repeat(ICON_PIXELS));
        assert_eq!(icon.encode(), expected);
    }

    #[test]
    fn decode_refuses_a_wrong_prefix() {
        let payload = "0".repeat(ICON_PIXELS);
        assert_eq!(PlayerIcon::decode(&format!("v2:{payload}")), None);
    }

    #[test]
    fn decode_refuses_one_character_short() {
        let payload = "0".repeat(ICON_PIXELS - 1);
        assert_eq!(PlayerIcon::decode(&format!("v1:{payload}")), None);
    }

    #[test]
    fn decode_refuses_one_character_long() {
        let payload = "0".repeat(ICON_PIXELS + 1);
        assert_eq!(PlayerIcon::decode(&format!("v1:{payload}")), None);
    }

    #[test]
    fn decode_refuses_a_non_hex_digit_in_the_middle() {
        let mut payload: Vec<u8> = "0".repeat(ICON_PIXELS).into_bytes();
        payload[128] = b'g';
        let s = format!("v1:{}", String::from_utf8(payload).unwrap());
        assert_eq!(PlayerIcon::decode(&s), None);
    }

    /// A sixteenth colour would be unencodable — `encode` emits one hex
    /// digit per pixel, and `'0'..='f'` is 16 values total with `'0'`
    /// already spent on transparent. Pinning the length here means that
    /// ceiling fails a fast, named test rather than surfacing as a palette
    /// entry the editor can paint with but no save can ever store.
    #[test]
    fn the_palette_has_room_for_exactly_fifteen_colours_because_one_hex_digit_encodes_them() {
        assert_eq!(ICON_PALETTE.len(), 15);
    }

    #[test]
    fn rgba_is_transparent_at_index_zero_and_opaque_otherwise() {
        let mut icon = PlayerIcon::default();
        icon.set(2, 2, 6);
        assert_eq!(icon.rgba(0, 0), (0, 0, 0, 0));
        let (r, g, b, a) = icon.rgba(2, 2);
        assert_eq!((r, g, b), ICON_PALETTE[5]);
        assert_eq!(a, 255);
    }
}
