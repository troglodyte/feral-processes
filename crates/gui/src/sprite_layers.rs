//! The two sprite folders as one library.
//!
//! Art ships in `assets/sprites/` (the `shipped` layer) and the player's own
//! edits live in `<data_dir>/sprites/` (the `user` layer). **Every write goes
//! to the user layer and the shipped folder is never touched**, so a player
//! can redraw anything and a revert is just deleting their files.
//!
//! **The whole-name rule:** if the user folder holds any file for a name
//! (`<name>.png`, `<name>.colour.png`, either `.png.off`, `<name>.anim.ron`),
//! only the user folder decides that name. Per-variant merging would let a
//! shipped `.colour.png` outrank a player's plain redraw, or resurrect art
//! the player switched off. An undecodable user file still owns its name: it
//! draws the glyph with a warning rather than quietly showing the shipped
//! art, and Revert clears it.
//!
//! Bevy-free on purpose: this is the part that decides what is on disk, and
//! it is tested without an `AssetServer`. `sprites.rs` is the glue over it.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use bevy::prelude::warn;
use feral_processes_app_core::{InstalledSprite, SpriteOp};
use feral_processes_engine::icon::{Canvas, FULL_COLOUR_SUFFIX};

use crate::sprites::{
    SpriteAnim, anim_path, frames_to_png, png_to_frames, read_frame_ms, scan_sprite_dir,
};

/// Which folder a sprite file lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Shipped,
    User,
}

/// One PNG the map should load: the folder and the file stem (`x` or
/// `x.colour`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadKey {
    pub layer: Layer,
    pub stem: String,
}

/// What the picker shows: the merged, decoded library.
#[derive(Debug, PartialEq)]
pub struct Library {
    pub enabled: HashMap<String, InstalledSprite>,
    pub disabled: HashMap<String, InstalledSprite>,
    /// Names the user layer owns.
    pub edited: HashSet<String>,
}

/// What a write did, and so what the frontend must do next.
#[derive(Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    /// A `Save` or `Enable` landed; load this file.
    Load(LoadKey),
    /// A `Disable` or `Revert` landed: take `name` out of the table, then
    /// load whatever still draws for it.
    Remove {
        name: String,
        then_load: Vec<LoadKey>,
    },
    /// The write did not happen; the sentence is for the player.
    Failed(String),
}

/// The two folders. `user` is `None` in tests and the arena, where nothing
/// may be written.
#[derive(Clone, Debug)]
pub struct SpriteLayers {
    pub shipped: PathBuf,
    pub user: Option<PathBuf>,
}

/// Every decoded PNG in one folder, split into enabled and disabled
/// (`<name>.png.off`) and keyed by name. A missing folder is the supported
/// empty state; a file that fails to decode is dropped from its map.
fn scan_dir(
    dir: &Path,
) -> (
    HashMap<String, InstalledSprite>,
    HashMap<String, InstalledSprite>,
) {
    let mut enabled = HashMap::new();
    let mut disabled = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (enabled, disabled);
    };
    for path in entries.filter_map(|entry| entry.ok().map(|entry| entry.path())) {
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let (stem, map) = match file_name.strip_suffix(".png.off") {
            Some(stem) => (stem, &mut disabled),
            None => match file_name.strip_suffix(".png") {
                Some(stem) => (stem, &mut enabled),
                None => continue,
            },
        };
        let (name, full_colour) = match stem.strip_suffix(FULL_COLOUR_SUFFIX) {
            Some(name) => (name, true),
            None => (stem, false),
        };
        if name.starts_with('@') {
            continue;
        }
        match png_to_frames(&path) {
            Some(frames) => {
                // The colour file wins whatever order the directory lists
                // them in — `SpriteTable::insert`'s own rule, so the editor
                // opens what the map draws.
                let wins = full_colour || map.get(name).is_none_or(|have| !have.full_colour);
                if wins {
                    map.insert(
                        name.to_string(),
                        InstalledSprite {
                            frames,
                            frame_ms: read_frame_ms(dir, name),
                            full_colour,
                        },
                    );
                }
            }
            None => warn!("{}: not a 16px sprite row, ignoring it", path.display()),
        }
    }
    (enabled, disabled)
}

/// The name a sprite file belongs to, or `None` for a file that is not
/// sprite state (or whose stem starts with `@`, `DRAWN_ICON_KEY`'s guard).
fn owning_name(file_name: &str) -> Option<&str> {
    let stem = file_name
        .strip_suffix(".png.off")
        .or_else(|| file_name.strip_suffix(".png"))
        .or_else(|| file_name.strip_suffix(".anim.ron"))?;
    let name = stem.strip_suffix(FULL_COLOUR_SUFFIX).unwrap_or(stem);
    (!name.starts_with('@')).then_some(name)
}

