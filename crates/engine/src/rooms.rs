//! Rooms: enclosed spaces detected from the base grid, given a role by what
//! stands in them and graded for quality — `assets/rooms/*.ron`.
//!
//! **Derived, never stored.** `detect` is a pure function of the grid and the
//! placed structures, recomputed whenever it is needed (`seams-base`: a cell's
//! kind is derived). `of_world` is the one place the world's pieces are
//! gathered into `detect`'s inputs.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use bevy_ecs::prelude::{Resource, World};
use serde::Deserialize;

use crate::base_grid::BaseGrid;
use crate::components::{Position, Structure};
use crate::floors::FloorDb;
use crate::structures::{StructureDb, StructureDef};
use crate::tuning::{
    ROOM_AREA_FULL, ROOM_AREA_WEIGHT, ROOM_BAND_FINE, ROOM_BAND_PLAIN, ROOM_BAND_SUPERB,
    ROOM_CROWD_FREE, ROOM_CROWD_WEIGHT, ROOM_FINISH_WEIGHT,
};

pub type RoomId = String;

/// One tag a room must hold, counted over the structure anchors inside it.
#[derive(Clone, Debug, Deserialize)]
pub struct TagCount {
    pub tag: String,
    pub min: u32,
    #[serde(default)]
    pub max: Option<u32>,
}

impl TagCount {
    fn satisfied_by(&self, count: u32) -> bool {
        count >= self.min && self.max.is_none_or(|m| count <= m)
    }
}

/// One room role, as authored in `assets/rooms/`.
#[derive(Clone, Debug, Deserialize)]
pub struct RoomDef {
    pub id: RoomId,
    pub name: String,
    /// The highest-priority matching def wins; a tie goes to the lower id.
    pub priority: i32,
    /// Every entry must be met. An empty list matches nothing.
    pub requires: Vec<TagCount>,
    /// Roommate thoughts apply in a living room.
    #[serde(default)]
    pub living: bool,
    /// Alt-overlay colour, as RGB so a mod needs no palette key.
    #[serde(default)]
    pub tint: (u8, u8, u8),
}

/// Every room role the install knows about, loaded from `assets/rooms/`.
#[derive(Resource, Default, Clone)]
pub struct RoomDb {
    defs: BTreeMap<RoomId, RoomDef>,
}

