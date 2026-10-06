//! What a program makes of where it is standing, loaded from
//! `assets/thoughts/`.
//!
//! **Half data, like perks.** Wording and weight are a file each; what fires
//! a thought is the closed `Trigger` enum, so a modder can reword, reweight
//! or delete a thought but not invent a trigger.
//!
//! **An empty database is valid and inert**, `MemoryDb`'s rule and for its
//! reason: a trigger with no def never fires, so deleting `assets/thoughts/`
//! restores the game as it was before situations existed. An absent
//! directory is therefore silent.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::{Commands, Component, Entity, Query, Res, Resource};
use bevy_ecs::system::SystemParam;

use crate::components::{
    MachineStatus, Memories, MemorySubject, Position, ProgramId, Structure, Tamed, Task, TaskKind,
};
use crate::disposition::Disposition;
use crate::game::party::{self, ProgramRole, Roles};
use crate::memories::MemoryDb;
use crate::resources::GameClock;
use crate::resources::PowerGrid;
use crate::structures::StructureDb;
use crate::tuning::SITUATION_MAX_TOTAL;

/// What can make a program think something about its surroundings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum Trigger {
    BesideRival,
    BesideFriend,
    Unpowered,
    MachineRunning,
    NoAmenity,
}

impl Trigger {
    /// Every variant, for the shipped-asset census. The `match` in
    /// `Trigger::index` is exhaustive, so a new variant fails to compile
    /// until it is placed here too.
    pub const ALL: [Trigger; 5] = [
        Trigger::BesideRival,
        Trigger::BesideFriend,
        Trigger::Unpowered,
        Trigger::MachineRunning,
        Trigger::NoAmenity,
    ];

    #[cfg(test)]
    fn index(self) -> usize {
        match self {
            Trigger::BesideRival => 0,
            Trigger::BesideFriend => 1,
            Trigger::Unpowered => 2,
            Trigger::MachineRunning => 3,
            Trigger::NoAmenity => 4,
        }
    }
}

/// One thought: how a trigger is worded and how hard it weighs.
#[derive(Clone, Debug, Deserialize)]
pub struct ThoughtDef {
    pub trigger: Trigger,
    /// What a screen row leads with.
    pub name: String,
    /// One line of flavour under it, in the player's vocabulary.
    pub blurb: String,
    /// Signed; positive lifts morale.
    pub intensity: f32,
}

/// Every thought the game knows about, one per trigger.
#[derive(Resource, Default)]
pub struct ThoughtDb {
    defs: BTreeMap<Trigger, ThoughtDef>,
}

impl ThoughtDb {
    /// Loads every `*.ron` def in `dir`, `MemoryDb::load_dir`'s shape. Two
    /// files naming one trigger: the first by file name wins and the second
    /// is warned about, so a trigger never counts twice.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = ThoughtDb::default();
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
        // Sorted, so "the first by file name wins" is the same every run.
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<ThoughtDef>(&text) {
                Ok(def) => {
                    // A NaN or infinite weight would poison every morale sum
                    // it reaches, and `f32` parses both from a mod's file.
                    if !def.intensity.is_finite() {
                        warnings.push(format!(
                            "skipped invalid thought file {path:?}: intensity is not finite"
                        ));
                        continue;
                    }
                    if db.defs.contains_key(&def.trigger) {
                        warnings.push(format!(
                            "skipped thought file {path:?}: {:?} already has a thought",
                            def.trigger
                        ));
                        continue;
                    }
                    db.defs.insert(def.trigger, def);
                }
                Err(e) => warnings.push(format!("skipped invalid thought file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, trigger: Trigger) -> Option<&ThoughtDef> {
        self.defs.get(&trigger)
    }
}

/// What one program's surroundings currently make it think: the triggers that
/// hold right now, written by `assess_situation_system` every tick.
///
/// Holds triggers rather than def ids, so a reworded asset needs no
/// migration and the fold resolves through `ThoughtDb` on read. Not saved,
/// `Stranded`'s precedent: the walk that produced it runs again next tick,
/// and a missing one folds as empty.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct Situation {
    pub thoughts: Vec<Trigger>,
}

/// Each active thought with its share of the situational total, after the
/// clamp. A trigger with no def contributes no row.
///
/// When the raw total's magnitude exceeds `SITUATION_MAX_TOTAL` every share
/// is scaled by `cap / |total|`, so the rows still sum to the clamped figure
/// and the memories page and the morale sum are one derivation.
pub(crate) fn scaled_rows<'a>(
    situation: &Situation,
    db: &'a ThoughtDb,
    felt_as: Disposition,
) -> Vec<(&'a ThoughtDef, f32)> {
    let mut rows: Vec<(&ThoughtDef, f32)> = situation
        .thoughts
        .iter()
        .filter_map(|t| db.get(*t))
        .map(|def| (def, felt_as.felt(def.intensity)))
        .collect();
    let total: f32 = rows.iter().map(|(_, v)| v).sum();
    if total.abs() > SITUATION_MAX_TOTAL {
        let scale = SITUATION_MAX_TOTAL / total.abs();
        for (_, v) in &mut rows {
            *v *= scale;
        }
    }
    rows
}

