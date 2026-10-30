use super::*;

/// A pending build site's slab and its edge.
///
/// **Grey, and deliberately colourless.** Every other channel on this map is
/// spoken for by a hue that means something — a biome's passability, a
/// machine's status outline, the plan's amber, the mark's green — and a site
/// is the one cell whose whole message is *nothing is here yet*. It reads as
/// an absence because it is one. The orange caret bouncing on top is what
/// carries "and somebody is working on it", so the slab does not have to.
///
/// The edge is darker than the fill rather than lighter, which is `rock`'s
/// `SHADE_BAND` rule in another shape: a bright rim would read as a
/// finished, lit structure. It is opaque so the ground beneath does not show
/// through and make the cell look half-drawn.
///
/// **The two greys stay outside `hud::palette`**, alone among the map's
/// overlays. The palette is addressed by role and has no word for
/// "unfinished construction"; the nearest entries are a pane border and a
/// bar trough, and reaching for one of those would be addressing a chrome
/// grey by its value, which is the one thing that file forbids.
pub(super) const BUILD_SITE_FILL: Color = Color::new(0.30, 0.30, 0.32, 1.0);
pub(super) const BUILD_SITE_EDGE: Color = Color::new(0.16, 0.16, 0.18, 1.0);
pub(super) const BUILD_SITE_EDGE_PX: f32 = 2.0;

/// A multi-cell structure's walkable non-anchor floor cells — dark
/// yellow/brown, and outside `hud::palette` for `BUILD_SITE_FILL`'s own
/// reason: the palette is addressed by role and has no word for "a
/// machine's own floor".
///
/// **Drawn with no biome pattern and no outline.** `marks::outline_open`
/// already owns the tile-edge ring, drawing a machine's walls as 2px lines
/// flush at the tile's edges, and the existing corner marks carry insets
/// specifically so nothing reads as painting one of those absent lines back
/// in — keeping this fill flat and unringed is what leaves the ring
/// available for the pin marks (`marks::pin_mark_rects`) instead.
pub(super) const STRUCTURE_FLOOR_FILL: Color = Color::new(0.30, 0.23, 0.09, 1.0);

/// The Excavation plan's three washes, all one hue so a plan reads as one
/// thing. A committed mark's fill is dim enough to walk over without the
/// base becoming unreadable and its edge carries the shape; the box being
/// previewed is brighter than either, because it is the thing about to
/// happen and has to read over a mark it may be drawn across.
///
/// The hue is `palette::PLAN`, and the three are built off it rather than
/// written out, so the alpha is the only thing that varies — three literals
/// is three chances for a plan to stop being one colour.
pub(super) const fn wash(alpha: f32) -> Color {
    Color::new(
        hud::palette::PLAN.r,
        hud::palette::PLAN.g,
        hud::palette::PLAN.b,
        alpha,
    )
}
pub(super) const MARK_FILL: Color = wash(0.18);
pub(super) const MARK_EDGE: Color = wash(0.45);
pub(super) const PREVIEW_FILL: Color = wash(0.35);
/// A stockpile zone's wash, off `palette::STOCKPILE` at the same strength as
/// a committed dig mark's, so a cell that is both reads as both.
pub(super) const STOCKPILE_FILL: Color = Color::new(
    hud::palette::STOCKPILE.r,
    hud::palette::STOCKPILE.g,
    hud::palette::STOCKPILE.b,
    0.18,
);

/// How strongly an Alt-held room tint covers the ground. Low enough that the
/// floor finish and the biome pattern still read through it.
pub(super) const ROOM_TINT_ALPHA: f32 = 0.28;

/// What a floor item draws as: an item in its own glyph, in the amber of
/// cargo; a downed program in the gray `x` the battle map gives a fallen
/// body, so the two boards say "disabled" the same way.
pub(crate) fn floor_item_mark(look: &FloorLook) -> (char, Color) {
    match look {
        FloorLook::Item(glyph) => (*glyph, hud::palette::WARN),
        FloorLook::Program => super::tactical::FALLEN_MARK,
    }
}

/// The ring the party's own tile wears while cutting tools are armed.
///
/// The plan's hue at full alpha, because a mark and a swing are the same
/// job: what the tools are out *for* is the rock a plan is drawn on. Full
/// alpha rather than the washes above since this is a mode and not a thing
/// drawn on the ground — it has to read over whatever the party is standing
/// on, a marked cell included.
pub(crate) const CUTTING_OUTLINE: Color = hud::palette::PLAN;

