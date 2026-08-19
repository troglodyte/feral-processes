//! The whole tuner as one call: build a workspace, climb, validate the
//! winner on unseen seeds, write the proposal.
//!
//! Kept out of the bin so it can be driven by a test at a small iteration
//! count. `main` is argument parsing and printing.

use super::objective::Objective;
use super::roster::Candidate;
use super::{constraints, eval, score, search, sides};
use feral_processes_engine::balance_sim::ReachRuleVerdict;
use feral_processes_engine::species::AptitudeFault;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The two censuses too expensive to reject a candidate by, run once on the
/// winner.
///
/// Neither is a rejection. The reach rule runs a level search per call and
/// would be paid on every candidate; the aptitude spread is a property of
/// the roster's whole distribution rather than of any one move. Both are
/// *reported* instead, because by the time these run a human is reading a
/// diff, and the rule about no silent caps applies to a proposal as much as
/// to a search.
pub struct PostCheck {
    pub reach: ReachRuleVerdict,
    pub aptitude: Vec<AptitudeFault>,
}

impl PostCheck {
    /// Measures the roster currently written into `workspace`.
    fn measure(workspace: &eval::Workspace) -> Result<PostCheck, String> {
        let db = workspace.species_db()?;
        Ok(PostCheck {
            reach: feral_processes_engine::balance_sim::reach_rule_verdict(&db),
            aptitude: feral_processes_engine::species::extraction_aptitude_faults(db.all()),
        })
    }

