//! What one creature is in a file — the gate on `Game::creature_save_for`.
//!
//! The builder is a single exhaustive struct literal, so a *missing* field is
//! a compile error and needs no test. What no compiler catches is a field
//! wired to the wrong read: `hp` taking `max_hp`, `staff` answered from the
//! party list, a tether resolved against the wrong entity. Every one of those
//! writes a save that loads, parses, and quietly describes a different
//! program than the one the player had.
//!
//! So the census below asserts a *value* on every field, and the values are
//! all different from each other on purpose — two fields holding the same
//! number cannot catch being crossed.
//!
//! The values are hand-written; **which fields exist is not**. The census
//! opens with an exhaustive destructure of `save::CreatureSave` carrying no
//! rest pattern, so a field added to the struct stops this file compiling
//! until somebody writes the assertion for it. Without that line the trade
//! was "covered only if somebody remembers", and what rides on the answer
//! grew: `creature_save_for` and `spawn_creature_from_save` are no longer
//! only the save path — they are also how a committed program's snapshot is
//! taken and given back, so a field nobody covered is a refunded program
//! that returns short its gear or its memories.

use super::support::*;
use crate::components::{
    Boss, BoughtStats, Carrying, Disgruntled, Downed, FusionCount, Grievance, KernelRing, Memories,
    Memory, MemorySubject, Needs, NestGuardian, OffShift, Potential, PowerReserve, PurchasedTiers,
    Pursuing, Refactors, Talents, TownPatrol, ZonePortal,
};
use crate::game::lifecycle::CreatureRestore;
use crate::*;

/// The five shapes a creature takes on disk, one entity each.
///
/// Split rather than piled onto one program because four of the fields are
/// **roles**, and the roles are mutually exclusive by construction (see
/// `ProgramRole`): nothing is in the party and wielded and away and on the
/// staff at once. A single-program fixture would have to leave three of the
/// four at their defaults, and could not tell a builder that answers all
/// four from one source apart from one that reads each.
struct Roster {
    /// In the party, and carrying every per-entity component a save reads.
    member: Entity,
    /// On the base staff, hauling, off its post and downed — the states only
    /// a working program can be in.
    worker: Entity,
    /// The wielded program.
    weapon: Entity,
    /// Away on a sortie.
    scout: Entity,
    /// Wild: no `Tamed`, so every owned-program field falls to its default,
    /// which is the other half of what the census checks.
    sentry: Entity,
}

