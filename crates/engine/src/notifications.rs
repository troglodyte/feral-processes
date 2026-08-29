//! What takes the whole screen for a moment, loaded from
//! `assets/notifications/`.
//!
//! One def per notification: a title, a paragraph, the art or glyph that
//! stands over it, and whether it may ever fire twice. The catalogue is
//! **data** and the triggers are Rust — the same half-data seam
//! `needs::NeedDef` and `memories::MemoryDef` sit on, and for the same
//! reason: *when* a notification fires is a hook into a particular moment in
//! a particular function, not something a `.ron` file can express.
//! `assets/notifications/README.md` is the schema reference.
//!
//! **An empty database is valid and inert**, exactly like `NeedDb`: nothing
//! is queued, no screen ever opens, and `Game::notify` answers a refusal
//! without a branch anywhere else. Deleting `assets/notifications/` restores
//! the pre-notification game rather than breaking an install, which is why an
//! absent directory is silent here. Never gate a trigger, a system or the
//! screen on the database being non-empty — that makes the property hold by
//! accident at one site and lapse at another.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::GlyphColor;

/// A notification's id — a string newtype for `NeedId`'s reason: a mod's
/// notification cannot be an enum variant. `transparent`, so a def names
/// itself in a `.ron` file as a plain quoted string.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NotificationId(String);

impl NotificationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NotificationId {
    fn from(s: &str) -> Self {
        NotificationId(s.to_string())
    }
}

impl std::fmt::Display for NotificationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether a notification may fire more than once.
///
/// Two policies rather than three. The obvious third — once per *run* —
/// was considered and rejected: its latch would have to live on the
/// session-only queue resource, so "once per run" would quietly mean "once
/// per session" and fire again on every reload. A name that lies is worse
/// than a missing policy, and this is an additive variant the day something
/// actually wants it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum Repeat {
    /// Fires every time its site is reached. A breach, a contract landing —
    /// news about *this* moment, which has happened again.
    #[default]
    Always,
    /// Fires once and never again, on this machine, across every run. The
    /// latch is `achievements::Profile::seen_notifications`, which is not the
    /// save file — so a tutorial seen in one run stays seen in the next.
    OnceEver,
}

/// One notification.
///
/// `id`, `title` and `body` are **required**: a def missing any of them is a
/// def with nothing to say. Everything else is `#[serde(default)]` from the
/// start, and any field added *later* must be too, per the standing rule for
/// `SpeciesDef`/`StructureDef`/`ItemDef`, so a mod's existing files keep
/// parsing untouched.
#[derive(Clone, Debug, Deserialize)]
pub struct NotificationDef {
    pub id: NotificationId,
    /// The heading. Short — it is drawn large and does not wrap.
    pub title: String,
    /// The paragraph under it, wrapped at draw time through `text::wrap`.
    pub body: String,
    /// A name in `assets/sprites/`. **Optional by construction**: the sprite
    /// *substitutes* for `glyph` and never draws beside it, and a name the
    /// texture table has nothing under falls back to the glyph — the
    /// `Painter::sprite` seam's own rule, inherited unchanged.
    #[serde(default)]
    pub sprite: Option<String>,
    /// What is drawn when there is no sprite, which today is always.
    #[serde(default = "default_glyph")]
    pub glyph: char,
    /// Resolved through `hud::palette::glyph`, the one table a content hue is
    /// drawn from. The renderer does not invent a colour for this screen.
    ///
    /// A **named** default rather than a bare `#[serde(default)]`:
    /// `GlyphColor` has no `Default`, and giving a shared component one just
    /// to shorten this line would put a meaning on it at every other site
    /// that reads it.
    #[serde(default = "default_color")]
    pub color: GlyphColor,
    #[serde(default)]
    pub repeat: Repeat,
}

fn default_glyph() -> char {
    '!'
}

fn default_color() -> GlyphColor {
    GlyphColor::White
}

