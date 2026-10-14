//! The light a battle map is lit by: a direction, a colour and a strength
//! chosen per board, and the shadows its cover and props throw.
//!
//! **Seeded off the board's own cells**, so a fight keeps its light every
//! frame and across a reload, and two fights rarely share one — the boards
//! all looked alike under one flat light. The renderer has no RNG stream to
//! draw from and must not take one from the engine, so the seed is a fold of
//! what the view already carries.
//!
//! **Lighting spends brightness, and brightness is how the board says what a
//! cell is** (`tactical::cell_color`'s rule). The level is clamped to a band
//! narrow enough that a lit `Blocked` cell stays darker than a shadowed
//! `Open` one and a shadowed `Cover` brighter than a lit `Rough` one. Open
//! and rough may cross in deep shadow; their art tells them apart.
//!
//! Only the ground and the props are lit. Washes, bodies and flashes are
//! drawn unlit, because they are what the player plays by.

use feral_processes_engine::tactical::map::Board;

use super::hud::palette;
use crate::paint::Color;

/// The band a cell's level is clamped to. The ratio of the two is what
/// keeps the kinds apart — `tactical`'s
/// `no_light_lets_blocked_or_cover_cross_another_kind`.
pub(super) const LEVEL_MIN: f32 = 0.80;
pub(super) const LEVEL_MAX: f32 = 1.20;

/// How much brighter the edge facing the light is than the middle, at full
/// strength.
const GRADIENT_SPAN: f32 = 0.20;

/// How much a shadow takes off the cell right behind its caster, at full
/// strength; it fades to nothing over its length.
const SHADOW_DEPTH: f32 = 0.45;

/// Cells a shadow reaches beyond the first at full strength: harsh light is
/// low light, and low light throws long shadows.
const SHADOW_EXTRA: f32 = 3.0;

/// The range a board's light strength is drawn from. Never zero, so every
/// board has some direction to it.
const STRENGTH_MIN: f32 = 0.35;
const STRENGTH_MAX: f32 = 1.0;

/// One board's light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Light {
    /// Unit vector from the board's middle toward the light, in board cells
    /// (y grows down the screen).
    pub toward: (f32, f32),
    pub cast: Color,
    pub strength: f32,
}

/// What one cell is lit by: a level and the board's cast, multiplied into a
/// colour by `apply`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Lit {
    pub level: f32,
    pub cast: Color,
}

impl Lit {
    /// No light at all: `apply` is the identity. What a draw outside a
    /// board — a test, a legend swatch — passes.
    pub const NONE: Lit = Lit {
        level: 1.0,
        cast: Color::new(1.0, 1.0, 1.0, 1.0),
    };

    pub fn apply(self, c: Color) -> Color {
        Color::new(
            c.r * self.level * self.cast.r,
            c.g * self.level * self.cast.g,
            c.b * self.level * self.cast.b,
            c.a,
        )
    }
}

/// A board's light worked out for every cell, once a frame.
pub(super) struct Lighting {
    side: i32,
    levels: Vec<f32>,
    cast: Color,
}

impl Lighting {
    /// The board's own light, with cover and every prop that blocks sight
    /// or gives cover throwing a shadow — `Board`'s own predicates, never
    /// `Board::cell`, so a prop is an occluder without a line of its own.
    pub fn of(board: &Board) -> Lighting {
        let light = board_light(board);
        let occludes = |x, y| board.is_cover(x, y) || board.blocks_sight(x, y);
        Lighting {
            side: board.side,
            levels: levels(board.side, &light, occludes),
            cast: light.cast,
        }
    }

    /// The light on `cell`; off the board, none.
    pub fn at(&self, (x, y): (i32, i32)) -> Lit {
        if x < 0 || y < 0 || x >= self.side || y >= self.side {
            return Lit::NONE;
        }
        Lit {
            level: self.levels[(y * self.side + x) as usize],
            cast: self.cast,
        }
    }
}

/// The light a board is lit by, folded from its cells.
pub(super) fn board_light(board: &Board) -> Light {
    let mut h = fnv(FNV_BASIS, board.side as u64);
    for (_, kind) in board.cells() {
        h = fnv(h, kind as u64);
    }
    light_from_seed(h)
}

fn light_from_seed(seed: u64) -> Light {
    let mut state = seed;
    let angle = unit(splitmix(&mut state)) * std::f32::consts::TAU;
    let casts = &palette::LIGHT_CASTS;
    // Lemire's high-bit reduction, never `%`: the low bits of a fold are
    // the weak ones.
    let pick = ((splitmix(&mut state) >> 32) * casts.len() as u64) >> 32;
    let strength = STRENGTH_MIN + (STRENGTH_MAX - STRENGTH_MIN) * unit(splitmix(&mut state));
    Light {
        toward: (angle.cos(), angle.sin()),
        cast: casts[pick as usize],
        strength,
    }
}

/// Every cell's level, row-major: the gradient toward the light, less the
/// deepest shadow falling on it, clamped to the band.
fn levels(side: i32, light: &Light, occludes: impl Fn(i32, i32) -> bool) -> Vec<f32> {
    let half = (side as f32 / 2.0).max(1.0);
    let middle = (side as f32 - 1.0) / 2.0;
    let mut shadow = vec![0.0f32; (side * side).max(0) as usize];
    let reach = 1 + (light.strength * SHADOW_EXTRA).round() as i32;
    let away = (-light.toward.0, -light.toward.1);
    for y in 0..side {
        for x in 0..side {
            if !occludes(x, y) {
                continue;
            }
            for k in 1..=reach {
                let sx = x + (k as f32 * away.0).round() as i32;
                let sy = y + (k as f32 * away.1).round() as i32;
                if sx < 0 || sy < 0 || sx >= side || sy >= side {
                    break;
                }
                let fade = 1.0 - (k - 1) as f32 / reach as f32;
                let depth = SHADOW_DEPTH * light.strength * fade;
                let cell = &mut shadow[(sy * side + sx) as usize];
                *cell = cell.max(depth);
            }
        }
    }
    (0..side * side)
        .map(|i| {
            let (x, y) = (i % side, i / side);
            let along = ((x as f32 - middle) * light.toward.0
                + (y as f32 - middle) * light.toward.1)
                / half;
            let gradient = GRADIENT_SPAN * light.strength * along;
            (1.0 + gradient - shadow[i as usize]).clamp(LEVEL_MIN, LEVEL_MAX)
        })
        .collect()
}

