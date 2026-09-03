//! The player's drawn map avatar and its save-string codec.
//!
//! **Two grids, and the ratio between them is the whole of this module's
//! geometry.** The *sprite* is 16x16 (`ICON_SIZE`) because
//! `assets/sprites/README.md` says that format is not negotiable. What the
//! player *draws* is 8x8 (`ICON_GRID`), and each drawn cell paints an
//! `ICON_CELL_PIXELS`-square block of that sprite — which under nearest
//! sampling is pixel-identical to a native 8x8 texture and leaves the
//! sprite seam untouched. `ICON_CELL_PIXELS` is that ratio, stated once so
//! no site has to assume it.
//!
//! A `PlayerIcon` is therefore 64 palette indices, one per drawn cell,
//! row-major from the top-left. Index 0 is transparent rather than a
//! fifteenth palette entry — an icon the player never opens the editor for
//! is legitimately blank, and a blank icon has to encode and decode like
//! any other rather than needing a special case at the edges of the
//! feature.

/// Opaque RGB triples, palette order. Index 0 is deliberately absent here:
/// it means transparent and is handled by `PlayerIcon::rgba`, the one place
/// an index becomes a colour, so the gui and any test agree about it.
///
/// Five steps of value, then ten hues. At this size the value ramp is the
/// half that reads — see `assets/sprites/README.md` on shading by value
/// rather than hue. Brown is on the list on purpose: without it a figure
/// this size has no skin, leather or wood to draw with.
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

/// The *sprite's* edge, in pixels, on both axes. Not negotiable — see
/// `assets/sprites/README.md`. Public because the gui's texture upload
/// sizes the image with it.
pub const ICON_SIZE: usize = 16;

/// The *drawn grid's* edge, in cells, on both axes. Public because
/// app-core's editor clamps its cursor to this grid and the gui draws that
/// many cells — a second `8` in either would be a copy of the format, not a
/// coincidence.
pub const ICON_GRID: usize = 8;

/// How many sprite pixels one drawn cell fills, on each axis. **The one
/// expression of the relationship between the two grids.** Every site that
/// crosses between them — the texture upload, the `v1` fold below — reads
/// this rather than assuming a `2`.
pub const ICON_CELL_PIXELS: usize = ICON_SIZE / ICON_GRID;

/// Total cell count, and the length of the encoded payload — one hex digit
/// per cell.
const ICON_CELLS: usize = ICON_GRID * ICON_GRID;

/// `encode`'s format tag.
const ENCODING_PREFIX: &str = "v2:";

/// The retired 16x16 format's tag. `decode` still reads it, folding each
/// `ICON_CELL_PIXELS` block down to one cell — see `decode_v1`. Anything
/// with neither prefix is refused outright rather than guessed at.
const V1_PREFIX: &str = "v1:";

/// `v1` carried one hex digit per *sprite pixel*.
const V1_DIGITS: usize = ICON_SIZE * ICON_SIZE;

/// The player's drawn map avatar.
///
/// One palette index per drawn cell, row-major from the top-left. There is
/// no separate alpha channel — index 0 stands for transparent, `1..=15`
/// index `ICON_PALETTE` at value `index - 1`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlayerIcon {
    cells: [u8; ICON_CELLS],
}

impl Default for PlayerIcon {
    /// All-transparent — stated by hand for symmetry with `clear`, which
    /// has to say the same thing.
    fn default() -> Self {
        PlayerIcon {
            cells: [0; ICON_CELLS],
        }
    }
}

impl PlayerIcon {
    /// The cell at `(x, y)`, or 0 (transparent) if either coordinate is
    /// off the 8x8 grid.
    pub fn get(&self, x: usize, y: usize) -> u8 {
        match Self::index_of(x, y) {
            Some(i) => self.cells[i],
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
            self.cells[i] = index;
        }
    }

    /// Resets every cell to transparent.
    pub fn clear(&mut self) {
        self.cells = [0; ICON_CELLS];
    }

