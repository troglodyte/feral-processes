use bevy_ecs::prelude::*;

use crate::components::{Experience, Needs, Player, Position, Stats, Structure};
use crate::progression;
use crate::resources::{DifficultyMode, GameClock, GameOver, MessageLog};

/// Gates what happens when the player's HP hits zero. Permadeath ends the
/// run (the caller is responsible for writing the history log once);
/// Forgiving mode is a soft respawn with a penalty, warping the player to
/// the nearest built structure if one exists (in place otherwise). Either
/// way, a mild XP setback applies too (see `progression::apply_setback_xp_penalty`).
pub fn death_handling_system(
    mut player_query: Query<(&mut Stats, &mut Needs, &mut Position, &mut Experience), With<Player>>,
    structure_query: Query<&Position, (With<Structure>, Without<Player>)>,
    difficulty: Res<DifficultyMode>,
    clock: Res<GameClock>,
    mut game_over: ResMut<GameOver>,
    mut log: ResMut<MessageLog>,
) {
    if game_over.reason.is_some() {
        return;
    }
    for (mut stats, mut needs, mut pos, mut exp) in &mut player_query {
        if stats.hp > 0 {
            continue;
        }
        match *difficulty {
            DifficultyMode::Permadeath => {
                log.push("FLATLINE. Your signal drops from the Grid for good.");
                game_over.reason = Some(format!("flatlined at cycle {}", clock.tick));
            }
            DifficultyMode::Forgiving => {
                stats.hp = (stats.max_hp / 2).max(1);
                needs.hunger = needs.hunger.max(40.0);
                needs.fatigue = needs.fatigue.max(40.0);
                let nearest = structure_query
                    .iter()
                    .min_by_key(|s_pos| (s_pos.x - pos.x).abs() + (s_pos.y - pos.y).abs());
                if let Some(nearest) = nearest {
                    *pos = *nearest;
                    log.push(
                        "Your connection is forcibly cut. You reboot at the nearest construction, battered but online.",
                    );
                } else {
                    log.push("Your connection is forcibly cut. You reboot, battered but online.");
                }
            }
        }
        let xp_lost = progression::apply_setback_xp_penalty(&mut exp);
        if xp_lost > 0 {
            log.push(format!("The crash costs you {xp_lost} XP."));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structures::StructureId;

    fn run_death_handling(world: &mut World) {
        let mut schedule = Schedule::default();
        schedule.add_systems(death_handling_system);
        schedule.run(world);
    }

    #[test]
    fn forgiving_death_warps_player_to_nearest_structure() {
        let mut world = World::new();
        world.insert_resource(DifficultyMode::Forgiving);
        world.insert_resource(GameClock::default());
        world.insert_resource(GameOver::default());
        world.insert_resource(MessageLog::default());

        let player = world
            .spawn((
                Player,
                Position { x: 0, y: 0 },
                Stats { hp: 0, max_hp: 10, atk: 1, def: 1 },
                Needs { hunger: 0.0, fatigue: 0.0 },
                Experience { level: 2, xp: 10, xp_to_next: 40 },
            ))
            .id();
        world.spawn((
            Structure { kind: StructureId::from("terminal") },
            Position { x: 5, y: 5 },
        ));
        world.spawn((
            Structure { kind: StructureId::from("data_cache") },
            Position { x: 1, y: 1 },
        ));

        run_death_handling(&mut world);

        let pos = *world.get::<Position>(player).unwrap();
        assert_eq!(pos, Position { x: 1, y: 1 }, "should warp to the nearest structure, not the farther one");
        let stats = world.get::<Stats>(player).unwrap();
        assert_eq!(stats.hp, 5, "forgiving death should still halve HP");
        let exp = world.get::<Experience>(player).unwrap();
        assert_eq!(exp.xp, 8, "death should also apply the mild XP setback penalty (20% of 10)");
        assert_eq!(exp.level, 2, "the XP setback should never de-level the player");
    }

    #[test]
    fn forgiving_death_stays_in_place_when_no_structures_exist() {
        let mut world = World::new();
        world.insert_resource(DifficultyMode::Forgiving);
        world.insert_resource(GameClock::default());
        world.insert_resource(GameOver::default());
        world.insert_resource(MessageLog::default());

        let player = world
            .spawn((
                Player,
                Position { x: 3, y: 4 },
                Stats { hp: 0, max_hp: 10, atk: 1, def: 1 },
                Needs { hunger: 0.0, fatigue: 0.0 },
                Experience { level: 2, xp: 10, xp_to_next: 40 },
            ))
            .id();

        run_death_handling(&mut world);

        let pos = *world.get::<Position>(player).unwrap();
        assert_eq!(pos, Position { x: 3, y: 4 }, "with no structures on the map, death should leave position untouched");
    }
}
