//! Where the settlements are.
//!
//! **A property of the map, not an event.** The world is divided into
//! square regions of `SETTLEMENT_REGION_CHUNKS` chunks; an FNV-1a fold of
//! the world seed and the region's coordinates answers, for each one,
//! whether it holds a settlement, roughly where, and which catalogue entry
//! stands there. Nothing is spawned into existence and nothing is ever
//! despawned — a town is simply *there* when the party arrives, and it is
//! there again if they walk away and come back.
//!
//! That is only possible because a breach stopped rebuilding the world. It
//! is the rule `rock::RockDb::kind_at` already follows for base space, and
//! it carries the same three prohibitions: no `resources::GameRng` (a draw
//! does not survive a save/load and shifts every later roll in the run), no
//! `StdRng` sequence (not guaranteed stable across a `rand` upgrade, so a
//! dependency bump would silently move every town in every world), and
//! **never `%`** — see `derive::index`.

use serde::{Deserialize, Serialize};

use crate::derive;
use crate::tuning::{SETTLEMENT_REGION_CHUNKS, SETTLEMENT_REGION_PERCENT};
use crate::world::CHUNK_SIZE;

use super::SettlementDb;

/// Which region a settlement belongs to.
///
/// The identity a saved relationship hangs off, and deliberately not an
/// `Entity` — `CreatureSave::sortie_index`'s reason, one level out: entity
/// ids are not stable across a save, and a region's coordinates are the one
/// name for this place that cannot drift.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SettlementKey {
    pub rx: i32,
    pub ry: i32,
}

/// A settlement the derivation says stands in a region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub key: SettlementKey,
    /// Where in the region it wants to stand.
    ///
    /// A *candidate*, not the final tile: the derivation cannot see the
    /// ground, so materialization walks out from here for somewhere
    /// standable. That stays deterministic — the map is permanent and
    /// itself a pure function of the seed — but it is why the resolved tile
    /// is recorded rather than re-derived.
    pub cell: (i32, i32),
    /// The catalogue id standing here.
    pub def_id: String,
}

/// How many tiles a region is across.
pub const REGION_TILES: i32 = SETTLEMENT_REGION_CHUNKS * CHUNK_SIZE;

/// The region a world tile falls in.
pub fn region_of(x: i32, y: i32) -> SettlementKey {
    SettlementKey {
        rx: x.div_euclid(REGION_TILES),
        ry: y.div_euclid(REGION_TILES),
    }
}

/// What stands in `key`, if anything.
///
/// Returns `None` for every region when the catalogue is empty, which is
/// what makes `assets/settlements/` deletable: the pre-settlement game is
/// an install with no files in it, not a flag.
pub fn settlement_at(seed: u32, db: &SettlementDb, key: SettlementKey) -> Option<Placement> {
    if db.is_empty() {
        return None;
    }
    // Three independent questions off one fold, each salted rather than
    // re-mixed from scratch: `FrameSpec::rng_seed`'s rule, so nothing here
    // can invent a second scheme that collides with the first.
    let base = region_seed(seed, key);
    if derive::index(salted(base, PRESENCE_SALT), 100) >= SETTLEMENT_REGION_PERCENT {
        return None;
    }
    // Inset from the region's edges, so two settlements in neighbouring
    // regions cannot end up within sight of each other and read as one
    // sprawl with a gap in it.
    let span = (REGION_TILES - 2 * REGION_EDGE_INSET).max(1) as usize;
    let ox = derive::index(salted(base, CELL_X_SALT), span) as i32;
    let oy = derive::index(salted(base, CELL_Y_SALT), span) as i32;
    let def = db
        .iter()
        .nth(derive::index(salted(base, DEF_SALT), db.len()))
        .expect("index is bounded by len");
    Some(Placement {
        key,
        cell: (
            key.rx * REGION_TILES + REGION_EDGE_INSET + ox,
            key.ry * REGION_TILES + REGION_EDGE_INSET + oy,
        ),
        def_id: def.id.clone(),
    })
}

/// How far a settlement stays clear of its region's border.
const REGION_EDGE_INSET: i32 = 24;

const PRESENCE_SALT: u64 = 0x5E77_1E00;
const CELL_X_SALT: u64 = 0x5E77_1E01;
const CELL_Y_SALT: u64 = 0x5E77_1E02;
const DEF_SALT: u64 = 0x5E77_1E03;

/// The value `derive::index` reduces: the world seed and a region.
///
/// **Every input goes in a byte at a time.** One XOR-then-multiply round
/// carries a difference only about the prime's own width upward, so folding
/// a region coordinate as a single word would leave neighbouring regions
/// differing nowhere near bit 63 — which is the bit `derive::index` reads,
/// and neighbouring regions is precisely the comparison this has to get
/// right. That is the measured failure `descriptions::Slot::tags`
/// documents, reached here by `rock::block_seed`'s route.
fn region_seed(seed: u32, key: SettlementKey) -> u64 {
    fold(
        0xcbf2_9ce4_8422_2325,
        [seed as u64, key.rx as i64 as u64, key.ry as i64 as u64],
    )
}