/// Every creature is named, so a census can find its row in a file where
/// entity ids no longer exist — `CustomName` is the only identity that
/// survives a save, which is why `Game::load` leans on it too.
///
/// Deliberately holds nothing resolved **by position**: a cronjob target, a
/// nest and a town are entities the load path has to find again on their
/// tiles, and standing in for one takes a real structure rather than a
/// component insert. Those live in `add_the_position_tethers` below, so the
/// round-trip test can seed this much and stay honest.
fn seed_the_roster(game: &mut Game) -> Roster {
    let member = spawn_tamed(game, 30, 6);
    let worker = spawn_tamed(game, 18, 4);
    let weapon = spawn_tamed(game, 22, 5);
    let scout = spawn_tamed(game, 25, 7);
    let sentry = spawn_wild_on_player_tile(game);

    // Everything that goes through an engine verb happens first: `enlist`,
    // `wear` and `wield_program` each tick, and a tick drains needs and
    // power. The hand-set values below would otherwise be the fixture's
    // numbers minus however much of a turn the setup cost.
    enlist(game, member);
    set_level(game, member, 6);
    wear(game, member, ids::OVERCLOCK_CORE);
    wear(game, member, ids::ABLATIVE_PLATING);
    game.wield_program(weapon).expect("wielding a program");

    // A sortie filed straight into the resource rather than through
    // `dispatch_sortie`, which wants a Relay standing and the stock to
    // provision it. `sortie_index` is written off this list and nothing
    // else, so the record only has to exist.
    let site = game
        .world
        .resource::<crate::sorties::SortieDb>()
        .iter()
        .next()
        .expect("the game ships sortie sites")
        .clone();
    game.world
        .resource_mut::<crate::resources::Sorties>()
        .0
        .push(crate::resources::Sortie {
            site,
            risk: 2,
            members: vec![scout],
            ticks_total: 40,
            ticks_elapsed: 3,
            battles_total: 5,
            battles_done: 1,
            aborted: false,
            loot: Vec::new(),
            programs: Vec::new(),
            xp: 0,
            kills: 0,
            casualties: Vec::new(),
        });

    let need = a_shipped_need(game);
    let mut memories = Memories::default();
    memories.0.push(Memory {
        def: crate::memories::MemoryId::from("a_memory"),
        subject: MemorySubject::BaseTile { x: 2, y: 9 },
        subject_name: Some("the mill".to_string()),
        reinforced: 12,
        strikes: 3,
    });

    game.world.entity_mut(member).insert((
        CustomName("Sable".to_string()),
        ZonePortal(4),
        PowerReserve::new(41.5),
        Potential {
            hp_roll: 1.11,
            atk_roll: 1.22,
            def_roll: 1.33,
            growth_roll: 1.44,
            assembly_roll: 1.55,
            extraction_roll: 1.66,
        },
        FusionCount(2),
        Refactors(3),
        PurchasedTiers(4),
        KernelRing(5),
        Talents(vec![crate::talents::TalentId::from("a_talent")]),
        BoughtStats {
            atk: 6,
            mitigation: 7,
            max_hp: 8,
            ever_bought: 10,
        },
        Rarity::Gold,
        Nemesis(9),
        crate::disposition::Disposition::Dogged,
        memories,
        FieldBuff {
            active: vec![ActiveFieldBuff {
                kind: FieldBuffKind::Regen,
                name: "Repair Loop".to_string(),
                power: 21,
                remaining: 62,
                interval: 4,
                source: BuffSource::Routine,
            }],
        },
    ));
    game.world
        .get_mut::<Needs>(member)
        .unwrap()
        .set(&need, 42.0);
    // Damaged, so a builder that wrote `max_hp` into `hp` (or the reverse)
    // has two different numbers to get wrong. `mitigation` is set for the
    // same reason — `spawn_tamed` leaves it at 1, which is close enough to
    // several other fields to be worth moving.
    {
        let mut stats = game.world.get_mut::<Stats>(member).unwrap();
        stats.hp = stats.max_hp - 13;
        stats.mitigation = 17;
    }

    game.world.entity_mut(worker).insert((
        CustomName("Quill".to_string()),
        Carrying {
            item: ItemId::from(ids::CORE_FRAGMENT),
            qty: 4,
        },
        OffShift { need },
        Disgruntled {
            grievance: Grievance::Sulking,
            stranded: false,
        },
        Downed,
    ));
    game.world
        .entity_mut(weapon)
        .insert(CustomName("Edge".to_string()));
    game.world
        .entity_mut(scout)
        .insert(CustomName("Ranger".to_string()));
    game.world.entity_mut(sentry).insert((
        CustomName("Warden".to_string()),
        Boss,
        Rarity::Prismatic,
    ));

    Roster {
        member,
        worker,
        weapon,
        scout,
        sentry,
    }
}

/// The three fields the save resolves **by position** rather than by entity
/// id — a cronjob's target, a nest and a town.
///
/// Kept out of `seed_the_roster` because the stand-ins here are bare
/// `Position` entities: enough for the save side, which reads nothing else
/// off them, and deliberately not enough for the load side, which looks for
/// a real structure, nest or settlement on the tile. Seeding these into the
/// round-trip test would fail it for the fixture's shortcut rather than for
/// anything the save did wrong.
fn add_the_position_tethers(game: &mut Game, roster: &Roster) {
    let machine = game.world.spawn(Position { x: 11, y: 13 }).id();
    game.world.entity_mut(roster.worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: machine,
        progress: 14,
        required: 30,
    });

    let nest = game.world.spawn(Position { x: 21, y: 22 }).id();
    let town = game.world.spawn(Position { x: 31, y: 32 }).id();
    game.world.entity_mut(roster.sentry).insert((
        NestGuardian { nest },
        TownPatrol { town },
        Pursuing,
    ));
}

/// The first need the game ships, whatever it is — the fixture wants a real
/// `NeedId` (`Game::load` writes the map back key for key) and does not care
/// which.
fn a_shipped_need(game: &Game) -> crate::needs::NeedId {
    game.world
        .resource::<crate::needs::NeedDb>()
        .iter()
        .next()
        .expect("the game ships needs")
        .id
        .clone()
}

/// Saves `game` and reads the file back **as data, not as text**: these
/// assertions are about the `CreatureSave` the engine built, and matching on
/// RON substrings would pass a value written under the wrong key.
fn creatures_on_disk(game: &mut Game, tag: &str) -> Vec<save::CreatureSave> {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("a.ron");
    game.save(&path).expect("save");
    save::load_from_file(&path)
        .expect("read the dump back")
        .creatures
}

