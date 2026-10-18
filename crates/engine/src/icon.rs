//! The player's drawn map avatar and its save-string codec.
//!
//! A `PlayerIcon` is one or two 16x16 `Canvas` frames and a pace, drawn on
//! `SPRITE_PALETTE` — the same palette and the same frames-plus-pace shape
//! as any other sprite, so the avatar is edited by the same editor and
//! uploads as an ordinary sprite sheet. Index 0 is transparent rather than a
//! palette entry: an avatar the player never drew is legitimately blank, and
//! a blank icon has to encode and decode like any other.
//!
//! The save string is `v3:<frame_ms>:<frame>[:<frame>]`. The retired `v1`
//! and `v2` formats still decode, each old colour remapped to its nearest
//! `SPRITE_PALETTE` entry, and re-save as `v3`.

/// The retired `v1`/`v2` formats' fifteen colours, kept only so `decode` can
/// remap them onto `SPRITE_PALETTE` (see `legacy_index`). Index 0 was
/// transparent; index `n` was entry `n - 1` here.
const LEGACY_ICON_PALETTE: [(u8, u8, u8); 15] = [
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

/// The palette every sprite, the player's avatar included, is drawn on. Wide
/// on purpose: it answers to no one-hex-digit save format.
///
/// Entries `1..=GREY_RAMP_LEN` of a sprite canvas (palette entries
/// `0..GREY_RAMP_LEN`) are the grey ramp; a pixel past it carries a hue.
pub const GREY_RAMP_LEN: u8 = 9;

/// Ordered ramp first, hues after: a nine-step **value** ramp, biased
/// bright, then nine hue families of six shades each, dark to light. The
/// families are fitted to the hues the shipped art in `assets/sprites/`
/// uses (a per-family k-means over its distinct opaque colours; yellow and
/// lime share one family), so a creature's colour ramps survive a load into
/// the editor. The bias
/// matters because the renderer hands egui a sprite's colour as a
/// **multiplying** tint (`assets/sprites/README.md`) — art authored near
/// white inherits the species hue, `biome_tint` and the damage dimming for
/// free, while a mid-grey ramp step multiplied by a dim species hue lands
/// near black. So the nine steps bunch toward the bright end rather than
/// spacing evenly — each gap smaller than the last, ending at pure white.
pub const SPRITE_PALETTE: [(u8, u8, u8); 63] = [
    (0x1c, 0x1c, 0x1c),
    (0x48, 0x48, 0x48),
    (0x70, 0x70, 0x70),
    (0x94, 0x94, 0x94),
    (0xb4, 0xb4, 0xb4),
    (0xd0, 0xd0, 0xd0),
    (0xe6, 0xe6, 0xe6),
    (0xf7, 0xf7, 0xf7),
    (0xff, 0xff, 0xff),
    // 0° red
    (0x45, 0x0a, 0x0a),
    (0x8d, 0x1c, 0x1c),
    (0xd8, 0x25, 0x25),
    (0xef, 0x44, 0x44),
    (0xf9, 0x71, 0x7a),
    (0xfc, 0xa5, 0xa7),
    // 20° orange/brown
    (0x1c, 0x0f, 0x05),
    (0x43, 0x20, 0x09),
    (0x80, 0x35, 0x12),
    (0x7a, 0x51, 0x28),
    (0xde, 0x5b, 0x0d),
    (0xa8, 0x87, 0x5a),
    // 50° yellow/lime
    (0x1a, 0x2e, 0x05),
    (0x45, 0x6e, 0x11),
    (0x6d, 0x96, 0x17),
    (0xf8, 0xb8, 0x13),
    (0xa3, 0xe6, 0x35),
    (0xd2, 0xbf, 0x85),
    // 140° green
    (0x05, 0x2e, 0x16),
    (0x14, 0x53, 0x2d),
    (0x15, 0x76, 0x3a),
    (0x16, 0xa3, 0x4a),
    (0x35, 0xd1, 0x6e),
    (0x86, 0xef, 0xac),
    // 160° emerald
    (0x02, 0x2c, 0x22),
    (0x05, 0x6a, 0x4d),
    (0x0a, 0x95, 0x7b),
    (0x11, 0xb9, 0x88),
    (0x30, 0xd5, 0xab),
    (0x84, 0xee, 0xce),
    // 190° cyan/teal
    (0x04, 0x2f, 0x2e),
    (0x12, 0x58, 0x53),
    (0x0f, 0x76, 0x6e),
    (0x07, 0x90, 0xb4),
    (0x27, 0xc3, 0xee),
    (0x67, 0xe8, 0xf9),
    // 230° blue
    (0x0c, 0x11, 0x25),
    (0x1c, 0x22, 0x48),
    (0x3d, 0x4a, 0x6e),
    (0x33, 0x57, 0xdb),
    (0x78, 0xa0, 0xea),
    (0xd3, 0xdc, 0xe7),
    // 280° violet
    (0x4a, 0x04, 0x4e),
    (0x79, 0x1a, 0x7f),
    (0xb2, 0x21, 0xc2),
    (0xba, 0x50, 0xf4),
    (0xd1, 0x7f, 0xfb),
    (0xf0, 0xab, 0xfc),
    // 340° rose
    (0x4c, 0x05, 0x19),
    (0x88, 0x13, 0x37),
    (0x9f, 0x12, 0x39),
    (0xbe, 0x12, 0x3c),
    (0xe1, 0x1d, 0x48),
    (0xf4, 0x3f, 0x5e),
];

/// Below this, a quantised sprite pixel is transparent regardless of its
/// colour — an editor pixel at under half opacity reads as "the artist
/// didn't mean to paint here," not as a washed-out swatch.
pub const SPRITE_ALPHA_THRESHOLD: u8 = 128;

/// The file-stem suffix that marks a sprite as authored in colour:
/// `depot.colour.png` is drawn under the key `depot` with its tint's hue
/// dropped. Here rather than in gui so the asset census can resolve a
/// `sprite:` override to either file. See `assets/sprites/README.md`.
pub const FULL_COLOUR_SUFFIX: &str = ".colour";

/// Maps an arbitrary RGBA pixel — a pixel read back from an existing sprite
/// PNG — onto `SPRITE_PALETTE`, nearest colour by squared euclidean
/// distance in RGB. A perceptual colour space is overkill at 63 swatches.
///
/// Returns 0 (transparent) below `SPRITE_ALPHA_THRESHOLD`, otherwise the
/// matched palette index **plus one** — the same convention `PlayerIcon`'s
/// codec uses: index 0 is transparent, index *n* is palette entry *n - 1*.
pub fn quantise(pixel: (u8, u8, u8, u8)) -> u8 {
    let (r, g, b, a) = pixel;
    if a < SPRITE_ALPHA_THRESHOLD {
        return 0;
    }
    let (mut best, mut best_dist) = (0usize, u32::MAX);
    for (i, &(pr, pg, pb)) in SPRITE_PALETTE.iter().enumerate() {
        let dr = r as i32 - pr as i32;
        let dg = g as i32 - pg as i32;
        let db = b as i32 - pb as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best = i;
            best_dist = dist;
        }
    }
    best as u8 + 1
}

