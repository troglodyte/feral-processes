//! Transient visual feedback: raid flashes on the map, hit feedback in
//! battle, damaged-structure tinting, and the log pane's raid flash.
//!
//! `App` knows nothing about any of this — like the volume control, it's
//! GUI-local state owned by the game loop and threaded into `render::draw`.
//! The one thing it does consume from the engine is `Game::take_effects`,
//! which reports raid outcomes a renderer can't otherwise observe (a raid
//! the shield network absorbs changes no state at all).
//!
//! The timing math lives in free functions so it can be unit-tested;
//! everything that reaches for `Painter` can't be.

use std::collections::HashMap;

use crate::paint::{Color, Painter};
use crate::text::Metrics;
use feral_processes_engine::{EffectKind, Entity, LogLine, MessageKind, VisualEffect};

/// Alpha a tile flash starts at, before fading linearly to nothing. Chosen
/// to read against the dim tile backgrounds without hiding the glyph.
pub const PEAK_FLASH_ALPHA: f32 = 0.55;
pub const HIT_FLASH_SECONDS: f64 = 0.30;
/// Longer than a hit — a structure vanishing from the map is worth a beat.
///
/// This is the spark burst's lifetime too, since the debris is derived from
/// the flash record rather than kept separately, so it is also how long the
/// wreckage has to cover `DESTROYED_SPARK_REACH`. Shortening it does not
/// make the burst snappier, it makes it faster and harder to see.
pub const DESTROYED_FLASH_SECONDS: f64 = 0.70;

/// How far a damaged structure's glyph is allowed to dim toward grey. Below
/// this it stops reading as a structure at all.
const MIN_TINT: f32 = 0.45;
/// Durability fraction under which a structure's tile picks up a red wash.
const CRITICAL_DURABILITY_FRACTION: f32 = 0.34;

const SHIELD_PULSE_MIN: f32 = 0.06;
const SHIELD_PULSE_MAX: f32 = 0.16;
const SHIELD_PULSE_HZ: f64 = 0.5;

/// How far the staffed mark lifts, how fast, and how far out of step two
/// marks are (in turns per step of the phase key). The phase offset is the
/// difference between a base that looks busy and one that looks like a
/// single blinking screen artifact.
const STAFFED_BOB_PX: f32 = 4.0;
const STAFFED_BOB_HZ: f64 = 1.0;
const STAFFED_BOB_PHASE_STEP: f64 = 0.15;
/// How many distinct phases marks are spread across before repeating.
const PHASE_KEYS: u64 = 64;

/// How slowly a stranded machine's mark blinks, and how far down it dims.
///
/// The floor is deliberately not zero. A mark that vanished outright would,
/// for half of every cycle, say "nobody is posted here" — the exact reading
/// the mark was introduced to stop grey `Idle` giving. Dimming to a fifth
/// reads as a warning light while still answering "someone is on this job"
/// at every instant.
const STRANDED_BLINK_HZ: f64 = 0.5;
const STRANDED_BLINK_MIN: f32 = 0.2;

/// How fast the lagging "ghost" bar drains, in HP per second.
const GHOST_DRAIN_PER_SECOND: f32 = 60.0;

/// How fast the camera closes the gap to the player, as an exponential decay
/// rate per second. Higher is snappier; this is the knob to turn if movement
/// feels sluggish or too abrupt.
const CAMERA_DECAY: f32 = 12.0;
/// How far behind the player the camera may fall, in tiles. `draw_playing_base`
/// draws exactly one extra ring of tiles to cover the offset, so a lag past one
/// tile would expose blank space at the trailing edge. Clamping here also means
/// a teleport — breaching a zone, taking a portal — arrives as a snap rather
/// than a long pan across unrelated terrain, with no separate threshold for it.
const CAMERA_MAX_LAG: f32 = 1.0;

const FLOAT_SECONDS: f64 = 0.6;
const FLOAT_RISE_PX: f32 = 24.0;

const LOG_FLASH_SECONDS: f64 = 0.35;

// --- Raid spark burst -------------------------------------------------
//
// The knobs for the debris a raided structure throws. A burst borrows its
// colour and its lifetime from the tile flash underneath it
// (`effect_color` / `effect_duration`), so it cannot outlive or clash with
// the flash it belongs to — only how *much* is thrown and how far is set
// here. `Deflected` throws nothing: the shield network absorbing a sweep is
// not something coming apart.