fn named<'a>(creatures: &'a [save::CreatureSave], name: &str) -> &'a save::CreatureSave {
    creatures
        .iter()
        .find(|c| c.custom_name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("{name} should have a row in the file"))
}

/// **The census.** Every field of `CreatureSave` an owned party member can
/// carry, asserted against the value the fixture gave it.
#[test]
fn a_rich_program_writes_every_field_it_was_given() {
    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let roster = seed_the_roster(&mut game);
    add_the_position_tethers(&mut game, &roster);

    let member = roster.member;
    let live = *game.world.get::<Stats>(member).unwrap();
    let exp = *game.world.get::<Experience>(member).unwrap();
    let program_id = game.world.get::<ProgramId>(member).unwrap().0;
    let position = *game.world.get::<Position>(member).unwrap();
    let species = game.world.get::<Creature>(member).unwrap().species.clone();
    let routines = game.world.get::<Routines>(member).unwrap().0.clone();
    let need = a_shipped_need(&game);

    let creatures = creatures_on_disk(&mut game, "creature_save_census");
    let saved = named(&creatures, "Sable");

    // **The completeness guard — do not tidy this into a `..`.** Every
    // binding is discarded and every assertion below still reads through
    // `saved`, so this pattern proves exactly one thing: that the list of
    // fields it names is the whole struct. Add a field to
    // `save::CreatureSave` and this stops compiling, which is the only
    // mechanism that makes somebody write the assertion for it. A rest
    // pattern here would compile forever and cover nothing — and the field
    // it silently dropped would be missing from a refunded program as well
    // as from a save (see this module's header).
    let save::CreatureSave {
        species: _,
        position: _,
        hp: _,
        max_hp: _,
        atk: _,
        mitigation: _,
        tamed: _,
        power: _,
        level: _,
        xp: _,
        xp_to_next: _,
        cronjob: _,
        party_slot: _,
        sortie_index: _,
        wielded: _,
        zone: _,
        custom_name: _,
        hp_roll: _,
        atk_roll: _,
        def_roll: _,
        growth_roll: _,
        assembly_roll: _,
        extraction_roll: _,
        fusions: _,
        refactors: _,
        purchased_tiers: _,
        ring: _,
        talents: _,
        bought_stats: _,
        routines: _,
        field_buffs: _,
        nest_position: _,
        patrol_position: _,
        pursuing: _,
        carrying: _,
        rarity: _,
        boss: _,
        equipment: _,
        nemesis_grudges: _,
        disposition: _,
        program_id: _,
        memories: _,
        needs: _,
        off_shift: _,
        disgruntled: _,
        staff: _,
        downed: _,
        disgruntled_stranded: _,
    } = saved;

    assert_eq!(saved.species, species, "species");
    assert_eq!(saved.position, (position.x, position.y), "position");
    assert_eq!(saved.hp, live.hp, "hp");
    assert_eq!(saved.max_hp, live.max_hp, "max_hp");
    assert_ne!(saved.hp, saved.max_hp, "the fixture damaged it");
    assert_eq!(saved.atk, live.atk, "atk");
    assert_eq!(saved.mitigation, 17, "mitigation");
    assert!(saved.tamed, "tamed");
    assert_eq!(saved.power, 41.5, "power");
    assert_eq!(saved.level, 6, "level");
    assert_eq!(saved.xp, exp.xp, "xp");
    assert_eq!(saved.xp_to_next, exp.xp_to_next, "xp_to_next");
    assert!(saved.cronjob.is_none(), "a party member holds no cronjob");
    assert_eq!(saved.party_slot, Some(0), "party_slot");
    assert_eq!(saved.sortie_index, None, "sortie_index");
    assert!(!saved.wielded, "wielded");
    assert_eq!(saved.zone, 4, "zone");
    assert_eq!(saved.custom_name.as_deref(), Some("Sable"), "custom_name");
    assert_eq!(saved.hp_roll, 1.11, "hp_roll");
    assert_eq!(saved.atk_roll, 1.22, "atk_roll");
    assert_eq!(saved.def_roll, 1.33, "def_roll");
    assert_eq!(saved.growth_roll, 1.44, "growth_roll");
    assert_eq!(saved.assembly_roll, 1.55, "assembly_roll");
    assert_eq!(saved.extraction_roll, 1.66, "extraction_roll");
    assert_eq!(saved.fusions, 2, "fusions");
    assert_eq!(saved.refactors, 3, "refactors");
    assert_eq!(saved.purchased_tiers, 4, "purchased_tiers");
    assert_eq!(saved.ring, 5, "ring");
    assert_eq!(saved.talents, vec!["a_talent".to_string()], "talents");
    assert_eq!(saved.bought_stats.atk, 6, "bought_stats.atk");
    assert_eq!(saved.bought_stats.mitigation, 7, "bought_stats.mitigation");
    assert_eq!(saved.bought_stats.max_hp, 8, "bought_stats.max_hp");
    assert_eq!(
        saved.bought_stats.ever_bought, 10,
        "bought_stats.ever_bought"
    );
    assert_eq!(saved.routines, routines, "routines");
    assert!(!routines.is_empty(), "the fixture installed a routine");
    assert_eq!(saved.field_buffs.len(), 1, "field_buffs");
    assert_eq!(saved.field_buffs[0].name, "Repair Loop", "buff name");
    assert_eq!(saved.field_buffs[0].power, 21, "buff power");
    assert_eq!(saved.field_buffs[0].remaining, 62, "buff remaining");
    assert_eq!(saved.field_buffs[0].interval, 4, "buff interval");
    assert!(saved.nest_position.is_none(), "nest_position");
    assert!(saved.patrol_position.is_none(), "patrol_position");
    assert!(!saved.pursuing, "pursuing");
    assert!(saved.carrying.is_none(), "carrying");
    assert_eq!(saved.rarity, Rarity::Gold, "rarity");
    assert!(!saved.boss, "boss");
    assert_eq!(
        saved
            .equipment
            .iter()
            .map(|(_, worn)| worn.item.to_string())
            .collect::<Vec<_>>(),
        vec![
            ids::OVERCLOCK_CORE.to_string(),
            ids::ABLATIVE_PLATING.to_string()
        ],
        "equipment"
    );
    assert_eq!(saved.nemesis_grudges, 9, "nemesis_grudges");
    assert_eq!(
        saved.disposition,
        Some(crate::disposition::Disposition::Dogged),
        "disposition"
    );
    assert_eq!(saved.program_id, program_id, "program_id");
    assert_ne!(program_id, 0, "the fixture minted a real id");
    assert_eq!(saved.memories.len(), 1, "memories");
    assert_eq!(saved.memories[0].def.to_string(), "a_memory", "memory def");
    assert_eq!(
        saved.memories[0].subject,
        MemorySubject::BaseTile { x: 2, y: 9 },
        "memory subject"
    );
    assert_eq!(
        saved.memories[0].subject_name.as_deref(),
        Some("the mill"),
        "memory subject_name"
    );
    assert_eq!(saved.memories[0].reinforced, 12, "memory reinforced");
    assert_eq!(saved.memories[0].strikes, 3, "memory strikes");
    assert_eq!(saved.needs.get(&need), Some(&42.0), "needs");
    assert_eq!(saved.off_shift, None, "off_shift");
    assert_eq!(saved.disgruntled, None, "disgruntled");
    assert!(!saved.staff, "staff");
    assert!(!saved.downed, "downed");
}

