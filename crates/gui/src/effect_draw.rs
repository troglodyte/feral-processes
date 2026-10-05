//! The drawing half of the battle-effects library: each travel and impact
//! primitive as a pure function of progress, plus the painter calls that
//! consume them.
//!
//! Every shape is a function of `(progress, cell, index)` and nothing else —
//! no clock, no RNG — which is what lets the tests assert timing without a
//! painter and what stops a jittered line strobing: `fx::spark_scatter`'s
//! rule, and its hash.

use crate::effects::{EffectDef, Impact, Travel};
use crate::fx::{BOLT_SECONDS, BOLT_THICKNESS_PX, spark_scatter};
use crate::paint::{Color, Painter};

/// Share of the flight one pulse of a `Pulses` volley spends in the air. The
/// rest is the stagger that spreads the volley's launches, so the last pulse
/// still lands by the end of `BOLT_SECONDS`.
const PULSE_FLIGHT_FRACTION: f32 = 0.4;
/// How much of its flight a pulse's lit length covers.
const PULSE_LENGTH_FRACTION: f32 = 0.18;
/// Longest a `Beam` may hold lit, as a share of the flight; the beam must
/// have time to grow before it holds.
const BEAM_MAX_HOLD: f32 = 0.9;

/// Kinks in a zap's path, and how far a kink strays from the straight line.
const ZAP_SEGMENTS: u32 = 8;
const ZAP_AMPLITUDE_TILES: f32 = 0.3;
/// Salt so a zap's kink and its crackle at one cell do not share a draw.
const CRACKLE_SALT: u32 = 0x1F12_3BB5;
const CRACKLE_LINES: u32 = 5;
const CRACKLE_REACH_TILES: f32 = 0.45;

const MUZZLE_SECONDS: f64 = 0.12;
const MUZZLE_LINES: u32 = 6;
const MUZZLE_REACH_TILES: f32 = 0.4;

/// Lifetimes of the single-cell and area impacts, from arrival.
const EXPLOSION_SECONDS: f64 = 0.45;
const ZAP_IMPACT_SECONDS: f64 = 0.18;
const SLASH_SECONDS: f64 = 0.2;
const SMOKE_SECONDS: f64 = 0.8;
const FLASH_SECONDS: f64 = 0.35;
/// Opacity of a flash at the instant it lands; low enough that the glyphs
/// under the tint stay readable.
const FLASH_PEAK_ALPHA: f32 = 0.45;

/// A `Ball`'s disc, drawn as a regular polygon because `Painter` has no
/// circle.
const BALL_RADIUS_TILES: f32 = 0.18;
const BALL_SIDES: u32 = 12;
/// Share of the flight after which the ball is on its target; it sits there
/// until the flight ends so the landing flash does not meet an empty gap.
const BALL_ARRIVE_FRACTION: f32 = 0.9;

const SMOKE_PUFFS: u32 = 4;
const SMOKE_RISE_TILES: f32 = 0.6;
const SMOKE_DRIFT_TILES: f32 = 0.25;
const SMOKE_GREY: f32 = 0.6;
const RING_SEGMENTS: u32 = 16;
const IMPACT_THICKNESS_PX: f32 = 2.0;

const FLASH_WHITE: Color = Color::new(1.0, 0.95, 0.8, 1.0);
const EXPLOSION_COLOR: Color = Color::new(1.0, 0.65, 0.2, 1.0);

/// How long the travel half of an effect takes.
pub(crate) fn travel_seconds(travel: Travel) -> f64 {
    match travel {
        Travel::None => 0.0,
        _ => BOLT_SECONDS,
    }
}

/// How long one impact is drawn after arrival. `Sparks` is the `Hit` cue's
/// own burst, which the engine's tactical cue already starts, so it adds no
/// lifetime here.
pub(crate) fn impact_seconds(impact: Impact) -> f64 {
    match impact {
        Impact::Sparks => 0.0,
        Impact::Explosion { .. } => EXPLOSION_SECONDS,
        Impact::Zap => ZAP_IMPACT_SECONDS,
        Impact::Slash => SLASH_SECONDS,
        Impact::Smoke => SMOKE_SECONDS,
        Impact::Flash => FLASH_SECONDS,
    }
}

/// How long a cue must be kept: its flight plus its longest impact, never
/// less than `BOLT_SECONDS` for a travelling effect.
pub(crate) fn blow_seconds(def: &EffectDef) -> f64 {
    let travel = travel_seconds(def.travel);
    let impacts = def
        .impact
        .iter()
        .map(|i| impact_seconds(*i))
        .fold(0.0, f64::max);
    let muzzle = if def.muzzle { MUZZLE_SECONDS } else { 0.0 };
    (travel + impacts).max(muzzle)
}

