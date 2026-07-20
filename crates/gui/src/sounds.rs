//! Plays `feral_processes_app_core::SoundEvent` cues through macroquad's
//! audio device. The `.wav` files are embedded in the binary (they're tiny
//! procedural blips, not sampled assets someone would want to mod) rather
//! than loaded from `assets_dir` at runtime.

use macroquad::audio::{self, PlaySoundParams, Sound};

use feral_processes_app_core::SoundEvent;

/// One loaded `Sound` per `SoundEvent` variant.
pub struct SoundBank {
    step: Sound,
    battle_start: Sound,
    attack: Sound,
    flee: Sound,
    victory: Sound,
    defeat: Sound,
}

impl SoundBank {
    pub async fn load() -> Self {
        async fn load(bytes: &[u8]) -> Sound {
            audio::load_sound_from_bytes(bytes)
                .await
                .expect("embedded sound effects are always valid wav data")
        }
        Self {
            step: load(include_bytes!("../../../assets/sounds/step.wav")).await,
            battle_start: load(include_bytes!("../../../assets/sounds/battle_start.wav")).await,
            attack: load(include_bytes!("../../../assets/sounds/attack.wav")).await,
            flee: load(include_bytes!("../../../assets/sounds/flee.wav")).await,
            victory: load(include_bytes!("../../../assets/sounds/victory.wav")).await,
            defeat: load(include_bytes!("../../../assets/sounds/defeat.wav")).await,
        }
    }

    pub fn play(&self, event: SoundEvent) {
        let sound = match event {
            SoundEvent::Step => &self.step,
            SoundEvent::BattleStart => &self.battle_start,
            SoundEvent::Attack => &self.attack,
            SoundEvent::Flee => &self.flee,
            SoundEvent::Victory => &self.victory,
            SoundEvent::Defeat => &self.defeat,
        };
        audio::play_sound(
            sound,
            PlaySoundParams {
                looped: false,
                volume: 0.6,
            },
        );
    }
}
