use crate::tests::support::*;

/// Every memory kind the engine actually writes, against the
/// `MemorySubjectKind` the trigger that writes it hands in.
///
/// The table is spelled out here rather than derived, because there is
/// nothing to derive it from: the catalogue is data and the triggers are
/// Rust, which is the seam `perks::Perk` sits on, and `MemoryDef` carries no
/// `trigger` field on purpose. Keeping the pairing in one census is what
/// stops a shipped def drifting into content nothing can ever write.
const MEMORY_TRIGGERS: &[(&str, crate::memories::MemorySubjectKind)] = {
    use crate::memories::MemorySubjectKind as K;
    &[
        // `Game::note_maul`, off `resolve_and_apply_attack`.
        ("mauled_by", K::Species),
        // `Game::form_victory_memories`, off `end_battle`.
        ("bonded_in_battle", K::Program),
        ("hard_won", K::Nothing),
        // `Game::note_strandings`, off `tick_inner`.
        ("stranded_at", K::BaseTile),
        // `Game::fray`, off the one edge where a need latches. Two defs,
        // one per branch: `frayed_here` where the base *had* an answer and
        // nothing routed to it, `ran_down` where it had none at all. The
        // second blames nothing, which is what lets it exist; see that
        // function.
        ("frayed_here", K::BaseTile),
        ("ran_down", K::Nothing),
        // `Game::note_unslotted`, off `tick_inner` on `note_postings`'
        // period: a stretch of having no slot, not an edge. Blames nothing,
        // `ran_down`'s reason — the missing room is the player's to build.
        ("unslotted", K::Nothing),
        // `Game::note_siphoned`, off `tick_inner` on its own slower
        // `SIPHON_GRUDGE_PERIOD`: a stretch of being held in a Power Siphon,
        // keyed to the siphon's base tile.
        ("siphoned", K::BaseTile),
        // `Game::damage_structure`, on both branches. The one work memory
        // that is an edge rather than a stretch of service, because a sweep
        // is an event and a posting is a standing state.
        ("swept_here", K::Structure),
        // `Game::note_idling`, off the edge where a serviced need reaches
        // `content` — once per stretch, never per tick.
        ("idled_with", K::Program),
        // `Game::note_postings`, off `tick_inner` on a period. Two subjects,
        // because a posting is both a place and a kind of work.
        ("settled_in", K::Structure),
        ("jammed_here", K::Structure),
        ("cutting_rock", K::Activity),
        // `Game::close_brawl`, off the tantrum step in
        // `schedule_base_labour`. One per side of the fight: the aggressor's
        // relief, which is what keeps the ratcheted rung from meaning
        // "fights forever", and the victim's grudge, which names the
        // aggressor and outlasts it.
        ("vented", K::Nothing),
        ("turned_on_me", K::Program),
        // The same call, for every other staff body within
        // `BOND_WITNESS_REACH` of the victim. Weaker, and names the
        // aggressor.
        ("saw_turn_on", K::Program),
        // `Game::note_sabotage`, off `tick_inner` on `INTERACTION_PERIOD`: the
        // witnesses of a sulking program spoiling a unit at a machine, about
        // the saboteur.
        ("saw_sabotage", K::Program),
        // `Game::note_departure`, off every door a program leaves play by
        // (its doc lists the six). The subject is the program that left, and
        // the four are one per `Departure` plus the relief for a rival going.
        ("lost_in_battle", K::Program),
        ("let_go", K::Program),
        ("became_part_of", K::Program),
        ("rid_of", K::Program),
        // `Game::note_respites`, off `tick_inner` on a period. The one
        // fondness written by an errand the program took itself, and the
        // mechanism the morale errand is built out of: morale has no reserve
        // to refill, so recuperating *is* coming away fond of the amenity.
        ("unwound_at", K::Structure),
        // `Game::note_comforts`, off `tick_inner` on the same period as
        // `note_postings` — a program standing on a finish whose `comfort`
        // names this def.
        ("at_ease_on", K::BaseTile),
        // `Game::note_interactions`, off `tick_inner` on `INTERACTION_PERIOD`:
        // what two idle programs say to each other, each def named by an
        // `assets/interactions/` file. The hearsay pair is what `gossip`
        // writes, by a firsthand memory's `spreads_as`.
        ("chatted_with", K::Program),
        ("talked_shop_with", K::Program),
        ("laughed_with", K::Program),
        ("complimented_by", K::Program),
        ("thanked_by", K::Program),
        ("commiserated_with", K::Program),
        ("complained_at_by", K::Program),
        ("slighted_by", K::Program),
        ("insulted_by", K::Program),
        ("heard_well_of", K::Program),
        ("heard_ill_of", K::Program),
    ]
};