/// Where pulse `index` of `count` is at flight progress `t` (0..1), as the
/// `(tail, head)` fractions of the line, or `None` outside its flight.
pub(crate) fn pulse_span(index: u8, count: u8, t: f32) -> Option<(f32, f32)> {
    let count = count.max(1) as f32;
    let launch = index as f32 / count * (1.0 - PULSE_FLIGHT_FRACTION);
    let along = (t - launch) / PULSE_FLIGHT_FRACTION;
    if !(0.0..1.0).contains(&along) {
        return None;
    }
    Some((
        (along - PULSE_LENGTH_FRACTION / PULSE_FLIGHT_FRACTION).max(0.0),
        along,
    ))
}

/// The fraction of the line a beam covers at flight progress `t`. It grows
/// until `1 - hold` and holds there to the end of the flight.
pub(crate) fn beam_head(t: f32, hold: f32) -> f32 {
    let grow = 1.0 - hold.clamp(0.0, BEAM_MAX_HOLD);
    (t / grow).clamp(0.0, 1.0)
}

/// A zap kink's sideways stray in -1..1, stable for a cell and index.
pub(crate) fn zap_offset(cell: (i32, i32), index: u32) -> f32 {
    spark_scatter(cell, index) * 2.0 - 1.0
}

/// An explosion ring's radius in tiles at impact progress `u` (0..1):
/// decelerating, so the blast reads as thrown outward.
pub(crate) fn explosion_radius(u: f32, radius: f32) -> f32 {
    radius * (1.0 - (1.0 - u.clamp(0.0, 1.0)).powi(2))
}

/// How far along the line a `Ball` is at flight progress `t`.
pub(crate) fn ball_along(t: f32) -> f32 {
    (t / BALL_ARRIVE_FRACTION).clamp(0.0, 1.0)
}

/// A flash's opacity at impact progress `u` (0..1): its peak, fading to 0.
pub(crate) fn flash_alpha(u: f32) -> f32 {
    FLASH_PEAK_ALPHA * (1.0 - u.clamp(0.0, 1.0))
}

fn faded(base: Color, alpha: f32) -> Color {
    Color::new(base.r, base.g, base.b, base.a * alpha.clamp(0.0, 1.0))
}

type Px = (f32, f32);

