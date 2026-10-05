//! Measures the base economy headlessly, and tunes its numbers toward target
//! ranges.
//!
//! ```sh
//! bench run --template chains --ticks 5000 [--seed 1] [--order patch_routine:9999] [--out report.ron]
//! bench tune dev-tuning/economy.ron [--out dir]
//! ```
//!
//! **`tune` writes a proposal, never an edit.** Patched files land in
//! `dev-tuning/out/bench-<objective>/` for a human to `diff -r` against
//! `assets/` and apply. It lives in the launcher because only `dev_template`
//! here can turn a template name into a save.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use feral_processes::bench::{
    objective::Objective,
    search::{search, write_proposal},
};
use feral_processes::dev_template;
use feral_processes_engine::bench::{self, RunOptions};

const USAGE: &str = "\
usage:
  bench run --template <name> --ticks <n> [--seed <n>] [--order <item:qty>]... [--out <report.ron>]
  bench tune <objective.ron> [--out <dir>]";

#[derive(Debug, PartialEq)]
enum Command {
    Run {
        template: String,
        ticks: u64,
        seed: u64,
        orders: Vec<(String, u32)>,
        out: Option<PathBuf>,
    },
    Tune {
        objective: PathBuf,
        out: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    match parse_args(&args).and_then(execute) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: &[&str]) -> Result<Command, String> {
    let bad = |why: &str| format!("{why}\n{USAGE}");
    match args {
        ["run", flags @ ..] => {
            let (mut template, mut ticks, mut seed, mut out) = (None, None, 0u64, None);
            let mut orders = Vec::new();
            let mut it = flags.iter();
            while let Some(&flag) = it.next() {
                let mut value = || {
                    it.next()
                        .copied()
                        .ok_or_else(|| bad(&format!("{flag} needs a value")))
                };
                let number = |v: &str| {
                    v.parse::<u64>()
                        .map_err(|_| bad(&format!("{flag} needs a whole number, got `{v}`")))
                };
                match flag {
                    "--template" => template = Some(value()?.to_string()),
                    "--ticks" => ticks = Some(number(value()?)?),
                    "--seed" => seed = number(value()?)?,
                    "--order" => orders.push(parse_order(value()?).map_err(|e| bad(&e))?),
                    "--out" => out = Some(PathBuf::from(value()?)),
                    other => return Err(bad(&format!("unknown argument {other}"))),
                }
            }
            Ok(Command::Run {
                template: template.ok_or_else(|| bad("--template is required"))?,
                ticks: ticks.ok_or_else(|| bad("--ticks is required"))?,
                seed,
                orders,
                out,
            })
        }
        ["tune", objective] if !objective.starts_with("--") => Ok(Command::Tune {
            objective: PathBuf::from(objective),
            out: None,
        }),
        ["tune", objective, "--out", out] if !objective.starts_with("--") => Ok(Command::Tune {
            objective: PathBuf::from(objective),
            out: Some(PathBuf::from(out)),
        }),
        _ => Err(USAGE.to_string()),
    }
}

/// `item:qty`, the last colon splitting so an item id may never need one.
fn parse_order(v: &str) -> Result<(String, u32), String> {
    let (item, qty) = v
        .rsplit_once(':')
        .ok_or_else(|| format!("--order needs item:qty, got `{v}`"))?;
    let qty = qty
        .parse::<u32>()
        .map_err(|_| format!("--order quantity must be a whole number, got `{qty}`"))?;
    if item.is_empty() {
        return Err(format!("--order needs an item before the colon, got `{v}`"));
    }
    Ok((item.to_string(), qty))
}

fn execute(cmd: Command) -> Result<(), String> {
    match cmd {
        Command::Run {
            template,
            ticks,
            seed,
            orders,
            out,
        } => run(&template, ticks, seed, orders, out.as_deref()),
        Command::Tune { objective, out } => tune(&objective, out),
    }
}

fn run(
    template: &str,
    ticks: u64,
    seed: u64,
    orders: Vec<(String, u32)>,
    out: Option<&Path>,
) -> Result<(), String> {
    let save = dev_template::resolve(template)?;
    let report = bench::run(
        &save,
        &dev_template::assets_dir(),
        RunOptions {
            ticks,
            seed,
            orders,
        },
    )?;
    let ron = ron::ser::to_string_pretty(&report, ron::ser::PrettyConfig::default())
        .map_err(|e| e.to_string())?;
    // Summary on stderr so stdout stays the data when no --out is given.
    eprintln!(
        "{template}: {ticks} ticks, seed {seed}: {} machines, {} lines, labour unworked {:.2}",
        report.economy.machines.len(),
        report.economy.lines.len(),
        report.economy.labour.mean_unworked_total
    );
    match out {
        Some(path) => {
            std::fs::write(path, ron).map_err(|e| format!("{}: {e}", path.display()))?;
            eprintln!("wrote {}", path.display());
        }
        None => println!("{ron}"),
    }
    Ok(())
}

fn tune(objective_path: &Path, out: Option<PathBuf>) -> Result<(), String> {
    let objective = Objective::load(objective_path)?;
    let stem = objective_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("objective path has no file name")?;
    let out = out.unwrap_or_else(|| {
        dev_template::repo_root()
            .join("dev-tuning/out")
            .join(format!("bench-{stem}"))
    });
    let save = dev_template::resolve(&objective.template)?;
    let proposal = search(
        &objective,
        &dev_template::assets_dir(),
        &save,
        &mut |line| eprintln!("{line}"),
    )?;
    write_proposal(&out, &objective, &proposal)?;
    println!(
        "wrote {} — diff -r it against assets/ before applying",
        out.display()
    );
    if !proposal.holds_up() {
        println!(
            "WARNING: the proposal does not beat the shipped numbers on held-out seeds \
             ({:.4} -> {:.4}); do not apply it.",
            proposal.holdout_before.error, proposal.holdout_after.error
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_takes_flags_in_any_order() {
        let a = parse_args(&[
            "run",
            "--template",
            "chains",
            "--ticks",
            "50",
            "--seed",
            "3",
        ]);
        let b = parse_args(&[
            "run",
            "--seed",
            "3",
            "--ticks",
            "50",
            "--template",
            "chains",
        ]);
        assert_eq!(a, b);
        assert_eq!(
            a.unwrap(),
            Command::Run {
                template: "chains".into(),
                ticks: 50,
                seed: 3,
                orders: vec![],
                out: None
            }
        );
    }

    #[test]
    fn run_takes_repeatable_orders() {
        let c = parse_args(&[
            "run",
            "--template",
            "chains",
            "--ticks",
            "5",
            "--order",
            "patch_routine:9999",
            "--order",
            "ice_breaker:7",
        ]);
        let Command::Run { orders, .. } = c.unwrap() else {
            panic!()
        };
        assert_eq!(
            orders,
            vec![("patch_routine".into(), 9999), ("ice_breaker".into(), 7)]
        );
        for bad in ["patch_routine", "patch_routine:x", ":5", "a:-1"] {
            assert!(
                parse_args(&["run", "--template", "c", "--ticks", "5", "--order", bad]).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn run_defaults_seed_and_takes_out() {
        let c = parse_args(&[
            "run",
            "--template",
            "chains",
            "--ticks",
            "5",
            "--out",
            "r.ron",
        ]);
        assert_eq!(
            c.unwrap(),
            Command::Run {
                template: "chains".into(),
                ticks: 5,
                seed: 0,
                orders: vec![],
                out: Some("r.ron".into())
            }
        );
    }

    #[test]
    fn run_rejects_what_it_cannot_use() {
        assert!(parse_args(&["run", "--ticks", "5"]).is_err());
        assert!(parse_args(&["run", "--template", "chains"]).is_err());
        assert!(parse_args(&["run", "--template", "chains", "--ticks", "x"]).is_err());
        assert!(parse_args(&["run", "--template", "chains", "--ticks"]).is_err());
        assert!(parse_args(&["run", "--template", "c", "--ticks", "5", "--bogus", "1"]).is_err());
    }

    #[test]
    fn tune_takes_an_objective_and_optional_out() {
        assert_eq!(
            parse_args(&["tune", "o.ron"]).unwrap(),
            Command::Tune {
                objective: "o.ron".into(),
                out: None
            }
        );
        assert_eq!(
            parse_args(&["tune", "o.ron", "--out", "d"]).unwrap(),
            Command::Tune {
                objective: "o.ron".into(),
                out: Some("d".into())
            }
        );
        assert!(parse_args(&["tune"]).is_err());
        assert!(parse_args(&["tune", "--out", "d"]).is_err());
        assert!(parse_args(&[]).is_err());
    }
}
