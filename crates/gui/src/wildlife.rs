//! Ambient specks on the surface map: small swarms that fade in, loiter,
//! drift a few tiles as a group and fade out again.
//!
//! Decoration only. Nothing in the engine knows they exist — they cannot be
//! targeted, block nothing, draw no RNG and are never saved — which is why
//! they live in the renderer and are computed rather than stored.
//!
//! **A pure function of the world and the clock**, `cloud_shade`'s
//! argument: a swarm keyed to screen position would crawl across the
//! terrain as the camera slid, and one keyed to a per-frame random draw
//! would strobe. The ground is cut into roosts `ROOST_TILES` across; each
//! roost runs a cycle of `CYCLE_SECS`, and for the first part of it a swarm
//! lives there, fully determined by `(roost, cycle)`. Holding no state is
//! what lets a swarm survive the camera leaving and coming back.

use crate::fx::smoothstep;
use std::f32::consts::TAU;

/// How wide a roost is, in tiles. A screenful (~33x19) holds a dozen or so,
/// and `ROOST_CHANCE` of them are inhabited at all.
const ROOST_TILES: i32 = 9;

/// The share of roosts that ever hold a swarm. Below one so the map has
/// quiet stretches rather than an even sprinkle.
const ROOST_CHANCE: f32 = 0.55;

/// One roost's cycle, in seconds: a swarm's life plus the gap before the
/// next. The life is shorter than the cycle by design — the gap is what
/// makes swarms come and go rather than standing as permanent fixtures.
const CYCLE_SECS: f32 = 30.0;

/// The shortest and longest a swarm lives, in seconds.
const LIFE_MIN_SECS: f32 = 11.0;
const LIFE_MAX_SECS: f32 = 19.0;

/// How long a speck takes to come into focus and to fade, in seconds.
const FADE_SECS: f32 = 2.5;

/// How far apart one swarm's members appear, in seconds, so a swarm
/// gathers rather than switching on as a block.
const ARRIVAL_STAGGER_SECS: f32 = 0.35;

/// Members per swarm, inclusive.
const MEMBERS_MIN: u32 = 3;
const MEMBERS_MAX: u32 = 7;

/// How far a swarm drifts over its life, in tiles: loiter, travel, loiter.
const ROAM_MIN_TILES: f32 = 2.0;
const ROAM_MAX_TILES: f32 = 5.0;

/// Where in its life a swarm sets off and where it settles again, as
/// fractions of that life: the first stretch loitering where it formed, the
/// last loitering where it ends up.
const SET_OFF: f32 = 0.3;
const SETTLE: f32 = 0.75;

/// A quick small tremble over a member's slow orbit, in tiles: what makes a
/// speck read as alive rather than as a planet.
const TREMBLE_TILES: f32 = 0.06;

/// How far a member circles from the swarm's centre, in tiles.
const SWARM_RADIUS_TILES: f32 = 0.75;

/// A speck's side as a fraction of a tile. A `.` to a `*`, roughly.
const SIZE_MIN: f32 = 0.07;
const SIZE_MAX: f32 = 0.14;

/// How much a loitering speck's brightness breathes, as a fraction of it.
const TWINKLE_DEPTH: f32 = 0.35;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpeckShape {
    Square,
    Diamond,
}

/// One speck at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Speck {
    /// In world tile units: tile `(x, y)` covers `[x, x + 1)` on each axis.
    pub pos: (f32, f32),
    /// Side length as a fraction of a tile.
    pub size: f32,
    pub shape: SpeckShape,
    /// 0..1, the fade and the twinkle folded together.
    pub alpha: f32,
}

impl Speck {
    /// The tile the speck is over, which is what decides whether it may be
    /// drawn and what colour it takes.
    pub fn tile(&self) -> (i32, i32) {
        (self.pos.0.floor() as i32, self.pos.1.floor() as i32)
    }
}

/// Every speck alive at `now` whose swarm could reach the tiles `min..=max`.
///
/// Roosts are gathered with a margin of the furthest a swarm can roam, so a
/// swarm born just off screen still drifts on; the caller filters by tile.
pub fn specks(min: (i32, i32), max: (i32, i32), now: f64) -> Vec<Speck> {
    let reach = (ROAM_MAX_TILES + SWARM_RADIUS_TILES).ceil() as i32 + 1;
    let roost_of = |v: i32| v.div_euclid(ROOST_TILES);
    let mut out = Vec::new();
    for ry in roost_of(min.1 - reach)..=roost_of(max.1 + reach) {
        for rx in roost_of(min.0 - reach)..=roost_of(max.0 + reach) {
            if let Some(swarm) = swarm_at((rx, ry), now) {
                out.extend((0..swarm.members).filter_map(|i| swarm.member(i)));
            }
        }
    }
    out
}