fn lerp(a: Px, b: Px, t: f32) -> Px {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// The travel primitive from `a` to `b` (cell centres) at `t`.
pub(crate) fn draw_travel(
    painter: &Painter,
    travel: Travel,
    cells: ((i32, i32), (i32, i32)),
    (a, b): (Px, Px),
    tile_px: f32,
    t: f32,
    color: Color,
) {
    match travel {
        Travel::Streak => {
            let tail = (t - crate::fx::BOLT_HEAD_FRACTION).max(0.0);
            let (hx, hy) = lerp(a, b, t);
            let (tx, ty) = lerp(a, b, tail);
            painter.line(
                tx,
                ty,
                hx,
                hy,
                BOLT_THICKNESS_PX,
                faded(color, 1.0 - t * 0.4),
            );
        }
        Travel::Pulses { count } => {
            for index in 0..count {
                if let Some((tail, head)) = pulse_span(index, count, t) {
                    let (tx, ty) = lerp(a, b, tail);
                    let (hx, hy) = lerp(a, b, head);
                    painter.line(tx, ty, hx, hy, BOLT_THICKNESS_PX, color);
                }
            }
        }
        Travel::Beam { hold } => {
            let (hx, hy) = lerp(a, b, beam_head(t, hold));
            painter.line(
                a.0,
                a.1,
                hx,
                hy,
                BOLT_THICKNESS_PX * 1.5,
                faded(color, 1.0 - t * 0.3),
            );
        }
        Travel::Zap => {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            let (nx, ny) = (-dy / len, dx / len);
            let point = |i: u32| {
                let on = lerp(a, b, i as f32 / ZAP_SEGMENTS as f32);
                // Ends pinned, so the arc leaves and lands on a glyph.
                let stray = if i == 0 || i == ZAP_SEGMENTS {
                    0.0
                } else {
                    zap_offset(cells.0, i) * ZAP_AMPLITUDE_TILES * tile_px
                };
                (on.0 + nx * stray, on.1 + ny * stray)
            };
            let color = faded(color, 1.0 - t * 0.6);
            for i in 0..ZAP_SEGMENTS {
                let (p, q) = (point(i), point(i + 1));
                painter.line(p.0, p.1, q.0, q.1, IMPACT_THICKNESS_PX, color);
            }
        }
        Travel::Ball => {
            let (x, y) = lerp(a, b, ball_along(t));
            let r = BALL_RADIUS_TILES * tile_px;
            let step = std::f32::consts::TAU / BALL_SIDES as f32;
            let points: Vec<Px> = (0..BALL_SIDES)
                .map(|i| {
                    (
                        x + (i as f32 * step).cos() * r,
                        y + (i as f32 * step).sin() * r,
                    )
                })
                .collect();
            painter.poly(&points, color);
        }
        Travel::None => {}
    }
}

/// A short burst of radial lines about `c`, hashed off `cell` and `salt`.
fn radial_lines(
    painter: &Painter,
    c: Px,
    cell: (i32, i32),
    salt: u32,
    count: u32,
    reach_px: f32,
    color: Color,
) {
    for i in 0..count {
        let angle = spark_scatter(cell, i ^ salt) * std::f32::consts::TAU;
        let len = reach_px * (0.5 + 0.5 * spark_scatter(cell, i ^ salt ^ CRACKLE_SALT));
        painter.line(
            c.0,
            c.1,
            c.0 + angle.cos() * len,
            c.1 + angle.sin() * len,
            IMPACT_THICKNESS_PX,
            color,
        );
    }
}

/// The brief flash on the attacker's cell, `age` seconds after the swing.
pub(crate) fn draw_muzzle(painter: &Painter, cell: (i32, i32), c: Px, tile_px: f32, age: f64) {
    let u = (age / MUZZLE_SECONDS) as f32;
    if (0.0..1.0).contains(&u) {
        radial_lines(
            painter,
            c,
            cell,
            0,
            MUZZLE_LINES,
            MUZZLE_REACH_TILES * tile_px,
            faded(FLASH_WHITE, 1.0 - u),
        );
    }
}

/// One impact on one cell, `age` seconds after arrival.
pub(crate) fn draw_impact(
    painter: &Painter,
    impact: Impact,
    cell: (i32, i32),
    c: Px,
    tile_px: f32,
    age: f64,
    color: Color,
) {
    let u = (age / impact_seconds(impact).max(f64::EPSILON)) as f32;
    if !(0.0..1.0).contains(&u) {
        return;
    }
    match impact {
        // The `Hit` cue's burst, started by the engine's tactical cue.
        Impact::Sparks => {}
        Impact::Explosion { radius } => {
            let r = explosion_radius(u, radius) * tile_px;
            let color = faded(EXPLOSION_COLOR, 1.0 - u);
            for i in 0..RING_SEGMENTS {
                let step = std::f32::consts::TAU / RING_SEGMENTS as f32;
                let (p, q) = (i as f32 * step, (i + 1) as f32 * step);
                painter.line(
                    c.0 + p.cos() * r,
                    c.1 + p.sin() * r,
                    c.0 + q.cos() * r,
                    c.1 + q.sin() * r,
                    BOLT_THICKNESS_PX,
                    color,
                );
            }
        }
        Impact::Zap => radial_lines(
            painter,
            c,
            cell,
            CRACKLE_SALT,
            CRACKLE_LINES,
            CRACKLE_REACH_TILES * tile_px,
            faded(FLASH_WHITE, 1.0 - u),
        ),
        Impact::Slash => {
            let half = tile_px * 0.4;
            let head = lerp(
                (c.0 - half, c.1 - half),
                (c.0 + half, c.1 + half),
                (u * 2.0).min(1.0),
            );
            painter.line(
                c.0 - half,
                c.1 - half,
                head.0,
                head.1,
                BOLT_THICKNESS_PX,
                faded(FLASH_WHITE, 1.0 - u),
            );
        }
        Impact::Flash => painter.rect(
            c.0 - tile_px / 2.0,
            c.1 - tile_px / 2.0,
            tile_px,
            tile_px,
            faded(color, flash_alpha(u)),
        ),
        Impact::Smoke => {
            for i in 0..SMOKE_PUFFS {
                let drift = (spark_scatter(cell, i ^ CRACKLE_SALT) - 0.5) * 2.0 * SMOKE_DRIFT_TILES;
                let size = tile_px * (0.2 + 0.2 * u);
                let x = c.0 + drift * tile_px * u - size / 2.0;
                let y = c.1
                    - SMOKE_RISE_TILES * tile_px * u * (0.5 + 0.5 * spark_scatter(cell, i))
                    - size / 2.0;
                painter.rect(
                    x,
                    y,
                    size,
                    size,
                    Color::new(SMOKE_GREY, SMOKE_GREY, SMOKE_GREY, 0.5 * (1.0 - u)),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pulse_finishes_inside_the_flight() {
        for count in 1..=8u8 {
            for index in 0..count {
                // Seen in flight at some t, reaching the target, and gone by 1.
                let reached = (0..1000).any(|s| {
                    pulse_span(index, count, s as f32 / 1000.0).is_some_and(|(_, h)| h > 0.99)
                });
                assert!(reached, "pulse {index}/{count} never arrives");
                assert!(pulse_span(index, count, 1.0).is_none());
            }
        }
    }

    #[test]
    fn a_beam_is_fully_grown_by_the_time_its_hold_begins() {
        assert_eq!(beam_head(0.5, 0.5), 1.0);
        assert_eq!(beam_head(0.25, 0.5), 0.5);
        assert_eq!(beam_head(0.0, 0.5), 0.0);
        assert_eq!(beam_head(1.0, 0.5), 1.0);
        // No hold: grows over the whole flight.
        assert_eq!(beam_head(0.5, 0.0), 0.5);
        // An over-long hold is clamped, so the head still grows.
        assert!(beam_head(0.5, 5.0) > 0.0);
        assert_eq!(beam_head(1.0, 5.0), 1.0);
    }

    #[test]
    fn an_explosion_ring_never_shrinks_and_ends_at_its_radius() {
        let mut last = 0.0;
        for s in 0..=100 {
            let r = explosion_radius(s as f32 / 100.0, 1.5);
            assert!(r >= last, "ring shrank at step {s}");
            last = r;
        }
        assert!((last - 1.5).abs() < 1e-6);
    }

    #[test]
    fn a_zap_kink_is_stable_for_a_cell_and_index() {
        for index in 0..ZAP_SEGMENTS {
            let first = zap_offset((3, -2), index);
            assert_eq!(first, zap_offset((3, -2), index));
            assert!((-1.0..=1.0).contains(&first));
        }
        assert_ne!(zap_offset((3, -2), 1), zap_offset((3, -2), 2));
    }

    #[test]
    fn a_cue_is_kept_until_its_longest_impact_ends() {
        let def = |travel, impact: Vec<Impact>| EffectDef {
            id: "t".into(),
            color: None,
            muzzle: false,
            travel,
            impact,
            shake: 0.0,
        };
        let plain = def(Travel::Streak, vec![Impact::Sparks]);
        assert_eq!(blow_seconds(&plain), BOLT_SECONDS);
        let boom = def(
            Travel::Streak,
            vec![Impact::Explosion { radius: 1.0 }, Impact::Smoke],
        );
        assert_eq!(blow_seconds(&boom), BOLT_SECONDS + SMOKE_SECONDS);
        assert_eq!(
            blow_seconds(&def(Travel::None, vec![Impact::Slash])),
            SLASH_SECONDS
        );
    }

    #[test]
    fn a_ball_arrives_inside_the_flight_and_never_backs_up() {
        let mut last = 0.0;
        for s in 0..=1000 {
            let along = ball_along(s as f32 / 1000.0);
            assert!(along >= last, "ball backed up at step {s}");
            last = along;
        }
        let arrived = (0..1000).find(|s| ball_along(*s as f32 / 1000.0) >= 1.0);
        assert!(arrived.is_some_and(|s| s < 1000), "ball never lands early");
        assert_eq!(ball_along(0.0), 0.0);
    }

    #[test]
    fn a_flash_only_fades_and_is_gone_by_its_lifetime() {
        let mut last = f32::MAX;
        for s in 0..=100 {
            let a = flash_alpha(s as f32 / 100.0);
            assert!(a <= last, "flash brightened at step {s}");
            last = a;
        }
        assert_eq!(flash_alpha(1.0), 0.0);
        assert!(flash_alpha(0.0) <= FLASH_PEAK_ALPHA);
        assert!(flash_alpha(0.0) > 0.0);
    }

    #[test]
    fn a_flash_extends_a_cue_by_its_lifetime() {
        let def = EffectDef {
            id: "t".into(),
            color: None,
            muzzle: false,
            travel: Travel::Ball,
            impact: vec![Impact::Flash],
            shake: 0.0,
        };
        assert_eq!(blow_seconds(&def), BOLT_SECONDS + FLASH_SECONDS);
    }
}
