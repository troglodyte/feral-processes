//! An instrument, not a gate: prints where the wild population actually sits
//! after entry, after camping, after travelling and after a session's worth of
//! milling around a base. Ignored by default; the numbers it produced are
//! written up in `docs/measurements/2026-08-18-wild-population-halo.md`.
//!
//! ```sh
//! cargo test -p feral-processes-engine probe_wild_density -- --ignored --nocapture
//! ```

use super::support::*;
use crate::components::{Hostile, Position, PowerReserve};
use crate::tuning::{WILD_LOCAL_DENSITY_TARGET, WILD_SPAWN_RADIUS_TILES};
use crate::*;
use bevy_ecs::prelude::With;

fn hostiles(game: &mut Game) -> Vec<(i32, i32)> {
    let mut q = game.world.query_filtered::<&Position, With<Hostile>>();
    q.iter(&game.world).map(|p| (p.x, p.y)).collect()
}

/// Hostiles within the density box (Chebyshev `WILD_SPAWN_RADIUS_TILES`) of a point.
fn box_count(pts: &[(i32, i32)], cx: i32, cy: i32) -> usize {
    pts.iter()
        .filter(|(x, y)| (x - cx).abs().max((y - cy).abs()) <= WILD_SPAWN_RADIUS_TILES)
        .count()
}

fn keep_alive(game: &mut Game) {
    let p = game.player_entity();
    if let Some(mut r) = game.world.get_mut::<PowerReserve>(p) {
        r.fill();
    }
}

fn rings(pts: &[(i32, i32)], ox: i32, oy: i32) -> String {
    let mut out = String::new();
    for band in 0..8 {
        let lo = band * 20;
        let hi = lo + 20;
        let n = pts
            .iter()
            .filter(|(x, y)| {
                let d = (x - ox).abs().max((y - oy).abs());
                d >= lo && d < hi
            })
            .count();
        out.push_str(&format!("  ring {lo:>3}-{hi:<3}: {n}\n"));
    }
    out
}

#[test]
#[ignore]
fn probe_wild_density() {
    let mut game = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let start = *game.world.get::<Position>(game.player_entity()).unwrap();
    let (ox, oy) = (start.x, start.y);

    let pts = hostiles(&mut game);
    println!("\n=== AT ZONE ENTRY (target {WILD_LOCAL_DENSITY_TARGET}/box) ===");
    println!("total hostiles: {}", pts.len());
    println!("{}", rings(&pts, ox, oy));
    println!("box at origin: {}", box_count(&pts, ox, oy));
    for d in [20, 40, 60, 100] {
        println!("box at +{d}x: {}", box_count(&pts, ox + d, oy));
    }

    // --- Camp: stand still, the way resting and tending machines does. ---
    let mut camped = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for i in 0..20_000 {
        if i % 100 == 0 {
            keep_alive(&mut camped);
        }
        camped.tick();
    }
    let pts = hostiles(&mut camped);
    println!("\n=== AFTER 20,000 TICKS STANDING STILL ===");
    println!("total hostiles: {}", pts.len());
    println!("{}", rings(&pts, ox, oy));
    println!("box at player: {}", box_count(&pts, ox, oy));

    // --- Travel: one tile per tick, the real walking rate. ---
    let mut trav = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut waypoints = Vec::new();
    for step in 1..=300 {
        if step % 100 == 0 {
            keep_alive(&mut trav);
        }
        {
            let p = trav.player_entity();
            let mut pos = trav.world.get_mut::<Position>(p).unwrap();
            pos.x += 1;
        }
        trav.tick();
        if step % 25 == 0 {
            let x = ox + step;
            waypoints.push((step, x));
        }
    }
    let pts = hostiles(&mut trav);
    println!("\n=== AFTER WALKING 300 TILES EAST (1 tile/tick) ===");
    println!("total hostiles: {}", pts.len());
    println!("{}", rings(&pts, ox, oy));
    for (step, x) in waypoints {
        println!(
            "  box centred {step:>3} tiles out: {} (target {WILD_LOCAL_DENSITY_TARGET})",
            box_count(&pts, x, oy)
        );
    }
    // --- Mill: potter about the base the way real play does. ---
    let mut mill = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut seed = 12345u64;
    for i in 0..20_000 {
        if i % 100 == 0 {
            keep_alive(&mut mill);
        }
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let dx = ((seed >> 33) % 3) as i32 - 1;
        let dy = ((seed >> 45) % 3) as i32 - 1;
        {
            let p = mill.player_entity();
            let mut pos = mill.world.get_mut::<Position>(p).unwrap();
            if (pos.x + dx - ox).abs() <= 8 && (pos.y + dy - oy).abs() <= 8 {
                pos.x += dx;
                pos.y += dy;
            }
        }
        mill.tick();
    }
    let pts = hostiles(&mut mill);
    println!("\n=== AFTER 20,000 TICKS MILLING WITHIN 8 TILES OF BASE ===");
    println!("total hostiles: {}", pts.len());
    println!("{}", rings(&pts, ox, oy));
    for d in [0, 25, 50, 75, 100] {
        println!(
            "  box centred {d:>3} tiles out: {} (target {WILD_LOCAL_DENSITY_TARGET})",
            box_count(&pts, ox + d, oy)
        );
    }
}