/// One swarm, frozen at one instant of its life.
struct Swarm {
    roost: (i32, i32),
    cycle: i64,
    /// Seconds since the swarm was born.
    age: f32,
    life: f32,
    members: u32,
    centre: (f32, f32),
}

/// The swarm living in `roost` at `now`, if there is one.
fn swarm_at(roost: (i32, i32), now: f64) -> Option<Swarm> {
    if draw(roost, 0, 0) >= ROOST_CHANCE {
        return None;
    }
    // Offset per roost so every roost's swarm does not blink on at once.
    let shifted = now + (draw(roost, 0, 1) * CYCLE_SECS) as f64;
    let cycle = (shifted / CYCLE_SECS as f64).floor() as i64;
    let age = (shifted - cycle as f64 * CYCLE_SECS as f64) as f32;
    let life = lerp(LIFE_MIN_SECS, LIFE_MAX_SECS, draw(roost, cycle, 2));
    if age >= life {
        return None;
    }
    let members =
        MEMBERS_MIN + (draw(roost, cycle, 3) * (MEMBERS_MAX - MEMBERS_MIN + 1) as f32) as u32;
    let members = members.min(MEMBERS_MAX);
    let start = (
        (roost.0 * ROOST_TILES) as f32 + draw(roost, cycle, 4) * ROOST_TILES as f32,
        (roost.1 * ROOST_TILES) as f32 + draw(roost, cycle, 5) * ROOST_TILES as f32,
    );
    let heading = draw(roost, cycle, 6) * TAU;
    let roam = lerp(ROAM_MIN_TILES, ROAM_MAX_TILES, draw(roost, cycle, 7));
    let travelled = roam * smoothstep(SET_OFF, SETTLE, age / life);
    // A slow sway on top of the straight run, so a loitering swarm is not
    // pinned to one point.
    let sway = 0.35 * (age * 0.4 + heading).sin();
    let centre = (
        start.0 + heading.cos() * travelled - heading.sin() * sway,
        start.1 + heading.sin() * travelled + heading.cos() * sway,
    );
    Some(Swarm {
        roost,
        cycle,
        age,
        life,
        members,
        centre,
    })
}

impl Swarm {
    /// Member `i`, or `None` while it has yet to arrive or after it has gone.
    fn member(&self, i: u32) -> Option<Speck> {
        let key = |salt: u32| draw(self.roost, self.cycle, 16 + i * 8 + salt);
        // Arrivals are staggered and departures mirror them, so the swarm
        // thins out in the order it gathered.
        let stagger = i as f32 * ARRIVAL_STAGGER_SECS;
        let alpha_fade = fade(self.age - stagger, self.life - 2.0 * stagger);
        if alpha_fade <= 0.0 {
            return None;
        }
        let radius = SWARM_RADIUS_TILES * lerp(0.25, 1.0, key(0));
        // Either direction round the centre, never stationary.
        let spin = lerp(0.3, 0.9, key(1)) * if key(2) < 0.5 { -1.0 } else { 1.0 };
        let angle = key(3) * TAU + spin * self.age;
        let jitter = (
            TREMBLE_TILES * (self.age * 3.1 + key(4) * TAU).sin(),
            TREMBLE_TILES * (self.age * 2.7 + key(5) * TAU).cos(),
        );
        let twinkle = 1.0 - TWINKLE_DEPTH * 0.5 * (1.0 + (self.age * 2.3 + key(6) * TAU).sin());
        Some(Speck {
            pos: (
                self.centre.0 + 0.5 + radius * angle.cos() + jitter.0,
                self.centre.1 + 0.5 + radius * angle.sin() + jitter.1,
            ),
            size: lerp(SIZE_MIN, SIZE_MAX, key(7)),
            shape: if key(7) > 0.6 {
                SpeckShape::Diamond
            } else {
                SpeckShape::Square
            },
            alpha: alpha_fade * twinkle,
        })
    }
}

