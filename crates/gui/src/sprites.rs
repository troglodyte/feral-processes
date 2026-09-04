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

/// Every PNG's file stem under `dir`, sorted for a deterministic load order.
///
/// A missing directory is the supported "no sprites shipped" state rather
/// than an error — the same warn-and-carry-on contract every asset database
/// in the engine keeps, and the property this whole module's doc comment
/// promises: deleting `assets/sprites/` must restore the glyph map exactly,
/// so this returns empty rather than panicking. A non-PNG file sitting
/// beside the sprites (a `README.md`, say) is filtered by extension, not
/// asked of the asset server.
///
/// **A stem starting with `@` is filtered out here**, not left to be a
/// filename nobody happens to ship: `@` is a legal filename character on
/// every platform this game ships to, so `@drawn.png` would otherwise scan
/// straight into `DRAWN_ICON_KEY`'s slot. See that constant's doc comment
/// for what a file claiming it would do.
fn scan_sprite_dir(dir: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "png"))
        .filter_map(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .filter(|stem| !stem.starts_with('@'))
        .collect();
    names.sort();
    names
}

/// The `SpriteTable` key the player's own drawing is registered under.
///
/// The `@` is what keeps it unreachable from anywhere else: every other key
/// comes from `scan_sprite_dir`, which throws out any stem starting with
/// `@` for exactly this reason, and a future `sprite:` field on a species
/// would draw from the same scan — so no file and no mod can claim the slot
/// the player drew for themselves.
pub const DRAWN_ICON_KEY: &str = "@drawn";

