//! What a nemesis is called, and what it says when a fight with it opens.
//!
//! Content, the way `descriptions.rs` and `sectors.rs` are: two `.ron` line
//! banks under `assets/nemesis/` — `names.ron` and `taunts.ron` — loaded by
//! `NemesisDb::load_dir` with the same skip-and-warn discipline every other
//! `load_dir` in this crate follows, per file rather than per directory, so
//! a mod can ship one bank without the other.
//!
//! **Selection never draws from `resources::GameRng`.** A name is chosen
//! inside `Game::mark_nemeses`, which `end_battle` runs at the close of
//! every fight including every arena scenario — a draw there would shift
//! the RNG stream for every scenario in `dev-arenas/`, and would not
//! survive a save/load either. It also never leans on an `StdRng` sequence,
//! whose output is not guaranteed stable across a `rand` upgrade. So both
//! picks fold the values they are derived from and reduce with
//! `derive::index`, the same shared reducer `descriptions.rs` and
//! `sectors.rs` use — see that module's doc for why `%` is the wrong tool.

use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::Deserialize;

use crate::components::Potential;
use crate::derive::index;

/// One bank file: a flat pool of lines, nothing else. Unlike
/// `descriptions::DescriptionDef` there is no subject or condition to key
/// on — a nemesis draws from one shared pool regardless of species, the
/// same shared-voice call the design doc makes for both banks (only 4 of
/// 17 shipped species author `SpeciesDef::taunts` today, so a per-species
/// nemesis pool would read generic for most of the roster).
#[derive(Clone, Debug, Default, Deserialize)]
struct LineBank {
    #[serde(default)]
    lines: Vec<String>,
}

/// The name pool and the taunt pool, loaded from `assets/nemesis/`.
///
/// **An empty database is valid and inert**, exactly like `SectorDb`:
/// `name` and `taunt` both return `None`, and `Game::mark_nemeses` already
/// treats a `None` name as "mark it, promote it, recharge it — just don't
/// rename it" rather than a fault. Deleting `assets/nemesis/` restores the
/// pre-nemesis game's naming, the same supported way to play that deleting
/// `assets/sectors/` or `assets/policies/enemy_battle.ron` is.
#[derive(Resource, Default)]
pub struct NemesisDb {
    names: Vec<String>,
    taunts: Vec<String>,
}

impl NemesisDb {
    /// Loads `names.ron` and `taunts.ron` from `dir`. Follows every other
    /// `load_dir` in the crate: a missing or malformed file is skipped with
    /// a warning rather than aborting startup — see
    /// `DescriptionDb::load_dir`. Unlike that one, there is no directory
    /// walk: the two files are named rather than discovered, because they
    /// are two different *kinds* of pool (a name and a taunt are not
    /// mergeable variants of one subject the way two `.ron` files
    /// describing `stack.door` are).
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut warnings = Vec::new();
        let names = load_bank(&dir.join("names.ron"), &mut warnings);
        let taunts = load_bank(&dir.join("taunts.ron"), &mut warnings);
        Ok((NemesisDb { names, taunts }, warnings))
    }

    /// The name `seed` derives, or `None` on an empty pool — which is a
    /// mod's prerogative (see the struct doc) and leaves `Game::mark_
    /// nemeses` to write nothing rather than fail.
    pub fn name(&self, seed: u64) -> Option<&str> {
        pick(&self.names, seed)
    }

    /// The taunt line for a nemesis with `grudges` grudges, or `None` on an
    /// empty pool. Folds `grudges` into `seed` itself — byte-wise, the same
    /// way `name_seed` folds its own inputs and for the same high-bit
    /// reason (see the module doc and `derive::index`) — so the fold lives
    /// in exactly one place rather than being repeated, possibly
    /// differently, at every call site.
    pub fn taunt(&self, seed: u64, grudges: u32) -> Option<&str> {
        let mut h = seed;
        for byte in (grudges as u64).to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        pick(&self.taunts, h)
    }
}

/// Loads one line bank, pushing a warning onto `warnings` and returning an
/// empty pool rather than failing the whole database. A missing file is
/// silent — the same "absent is the pre-feature game" rule `SectorDb::
/// load_dir` states for a missing directory, applied per file instead of
/// per directory: `names.ron` missing while `taunts.ron` ships (or the
/// reverse) is still a usable, half-authored install rather than an
/// all-or-nothing one.
fn load_bank(path: &Path, warnings: &mut Vec<String>) -> Vec<String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            warnings.push(format!("skipped unreadable nemesis file {path:?}: {e}"));
            return Vec::new();
        }
    };
    match ron::from_str::<LineBank>(&text) {
        Ok(bank) => bank.lines,
        Err(e) => {
            warnings.push(format!("skipped invalid nemesis file {path:?}: {e}"));
            Vec::new()
        }
    }
}

/// Folds a species id and an individual's `Potential` rolls into the seed
/// `NemesisDb::name` indexes by — a property of *which program this is*,
/// not of the moment it was marked. Two nemeses of the same species get
/// different names because their rolls differ; the same nemesis re-derived
/// gets the same name, though nothing depends on that holding across a
/// bank edit or a `rand` upgrade, since this is arithmetic rather than an
/// `StdRng` sequence.
///
/// FNV-1a, folded a byte at a time for the reason `descriptions::Slot::tags`
/// documents and measures: one XOR-then-multiply round only carries a
/// difference about the prime's own width (~41 bits) upward, so folding
/// each roll in as a single 32-bit word would leave two close rolls landing
/// on the same high bits — which is the bit `derive::index` actually reads.
pub fn name_seed(species: &str, potential: &Potential) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for byte in species.as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    for roll in [
        potential.hp_roll,
        potential.atk_roll,
        potential.def_roll,
        potential.growth_roll,
    ] {
        for byte in roll.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// Indexes `pool` by `seed`, or `None` when the pool is empty — legal for
/// either bank, and how a mod-free/partially-authored install behaves.
fn pick(pool: &[String], seed: u64) -> Option<&str> {
    if pool.is_empty() {
        return None;
    }
    Some(&pool[index(seed, pool.len())])
}
