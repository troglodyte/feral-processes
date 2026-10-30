use super::*;

const ALL_BIOMES: [Biome; 9] = [
    Biome::DataVoid,
    Biome::Deadlock,
    Biome::NullSector,
    Biome::Backplane,
    Biome::OpenGrid,
    Biome::BlackIce,
    Biome::Platform,
    Biome::Excavated,
    Biome::Entropy,
];
const WALKABLE_BIOMES: [Biome; 6] = [
    Biome::Deadlock,
    Biome::NullSector,
    Biome::Backplane,
    Biome::OpenGrid,
    Biome::Platform,
    Biome::Excavated,
];
const ALL_KINDS: [BattleCell; 4] = [
    BattleCell::Open,
    BattleCell::Rough,
    BattleCell::Cover,
    BattleCell::Blocked,
];
/// The most a biome's cast may spread its channels — well under any
/// wash's own hue, so a cast is never read as a wash.
const MAX_BIOME_SATURATION: f32 = 0.12;

fn luminance(c: Color) -> f32 {
    0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
}

#[test]
fn a_biomes_ground_tile_is_tried_before_the_kinds_default() {
    for biome in ALL_BIOMES {
        for kind in ALL_KINDS {
            let [first, second] = ground_sprite_keys(biome, kind);
            assert!(first.starts_with("ground_"), "{first}");
            assert_eq!(first.matches('_').count(), 2, "one word a biome: {first}");
            assert!(
                first.ends_with(&second["ground".len()..]),
                "{first} / {second}"
            );
            assert_eq!(second.matches('_').count(), 1, "{second}");
        }
    }
    assert_eq!(
        ground_sprite_keys(Biome::Backplane, BattleCell::Cover),
        [
            "ground_backplane_cover".to_string(),
            "ground_cover".to_string()
        ]
    );
}

#[test]
fn ground_tint_keeps_the_kinds_brightness_order_in_every_walkable_biome() {
    for biome in WALKABLE_BIOMES {
        let [blocked, open, rough, cover] = [
            BattleCell::Blocked,
            BattleCell::Open,
            BattleCell::Rough,
            BattleCell::Cover,
        ]
        .map(|k| luminance(ground_tint(biome, k, Lit::NONE)));
        assert!(
            blocked < open && open < rough && rough < cover,
            "{biome:?}: {blocked} {open} {rough} {cover}"
        );
    }
}

/// The user's call on lighting against `cell_color`'s rule: whatever
/// the light, blocked stays the darkest ground and cover the brightest.
/// Open and rough may cross in deep shadow — their art differs.
#[test]
fn no_light_lets_blocked_or_cover_cross_another_kind() {
    use super::super::super::lighting::{LEVEL_MAX, LEVEL_MIN};
    for biome in WALKABLE_BIOMES {
        for cast in palette::LIGHT_CASTS {
            let lit = |kind, level| Lit { level, cast }.apply(cell_color(kind));
            let art = |kind, level| ground_tint(biome, kind, Lit { level, cast });
            for draw in [&lit as &dyn Fn(BattleCell, f32) -> Color, &art] {
                let bright = |k| luminance(draw(k, LEVEL_MAX));
                let dim = |k| luminance(draw(k, LEVEL_MIN));
                assert!(
                    bright(BattleCell::Blocked) < dim(BattleCell::Open),
                    "{biome:?} {cast:?}"
                );
                assert!(
                    bright(BattleCell::Rough) < dim(BattleCell::Cover),
                    "{biome:?} {cast:?}"
                );
            }
        }
    }
}

#[test]
fn no_biome_cast_is_saturated_enough_to_read_as_a_wash() {
    for biome in ALL_BIOMES {
        let t = palette::ground_cast(biome);
        let spread = t.r.max(t.g).max(t.b) - t.r.min(t.g).min(t.b);
        assert!(spread < MAX_BIOME_SATURATION, "{biome:?}: {spread}");
    }
}

/// The textures each ground draw puts on screen, given which keys have
/// art loaded.
fn ground_drawn(keys: &[(&str, u64)]) -> (Vec<bevy_egui::egui::TextureId>, usize) {
    let mut table = crate::paint::SpriteTable::default();
    for &(key, id) in keys {
        table.insert_still(key, bevy_egui::egui::TextureId::User(id));
    }
    let (_, shapes) = crate::paint::with_sprites(table, |p| {
        draw_ground(
            p,
            Biome::Backplane,
            BattleCell::Cover,
            Lit::NONE,
            10.0,
            10.0,
            15.0,
        )
    });
    let images = crate::paint::painted_images(&shapes)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    (
        images,
        painted_rect_fill_count(&shapes, cell_color(BattleCell::Cover)),
    )
}

#[test]
fn a_ground_cell_draws_its_biomes_tile_over_the_fill() {
    let (images, fills) = ground_drawn(&[("ground_backplane_cover", 1), ("ground_cover", 2)]);
    assert_eq!(images, vec![bevy_egui::egui::TextureId::User(1)]);
    assert_eq!(fills, 1, "the flat fill stays under the art");
}

/// The fill is opaque, so art painted before it would be hidden and the
/// board would read as flat squares again.
#[test]
fn a_ground_cells_art_is_painted_after_its_fill() {
    let mut table = crate::paint::SpriteTable::default();
    table.insert_still("ground_cover", bevy_egui::egui::TextureId::User(2));
    let (_, shapes) = crate::paint::with_sprites(table, |p| {
        draw_ground(
            p,
            Biome::Backplane,
            BattleCell::Cover,
            Lit::NONE,
            10.0,
            10.0,
            15.0,
        )
    });
    assert!(
        matches!(
            shapes.last().map(|s| &s.shape),
            Some(bevy_egui::egui::Shape::Mesh(_))
        ),
        "the art must be the last thing drawn"
    );
}

#[test]
fn a_ground_cell_without_a_biome_tile_draws_the_kinds_default() {
    let (images, _) = ground_drawn(&[("ground_cover", 2)]);
    assert_eq!(images, vec![bevy_egui::egui::TextureId::User(2)]);
}

#[test]
fn a_ground_cell_with_no_art_is_the_flat_fill_alone() {
    let (images, fills) = ground_drawn(&[]);
    assert!(images.is_empty(), "{images:?}");
    assert_eq!(fills, 1);
}