impl RoomDb {
    /// `FloorDb::load_dir`'s shape: an absent directory is the empty,
    /// supported catalogue (no room has a role); a malformed file is skipped
    /// with one warning.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = RoomDb::default();
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        // Sorted so two files claiming one id resolve the same way every run.
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<RoomDef>(&text) {
                Ok(def) => {
                    db.defs.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid room file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&RoomDef> {
        self.defs.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RoomDef> {
        self.defs.values()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RoomBand {
    Cramped,
    Plain,
    Fine,
    Superb,
}

/// A structure standing in base space: its anchor and its def.
pub struct PlacedStructure<'a> {
    pub at: (i32, i32),
    pub def: &'a StructureDef,
}

#[derive(Clone, Debug)]
pub struct Room {
    /// Sorted, so a room is the same value however it was found.
    pub cells: Vec<(i32, i32)>,
    /// `None` for the commons and for space no `RoomDef` matches.
    pub role: Option<RoomId>,
    pub living: bool,
    pub band: RoomBand,
}

#[derive(Clone, Debug, Default)]
pub struct Rooms {
    pub rooms: Vec<Room>,
    by_cell: HashMap<(i32, i32), usize>,
}

impl Rooms {
    /// The room standing on `(x, y)`; a boundary cell (rock, wall, door) is
    /// in none.
    pub fn room_at(&self, x: i32, y: i32) -> Option<&Room> {
        self.by_cell.get(&(x, y)).map(|&i| &self.rooms[i])
    }

    pub fn same_room(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        match (self.by_cell.get(&a), self.by_cell.get(&b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    }
}

/// Quality score from the room's area, how many cells carry a comfort finish
/// and how many structure anchors it holds. Pure, so the weights are
/// testable without a grid.
pub fn quality(area: usize, finished: usize, anchors: usize) -> f32 {
    if area == 0 {
        return 0.0;
    }
    let cells = area as f32;
    let area_term = (cells / ROOM_AREA_FULL).min(1.0);
    let finish = finished as f32 / cells;
    let crowd = (anchors as f32 / cells - ROOM_CROWD_FREE).max(0.0);
    ROOM_AREA_WEIGHT * area_term + ROOM_FINISH_WEIGHT * finish - ROOM_CROWD_WEIGHT * crowd
}

pub fn band_of(score: f32) -> RoomBand {
    if score < ROOM_BAND_PLAIN {
        RoomBand::Cramped
    } else if score < ROOM_BAND_FINE {
        RoomBand::Plain
    } else if score < ROOM_BAND_SUPERB {
        RoomBand::Fine
    } else {
        RoomBand::Superb
    }
}

fn role_of<'d>(anchors: &[&PlacedStructure], rooms: &'d RoomDb) -> Option<&'d RoomDef> {
    let count = |tag: &str| {
        anchors
            .iter()
            .filter(|p| p.def.room_tags.iter().any(|t| t == tag))
            .count() as u32
    };
    rooms
        .iter()
        .filter(|d| !d.requires.is_empty())
        .filter(|d| d.requires.iter().all(|r| r.satisfied_by(count(&r.tag))))
        // The lower id wins a priority tie, so the result never depends on
        // iteration order.
        .min_by_key(|d| (std::cmp::Reverse(d.priority), d.id.as_str()))
}

/// Detects every room on `grid`. A boundary is rock, a barrier's anchor or a
/// door's anchor; a room is a 4-connected set of walkable non-boundary cells,
/// and only a non-boundary structure's anchor counts toward its contents.
/// The region holding `home` is the commons and never gets a role.
pub fn detect(
    grid: &BaseGrid,
    placed: &[PlacedStructure],
    rooms: &RoomDb,
    floors: &FloorDb,
    home: (i32, i32),
) -> Rooms {
    let boundary: BTreeSet<(i32, i32)> = placed
        .iter()
        .filter(|p| p.def.barrier || p.def.door)
        .map(|p| p.at)
        .collect();
    let open = |c: (i32, i32)| grid.walkable(c.0, c.1) && !boundary.contains(&c);
    let mut found = Rooms::default();
    for (&start, _) in grid.iter() {
        if found.by_cell.contains_key(&start) || !open(start) {
            continue;
        }
        let index = found.rooms.len();
        let mut cells = vec![start];
        found.by_cell.insert(start, index);
        let mut next = 0;
        while next < cells.len() {
            let (x, y) = cells[next];
            next += 1;
            for n in [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)] {
                if open(n) && !found.by_cell.contains_key(&n) {
                    found.by_cell.insert(n, index);
                    cells.push(n);
                }
            }
        }
        cells.sort_unstable();
        let anchors: Vec<&PlacedStructure> = placed
            .iter()
            .filter(|p| found.by_cell.get(&p.at) == Some(&index) && open(p.at))
            .collect();
        let finished = cells
            .iter()
            .filter(|c| {
                grid.finish_at(c.0, c.1)
                    .and_then(|id| floors.get(id))
                    .is_some_and(|f| f.comfort.is_some())
            })
            .count();
        let role = if cells.contains(&home) {
            None
        } else {
            role_of(&anchors, rooms)
        };
        found.rooms.push(Room {
            band: band_of(quality(cells.len(), finished, anchors.len())),
            living: role.is_some_and(|d| d.living),
            role: role.map(|d| d.id.clone()),
            cells,
        });
    }
    found
}

/// `PlacedStructure`s for every structure that resolves in `db`. The one
/// builder, so a system and a `Game` method cannot disagree on what is placed.
pub fn placed_structures<'a>(
    sites: impl Iterator<Item = (&'a Structure, &'a Position)>,
    db: &'a StructureDb,
) -> Vec<PlacedStructure<'a>> {
    sites
        .filter_map(|(s, p)| {
            db.get(s.kind.as_str()).map(|def| PlacedStructure {
                at: (p.x, p.y),
                def,
            })
        })
        .collect()
}

/// The rooms of the live world. A world with no Home has no commons to
/// anchor, so no base and no rooms.
pub fn of_world(world: &World) -> Rooms {
    let (Some(grid), Some(structures), Some(rooms), Some(floors)) = (
        world.get_resource::<BaseGrid>(),
        world.get_resource::<StructureDb>(),
        world.get_resource::<RoomDb>(),
        world.get_resource::<FloorDb>(),
    ) else {
        return Rooms::default();
    };
    let sites: Vec<(&Structure, &Position)> = world
        .iter_entities()
        .filter_map(|e| Some((e.get::<Structure>()?, e.get::<Position>()?)))
        .collect();
    let placed = placed_structures(sites.iter().copied(), structures);
    let Some(home) = placed
        .iter()
        .find(|p| p.def.id == crate::HOME_STRUCTURE_ID)
        .map(|p| p.at)
    else {
        return Rooms::default();
    };
    detect(grid, &placed, rooms, floors, home)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::support::{scratch_assets_dir, test_assets_dir};

    fn def(id: &str, tags: &[&str], barrier: bool, door: bool) -> StructureDef {
        let tags: Vec<String> = tags.iter().map(|t| format!("{t:?}")).collect();
        ron::from_str(&format!(
            "(id: {id:?}, name: \"x\", description: \"x\", glyph: 'x', color: White, \
             build_cost: [], work: None, barrier: {barrier}, door: {door}, room_tags: [{}])",
            tags.join(",")
        ))
        .unwrap()
    }

    fn shipped() -> (RoomDb, FloorDb) {
        let (rooms, w) = RoomDb::load_dir(&test_assets_dir().join("rooms")).unwrap();
        assert!(w.is_empty(), "{w:?}");
        let (floors, _) = FloorDb::load_dir(&test_assets_dir().join("floors")).unwrap();
        (rooms, floors)
    }

    /// A `w` by `h` floored block at the origin.
    fn block(w: i32, h: i32) -> BaseGrid {
        let mut g = BaseGrid::default();
        for x in 0..w {
            for y in 0..h {
                g.lay_floor(x, y);
            }
        }
        g
    }

    fn run(grid: &BaseGrid, placed: &[(&StructureDef, (i32, i32))], home: (i32, i32)) -> Rooms {
        let (rooms, floors) = shipped();
        let placed: Vec<_> = placed
            .iter()
            .map(|(def, at)| PlacedStructure { at: *at, def })
            .collect();
        detect(grid, &placed, &rooms, &floors, home)
    }

    fn role(r: &Rooms, c: (i32, i32)) -> Option<&str> {
        r.room_at(c.0, c.1)?.role.as_deref()
    }

    #[test]
    fn rock_encloses_a_room() {
        let g = block(3, 3);
        let r = run(&g, &[], (99, 99));
        assert_eq!(r.rooms.len(), 1);
        assert_eq!(r.rooms[0].cells.len(), 9);
        assert!(r.room_at(5, 5).is_none());
    }

    #[test]
    fn a_door_splits_a_corridor_from_a_room_and_a_wall_splits_two_rooms() {
        let g = block(5, 1);
        let door = def("door", &[], false, true);
        let wall = def("wall", &[], true, false);
        let r = run(&g, &[(&door, (2, 0))], (99, 99));
        assert_eq!(r.rooms.len(), 2);
        assert!(!r.same_room((1, 0), (3, 0)));
        assert!(r.room_at(2, 0).is_none());
        let r = run(&g, &[(&wall, (2, 0))], (99, 99));
        assert_eq!(r.rooms.len(), 2);
        assert!(r.room_at(2, 0).is_none());
    }

    #[test]
    fn open_cells_count_toward_a_room() {
        let mut g = block(2, 1);
        g.open(2, 0, 0);
        let r = run(&g, &[], (99, 99));
        assert_eq!(r.rooms.len(), 1);
        assert_eq!(r.rooms[0].cells.len(), 3);
    }

    #[test]
    fn the_commons_is_kept_but_never_gets_a_role() {
        let g = block(4, 4);
        let home = def("home", &[], false, false);
        let bed = def("defrag_bay", &["bed"], false, false);
        let r = run(&g, &[(&home, (0, 0)), (&bed, (1, 1))], (0, 0));
        let room = r.room_at(1, 1).unwrap();
        assert!(room.role.is_none());
        assert!(!room.living);
    }

    #[test]
    fn a_non_barrier_anchor_stays_in_its_room() {
        let g = block(3, 1);
        let bed = def("defrag_bay", &["bed"], false, false);
        let r = run(&g, &[(&bed, (1, 0))], (99, 99));
        assert_eq!(r.rooms.len(), 1);
        assert!(r.room_at(1, 0).is_some());
        assert_eq!(role(&r, (1, 0)), Some("quarters"));
    }

    #[test]
    fn one_bed_is_quarters_and_two_are_a_dormitory() {
        let g = block(4, 1);
        let bed = def("defrag_bay", &["bed"], false, false);
        let one = run(&g, &[(&bed, (0, 0))], (99, 99));
        assert_eq!(role(&one, (0, 0)), Some("quarters"));
        assert!(one.room_at(0, 0).unwrap().living);
        let two = run(&g, &[(&bed, (0, 0)), (&bed, (1, 0))], (99, 99));
        assert_eq!(role(&two, (0, 0)), Some("dormitory"));
    }

    #[test]
    fn a_bed_outranks_a_lathe_and_recreation_outranks_a_workshop() {
        let g = block(4, 1);
        let bed = def("defrag_bay", &["bed"], false, false);
        let lathe = def("lathe", &["workshop"], false, false);
        let sandbox = def("sandbox", &["recreation"], false, false);
        let r = run(&g, &[(&bed, (0, 0)), (&lathe, (1, 0))], (99, 99));
        assert_eq!(role(&r, (0, 0)), Some("quarters"));
        let r = run(&g, &[(&sandbox, (0, 0)), (&lathe, (1, 0))], (99, 99));
        assert_eq!(role(&r, (0, 0)), Some("rec_room"));
        let r = run(&g, &[(&lathe, (1, 0))], (99, 99));
        assert_eq!(role(&r, (0, 0)), Some("workshop"));
        assert!(!r.room_at(0, 0).unwrap().living);
    }

    #[test]
    fn space_matching_no_role_is_roleless() {
        let g = block(2, 2);
        let wall = def("wall", &[], true, false);
        let r = run(&g, &[(&wall, (0, 0))], (99, 99));
        assert_eq!(role(&r, (1, 1)), None);
        assert!(r.room_at(1, 1).is_some());
    }

    #[test]
    fn a_priority_tie_goes_to_the_lower_id() {
        let dir = scratch_assets_dir("rooms_tie");
        std::fs::create_dir_all(&dir).unwrap();
        for id in ["b_room", "a_room"] {
            std::fs::write(
                dir.join(format!("{id}.ron")),
                format!("(id: {id:?}, name: \"n\", priority: 5, requires: [(tag: \"t\", min: 1)])"),
            )
            .unwrap();
        }
        let (rooms, _) = RoomDb::load_dir(&dir).unwrap();
        let (_, floors) = shipped();
        let t = def("thing", &["t"], false, false);
        let g = block(2, 1);
        let placed = [PlacedStructure {
            at: (0, 0),
            def: &t,
        }];
        let r = detect(&g, &placed, &rooms, &floors, (99, 99));
        assert_eq!(role(&r, (0, 0)), Some("a_room"));
    }

    #[test]
    fn bands_flip_at_each_threshold_from_both_sides() {
        assert_eq!(band_of(ROOM_BAND_PLAIN - 0.001), RoomBand::Cramped);
        assert_eq!(band_of(ROOM_BAND_PLAIN), RoomBand::Plain);
        assert_eq!(band_of(ROOM_BAND_FINE - 0.001), RoomBand::Plain);
        assert_eq!(band_of(ROOM_BAND_FINE), RoomBand::Fine);
        assert_eq!(band_of(ROOM_BAND_SUPERB - 0.001), RoomBand::Fine);
        assert_eq!(band_of(ROOM_BAND_SUPERB), RoomBand::Superb);
    }

    #[test]
    fn quality_weighs_area_finish_and_crowding() {
        // A bare 16-cell room is Plain, half-finished is Fine, fully
        // finished is Superb.
        assert_eq!(band_of(quality(16, 0, 0)), RoomBand::Plain);
        assert_eq!(band_of(quality(16, 8, 0)), RoomBand::Fine);
        assert_eq!(band_of(quality(16, 16, 0)), RoomBand::Superb);
        assert_eq!(band_of(quality(4, 0, 0)), RoomBand::Cramped);
        // Anchors up to the free share cost nothing; beyond it they do.
        assert_eq!(quality(16, 0, 4), quality(16, 0, 0));
        assert!(quality(16, 0, 8) < quality(16, 0, 4));
        assert_eq!(band_of(quality(16, 16, 8)), RoomBand::Fine);
    }

    #[test]
    fn a_finished_room_detects_as_a_finer_band() {
        let mut g = block(4, 4);
        let bed = def("defrag_bay", &["bed"], false, false);
        let plain = run(&g, &[(&bed, (0, 0))], (99, 99));
        assert_eq!(plain.room_at(0, 0).unwrap().band, RoomBand::Plain);
        let (_, floors) = shipped();
        let comfy = floors.iter().find(|f| f.comfort.is_some()).unwrap();
        for (x, y) in plain.rooms[0].cells.clone() {
            g.set_finish(x, y, comfy.id.clone());
        }
        let r = run(&g, &[(&bed, (0, 0))], (99, 99));
        assert_eq!(r.room_at(0, 0).unwrap().band, RoomBand::Superb);
    }

    #[test]
    fn an_absent_directory_is_empty_and_a_malformed_file_is_skipped() {
        let dir = scratch_assets_dir("rooms_absent");
        let (db, w) = RoomDb::load_dir(&dir).unwrap();
        assert!(db.is_empty() && w.is_empty());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.ron"), "(id: \"x\", oh no").unwrap();
        std::fs::copy(
            test_assets_dir().join("rooms/quarters.ron"),
            dir.join("quarters.ron"),
        )
        .unwrap();
        let (db, w) = RoomDb::load_dir(&dir).unwrap();
        assert_eq!(db.iter().count(), 1);
        assert_eq!(w.len(), 1);
    }
}
