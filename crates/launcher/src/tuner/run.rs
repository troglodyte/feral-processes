//! The whole tuner as one call: build a workspace, climb, validate the
//! winner on unseen seeds, write the proposal.
//!
//! Kept out of the bin so it can be driven by a test at a small iteration
//! count. `main` is argument parsing and printing.

use super::objective::Objective;
use super::roster::Candidate;
use super::{constraints, eval, score, search};
use std::path::Path;

pub struct Proposal {
    pub candidate: Candidate,
    /// The shipped roster the search started from. Carried so the report
    /// can show what *moved* rather than listing every field of every
    /// species, most of which a candidate leaves untouched.
    pub baseline: Candidate,
    pub outcome: search::Outcome,
    /// Per-target win rates on seeds the search never saw, for the shipped
    /// roster and for the proposal.
    pub holdout_before: Vec<f32>,
    pub holdout_after: Vec<f32>,
    pub holdout_error_before: f32,
    pub holdout_error_after: f32,
}

impl Proposal {
    /// Whether the proposal beat the shipped roster on seeds it was not
    /// tuned against.
    ///
    /// The search compares candidates on one pinned seed set so differences
    /// are the roster rather than noise — which is exactly the setup that
    /// rewards overfitting to those seeds. A proposal that wins on training
    /// and not here is reported as such rather than applied.
    pub fn holds_up(&self) -> bool {
        self.holdout_error_after < self.holdout_error_before
    }
}

/// Runs the search and validates the winner.
///
/// `log` gets progress; the caller decides whether that is stderr or
/// nothing. Returns the proposal without writing anything — deciding where
/// files land is the bin's business, and a run that only measures should be
/// able to skip it.
pub fn run(
    assets_dir: &Path,
    objective: &Objective,
    log: &mut dyn FnMut(&str),
) -> Result<Proposal, String> {
    let workspace = eval::Workspace::new(assets_dir)?;
    let baseline = workspace.baseline();

    let mut evaluated = 0u32;
    let outcome = {
        let workspace = &workspace;
        search::search(baseline.clone(), objective, |candidate| {
            workspace.apply(candidate).ok()?;
            let defs = workspace.species_defs().ok()?;
            // Rejected before any fight runs, which is both the correct
            // reading and the cheap one.
            constraints::check(&defs).ok()?;
            let summaries = eval::measure(
                workspace,
                &objective.targets,
                objective.seeds,
                training_seed(objective),
            )
            .ok()?;
            evaluated += 1;
            if evaluated.is_multiple_of(10) {
                log(&format!("{evaluated} candidates fought"));
            }
            score::total_error(&objective.targets, &summaries).ok()
        })?
    };

    log(&format!(
        "search done: {} fought, {} rejected, error {:.4} -> {:.4}",
        outcome.evaluated, outcome.rejected, outcome.baseline_error, outcome.best_error
    ));

    let holdout_seed = holdout_seed(objective);
    workspace.apply(&baseline)?;
    let before = eval::measure(
        &workspace,
        &objective.targets,
        objective.holdout_seeds,
        holdout_seed,
    )?;
    workspace.apply(&outcome.best)?;
    let after = eval::measure(
        &workspace,
        &objective.targets,
        objective.holdout_seeds,
        holdout_seed,
    )?;

    Ok(Proposal {
        candidate: outcome.best.clone(),
        baseline,
        holdout_before: before.iter().map(|s| s.win_rate).collect(),
        holdout_after: after.iter().map(|s| s.win_rate).collect(),
        holdout_error_before: score::total_error(&objective.targets, &before)?,
        holdout_error_after: score::total_error(&objective.targets, &after)?,
        outcome,
    })
}

/// The seed set every candidate is compared on.
fn training_seed(objective: &Objective) -> u64 {
    objective.search_seed
}

/// A seed set disjoint from the training one.
///
/// `arena::run` walks `seed..seed + reps`, so starting the holdout at
/// `search_seed + seeds` is what makes "unseen" true rather than merely
/// intended.
fn holdout_seed(objective: &Objective) -> u64 {
    objective.search_seed + objective.seeds as u64
}

/// Writes the proposal: patched species files plus a report to read.
pub fn write_proposal(
    out_dir: &Path,
    assets_dir: &Path,
    objective: &Objective,
    proposal: &Proposal,
) -> Result<(), String> {
    let species_out = out_dir.join("species");
    std::fs::create_dir_all(&species_out)
        .map_err(|e| format!("cannot create {}: {e}", species_out.display()))?;

    let workspace = eval::Workspace::new(assets_dir)?;
    workspace.apply(&proposal.candidate)?;
    for entry in std::fs::read_dir(workspace.dir().join("species")).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let name = path.file_name().ok_or("species file with no name")?;
        std::fs::copy(&path, species_out.join(name)).map_err(|e| e.to_string())?;
    }

    std::fs::write(out_dir.join("report.md"), report(objective, proposal))
        .map_err(|e| format!("cannot write report: {e}"))?;
    Ok(())
}

fn report(objective: &Objective, proposal: &Proposal) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(out, "# Roster proposal\n");
    let _ = writeln!(
        out,
        "Candidates fought: {}  \nRejected by constraints: {}  \nTraining error: {:.4} -> {:.4}\n",
        proposal.outcome.evaluated,
        proposal.outcome.rejected,
        proposal.outcome.baseline_error,
        proposal.outcome.best_error
    );

    let _ = writeln!(
        out,
        "## Held-out seeds\n\nSeeds the search never saw. A proposal that \
         wins on training and loses here is overfitted to its seed set.\n"
    );
    let _ = writeln!(
        out,
        "Holdout error: {:.4} -> {:.4} — **{}**\n",
        proposal.holdout_error_before,
        proposal.holdout_error_after,
        if proposal.holds_up() {
            "holds up"
        } else {
            "DOES NOT HOLD UP; do not apply"
        }
    );

    let _ = writeln!(out, "| scenario | want | shipped | proposed |");
    let _ = writeln!(out, "|---|---|---|---|");
    for (i, target) in objective.targets.iter().enumerate() {
        let _ = writeln!(
            out,
            "| {} | {:.0}% | {:.0}% | {:.0}% |",
            target.scenario,
            target.want_win_rate * 100.0,
            proposal.holdout_before.get(i).copied().unwrap_or(0.0) * 100.0,
            proposal.holdout_after.get(i).copied().unwrap_or(0.0) * 100.0,
        );
    }

    let _ = writeln!(
        out,
        "\n## Fields moved\n\n```\n{}```\n",
        proposal.candidate.summary(&proposal.baseline)
    );
    let _ = writeln!(
        out,
        "## What this did not measure\n\n\
         The headless arena plays the game's own All-Attack every round, so \
         no companion Specials ever fire and ability magnitudes go \
         unmeasured. Every number above is a *floor* on what a real party \
         outputs. Play a proposal before applying it: `FERAL_DEV_ARENA=1 \
         cargo run`, `[R]`, `[L]` the scenario."
    );
    out
}
