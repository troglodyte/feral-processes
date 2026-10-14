//! The light a Stack frame is lit by, as a `Lit` per corridor surface.
//!
//! The light comes from a fixed **world** direction per frame (seeded off the
//! frame by the engine, drawn by `lighting::light_from_seed` like a battle
//! board's), so one flank wall is lit and the other shaded, and turning the
//! party swaps them — an orientation cue as well as a look. The corridor is
//! drawn in view space, so each surface's world normal is built from the
//! party's facing here and nowhere else.
//!
//! Only geometry is lit. Marks, the heading and the frame map are what the
//! player reads and stay as they are (see `stack.rs`).

use feral_processes_engine::stack::Dir;

use super::lighting::{LEVEL_MAX, LEVEL_MIN, Light, Lit, light_from_seed};

/// How far a wall's level swings from 1.0 at full strength facing straight
/// into or away from the light. Wider than the band on purpose: the band
/// clamps the strongest lights, and that clamp is what keeps a lit wall from
/// outshining a door's hue (`stack.rs` tests hold the worst case).
pub(super) const WALL_SPAN: f32 = 0.22;

/// The floor and ceiling take a gentler gradient than the walls: they are
/// big, flat and nearly all of the screen, so the same swing reads as a
/// wash rather than as a light.
pub(super) const FLOOR_SPAN: f32 = 0.10;

/// How much a floor shadow takes off, at full strength facing the caster.
/// Enough to read against the floor gradient (which spans only `FLOOR_SPAN`)
/// without reaching `UNLIT`'s darkness.
pub(super) const SHADOW: f32 = 0.20;

/// The cone's reach in cells, which turns a cell's offset from the party
/// into the unit-ish vector the floor gradient dots with the light.
const REACH: f32 = 4.0;

/// A flank wall: which side of an open cell it stands on, as the party sees
/// it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Side {
    Left,
    Right,
}

/// The half of a floor quad nearest a neighbouring cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Half {
    Left,
    Right,
    /// Toward the party.
    Near,
    /// Away from the party.
    Far,
}

/// Which orthogonal neighbours of a cell are solid *and inside the cone*.
/// A neighbour outside the cone is passed as `false`: the cone is what the
/// party can see, so it throws no shadow either.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Solids {
    pub left: bool,
    pub right: bool,
    pub near: bool,
    pub far: bool,
}

/// One frame's light, with the party's basis it is read in.
#[derive(Clone, Copy, Debug)]
pub(super) struct StackLight {
    light: Light,
    forward: (f32, f32),
    right: (f32, f32),
}

fn unit((x, y): (i32, i32)) -> (f32, f32) {
    (x as f32, y as f32)
}

fn dot(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.0 + a.1 * b.1
}

impl StackLight {
    pub fn new(seed: u64, facing: Dir) -> StackLight {
        StackLight {
            light: light_from_seed(seed),
            forward: unit(facing.delta()),
            right: unit(facing.right_delta()),
        }
    }

    fn lit(&self, level: f32) -> Lit {
        Lit {
            level: level.clamp(LEVEL_MIN, LEVEL_MAX),
            cast: self.light.cast,
        }
    }

    fn facing_light(&self, normal: (f32, f32), span: f32) -> f32 {
        1.0 + span * self.light.strength * dot(normal, self.light.toward)
    }

    /// A flank wall seen edge-on: the left one faces right, into the
    /// corridor, and the right one faces left.
    pub fn wall(&self, side: Side) -> Lit {
        let normal = match side {
            Side::Left => self.right,
            Side::Right => (-self.right.0, -self.right.1),
        };
        self.lit(self.facing_light(normal, WALL_SPAN))
    }

    /// A face ahead (rock, door, sealed door), which faces the party.
    pub fn face(&self) -> Lit {
        self.lit(self.facing_light((-self.forward.0, -self.forward.1), WALL_SPAN))
    }

    /// Floor and ceiling of the cell `depth` ahead and `lateral` to the
    /// right: brighter toward the light's side of the cone.
    pub fn floor(&self, depth: usize, lateral: i32) -> Lit {
        let offset = (
            (lateral as f32 * self.right.0 + depth as f32 * self.forward.0) / REACH,
            (lateral as f32 * self.right.1 + depth as f32 * self.forward.1) / REACH,
        );
        self.lit(self.facing_light(offset, FLOOR_SPAN))
    }