/// The situational term of morale: the sum of `scaled_rows`.
///
/// A free function for `memories::sum_intensity`'s reason: `task_progress_system`
/// has no `Game` to ask, and two folds could disagree about whether an
/// unresolvable trigger counts.
pub(crate) fn sum(situation: &Situation, db: &ThoughtDb, felt_as: Disposition) -> f32 {
    scaled_rows(situation, db, felt_as)
        .iter()
        .map(|(_, v)| v)
        .sum()
}

/// One staff program as `assess` reads it: the values, not the entity, so the
/// system and a test fill the same struct.
pub(crate) struct Body<'a> {
    pub entity: Entity,
    pub pos: Position,
    pub id: ProgramId,
    pub task: Option<(TaskKind, Entity)>,
    pub store: Option<&'a Memories>,
    pub felt_as: Disposition,
}

/// What the base says about a body's surroundings, other than the bodies.
pub(crate) struct Surroundings<'a> {
    pub grid: &'a PowerGrid,
    pub memories: &'a MemoryDb,
    pub now: u64,
    /// `offshift::Amenities::any` over the base's structures.
    pub has_amenity: bool,
    /// A machine's `MachineStatus`; `None` for an entity that has none.
    pub status: &'a dyn Fn(Entity) -> Option<MachineStatus>,
}

/// Whether two tiles are neighbours — Chebyshev distance one. `assess`'s
/// `BesideRival` and `Game::refuses_post` both read it, so the thought and
/// the refusal agree about who is beside whom.
pub(crate) fn is_beside(a: Position, b: Position) -> bool {
    chebyshev(a, b) <= 1
}

/// Tiles between two positions, diagonals counting as one step.
pub(crate) fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

/// How `body` rates `other`: `memories::opinion_about` through `bonds::band`,
/// the call `Game::bond` makes. Every trigger that reads a bond goes through
/// here, so they cannot disagree about who is a rival.
fn bond_band(body: &Body, other: &Body, around: &Surroundings) -> crate::bonds::Bond {
    let opinion = body.store.map_or(0.0, |store| {
        crate::memories::opinion_about(
            store,
            around.memories,
            around.now,
            body.felt_as,
            &MemorySubject::Program(other.id),
            true,
        )
    });
    crate::bonds::band(opinion)
}

/// The other bodies that stand in the base, never the body itself: a posted
/// guard's `Position` is where it was assigned, not a tile it occupies.
fn walking_others<'a, 'b>(
    bodies: &'a [Body<'b>],
    body: &'a Body,
) -> impl Iterator<Item = &'a Body<'b>> {
    bodies.iter().filter(move |other| {
        other.entity != body.entity
            && party::walks_the_base(Some(ProgramRole::Staff), other.task.map(|(k, _)| k))
    })
}

