use super::*;

/// The mark of a disabled body, on the battle map and, for a downed program
/// lying on the floor, the base map: one constant so the two cannot drift.
pub(crate) const FALLEN_MARK: (char, Color) = ('x', palette::FALLEN);

/// The ground, by kind. Brightness and nothing else carries passability,
/// `RockDb`'s rule from base space: hue is spoken for by what is standing
/// on a cell, and a kind may not spend it.
pub(super) fn cell_color(kind: BattleCell) -> Color {
    match kind {
        BattleCell::Open => Color::new(0.13, 0.15, 0.18, 1.0),
        BattleCell::Rough => Color::new(0.19, 0.18, 0.14, 1.0),
        BattleCell::Cover => Color::new(0.30, 0.31, 0.34, 1.0),
        BattleCell::Blocked => Color::new(0.05, 0.06, 0.08, 1.0),
    }
}

/// A ground cell's sprite keys in the order they are tried: the biome's own
/// tile, then the kind's default. One word a biome, so a key has exactly two
/// underscores and a modder can read the name back into its parts.
pub(super) fn ground_sprite_keys(biome: Biome, kind: BattleCell) -> [String; 2] {
    let kind = match kind {
        BattleCell::Open => "open",
        BattleCell::Rough => "rough",
        BattleCell::Cover => "cover",
        BattleCell::Blocked => "blocked",
    };
    let biome = match biome {
        Biome::DataVoid => "datavoid",
        Biome::Deadlock => "deadlock",
        Biome::NullSector => "nullsector",
        Biome::Backplane => "backplane",
        Biome::OpenGrid => "opengrid",
        Biome::BlackIce => "blackice",
        Biome::Platform => "platform",
        Biome::Excavated => "excavated",
        Biome::Entropy => "entropy",
    };
    [format!("ground_{biome}_{kind}"), format!("ground_{kind}")]
}

/// How bright near-white ground art draws, by kind — `cell_color`'s order,
/// lifted so the art's pattern shows through.
pub(super) fn ground_level(kind: BattleCell) -> f32 {
    match kind {
        BattleCell::Blocked => 0.14,
        BattleCell::Open => 0.22,
        BattleCell::Rough => 0.27,
        BattleCell::Cover => 0.42,
    }
}

/// The tint ground art draws with: the kind's brightness times the biome's
/// muted cast, under the cell's light — which is clamped narrow enough that
/// the kind still reads by brightness (`lighting`'s own doc).
pub(super) fn ground_tint(biome: Biome, kind: BattleCell, lit: Lit) -> Color {
    let level = ground_level(kind);
    let cast = palette::ground_cast(biome);
    lit.apply(Color::new(
        cast.r * level,
        cast.g * level,
        cast.b * level,
        1.0,
    ))
}

/// One cell's ground: the flat fill, then the first ground art that loads
/// over it. The fill is both the look with no art and the backdrop under a
/// transparent pixel.
pub(super) fn draw_ground(
    painter: &Painter,
    biome: Biome,
    kind: BattleCell,
    lit: Lit,
    px: f32,
    py: f32,
    size: f32,
) {
    painter.rect(px, py, size, size, lit.apply(cell_color(kind)));
    let tint = ground_tint(biome, kind, lit);
    for key in ground_sprite_keys(biome, kind) {
        if painter.sprite(&key, px, py, size, tint, 0) {
            break;
        }
    }
}

/// How bright a prop's art draws, against `ground_level`'s 0.22 to 0.42: a
/// set piece has to stand clear of the floor it is on, and brightness is all
/// it spends, so the cast is the biome's muted one.
pub(super) const PROP_LEVEL: f32 = 0.72;

/// Decoration draws dimmer than cover so it reads as background — scorch and
/// cable are things on the floor, not things in the way.
pub(super) const DECORATION_LEVEL: f32 = 0.5;

/// The sprite keys a prop tries, first hit wins: the cracked art once less
/// than half its hit points remain, then the whole-piece art.
pub(super) fn prop_sprite_keys(prop: &PropView) -> Vec<String> {
    let mut keys = Vec::new();
    if let (Some(hp), Some(max)) = (prop.hp, prop.max_hp)
        && hp * 2 < max
    {
        keys.push(format!("{}_cracked", prop.sprite));
    }
    keys.push(prop.sprite.clone());
    keys
}

/// One glyph per class when no art loads: decoration, volatile, breakable,
/// and the wall nothing breaks.
pub(super) fn prop_fallback_glyph(prop: &PropView) -> &'static str {
    if prop.decoration {
        ","
    } else if prop.volatile {
        "*"
    } else if prop.hp.is_some() {
        "%"
    } else {
        "#"
    }
}

/// One prop over its cell's ground, before any body. Art or glyph, never
/// both: a sprite substitutes for the glyph.
pub(super) fn draw_prop(
    painter: &Painter,
    biome: Biome,
    lit: Lit,
    prop: &PropView,
    (px, py): (f32, f32),
    size: f32,
    glyph_px: u16,
) {
    let level = if prop.decoration {
        DECORATION_LEVEL
    } else {
        PROP_LEVEL
    };
    let cast = palette::ground_cast(biome);
    let tint = lit.apply(Color::new(
        cast.r * level,
        cast.g * level,
        cast.b * level,
        1.0,
    ));
    if prop_sprite_keys(prop)
        .iter()
        .any(|key| painter.sprite(key, px, py, size, tint, 0))
    {
        return;
    }
    let glyph = prop_fallback_glyph(prop);
    let dims = painter.measure_map(glyph, glyph_px);
    painter.map(
        glyph,
        px + (size - dims.width) / 2.0,
        py + (size + dims.height) / 2.0,
        glyph_px,
        tint,
    );
}

/// How far into a tile a body's art is inset, matching the surface map's own
/// centring of a glyph in its cell.
pub(super) fn sprite_inset(tile_px: f32, glyph_px: u16) -> f32 {
    (tile_px - glyph_px as f32) / 2.0
}
