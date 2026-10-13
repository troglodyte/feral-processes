//! Plays `feral_processes_app_core::SoundEvent` cues through `bevy_audio`.
//! The `.wav` files are embedded in the binary (they're tiny procedural
//! blips, not sampled assets someone would want to mod) rather than loaded
//! from `assets_dir` at runtime.

use std::sync::Arc;

use bevy::audio::Volume;
use bevy::prelude::*;

use feral_processes_app_core::SoundEvent;

/// One loaded handle per `SoundEvent` variant.
///
/// The clips are registered straight into `Assets<AudioSource>` from the
/// embedded bytes rather than going through `AssetServer`, which would want
/// a path on disk. That also means nothing is in flight: the handles are
/// usable the moment this returns, so there is no early window in which a
/// cue would be dropped for not having finished loading.
#[derive(Resource)]
pub struct SoundBank {
    step: Handle<AudioSource>,
    battle_start: Handle<AudioSource>,
    hit: Handle<AudioSource>,
    crit: Handle<AudioSource>,
    miss: Handle<AudioSource>,
    flee: Handle<AudioSource>,
    victory: Handle<AudioSource>,
    defeat: Handle<AudioSource>,
    mine: Handle<AudioSource>,
    reaction: Handle<AudioSource>,
    round_end: Handle<AudioSource>,
    ambient_drone: Handle<AudioSource>,
    ambient_blips: [Handle<AudioSource>; AMBIENT_BLIPS],
}

/// How many one-shot ambient clips `Ambience` chooses between.
pub const AMBIENT_BLIPS: usize = 6;

/// The base's looping hum; at most one exists, kept in step with
/// `Ambience::is_active` by `sync_drone`.
#[derive(Component)]
pub struct AmbientDrone;

impl SoundBank {
    pub fn load(sources: &mut Assets<AudioSource>) -> Self {
        let mut add = |bytes: &'static [u8]| {
            sources.add(AudioSource {
                bytes: Arc::from(bytes),
            })
        };
        Self {
            step: add(include_bytes!("../../../assets/sounds/step.wav")),
            battle_start: add(include_bytes!("../../../assets/sounds/battle_start.wav")),
            hit: add(include_bytes!("../../../assets/sounds/hit.wav")),
            crit: add(include_bytes!("../../../assets/sounds/crit.wav")),
            miss: add(include_bytes!("../../../assets/sounds/miss.wav")),
            flee: add(include_bytes!("../../../assets/sounds/flee.wav")),
            victory: add(include_bytes!("../../../assets/sounds/victory.wav")),
            defeat: add(include_bytes!("../../../assets/sounds/defeat.wav")),
            mine: add(include_bytes!("../../../assets/sounds/mine.wav")),
            reaction: add(include_bytes!("../../../assets/sounds/reaction.wav")),
            round_end: add(include_bytes!("../../../assets/sounds/round_end.wav")),
            ambient_drone: add(include_bytes!("../../../assets/sounds/ambient_drone.wav")),
            ambient_blips: [
                add(include_bytes!("../../../assets/sounds/ambient_beep.wav")),
                add(include_bytes!("../../../assets/sounds/ambient_chirp.wav")),
                add(include_bytes!(
                    "../../../assets/sounds/ambient_warble_up.wav"
                )),
                add(include_bytes!(
                    "../../../assets/sounds/ambient_warble_down.wav"
                )),
                add(include_bytes!("../../../assets/sounds/ambient_ping.wav")),
                add(include_bytes!("../../../assets/sounds/ambient_trill.wav")),
            ],
        }
    }

    /// `volume` is the caller's current master volume (see the `[`/`]`
    /// controls in `frame`), applied as-is with no further scaling — each wav
    /// was synthesized at a level already balanced against the others, so
    /// there's no separate per-sound mix to fold in.
    ///
    /// Each cue is spawned as its own entity that despawns when the clip
    /// ends, which is what lets overlapping cues sound together instead of
    /// cutting each other off.
    pub fn play(&self, commands: &mut Commands, event: SoundEvent, volume: f32) {
        let source = match event {
            SoundEvent::Step => &self.step,
            SoundEvent::BattleStart => &self.battle_start,
            SoundEvent::Hit => &self.hit,
            SoundEvent::Crit => &self.crit,
            SoundEvent::Miss => &self.miss,
            SoundEvent::Flee => &self.flee,
            SoundEvent::Victory => &self.victory,
            SoundEvent::Defeat => &self.defeat,
            SoundEvent::Mine => &self.mine,
            SoundEvent::Reaction => &self.reaction,
            SoundEvent::RoundEnd => &self.round_end,
        };
        self.play_clip(commands, source, volume);
    }

    /// `blip` is an index from `Ambience::tick`.
    pub fn play_blip(&self, commands: &mut Commands, blip: usize, volume: f32) {
        self.play_clip(commands, &self.ambient_blips[blip], volume);
    }

    fn play_clip(&self, commands: &mut Commands, source: &Handle<AudioSource>, volume: f32) {
        commands.spawn((
            AudioPlayer::new(source.clone()),
            PlaybackSettings {
                volume: Volume::Linear(volume),
                ..PlaybackSettings::DESPAWN
            },
        ));
    }

    pub fn spawn_drone(&self, commands: &mut Commands, volume: f32) {
        commands.spawn((
            AmbientDrone,
            AudioPlayer::new(self.ambient_drone.clone()),
            PlaybackSettings {
                volume: Volume::Linear(volume),
                ..PlaybackSettings::LOOP
            },
        ));
    }
}