/// The other half of the census: the fields no party member can carry.
///
/// Four of them are **roles**, and a role is not a component — it is derived
/// from the party list, the wield and the sortie roster. A builder that
/// answered all four from one source would satisfy the census above, where
/// three of the four are `false`, and be wrong here. The other three are the
/// **by-position** tethers, whose whole risk is resolving against the wrong
/// entity's `Position` — which only shows up when the tiles differ.
#[test]
fn the_roles_and_the_tethers_are_written_per_creature() {
    let mut game = Game::new(20260908, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let roster = seed_the_roster(&mut game);
    add_the_position_tethers(&mut game, &roster);
    let worker_id = game.world.get::<ProgramId>(roster.worker).unwrap().0;
    let weapon_id = game.world.get::<ProgramId>(roster.weapon).unwrap().0;
    let scout_id = game.world.get::<ProgramId>(roster.scout).unwrap().0;
    let sentry_species = game
        .world
        .get::<Creature>(roster.sentry)
        .unwrap()
        .species
        .clone();
    let need = a_shipped_need(&game);

    let creatures = creatures_on_disk(&mut game, "creature_save_roles");

    let worker = named(&creatures, "Quill");
    assert!(worker.staff, "the worker is on the staff");
    assert_eq!(worker.party_slot, None, "the worker is not in the party");
    assert!(!worker.wielded, "the worker is not wielded");
    assert_eq!(worker.sortie_index, None, "the worker is at home");
    assert_eq!(worker.program_id, worker_id, "program_id");
    let cronjob = worker.cronjob.as_ref().expect("the worker holds a cronjob");
    assert_eq!(cronjob.target_position, (11, 13), "cronjob target_position");
    assert_eq!(cronjob.progress, 14, "cronjob progress");
    assert_eq!(cronjob.required, 30, "cronjob required");
    assert!(
        matches!(cronjob.kind, save::CronjobKind::GatherResource),
        "cronjob kind"
    );
    assert_eq!(
        worker.carrying,
        Some((ItemId::from(ids::CORE_FRAGMENT), 4)),
        "carrying"
    );
    assert_eq!(worker.off_shift, Some(need), "off_shift");
    assert_eq!(worker.disgruntled, Some(Grievance::Sulking), "disgruntled");
    assert!(worker.downed, "downed");

    let weapon = named(&creatures, "Edge");
    assert_eq!(
        weapon.program_id, weapon_id,
        "the wielded row is that program"
    );
    assert!(weapon.wielded, "the wielded program says so");
    assert!(!weapon.staff, "wielding is not staffing");
    assert_eq!(weapon.party_slot, None, "wielding is not party membership");

    let scout = named(&creatures, "Ranger");
    assert_eq!(scout.program_id, scout_id, "the away row is that program");
    assert_eq!(scout.sortie_index, Some(0), "the scout is away on sortie 0");
    assert!(!scout.staff, "a program away is not on the staff");
    assert_eq!(scout.party_slot, None, "a program away is not in the party");

    let sentry = named(&creatures, "Warden");
    assert_eq!(sentry.species, sentry_species, "species");
    assert!(!sentry.tamed, "the sentry is wild");
    assert_eq!(sentry.nest_position, Some((21, 22)), "nest_position");
    assert_eq!(sentry.patrol_position, Some((31, 32)), "patrol_position");
    assert!(sentry.pursuing, "pursuing");
    assert!(sentry.boss, "boss");
    assert_eq!(sentry.rarity, Rarity::Prismatic, "rarity");
    // The defaults an untamed creature falls to, which is the branch the
    // census above never takes.
    assert_eq!(sentry.level, 1, "a wild creature does not level");
    assert_eq!(sentry.program_id, 0, "a wild creature has no program id");
    assert_eq!(
        sentry.disposition, None,
        "a wild creature has no temperament"
    );
    assert!(
        sentry.memories.is_empty(),
        "a wild creature remembers nothing"
    );
    assert!(sentry.equipment.is_empty(), "a wild creature wears nothing");
    assert!(!sentry.staff, "a wild creature is on nobody's payroll");
    assert_eq!(sentry.zone, 1, "a wild creature spawned outside a portal");
}

/// Saving, loading and saving again must write the same lines.
///
/// The **symmetry** gate, not the field gate: it catches a field the builder
/// writes and the loader ignores, or the reverse, which a census cannot see
/// because a census only ever looks at one dump.
///
/// **A multiset of lines rather than the bytes**, and that is a statement
/// about the save rather than a concession by this test. The `creatures`
/// array's order is not stable across a load — a program's archetype at
/// spawn is not its archetype after `Game::load` rebuilds it, and bevy
/// iterates by archetype — so the same roster comes back in a different
/// order with nothing lost. Making the emission order stable is a change to
/// the save format and belongs in its own commit; until then a byte
/// comparison would fail for a reason that has nothing to do with what this
/// asserts.
#[test]
fn a_load_then_save_writes_the_same_lines() {
    let dir = scratch_assets_dir("save_roundtrip");
    std::fs::create_dir_all(&*dir).unwrap();
    let first = dir.join("a.ron");
    let second = dir.join("b.ron");

    let mut game = Game::new(20260909, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    seed_the_roster(&mut game);

    game.save(&first).expect("first save");
    let mut loaded = Game::load(&first, &test_assets_dir()).expect("load");
    loaded.save(&second).expect("second save");

    let sorted = |path: &std::path::Path| {
        let mut lines: Vec<String> = std::fs::read_to_string(path)
            .expect("read a dump")
            .lines()
            .map(str::to_string)
            .collect();
        lines.sort();
        lines
    };
    assert_eq!(
        sorted(&first),
        sorted(&second),
        "a load-then-save must reproduce every line of the save it read"
    );
}

/// Snapshot a live program, delete it, and hand the snapshot straight back
/// to the loader: what comes out must be the same program.
///
/// This is the refund path with the build order taken out of the middle of
/// it — `Game::creature_save_for` and `Game::spawn_creature_from_save` are a
/// pair, and a cancelled build order runs them back to back. The round-trip
/// gate above only ever exercises them through a *file*, which is a slower
/// loop and a coarser assertion: it compares two dumps, so a field the
/// loader mangles and the builder re-derives the same way passes it.
///
/// The assertions that carry weight here are about **identity**, not shape:
///
/// - A program that comes back under a fresh `ProgramId` orphans its own
///   memories *and* every other program's memories naming it as their
///   subject, so the refund would quietly cost the base its whole social
///   record of one companion.
/// - A memory re-stamped on the way back has its decay curve reset.
///   Intensity is derived from `GameClock` on every read and never stored
///   (`components::Memory`), so a fresh timestamp makes a refund *deepen* an
///   old grudge rather than preserve it.
#[test]
fn a_snapshot_respawns_as_the_same_program() {
    let mut game = Game::new(20260910, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let original = spawn_tamed(&mut game, 30, 6);
    let mut memories = Memories::default();
    memories.0.push(Memory {
        def: crate::memories::MemoryId::from("a_memory"),
        subject: MemorySubject::BaseTile { x: 2, y: 9 },
        subject_name: Some("the mill".to_string()),
        reinforced: 12,
        strikes: 3,
    });
    game.world
        .entity_mut(original)
        .insert((ZonePortal(4), memories));
    game.rename_companion(original, Some("Bellwether".to_string()))
        .expect("naming a program");

    // Read off the live program before it is deleted: the point of the test
    // is that the restored one answers these the same way, and comparing
    // against hand-written literals would only pin the fixture.
    let player = game.player_entity();
    let label = game.creature_label(original);
    let program_id = game.world.get::<ProgramId>(original).unwrap().0;
    assert_ne!(program_id, 0, "the fixture minted a real id");

    let snapshot = game.creature_save_for(original).expect("snapshot");
    game.world.despawn(original);

    let mut restore = CreatureRestore::new(player, 99, HashMap::new());
    let restored = game
        .spawn_creature_from_save(&snapshot, &mut restore)
        .expect("the snapshot names a species the game still ships");

    assert_eq!(
        game.creature_label(restored),
        label,
        "the name it answers to"
    );
    assert_eq!(game.zone_tier(restored), 4, "the tier it is scaled to");
    assert_eq!(
        game.world.get::<ProgramId>(restored).map(|p| p.0),
        Some(program_id),
        "a restored program keeps its own name, so its memories still find it",
    );
    assert_eq!(
        restore.next_program_id, 99,
        "an id the snapshot already carries is never reissued",
    );
    assert_eq!(
        game.world.get::<Tamed>(restored).map(|t| t.owner),
        Some(player),
        "a restored program is still the player's",
    );
    let remembered = game
        .world
        .get::<Memories>(restored)
        .expect("a restored program still has somewhere to remember");
    assert_eq!(remembered.0.len(), 1, "the memory came back");
    assert_eq!(
        remembered.0[0].reinforced, 12,
        "a memory keeps its original timestamp, or its decay curve resets",
    );
    assert_eq!(remembered.0[0].strikes, 3, "and its strike count");
    assert_eq!(
        remembered.0[0].subject,
        MemorySubject::BaseTile { x: 2, y: 9 },
        "and what it is about",
    );
}

/// The one case that *does* mint: the `0` sentinel a file written before
/// program ids existed carries for everybody on the roster.
///
/// The counter belongs to the caller and not to the restore, which is the
/// whole reason it lives on `CreatureRestore` — minting is a property of a
/// pass over the *whole* creature array, since two sentinels in one file
/// must not both come back as the same program. A counter reset per call
/// would pass the test above and fail this one.
#[test]
fn a_snapshot_with_the_sentinel_id_mints_from_the_caller_counter() {
    let mut game = Game::new(20260911, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let original = spawn_tamed(&mut game, 30, 6);
    let player = game.player_entity();
    let mut snapshot = game.creature_save_for(original).expect("snapshot");
    game.world.despawn(original);
    snapshot.program_id = 0;

    let mut restore = CreatureRestore::new(player, 77, HashMap::new());
    let first = game
        .spawn_creature_from_save(&snapshot, &mut restore)
        .expect("respawn");
    let second = game
        .spawn_creature_from_save(&snapshot, &mut restore)
        .expect("respawn again");

    assert_eq!(
        game.world.get::<ProgramId>(first).map(|p| p.0),
        Some(77),
        "the sentinel takes the caller's next id",
    );
    assert_eq!(
        game.world.get::<ProgramId>(second).map(|p| p.0),
        Some(78),
        "and the second sentinel takes the one after it, not the same one",
    );
    assert_eq!(
        restore.next_program_id, 79,
        "the counter the caller writes back to `NextProgramId` has moved twice",
    );
}

/// The committed program must survive a real file round trip. A RON
/// round-trip test cannot catch a field that fails to serialise, so this
/// goes through the disk.
#[test]
fn a_build_sites_program_survives_a_save_and_load() {
    let dir = scratch_assets_dir("build_site_program_roundtrip");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("s.ron");

    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 30, 6);
    let snapshot = game.creature_save_for(program).expect("snapshot");

    game.world.spawn((
        BuildSite {
            program: Some(snapshot.clone()),
            ..BuildSite::new("fabricator".to_string(), vec![])
        },
        Position { x: 1, y: 1 },
    ));

    game.save(&path).expect("save");
    let mut loaded = Game::load(&path, &test_assets_dir()).expect("load");

    let sites = loaded.build_site_programs();
    assert_eq!(sites.len(), 1, "the site came back");
    assert_eq!(
        sites[0].as_ref().map(|c| c.program_id),
        Some(snapshot.program_id),
        "and it is still holding the program it was given",
    );
}

/// And the other shape, which a Depot made reachable: an order holding
/// **nobody**. `StructureDef::needs_program` exempts anything that declares
/// `costs_no_program`, so a filed shelf carries no snapshot at all — and a
/// save that
/// resurrected one, or refused to load a site without one, would be a run
/// gaining a program it never owned or losing a base it did.
#[test]
fn a_program_less_build_site_survives_a_save_and_load() {
    let dir = scratch_assets_dir("build_site_no_program_roundtrip");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("s.ron");

    let mut game = Game::new(20260908, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.spawn((
        BuildSite {
            program: None,
            ..BuildSite::new("depot".to_string(), vec![])
        },
        Position { x: 1, y: 1 },
    ));

    game.save(&path).expect("save");
    let mut loaded = Game::load(&path, &test_assets_dir()).expect("load");

    let sites = loaded.build_site_programs();
    assert_eq!(sites.len(), 1, "the site came back");
    assert!(
        sites[0].is_none(),
        "and it is still holding nobody — a shelf commits no body at either end"
    );
}

/// **Spec test 10 — the whole journey, through the disk.**
///
/// `refund_program` has only ever been handed a snapshot taken moments
/// earlier in the same process, which proves nothing about the field that
/// carries it between sessions. So this one files a real order, writes it to
/// a real file, reads it back into a fresh `Game` and *then* calls the order
/// off — and the program that comes back has to be the one that went in,
/// down to the `ProgramId` its memories are keyed to.
///
/// A RON round trip cannot substitute: `#[serde(skip)]` on
/// `BuildSite::program` would leave one of those green (see
/// `SAVE_FORMAT_VERSION`'s docs), and every assertion below would then be
/// made against a snapshot that never left memory.
#[test]
fn a_reloaded_order_still_gives_its_program_back_on_a_cancel() {
    let dir = scratch_assets_dir("cancel_after_reload");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("s.ron");

    let mut game = Game::new(20260907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    // The spare: an order may never take the base to zero programs.
    tame_at_zone(&mut game, 1);
    let spend = tame_at_zone(&mut game, 1);
    set_level(&mut game, spend, 6);
    wear(&mut game, spend, ids::OVERCLOCK_CORE);
    game.rename_companion(spend, Some("Bellwether".to_string()))
        .expect("named");
    let program_id = game.world.get::<ProgramId>(spend).unwrap().0;
    assert_ne!(program_id, 0, "the fixture minted a real id");

    game.place_structure("mining_node", 1, 0, Some(spend))
        .expect("filed");
    game.save(&path).expect("save");

    let mut loaded = Game::load(&path, &test_assets_dir()).expect("load");
    let (px, py) = loaded
        .base_pos()
        .expect("the party reloads inside the base");
    let site = loaded
        .build_site_at(px + 1, py)
        .expect("the order came back off the disk");
    loaded.cancel_build_request(site).expect("cancelled");

    let back = loaded
        .owned_pets()
        .into_iter()
        .find(|p| p.name.contains("Bellwether"))
        .expect("the committed program is back on the roster after a reload");
    assert_eq!(
        loaded.world.get::<ProgramId>(back.entity).unwrap().0,
        program_id,
        "the same program by name, not a fresh one wearing its stats"
    );
    assert_eq!(back.level, 6, "and at the level it was committed at");
    assert_eq!(
        loaded
            .world
            .get::<Equipment>(back.entity)
            .and_then(|e| e.weapon.as_ref().map(|w| w.copy.item.to_string())),
        Some(ids::OVERCLOCK_CORE.to_string()),
        "still wearing what it went in wearing"
    );
}

/// The two build rolls are additive behind `#[serde(default = "neutral_roll")]`,
/// so they cost no `SAVE_FORMAT_VERSION` bump. A bare `#[serde(default)]` on
/// an `f32` is `0.0`, which would load every program in every pre-feature
/// save as permanently the worst builder in the game — silently, since
/// nothing on the screen would say so until a machine came out slow.
#[test]
fn a_save_written_without_the_build_rolls_loads_them_neutral() {
    let mut game = Game::new(4211, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 20, 3);
    game.world.entity_mut(pet).insert(Potential {
        hp_roll: 1.1,
        atk_roll: 1.1,
        def_roll: 1.1,
        growth_roll: 1.1,
        assembly_roll: 1.18,
        extraction_roll: 0.82,
    });

    let path = std::env::temp_dir().join(format!(
        "feral_processes_legacy_build_rolls_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let stripped: String = text
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("assembly_roll:") && !t.starts_with("extraction_roll:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !stripped.contains("assembly_roll") && !stripped.contains("extraction_roll"),
        "the fixture must actually remove the keys or the test proves nothing"
    );
    assert!(
        text.contains("assembly_roll"),
        "and they must have been there to remove"
    );
    std::fs::write(&path, stripped).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).expect("a pre-feature save still loads");
    let _ = std::fs::remove_file(&path);

    let mut seen = 0;
    let mut q = loaded.world.query::<&Potential>();
    for p in q.iter(&loaded.world) {
        assert_eq!(p.assembly_roll, 1.0, "assembly_roll defaulted wrong");
        assert_eq!(p.extraction_roll, 1.0, "extraction_roll defaulted wrong");
        seen += 1;
    }
    assert!(seen > 0, "the save carried no creature to check");
}

/// A machine remembers how well it was built — through a real file round
/// trip, not a RON one, because a `#[serde(skip)]` leaves a RON round trip
/// green while the save on disk carries nothing.
#[test]
fn a_machines_build_quality_survives_a_save() {
    let mut game = crate::tests::support::base_with_a_built_node(4212, 1.0, 0.82);
    let node = crate::tests::support::first_structure(&mut game, "mining_node");
    let before = game.work_ticks_for(node, crate::tuning::DEFAULT_BASE_SPEED);
    assert!(
        before > 10,
        "the fixture built a machine at the shipped rate"
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_build_quality_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let reloaded = crate::tests::support::first_structure(&mut loaded, "mining_node");
    assert_eq!(
        loaded.work_ticks_for(reloaded, crate::tuning::DEFAULT_BASE_SPEED),
        before,
        "the reloaded machine runs at a different rate than the one that was saved"
    );
}

/// The structure half of the missing-key rule: a pre-feature save loads
/// every machine at its shipped rate, not at zero.
#[test]
fn a_save_written_without_build_quality_loads_it_neutral() {
    let mut game = crate::tests::support::base_with_a_built_node(4213, 1.0, 0.82);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_legacy_build_quality_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("build_quality"),
        "the key must have been there to remove"
    );
    let stripped: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("build_quality:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !stripped.contains("build_quality"),
        "the fixture must actually remove the key or the test proves nothing"
    );
    std::fs::write(&path, stripped).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).expect("a pre-feature save still loads");
    let _ = std::fs::remove_file(&path);

    let node = crate::tests::support::first_structure(&mut loaded, "mining_node");
    assert_eq!(
        loaded.work_ticks_for(node, crate::tuning::DEFAULT_BASE_SPEED),
        10,
        "a machine with no stored figure must cycle at its def's shipped rate"
    );
}
