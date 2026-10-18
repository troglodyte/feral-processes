//! `Mode::SpritePicker` and `Mode::SpriteEditor` — every name the map can
//! draw a sprite for, both gates that keep the whole dev sprite editor out
//! of a player's build, and the editing screen itself. See
//! `docs/superpowers/specs/2026-09-04-dev-sprite-editor-design.md`.
//!
//! The Forge is a host of `sprite_editor::SpriteEditor`: what is this
//! sink's own is which subject it opened for, the `[s]`/`Esc` keys the
//! editor leaves unhandled, the write cue, and the mouse entry point.
//! Drawing is `Mode::SpriteEditor`'s Task 7, not this module's.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use feral_processes_engine::DEFAULT_PLAYER_SPRITE;
use feral_processes_engine::abilities::AbilityDb;
use feral_processes_engine::components::GlyphColor;
use feral_processes_engine::icon::{Canvas, GREY_RAMP_LEN};
use feral_processes_engine::species::SpeciesDb;
use feral_processes_engine::structures::StructureDb;
use feral_processes_engine::tactical::props::PropDb;

use crate::app::sprite_editor::{
    DEFAULT_SPRITE_FRAME_MS, EditorChrome, FORGE_BUTTONS, PointerButton, PointerHit, PointerPhase,
    SpriteEditor, SpriteEditorView, SubjectTint,
};
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

/// A floor finish's placeholder glyph in the picker — never actually drawn
/// on the map, since a finish is a fill and an edge over a `Platform` cell
/// rather than a glyph substituting for one the way `sprite` does. Chosen to
/// read as "ground" rather than as a stray character.
const FLOOR_SPRITE_GLYPH: char = '_';

/// The glyph a prop subject previews against, `draw_prop`'s breakable class.
const PROP_SPRITE_GLYPH: char = '%';

/// Whether `FERAL_DEV_SPRITES` — or the `FERAL_DEV` master switch — was set
/// when this `App` was built. Same predicate as `dev_arena_enabled` and
/// `dev_console_enabled` — one answer to "is a dev tool on" is the rule
/// `dev_console::dev_tool_flag`'s doc comment records, and two answers is
/// drift this repo has already caught.
pub(crate) fn dev_sprite_forge_flag() -> bool {
    super::dev_console::dev_tool_flag("FERAL_DEV_SPRITES")
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
pub(crate) type StaticSpriteSubject = (String, String, char, SubjectTint);

/// One row of `Mode::SpritePicker`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteSubject {
    /// The sprite lookup key — `SpeciesDef::sprite_name()` /
    /// `StructureDef::sprite_name()` / `FloorDef::sprite_name()`, **never**
    /// the def's own `id`: the optional `sprite:` override is exactly what
    /// decides which file the loader looks for, so two defs may legitimately
    /// share one name.
    pub name: String,
    /// The def's own display name, for a screen that reads better than a
    /// list of file stems.
    pub label: String,
    /// The glyph this subject draws in place of today, in its own palette
    /// hue — what the picker shows for a subject with no art yet.
    pub glyph: char,
    /// What this subject's preview is multiplied against.
    ///
    /// `player` is `Glyph(None)` rather than `Glyph(Some(GlyphColor::Cyan))`,
    /// the colour the player entity happens to spawn with: the HUD seam's
    /// rule is that the player's `@` wears the **`PLAYER` role colour**, not
    /// an authored hue, and `render/base.rs` never reads
    /// `glyph_color(GlyphColor::Cyan)` for it — reading `Cyan` here and
    /// drawing it through the same table every other subject uses would show
    /// the picker a colour the map never actually paints. `anchor` **is**
    /// `Glyph(Some(GlyphColor::Gray))`: it has no role-based override, so its
    /// true drawn colour is an ordinary glyph-table lookup like any species
    /// or structure's.
    pub tint: SubjectTint,
    pub art: SpriteArt,
}