/// Where a tile's con read lands — on the glyph's own hue, or in a corner
/// earmark, **never both and never neither**.
///
/// The glyph is the better home by a distance: it is the ink the eye lands
/// on when scanning a screenful, and it costs only the species' authored
/// hue, which answers *what is this* — a question asked on a tile the
/// player has already stopped at. Two tiles cannot spend it, and the
/// earmark is what they pay instead. One whose ink is a **sprite**, because
/// art is authored near-white and egui *multiplies* this colour through it,
/// so a con rung would repaint the drawing rather than tint it. And a
/// **boss**, whose magenta is the ink.
///
/// One value rather than two conditions agreeing at two draw sites. A tile
/// wearing a rung on its glyph *and* in its corner reads as two different
/// creatures; a tile wearing it in neither reads as harmless. Both are
/// silent, and neither would fail to compile.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ConRead {
    /// The glyph carries it and the corner stays bare.
    Glyph(GlyphColor),
    /// The glyph's hue is spoken for, so the corner carries it.
    Earmark(GlyphColor),
    /// Not a hostile — *no reading*, which is not the same as a reading
    /// worth nothing.
    None,
}

impl ConRead {
    /// `drew_sprite` is the sprite call's **own answer** and never
    /// `sprite.is_some()`. A name the table has nothing under falls back to
    /// the glyph — that is what keeps `assets/sprites/` optional — and that
    /// glyph is free to carry the rung. Gated on the name instead, an
    /// unloaded sprite costs a tile the better of the two homes for a
    /// reason the player cannot see.
    pub(crate) fn of(difficulty: Option<GlyphColor>, is_boss: bool, drew_sprite: bool) -> Self {
        match difficulty {
            Some(rung) if is_boss || drew_sprite => Self::Earmark(rung),
            Some(rung) => Self::Glyph(rung),
            None => Self::None,
        }
    }

    /// The ink the glyph takes: the rung when the glyph is carrying the
    /// read, and the tile's authored colour otherwise.
    ///
    /// Takes `vig` because it is applied *after* the tile has already been
    /// vignetted — a rung mixed in here would otherwise burn at full
    /// brightness at the edge of the light, which is the one place the
    /// player is squinting anyway.
    pub(crate) fn glyph_ink(self, authored: Color, vig: f32) -> Color {
        match self {
            Self::Glyph(rung) => {
                let c = glyph_color(rung);
                Color::new(c.r * vig, c.g * vig, c.b * vig, authored.a)
            }
            _ => authored,
        }
    }

    /// The rung the corner draws, if the corner is the one carrying it.
    pub(crate) fn earmark(self) -> Option<GlyphColor> {
        match self {
            Self::Earmark(rung) => Some(rung),
            _ => None,
        }
    }
}

/// What the top-left corner draws this frame — the con earmark, or the Alt
/// marker in its place while `reveal` is held over an `unseen_routine`
/// tile (spec §4 "The Alt marker"). A free function for `ConRead::of`'s own
/// reason: the tile loop is too big to reach with a test, and this is the
/// whole of the gate a hand-written copy in that loop could get wrong.
///
/// Returns `(earmark, marker)` rather than one combined enum because the
/// two draw through different calls (`draw_difficulty_mark` takes an
/// `Option<GlyphColor>`, `draw_unseen_marker` takes nothing but position) —
/// a third shape here would be a translation step between this and both of
/// them.
pub(crate) fn corner_marker(
    con: ConRead,
    unseen_routine: bool,
    reveal: bool,
) -> (Option<GlyphColor>, bool) {
    if reveal && unseen_routine {
        (None, true)
    } else {
        (con.earmark(), false)
    }
}

/// How bright a biome's pattern is against its own ground fill, and how
/// bright the rim along the edge of the walkable world is against both.
/// The ground stays at `GROUND_LEVEL` so terrain never competes with the
/// entity glyphs standing on it — the whole point of keeping glyphs for
/// actors is that they win.
pub(super) const GROUND_LEVEL: f32 = 0.18;

/// How many tiles beyond the visible pane `draw_surface_map` fetches. One for
/// the camera to slide in from, one so that tile has neighbours to compare
/// biomes against. See the call site for why both are needed.
pub(crate) const RINGS: i32 = 2;