/// Where the loaded sprites live between the asset server and the renderer.
#[derive(Resource, Default)]
pub struct Sprites {
    /// Still loading, or waiting to be handed to egui. Owned names because
    /// they come off a directory scan rather than a `&'static str` list.
    pending: Vec<(String, Handle<Image>)>,
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
    /// nothing for an equal one — `PartialEq` over 64 bytes is cheaper
    /// than any dirty flag would be to keep honest, and the alternative is
    /// a texture minted per frame and none ever freed. When it does change,
    /// the previous registration is taken back *before* the new one is
    /// added, for that same reason: `add_image` was handed a strong handle,
    /// so `EguiUserTextures` is what keeps the old image alive.
    ///
    /// A **blank canvas is not a drawing.** The editor lets a player keep
    /// one, and an all-transparent icon drawn in place of the `@` is a
    /// player with no tile at all — `Painter::sprite` would report that it
    /// drew, so the glyph fallback never runs. Filtered here, in the one
    /// place the key is minted, rather than at each of the two draw sites.
    ///
    /// **The player draws 8x8 and the sprite stays 16x16**, so each drawn
    /// cell fills an `ICON_CELL_PIXELS` square of the buffer below. Under
    /// nearest sampling that is pixel-identical to a native 8x8 texture,
    /// and it leaves the sprite format — which `assets/sprites/README.md`
    /// says is not negotiable — completely alone.
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
        // Built through `PlayerIcon::pixel_rgba` — the one place a drawn
        // cell becomes its pixel block, over `rgba`, the one place an index
        // becomes a colour and index 0 becomes a transparent pixel rather
        // than an opaque black one.
        let mut bytes = Vec::with_capacity(ICON_SIZE * ICON_SIZE * 4);
        for y in 0..ICON_SIZE {
            for x in 0..ICON_SIZE {
                let (r, g, b, a) = icon.pixel_rgba(x, y);
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

/// Asks the asset server for every sprite on disk, filtered
/// nearest-neighbour.
///
/// `nearest` is the whole point: the map draws a 16px sprite at 16, 32, 48
/// or 64px, and bevy_egui binds the image's *own* sampler when it renders a
/// user texture — so this one setting is what decides whether pixel art
/// stays crisp or resamples into mush. Bevy's default is linear.
///
/// The directory comes from `Frontend.app.assets_dir()` rather than being
/// re-resolved here — it is the same path `asset_plugin` already fed to
/// `AssetPlugin::file_path`, and `crates/launcher/src/paths.rs` is the one
/// place a runtime path gets decided. Reading it a second way here would be
/// a second site free to disagree with it.
pub fn load(
    asset_server: Res<AssetServer>,
    frontend: Res<crate::Frontend>,
    mut sprites: ResMut<Sprites>,
) {
    let dir = frontend.app.assets_dir().join("sprites");
    for name in scan_sprite_dir(&dir) {
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
                ready.push((name.clone(), handle.clone()));
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
    use feral_processes_engine::ICON_CELL_PIXELS;

    /// A drawing with one lit cell — enough to be non-blank, and it pins
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

    /// **Each drawn cell fills its own `ICON_CELL_PIXELS` square of the
    /// texture.** The player edits an 8x8 grid and the sprite stays 16x16,
    /// so the upload is where the two meet — and under nearest sampling a
    /// correctly-expanded block is pixel-identical to a native 8x8 texture.
    /// Asserted on the bytes: a lit cell must be four opaque pixels in a
    /// 2x2 square at twice its coordinates, and its neighbours must be
    /// untouched.
    #[test]
    fn a_drawn_cell_is_uploaded_as_its_whole_pixel_block() {
        let (mut sprites, mut images, mut textures) = fixtures();
        let icon = a_drawing();

        sprites.sync_drawn_icon(Some(&icon), &mut images, &mut textures);

        let (_, handle) = sprites.drawn.as_ref().expect("the icon must be kept");
        let data = images
            .get(handle)
            .expect("the image must be in the assets")
            .data
            .as_ref()
            .expect("the image must carry its pixels")
            .clone();
        assert_eq!(data.len(), ICON_SIZE * ICON_SIZE * 4);

        let pixel = |x: usize, y: usize| {
            let at = (y * ICON_SIZE + x) * 4;
            (data[at], data[at + 1], data[at + 2], data[at + 3])
        };
        // `a_drawing` paints cell (2, 1), so pixels (4..6, 2..4).
        let (cx, cy) = (2, 1);
        let want = icon.rgba(cx, cy);
        assert_eq!(want.3, 255, "the fixture's cell must be opaque");
        for dy in 0..ICON_CELL_PIXELS {
            for dx in 0..ICON_CELL_PIXELS {
                let (x, y) = (cx * ICON_CELL_PIXELS + dx, cy * ICON_CELL_PIXELS + dy);
                assert_eq!(pixel(x, y), want, "pixel ({x}, {y}) is inside the block");
            }
        }
        for (x, y) in [(3, 2), (6, 2), (4, 1), (4, 4)] {
            assert_eq!(
                pixel(x, y),
                (0, 0, 0, 0),
                "pixel ({x}, {y}) is outside the block and must stay bare"
            );
        }
        assert_eq!(
            data.chunks_exact(4).filter(|p| p[3] == 255).count(),
            ICON_CELL_PIXELS * ICON_CELL_PIXELS,
            "one lit cell is exactly one block of opaque pixels"
        );
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
                let (r, g, b, a) = icon.pixel_rgba(x, y);
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
        second.set(5, 5, 3);
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
    /// one, and an all-transparent icon drawn in place of the `@` is a
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

    /// Where the real shipped sprites live, for the tests that scan it
    /// directly rather than through a `Frontend`/`App` the scan function
    /// itself does not depend on.
    fn shipped_sprites_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/sprites")
    }

    /// The table holds every PNG shipped, keyed by its file stem.
    ///
    /// This is the inversion the whole task turns on: a name with no file
    /// behind it is now unreachable, because the scan is the one and only
    /// source of what may be asked for. Names rather than a hardcoded count,
    /// so a third sprite lands in the assertion just by dropping a file in.
    #[test]
    fn the_scan_finds_every_shipped_sprite_by_stem() {
        let names = scan_sprite_dir(&shipped_sprites_dir());
        assert!(
            !names.is_empty(),
            "no sprites shipped, so this proved nothing"
        );
        for entry in std::fs::read_dir(shipped_sprites_dir()).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "png") {
                let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
                assert!(
                    names.contains(&stem),
                    "`{stem}` is a shipped PNG the scan did not find"
                );
            }
        }
    }

    /// Deleting `assets/sprites/` must restore the glyph map exactly — the
    /// property the whole sprite seam rests on. A missing directory is not
    /// an error the scan may propagate.
    #[test]
    fn a_missing_sprite_directory_scans_empty_without_panicking() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("this/directory/does/not/exist");
        assert_eq!(scan_sprite_dir(&dir), Vec::<String>::new());
    }

    /// A non-PNG file sitting in the directory (a `README.md`, an editor
    /// swap file) is never handed to the asset server — the scan filters on
    /// extension, so the loader never even asks for it.
    #[test]
    fn a_non_png_file_in_the_directory_is_ignored() {
        let dir = std::env::temp_dir().join(format!(
            "feral_processes_gui_scan_sprite_dir_test_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("player.png"),
            b"not real png bytes, irrelevant to the scan",
        )
        .unwrap();
        std::fs::write(dir.join("README.md"), b"not a sprite").unwrap();

        let names = scan_sprite_dir(&dir);

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(names, vec!["player".to_string()]);
    }

    /// `@` is a legal filename character on every platform this ships to,
    /// so an `@`-prefixed file must be filtered by the scan itself rather
    /// than trusted never to exist. `@drawn.png` specifically would
    /// otherwise land in exactly `DRAWN_ICON_KEY`'s slot and — since
    /// `register` writes it after `sync_drawn_icon` already has, in an
    /// unordered `PreUpdate` pair — permanently shadow the player's own
    /// drawing, or draw as it on a blank canvas that never even reaches
    /// `register`'s overwrite.
    #[test]
    fn an_at_prefixed_stem_is_never_scanned() {
        let dir = std::env::temp_dir().join(format!(
            "feral_processes_gui_scan_sprite_dir_at_test_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("@drawn.png"), b"not real png bytes, irrelevant").unwrap();
        std::fs::write(dir.join("@other.png"), b"not real png bytes, irrelevant").unwrap();
        std::fs::write(dir.join("player.png"), b"not real png bytes, irrelevant").unwrap();

        let names = scan_sprite_dir(&dir);

        std::fs::remove_dir_all(&dir).ok();
        assert!(
            !names.contains(&DRAWN_ICON_KEY.to_string()),
            "the scan must never be able to claim the runtime-only drawn-icon key"
        );
        assert_eq!(
            names,
            vec!["player".to_string()],
            "every @-prefixed stem must be filtered, not just @drawn"
        );
    }

    // No test here for "a malformed image is skipped with a warning and the
    // rest still load". That behaviour lives entirely in `register`'s
    // `LoadState::Failed` arm above — the scan never opens a file, only
    // lists names by extension, so a malformed PNG passes the scan exactly
    // like a valid one and the two are indistinguishable at this layer.
    // `register`'s arm predates this task and is untouched by it.
    //
    // Exercising it for real needs a live `AssetServer` actually decoding a
    // file asynchronously, which only resolves once bevy's IO task pool has
    // run and requires polling `app.update()` against wall-clock time until
    // it does — exactly the `sleep()`-driven, wall-clock-dependent shape
    // CLAUDE.md's Testing section rules out ("No flaky tests. No sleep(),
    // no wall-clock dependence"). No amount of getting the harness right
    // changes that shape, so this task adds no test for it.
}