/// A sprite the frontend found on disk: its frames and which file variant
/// holds them. The variant is a fact about the installed art, carried with
/// it, so the editor never has to guess which file a save belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledSprite {
    /// One or two 16x16 canvases, never empty.
    pub frames: Vec<Canvas>,
    /// How long each frame shows; meaningless for a single frame.
    pub frame_ms: u32,
    /// `<name>.colour.png` rather than `<name>.png` — drawn with its tint's
    /// hue dropped (`assets/sprites/README.md`).
    pub full_colour: bool,
}

/// Whether art with no file yet must save as the colour variant: a hued
/// sprite saved as plain `.png` would be multiplied by the species tint and
/// go muddy. Index 0 is transparent and `1..=9` the grey ramp, so any index
/// past 9 is a hue.
fn saves_as_full_colour(frames: &[Canvas]) -> bool {
    frames.iter().any(|canvas| {
        let edge = canvas.edge();
        (0..edge).any(|y| (0..edge).any(|x| canvas.get(x, y) > GREY_RAMP_LEN))
    })
}

/// One `Mode::SpriteEditor` session — the shared editor plus which subject
/// this is. The subject is a name rather than an index into
/// `App::sprite_subjects()`: that list is rebuilt (and re-sorted, once a save
/// changes an `art` state) on every read, so an index into it would go stale
/// the moment this session's own save landed.
pub(crate) struct ForgeSession {
    pub(crate) editor: SpriteEditor,
    pub(crate) subject: String,
    /// The variant the art was opened with; `None` is new art, whose variant
    /// is decided by what is drawn on it.
    pub(crate) full_colour: Option<bool>,
}

impl ForgeSession {
    /// The variant a save writes: the one the art was loaded as, else what
    /// the frames need.
    fn full_colour(&self) -> bool {
        self.full_colour
            .unwrap_or_else(|| saves_as_full_colour(&self.editor.frames()))
    }
}

/// One cue for the frontend to act on. app-core queues it and forgets —
/// `App::take_sounds`'s pattern, in the direction of a file instead of a
/// speaker. **app-core never opens a file and never learns what a PNG
/// is**: `Save` carries exactly the `Canvas` the loader already knows how
/// to encode from, and every op carries the variant (`full_colour`) so the
/// frontend derives the one file it touches from `(name, variant)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteWrite {
    pub name: String,
    pub op: SpriteOp,
}

/// What a queued `SpriteWrite` asks the frontend to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpriteOp {
    /// Write `assets/sprites/<name>.png` (`<name>.colour.png` when
    /// `full_colour`) from these frames side by side, replacing whatever was
    /// there. `frame_ms` is only written for two or more frames.
    Save {
        frames: Vec<Canvas>,
        frame_ms: u32,
        full_colour: bool,
    },
    /// Rename `<name>.png.off` back to `<name>.png` (or the colour pair).
    Enable { full_colour: bool },
    /// Rename `<name>.png` to `<name>.png.off` (or the colour pair).
    Disable { full_colour: bool },
}

impl SpriteOp {
    pub fn full_colour(&self) -> bool {
        match self {
            SpriteOp::Save { full_colour, .. }
            | SpriteOp::Enable { full_colour }
            | SpriteOp::Disable { full_colour } => *full_colour,
        }
    }
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
        enabled: HashMap<String, InstalledSprite>,
        disabled: HashMap<String, InstalledSprite>,
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

    /// The one door into `Mode::SpritePicker`, shared by the main menu's
    /// `[D]` row and the dev console's `Open Sprite Forge` row.
    ///
    /// **`menu_origin` is what tells Esc which door it came through** —
    /// `handle_group_menu_key`'s seam, reused rather than a second slot,
    /// because the two doors stand on opposite sides of a run: the menu
    /// door has no game behind it and the console door has a map the player
    /// is standing on. Written here rather than at each caller so the
    /// picker's Esc has exactly one rule to follow.
    ///
    /// The refusal is reachable only through the console: the main menu
    /// builds its key list off `sprite_forge_enabled` and never offers the
    /// row, where the keypad hides no row at all (see `DEV_ROWS`).
    pub(crate) fn open_sprite_forge(&mut self) {
        if !self.sprite_forge_enabled() {
            self.refuse(
                "Sprite Forge is off: set FERAL_DEV_SPRITES and run from a checkout.".to_string(),
            );
            return;
        }
        self.status_line = None;
        self.menu_origin = Some(self.mode);
        self.mode = Mode::SpritePicker;
    }

