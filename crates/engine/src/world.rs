use std::collections::HashMap;

use bevy_ecs::prelude::Resource;
use noise::{NoiseFn, Perlin};
use serde::{Deserialize, Serialize};

pub const CHUNK_SIZE: i32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Biome {
    DataVoid,
    StaticField,
    NullSector,
    Mainframe,
    OpenGrid,
    BlackIce,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Tile {
    pub biome: Biome,
    pub walkable: bool,
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

        let walkable = !matches!(biome, Biome::DataVoid | Biome::BlackIce);
        Tile { biome, walkable }
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
