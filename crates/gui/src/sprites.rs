//! Loading the map's one-cell sprites.
//!
//! Sprites live in `assets/sprites/` as 16x16 RGBA PNGs — see that
//! directory's `README.md` for why the size is not negotiable and why they
//! are authored near-white. This module gets them onto the GPU and hands
//! the renderer a `SpriteTable` of `egui::TextureId`s to draw with.
//!
//! The whole thing is optional by construction. A missing directory, a
//! missing file, or a file that fails to decode all end the same way: the
//! name is absent from the table, `Painter::sprite` reports that it drew
//! nothing, and the caller falls back to the entity's glyph. Deleting
//! `assets/sprites/` therefore restores the glyph map exactly, the same
//! supported way deleting `assets/sectors/` restores undifferentiated
//! zones.

use std::sync::Arc;

use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use feral_processes_engine::{ICON_SIZE, PlayerIcon};

use crate::paint::SpriteTable;

/// The sprites the map looks for, by the name the renderer asks for.
///
/// A list rather than a directory walk: the renderer asks for a *name*, so
/// something has to say which names exist, and a constant here is one line
/// per sprite against a filesystem scan that would have to run before the
/// asset server is available anyway. This is the minimum proof — when
/// sprites become a `sprite:` field on species and structures, the names
/// come from the asset files and this list goes away.
const SPRITES: &[&str] = &["player", "anchor"];

/// The `SpriteTable` key the player's own drawing is registered under.
///
/// The `@` is what keeps it unreachable from anywhere else: `load` builds
/// every other key from a filename, and a future `sprite:` field on a
/// species would too, so no file and no mod can claim the slot the player
/// drew for themselves.
pub const DRAWN_ICON_KEY: &str = "@drawn";

/// Where the loaded sprites live between the asset server and the renderer.
#[derive(Resource, Default)]
pub struct Sprites {
    /// Still loading, or waiting to be handed to egui.
    pending: Vec<(&'static str, Handle<Image>)>,
    /// What the renderer draws from. Refcounted so the per-frame `Painter`
    /// costs one atomic bump rather than a copy.
    table: Arc<SpriteTable>,
    /// The drawing currently on the GPU, and the handle it was uploaded
    /// under. The icon is kept beside the handle so `sync_drawn_icon` can
    /// answer "has this changed?" by value, and the handle is kept so the
    /// old registration can be taken back before a new one replaces it.
    drawn: Option<(PlayerIcon, Handle<Image>)>,
}

impl Sprites {
    pub fn table(&self) -> Arc<SpriteTable> {
        Arc::clone(&self.table)
    }