/// A shipped need with nothing in the base that services it is a need a
/// program can only run down — it would announce a stall, hold a grudge and
/// never recover, which reads as the feature being broken rather than as the
/// player being short a building.
///
/// A *shipped-content* rule, `MEMORY_TRIGGERS`' kind, and it does not
/// contradict the empty-catalogue property: zero needs and zero amenities
/// passes, because there is nothing to walk.
#[test]
fn every_shipped_need_has_a_shipped_amenity() {
    use crate::needs::NeedDb;
    use crate::structures::StructureDb;

    let (needs, _) = NeedDb::load_dir(&test_assets_dir().join("needs")).unwrap();
    let (structures, _) = StructureDb::load_dir(&test_assets_dir().join("structures")).unwrap();
    assert!(
        needs.iter().count() > 0,
        "the census must walk a real catalogue"
    );

    for def in needs.iter() {
        assert!(
            structures
                .all()
                .any(|s| s.services.iter().any(|svc| svc.need == def.id)),
            "{} ships in assets/needs but no structure services it, so a program              that runs it down can never recover",
            def.id.as_str()
        );
    }
}

/// What the shipped content charges a body for.
///
/// **A body is spent on a machine that will take a body.** The cost follows
/// `StructureDef::runs_a_job`, so the census is written the other way round
/// — over the fields a structure authors for some *other* purpose — and it
/// is a real statement about content rather than a re-derivation: a shelf, a
/// rack, a Teardown Rig, the doorway out of the sector, anything that
/// defends, repairs, services a need or trades costs nobody, and every
/// producer costs one. A
/// Shield that grew a `machine:` line would fail both arms at once.
///
/// Deliberately a census over *shipped assets* and not a rule in
/// `StructureDef` — a mod is free to build a bench that defends.
#[test]
fn the_shipped_program_cost_follows_the_job() {
    use crate::structures::{StructureCategory, StructureDb};

    let (structures, _) = StructureDb::load_dir(&test_assets_dir().join("structures")).unwrap();
    let mut free = 0;
    let mut costed = 0;

    for def in structures.all() {
        if def.category() == StructureCategory::Home {
            continue;
        }
        let idle = def.stores
            || def.strips.is_some()
            || def.racks.is_some()
            || def.zone_portal
            || def.raid_defense > 0
            || def.repair.is_some()
            || def.trade.is_some()
            || !def.services.is_empty();
        if idle {
            free += 1;
            assert!(
                !def.needs_program(),
                "{} runs no job, so there is nothing at it for a committed program \
                 to stand at",
                def.id
            );
        }
        if def.work.is_some() || def.machine.is_some() {
            costed += 1;
            assert!(
                def.needs_program(),
                "{} is staffed, and a program is what it is staffed with",
                def.id
            );
        }
    }

    assert!(free >= 9, "the shipped depot ladder alone runs to Mk6");
    assert!(
        costed >= 5,
        "the shipped assets define nodes, benches and a rig"
    );
}

