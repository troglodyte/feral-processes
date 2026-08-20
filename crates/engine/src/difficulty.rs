use bevy_ecs::prelude::*;

use crate::components::{
    BaseAnchor, Experience, FieldBuff, Player, Position, PowerReserve, Stats, drop_until_rest_buffs,
};
use crate::game::stack::StackLocale;
use crate::progression;
use crate::resources::{DifficultyMode, GameClock, GameOver, MessageLog, Party};
use crate::tuning::{FORGIVING_RESPAWN_HP_DIVISOR, FORGIVING_RESPAWN_NEED_FLOOR};

/// The party's field buffs as one system parameter — the same move
/// `StackLocale` makes, and for the same reason: a system that has to end a
/// loadout asks for the set rather than for a query and a resource it could
/// take one of.
///
/// `drop_until_rest` below is the system-side twin of
/// `Game::drop_until_rest_buffs_on_party`, sharing its implementation
/// through `components::drop_until_rest_buffs` — and, like that method,
/// walking the player then `Party`, which is the only set a cast can arm a
/// buff on.
#[derive(bevy_ecs::system::SystemParam)]
pub(crate) struct PartyFieldBuffs<'w, 's> {
    buffs: Query<'w, 's, &'static mut FieldBuff>,
    party: Res<'w, Party>,
}

impl PartyFieldBuffs<'_, '_> {
    /// Ends every until-rest buff on `player` and their party, returning the
    /// names of what went so the caller can announce it.
    fn drop_until_rest(&mut self, player: Entity) -> Vec<String> {
        let mut dropped = Vec::new();
        for entity in std::iter::once(player).chain(self.party.0.iter().copied()) {
            if let Ok(mut buff) = self.buffs.get_mut(entity) {
                dropped.extend(drop_until_rest_buffs(&mut buff));
            }
        }
        dropped
    }
}

/// How a death is written down: the run's verdict, the clock that stamps it
/// and the log it is announced in. One system parameter rather than three,
/// the same move `StackLocale` and `PartyFieldBuffs` make — and here the
/// grouping is what the three are *for*, since `GameClock` is read nowhere
/// else in this file and exists only to date the flatline.
#[derive(bevy_ecs::system::SystemParam)]
pub(crate) struct DeathReport<'w> {
    game_over: ResMut<'w, GameOver>,
    clock: Res<'w, GameClock>,
    log: ResMut<'w, MessageLog>,
}

impl DeathReport<'_> {
    /// Whether the run is already over, so nothing here should fire again.
    fn run_already_over(&self) -> bool {
        self.game_over.reason.is_some()
    }

    /// Ends the run, stamped with the tick it ended on.
    fn flatline(&mut self) {
        self.log
            .push("FLATLINE. Your signal drops from the Grid for good.");
        self.game_over.reason = Some(format!("flatlined at cycle {}", self.clock.tick));
    }

    fn push(&mut self, line: impl Into<String>) {
        self.log.push(line);
    }
}

