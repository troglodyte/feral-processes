//! What a program is like, apart from what it can do in a fight — loaded
//! from `assets/attributes/`.
//!
//! One def per attribute: its name in this setting's words, the old-school
//! word beside it, two lengths of player-facing prose, and the base and
//! spread a body's own value is minted within. **Nothing reads these
//! numbers.** They are what the dossier page shows, and each one's eventual
//! mechanic is designed for the number rather than around it — see
//! `docs/superpowers/specs/2026-09-21-program-attributes-design.md` §2.
//!
//! **Data where `disposition.rs` is Rust, and that line is the whole
//! reason this directory exists.** A `Disposition` ships no name, no blurb
//! and no glyph, only multipliers on numbers the sim already computes,
//! which puts it with `tuning.rs` on the not-moddable side. An attribute is
//! the inverse: a name, a legacy name and two lines of prose, and today no
//! multiplier at all. That is content by the same test that put species,
//! needs, memories and the perk catalogue in `assets/`.
//!
//! **An empty database is valid and inert**, exactly like `NeedDb`: nothing
//! is minted, every body's store stays empty and the dossier page reports
//! no rows. Deleting `assets/attributes/` restores the pre-attribute game
//! rather than breaking an install. Never gate a mint, a reader or a draw
//! on the database being non-empty — that makes the property hold by
//! accident at one site and lapse at another.

use crate::components::Attributes;
use crate::derive;
use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// An attribute's id — a string newtype for `NeedId`'s reason: a mod's
/// attribute cannot be an enum variant. `transparent`, so a def names itself
/// in a `.ron` file as a plain quoted string rather than as
/// `AttributeId("...")`.
///
/// It derives `Ord` for `components::Attributes`' `BTreeMap` — `Stock`'s
/// reason, since iteration order feeds the save encoding and a `HashMap`
/// would make the file differ run to run.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AttributeId(String);

impl AttributeId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for AttributeId {
    fn from(s: &str) -> Self {
        AttributeId(s.to_string())
    }
}

impl std::fmt::Display for AttributeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One attribute.
///
/// These seven fields are the initial schema and every one of them is
/// **required**: an attribute with no prose is the thing this feature
/// exists to avoid, and one with no `base` cannot be minted. Any field
/// added *later* must be `#[serde(default)]`, per the standing rule for
/// `SpeciesDef`/`StructureDef`/`ItemDef`, so a mod's existing files keep
/// parsing untouched — but do not retroactively default these.
///
/// The field that is deliberately **absent** is the one saying what the
/// attribute *does*. It lands with the mechanic that does it, as a second
/// authored field, because a gloss promising an effect is a claim the
/// player will test and find false.
#[derive(Clone, Debug, Deserialize)]
pub struct AttributeDef {
    pub id: AttributeId,
    /// What the dossier row leads with, in this setting's vocabulary.
    pub name: String,
    /// The old-school word in parentheses after it — `"Willpower"`,
    /// `"Luck"`. What lets a player who has never read a line of code know
    /// what they are looking at.
    pub legacy: String,
    /// The one-line gloss beside the number.
    pub short: String,
    /// The page's prose: what a high or low value says about this program.
    /// Never what it does — see the struct doc.
    pub meaning: String,
    /// The value a body with nothing authored for it mints around.
    pub base: i32,
    /// How far either side of the base a body's own value may land. `0` is
    /// legal and means every body reads the same number.
    pub spread: i32,
}

/// Every attribute the game knows about, loaded from `assets/attributes/`.
///
/// See the module doc for why an empty database is a supported state rather
/// than an install fault.
#[derive(Resource, Default)]
pub struct AttributeDb {
    defs: BTreeMap<AttributeId, AttributeDef>,
}

