//! Rolling the fight a context would field, instead of authoring one.
//!
//! Every rule here already exists somewhere in the game and is *called*
//! rather than restated: which programs a biome holds, how big a group the
//! zone allows, what a descent costs. What this module owns is the staging —
//! stamping the tile so a biome other than the spawn point's is reachable at
//! all, and going underground for real when a depth is named.

use super::scenario::Encounter;
use crate::battle::EnemyGroup;
use crate::game::spawning::SpawnEscalation;
use crate::world::Tile;
use crate::*;

/// The groups a rolled `encounter` fields, spawned for real.
///
/// **Capped, where an authored composition is not.** A rolled pack *is* the
/// game's own fight, so it takes the game's own `group_pack` ceilings;
/// `setup::build_opponents` deliberately leaves an authored one uncapped
/// because explicit authoring is the point of a tester.
pub(crate) fn roll(game: &mut Game, encounter: &Encounter) -> Result<Vec<EnemyGroup>, String> {
    let pos = *game
        .world
        .get::<Position>(game.player_entity())
        .ok_or_else(|| "the player has no position to roll around".to_string())?;
    let (x, y) = (pos.x, pos.y);

    // The same sparse overlay `stamp_platform` writes through, so nothing new
    // is introduced to the map. Without it only the biome under the spawn
    // point is reachable, which leaves most of the encounter tables
    // unaskable.
    let biome = match encounter {
        Encounter::Field { biome }
        | Encounter::Stack { biome, .. }
        | Encounter::Lair { biome, .. } => *biome,
    };
    game.world.resource_mut::<WorldMap>().set_override(
        x,
        y,
        Tile {
            biome,
            walkable: biome.walkable(),
        },
    );

    let pack = match encounter {
        // The two halves of `try_spawn_habitat_creature`, minus its nest
        // branch: a nest is not a fight, and a roll that placed one would
        // leave the arena with nobody to fight at all.
        Encounter::Field { .. } => match game.pick_habitat_species(x, y, true) {
            Some((species, is_boss)) => {
                game.spawn_pack(&species, is_boss, x, y, SpawnEscalation::surface())
            }
            None => Vec::new(),
        },
        // A real descent, through the one way into a frame. The party is
        // genuinely underground for the fight, so the depth multiplier and
        // Trace apply as they do in play — and, as in play, an ambush is
        // never a boss: `stack_encounter_pack` passes `allow_boss: false`,
        // so a lair guardian is not reachable from here at all. `frames` is
        // what the link would say, raised only as far as the depth asked
        // for needs.
        Encounter::Stack { depth, .. } => {
            let frames = game.frames_at((x, y)).max(*depth);
            game.descend_to(*depth, frames, (x, y));
            let pack = game.stack_encounter_pack();
            // The one thing a rolled pack does not keep. `end_battle`
            // despawns whatever still carries `StackSpawn`, and `Watch`
            // reads the outcome off the opponents — so the sweep deletes the
            // evidence a defeat ever happened, and every lost Stack fight
            // reports as a win. The tag exists to stop a jacked-out-on pack
            // waiting at the link mouth of a run that continues; a rep has
            // no such run, so nothing about the *fight* changes.
            for &member in &pack {
                game.world.entity_mut(member).remove::<StackSpawn>();
            }
            pack
        }
        // The same descent, then `rouse_lair`'s own two steps — which
        // species guards this stack, and the pack that species brings —
        // without the three things that make it an *event*: the cell
        // underfoot, the `FrameMemory` record of a cleared lair, and the
        // narration. A scenario asks for the guardian directly, so none of
        // the machinery that decides *when* you meet it applies; what it
        // must not do is restate *what* you meet, which is why both steps
        // are calls.
        Encounter::Lair { depth, .. } => {
            let frames = game.frames_at((x, y)).max(*depth);
            game.descend_to(*depth, frames, (x, y));
            let pos = game.stack_pos().expect("`descend_to` installed a frame");
            let pack = match game.pick_lair_species(pos) {
                Some((species, is_boss)) => {
                    let esc = game.stack_escalation(pos.depth);
                    let (ex, ey) = pos.entrance;
                    game.spawn_pack(&species, is_boss, ex, ey, esc)
                }
                None => Vec::new(),
            };
            for &member in &pack {
                game.world.entity_mut(member).remove::<StackSpawn>();
            }
            pack
        }
    };

    if pack.is_empty() {
        return Err(format!(
            "nothing lives on {biome:?} — that biome fields no encounter"
        ));
    }
    Ok(game.group_pack(pack))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::scenario::{PlayerSource, Scenario};
    use crate::arena::setup::build_player;
    use crate::species::SpeciesDb;
    use crate::tests::support::test_assets_dir;
    use crate::world::Biome;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    /// A staged arena player, seeded the way `stage` seeds one — the roll
    /// spends `GameRng`, so a fixture that skipped this would measure
    /// `Game::new(0)`'s stream at every seed.
    fn arena(zone: u32, seed: u64) -> Game {
        let s = Scenario {
            player: PlayerSource::Fresh { level: 10, zone },
            ..Scenario::default()
        };
        let mut game = build_player(&s, &test_assets_dir()).unwrap();
        game.world
            .insert_resource(resources::GameRng(StdRng::seed_from_u64(seed)));
        game
    }

    fn members(groups: &[EnemyGroup]) -> Vec<Entity> {
        groups.iter().flat_map(|g| g.members.clone()).collect()
    }

    fn total_hp(game: &Game, groups: &[EnemyGroup]) -> i32 {
        members(groups)
            .into_iter()
            .filter_map(|e| game.world.get::<Stats>(e))
            .map(|s| s.max_hp)
            .sum()
    }

    #[test]
    fn a_field_roll_fields_only_that_biomes_residents() {
        let mut game = arena(3, 7);
        let groups = roll(
            &mut game,
            &Encounter::Field {
                biome: Biome::Mainframe,
            },
        )
        .unwrap();

        // Asserted against the real roster rather than a fixture list, so it
        // stays honest when the species files change. Both pools: a boss
        // lives on the biome too, and this seed draws one — with an escort
        // from the same habitat, which is `spawn_pack`'s rule rather than
        // anything this module decides.
        let db = game.world.resource::<SpeciesDb>();
        let resident: Vec<String> = db
            .habitat_matches(Biome::Mainframe)
            .into_iter()
            .chain(db.boss_habitat_matches(Biome::Mainframe))
            .map(|d| d.id.clone())
            .collect();
        assert!(!groups.is_empty());
        for group in &groups {
            assert!(
                resident.contains(&group.species),
                "`{}` does not live on Mainframe: {resident:?}",
                group.species
            );
        }
    }

    #[test]
    fn a_stack_roll_leaves_the_party_underground_at_the_depth_asked_for() {
        let mut game = arena(3, 7);
        let groups = roll(
            &mut game,
            &Encounter::Stack {
                biome: Biome::OpenGrid,
                depth: 4,
            },
        )
        .unwrap();

        let pos = game.stack_pos().expect("the party is underground");
        assert_eq!(pos.depth, 4);
        for member in members(&groups) {
            assert!(
                game.world.get::<components::StackSpawn>(member).is_none(),
                "a tagged member is one `end_battle` sweeps, taking the \
                 evidence of a defeat with it"
            );
        }
    }

    #[test]
    fn a_deeper_stack_roll_hits_harder() {
        // Deterministic rather than sampled: one seed, one biome, two
        // depths. Depth moves three things at once — the stat multiplier,
        // the group-size ceiling and how many groups the frame fields — so
        // the two packs are not the same programs scaled up, and only their
        // total weight is comparable.
        let deep = |depth: u32| {
            let mut game = arena(3, 7);
            let groups = roll(
                &mut game,
                &Encounter::Stack {
                    biome: Biome::OpenGrid,
                    depth,
                },
            )
            .unwrap();
            total_hp(&game, &groups)
        };

        assert!(
            deep(5) > deep(1),
            "depth 5 fielded no more than depth 1: {} vs {}",
            deep(5),
            deep(1)
        );
    }

    #[test]
    fn a_lair_roll_fields_the_boss_a_stack_roll_never_can() {
        // The two underground encounters at one depth and one biome. The
        // ambush is asserted to hold *no* boss rather than merely a different
        // species, because that is the property `Lair` exists to reach past:
        // `stack_encounter_pack` passes `allow_boss: false`, so before this
        // variant the guardian was unmeasurable.
        let boss_count = |encounter: Encounter| {
            let mut game = arena(3, 7);
            let groups = roll(&mut game, &encounter).unwrap();
            members(&groups)
                .into_iter()
                .filter(|&e| {
                    let species = &game.world.get::<Creature>(e).unwrap().species;
                    game.world
                        .resource::<SpeciesDb>()
                        .get(species)
                        .is_some_and(|d| d.is_boss)
                })
                .count()
        };

        let biome = Biome::Mainframe;
        assert_eq!(
            boss_count(Encounter::Lair { biome, depth: 4 }),
            1,
            "a lair fields exactly one guardian"
        );
        assert_eq!(
            boss_count(Encounter::Stack { biome, depth: 4 }),
            0,
            "an ambush at the same depth fields no boss"
        );
    }

    #[test]
    fn a_lair_roll_leaves_the_party_underground_at_the_depth_asked_for() {
        let mut game = arena(3, 7);
        let groups = roll(
            &mut game,
            &Encounter::Lair {
                biome: Biome::Mainframe,
                depth: 4,
            },
        )
        .unwrap();

        let pos = game.stack_pos().expect("the party is underground");
        assert_eq!(pos.depth, 4);
        for member in members(&groups) {
            assert!(
                game.world.get::<components::StackSpawn>(member).is_none(),
                "a tagged member is one `end_battle` sweeps, taking the \
                 evidence of a defeat with it"
            );
        }
    }

    #[test]
    fn a_rolled_pack_is_capped_by_the_zones_own_ceilings() {
        let mut game = arena(5, 12);
        let groups = roll(
            &mut game,
            &Encounter::Stack {
                biome: Biome::NullSector,
                depth: 5,
            },
        )
        .unwrap();

        assert!(groups.len() <= game.enemy_group_ceiling(), "{groups:?}");
        for group in &groups {
            assert!(
                group.members.len() <= game.group_size_ceiling(),
                "{group:?}"
            );
        }
    }

    #[test]
    fn zone_one_field_fields_only_the_opening_ring() {
        // The arena's player stands at the danger origin, so zone 1 fields
        // what a fresh run meets rather than zone 1's whole roster. A stated
        // limit of the design, pinned here rather than left to be
        // rediscovered.
        let mut game = arena(1, 3);
        let groups = roll(
            &mut game,
            &Encounter::Field {
                biome: Biome::OpenGrid,
            },
        )
        .unwrap();

        for group in &groups {
            let def = game
                .world
                .resource::<SpeciesDb>()
                .get(&group.species)
                .expect("a rolled species is in the roster")
                .clone();
            assert!(
                crate::balance_sim::beatable_by_a_fresh_player(&def),
                "zone 1 fielded `{}`, which a fresh player does not beat",
                group.species
            );
        }
    }

    #[test]
    fn a_biome_nothing_lives_in_is_an_err_naming_it() {
        let mut game = arena(3, 7);
        let err = roll(
            &mut game,
            &Encounter::Field {
                biome: Biome::Platform,
            },
        )
        .unwrap_err();
        assert!(err.contains("Platform"), "{err}");
    }
}
