use bevy_ecs::prelude::*;

use crate::components::{Needs, Player, Stats};
use crate::resources::{DifficultyMode, GameClock, GameOver, MessageLog};

/// Gates what happens when the player's HP hits zero. Permadeath ends the
/// run (the caller is responsible for writing the history log once);
/// Forgiving mode is a soft respawn-in-place with a penalty.
pub fn death_handling_system(
    mut query: Query<(&mut Stats, &mut Needs), With<Player>>,
    difficulty: Res<DifficultyMode>,
    clock: Res<GameClock>,
    mut game_over: ResMut<GameOver>,
    mut log: ResMut<MessageLog>,
) {
    if game_over.reason.is_some() {
        return;
    }
    for (mut stats, mut needs) in &mut query {
        if stats.hp > 0 {
            continue;
        }
        match *difficulty {
            DifficultyMode::Permadeath => {
                log.push("FLATLINE. Your signal drops from the Grid for good.");
                game_over.reason = Some(format!("flatlined at cycle {}", clock.tick));
            }
            DifficultyMode::Forgiving => {
                log.push("Your connection is forcibly cut. You reboot, battered but online.");
                stats.hp = (stats.max_hp / 2).max(1);
                needs.hunger = needs.hunger.max(40.0);
                needs.fatigue = needs.fatigue.max(40.0);
            }
        }
    }
}