/// Each body's situation, in `bodies` order. The one derivation behind both
/// `assess_situation_system` and any caller that needs an answer outside the
/// schedule.
///
/// **Beside is Chebyshev distance 1, excluding the body itself** — the
/// adjacency `drift_idle_staff` scans — and only bodies `walks_the_base`
/// count as neighbours, since a posted guard's `Position` is wherever it was
/// assigned and not a cell it stands in. The bond is
/// `memories::opinion_about` through `bonds::band`, the call
/// `Game::bond` makes, so the two cannot disagree about who is a rival.
///
/// The machine terms read a `GatherResource` posting only: a guard, digger or
/// builder is not working a machine.
pub(crate) fn assess(bodies: &[Body], around: &Surroundings) -> Vec<Situation> {
    bodies
        .iter()
        .map(|body| {
            let mut thoughts = Vec::new();
            if party::walks_the_base(Some(ProgramRole::Staff), body.task.map(|(k, _)| k)) {
                let neighbours =
                    || walking_others(bodies, body).filter(|o| is_beside(o.pos, body.pos));
                if neighbours().any(|o| bond_band(body, o, around).avoids()) {
                    thoughts.push(Trigger::BesideRival);
                }
                if neighbours().any(|o| bond_band(body, o, around).grieves()) {
                    thoughts.push(Trigger::BesideFriend);
                }
            }
            if let Some((TaskKind::GatherResource, machine)) = body.task {
                if around.grid.is_dark(machine) {
                    thoughts.push(Trigger::Unpowered);
                } else if (around.status)(machine) == Some(MachineStatus::Running) {
                    thoughts.push(Trigger::MachineRunning);
                }
            }
            if !around.has_amenity {
                thoughts.push(Trigger::NoAmenity);
            }
            Situation { thoughts }
        })
        .collect()
}

/// What `assess_situation_system` reads of each candidate. Aliased for the
/// `type_complexity` reason `Needful` is.
type Assessed<'w> = (
    Entity,
    &'w Tamed,
    &'w Position,
    &'w ProgramId,
    Option<&'w Task>,
    Option<&'w Memories>,
    Option<&'w Disposition>,
    Option<&'w Situation>,
);

/// The read-only lookups `assess_situation_system` needs, bundled for
/// `CronjobLookups`' reason: the parameter list would trip clippy's
/// argument-count threshold.
#[derive(SystemParam)]
pub struct AssessLookups<'w, 's> {
    statuses: Query<'w, 's, &'static MachineStatus>,
    sites: Query<'w, 's, (&'static Structure, &'static Position)>,
    structure_db: Res<'w, StructureDb>,
    grid: Res<'w, PowerGrid>,
    memories: Res<'w, MemoryDb>,
    clock: Res<'w, GameClock>,
}