/// A def whose declared `subject` no trigger can satisfy is dead content:
/// every `remember` of it is refused as `WrongSubject`, and nothing else in
/// the build says so. A def with no trigger at all is worse — it can never be
/// written, and reads as a memory the player simply never earns.
///
/// This census walks the *defs* and not the variants, which is what let
/// `MemorySubjectKind::Structure` and `::Activity` ship as variants with no
/// def and no trigger at all — a variant with no writer costs nothing, while
/// an enum that has to grow costs a migration.
#[test]
fn every_shipped_memory_def_is_reachable_from_a_trigger() {
    use crate::memories::MemoryDb;

    let (db, _) = MemoryDb::load_dir(&test_assets_dir().join("memories")).unwrap();
    assert!(
        db.all().count() > 0,
        "the census must walk a real catalogue"
    );

    for def in db.all() {
        let id = def.id.as_str();
        let (_, writes) = MEMORY_TRIGGERS
            .iter()
            .find(|(name, _)| *name == id)
            .unwrap_or_else(|| {
                panic!(
                    "{id} ships in assets/memories but nothing in the engine                      writes it — add the trigger, or add it to MEMORY_TRIGGERS                      if this census has fallen behind"
                )
            });
        assert_eq!(
            def.subject, *writes,
            "{id} declares a subject its trigger never hands in, so every              write of it is refused"
        );
    }
}

/// The shipped interaction catalogue loads clean against the shipped
/// memories, so no interaction is silently dropped for a dangling id.
#[test]
fn every_shipped_interaction_loads_without_a_warning() {
    use crate::interactions::InteractionDb;
    use crate::memories::MemoryDb;

    let (memories, _) = MemoryDb::load_dir(&test_assets_dir().join("memories")).unwrap();
    let (db, warnings) =
        InteractionDb::load_dir(&test_assets_dir().join("interactions"), &memories).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(db.iter().count(), 10, "the full set ships");
}

/// What the shipped interactions can say. Slot names and line counts need no
/// check here: an exchange that breaks either is dropped with a warning, and
/// `every_shipped_interaction_loads_without_a_warning` holds that at zero.
#[test]
fn every_shipped_interaction_has_exchanges_that_fit_the_topic_rules() {
    use crate::interactions::{InteractionDb, slots};
    use crate::memories::MemoryDb;

    let (memories, _) = MemoryDb::load_dir(&test_assets_dir().join("memories")).unwrap();
    let (db, _) =
        InteractionDb::load_dir(&test_assets_dir().join("interactions"), &memories).unwrap();
    let uses_topic = |lines: &Vec<crate::interactions::Line>| {
        lines.iter().any(|l| slots(&l.text).any(|s| s == "topic"))
    };
    for def in db.iter() {
        assert!(!def.exchanges.is_empty(), "{} says nothing", def.id);
        if def.gossip {
            assert!(
                def.exchanges.iter().all(uses_topic),
                "{}: a rumour is always about someone",
                def.id
            );
        } else {
            assert!(
                !def.exchanges.iter().all(uses_topic),
                "{} has no topic-free exchange, so it could say nothing with nothing on its mind",
                def.id
            );
        }
    }
    for id in ["small_talk", "shop_talk", "complain", "commiserate"] {
        let def = db.get(id).unwrap();
        assert!(
            def.exchanges.iter().any(uses_topic),
            "{id} never names a topic"
        );
    }
}