fn sprite_stem(name: &str, full_colour: bool) -> String {
    if full_colour {
        format!("{name}{FULL_COLOUR_SUFFIX}")
    } else {
        name.to_string()
    }
}

/// The one place a sprite's file path is derived: `<name>.png`, or
/// `<name>.colour.png` for the colour variant, plus `.off` when disabled.
/// Each variant is its own file, so a write never touches the other's.
fn sprite_path(dir: &Path, name: &str, full_colour: bool, off: bool) -> PathBuf {
    let stem = sprite_stem(name, full_colour);
    dir.join(if off {
        format!("{stem}.png.off")
    } else {
        format!("{stem}.png")
    })
}

/// Keeps `<name>.anim.ron` in step with a save: written for a sheet that
/// animates, removed for one that no longer does, so deleting frame 2 and
/// saving leaves nothing behind that claims a pace for a still sprite.
/// Best-effort like the `.off` retirement: the PNG already landed.
fn write_anim(dir: &Path, name: &str, frames: usize, frame_ms: u32) {
    let path = anim_path(dir, name);
    if frames < 2 {
        if path.exists()
            && let Err(e) = std::fs::remove_file(&path)
        {
            warn!("sprite `{name}` saved, but its stale `.anim.ron` could not be removed: {e}");
        }
        return;
    }
    let written = ron::to_string(&SpriteAnim { frame_ms })
        .map_err(std::io::Error::other)
        .and_then(|text| std::fs::write(&path, text));
    if let Err(e) = written {
        warn!("sprite `{name}` saved, but its `.anim.ron` could not be written: {e}");
    }
}

impl SpriteLayers {
    pub fn dir(&self, layer: Layer) -> Option<&Path> {
        match layer {
            Layer::Shipped => Some(&self.shipped),
            Layer::User => self.user.as_deref(),
        }
    }

    /// Every name the user folder holds a file for. A missing folder is an
    /// empty set, with no warning: it does not exist until the first save.
    pub fn user_names(&self) -> HashSet<String> {
        let Some(entries) = self.user.as_ref().and_then(|u| std::fs::read_dir(u).ok()) else {
            return HashSet::new();
        };
        entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let file_name = entry.file_name();
                owning_name(file_name.to_str()?).map(str::to_string)
            })
            .collect()
    }

    /// Every enabled stem the map should load, the user layer's names
    /// shadowing the shipped ones.
    pub fn load_list(&self) -> Vec<LoadKey> {
        let owned = self.user_names();
        let key = |layer| move |stem: String| LoadKey { layer, stem };
        let shipped = scan_sprite_dir(&self.shipped)
            .into_iter()
            .filter(|stem| !owned.contains(base_name(stem)))
            .map(key(Layer::Shipped));
        let user = self
            .user
            .iter()
            .flat_map(|dir| scan_sprite_dir(dir))
            .map(key(Layer::User));
        shipped.chain(user).collect()
    }

    /// The decoded library with shadowing applied. Pace is read from the
    /// folder that owns the name, since each scan reads its own.
    pub fn library(&self) -> Library {
        let (mut enabled, mut disabled) = scan_dir(&self.shipped);
        let edited = self.user_names();
        enabled.retain(|name, _| !edited.contains(name));
        disabled.retain(|name, _| !edited.contains(name));
        if let Some(user) = &self.user {
            let (user_enabled, user_disabled) = scan_dir(user);
            enabled.extend(user_enabled);
            disabled.extend(user_disabled);
        }
        Library {
            enabled,
            disabled,
            edited,
        }
    }

    /// Performs the one write `op` asks for under `name`, always inside the
    /// user folder.
    ///
    /// **Per variant, `<stem>.png` and `<stem>.png.off` never both exist**
    /// in the user folder: `Save` retires a stale `.off`, and `Enable` and
    /// `Disable` refuse rather than clobber an occupied destination. Without
    /// that, disable, edit, save, disable lost the original art.
    pub fn apply(&self, name: &str, op: SpriteOp) -> WriteOutcome {
        let Some(user) = self.dir(Layer::User) else {
            return failed(name, op_verb(&op), "there is no folder for your sprites");
        };
        let verb = op_verb(&op);
        match self.apply_in(user, name, op) {
            Ok(outcome) => outcome,
            Err(e) => {
                warn!("sprite `{name}` write failed: {e}");
                failed(name, verb, &e.to_string())
            }
        }
    }

    fn apply_in(&self, user: &Path, name: &str, op: SpriteOp) -> std::io::Result<WriteOutcome> {
        match op {
            SpriteOp::Save {
                frames,
                frame_ms,
                full_colour,
            } => save(user, name, &frames, frame_ms, full_colour),
            SpriteOp::Enable { full_colour } => enable(user, name, full_colour),
            SpriteOp::Disable { full_colour } => self.disable(user, name, full_colour),
            SpriteOp::Revert => self.revert(user, name),
        }
    }

    fn disable(&self, user: &Path, name: &str, full_colour: bool) -> std::io::Result<WriteOutcome> {
        let on = sprite_path(user, name, full_colour, false);
        let off = sprite_path(user, name, full_colour, true);
        if off.exists() {
            return Err(occupied(&off));
        }
        if on.exists() {
            std::fs::rename(&on, &off)?;
        } else {
            // Switching off shipped art: the player's folder gets an `.off`
            // copy, which by the whole-name rule hides it.
            std::fs::create_dir_all(user)?;
            // The `.anim.ron` goes first: the `.off` is what hides the name,
            // so a failure between the two leaves the shipped art showing.
            let anim = anim_path(&self.shipped, name);
            if anim.exists() {
                std::fs::copy(&anim, anim_path(user, name))?;
            }
            std::fs::copy(sprite_path(&self.shipped, name, full_colour, false), &off)?;
        }
        let plain_remains = full_colour && sprite_path(user, name, false, false).exists();
        Ok(WriteOutcome::Remove {
            name: name.to_string(),
            then_load: if plain_remains {
                vec![user_key(name.to_string())]
            } else {
                Vec::new()
            },
        })
    }

    fn revert(&self, user: &Path, name: &str) -> std::io::Result<WriteOutcome> {
        let mut files = vec![anim_path(user, name)];
        for full_colour in [false, true] {
            files.push(sprite_path(user, name, full_colour, false));
            files.push(sprite_path(user, name, full_colour, true));
        }
        for file in files {
            match std::fs::remove_file(&file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
                _ => {}
            }
        }
        let then_load = scan_sprite_dir(&self.shipped)
            .into_iter()
            .filter(|stem| base_name(stem) == name)
            .map(|stem| LoadKey {
                layer: Layer::Shipped,
                stem,
            })
            .collect();
        Ok(WriteOutcome::Remove {
            name: name.to_string(),
            then_load,
        })
    }
}

