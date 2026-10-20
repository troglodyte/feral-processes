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

use crate::effect_draw;
use crate::effects::{EffectDef, EffectLibrary, Impact, Travel};
use crate::paint::Rect;
use crate::paint::{Color, Painter};
use crate::render::hud::palette;
use crate::text::Metrics;
use feral_processes_engine::components::GlyphColor;
use feral_processes_engine::{
    BoltCue, EffectKind, Entity, Game, Impact as BattleImpact, ImpactKind, ImpactTarget, LogLine,
    MessageKind, RoutineCue, SpeechCue, TacticalFxCue, TacticalFxKind, TransitCue, VisualEffect,
};

/// Alpha a tile flash starts at, before fading linearly to nothing. Chosen
/// to read against the dim tile backgrounds without hiding the glyph.
pub const PEAK_FLASH_ALPHA: f32 = 0.55;
pub const HIT_FLASH_SECONDS: f64 = 0.30;
/// How long a blow's streak takes to cross from the swinger to what it hit.
///
/// **Derived from the tactical turn rate rather than restated**, so a retune
/// of the pace cannot leave a streak still in flight when the next body
/// acts — which reads as two attacks landing at once. A third of a beat:
/// long enough to follow across three cells, short enough to leave a clear
/// gap before the next turn.
const BOLT_BEAT_FRACTION: f64 = 1.0 / 3.0;
pub const BOLT_SECONDS: f64 =
    (1.0 / feral_processes_app_core::TACTICAL_TURNS_PER_SECOND as f64) * BOLT_BEAT_FRACTION;

/// How long the battle camera stays on a body after it has stopped acting,
/// as a fraction of the hand-over it has to fit inside.
///
/// **Derived rather than restated**, `BOLT_SECONDS`'s rule and for its
/// reason: a fight hands the turn on inside the same call that resolves the
/// blow, so the hold — and the pan after it — have to end before the *next*
/// body acts, and a figure in seconds cannot follow a retune of the pace
/// there.
///
/// **Derived from `TACTICAL_HANDOVER_SECONDS` and not the turn beat**, which
/// is what it used to read: the beat is the wait between a body arriving and
/// striking, and the camera has nothing to do in it. Against a 0.625s beat
/// the hold could only reach 0.344s, of which `HIT_FLASH_SECONDS` spent
/// 0.30 — 44ms of stillness, which is not a pause anybody sees.
///
/// At 0.62 of the hand-over the hold outlasts the flash by a quarter of a
/// second and still leaves the pan room to land, which
/// `the_hold_and_the_pan_both_fit_inside_the_hand_over` checks against the
/// real easing rather than against an estimate of it.
const CAMERA_DWELL_HANDOVER_FRACTION: f64 = 0.62;
pub const CAMERA_DWELL_SECONDS: f64 =
    feral_processes_app_core::TACTICAL_HANDOVER_SECONDS as f64 * CAMERA_DWELL_HANDOVER_FRACTION;
/// How long a battle-map body takes to glide one cell — a walk's own step,
/// so a walk reads as one continuous movement rather than a run of hops.
/// Derived from `TACTICAL_STEPS_PER_SECOND` for `BOLT_SECONDS`' reason. The
/// longest a glide takes, not the only figure: see `Fx::slid_cell`.
pub(crate) const SLIDE_SECONDS: f64 =
    1.0 / feral_processes_app_core::TACTICAL_STEPS_PER_SECOND as f64;
/// How much shorter than a whole glide the gap since a body's last step has
/// to be before the next glide is paced to it instead. A wild walk is paced
/// against frames, so where the refresh rate does not divide a walk step its
/// gaps alternate a frame either side of `SLIDE_SECONDS` — and a glide
/// paced to the short one lands a frame early and stands still for a frame.
/// A held key's repeat is well under this; a frame of jitter never is.
const FAST_STEP_FRACTION: f64 = 0.75;

/// A hold that ends inside the flash it is there to show is the bug this
/// whole mechanism exists to fix, so it is a build failure. That the *pan*
/// also lands in time is measured instead — see
/// `the_hold_and_the_pan_both_fit_inside_the_hand_over` — because its length
/// falls out of `CAMERA_DECAY` and is not a constant to compare against.
const _: () = assert!(
    CAMERA_DWELL_SECONDS > HIT_FLASH_SECONDS,
    "the camera leaves the attacker while the blow is still lit"
);

/// How thick the streak is drawn, and how long its lit head is as a fraction
/// of the whole flight. A head rather than a full line, so the eye reads a
/// direction rather than a static beam.
///
/// `pub(crate)` because `tactical.rs`'s forecast draws in the same
/// thickness — a profiled hostile's projected walk is `BoltCue`'s line
/// vocabulary, not a second-guessed weight.
pub(crate) const BOLT_THICKNESS_PX: f32 = 2.5;
pub(crate) const BOLT_HEAD_FRACTION: f32 = 0.35;

/// Screen shake. Impacts add their effect's `shake` to a stored energy, the
/// energy is capped so a volley cannot throw the board off the screen, and it
/// decays exponentially with this time constant.
const SHAKE_DECAY_SECONDS: f32 = 0.3;
const SHAKE_MAX_ENERGY: f32 = 1.5;
/// Below this the energy is zeroed rather than left to decay forever.
const SHAKE_FLOOR: f32 = 0.01;
/// The widest the board is ever thrown, in tiles, at full (1.0) energy.
const SHAKE_MAX_OFFSET_TILES: f32 = 0.2;
/// How often the wobble picks a new direction.
const SHAKE_WOBBLE_HZ: f64 = 30.0;
const SHAKE_Y_SALT: u32 = 0x68E3_1DA4;

/// The two cell centres a line between them runs through.
///
/// `to_px` answers a cell's **top-left**, so both ends offset by half a
/// tile to meet glyph centre to glyph centre — a line drawn corner to
/// corner would leave the glyph it came from and arrive beside the one it
/// hit. Shared by a landing blow's streak, which draws only the head of
/// this segment, and a forecast's projected walk in `tactical.rs`, which
/// draws it whole: the offset is geometry neither owns, and a copy of it
/// is the one that drifts if the two are ever drawn at a different tile
/// size.
pub(crate) fn cell_centers(
    to_px: impl Fn((i32, i32)) -> (f32, f32),
    tile_px: f32,
    from: (i32, i32),
    to: (i32, i32),
) -> ((f32, f32), (f32, f32)) {
    let half = tile_px / 2.0;
    let (ax, ay) = to_px(from);
    let (bx, by) = to_px(to);
    ((ax + half, ay + half), (bx + half, by + half))
}

/// Longer than a hit — a structure vanishing from the map is worth a beat.
///
/// This is the spark burst's lifetime too, since the debris is derived from
/// the flash record rather than kept separately, so it is also how long the
/// wreckage has to cover `DESTROYED_SPARK_REACH`. Shortening it does not
/// make the burst snappier, it makes it faster and harder to see.
pub const DESTROYED_FLASH_SECONDS: f64 = 0.70;

/// How long a drop trooper's landing washes its cell green — a
/// destruction's span rather than a hit's, since it marks where a new body
/// now stands and a hit-length blink is gone before the eye finds it.
pub const LANDING_FLASH_SECONDS: f64 = DESTROYED_FLASH_SECONDS;

/// How long a battle map's `-N`/`+N` lives — it rises and fades over the
/// same span, since a rise that outlasted its own fade would end on a
/// visible snap rather than dying out with it.
const NUMBER_MARK_SECONDS: f64 = 0.9;
/// How far a battle map's number rises over its life, as a fraction of a
/// tile.
const NUMBER_MARK_RISE: f32 = 0.55;

/// How long one rising mark over a body — the `+` a Repair Bay mends with,
/// the `^` an amenity refills with — takes to float up and fade out, and how
/// far it floats as a share of the tile. The next one starts back at rest,
/// so a long recovery reads as a stream of marks rising off the body rather
/// than one mark bouncing on it.
const RECOVERY_FLOAT_SECONDS: f64 = 1.0;
const RECOVERY_FLOAT_RISE: f32 = 0.5;

/// How long a reaction's `!` lives. Shorter than a heal's mark: it has to
/// be read beside the swing it announces, not after it.
pub const REACTION_MARK_SECONDS: f64 = 0.6;
/// How high a reaction's `!` pops, as a fraction of a tile.
const REACTION_MARK_HEIGHT: f32 = 0.4;
/// The share of its life a reaction's `!` spends rising before it holds.
const REACTION_MARK_RISE: f32 = 0.2;

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
pub(crate) const PHASE_KEYS: u64 = 64;

/// What share of the room it has a subject under study rattles within, how
/// often it re-sites, and how far out of step two subjects are.
///
/// **A share and not a pixel count.** The ink already sits inside a margin —
/// `(tile_px - glyph_px) / 2`, which is 2px at zoom 1 and grows with every
/// step of the ladder — and that margin is exactly the room the rattle has
/// before it reaches the pen's marks on the tile-edge ring. Spending a
/// fraction of it makes the bound structural at every zoom, where a fixed
/// `2.0` was both flush against them at zoom 1 and invisible at zoom 4.
///
/// **Sample-and-hold, and that is the whole of the rattle.** The offset is
/// held flat for a step and then snaps somewhere else, which is what reads as
/// strain rather than as a body swaying. Nothing eases between steps: an ease
/// at this rate is a wobble, and a wobble reads as comfortable.
///
/// `STRAIN_STEP_HZ` is the one number that decides whether this is a rattle
/// or television static. Fast enough and every frame gets its own offset,
/// which is the failure mode `cloud_shade` records; ~14 Hz holds each
/// position for several frames at any rate the game runs at.
const STRAIN_JITTER_SHARE: f32 = 0.75;
const STRAIN_STEP_HZ: f64 = 14.0;
const STRAIN_PHASE_STEP: f64 = 0.37;

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
pub(crate) const CAMERA_MAX_LAG: f32 = 1.0;

/// How long a body takes to cross one cell of base space on its way out
/// through the anchor, and how far apart a squad's bodies set off.
///
/// A walk is drawn in wall-clock seconds and not in ticks, because nothing
/// in the simulation is walking: the engine queued the whole path in one
/// call and the bodies are already away. The stagger is what makes four
/// programs read as a file leaving rather than as one glyph with a smear
/// behind it.
const TRANSIT_SECONDS_PER_CELL: f64 = 0.12;
const TRANSIT_STAGGER_SECONDS: f64 = 0.18;

const FLOAT_SECONDS: f64 = 0.6;
const FLOAT_RISE_PX: f32 = 24.0;

/// How far apart, in x, two impact floats on one row are drawn while the
/// earlier is still alive. A round resolves several blows on a row inside
/// one float's lifetime; at one x they print over each other.
const IMPACT_STAGGER_PX: f32 = 18.0;
/// The `KO` stamp over the picture is the picture's side over this.
pub(crate) const KO_STAMP_DIVISOR: u16 = 3;
/// A crit's float is drawn this much larger than a plain hit's.
const CRIT_FLOAT_SCALE: f32 = 1.5;
/// A row's background tint after a hit, crit or heal fades over this long.
const ROW_FLASH_SECONDS: f64 = 0.3;
const ROW_FLASH_PEAK_ALPHA: f32 = 0.35;
/// The picture window's reaction to a blow: a sideways shake that dies
/// away, with a tint that fades alongside it.
const PORTRAIT_NUDGE_SECONDS: f64 = 0.25;
const PORTRAIT_NUDGE_PX: f32 = 6.0;
const PORTRAIT_NUDGE_CYCLES: f32 = 2.0;
const PORTRAIT_FLASH_PEAK_ALPHA: f32 = 0.5;
/// The `KO` stamp over the picture fades out over this long.
const PORTRAIT_KO_SECONDS: f64 = 1.0;

const IMPACT_HIT_COLOR: Color = Color::new(0.9, 0.25, 0.25, 1.0);
const IMPACT_CRIT_COLOR: Color = Color::new(1.0, 1.0, 1.0, 1.0);
const IMPACT_HEAL_COLOR: Color = Color::new(0.35, 0.85, 0.4, 1.0);
const IMPACT_DIM_COLOR: Color = Color::new(0.6, 0.6, 0.65, 1.0);

const LOG_FLASH_SECONDS: f64 = 0.35;
/// How long a battle map's `ROUND N` banner takes to fade out. It never
/// holds the fight's pacing — the next body acts underneath it — so this
/// is a read-at-a-glance length, not a wait.
const ROUND_BANNER_SECONDS: f64 = 1.2;

// --- Cloud shadows ----------------------------------------------------
//
// Patches of shade drifting over the zone map. Pure atmosphere: they mean
// nothing, cost nothing and are the one thing on this map that moves
// without something having happened. The zone map alone — base space is a
// pocket underground and the Stack draws through `render/stack.rs`.
//
// **They are not the weather.** `environment::StaticEvent` is a real
// mechanical layer that claims a whole *biome* at a time and reads out as a
// word on the map frame's top border. A shadow drifting over a biome under
// a Thread Storm is not that storm and does not mark it. That collision was
// weighed and accepted when the feature was chosen; if the shade is ever
// wanted as the tell for Static, the honest version gates the whole field
// on the event rather than trying to make a drifting patch stand for a
// zone-wide condition.

/// How far into shadow the deepest part of a cloud takes the ground, as a
/// fraction off its lit brightness.
///
/// Bounded under the `SHADE_JITTER` band's neighbour-to-neighbour spread
/// times a few, so a cloud reads as a large soft thing passing over texture
/// rather than as the texture itself changing. Deeper than about a third
/// and the ground under it stops answering which biome it is, which is the
/// one thing the map's brightness is genuinely *for* — so 0.30 is close to
/// the ceiling and the next request for a darker shadow is a request to
/// give that up.
///
/// It costs nothing in legibility because the shadow rides `shade` and
/// never `vig`: glyphs are drawn at full vignette brightness under a cloud,
/// so nothing standing on the ground dims with it. The vignette floors in
/// `render/base.rs` are the constrained pair, not this.
const CLOUD_DEPTH: f32 = 0.30;

/// How fast the field travels, in tiles per second, and in which direction.
///
/// Off-axis, and the two components deliberately not equal: an axis-aligned
/// wind marches the shadow front down a row of tiles, and an exactly
/// diagonal one bands the map along its diagonal — `tile_hash`'s reason for
/// mixing its two axes with different constants, arrived at again here.
///
/// Slow enough that the motion is caught rather than watched: at 0.64 the
/// leading edge crosses a tile about every second and a half and a whole
/// patch takes a little over half a minute to pass over you.
///
/// Doubled from the (0.32, 0.13) it shipped at, which read as still — the
/// `CLOUD_EDGE0`..`CLOUD_EDGE1` band spreads the leading edge over several
/// tiles by design, so at three seconds a tile no single tile visibly
/// changed inside the time a player looks at one. The direction is
/// deliberately untouched; only the rate moved.
const CLOUD_WIND: (f32, f32) = (0.64, 0.26);

/// How large a patch is, in tiles, as the wavelength of the field's slowest
/// component. Around half a screen at zoom 1, so a cloud is a thing the map
/// is partly under rather than a mottling of it.
const CLOUD_SCALE: f32 = 22.0;

/// The band the raw field is mapped through to decide cover. Everything at
/// or below `CLOUD_EDGE0` is full daylight and everything at or above
/// `CLOUD_EDGE1` is fully shaded; between them is the soft edge.
///
/// The gap between them is what makes a shadow *creep* rather than *step*.
/// The field is sampled per world coordinate — it has to be, since anything
/// keyed to screen position crawls across the terrain as the camera slides
/// — so the edge advances a tile at a time. A wide band spreads that edge
/// over several tiles, where a hard threshold would switch each tile from
/// lit to dark in one frame and read as a checkerboard wiping across the
/// ground.
const CLOUD_EDGE0: f32 = 0.52;
const CLOUD_EDGE1: f32 = 0.92;

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
/// A raid's three flashes, from the HUD's palette rather than from three
/// values beside it — the only place outside `render/` that reaches for it.
///
/// `THREAT` is the whole reason: the palette reserves br red for hostility
/// and inbound harm, and a structure taking a hit in a raid is the most
/// literal thing that phrase describes on this screen. Painted in a red of
/// its own, the one cue the reservation exists for would have been the one
/// cue not drawn from it.
const FLASH_RED: Color = palette::THREAT;
const FLASH_CYAN: Color = palette::glyph(GlyphColor::Cyan);
const FLASH_WHITE: Color = palette::EMPHASIS;

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

