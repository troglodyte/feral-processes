//! `Mode::SpritePicker` — every name the map can draw a sprite for, and both
//! gates that keep the whole dev sprite editor out of a player's build. See
//! `docs/superpowers/specs/2026-09-04-dev-sprite-editor-design.md`.
//!
//! This module owns the list and the gate only. Drawing on a canvas is
//! `Mode::SpriteEditor`'s job — a later mode, not built yet — so
//! `handle_sprite_picker_key` below only scrolls and backs out.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use feral_processes_engine::DEFAULT_PLAYER_SPRITE;
use feral_processes_engine::abilities::AbilityDb;
use feral_processes_engine::icon::Canvas;
use feral_processes_engine::species::SpeciesDb;
use feral_processes_engine::structures::StructureDb;

use crate::{App, GameKey, Mode};

/// The name `assets/sprites/anchor.png` is looked up under — the one place
/// this string is authored beside `crates/gui/src/render/base.rs`'s
/// `Some("anchor")`. There is no `sprite_name()` to call for it: the anchor
/// is not a def, it is one hand-spawned entity (`components::BaseAnchor`).
const ANCHOR_SPRITE_NAME: &str = "anchor";

/// The anchor's own glyph — `Glyph { ch: '#', .. }`, written once at spawn
/// in `Game::new_with`/`Game::load` (`crates/engine/src/game/lifecycle.rs`)
/// and not exported as a constant there, since nothing else needs it back.
const ANCHOR_GLYPH: char = '#';

/// The player's own glyph off the map. Named here for `ANCHOR_GLYPH`'s
/// reason — see the HUD seam: "The player's `@` is a role and is read off
/// `is_player`."
const PLAYER_GLYPH: char = '@';

/// Whether `FERAL_DEV_SPRITES` was set when this `App` was built. Same
/// predicate as `dev_arena_enabled` and `dev_console_enabled` — one answer
/// to "is a dev flag set" is the rule `dev_console::dev_flag`'s doc comment
/// records, and two answers is drift this repo has already caught.
pub(crate) fn dev_sprite_forge_flag() -> bool {
    super::dev_console::dev_flag("FERAL_DEV_SPRITES")
}

/// Whether a subject in `App::sprite_subjects` has art, and if so whether
/// the map draws it. `Off` is not `None`: the art is still on disk, and the
/// picker's toggle is a rename (`<name>.png` <-> `<name>.png.off`), never a
/// delete — see `assets/sprites/README.md`'s naming rule and the design
/// doc's "Turning art off is a rename, not a delete".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpriteArt {
    /// `assets/sprites/<name>.png` has never been drawn.
    None,
    /// Art exists and the map draws it.
    On,
    /// Art exists on disk but is switched off — the glyph draws instead.
    Off,
}

/// One row of `Mode::SpritePicker`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteSubject {
    /// The sprite lookup key — `SpeciesDef::sprite_name()` /
    /// `StructureDef::sprite_name()`, **never** the def's own `id`: the
    /// optional `sprite:` override is exactly what decides which file the
    /// loader looks for, so two defs may legitimately share one name.
    pub name: String,
    /// The def's own display name, for a screen that reads better than a
    /// list of file stems.
    pub label: String,
    /// The glyph this subject draws in place of today, in its own palette
    /// hue — what the picker shows for a subject with no art yet.
    pub glyph: char,
    pub art: SpriteArt,
}

impl App {
    /// Where the picker and (eventually) the editor read and write PNGs —
    /// `assets/sprites/` in the checkout behind this build, or `None` if
    /// there isn't one. Installed by the launcher unconditionally within a
    /// checkout, mirroring `install_dev_templates`: the flag alone decides
    /// visibility from there, so installing this only when
    /// `FERAL_DEV_SPRITES` is set would make one flag mean two things. An
    /// installed build has no checkout to resolve this from at all, which
    /// is the other half of the gate — see `sprite_forge_enabled`.
    pub fn install_sprite_dir(&mut self, dir: PathBuf) {
        self.sprite_dir = Some(dir);
    }

    /// The frontend's decoded, quantised sprite library — `SpriteArt::On`
    /// for a name in `enabled`, `SpriteArt::Off` for one in `disabled`,
    /// `SpriteArt::None` for neither. app-core does no file I/O of its own;
    /// a real frontend fills both sets by scanning `assets/sprites/` (Task
    /// 8's job, not this one's).
    pub fn install_sprite_library(
        &mut self,
        enabled: HashMap<String, Canvas>,
        disabled: HashSet<String>,
    ) {
        self.sprite_library = enabled;
        self.sprite_disabled = disabled;
    }