const FNV_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

fn fnv(h: u64, word: u64) -> u64 {
    word.to_le_bytes()
        .iter()
        .fold(h, |h, &b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The top 24 bits as a fraction in `[0, 1)` — all an `f32` can hold.
fn unit(x: u64) -> f32 {
    (x >> 40) as f32 / (1u64 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::tactical::map::{BattleSpec, generate};
    use feral_processes_engine::tactical::props::PropDb;
    use feral_processes_engine::world::Biome;

    fn board(seed: u32) -> Board {
        generate(
            BattleSpec {
                world_seed: seed,
                site: (3, 4),
                tick: 0,
                zone: 1,
                biome: Biome::OpenGrid,
                bodies: 6,
            },
            &PropDb::default(),
        )
    }

    fn light_due_east(strength: f32) -> Light {
        Light {
            toward: (1.0, 0.0),
            cast: Color::new(1.0, 1.0, 1.0, 1.0),
            strength,
        }
    }

    #[test]
    fn a_board_is_always_lit_the_same_way() {
        assert_eq!(board_light(&board(7)), board_light(&board(7)));
    }

    #[test]
    fn different_boards_get_different_light() {
        let lights: Vec<Light> = (0..40).map(|s| board_light(&board(s))).collect();
        let quadrants: std::collections::BTreeSet<(bool, bool)> = lights
            .iter()
            .map(|l| (l.toward.0 > 0.0, l.toward.1 > 0.0))
            .collect();
        assert_eq!(quadrants.len(), 4, "the light comes from every side");
        let casts = palette::LIGHT_CASTS
            .iter()
            .filter(|c| lights.iter().any(|l| l.cast == **c))
            .count();
        assert_eq!(casts, palette::LIGHT_CASTS.len(), "every cast is drawn");
        let (lo, hi) = lights.iter().fold((f32::MAX, f32::MIN), |(lo, hi), l| {
            (lo.min(l.strength), hi.max(l.strength))
        });
        assert!(hi - lo > 0.4, "harsh and soft light both: {lo}..{hi}");
        for l in &lights {
            let len = (l.toward.0.powi(2) + l.toward.1.powi(2)).sqrt();
            assert!((len - 1.0).abs() < 1e-4, "{l:?}");
        }
    }

    #[test]
    fn the_side_facing_the_light_is_brighter() {
        let lv = levels(9, &light_due_east(1.0), |_, _| false);
        assert!(lv[4 * 9 + 8] > lv[4 * 9 + 4] && lv[4 * 9 + 4] > lv[4 * 9]);
        assert!(
            (lv[4 * 9 + 4] - 1.0).abs() < 1e-4,
            "the middle is unlit by the gradient"
        );
    }

    #[test]
    fn a_shadow_falls_away_from_the_light_and_fades() {
        let flat = levels(9, &light_due_east(1.0), |_, _| false);
        let lv = levels(9, &light_due_east(1.0), |x, y| (x, y) == (4, 4));
        let drop = |i: usize| flat[i] - lv[i];
        let row = 4 * 9;
        assert!(
            drop(row + 3) > drop(row + 2),
            "{} {}",
            drop(row + 3),
            drop(row + 2)
        );
        assert!(drop(row + 2) > 0.0);
        assert_eq!(drop(row + 5), 0.0, "nothing falls toward the light");
        assert_eq!(drop(3 * 9 + 3), 0.0, "nor off the line");
    }

    #[test]
    fn harsh_light_throws_a_longer_shadow_than_soft() {
        let reach = |strength: f32| {
            let flat = levels(16, &light_due_east(strength), |_, _| false);
            let lv = levels(16, &light_due_east(strength), |x, y| (x, y) == (12, 8));
            (0..12)
                .filter(|&x| flat[8 * 16 + x] > lv[8 * 16 + x])
                .count()
        };
        assert!(
            reach(1.0) > reach(STRENGTH_MIN),
            "{} {}",
            reach(1.0),
            reach(STRENGTH_MIN)
        );
    }

    #[test]
    fn every_level_stays_in_the_band() {
        for s in 0..20 {
            let b = board(s);
            let lighting = Lighting::of(&b);
            for (cell, _) in b.cells() {
                let level = lighting.at(cell).level;
                assert!(
                    (LEVEL_MIN..=LEVEL_MAX).contains(&level),
                    "{cell:?}: {level}"
                );
            }
        }
    }

    #[test]
    fn off_the_board_is_unlit() {
        let lighting = Lighting::of(&board(1));
        assert_eq!(lighting.at((-1, 0)), Lit::NONE);
        assert_eq!(lighting.at((0, 10_000)), Lit::NONE);
    }

    /// Wide enough to show, narrow enough to stay off the washes' hues.
    #[test]
    fn no_light_cast_is_saturated_enough_to_read_as_a_wash() {
        for c in palette::LIGHT_CASTS {
            let spread = c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b);
            assert!(spread < 0.165, "{c:?}: {spread}");
            assert!(c.r.max(c.g).max(c.b) <= 1.0);
        }
    }
}
