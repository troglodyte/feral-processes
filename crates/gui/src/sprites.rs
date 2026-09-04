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

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use feral_processes_app_core::SpriteOp;
use feral_processes_engine::icon::{Canvas, quantise, sprite_rgba};
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

/// Encodes `canvas` through `sprite_rgba` — the quantiser's exact inverse —
/// and writes it to `path` as a PNG, overwriting whatever was there.
///
/// The one door the sprite editor's `[s]` reaches disk through. `canvas`'s
/// own `edge()` sizes the image rather than assuming `ICON_SIZE`: the sprite
/// canvas is always 16x16 by construction (`app-core`'s `SPRITE_EDGE`), but
/// this codec has no reason to assume that of its caller.
pub fn canvas_to_png(canvas: &Canvas, path: &Path) -> std::io::Result<()> {
    let edge = canvas.edge();
    let mut buf = image::RgbaImage::new(edge as u32, edge as u32);
    for y in 0..edge {
        for x in 0..edge {
            let (r, g, b, a) = sprite_rgba(canvas.get(x, y));
            buf.put_pixel(x as u32, y as u32, image::Rgba([r, g, b, a]));
        }
    }
    buf.save(path)
        .map_err(|e| std::io::Error::other(format!("{path:?}: {e}")))
}

/// Decodes a PNG at `path` back into a `Canvas`, quantising every pixel onto
/// `SPRITE_PALETTE` through `quantise` — the codec's other half.
///
/// **The format is guessed from the bytes, not the extension.** A disabled
/// sprite's real PNG data sits behind a `.png.off` path (`scan_library`'s own
/// naming rule), and `image::open`'s extension-based dispatch cannot decode
/// that; `ImageReader::with_guessed_format` sniffs the magic bytes instead,
/// so this one function reads both an enabled sprite and its disabled
/// counterpart identically.
///
/// **`None` on any failure**: missing file, a file that isn't a PNG, a
/// corrupt one, or — I3's fix — one that is not exactly `ICON_SIZE` square.
/// `assets/sprites/README.md` calls 16x16 non-negotiable and `text::map_cell`
/// depends on it; a merely-square image used to pass here and open an
/// off-format editor that would write the same wrong size back out. This is
/// the same warn-and-carry-on contract `register`'s `LoadState::Failed` arm
/// already keeps for a sprite that fails to load through the asset server.
/// There is nothing to log to here (this runs off the render thread, ahead
/// of the frame that would show a refusal), so the caller decides what a
/// `None` means.
pub fn png_to_canvas(path: &Path) -> Option<Canvas> {
    let img = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?
        .into_rgba8();
    let (w, h) = img.dimensions();
    if w as usize != ICON_SIZE || h as usize != ICON_SIZE {
        return None;
    }
    let edge = w as usize;
    let mut canvas = Canvas::new(edge);
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x, y);
            canvas.set(x as usize, y as usize, quantise((p[0], p[1], p[2], p[3])));
        }
    }
    Some(canvas)
}

/// Everything installed art `dir` holds: every enabled sprite decoded to a
/// `Canvas`, keyed by name, and every disabled one (a `<name>.png.off` file
/// — see `assets/sprites/README.md`'s naming rule) decoded the same way.
///
/// **Both maps carry pixels, not bare names — I2's fix.** `App::
/// install_sprite_library`'s `disabled` half used to be a `HashSet<String>`,
/// so `Enter` on an `Off` subject had no art to open and fell back to blank;
/// decoding `.png.off` files here (through `png_to_canvas`'s guessed-format
/// read, since the real extension is `off`) is what lets the picker's own
/// promise — "toggling it off... keeps the art on disk" — reach the one tool
/// that can show it back to the player.
///
/// **Deliberately not `scan_sprite_dir`.** That scan answers "what may the
/// asset server load," a list of names; this answers "what does the sprite
/// editor's library actually look like," which needs the pixels neither the
/// loader nor the map ever ask for. A missing directory, like
/// `scan_sprite_dir`, is the supported empty state rather than an error. A
/// `.png`/`.png.off` that fails to decode is silently dropped from its map
/// rather than surfaced — the same contract `png_to_canvas` keeps on its
/// own.
pub fn scan_library(dir: &Path) -> (HashMap<String, Canvas>, HashMap<String, Canvas>) {
    let mut enabled = HashMap::new();
    let mut disabled = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (enabled, disabled);
    };
    for path in entries.filter_map(|entry| entry.ok().map(|entry| entry.path())) {
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Some(name) = file_name.strip_suffix(".png.off") {
            if !name.starts_with('@')
                && let Some(canvas) = png_to_canvas(&path)
            {
                disabled.insert(name.to_string(), canvas);
            }
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "png") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.starts_with('@') {
            continue;
        }
        if let Some(canvas) = png_to_canvas(&path) {
            enabled.insert(stem.to_string(), canvas);
        }
    }
    (enabled, disabled)
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