/// `quantise`'s inverse for a valid index: 0 is transparent, and
/// `1..=SPRITE_PALETTE.len()` indexes `SPRITE_PALETTE` at `index - 1`.
pub fn sprite_rgba(index: u8) -> (u8, u8, u8, u8) {
    match index {
        0 => (0, 0, 0, 0),
        index => {
            let (r, g, b) = SPRITE_PALETTE[index as usize - 1];
            (r, g, b, 255)
        }
    }
}

/// The sprite's edge, in pixels, on both axes. Not negotiable — see
/// `assets/sprites/README.md`.
pub const ICON_SIZE: usize = 16;

/// Frames a sprite sheet may hold. The gui loader's size check and the
/// forge's `[1]`/`[2]` keys read this one number.
pub const MAX_SPRITE_FRAMES: usize = 2;

/// How long a frame shows when no `.anim.ron` says otherwise. Defined here
/// because the forge needs it for a blank subject and the avatar's legacy
/// formats decode to it; the gui loader imports it.
pub const DEFAULT_SPRITE_FRAME_MS: u32 = 600;

/// Cells in one frame, and half the length of its encoded hex.
const ICON_CELLS: usize = ICON_SIZE * ICON_SIZE;

/// `encode`'s format tag.
const V3_PREFIX: &str = "v3";

