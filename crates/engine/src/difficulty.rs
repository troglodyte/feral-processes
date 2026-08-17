use bevy_ecs::prelude::*;

use crate::components::{Experience, Needs, Player, Position, Stats, Structure};
use crate::game::stack::StackLocale;
use crate::progression;
use crate::resources::{DifficultyMode, GameClock, GameOver, MessageLog};
use crate::tuning::{FORGIVING_RESPAWN_HP_DIVISOR, FORGIVING_RESPAWN_NEED_FLOOR};

/// Gates what happens when the player's HP hits zero. Permadeath ends the
/// run (the caller is responsible for writing the history log once);
/// Forgiving mode is a soft respawn with a penalty, warping the player to
/// the nearest built structure if one exists (in place otherwise). Either
/// way, a mild XP setback applies too (see `progression::apply_setback_xp_penalty`).
///
/// A reboot underground surfaces the party first (see `stack::surfaced`).
/// The warp target is a *surface* structure and `Position` is pinned to the
/// entrance tile while `Locale::Stack` is live, so writing one without the
/// other left the party in the maze with their way out overwritten. This is
/// the one reset that doesn't go through `Game::clear_stack`, because a
/// system has no `Game` — it shares that function's implementation instead.
pub(crate) fn death_handling_system(
    mut player_query: Query<(&mut Stats, &mut Needs, &mut Position, &mut Experience), With<Player>>,
    structure_query: Query<&Position, (With<Structure>, Without<Player>)>,
    difficulty: Res<DifficultyMode>,
    clock: Res<GameClock>,
    mut game_over: ResMut<GameOver>,
    mut log: ResMut<MessageLog>,
    mut stack_locale: StackLocale,
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
                stats.hp = (stats.max_hp / FORGIVING_RESPAWN_HP_DIVISOR).max(1);
                needs.hunger = needs.hunger.max(FORGIVING_RESPAWN_NEED_FLOOR);
                // Before the warp, not after: `Position` is the entrance
                // tile until the locale drops, and the line below overwrites
                // it. Unconditional on a structure being found — a reboot
                // with no base still has to get you out of the Stack, back
                // onto the entrance you walked in through.
                if stack_locale.is_underground() {
                    stack_locale.surface();
                    log.push("The Stack ejects your process. You come to on open grid.");
                }
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
    use crate::resources::{CurrentStack, Locale, Trace};
    use crate::structures::StructureId;

    fn run_death_handling(world: &mut World) {
        // Every death down here is a surface death, and `init_resource`
        // leaves alone anything a caller set for itself. Dying underground
        // needs a real frame to be ejected from, so it is tested against a
        // whole `Game` instead — see
        // `tests::stack::a_forgiving_death_underground_surfaces_the_party_at_their_base`.
        world.init_resource::<Locale>();
        world.init_resource::<CurrentStack>();
        world.init_resource::<Trace>();
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
                Stats {
                    hp: 0,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
                Needs { hunger: 0.0 },
                Experience {
                    level: 2,
                    xp: 10,
                    xp_to_next: 40,
                },
            ))
            .id();
        world.spawn((
            Structure {
                kind: StructureId::from("recharger_node"),
            },
            Position { x: 5, y: 5 },
        ));
        world.spawn((
            Structure {
                kind: StructureId::from("data_cache"),
            },
            Position { x: 1, y: 1 },
        ));

        run_death_handling(&mut world);

        let pos = *world.get::<Position>(player).unwrap();
        assert_eq!(
            pos,
            Position { x: 1, y: 1 },
            "should warp to the nearest structure, not the farther one"
        );
        let stats = world.get::<Stats>(player).unwrap();
        assert_eq!(stats.hp, 5, "forgiving death should still halve HP");
        let exp = world.get::<Experience>(player).unwrap();
        assert_eq!(
            exp.xp, 8,
            "death should also apply the mild XP setback penalty (20% of 10)"
        );
        assert_eq!(
            exp.level, 2,
            "the XP setback should never de-level the player"
        );
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
                Stats {
                    hp: 0,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
                Needs { hunger: 0.0 },
                Experience {
                    level: 2,
                    xp: 10,
                    xp_to_next: 40,
                },
            ))
            .id();

        run_death_handling(&mut world);

        let pos = *world.get::<Position>(player).unwrap();
        assert_eq!(
            pos,
            Position { x: 3, y: 4 },
            "with no structures on the map, death should leave position untouched"
        );
    }
}