/// Installs `App::sprite_library`/`sprite_disabled` once at startup, so the
/// picker's art column and the editor's opening canvas answer "what's
/// installed" correctly from the very first frame rather than only after
/// the first save.
///
/// Gated on `sprite_forge_enabled()` — the picker and editor are the only
/// readers of this state, and both are unreachable without the flag *and*
/// a checkout, so decoding every shipped sprite into a `Canvas` here would
/// be pure cost with no reader on every other build and every ordinary run.
pub fn install_library(mut frontend: ResMut<crate::Frontend>) {
    if !frontend.app.sprite_forge_enabled() {
        return;
    }
    let dir = frontend.app.assets_dir().join("sprites");
    let (enabled, disabled) = scan_library(&dir);
    frontend.app.install_sprite_library(enabled, disabled);
}

/// What `apply_sprite_write` did, and what `drain_writes` must therefore do
/// next — the frontend's reload/removal half, kept out of the pure function
/// below so that half stays testable without a bevy `AssetServer`.
#[derive(Debug, PartialEq, Eq)]
enum WriteOutcome {
    /// A `Save` or `Enable` landed; `<name>.png` should be (re)loaded into
    /// the table.
    Reload,
    /// A `Disable` landed; the name should come straight back out of the
    /// table rather than waiting on a load that will never happen.
    Disabled,
    /// The write did not happen — already warned to the log; nothing else
    /// to do.
    Failed,
}

/// The pure half of `drain_writes`: performs the one write or rename `op`
/// asks for under `name`, inside `dir`, and reports what happened. No bevy
/// resource in sight, which is what makes the seam this closes —
/// `apply_sprite_write` writes are TDD tests, `drain_writes` is glue.
///
/// **The invariant this maintains: `<name>.png` and `<name>.png.off` never
/// both exist.** That is I1's fix for the loss chain the final review
/// caught — `t` disable, Enter (now reopens the disabled art, not blank —
/// I2), edit, `s` save used to write `<name>.png` *beside* the still-present
/// `.off`, so `scan_library` reported the name in both maps and the next `t`
/// clobbered the `.off` backup with whatever had just been saved. `Save`
/// below retires a stale `.off` the moment it writes an enabled copy, and
/// `Enable`/`Disable` each refuse — warning rather than clobbering — if
/// their destination is already occupied, which closes the class even for a
/// pair of files left in that state by a build that shipped before this fix.
fn apply_sprite_write(dir: &Path, name: &str, op: SpriteOp) -> WriteOutcome {
    let path = dir.join(format!("{name}.png"));
    let off_path = dir.join(format!("{name}.png.off"));
    match op {
        SpriteOp::Save(canvas) => {
            if let Err(e) = canvas_to_png(&canvas, &path) {
                warn!("sprite `{name}` failed to save: {e}");
                return WriteOutcome::Failed;
            }
            // Retires a stale disabled backup under the same name — see this
            // function's own doc comment. Best-effort: a backup that is
            // already gone (the common case) is not a failure.
            if off_path.exists()
                && let Err(e) = std::fs::remove_file(&off_path)
            {
                warn!(
                    "sprite `{name}` saved, but its stale `.png.off` backup could \
                     not be cleared: {e}"
                );
            }
            WriteOutcome::Reload
        }
        SpriteOp::Enable => {
            if path.exists() {
                warn!(
                    "sprite `{name}` failed to enable: `{name}.png` already exists; \
                     leaving `{name}.png.off` in place rather than overwriting it"
                );
                return WriteOutcome::Failed;
            }
            if let Err(e) = std::fs::rename(&off_path, &path) {
                warn!("sprite `{name}` failed to enable: {e}");
                return WriteOutcome::Failed;
            }
            WriteOutcome::Reload
        }
        SpriteOp::Disable => {
            if off_path.exists() {
                warn!(
                    "sprite `{name}` failed to disable: `{name}.png.off` already exists; \
                     leaving `{name}.png` in place rather than overwriting it"
                );
                return WriteOutcome::Failed;
            }
            if let Err(e) = std::fs::rename(&path, &off_path) {
                warn!("sprite `{name}` failed to disable: {e}");
                return WriteOutcome::Failed;
            }
            WriteOutcome::Disabled
        }
    }
}