    /// Puts the player's drawn icon on the GPU, or takes it back off.
    ///
    /// This is the only texture the game builds at runtime; everything else
    /// in the table came off disk through `load`. It is asked every frame,
    /// so the first thing it does is compare the icon **by value** and do
    /// nothing for an equal one — `PartialEq` over 256 bytes is cheaper
    /// than any dirty flag would be to keep honest, and the alternative is
    /// a texture minted per frame and none ever freed. When it does change,
    /// the previous registration is taken back *before* the new one is
    /// added, for that same reason: `add_image` was handed a strong handle,
    /// so `EguiUserTextures` is what keeps the old image alive.
    ///
    /// A **blank canvas is not a drawing.** The editor lets a player keep
    /// one, and 256 transparent pixels drawn in place of the `@` is a
    /// player with no tile at all — `Painter::sprite` would report that it
    /// drew, so the glyph fallback never runs. Filtered here, in the one
    /// place the key is minted, rather than at each of the two draw sites.
    ///
    /// `ImageSampler::nearest()` is `load`'s reason exactly: bevy_egui
    /// binds the image's *own* sampler and bevy's default is linear, so
    /// without this line the drawing is filtered bilinearly at every zoom
    /// above 1 and reads as mush.
    pub fn sync_drawn_icon(
        &mut self,
        icon: Option<&PlayerIcon>,
        images: &mut Assets<Image>,
        textures: &mut EguiUserTextures,
    ) {
        let icon = icon.filter(|i| !i.is_blank());
        if self.drawn.as_ref().map(|(kept, _)| kept) == icon {
            return;
        }
        if let Some((_, old)) = self.drawn.take() {
            textures.remove_image(&old);
        }
        // `Arc::make_mut` for `register`'s reason: the renderer may be
        // holding a clone of the old table from this frame.
        let table = Arc::make_mut(&mut self.table);
        let Some(icon) = icon else {
            table.remove(DRAWN_ICON_KEY);
            return;
        };
        // Built through `PlayerIcon::rgba` — the one place an index becomes
        // a colour, and the one place index 0 becomes a transparent pixel
        // rather than an opaque black one.
        let mut bytes = Vec::with_capacity(ICON_SIZE * ICON_SIZE * 4);
        for y in 0..ICON_SIZE {
            for x in 0..ICON_SIZE {
                let (r, g, b, a) = icon.rgba(x, y);
                bytes.extend_from_slice(&[r, g, b, a]);
            }
        }
        let mut image = Image::new(
            Extent3d {
                width: ICON_SIZE as u32,
                height: ICON_SIZE as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            bytes,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::nearest();
        let handle = images.add(image);
        let id = textures.add_image(EguiTextureHandle::Strong(handle.clone()));
        table.insert(DRAWN_ICON_KEY, id);
        self.drawn = Some((icon.clone(), handle));
    }
}

/// Asks the asset server for every sprite, filtered nearest-neighbour.
///
/// `nearest` is the whole point: the map draws a 16px sprite at 16, 32, 48
/// or 64px, and bevy_egui binds the image's *own* sampler when it renders a
/// user texture — so this one setting is what decides whether pixel art
/// stays crisp or resamples into mush. Bevy's default is linear.
pub fn load(asset_server: Res<AssetServer>, mut sprites: ResMut<Sprites>) {
    for &name in SPRITES {
        let handle = asset_server
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::nearest();
            })
            .load(format!("sprites/{name}.png"));
        sprites.pending.push((name, handle));
    }
}

/// Hands each sprite to egui once its pixels have actually arrived.
///
/// Registration is deliberately *not* done at load time. `add_image` mints
/// a `TextureId` eagerly, before the image exists, so registering up front
/// would put a name in the table that draws an unbacked quad for the first
/// frames of a run. Gating on `LoadState::Loaded` means the table only ever
/// holds sprites that can be drawn, and the glyph covers the gap at no cost.
///
/// A sprite that fails to load — absent, malformed, not a PNG — is dropped
/// with a warning and never retried, which is the same warn-and-carry-on
/// contract every asset database in the engine has.
pub fn register(
    asset_server: Res<AssetServer>,
    mut sprites: ResMut<Sprites>,
    mut textures: ResMut<EguiUserTextures>,
) {
    if sprites.pending.is_empty() {
        return;
    }
    let mut ready = Vec::new();
    sprites.pending.retain(|(name, handle)| {
        match asset_server.get_load_state(handle) {
            Some(LoadState::Loaded) => {
                ready.push((*name, handle.clone()));
                false
            }
            Some(LoadState::Failed(e)) => {
                warn!("sprite `{name}` did not load, falling back to its glyph: {e}");
                false
            }
            // Still in flight, or the server has not seen it yet.
            _ => true,
        }
    });
    if ready.is_empty() {
        return;
    }
    // `Arc::make_mut` rather than rebuilding: the renderer may be holding a
    // clone of the old table from this frame, and this leaves that one alone.
    let table = Arc::make_mut(&mut sprites.table);
    for (name, handle) in ready {
        let id = textures.add_image(EguiTextureHandle::Strong(handle));
        table.insert(name, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::image::ImageFilterMode;

    /// A drawing with one lit pixel — enough to be non-blank, and it pins
    /// the byte order at a coordinate that is not the origin.
    fn a_drawing() -> PlayerIcon {
        let mut icon = PlayerIcon::default();
        icon.set(2, 1, 6);
        icon
    }

    fn fixtures() -> (Sprites, Assets<Image>, EguiUserTextures) {
        (
            Sprites::default(),
            Assets::<Image>::default(),
            EguiUserTextures::default(),
        )
    }

    /// The drawing reaches the table, at the size and format the sprite
    /// seam is built around, **nearest-sampled**.
    ///
    /// `nearest` is the single load-bearing line of the upload:
    /// `bevy_egui` binds the image's own sampler and bevy's default is
    /// linear, so without it the icon is drawn at 32, 48 or 64px through a
    /// bilinear filter and reads as mush. Nothing on screen says which
    /// filter ran, which is why it is asserted here.
    #[test]
    fn a_drawn_icon_is_uploaded_nearest_sampled() {
        let (mut sprites, mut images, mut textures) = fixtures();

        sprites.sync_drawn_icon(Some(&a_drawing()), &mut images, &mut textures);

        let (_, handle) = sprites.drawn.as_ref().expect("the icon must be kept");
        assert!(
            sprites.table().get(DRAWN_ICON_KEY).is_some(),
            "the drawing must reach the table the renderer draws from"
        );
        let image = images.get(handle).expect("the image must be in the assets");
        assert_eq!(image.width(), ICON_SIZE as u32);
        assert_eq!(image.height(), ICON_SIZE as u32);
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
        let ImageSampler::Descriptor(d) = &image.sampler else {
            panic!("the icon must carry its own sampler, not bevy's linear default");
        };
        assert_eq!(d.mag_filter, ImageFilterMode::Nearest);
        assert_eq!(d.min_filter, ImageFilterMode::Nearest);
    }

    /// The bytes are `PlayerIcon::rgba`'s answer, transparency included.
    ///
    /// Index 0 is transparent rather than a colour, and `rgba` is the one
    /// place that is decided — a second derivation here is what would let
    /// the map draw an opaque black square where the player left the canvas
    /// bare.
    #[test]
    fn the_uploaded_pixels_are_the_icons_own_rgba() {
        let (mut sprites, mut images, mut textures) = fixtures();
        let icon = a_drawing();

        sprites.sync_drawn_icon(Some(&icon), &mut images, &mut textures);

        let (_, handle) = sprites.drawn.as_ref().expect("the icon must be kept");
        let data = images
            .get(handle)
            .expect("the image must be in the assets")
            .data
            .as_ref()
            .expect("the image must carry its pixels");
        assert_eq!(data.len(), ICON_SIZE * ICON_SIZE * 4);
        for y in 0..ICON_SIZE {
            for x in 0..ICON_SIZE {
                let at = (y * ICON_SIZE + x) * 4;
                let (r, g, b, a) = icon.rgba(x, y);
                assert_eq!(
                    (data[at], data[at + 1], data[at + 2], data[at + 3]),
                    (r, g, b, a),
                    "pixel ({x}, {y})"
                );
            }
        }
    }

    /// Redrawing the same icon must not mint a second texture. The map asks
    /// for this every frame, so an upload per frame is an unbounded leak.
    #[test]
    fn an_unchanged_icon_uploads_no_second_texture() {
        let (mut sprites, mut images, mut textures) = fixtures();
        let icon = a_drawing();

        sprites.sync_drawn_icon(Some(&icon), &mut images, &mut textures);
        let first = sprites.drawn.as_ref().expect("kept").1.clone();
        let id = sprites.table().get(DRAWN_ICON_KEY);
        sprites.sync_drawn_icon(Some(&icon), &mut images, &mut textures);

        assert_eq!(images.len(), 1, "an equal icon must upload nothing");
        assert_eq!(sprites.drawn.as_ref().expect("kept").1, first);
        assert_eq!(sprites.table().get(DRAWN_ICON_KEY), id);
    }

    /// A *different* icon replaces the old one, and the old registration is
    /// taken back. Without the removal `EguiUserTextures` keeps a strong
    /// handle to every icon the player ever drew.
    #[test]
    fn a_changed_icon_removes_the_previous_registration() {
        let (mut sprites, mut images, mut textures) = fixtures();

        sprites.sync_drawn_icon(Some(&a_drawing()), &mut images, &mut textures);
        let old = sprites.drawn.as_ref().expect("kept").1.clone();
        let mut second = a_drawing();
        second.set(9, 9, 3);
        sprites.sync_drawn_icon(Some(&second), &mut images, &mut textures);

        assert!(
            textures.image_id(&old).is_none(),
            "the previous registration leaked; egui still holds a texture per redraw"
        );
        assert_eq!(sprites.drawn.as_ref().expect("kept").0, second);
        assert!(sprites.table().get(DRAWN_ICON_KEY).is_some());
    }

    /// Clearing the drawing takes the key back out, so the player's tile
    /// falls through to their named sprite and then their glyph — the two
    /// rungs under this one.
    #[test]
    fn no_icon_leaves_the_key_absent() {
        let (mut sprites, mut images, mut textures) = fixtures();

        sprites.sync_drawn_icon(Some(&a_drawing()), &mut images, &mut textures);
        let old = sprites.drawn.as_ref().expect("kept").1.clone();
        sprites.sync_drawn_icon(None, &mut images, &mut textures);

        assert!(sprites.drawn.is_none());
        assert!(textures.image_id(&old).is_none());
        assert!(
            sprites.table().get(DRAWN_ICON_KEY).is_none(),
            "a cleared drawing must leave nothing for the map to draw"
        );
    }

    /// **A blank canvas is not a drawing.** The editor lets a player keep
    /// one, and 256 transparent pixels drawn in place of the `@` is a
    /// player with no tile at all — `Painter::sprite` would report that it
    /// drew, so the glyph fallback never runs. Filtered here, in the one
    /// place the key is minted, rather than at each of the two draw sites.
    #[test]
    fn a_blank_icon_is_not_uploaded() {
        let (mut sprites, mut images, mut textures) = fixtures();

        sprites.sync_drawn_icon(Some(&PlayerIcon::default()), &mut images, &mut textures);

        assert!(sprites.drawn.is_none());
        assert_eq!(images.len(), 0);
        assert!(sprites.table().get(DRAWN_ICON_KEY).is_none());
    }

    /// Every name the loader asks for must have a file where it asks for it.
    ///
    /// The asset server resolves a missing path asynchronously and reports
    /// it as a load failure several frames later, by which time the only
    /// symptom is a glyph where a sprite was expected — which is also
    /// exactly what a correctly-working fallback looks like. Nothing else
    /// distinguishes "no art yet" from "the path is wrong", so it is
    /// asserted here against the real directory.
    #[test]
    fn every_sprite_the_loader_asks_for_is_on_disk() {
        // The prefix the loader joins onto the asset root, kept beside the
        // `load` call it mirrors rather than spelled twice.
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        assert!(
            !SPRITES.is_empty(),
            "no sprites asked for, so this proved nothing"
        );
        for name in SPRITES {
            let path = root.join(format!("sprites/{name}.png"));
            assert!(
                path.is_file(),
                "the loader asks for `sprites/{name}.png`, which is not at {}",
                path.display()
            );
        }
    }
}
