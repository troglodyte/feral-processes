//! What the rotten patches of a Stack frame have to say for themselves.
//!
//! Content, like species and items — `assets/crash_logs/*.ron`, one file
//! per entry, so adding a line is a file drop rather than a code change.
//! Read by `Game::listen` when the party is standing on a `CellKind::Fault`
//! or `CellKind::Corruption`; nothing else consults it.
//!
//! **Which line a given patch of rot reads is a property of the place** —
//! derived from the zone, the depth and the cell, never from
//! `resources::GameRng`. Two reasons, both already learned here: a draw
//! from the shared stream does not survive a save/load, so the same
//! corrupted tile would say something different after a reload; and
//! drawing from it to pick a cosmetic string shifts every later roll in
//! the run. `Game::orphan_species` follows the same rule for the same
//! reasons.

use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::Deserialize;

/// One crash-log file: an id, and the lines it contributes to the pool.
#[derive(Clone, Debug, Deserialize)]
pub struct CrashLogDef {
    pub id: String,
    #[serde(default)]
    pub lines: Vec<String>,
}

/// Every shipped and modded crash-log line, flattened into one pool.
///
/// A flat `Vec` rather than a map keyed by id because nothing ever asks for
/// a *particular* entry — the id exists to give the pool a stable order and
/// to name a file in a warning, not to be looked up.
#[derive(Resource, Default)]
pub struct CrashLogDb {
    lines: Vec<String>,
}

impl CrashLogDb {
    /// Loads every `*.ron` crash log in `dir`. A malformed file is skipped
    /// with a returned warning rather than aborting the load, same as
    /// `AbilityDb::load_dir`.
    ///
    /// **The pool is sorted by id, not by directory order.**
    /// `std::fs::read_dir` returns entries in no defined order, so without
    /// the sort the same cell would read a different line between runs and
    /// across a reload — the whole property `line_for` exists to provide,
    /// lost to something no test of `line_for` alone would see. The
    /// `assembler_system` position sort exists against the same class of
    /// bug.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut defs: Vec<CrashLogDef> = Vec::new();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<CrashLogDef>(&text) {
                Ok(def) => defs.push(def),
                Err(e) => warnings.push(format!("skipped invalid crash log file {path:?}: {e}")),
            }
        }
        defs.sort_by(|a, b| a.id.cmp(&b.id));
        let lines = defs.into_iter().flat_map(|def| def.lines).collect();
        Ok((CrashLogDb { lines }, warnings))
    }

    /// Every loaded line, in pool order.
    pub fn all(&self) -> &[String] {
        &self.lines
    }

    /// What the rot on `cell`, `depth` frames under zone `zone`, says —
    /// or `None` when no crash logs are loaded at all, which is a mod's
    /// prerogative and must leave the key working rather than divide by
    /// zero.
    ///
    /// A fixed mix of the four inputs rather than a hash, so the answer is
    /// stable across builds as well as across a reload. `rem_euclid`
    /// rather than `%` because frame coordinates are signed.
    pub fn line_for(&self, zone: u32, depth: u32, cell: (i32, i32)) -> Option<&str> {
        if self.lines.is_empty() {
            return None;
        }
        let mixed = zone as i64 * 31 + depth as i64 * 17 + cell.0 as i64 * 7 + cell.1 as i64 * 3;
        let index = mixed.rem_euclid(self.lines.len() as i64) as usize;
        Some(&self.lines[index])
    }
}