/// 0 → 1 over `FADE_SECS` from `age` 0, 1 → 0 over the last `FADE_SECS`
/// before `life`, eased at both ends; 0 outside `0..life`.
fn fade(age: f32, life: f32) -> f32 {
    if age <= 0.0 || age >= life {
        return 0.0;
    }
    let edge = FADE_SECS.min(life / 2.0);
    smoothstep(0.0, edge, age) * smoothstep(0.0, edge, life - age)
}

/// A stable draw in 0..1 for one `(roost, cycle, salt)`. Keyed on nothing
/// that moves frame to frame, `spark_scatter`'s reason: a re-roll per frame
/// is a speck teleporting.
fn draw(roost: (i32, i32), cycle: i64, salt: u32) -> f32 {
    let mut h = (roost.0 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (roost.1 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (cycle as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ (salt as u64).wrapping_mul(0x85EB_CA77_C2B2_AE63);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window large enough to hold many inhabited roosts.
    const MIN: (i32, i32) = (-40, -40);
    const MAX: (i32, i32) = (40, 40);

    #[test]
    fn a_speck_glides_rather_than_jumping_between_frames() {
        let frame = 1.0 / 60.0;
        let mut compared = 0;
        for step in 0..600 {
            let t = step as f64 * 0.37;
            for roost in roosts() {
                let (Some(a), Some(b)) = (swarm_at(roost, t), swarm_at(roost, t + frame)) else {
                    continue;
                };
                if a.cycle != b.cycle {
                    continue;
                }
                for i in 0..a.members {
                    if let (Some(p), Some(q)) = (a.member(i), b.member(i)) {
                        let d = ((p.pos.0 - q.pos.0).powi(2) + (p.pos.1 - q.pos.1).powi(2)).sqrt();
                        assert!(d < 0.05, "speck moved {d} tiles in one frame at t={t}");
                        compared += 1;
                    }
                }
            }
        }
        assert!(compared > 1000, "only {compared} pairs compared");
    }

    #[test]
    fn a_swarm_fades_in_and_out_rather_than_popping() {
        let mut seen = 0;
        for roost in roosts() {
            for step in 0..3000 {
                let t = step as f64 * 0.05;
                let Some(s) = swarm_at(roost, t) else {
                    continue;
                };
                for i in 0..s.members {
                    let stagger = i as f32 * ARRIVAL_STAGGER_SECS;
                    let at_edge = s.age - stagger < 0.2 || s.life - stagger - s.age < 0.2;
                    if at_edge && let Some(m) = s.member(i) {
                        assert!(
                            m.alpha < 0.05,
                            "speck at full strength at its edge: {}",
                            m.alpha
                        );
                        seen += 1;
                    }
                }
            }
        }
        assert!(seen > 0, "no speck was ever caught at its edge");
    }

    #[test]
    fn swarms_come_and_go() {
        let roost = roosts().next().expect("some roost is inhabited");
        let alive = (0..600)
            .filter(|step| swarm_at(roost, *step as f64 * 0.25).is_some())
            .count();
        assert!(alive > 0, "the roost never held a swarm");
        assert!(alive < 600, "the roost's swarm never left");
    }

    #[test]
    fn a_swarm_stays_together() {
        for roost in roosts() {
            for step in 0..200 {
                let Some(s) = swarm_at(roost, step as f64 * 0.7) else {
                    continue;
                };
                let reach = SWARM_RADIUS_TILES + 0.1;
                for i in 0..s.members {
                    if let Some(m) = s.member(i) {
                        let d = ((m.pos.0 - 0.5 - s.centre.0).powi(2)
                            + (m.pos.1 - 0.5 - s.centre.1).powi(2))
                        .sqrt();
                        assert!(d <= reach, "member strayed {d} tiles from its swarm");
                    }
                }
            }
        }
    }

    #[test]
    fn some_ground_is_quiet_and_some_is_busy() {
        let total = (-10..10)
            .flat_map(|y| (-10..10).map(move |x| (x, y)))
            .filter(|r| draw(*r, 0, 0) < ROOST_CHANCE)
            .count();
        assert!(total > 50 && total < 350, "{total} of 400 roosts inhabited");
        assert!(!specks(MIN, MAX, 12.0).is_empty());
    }

    fn roosts() -> impl Iterator<Item = (i32, i32)> {
        (-4..4)
            .flat_map(|y| (-4..4).map(move |x| (x, y)))
            .filter(|r| draw(*r, 0, 0) < ROOST_CHANCE)
    }
}