/// A rumour is weaker than what it is told from, and tells nothing further:
/// the target of `spreads_as` is a `Program`-subject def that does not itself
/// spread, which is what keeps gossip one hop.
#[test]
fn every_spreads_as_is_a_weaker_program_memory_that_does_not_spread() {
    use crate::memories::{MemoryDb, MemoryId, MemorySubjectKind};

    let (db, _) = MemoryDb::load_dir(&test_assets_dir().join("memories")).unwrap();
    let mut spreading = 0;
    for def in db.all() {
        let Some(target) = &def.spreads_as else {
            continue;
        };
        spreading += 1;
        let hearsay = db
            .get(&MemoryId::from(target.as_str()))
            .unwrap_or_else(|| panic!("{} spreads as {target}, which does not ship", def.id));
        assert_eq!(hearsay.subject, MemorySubjectKind::Program, "{target}");
        assert!(hearsay.spreads_as.is_none(), "{target} would be retold");
        assert!(
            hearsay.valence.abs() < def.valence.abs(),
            "{target} is not weaker than {}",
            def.id
        );
        assert_eq!(
            hearsay.valence.signum(),
            def.valence.signum(),
            "{target} would turn {} into its opposite",
            def.id
        );
    }
    assert_eq!(spreading, 5, "the five firsthand Program memories spread");
}

/// `note_departure` writes by id, and a departure memory that does not carry
/// `departure: true` would be counted by the bond read it is meant to be
/// left out of. So every id it can write must resolve, flagged.
#[test]
fn every_departure_def_ships_flagged() {
    use crate::bonds::{Departure, RELIEF_DEF};
    use crate::memories::{MemoryDb, MemoryId};

    let (db, _) = MemoryDb::load_dir(&test_assets_dir().join("memories")).unwrap();
    let written = [Departure::Fell, Departure::LetGo, Departure::Fused]
        .map(Departure::grief_def)
        .into_iter()
        .chain([RELIEF_DEF]);
    for id in written {
        let def = db
            .get(&MemoryId::from(id))
            .unwrap_or_else(|| panic!("{id} is written by note_departure but does not ship"));
        assert!(def.departure, "{id} ships without `departure: true`");
    }
}

/// The memory-catalogue census, over the **real** `assets/memories/` rather
/// than a fixture. `MemoryDb::load_dir` refuses nothing beyond a file that
/// will not parse — a mod's def is never turned away for being nonsense — so
/// this is what holds the shipped catalogue to what a memory has to be for
/// `Memory::intensity` to mean anything.
#[test]
fn every_shipped_memory_def_is_well_formed() {
    use crate::memories::MemoryDb;

    let dir = test_assets_dir().join("memories");
    let (db, warnings) = MemoryDb::load_dir(&dir).unwrap();
    assert!(
        warnings.is_empty(),
        "the shipped catalogue must load clean: {warnings:?}"
    );

    // Against the file count rather than against a set of ids, because the db
    // is keyed by id: two files claiming one id collapse into a single entry
    // and nothing else in the load would say so.
    let files = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
        .count();
    assert!(
        files > 0,
        "the census must actually walk assets/memories, or it passes vacuously"
    );
    assert_eq!(
        db.all().count(),
        files,
        "two shipped files claim one memory id, so one of them is unreachable"
    );

    for def in db.all() {
        let id = def.id.as_str();
        assert!(!def.name.is_empty(), "{id} has no name for a screen row");
        assert!(!def.blurb.is_empty(), "{id} has no blurb");
        assert!(
            def.valence.is_finite() && def.valence != 0.0,
            "{id} has a valence of {}, so remembering it would be worth nothing",
            def.valence
        );
        assert!(
            def.half_life > 0,
            "{id} has a half_life of 0, which divides by zero in intensity"
        );
        assert!(
            def.strike_cap >= 1,
            "{id} has a strike_cap of 0, so every strike of it is worth nothing"
        );
    }
}

