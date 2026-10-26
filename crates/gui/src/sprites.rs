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
//! supported way deleting `assets/settlements/` restores a world with no
//! towns in it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use feral_processes_app_core::{DEFAULT_SPRITE_FRAME_MS, MAX_SPRITE_FRAMES};
use feral_processes_engine::icon::{Canvas, FULL_COLOUR_SUFFIX, quantise, sprite_rgba};
use feral_processes_engine::{ICON_SIZE, PlayerIcon};

use crate::paint::SpriteTable;
use crate::sprite_layers::{Layer, LoadKey, SpriteLayers, WriteOutcome};

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
pub(crate) fn scan_sprite_dir(dir: &std::path::Path) -> Vec<String> {
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

/// Encodes `frames` through `sprite_rgba` — the quantiser's exact inverse —
/// and writes them side by side as one `16n x 16` PNG, overwriting whatever
/// was there.
///
/// The one door the sprite editor's `[s]` reaches disk through. The frames'
/// own `edge()` sizes each cell rather than assuming `ICON_SIZE`: the sprite
/// canvas is always 16x16 by construction (`app-core`'s `SPRITE_EDGE`), but
/// this codec has no reason to assume that of its caller.
pub fn frames_to_png(frames: &[Canvas], path: &Path) -> std::io::Result<()> {
    let edge = frames.first().map_or(ICON_SIZE, Canvas::edge);
    let mut buf = image::RgbaImage::new((edge * frames.len()) as u32, edge as u32);
    for (i, canvas) in frames.iter().enumerate() {
        for y in 0..edge {
            for x in 0..edge {
                let (r, g, b, a) = sprite_rgba(canvas.get(x, y));
                buf.put_pixel((i * edge + x) as u32, y as u32, image::Rgba([r, g, b, a]));
            }
        }
    }
    buf.save(path)
        .map_err(|e| std::io::Error::other(format!("{path:?}: {e}")))
}

/// Decodes a PNG at `path` back into its frames, quantising every pixel onto
/// `SPRITE_PALETTE` through `quantise` — the codec's other half.
///
/// **The format is guessed from the bytes, not the extension.** A disabled
/// sprite's real PNG data sits behind a `.png.off` path (the `.off` naming
/// rule in `sprite_layers`), and `image::open`'s extension-based dispatch cannot decode
/// that; `ImageReader::with_guessed_format` sniffs the magic bytes instead,
/// so this one function reads both an enabled sprite and its disabled
/// counterpart identically.
///
/// **`None` on any failure**: missing file, a file that isn't a PNG, a
/// corrupt one, or — I3's fix — one `frames_in` refuses. `assets/sprites/
/// README.md` calls the 16px cell non-negotiable and `text::map_cell`
/// depends on it; a merely-square image used to pass here and open an
/// off-format editor that would write the same wrong size back out. This is
/// the same warn-and-carry-on contract `register`'s `LoadState::Failed` arm
/// already keeps for a sprite that fails to load through the asset server.
/// There is nothing to log to here (this runs off the render thread, ahead
/// of the frame that would show a refusal), so the caller decides what a
/// `None` means.
pub fn png_to_frames(path: &Path) -> Option<Vec<Canvas>> {
    let img = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?
        .into_rgba8();
    let (w, h) = img.dimensions();
    let count = frames_in(w, h)?;
    let edge = ICON_SIZE as u32;
    Some(
        (0..count as u32)
            .map(|i| {
                let mut canvas = Canvas::new(ICON_SIZE);
                for y in 0..edge {
                    for x in 0..edge {
                        let p = img.get_pixel(i * edge + x, y);
                        canvas.set(x as usize, y as usize, quantise((p[0], p[1], p[2], p[3])));
                    }
                }
                canvas
            })
            .collect(),
    )
}

/// The `SpriteTable` key the player's own drawing is registered under.
///
/// The `@` is what keeps it unreachable from anywhere else: every other key
/// comes from `scan_sprite_dir`, which throws out any stem starting with
/// `@` for exactly this reason, and a species or structure's own `sprite:`
/// field draws from the same scan through `sprite_name()`, which falls back
/// to the def's id for an `@`-prefixed override rather than honouring it —
/// so no file and no def, shipped or modded, can claim the slot the player
/// drew for themselves.
pub const DRAWN_ICON_KEY: &str = "@drawn";

/// Where the loaded sprites live between the asset server and the renderer.
#[derive(Resource, Default)]
pub struct Sprites {
    /// Still loading, or waiting to be handed to egui. Keyed by folder and
    /// stem because they come off a directory scan, and because `register`
    /// reads the pace from the folder the art came from.
    pending: Vec<(LoadKey, Handle<Image>)>,
    /// What the renderer draws from. Refcounted so the per-frame `Painter`
    /// costs one atomic bump rather than a copy.
    table: Arc<SpriteTable>,
    /// The drawing currently on the GPU, and the handle it was uploaded
    /// under. The icon is kept beside the handle so `sync_drawn_icon` can
    /// answer "has this changed?" by value, and the handle is kept so the
    /// old registration can be taken back before a new one replaces it.
    drawn: Option<(PlayerIcon, Handle<Image>)>,
    /// The handle each table stem was registered under. Every `Assets::add`
    /// reload mints a new image, and `EguiUserTextures` holds it strongly,
    /// so a name leaving the table must hand its handle back too.
    handles: HashMap<String, Handle<Image>>,
}

impl Sprites {
    pub fn table(&self) -> Arc<SpriteTable> {
        Arc::clone(&self.table)
    }

    /// Takes `name` out of the table, frees the textures it was drawn from,
    /// and forgets any load still queued for it — a late load for the old
    /// art must not land over what the caller is about to queue.
    fn drop_name(&mut self, name: &str, textures: &mut EguiUserTextures) {
        // `Arc::make_mut` for `register`'s reason: the renderer may be
        // holding a clone of the old table from this frame.
        Arc::make_mut(&mut self.table).remove(name);
        self.pending.retain(|(key, _)| {
            key.stem
                .strip_suffix(FULL_COLOUR_SUFFIX)
                .unwrap_or(&key.stem)
                != name
        });
        for stem in [name.to_string(), format!("{name}{FULL_COLOUR_SUFFIX}")] {
            if let Some(handle) = self.handles.remove(&stem) {
                textures.remove_image(&handle);
            }
        }
    }

    /// Puts the player's drawn icon on the GPU, or takes it back off.
    ///
    /// This is the only texture the game builds at runtime; everything else
    /// in the table came off disk through `load`. It is asked every frame,
    /// so the first thing it does is compare the icon **by value** and do
    /// nothing for an equal one — `PartialEq` over a few hundred bytes is cheaper
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
    /// **The avatar uploads as an ordinary sprite sheet**: `ICON_SIZE * n`
    /// wide, `ICON_SIZE` tall, frame `f` at x offset `f * ICON_SIZE`, and
    /// registered under `DRAWN_ICON_KEY` with its own pace — the shape
    /// `load` makes of a file, so the draw sites need no second path.
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
        // Through `PlayerIcon::rgba`, the one place an index becomes a
        // colour and index 0 becomes a transparent pixel rather than an
        // opaque black one.
        let frames = icon.frames().len();
        let mut bytes = Vec::with_capacity(ICON_SIZE * frames * ICON_SIZE * 4);
        for y in 0..ICON_SIZE {
            for frame in 0..frames {
                for x in 0..ICON_SIZE {
                    let (r, g, b, a) = icon.rgba(frame, x, y);
                    bytes.extend_from_slice(&[r, g, b, a]);
                }
            }
        }
        let mut image = Image::new(
            Extent3d {
                width: (ICON_SIZE * frames) as u32,
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
        table.insert(DRAWN_ICON_KEY, id, frames, icon.frame_ms());
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
    mut images: ResMut<Assets<Image>>,
) {
    let layers = layers_of(&frontend.app);
    for key in layers.load_list() {
        queue_load(&asset_server, &mut images, &layers, &mut sprites, key);
    }
}

/// Installs `App::sprite_library`/`sprite_disabled` once before the first
/// frame — and before any scripted `--keys` are replayed, which would
/// otherwise open the editor on an empty library — so the
/// picker's art column and the editor's opening canvas answer "what's
/// installed" correctly from the very first frame rather than only after
/// the first save.
pub fn install_library(app: &mut feral_processes_app_core::App) {
    let library = layers_of(app).library();
    app.install_sprite_library(library.enabled, library.disabled, library.edited);
}

/// The two sprite folders as the app currently knows them.
fn layers_of(app: &feral_processes_app_core::App) -> SpriteLayers {
    SpriteLayers {
        shipped: app.assets_dir().join("sprites"),
        user: app.user_sprite_dir().map(Path::to_path_buf),
    }
}

/// A sheet's `<name>.anim.ron`: how long each cell shows. Per name, not per
/// variant, so the plain and `.colour` sheets of one sprite keep one pace.
#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct SpriteAnim {
    pub frame_ms: u32,
}

pub(crate) fn anim_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.anim.ron"))
}

/// The pace for `name`'s sheet. A missing file is the ordinary case and is
/// silent; a malformed one warns and falls back, like every other asset
/// file, so a bad edit never costs the sprite itself.
pub fn read_frame_ms(dir: &Path, name: &str) -> u32 {
    let path = anim_path(dir, name);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return DEFAULT_SPRITE_FRAME_MS;
    };
    match ron::from_str::<SpriteAnim>(&text) {
        Ok(anim) if anim.frame_ms > 0 => anim.frame_ms,
        Ok(_) => {
            warn!(
                "{}: frame_ms must be positive, using the default",
                path.display()
            );
            DEFAULT_SPRITE_FRAME_MS
        }
        Err(e) => {
            warn!("{}: {e}, using the default", path.display());
            DEFAULT_SPRITE_FRAME_MS
        }
    }
}