/// The cell `battle_center` aims the camera at, given a body's own anchor
/// and footprint — the anchor itself at `footprint` 1, so an ordinary body
/// is untouched.
///
/// Integer division rather than a fractional centre: this answers a whole
/// cell, so a footprint whose side is odd lands exactly
/// on its own middle cell, and the shipped 2x2 formation rounds to the
/// block's own far corner rather than the near one `anchor` alone would
/// read as. That is closer to the middle by half a cell either way — there
/// is no better whole-cell answer for an even side — and it is what keeps
/// the camera from reading as biased toward a squad's least-visited corner.
fn footprint_center(anchor: (i32, i32), footprint: u8) -> (i32, i32) {
    let half = i32::from(footprint) / 2;
    (anchor.0 + half, anchor.1 + half)
}

/// Moves the camera one frame closer to the player along one axis.
///
/// Exponential rather than `ghost_step`'s fixed rate, so the camera
/// decelerates into place instead of arriving at full speed, and expressed as
/// a decay over `dt` so the glide lasts the same wall-clock time whatever the
/// framerate.
/// `max_lag` is `None` for a caller with no trailing edge to protect — see
/// `CAMERA_MAX_LAG`, whose argument is the surface map's extra ring of tiles
/// and applies to nothing else.
fn camera_step(cam: f32, target: f32, dt: f32, max_lag: Option<f32>) -> f32 {
    let eased = cam + (target - cam) * (1.0 - (-CAMERA_DECAY * dt).exp());
    match max_lag {
        Some(lag) => eased.clamp(target - lag, target + lag),
        None => eased,
    }
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

/// Where a subject under study sits this instant, as an offset off the middle
/// of its tile — **held flat for a step, then snapped somewhere else**.
///
/// The held value is an incommensurate sine sum sampled at the step boundary,
/// `cloud_shade`'s argument in one dimension: a hash per step would do the
/// same job, but the frequencies here have no common period, so a player who
/// sits and watches a pinned program never sees the rattle come round. The
/// two axes are summed from different frequencies so the body does not travel
/// along a diagonal, which is what a shared signal scaled twice would read as.
///
/// Phase-keyed by the body for `staffed_bob_offset`'s reason, and wrapped the
/// same way: two subjects in two Stations rattle out of step, so the pens read
/// as two programs under strain rather than as one animation drawn twice.
fn strain_offset(time: f64, phase_key: u64) -> (f32, f32) {
    // The step's own instant, not the frame's — everything below is a
    // function of this, so the offset is constant for the whole step.
    let t = (time * STRAIN_STEP_HZ).floor() / STRAIN_STEP_HZ;
    let phase = (phase_key % PHASE_KEYS) as f64 * STRAIN_PHASE_STEP;
    let tau = std::f64::consts::TAU;
    let x = 0.6 * (tau * (t * 1.00 + phase)).sin() + 0.4 * (tau * (t * 2.37 + phase)).sin();
    let y = 0.6 * (tau * (t * 1.43 - phase)).sin() + 0.4 * (tau * (t * 3.11 - phase)).sin();
    // The two amplitudes sum to one, so this is -1..1 on both axes by
    // construction and `strain_jitter`'s margin is the only scale.
    (x as f32, y as f32)
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

/// How much of its lit brightness the ground at `world` keeps under the
/// clouds at `now`: `1.0` in the open, down to `1.0 - CLOUD_DEPTH` under the
/// deepest part of a patch.
///
/// **Three sines and not `tile_hash`**, which is sitting right there and is
/// the wrong tool twice over. A hash keyed on the coordinate gives white
/// noise, and what is wanted is patches; a hash keyed on the coordinate
/// *and* the clock gives white noise that changes every frame, which is
/// television static. What makes this read as a shadow is that the whole
/// field translates — the sample point is walked upwind by `CLOUD_WIND *
/// now` and nothing else about it depends on time, so the ground downwind
/// of you later is the ground you are standing on now. That is a property a
/// test can hold, and `the_cloud_field_drifts_rather_than_flickering` is
/// what holds it.
///
/// Three components rather than one because a single sine is a set of
/// parallel stripes however it is angled. The frequencies are
/// incommensurate and the angles unrelated, so the sum has no period a
/// player could sit still and watch come round.
///
/// Keyed on the **world** coordinate for `tile_shade`'s reason: the camera
/// slides continuously across tiles, and a field tied to screen position
/// would crawl over the terrain as it went.
fn cloud_shade(world: (i32, i32), now: f64) -> f32 {
    let x = (world.0 as f64 - CLOUD_WIND.0 as f64 * now) / CLOUD_SCALE as f64;
    let y = (world.1 as f64 - CLOUD_WIND.1 as f64 * now) / CLOUD_SCALE as f64;
    let tau = std::f64::consts::TAU;
    // Amplitudes fall off with frequency, so the coarse component decides
    // where a patch is and the two finer ones only rough up its edge.
    let f = 0.55 * (tau * (x * 1.00 + y * 0.35)).sin()
        + 0.30 * (tau * (x * -0.62 + y * 1.43)).sin()
        + 0.15 * (tau * (x * 1.91 + y * 1.07)).sin();
    // The sum runs -1..1 by construction; fold it to 0..1 before the band.
    let cover = smoothstep(CLOUD_EDGE0, CLOUD_EDGE1, (f as f32 + 1.0) / 2.0);
    1.0 - CLOUD_DEPTH * cover
}

/// The usual cubic ease between two edges, clamped outside them. Local
/// because this is the only thing in the module that needs one — the bobs
/// and pulses are all trigonometric and the flashes are all linear.
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How many sparks a burst throws and how far they reach, in tiles.
fn spark_burst(kind: EffectKind) -> (u32, f32) {
    match kind {
        // A brawl is a hit in all three of these tables. The variant exists
        // to carry *sound*, never a different look.
        EffectKind::Hit | EffectKind::Brawl => (HIT_SPARKS, HIT_SPARK_REACH),
        EffectKind::Destroyed => (DESTROYED_SPARKS, DESTROYED_SPARK_REACH),
        EffectKind::Deflected => (0, 0.0),
        // Sound only — see `EffectKind::Mine`. Zero here *and* zero in
        // `effect_duration`, since `Deflected` shows that no sparks alone
        // still leaves a flash.
        EffectKind::Mine => (0, 0.0),
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
pub(crate) fn spark_scatter(pos: (i32, i32), index: u32) -> f32 {
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

/// How far along its rise a floating mark is at `t`, 0..1 across its life:
/// eased out, so it pops off the body and slows as it fades. The one curve
/// a Repair Bay's `+` and a battle map's numbers both rise on.
fn float_rise(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(2)
}

/// How high a reaction's `!` sits above its rest position at `t`: a snap
/// up over `REACTION_MARK_RISE`, then held while it fades — a pop rather
/// than a number's steady rise, so the two never read as the same news.
fn reaction_mark_height(t: f32) -> f32 {
    REACTION_MARK_HEIGHT * (t / REACTION_MARK_RISE).min(1.0)
}

/// How high a mending body's `+` sits above its rest position, as a share
/// of the tile, and how opaque it is. Eased out, so each mark pops off the
/// patient and slows as it fades, and phase-keyed for `staffed_bob_offset`'s
/// reason: two patients float out of step.
fn recovery_float(time: f64, phase_key: u64) -> (f32, f32) {
    let turns =
        time / RECOVERY_FLOAT_SECONDS + (phase_key % PHASE_KEYS) as f64 * STAFFED_BOB_PHASE_STEP;
    let t = turns.fract() as f32;
    (RECOVERY_FLOAT_RISE * float_rise(t), cell_mark_alpha(t))
}

/// A cell mark fades linearly across its whole life, `draw_floats`' own
/// curve — there is no separate hold, since the motion already gives the
/// eye enough time to catch it.
fn cell_mark_alpha(t: f32) -> f32 {
    (1.0 - t).clamp(0.0, 1.0)
}

/// How long a walk of `cells` cells is on screen. Zero for a walk with
/// nowhere to go, which is what retires it on the frame it arrives.
fn walk_seconds(cells: usize) -> f64 {
    cells.saturating_sub(1) as f64 * TRANSIT_SECONDS_PER_CELL
}

/// Which leg of a walk a body is on `elapsed` seconds in, and how far along
/// that leg — or `None` when it has not set off yet (its stagger has not
/// come round) or has already reached the far end.
fn walk_progress(elapsed: f64, cells: usize) -> Option<(usize, f32)> {
    if elapsed < 0.0 || cells < 2 {
        return None;
    }
    let t = elapsed / TRANSIT_SECONDS_PER_CELL;
    let leg = t.floor() as usize;
    if leg >= cells - 1 {
        return None;
    }
    Some((leg, t.fract() as f32))
}

fn effect_duration(kind: EffectKind) -> f64 {
    match kind {
        EffectKind::Hit | EffectKind::Deflected | EffectKind::Brawl => HIT_FLASH_SECONDS,
        EffectKind::Destroyed => DESTROYED_FLASH_SECONDS,
        // Zero is how a kind opts out of being drawn at all: `begin_frame`
        // pushes the flash and then retains on `now - start < duration`, so
        // a zero-length flash is gone on the frame it arrived. The struck
        // cell's own progress bar is the picture.
        EffectKind::Mine => 0.0,
    }
}

fn effect_color(kind: EffectKind) -> Color {
    match kind {
        EffectKind::Hit | EffectKind::Brawl => FLASH_RED,
        EffectKind::Deflected => FLASH_CYAN,
        EffectKind::Destroyed => FLASH_WHITE,
        // Never reached: a zero-length flash is dropped before anything
        // asks what colour it is. Answering `FLASH_RED` anyway rather than
        // panicking, since a cue turning into a crash is the worse failure.
        EffectKind::Mine => FLASH_RED,
    }
}

struct TileFlash {
    pos: (i32, i32),
    kind: EffectKind,
    start: f64,
}

/// A glyph lifted over a body's cell on a tactical battle map, drawn from a
/// `TacticalFxCue` the engine queued and forgot — `TileFlash`'s counterpart
/// for the kinds of tactical fx that draw nothing like a wash or a spark
/// burst.
struct CellMark {
    pos: (i32, i32),
    kind: MarkKind,
    start: f64,
}

/// What a `CellMark` is, and so everything about how it draws. The text,
/// the hue, the life and the motion are the whole of the difference between
/// two marks, which is why they are one list and one draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MarkKind {
    /// A red `-N` rising off a body that was hit, in the hit's own wash
    /// colour so the number and the flash read as one blow.
    Damage(i32),
    /// A green `+N` rising off a body that was healed. `HEALTHY` rather
    /// than a new palette role — Integrity coming back is what it means.
    Heal(i32),
    /// A `!` popping over a body that took an opportunity swing.
    /// `ATTENTION`, not `THREAT`: the reactor may be on the player's side,
    /// and `THREAT` is reserved for a structure taking a hit.
    Reaction,
}

impl MarkKind {
    fn text(self) -> String {
        match self {
            MarkKind::Damage(n) => format!("-{n}"),
            MarkKind::Heal(n) => format!("+{n}"),
            MarkKind::Reaction => "!".to_string(),
        }
    }

    fn color(self) -> Color {
        match self {
            MarkKind::Damage(_) => FLASH_RED,
            MarkKind::Heal(_) => palette::HEALTHY,
            MarkKind::Reaction => palette::ATTENTION,
        }
    }

    fn seconds(self) -> f64 {
        match self {
            MarkKind::Damage(_) | MarkKind::Heal(_) => NUMBER_MARK_SECONDS,
            MarkKind::Reaction => REACTION_MARK_SECONDS,
        }
    }

    fn height(self, t: f32) -> f32 {
        match self {
            MarkKind::Damage(_) | MarkKind::Heal(_) => NUMBER_MARK_RISE * float_rise(t),
            MarkKind::Reaction => reaction_mark_height(t),
        }
    }
}

/// A body walking across base space, drawn from a `TransitCue` the engine
/// queued and forgot. `start` may be in the future — a squad's bodies are
/// staggered so they leave one after another.
struct Walker {
    glyph: char,
    color: GlyphColor,
    path: Vec<(i32, i32)>,
    start: f64,
}

/// The mark over a program that has just spoken — a quotation mark, which
/// both the UI face (DejaVu) and the map face (UNSCII) carry.
pub const SPEECH_GLYPH: char = '"';
/// How long the mark stays up. Long enough to catch the eye on a quiet base
/// map, short enough that two chats in a row do not blur into one.
pub const SPEECH_SECONDS: f64 = 1.5;

/// A conversation's mark, drawn from a `SpeechCue` the engine queued and
/// forgot.
struct SpeechMark {
    cell: (i32, i32),
    start: f64,
}

/// The library effects a destroyed prop plays, and the ink they burn in.
const PROP_BLAST_FX: &str = "explosion";
const PROP_BLAST_LARGE_FX: &str = "explosion_large";
const PROP_BLAST_COLOR: GlyphColor = GlyphColor::Orange;

/// A blow in flight, drawn from a `BoltCue` the engine queued and forgot.
///
/// **No stagger, where a `Walker` has one.** A squad files out of the base
/// one behind the other; a sweep's streaks all leave the same swinger in the
/// same instant, and staggering them would read as several attacks.
struct Bolt {
    from: (i32, i32),
    to: (i32, i32),
    /// Every covered cell of an aimed routine; empty for a single blow, whose
    /// impacts land on `to`.
    cells: Vec<(i32, i32)>,
    color: GlyphColor,
    def: EffectDef,
    start: f64,
    /// Whether its impact has already added its shake.
    shaken: bool,
}

/// Which body the battle camera is aimed at, and how long it may stay there.
///
/// `release` is pushed forward on every frame the same body is still acting,
/// so the dwell is measured from the hand-over without anything having to
/// notice one happened: the hold expires `CAMERA_DWELL_SECONDS` after the
/// last frame this body held the turn. `seen` is the frame it was last asked
/// at, which is how a board coming back on screen is told from a board that
/// never left it.
struct CameraHold {
    key: u64,
    cell: (f32, f32),
    release: f64,
    seen: f64,
}

/// A battle-map body's glide from where it was drawn toward the cell it
/// now stands on — see `Fx::slid_cell`.
struct Slide {
    from: (f32, f32),
    to: (i32, i32),
    start: f64,
    seconds: f64,
}

impl Slide {
    /// Started a whole glide ago, so a body's first step after standing
    /// still — or after first being seen — takes the full glide rather than
    /// reading the wait as a fast walk's pace.
    fn at_rest(cell: (i32, i32), now: f64) -> Self {
        Slide {
            from: (cell.0 as f32, cell.1 as f32),
            to: cell,
            start: now - SLIDE_SECONDS,
            seconds: SLIDE_SECONDS,
        }
    }

    fn position(&self, now: f64) -> (f32, f32) {
        let t = if self.seconds > 0.0 {
            ((now - self.start) / self.seconds).clamp(0.0, 1.0) as f32
        } else {
            1.0
        };
        (
            self.from.0 + (self.to.0 as f32 - self.from.0) * t,
            self.from.1 + (self.to.1 as f32 - self.from.1) * t,
        )
    }
}

struct FloatingNumber {
    text: String,
    x: f32,
    y: f32,
    color: Color,
    start: f64,
    /// Multiplier on the label size; 1.0 for everything but a crit.
    scale: f32,
    /// The row an impact float belongs to, for staggering. `None` for
    /// floats not tied to a roster row.
    target: Option<ImpactTarget>,
}

struct RowFlash {
    target: ImpactTarget,
    color: Color,
    start: f64,
}

/// The picture window's reaction this frame, handed to the portrait draw.
/// `Default` is the window at rest.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PortraitReaction {
    pub nudge_x: f32,
    /// Tint colour (rgb) and its alpha.
    pub flash: Option<(Color, f32)>,
    /// Alpha of the `KO` stamp.
    pub ko: Option<f32>,
}

/// What `Fx::bar_ghost` hands back for one frame of one HP bar.
pub struct BarFx {
    /// The trailing value the ghost band is drawn at, easing toward the
    /// real one.
    pub ghost: f32,
}

#[derive(Clone, Copy)]
struct BarTracking {
    ghost: f32,
}

pub struct Fx {
    pub enabled: bool,
    now: f64,
    flashes: Vec<TileFlash>,
    tactical_flashes: Vec<TileFlash>,
    cell_marks: Vec<CellMark>,
    /// Board cells a drop trooper landed on, and when. Not a `TileFlash`:
    /// that is keyed by a world `EffectKind`, and a landing is not one.
    landings: Vec<((i32, i32), f64)>,
    walkers: Vec<Walker>,
    speech: Vec<SpeechMark>,
    bolts: Vec<Bolt>,
    /// Named looks for `BoltCue.fx`; only the built-in `streak` until
    /// `effects::load` fills it.
    pub(crate) library: EffectLibrary,
    shake_energy: f32,
    floats: Vec<FloatingNumber>,
    row_flashes: Vec<RowFlash>,
    /// Where each roster row was last drawn. A row whose group has died is
    /// gone from the view, but the blow that killed it can still be waiting
    /// to land; it falls back to this.
    rows: HashMap<ImpactTarget, Rect>,
    portrait_hit: Option<(f64, bool)>,
    portrait_ko: Option<f64>,
    bars: HashMap<ImpactTarget, BarTracking>,
    camera: Option<(f32, f32)>,
    camera_hold: Option<CameraHold>,
    slides: HashMap<u64, Slide>,
    log_flash_until: f64,
    last_log_line: Option<LogLine>,
    round_seen: Option<u32>,
    round_banner: Option<(u32, f64)>,
    map_click: Option<crate::render::MapClickLayout>,
}

/// Everything the engine queued for one frame, drained together so the
/// frame loop and the tests take the same set.
#[derive(Default)]
pub struct FrameCues {
    pub effects: Vec<VisualEffect>,
    pub transits: Vec<TransitCue>,
    pub bolts: Vec<BoltCue>,
    pub routines: Vec<RoutineCue>,
    pub tactical_fx: Vec<TacticalFxCue>,
    pub speech: Vec<SpeechCue>,
}

impl FrameCues {
    pub fn take(game: &mut Game) -> Self {
        Self {
            effects: game.take_effects(),
            transits: game.take_transits(),
            bolts: game.take_bolts(),
            routines: game.take_routine_cues(),
            tactical_fx: game.take_tactical_fx(),
            speech: game.take_speech(),
        }
    }
}

impl Fx {
    pub fn new() -> Self {
        Self {
            enabled: true,
            now: 0.0,
            flashes: Vec::new(),
            tactical_flashes: Vec::new(),
            cell_marks: Vec::new(),
            landings: Vec::new(),
            walkers: Vec::new(),
            speech: Vec::new(),
            bolts: Vec::new(),
            library: EffectLibrary::default(),
            shake_energy: 0.0,
            floats: Vec::new(),
            row_flashes: Vec::new(),
            rows: HashMap::new(),
            portrait_hit: None,
            portrait_ko: None,
            bars: HashMap::new(),
            camera: None,
            camera_hold: None,
            slides: HashMap::new(),
            log_flash_until: 0.0,
            last_log_line: None,
            round_seen: None,
            round_banner: None,
            map_click: None,
        }
    }

    /// Stashes the map pane's layout for this frame — written by
    /// `draw_surface_map` when it draws the ordinary surface/base grid and
    /// by `draw_tactical_map` for a battle board, cleared by
    /// `draw_playing_base` before its third branch (a Stack corridor) so a
    /// click there finds nothing to invert rather than reading a stale
    /// layout from a screen the player has since left. Read back by
    /// `lib.rs::handle_map_pointer`.
    pub(crate) fn set_map_click(&mut self, layout: Option<crate::render::MapClickLayout>) {
        self.map_click = layout;
    }

    /// This frame's map pane layout, if the surface grid or a battle board
    /// drew one — see `set_map_click`.
    pub(crate) fn map_click(&self) -> Option<crate::render::MapClickLayout> {
        self.map_click
    }

    /// Called once per frame before drawing: stamps the frame's time,
    /// takes in newly queued engine effects and walks, and retires expired
    /// ones. Every queue in `cues` is always consumed, even when disabled, so
    /// none of the engine's can sit permanently at its cap.
    pub fn begin_frame(&mut self, now: f64, cues: FrameCues, in_battle: bool) {
        let FrameCues {
            effects,
            transits,
            bolts,
            routines,
            tactical_fx,
            speech,
        } = cues;
        let dt = (now - self.now).max(0.0) as f32;
        self.now = now;
        self.shake_energy *= (-dt / SHAKE_DECAY_SECONDS).exp();
        if !self.enabled || !in_battle || self.shake_energy < SHAKE_FLOOR {
            self.shake_energy = 0.0;
        }
        if self.enabled {
            for e in effects {
                self.flashes.push(TileFlash {
                    pos: e.pos,
                    kind: e.kind,
                    start: now,
                });
            }
            for (index, cue) in transits.into_iter().enumerate() {
                self.walkers.push(Walker {
                    glyph: cue.glyph,
                    color: cue.color,
                    path: cue.path,
                    start: now + index as f64 * TRANSIT_STAGGER_SECONDS,
                });
            }
            for cue in speech {
                self.speech.push(SpeechMark {
                    cell: cue.cell,
                    start: now,
                });
            }
            for cue in bolts {
                let def = self.library.get(cue.fx.as_deref()).clone();
                self.bolts.push(Bolt {
                    from: cue.from,
                    to: cue.to,
                    cells: Vec::new(),
                    color: cue.color,
                    def,
                    start: now,
                    shaken: false,
                });
            }
            for cue in routines {
                let def = self.library.get(cue.fx.as_deref()).clone();
                self.bolts.push(Bolt {
                    from: cue.from,
                    to: cue.aim,
                    cells: cue.cells,
                    color: cue.color,
                    def,
                    start: now,
                    shaken: false,
                });
            }
            // Split by kind here rather than at the engine: a hit reuses
            // `EffectKind::Hit`'s own wash and spark burst — the same red
            // flash and debris a raid draws — while a heal has nothing in
            // that table to reuse and gets its own list instead.
            for cue in tactical_fx {
                match cue.kind {
                    TacticalFxKind::Hit(dealt) => {
                        self.tactical_flashes.push(TileFlash {
                            pos: cue.pos,
                            kind: EffectKind::Hit,
                            start: now,
                        });
                        self.cell_marks.push(CellMark {
                            pos: cue.pos,
                            kind: MarkKind::Damage(dealt),
                            start: now,
                        });
                    }
                    TacticalFxKind::Heal(restored) => self.cell_marks.push(CellMark {
                        pos: cue.pos,
                        kind: MarkKind::Heal(restored),
                        start: now,
                    }),
                    TacticalFxKind::Reaction => self.cell_marks.push(CellMark {
                        pos: cue.pos,
                        kind: MarkKind::Reaction,
                        start: now,
                    }),
                    TacticalFxKind::Landing => self.landings.push((cue.pos, now)),
                    // Nothing flies to a prop, so the library effect's travel
                    // is dropped and only its impacts and shake play; the
                    // same bolt path then draws and shakes it with no second
                    // mechanism.
                    TacticalFxKind::PropDestroyed { volatile } => {
                        let id = if volatile {
                            PROP_BLAST_LARGE_FX
                        } else {
                            PROP_BLAST_FX
                        };
                        let mut def = self.library.get(Some(id)).clone();
                        def.travel = Travel::None;
                        self.bolts.push(Bolt {
                            from: cue.pos,
                            to: cue.pos,
                            cells: Vec::new(),
                            color: PROP_BLAST_COLOR,
                            def,
                            start: now,
                            shaken: false,
                        });
                    }
                }
            }
        }
        self.flashes
            .retain(|f| now - f.start < effect_duration(f.kind));
        self.tactical_flashes
            .retain(|f| now - f.start < effect_duration(f.kind));
        self.cell_marks.retain(|m| now - m.start < m.kind.seconds());
        self.landings
            .retain(|&(_, start)| now - start < LANDING_FLASH_SECONDS);
        self.walkers
            .retain(|w| now - w.start < walk_seconds(w.path.len()));
        // Each impact's shake lands on arrival, after the flight, so a
        // blast kicks the board when it hits rather than when it is fired.
        if self.enabled && in_battle {
            for bolt in self.bolts.iter_mut().filter(|b| !b.shaken) {
                if now - bolt.start >= effect_draw::travel_seconds(bolt.def.travel) {
                    bolt.shaken = true;
                    self.shake_energy = (self.shake_energy + bolt.def.shake).min(SHAKE_MAX_ENERGY);
                }
            }
        }
        self.bolts
            .retain(|b| now - b.start < effect_draw::blow_seconds(&b.def).max(BOLT_SECONDS));
        self.speech.retain(|s| now - s.start < SPEECH_SECONDS);
        self.floats.retain(|f| now - f.start < FLOAT_SECONDS);
        self.row_flashes
            .retain(|f| now - f.start < ROW_FLASH_SECONDS);
        if !in_battle {
            self.clear_bars();
            // A battle map's own flashes and marks, cleared with the bars
            // for the same reason: board cell indices are reused between
            // fights, and a hit lingering past the fight it landed in would
            // paint the wrong body's cell in whatever opens next.
            self.tactical_flashes.clear();
            self.cell_marks.clear();
            self.landings.clear();
            // A blow still in flight would shake the base map's camera on
            // arrival, after the battle it was fired in has ended.
            self.bolts.clear();
            // Rows are keyed by group ids that only mean something inside
            // their own fight.
            self.rows.clear();
            self.row_flashes.clear();
            self.portrait_hit = None;
            self.portrait_ko = None;
        }
    }

    /// How much of its lit brightness the ground at `world` keeps under this
    /// frame's clouds — see [`cloud_shade`], of which this is the frame's
    /// clock plus the effects toggle.
    ///
    /// Off with effects, and exactly `1.0` rather than nearly so: this is a
    /// multiplier into the ground's brightness, and a player who has turned
    /// the moving parts off should get the same pixels they got before the
    /// feature shipped.
    pub fn cloud_shade(&self, world: (i32, i32)) -> f32 {
        if self.enabled {
            cloud_shade(world, self.now)
        } else {
            1.0
        }
    }

    /// The tint to overlay on the tile at `pos`, if a flash is active there
    /// in `flashes` — the shared core `tile_flash` and `tactical_tile_flash`
    /// each call rather than restate, so a raid's wash and a tactical hit's
    /// read from exactly one rule with two different lists behind it.
    /// Overlapping flashes take the newest, so a structure destroyed by the
    /// blow that damaged it shows the destruction rather than both at once.
    fn flash_in(flashes: &[TileFlash], now: f64, pos: (i32, i32)) -> Option<Color> {
        let flash = flashes
            .iter()
            .filter(|f| f.pos == pos)
            .max_by(|a, b| a.start.total_cmp(&b.start))?;
        let alpha = flash_alpha(now - flash.start, effect_duration(flash.kind));
        if alpha <= 0.0 {
            return None;
        }
        let c = effect_color(flash.kind);
        Some(Color::new(c.r, c.g, c.b, alpha))
    }

    /// The tint to overlay on the world tile at `pos`, if a raid or a base
    /// brawl has a flash active there.
    pub fn tile_flash(&self, pos: (i32, i32)) -> Option<Color> {
        Self::flash_in(&self.flashes, self.now, pos)
    }

    /// The same wash, read off a tactical battle map's own flashes.
    ///
    /// **A separate list keyed by board cell, not `flashes` keyed by world
    /// tile.** A hit on a body standing on a battle map is cued in
    /// `TacticalBattle` cells (`BoltCue`'s reason), and a coordinate from
    /// one space read against the other's list would answer for the wrong
    /// tile whenever the two happened to share a pair of small integers.
    ///
    /// A landing's green wash joins it here, newest first as `flash_in`
    /// rules, so a trooper hit the instant it lands shows the hit.
    pub fn tactical_tile_flash(&self, pos: (i32, i32)) -> Option<Color> {
        let landed = self
            .landings
            .iter()
            .filter(|(at, _)| *at == pos)
            .map(|&(_, start)| start)
            .max_by(f64::total_cmp);
        let hit = self
            .tactical_flashes
            .iter()
            .filter(|f| f.pos == pos)
            .map(|f| f.start)
            .max_by(f64::total_cmp);
        match landed {
            Some(start) if hit.is_none_or(|h| start > h) => {
                let alpha = flash_alpha(self.now - start, LANDING_FLASH_SECONDS);
                let c = palette::HEALTHY;
                (alpha > 0.0).then(|| Color::new(c.r, c.g, c.b, alpha))
            }
            _ => Self::flash_in(&self.tactical_flashes, self.now, pos),
        }
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
    /// `to_px` maps a cell to its top-left corner in the pane, since sparks
    /// cross tile boundaries and so cannot be drawn from inside the
    /// caller's tile loop. Shared by `draw_bursts` and
    /// `draw_tactical_bursts`, the two callers reading a different list off
    /// a different coordinate space through the same `to_px`.
    fn draw_bursts_in(
        flashes: &[TileFlash],
        now: f64,
        painter: &Painter,
        tile_px: f32,
        to_px: impl Fn((i32, i32)) -> (f32, f32),
    ) {
        for flash in flashes {
            let (count, reach) = spark_burst(flash.kind);
            if count == 0 {
                continue;
            }
            let t = ((now - flash.start) / effect_duration(flash.kind)) as f32;
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

    /// Draws the debris every live burst on the zone or base map is
    /// currently throwing.
    pub fn draw_bursts(
        &self,
        painter: &Painter,
        tile_px: f32,
        to_px: impl Fn((i32, i32)) -> (f32, f32),
    ) {
        Self::draw_bursts_in(&self.flashes, self.now, painter, tile_px, to_px);
    }

    /// The same debris, off a tactical battle map's own flashes — a body
    /// struck on a board throws exactly the burst `EffectKind::Hit` throws
    /// on a raided structure, called through `draw_bursts_in` rather than
    /// restated.
    pub fn draw_tactical_bursts(
        &self,
        painter: &Painter,
        tile_px: f32,
        to_px: impl Fn((i32, i32)) -> (f32, f32),
    ) {
        Self::draw_bursts_in(&self.tactical_flashes, self.now, painter, tile_px, to_px);
    }

    /// Draws every cell mark currently over a body on a tactical battle
    /// map — a hit's `-N`, a heal's `+N`, a reaction's `!` — centred the way
    /// `draw_walkers` centres a glyph on measured ink, lifted by its kind's
    /// own height curve and faded by `cell_mark_alpha`.
    pub fn draw_cell_marks(
        &self,
        painter: &Painter,
        tile_px: f32,
        glyph_px: u16,
        to_px: impl Fn((i32, i32)) -> (f32, f32),
    ) {
        for mark in &self.cell_marks {
            let t = ((self.now - mark.start) / mark.kind.seconds()) as f32;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let text = mark.kind.text();
            let (ox, oy) = to_px(mark.pos);
            let dims = painter.measure_map(&text, glyph_px);
            let x = ox + (tile_px - dims.width) / 2.0;
            let y = oy + (tile_px + dims.height) / 2.0 - mark.kind.height(t) * tile_px;
            let base = mark.kind.color();
            let color = Color::new(base.r, base.g, base.b, cell_mark_alpha(t));
            painter.map(&text, x, y, glyph_px, color);
        }
    }

    /// Draws every body currently walking across base space, interpolated
    /// between the two cells of the leg it is on.
    ///
    /// Outside the caller's tile loop for `draw_bursts`' reason: a walker
    /// spends most of its life between two tiles and belongs to neither.
    ///
    /// It is drawn from the same measured-ink centring the map's own glyphs
    /// use, so a body mid-walk sits exactly where it would if it were
    /// standing — `Painter::map` takes a *baseline*, and reading it as a
    /// top-left is a half-cell offset that reads as a camera fault.
    pub fn draw_walkers(
        &self,
        painter: &Painter,
        tile_px: f32,
        glyph_px: u16,
        to_px: impl Fn((i32, i32)) -> (f32, f32),
    ) {
        for walker in &self.walkers {
            let Some((leg, along)) = walk_progress(self.now - walker.start, walker.path.len())
            else {
                continue;
            };
            let (ax, ay) = to_px(walker.path[leg]);
            let (bx, by) = to_px(walker.path[leg + 1]);
            let (px, py) = (ax + (bx - ax) * along, ay + (by - ay) * along);
            let glyph = walker.glyph.to_string();
            let dims = painter.measure_map(&glyph, glyph_px);
            painter.map(
                &glyph,
                px + (tile_px - dims.width) / 2.0,
                py + (tile_px + dims.height) / 2.0,
                glyph_px,
                palette::glyph(walker.color),
            );
        }
    }

    /// Every speech mark still up, centred over its cell's top edge and
    /// fading out. Base-space only, `draw_walkers`' gate: the cell is a
    /// base-space one.
    pub fn draw_speech(
        &self,
        painter: &Painter,
        tile_px: f32,
        glyph_px: u16,
        to_px: impl Fn((i32, i32)) -> (f32, f32),
    ) {
        let glyph = SPEECH_GLYPH.to_string();
        for mark in &self.speech {
            let t = ((self.now - mark.start) / SPEECH_SECONDS) as f32;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let (ox, oy) = to_px(mark.cell);
            let dims = painter.measure_map(&glyph, glyph_px);
            let base = palette::EMPHASIS;
            painter.map(
                &glyph,
                ox + (tile_px - dims.width) / 2.0,
                oy + dims.height * 0.5,
                glyph_px,
                Color::new(base.r, base.g, base.b, base.a * (1.0 - t)),
            );
        }
    }

    /// Every blow currently in flight, as a lit head running from the body
    /// that swung to the body it landed on.
    ///
    /// `to_px` answers a cell's **top-left**, so the streak is offset half a
    /// tile to run centre to centre — a line drawn corner to corner leaves
    /// the glyph it came from and arrives beside the one it hit. Not
    /// `Painter::map`'s measured-ink baseline, which `draw_walkers` needs
    /// and this does not: a line has no ink to centre.
    pub fn draw_bolts(
        &self,
        painter: &Painter,
        to_px: impl Fn((i32, i32)) -> (f32, f32),
        tile_px: f32,
    ) {
        for bolt in &self.bolts {
            let age = self.now - bolt.start;
            let (ca, cb) = cell_centers(&to_px, tile_px, bolt.from, bolt.to);
            let color = palette::glyph(bolt.def.color.unwrap_or(bolt.color));
            if bolt.def.muzzle {
                effect_draw::draw_muzzle(painter, bolt.from, ca, tile_px, age);
            }
            let flight = effect_draw::travel_seconds(bolt.def.travel);
            if age < flight {
                let along = (age / BOLT_SECONDS).clamp(0.0, 1.0) as f32;
                effect_draw::draw_travel(
                    painter,
                    bolt.def.travel,
                    (bolt.from, bolt.to),
                    (ca, cb),
                    tile_px,
                    along,
                    color,
                );
            }
            let landed = age - flight;
            if landed < 0.0 {
                continue;
            }
            let cells: &[(i32, i32)] = if bolt.cells.is_empty() {
                std::slice::from_ref(&bolt.to)
            } else {
                &bolt.cells
            };
            for impact in &bolt.def.impact {
                // An area covers every cell; a single-cell impact lands on
                // the aim alone — `Sparks` is the blow's own hit burst and
                // `Slash` swipes from the attacker, so neither means
                // anything on a cell nobody aimed at.
                let area = matches!(
                    impact,
                    Impact::Explosion { .. } | Impact::Smoke | Impact::Flash | Impact::Zap
                );
                let targets: &[(i32, i32)] = if area {
                    cells
                } else {
                    std::slice::from_ref(&bolt.to)
                };
                for &cell in targets {
                    let (c, _) = cell_centers(&to_px, tile_px, cell, cell);
                    effect_draw::draw_impact(
                        painter,
                        *impact,
                        cell,
                        (ca, c),
                        tile_px,
                        landed,
                        color,
                    );
                }
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

    /// How far to offset a mark whose rest position is the **centre** of its
    /// tile rather than an inset off its bottom edge.
    ///
    /// The same curve `staffed_bob` rides — same rate, same entity phase, so
    /// the two motions on one map still agree with each other rather than
    /// being two independently-invented bounces — recentred on zero instead
    /// of anchored at it. `staffed_bob` is upward-only because the staffed
    /// mark's rest position is an inset a down-swing would spend; a build
    /// site's caret sits in the middle of its slab with room on both sides,
    /// and anchored upward it spends its whole cycle at or above centre and
    /// reads as a caret sitting high in the tile rather than one bouncing
    /// in it.
    ///
    /// Signed, and `STAFFED_BOB_PX` stays private to this module: doing the
    /// subtraction at the call site would put the relationship between the
    /// two curves in the renderer, where nothing keeps them one curve.
    pub fn centred_bob(&self, entity: Entity) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        staffed_bob_offset(self.now, entity.to_bits()) - STAFFED_BOB_PX / 2.0
    }

    /// How far above its rest position to draw a body's rising mark — the
    /// mending `+`, the amenity `^` — this frame, in pixels of a `tile_px`
    /// tile, and at what alpha. The rise is a
    /// share of the tile so it reads the same at every zoom, and that share
    /// stays private here, `strain_jitter`'s rule.
    pub fn recovery_float(&self, entity: Entity, tile_px: f32) -> (f32, f32) {
        if !self.enabled {
            return (0.0, 1.0);
        }
        let (rise, alpha) = recovery_float(self.now, entity.to_bits());
        (rise * tile_px, alpha)
    }

    /// How far to offset the **ink** of a body a research project is
    /// spending right now — `views::PinMark::Strained`'s own motion.
    ///
    /// The ink alone, never the pin marks: those four squares are the pen,
    /// and a pen that shakes with what it is holding says the apparatus is
    /// loose rather than that the body is under stress. They also sit on the
    /// tile-edge ring, where any offset at all overhangs the neighbouring
    /// cell, while the glyph sits `(tile_px - glyph_px) / 2` inside it and
    /// has the room to spare.
    ///
    /// `margin` is the room the ink has — the caller's own
    /// `(tile_px - glyph_px) / 2` — and the **share** of it spent stays
    /// private to this module, `centred_bob`'s rule: an amplitude the call
    /// site could reach is an amplitude the call site can disagree about.
    /// Passing the margin rather than a zoom keeps the bound structural, so
    /// the rattle cannot reach the pen at any step of the ladder.
    pub fn strain_jitter(&self, entity: Entity, margin: f32) -> (f32, f32) {
        if !self.enabled {
            return (0.0, 0.0);
        }
        let (x, y) = strain_offset(self.now, entity.to_bits());
        let reach = margin * STRAIN_JITTER_SHARE;
        (reach * x, reach * y)
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
    /// Keyed by `ImpactTarget`, the same name an impact uses for its row, so
    /// a party slot and an enemy group can never share a tracker however
    /// high the group-id counter climbs, and a group keeps its ghost when a
    /// dead one ahead of it re-letters the rest.
    pub fn bar_ghost(&mut self, key: ImpactTarget, value: i32, dt: f32) -> BarFx {
        // A first frame with no prior reading seeds the ghost at the real
        // value, so entering a battle doesn't animate a drain from zero.
        let previous = self.bars.get(&key).copied();
        let mut ghost = previous.map_or(value as f32, |prev| prev.ghost);
        ghost = if self.enabled {
            ghost_step(ghost, value as f32, dt)
        } else {
            value as f32
        };
        self.bars.insert(key, BarTracking { ghost });
        BarFx { ghost }
    }

    /// How far the camera currently sits from the player, in tiles, for the
    /// tile loop to shift the whole grid by. Trails rather than snapping, so
    /// a step reads as the map gliding under a player who leads it.
    ///
    /// The player is drawn as an ordinary entity at its own world position,
    /// so it drifts off centre while the camera catches up without needing a
    /// case of its own.
    pub fn camera_offset(
        &mut self,
        player: (i32, i32),
        dt: f32,
        max_lag: Option<f32>,
    ) -> (f32, f32) {
        self.camera_offset_toward((player.0 as f32, player.1 as f32), player, dt, max_lag)
    }

    /// `camera_offset` toward a target between cells — a battle-map body
    /// part-way through a glide. The offset is measured from `anchor`, the
    /// whole cell the caller lays its tiles out around, so the target's own
    /// fraction is in it.
    pub fn camera_offset_toward(
        &mut self,
        target: (f32, f32),
        anchor: (i32, i32),
        dt: f32,
        max_lag: Option<f32>,
    ) -> (f32, f32) {
        let anchor_f = (anchor.0 as f32, anchor.1 as f32);
        // Effects-off means an instant camera, as it does an instant HP bar.
        // The position is still tracked so re-enabling doesn't glide in from
        // wherever the camera was last left.
        if !self.enabled {
            self.camera = Some(target);
            return (target.0 - anchor_f.0, target.1 - anchor_f.1);
        }
        // A first frame with no prior reading seeds at the player, so loading
        // a world doesn't pan in from the origin.
        let previous = self.camera.unwrap_or(target);
        let cam = (
            camera_step(previous.0, target.0, dt, max_lag),
            camera_step(previous.1, target.1, dt, max_lag),
        );
        self.camera = Some(cam);
        let (sx, sy) = self.shake_offset();
        (cam.0 - anchor_f.0 + sx, cam.1 - anchor_f.1 + sy)
    }

    /// The board's thrown offset this frame, in tiles: a wobble hashed off
    /// the frame's time bucket, scaled by the stored energy and never more
    /// than `SHAKE_MAX_OFFSET_TILES`. Zero with effects off.
    fn shake_offset(&self) -> (f32, f32) {
        if !self.enabled || self.shake_energy <= 0.0 {
            return (0.0, 0.0);
        }
        let bucket = (self.now * SHAKE_WOBBLE_HZ).floor() as i32;
        let wobble = |salt: u32| spark_scatter((bucket, 0), salt) * 2.0 - 1.0;
        let reach = self.shake_energy.min(1.0) * SHAKE_MAX_OFFSET_TILES;
        (wobble(0) * reach, wobble(SHAKE_Y_SALT) * reach)
    }

    /// Which cell the **battle** camera should be aimed at, given whose turn
    /// it is and where that body stands.
    ///
    /// Not simply the acting body's cell, because a fight hands the turn on
    /// inside the same call that resolves the blow: aimed at the live answer
    /// the camera leaves the attacker while its streak is still in flight and
    /// its hit flash still lit, so the one thing the screen exists to show
    /// happens off screen.
    ///
    /// **Two arms, not three.** A body that is still acting and a body being
    /// latched onto fresh want the same write — follow the cell, push the
    /// release out — and the difference between them is only which key was
    /// there before. Following the cell is what keeps a *walk* centred: its
    /// steps are `TACTICAL_STEPS_PER_SECOND` apart, and a hold would fall
    /// behind one.
    ///
    /// Effects-off means an instant camera here too, so the dwell is skipped
    /// rather than shortened: with no streak and no flash drawn there is
    /// nothing to hold the camera for, and the hold would read as lag.
    ///
    /// `footprint` is the acting body's own — `footprint_center` folds it
    /// into the cell before anything below ever sees it, so the hold, the
    /// dwell and a walk's own tracking all follow the footprint's own
    /// centre rather than each having to know a squad exists.
    pub fn battle_center(
        &mut self,
        acting: Option<(Entity, (i32, i32), u8)>,
    ) -> Option<(f32, f32)> {
        // A gap wider than the dwell means no battle map was drawn last
        // frame, so this is a fight *opening* rather than one already on
        // screen: both the hold and the camera's easing belong to a board
        // that is gone, and easing in from wherever the surface map left the
        // camera is a long pan across unrelated ground.
        if self
            .camera_hold
            .as_ref()
            .is_none_or(|hold| self.now - hold.seen > CAMERA_DWELL_SECONDS)
        {
            self.camera_hold = None;
            self.camera = None;
            // The glides belong to the board that is gone too.
            self.slides.clear();
        }
        // Followed through the body's glide, not its cell: aimed at the
        // cell, the camera surged after every hop of a walk and all but
        // stopped before the next.
        let acting = acting.map(|(entity, cell, footprint)| {
            let (x, y) = self.slid_cell(entity, cell);
            let (cx, cy) = footprint_center(cell, footprint);
            (entity, (x + (cx - cell.0) as f32, y + (cy - cell.1) as f32))
        });
        // Nobody acting is a board still on screen — a finished fight's,
        // under its results popup — so the hold is refreshed rather than
        // left to lapse: a lapsed hold reads as a board that went away, and
        // the camera swings to the middle of the board while the results
        // are being read.
        let Some((entity, cell)) = acting else {
            let now = self.now;
            return self.camera_hold.as_mut().map(|hold| {
                hold.seen = now;
                hold.cell
            });
        };
        let key = entity.to_bits();
        let (now, dwell) = (self.now, self.enabled);
        match &mut self.camera_hold {
            // A different body, and the one before it is still owed its hold.
            Some(hold) if hold.key != key && dwell && now < hold.release => {
                hold.seen = now;
                Some(hold.cell)
            }
            Some(hold) => {
                hold.key = key;
                hold.cell = cell;
                hold.release = now + CAMERA_DWELL_SECONDS;
                hold.seen = now;
                Some(cell)
            }
            None => {
                self.camera_hold = Some(CameraHold {
                    key,
                    cell,
                    release: now + CAMERA_DWELL_SECONDS,
                    seen: now,
                });
                Some(cell)
            }
        }
    }

    /// Where a battle-map body is drawn this frame, in cells: its own cell
    /// at rest, and part-way along a glide after a one-cell step — for
    /// `SLIDE_SECONDS`, or for the time since its last step when that was
    /// sooner. The engine moves a body a whole cell at a time, so
    /// drawn straight off its cell a walk is a run of hops.
    ///
    /// A move of more than one cell — a blink, a body placed fresh — is a
    /// cut rather than a dash across the board. A step taken mid-glide
    /// starts from where the body is drawn, so a walk never jumps back.
    pub fn slid_cell(&mut self, entity: Entity, cell: (i32, i32)) -> (f32, f32) {
        let now = self.now;
        if !self.enabled {
            self.slides
                .insert(entity.to_bits(), Slide::at_rest(cell, now));
            return (cell.0 as f32, cell.1 as f32);
        }
        let slide = self
            .slides
            .entry(entity.to_bits())
            .or_insert_with(|| Slide::at_rest(cell, now));
        if slide.to != cell {
            let one_step = (slide.to.0 - cell.0).abs().max((slide.to.1 - cell.1).abs()) == 1;
            *slide = if one_step {
                // From where the body is drawn, so a step mid-glide never
                // jumps, and paced to the body's own last step when that
                // came faster than a walk's — a held key repeats faster — so
                // the glide lands before the next step instead of piling up
                // behind it. A pace that just quickened is one glide late to
                // be learnt, and the next glide covers the shortfall.
                Slide {
                    from: slide.position(now),
                    to: cell,
                    start: now,
                    seconds: {
                        let gap = (now - slide.start).max(0.0);
                        if gap < SLIDE_SECONDS * FAST_STEP_FRACTION {
                            gap
                        } else {
                            SLIDE_SECONDS
                        }
                    },
                }
            } else {
                Slide::at_rest(cell, now)
            };
        }
        slide.position(now)
    }

    /// Drops every bar tracker, so the next battle seeds its ghosts fresh
    /// rather than easing down from the last fight's numbers. Party slots
    /// are reused between battles, which is exactly when stale state would
    /// show.
    pub fn clear_bars(&mut self) {
        self.bars.clear();
    }

    /// Records where a roster row is drawn this frame. Called for every row
    /// before the frame's impacts land, so an impact finds its row by
    /// target and a row that has just left the view is still found at the
    /// place it was last drawn.
    pub fn note_row(&mut self, target: ImpactTarget, rect: Rect) {
        self.rows.insert(target, rect);
    }

    /// Lands an impact on its row's last known rect. Dropped when the row
    /// was never drawn, which is a target from a fight not yet on screen.
    pub fn land(&mut self, impact: BattleImpact) {
        if let Some(row) = self.rows.get(&impact.target).copied() {
            self.impact(impact, row);
        }
    }

    /// One impact: a styled float over `row`, a flash behind it for a hit,
    /// crit or heal, and the picture window's reaction.
    pub fn impact(&mut self, impact: BattleImpact, row: Rect) {
        self.picture_reacts(impact.kind);
        if !self.enabled {
            return;
        }
        let (text, color, scale) = match impact.kind {
            ImpactKind::Damage(n) => (format!("-{n}"), IMPACT_HIT_COLOR, 1.0),
            ImpactKind::Crit(n) => (format!("CRIT -{n}"), IMPACT_CRIT_COLOR, CRIT_FLOAT_SCALE),
            ImpactKind::Heal(n) => (format!("+{n}"), IMPACT_HEAL_COLOR, 1.0),
            ImpactKind::Miss => ("miss".to_string(), IMPACT_DIM_COLOR, 1.0),
            ImpactKind::Fumble => ("fumble".to_string(), IMPACT_DIM_COLOR, 1.0),
            ImpactKind::Kill => ("KO".to_string(), IMPACT_HIT_COLOR, 1.0),
        };
        let alive = self
            .floats
            .iter()
            .filter(|f| f.target == Some(impact.target))
            .count();
        self.floats.push(FloatingNumber {
            text,
            x: row.x + row.w / 2.0 + alive as f32 * IMPACT_STAGGER_PX,
            y: row.y + row.h / 2.0,
            color,
            start: self.now,
            scale,
            target: Some(impact.target),
        });
        let flash = match impact.kind {
            ImpactKind::Damage(_) => Some(IMPACT_HIT_COLOR),
            ImpactKind::Crit(_) => Some(IMPACT_CRIT_COLOR),
            ImpactKind::Heal(_) => Some(IMPACT_HEAL_COLOR),
            ImpactKind::Miss | ImpactKind::Fumble | ImpactKind::Kill => None,
        };
        if let Some(color) = flash {
            self.row_flashes.push(RowFlash {
                target: impact.target,
                color,
                start: self.now,
            });
        }
    }

    fn picture_reacts(&mut self, kind: ImpactKind) {
        if !self.enabled {
            return;
        }
        match kind {
            ImpactKind::Damage(_) => self.portrait_hit = Some((self.now, false)),
            ImpactKind::Crit(_) => self.portrait_hit = Some((self.now, true)),
            ImpactKind::Kill => self.portrait_ko = Some(self.now),
            ImpactKind::Heal(_) | ImpactKind::Miss | ImpactKind::Fumble => {}
        }
    }

    /// The newest flash on a row: its colour and the alpha to tint the
    /// row's background with, or `None` once it has faded.
    pub fn row_flash(&self, target: ImpactTarget) -> Option<(Color, f32)> {
        let flash = self
            .row_flashes
            .iter()
            .filter(|f| f.target == target)
            .max_by(|a, b| a.start.total_cmp(&b.start))?;
        let t = ((self.now - flash.start) / ROW_FLASH_SECONDS) as f32;
        (t < 1.0).then_some((flash.color, ROW_FLASH_PEAK_ALPHA * (1.0 - t)))
    }

    pub fn portrait_reaction(&self) -> PortraitReaction {
        let mut reaction = PortraitReaction::default();
        if let Some((start, crit)) = self.portrait_hit {
            let t = ((self.now - start) / PORTRAIT_NUDGE_SECONDS) as f32;
            if (0.0..1.0).contains(&t) {
                reaction.nudge_x = (t * PORTRAIT_NUDGE_CYCLES * std::f32::consts::TAU).sin()
                    * PORTRAIT_NUDGE_PX
                    * (1.0 - t);
                let color = if crit {
                    IMPACT_CRIT_COLOR
                } else {
                    IMPACT_HIT_COLOR
                };
                reaction.flash = Some((color, PORTRAIT_FLASH_PEAK_ALPHA * (1.0 - t)));
            }
        }
        if let Some(start) = self.portrait_ko {
            let t = ((self.now - start) / PORTRAIT_KO_SECONDS) as f32;
            if (0.0..1.0).contains(&t) {
                reaction.ko = Some(1.0 - t);
            }
        }
        reaction
    }

    #[cfg(test)]
    pub(crate) fn float_anchors(&self) -> Vec<(String, f32, f32)> {
        self.floats
            .iter()
            .map(|f| (f.text.clone(), f.x, f.y))
            .collect()
    }

    pub fn draw_floats(&self, painter: &Painter, m: &Metrics) {
        for f in &self.floats {
            let t = ((self.now - f.start) / FLOAT_SECONDS) as f32;
            let color = Color::new(f.color.r, f.color.g, f.color.b, 1.0 - t);
            let size = (m.label() as f32 * f.scale) as u16;
            painter.ui(&f.text, f.x, f.y - FLOAT_RISE_PX * t, size, color);
        }
    }

    /// Watches the open battle map's round and answers whether it has just
    /// wrapped, raising the `ROUND N` banner when it has.
    ///
    /// `None` — no fight open — is what ends the banner, rather than
    /// `begin_frame`'s `in_battle` clear, because the round is the one thing
    /// here that must also know *which* fight it is watching: a fight's
    /// first round, and a later fight's opening at 1, are both sightings
    /// rather than wraps.
    ///
    /// Reported whether or not effects are on: the caller plays the wrap's
    /// sound off the answer, and sound is not a visual effect.
    pub fn observe_round(&mut self, round: Option<u32>) -> bool {
        let wrapped = matches!((self.round_seen, round), (Some(seen), Some(now)) if now > seen);
        self.round_seen = round;
        match round {
            None => self.round_banner = None,
            Some(now) if wrapped && self.enabled => self.round_banner = Some((now, self.now)),
            Some(_) => {}
        }
        wrapped
    }

    /// The banner to draw this frame — the round it names and its alpha —
    /// or `None` once it has faded.
    pub fn round_banner(&self) -> Option<(u32, f32)> {
        let (round, start) = self.round_banner?;
        let t = (self.now - start) / ROUND_BANNER_SECONDS;
        (t < 1.0).then_some((round, (1.0 - t * t) as f32))
    }

    /// Watches for a newly logged raid or tantrum line and starts the log
    /// pane's flash. Compares the last line rather than counting lines, since
    /// `message_log` only ever returns a window of recent ones.
    pub fn observe_log(&mut self, last_line: Option<&LogLine>) {
        let changed = last_line != self.last_log_line.as_ref();
        if changed
            && self.enabled
            && last_line.is_some_and(|l| matches!(l.kind, MessageKind::Raid | MessageKind::Tantrum))
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

    /// A volatile prop's destruction plays the bigger library effect, and a
    /// plain one the ordinary explosion; both shake the board on the cue.
    #[test]
    fn a_destroyed_prop_plays_an_explosion_at_its_cell() {
        use feral_processes_engine::{TacticalFxCue, TacticalFxKind};
        for (volatile, id) in [(false, "explosion"), (true, "explosion_large")] {
            let mut fx = with_library();
            fx.begin_frame(
                0.0,
                FrameCues {
                    tactical_fx: vec![TacticalFxCue {
                        pos: (4, 5),
                        kind: TacticalFxKind::PropDestroyed { volatile },
                    }],
                    ..Default::default()
                },
                true,
            );
            assert_eq!(fx.bolts.len(), 1);
            assert_eq!(fx.bolts[0].to, (4, 5));
            assert_eq!(fx.bolts[0].def.id, id);
            fx.begin_frame(0.01, FrameCues::default(), true);
            assert!(fx.shake_energy > 0.0, "{id} did not shake the board");
        }
    }

    /// A trooper's landing washes its cell green for `LANDING_FLASH_SECONDS`
    /// and then clears — a board cell, so it reads through
    /// `tactical_tile_flash` alone.
    #[test]
    fn a_landing_washes_its_cell_green_then_clears() {
        use feral_processes_engine::{TacticalFxCue, TacticalFxKind};
        let mut fx = Fx::new();
        let cue = TacticalFxCue {
            pos: (2, 3),
            kind: TacticalFxKind::Landing,
        };
        fx.begin_frame(
            0.0,
            FrameCues {
                tactical_fx: vec![cue],
                ..Default::default()
            },
            true,
        );
        let wash = fx
            .tactical_tile_flash((2, 3))
            .expect("the landing cell flashes");
        let green = palette::HEALTHY;
        assert_eq!((wash.r, wash.g, wash.b), (green.r, green.g, green.b));
        assert!(fx.tactical_tile_flash((3, 3)).is_none(), "only that cell");
        assert!(fx.tile_flash((2, 3)).is_none(), "never a world tile");

        fx.begin_frame(LANDING_FLASH_SECONDS + 0.01, FrameCues::default(), true);
        assert!(fx.tactical_tile_flash((2, 3)).is_none());
    }
    /// **The reservation, at the one cue it was written for.** br red means
    /// hostility and inbound harm; a structure taking a hit in a raid is
    /// that, and the flash is what says so on the map. Asserted against the
    /// palette rather than against a literal, so the whole family moves
    /// together when the palette is retuned.
    ///
    /// The deflection and the destruction are checked too, because the three
    /// are one vocabulary: a deflection is the shield answering, which is not
    /// harm landing, and a destroyed structure is the loudest thing the
    /// screen can say rather than a fourth colour.
    /// A body that has not set off yet, one part way along, and one that
    /// has arrived. The stagger is what makes the first case reachable: a
    /// squad's second body is stamped with a start in the future, so the
    /// walk it is on has a negative elapsed until its turn comes round.
    #[test]
    fn a_walk_reports_the_leg_it_is_on_and_nothing_outside_it() {
        assert_eq!(walk_progress(-0.05, 4), None, "its stagger has not come up");
        assert_eq!(walk_progress(0.0, 4), Some((0, 0.0)));

        let (leg, along) = walk_progress(TRANSIT_SECONDS_PER_CELL * 1.5, 4).expect("mid-walk");
        assert_eq!(leg, 1, "one and a half cells along is the second leg");
        assert!((along - 0.5).abs() < 1e-5, "and halfway across it: {along}");

        assert_eq!(
            walk_progress(walk_seconds(4), 4),
            None,
            "a body that has reached the far end is not on any leg of the walk"
        );
        assert_eq!(walk_progress(0.0, 1), None, "a walk with nowhere to go");
    }

    /// A walker is drawn *between* its two cells, and is gone once it has
    /// arrived.
    ///
    /// The interpolation is the whole feature: asserting only that the glyph
    /// was painted passes against a walker that teleports cell to cell, so
    /// this reads the position back and pins it between the two ends.
    #[test]
    fn a_walker_is_drawn_between_the_cells_of_its_leg() {
        const CELL: f32 = 20.0;
        let to_px = |world: (i32, i32)| (world.0 as f32 * CELL, world.1 as f32 * CELL);
        let cue = TransitCue {
            glyph: 'W',
            color: GlyphColor::Cyan,
            path: vec![(0, 0), (1, 0), (2, 0)],
        };

        let mut fx = Fx::new();
        fx.begin_frame(
            0.0,
            FrameCues {
                transits: vec![cue.clone()],
                ..Default::default()
            },
            false,
        );
        // Half a cell in, so the body is between (0, 0) and (1, 0).
        fx.begin_frame(TRANSIT_SECONDS_PER_CELL * 0.5, FrameCues::default(), false);
        let (_, shapes) = crate::paint::with_painter(|p| fx.draw_walkers(p, CELL, 16, to_px));

        let drawn: Vec<(String, f32)> = shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                bevy_egui::egui::Shape::Text(t) => Some((t.galley.text().to_string(), t.pos.x)),
                _ => None,
            })
            .collect();
        let (_, x) = drawn
            .iter()
            .find(|(text, _)| text == "W")
            .unwrap_or_else(|| panic!("the walker must be painted: {drawn:?}"));
        assert!(
            *x > 0.0 && *x < CELL,
            "a body half a cell along must be drawn between the two, not on either: {x}"
        );

        // Past the end of the walk it is retired rather than parked on the
        // last cell — an away program has left, and one that is home is
        // drawn by the map itself.
        fx.begin_frame(walk_seconds(3) + 1.0, FrameCues::default(), false);
        let (_, shapes) = crate::paint::with_painter(|p| fx.draw_walkers(p, CELL, 16, to_px));
        assert!(
            !crate::paint::painted_text(&shapes).iter().any(|t| t == "W"),
            "a finished walk must leave nothing behind"
        );
    }

    /// A bolt is gone before the next body acts. One tactical beat is
    /// `1.0 / TACTICAL_TURNS_PER_SECOND`; a streak still on screen when the
    /// next swing lands reads as two attacks at once.
    #[test]
    fn a_bolt_expires_inside_one_tactical_beat() {
        assert!(
            BOLT_SECONDS < 1.0 / feral_processes_app_core::TACTICAL_TURNS_PER_SECOND as f64,
            "a bolt outlives the beat it was fired in"
        );
    }

    /// It is drawn while it lives and not once it has expired.
    #[test]
    fn a_bolt_is_drawn_while_it_lives_and_not_after() {
        let mut fx = Fx::new();
        let cue = BoltCue {
            from: (0, 0),
            to: (3, 0),
            color: GlyphColor::Cyan,
            fx: None,
        };
        fx.begin_frame(
            0.0,
            FrameCues {
                bolts: vec![cue],
                ..Default::default()
            },
            true,
        );
        let to_px = |(x, y): (i32, i32)| (x as f32 * 16.0, y as f32 * 16.0);
        let (_, shapes) = crate::paint::with_painter(|p| fx.draw_bolts(p, to_px, 16.0));
        assert!(
            crate::paint::painted_line_count(&shapes) > 0,
            "a live bolt drew nothing"
        );

        fx.begin_frame(BOLT_SECONDS + 0.01, FrameCues::default(), true);
        let (_, shapes) = crate::paint::with_painter(|p| fx.draw_bolts(p, to_px, 16.0));
        assert_eq!(
            crate::paint::painted_line_count(&shapes),
            0,
            "an expired bolt is still drawing"
        );
    }

    /// A disabled `Fx` still drains the queue and draws nothing — the same
    /// contract every other cue has, and what stops the engine's queue
    /// sitting at its cap for a player who turned effects off.
    #[test]
    fn a_disabled_fx_draws_no_bolt() {
        let mut fx = Fx::new();
        fx.enabled = false;
        fx.begin_frame(
            0.0,
            FrameCues {
                bolts: vec![BoltCue {
                    from: (0, 0),
                    to: (2, 0),
                    color: GlyphColor::Cyan,
                    fx: None,
                }],
                ..Default::default()
            },
            true,
        );
        let to_px = |(x, y): (i32, i32)| (x as f32 * 16.0, y as f32 * 16.0);
        let (_, shapes) = crate::paint::with_painter(|p| fx.draw_bolts(p, to_px, 16.0));
        assert_eq!(crate::paint::painted_line_count(&shapes), 0);
    }

    fn with_library() -> Fx {
        let mut fx = Fx::new();
        fx.library = EffectLibrary::load_dir(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/effects"),
        );
        fx
    }

    fn blast(id: &str) -> FrameCues {
        FrameCues {
            bolts: vec![BoltCue {
                from: (0, 0),
                to: (2, 0),
                color: GlyphColor::Cyan,
                fx: Some(id.to_string()),
            }],
            ..Default::default()
        }
    }

    /// Two impacts in the same frame add, a third is held at the cap, and the
    /// energy then decays to exactly zero.
    #[test]
    fn shake_stacks_additively_caps_and_decays_to_zero() {
        let mut fx = with_library();
        let mut cues = blast("explosion");
        cues.bolts.extend(blast("explosion").bolts);
        fx.begin_frame(0.0, cues, true);
        fx.begin_frame(BOLT_SECONDS + 0.001, FrameCues::default(), true);
        let two = fx.shake_energy;
        assert!(two > 1.0, "two blasts should stack above one: {two}");
        assert!(two <= SHAKE_MAX_ENERGY);

        let mut cues = blast("explosion");
        cues.bolts.extend(blast("explosion").bolts);
        cues.bolts.extend(blast("explosion").bolts);
        let t = BOLT_SECONDS + 0.01;
        fx.begin_frame(t, cues, true);
        fx.begin_frame(t + BOLT_SECONDS + 0.001, FrameCues::default(), true);
        assert_eq!(fx.shake_energy, SHAKE_MAX_ENERGY, "energy exceeded its cap");

        let mut now = t + BOLT_SECONDS + 0.001;
        let mut last = fx.shake_energy;
        for _ in 0..400 {
            now += 0.016;
            fx.begin_frame(now, FrameCues::default(), true);
            assert!(fx.shake_energy <= last);
            last = fx.shake_energy;
        }
        assert_eq!(fx.shake_energy, 0.0);
    }

    #[test]
    fn shake_moves_the_battle_camera_by_less_than_its_cap() {
        let mut fx = with_library();
        fx.begin_frame(0.0, blast("explosion"), true);
        fx.begin_frame(BOLT_SECONDS + 0.001, FrameCues::default(), true);
        let (ox, oy) = fx.camera_offset((0, 0), 0.0, None);
        assert!(ox != 0.0 || oy != 0.0, "a live shake moved nothing");
        assert!(ox.abs() <= SHAKE_MAX_OFFSET_TILES && oy.abs() <= SHAKE_MAX_OFFSET_TILES);
    }

    /// An explosion that is a fight's last blow lands after the fight is
    /// over; its shake must not carry onto the base map's camera.
    #[test]
    fn a_blast_in_flight_when_the_battle_ends_does_not_shake_the_base() {
        let mut fx = with_library();
        fx.begin_frame(0.0, blast("explosion"), true);
        fx.begin_frame(BOLT_SECONDS + 0.001, FrameCues::default(), false);
        assert_eq!(fx.shake_energy, 0.0);
        assert_eq!(fx.camera_offset((0, 0), 0.0, None), (0.0, 0.0));
    }

    #[test]
    fn a_routine_cue_draws_an_explosion_on_every_covered_cell() {
        let mut fx = with_library();
        fx.begin_frame(
            0.0,
            FrameCues {
                routines: vec![RoutineCue {
                    from: (0, 0),
                    aim: (3, 0),
                    cells: vec![(3, 0), (4, 0)],
                    color: GlyphColor::Cyan,
                    fx: Some("explosion".into()),
                }],
                ..Default::default()
            },
            true,
        );
        let to_px = |(x, y): (i32, i32)| (x as f32 * 16.0, y as f32 * 16.0);
        let lines_at = |fx: &mut Fx, at: f64| {
            fx.begin_frame(at, FrameCues::default(), true);
            let (_, shapes) = crate::paint::with_painter(|p| fx.draw_bolts(p, to_px, 16.0));
            crate::paint::painted_line_count(&shapes)
        };
        // Mid-flight: one streak. After arrival: a ring on each of two cells.
        let flight = lines_at(&mut fx, BOLT_SECONDS * 0.5);
        let landed = lines_at(&mut fx, BOLT_SECONDS + 0.1);
        assert!(flight >= 1);
        assert!(landed >= 2 * 16, "expected a ring per cell, got {landed}");
    }

    /// Landed line count of a `zap` cue over `cells`.
    fn zap_lines(cells: Vec<(i32, i32)>) -> usize {
        let mut fx = with_library();
        fx.begin_frame(
            0.0,
            FrameCues {
                routines: vec![RoutineCue {
                    from: (0, 0),
                    aim: (3, 0),
                    cells,
                    color: GlyphColor::Cyan,
                    fx: Some("zap".into()),
                }],
                ..Default::default()
            },
            true,
        );
        let to_px = |(x, y): (i32, i32)| (x as f32 * 16.0, y as f32 * 16.0);
        fx.begin_frame(BOLT_SECONDS + 0.01, FrameCues::default(), true);
        let (_, shapes) = crate::paint::with_painter(|p| fx.draw_bolts(p, to_px, 16.0));
        crate::paint::painted_line_count(&shapes)
    }

    #[test]
    fn a_zap_crackles_on_every_covered_cell() {
        let one = zap_lines(vec![(3, 0)]);
        assert!(one > 0);
        assert_eq!(zap_lines(vec![(3, 0), (4, 0), (5, 0)]), 3 * one);
    }

    /// Draws `id`'s cue over `cells` and returns (mid-flight, landed) rect
    /// and polygon counts.
    fn heal_counts(cells: Vec<(i32, i32)>) -> ((usize, usize), (usize, usize)) {
        let mut fx = with_library();
        fx.begin_frame(
            0.0,
            FrameCues {
                routines: vec![RoutineCue {
                    from: (0, 0),
                    aim: (3, 0),
                    cells,
                    color: GlyphColor::Cyan,
                    fx: Some("heal".into()),
                }],
                ..Default::default()
            },
            true,
        );
        let to_px = |(x, y): (i32, i32)| (x as f32 * 16.0, y as f32 * 16.0);
        let mut at = |t: f64| {
            fx.begin_frame(t, FrameCues::default(), true);
            let (_, shapes) = crate::paint::with_painter(|p| fx.draw_bolts(p, to_px, 16.0));
            let polys = shapes
                .iter()
                .filter(|cs| matches!(&cs.shape, bevy_egui::egui::Shape::Path(_)))
                .count();
            (crate::paint::painted_rect_widths(&shapes).len(), polys)
        };
        (at(BOLT_SECONDS * 0.5), at(BOLT_SECONDS + 0.05))
    }

    #[test]
    fn a_heal_flies_a_ball_then_flashes_every_covered_cell() {
        let ((rects, balls), (landed_rects, _)) = heal_counts(vec![(3, 0), (4, 0), (5, 0)]);
        assert_eq!(rects, 0, "no flash before the ball lands");
        assert_eq!(balls, 1);
        assert_eq!(landed_rects, 3);
    }

    #[test]
    fn a_single_cell_heal_flashes_the_aim() {
        let (_, (landed_rects, _)) = heal_counts(vec![]);
        assert_eq!(landed_rects, 1);
    }

    /// A disabled `Fx` draws no pulses or explosion and produces no shake.
    #[test]
    fn a_disabled_fx_draws_no_effect_and_no_shake() {
        let mut fx = with_library();
        fx.enabled = false;
        for id in ["laser_pulse", "explosion"] {
            fx.begin_frame(0.0, blast(id), true);
            fx.begin_frame(BOLT_SECONDS + 0.01, FrameCues::default(), true);
            let to_px = |(x, y): (i32, i32)| (x as f32 * 16.0, y as f32 * 16.0);
            let (_, shapes) = crate::paint::with_painter(|p| fx.draw_bolts(p, to_px, 16.0));
            assert_eq!(crate::paint::painted_line_count(&shapes), 0);
        }
        assert_eq!(fx.shake_energy, 0.0);
        assert_eq!(fx.camera_offset((0, 0), 0.0, None), (0.0, 0.0));
    }

    /// Both alert kinds start the log pane's border flash. A tantrum is the
    /// second thing the base says that the player must not miss, and
    /// `MessageKind::Tantrum` exists precisely so it does not have to
    /// masquerade as a sweep to get it.
    #[test]
    fn a_tantrum_line_flashes_the_log_pane() {
        fn line(kind: MessageKind) -> LogLine {
            LogLine {
                kind,
                source: feral_processes_engine::MessageSource::Base,
                text: format!("{kind:?}"),
                outcome: None,
                impact: None,
            }
        }

        for kind in [MessageKind::Raid, MessageKind::Tantrum] {
            let mut fx = Fx::new();
            fx.begin_frame(1.0, FrameCues::default(), false);
            fx.observe_log(Some(&line(kind)));
            assert!(
                fx.log_flash_until > fx.now,
                "{kind:?} must start the pane's flash"
            );
        }

        let mut fx = Fx::new();
        fx.begin_frame(1.0, FrameCues::default(), false);
        fx.observe_log(Some(&line(MessageKind::Info)));
        assert!(
            fx.log_flash_until <= fx.now,
            "and ordinary chatter must not"
        );
    }

    fn at(fx: &mut Fx, now: f64) {
        fx.begin_frame(now, FrameCues::default(), false);
    }

    /// A wrap is a round *rising* within one fight: the first round a fight
    /// is seen at is its opening, and a later fight starting over at 1 is
    /// not a wrap either.
    #[test]
    fn only_a_rising_round_within_one_fight_is_a_wrap() {
        let mut fx = Fx::new();
        at(&mut fx, 0.0);
        assert!(!fx.observe_round(None));
        assert!(!fx.observe_round(Some(1)), "a fight's opening round");
        assert!(!fx.observe_round(Some(1)), "a round held across frames");
        assert!(fx.observe_round(Some(2)), "the wrap");
        assert!(!fx.observe_round(Some(2)), "heard once, not every frame");
        assert!(!fx.observe_round(None));
        assert!(!fx.observe_round(Some(1)), "the next fight's opening");
    }

    #[test]
    fn a_wrap_raises_a_banner_that_fades_and_retires() {
        let mut fx = Fx::new();
        at(&mut fx, 0.0);
        fx.observe_round(Some(1));
        assert!(fx.round_banner().is_none());

        at(&mut fx, 1.0);
        fx.observe_round(Some(2));
        let (round, alpha) = fx.round_banner().expect("a wrap raises the banner");
        assert_eq!(round, 2);
        assert!(alpha > 0.9, "{alpha}");

        at(&mut fx, 1.0 + ROUND_BANNER_SECONDS * 0.8);
        let (_, later) = fx.round_banner().expect("still fading");
        assert!(later < alpha, "{later} should have faded from {alpha}");

        at(&mut fx, 1.0 + ROUND_BANNER_SECONDS);
        assert!(fx.round_banner().is_none(), "retired on its own timer");
    }

    /// The banner goes with the fight, and never shows at all with effects
    /// off — though the wrap is still reported, since sound is not a visual
    /// effect.
    #[test]
    fn a_banner_leaves_with_its_fight_and_stays_off_with_effects() {
        let mut fx = Fx::new();
        at(&mut fx, 0.0);
        fx.observe_round(Some(1));
        fx.observe_round(Some(2));
        fx.observe_round(None);
        assert!(fx.round_banner().is_none());

        let mut quiet = Fx::new();
        quiet.enabled = false;
        at(&mut quiet, 0.0);
        quiet.observe_round(Some(1));
        assert!(quiet.observe_round(Some(2)), "the sound still plays");
        assert!(quiet.round_banner().is_none());
    }

    #[test]
    fn a_raid_flash_is_drawn_from_the_palette() {
        assert_eq!(effect_color(EffectKind::Hit), palette::THREAT);
        assert_eq!(effect_color(EffectKind::Brawl), palette::THREAT);
        assert_ne!(effect_color(EffectKind::Deflected), palette::THREAT);
        assert_ne!(effect_color(EffectKind::Destroyed), palette::THREAT);
        assert_eq!(effect_color(EffectKind::Destroyed), palette::EMPHASIS);
    }

    use super::*;

    /// The ghost trail used to track exactly two HP scalars. A roster has
    /// one bar per enemy group and per party slot, and they must animate
    /// independently — a shared ghost would make every bar jump whenever
    /// any one of them changed.
    #[test]
    fn bar_ghosts_are_tracked_independently_per_key() {
        let mut fx = Fx::new();
        fx.bar_ghost(group(1), 100, 0.0);
        fx.bar_ghost(group(2), 100, 0.0);

        let a = fx.bar_ghost(group(1), 40, 0.016);
        let b = fx.bar_ghost(group(2), 100, 0.016);

        assert!(
            b.ghost > a.ghost,
            "the two ghosts must not share state: {} vs {}",
            a.ghost,
            b.ghost
        );
    }

    fn group(n: u32) -> ImpactTarget {
        ImpactTarget::Group(feral_processes_engine::battle::GroupId::new(n))
    }

    /// The party and the groups share one tracker map. A group-id counter
    /// has no upper bound, so a fixed offset between the two ranges would
    /// eventually meet; the key is the target itself.
    #[test]
    fn a_high_group_id_does_not_share_a_ghost_with_a_party_slot() {
        let mut fx = Fx::new();
        fx.bar_ghost(ImpactTarget::Party(0), 100, 0.0);
        fx.bar_ghost(group(1000), 100, 0.0);

        let hostile = fx.bar_ghost(group(1000), 10, 0.1);
        assert!(hostile.ghost < 100.0, "the group's own ghost drains");
        let party = fx.bar_ghost(ImpactTarget::Party(0), 50, 0.0);

        assert_eq!(
            party.ghost, 100.0,
            "the party slot inherited the group's drain"
        );
    }

    fn battle_at(fx: &mut Fx, now: f64) {
        fx.begin_frame(now, FrameCues::default(), true);
    }

    const A_ROW: Rect = Rect::new(10.0, 100.0, 400.0, 20.0);

    fn dmg(target: ImpactTarget, n: i32) -> BattleImpact {
        BattleImpact {
            target,
            kind: ImpactKind::Damage(n),
        }
    }

    /// Several blows land on one row inside one float's lifetime; at one x
    /// they would print over each other.
    #[test]
    fn two_impacts_on_one_row_do_not_share_an_x() {
        let mut fx = Fx::new();
        battle_at(&mut fx, 0.0);
        fx.impact(dmg(group(1), 4), A_ROW);
        battle_at(&mut fx, FLOAT_SECONDS * 0.5);
        fx.impact(dmg(group(1), 5), A_ROW);
        let anchors = fx.float_anchors();
        assert_eq!(anchors.len(), 2);
        assert!(
            (anchors[0].1 - anchors[1].1).abs() >= IMPACT_STAGGER_PX,
            "{anchors:?}"
        );
    }

    #[test]
    fn a_stagger_resets_once_the_earlier_float_is_gone() {
        let mut fx = Fx::new();
        battle_at(&mut fx, 0.0);
        fx.impact(dmg(group(1), 4), A_ROW);
        battle_at(&mut fx, FLOAT_SECONDS + 0.1);
        fx.impact(dmg(group(1), 5), A_ROW);
        let anchors = fx.float_anchors();
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].1, A_ROW.x + A_ROW.w / 2.0);
    }

    #[test]
    fn a_row_flash_fades_to_nothing_after_its_duration() {
        let mut fx = Fx::new();
        battle_at(&mut fx, 0.0);
        fx.impact(dmg(group(1), 4), A_ROW);
        let (_, alpha) = fx.row_flash(group(1)).expect("a hit flashes its row");
        assert!(alpha > 0.0);
        assert!(fx.row_flash(group(2)).is_none(), "other rows are untouched");
        battle_at(&mut fx, ROW_FLASH_SECONDS + 0.01);
        assert!(fx.row_flash(group(1)).is_none());
    }

    #[test]
    fn crit_and_heal_flash_but_miss_fumble_and_kill_do_not() {
        for (kind, flashes) in [
            (ImpactKind::Crit(9), true),
            (ImpactKind::Heal(3), true),
            (ImpactKind::Miss, false),
            (ImpactKind::Fumble, false),
            (ImpactKind::Kill, false),
        ] {
            let mut fx = Fx::new();
            battle_at(&mut fx, 0.0);
            fx.impact(
                BattleImpact {
                    target: group(1),
                    kind,
                },
                A_ROW,
            );
            assert_eq!(fx.row_flash(group(1)).is_some(), flashes, "{kind:?}");
            assert_eq!(fx.float_anchors().len(), 1, "{kind:?} still draws a mark");
        }
    }

    /// The blow that killed a group can reach the screen after the group's
    /// row has left the view (and, for a companion, before the swing's own
    /// damage), so any impact whose row is gone lands where the row was.
    #[test]
    fn an_impact_on_a_vanished_row_lands_at_its_last_known_rect() {
        let mut fx = Fx::new();
        battle_at(&mut fx, 0.0);
        fx.note_row(group(3), A_ROW);
        // The row is no longer noted by the view this frame; the memory stays.
        fx.land(BattleImpact {
            target: group(3),
            kind: ImpactKind::Kill,
        });
        fx.land(dmg(group(3), 7));
        let anchors = fx.float_anchors();
        assert_eq!(anchors.len(), 2);
        let centre = A_ROW.x + A_ROW.w / 2.0;
        assert_eq!(anchors[0].1, centre);
        assert_eq!(anchors[1].1, centre + IMPACT_STAGGER_PX);
        for (_, _, y) in anchors {
            assert_eq!(y, A_ROW.y + A_ROW.h / 2.0);
        }
    }

    #[test]
    fn an_impact_on_a_row_never_drawn_is_dropped() {
        let mut fx = Fx::new();
        battle_at(&mut fx, 0.0);
        fx.land(dmg(group(3), 7));
        assert!(fx.float_anchors().is_empty());
        assert_eq!(fx.portrait_reaction(), PortraitReaction::default());
    }

    #[test]
    fn the_picture_nudges_on_a_blow_and_stamps_ko_on_a_kill() {
        let mut fx = Fx::new();
        battle_at(&mut fx, 0.0);
        assert_eq!(fx.portrait_reaction(), PortraitReaction::default());
        fx.impact(dmg(group(1), 4), A_ROW);
        battle_at(&mut fx, PORTRAIT_NUDGE_SECONDS * 0.1);
        let hit = fx.portrait_reaction();
        assert!(hit.flash.is_some() && hit.nudge_x != 0.0, "{hit:?}");
        assert!(hit.ko.is_none());
        fx.impact(
            BattleImpact {
                target: group(1),
                kind: ImpactKind::Kill,
            },
            A_ROW,
        );
        assert!(fx.portrait_reaction().ko.is_some());
        battle_at(&mut fx, PORTRAIT_KO_SECONDS + 1.0);
        assert_eq!(fx.portrait_reaction(), PortraitReaction::default());
    }

    #[test]
    fn effects_off_draws_no_impact_cues() {
        let mut fx = Fx::new();
        fx.enabled = false;
        battle_at(&mut fx, 0.0);
        fx.impact(dmg(group(1), 4), A_ROW);
        assert!(fx.float_anchors().is_empty());
        assert!(fx.row_flash(group(1)).is_none());
        assert_eq!(fx.portrait_reaction(), PortraitReaction::default());
    }

    /// Entering a battle must not animate a drain from zero, and group
    /// indices are reused between fights — so a stale tracker would show
    /// the next battle draining from the last one's numbers.
    #[test]
    fn a_bars_first_frame_seeds_its_ghost_rather_than_draining_from_zero() {
        let mut fx = Fx::new();
        let first = fx.bar_ghost(group(7), 250, 0.016);
        assert_eq!(first.ghost, 250.0);

        fx.bar_ghost(group(7), 10, 0.016);
        fx.clear_bars();
        let fresh = fx.bar_ghost(group(7), 250, 0.016);
        assert_eq!(
            fresh.ghost, 250.0,
            "a cleared tracker must seed fresh, not ease down from the old fight"
        );
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
        let cam = camera_step(0.0, 1.0, 0.016, Some(CAMERA_MAX_LAG));
        assert!(cam > 0.0, "the camera should move");
        assert!(cam < 1.0, "the camera should lag, not snap");
    }

    /// A per-frame fraction would glide for a different *duration* on a
    /// 144Hz machine than on a 30Hz one. The exponential form must not.
    #[test]
    fn camera_step_covers_the_same_ground_however_the_frames_are_sliced() {
        let coarse = camera_step(0.0, 1.0, 0.1, Some(CAMERA_MAX_LAG));
        let mut fine = 0.0;
        for _ in 0..10 {
            fine = camera_step(fine, 1.0, 0.01, Some(CAMERA_MAX_LAG));
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
            cam = camera_step(cam, 5.0, 0.016, Some(CAMERA_MAX_LAG));
        }
        assert!((cam - 5.0).abs() < 1e-3, "settled at {cam}");
    }

    /// The tile loop draws exactly one extra ring to cover the offset, so a
    /// camera further behind than that would expose blank space at the
    /// trailing edge. Holding a movement key is what would otherwise do it.
    #[test]
    fn camera_step_never_falls_further_behind_than_the_extra_ring_covers() {
        assert_eq!(
            camera_step(0.0, 40.0, 0.016, Some(CAMERA_MAX_LAG)).max(0.0),
            40.0 - CAMERA_MAX_LAG
        );
        assert_eq!(
            camera_step(0.0, -40.0, 0.016, Some(CAMERA_MAX_LAG)),
            -40.0 + CAMERA_MAX_LAG
        );
    }

    /// The hold reads the blow and the pan lands, both inside the hand-over.
    ///
    /// **Three figures that have to be read together and live in two
    /// crates**, which is exactly the shape that drifts: the hold is a
    /// fraction of `TACTICAL_HANDOVER_SECONDS` in app-core, the flash it has
    /// to outlast is `HIT_FLASH_SECONDS` here, and the pan's length is not a
    /// constant at all — it falls out of `CAMERA_DECAY`. So the pan is
    /// *measured*, by easing the real function across the widest board the
    /// game ships in the time the hold leaves over, rather than compared
    /// against a number somebody worked out once.
    ///
    /// Raising the dwell fraction without raising the hand-over fails this,
    /// which is the whole point of having it: the next body would begin its
    /// turn with the camera still travelling. That the hold outlasts the
    /// flash is a `const` assertion beside the constant instead.
    #[test]
    fn the_hold_and_the_pan_both_fit_inside_the_hand_over() {
        use feral_processes_app_core::TACTICAL_HANDOVER_SECONDS;
        use feral_processes_engine::tuning::TACTICAL_BOARD_LARGE;

        let left = TACTICAL_HANDOVER_SECONDS as f64 - CAMERA_DWELL_SECONDS;
        assert!(left > 0.0, "the hold outlasts the hand-over that covers it");

        // The worst pan there is: corner to corner of the largest board, at
        // the frame rate, with no lag clamp — the battle map's own call.
        let target = TACTICAL_BOARD_LARGE as f32;
        let dt = 1.0 / 60.0;
        let mut cam = 0.0;
        let mut elapsed = 0.0;
        while elapsed < left {
            cam = camera_step(cam, target, dt, None);
            elapsed += dt as f64;
        }
        assert!(
            target - cam < 0.5,
            "the pan is still {:.2} tiles out when the next body acts",
            target - cam
        );
    }

    /// ...and a caller with no trailing edge to protect gets a real glide.
    ///
    /// The battle map is that caller: it walks the whole board and skips
    /// what falls outside the pane, so there is no ring to run out of. Under
    /// the clamp a body on the far side of the board is one tile away after
    /// a single frame, which is a cut rather than a pan — the point of the
    /// `None` arm, and the thing a clamp left in by habit would undo
    /// silently.
    #[test]
    fn camera_step_glides_when_no_lag_is_given() {
        let unclamped = camera_step(0.0, 40.0, 0.016, None);
        assert!(
            unclamped < 40.0 - CAMERA_MAX_LAG,
            "an unclamped step arrived within the clamp's own distance: {unclamped}"
        );
        assert!(unclamped > 0.0, "an unclamped step did not move at all");
    }

    #[test]
    fn the_cameras_first_frame_seeds_at_the_player_rather_than_panning_in() {
        let mut fx = Fx::new();
        assert_eq!(
            fx.camera_offset((120, -80), 0.016, Some(CAMERA_MAX_LAG)),
            (0.0, 0.0)
        );
    }

    #[test]
    fn the_camera_trails_the_player_after_a_step_and_then_catches_up() {
        let mut fx = Fx::new();
        fx.camera_offset((0, 0), 0.016, Some(CAMERA_MAX_LAG));

        let (ox, oy) = fx.camera_offset((1, 0), 0.016, Some(CAMERA_MAX_LAG));
        assert!(ox < 0.0, "the camera should still be behind: {ox}");
        assert_eq!(oy, 0.0, "an axis that didn't move must not drift");

        for _ in 0..200 {
            fx.camera_offset((1, 0), 0.016, Some(CAMERA_MAX_LAG));
        }
        let (ox, _) = fx.camera_offset((1, 0), 0.016, Some(CAMERA_MAX_LAG));
        assert!(ox.abs() < 1e-3, "the camera should settle: {ox}");
    }

    /// Effects-off means an instant camera, the same way it means an instant
    /// HP bar — and the next frame must not then glide from a stale reading.
    #[test]
    fn the_camera_snaps_while_effects_are_disabled() {
        let mut fx = Fx::new();
        fx.enabled = false;
        fx.camera_offset((0, 0), 0.016, Some(CAMERA_MAX_LAG));
        assert_eq!(
            fx.camera_offset((30, 30), 0.016, Some(CAMERA_MAX_LAG)),
            (0.0, 0.0)
        );

        fx.enabled = true;
        assert_eq!(
            fx.camera_offset((30, 30), 0.016, Some(CAMERA_MAX_LAG)),
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

    /// The battle camera aims one cell in from a squad's own anchor, not at
    /// the corner an ordinary body would centre on — `footprint_center`'s
    /// own rule for the shipped 2x2 formation.
    #[test]
    fn battle_center_aims_at_a_squads_own_footprint_not_its_anchor() {
        let mut fx = Fx::new();
        let entity = Entity::from_raw_u32(1).unwrap();
        let center = fx
            .battle_center(Some((entity, (4, 4), 2)))
            .expect("a body is acting");
        assert_eq!(
            center,
            (5.0, 5.0),
            "a 2x2 footprint anchored at (4,4) must not centre on the bare anchor"
        );
    }

    /// A body that has not moved is drawn on its own cell.
    #[test]
    fn a_body_at_rest_is_drawn_on_its_cell() {
        let mut fx = Fx::new();
        let entity = Entity::from_raw_u32(1).unwrap();
        battle_at(&mut fx, 1.0);
        assert_eq!(fx.slid_cell(entity, (3, 4)), (3.0, 4.0));
    }

    /// A one-cell step glides across the step's own duration rather than
    /// landing in one frame: halfway through it is halfway there, and at
    /// the end it is on the new cell.
    #[test]
    fn a_one_cell_step_glides_over_one_step() {
        let mut fx = Fx::new();
        let entity = Entity::from_raw_u32(1).unwrap();
        battle_at(&mut fx, 1.0);
        fx.slid_cell(entity, (3, 4));
        battle_at(&mut fx, 1.0 + 1.0 / 60.0);
        let first = fx.slid_cell(entity, (4, 4));
        assert_eq!(first, (3.0, 4.0), "the slide starts where the body stood");
        battle_at(&mut fx, 1.0 + 1.0 / 60.0 + SLIDE_SECONDS / 2.0);
        let (x, y) = fx.slid_cell(entity, (4, 4));
        assert!((x - 3.5).abs() < 1e-3 && y == 4.0, "halfway: {x},{y}");
        battle_at(&mut fx, 1.0 + 1.0 / 60.0 + SLIDE_SECONDS);
        assert_eq!(fx.slid_cell(entity, (4, 4)), (4.0, 4.0));
    }

    /// A held key steps the player every `REPEAT_INTERVAL` once it has
    /// been held `REPEAT_DELAY` — faster than a wild walk — and each glide
    /// must still land before the next step, or the body is drawn behind its
    /// cell and a blow struck after the run lands ahead of it. Timed off the
    /// real key repeat, press then delay then interval.
    #[test]
    fn a_held_key_walk_lands_each_glide_before_the_next_step() {
        use crate::keys::{REPEAT_DELAY, REPEAT_INTERVAL};
        let mut fx = Fx::new();
        let entity = Entity::from_raw_u32(1).unwrap();
        battle_at(&mut fx, 0.5);
        fx.slid_cell(entity, (0, 0));
        let steps: Vec<f64> = (0..8)
            .map(|n| match n {
                0 => 1.0,
                n => 1.0 + REPEAT_DELAY + REPEAT_INTERVAL * f64::from(n - 1),
            })
            .collect();
        for (i, pair) in steps.windows(2).enumerate() {
            let x = i as i32 + 1;
            battle_at(&mut fx, pair[0]);
            fx.slid_cell(entity, (x, 0));
            battle_at(&mut fx, pair[1] - 1e-6);
            let (drawn, _) = fx.slid_cell(entity, (x, 0));
            // Step 2 is the one glide paced to the repeat *delay* before the
            // interval is learnt, so it is still short of its cell when step
            // 3 comes — by under half a cell. Every step after it lands.
            let behind = x as f32 - drawn;
            let bound = if x == 2 { 0.5 } else { 1e-3 };
            assert!(
                behind < bound,
                "step {x}: drawn at {drawn}, behind its cell"
            );
        }
    }

    /// A wild walk is paced against frames, so at a refresh rate that does
    /// not divide a walk step its gaps alternate a frame short and a frame
    /// long. A glide shortened to the short gap lands a frame early and the
    /// body stands still for a frame every other step — so it must move on
    /// every frame of the walk, at every common rate.
    #[test]
    fn a_frame_quantised_walk_never_stalls_a_frame() {
        let step = 1.0 / f64::from(feral_processes_app_core::TACTICAL_STEPS_PER_SECOND);
        for hz in [60.0, 75.0, 144.0, 165.0] {
            let mut fx = Fx::new();
            let entity = Entity::from_raw_u32(1).unwrap();
            let dt = 1.0 / hz;
            battle_at(&mut fx, 1.0);
            fx.slid_cell(entity, (0, 0));
            let (mut now, mut carry, mut cell) = (1.0, 0.0, 0);
            let mut last = 0.0;
            for frame in 0..(hz as usize * 2) {
                now += dt;
                carry += dt;
                if carry >= step {
                    carry -= step;
                    cell += 1;
                }
                battle_at(&mut fx, now);
                let (x, _) = fx.slid_cell(entity, (cell, 0));
                if cell > 1 {
                    assert!(
                        x > last,
                        "{hz} Hz, frame {frame}: the body stood still at {x}"
                    );
                }
                last = x;
            }
        }
    }

    /// Steps faster than any glide never let the body trail: the one glide
    /// paced before the speed was learnt leaves it under two cells behind,
    /// and from then on it is never more than the one cell it is gliding.
    #[test]
    fn fast_steps_never_leave_a_body_trailing() {
        let mut fx = Fx::new();
        let entity = Entity::from_raw_u32(1).unwrap();
        battle_at(&mut fx, 1.0);
        fx.slid_cell(entity, (0, 0));
        for x in 1..=20 {
            battle_at(&mut fx, 1.0 + 0.01 * f64::from(x));
            let (drawn, _) = fx.slid_cell(entity, (x, 0));
            let bound = if x <= 2 { 2.0 } else { 1.0 };
            assert!(
                x as f32 - drawn <= bound + 1e-4,
                "step {x}: drawn at {drawn}"
            );
        }
    }

    /// A jump of more than one cell — a blink, a body placed fresh — is a
    /// cut, not a dash across the board.
    #[test]
    fn a_jump_of_more_than_one_cell_snaps() {
        let mut fx = Fx::new();
        let entity = Entity::from_raw_u32(1).unwrap();
        battle_at(&mut fx, 1.0);
        fx.slid_cell(entity, (3, 4));
        battle_at(&mut fx, 1.1);
        assert_eq!(fx.slid_cell(entity, (6, 4)), (6.0, 4.0));
    }

    /// Effects off means an instant body, as it means an instant camera.
    #[test]
    fn effects_off_draws_a_step_on_its_new_cell_at_once() {
        let mut fx = Fx::new();
        fx.enabled = false;
        let entity = Entity::from_raw_u32(1).unwrap();
        battle_at(&mut fx, 1.0);
        fx.slid_cell(entity, (3, 4));
        battle_at(&mut fx, 1.1);
        assert_eq!(fx.slid_cell(entity, (4, 4)), (4.0, 4.0));
    }

    /// **The stutter this exists to fix.** A body walking one cell a step
    /// used to jump, and the camera eased after every jump — a surge at the
    /// start of each step and a near-stop at its end. Following the slide,
    /// the camera's per-frame travel through the middle of a walk is
    /// steady: no frame moves it much more or less than the walk's own
    /// speed.
    #[test]
    fn the_battle_camera_pans_steadily_through_a_walk() {
        let mut fx = Fx::new();
        let entity = Entity::from_raw_u32(1).unwrap();
        let dt = 1.0 / 60.0;
        let step = 1.0 / f64::from(feral_processes_app_core::TACTICAL_STEPS_PER_SECOND);
        let mut now = 1.0;
        let mut last = None;
        let mut moves = Vec::new();
        while now < 1.0 + step * 10.0 {
            battle_at(&mut fx, now);
            let walked = ((now - 1.0) / step).floor() as i32;
            let target = fx
                .battle_center(Some((entity, (walked, 0), 1)))
                .expect("a body is acting");
            let anchor = (target.0.round() as i32, target.1.round() as i32);
            let (ox, _) = fx.camera_offset_toward(target, anchor, dt as f32, None);
            let cam = anchor.0 as f32 + ox;
            if let Some(prev) = last {
                moves.push(cam - prev);
            }
            last = Some(cam);
            now += dt;
        }
        let per_frame = dt as f32 / step as f32;
        // The second half, after the camera has caught the walk's pace.
        for (i, m) in moves[moves.len() / 2..].iter().enumerate() {
            assert!(
                (m - per_frame).abs() < per_frame * 0.25,
                "frame {i}: the camera moved {m} where the walk moves {per_frame}"
            );
        }
    }

    /// A body with no `Squad` reads as footprint 1, and the camera must
    /// still aim at its own anchor — the invariant every other reader of
    /// footprint holds, held here too.
    #[test]
    fn battle_center_is_the_bare_anchor_without_a_squad() {
        let mut fx = Fx::new();
        let entity = Entity::from_raw_u32(1).unwrap();
        let center = fx
            .battle_center(Some((entity, (4, 4), 1)))
            .expect("a body is acting");
        assert_eq!(center, (4.0, 4.0));
    }

    /// **Sample-and-hold is the rattle**, and this is the half a smooth curve
    /// would fail: the offset is one value for the whole of a step and a
    /// different one in the next. Asserted across several steps rather than
    /// one boundary, so a curve that happens to be flat somewhere cannot pass.
    #[test]
    fn a_strain_offset_holds_for_a_step_and_then_snaps() {
        let key = 7u64;
        let step = 1.0 / STRAIN_STEP_HZ;
        for n in 0..12 {
            let base = n as f64 * step;
            let early = strain_offset(base + step * 0.05, key);
            let late = strain_offset(base + step * 0.95, key);
            assert_eq!(
                early, late,
                "the offset must not move inside step {n}: {early:?} then {late:?}"
            );
            let next = strain_offset(base + step * 1.05, key);
            assert_ne!(
                early, next,
                "step {n} and the one after it must not land in the same place"
            );
        }
    }

    /// The raw curve is normalised, which is what lets `strain_jitter` treat
    /// the caller's margin as the only scale.
    #[test]
    fn a_strain_offset_is_normalised() {
        for key in 0..PHASE_KEYS {
            for n in 0..400 {
                let (dx, dy) = strain_offset(n as f64 / STRAIN_STEP_HZ, key);
                assert!(
                    dx.abs() <= 1.0 + 1e-4 && dy.abs() <= 1.0 + 1e-4,
                    "key {key} step {n} is not normalised: {dx}, {dy}"
                );
            }
        }
    }

    /// **The margin is the budget**, at every step of the zoom ladder: the
    /// ink rattles inside the room it already has and never reaches the pin
    /// marks on the tile-edge ring. `map_cell` gives a margin of 2px at
    /// zoom 1 and 2px more at every step above it, so the tightest zoom is
    /// the case that has to hold.
    #[test]
    fn a_strained_body_rattles_inside_the_margin_it_was_given() {
        let mut fx = Fx::new();
        let entity = Entity::from_raw_u32(11).unwrap();
        for zoom in 1..=4 {
            let margin =
                (crate::text::map_cell(zoom).0 - crate::text::map_cell(zoom).1 as f32) / 2.0;
            for n in 0..200 {
                fx.now = n as f64 / STRAIN_STEP_HZ;
                let (dx, dy) = fx.strain_jitter(entity, margin);
                assert!(
                    dx.abs() < margin && dy.abs() < margin,
                    "zoom {zoom} step {n} reached the pen: {dx}, {dy} against {margin}"
                );
            }
        }
    }

    /// **The two axes are not one signal scaled twice.** Summed from the same
    /// frequencies, the body would slide along a diagonal and read as drifting
    /// rather than as rattling — so the axes must disagree, and not merely at
    /// one lucky instant.
    #[test]
    fn a_strain_offset_does_not_travel_on_a_diagonal() {
        let key = 3u64;
        let apart = (0..200)
            .filter(|n| {
                let (dx, dy) = strain_offset(*n as f64 / STRAIN_STEP_HZ, key);
                (dx - dy).abs() > 0.2
            })
            .count();
        assert!(
            apart > 150,
            "the two axes track each other: only {apart} of 200 steps differ"
        );
    }

    /// Two subjects in two Stations rattle out of step — `staffed_bob`'s own
    /// rule, so two pens read as two programs under strain rather than as one
    /// animation drawn twice.
    #[test]
    fn two_strained_subjects_rattle_out_of_step() {
        let together = (0..200)
            .filter(|n| {
                let t = *n as f64 / STRAIN_STEP_HZ;
                strain_offset(t, 1) == strain_offset(t, 2)
            })
            .count();
        assert_eq!(
            together, 0,
            "two keys landed on the same offset {together} times out of 200"
        );
    }

    /// Animations off is still off — `staffed_bob`'s and `stranded_blink`'s
    /// own rule, which `strain_jitter` joins rather than restates.
    #[test]
    fn a_disabled_fx_strains_nobody() {
        let mut fx = Fx::new();
        fx.enabled = false;
        fx.now = 4.2;
        assert_eq!(
            fx.strain_jitter(Entity::from_raw_u32(9).unwrap(), 8.0),
            (0.0, 0.0)
        );
    }

    /// The mark bobs *up* out of its rest position and never below it. Its
    /// rest position is held off the tile's bottom edge by an inset that
    /// `render/base.rs` documents as load-bearing, and a negative offset
    /// would spend that inset.
    /// The build caret's bob straddles its rest position instead of sitting
    /// on top of it.
    ///
    /// Asserted as a **mean of zero across half a period**, which is what
    /// "centred" actually means and is phase-free: a raised cosine and the
    /// same curve half a period later sum to exactly the amplitude whatever
    /// the entity's phase, so this cannot pass by picking a lucky key. A
    /// range check alone would pass against a curve biased anywhere inside
    /// the band.
    #[test]
    fn the_centred_bob_swings_evenly_either_side_of_rest() {
        let fx = Fx::new();
        let half = STAFFED_BOB_PX / 2.0;
        for key in 0..64u64 {
            let at_rest = staffed_bob_offset(0.0, key) - half;
            let half_period = staffed_bob_offset(0.5 / STAFFED_BOB_HZ, key) - half;
            assert!(
                (at_rest + half_period).abs() < 1e-4,
                "the caret leans one way: {at_rest} and {half_period}"
            );
            assert!(
                (-half..=half).contains(&at_rest),
                "offset {at_rest} escaped the band"
            );
        }
        let entity = Entity::from_raw_u32(7).unwrap();
        assert_eq!(
            fx.centred_bob(entity),
            staffed_bob_offset(fx.now, entity.to_bits()) - half,
            "and the accessor is that curve, not a second one"
        );
    }

    #[test]
    fn a_battle_number_only_ever_rises() {
        let heights: Vec<f32> = (0..=10)
            .map(|i| MarkKind::Damage(1).height(i as f32 / 10.0))
            .collect();
        assert_eq!(heights[0], 0.0, "it starts on the body: {heights:?}");
        assert!(
            heights.windows(2).all(|w| w[1] > w[0]),
            "it must climb every frame, never bounce: {heights:?}"
        );
        assert_eq!(
            MarkKind::Damage(1).height(0.5),
            MarkKind::Heal(1).height(0.5),
            "a hit's number and a heal's are one motion"
        );
    }

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

    /// A mending body's `+` floats up and fades, and the next one starts
    /// back at rest — it never comes back *down* while it can be seen. The
    /// only frame on which the lift falls is the restart, and that frame is
    /// at rest and opaque, so what the eye reads is a new mark rather than
    /// the old one sinking.
    #[test]
    fn the_recovery_float_only_rises_and_restarts_at_rest() {
        for key in 0..7 {
            let samples: Vec<(f32, f32)> = (0..400)
                .map(|i| recovery_float(i as f64 * 0.01, key))
                .collect();
            for pair in samples.windows(2) {
                let ((lift0, alpha0), (lift1, alpha1)) = (pair[0], pair[1]);
                assert!(
                    (0.0..=RECOVERY_FLOAT_RISE).contains(&lift1),
                    "lift {lift1} escaped the band"
                );
                if lift1 < lift0 {
                    assert!(
                        lift1 < 0.1 * RECOVERY_FLOAT_RISE && alpha1 > 0.9,
                        "the mark sank ({lift0} -> {lift1}) while visible (alpha {alpha1})"
                    );
                } else {
                    assert!(alpha1 <= alpha0, "the mark brightened as it rose");
                }
            }
            assert!(
                samples
                    .iter()
                    .any(|&(lift, _)| lift > 0.8 * RECOVERY_FLOAT_RISE),
                "the mark never floated anywhere"
            );
        }
    }

    #[test]
    fn two_patients_float_out_of_step() {
        let (a, b) = (recovery_float(0.3, 4), recovery_float(0.3, 5));
        assert!(
            (a.0 - b.0).abs() > 0.01,
            "marks moved in lockstep: {a:?} vs {b:?}"
        );
    }

    /// Animations off is a mark at rest and fully drawn — never one faded
    /// out, which would hide the recovery rather than still it.
    #[test]
    fn a_disabled_fx_holds_the_recovery_mark_at_rest() {
        let mut fx = Fx::new();
        fx.enabled = false;
        fx.now = 4.2;
        assert_eq!(
            fx.recovery_float(Entity::from_raw_u32(9).unwrap(), 20.0),
            (0.0, 1.0)
        );
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
        fx.begin_frame(0.3, FrameCues::default(), false);
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

    /// A mining cue is sound and nothing else. It rides the effects queue
    /// because that is the one channel the engine has for a tick-driven
    /// base-space event, so the thing to hold is that riding it draws
    /// nothing: a flash on a cell that is already wearing a progress bar is
    /// noise, and a crew cutting all night would strobe the base.
    ///
    /// Asserted through `tile_flash` rather than on `effect_duration`
    /// directly — zero duration is the mechanism, and what matters is that
    /// the mechanism reaches the screen.
    #[test]
    fn a_mining_cue_leaves_no_flash_on_the_cell_it_struck() {
        let mut fx = Fx::new();
        let cell = (4, -2);

        fx.begin_frame(
            1.0,
            FrameCues {
                effects: vec![VisualEffect {
                    pos: cell,
                    kind: EffectKind::Mine,
                }],
                ..Default::default()
            },
            false,
        );

        assert_eq!(
            fx.tile_flash(cell),
            None,
            "a mining cue lit the cell it was queued on"
        );
    }

    /// The control for the test above: the same call with a kind that *does*
    /// draw must light the cell. Without this, deleting the push in
    /// `begin_frame` altogether would leave that test green.
    #[test]
    fn a_hit_on_the_same_cell_does_flash() {
        let mut fx = Fx::new();
        let cell = (4, -2);

        fx.begin_frame(
            1.0,
            FrameCues {
                effects: vec![VisualEffect {
                    pos: cell,
                    kind: EffectKind::Hit,
                }],
                ..Default::default()
            },
            false,
        );

        assert!(
            fx.tile_flash(cell).is_some(),
            "the control case stopped flashing, so the Mine assertion proves nothing"
        );
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
        fx.begin_frame(0.3, FrameCues::default(), false);
        fx.enabled = false;
        assert_eq!(fx.staffed_bob(Entity::from_raw_u32(4).unwrap()), 0.0);
    }

    /// The sample grid the field tests sweep: wide enough to hold several
    /// patches at `CLOUD_SCALE`, so a census over it is a census of the
    /// map rather than of one cloud.
    fn cloud_field(now: f64) -> Vec<f32> {
        (-40..40)
            .flat_map(|y| (-40..40).map(move |x| (x, y)))
            .map(|w| cloud_shade(w, now))
            .collect()
    }

    #[test]
    fn cloud_shade_stays_inside_its_band() {
        for now in [0.0, 3.5, 41.0, 600.0] {
            for v in cloud_field(now) {
                assert!(
                    (1.0 - CLOUD_DEPTH..=1.0).contains(&v),
                    "{v} is outside the shadow band at t={now}"
                );
            }
        }
    }

    #[test]
    fn cloud_shade_is_stable_for_a_given_tile_and_instant() {
        assert_eq!(cloud_shade((3, -9), 12.5), cloud_shade((3, -9), 12.5));
    }

    #[test]
    fn cloud_shade_leaves_some_ground_clear_and_shadows_some() {
        let field = cloud_field(0.0);
        assert!(
            field.iter().any(|v| *v > 0.999),
            "no tile is in full daylight"
        );
        assert!(
            field.iter().any(|v| *v < 1.0 - CLOUD_DEPTH * 0.9),
            "no tile is deep in shadow"
        );
    }

    #[test]
    fn cloud_cover_is_a_minority_of_the_map() {
        // Both ends fail loudly on purpose: a field that shadows everything
        // reads as the map having gone dark, and one that shadows almost
        // nothing reads as the feature never having shipped.
        for now in [0.0, 77.0, 250.0] {
            let field = cloud_field(now);
            let covered = field.iter().filter(|v| **v < 0.999).count() as f32;
            let fraction = covered / field.len() as f32;
            assert!(
                (0.15..0.55).contains(&fraction),
                "{fraction} of the map is shadowed at t={now}"
            );
        }
    }

    /// The test the other five cannot fail: every one of them passes against
    /// a field that is re-hashed per tile per frame, which is TV static. What
    /// makes this a *shadow* is that the whole field translates — so sampling
    /// it downwind of where it was, later by exactly that much, must find the
    /// same ground.
    #[test]
    fn the_cloud_field_drifts_rather_than_flickering() {
        // Only whole-tile drifts can be checked, since the field is sampled
        // at integer coordinates — so the elapsed times here are the ones
        // `CLOUD_WIND` carries an exact number of tiles over. A `dt` that
        // lands mid-tile compares two points a fraction of a tile apart and
        // fails against a field that is drifting perfectly well.
        for dt in [100.0, 200.0] {
            let (dx, dy) = (CLOUD_WIND.0 as f64 * dt, CLOUD_WIND.1 as f64 * dt);
            assert!(
                (dx - dx.round()).abs() < 1e-4 && (dy - dy.round()).abs() < 1e-4,
                "{dt}s of wind is ({dx}, {dy}), which is not a whole number of tiles"
            );
            for (x, y) in [(0, 0), (11, -4), (-23, 17), (5, 31)] {
                let downwind = (x + dx.round() as i32, y + dy.round() as i32);
                let before = cloud_shade((x, y), 0.0);
                let after = cloud_shade(downwind, dt);
                assert!(
                    (before - after).abs() < 0.005,
                    "({x},{y}) read {before} but {downwind:?} read {after} {dt}s later"
                );
            }
        }
    }

    #[test]
    fn the_wind_is_off_axis_so_a_front_does_not_march_down_a_row() {
        assert!(CLOUD_WIND.0 != 0.0 && CLOUD_WIND.1 != 0.0);
        assert!(
            (CLOUD_WIND.0 - CLOUD_WIND.1).abs() > f32::EPSILON,
            "an exactly diagonal wind bands the map the way a symmetric hash does"
        );
    }

    #[test]
    fn clouds_are_off_when_effects_are() {
        let mut fx = Fx::new();
        fx.begin_frame(40.0, FrameCues::default(), false);
        fx.enabled = false;
        for w in [(0, 0), (7, 7), (-13, 2)] {
            assert_eq!(fx.cloud_shade(w), 1.0);
        }
    }

    #[test]
    fn clouds_move_with_the_frame_clock() {
        let mut fx = Fx::new();
        fx.begin_frame(0.0, FrameCues::default(), false);
        let before: Vec<f32> = cloud_field(0.0);
        fx.begin_frame(45.0, FrameCues::default(), false);
        let after: Vec<f32> = cloud_field(45.0);
        assert!(
            before != after,
            "the field must be somewhere else three quarters of a minute later"
        );
    }
}
