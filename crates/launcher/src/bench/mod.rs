//! The base bench's launcher half: knobs a search turns, the objective it
//! aims at, and the search.

pub mod knob;
pub mod objective;
pub mod search;

/// A `chains` save in a directory of its own, generated once per test
/// process. `dev_template::resolve` writes one shared working copy, which
/// parallel tests would race on.
#[cfg(test)]
pub(crate) fn chains_save() -> std::path::PathBuf {
    use std::sync::OnceLock;
    static SAVE: OnceLock<std::path::PathBuf> = OnceLock::new();
    SAVE.get_or_init(|| {
        let path =
            std::env::temp_dir().join(format!("feral_bench_chains_{}.bin", std::process::id()));
        crate::dev_template::generate("chains", &path).expect("chains template generates");
        path
    })
    .clone()
}

#[cfg(test)]
mod tests {
    use super::chains_save;
    use crate::dev_template::assets_dir;
    use feral_processes_engine::bench::{self, RunOptions};

    fn run(ticks: u64, seed: u64) -> bench::BenchReport {
        bench::run(&chains_save(), &assets_dir(), RunOptions { ticks, seed }).unwrap()
    }

    #[test]
    fn every_chains_machine_accounts_for_every_tick_and_the_line_produces() {
        let report = run(300, 1);
        assert!(!report.economy.machines.is_empty());
        for m in &report.economy.machines {
            assert_eq!(m.status_ticks.values().sum::<u64>(), 300, "{m:?}");
        }
        assert!(
            report.economy.lines.iter().any(|l| l.output_per_1000 > 0.0),
            "{:?}",
            report.economy.lines
        );
    }

    #[test]
    fn two_seeds_on_chains_differ_beyond_the_seed_field() {
        let (a, b) = (run(300, 1), run(300, 2));
        let mut b_as_a = b.clone();
        b_as_a.seed = a.seed;
        assert_ne!(
            a.economy, b_as_a.economy,
            "seed changed nothing the report shows"
        );
    }
}