/// How many 16x16 cells a `width` x `height` image holds, or `None` when it
/// is not a whole row of at most `MAX_SPRITE_FRAMES`.
fn frames_in(width: u32, height: u32) -> Option<usize> {
    let cell = ICON_SIZE as u32;
    if height != cell || !width.is_multiple_of(cell) {
        return None;
    }
    let frames = (width / cell) as usize;
    (1..=MAX_SPRITE_FRAMES).contains(&frames).then_some(frames)
}

/// Where one sprite's pixels come from.
#[derive(Debug, PartialEq)]
enum Route {
    /// The asset server, under the shipped `assets/` root.
    Asset(String),
    /// A file in the player's folder, read straight off the disk.
    File(PathBuf),
}

fn route(layers: &SpriteLayers, key: &LoadKey) -> Option<Route> {
    match key.layer {
        Layer::Shipped => Some(Route::Asset(format!("sprites/{}.png", key.stem))),
        Layer::User => layers
            .dir(Layer::User)
            .map(|dir| Route::File(dir.join(format!("{}.png", key.stem)))),
    }
}

/// A user-folder PNG as a nearest-sampled image, or the reason it is not one.
fn read_user_image(path: &Path) -> Result<Image, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Image::from_buffer(
        &bytes,
        bevy::image::ImageType::Extension("png"),
        bevy::image::CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::default(),
    )
    .map_err(|e| e.to_string())
}

