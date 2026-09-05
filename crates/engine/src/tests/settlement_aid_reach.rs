//! Whether the aid radii reach anything at all.
//!
//! Both of these shipped as flat numbers guessed against nothing, and both
//! were dead: `SETTLEMENT_GARRISON_RADIUS` at 40 found a town near the
//! anchor in 1.6% of worlds, and `ROUTE_PREDATION_RADIUS` at 15 found one
//! beside a lane in none of 2,000. A radius is only meaningful against
//! `settlements::placement::REGION_TILES`, which is what decides how far
//! apart towns can be, so these are gates on *reach* rather than on the
//! numbers themselves — a retune that flattens either constant back fails
//! here rather than shipping a feature the player can never see.
//!
//! The full sweep is `aid_reach_probe` below, written up in
//! `docs/measurements/2026-09-05-settlement-aid-reach.md`.

use crate::settlements::placement::{REGION_TILES, SettlementKey, region_of, settlement_at};
use crate::settlements::{SettlementDb, placement};
use crate::tuning::{ROUTE_PREDATION_RADIUS, SETTLEMENT_GARRISON_RADIUS};

use super::support::test_assets_dir;

fn catalogue() -> SettlementDb {
    SettlementDb::load_dir(&test_assets_dir().join("settlements"))
        .expect("the catalogue loads")
        .0
}

/// A fixed ladder of seeds, so this samples worlds rather than one world and
/// stays deterministic. Odd multiplier, so consecutive `i` land far apart.
fn seeds(n: u32) -> impl Iterator<Item = u32> {
    (0..n).map(|i| i.wrapping_mul(2_654_435_761).wrapping_add(12_345))
}

/// Every settlement candidate whose region is within `span` regions of the
/// one holding `origin`, nearest first.
///
/// Candidate cells, not resolved tiles: materialization walks out from here
/// for standable ground, bounded by `SETTLEMENT_SITE_SEARCH_TILES`, which is
/// small against the distances this measures.
fn towns_near(seed: u32, db: &SettlementDb, origin: (i32, i32), span: i32) -> Vec<(i32, i32)> {
    let home = region_of(origin.0, origin.1);
    let mut out: Vec<(i32, i32)> = (-span..=span)
        .flat_map(|dy| (-span..=span).map(move |dx| (dx, dy)))
        .filter_map(|(dx, dy)| {
            settlement_at(
                seed,
                db,
                SettlementKey {
                    rx: home.rx + dx,
                    ry: home.ry + dy,
                },
            )
        })
        .map(|p| p.cell)
        .collect();
    out.sort_by_key(|&t| chebyshev(t, origin));
    out
}