/// A notification the queue is holding: the **whole resolved def**, never an
/// id.
///
/// `ActiveContract`'s rule and `Sortie`'s. What travels is the finished text,
/// so a `.ron` file edited or deleted between the push and the draw cannot
/// strand or silently rewrite something already queued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub sprite: Option<String>,
    pub glyph: char,
    pub color: GlyphColor,
}

impl From<&NotificationDef> for Notification {
    fn from(def: &NotificationDef) -> Self {
        Notification {
            title: def.title.clone(),
            body: def.body.clone(),
            sprite: def.sprite.clone(),
            glyph: def.glyph,
            color: def.color,
        }
    }
}

/// Every notification the game knows about, loaded from
/// `assets/notifications/`.
///
/// See the module doc for why an empty database is a supported state rather
/// than an install fault.
#[derive(Resource, Default)]
pub struct NotificationDb {
    defs: BTreeMap<NotificationId, NotificationDef>,
}

impl NotificationDb {
    /// Loads every `*.ron` def in `dir`. Follows `NeedDb::load_dir` line for
    /// line: an absent directory is silent, and a malformed file costs the
    /// game that one notification and nothing else rather than stopping a
    /// player reaching the main menu over somebody else's mod.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = NotificationDb::default();
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        // Sorted, because two files claiming one id must resolve the same way
        // every run — `NeedDb::load_dir`'s rule.
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<NotificationDef>(&text) {
                Ok(def) => {
                    db.defs.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid notification file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &NotificationId) -> Option<&NotificationDef> {
        self.defs.get(id)
    }

    /// **Sorted by id.** Every caller iterates this; an unsorted walk is where
    /// a nondeterministic tie-break gets in.
    pub fn iter(&self) -> impl Iterator<Item = &NotificationDef> {
        self.defs.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def_text(id: &str, title: &str) -> String {
        format!("(\n    id: \"{id}\",\n    title: \"{title}\",\n    body: \"Some prose.\",\n)\n")
    }

    fn load(files: &[(&str, String)]) -> (NotificationDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir("notifications");
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        NotificationDb::load_dir(&dir).unwrap()
    }

    #[test]
    fn a_malformed_file_is_skipped_and_warns_without_losing_its_neighbours() {
        let (db, warnings) = load(&[
            ("bad.ron", "(id: \"broken\", title:".to_string()),
            ("good.ron", def_text("milestone_breach", "New Sector")),
        ]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("bad.ron"), "{warnings:?}");
        assert!(db.get(&NotificationId::from("milestone_breach")).is_some());
        assert!(db.get(&NotificationId::from("broken")).is_none());
    }

    /// Deleting `assets/notifications/` is a supported way to play, so an
    /// absent directory is not even a warning.
    #[test]
    fn an_absent_directory_loads_an_empty_database_silently() {
        let dir = crate::tests::support::scratch_assets_dir("notifications_absent");
        assert!(!dir.exists(), "the fixture must not create the directory");
        let (db, warnings) =
            NotificationDb::load_dir(&dir).expect("an absent directory is not an error");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(db.iter().count(), 0);
    }

    #[test]
    fn iteration_is_in_id_order_however_the_files_were_written() {
        let (db, warnings) = load(&[
            ("z.ron", def_text("tutorial_first_raid", "Sweep")),
            ("a.ron", def_text("milestone_breach", "New Sector")),
        ]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let ids: Vec<&str> = db.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["milestone_breach", "tutorial_first_raid"]);
    }

    /// Only three fields are required. A mod shipping the minimum must parse,
    /// or every later `#[serde(default)]` promise is already broken.
    #[test]
    fn a_def_naming_only_the_three_required_fields_parses() {
        let (db, warnings) = load(&[("m.ron", def_text("bare", "Bare"))]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let def = db.get(&NotificationId::from("bare")).unwrap();
        assert!(def.sprite.is_none());
        assert_eq!(def.repeat, Repeat::Always, "free by default");
    }
}
