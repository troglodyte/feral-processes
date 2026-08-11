//! Measuring a candidate roster by fighting it.
//!
//! The whole integration seam with the game is `arena::run`'s `assets_dir`
//! parameter: write a candidate's species files into a scratch install, run
//! the real fight against it, read the summary. The engine needs no change
//! and learns nothing about this tool.
//!
//! Two costs shape the design. Copying ~190 asset files per candidate would
//! be 38,000 file copies over a 200-iteration run, so the workspace is built
//! **once** and only its species files are rewritten per candidate — from
//! cached pristine text, so a patch is never applied on top of a patch. And
//! the scratch install cleans itself up on `Drop` rather than at the end of
//! a success path: the engine's own test fixtures leaked 53 of these per run
//! by relying on the latter, exhausted the machine's inodes, and failed
//! builds with an error naming none of it.

use super::roster::Candidate;
use super::score::Target;
use feral_processes_engine::arena::{self, Scenario, Summary};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A scratch asset install a candidate roster is written into.
pub struct Workspace {
    dir: PathBuf,
    /// species id -> (file name, original text). Patches always apply to
    /// the original, never to the previous candidate's output.
    pristine: BTreeMap<String, (String, String)>,
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Workspace {
    /// Copies `assets_dir` into a fresh scratch install.
    pub fn new(assets_dir: &Path) -> Result<Workspace, String> {
        let dir = std::env::temp_dir().join(format!(
            "feral_processes_tuner_{}_{}",
            std::process::id(),
            // Distinguishes concurrent workspaces in one process without a
            // clock, which a reproducible tool has no business reading.
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        copy_tree(assets_dir, &dir)?;

        let mut pristine = BTreeMap::new();
        let species_dir = dir.join("species");
        for entry in std::fs::read_dir(&species_dir)
            .map_err(|e| format!("cannot read {}: {e}", species_dir.display()))?
        {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let Some(id) = species_id(&text) else {
                continue;
            };
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            pristine.insert(id, (name, text));
        }
        if pristine.is_empty() {
            return Err(format!("no species files under {}", species_dir.display()));
        }
        Ok(Workspace { dir, pristine })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The shipped value of every movable field, per species — the point
    /// the search starts from.
    ///
    /// Species in `frozen` are left out of the candidate entirely rather
    /// than being filtered later. That is what makes the freeze structural:
    /// a species the candidate does not carry cannot be chosen by `perturb`,
    /// cannot be written by `apply`, and cannot be listed by `summary`, so a
    /// second search operator added later cannot forget to honour it. See
    /// `sides::player_fielded` for what goes in the set and why.
    pub fn baseline(&self, frozen: &BTreeSet<String>) -> Candidate {
        let mut species = BTreeMap::new();
        for (id, (_, text)) in &self.pristine {
            if frozen.contains(id) {
                continue;
            }
            species.insert(id.clone(), super::roster::read_fields(text));
        }
        Candidate { species }
    }

    /// Writes `candidate` into the install, each file patched from its
    /// original text.
    pub fn apply(&self, candidate: &Candidate) -> Result<(), String> {
        for (id, changes) in &candidate.species {
            let Some((name, original)) = self.pristine.get(id) else {
                return Err(format!("candidate names unknown species {id:?}"));
            };
            let patched = super::roster::patch_species(original, changes);
            std::fs::write(self.dir.join("species").join(name), patched)
                .map_err(|e| format!("cannot write {name}: {e}"))?;
        }
        Ok(())
    }

    /// Every species in the install, as the engine would read them.
    ///
    /// Parsed from what is on disk rather than from the candidate, so the
    /// constraint check sees the same bytes the fight will.
    pub fn species_defs(&self) -> Result<Vec<feral_processes_engine::species::SpeciesDef>, String> {
        let mut defs = Vec::new();
        for (name, _) in self.pristine.values() {
            let path = self.dir.join("species").join(name);
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let def = ron::from_str(&text).map_err(|e| format!("{name}: {e}"))?;
            defs.push(def);
        }
        Ok(defs)
    }
}

static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Fights every target against whatever is currently in `workspace`.
///
/// `reps` and `seed_base` override each scenario file's own, because a
/// hand-built scenario is usually `reps: 1` for the transcript and because
/// candidates must be compared on identical fights — a candidate scored on
/// different seeds than its rival is scored on noise.
pub fn measure(
    workspace: &Workspace,
    targets: &[Target],
    reps: u32,
    seed_base: u64,
) -> Result<Vec<Summary>, String> {
    let mut summaries = Vec::with_capacity(targets.len());
    for target in targets {
        let mut scenario = Scenario::load(Path::new(&target.scenario))
            .map_err(|e| format!("{}: {e}", target.scenario))?;
        // A scenario naming a template has to become a `Save` before the
        // engine sees it. Shared with the `arena` bin rather than copied:
        // a scenario that runs there must measure identically here.
        crate::dev_template::resolve_scenario(&mut scenario)
            .map_err(|e| format!("{}: {e}", target.scenario))?;
        scenario.reps = reps;
        scenario.seed = seed_base;
        // No telemetry: a search runs this scenario thousands of times, and
        // recording every one of those fights is exactly what
        // `RunOptions::telemetry` documents it is not for.
        let (report, _) = arena::run(&scenario, workspace.dir(), arena::RunOptions::default())
            .map_err(|e| format!("{}: {e}", target.scenario))?;
        summaries.push(report.summary());
    }
    Ok(summaries)
}

/// The `id: "..."` a species file declares.
fn species_id(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let rest = line.trim_start().strip_prefix("id:")?;
        Some(
            rest.trim()
                .trim_end_matches(',')
                .trim_matches('"')
                .to_string(),
        )
    })
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    for entry in
        std::fs::read_dir(from).map_err(|e| format!("cannot read {}: {e}", from.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("cannot copy {}: {e}", src.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuner::roster::Field;

    fn assets() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    #[test]
    fn a_workspace_mirrors_the_shipped_species() {
        let ws = Workspace::new(&assets()).unwrap();
        let defs = ws.species_defs().unwrap();
        assert!(defs.len() > 5, "got {} species", defs.len());
        assert!(defs.iter().any(|d| d.id == "drone"));
    }

    #[test]
    fn the_baseline_reads_the_shipped_numbers() {
        let ws = Workspace::new(&assets()).unwrap();
        let base = ws.baseline(&BTreeSet::new());
        let drone = base.species.get("drone").expect("drone is shipped");
        assert!(drone.iter().any(|(f, _)| *f == Field::BaseHp));
    }

    #[test]
    fn a_frozen_species_never_enters_the_candidate() {
        // Structural rather than a check the search has to remember: a
        // species that is not in the candidate cannot be picked by
        // `perturb`, cannot be written by `apply`, and cannot appear in the
        // report. A second search operator added later inherits all three
        // for free.
        let mut frozen = BTreeSet::new();
        frozen.insert("scrapper".to_string());

        let ws = Workspace::new(&assets()).unwrap();
        let base = ws.baseline(&frozen);
        assert!(!base.species.contains_key("scrapper"));
        assert!(
            base.species.contains_key("drone"),
            "freezing one species must not empty the roster"
        );
    }

    #[test]
    fn freezing_every_species_leaves_an_empty_candidate() {
        // `run` turns this into an error rather than letting the search
        // spend its whole iteration budget re-evaluating one roster.
        let ws = Workspace::new(&assets()).unwrap();
        let all: BTreeSet<String> = ws.baseline(&BTreeSet::new()).species.into_keys().collect();
        assert!(ws.baseline(&all).species.is_empty());
    }

    #[test]
    fn applying_a_candidate_changes_what_the_engine_reads() {
        let ws = Workspace::new(&assets()).unwrap();
        let mut candidate = ws.baseline(&BTreeSet::new());
        candidate
            .species
            .insert("drone".into(), vec![(Field::BaseHp, 137.0)]);
        ws.apply(&candidate).unwrap();

        let drone = ws
            .species_defs()
            .unwrap()
            .into_iter()
            .find(|d| d.id == "drone")
            .unwrap();
        assert_eq!(drone.base_hp, 137);
    }

    #[test]
    fn a_patch_applies_to_the_original_never_to_the_previous_patch() {
        // Applying candidates in sequence must not compound. Without the
        // pristine cache the second apply would patch the first's output,
        // which happens to be harmless for a replace but is exactly how an
        // insert-path field would be written twice.
        let ws = Workspace::new(&assets()).unwrap();
        let mut candidate = ws.baseline(&BTreeSet::new());
        for hp in [200.0, 55.0] {
            candidate
                .species
                .insert("drone".into(), vec![(Field::BaseHp, hp)]);
            ws.apply(&candidate).unwrap();
        }
        let drone = ws
            .species_defs()
            .unwrap()
            .into_iter()
            .find(|d| d.id == "drone")
            .unwrap();
        assert_eq!(drone.base_hp, 55);

        let text = std::fs::read_to_string(ws.dir().join("species").join("drone.ron")).unwrap();
        assert_eq!(
            text.lines().filter(|l| l.contains("base_hp:")).count(),
            1,
            "base_hp written more than once:\n{text}"
        );
    }

    #[test]
    fn a_candidate_naming_an_unknown_species_is_an_error() {
        let ws = Workspace::new(&assets()).unwrap();
        let mut candidate = Candidate::default();
        candidate
            .species
            .insert("no_such_program".into(), vec![(Field::BaseHp, 10.0)]);
        assert!(ws.apply(&candidate).is_err());
    }

    #[test]
    fn a_workspace_removes_itself_when_dropped() {
        // The bug this tool was written next to: 5,437 of these accumulated
        // in /tmp from fixtures that cleaned up only on the success path.
        let path = {
            let ws = Workspace::new(&assets()).unwrap();
            ws.dir().to_path_buf()
        };
        assert!(!path.exists(), "{} outlived its Workspace", path.display());
    }
}