/// A structure coming down: the loud one, and the only burst that reaches
/// past its own tile.
const DESTROYED_SPARKS: u32 = 20;
const DESTROYED_SPARK_REACH: f32 = 3.2;
/// A structure merely taking a hit. Kept inside its own tile and thin
/// enough that a sweep chewing through a large base doesn't fill the pane.
///
/// The reach is what enforces "inside its own tile", so it stays under 1.0
/// however loud the rest of this gets — a sweep lands one of these on every
/// structure it damages, and neighbours' debris overlapping would turn a
/// raid on a large base into a solid sheet.
/// With the reach pinned under a tile, the count and the lifetime are where
/// a hit's drama has to come from — fourteen sparks inside one tile read as
/// a structure spitting debris where five read as a smudge.
const HIT_SPARKS: u32 = 14;
const HIT_SPARK_REACH: f32 = 0.95;
/// How long a spark's streak is drawn at full speed, in tiles, and how
/// thick. The drawn length is this scaled by `spark_streak`.
///
/// A tile is `BASE_TILE_PX` (20px) at zoom 1, so these are small absolute
/// numbers — 0.7 of a tile is a 14px streak. The first pass at this was a
/// quarter the size and could not be seen at all against the tile wash
/// underneath it, which is the trap: reach and streak read as generous
/// fractions while being a handful of pixels.
const SPARK_STREAK_TILES: f32 = 0.7;
const SPARK_THICKNESS: f32 = 3.0;
/// How far a spark may swing off its evenly-spaced spoke, in radians.
///
/// Bounded at under a full spoke's width on purpose: sparks are spread
/// evenly and then jittered, rather than aimed at random angles, so a burst
/// can never happen to throw everything one way and read as a leak instead
/// of an explosion.
///
/// This is coupled to `DESTROYED_SPARKS` and falls as that rises: twenty
/// spokes are 0.31 radians apart, so the swing has to fit inside that. The
/// raggedness a bigger burst loses here it more than gets back from the
/// speed spread below, which a dense burst shows off far better than a
/// sparse one.
const SPARK_ANGLE_JITTER: f32 = 0.28;
/// Slowest a jittered spark may fly, as a fraction of the full reach. Under
/// 1.0 so the debris arrives ragged rather than as an expanding ring, and
/// well under it so the burst spans a real range of distances — this is
/// what stops twenty evenly-spread sparks reading as a starburst decal.
const SPARK_MIN_SPEED: f32 = 0.35;
/// Keeps a spark's speed draw independent of its angle draw.
const SPARK_SPEED_SALT: u32 = 0x5BF0_3635;

// Local copies of the palette `render.rs` draws with, rather than
// macroquad's harsher primaries.
const FLASH_RED: Color = Color::new(0.9, 0.25, 0.25, 1.0);
const FLASH_CYAN: Color = Color::new(0.25, 0.85, 0.85, 1.0);
const FLASH_WHITE: Color = Color::new(0.95, 0.95, 0.95, 1.0);

fn flash_alpha(elapsed: f64, duration: f64) -> f32 {
    if elapsed >= duration || duration <= 0.0 {
        return 0.0;
    }
    PEAK_FLASH_ALPHA * (1.0 - (elapsed / duration) as f32)
}

/// Brightness multiplier for a structure's glyph given its durability —
/// 1.0 at full health, easing down to `MIN_TINT` as it's worn away.
fn damaged_tint(hp: u32, max_hp: u32) -> f32 {
    if max_hp == 0 {
        return 1.0;
    }
    let fraction = (hp as f32 / max_hp as f32).clamp(0.0, 1.0);
    MIN_TINT + (1.0 - MIN_TINT) * fraction
}

/// Moves a lagging bar value one frame closer to the real one. Drains at a
/// fixed rate so a big hit reads as motion; refills snap, since a heal
/// doesn't need the same emphasis.
fn ghost_step(ghost: f32, current: f32, dt: f32) -> f32 {
    if ghost <= current {
        return current;
    }
    (ghost - GHOST_DRAIN_PER_SECOND * dt).max(current)
}

/// Moves the camera one frame closer to the player along one axis.
///
/// Exponential rather than `ghost_step`'s fixed rate, so the camera
/// decelerates into place instead of arriving at full speed, and expressed as
/// a decay over `dt` so the glide lasts the same wall-clock time whatever the
/// framerate.
fn camera_step(cam: f32, target: f32, dt: f32) -> f32 {
    let eased = cam + (target - cam) * (1.0 - (-CAMERA_DECAY * dt).exp());
    eased.clamp(target - CAMERA_MAX_LAG, target + CAMERA_MAX_LAG)
}

fn shield_pulse_alpha(time: f64) -> f32 {
    let mid = (SHIELD_PULSE_MIN + SHIELD_PULSE_MAX) / 2.0;
    let half = (SHIELD_PULSE_MAX - SHIELD_PULSE_MIN) / 2.0;
    mid + half * (time * SHIELD_PULSE_HZ * std::f64::consts::TAU).sin() as f32
}

/// How far the staffed mark sits above its rest position this frame.
///
/// Upward only, which is why this is a raised cosine rather than the sine
/// `shield_pulse_alpha` uses: the mark's rest position is held off the
/// tile's bottom edge by an inset `render/base.rs` documents as
/// load-bearing, and a down-swing would spend it.
fn staffed_bob_offset(time: f64, phase_key: u64) -> f32 {
    // Wrapped rather than used whole: an entity id climbs into the millions
    // over a run, and a phase that large is both a needlessly big float and
    // no more out of step than a small one.
    let turns = time * STAFFED_BOB_HZ + (phase_key % PHASE_KEYS) as f64 * STAFFED_BOB_PHASE_STEP;
    STAFFED_BOB_PX * (1.0 - (turns * std::f64::consts::TAU).cos()) as f32 / 2.0
}

/// Alpha for a stranded machine's mark: a slow square wave rather than the
/// sine `shield_pulse_alpha` uses, because this is an alarm and has to read
/// as switching rather than as breathing.
fn stranded_blink_alpha(time: f64) -> f32 {
    if (time * STRANDED_BLINK_HZ).fract() < 0.5 {
        1.0
    } else {
        STRANDED_BLINK_MIN
    }
}

