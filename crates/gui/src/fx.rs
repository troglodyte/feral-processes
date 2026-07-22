//! Transient visual feedback for the macroquad frontend: raid flashes on
//! the map, hit feedback in battle, damaged-structure tinting, and the log
//! pane's raid flash.
//!
//! `App` knows nothing about any of this — like the volume control, it's
//! GUI-local state owned by `game_loop` and threaded into `render::draw`.
//! The one thing it does consume from the engine is `Game::take_effects`,
//! which reports raid outcomes a renderer can't otherwise observe (a raid
//! the shield network absorbs changes no state at all).
//!
//! The timing math lives in free functions so it can be unit-tested;
//! everything that touches macroquad's draw calls can't be.

use macroquad::prelude::*;

use crate::text::{Fonts, Metrics};
use feral_processes_engine::{EffectKind, MessageKind, VisualEffect};

/// Alpha a tile flash starts at, before fading linearly to nothing. Chosen
/// to read against the dim tile backgrounds without hiding the glyph.
pub const PEAK_FLASH_ALPHA: f32 = 0.55;
pub const HIT_FLASH_SECONDS: f64 = 0.25;
/// Longer than a hit — a structure vanishing from the map is worth a beat.
pub const DESTROYED_FLASH_SECONDS: f64 = 0.40;

/// How far a damaged structure's glyph is allowed to dim toward grey. Below
/// this it stops reading as a structure at all.
const MIN_TINT: f32 = 0.45;
/// Durability fraction under which a structure's tile picks up a red wash.
const CRITICAL_DURABILITY_FRACTION: f32 = 0.34;

const SHIELD_PULSE_MIN: f32 = 0.06;
const SHIELD_PULSE_MAX: f32 = 0.16;
const SHIELD_PULSE_HZ: f64 = 0.5;

/// How fast the lagging "ghost" bar drains, in HP per second.
const GHOST_DRAIN_PER_SECOND: f32 = 60.0;

const FLOAT_SECONDS: f64 = 0.6;
const FLOAT_RISE_PX: f32 = 24.0;

const LOG_FLASH_SECONDS: f64 = 0.35;

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

fn shield_pulse_alpha(time: f64) -> f32 {
    let mid = (SHIELD_PULSE_MIN + SHIELD_PULSE_MAX) / 2.0;
    let half = (SHIELD_PULSE_MAX - SHIELD_PULSE_MIN) / 2.0;
    mid + half * (time * SHIELD_PULSE_HZ * std::f64::consts::TAU).sin() as f32
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

/// What `Fx::battle_frame` hands back for one frame of the battle screen.
pub struct BattleFx {
    pub wild_ghost: f32,
    pub player_ghost: f32,
    /// HP lost since the previous frame, for spawning a floating number.
    pub wild_damage: i32,
    pub player_damage: i32,
}

#[derive(Default)]
struct BattleTracking {
    wild_hp: Option<i32>,
    player_hp: Option<i32>,
    wild_ghost: f32,
    player_ghost: f32,
}

pub struct Fx {
    pub enabled: bool,
    now: f64,
    flashes: Vec<TileFlash>,
    floats: Vec<FloatingNumber>,
    battle: BattleTracking,
    log_flash_until: f64,
    last_log_line: Option<(MessageKind, String)>,
}

impl Fx {
    pub fn new() -> Self {
        Self {
            enabled: true,
            now: 0.0,
            flashes: Vec::new(),
            floats: Vec::new(),
            battle: BattleTracking::default(),
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
            self.battle = BattleTracking::default();
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

    pub fn battle_frame(&mut self, wild_hp: i32, player_hp: i32, dt: f32) -> BattleFx {
        let wild_damage = self.battle.wild_hp.map_or(0, |prev| prev - wild_hp).max(0);
        let player_damage = self
            .battle
            .player_hp
            .map_or(0, |prev| prev - player_hp)
            .max(0);
        // A first frame with no prior reading seeds the ghosts at the real
        // values, so entering a battle doesn't animate a drain from zero.
        if self.battle.wild_hp.is_none() {
            self.battle.wild_ghost = wild_hp as f32;
            self.battle.player_ghost = player_hp as f32;
        }
        self.battle.wild_hp = Some(wild_hp);
        self.battle.player_hp = Some(player_hp);
        if self.enabled {
            self.battle.wild_ghost = ghost_step(self.battle.wild_ghost, wild_hp as f32, dt);
            self.battle.player_ghost = ghost_step(self.battle.player_ghost, player_hp as f32, dt);
        } else {
            self.battle.wild_ghost = wild_hp as f32;
            self.battle.player_ghost = player_hp as f32;
        }
        BattleFx {
            wild_ghost: self.battle.wild_ghost,
            player_ghost: self.battle.player_ghost,
            wild_damage,
            player_damage,
        }
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

    pub fn draw_floats(&self, fonts: &Fonts, m: &Metrics) {
        for f in &self.floats {
            let t = ((self.now - f.start) / FLOAT_SECONDS) as f32;
            let color = Color::new(f.color.r, f.color.g, f.color.b, 1.0 - t);
            fonts.ui(&f.text, f.x, f.y - FLOAT_RISE_PX * t, m.label(), color);
        }
    }

    /// Watches for a newly logged raid line and starts the log pane's
    /// flash. Compares the last line rather than counting lines, since
    /// `message_log` only ever returns a window of recent ones.
    pub fn observe_log(&mut self, last_line: Option<&(MessageKind, String)>) {
        let changed = last_line != self.last_log_line.as_ref();
        if changed
            && self.enabled
            && let Some((MessageKind::Raid, _)) = last_line
        {
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
    fn shield_pulse_alpha_stays_inside_its_band() {
        for i in 0..200 {
            let a = shield_pulse_alpha(i as f64 * 0.05);
            assert!(
                (SHIELD_PULSE_MIN..=SHIELD_PULSE_MAX).contains(&a),
                "alpha {a} escaped the band"
            );
        }
    }
}