impl AttributeDb {
    /// Loads every `*.ron` def in `dir`. Follows `NeedDb::load_dir` line for
    /// line: an absent directory is silent, and a malformed file costs the
    /// game that one attribute and nothing else rather than stopping a
    /// player reaching the main menu over somebody else's mod.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = AttributeDb::default();
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
        // every run — `MemoryDb::load_dir`'s rule.
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<AttributeDef>(&text) {
                Ok(def) => {
                    db.defs.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid attribute file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &AttributeId) -> Option<&AttributeDef> {
        self.defs.get(id)
    }

    /// **Sorted by id.** Every caller iterates this, and the dossier page
    /// draws it in that order; an unsorted walk is where a nondeterministic
    /// tie-break gets in.
    pub fn iter(&self) -> impl Iterator<Item = &AttributeDef> {
        self.defs.values()
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

/// This feature's own fold salt, so nothing derived here can collide with
/// anything else derived from the same world seed — `FrameSpec::salted`'s
/// rule, one scheme rather than a second seed source.
const ATTRIBUTE_SALT: u64 = 0xA771_B17E_5EED_0001;

/// A wild body's seed: the world, the tile it stood on, what it is, and how
/// deep the run had gone.
///
/// The species id is folded **last and as bytes**, so every byte of it gets
/// its own XOR-and-multiply round. That is what carries a one-character
/// difference up to bit 63, which is the bit `derive::index` actually reads
/// — see `derive::index`'s doc for why a value folded in as one whole word
/// never reaches it.
///
/// Two bodies of one species spawned on one tile in one zone mint
/// identically. Accepted: it is invisible flavour, and the alternative — a
/// spawn counter as a new `Resource` — reshuffles bevy's query iteration
/// order across the whole engine.
pub fn body_seed(world_seed: u32, x: i32, y: i32, species: &str, zone: u32) -> u64 {
    let h = derive::fold(
        derive::FNV_BASIS,
        &[
            ATTRIBUTE_SALT,
            world_seed as u64,
            x as i64 as u64,
            y as i64 as u64,
            zone as u64,
        ],
    );
    derive::fold_bytes(h, species.as_bytes())
}

/// A program with no place: one fused from two parents, minted off the
/// `ProgramId` `Game::roster_parts` just handed it. `Disposition::seed`'s
/// own input, for its reason.
pub fn program_seed(program_id: u32) -> u64 {
    derive::fold(derive::FNV_BASIS, &[ATTRIBUTE_SALT, program_id as u64])
}

/// The player, who has no species and no spawn tile worth folding.
pub fn player_seed(world_seed: u32) -> u64 {
    derive::fold(derive::FNV_BASIS, &[ATTRIBUTE_SALT, world_seed as u64])
}

/// Every attribute the catalogue knows about, minted for one body.
///
/// `authored` is the body's own base per attribute — a species' or a
/// class's `attributes:` map — and an id it does not name falls through to
/// the def's own `base`. Keyed by `String` rather than `AttributeId`
/// because that is the shape a `#[serde(default)]` field on `SpeciesDef`
/// takes without either db depending on the other.
///
/// **Spends no `resources::GameRng` draw.** A draw does not survive a
/// save/load, so the same program would read differently after a reload,
/// and a fourth draw at the wild spawn would shift every later roll in the
/// run — moving seed-luck tests, arena baselines and `balance_sim`'s
/// reference numbers for a cosmetic value.
///
/// Each attribute salts the body's seed with **its own id** rather than
/// reading a second slice of one number, `CaravanVisit`'s rule: a sixth
/// attribute dropped into the directory must not move the five beside it.
pub fn mint(db: &AttributeDb, seed: u64, authored: &BTreeMap<String, i32>) -> Attributes {
    let mut out = Attributes::default();
    for def in db.iter() {
        let base = authored.get(def.id.as_str()).copied().unwrap_or(def.base);
        // `spread: 0` gives a span of one, which `index` answers `0` for —
        // a fixed attribute, and no special case.
        let span = (2 * def.spread + 1).max(1) as usize;
        let offset = derive::index(derive::fold_bytes(seed, def.id.as_str().as_bytes()), span);
        out.set(&def.id, base - def.spread + offset as i32);
    }
    out
}

/// A build revision for one body — `"rev 4.17"`.
///
/// Derived, never stored, `handles::of`'s model: no pool, no storage and no
/// content, so a save written before this feature has one the moment it
/// loads. Each half folds the seed with **its own tag** rather than reading
/// two slices of one number, so neither can move the other.
pub fn revision(seed: u64) -> String {
    let major = 1 + derive::index(derive::fold_bytes(seed, b"rev.major"), 9);
    let minor = derive::index(derive::fold_bytes(seed, b"rev.minor"), 100);
    format!("rev {major}.{minor}")
}

/// A checksum for one body — `"0x3F9A21"`, six hex digits.
///
/// The high bits of its own fold, for `derive::index`'s reason: the low
/// bits of a fold are the ones the final multiply provably never disturbs,
/// so a mask over them would leave two neighbouring bodies rhyming.
pub fn checksum(seed: u64) -> String {
    let h = derive::fold_bytes(seed, b"checksum");
    format!("0x{:06X}", ((h >> 40) as u32) & 0x00FF_FFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A well-formed def, differing only in the fields a test cares about.
    fn def_text(id: &str, name: &str) -> String {
        format!(
            "(\n    id: \"{id}\",\n    name: \"{name}\",\n    legacy: \"Willpower\",\n    \
             short: \"holds together under stress\",\n    meaning: \"How cleanly this \
             program keeps its own state.\",\n    base: 50,\n    spread: 12,\n)\n"
        )
    }

    fn load(files: &[(&str, String)]) -> (AttributeDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir("attributes");
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        AttributeDb::load_dir(&dir).unwrap()
    }

    #[test]
    fn the_shipped_defs_load_and_are_found_by_id() {
        let (db, warnings) =
            AttributeDb::load_dir(&crate::tests::support::test_assets_dir().join("attributes"))
                .unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        for id in ["persistence", "entropy", "bandwidth", "footprint", "parity"] {
            let def = db
                .get(&AttributeId::from(id))
                .unwrap_or_else(|| panic!("{id}"));
            assert!(
                def.spread < def.base,
                "{id}: a spread at or above the base can mint a negative attribute"
            );
            assert!(!def.legacy.is_empty(), "{id} must name its old-school word");
        }
    }

    #[test]
    fn a_malformed_file_is_skipped_and_warns_without_losing_its_neighbours() {
        let (db, warnings) = load(&[
            ("bad.ron", "(id: \"broken\", name:".to_string()),
            ("good.ron", def_text("persistence", "Persistence")),
        ]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("bad.ron"), "{warnings:?}");
        assert!(db.get(&AttributeId::from("persistence")).is_some());
        assert!(db.get(&AttributeId::from("broken")).is_none());
    }

    /// Deleting `assets/attributes/` is a supported way to play, so an
    /// absent directory is not even a warning.
    #[test]
    fn an_absent_directory_loads_an_empty_database_silently() {
        let dir = crate::tests::support::scratch_assets_dir("attributes_absent");
        assert!(!dir.exists(), "the fixture must not create the directory");
        let (db, warnings) =
            AttributeDb::load_dir(&dir).expect("an absent directory is not an error");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(db.iter().count(), 0);
    }

    /// Every caller iterates `iter`, and the page draws it in that order, so
    /// it is sorted by id whatever order the directory hands its entries
    /// back in.
    #[test]
    fn iteration_is_in_id_order_however_the_files_were_written() {
        let (db, warnings) = load(&[
            ("z.ron", def_text("parity", "Parity")),
            ("a.ron", def_text("bandwidth", "Bandwidth")),
        ]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let ids: Vec<&str> = db.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["bandwidth", "parity"]);
    }

    fn two_defs() -> AttributeDb {
        let (db, _) = load(&[
            ("a.ron", def_text("bandwidth", "Bandwidth")),
            ("b.ron", def_text("entropy", "Entropy")),
        ]);
        db
    }

    /// The property every seeded baseline in the repo rests on, at this
    /// level: the same place and the same body always mint the same
    /// numbers, so a save/load cannot change who a program is.
    #[test]
    fn minting_is_deterministic() {
        let db = two_defs();
        let authored = BTreeMap::new();
        let seed = body_seed(1234, -7, 19, "crawler", 3);
        assert_eq!(mint(&db, seed, &authored), mint(&db, seed, &authored));
    }

    /// Every minted value lands inside the def's own window, including at
    /// the window's edges, so no reader needs to clamp.
    #[test]
    fn every_minted_value_is_inside_base_plus_or_minus_spread() {
        let db = two_defs();
        let authored = BTreeMap::new();
        for n in 0..500u32 {
            let attrs = mint(
                &db,
                body_seed(n, n as i32, -(n as i32), "crawler", 1),
                &authored,
            );
            for (id, value) in attrs.iter() {
                let def = db.get(id).unwrap();
                assert!(
                    (def.base - def.spread..=def.base + def.spread).contains(&value),
                    "{id} minted {value} outside {}±{}",
                    def.base,
                    def.spread
                );
            }
        }
    }

    /// An authored base replaces the def's, and the spread still applies
    /// around it — which is what makes a species' own numbers show through
    /// while two of that species still differ.
    #[test]
    fn an_authored_base_replaces_the_defs_own() {
        let db = two_defs();
        let mut authored = BTreeMap::new();
        authored.insert("bandwidth".to_string(), 90);
        let def = db.get(&AttributeId::from("bandwidth")).unwrap();
        for n in 0..200u32 {
            let value = mint(&db, body_seed(n, 0, 0, "crawler", 1), &authored)
                .get(&AttributeId::from("bandwidth"))
                .unwrap();
            assert!(
                (90 - def.spread..=90 + def.spread).contains(&value),
                "authored base ignored: minted {value}"
            );
        }
    }

    /// Two attributes must not move in lockstep. `derive::index` reads the
    /// high bits and each attribute salts the body's seed with its own id,
    /// so a two-entry catalogue decorrelates the same as a larger one —
    /// the trap `descriptions::Slot::tags` carries the measurement for, and
    /// the one a `%` reduction passes every casual test while failing.
    #[test]
    fn two_attributes_do_not_move_in_lockstep() {
        let db = two_defs();
        let authored = BTreeMap::new();
        let mut joint = std::collections::HashSet::new();
        for n in 0..2000u32 {
            let attrs = mint(&db, body_seed(n, 0, 0, "crawler", 1), &authored);
            let a = attrs.get(&AttributeId::from("bandwidth")).unwrap();
            let b = attrs.get(&AttributeId::from("entropy")).unwrap();
            joint.insert((a > 50, b > 50));
        }
        assert_eq!(
            joint.len(),
            4,
            "two attributes reached only {} of 4 joint outcomes; the \
             reduction is correlating them",
            joint.len()
        );
    }

    /// An empty catalogue mints an empty store, with no branch at any call
    /// site — the pre-attribute game.
    #[test]
    fn an_empty_catalogue_mints_an_empty_store() {
        let db = AttributeDb::default();
        let attrs = mint(&db, body_seed(1, 2, 3, "crawler", 1), &BTreeMap::new());
        assert!(attrs.is_empty());
    }

    /// Pinned, `handles_are_pinned`'s reason: a derived label with no pool
    /// and no storage is exactly the kind of function a refactor can
    /// quietly turn into a constant, and these assertions are the only
    /// thing that would notice.
    #[test]
    fn the_header_is_pinned() {
        let seed = body_seed(7, 3, -4, "crawler", 2);
        assert_eq!(revision(seed), revision(seed));
        assert_eq!(checksum(seed), checksum(seed));
        assert!(revision(seed).starts_with("rev "), "{}", revision(seed));
        assert_eq!(checksum(seed).len(), 8, "{}", checksum(seed));
        assert!(checksum(seed).starts_with("0x"), "{}", checksum(seed));
    }

    /// Two bodies do not share a header. Both halves are folded off the
    /// body's own seed with their own tag, `CaravanVisit`'s rule, so
    /// neither can shift the other.
    #[test]
    fn two_bodies_get_two_headers() {
        let mut revisions = std::collections::HashSet::new();
        let mut checksums = std::collections::HashSet::new();
        for n in 0..500u32 {
            let seed = body_seed(n, 0, 0, "crawler", 1);
            revisions.insert(revision(seed));
            checksums.insert(checksum(seed));
        }
        assert!(
            revisions.len() > 100,
            "only {} revisions in 500",
            revisions.len()
        );
        assert!(
            checksums.len() > 450,
            "only {} checksums in 500",
            checksums.len()
        );
    }
}