/// How many sparks a burst throws and how far they reach, in tiles.
fn spark_burst(kind: EffectKind) -> (u32, f32) {
    match kind {
        EffectKind::Hit => (HIT_SPARKS, HIT_SPARK_REACH),
        EffectKind::Destroyed => (DESTROYED_SPARKS, DESTROYED_SPARK_REACH),
        EffectKind::Deflected => (0, 0.0),
    }
}

/// Fraction of its reach a spark has covered at `t`, where `t` runs 0..1
/// across the burst's lifetime. Decelerating, so debris reads as thrown
/// rather than as an expanding ring.
fn spark_spread(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

/// A stable pseudo-random draw in 0..1 for one spark of one burst.
///
/// Deliberately a function of the tile and the spark index and *nothing
/// else* — there is no time argument, which is what stops a spark being
/// re-rolled every frame and strobing instead of flying.
fn spark_scatter(pos: (i32, i32), index: u32) -> f32 {
    let mut h = (pos.0 as u32).wrapping_mul(0x9E37_79B9)
        ^ (pos.1 as u32).wrapping_mul(0x85EB_CA6B)
        ^ index.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// The direction spark `index` of `count` flies, in radians: an evenly
/// spaced spoke, jittered off it by less than half the spacing.
fn spark_angle(pos: (i32, i32), index: u32, count: u32) -> f32 {
    let spoke = index as f32 / count as f32 * std::f32::consts::TAU;
    spoke + (spark_scatter(pos, index) - 0.5) * SPARK_ANGLE_JITTER
}

/// How fast spark `index` flies, as a fraction of its burst's full reach.
///
/// Salted off the draw `spark_angle` makes, or the two would correlate and
/// every spark jittered clockwise would also be the fast one — a burst with
/// a visible twist to it rather than a scatter.
fn spark_speed(pos: (i32, i32), index: u32) -> f32 {
    SPARK_MIN_SPEED + (1.0 - SPARK_MIN_SPEED) * spark_scatter(pos, index ^ SPARK_SPEED_SALT)
}

/// How long spark `index`'s trail is drawn at `t`, in tiles.
///
/// Proportional to how fast the spark is actually moving, which is both its
/// own speed draw and the deceleration `spark_spread` applies to the whole
/// burst. A fixed length instead reads as a ring of dashes sliding outward;
/// this opens with long streaks and settles into short ones.
fn spark_streak(pos: (i32, i32), index: u32, t: f32) -> f32 {
    SPARK_STREAK_TILES * spark_speed(pos, index) * (1.0 - t).clamp(0.0, 1.0)
}

fn spark_alpha(t: f32) -> f32 {
    (1.0 - t).clamp(0.0, 1.0)
}

fn effect_duration(kind: EffectKind) -> f64 {
    match kind {
        EffectKind::Hit | EffectKind::Deflected => HIT_FLASH_SECONDS,
        EffectKind::Destroyed => DESTROYED_FLASH_SECONDS,
    }
}

fn effect_color(kind: EffectKind) -> Color {
    match kind {
        EffectKind::Hit => FLASH_RED,
        EffectKind::Deflected => FLASH_CYAN,
        EffectKind::Destroyed => FLASH_WHITE,
    }
}

struct TileFlash {
    pos: (i32, i32),
    kind: EffectKind,
    start: f64,
}

struct FloatingNumber {
    text: String,
    x: f32,
    y: f32,
    color: Color,
    start: f64,
}

/// What `Fx::bar_ghost` hands back for one frame of one HP bar.
pub struct BarFx {
    /// The trailing value the ghost band is drawn at, easing toward the
    /// real one.
    pub ghost: f32,
    /// Value lost since the previous frame, for spawning a floating number.
    pub damage: i32,
}

#[derive(Clone, Copy)]
struct BarTracking {
    value: i32,
    ghost: f32,
}

pub struct Fx {
    pub enabled: bool,
    now: f64,
    flashes: Vec<TileFlash>,
    floats: Vec<FloatingNumber>,
    bars: HashMap<u64, BarTracking>,
    camera: Option<(f32, f32)>,
    log_flash_until: f64,
    last_log_line: Option<LogLine>,
}

impl Fx {
    pub fn new() -> Self {
        Self {
            enabled: true,
            now: 0.0,
            flashes: Vec::new(),
            floats: Vec::new(),
            bars: HashMap::new(),
            camera: None,
            log_flash_until: 0.0,
            last_log_line: None,
        }
    }

    /// Called once per frame before drawing: stamps the frame's time,
    /// takes in newly queued engine effects, and retires expired ones.
    /// `effects` is always consumed, even when disabled, so the engine's
    /// queue can't sit permanently at its cap.
    pub fn begin_frame(&mut self, now: f64, effects: Vec<VisualEffect>, in_battle: bool) {
        self.now = now;
        if self.enabled {
            for e in effects {
                self.flashes.push(TileFlash {
                    pos: e.pos,
                    kind: e.kind,
                    start: now,
                });
            }
        }
        self.flashes
            .retain(|f| now - f.start < effect_duration(f.kind));
        self.floats.retain(|f| now - f.start < FLOAT_SECONDS);
        if !in_battle {
            self.clear_bars();
        }
    }

    /// The tint to overlay on the tile at `pos`, if a flash is active there.
    /// Overlapping flashes take the newest, so a structure destroyed by the
    /// blow that damaged it shows the destruction rather than both at once.
    pub fn tile_flash(&self, pos: (i32, i32)) -> Option<Color> {
        let flash = self
            .flashes
            .iter()
            .filter(|f| f.pos == pos)
            .max_by(|a, b| a.start.total_cmp(&b.start))?;
        let alpha = flash_alpha(self.now - flash.start, effect_duration(flash.kind));
        if alpha <= 0.0 {
            return None;
        }
        let c = effect_color(flash.kind);
        Some(Color::new(c.r, c.g, c.b, alpha))
    }

    /// Draws the debris every live burst is currently throwing.
    ///
    /// The sparks are derived from the same `TileFlash` records the tile
    /// wash is drawn from rather than stored beside them, so a burst shares
    /// its colour, its lifetime and its retirement with the flash under it
    /// and the two cannot drift apart.
    ///
    /// Overlapping bursts all draw, unlike `tile_flash`, which takes only
    /// the newest. A structure destroyed by the blow that damaged it should
    /// throw both the hit's spatter and the destruction's wreckage — two
    /// washes on one tile would merely muddy each other, but two lots of
    /// debris just read as more debris.
    ///
    /// `to_px` maps a world tile to its top-left corner in the pane, since
    /// sparks cross tile boundaries and so cannot be drawn from inside the
    /// caller's tile loop.
    pub fn draw_bursts(
        &self,
        painter: &Painter,
        tile_px: f32,
        to_px: impl Fn((i32, i32)) -> (f32, f32),
    ) {
        for flash in &self.flashes {
            let (count, reach) = spark_burst(flash.kind);
            if count == 0 {
                continue;
            }
            let t = ((self.now - flash.start) / effect_duration(flash.kind)) as f32;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let base = effect_color(flash.kind);
            let color = Color::new(base.r, base.g, base.b, spark_alpha(t));
            let (ox, oy) = to_px(flash.pos);
            let (cx, cy) = (ox + tile_px / 2.0, oy + tile_px / 2.0);
            let spread = spark_spread(t);
            for index in 0..count {
                let angle = spark_angle(flash.pos, index, count);
                let (dx, dy) = (angle.cos(), angle.sin());
                let head = spread * spark_speed(flash.pos, index) * reach * tile_px;
                // The streak is the spark's own trail, so it is measured
                // back from the head rather than drawn ahead of it — a
                // streak that led would poke out of the tile before the
                // spark had travelled anywhere.
                let tail = (head - spark_streak(flash.pos, index, t) * tile_px).max(0.0);
                painter.line(
                    cx + dx * tail,
                    cy + dy * tail,
                    cx + dx * head,
                    cy + dy * head,
                    SPARK_THICKNESS,
                    color,
                );
            }
        }
    }

    /// Dims a structure's glyph by how worn down it is, and reports whether
    /// it's critical enough to warrant a red tile wash.
    pub fn structure_condition(
        &self,
        durability: Option<(u32, u32)>,
        color: Color,
    ) -> (Color, bool) {
        let Some((hp, max_hp)) = durability.filter(|_| self.enabled) else {
            return (color, false);
        };
        let tint = damaged_tint(hp, max_hp);
        let critical =
            max_hp > 0 && (hp as f32 / max_hp as f32) < CRITICAL_DURABILITY_FRACTION && hp > 0;
        (
            Color::new(color.r * tint, color.g * tint, color.b * tint, color.a),
            critical,
        )
    }

    /// The shield network's ambient outline color, when one is standing.
    pub fn shield_outline(&self, active: bool) -> Option<Color> {
        if !self.enabled || !active {
            return None;
        }
        Some(Color::new(0.4, 0.8, 1.0, shield_pulse_alpha(self.now)))
    }

    /// How far to lift the "someone is on this job" mark this frame.
    ///
    /// Keyed by the entity wearing the mark, not by the tile it is standing
    /// on. It used to be the tile, on the argument that the phase belongs to
    /// the place — which was true while the only thing wearing a mark was a
    /// building. A posted worker walks, so a tile key would re-phase the bob
    /// on every step and read as a stutter each time the program moved.
    /// Taking `Entity` rather than a pair is what stops a caller handing
    /// back the tile.
    pub fn staffed_bob(&self, entity: Entity) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        staffed_bob_offset(self.now, entity.to_bits())
    }

    /// Alpha for a mark whose machine is full with nowhere to unload.
    ///
    /// Deliberately *not* phase-keyed the way `staffed_bob` is: two stranded
    /// machines are one condition — the base has run out of storage — and
    /// blinking them together says so, where staggering them would read as
    /// two unrelated problems. Working marks are the opposite case, which is
    /// why only they are spread out.
    pub fn stranded_blink(&self) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        stranded_blink_alpha(self.now)
    }

    /// One frame of the trailing "ghost" band behind an HP bar, tracked
    /// per `key` so every roster row — each enemy group and each party slot
    /// — animates independently. A shared tracker would make every bar jump
    /// whenever any one of them changed.
    ///
    /// Callers pick the key; `render` uses the group index for enemies and
    /// `PARTY_BAR_KEY_BASE + slot` for the party, which cannot collide.
    pub fn bar_ghost(&mut self, key: u64, value: i32, dt: f32) -> BarFx {
        // A first frame with no prior reading seeds the ghost at the real
        // value, so entering a battle doesn't animate a drain from zero.
        let previous = self.bars.get(&key).copied();
        let damage = previous.map_or(0, |prev| prev.value - value).max(0);
        let mut ghost = previous.map_or(value as f32, |prev| prev.ghost);
        ghost = if self.enabled {
            ghost_step(ghost, value as f32, dt)
        } else {
            value as f32
        };
        self.bars.insert(key, BarTracking { value, ghost });
        BarFx { ghost, damage }
    }

    /// How far the camera currently sits from the player, in tiles, for the
    /// tile loop to shift the whole grid by. Trails rather than snapping, so
    /// a step reads as the map gliding under a player who leads it.
    ///
    /// The player is drawn as an ordinary entity at its own world position,
    /// so it drifts off centre while the camera catches up without needing a
    /// case of its own.
    pub fn camera_offset(&mut self, player: (i32, i32), dt: f32) -> (f32, f32) {
        let target = (player.0 as f32, player.1 as f32);
        // Effects-off means an instant camera, as it does an instant HP bar.
        // The position is still tracked so re-enabling doesn't glide in from
        // wherever the camera was last left.
        if !self.enabled {
            self.camera = Some(target);
            return (0.0, 0.0);
        }
        // A first frame with no prior reading seeds at the player, so loading
        // a world doesn't pan in from the origin.
        let previous = self.camera.unwrap_or(target);
        let cam = (
            camera_step(previous.0, target.0, dt),
            camera_step(previous.1, target.1, dt),
        );
        self.camera = Some(cam);
        (cam.0 - target.0, cam.1 - target.1)
    }

    /// Drops every bar tracker, so the next battle seeds its ghosts fresh
    /// rather than easing down from the last fight's numbers. Group indices
    /// are reused between battles, which is exactly when stale state would
    /// show.
    pub fn clear_bars(&mut self) {
        self.bars.clear();
    }

    pub fn spawn_float(&mut self, text: String, x: f32, y: f32, color: Color) {
        if !self.enabled {
            return;
        }
        self.floats.push(FloatingNumber {
            text,
            x,
            y,
            color,
            start: self.now,
        });
    }

    pub fn draw_floats(&self, painter: &Painter, m: &Metrics) {
        for f in &self.floats {
            let t = ((self.now - f.start) / FLOAT_SECONDS) as f32;
            let color = Color::new(f.color.r, f.color.g, f.color.b, 1.0 - t);
            painter.ui(&f.text, f.x, f.y - FLOAT_RISE_PX * t, m.label(), color);
        }
    }

    /// Watches for a newly logged raid line and starts the log pane's
    /// flash. Compares the last line rather than counting lines, since
    /// `message_log` only ever returns a window of recent ones.
    pub fn observe_log(&mut self, last_line: Option<&LogLine>) {
        let changed = last_line != self.last_log_line.as_ref();
        if changed && self.enabled && last_line.is_some_and(|l| l.kind == MessageKind::Raid) {
            self.log_flash_until = self.now + LOG_FLASH_SECONDS;
        }
        if changed {
            self.last_log_line = last_line.cloned();
        }
    }

    /// Blends `border` toward red for the tail of a raid flash.
    pub fn log_border(&self, border: Color) -> Color {
        let remaining = self.log_flash_until - self.now;
        if remaining <= 0.0 {
            return border;
        }
        let t = (remaining / LOG_FLASH_SECONDS) as f32;
        Color::new(
            border.r + (FLASH_RED.r - border.r) * t,
            border.g + (FLASH_RED.g - border.g) * t,
            border.b + (FLASH_RED.b - border.b) * t,
            border.a,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ghost trail used to track exactly two HP scalars. A roster has
    /// one bar per enemy group and per party slot, and they must animate
    /// independently — a shared ghost would make every bar jump whenever
    /// any one of them changed.
    #[test]
    fn bar_ghosts_are_tracked_independently_per_key() {
        let mut fx = Fx::new();
        fx.bar_ghost(1, 100, 0.0);
        fx.bar_ghost(2, 100, 0.0);

        let a = fx.bar_ghost(1, 40, 0.016);
        let b = fx.bar_ghost(2, 100, 0.016);

        assert_eq!(a.damage, 60, "key 1 took the hit");
        assert_eq!(b.damage, 0, "key 2 was untouched and must not inherit it");
        assert!(
            b.ghost > a.ghost,
            "the two ghosts must not share state: {} vs {}",
            a.ghost,
            b.ghost
        );
    }

    /// Entering a battle must not animate a drain from zero, and group
    /// indices are reused between fights — so a stale tracker would show
    /// the next battle draining from the last one's numbers.
    #[test]
    fn a_bars_first_frame_seeds_its_ghost_rather_than_draining_from_zero() {
        let mut fx = Fx::new();
        let first = fx.bar_ghost(7, 250, 0.016);
        assert_eq!(first.ghost, 250.0);
        assert_eq!(first.damage, 0);

        fx.bar_ghost(7, 10, 0.016);
        fx.clear_bars();
        let fresh = fx.bar_ghost(7, 250, 0.016);
        assert_eq!(
            fresh.ghost, 250.0,
            "a cleared tracker must seed fresh, not ease down from the old fight"
        );
        assert_eq!(fresh.damage, 0);
    }

    #[test]
    fn flash_alpha_is_at_peak_the_instant_a_flash_starts() {
        assert_eq!(flash_alpha(0.0, HIT_FLASH_SECONDS), PEAK_FLASH_ALPHA);
    }

    #[test]
    fn flash_alpha_is_half_peak_halfway_through() {
        let a = flash_alpha(HIT_FLASH_SECONDS / 2.0, HIT_FLASH_SECONDS);
        assert!((a - PEAK_FLASH_ALPHA / 2.0).abs() < 1e-5, "got {a}");
    }

    #[test]
    fn flash_alpha_is_zero_once_the_duration_has_elapsed() {
        assert_eq!(flash_alpha(HIT_FLASH_SECONDS, HIT_FLASH_SECONDS), 0.0);
        assert_eq!(flash_alpha(99.0, HIT_FLASH_SECONDS), 0.0);
    }

    #[test]
    fn damaged_tint_is_full_brightness_at_full_durability() {
        assert_eq!(damaged_tint(30, 30), 1.0);
    }

    #[test]
    fn damaged_tint_dims_as_durability_drops() {
        assert!(damaged_tint(15, 30) < damaged_tint(29, 30));
    }

    #[test]
    fn damaged_tint_never_dims_past_the_readable_floor() {
        assert_eq!(damaged_tint(0, 30), MIN_TINT);
        assert!(damaged_tint(1, 1000) >= MIN_TINT);
    }

    #[test]
    fn damaged_tint_treats_a_zero_max_as_undamaged() {
        assert_eq!(damaged_tint(0, 0), 1.0);
    }

    #[test]
    fn ghost_step_eases_down_toward_the_current_value() {
        let ghost = ghost_step(100.0, 50.0, 0.1);
        assert!(ghost < 100.0, "the ghost should drain");
        assert!(ghost > 50.0, "the ghost should lag behind, not snap");
    }

    #[test]
    fn ghost_step_never_drains_below_the_current_value() {
        assert_eq!(ghost_step(51.0, 50.0, 10.0), 50.0);
    }

    #[test]
    fn ghost_step_snaps_up_when_the_bar_refills() {
        assert_eq!(ghost_step(20.0, 80.0, 0.016), 80.0);
    }

    #[test]
    fn camera_step_eases_toward_the_target_without_arriving_in_one_frame() {
        let cam = camera_step(0.0, 1.0, 0.016);
        assert!(cam > 0.0, "the camera should move");
        assert!(cam < 1.0, "the camera should lag, not snap");
    }

    /// A per-frame fraction would glide for a different *duration* on a
    /// 144Hz machine than on a 30Hz one. The exponential form must not.
    #[test]
    fn camera_step_covers_the_same_ground_however_the_frames_are_sliced() {
        let coarse = camera_step(0.0, 1.0, 0.1);
        let mut fine = 0.0;
        for _ in 0..10 {
            fine = camera_step(fine, 1.0, 0.01);
        }
        assert!(
            (coarse - fine).abs() < 1e-4,
            "one 0.1s step {coarse} vs ten 0.01s steps {fine}"
        );
    }

    #[test]
    fn camera_step_converges_on_the_target() {
        let mut cam = 0.0;
        for _ in 0..200 {
            cam = camera_step(cam, 5.0, 0.016);
        }
        assert!((cam - 5.0).abs() < 1e-3, "settled at {cam}");
    }

    /// The tile loop draws exactly one extra ring to cover the offset, so a
    /// camera further behind than that would expose blank space at the
    /// trailing edge. Holding a movement key is what would otherwise do it.
    #[test]
    fn camera_step_never_falls_further_behind_than_the_extra_ring_covers() {
        assert_eq!(
            camera_step(0.0, 40.0, 0.016).max(0.0),
            40.0 - CAMERA_MAX_LAG
        );
        assert_eq!(camera_step(0.0, -40.0, 0.016), -40.0 + CAMERA_MAX_LAG);
    }

    #[test]
    fn the_cameras_first_frame_seeds_at_the_player_rather_than_panning_in() {
        let mut fx = Fx::new();
        assert_eq!(fx.camera_offset((120, -80), 0.016), (0.0, 0.0));
    }

    #[test]
    fn the_camera_trails_the_player_after_a_step_and_then_catches_up() {
        let mut fx = Fx::new();
        fx.camera_offset((0, 0), 0.016);

        let (ox, oy) = fx.camera_offset((1, 0), 0.016);
        assert!(ox < 0.0, "the camera should still be behind: {ox}");
        assert_eq!(oy, 0.0, "an axis that didn't move must not drift");

        for _ in 0..200 {
            fx.camera_offset((1, 0), 0.016);
        }
        let (ox, _) = fx.camera_offset((1, 0), 0.016);
        assert!(ox.abs() < 1e-3, "the camera should settle: {ox}");
    }

    /// Effects-off means an instant camera, the same way it means an instant
    /// HP bar — and the next frame must not then glide from a stale reading.
    #[test]
    fn the_camera_snaps_while_effects_are_disabled() {
        let mut fx = Fx::new();
        fx.enabled = false;
        fx.camera_offset((0, 0), 0.016);
        assert_eq!(fx.camera_offset((30, 30), 0.016), (0.0, 0.0));

        fx.enabled = true;
        assert_eq!(
            fx.camera_offset((30, 30), 0.016),
            (0.0, 0.0),
            "re-enabling must not reveal a camera left behind at the origin"
        );
    }

    #[test]
    fn shield_pulse_alpha_stays_inside_its_band() {
        for i in 0..200 {
            let a = shield_pulse_alpha(i as f64 * 0.05);
            assert!(
                (SHIELD_PULSE_MIN..=SHIELD_PULSE_MAX).contains(&a),
                "alpha {a} escaped the band"
            );
        }
    }

    /// The mark bobs *up* out of its rest position and never below it. Its
    /// rest position is held off the tile's bottom edge by an inset that
    /// `render/base.rs` documents as load-bearing, and a negative offset
    /// would spend that inset.
    #[test]
    fn the_staffed_bob_only_ever_lifts_the_mark() {
        for i in 0..400 {
            let dy = staffed_bob_offset(i as f64 * 0.02, i % 7);
            assert!(
                (0.0..=STAFFED_BOB_PX).contains(&dy),
                "offset {dy} escaped the band"
            );
        }
    }

    /// Two marks are deliberately out of step: a base full of them should
    /// read as separate workers, not one synchronised pulse.
    #[test]
    fn two_staffed_marks_bob_out_of_phase() {
        let (a, b) = (staffed_bob_offset(0.3, 4), staffed_bob_offset(0.3, 5));
        assert!((a - b).abs() > 0.1, "marks moved in lockstep: {a} vs {b}");
    }

    /// The phase belongs to the program, not to the ground under it. A
    /// posted worker walks a tile per tick, and a phase that reset on each
    /// step would read as a stutter every time it moved.
    #[test]
    fn a_marks_phase_does_not_depend_on_where_it_is_standing() {
        let mut fx = Fx::new();
        fx.begin_frame(0.3, Vec::new(), false);
        let worker = Entity::from_raw_u32(7).unwrap();
        let before = fx.staffed_bob(worker);
        // Same entity, same frame — the only thing a step changes is the
        // tile, which this no longer reads.
        assert_eq!(fx.staffed_bob(worker), before);
    }

    /// A stranded mark dims but never disappears. Gone for half of every
    /// cycle would say "nobody is posted here" — the reading the mark exists
    /// to prevent — and the machine is in fact still staffed, just stuck.
    #[test]
    fn a_stranded_mark_dims_but_never_goes_out() {
        let mut seen_bright = false;
        let mut seen_dim = false;
        for i in 0..400 {
            let a = stranded_blink_alpha(i as f64 * 0.02);
            assert!(a >= STRANDED_BLINK_MIN, "the mark went out entirely: {a}");
            assert!(a <= 1.0);
            seen_bright |= a == 1.0;
            seen_dim |= a < 1.0;
        }
        assert!(seen_bright && seen_dim, "it never blinked at all");
    }

    /// Slow enough to read as an alarm rather than a flicker: a full cycle
    /// takes seconds, so the mark holds each state long enough to be seen.
    #[test]
    fn the_stranded_blink_holds_each_state_for_about_a_second() {
        let dwell = 0.5 / STRANDED_BLINK_HZ;
        assert!(
            dwell >= 0.75,
            "each half of the cycle lasts {dwell}s, which reads as a flicker"
        );
        assert_eq!(stranded_blink_alpha(0.0), 1.0);
        assert_eq!(stranded_blink_alpha(dwell * 1.5), STRANDED_BLINK_MIN);
    }

    #[test]
    fn a_destroyed_burst_throws_more_sparks_and_further_than_a_hit() {
        let (destroyed, d_reach) = spark_burst(EffectKind::Destroyed);
        let (hit, h_reach) = spark_burst(EffectKind::Hit);
        assert!(destroyed > hit, "{destroyed} vs {hit}");
        assert!(d_reach > h_reach, "{d_reach} vs {h_reach}");
    }

    /// A hit lands on every damaging blow of a sweep, so its debris has to
    /// stay on the tile that took it. Only a structure actually coming down
    /// is allowed to throw wreckage over its neighbours.
    #[test]
    fn only_a_destroyed_burst_reaches_past_its_own_tile() {
        assert!(spark_burst(EffectKind::Hit).1 < 1.0);
        assert!(spark_burst(EffectKind::Destroyed).1 > 1.0);
    }

    /// The shield network absorbing a sweep changes no state at all, which
    /// is why it has a flash — but nothing came apart, so nothing is thrown.
    #[test]
    fn a_deflected_sweep_throws_no_sparks() {
        assert_eq!(spark_burst(EffectKind::Deflected).0, 0);
    }

    #[test]
    fn a_spark_starts_at_the_tile_and_is_at_full_reach_when_it_dies() {
        assert_eq!(spark_spread(0.0), 0.0);
        assert!(
            (spark_spread(1.0) - 1.0).abs() < 1e-6,
            "{}",
            spark_spread(1.0)
        );
    }

    /// Debris flung outward slows as it goes. Flying at a constant rate
    /// reads as an expanding ring rather than as something thrown.
    #[test]
    fn sparks_decelerate_rather_than_flying_at_a_constant_rate() {
        let first_half = spark_spread(0.5) - spark_spread(0.0);
        let second_half = spark_spread(1.0) - spark_spread(0.5);
        assert!(
            first_half > second_half,
            "covered {first_half} then {second_half} — that is constant or accelerating"
        );
    }

    #[test]
    fn a_spark_never_flies_back_toward_the_tile() {
        let mut previous = 0.0;
        for i in 0..=100 {
            let spread = spark_spread(i as f32 / 100.0);
            assert!(spread >= previous, "went backwards at t={i}: {spread}");
            previous = spread;
        }
    }

    /// Sparks are spread evenly and *then* jittered, so a burst can never
    /// happen to throw everything one way and read as a leak rather than an
    /// explosion. This is what that buys.
    #[test]
    fn a_bursts_sparks_are_spread_around_the_tile_rather_than_clustered() {
        // Both throwing kinds, because the jitter is shared and the counts
        // are not: the thinner burst has the wider spacing to stay inside,
        // so raising one count without the other is what would break this.
        for kind in [EffectKind::Hit, EffectKind::Destroyed] {
            let count = spark_burst(kind).0;
            let spacing = std::f32::consts::TAU / count as f32;
            for index in 0..count {
                let angle = spark_angle((3, -7), index, count);
                let spoke = index as f32 / count as f32 * std::f32::consts::TAU;
                assert!(
                    (angle - spoke).abs() <= SPARK_ANGLE_JITTER / 2.0,
                    "{kind:?} spark {index} swung {} off its spoke",
                    angle - spoke
                );
            }
            assert!(
                SPARK_ANGLE_JITTER < spacing,
                "{kind:?} jitter {SPARK_ANGLE_JITTER} is wider than its {spacing} \
                 spacing, so sparks can cross into each other's spokes and cluster"
            );
        }
    }

    /// A sweep hits several structures at once. If the scatter were keyed
    /// only on the spark index, every burst in that raid would be the same
    /// shape and the base would look stamped rather than wrecked.
    #[test]
    fn two_tiles_bursting_at_once_do_not_scatter_identically() {
        let count = DESTROYED_SPARKS;
        let differs =
            (0..count).any(|i| spark_angle((0, 0), i, count) != spark_angle((1, 0), i, count));
        assert!(
            differs,
            "adjacent bursts threw debris in identical directions"
        );
    }

    /// Even spokes fix the *directions*, which alone would put every spark
    /// the same distance out at every instant — an expanding ring wearing a
    /// starburst's clothes. Varying the speeds is what makes the debris
    /// ragged.
    #[test]
    fn sparks_fly_at_a_range_of_speeds_rather_than_as_one_ring() {
        let speeds: Vec<f32> = (0..DESTROYED_SPARKS)
            .map(|i| spark_speed((2, 5), i))
            .collect();
        for s in &speeds {
            assert!(
                (SPARK_MIN_SPEED..=1.0).contains(s),
                "speed {s} escaped the band"
            );
        }
        let slowest = speeds.iter().copied().fold(f32::MAX, f32::min);
        let fastest = speeds.iter().copied().fold(f32::MIN, f32::max);
        assert!(
            fastest - slowest > 0.1,
            "the whole burst flew at one speed: {slowest}..{fastest}"
        );
    }

    /// A trail of fixed length reads as a ring of dashes sliding outward.
    /// Tying it to how fast the spark is actually travelling is what makes a
    /// burst open with long streaks and settle into short ones — `spark_spread`
    /// decelerates, so the two have to move together.
    #[test]
    fn a_sparks_streak_shortens_as_it_slows() {
        let (pos, index) = ((4, 2), 3);
        let opening = spark_streak(pos, index, 0.0);
        let middle = spark_streak(pos, index, 0.5);
        let dying = spark_streak(pos, index, 1.0);
        assert!(opening > middle, "opening {opening}, middle {middle}");
        assert!(middle > dying, "middle {middle}, dying {dying}");
        assert_eq!(dying, 0.0, "a stopped spark still had a trail");
    }

    /// Sampled at one instant, so this is about the sparks differing from
    /// each other rather than about the fade above: the burst's own speed
    /// spread has to reach the trails, or every streak is the same length
    /// and the raggedness `spark_speed` buys is thrown away.
    #[test]
    fn a_faster_spark_drags_a_longer_streak() {
        let pos = (2, 5);
        let (slow, fast) = (0..DESTROYED_SPARKS).fold((0, 0), |(slow, fast), i| {
            let s = spark_speed(pos, i);
            (
                if s < spark_speed(pos, slow) { i } else { slow },
                if s > spark_speed(pos, fast) { i } else { fast },
            )
        });
        assert!(
            spark_streak(pos, fast, 0.3) > spark_streak(pos, slow, 0.3),
            "spark {fast} at speed {} trailed no further than {slow} at {}",
            spark_speed(pos, fast),
            spark_speed(pos, slow)
        );
    }

    #[test]
    fn spark_alpha_is_full_at_the_start_and_out_at_the_end() {
        assert_eq!(spark_alpha(0.0), 1.0);
        assert_eq!(spark_alpha(1.0), 0.0);
        assert!(spark_alpha(0.5) > 0.0 && spark_alpha(0.5) < 1.0);
    }

    #[test]
    fn the_staffed_mark_holds_still_while_effects_are_disabled() {
        let mut fx = Fx::new();
        fx.begin_frame(0.3, Vec::new(), false);
        fx.enabled = false;
        assert_eq!(fx.staffed_bob(Entity::from_raw_u32(4).unwrap()), 0.0);
    }
}