/// Continues the fold with one more word, so each question off a region
/// gets its own answer without a second scheme.
fn salted(base: u64, salt: u64) -> u64 {
    fold(base, [salt])
}

fn fold<const N: usize>(mut h: u64, words: [u64; N]) -> u64 {
    for word in words {
        for byte in word.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlements::{SettlementDb, SettlementKind, Specialty, Temperament};

    fn shipped() -> SettlementDb {
        let (db, warnings) =
            SettlementDb::load_dir(&crate::tests::support::test_assets_dir().join("settlements"))
                .unwrap();
        assert!(
            warnings.is_empty(),
            "shipped settlements warned: {warnings:?}"
        );
        assert!(
            !db.is_empty(),
            "test premise: the shipped pool is not empty"
        );
        db
    }

    fn keys(n: i32) -> impl Iterator<Item = SettlementKey> {
        (-n..n).flat_map(move |ry| (-n..n).map(move |rx| SettlementKey { rx, ry }))
    }

    /// The whole point: ask twice, get the same answer.
    #[test]
    fn a_region_answers_the_same_every_time() {
        let db = shipped();
        for key in keys(6) {
            assert_eq!(
                settlement_at(4242, &db, key),
                settlement_at(4242, &db, key),
                "region {key:?} is not a function of the seed"
            );
        }
    }

    /// Deleting `assets/settlements/` is the pre-settlement game, so the
    /// derivation must answer nothing rather than divide by zero.
    #[test]
    fn an_empty_catalogue_places_nothing_anywhere() {
        let db = SettlementDb::default();
        for key in keys(8) {
            assert_eq!(settlement_at(4242, &db, key), None);
        }
    }

    /// The `%` trap, which is why `derive::index` exists. Reduced with `%`
    /// against a small pool, neighbouring regions read one low bit of a
    /// coordinate that differs by one — so they anti-correlate, and the
    /// map lays out in stripes that look arbitrary one region at a time.
    ///
    /// Asserted as coverage of the pool rather than as a distribution test:
    /// every shipped settlement must actually appear somewhere in a modest
    /// sweep, which a stripe cannot manage.
    #[test]
    fn every_shipped_settlement_stands_somewhere() {
        let db = shipped();
        let mut seen: Vec<&str> = keys(12)
            .filter_map(|key| settlement_at(4242, &db, key))
            .map(|p| {
                db.get(&p.def_id)
                    .expect("placement names a real def")
                    .id
                    .as_str()
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        seen.sort_unstable();
        let mut all: Vec<&str> = db.iter().map(|d| d.id.as_str()).collect();
        all.sort_unstable();
        assert_eq!(seen, all, "some settlement never stands anywhere");
    }

    /// Neighbouring regions are the comparison the byte-at-a-time fold
    /// exists for, so they are the ones worth counting: a fold that leaves
    /// adjacent regions correlated shows up here as a presence rate far off
    /// the authored one.
    #[test]
    fn presence_lands_near_its_authored_rate() {
        let db = shipped();
        let sweep: Vec<_> = keys(20).collect();
        let placed = sweep
            .iter()
            .filter(|&&key| settlement_at(4242, &db, key).is_some())
            .count();
        let rate = 100 * placed / sweep.len();
        assert!(
            rate.abs_diff(SETTLEMENT_REGION_PERCENT) <= 8,
            "{placed} of {} regions held a settlement ({rate}%), against an authored \
             {SETTLEMENT_REGION_PERCENT}%",
            sweep.len()
        );
    }

    /// A settlement stands inside its own region, clear of the border, or
    /// two in neighbouring regions read as one sprawl with a gap in it.
    #[test]
    fn a_settlement_stands_well_inside_its_own_region() {
        let db = shipped();
        for key in keys(10) {
            let Some(p) = settlement_at(4242, &db, key) else {
                continue;
            };
            assert_eq!(region_of(p.cell.0, p.cell.1), key, "{p:?} left its region");
            let (lx, ly) = (
                p.cell.0 - key.rx * REGION_TILES,
                p.cell.1 - key.ry * REGION_TILES,
            );
            for offset in [lx, ly] {
                assert!(
                    (REGION_EDGE_INSET..REGION_TILES - REGION_EDGE_INSET).contains(&offset),
                    "{p:?} sits {offset} into its region, inside the {REGION_EDGE_INSET}-tile inset"
                );
            }
        }
    }

    /// Two worlds are two different maps. Without this the seed is not
    /// reaching the fold at all, and every run would share a layout.
    #[test]
    fn a_different_seed_lays_the_towns_out_differently() {
        let db = shipped();
        let layout = |seed| {
            keys(10)
                .map(|key| settlement_at(seed, &db, key).map(|p| p.cell))
                .collect::<Vec<_>>()
        };
        assert_ne!(layout(4242), layout(99));
    }

    fn game(seed: u32) -> crate::Game {
        crate::Game::new(
            seed,
            crate::DifficultyMode::Forgiving,
            &crate::tests::support::test_assets_dir(),
        )
        .unwrap()
    }

    fn known(
        game: &crate::Game,
    ) -> &std::collections::BTreeMap<SettlementKey, crate::resources::KnownSettlement> {
        &game.world.resource::<crate::resources::Settlements>().0
    }

    /// The feature produces something, which is the assertion a derivation
    /// this indirect most needs: every part of it can be correct while
    /// nothing ever reaches the map.
    #[test]
    fn a_new_run_materializes_the_settlements_around_it() {
        let game = game(4242);
        assert!(
            !known(&game).is_empty(),
            "no settlement materialized anywhere near the party"
        );
    }

    /// A town on unwalkable ground is a town nobody can reach.
    #[test]
    fn every_materialized_settlement_stands_on_ground_you_can_walk_to() {
        for seed in [4242u32, 40, 16, 945, 7] {
            let mut game = game(seed);
            let sites: Vec<(i32, i32)> = known(&game).values().map(|s| s.tile).collect();
            for tile in sites {
                assert!(
                    game.world
                        .resource_mut::<crate::world::WorldMap>()
                        .tile(tile.0, tile.1)
                        .walkable,
                    "seed {seed}: a settlement stands on ground at {tile:?} nobody can reach"
                );
            }
        }
    }

    /// Each settlement is drawn exactly once. The pass runs every tick, so
    /// a missing "already known" check stacks a fresh entity on the tile
    /// each time and reads as the glyph getting brighter.
    #[test]
    fn walking_does_not_materialize_a_settlement_twice() {
        let mut game = game(4242);
        let before = known(&game).len();
        for _ in 0..40 {
            game.tick();
        }
        assert_eq!(known(&game).len(), before, "the record grew on its own");

        let mut query = game
            .world
            .query::<(&crate::components::Settlement, &crate::components::Position)>();
        let drawn: Vec<_> = query
            .iter(&game.world)
            .map(|(s, p)| (s.key, p.x, p.y))
            .collect();
        let distinct: std::collections::BTreeSet<_> = drawn.iter().map(|(k, _, _)| *k).collect();
        assert_eq!(
            drawn.len(),
            distinct.len(),
            "a settlement is drawn more than once: {drawn:?}"
        );
        assert_eq!(distinct.len(), before, "a known settlement is not drawn");
    }

    /// A place the party has walked to has to still be there, at the same
    /// tile, under the same name.
    #[test]
    fn a_settlement_survives_a_save_and_load() {
        let dir = crate::tests::support::scratch_assets_dir("settlement_save");
        std::fs::create_dir_all(&*dir).unwrap();
        let path = dir.join("save.bin");

        let mut game = game(4242);
        let before = known(&game).clone();
        assert!(!before.is_empty(), "test premise: something materialized");
        game.save(&path).unwrap();

        let mut loaded =
            crate::Game::load(&path, &crate::tests::support::test_assets_dir()).unwrap();
        assert_eq!(
            known(&loaded),
            &before,
            "a settlement moved or was forgotten"
        );

        let mut query = loaded.world.query::<&crate::components::Settlement>();
        assert_eq!(
            query.iter(&loaded.world).count(),
            before.len(),
            "a loaded settlement has no entity to draw"
        );
    }

    /// The catalogue is the whole of what a settlement is, so a shipped file
    /// authoring a variant no formula knows about is caught here rather than
    /// on the screen.
    #[test]
    fn every_shipped_settlement_authors_a_known_shape() {
        for def in shipped().iter() {
            assert!(!def.blurb.trim().is_empty(), "{} has no blurb", def.id);
            assert!(
                matches!(def.kind, SettlementKind::Mainframe | SettlementKind::Server),
                "{} has no kind",
                def.id
            );
            assert!(
                matches!(
                    def.specialty,
                    Specialty::Gear
                        | Specialty::Materials
                        | Specialty::Routines
                        | Specialty::Programs
                ),
                "{} has no specialty",
                def.id
            );
            assert!(
                matches!(
                    def.temperament,
                    Temperament::Open | Temperament::Guarded | Temperament::Mercantile
                ),
                "{} has no temperament",
                def.id
            );
        }
    }
}