/// Starts one sprite on its way to `register`.
///
/// The player's folder is read directly instead of through the asset server.
/// The server dedups by path while any handle to it lives, and the egui
/// registration keeps one, so a second save of the same name would get the
/// old, already-loaded handle back and redraw the old pixels. (`reload` is no
/// better: it leaves the load state at `Loaded` while the new bytes are in
/// flight.) A fresh `Assets::add` is a fresh handle every time. The shipped
/// art never changes at runtime, so it keeps the server's cache.
fn queue_load(
    asset_server: &AssetServer,
    images: &mut Assets<Image>,
    layers: &SpriteLayers,
    sprites: &mut Sprites,
    key: LoadKey,
) {
    let handle = match route(layers, &key) {
        Some(Route::Asset(path)) => asset_server
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::nearest();
            })
            .load(path),
        Some(Route::File(path)) => match read_user_image(&path) {
            Ok(image) => images.add(image),
            // The user file still owns the name, so no fallback to shipped art.
            Err(e) => {
                warn!(
                    "sprite `{}` did not load, falling back to its glyph: {e}",
                    key.stem
                );
                return;
            }
        },
        None => return,
    };
    sprites.pending.push((key, handle));
}

/// Drains `App::take_sprite_writes`, performs each one through
/// `SpriteLayers::apply`, and — reusing `load`'s own upload path rather than
/// a second one — pushes a landed `Save`/`Enable` onto `Sprites::pending` so
/// `register` (ordered directly after this, in `PreUpdate`) can land it in
/// the table this same frame.
///
/// A `Disable` or `Revert` takes the name back out of the table itself,
/// through `SpriteTable::remove`, so the art stops drawing on the same frame
/// rather than lagging a load cycle behind, and then loads whatever still
/// draws for that name. A save takes the name out first too: the table keeps
/// a colour entry over a plain one, so a plain redraw would otherwise stay
/// hidden behind the colour art it replaced.
///
/// The library is rescanned and reinstalled once, after every write in the
/// batch, rather than per write.
pub fn drain_writes(
    asset_server: Res<AssetServer>,
    mut frontend: ResMut<crate::Frontend>,
    mut sprites: ResMut<Sprites>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<EguiUserTextures>,
) {
    let writes = frontend.app.take_sprite_writes();
    if writes.is_empty() {
        return;
    }
    let layers = layers_of(&frontend.app);
    for write in writes {
        match layers.apply(&write.name, write.op) {
            WriteOutcome::Load(key) => {
                sprites.drop_name(&write.name, &mut textures);
                queue_load(&asset_server, &mut images, &layers, &mut sprites, key);
            }
            WriteOutcome::Remove { name, then_load } => {
                sprites.drop_name(&name, &mut textures);
                for key in then_load {
                    queue_load(&asset_server, &mut images, &layers, &mut sprites, key);
                }
            }
            WriteOutcome::Failed(message) => frontend.app.report_sprite_write_failure(message),
        }
    }
    let library = layers.library();
    frontend
        .app
        .install_sprite_library(library.enabled, library.disabled, library.edited);
}

