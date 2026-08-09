//! Proposes species stats that hit authored fight targets, by measuring
//! real arena fights rather than by feel.
//!
//! ```sh
//! tuner dev-tuning/objective.ron                  # propose into dev-tuning/out/
//! tuner dev-tuning/objective.ron --out somewhere  # ...or elsewhere
//! tuner dev-tuning/objective.ron --measure        # score the shipped roster, change nothing
//! ```
//!
//! **The output is a proposal, never an edit.** Nothing here writes into
//! `assets/`; tuned files land in the out directory for a human to diff and
//! apply. An unattended process rewriting game content is not what this is.
//!
//! It lives in the launcher for the same reason `arena` does: it resolves
//! `dev-arenas/` scenarios that may name a `dev-saves/` template, and
//! `dev_template` is the launcher's.

use std::path::PathBuf;
use std::process::ExitCode;

use feral_processes::tuner::{objective::Objective, run};

const USAGE: &str = "\
usage:
  tuner <objective.ron> [--out <dir>]   search, then write a proposal
  tuner <objective.ron> --measure       score the shipped roster and stop";

const DEFAULT_OUT: &str = "dev-tuning/out";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    match parse_args(&args).and_then(|cmd| execute(&cmd)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, PartialEq)]
struct Command {
    objective: PathBuf,
    out: PathBuf,
    measure_only: bool,
}

fn parse_args(args: &[&str]) -> Result<Command, String> {
    let mut objective = None;
    let mut out = PathBuf::from(DEFAULT_OUT);
    let mut measure_only = false;
    let mut rest = args.iter();

    while let Some(arg) = rest.next() {
        match *arg {
            "--measure" => measure_only = true,
            "--out" => {
                out = rest
                    .next()
                    .ok_or_else(|| format!("--out needs a directory\n{USAGE}"))?
                    .into()
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown flag {other}\n{USAGE}"));
            }
            other if objective.is_none() => objective = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument {other}\n{USAGE}")),
        }
    }

    Ok(Command {
        objective: objective.ok_or_else(|| USAGE.to_string())?,
        out,
        measure_only,
    })
}

fn execute(cmd: &Command) -> Result<(), String> {
    let objective = Objective::load(&cmd.objective)?;
    // Resolved from the repo root rather than the cwd, the same way `arena`
    // finds its assets — so a tuner run from a subdirectory measures the
    // same game.
    let assets = feral_processes::dev_template::assets_dir();
    let assets = assets.as_path();
    if !assets.is_dir() {
        return Err(format!("no assets directory at {}", assets.display()));
    }

    let mut log = |line: &str| eprintln!("{line}");

    if cmd.measure_only {
        let workspace = feral_processes::tuner::eval::Workspace::new(assets)?;
        let summaries = feral_processes::tuner::eval::measure(
            &workspace,
            &objective.targets,
            objective.seeds,
            objective.search_seed,
        )?;
        for (target, summary) in objective.targets.iter().zip(&summaries) {
            println!(
                "{:40} win {:>5.1}%  (want {:>5.1}%)   hp {:>5.1}%  (want {:>5.1}%)",
                target.scenario,
                summary.win_rate * 100.0,
                target.want_win_rate * 100.0,
                summary.mean_player_hp_fraction * 100.0,
                target.want_hp_left * 100.0,
            );
        }
        return Ok(());
    }

    let proposal = run::run(assets, &objective, &mut log)?;
    run::write_proposal(&cmd.out, assets, &objective, &proposal)?;

    println!(
        "wrote {} — diff it against assets/species/ before applying",
        cmd.out.display()
    );
    if !proposal.holds_up() {
        println!(
            "WARNING: the proposal is worse than the shipped roster on held-out \
             seeds ({:.4} -> {:.4}). It is overfitted to the search's seed set; \
             do not apply it.",
            proposal.holdout_error_before, proposal.holdout_error_after
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_objective_path_is_required() {
        assert!(parse_args(&[]).is_err());
    }

    #[test]
    fn the_out_directory_defaults_into_dev_tuning() {
        let cmd = parse_args(&["dev-tuning/objective.ron"]).unwrap();
        assert_eq!(cmd.out, PathBuf::from(DEFAULT_OUT));
        assert!(!cmd.measure_only);
    }

    #[test]
    fn out_and_measure_are_parsed() {
        let cmd = parse_args(&["o.ron", "--out", "/tmp/p", "--measure"]).unwrap();
        assert_eq!(cmd.objective, PathBuf::from("o.ron"));
        assert_eq!(cmd.out, PathBuf::from("/tmp/p"));
        assert!(cmd.measure_only);
    }

    #[test]
    fn an_unknown_flag_is_refused_rather_than_ignored() {
        // Silently ignoring a mistyped flag is how a run that was supposed
        // to only measure quietly writes a proposal instead.
        assert!(parse_args(&["o.ron", "--dry-run"]).is_err());
    }

    #[test]
    fn a_dangling_out_flag_is_refused() {
        assert!(parse_args(&["o.ron", "--out"]).is_err());
    }
}
