//! Folding a wild pack's same-species runs into squads before a battle map
//! ever sizes itself for them.
//!
//! **Pure and draws no RNG.** `open_tactical_battle_at` calls this after the
//! opening bearing has already been derived from the original, unfolded
//! pack — see that function's own doc for the two silent-degradation sites
//! (`gather_pack`, `open_tactical_battle`) folding one call earlier would
//! reach.

use bevy_ecs::prelude::{Entity, World};

use crate::components::Creature;
use crate::species::SpeciesId;
use crate::tuning::FORMATIONS;

/// One piece a pack has been cut into: either a body fighting alone, or a
/// same-species set large enough to fold into a formation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Single(Entity),
    Squad {
        /// The lead is always [0] — the body the player bumped, if this set
        /// contains it, otherwise the first member of the set in pack
        /// order. `Game::spawn_squad` reads routines and cooldowns off it;
        /// `Game::decompile_squad` is who a successful capture pulls out.
        members: Vec<Entity>,
        /// Index into `tuning::FORMATIONS`.
        formation: usize,
    },
}

/// Cuts `pack` into pieces: for each species, in the pack's own order, as
/// many `FORMATIONS` sets (largest first) as the count allows, with
/// whatever is left over as single bodies.
///
/// A body with no `Creature` cannot be grouped by species at all — it is
/// always a `Piece::Single`, appended after every species' own pieces,
/// which real play never has to reach (`gather_pack`'s query already
/// requires `Hostile`, and every wild spawner writes `Creature` beside it),
/// but which is safer than dropping a body from the fight silently.
pub fn plan(pack: &[Entity], world: &World) -> Vec<Piece> {
    let bumped = pack.first().copied();

    let mut order: Vec<SpeciesId> = Vec::new();
    let mut by_species: std::collections::HashMap<SpeciesId, Vec<Entity>> =
        std::collections::HashMap::new();
    let mut speciesless: Vec<Entity> = Vec::new();
    for &entity in pack {
        match world.get::<Creature>(entity).map(|c| c.species.clone()) {
            Some(species) => {
                by_species
                    .entry(species.clone())
                    .or_insert_with(|| {
                        order.push(species.clone());
                        Vec::new()
                    })
                    .push(entity);
            }
            None => speciesless.push(entity),
        }
    }

    // `FORMATIONS` largest-first, resolved once rather than per species —
    // the table ships with one row today, but a mod's second row need not
    // already be sorted for "largest-first" to hold.
    let mut by_size: Vec<usize> = (0..FORMATIONS.len()).collect();
    by_size.sort_by_key(|&i| std::cmp::Reverse(FORMATIONS[i].members));

    let mut pieces = Vec::new();
    for species in order {
        let mut members = by_species.remove(&species).unwrap_or_default();
        loop {
            let cut = by_size
                .iter()
                .copied()
                .find(|&fi| members.len() >= FORMATIONS[fi].members);
            let Some(formation) = cut else {
                break;
            };
            let need = FORMATIONS[formation].members;
            let mut set: Vec<Entity> = members.drain(0..need).collect();
            if let Some(bumped) = bumped
                && let Some(pos) = set.iter().position(|&e| e == bumped)
            {
                set.swap(0, pos);
            }
            pieces.push(Piece::Squad {
                members: set,
                formation,
            });
        }
        pieces.extend(members.into_iter().map(Piece::Single));
    }
    pieces.extend(speciesless.into_iter().map(Piece::Single));
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Hostile, Stats};

    fn spawn(world: &mut World, species: &str) -> Entity {
        world
            .spawn((
                Creature {
                    species: species.to_string(),
                },
                Hostile,
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    mitigation: 0,
                },
            ))
            .id()
    }

    fn pack_of(world: &mut World, species: &str, count: usize) -> Vec<Entity> {
        (0..count).map(|_| spawn(world, species)).collect()
    }

    #[test]
    fn nine_of_a_species_fold_into_one_squad_and_four_singles() {
        let mut world = World::new();
        let pack = pack_of(&mut world, "a", 9);
        let pieces = plan(&pack, &world);

        let squads: Vec<&Piece> = pieces
            .iter()
            .filter(|p| matches!(p, Piece::Squad { .. }))
            .collect();
        let singles: Vec<&Piece> = pieces
            .iter()
            .filter(|p| matches!(p, Piece::Single(_)))
            .collect();
        assert_eq!(squads.len(), 1, "9 of a kind should cut exactly one squad");
        assert_eq!(singles.len(), 4, "9 of a kind should leave four singles");
        let Piece::Squad { members, formation } = squads[0] else {
            unreachable!()
        };
        assert_eq!(*formation, 0);
        assert_eq!(members.len(), 5);
    }

    #[test]
    fn ten_of_a_species_fold_into_two_squads() {
        let mut world = World::new();
        let pack = pack_of(&mut world, "a", 10);
        let pieces = plan(&pack, &world);
        let squads = pieces
            .iter()
            .filter(|p| matches!(p, Piece::Squad { .. }))
            .count();
        assert_eq!(squads, 2, "10 of a kind should cut two squads");
        assert!(
            pieces.iter().all(|p| !matches!(p, Piece::Single(_))),
            "10 of a kind should leave no singles"
        );
    }

    #[test]
    fn four_of_a_species_never_folds() {
        let mut world = World::new();
        let pack = pack_of(&mut world, "a", 4);
        let pieces = plan(&pack, &world);
        assert_eq!(pieces.len(), 4);
        assert!(pieces.iter().all(|p| matches!(p, Piece::Single(_))));
    }

    /// A pack spanning two species folds each species on its own — one
    /// squad from the first nine, nothing from the second's lone straggler.
    #[test]
    fn a_mixed_pack_folds_per_species() {
        let mut world = World::new();
        let mut pack = pack_of(&mut world, "a", 9);
        pack.push(spawn(&mut world, "b"));
        let pieces = plan(&pack, &world);

        let squads: Vec<&Piece> = pieces
            .iter()
            .filter(|p| matches!(p, Piece::Squad { .. }))
            .collect();
        assert_eq!(squads.len(), 1);
        let singles: Vec<Entity> = pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Single(e) => Some(*e),
                Piece::Squad { .. } => None,
            })
            .collect();
        assert_eq!(singles.len(), 5, "four leftover 'a' plus the lone 'b'");
        assert_eq!(singles.last().copied(), pack.last().copied());
    }

    /// The bumped body (`pack[0]`) leads whichever set it lands in — even
    /// when that set is not the first one cut.
    #[test]
    fn the_bumped_body_leads_its_set() {
        let mut world = World::new();
        let pack = pack_of(&mut world, "a", 10);
        let bumped = pack[7];
        // Re-derive the pack with the bumped body still at [0] — `plan`
        // reads `pack.first()` as "the body the player bumped", exactly as
        // `gather_pack` builds its own pack with the anchor first.
        let mut reordered = vec![bumped];
        reordered.extend(pack.iter().copied().filter(|&e| e != bumped));
        let pieces = plan(&reordered, &world);
        let containing = pieces
            .iter()
            .find_map(|p| match p {
                Piece::Squad { members, .. } if members.contains(&bumped) => Some(members),
                _ => None,
            })
            .expect("the bumped body must land in some squad");
        assert_eq!(containing[0], bumped, "the bumped body must lead its set");
    }

    #[test]
    fn plan_draws_no_rng_and_stays_pure() {
        let mut world = World::new();
        let pack = pack_of(&mut world, "a", 9);
        assert_eq!(plan(&pack, &world), plan(&pack, &world));
    }
}