/// The retired 8x8 format's tag: one hex digit per cell, each cell a 2x2
/// block of the sprite.
const V2_PREFIX: &str = "v2";

/// The retired 16x16 format's tag: one hex digit per pixel.
const V1_PREFIX: &str = "v1";

/// `v2`'s edge, in cells.
const V2_GRID: usize = 8;

/// A square grid of palette indices, with no opinion about which palette —
/// that belongs to whoever draws on it. `PlayerIcon` is one or two `Canvas`es at
/// `ICON_SIZE`; a second editor at a different edge is another.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Canvas {
    edge: usize,
    cells: Vec<u8>,
}

impl Canvas {
    /// A blank (all-zero) canvas, `edge` cells on each side.
    pub fn new(edge: usize) -> Canvas {
        Canvas {
            edge,
            cells: vec![0; edge * edge],
        }
    }

    /// The grid's edge length, in cells.
    pub fn edge(&self) -> usize {
        self.edge
    }

    /// The cell at `(x, y)`, or 0 if either coordinate is off the grid.
    pub fn get(&self, x: usize, y: usize) -> u8 {
        match self.index_of(x, y) {
            Some(i) => self.cells[i],
            None => 0,
        }
    }

    /// Paints `(x, y)` with `index`. An out-of-range coordinate is dropped
    /// rather than panicking. Carries no palette-range guard — `Canvas`
    /// does not know which palette it is drawn from, so that check belongs
    /// to the caller.
    pub fn set(&mut self, x: usize, y: usize, index: u8) {
        if let Some(i) = self.index_of(x, y) {
            self.cells[i] = index;
        }
    }

    /// Resets every cell to 0.
    pub fn clear(&mut self) {
        self.cells.fill(0);
    }

    /// Whether every cell is 0.
    pub fn is_blank(&self) -> bool {
        self.cells.iter().all(|&p| p == 0)
    }

    fn index_of(&self, x: usize, y: usize) -> Option<usize> {
        if x < self.edge && y < self.edge {
            Some(y * self.edge + x)
        } else {
            None
        }
    }
}

/// The player's drawn map avatar: one or two `ICON_SIZE` frames of palette
/// indices and the pace they play at.
///
/// Fields are private so every value in existence passed `new`'s guard —
/// `Canvas::set` has no palette-range check of its own, so this is the one
/// place an index past `SPRITE_PALETTE` is refused, for the editor and the
/// decoder alike.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlayerIcon {
    frames: Vec<Canvas>,
    frame_ms: u32,
}

impl Default for PlayerIcon {
    /// One blank frame at the default pace.
    fn default() -> Self {
        PlayerIcon {
            frames: vec![Canvas::new(ICON_SIZE)],
            frame_ms: DEFAULT_SPRITE_FRAME_MS,
        }
    }
}

/// A legacy palette index as a `SPRITE_PALETTE` index: 0 stays transparent,
/// `n` becomes the nearest sprite colour to the old colour `n - 1`.
fn legacy_index(old: u8) -> u8 {
    match old {
        0 => 0,
        n => {
            let (r, g, b) = LEGACY_ICON_PALETTE[n as usize - 1];
            quantise((r, g, b, 255))
        }
    }
}

impl PlayerIcon {
    /// `None` when `frames` is empty or longer than `MAX_SPRITE_FRAMES`, a
    /// canvas is not `ICON_SIZE` square, a cell is past `SPRITE_PALETTE`, or
    /// `frame_ms` is 0 (a frame that never shows).
    pub fn new(frames: Vec<Canvas>, frame_ms: u32) -> Option<PlayerIcon> {
        if frames.is_empty() || frames.len() > MAX_SPRITE_FRAMES || frame_ms == 0 {
            return None;
        }
        for canvas in &frames {
            if canvas.edge() != ICON_SIZE {
                return None;
            }
            let in_palette = (0..ICON_SIZE)
                .all(|y| (0..ICON_SIZE).all(|x| canvas.get(x, y) as usize <= SPRITE_PALETTE.len()));
            if !in_palette {
                return None;
            }
        }
        Some(PlayerIcon { frames, frame_ms })
    }