    /// Whether the main menu offers the sprite picker at all — the flag
    /// *and* a sprite dir installed, so a shipped build (no checkout, so no
    /// `install_sprite_dir` call ever lands) cannot offer a screen whose
    /// entire purpose is writing into a source tree it does not have.
    pub fn sprite_forge_enabled(&self) -> bool {
        self.sprite_forge_flag && self.sprite_dir.is_some()
    }

    /// Every name the map can draw a sprite for: each species def, each
    /// structure def, and the two names hardcoded in Rust (`player` via
    /// `DEFAULT_PLAYER_SPRITE`, `anchor`). Sorted by `name` and
    /// de-duplicated on it.
    ///
    /// **Two halves with two different lifetimes.** The name/label/glyph
    /// triple is *static* — nothing in `assets/species`/`assets/structures`
    /// changes while a session runs — so it is parsed once, on first call,
    /// and cached in `sprite_static_subjects`; a `Mode::SpritePicker` draw
    /// call reads this every frame the screen is open, and re-parsing three
    /// asset directories that often would be dozens of `.ron` files loaded
    /// per second for as long as the picker stays up. `art`, in contrast,
    /// must stay live: it is what `SpriteEditor`'s save and toggle change,
    /// and Task 8's whole point is the map updating without a restart, so
    /// it is looked up in `sprite_library`/`sprite_disabled` fresh on every
    /// call. The cache still starts empty and is filled only on first use,
    /// so a session that never opens the picker parses nothing at all —
    /// the property a per-call parse was protecting, kept without the
    /// per-frame cost.
    ///
    /// `&mut self` rather than a `RefCell`-backed `&self`: `App` is wrapped
    /// in a bevy `Resource` (`crates/gui/src/lib.rs`'s `Frontend`), which
    /// requires `Sync`, and `RefCell` does not implement it — the frontend's
    /// own `frame` system already holds `App` through `ResMut<Frontend>`,
    /// so there is a genuine mutable borrow available at every call site.
    pub fn sprite_subjects(&mut self) -> Vec<SpriteSubject> {
        if self.sprite_static_subjects.is_none() {
            self.sprite_static_subjects = Some(Self::load_static_sprite_subjects(&self.assets_dir));
        }
        let static_subjects = self
            .sprite_static_subjects
            .as_ref()
            .expect("populated immediately above if it wasn't already");

        static_subjects
            .iter()
            .map(|(name, label, glyph)| {
                let art = if self.sprite_library.contains_key(name) {
                    SpriteArt::On
                } else if self.sprite_disabled.contains(name) {
                    SpriteArt::Off
                } else {
                    SpriteArt::None
                };
                SpriteSubject {
                    name: name.clone(),
                    label: label.clone(),
                    glyph: *glyph,
                    art,
                }
            })
            .collect()
    }

    /// The static half of `sprite_subjects`, parsed once. Every species def,
    /// every structure def, and the two hardcoded names, keyed on
    /// `sprite_name()` through a `BTreeMap` — which is the de-duplication
    /// *and* the "sort by name" rule in one structure.
    ///
    /// `SpeciesDb::all`/`StructureDb::all` are both already deterministic
    /// (each sorts its own defs), so which def wins a shared name is stable
    /// run to run: structures after species, and the two hardcoded names
    /// last of all.
    fn load_static_sprite_subjects(assets_dir: &Path) -> Vec<(String, String, char)> {
        let (abilities, _) = AbilityDb::load_dir(&assets_dir.join("abilities")).unwrap_or_default();
        let (species, _) =
            SpeciesDb::load_dir(&assets_dir.join("species"), &abilities).unwrap_or_default();
        let (structures, _) =
            StructureDb::load_dir(&assets_dir.join("structures")).unwrap_or_default();

        let mut by_name: BTreeMap<String, (String, char)> = BTreeMap::new();
        for def in species.all() {
            by_name.insert(def.sprite_name().to_string(), (def.name.clone(), def.glyph));
        }
        for def in structures.all() {
            by_name.insert(def.sprite_name().to_string(), (def.name.clone(), def.glyph));
        }
        by_name.insert(
            DEFAULT_PLAYER_SPRITE.to_string(),
            ("Player".to_string(), PLAYER_GLYPH),
        );
        by_name.insert(
            ANCHOR_SPRITE_NAME.to_string(),
            ("Anchor".to_string(), ANCHOR_GLYPH),
        );

        by_name
            .into_iter()
            .map(|(name, (label, glyph))| (name, label, glyph))
            .collect()
    }

    /// `Esc` is the whole of this screen for now — `[t]`'s toggle and
    /// `Enter`'s dive into `Mode::SpriteEditor` arrive with that mode.
    /// Up/Down still scroll the list, the same read-only-screen idiom
    /// `handle_recipes_key` uses.
    pub(crate) fn handle_sprite_picker_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::MainMenu;
            return;
        }
        let rows = self.sprite_subjects().len();
        self.scroll(key, rows);
    }
}