    pub fn holds(&self) -> bool {
        self.reach.holds() && self.aptitude.is_empty()
    }
}

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
    /// Species held at their shipped numbers because the player fields them
    /// in one of the targets. Carried so the report can say what was *not*
    /// searched — a proposal that leaves a species alone and a proposal that
    /// was never allowed to touch it read identically otherwise.
    pub frozen: BTreeSet<String>,
    /// How many candidates each constraint threw out, by the rejection's own
    /// wording.
    ///
    /// A bare count says a search was turned away without saying from what,
    /// and the two readings need different fixes: bounds set wrong is a
    /// config edit, while a legal move set thinner than the search space is
    /// a reason to narrow `perturb` so it stops proposing the illegal.
    pub rejections: BTreeMap<String, u32>,
    /// The expensive censuses, run on the winner only.
    pub post_check: PostCheck,
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
    let frozen = sides::player_fielded(&objective.targets)?;
    let baseline = workspace.baseline(&frozen);
    if baseline.species.is_empty() {
        return Err(format!(
            "every species in the roster is fielded by the player in some target, so \
             there is nothing left for the search to move ({} frozen)",
            frozen.len()
        ));
    }
    if !frozen.is_empty() {
        log(&format!(
            "frozen (the player fields these): {}",
            frozen.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }

    let mut evaluated = 0u32;
    let mut rejections: BTreeMap<String, u32> = BTreeMap::new();
    let outcome = {
        let workspace = &workspace;
        let rejections = &mut rejections;
        search::search(baseline.clone(), objective, |candidate| {
            workspace.apply(candidate).ok()?;
            let defs = workspace.species_defs().ok()?;
            // Rejected before any fight runs, which is both the correct
            // reading and the cheap one. The reason is tallied here rather
            // than inside `search`, which sees only `None` — keeping the
            // search generic over what an evaluation means.
            if let Err(rejection) = constraints::check(&defs) {
                *rejections.entry(rejection_label(&rejection)).or_default() += 1;
                return None;
            }
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
    for (reason, count) in &rejections {
        log(&format!("  {count} rejected: {reason}"));
    }

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
    // With the winner still installed, so this measures the proposal rather
    // than whatever the search last wrote.
    let post_check = PostCheck::measure(&workspace)?;

    Ok(Proposal {
        candidate: outcome.best.clone(),
        baseline,
        holdout_before: before.iter().map(|s| s.win_rate).collect(),
        holdout_after: after.iter().map(|s| s.win_rate).collect(),
        holdout_error_before: score::total_error(&objective.targets, &before)?,
        holdout_error_after: score::total_error(&objective.targets, &after)?,
        frozen,
        rejections,
        post_check,
        outcome,
    })
}

/// A rejection's wording, without the particular species or numbers that
/// tripped it — so a tally counts *rules*, not individual candidates.
fn rejection_label(rejection: &constraints::Rejection) -> String {
    match rejection {
        constraints::Rejection::OffClassShape { .. } => {
            "a species' stat block no longer agrees with its class".into()
        }
        other => other.to_string(),
    }
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

    if !proposal.rejections.is_empty() {
        let _ = writeln!(
            out,
            "### Why candidates were rejected\n\n\
             A search that spends most of its iteration budget being turned away \
             proposes nothing, and the count alone does not say which fix that calls for \
             — bounds set wrong is a config edit, a legal move set thinner than the \
             search space is a reason to narrow `perturb`.\n"
        );
        for (reason, count) in &proposal.rejections {
            let _ = writeln!(out, "- **{count}x** {reason}");
        }
        let _ = writeln!(out);
    }

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
        "## Censuses too expensive to reject by\n\n\
         Run once, on the winner. A proposal that breaks one of these is **not** \
         rejected — it is reported, because by now a human is reading a diff and \
         deciding. Both are the shipped suite's own checks, called rather than \
         restated.\n"
    );
    let _ = writeln!(
        out,
        "- Reach rule: {} — **{}**",
        proposal.post_check.reach,
        if proposal.post_check.reach.holds() {
            "holds"
        } else {
            "BROKEN: a full pack is no easier than the same enemies all in melee range, \
             so `ENGAGED_GROUPS` is buying nothing"
        }
    );
    if proposal.post_check.aptitude.is_empty() {
        let _ = writeln!(
            out,
            "- Extraction aptitude still cuts across the difficulty ladder — **holds**\n"
        );
    } else {
        let _ = writeln!(
            out,
            "- Extraction aptitude has collapsed back into the difficulty ladder — \
             **BROKEN**:"
        );
        for fault in &proposal.post_check.aptitude {
            let _ = writeln!(out, "  - {fault}");
        }
        let _ = writeln!(out);
    }

    if !proposal.frozen.is_empty() {
        let _ = writeln!(
            out,
            "## Frozen\n\n\
             The player fields these in one of the targets, so the search was not \
             allowed to move them. A target says a fight should be won *N*% of the \
             time; it does not say whether to get there by weakening the opponent or \
             the party, and a stat lowered to satisfy one fight applies to that \
             species everywhere in the game.\n\n\
             {}\n",
            proposal
                .frozen
                .iter()
                .map(|s| format!("- `{s}`"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuner::score::Target;

    fn repo(rel: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel)
    }

    /// The smallest objective that still fields a party: one target, one
    /// fight per seed, few iterations. This runs real battles, so every
    /// number here is chosen to keep it a test rather than a tuning run.
    fn tiny_objective() -> Objective {
        use crate::tuner::objective::Bound;
        use crate::tuner::roster::Field;
        Objective {
            seeds: 1,
            holdout_seeds: 1,
            iterations: 3,
            search_seed: 5,
            targets: vec![Target {
                scenario: repo("dev-arenas/full-group.ron")
                    .to_string_lossy()
                    .into_owned(),
                reps: 1,
                want_win_rate: 0.75,
                want_hp_left: 0.45,
                weight: 1.0,
            }],
            bounds: Field::ALL
                .iter()
                .map(|f| Bound {
                    field: f.key().into(),
                    min: 0.0,
                    max: 100.0,
                })
                .collect(),
        }
    }

    #[test]
    fn the_search_never_proposes_a_change_to_a_species_the_player_fields() {
        // The regression this whole module exists for. `full-group.ron`
        // fields three Scrappers against four rootkits, and the first real
        // tuner run satisfied its win-rate target partly by dropping
        // `scrapper.base_mitigation` from 5 to its bound floor of 0 — a nerf to a
        // companion the player tames and keeps, applied everywhere in the
        // game, to move one authored fight.
        let assets = repo("assets");
        let objective = tiny_objective();
        let proposal = run(&assets, &objective, &mut |_| {}).expect("tuner run");

        assert!(
            proposal.frozen.contains("scrapper"),
            "scrapper is the party in this scenario; frozen was {:?}",
            proposal.frozen
        );
        assert!(
            !proposal.candidate.species.contains_key("scrapper"),
            "a frozen species must not be in the candidate at all"
        );
        assert!(
            proposal.candidate.species.contains_key("rootkit"),
            "the opponent must still be movable, or the freeze froze the fight"
        );
    }

    /// The two censuses the search cannot afford to reject by still have to
    /// reach the person applying the proposal. A silent pass and a check
    /// that never ran read identically in a report, which is the whole
    /// reason this asserts on the rendered text rather than on `PostCheck`.
    #[test]
    fn the_report_carries_the_censuses_the_search_could_not_afford() {
        let objective = tiny_objective();
        let proposal = run(&repo("assets"), &objective, &mut |_| {}).expect("tuner run");
        let text = report(&objective, &proposal);

        assert!(
            proposal.post_check.holds(),
            "the shipped roster must pass both post-checks: reach {}, aptitude {:?}",
            proposal.post_check.reach,
            proposal.post_check.aptitude
        );
        let section = text
            .split_once("## Censuses too expensive to reject by")
            .expect("report has a post-check section")
            .1;
        assert!(
            section.contains("Reach rule:") && section.contains("with the reach rule"),
            "the reach rule's own numbers must be in the report, not just a verdict:\n{section}"
        );
        assert!(
            section.contains("Extraction aptitude"),
            "the aptitude census is missing from:\n{section}"
        );
    }

    #[test]
    fn the_report_says_which_species_were_never_searched() {
        // A species the search left alone and a species the search was
        // forbidden to touch produce the same empty diff, and the person
        // reading the proposal has to be able to tell them apart — the
        // second is the tool declining to answer a question.
        let objective = tiny_objective();
        let proposal = run(&repo("assets"), &objective, &mut |_| {}).expect("tuner run");
        let text = report(&objective, &proposal);

        let frozen_section = text
            .split_once("## Frozen")
            .expect("a run with a fielded party must report a Frozen section")
            .1;
        assert!(
            frozen_section.contains("scrapper"),
            "Frozen section did not name scrapper:\n{frozen_section}"
        );
        assert!(
            !text
                .split_once("## Frozen")
                .unwrap()
                .0
                .split_once("## Fields moved")
                .expect("report has a Fields moved section")
                .1
                .contains("scrapper"),
            "a frozen species must not appear as a field that moved:\n{text}"
        );
    }
}
