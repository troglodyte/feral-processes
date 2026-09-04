//! `Mode::SpritePicker` and `Mode::SpriteEditor` — every name the map can
//! draw a sprite for, both gates that keep the whole dev sprite editor out
//! of a player's build, and the editing screen itself. See
//! `docs/superpowers/specs/2026-09-04-dev-sprite-editor-design.md`.
//!
//! `SpriteEditor` composes a `CanvasEditor` exactly as `IconEditor` does —
//! see that module's doc comment for why the mechanics are shared and the
//! sink is not. What is this sink's own: which subject it opened for, the
//! `[g]`/`[s]` keys on top of the shared table, the write cue, and the
//! mouse entry point. Drawing is `Mode::SpriteEditor`'s Task 7, not this
//! module's.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use feral_processes_engine::DEFAULT_PLAYER_SPRITE;
use feral_processes_engine::abilities::AbilityDb;
use feral_processes_engine::components::GlyphColor;
use feral_processes_engine::icon::{Canvas, SPRITE_PALETTE};
use feral_processes_engine::species::SpeciesDb;
use feral_processes_engine::structures::StructureDb;

use crate::app::canvas_editor::{CanvasEditor, CanvasKey, CanvasView};
use crate::{App, GameKey, Mode};

/// The sprite canvas's edge — always 16, never a brush-dependent size (the
/// brush is a footprint over this fixed grid, not a second canvas format;
/// see the design doc's "The canvas is 16x16 always"). `ICON_SIZE` is the
/// same 16, named for the *pixel* geometry the icon editor draws at; reused
/// here rather than a second `pub const SPRITE_EDGE = 16` because the two
/// really are one number, documented as such in `engine::icon`'s own doc
/// comment ("The *sprite* is 16x16 (`ICON_SIZE`)").
const SPRITE_EDGE: usize = feral_processes_engine::ICON_SIZE;

/// The name `assets/sprites/anchor.png` is looked up under — the one place
/// this string is authored beside `crates/gui/src/render/base.rs`'s
/// `Some("anchor")`. There is no `sprite_name()` to call for it: the anchor
/// is not a def, it is one hand-spawned entity (`components::BaseAnchor`).
const ANCHOR_SPRITE_NAME: &str = "anchor";

/// The anchor's own glyph — `Glyph { ch: '#', .. }`, written once at spawn
/// in `Game::new_with`/`Game::load` (`crates/engine/src/game/lifecycle.rs`)
/// and not exported as a constant there, since nothing else needs it back.
const ANCHOR_GLYPH: char = '#';

/// The anchor's own colour — the same `GlyphColor::Gray` written once at the
/// same spawn site as `ANCHOR_GLYPH`. Unlike the player, the anchor's tile is
/// never re-coloured by a role: `render/base.rs` reads its glyph colour off
/// `EntityView::color` exactly as it would for a species or a structure, so
/// this is genuinely "what render/base.rs draws it with," not a second
/// guess at it.
const ANCHOR_GLYPH_COLOR: GlyphColor = GlyphColor::Gray;

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

/// `App::sprite_subjects`' cached static half, before `art` is attached —
/// named so `App::sprite_static_subjects`'s field type in `lib.rs` isn't a
/// bare four-tuple clippy's `type_complexity` lint flags on sight.
pub(crate) type StaticSpriteSubject = (String, String, char, Option<GlyphColor>);

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
    /// The def's own `GlyphColor` — `SpeciesDef::color` / `StructureDef::
    /// color` — or `None` for the one subject that doesn't have one.
    ///
    /// `player` is `None` rather than the `GlyphColor::Cyan` the player
    /// entity happens to spawn with: the HUD seam's rule is that the
    /// player's `@` wears the **`PLAYER` role colour**, not an authored
    /// hue, and `render/base.rs` never reads `glyph_color(GlyphColor::Cyan)`
    /// for it — reading `Cyan` here and drawing it through the same table
    /// every other subject uses would show the picker a colour the map
    /// never actually paints. `anchor` **is** `Some(GlyphColor::Gray)`: it
    /// has no role-based override, so its true drawn colour is an ordinary
    /// glyph-table lookup like any species or structure's.
    pub color: Option<GlyphColor>,
    pub art: SpriteArt,
}

/// One `Mode::SpriteEditor` session — `CanvasEditor`'s shared mechanics
/// plus which subject this is. The subject is a name rather than an index
/// into `App::sprite_subjects()`: that list is rebuilt (and re-sorted, once
/// a save changes an `art` state) on every read, so an index into it would
/// go stale the moment this editor's own save landed.
pub(crate) struct SpriteEditor {
    editor: CanvasEditor,
    subject: String,
}

