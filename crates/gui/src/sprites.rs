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

use bevy::asset::LoadState;
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use crate::paint::SpriteTable;

/// The sprites the map looks for, by the name the renderer asks for.
///
/// A list rather than a directory walk: the renderer asks for a *name*, so
/// something has to say which names exist, and a constant here is one line
/// per sprite against a filesystem scan that would have to run before the
/// asset server is available anyway. This is the minimum proof — when
/// sprites become a `sprite:` field on species and structures, the names
/// come from the asset files and this list goes away.
const SPRITES: &[&str] = &["player"];

/// Where the loaded sprites live between the asset server and the renderer.
#[derive(Resource, Default)]
pub struct Sprites {
    /// Still loading, or waiting to be handed to egui.
    pending: Vec<(&'static str, Handle<Image>)>,
    /// What the renderer draws from. Refcounted so the per-frame `Painter`
    /// costs one atomic bump rather than a copy.
    table: Arc<SpriteTable>,
}

impl Sprites {
    pub fn table(&self) -> Arc<SpriteTable> {
        Arc::clone(&self.table)
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
    use super::SPRITES;

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