/// The pace for the sheet `key` names. It lives beside the art it describes:
/// the folder that owns the name, not always the shipped one.
fn pace_for(layers: &SpriteLayers, key: &LoadKey) -> u32 {
    let name = key
        .stem
        .strip_suffix(FULL_COLOUR_SUFFIX)
        .unwrap_or(&key.stem);
    layers
        .dir(key.layer)
        .map_or(DEFAULT_SPRITE_FRAME_MS, |dir| read_frame_ms(dir, name))
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
    images: Res<Assets<Image>>,
    frontend: Res<crate::Frontend>,
) {
    if sprites.pending.is_empty() {
        return;
    }
    let mut ready = Vec::new();
    sprites.pending.retain(|(key, handle)| {
        match asset_server.get_load_state(handle) {
            Some(LoadState::Loaded) => {
                ready.push((key.clone(), handle.clone()));
                false
            }
            Some(LoadState::Failed(e)) => {
                warn!(
                    "sprite `{}` did not load, falling back to its glyph: {e}",
                    key.stem
                );
                false
            }
            // A player's sprite was added to `Assets` directly, so the
            // server has no state for it: the pixels are already here.
            None if images.contains(handle) => {
                ready.push((key.clone(), handle.clone()));
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
    let layers = layers_of(&frontend.app);
    for (key, handle) in ready {
        let name = key.stem.clone();
        let frames = images
            .get(&handle)
            .and_then(|image| frames_in(image.width(), image.height()));
        let Some(frames) = frames else {
            warn!(
                "sprite `{name}` is not a 16px-tall row of 1-{MAX_SPRITE_FRAMES} cells, falling back to its glyph"
            );
            continue;
        };
        let id = textures.add_image(EguiTextureHandle::Strong(handle.clone()));
        let pace_ms = pace_for(&layers, &key);
        Arc::make_mut(&mut sprites.table).insert(name.as_str(), id, frames, pace_ms);
        if let Some(old) = sprites.handles.insert(name, handle) {
            textures.remove_image(&old);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy::image::ImageFilterMode;
    use feral_processes_app_core::InstalledSprite;
    use feral_processes_engine::sprite_rgba;

    pub(crate) fn plain(canvas: Canvas) -> InstalledSprite {
        InstalledSprite {
            frames: vec![canvas],
            frame_ms: DEFAULT_SPRITE_FRAME_MS,
            full_colour: false,
        }
    }

    /// One-frame views of the sheet codec, for the tests whose subject is
    /// the toggle and variant rules rather than the frame count.
    pub(crate) fn canvas_to_png(canvas: &Canvas, path: &Path) -> std::io::Result<()> {
        frames_to_png(std::slice::from_ref(canvas), path)
    }

    pub(crate) fn png_to_canvas(path: &Path) -> Option<Canvas> {
        let mut frames = png_to_frames(path)?;
        (frames.len() == 1).then(|| frames.remove(0))
    }

    /// A drawing with one lit cell — enough to be non-blank, and it pins
    /// the byte order at a coordinate that is not the origin.
    fn a_drawing() -> PlayerIcon {
        an_icon(&[&[(2, 1, 6)]], DEFAULT_SPRITE_FRAME_MS)
    }

    /// An icon whose frames have the listed cells painted.
    fn an_icon(frames: &[&[(usize, usize, u8)]], frame_ms: u32) -> PlayerIcon {
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

    fn image_pixel(image: &Image, x: usize, y: usize) -> (u8, u8, u8, u8) {
        let data = image
            .data
            .as_ref()
            .expect("the image must carry its pixels");
        let at = (y * image.width() as usize + x) * 4;
        (data[at], data[at + 1], data[at + 2], data[at + 3])
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

    /// **Frames sit side by side in one sheet**, each at `f * ICON_SIZE`,
    /// registered with the icon's own pace. Asserted on the bytes, with a
    /// cell in the second frame to catch frames stacked or overwritten.
    #[test]
    fn a_two_frame_icon_uploads_a_32x16_sheet_and_registers_its_pace() {
        let (mut sprites, mut images, mut textures) = fixtures();
        let icon = an_icon(&[&[(1, 0, 5)], &[(2, 3, 9)]], 450);

        sprites.sync_drawn_icon(Some(&icon), &mut images, &mut textures);

        let (_, handle) = sprites.drawn.as_ref().expect("the icon must be kept");
        let image = images.get(handle).expect("the image must be in the assets");
        assert_eq!((image.width(), image.height()), (32, 16));
        assert_eq!(image_pixel(image, 1, 0), sprite_rgba(5));
        assert_eq!(image_pixel(image, 16 + 2, 3), sprite_rgba(9));
        assert_eq!(image_pixel(image, 2, 3), (0, 0, 0, 0));
        assert_eq!(image_pixel(image, 16 + 1, 0), (0, 0, 0, 0));
        let entry = sprites.table().get(DRAWN_ICON_KEY).unwrap();
        assert_eq!((entry.frames, entry.frame_ms), (2, 450));
    }

    #[test]
    fn a_one_frame_icon_uploads_a_16x16_sheet() {
        let (mut sprites, mut images, mut textures) = fixtures();

        sprites.sync_drawn_icon(Some(&a_drawing()), &mut images, &mut textures);

        let (_, handle) = sprites.drawn.as_ref().expect("the icon must be kept");
        let image = images.get(handle).expect("the image must be in the assets");
        assert_eq!((image.width(), image.height()), (16, 16));
        assert_eq!(image_pixel(image, 2, 1), sprite_rgba(6));
        assert_eq!(
            image
                .data
                .as_ref()
                .unwrap()
                .chunks_exact(4)
                .filter(|p| p[3] == 255)
                .count(),
            1,
            "one lit cell is exactly one opaque pixel"
        );
        let entry = sprites.table().get(DRAWN_ICON_KEY).unwrap();
        assert_eq!((entry.frames, entry.frame_ms), (1, DEFAULT_SPRITE_FRAME_MS));
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
        let second = an_icon(&[&[(2, 1, 6), (5, 5, 3)]], DEFAULT_SPRITE_FRAME_MS);
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

    /// Worst allowed RGB distance from any opaque shipped pixel to its
    /// nearest `SPRITE_PALETTE` entry. Measured over every opaque pixel in
    /// `assets/sprites/` (alpha >= `SPRITE_ALPHA_THRESHOLD`, euclidean RGB):
    /// max 71.8 (`(0,255,204)` in `depot.colour.png`), mean 13.06. A palette
    /// edit that makes shipped art snap worse fails here.
    const SPRITE_SNAP_BOUND: f64 = 72.0;

    /// Lives in gui because the engine has no PNG decoder and gains none for
    /// a test.
    #[test]
    fn every_shipped_sprite_snaps_within_the_bound() {
        use feral_processes_engine::icon::{SPRITE_ALPHA_THRESHOLD, SPRITE_PALETTE, quantise};
        let mut worst = (0.0f64, String::new());
        let mut seen = 0;
        for entry in std::fs::read_dir(shipped_sprites_dir()).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "png") {
                continue;
            }
            let img = image::open(&path).unwrap().into_rgba8();
            for p in img.pixels().filter(|p| p[3] >= SPRITE_ALPHA_THRESHOLD) {
                seen += 1;
                let (r, g, b) = SPRITE_PALETTE[quantise((p[0], p[1], p[2], p[3])) as usize - 1];
                let d = |a: u8, b: u8| (a as f64 - b as f64).powi(2);
                let nearest = (d(p[0], r) + d(p[1], g) + d(p[2], b)).sqrt();
                if nearest > worst.0 {
                    worst = (nearest, format!("{path:?} {:?}", p.0));
                }
            }
        }
        assert!(seen > 0, "no opaque pixels, so this proved nothing");
        assert!(
            worst.0 < SPRITE_SNAP_BOUND,
            "worst snap {:.1} at {} exceeds {SPRITE_SNAP_BOUND}",
            worst.0,
            worst.1
        );
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
    fn pending_keys_route_by_layer() {
        let layers = SpriteLayers {
            shipped: PathBuf::from("/shipped/sprites"),
            user: Some(PathBuf::from("/data/sprites")),
        };
        let key = |layer, stem: &str| LoadKey {
            layer,
            stem: stem.to_string(),
        };
        assert_eq!(
            route(&layers, &key(Layer::Shipped, "x.colour")),
            Some(Route::Asset("sprites/x.colour.png".to_string()))
        );
        assert_eq!(
            route(&layers, &key(Layer::User, "x")),
            Some(Route::File(PathBuf::from("/data/sprites/x.png")))
        );
        let none = SpriteLayers {
            user: None,
            ..layers
        };
        assert_eq!(route(&none, &key(Layer::User, "x")), None);
    }

    #[test]
    fn pace_is_read_from_the_folder_that_owns_the_key() {
        let root = codec_test_dir("pace_for");
        let (shipped, user) = (root.join("shipped"), root.join("user"));
        std::fs::create_dir_all(&shipped).unwrap();
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(anim_path(&shipped, "x"), "(frame_ms: 400)").unwrap();
        std::fs::write(anim_path(&user, "x"), "(frame_ms: 250)").unwrap();
        let layers = SpriteLayers {
            shipped,
            user: Some(user),
        };
        let key = |layer, stem: &str| LoadKey {
            layer,
            stem: stem.to_string(),
        };

        let from_user = pace_for(&layers, &key(Layer::User, "x"));
        let from_user_colour = pace_for(&layers, &key(Layer::User, "x.colour"));
        let from_shipped = pace_for(&layers, &key(Layer::Shipped, "x"));
        let no_user = pace_for(
            &SpriteLayers {
                user: None,
                ..layers
            },
            &key(Layer::User, "x"),
        );

        std::fs::remove_dir_all(&root).ok();
        assert_eq!(from_user, 250);
        assert_eq!(from_user_colour, 250);
        assert_eq!(from_shipped, 400);
        assert_eq!(no_user, DEFAULT_SPRITE_FRAME_MS);
    }

    #[test]
    fn a_user_png_reads_nearest_sampled_and_a_corrupt_one_errs() {
        let dir = codec_test_dir("user_image");
        let good = dir.join("g.png");
        frames_to_png(&[a_sprite_canvas()], &good).unwrap();
        let image = read_user_image(&good).unwrap();
        assert_eq!((image.width(), image.height()), (16, 16));
        assert!(matches!(
            image.sampler,
            ImageSampler::Descriptor(ref d) if d.mag_filter == bevy::image::ImageFilterMode::Nearest
        ));
        let bad = dir.join("b.png");
        std::fs::write(&bad, b"nope").unwrap();
        assert!(read_user_image(&bad).is_err());
        assert!(read_user_image(&dir.join("missing.png")).is_err());
    }

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

    /// A fresh, unique scratch directory under the OS temp dir — never
    /// `assets/sprites/`. `tag` keeps two tests' directories from ever
    /// colliding even when run in parallel, the same disambiguator
    /// `an_at_prefixed_stem_is_never_scanned` inlines by hand above.
    pub(crate) fn codec_test_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "feral_processes_gui_sprites_codec_{tag}_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn frames_in_counts_whole_16px_cells() {
        assert_eq!(frames_in(16, 16), Some(1));
        assert_eq!(frames_in(32, 16), Some(2));
        assert_eq!(frames_in(24, 16), None);
        assert_eq!(frames_in(48, 16), None);
        assert_eq!(frames_in(16, 32), None);
        assert_eq!(frames_in(0, 16), None);
    }

    #[test]
    fn frame_ms_reads_the_file_and_defaults_otherwise() {
        let dir = codec_test_dir("frame_ms");
        assert_eq!(read_frame_ms(&dir, "none"), DEFAULT_SPRITE_FRAME_MS);
        std::fs::write(anim_path(&dir, "ok"), "(frame_ms: 250)").unwrap();
        assert_eq!(read_frame_ms(&dir, "ok"), 250);
        std::fs::write(anim_path(&dir, "bad"), "not ron").unwrap();
        assert_eq!(read_frame_ms(&dir, "bad"), DEFAULT_SPRITE_FRAME_MS);
        std::fs::write(anim_path(&dir, "zero"), "(frame_ms: 0)").unwrap();
        assert_eq!(read_frame_ms(&dir, "zero"), DEFAULT_SPRITE_FRAME_MS);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A canvas with a few painted cells — enough to exercise more than one
    /// palette index and a still-transparent corner.
    pub(crate) fn a_sprite_canvas() -> Canvas {
        let mut canvas = Canvas::new(ICON_SIZE);
        canvas.set(0, 0, 1);
        canvas.set(3, 5, 12);
        canvas.set(15, 15, 19);
        canvas
    }

    /// The round trip `SPRITE_PALETTE`'s quantiser makes loss-free: every
    /// painted index writes to a pixel that quantises straight back to the
    /// same index, because `sprite_rgba` and `quantise` are exact inverses
    /// on the palette's own colours.
    #[test]
    fn a_canvas_written_and_read_back_is_the_same_canvas() {
        let dir = codec_test_dir("roundtrip");
        let path = dir.join("subject.png");
        let canvas = a_sprite_canvas();

        canvas_to_png(&canvas, &path).expect("a fresh temp dir must accept the write");
        let read_back = png_to_canvas(&path).expect("a file this codec just wrote must decode");

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(read_back, canvas);
    }

    /// The written file is a real 16x16 RGBA PNG — the exact contract
    /// `crates/gui/tests/sprites.rs::the_shipped_sprites_are_one_cell`
    /// polices for the shipped art, now true of a save this codec produces
    /// too.
    #[test]
    fn a_written_file_is_16x16_rgba() {
        let dir = codec_test_dir("dims");
        let path = dir.join("subject.png");

        canvas_to_png(&a_sprite_canvas(), &path).unwrap();
        let img = image::open(&path).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(
            (img.width(), img.height()),
            (ICON_SIZE as u32, ICON_SIZE as u32)
        );
        assert_eq!(img.color(), image::ColorType::Rgba8);
    }

    /// A corrupt or non-PNG file at the expected path is a decode failure,
    /// not a panic — the same contract every asset database in this repo
    /// keeps for a malformed file on disk.
    #[test]
    fn png_to_canvas_on_a_corrupt_file_is_none() {
        let dir = codec_test_dir("corrupt");
        let path = dir.join("subject.png");
        std::fs::write(&path, b"not a png at all").unwrap();

        let result = png_to_canvas(&path);

        std::fs::remove_dir_all(&dir).ok();
        assert!(result.is_none());
    }

    /// A missing file is the same "no answer" as a corrupt one — `png_to_
    /// canvas` never distinguishes "absent" from "unreadable," matching
    /// `Painter::sprite`'s own single fallback.
    #[test]
    fn png_to_canvas_on_a_missing_file_is_none() {
        let dir = codec_test_dir("missing");
        let path = dir.join("nothing_here.png");

        let result = png_to_canvas(&path);

        std::fs::remove_dir_all(&dir).ok();
        assert!(result.is_none());
    }

    /// **I3.** A square PNG that is not exactly `ICON_SIZE` wide must be
    /// refused, not silently accepted at its own size — `assets/sprites/
    /// README.md` calls 16x16 non-negotiable, and `text::map_cell`'s zoom
    /// ladder is built on every sprite being that size. Before this fix the
    /// only check was `w == h`, so a 24x24 drop would open a 24x24 editor
    /// and `[s]` would write a 24x24 file straight back out.
    #[test]
    fn png_to_canvas_refuses_a_square_image_of_the_wrong_size() {
        let dir = codec_test_dir("wrong_size");
        let path = dir.join("subject.png");
        let img = image::RgbaImage::new(24, 24);
        img.save(&path).unwrap();

        let result = png_to_canvas(&path);

        std::fs::remove_dir_all(&dir).ok();
        assert!(
            result.is_none(),
            "a 24x24 PNG must be refused, not opened at the wrong size"
        );
    }

    pub(crate) fn two_frames() -> Vec<Canvas> {
        let mut second = a_sprite_canvas();
        second.set(7, 7, 4);
        vec![a_sprite_canvas(), second]
    }

    #[test]
    fn a_two_frame_sheet_written_and_read_back_is_the_same_frames() {
        let dir = codec_test_dir("two_frame_roundtrip");
        let path = dir.join("subject.png");

        frames_to_png(&two_frames(), &path).unwrap();
        let read_back = png_to_frames(&path);

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(read_back, Some(two_frames()));
    }

    #[test]
    fn a_two_frame_file_is_32x16() {
        let dir = codec_test_dir("two_frame_dims");
        let path = dir.join("subject.png");

        frames_to_png(&two_frames(), &path).unwrap();
        let img = image::open(&path).unwrap();

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!((img.width(), img.height()), (32, 16));
    }

    #[test]
    fn png_to_frames_refuses_a_row_that_is_not_one_or_two_whole_cells() {
        let dir = codec_test_dir("odd_widths");
        for (w, h) in [(24, 16), (48, 16), (16, 32)] {
            let path = dir.join(format!("{w}x{h}.png"));
            image::RgbaImage::new(w, h).save(&path).unwrap();
            assert!(png_to_frames(&path).is_none(), "{w}x{h}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