/// Drains `App::take_sprite_writes`, performs the write or the rename each
/// one asks for (`apply_sprite_write`, above), and — reusing `load`'s own
/// upload path rather than a second one — pushes a `Save`/`Enable`'s reload
/// onto `Sprites::pending` so `register` (ordered directly after this, in
/// `PreUpdate`) can land it in the table this same frame.
///
/// A `Disable` never reaches `pending` at all: the file is gone from under
/// the name the instant the rename lands, so there is nothing left to
/// (re)load — it takes the name back out of the table itself, through
/// `SpriteTable::remove`, so a disabled sprite stops drawing on the same
/// frame the toggle was pressed rather than lagging a load cycle behind.
///
/// The library is rescanned and reinstalled once, after every write in the
/// batch, rather than per write: the picker and editor only ever read it on
/// the frame they're drawn, so one rescan per drained batch is enough to
/// keep both from going stale, at a fraction of the cost of one rescan per
/// op.
pub fn drain_writes(
    asset_server: Res<AssetServer>,
    mut frontend: ResMut<crate::Frontend>,
    mut sprites: ResMut<Sprites>,
) {
    let writes = frontend.app.take_sprite_writes();
    if writes.is_empty() {
        return;
    }
    let dir = frontend.app.assets_dir().join("sprites");
    for write in writes {
        let name = write.name;
        match apply_sprite_write(&dir, &name, write.op) {
            WriteOutcome::Reload => {
                let handle = asset_server
                    .load_builder()
                    .with_settings(|settings: &mut ImageLoaderSettings| {
                        settings.sampler = ImageSampler::nearest();
                    })
                    .load(format!("sprites/{name}.png"));
                sprites.pending.push((name, handle));
            }
            WriteOutcome::Disabled => {
                // `Arc::make_mut` for `register`'s reason: the renderer may
                // be holding a clone of the old table from this frame.
                Arc::make_mut(&mut sprites.table).remove(&name);
            }
            WriteOutcome::Failed => {}
        }
    }
    let (enabled, disabled) = scan_library(&dir);
    frontend.app.install_sprite_library(enabled, disabled);
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

    /// A fresh, unique scratch directory under the OS temp dir — never
    /// `assets/sprites/`. `tag` keeps two tests' directories from ever
    /// colliding even when run in parallel, the same disambiguator
    /// `an_at_prefixed_stem_is_never_scanned` inlines by hand above.
    fn codec_test_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "feral_processes_gui_sprites_codec_{tag}_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A canvas with a few painted cells — enough to exercise more than one
    /// palette index and a still-transparent corner.
    fn a_sprite_canvas() -> Canvas {
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

    /// `scan_library` reads the disabled *art* off a `.png.off` rename, and
    /// the same file must be invisible to `scan_sprite_dir` — the loader's
    /// own scan — without that scan needing to know `.off` exists at all:
    /// its plain `extension() == "png"` filter already excludes it.
    ///
    /// The `.off` file is a real PNG renamed, not garbage bytes — I2's fix
    /// means `scan_library` now decodes it (through `png_to_canvas`'s
    /// guessed-format read, since the real extension no longer says `png`),
    /// so this pins the decoded pixels match what was written, not just
    /// that the name was noticed.
    #[test]
    fn a_disabled_file_is_absent_from_the_loader_scan_and_present_in_the_librarys_disabled_set() {
        let dir = codec_test_dir("disabled");
        canvas_to_png(&a_sprite_canvas(), &dir.join("on_subject.png")).unwrap();
        let off_canvas = a_sprite_canvas();
        canvas_to_png(&off_canvas, &dir.join("off_subject.png")).unwrap();
        std::fs::rename(dir.join("off_subject.png"), dir.join("off_subject.png.off")).unwrap();

        let loader_names = scan_sprite_dir(&dir);
        let (enabled, disabled) = scan_library(&dir);

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(
            loader_names,
            vec!["on_subject".to_string()],
            "a .png.off file's extension is \"off\", not \"png\" — already invisible to the \
             loader's own filter, with no change to scan_sprite_dir"
        );
        assert!(enabled.contains_key("on_subject"));
        assert_eq!(
            disabled.get("off_subject"),
            Some(&off_canvas),
            "a disabled sprite's pixels must decode to what was written, not just be noticed by name"
        );
        assert!(
            !enabled.contains_key("off_subject"),
            "a disabled sprite's pixels are not installed as art the map may draw"
        );
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

    /// **I1 + I2, the exact sequence the final review's I1 finding walks
    /// through: `t` (disable), Enter (open), draw, `s` (save), `t` (disable
    /// again).** Before this task's fix that sequence destroyed the
    /// original art: Enter opened a blank canvas instead of the disabled
    /// art (I2), so the player's `s` wrote a fresh file *beside* the still-
    /// present `.png.off` backup, and the final `t` clobbered that backup
    /// with the new drawing via a bare `fs::rename` (I1) — the original was
    /// gone with no warning and no way back, on a screen whose entire
    /// subject is unbacked-up work.
    ///
    /// This test drives `apply_sprite_write` and `scan_library` through the
    /// same steps a player's keystrokes produce (`App::handle_sprite_
    /// picker_key`'s `t`/`Enter`, `App::handle_sprite_editor_key`'s `s`) and
    /// checks the property that actually matters at each one: Enter's
    /// fallback shows the *real* art, not blank; the fold at Save retires
    /// the stale backup so nothing is left for a later Disable to clobber;
    /// and the art recoverable at the end is exactly what was drawn — never
    /// silently lost, never silently reverted.
    #[test]
    fn the_t_enter_draw_s_t_sequence_does_not_silently_destroy_the_original_art() {
        let dir = codec_test_dir("i1_i2_sequence");
        let original = a_sprite_canvas();
        canvas_to_png(&original, &dir.join("subject.png")).unwrap();

        // `t`: disable. The art moves to `.png.off`, unmodified.
        assert_eq!(
            apply_sprite_write(&dir, "subject", SpriteOp::Disable),
            WriteOutcome::Disabled
        );
        assert!(!dir.join("subject.png").exists());
        assert!(dir.join("subject.png.off").exists());

        // Enter: `handle_sprite_picker_key`'s real fallback is `sprite_
        // library.get(name).or_else(|| sprite_disabled.get(name))` —
        // `scan_library`'s `disabled` map is what feeds that second half,
        // so reading it back here is the same lookup the picker makes.
        // I2's fix is that this is `original`, not a blank canvas.
        let (_, disabled) = scan_library(&dir);
        let opened = disabled
            .get("subject")
            .cloned()
            .expect("I2: Enter on an Off subject must find its art, not open blank");
        assert_eq!(
            opened, original,
            "the editor must open on the art that was disabled, not a blank canvas"
        );

        // Draw over it — a real, visible edit, made with the original
        // still on screen (not the blind edit the blank-canvas bug invited).
        let mut edited = opened;
        edited.set(1, 1, 3);
        assert_ne!(
            edited, original,
            "the fixture must actually change something"
        );

        // `s`: save. I1's fold must retire the stale `.png.off` backup the
        // instant an enabled copy lands, or the next disable has two
        // different files to choose between.
        assert_eq!(
            apply_sprite_write(&dir, "subject", SpriteOp::Save(edited.clone())),
            WriteOutcome::Reload
        );
        assert!(dir.join("subject.png").exists());
        assert!(
            !dir.join("subject.png.off").exists(),
            "I1: saving an enabled copy must retire the stale disabled backup, \
             or scan_library reports this name in both maps"
        );

        // `t`: disable again. With the invariant held, there is nothing
        // left under `subject.png.off` to clobber.
        assert_eq!(
            apply_sprite_write(&dir, "subject", SpriteOp::Disable),
            WriteOutcome::Disabled
        );

        let (enabled_final, disabled_final) = scan_library(&dir);
        assert!(
            !enabled_final.contains_key("subject"),
            "disabled means disabled — the map must not still list it as on"
        );
        assert_eq!(
            disabled_final.get("subject"),
            Some(&edited),
            "the final recoverable art must be exactly the edit that was saved — \
             not the stale original, and not lost"
        );
    }

    /// `Enable`/`Disable` refuse rather than clobber when their destination
    /// is already occupied — I1's defensive half, for a pair of files a
    /// pre-fix build could have left in that state on disk. The source file
    /// stays exactly as it was; only a warning is logged.
    #[test]
    fn disable_refuses_to_clobber_an_existing_off_backup() {
        let dir = codec_test_dir("disable_refuses");
        let current = a_sprite_canvas();
        canvas_to_png(&current, &dir.join("subject.png")).unwrap();
        let stale_backup = Canvas::new(ICON_SIZE);
        canvas_to_png(&stale_backup, &dir.join("stale.png")).unwrap();
        std::fs::rename(dir.join("stale.png"), dir.join("subject.png.off")).unwrap();

        let outcome = apply_sprite_write(&dir, "subject", SpriteOp::Disable);

        let enabled_survived = png_to_canvas(&dir.join("subject.png"));
        let backup_untouched = png_to_canvas(&dir.join("subject.png.off"));
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(outcome, WriteOutcome::Failed);
        assert_eq!(
            enabled_survived,
            Some(current),
            "the enabled copy must survive a refused disable"
        );
        assert_eq!(
            backup_untouched,
            Some(stale_backup),
            "the existing backup must be untouched, not clobbered"
        );
    }

    /// `Enable`'s own mirror of the test above.
    #[test]
    fn enable_refuses_to_clobber_an_existing_enabled_file() {
        let dir = codec_test_dir("enable_refuses");
        let current = a_sprite_canvas();
        canvas_to_png(&current, &dir.join("subject.png")).unwrap();
        let stale_backup = Canvas::new(ICON_SIZE);
        canvas_to_png(&stale_backup, &dir.join("stale.png")).unwrap();
        std::fs::rename(dir.join("stale.png"), dir.join("subject.png.off")).unwrap();

        let outcome = apply_sprite_write(&dir, "subject", SpriteOp::Enable);

        let enabled_untouched = png_to_canvas(&dir.join("subject.png"));
        let backup_survived = png_to_canvas(&dir.join("subject.png.off"));
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(outcome, WriteOutcome::Failed);
        assert_eq!(
            enabled_untouched,
            Some(current),
            "the enabled copy must be untouched, not clobbered"
        );
        assert_eq!(
            backup_survived,
            Some(stale_backup),
            "the backup must survive a refused enable"
        );
    }

    /// **M7, final review.** Key repeat on `t` can queue the same `Disable`
    /// twice inside one frame, before `sprite_library`/`sprite_disabled`
    /// have a chance to reflect the first one — `handle_sprite_picker_key`
    /// reads `subject.art` live and queues without touching that state
    /// itself. Not a new fix: I1's clobber-refusal already closes the harm,
    /// so this pins the property rather than changing behaviour — the first
    /// write in the batch lands, the second finds its destination already
    /// occupied and is refused, and the art is intact either way.
    #[test]
    fn a_doubled_disable_in_one_batch_is_refused_not_clobbered() {
        let dir = codec_test_dir("doubled_disable");
        let art = a_sprite_canvas();
        canvas_to_png(&art, &dir.join("subject.png")).unwrap();

        let first = apply_sprite_write(&dir, "subject", SpriteOp::Disable);
        let second = apply_sprite_write(&dir, "subject", SpriteOp::Disable);

        let recovered = png_to_canvas(&dir.join("subject.png.off"));
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(first, WriteOutcome::Disabled);
        assert_eq!(
            second,
            WriteOutcome::Failed,
            "the second of a doubled toggle must be refused, not applied twice"
        );
        assert_eq!(
            recovered,
            Some(art),
            "the art must be intact after the doubled toggle"
        );
    }
}