impl SpriteEditor {
    /// Opens on `canvas` — `handle_sprite_picker_key`'s own resolution of
    /// `App::sprite_library` (enabled) then `App::sprite_disabled` (off),
    /// falling back to a blank 16x16 canvas only when the subject has never
    /// had art at all. Blank is a legitimate opening state here, unlike the
    /// player's own `@`: nothing filters it away before it can be saved.
    fn open(subject: String, canvas: Canvas) -> SpriteEditor {
        SpriteEditor {
            editor: CanvasEditor::open(canvas, SPRITE_PALETTE.len() as u8),
            subject,
        }
    }

    /// What the screen draws — `CanvasEditor`'s own view, the subject name
    /// for a header, and the palette a `CanvasView`'s bare indices need to
    /// become colour.
    fn view(&self) -> SpriteEditorView {
        SpriteEditorView {
            canvas: self.editor.view(),
            subject: self.subject.clone(),
            palette: &SPRITE_PALETTE,
        }
    }
}

/// What `Mode::SpriteEditor` draws.
pub struct SpriteEditorView {
    pub canvas: CanvasView,
    pub subject: String,
    pub palette: &'static [(u8, u8, u8)],
}

/// One cue for the frontend to act on. app-core queues it and forgets —
/// `App::take_sounds`'s pattern, in the direction of a file instead of a
/// speaker. **app-core never opens a file and never learns what a PNG
/// is**: `Save` carries exactly the `Canvas` the loader already knows how
/// to encode from, and `Enable`/`Disable` carry nothing because the toggle
/// is a rename on the name alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteWrite {
    pub name: String,
    pub op: SpriteOp,
}

/// What a queued `SpriteWrite` asks the frontend to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpriteOp {
    /// Write `assets/sprites/<name>.png` from this canvas, replacing
    /// whatever was there.
    Save(Canvas),
    /// Rename `<name>.png.off` back to `<name>.png`.
    Enable,
    /// Rename `<name>.png` to `<name>.png.off`.
    Disable,
}

/// Where a pointer landed on `Mode::SpriteEditor`, already resolved to a
/// cell or a swatch by the gui — never a pixel. The gui owns the canvas and
/// swatch rects (it draws them), so it tests the pointer against those
/// itself; app-core never receives a pixel and never learns the rects
/// exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerHit {
    Cell(u8, u8),
    Swatch(u8),
}

/// Which mouse button a `PointerHit` was reported for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    /// Paints index 0 — erase, the same thing `Backspace` already means on
    /// this editor.
    Secondary,
}

/// Where in a click-or-drag gesture a `PointerHit` was reported.
///
/// **A whole drag is one undo entry, not one per cell.** `Down` opens a
/// stroke (`CanvasEditor::begin_stroke`, one snapshot taken before anything
/// is known to change), `Up` closes it (`end_stroke`), and `Drag` neither
/// opens nor closes one — every cell the gui reports while the button is
/// held lands inside the same stroke `Down` opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerPhase {
    Down,
    Drag,
    Up,
}

impl App {
    /// Marks that a sprite dir exists for this build to read and write
    /// PNGs through — installed by the launcher unconditionally within a
    /// checkout, mirroring `install_dev_templates`: the flag alone decides
    /// visibility from there, so installing this only when
    /// `FERAL_DEV_SPRITES` is set would make one flag mean two things. An
    /// installed build has no checkout to resolve this from at all, which
    /// is the other half of the gate — see `sprite_forge_enabled`.
    ///
    /// **Takes no path** (M1, final review): the real directory every
    /// read/write actually uses is `assets_dir().join("sprites")`, derived
    /// fresh at each site in `crates/gui/src/sprites.rs` — a path stored
    /// here was never read back as one, only tested with `.is_some()`, so
    /// it could silently name a different directory than the one really in
    /// use (e.g. under `--assets <override>`) with nothing to catch it.
    pub fn install_sprite_dir(&mut self) {
        self.sprite_dir_installed = true;
    }

    /// The frontend's decoded, quantised sprite library — `SpriteArt::On`
    /// for a name in `enabled`, `SpriteArt::Off` for one in `disabled`,
    /// `SpriteArt::None` for neither. app-core does no file I/O of its own;
    /// a real frontend fills both maps by scanning `assets/sprites/` (Task
    /// 8's job, not this one's). `disabled` is decoded pixels, not bare
    /// names, so `Enter` on an `Off` subject can reopen the art itself — see
    /// `sprite_disabled`'s own doc comment.
    pub fn install_sprite_library(
        &mut self,
        enabled: HashMap<String, Canvas>,
        disabled: HashMap<String, Canvas>,
    ) {
        self.sprite_library = enabled;
        self.sprite_disabled = disabled;
    }