    /// Every name the map can draw a sprite for: each species def, each
    /// structure def, each floor finish, and the two names hardcoded in Rust
    /// (`player` via `DEFAULT_PLAYER_SPRITE`, `anchor`). Sorted by `name`
    /// and de-duplicated on it.
    ///
    /// **Two halves with two different lifetimes.** The name/label/glyph
    /// triple is *static* — nothing in `assets/species`/`assets/structures`/
    /// `assets/floors` changes while a session runs — so it is parsed once,
    /// on first call,
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
            .map(|(name, label, glyph, tint)| {
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
                    tint: *tint,
                    art,
                }
            })
            .collect()
    }

    /// The static half of `sprite_subjects`, parsed once. Every species def,
    /// every structure def, every floor finish, and the two hardcoded names,
    /// keyed on `sprite_name()` through a `BTreeMap` — which is the
    /// de-duplication *and* the "sort by name" rule in one structure.
    ///
    /// `SpeciesDb::all`/`StructureDb::all`/`FloorDb::iter` are all already
    /// deterministic (each sorts its own defs), so which def wins a shared
    /// name is stable run to run: structures after species, floors after
    /// structures, and the two hardcoded names last of all.
    fn load_static_sprite_subjects(assets_dir: &Path) -> Vec<StaticSpriteSubject> {
        let (abilities, _) = AbilityDb::load_dir(&assets_dir.join("abilities")).unwrap_or_default();
        let (species, _) =
            SpeciesDb::load_dir(&assets_dir.join("species"), &abilities).unwrap_or_default();
        let (structures, _) =
            StructureDb::load_dir(&assets_dir.join("structures")).unwrap_or_default();
        let (floors, _) =
            feral_processes_engine::floors::FloorDb::load_dir(&assets_dir.join("floors"))
                .unwrap_or_default();

        let (props, _) = PropDb::load_dir(&assets_dir.join("battle-props")).unwrap_or_default();

        let mut by_name: BTreeMap<String, (String, char, SubjectTint)> = BTreeMap::new();
        for def in species.all() {
            by_name.insert(
                def.sprite_name().to_string(),
                (
                    def.name.clone(),
                    def.glyph,
                    SubjectTint::Glyph(Some(def.color)),
                ),
            );
        }
        for def in structures.all() {
            by_name.insert(
                def.sprite_name().to_string(),
                (
                    def.name.clone(),
                    def.glyph,
                    SubjectTint::Glyph(Some(def.color)),
                ),
            );
        }
        for def in floors.iter() {
            by_name.insert(
                def.sprite_name().to_string(),
                (
                    def.name.clone(),
                    FLOOR_SPRITE_GLYPH,
                    SubjectTint::Shade(def.shade),
                ),
            );
        }
        // A prop is drawn near-white and tinted by brightness alone, so it
        // previews against a neutral grey. Destructible pieces list their
        // `_cracked` twin too: it is a separate file the renderer swaps in.
        for def in props.pieces().filter(|d| !d.sprite.is_empty()) {
            let label = def.id.replace('_', " ");
            let tint = SubjectTint::Glyph(Some(GlyphColor::Gray));
            by_name.insert(def.sprite.clone(), (label.clone(), PROP_SPRITE_GLYPH, tint));
            if def.hp.is_some() {
                by_name.insert(
                    format!("{}_cracked", def.sprite),
                    (format!("{label} (cracked)"), PROP_SPRITE_GLYPH, tint),
                );
            }
        }
        // `Glyph(None)`, not `Glyph(Some(GlyphColor::Cyan))` — see
        // `SpriteSubject::tint`'s own doc comment for why the player has no
        // authored hue at all.
        by_name.insert(
            DEFAULT_PLAYER_SPRITE.to_string(),
            ("Player".to_string(), PLAYER_GLYPH, SubjectTint::Glyph(None)),
        );
        by_name.insert(
            ANCHOR_SPRITE_NAME.to_string(),
            (
                "Anchor".to_string(),
                ANCHOR_GLYPH,
                SubjectTint::Glyph(Some(ANCHOR_GLYPH_COLOR)),
            ),
        );

        by_name
            .into_iter()
            .map(|(name, (label, glyph, tint))| (name, label, glyph, tint))
            .collect()
    }

    /// `Esc` backs out through `close_screen`, which is the main menu for
    /// the `[D]` row and the dev keypad for the console's — see
    /// `open_sprite_forge`. `[t]` toggles a
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
            self.close_screen();
            return;
        }
        let subjects = self.sprite_subjects();
        if key == GameKey::Char('t') {
            if let Some(subject) = subjects.get(self.menu_selected) {
                let op = match subject.art {
                    SpriteArt::On => Some(SpriteOp::Disable {
                        full_colour: self.sprite_library[&subject.name].full_colour,
                    }),
                    SpriteArt::Off => Some(SpriteOp::Enable {
                        full_colour: self.sprite_disabled[&subject.name].full_colour,
                    }),
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
            self.open_sprite_subject(&subjects[idx].name);
        }
    }

    /// Opens `Mode::SpriteEditor` on `name`'s art — what `Enter` and a double
    /// click both do. Enabled art first, then the disabled copy, and a blank
    /// canvas only for a subject that never had any.
    fn open_sprite_subject(&mut self, name: &str) {
        let installed = self
            .sprite_library
            .get(name)
            .or_else(|| self.sprite_disabled.get(name));
        let (frames, frame_ms, full_colour) = match installed {
            Some(sprite) => (
                sprite.frames.clone(),
                sprite.frame_ms,
                Some(sprite.full_colour),
            ),
            None => (Vec::new(), DEFAULT_SPRITE_FRAME_MS, None),
        };
        self.sprite_editor = Some(ForgeSession {
            editor: SpriteEditor::open(frames, frame_ms),
            subject: name.to_string(),
            full_colour,
        });
        self.mode = Mode::SpriteEditor;
    }

    /// What `Mode::SpriteEditor` draws, or `None` while it is not open.
    pub fn sprite_editor_view(&self) -> Option<SpriteEditorView> {
        let session = self.sprite_editor.as_ref()?;
        // A subject missing from its own picker list is unreachable — `Enter`
        // only ever opens a name the list just offered — and draws unmodified
        // rather than panicking if a future caller breaks that.
        let tint = self
            .sprite_static_subjects
            .as_ref()
            .and_then(|subjects| subjects.iter().find(|s| s.0 == session.subject))
            .map(|s| s.3);
        Some(session.editor.view(EditorChrome {
            title: session.subject.clone(),
            buttons: FORGE_BUTTONS.to_vec(),
            tint,
            full_colour: session.full_colour(),
        }))
    }

    /// Drains every `SpriteWrite` queued since the last call —
    /// `App::take_sounds`'s seam: app-core queues and forgets, the frontend
    /// (Task 8) drains, performs the write or rename, and re-installs the
    /// library so the map updates without a restart.
    pub fn take_sprite_writes(&mut self) -> Vec<SpriteWrite> {
        std::mem::take(&mut self.pending_sprite_writes)
    }

    /// `Mode::SpriteEditor`'s host keys, on top of the shared editor's own:
    /// `[s]` queues a `Save` carrying the canvas as it stands, and `Esc`
    /// leaves for `Mode::SpritePicker` without queueing anything — a blank
    /// canvas is a legitimate save here, so `Esc` and `[s]` differ only in
    /// whether a cue is raised at all, never in what the cue would have
    /// carried.
    pub(crate) fn handle_sprite_editor_key(&mut self, key: GameKey) {
        let Some(session) = &mut self.sprite_editor else {
            return;
        };
        match key {
            GameKey::Esc => {
                self.sprite_editor = None;
                self.mode = Mode::SpritePicker;
            }
            GameKey::Char('s') => {
                // Pinned after the first save: re-deriving would let a later
                // hue-free save write `x.png` beside the `x.colour.png`
                // already on disk, and the stale colour file would win.
                let full_colour = session.full_colour();
                session.full_colour = Some(full_colour);
                let write = SpriteWrite {
                    name: session.subject.clone(),
                    op: SpriteOp::Save {
                        frames: session.editor.frames(),
                        frame_ms: session.editor.frame_ms(),
                        full_colour,
                    },
                };
                self.pending_sprite_writes.push(write);
            }
            _ => {
                session.editor.handle_key(key);
            }
        }
    }

    /// The mouse's one entry point — routed only while `Mode::SpriteEditor`
    /// or `Mode::SpritePicker` is open, or while the creation wizard's editor
    /// is, every other mode drops it silently, since nothing else in
    /// the game reads a pointer at all. `phase` governs the stroke
    /// (`PointerPhase`'s own doc comment); `hit` decides what happens at
    /// it — a `Cell` paints (the selected swatch on `Primary`, index 0 —
    /// erase — on `Secondary`), a `Swatch` selects.
    ///
    /// **`PointerHit::Swatch` carries `swatch_at`'s 0-based drawn position,
    /// and that is the palette index itself**: `draw_swatch_grid` draws the
    /// transparent swatch (index 0) first and palette entry `n` at position
    /// `n`. It used to be a `+ 1` conversion here, and getting that wrong
    /// selected the swatch left of the one outlined; with no conversion
    /// there is nothing at this seam to get wrong.
    pub fn handle_pointer(&mut self, hit: PointerHit, button: PointerButton, phase: PointerPhase) {
        match self.mode {
            Mode::SpriteEditor => self.handle_editor_pointer(hit, button, phase),
            Mode::SpritePicker => self.handle_picker_pointer(hit, button, phase),
            Mode::CreateCharacter => self.handle_creation_pointer(hit, button, phase),
            _ => {}
        }
    }

    /// A press on a picker row selects it, and a double click on the row that
    /// is already selected opens it — `Enter`'s own door. An index past the
    /// list is ignored, since the gui sizes its rects off the same list but a
    /// stale frame must not be able to select nothing.
    fn handle_picker_pointer(
        &mut self,
        hit: PointerHit,
        button: PointerButton,
        phase: PointerPhase,
    ) {
        let PointerHit::Subject(index) = hit else {
            return;
        };
        if button != PointerButton::Primary {
            return;
        }
        let subjects = self.sprite_subjects();
        let Some(subject) = subjects.get(index) else {
            return;
        };
        match phase {
            PointerPhase::Down => self.menu_selected = index,
            PointerPhase::DoubleClick => {
                if self.menu_selected == index {
                    self.open_sprite_subject(&subject.name);
                } else {
                    self.menu_selected = index;
                }
            }
            PointerPhase::Drag | PointerPhase::Up => {}
        }
    }

    fn handle_editor_pointer(
        &mut self,
        hit: PointerHit,
        button: PointerButton,
        phase: PointerPhase,
    ) {
        // A button is a key press, not a gesture: only a primary `Down`
        // acts, and it opens no stroke.
        if let PointerHit::Button(pressed) = hit {
            if phase == PointerPhase::Down && button == PointerButton::Primary {
                self.handle_sprite_editor_key(pressed.key());
            }
            return;
        }
        let Some(session) = &mut self.sprite_editor else {
            return;
        };
        session.editor.handle_pointer(hit, button, phase);
    }
}