/// Writes every staff program's `Situation`, once a tick, between
/// `idle_machine_system` and `task_progress_system`.
///
/// `MachineStatus` is written later in the chain, so `MachineRunning` reads
/// the previous tick's status — the one-tick lag `Stranded` accepts. The
/// grid (`power_grid_system`) has already run, so `Unpowered` is this tick's.
/// The two machine thoughts are exclusive: a dark machine is never "running",
/// whatever last tick's status says.
pub fn assess_situation_system(
    programs: Query<Assessed>,
    lookups: AssessLookups,
    roles: Roles,
    mut commands: Commands,
) {
    let AssessLookups {
        statuses,
        sites,
        structure_db,
        grid,
        memories,
        clock,
    } = lookups;
    let amenities = crate::game::base::offshift::Amenities::build(
        sites.iter().map(|(s, p)| (&s.kind, p)),
        &structure_db,
    );
    // A program that has left the staff (party, sortie, outpost, study) keeps
    // nothing of the base it stood in: its page and its morale read the
    // `Situation`, and nobody else would ever clear it.
    for (entity, tamed, .., current) in programs.iter() {
        if current.is_some() && roles.of(entity, tamed.owner) != Some(ProgramRole::Staff) {
            commands.entity(entity).remove::<Situation>();
        }
    }
    let mut staff: Vec<(Body, Option<&Situation>)> = programs
        .iter()
        .filter(|(entity, tamed, ..)| roles.of(*entity, tamed.owner) == Some(ProgramRole::Staff))
        .map(|(entity, _, pos, id, task, store, disposition, current)| {
            let body = Body {
                entity,
                pos: *pos,
                id: *id,
                task: task.map(|t| (t.kind, t.target)),
                store,
                felt_as: disposition.copied().unwrap_or_default(),
            };
            (body, current)
        })
        .collect();
    // A total order, not bevy's iteration order: the answer does not depend on
    // the order today, but a reader should not have to prove that.
    staff.sort_by_key(|(body, _)| body.entity);
    let (bodies, current): (Vec<Body>, Vec<Option<&Situation>>) = staff.into_iter().unzip();
    let status = |e: Entity| statuses.get(e).ok().copied();
    let around = Surroundings {
        grid: &grid,
        memories: &memories,
        now: clock.tick,
        has_amenity: amenities.any(),
        status: &status,
    };
    let answers = assess(&bodies, &around);
    for ((body, current), answer) in bodies.iter().zip(current).zip(answers) {
        if current != Some(&answer) {
            commands.entity(body.entity).insert(answer);
        }
    }
}

impl crate::Game {
    /// Runs the assessment once, outside the schedule, so `Game::morale` agrees
    /// with the next tick's fold from the moment a game exists. Runs the same
    /// system the schedule does rather than a second gather of its inputs.
    pub(crate) fn assess_situations(&mut self) {
        use bevy_ecs::system::RunSystemOnce;
        self.world
            .run_system_once(assess_situation_system)
            .expect("the assessment's parameters are all registered at construction");
    }
}