    /// Whether every cell is transparent — a player who never opened the
    /// editor, or who cleared it.
    pub fn is_blank(&self) -> bool {
        self.cells.iter().all(|&p| p == 0)
    }

    /// The cell at `(x, y)` as RGBA. The one place index 0 becomes a
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

    /// The *sprite pixel* at `(px, py)` as RGBA — the one place a drawn
    /// cell becomes its `ICON_CELL_PIXELS` square block, so the gui's
    /// upload expands the grid without restating the ratio.
    pub fn pixel_rgba(&self, px: usize, py: usize) -> (u8, u8, u8, u8) {
        self.rgba(px / ICON_CELL_PIXELS, py / ICON_CELL_PIXELS)
    }

    /// `"v2:"` followed by 64 lowercase hex digits, row-major — one per
    /// drawn cell, `decode`'s exact inverse.
    pub fn encode(&self) -> String {
        let mut s = String::with_capacity(ENCODING_PREFIX.len() + ICON_CELLS);
        s.push_str(ENCODING_PREFIX);
        for &p in &self.cells {
            s.push(std::char::from_digit(p as u32, 16).expect("palette index fits one hex digit"));
        }
        s
    }

    /// The inverse of `encode`, plus the one retired format.
    ///
    /// Strict and total within each: a wrong length or a non-hex digit
    /// returns `None` rather than recovering partially — the caller falls
    /// back to the glyph, which is already correct on its own. A `v1`
    /// payload decodes to a perfectly ordinary icon and re-saves as `v2`;
    /// there is no second kind of `PlayerIcon`.
    pub fn decode(s: &str) -> Option<PlayerIcon> {
        if let Some(digits) = s.strip_prefix(ENCODING_PREFIX) {
            return Self::decode_v2(digits);
        }
        if let Some(digits) = s.strip_prefix(V1_PREFIX) {
            return Self::decode_v1(digits);
        }
        None
    }

    fn decode_v2(digits: &str) -> Option<PlayerIcon> {
        if digits.len() != ICON_CELLS {
            return None;
        }
        let mut cells = [0u8; ICON_CELLS];
        for (i, c) in digits.chars().enumerate() {
            cells[i] = c.to_digit(16)? as u8;
        }
        Some(PlayerIcon { cells })
    }

    /// Folds a 16x16 `v1` payload onto the 8x8 grid.
    ///
    /// A cell takes **the most frequent non-transparent index** in its
    /// block, ties broken in reading order; a block whose pixels are all
    /// transparent stays transparent. That preserves the silhouette, which
    /// is what survives the halving — sampling one corner of each block
    /// instead would drop a one-pixel outline entirely and turn a drawn
    /// figure into confetti.
    fn decode_v1(digits: &str) -> Option<PlayerIcon> {
        if digits.len() != V1_DIGITS {
            return None;
        }
        let mut pixels = [0u8; V1_DIGITS];
        for (i, c) in digits.chars().enumerate() {
            pixels[i] = c.to_digit(16)? as u8;
        }
        let mut cells = [0u8; ICON_CELLS];
        for cy in 0..ICON_GRID {
            for cx in 0..ICON_GRID {
                let mut block = Vec::with_capacity(ICON_CELL_PIXELS * ICON_CELL_PIXELS);
                for dy in 0..ICON_CELL_PIXELS {
                    for dx in 0..ICON_CELL_PIXELS {
                        let x = cx * ICON_CELL_PIXELS + dx;
                        let y = cy * ICON_CELL_PIXELS + dy;
                        block.push(pixels[y * ICON_SIZE + x]);
                    }
                }
                // Walking the block in reading order and taking a value
                // only on a *strictly* greater count is what breaks a tie
                // toward the earlier pixel.
                let (mut best, mut best_count) = (0u8, 0usize);
                for &v in &block {
                    if v == 0 {
                        continue;
                    }
                    let count = block.iter().filter(|&&p| p == v).count();
                    if count > best_count {
                        best = v;
                        best_count = count;
                    }
                }
                cells[cy * ICON_GRID + cx] = best;
            }
        }
        Some(PlayerIcon { cells })
    }