    /// The shadow each solid neighbour toward the light throws on the half
    /// of the floor nearest it. A neighbour away from the light throws none.
    /// Each half has exactly one neighbour, so "deepest per half" is that
    /// neighbour's own depth.
    pub fn floor_shadows(
        &self,
        depth: usize,
        lateral: i32,
        solids: Solids,
    ) -> [Option<(Half, Lit)>; 4] {
        let floor = self.floor(depth, lateral).level;
        let neg = |v: (f32, f32)| (-v.0, -v.1);
        let casters = [
            (Half::Left, solids.left, neg(self.right)),
            (Half::Right, solids.right, self.right),
            (Half::Near, solids.near, neg(self.forward)),
            (Half::Far, solids.far, self.forward),
        ];
        let mut shadows = casters.map(|(half, solid, dir)| {
            let toward = dot(dir, self.light.toward);
            (solid && toward > 0.0).then(|| {
                (
                    half,
                    self.lit(floor - SHADOW * self.light.strength * toward),
                )
            })
        });
        let level = |s: &Option<(Half, Lit)>| s.as_ref().map_or(f32::INFINITY, |(_, l)| l.level);
        shadows.sort_by(|a, b| level(b).total_cmp(&level(a)));
        shadows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FACINGS: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

    /// The first seed whose light points `want` along world x (or y) — found
    /// rather than hardcoded so the test says what it needs of the seed.
    fn seed_where(want: impl Fn((f32, f32)) -> bool) -> u64 {
        (0..10_000u64)
            .find(|&s| want(light_from_seed(s).toward))
            .expect("no seed in 0..10000 has a light pointing that way")
    }

    #[test]
    fn the_floor_brightens_toward_the_light() {
        let seed = seed_where(|t| t.0 > 0.6);
        let n = StackLight::new(seed, Dir::North);
        assert!(n.floor(1, 1).level > n.floor(1, -1).level);
    }

    #[test]
    fn overlapping_shadows_come_back_deepest_last() {
        // Left and near both toward the light: they share the near-left
        // corner, and the deeper must be painted over the lighter.
        // The left shadow is the deeper one, so the natural order (left before
        // near) is the wrong one and only the sort puts it last.
        let seed = seed_where(|t| t.1 > 0.1 && -t.0 > t.1 + 0.1);
        let light = StackLight::new(seed, Dir::North);
        let solids = Solids {
            left: true,
            right: false,
            near: true,
            far: false,
        };
        let levels: Vec<f32> = light
            .floor_shadows(1, 0, solids)
            .into_iter()
            .flatten()
            .map(|(_, l)| l.level)
            .collect();
        assert_eq!(levels.len(), 2);
        assert!(levels[0] >= levels[1]);
    }

    #[test]
    fn north_and_south_swap_the_brighter_flank_wall() {
        // Light with a clear east component: facing north the left wall
        // faces east (toward it), facing south the right wall does.
        let seed = seed_where(|t| t.0 > 0.6);
        let n = StackLight::new(seed, Dir::North);
        let s = StackLight::new(seed, Dir::South);
        assert!(n.wall(Side::Left).level > n.wall(Side::Right).level);
        assert!(s.wall(Side::Right).level > s.wall(Side::Left).level);
    }

    #[test]
    fn east_and_west_swap_the_brighter_flank_wall() {
        // Clear south component (y grows down): facing east the right-hand
        // wall faces north, away; the left faces south, toward the light.
        let seed = seed_where(|t| t.1 > 0.6);
        let e = StackLight::new(seed, Dir::East);
        let w = StackLight::new(seed, Dir::West);
        assert!(e.wall(Side::Left).level > e.wall(Side::Right).level);
        assert!(w.wall(Side::Right).level > w.wall(Side::Left).level);
    }

    #[test]
    fn a_face_is_brighter_with_the_light_behind_the_party() {
        // toward.y > 0 is a light to the south: behind a north-facing party,
        // in front of a south-facing one.
        let seed = seed_where(|t| t.1 > 0.6);
        let behind = StackLight::new(seed, Dir::North).face().level;
        let ahead = StackLight::new(seed, Dir::South).face().level;
        assert!(behind > ahead, "{behind} vs {ahead}");
    }

    #[test]
    fn every_level_stays_in_the_band() {
        for seed in 0..500u64 {
            for facing in FACINGS {
                let l = StackLight::new(seed, facing);
                let mut levels = vec![
                    l.wall(Side::Left).level,
                    l.wall(Side::Right).level,
                    l.face().level,
                ];
                for depth in 0..5 {
                    for lateral in -3..=3 {
                        levels.push(l.floor(depth, lateral).level);
                        let all = Solids {
                            left: true,
                            right: true,
                            near: true,
                            far: true,
                        };
                        for (_, lit) in l.floor_shadows(depth, lateral, all).into_iter().flatten() {
                            levels.push(lit.level);
                        }
                    }
                }
                for level in levels {
                    assert!(
                        (LEVEL_MIN..=LEVEL_MAX).contains(&level),
                        "seed {seed} {facing:?}: level {level}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_solid_neighbour_toward_the_light_shadows_its_half_and_only_that() {
        // Light to the east, facing north: the right-hand neighbour is
        // toward it, the left-hand one away.
        let seed = seed_where(|t| t.0 > 0.6);
        let l = StackLight::new(seed, Dir::North);
        let both = Solids {
            left: true,
            right: true,
            ..Solids::default()
        };
        let shadows = l.floor_shadows(1, 0, both);
        let halves: Vec<Half> = shadows.iter().flatten().map(|(h, _)| *h).collect();
        assert_eq!(halves, vec![Half::Right], "away from the light or none");
        let (_, lit) = shadows.iter().flatten().next().unwrap();
        assert!(
            lit.level < l.floor(1, 0).level,
            "the shadow does not darken the floor"
        );
    }

    #[test]
    fn a_neighbour_outside_the_cone_throws_no_shadow() {
        let seed = seed_where(|t| t.0 > 0.6);
        let l = StackLight::new(seed, Dir::North);
        let shadows = l.floor_shadows(1, 0, Solids::default());
        assert!(shadows.iter().all(Option::is_none));
    }
}