/// A finish naming a `comfort` id nothing in `assets/memories/` defines
/// would make `Game::note_comforts` a silent no-op for it — `remember`'s
/// resolve-first rule swallows a dangling id rather than panicking, so
/// nothing else in the build would say so.
#[test]
fn every_shipped_floor_comfort_names_a_shipped_memory() {
    use crate::floors::FloorDb;
    use crate::memories::MemoryDb;

    let (floors, _) = FloorDb::load_dir(&test_assets_dir().join("floors")).unwrap();
    let (memories, _) = MemoryDb::load_dir(&test_assets_dir().join("memories")).unwrap();
    assert!(
        floors.iter().count() > 0,
        "the census must walk a real catalogue"
    );

    for def in floors.iter() {
        let Some(comfort) = &def.comfort else {
            continue;
        };
        let mem = memories
            .all()
            .find(|m| m.id.as_str() == comfort)
            .unwrap_or_else(|| {
                panic!(
                    "{} names comfort {comfort:?}, which no shipped memory defines",
                    def.id.as_str()
                )
            });
        assert_eq!(
            mem.subject,
            crate::memories::MemorySubjectKind::BaseTile,
            "{} names comfort {comfort:?}, whose subject is not BaseTile",
            def.id.as_str()
        );
    }
}

/// The shipped set must cover both halves of `comfort`, or one branch of
/// `Game::note_comforts` is untested against real content.
#[test]
fn shipped_floors_include_one_with_and_one_without_comfort() {
    use crate::floors::FloorDb;

    let (floors, _) = FloorDb::load_dir(&test_assets_dir().join("floors")).unwrap();
    assert!(
        floors.iter().any(|d| d.comfort.is_some()),
        "no shipped finish declares a comfort"
    );
    assert!(
        floors.iter().any(|d| d.comfort.is_none()),
        "every shipped finish declares a comfort — nothing is purely cosmetic"
    );
}

/// `stack_decay` is a ratio in `(0, 1]`: at 1 it is inert (the linear
/// stacking every def shipped with before this field existed), at or below 0
/// it either divides by zero in `Memory::intensity_with`'s closed form or
/// makes reinforcement grow rather than shrink.
#[test]
fn every_memory_stack_decay_is_in_range() {
    use crate::memories::MemoryDb;

    let (db, _) = MemoryDb::load_dir(&test_assets_dir().join("memories")).unwrap();
    let mut checked = 0;
    for def in db.all() {
        checked += 1;
        assert!(
            def.stack_decay > 0.0 && def.stack_decay <= 1.0,
            "{} has a stack_decay of {}, outside (0, 1]",
            def.id.as_str(),
            def.stack_decay
        );
    }
    assert!(
        checked > 0,
        "the census must actually walk assets/memories, or it passes vacuously"
    );
}

/// `mood` is a share in `[0, 1]` of `Read::Morale`, and a `mood` of exactly
/// 0 says a def is worth *nothing* on that read — which is only a real
/// statement about content when something can still read its `Opinion`.
/// `Game::opinion_of` is only ever asked about `Program` and `BaseTile`
/// (`base/tantrum.rs`, `base/morale.rs`'s drift rejection and
/// `base/work_orders.rs`'s `refuses_post`), so a `mood: 0` def declaring any
/// other subject kind is content nothing can ever read.
///
/// A new `opinion_of` reader must extend this list.
const OPINION_READ_SUBJECTS: &[crate::memories::MemorySubjectKind] = &[
    crate::memories::MemorySubjectKind::Program,
    crate::memories::MemorySubjectKind::BaseTile,
];

#[test]
fn every_memory_mood_is_in_range_and_read_somewhere() {
    use crate::memories::MemoryDb;

    let (db, _) = MemoryDb::load_dir(&test_assets_dir().join("memories")).unwrap();
    let mut checked = 0;
    for def in db.all() {
        checked += 1;
        assert!(
            (0.0..=1.0).contains(&def.mood),
            "{} has a mood of {}, outside [0, 1]",
            def.id.as_str(),
            def.mood
        );
        if def.mood == 0.0 {
            assert!(
                OPINION_READ_SUBJECTS.contains(&def.subject),
                "{} has mood: 0.0 but declares subject {:?}, which no \
                 opinion_of reader ever asks about — it would be worth \
                 nothing on every read",
                def.id.as_str(),
                def.subject
            );
        }
    }
    assert!(
        checked > 0,
        "the census must actually walk assets/memories, or it passes vacuously"
    );
}