fn save(
    user: &Path,
    name: &str,
    frames: &[Canvas],
    frame_ms: u32,
    full_colour: bool,
) -> std::io::Result<WriteOutcome> {
    std::fs::create_dir_all(user)?;
    frames_to_png(frames, &sprite_path(user, name, full_colour, false))?;
    write_anim(user, name, frames.len(), frame_ms);
    let off = sprite_path(user, name, full_colour, true);
    if off.exists()
        && let Err(e) = std::fs::remove_file(&off)
    {
        warn!("sprite `{name}` saved, but its stale `.png.off` backup could not be cleared: {e}");
    }
    Ok(WriteOutcome::Load(user_key(sprite_stem(name, full_colour))))
}

fn enable(user: &Path, name: &str, full_colour: bool) -> std::io::Result<WriteOutcome> {
    let on = sprite_path(user, name, full_colour, false);
    if on.exists() {
        return Err(occupied(&on));
    }
    std::fs::rename(sprite_path(user, name, full_colour, true), &on)?;
    Ok(WriteOutcome::Load(user_key(sprite_stem(name, full_colour))))
}

fn occupied(path: &Path) -> std::io::Error {
    std::io::Error::other(format!("`{}` already exists", path.display()))
}

fn base_name(stem: &str) -> &str {
    stem.strip_suffix(FULL_COLOUR_SUFFIX).unwrap_or(stem)
}

fn user_key(stem: String) -> LoadKey {
    LoadKey {
        layer: Layer::User,
        stem,
    }
}

/// What the player was doing, for the failure line: "Couldn't <verb> sprite".
fn op_verb(op: &SpriteOp) -> &'static str {
    match op {
        SpriteOp::Save { .. } => "save",
        SpriteOp::Enable { .. } => "turn on",
        SpriteOp::Disable { .. } => "turn off",
        SpriteOp::Revert => "revert",
    }
}