    pub fn frames(&self) -> &[Canvas] {
        &self.frames
    }

    pub fn frame_ms(&self) -> u32 {
        self.frame_ms
    }

    /// Whether every frame is transparent — a player who never opened the
    /// editor, or who cleared it.
    pub fn is_blank(&self) -> bool {
        self.frames.iter().all(Canvas::is_blank)
    }

    /// The cell at `(x, y)` of `frame` as RGBA. Panics if `frame` is out of
    /// range, like slice indexing.
    pub fn rgba(&self, frame: usize, x: usize, y: usize) -> (u8, u8, u8, u8) {
        sprite_rgba(self.frames[frame].get(x, y))
    }

    /// `v3:<frame_ms>:<frame>[:<frame>]`, each frame 512 lowercase hex
    /// digits, row-major, two per cell. `decode`'s exact inverse.
    pub fn encode(&self) -> String {
        let mut s = format!("{V3_PREFIX}:{}", self.frame_ms);
        for canvas in &self.frames {
            s.push(':');
            for y in 0..ICON_SIZE {
                for x in 0..ICON_SIZE {
                    s.push_str(&format!("{:02x}", canvas.get(x, y)));
                }
            }
        }
        s
    }

    /// The inverse of `encode`, plus the two retired formats.
    ///
    /// Strict and total: anything malformed returns `None` rather than
    /// being clamped or partly recovered — the caller falls back to the
    /// glyph, which is already correct on its own. A `v1`/`v2` payload
    /// decodes to an ordinary one-frame icon and re-saves as `v3`.
    pub fn decode(s: &str) -> Option<PlayerIcon> {
        let (prefix, rest) = s.split_once(':')?;
        match prefix {
            V3_PREFIX => Self::decode_v3(rest),
            V2_PREFIX => Self::decode_v2(rest),
            V1_PREFIX => Self::decode_v1(rest),
            _ => None,
        }
    }

    fn decode_v3(rest: &str) -> Option<PlayerIcon> {
        let (pace, frames) = rest.split_once(':')?;
        let frame_ms = pace.parse::<u32>().ok()?;
        let frames = frames
            .split(':')
            .map(Self::decode_v3_frame)
            .collect::<Option<Vec<_>>>()?;
        PlayerIcon::new(frames, frame_ms)
    }

    fn decode_v3_frame(digits: &str) -> Option<Canvas> {
        if digits.len() != ICON_CELLS * 2 {
            return None;
        }
        let nibbles = digits
            .chars()
            .map(|c| c.to_digit(16).map(|d| d as u8))
            .collect::<Option<Vec<_>>>()?;
        let mut canvas = Canvas::new(ICON_SIZE);
        for (i, pair) in nibbles.chunks_exact(2).enumerate() {
            canvas.set(i % ICON_SIZE, i / ICON_SIZE, pair[0] * 16 + pair[1]);
        }
        Some(canvas)
    }

    /// A `v2` cell is a 2x2 block of the sprite.
    fn decode_v2(digits: &str) -> Option<PlayerIcon> {
        if digits.len() != V2_GRID * V2_GRID {
            return None;
        }
        let scale = ICON_SIZE / V2_GRID;
        let mut canvas = Canvas::new(ICON_SIZE);
        for (i, c) in digits.chars().enumerate() {
            let index = legacy_index(c.to_digit(16)? as u8);
            let (cx, cy) = (i % V2_GRID, i / V2_GRID);
            for dy in 0..scale {
                for dx in 0..scale {
                    canvas.set(cx * scale + dx, cy * scale + dy, index);
                }
            }
        }
        PlayerIcon::new(vec![canvas], DEFAULT_SPRITE_FRAME_MS)
    }

