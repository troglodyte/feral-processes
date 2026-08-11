//! Runs a battle scenario offline and reports what happened.
//!
//! ```sh
//! cargo run --bin arena -- dev-arenas/opening-fight.ron
//! cargo run --bin arena -- dev-arenas/full-group.ron --out report.ron
//! ```
//!
//! The measuring is all `feral_processes_engine::arena`; this is the CLI
//! around it. It lives in the launcher crate for one reason the engine
//! cannot supply: `dev_template` is in the launcher's `[lib]`, so only a bin
//! here can turn `player: Template("extraction")` into a save to load.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use feral_processes::dev_template;
use feral_processes_engine::arena::{self, Report, Scenario};

const USAGE: &str = "\
usage:
  arena <scenario.ron> [--out <report.ron>]   run a scenario
  arena templates                             list the saves in dev-saves/";

const DEFAULT_REPORT: &str = "arena-report.ron";

#[derive(Debug, PartialEq)]
enum Command {
    Run { scenario: PathBuf, out: PathBuf },
    Templates,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    let result = match parse_args(&args) {
        Ok(Command::Templates) => {
            println!("{}", dev_template::known());
            Ok(())
        }
        Ok(Command::Run { scenario, out }) => run(&scenario, &out),
        Err(e) => Err(e),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: &[&str]) -> Result<Command, String> {
    match args {
        ["templates"] => Ok(Command::Templates),
        [scenario] if !scenario.starts_with("--") => Ok(Command::Run {
            scenario: PathBuf::from(scenario),
            out: PathBuf::from(DEFAULT_REPORT),
        }),
        [scenario, "--out", out] if !scenario.starts_with("--") => Ok(Command::Run {
            scenario: PathBuf::from(scenario),
            out: PathBuf::from(out),
        }),
        _ => Err(USAGE.to_string()),
    }
}

fn run(scenario_path: &Path, out: &Path) -> Result<(), String> {
    let mut scenario = Scenario::load(scenario_path)?;
    dev_template::resolve_scenario(&mut scenario)?;

    let (report, _) = arena::run(
        &scenario,
        &dev_template::assets_dir(),
        arena::RunOptions::default(),
    )?;

    // stderr, so piping stdout to a file keeps the data clean.
    for warning in &report.warnings {
        eprintln!("warning: {warning}");
    }
    print_result(&report);

    std::fs::write(out, report.to_ron()?).map_err(|e| format!("{}: {e}", out.display()))?;
    eprintln!("wrote {}", out.display());
    Ok(())
}

/// One rep is a fight to read; many are a sample to summarise. Printing the
/// transcript fifty times over would bury the number the run was for.
fn print_result(report: &Report) {
    if let [rep] = report.reps.as_slice() {
        for line in &rep.transcript {
            println!("{line}");
        }
        println!();
        // Above the outcome, and only here: a rolled encounter fields a
        // fresh composition per rep, and the aggregate below is a
        // distribution the individual packs would only clutter.
        if !rep.composition.is_empty() {
            println!(
                "fought          {}",
                rep.composition
                    .iter()
                    .map(|(species, count)| format!("{species} x{count}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        println!(
            "{} in {} rounds, {:.0}% HP left, {} companions down (seed {})",
            if rep.won { "WON" } else { "LOST" },
            rep.rounds,
            rep.player_hp_fraction * 100.0,
            rep.companions_downed,
            rep.seed
        );
        return;
    }

    let s = report.summary();
    println!("reps            {}", s.reps);
    println!(
        "win rate        {:.1}% ({}/{})",
        s.win_rate * 100.0,
        s.wins,
        s.reps
    );
    println!(
        "rounds          mean {:.1}, median {}",
        s.mean_rounds, s.median_rounds
    );
    println!(
        "player HP left  mean {:.0}%",
        s.mean_player_hp_fraction * 100.0
    );
    println!("companions down mean {:.2}", s.mean_companions_downed);
    if s.loss_seeds.is_empty() {
        println!("losses          none");
    } else {
        // The actionable line: any of these replays alone by pinning it as
        // the scenario's `seed` with `reps: 1`.
        println!(
            "loss seeds      {}",
            s.loss_seeds
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_path_runs_that_scenario_into_the_default_report() {
        assert_eq!(
            parse_args(&["dev-arenas/opening-fight.ron"]).unwrap(),
            Command::Run {
                scenario: PathBuf::from("dev-arenas/opening-fight.ron"),
                out: PathBuf::from(DEFAULT_REPORT),
            }
        );
    }

    #[test]
    fn out_redirects_the_report() {
        assert_eq!(
            parse_args(&["s.ron", "--out", "r.ron"]).unwrap(),
            Command::Run {
                scenario: PathBuf::from("s.ron"),
                out: PathBuf::from("r.ron"),
            }
        );
    }

    #[test]
    fn templates_lists_the_library() {
        assert_eq!(parse_args(&["templates"]).unwrap(), Command::Templates);
    }

    #[test]
    fn an_unknown_flag_is_an_err_carrying_the_usage() {
        let err = parse_args(&["s.ron", "--reps", "5"]).unwrap_err();
        assert!(err.contains("usage:"), "{err}");
    }

    #[test]
    fn no_args_at_all_is_an_err_carrying_the_usage() {
        let err = parse_args(&[]).unwrap_err();
        assert!(err.contains("usage:"), "{err}");
    }

    #[test]
    fn a_missing_scenario_file_is_a_clean_error_naming_it() {
        let err = run(Path::new("no-such-scenario.ron"), Path::new("/dev/null")).unwrap_err();
        assert!(err.contains("no-such-scenario.ron"), "{err}");
    }
}
