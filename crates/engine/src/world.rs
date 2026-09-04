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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Biome {
    DataVoid,
    /// Renamed from `StaticField`; the alias is what keeps every save and
    /// species mod written before the rename loading, and is why this cost
    /// no `SAVE_FORMAT_VERSION` bump.
    #[serde(alias = "StaticField")]
    Deadlock,
    NullSector,
    /// Renamed from `Mainframe`, which the settlements work needs back as
    /// the word for a city. Same alias trick as `Deadlock` above, for the
    /// same reason and at the same price: no `SAVE_FORMAT_VERSION` bump,
    /// and every save and species mod written before the rename keeps
    /// loading.
    #[serde(alias = "Mainframe")]
    Backplane,
    OpenGrid,
    BlackIce,
    /// Laid base floor — `base_grid::BaseCell::Floor`. **Never produced by
    /// `classify` and never written into a `WorldMap` any more**: the base
    /// went out of phase into `base_grid::BaseGrid`, its own coordinate
    /// space, and this variant survives only as the rendering vocabulary
    /// `Game::view_tiles` synthesises for it — see that function for the
    /// three-way mapping. No shipped species lists it as a habitat, which is
    /// the entire mechanism behind a base being a safe haven:
    /// `Game::try_spawn_habitat_creature` already bails when both candidate
    /// pools come back empty, so no spawn-suppression code exists anywhere.
    Platform,
    /// Carved-out base space, not yet floored — `BaseCell::Open`. Walkable,
    /// like `Platform`, and the same rendering-only status: synthesised by
    /// `Game::view_tiles`, never written into a `WorldMap`, never produced
    /// by `classify`.
    Excavated,
    /// Solid, unmined base space — what `Game::view_tiles` draws for every
    /// base coordinate `BaseGrid` has no cell for. The base's equivalent of
    /// a hole in the map: nothing is ever placed there, the same as
    /// `DataVoid` and `BlackIce` on the surface.
    Entropy,
}

impl Biome {
    /// Whether terrain of this kind can be stood on. The one definition:
    /// `WorldMap::classify` stamps it onto every `Tile` it produces, and
    /// anything asking the question of a biome rather than of a tile — a
    /// census over the roster, say — asks here rather than repeating the
    /// match. An unwalkable biome is a hole in the map, so nothing is ever
    /// placed there: no spawn, no structure, and no Stack link.
    pub fn walkable(self) -> bool {
        !matches!(self, Biome::DataVoid | Biome::BlackIce | Biome::Entropy)
    }