fn failed(name: &str, verb: &str, cause: &str) -> WriteOutcome {
    WriteOutcome::Failed(format!("Couldn't {verb} sprite `{name}`: {cause}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sprites::tests::{
        a_sprite_canvas, canvas_to_png, codec_test_dir, plain, png_to_canvas, two_frames,
    };
    use feral_processes_app_core::DEFAULT_SPRITE_FRAME_MS;
    use feral_processes_engine::ICON_SIZE;

    /// Runs one op against a user-only layering: the shipped folder points
    /// nowhere, so these tests see the user layer's own rules.
    fn apply(dir: &Path, name: &str, op: SpriteOp) -> WriteOutcome {
        SpriteLayers {
            shipped: PathBuf::from("/nonexistent/shipped-sprites"),
            user: Some(dir.to_path_buf()),
        }
        .apply(name, op)
    }

    fn user_key(stem: &str) -> LoadKey {
        LoadKey {
            layer: Layer::User,
            stem: stem.to_string(),
        }
    }

    fn shipped_key(stem: &str) -> LoadKey {
        LoadKey {
            layer: Layer::Shipped,
            stem: stem.to_string(),
        }
    }

    fn loaded(stem: &str) -> WriteOutcome {
        WriteOutcome::Load(user_key(stem))
    }

    fn removed(name: &str, then_load: &[&str]) -> WriteOutcome {
        WriteOutcome::Remove {
            name: name.to_string(),
            then_load: then_load.iter().map(|s| user_key(s)).collect(),
        }
    }

    /// A shipped folder (created) and a user folder path (not created).
    fn layers(tag: &str) -> (PathBuf, SpriteLayers) {
        let root = codec_test_dir(tag);
        let shipped = root.join("shipped");
        std::fs::create_dir_all(&shipped).unwrap();
        let layers = SpriteLayers {
            shipped,
            user: Some(root.join("user")),
        };
        (root, layers)
    }

    fn user_dir(layers: &SpriteLayers) -> &Path {
        layers.user.as_deref().unwrap()
    }

    fn folder_bytes(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .map(|e| {
                        (
                            e.file_name().to_string_lossy().into_owned(),
                            std::fs::read(e.path()).unwrap(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        files
    }

    fn user_files(layers: &SpriteLayers) -> Vec<String> {
        folder_bytes(user_dir(layers))
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }

    #[test]
    fn missing_user_dir_is_an_empty_layer() {
        let (root, layers) = layers("missing_user");
        write_png(&layers.shipped.join("x.png"), &dot(3));

        let library = layers.library();
        let shipped_only = scan_dir(&layers.shipped);
        let keys = layers.load_list();
        std::fs::remove_dir_all(&root).ok();

        assert!(library.edited.is_empty());
        assert_eq!((library.enabled, library.disabled), shipped_only);
        assert_eq!(keys, vec![shipped_key("x")]);
    }

    #[test]
    fn a_user_file_shadows_both_shipped_variants() {
        let (root, layers) = layers("shadow_both");
        write_png(&layers.shipped.join("x.colour.png"), &dot(12));
        write_png(&layers.shipped.join("x.png"), &dot(1));
        write_png(&user_dir(&layers).join("x.png"), &dot(2));
        // `write_png` does not make the folder.
        let keys = layers.load_list();
        let library = layers.library();
        std::fs::remove_dir_all(&root).ok();

        assert!(!library.enabled["x"].full_colour);
        assert_eq!(keys, vec![user_key("x")]);
    }

    #[test]
    fn a_user_off_file_alone_hides_shipped_art() {
        let (root, layers) = layers("off_hides");
        write_png(&layers.shipped.join("x.png"), &dot(1));
        write_png(&user_dir(&layers).join("x.png.off"), &dot(2));

        let library = layers.library();
        let keys = layers.load_list();
        std::fs::remove_dir_all(&root).ok();

        assert!(library.disabled.contains_key("x"));
        assert!(!library.enabled.contains_key("x"));
        assert!(keys.is_empty());
    }

    #[test]
    fn pace_comes_from_the_owning_layer() {
        let (root, layers) = layers("pace");
        write_png(&layers.shipped.join("x.png"), &dot(1));
        std::fs::write(anim_path(&layers.shipped, "x"), "(frame_ms: 300)").unwrap();
        std::fs::create_dir_all(user_dir(&layers)).unwrap();
        frames_to_png(&two_frames(), &user_dir(&layers).join("x.png")).unwrap();

        let library = layers.library();
        std::fs::remove_dir_all(&root).ok();

        assert_eq!(library.enabled["x"].frame_ms, DEFAULT_SPRITE_FRAME_MS);
    }

    #[test]
    fn save_creates_the_user_dir_and_never_touches_shipped() {
        let (root, layers) = layers("save_creates");
        write_png(&layers.shipped.join("x.png"), &dot(1));
        std::fs::write(anim_path(&layers.shipped, "x"), "(frame_ms: 300)").unwrap();
        let before = folder_bytes(&layers.shipped);

        let outcome = layers.apply("x", save(dot(5), false));

        let after = folder_bytes(&layers.shipped);
        let made = user_dir(&layers).join("x.png").exists();
        std::fs::remove_dir_all(&root).ok();
        assert_eq!(outcome, loaded("x"));
        assert!(made);
        assert_eq!(before, after);
    }

    #[test]
    fn disable_then_enable_a_shipped_sprite_then_revert() {
        let (root, layers) = layers("shipped_toggle");
        write_png(&layers.shipped.join("x.png"), &dot(1));
        let shipped_bytes = std::fs::read(layers.shipped.join("x.png")).unwrap();

        let disable = layers.apply("x", SpriteOp::Disable { full_colour: false });
        let off_bytes = std::fs::read(user_dir(&layers).join("x.png.off")).unwrap();
        let hidden = layers.library();
        let enable = layers.apply("x", SpriteOp::Enable { full_colour: false });
        let owned = layers.library();
        let revert = layers.apply("x", SpriteOp::Revert);
        let reverted = layers.library();
        std::fs::remove_dir_all(&root).ok();

        assert_eq!(disable, removed("x", &[]));
        assert_eq!(off_bytes, shipped_bytes);
        assert!(hidden.disabled.contains_key("x") && !hidden.enabled.contains_key("x"));
        assert_eq!(enable, loaded("x"));
        assert!(owned.edited.contains("x") && owned.enabled.contains_key("x"));
        assert_eq!(
            revert,
            WriteOutcome::Remove {
                name: "x".into(),
                then_load: vec![shipped_key("x")]
            }
        );
        assert!(reverted.edited.is_empty());
        assert!(reverted.enabled.contains_key("x"));
    }

    #[test]
    fn revert_then_save_leaves_no_leftovers() {
        let (root, layers) = layers("revert_save");
        // A disabled colour sheet: its `.off` is one leftover. The two-frame
        // plain save after it is saved last because a one-frame save clears
        // the `.anim.ron`, and that file is the other.
        layers.apply(
            "x",
            SpriteOp::Save {
                frames: vec![dot(2)],
                frame_ms: DEFAULT_SPRITE_FRAME_MS,
                full_colour: true,
            },
        );
        layers.apply("x", SpriteOp::Disable { full_colour: true });
        layers.apply("x", two_frame_save(900));
        let before = user_files(&layers);

        layers.apply("x", SpriteOp::Revert);
        let reverted = user_files(&layers);
        layers.apply("x", save(dot(4), false));

        let files = user_files(&layers);
        std::fs::remove_dir_all(&root).ok();
        assert!(before.contains(&"x.anim.ron".to_string()), "{before:?}");
        assert!(
            before.contains(&"x.colour.png.off".to_string()),
            "{before:?}"
        );
        assert!(reverted.is_empty(), "{reverted:?}");
        assert_eq!(files, vec!["x.png".to_string()]);
    }

    #[test]
    fn a_corrupt_user_file_still_shadows_shipped() {
        let (root, layers) = layers("corrupt_shadow");
        write_png(&layers.shipped.join("x.png"), &dot(1));
        std::fs::create_dir_all(user_dir(&layers)).unwrap();
        std::fs::write(user_dir(&layers).join("x.png"), b"garbage").unwrap();

        let library = layers.library();
        let keys = layers.load_list();
        std::fs::remove_dir_all(&root).ok();

        assert!(library.edited.contains("x"));
        assert!(!library.enabled.contains_key("x"));
        assert!(!keys.contains(&shipped_key("x")));
    }

    #[test]
    fn no_user_dir_fails_a_write() {
        let root = codec_test_dir("no_user_dir");
        let shipped = root.join("shipped");
        std::fs::create_dir_all(&shipped).unwrap();
        let layers = SpriteLayers {
            shipped: shipped.clone(),
            user: None,
        };

        let outcome = layers.apply("x", save(dot(4), false));

        let written = folder_bytes(&shipped);
        std::fs::remove_dir_all(&root).ok();
        assert!(matches!(outcome, WriteOutcome::Failed(_)));
        assert!(written.is_empty());
    }

    #[test]
    fn a_failed_write_names_the_sprite_and_the_cause() {
        let root = codec_test_dir("failed_message");
        // A file where the user folder should be: create_dir_all must fail.
        let blocker = root.join("user");
        std::fs::write(&blocker, b"in the way").unwrap();
        let layers = SpriteLayers {
            shipped: root.join("shipped"),
            user: Some(blocker),
        };

        let outcome = layers.apply("x", save(dot(4), false));

        std::fs::remove_dir_all(&root).ok();
        let WriteOutcome::Failed(message) = outcome else {
            panic!("expected a failure, got {outcome:?}");
        };
        assert!(
            message.starts_with("Couldn't save sprite `x`: "),
            "{message}"
        );
    }

    /// `scan_dir` reads the disabled *art* off a `.png.off` rename, and the
    /// same file must be invisible to `scan_sprite_dir` — the loader's own
    /// scan — without that scan needing to know `.off` exists at all: its
    /// plain `extension() == "png"` filter already excludes it.
    ///
    /// The `.off` file is a real PNG renamed, not garbage bytes, and
    /// `scan_dir` decodes it (through `png_to_canvas`'s guessed-format read,
    /// since the real extension no longer says `png`), so this pins that the
    /// decoded pixels match what was written, not just that the name was
    /// noticed.
    #[test]
    fn a_disabled_file_is_absent_from_the_loader_scan_and_present_in_the_librarys_disabled_set() {
        let dir = codec_test_dir("disabled");
        canvas_to_png(&a_sprite_canvas(), &dir.join("on_subject.png")).unwrap();
        let off_canvas = a_sprite_canvas();
        canvas_to_png(&off_canvas, &dir.join("off_subject.png")).unwrap();
        std::fs::rename(dir.join("off_subject.png"), dir.join("off_subject.png.off")).unwrap();

        let loader_names = scan_sprite_dir(&dir);
        let (enabled, disabled) = scan_dir(&dir);

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
            Some(&plain(off_canvas.clone())),
            "a disabled sprite's pixels must decode to what was written, not just be noticed by name"
        );
        assert!(
            !enabled.contains_key("off_subject"),
            "a disabled sprite's pixels are not installed as art the map may draw"
        );
    }

    fn two_frame_save(frame_ms: u32) -> SpriteOp {
        SpriteOp::Save {
            frames: two_frames(),
            frame_ms,
            full_colour: false,
        }
    }

    #[test]
    fn saving_two_frames_writes_a_sheet_and_an_anim_file_that_reads_back() {
        let dir = codec_test_dir("save_two_frames");

        let outcome = apply(&dir, "subject", two_frame_save(900));

        let img = image::open(dir.join("subject.png")).unwrap();
        let ms = read_frame_ms(&dir, "subject");
        let (enabled, _) = scan_dir(&dir);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(outcome, loaded("subject"));
        assert_eq!((img.width(), img.height()), (32, 16));
        assert_eq!(ms, 900);
        assert_eq!(enabled["subject"].frames, two_frames());
        assert_eq!(enabled["subject"].frame_ms, 900);
    }

    #[test]
    fn saving_one_frame_removes_a_stale_anim_file() {
        let dir = codec_test_dir("save_one_frame_clears_anim");
        apply(&dir, "subject", two_frame_save(900));
        assert!(anim_path(&dir, "subject").exists());

        apply(&dir, "subject", save(dot(3), false));

        let gone = !anim_path(&dir, "subject").exists();
        let img = image::open(dir.join("subject.png")).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(gone);
        assert_eq!((img.width(), img.height()), (16, 16));
    }

    /// **`t` (disable), Enter (open), draw, `s` (save), `t` (disable
    /// again).** Without the I1/I2 guards that sequence destroyed the
    /// original art: Enter opened a blank canvas instead of the disabled
    /// art (I2), so the player's `s` wrote a fresh file *beside* the still-
    /// present `.png.off` backup, and the final `t` clobbered that backup
    /// with the new drawing via a bare `fs::rename` (I1) — the original was
    /// gone with no warning and no way back, on a screen whose entire
    /// subject is unbacked-up work.
    ///
    /// This test drives `SpriteLayers::apply` and `scan_dir` through the
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
            apply(&dir, "subject", SpriteOp::Disable { full_colour: false }),
            removed("subject", &[])
        );
        assert!(!dir.join("subject.png").exists());
        assert!(dir.join("subject.png.off").exists());

        // Enter: `handle_sprite_picker_key`'s real fallback is `sprite_
        // library.get(name).or_else(|| sprite_disabled.get(name))` —
        // `scan_dir`'s `disabled` map is what feeds that second half,
        // so reading it back here is the same lookup the picker makes.
        // I2's fix is that this is `original`, not a blank canvas.
        let (_, disabled) = scan_dir(&dir);
        let opened = disabled
            .get("subject")
            .map(|sprite| sprite.frames[0].clone())
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
            apply(
                &dir,
                "subject",
                SpriteOp::Save {
                    frames: vec![edited.clone()],
                    frame_ms: DEFAULT_SPRITE_FRAME_MS,
                    full_colour: false
                }
            ),
            loaded("subject")
        );
        assert!(dir.join("subject.png").exists());
        assert!(
            !dir.join("subject.png.off").exists(),
            "I1: saving an enabled copy must retire the stale disabled backup, \
             or the library reports this name in both maps"
        );

        // `t`: disable again. With the invariant held, there is nothing
        // left under `subject.png.off` to clobber.
        assert_eq!(
            apply(&dir, "subject", SpriteOp::Disable { full_colour: false }),
            removed("subject", &[])
        );

        let (enabled_final, disabled_final) = scan_dir(&dir);
        assert!(
            !enabled_final.contains_key("subject"),
            "disabled means disabled — the map must not still list it as on"
        );
        assert_eq!(
            disabled_final.get("subject"),
            Some(&plain(edited.clone())),
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

        let outcome = apply(&dir, "subject", SpriteOp::Disable { full_colour: false });

        let enabled_survived = png_to_canvas(&dir.join("subject.png"));
        let backup_untouched = png_to_canvas(&dir.join("subject.png.off"));
        std::fs::remove_dir_all(&dir).ok();

        assert!(matches!(outcome, WriteOutcome::Failed(_)));
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

        let outcome = apply(&dir, "subject", SpriteOp::Enable { full_colour: false });

        let enabled_untouched = png_to_canvas(&dir.join("subject.png"));
        let backup_survived = png_to_canvas(&dir.join("subject.png.off"));
        std::fs::remove_dir_all(&dir).ok();

        assert!(matches!(outcome, WriteOutcome::Failed(_)));
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

        let first = apply(&dir, "subject", SpriteOp::Disable { full_colour: false });
        let second = apply(&dir, "subject", SpriteOp::Disable { full_colour: false });

        let recovered = png_to_canvas(&dir.join("subject.png.off"));
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(first, removed("subject", &[]));
        assert!(
            matches!(second, WriteOutcome::Failed(_)),
            "the second of a doubled toggle must be refused, not applied twice"
        );
        assert_eq!(
            recovered,
            Some(art),
            "the art must be intact after the doubled toggle"
        );
    }

    /// `canvas_to_png` dispatches on the extension, so a `.off` file is
    /// written under its `.png` name and renamed, as a real toggle does.
    fn write_png(path: &Path, canvas: &Canvas) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        match path.to_str().and_then(|p| p.strip_suffix(".off")) {
            Some(png) => {
                canvas_to_png(canvas, Path::new(png)).unwrap();
                std::fs::rename(png, path).unwrap();
            }
            None => canvas_to_png(canvas, path).unwrap(),
        }
    }

    fn dot(index: u8) -> Canvas {
        let mut canvas = Canvas::new(ICON_SIZE);
        canvas.set(2, 2, index);
        canvas
    }

    fn plain_or_colour(canvas: Canvas, full_colour: bool) -> InstalledSprite {
        InstalledSprite {
            full_colour,
            ..plain(canvas)
        }
    }

    fn save(canvas: Canvas, full_colour: bool) -> SpriteOp {
        SpriteOp::Save {
            frames: vec![canvas],
            frame_ms: DEFAULT_SPRITE_FRAME_MS,
            full_colour,
        }
    }

    #[test]
    fn a_colour_file_is_keyed_without_its_suffix_enabled_or_off() {
        let dir = codec_test_dir("colour_keying");
        write_png(&dir.join("on.colour.png"), &dot(12));
        write_png(&dir.join("off.colour.png.off"), &dot(13));

        let (enabled, disabled) = scan_dir(&dir);
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(enabled.get("on"), Some(&plain_or_colour(dot(12), true)));
        assert_eq!(disabled.get("off"), Some(&plain_or_colour(dot(13), true)));
        assert!(!enabled.contains_key("on.colour") && !disabled.contains_key("off.colour"));
    }

    /// The loader's own precedence (`SpriteTable::insert`): the editor must
    /// open what the map draws.
    #[test]
    fn the_colour_file_wins_when_both_variants_exist() {
        let dir = codec_test_dir("colour_precedence");
        write_png(&dir.join("both.png"), &dot(1));
        write_png(&dir.join("both.colour.png"), &dot(12));
        write_png(&dir.join("late.colour.png"), &dot(12));
        write_png(&dir.join("late.png"), &dot(1));
        write_png(&dir.join("gone.png.off"), &dot(1));
        write_png(&dir.join("gone.colour.png.off"), &dot(12));

        let (enabled, disabled) = scan_dir(&dir);
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(enabled["both"], plain_or_colour(dot(12), true));
        assert_eq!(enabled["late"], plain_or_colour(dot(12), true));
        assert_eq!(disabled["gone"], plain_or_colour(dot(12), true));
    }

    #[test]
    fn disabling_colour_art_keeps_the_plain_twin_drawing() {
        let dir = codec_test_dir("colour_disable_plain_twin");
        write_png(&dir.join("x.colour.png"), &dot(12));
        write_png(&dir.join("x.png"), &dot(1));
        write_png(&dir.join("y.colour.png"), &dot(12));

        let with_twin = apply(&dir, "x", SpriteOp::Disable { full_colour: true });
        let without = apply(&dir, "y", SpriteOp::Disable { full_colour: true });
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(with_twin, removed("x", &["x"]));
        assert_eq!(without, removed("y", &[]));
    }

    #[test]
    fn a_plain_file_is_still_plain() {
        let dir = codec_test_dir("plain_still_plain");
        write_png(&dir.join("x.png"), &dot(3));
        let (enabled, _) = scan_dir(&dir);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(enabled["x"], plain_or_colour(dot(3), false));
    }

    #[test]
    fn saving_a_colour_sprite_writes_only_the_colour_file() {
        let dir = codec_test_dir("colour_save");
        write_png(&dir.join("x.colour.png"), &dot(12));
        std::fs::write(dir.join("x.png"), b"sentinel").unwrap();

        let outcome = apply(&dir, "x", save(dot(14), true));

        let plain_bytes = std::fs::read(dir.join("x.png")).unwrap();
        let saved = png_to_canvas(&dir.join("x.colour.png"));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(outcome, loaded("x.colour"));
        assert_eq!(
            plain_bytes, b"sentinel",
            "the plain twin is not this sprite's file"
        );
        assert_eq!(saved, Some(dot(14)));
    }

    #[test]
    fn saving_a_fresh_colour_sprite_creates_no_plain_file() {
        let dir = codec_test_dir("colour_save_fresh");
        apply(&dir, "x", save(dot(14), true));
        let (colour, plain) = (
            dir.join("x.colour.png").exists(),
            dir.join("x.png").exists(),
        );
        std::fs::remove_dir_all(&dir).ok();
        assert!(colour && !plain);
    }

    #[test]
    fn disable_and_enable_move_only_the_colour_pair() {
        let dir = codec_test_dir("colour_toggle");
        write_png(&dir.join("x.colour.png"), &dot(12));
        std::fs::write(dir.join("x.png.off"), b"sentinel").unwrap();

        let disable = apply(&dir, "x", SpriteOp::Disable { full_colour: true });
        let after_disable = (
            dir.join("x.colour.png").exists(),
            dir.join("x.colour.png.off").exists(),
        );
        let enable = apply(&dir, "x", SpriteOp::Enable { full_colour: true });
        let after_enable = (
            dir.join("x.colour.png").exists(),
            dir.join("x.colour.png.off").exists(),
        );
        let sentinel = std::fs::read(dir.join("x.png.off")).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(disable, removed("x", &[]));
        assert_eq!(after_disable, (false, true));
        assert_eq!(enable, loaded("x.colour"));
        assert_eq!(after_enable, (true, false));
        assert_eq!(
            sentinel, b"sentinel",
            "the plain `.off` is a different sprite's file"
        );
    }

    #[test]
    fn a_colour_save_retires_only_its_own_stale_off_backup() {
        let dir = codec_test_dir("colour_never_both");
        write_png(&dir.join("x.colour.png.off"), &dot(12));
        std::fs::write(dir.join("x.png.off"), b"sentinel").unwrap();

        apply(&dir, "x", save(dot(14), true));

        let state = (
            dir.join("x.colour.png").exists(),
            dir.join("x.colour.png.off").exists(),
            dir.join("x.png.off").exists(),
        );
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(state, (true, false, true));
    }

    #[test]
    fn a_colour_enable_refuses_to_clobber_an_existing_colour_file() {
        let dir = codec_test_dir("colour_enable_refuse");
        write_png(&dir.join("x.colour.png.off"), &dot(13));
        write_png(&dir.join("x.colour.png"), &dot(12));

        let outcome = apply(&dir, "x", SpriteOp::Enable { full_colour: true });
        let kept = png_to_canvas(&dir.join("x.colour.png"));
        std::fs::remove_dir_all(&dir).ok();
        assert!(matches!(outcome, WriteOutcome::Failed(_)));
        assert_eq!(kept, Some(dot(12)));
    }
}
