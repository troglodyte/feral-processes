use std::collections::HashMap;

use bevy_ecs::prelude::Resource;
use noise::{NoiseFn, Perlin};
use serde::{Deserialize, Serialize};

pub const CHUNK_SIZE: i32 = 32;

/// The eight neighbouring tiles, matching the game's 8-directional
/// movement. Chebyshev distance is "how many moves away" throughout the
/// engine because of this set; a fourth direction scheme would have to
/// change every distance comparison with it.
pub(crate) const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Biome {
    DataVoid,
    StaticField,
    NullSector,
    Mainframe,
    OpenGrid,
    BlackIce,
    /// The floor of a player base, stamped across the build radius when a
    /// Home is deployed (`Game::stamp_platform`) and never produced by
    /// `classify`. No shipped species lists it as a habitat, which is the
    /// entire mechanism behind a base being a safe haven:
    /// `Game::try_spawn_habitat_creature` already bails when both candidate
    /// pools come back empty, so no spawn-suppression code exists anywhere.
    Platform,
}

impl Biome {
    /// Whether terrain of this kind can be stood on. The one definition:
    /// `WorldMap::classify` stamps it onto every `Tile` it produces, and
    /// anything asking the question of a biome rather than of a tile — a
    /// census over the roster, say — asks here rather than repeating the
    /// match. An unwalkable biome is a hole in the map, so nothing is ever
    /// placed there: no spawn, no structure, and no Stack link.
    pub fn walkable(self) -> bool {
        !matches!(self, Biome::DataVoid | Biome::BlackIce)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Tile {
    pub biome: Biome,
    pub walkable: bool,
}

impl Tile {
    /// Whether a hostile may stand here: walkable, and not the base slab.
    ///
    /// `walkable` alone is not the rule, and the slab is why. It is the one
    /// safe ground in the game, established in three places already —
    /// `Game::maybe_ambush` refuses to roll while the player stands on it,
    /// `Game::stamp_platform` purges whatever is standing there when the
    /// floor is laid, and `pursuit_field` keeps a provoked swarm off it.
    ///
    /// `wander_ai_system` is the fourth reader and was the one that quietly
    /// disagreed: it checked `walkable` alone, so an ordinary wild program
    /// could stroll onto a base that a *pursuing* guardian was forbidden to
    /// enter. That went unnoticed because `stamp_platform` purges the slab
    /// as it is laid and the wild population was small enough that few
    /// programs ever stood next to one — raising the population is what made
    /// a program adjacent to the edge, and therefore one wander step from
    /// the inside, the common case rather than the rare one.
    ///
    /// One predicate rather than four copies of `walkable && biome !=
    /// Platform`, because the copy that drifts is the one nobody runs.
    pub(crate) fn open_to_hostiles(&self) -> bool {
        self.walkable && self.biome != Biome::Platform
    }
}

struct Chunk {
    tiles: Vec<Tile>,
}

/// Two-tier world map: a coarse noise field classified into biomes, sampled
/// lazily per chunk, plus a sparse overlay of player-caused tile changes.
/// Only the seed and the overlay are ever persisted — chunks regenerate
/// deterministically from the seed on demand.
#[derive(Resource)]
pub struct WorldMap {
    seed: u32,
    elevation: Perlin,
    moisture: Perlin,
    temperature: Perlin,
    chunks: HashMap<(i32, i32), Chunk>,
    overrides: HashMap<(i32, i32), Tile>,
}

impl WorldMap {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            elevation: Perlin::new(seed),
            moisture: Perlin::new(seed.wrapping_add(1)),
            temperature: Perlin::new(seed.wrapping_add(2)),
            chunks: HashMap::new(),
            overrides: HashMap::new(),
        }
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    pub fn overrides(&self) -> &HashMap<(i32, i32), Tile> {
        &self.overrides
    }

    pub fn restore_overrides(&mut self, overrides: HashMap<(i32, i32), Tile>) {
        self.overrides = overrides;
    }

    fn classify(&self, wx: i32, wy: i32) -> Tile {
        let e = self.elevation.get([wx as f64 * 0.04, wy as f64 * 0.04]);
        let m = self.moisture.get([wx as f64 * 0.05, wy as f64 * 0.05]);
        let lat_falloff = (wy as f64).abs() * 0.0015;
        let t = (self.temperature.get([wx as f64 * 0.03, wy as f64 * 0.03]) * 0.5
            + (1.0 - lat_falloff))
            .clamp(-1.0, 1.0);

        let biome = if e < -0.3 {
            Biome::DataVoid
        } else if e > 0.55 {
            Biome::BlackIce
        } else if t < -0.3 {
            Biome::StaticField
        } else if t > 0.3 && m < -0.1 {
            Biome::NullSector
        } else if m > 0.15 {
            Biome::Mainframe
        } else {
            Biome::OpenGrid
        };

        Tile {
            biome,
            walkable: biome.walkable(),
        }
    }

    fn ensure_chunk(&mut self, cx: i32, cy: i32) {
        if self.chunks.contains_key(&(cx, cy)) {
            return;
        }
        let mut tiles = Vec::with_capacity((CHUNK_SIZE * CHUNK_SIZE) as usize);
        for ty in 0..CHUNK_SIZE {
            for tx in 0..CHUNK_SIZE {
                tiles.push(self.classify(cx * CHUNK_SIZE + tx, cy * CHUNK_SIZE + ty));
            }
        }
        self.chunks.insert((cx, cy), Chunk { tiles });
    }

    pub fn tile(&mut self, x: i32, y: i32) -> Tile {
        if let Some(t) = self.overrides.get(&(x, y)) {
            return *t;
        }
        let (cx, cy) = (x.div_euclid(CHUNK_SIZE), y.div_euclid(CHUNK_SIZE));
        self.ensure_chunk(cx, cy);
        let (lx, ly) = (x.rem_euclid(CHUNK_SIZE), y.rem_euclid(CHUNK_SIZE));
        self.chunks[&(cx, cy)].tiles[(ly * CHUNK_SIZE + lx) as usize]
    }

    pub fn set_override(&mut self, x: i32, y: i32, tile: Tile) {
        self.overrides.insert((x, y), tile);
    }

    /// Drops any override at `(x, y)`, so the tile reverts to whatever the
    /// seed generates there.
    pub fn clear_override(&mut self, x: i32, y: i32) {
        self.overrides.remove(&(x, y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_yields_identical_terrain() {
        let mut a = WorldMap::new(42);
        let mut b = WorldMap::new(42);
        for (x, y) in [(0, 0), (5, -5), (100, 40), (-30, 17)] {
            assert_eq!(a.tile(x, y).biome, b.tile(x, y).biome);
        }
    }

    #[test]
    fn different_seeds_can_diverge() {
        let mut a = WorldMap::new(1);
        let mut b = WorldMap::new(2);
        let biomes_a: Vec<_> = (0..40).map(|x| a.tile(x, 0).biome).collect();
        let biomes_b: Vec<_> = (0..40).map(|x| b.tile(x, 0).biome).collect();
        assert_ne!(biomes_a, biomes_b);
    }

    #[test]
    fn classify_never_produces_the_platform_biome() {
        let mut map = WorldMap::new(4242);
        for x in -60..60 {
            for y in -60..60 {
                assert_ne!(
                    map.tile(x, y).biome,
                    Biome::Platform,
                    "Platform is stamped where a Home is deployed, never generated — ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn overrides_take_priority_over_generated_terrain() {
        let mut map = WorldMap::new(7);
        map.set_override(
            3,
            3,
            Tile {
                biome: Biome::DataVoid,
                walkable: false,
            },
        );
        let tile = map.tile(3, 3);
        assert_eq!(tile.biome, Biome::DataVoid);
        assert!(!tile.walkable);
    }
}