    /// What the player calls this ground.
    ///
    /// An exhaustive match rather than data: mods extend species,
    /// structures, items and environments, but the biome set is a fixed
    /// enum that `classify` sorts noise into, and a name for a variant that
    /// cannot exist is not a thing a file can usefully say.
    pub fn name(self) -> &'static str {
        match self {
            Biome::DataVoid => "Data Void",
            Biome::Deadlock => "Deadlock",
            Biome::NullSector => "Null Sector",
            Biome::Backplane => "Backplane",
            Biome::OpenGrid => "Open Grid",
            Biome::BlackIce => "Black Ice",
            Biome::Platform => "Platform",
            Biome::Excavated => "Excavated",
            Biome::Entropy => "Entropy",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Tile {
    pub biome: Biome,
    pub walkable: bool,
    /// How bright this cell's rock face is drawn, for a base-space cell that
    /// is an *exposed* face — solid, with air orthogonally against it. `None`
    /// everywhere on the zone surface, and on every base cell that is walked
    /// on or buried behind another.
    ///
    /// **Deliberately `#[serde(skip)]`.** `Tile` lands in the save through
    /// `SaveData::tile_overrides`, and this is a derived display value
    /// recomputed every frame from `rock::RockDb::wall_at` — storing it would
    /// let a save disagree with the world it came from, and would put a
    /// renderer's concern in the save format.
    ///
    /// The rule it exists to serve: colouring every wall would hand the
    /// player a map of everything they will ever dig, so only faces they can
    /// actually see carry a kind. Exposing a face is the act of prospecting.
    #[serde(skip)]
    pub rock_shade: Option<f32>,
}

impl Tile {
    /// Whether a hostile may stand here: walkable, and not the base slab.
    ///
    /// **Unreachable now that nothing writes `Biome::Platform` into a
    /// `WorldMap`** — the base is out of phase and its floor is
    /// `base_grid::BaseGrid`, not a tile override. Left as-is rather than
    /// deleted; slice 2/3 is where the slab-era readers below get replaced
    /// with a base-space equivalent of this rule, and `stamp_platform`
    /// (named below) no longer exists.
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

/// The five thresholds `WorldMap::classify` sorts three Perlin fields into
/// six biomes with.
///
/// A value rather than five literals because it is the *only* knob a sector
/// trait turns: a sector shifts where the biome boundaries fall, and its
/// look, its roster and where it can be built all fall out of that one
/// change — `Game::habitat_pools` filters species by the tile's biome and
/// Where one biome gives way to the next.
///
/// These were a `SectorShape` value for as long as a breach rebuilt the
/// map: `assets/sectors/` shipped per-zone deltas over them, so the
/// ground you arrived on read differently from the ground you left. The
/// world is persistent now — there is one map for the run, and a breach
/// raises a tier rather than carving new terrain — so a per-zone shape
/// has nothing left to vary. Back to constants, which is what they were
/// before sectors existed.
///
/// Geographic variety comes back as content standing *on* the map rather
/// than as a reshuffle of the noise under it; that is what settlements
/// are for.
const VOID_ELEVATION: f64 = -0.3;
const BLACK_ICE_ELEVATION: f64 = 0.55;
const DEADLOCK_TEMPERATURE: f64 = -0.3;
const NULL_TEMPERATURE: f64 = 0.3;
const NULL_MOISTURE: f64 = -0.1;
const BACKPLANE_MOISTURE: f64 = 0.15;

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

        let biome = if e < VOID_ELEVATION {
            Biome::DataVoid
        } else if e > BLACK_ICE_ELEVATION {
            Biome::BlackIce
        } else if t < DEADLOCK_TEMPERATURE {
            Biome::Deadlock
        } else if t > NULL_TEMPERATURE && m < NULL_MOISTURE {
            Biome::NullSector
        } else if m > BACKPLANE_MOISTURE {
            Biome::Backplane
        } else {
            Biome::OpenGrid
        };

        Tile {
            biome,
            walkable: biome.walkable(),
            rock_shade: None,
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

    /// Seed 4242 over `x in 0..48`, `y in 0..24`, one character per biome,
    /// captured from the implementation as it stood *before* `SectorShape`
    /// existed. A pin rather than a tautology: it was produced by the old
    /// hardcoded `classify` and pasted here, so `NEUTRAL` reproducing it is
    /// evidence generation did not move.
    const NEUTRAL_TERRAIN_4242: &str = "\
oonnnnnnnnnnnnnnnnnoooommmmmoovvvvvvvvvvvvvvvvmm
ooonnnnnnnnnnnnnnnoooommmmmmmvvvvvvvvvvvvvvvvmmm
mooonnnnnnnnnnnnnooommmmmmmmvvvvvvvvvvvvvvvvvmmm
mmooonnnnnnnnnnnooommmmmmmmvvvvvvvvvvvvvvvvvmmmm
mmmooonnnnnnnnnooommmmmmmmvvvvvvvvvvvvvvvvvmmmmm
mmmmmoooonnnoooommmmmmmmmvvvvvvvvvvvvvvvvvvmmmmm
mmmmmmmooooooommmmmmmmmmvvvvvvvvvvvvvvvvvvmmmmmm
mmmmmmmmmmmmmmmmmmmmmmvvvvvvvvvvvvvvvvvvvmmmmmmm
mmmmmmmmmmmmmmmmmmmmvvvvvvvvvvvvvvvvvvvvvmmmmmmm
mmmmmmmmmmmmmmmmmmvvvvvvvvvvvvvvvvvvvvvvmmmmmmmm
mmmmmmmmmmmmmmmmvvvvvvvvvvvvvvvvvvvvvvvvmmmmmmmi
mmmmmmmmmmmmvvvvvvvvvvvvvvvvvvvvvvvvvvvmmmmmmmii
mmmmmmmmmvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvmmmmmmmii
mmmmmmmvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvmmmmmmmiii
mmmmmvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvmmmmmmmiii
mmmmvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvmmmmmmmmiii
mmmvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvmmmmmmmmiii
mmmvvvvvvvvvvvvvvmmvvvvvvvvvvvvvvvvvvmmmmmoooiii
mmvvvvvvvvvvvvmmmmmmmovvvvvvvvvvvvvvmmmmmooooiii
oovvvvvvvvvvvmmmmmmoooovvvvvvvvvvvvvmmmoooonnnno
oovvvvvvvvvvmmmmmmmooonnnvvvvvvvvvvmmmoooonnnnnn
ooovvvvvvvvmmmmmmmooonnnnnvvvvvvvvooooooonnnnnnn
nooovvvvvvmmmmmmmooonnnnnnnnvvvvvvoooooonnnnnnnn
nnooovvvmmmmmmmmooonnnnnnnnnnnvvooooooonnnnnnnnn
";

    /// The same region the capture covers, rendered the same way.
    fn render(map: &mut WorldMap) -> String {
        let mut out = String::new();
        for y in 0..24 {
            for x in 0..48 {
                out.push(match map.tile(x, y).biome {
                    Biome::DataVoid => 'v',
                    Biome::BlackIce => 'i',
                    Biome::Deadlock => 's',
                    Biome::NullSector => 'n',
                    Biome::Backplane => 'm',
                    Biome::OpenGrid => 'o',
                    Biome::Platform => 'p',
                    // Base-space rendering vocabulary only — `classify`
                    // never produces either, so this map capture can never
                    // actually reach them.
                    Biome::Excavated | Biome::Entropy => {
                        unreachable!("classify never produces a base-space biome")
                    }
                });
            }
            out.push('\n');
        }
        out
    }

    /// The gate on retiring sectors: `classify`'s constants must generate
    /// what the neutral `SectorShape` generated, tile for tile, against
    /// terrain captured before the thresholds were ever a value. Zone 1 was
    /// always neutral and the opening ring's roster is decided by its biome
    /// mix, so this is also the assurance that a new run opens on exactly
    /// the ground it used to.
    #[test]
    fn the_constants_generate_exactly_what_the_neutral_shape_did() {
        let mut map = WorldMap::new(4242);
        assert_eq!(render(&mut map), NEUTRAL_TERRAIN_4242);
    }

    /// The rename is free exactly as long as the alias carries it: a save
    /// or a species mod written before `Mainframe` became `Backplane` must
    /// still load, which is why `SAVE_FORMAT_VERSION` did not move for it.
    ///
    /// `Deadlock` is asserted alongside because it is the same trick one
    /// rename earlier, and a test naming only the new one would not notice
    /// the older alias being dropped.
    #[test]
    fn a_biome_written_under_its_old_name_still_loads() {
        assert_eq!(
            ron::from_str::<Biome>("Mainframe").unwrap(),
            Biome::Backplane
        );
        assert_eq!(
            ron::from_str::<Biome>("StaticField").unwrap(),
            Biome::Deadlock
        );
        // And the current spelling round-trips, or the alias would be the
        // only way to name it.
        assert_eq!(
            ron::from_str::<Biome>(&ron::to_string(&Biome::Backplane).unwrap()).unwrap(),
            Biome::Backplane
        );
    }

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
                rock_shade: None,
            },
        );
        let tile = map.tile(3, 3);
        assert_eq!(tile.biome, Biome::DataVoid);
        assert!(!tile.walkable);
    }
}