    fn index_of(x: usize, y: usize) -> Option<usize> {
        if x < ICON_GRID && y < ICON_GRID {
            Some(y * ICON_GRID + x)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One `v1` payload: 256 hex digits, row-major over the retired 16x16
    /// pixel grid, built from a closure so a test can state the shape it
    /// cares about and nothing else.
    fn v1(pixel: impl Fn(usize, usize) -> u8) -> String {
        let mut s = String::from(V1_PREFIX);
        for y in 0..ICON_SIZE {
            for x in 0..ICON_SIZE {
                s.push(std::char::from_digit(pixel(x, y) as u32, 16).unwrap());
            }
        }
        s
    }

    #[test]
    fn a_drawn_icon_round_trips_through_encode_and_decode_unchanged() {
        let mut icon = PlayerIcon::default();
        icon.set(0, 0, 1);
        icon.set(7, 7, 15);
        icon.set(3, 5, 6);
        let decoded = PlayerIcon::decode(&icon.encode()).expect("a well-formed encoding decodes");
        assert_eq!(icon, decoded);
    }

    /// The encoding's shape, stated as a literal: the tag plus one hex
    /// digit per drawn cell, 67 characters in all.
    #[test]
    fn a_default_icon_encodes_to_the_prefix_plus_64_zeros() {
        let icon = PlayerIcon::default();
        let expected = format!("v2:{}", "0".repeat(ICON_CELLS));
        assert_eq!(icon.encode(), expected);
        assert_eq!(icon.encode().len(), 67);
    }

    #[test]
    fn decode_refuses_a_wrong_prefix() {
        let payload = "0".repeat(ICON_CELLS);
        assert_eq!(PlayerIcon::decode(&format!("v3:{payload}")), None);
        assert_eq!(PlayerIcon::decode(&payload), None);
    }

    #[test]
    fn decode_refuses_one_character_short() {
        let payload = "0".repeat(ICON_CELLS - 1);
        assert_eq!(PlayerIcon::decode(&format!("v2:{payload}")), None);
    }

    #[test]
    fn decode_refuses_one_character_long() {
        let payload = "0".repeat(ICON_CELLS + 1);
        assert_eq!(PlayerIcon::decode(&format!("v2:{payload}")), None);
    }

    #[test]
    fn decode_refuses_a_non_hex_digit_in_the_middle() {
        let mut payload: Vec<u8> = "0".repeat(ICON_CELLS).into_bytes();
        payload[32] = b'g';
        let s = format!("v2:{}", String::from_utf8(payload).unwrap());
        assert_eq!(PlayerIcon::decode(&s), None);
    }

    /// A `v1` payload of the wrong length is refused on its own terms —
    /// 64 digits behind a `v1:` tag is not a short `v1`, it is a `v2`
    /// payload wearing the wrong tag, and both are equally unreadable.
    #[test]
    fn decode_refuses_a_v1_payload_of_the_wrong_length() {
        assert_eq!(
            PlayerIcon::decode(&format!("v1:{}", "0".repeat(ICON_CELLS))),
            None
        );
        assert_eq!(
            PlayerIcon::decode(&format!("v1:{}", "0".repeat(V1_DIGITS + 1))),
            None
        );
        assert_eq!(
            PlayerIcon::decode(&format!("v1:{}", "g".repeat(V1_DIGITS))),
            None
        );
    }

    /// **The `v1` fold's majority rule.** A block holding three of one
    /// colour and one of another takes the majority, not the first pixel —
    /// which is what fails against a decoder that simply samples the
    /// top-left of each block.
    #[test]
    fn a_v1_block_folds_to_its_most_frequent_non_transparent_colour() {
        // Block (0, 0) reads [5, 3 / 3, 3] — 3 is the majority, 5 is first.
        let s = v1(|x, y| match (x, y) {
            (0, 0) => 5,
            (1, 0) | (0, 1) | (1, 1) => 3,
            _ => 0,
        });
        let icon = PlayerIcon::decode(&s).expect("a v1 payload still decodes");
        assert_eq!(icon.get(0, 0), 3, "the majority colour wins the block");
    }

    /// A block whose pixels are all transparent stays transparent — the
    /// silhouette's holes are half of what reads at this size.
    #[test]
    fn a_wholly_transparent_v1_block_stays_transparent() {
        let s = v1(|x, y| if x < 2 && y < 2 { 7 } else { 0 });
        let icon = PlayerIcon::decode(&s).expect("a v1 payload still decodes");
        assert_eq!(icon.get(0, 0), 7);
        assert_eq!(icon.get(1, 0), 0, "an untouched block is transparent");
        assert_eq!(icon.get(3, 3), 0);
    }

    /// A two-two split takes the earlier of the two in reading order, and
    /// a single lit pixel among three transparent ones still carries the
    /// block — transparency never outvotes a colour.
    #[test]
    fn a_v1_tie_breaks_in_reading_order_and_one_lit_pixel_carries_its_block() {
        let s = v1(|x, y| match (x, y) {
            // Block (0, 0): [9, 9 / 4, 4] — a tie, and 9 reads first.
            (0, 0) | (1, 0) => 9,
            (0, 1) | (1, 1) => 4,
            // Block (1, 1): one lit pixel in the block's last cell.
            (3, 3) => 2,
            _ => 0,
        });
        let icon = PlayerIcon::decode(&s).expect("a v1 payload still decodes");
        assert_eq!(icon.get(0, 0), 9, "a tie breaks toward reading order");
        assert_eq!(icon.get(1, 1), 2, "transparency does not outvote a colour");
    }

    /// A decoded `v1` is an ordinary icon and re-saves as `v2` — there is
    /// no second kind of `PlayerIcon` and no format that survives a save.
    #[test]
    fn a_decoded_v1_icon_re_encodes_as_v2() {
        let s = v1(|x, y| if x < 2 && y < 2 { 7 } else { 0 });
        let icon = PlayerIcon::decode(&s).expect("a v1 payload still decodes");
        let re = icon.encode();
        assert!(re.starts_with("v2:"));
        assert_eq!(PlayerIcon::decode(&re), Some(icon));
    }

    /// A sixteenth colour would be unencodable — `encode` emits one hex
    /// digit per cell, and `'0'..='f'` is 16 values total with `'0'`
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

    /// **The two grids' ratio, asserted rather than assumed.** The drawn
    /// grid has to divide the sprite exactly, or a cell's block would fall
    /// off the texture's edge.
    #[test]
    fn the_drawn_grid_divides_the_sprite_exactly() {
        assert_eq!(ICON_SIZE % ICON_GRID, 0);
        assert_eq!(ICON_CELL_PIXELS, 2);
        assert_eq!(ICON_GRID * ICON_CELL_PIXELS, ICON_SIZE);
    }

    /// One drawn cell covers an `ICON_CELL_PIXELS` square of the sprite,
    /// and every pixel of that square is the cell's own colour.
    #[test]
    fn a_drawn_cell_covers_its_whole_pixel_block() {
        let mut icon = PlayerIcon::default();
        icon.set(3, 5, 11);
        let want = icon.rgba(3, 5);
        for dy in 0..ICON_CELL_PIXELS {
            for dx in 0..ICON_CELL_PIXELS {
                let (px, py) = (3 * ICON_CELL_PIXELS + dx, 5 * ICON_CELL_PIXELS + dy);
                assert_eq!(icon.pixel_rgba(px, py), want, "pixel ({px}, {py})");
            }
        }
        assert_eq!(
            icon.pixel_rgba(5, 10),
            (0, 0, 0, 0),
            "the block's neighbour"
        );
        assert_eq!(icon.pixel_rgba(6, 9), (0, 0, 0, 0), "the block's neighbour");
    }
}
