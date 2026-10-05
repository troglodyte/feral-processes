//! The base bench's launcher half: knobs a search turns, the objective it
//! aims at, and the search.

pub mod knob;
pub mod objective;
pub mod search;

/// A `chains` save at one fixed path, generated once per test process.
/// `dev_template::resolve` writes one shared working copy, which parallel
/// tests would race on. A static cannot be dropped, so the path is the same
/// for every process (one file ever left behind, not one per run) and is
/// written under a private name and renamed into place, so a process reading
/// it never sees another's half-written copy.
#[cfg(test)]
pub(crate) fn chains_save() -> std::path::PathBuf {
    use std::sync::OnceLock;
    static SAVE: OnceLock<std::path::PathBuf> = OnceLock::new();
    SAVE.get_or_init(|| {
        let dir = std::env::temp_dir();
        let path = dir.join("feral_bench_chains.bin");
        let private = dir.join(format!("feral_bench_chains_{}.tmp", std::process::id()));
        crate::dev_template::generate("chains", &private).expect("chains template generates");
        std::fs::rename(&private, &path).expect("chains save moves into place");
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
        bench::run(
            &chains_save(),
            &assets_dir(),
            RunOptions {
                ticks,
                seed,
                orders: vec![],
                sieges: true,
            },
        )
        .unwrap()
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
    fn an_order_posts_workers_who_drain_the_bay_past_its_capacity() {
        // Seed 1 passes the bay's capacity only around tick 2000: the line
        // starves of power cells soon after the first batch (see the
        // with-orders section of the chains measurement doc).
        let ticks = 2000;
        let report = bench::run(
            &chains_save(),
            &assets_dir(),
            RunOptions {
                ticks,
                seed: 1,
                orders: vec![("patch_routine".into(), 9999)],
                sieges: true,
            },
        )
        .unwrap();
        let bay = report
            .economy
            .machines
            .iter()
            .find(|m| m.kind == "assembly_bay")
            .unwrap();
        let capacity = 10; // assets/structures/assembly_bay.ron
        assert!(bay.units > capacity, "no drain: {bay:?}");
        assert!(report.economy.labour.mean_wanted > 0.0);
        for m in &report.economy.machines {
            assert_eq!(m.status_ticks.values().sum::<u64>(), ticks, "{m:?}");
        }
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

    #[test]
    fn a_bench_economy_run_samples_every_staff_member_every_tick() {
        let out = std::env::temp_dir().join(format!(
            "feral_bench_staff_economy_{}.bin",
            std::process::id()
        ));
        crate::dev_template::generate("bench-economy", &out).unwrap();
        let ticks = 50;
        let report = bench::run(
            &out,
            &assets_dir(),
            RunOptions {
                ticks,
                seed: 1,
                orders: vec![],
                sieges: true,
            },
        )
        .unwrap();
        let _ = std::fs::remove_file(&out);
        assert_eq!(report.stopped_at, None);
        let staff = &report.staff;
        assert_eq!(staff.staff_ticks, 15 * ticks);
        assert_eq!(staff.end_morale.len(), 15);
        for share in [staff.on_shift_share, staff.sulking_share]
            .into_iter()
            .chain(staff.rung_share.values().copied())
            .chain(staff.needs.values().map(|n| n.critical_share))
        {
            assert!((0.0..=1.0).contains(&share), "{share}");
        }
        assert!((staff.rung_share.values().sum::<f32>() - 1.0).abs() < 1e-4);
        assert!(!staff.needs.is_empty());
    }

    #[test]
    fn a_bench_economy_run_reports_every_catalogue_memory_and_band() {
        let out = std::env::temp_dir().join(format!(
            "feral_bench_memories_economy_{}.bin",
            std::process::id()
        ));
        crate::dev_template::generate("bench-economy", &out).unwrap();
        let report = bench::run(
            &out,
            &assets_dir(),
            RunOptions {
                ticks: 50,
                seed: 1,
                orders: vec![],
                sieges: true,
            },
        )
        .unwrap();
        let _ = std::fs::remove_file(&out);
        let memories = &report.memories;
        let mut ids: Vec<String> = std::fs::read_dir(assets_dir().join("memories"))
            .unwrap()
            .filter_map(|e| e.ok()?.path().file_stem()?.to_str().map(str::to_string))
            .filter(|stem| stem != "README")
            .collect();
        ids.sort();
        assert!(!ids.is_empty());
        assert_eq!(memories.fired.keys().cloned().collect::<Vec<_>>(), ids);
        assert_eq!(memories.formed.keys().cloned().collect::<Vec<_>>(), ids);
        assert!((memories.morale_band_share.values().sum::<f32>() - 1.0).abs() < 1e-4);
        for id in &ids {
            assert!(
                report
                    .measure(&format!("memories.fired_per_1000.{id}"))
                    .is_ok()
            );
        }
    }

    #[test]
    fn bench_economy_runs_past_the_siege_point_with_sieges_off() {
        // Seed 1 with these orders is stopped by a siege near tick 3,789;
        // see dev-tuning/economy-bench.ron.
        let out =
            std::env::temp_dir().join(format!("feral_bench_no_sieges_{}.bin", std::process::id()));
        crate::dev_template::generate("bench-economy", &out).unwrap();
        let report = bench::run(
            &out,
            &assets_dir(),
            RunOptions {
                ticks: 5000,
                seed: 1,
                orders: ["patch_routine", "bytecode_block", "ice_breaker"]
                    .map(|item| (item.to_string(), 9999))
                    .to_vec(),
                sieges: false,
            },
        )
        .unwrap();
        let _ = std::fs::remove_file(&out);
        assert_eq!(report.stopped_at, None);
    }
}