    /// A `v1` pixel keeps its place; only its colour is remapped.
    fn decode_v1(digits: &str) -> Option<PlayerIcon> {
        if digits.len() != ICON_CELLS {
            return None;
        }
        let mut canvas = Canvas::new(ICON_SIZE);
        for (i, c) in digits.chars().enumerate() {
            canvas.set(
                i % ICON_SIZE,
                i / ICON_SIZE,
                legacy_index(c.to_digit(16)? as u8),
            );
        }
        PlayerIcon::new(vec![canvas], DEFAULT_SPRITE_FRAME_MS)
    }
}

#[cfg(test)]
mod canvas_tests {
    use super::*;

    #[test]
    fn a_fresh_canvas_is_blank_and_reads_zero_everywhere() {
        let canvas = Canvas::new(16);
        assert!(canvas.is_blank());
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(canvas.get(x, y), 0, "({x}, {y})");
            }
        }
    }

    #[test]
    fn set_then_get_round_trips() {
        let mut canvas = Canvas::new(16);
        canvas.set(3, 5, 9);
        assert_eq!(canvas.get(3, 5), 9);
    }

    #[test]
    fn an_off_grid_set_is_dropped_and_an_off_grid_get_is_zero() {
        let mut canvas = Canvas::new(16);
        canvas.set(16, 0, 9);
        canvas.set(0, 16, 9);
        assert_eq!(canvas.get(16, 0), 0);
        assert_eq!(canvas.get(0, 16), 0);
        assert!(
            canvas.is_blank(),
            "the dropped writes must not have landed anywhere"
        );
    }

    #[test]
    fn clear_blanks_a_painted_canvas() {
        let mut canvas = Canvas::new(16);
        canvas.set(0, 0, 1);
        canvas.set(15, 15, 7);
        canvas.clear();
        assert!(canvas.is_blank());
        assert_eq!(canvas.get(0, 0), 0);
        assert_eq!(canvas.get(15, 15), 0);
    }

    #[test]
    fn two_canvases_of_the_same_edge_and_cells_are_equal() {
        let mut a = Canvas::new(16);
        let mut b = Canvas::new(16);
        a.set(2, 2, 4);
        b.set(2, 2, 4);
        assert_eq!(a, b);
        b.set(2, 2, 5);
        assert_ne!(a, b);
    }

    #[test]
    fn edge_reports_the_grid_it_was_built_with() {
        let canvas = Canvas::new(16);
        assert_eq!(canvas.edge(), 16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted(frames: &[&[(usize, usize, u8)]], frame_ms: u32) -> PlayerIcon {
        let canvases = frames
            .iter()
            .map(|cells| {
                let mut canvas = Canvas::new(ICON_SIZE);
                for &(x, y, index) in *cells {
                    canvas.set(x, y, index);
                }
                canvas
            })
            .collect();
        PlayerIcon::new(canvases, frame_ms).unwrap()
    }

    #[test]
    fn a_two_frame_icon_round_trips_through_encode_and_decode_unchanged() {
        let icon = painted(&[&[(0, 0, 1), (15, 15, 63)], &[(7, 7, 30)]], 450);
        let s = icon.encode();
        assert!(s.starts_with("v3:450:"));
        assert_eq!(s.len(), "v3:450:".len() + 512 + 1 + 512);
        assert_eq!(PlayerIcon::decode(&s), Some(icon));
    }

    #[test]
    fn a_default_icon_encodes_to_one_frame_of_512_zeros_at_the_default_pace() {
        assert_eq!(
            PlayerIcon::default().encode(),
            format!("v3:{DEFAULT_SPRITE_FRAME_MS}:{}", "0".repeat(512))
        );
    }

    #[test]
    fn decode_refuses_every_malformed_shape() {
        let frame = "0".repeat(512);
        for bad in [
            format!("v4:600:{frame}"),                 // unknown future prefix
            "v3:600:".to_string(),                     // no frame
            format!("v3:600:{frame}:{frame}:{frame}"), // three frames
            format!("v3:600:{}", "0".repeat(511)),
            format!("v3:600:{}", "0".repeat(513)),
            format!("v3:600:{}g{}", "0".repeat(255), "0".repeat(256)),
            format!("v3:600:{}40{}", "0".repeat(254), "0".repeat(256)), // 0x40 = 64 > 63
            format!("v3:0:{frame}"),
            format!("v3:-5:{frame}"),
            format!("v3:six:{frame}"),
            format!("v3::{frame}"),
            format!("v2:{}", "0".repeat(63)),
            format!("v2:{}", "0".repeat(65)),
            format!("v1:{}", "0".repeat(255)),
            format!("v1:{}g{}", "0".repeat(100), "0".repeat(155)),
        ] {
            assert_eq!(PlayerIcon::decode(&bad), None, "{bad:.40}");
        }
    }

    #[test]
    fn any_positive_pace_decodes_and_the_top_palette_index_round_trips() {
        let icon = painted(&[&[(3, 3, 63)]], u32::MAX);
        assert_eq!(PlayerIcon::decode(&icon.encode()), Some(icon));
    }

    #[test]
    fn new_refuses_what_decode_refuses() {
        let c = Canvas::new(ICON_SIZE);
        assert!(PlayerIcon::new(vec![], 600).is_none());
        assert!(PlayerIcon::new(vec![c.clone(); 3], 600).is_none());
        assert!(PlayerIcon::new(vec![Canvas::new(8)], 600).is_none());
        assert!(PlayerIcon::new(vec![c.clone()], 0).is_none());
        let mut over = c.clone();
        over.set(0, 0, 64);
        assert!(PlayerIcon::new(vec![over], 600).is_none());
        assert!(PlayerIcon::new(vec![c], 600).is_some());
    }

    #[test]
    fn a_v2_cell_becomes_a_2x2_block_in_the_remapped_colour() {
        let mut digits = "0".repeat(64).into_bytes();
        digits[8 + 2] = b'f'; // cell (2, 1), old index 15
        let icon =
            PlayerIcon::decode(&format!("v2:{}", String::from_utf8(digits).unwrap())).unwrap();
        assert_eq!(icon.frames().len(), 1);
        assert_eq!(icon.frame_ms(), DEFAULT_SPRITE_FRAME_MS);
        let want = legacy_index(15);
        assert!((1..=63).contains(&want));
        for (x, y) in [(4, 2), (5, 2), (4, 3), (5, 3)] {
            assert_eq!(icon.frames()[0].get(x, y), want, "({x},{y})");
        }
        assert_eq!(icon.frames()[0].get(6, 2), 0);
        assert!(icon.encode().starts_with("v3:"));
    }

    #[test]
    fn a_v1_pixel_keeps_its_place_and_takes_the_remapped_colour() {
        let mut digits = "0".repeat(256).into_bytes();
        digits[16 * 9 + 13] = b'3';
        let icon =
            PlayerIcon::decode(&format!("v1:{}", String::from_utf8(digits).unwrap())).unwrap();
        assert_eq!(icon.frames()[0].get(13, 9), legacy_index(3));
        assert_eq!(icon.frames()[0].get(12, 9), 0);
    }

    #[test]
    fn every_legacy_index_remaps_inside_the_sprite_palette() {
        assert_eq!(legacy_index(0), 0);
        for n in 1..=15u8 {
            assert!(
                (1..=SPRITE_PALETTE.len() as u8).contains(&legacy_index(n)),
                "{n}"
            );
        }
    }

    #[test]
    fn rgba_is_transparent_at_index_zero_and_the_palette_colour_otherwise() {
        let icon = painted(&[&[(1, 1, 5)]], 600);
        assert_eq!(icon.rgba(0, 0, 0).3, 0);
        let (r, g, b) = SPRITE_PALETTE[4];
        assert_eq!(icon.rgba(0, 1, 1), (r, g, b, 255));
    }

    #[test]
    fn is_blank_needs_every_frame_blank() {
        assert!(PlayerIcon::default().is_blank());
        assert!(!painted(&[&[], &[(0, 0, 1)]], 600).is_blank());
    }
}
