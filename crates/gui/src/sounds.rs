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
    attack: Handle<AudioSource>,
    flee: Handle<AudioSource>,
    victory: Handle<AudioSource>,
    defeat: Handle<AudioSource>,
}

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
            attack: add(include_bytes!("../../../assets/sounds/attack.wav")),
            flee: add(include_bytes!("../../../assets/sounds/flee.wav")),
            victory: add(include_bytes!("../../../assets/sounds/victory.wav")),
            defeat: add(include_bytes!("../../../assets/sounds/defeat.wav")),
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
            SoundEvent::Attack => &self.attack,
            SoundEvent::Flee => &self.flee,
            SoundEvent::Victory => &self.victory,
            SoundEvent::Defeat => &self.defeat,
        };
        commands.spawn((
            AudioPlayer::new(source.clone()),
            PlaybackSettings {
                volume: Volume::Linear(volume),
                ..PlaybackSettings::DESPAWN
            },
        ));
    }
}