fn chebyshev(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// `routes::settlements_near_route`'s measure, which is private to that
/// module. Kept as a copy deliberately: this file is an instrument, and an
/// instrument that shares the implementation it is measuring cannot report
/// that the implementation moved.
fn segment_distance(point: (i32, i32), a: (i32, i32), b: (i32, i32)) -> f64 {
    let (px, py) = (point.0 as f64, point.1 as f64);
    let (ax, ay) = (a.0 as f64, a.1 as f64);
    let (bx, by) = (b.0 as f64, b.1 as f64);
    let (abx, aby) = (bx - ax, by - ay);
    let len_sq = abx * abx + aby * aby;
    let t = if len_sq > 0.0 {
        (((px - ax) * abx + (py - ay) * aby) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (cx, cy) = (ax + t * abx, ay + t * aby);
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

/// Fraction of sampled worlds with a town inside the garrison radius of the
/// anchor. The anchor is the founding tile, which is at or beside the zone
/// spawn point, so the origin stands in for it.
fn worlds_with_a_garrison_candidate(db: &SettlementDb, n: u32) -> f64 {
    let hits = seeds(n)
        .filter(|&seed| {
            towns_near(seed, db, (0, 0), 3)
                .first()
                .is_some_and(|&t| chebyshev(t, (0, 0)) <= SETTLEMENT_GARRISON_RADIUS)
        })
        .count();
    hits as f64 / n as f64
}

/// Fraction of sampled worlds where a lane from the anchor to its
/// `nth`-nearest town passes within the predation radius of some *other*
/// town. `nth` is zero-based, so 0 is the nearest town.
fn lanes_with_a_predation_candidate(db: &SettlementDb, n: u32, nth: usize) -> f64 {
    let mut sampled = 0u32;
    let mut hits = 0u32;
    for seed in seeds(n) {
        let towns = towns_near(seed, db, (0, 0), 4);
        let Some(&destination) = towns.get(nth) else {
            continue;
        };
        sampled += 1;
        if towns
            .iter()
            .filter(|&&t| t != destination)
            .any(|&t| segment_distance(t, (0, 0), destination) <= ROUTE_PREDATION_RADIUS as f64)
        {
            hits += 1;
        }
    }
    hits as f64 / sampled.max(1) as f64
}

#[test]
fn a_garrison_is_reachable_in_a_reasonable_share_of_worlds() {
    let share = worlds_with_a_garrison_candidate(&catalogue(), 500);
    assert!(
        share >= 0.25,
        "only {:.1}% of worlds have a town within SETTLEMENT_GARRISON_RADIUS ({}) of the \
         anchor, against REGION_TILES {REGION_TILES}. The aid ladder's passive half is \
         unreachable by geometry — scale the radius off region spacing rather than nudging it.",
        share * 100.0,
        SETTLEMENT_GARRISON_RADIUS,
    );
}

#[test]
fn a_lane_to_a_farther_market_can_be_preyed_on() {
    // The nearest town's lane is deliberately not the subject: it is short
    // and points away from everywhere else, so it is unpreyable at any
    // radius. Risk is a property of hauling *past* somebody.
    let db = catalogue();
    let share = lanes_with_a_predation_candidate(&db, 500, 2);
    assert!(
        share >= 0.10,
        "only {:.1}% of lanes to a third-nearest market pass within ROUTE_PREDATION_RADIUS \
         ({ROUTE_PREDATION_RADIUS}) of another town, against REGION_TILES {REGION_TILES}. \
         Predation cannot fire at this radius whatever the party's standing.",
        share * 100.0,
    );
}

#[test]
fn both_radii_are_expressed_against_region_spacing() {
    // The failure this closes is a flat number that reads fine in a diff and
    // is dead in play, so the guard is the ratio and not the value.
    assert_eq!(SETTLEMENT_GARRISON_RADIUS, REGION_TILES / 2);
    assert_eq!(ROUTE_PREDATION_RADIUS, REGION_TILES / 4);
    assert_eq!(REGION_TILES, placement::REGION_TILES);
}

/// The sweep behind the numbers in the doc comments — ignored by default.
///
/// ```sh
/// cargo test -p feral-processes-engine aid_reach_probe -- --ignored --nocapture
/// ```
#[test]
#[ignore]
fn aid_reach_probe() {
    let db = catalogue();
    let n = 2_000;

    let mut nearest: Vec<i32> = seeds(n)
        .filter_map(|seed| {
            towns_near(seed, &db, (0, 0), 3)
                .first()
                .map(|&t| chebyshev(t, (0, 0)))
        })
        .collect();
    nearest.sort_unstable();
    let pct = |p: usize| nearest[(nearest.len() * p / 100).min(nearest.len() - 1)];
    println!("REGION_TILES = {REGION_TILES}, {n} worlds sampled\n");
    println!("Chebyshev from the anchor to the nearest town:");
    println!(
        "  p10={} p25={} p50={} p75={} p90={}",
        pct(10),
        pct(25),
        pct(50),
        pct(75),
        pct(90)
    );
    for radius in [40, 60, 64, 96, 128, 160, 192, 256] {
        let hit = nearest.iter().filter(|&&d| d <= radius).count();
        println!(
            "  radius {radius:3}: {:5.1}% of worlds could garrison",
            100.0 * hit as f64 / nearest.len() as f64
        );
    }

    for nth in 0..3 {
        println!("\nlane from the anchor to its nearest+{nth} town:");
        for radius in [15, 32, 64, 96, 128] {
            let mut sampled = 0u32;
            let mut hits = 0u32;
            for seed in seeds(n) {
                let towns = towns_near(seed, &db, (0, 0), 4);
                let Some(&destination) = towns.get(nth) else {
                    continue;
                };
                sampled += 1;
                if towns
                    .iter()
                    .filter(|&&t| t != destination)
                    .any(|&t| segment_distance(t, (0, 0), destination) <= radius as f64)
                {
                    hits += 1;
                }
            }
            println!(
                "  radius {radius:3}: {:5.1}% carry a predation candidate",
                100.0 * hits as f64 / sampled.max(1) as f64
            );
        }
    }
}