/// Gates what happens when the player's HP hits zero. Permadeath ends the
/// run (the caller is responsible for writing the history log once);
/// Forgiving mode is a soft respawn with a penalty, warping the player to
/// the base's anchor if one exists (in place otherwise). Either way, a mild
/// XP setback applies too (see `progression::apply_setback_xp_penalty`).
///
/// **The anchor, and not the nearest structure.** This used to warp to the
/// nearest `Structure`'s `Position`, which was the right answer while the
/// base stood on the zone surface. Every structure is in base space now, so
/// that position is in a different coordinate space from the player's, and
/// writing one straight into the other drops the party on whatever surface
/// tile happens to carry the same numbers — with no walkability check. It is
/// the same cross-space write `Game::use_symlink` had, and it gets the same
/// answer: the anchor is the base's one presence on the zone surface, so a
/// reboot at your base means a reboot at its door.
///
/// A reboot underground surfaces the party first (see `stack::surfaced`).
/// `Position` is pinned to the entrance tile while `Locale::Stack` is live,
/// so writing one without the other left the party in the maze with their
/// way out overwritten. This is the one reset that doesn't go through
/// `Game::clear_stack`, because a system has no `Game` — it shares that
/// function's implementation instead.
pub(crate) fn death_handling_system(
    mut player_query: Query<
        (
            Entity,
            &mut Stats,
            &mut PowerReserve,
            &mut Position,
            &mut Experience,
        ),
        With<Player>,
    >,
    anchor_query: Query<&Position, (With<BaseAnchor>, Without<Player>)>,
    mut field_buffs: PartyFieldBuffs,
    difficulty: Res<DifficultyMode>,
    mut report: DeathReport,
    mut stack_locale: StackLocale,
) {
    if report.run_already_over() {
        return;
    }
    for (player, mut stats, mut needs, mut pos, mut exp) in &mut player_query {
        if stats.hp > 0 {
            continue;
        }
        match *difficulty {
            DifficultyMode::Permadeath => report.flatline(),
            DifficultyMode::Forgiving => {
                stats.hp = (stats.max_hp / FORGIVING_RESPAWN_HP_DIVISOR).max(1);
                needs.raise_to_at_least(FORGIVING_RESPAWN_NEED_FLOOR);
                // Before the warp, not after: `Position` is the entrance
                // tile until the locale drops, and the line below overwrites
                // it. Unconditional on a structure being found — a reboot
                // with no base still has to get you out of the Stack, back
                // onto the entrance you walked in through.
                if stack_locale.is_underground() {
                    stack_locale.surface();
                    report.push("The Stack ejects your process. You come to on open grid.");
                }
                if let Some(anchor) = anchor_query.iter().next() {
                    *pos = *anchor;
                    report.push(
                        "Your connection is forcibly cut. You reboot at your anchor, battered but online.",
                    );
                } else {
                    report
                        .push("Your connection is forcibly cut. You reboot, battered but online.");
                }
                // A reboot ends the expedition, so it ends the loadout that
                // was bought for it — the second of the two things that drop
                // an until-rest buff, and the reason the drop is a free
                // function on the component rather than a `Game` method: a
                // system has no `Game` to reach through, the same split this
                // arm already makes for `stack::surfaced` above. The
                // turn-counted buffs are deliberately untouched; they carry
                // their own clocks and a death is not one of them.
                for name in field_buffs.drop_until_rest(player) {
                    report.push(format!("{name} fades."));
                }
            }
        }
        let xp_lost = progression::apply_setback_xp_penalty(&mut exp);
        if xp_lost > 0 {
            report.push(format!("The crash costs you {xp_lost} XP."));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Structure;
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
        // The Forgiving arm drops the party's until-rest field buffs, so the
        // system reads `Party` now. A bare `World` has none.
        world.init_resource::<Party>();
        let mut schedule = Schedule::default();
        schedule.add_systems(death_handling_system);
        schedule.run(world);
    }

    #[test]
    fn forgiving_death_warps_player_to_the_anchor() {
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
                    mitigation: 1,
                },
                PowerReserve::new(0.0),
                Experience {
                    level: 2,
                    xp: 10,
                    xp_to_next: 40,
                },
            ))
            .id();
        // Structures stand in *base space*, so their `Position` is not a
        // tile the player can be put on — the anchor is. Both are spawned so
        // the assertion below distinguishes the two answers: a warp that
        // still read the nearest structure would land on (1, 1).
        world.spawn((
            Structure {
                kind: StructureId::from("data_cache"),
            },
            Position { x: 1, y: 1 },
        ));
        world.spawn((BaseAnchor, Position { x: 5, y: 5 }));

        run_death_handling(&mut world);

        let pos = *world.get::<Position>(player).unwrap();
        assert_eq!(
            pos,
            Position { x: 5, y: 5 },
            "a reboot lands on the anchor — a structure's Position is in another \
             coordinate space and would put the party on an arbitrary surface tile"
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
    fn forgiving_death_stays_in_place_when_no_anchor_exists() {
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
                    mitigation: 1,
                },
                PowerReserve::new(0.0),
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

    /// A reboot ends the expedition, so it ends the loadout bought for it —
    /// the second of the two things that drop an until-rest buff, and the one
    /// that has no `Game` to reach through. The turn-counted buffs are
    /// deliberately untouched: they carry their own clocks and a death is not
    /// one of them.
    #[test]
    fn a_forgiving_death_drops_until_rest_buffs_and_keeps_counted_ones() {
        use crate::components::{ActiveFieldBuff, BuffSource, FieldBuffKind};

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
                    mitigation: 1,
                },
                PowerReserve::new(0.0),
                Experience {
                    level: 2,
                    xp: 10,
                    xp_to_next: 40,
                },
                FieldBuff {
                    active: vec![
                        ActiveFieldBuff {
                            kind: FieldBuffKind::Atk,
                            name: "Overclock Single".to_string(),
                            power: 4,
                            remaining: 0,
                            interval: 1,
                            source: BuffSource::Routine,
                        },
                        ActiveFieldBuff {
                            kind: FieldBuffKind::Regen,
                            name: "Repair Loop Single".to_string(),
                            power: 2,
                            remaining: 30,
                            interval: 1,
                            source: BuffSource::Routine,
                        },
                    ],
                },
            ))
            .id();

        run_death_handling(&mut world);

        let active = &world.get::<FieldBuff>(player).unwrap().active;
        assert_eq!(active.len(), 1, "{active:?}");
        assert_eq!(active[0].kind, FieldBuffKind::Regen);
        assert_eq!(
            active[0].remaining, 30,
            "a death does not age a counted buff"
        );
    }
}