    /// Whether the main menu offers the sprite picker at all — the flag
    /// *and* a sprite dir installed, so a shipped build (no checkout, so no
    /// `install_sprite_dir` call ever lands) cannot offer a screen whose
    /// entire purpose is writing into a source tree it does not have.
    pub fn sprite_forge_enabled(&self) -> bool {
        self.sprite_forge_flag && self.sprite_dir_installed
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
            .map(|(name, label, glyph, color)| {
                let art = if self.sprite_library.contains_key(name) {
                    SpriteArt::On
                } else if self.sprite_disabled.contains_key(name) {
                    SpriteArt::Off
                } else {
                    SpriteArt::None
                };
                SpriteSubject {
                    name: name.clone(),
                    label: label.clone(),
                    glyph: *glyph,
                    color: *color,
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
    fn load_static_sprite_subjects(assets_dir: &Path) -> Vec<StaticSpriteSubject> {
        let (abilities, _) = AbilityDb::load_dir(&assets_dir.join("abilities")).unwrap_or_default();
        let (species, _) =
            SpeciesDb::load_dir(&assets_dir.join("species"), &abilities).unwrap_or_default();
        let (structures, _) =
            StructureDb::load_dir(&assets_dir.join("structures")).unwrap_or_default();

        let mut by_name: BTreeMap<String, (String, char, Option<GlyphColor>)> = BTreeMap::new();
        for def in species.all() {
            by_name.insert(
                def.sprite_name().to_string(),
                (def.name.clone(), def.glyph, Some(def.color)),
            );
        }
        for def in structures.all() {
            by_name.insert(
                def.sprite_name().to_string(),
                (def.name.clone(), def.glyph, Some(def.color)),
            );
        }
        // `None`, not `Some(GlyphColor::Cyan)` — see `SpriteSubject::color`'s
        // own doc comment for why the player has no authored hue at all.
        by_name.insert(
            DEFAULT_PLAYER_SPRITE.to_string(),
            ("Player".to_string(), PLAYER_GLYPH, None),
        );
        by_name.insert(
            ANCHOR_SPRITE_NAME.to_string(),
            ("Anchor".to_string(), ANCHOR_GLYPH, Some(ANCHOR_GLYPH_COLOR)),
        );

        by_name
            .into_iter()
            .map(|(name, (label, glyph, color))| (name, label, glyph, color))
            .collect()
    }

    /// `Esc` backs all the way out to the main menu. `[t]` toggles a
    /// subject with art between `On` and `Off` (queuing the rename cue) and
    /// does nothing on one with none to toggle. `Enter` opens
    /// `Mode::SpriteEditor` on the highlighted subject — loading its
    /// installed canvas from `sprite_library` if it has one, falling back to
    /// `sprite_disabled` for an `Off` subject (**I2's fix**: the picker
    /// already says the art survived the toggle, so the one tool that can
    /// read it back must actually do so rather than opening blank), and
    /// only a genuinely art-less subject opens blank. Up/Down still scroll
    /// the list, the same read-only-screen idiom `handle_recipes_key` uses —
    /// `selected_index` is what both Enter and the arrows go through, so
    /// there is one place that owns the highlight.
    pub(crate) fn handle_sprite_picker_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::MainMenu;
            return;
        }
        let subjects = self.sprite_subjects();
        if key == GameKey::Char('t') {
            if let Some(subject) = subjects.get(self.menu_selected) {
                let op = match subject.art {
                    SpriteArt::On => Some(SpriteOp::Disable),
                    SpriteArt::Off => Some(SpriteOp::Enable),
                    SpriteArt::None => None,
                };
                if let Some(op) = op {
                    self.pending_sprite_writes.push(SpriteWrite {
                        name: subject.name.clone(),
                        op,
                    });
                }
            }
            return;
        }
        if let Some(idx) = self.selected_index(key, subjects.len()) {
            let subject = &subjects[idx];
            let canvas = self
                .sprite_library
                .get(&subject.name)
                .or_else(|| self.sprite_disabled.get(&subject.name))
                .cloned()
                .unwrap_or_else(|| Canvas::new(SPRITE_EDGE));
            self.sprite_editor = Some(SpriteEditor::open(subject.name.clone(), canvas));
            self.mode = Mode::SpriteEditor;
        }
    }

    /// What `Mode::SpriteEditor` draws, or `None` while it is not open.
    pub fn sprite_editor_view(&self) -> Option<SpriteEditorView> {
        self.sprite_editor.as_ref().map(SpriteEditor::view)
    }

    /// Drains every `SpriteWrite` queued since the last call —
    /// `App::take_sounds`'s seam: app-core queues and forgets, the frontend
    /// (Task 8) drains, performs the write or rename, and re-installs the
    /// library so the map updates without a restart.
    pub fn take_sprite_writes(&mut self) -> Vec<SpriteWrite> {
        std::mem::take(&mut self.pending_sprite_writes)
    }

    /// `Mode::SpriteEditor`'s own keys, on top of `CanvasEditor`'s shared
    /// table (arrows, `Space`/`Backspace`, `u`, `x`, `Tab`), taken back
    /// unhandled: `[g]` toggles the brush 1<->2, `[s]` queues a `Save`
    /// carrying the canvas as it stands, and `Esc` leaves for
    /// `Mode::SpritePicker` without queueing anything — a blank canvas is a
    /// legitimate save here (see the module doc comment), so `Esc` and
    /// `[s]` differ only in whether a cue is raised at all, never in what
    /// the cue would have carried.
    pub(crate) fn handle_sprite_editor_key(&mut self, key: GameKey) {
        let Some(sprite_editor) = &mut self.sprite_editor else {
            return;
        };
        match key {
            GameKey::Esc => {
                self.sprite_editor = None;
                self.mode = Mode::SpritePicker;
            }
            GameKey::Char('g') => {
                let next = if sprite_editor.editor.view().brush == 1 {
                    2
                } else {
                    1
                };
                sprite_editor.editor.set_brush(next);
            }
            GameKey::Char('s') => {
                let write = SpriteWrite {
                    name: sprite_editor.subject.clone(),
                    op: SpriteOp::Save(sprite_editor.editor.canvas().clone()),
                };
                self.pending_sprite_writes.push(write);
            }
            _ => {
                let _: CanvasKey = sprite_editor.editor.handle_key(key);
            }
        }
    }

    /// The mouse's one entry point — routed only while `Mode::SpriteEditor`
    /// is open, every other mode drops it silently, since nothing else in
    /// the game reads a pointer at all. `phase` governs the stroke
    /// (`PointerPhase`'s own doc comment); `hit` decides what happens at
    /// it — a `Cell` paints (the selected swatch on `Primary`, index 0 —
    /// erase — on `Secondary`), a `Swatch` selects.
    ///
    /// **`PointerHit::Swatch` carries `swatch_at`'s 0-based drawn
    /// position** (its own doc comment says so), while `pick_swatch` — and
    /// `CanvasView::selected` it writes — is 1-based (`FIRST_COLOUR = 1`;
    /// `draw_swatch_row` outlines the swatch where `selected == i + 1`). The
    /// `+ 1` below is that one conversion, made once at the seam neither
    /// side's own test could see: `swatch_at` correctly tests its own
    /// 0-based answer, `pick_swatch` correctly tests its own 1-based input,
    /// and nothing crossed the boundary between the two until this line
    /// existed. Get this wrong and the mouse selects the swatch to the left
    /// of the one it outlines, and the palette's last entry is unreachable.
    pub fn handle_pointer(&mut self, hit: PointerHit, button: PointerButton, phase: PointerPhase) {
        if self.mode != Mode::SpriteEditor {
            return;
        }
        let Some(sprite_editor) = &mut self.sprite_editor else {
            return;
        };
        if phase == PointerPhase::Down {
            sprite_editor.editor.begin_stroke();
        }
        match hit {
            PointerHit::Cell(x, y) => {
                let index = match button {
                    PointerButton::Primary => sprite_editor.editor.view().selected,
                    PointerButton::Secondary => 0,
                };
                // M5, final review: snap to the brush grid before painting,
                // or brush 2 anchors on whatever odd coordinate the pointer
                // happened to land on — see `snap_to_brush`'s own doc
                // comment.
                let (x, y) = sprite_editor.editor.snap_to_brush(x, y);
                sprite_editor.editor.paint_at(x, y, index);
            }
            PointerHit::Swatch(index) => sprite_editor.editor.pick_swatch(index + 1),
        }
        if phase == PointerPhase::Up {
            sprite_editor.editor.end_stroke();
        }
    }
}