/// A program's whole morale: what it remembers plus what its surroundings
/// make it think.
///
/// **The one derivation behind `Game::morale` and `CycleModifiers::morale`**,
/// `memories::sum_intensity`'s reason one level up: `task_progress_system`
/// has no `Game` to ask, and two sums of the same two terms would eventually
/// disagree about one of them. A missing `Memories` or `Situation` folds as
/// empty, never as a panic.
pub(crate) fn morale(
    store: Option<&Memories>,
    situation: Option<&Situation>,
    memories: &MemoryDb,
    thoughts: &ThoughtDb,
    now: u64,
    felt_as: Disposition,
) -> f32 {
    let remembered = store.map_or(0.0, |store| {
        crate::memories::sum_intensity(
            store,
            memories,
            now,
            felt_as,
            crate::memories::Read::Morale,
            |_| true,
        )
    });
    let situational = situation.map_or(0.0, |s| sum(s, thoughts, felt_as));
    remembered + situational
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def_text(trigger: &str, name: &str) -> String {
        format!(
            "(\n    trigger: {trigger},\n    name: \"{name}\",\n    blurb: \"b\",\n    \
             intensity: -3.0,\n)\n"
        )
    }

    fn load(files: &[(&str, String)]) -> (ThoughtDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir("thoughts");
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        ThoughtDb::load_dir(&dir).unwrap()
    }

    fn shipped() -> ThoughtDb {
        let dir = crate::tests::support::test_assets_dir().join("thoughts");
        ThoughtDb::load_dir(&dir).unwrap().0
    }

    fn all_negative() -> Situation {
        Situation {
            thoughts: vec![Trigger::BesideRival, Trigger::Unpowered, Trigger::NoAmenity],
        }
    }

    #[test]
    fn a_raw_total_past_the_cap_is_clamped_to_it() {
        let db = shipped();
        // -3 + -2 + -2 = -7 raw.
        let s = all_negative();
        let total = sum(&s, &db, Disposition::default());
        assert!((total + SITUATION_MAX_TOTAL).abs() < 1e-5, "{total}");
    }

    #[test]
    fn under_the_cap_nothing_is_scaled() {
        let db = shipped();
        let s = Situation {
            thoughts: vec![Trigger::BesideRival, Trigger::MachineRunning],
        };
        assert!((sum(&s, &db, Disposition::default()) + 2.0).abs() < 1e-5);
    }

    #[test]
    fn rows_sum_to_the_total_under_the_clamp() {
        let db = shipped();
        let s = all_negative();
        let rows = scaled_rows(&s, &db, Disposition::default());
        assert_eq!(rows.len(), 3);
        let by_rows: f32 = rows.iter().map(|(_, v)| v).sum();
        assert_eq!(by_rows, sum(&s, &db, Disposition::default()));
        assert!(rows.iter().all(|(_, v)| *v < 0.0));
    }

    #[test]
    fn an_unresolved_trigger_contributes_nothing() {
        let (db, _) = load(&[("a.ron", def_text("Unpowered", "Dark"))]);
        let s = Situation {
            thoughts: vec![Trigger::BesideRival, Trigger::Unpowered],
        };
        let rows = scaled_rows(&s, &db, Disposition::default());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0.trigger, Trigger::Unpowered);
    }

    #[test]
    fn an_empty_db_sums_to_zero() {
        let s = all_negative();
        assert_eq!(sum(&s, &ThoughtDb::default(), Disposition::default()), 0.0);
        assert!(scaled_rows(&s, &ThoughtDb::default(), Disposition::default()).is_empty());
    }

    #[test]
    fn an_abrasive_program_feels_a_negative_thought_heavier() {
        let db = shipped();
        let s = Situation {
            thoughts: vec![Trigger::BesideRival],
        };
        let neutral = sum(&s, &db, Disposition::default());
        let abrasive = sum(&s, &db, Disposition::Abrasive);
        assert!(abrasive < neutral, "{abrasive} vs {neutral}");
    }

    #[test]
    fn a_well_formed_def_loads() {
        let (db, warnings) = load(&[("a.ron", def_text("BesideRival", "Beside a rival"))]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let def = db.get(Trigger::BesideRival).expect("loaded");
        assert_eq!(def.name, "Beside a rival");
        assert_eq!(def.intensity, -3.0);
        assert!(db.get(Trigger::Unpowered).is_none());
    }

    #[test]
    fn a_malformed_file_is_skipped_without_losing_its_neighbours() {
        let (db, warnings) = load(&[
            ("a_bad.ron", "(trigger: Nope".to_string()),
            ("b_good.ron", def_text("Unpowered", "Dark")),
        ]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("a_bad.ron"), "{warnings:?}");
        assert!(db.get(Trigger::Unpowered).is_some());
    }

    #[test]
    fn a_second_def_for_one_trigger_is_warned_and_the_first_kept() {
        let (db, warnings) = load(&[
            ("a.ron", def_text("NoAmenity", "First")),
            ("b.ron", def_text("NoAmenity", "Second")),
        ]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("b.ron"), "{warnings:?}");
        assert_eq!(db.get(Trigger::NoAmenity).unwrap().name, "First");
    }

    #[test]
    fn an_absent_directory_is_empty_and_silent() {
        let dir = crate::tests::support::scratch_assets_dir("thoughts_absent");
        let (db, warnings) = ThoughtDb::load_dir(&dir).unwrap();
        assert!(warnings.is_empty());
        assert!(Trigger::ALL.iter().all(|t| db.get(*t).is_none()));
    }

    #[test]
    fn all_lists_every_variant_once_in_index_order() {
        for (i, t) in Trigger::ALL.iter().enumerate() {
            assert_eq!(t.index(), i);
        }
    }

    // ---- assess ----

    use crate::components::Memory;
    use crate::memories::MemoryId;
    use bevy_ecs::world::World;

    const NOW: u64 = 100;

    fn store_of(def: &str, about: u32, strikes: u32) -> Memories {
        Memories(vec![Memory {
            def: MemoryId::from(def),
            subject: MemorySubject::Program(ProgramId(about)),
            subject_name: None,
            reinforced: NOW,
            strikes,
        }])
    }

    fn rival_store(about: u32) -> Memories {
        store_of("turned_on_me", about, 1)
    }

    fn friend_store(about: u32) -> Memories {
        store_of("bonded_in_battle", about, 2)
    }

    fn entities(n: usize) -> Vec<Entity> {
        let mut world = World::new();
        (0..n).map(|_| world.spawn_empty().id()).collect()
    }

    fn body<'a>(entity: Entity, id: u32, at: (i32, i32), store: Option<&'a Memories>) -> Body<'a> {
        Body {
            entity,
            pos: Position { x: at.0, y: at.1 },
            id: ProgramId(id),
            task: None,
            store,
            felt_as: Disposition::default(),
        }
    }

    /// Runs `assess` with a base that has an amenity, so `NoAmenity` stays out
    /// of the way unless a test asks for it.
    fn assess_with(
        bodies: &[Body],
        grid: &PowerGrid,
        status: &dyn Fn(Entity) -> Option<MachineStatus>,
        has_amenity: bool,
    ) -> Vec<Situation> {
        let db = MemoryDb::load_dir(&crate::tests::support::test_assets_dir().join("memories"))
            .unwrap()
            .0;
        assess(
            bodies,
            &Surroundings {
                grid,
                memories: &db,
                now: NOW,
                has_amenity,
                status,
            },
        )
    }

    fn plain(bodies: &[Body]) -> Vec<Situation> {
        assess_with(bodies, &PowerGrid::default(), &|_| None, true)
    }

    #[test]
    fn a_rival_on_a_diagonal_fires_and_two_tiles_away_does_not() {
        let e = entities(2);
        let store = rival_store(2);
        let near = [
            body(e[0], 1, (5, 5), Some(&store)),
            body(e[1], 2, (6, 6), None),
        ];
        assert_eq!(plain(&near)[0].thoughts, vec![Trigger::BesideRival]);
        let far = [
            body(e[0], 1, (5, 5), Some(&store)),
            body(e[1], 2, (7, 5), None),
        ];
        assert!(plain(&far)[0].thoughts.is_empty());
    }

    #[test]
    fn a_friend_beside_fires_and_the_other_side_of_the_room_does_not() {
        let e = entities(2);
        let store = friend_store(2);
        let near = [
            body(e[0], 1, (5, 5), Some(&store)),
            body(e[1], 2, (5, 4), None),
        ];
        assert_eq!(plain(&near)[0].thoughts, vec![Trigger::BesideFriend]);
        let far = [
            body(e[0], 1, (5, 5), Some(&store)),
            body(e[1], 2, (5, 8), None),
        ];
        assert!(plain(&far)[0].thoughts.is_empty());
    }

    #[test]
    fn the_bond_is_the_holders_view_and_a_neutral_neighbour_fires_neither() {
        let e = entities(2);
        let store = rival_store(2);
        // Only program 1 holds the grudge, so program 2 beside it reads
        // nothing.
        let bodies = [
            body(e[0], 1, (5, 5), Some(&store)),
            body(e[1], 2, (5, 6), None),
        ];
        let got = plain(&bodies);
        assert_eq!(got[0].thoughts, vec![Trigger::BesideRival]);
        assert!(got[1].thoughts.is_empty());
        // A neighbour nobody has a memory of is neutral.
        let empty = Memories::default();
        let neutral = [
            body(e[0], 1, (5, 5), Some(&empty)),
            body(e[1], 2, (5, 6), Some(&empty)),
        ];
        assert!(plain(&neutral).iter().all(|s| s.thoughts.is_empty()));
    }

    #[test]
    fn a_body_is_never_beside_itself() {
        let e = entities(1);
        let store = rival_store(1);
        let alone = [body(e[0], 1, (5, 5), Some(&store))];
        assert!(plain(&alone)[0].thoughts.is_empty());
    }

    #[test]
    fn a_guard_is_not_a_neighbour() {
        let e = entities(3);
        let store = rival_store(2);
        let mut guard = body(e[1], 2, (5, 6), None);
        guard.task = Some((TaskKind::Guard, e[2]));
        let bodies = [body(e[0], 1, (5, 5), Some(&store)), guard];
        assert!(plain(&bodies)[0].thoughts.is_empty());
    }

    fn posted(e: &[Entity]) -> Vec<Body<'static>> {
        let mut b = body(e[0], 1, (5, 5), None);
        b.task = Some((TaskKind::GatherResource, e[1]));
        vec![b]
    }

    #[test]
    fn a_dark_machine_is_unpowered_and_a_powered_one_is_not() {
        let e = entities(2);
        let mut grid = PowerGrid::default();
        grid.dark.insert(e[1]);
        let running = |_: Entity| Some(MachineStatus::Unpowered);
        let dark = assess_with(&posted(&e), &grid, &running, true);
        assert_eq!(dark[0].thoughts, vec![Trigger::Unpowered]);
        let lit = assess_with(&posted(&e), &PowerGrid::default(), &running, true);
        assert!(lit[0].thoughts.is_empty());
    }

    #[test]
    fn a_non_finite_intensity_is_skipped_with_a_warning() {
        let (db, warnings) = load(&[
            (
                "a.ron",
                def_text("Unpowered", "Dark").replace("-3.0", "inf"),
            ),
            (
                "b.ron",
                def_text("NoAmenity", "Bare").replace("-3.0", "NaN"),
            ),
        ]);
        assert!(db.get(Trigger::Unpowered).is_none());
        assert!(db.get(Trigger::NoAmenity).is_none());
        assert!(
            warnings.iter().all(|w| w.contains("intensity")),
            "{warnings:?}"
        );
        assert_eq!(warnings.len(), 2, "{warnings:?}");
    }

    #[test]
    fn a_dark_machine_that_ran_last_tick_is_only_unpowered() {
        let e = entities(2);
        let mut grid = PowerGrid::default();
        grid.dark.insert(e[1]);
        let t = assess_with(&posted(&e), &grid, &|_| Some(MachineStatus::Running), true);
        assert_eq!(t[0].thoughts, vec![Trigger::Unpowered]);
    }

    #[test]
    fn running_fires_and_starved_does_not() {
        let e = entities(2);
        let grid = PowerGrid::default();
        let running = assess_with(&posted(&e), &grid, &|_| Some(MachineStatus::Running), true);
        assert_eq!(running[0].thoughts, vec![Trigger::MachineRunning]);
        let starved = assess_with(&posted(&e), &grid, &|_| Some(MachineStatus::Starved), true);
        assert!(starved[0].thoughts.is_empty());
    }

    #[test]
    fn an_unposted_body_has_no_machine_thought() {
        let e = entities(1);
        let grid = PowerGrid::default();
        let idle = [body(e[0], 1, (5, 5), None)];
        let got = assess_with(&idle, &grid, &|_| Some(MachineStatus::Running), true);
        assert!(got[0].thoughts.is_empty());
    }

    #[test]
    fn no_amenity_fires_only_when_the_base_has_none() {
        let e = entities(1);
        let idle = [body(e[0], 1, (5, 5), None)];
        let grid = PowerGrid::default();
        let none = assess_with(&idle, &grid, &|_| None, false);
        assert_eq!(none[0].thoughts, vec![Trigger::NoAmenity]);
        let some = assess_with(&idle, &grid, &|_| None, true);
        assert!(some[0].thoughts.is_empty());
    }
}
