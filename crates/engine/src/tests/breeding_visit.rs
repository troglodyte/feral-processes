//! The breeding visit: both parents walk to the bay, pause, and only then
//! does the child start. Everything here that needs the walk drives it
//! through `Game::tick`.

use super::breeding::{bay_game, finish_visit, loaded_bay, save_and_load, seeds_in_pack};
use super::needs::{coherence, reserve, set_reserve, threshold};
use super::support::*;
use crate::breeding::{BreedRefusal, VisitPause};
use crate::components::{
    BreedReadyAt, BreedingVisit, Downed, Durability, Incubator, Memories, MemorySubject, Needs,
    OffShift, Position, ProgramId, Structure, UnderStudy,
};
use crate::game::base::breeding_visit::{
    CancelReason, ParentState, VisitInput, VisitOutcome, decide_visits,
};
use crate::game::party::ProgramRole;
use crate::resources::{Sortie, Sorties};
use crate::tuning::*;
use crate::*;

fn pause_left(game: &Game, bay: Entity) -> Option<u64> {
    game.world.get::<Incubator>(bay).unwrap().slots[0]
        .as_ref()
        .and_then(|c| c.visit)
        .map(|v| v.pause_left)
}

/// Ticks until both parents are beside the bay and the pause has begun.
fn until_paused(game: &mut Game, bay: Entity) {
    for _ in 0..400 {
        if pause_left(game, bay).is_some_and(|p| p < BREEDING_VISIT_PAUSE_TICKS) {
            return;
        }
        game.tick();
    }
    panic!("the parents never reached the bay");
}

fn memory_defs(game: &Game, who: Entity) -> Vec<(String, MemorySubject)> {
    game.world
        .get::<Memories>(who)
        .map(|m| {
            m.0.iter()
                .map(|m| (m.def.to_string(), m.subject.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn marked(game: &mut Game) -> usize {
    game.world
        .query::<&BreedingVisit>()
        .iter(&game.world)
        .count()
}

/// What every cancelled visit must leave behind.
fn assert_called_off(game: &mut Game, bay: Option<Entity>, survivors: &[Entity], seeds: u32) {
    assert_eq!(seeds_in_pack(game), seeds, "the seed is back in the pack");
    if let Some(bay) = bay {
        assert_eq!(game.world.get::<Incubator>(bay).unwrap().slots, vec![None]);
    }
    assert_eq!(marked(game), 0, "no parent still carries the marker");
    for &p in survivors {
        assert!(game.world.get::<BreedReadyAt>(p).is_none(), "no cooldown");
        assert!(
            !memory_defs(game, p).iter().any(|(d, _)| d == "bred_with"),
            "no memory"
        );
    }
}

fn parent(role: Option<ProgramRole>, arrived: bool) -> ParentState {
    ParentState {
        role,
        downed: false,
        arrived,
        unreachable: false,
    }
}

fn input(parents: Vec<ParentState>, pause_left: u64) -> VisitInput {
    // A real entity is never read by `decide_visits`.
    VisitInput {
        bay: Entity::PLACEHOLDER,
        slot: 0,
        parents,
        pause_left,
    }
}

// ---- decide ----

#[test]
fn decide_waits_until_both_have_arrived_then_ticks_then_completes() {
    let v = Some(ProgramRole::Visiting);
    let out = decide_visits(&[
        input(vec![parent(v, true), parent(v, false)], 60),
        input(vec![parent(v, true), parent(v, true)], 60),
        input(vec![parent(v, true), parent(v, true)], 1),
        input(vec![parent(v, true), parent(v, true)], 0),
    ]);
    assert_eq!(
        out,
        vec![
            VisitOutcome::Wait,
            VisitOutcome::Tick,
            VisitOutcome::Tick,
            VisitOutcome::Complete
        ]
    );
}

#[test]
fn decide_cancels_for_each_reason() {
    let v = Some(ProgramRole::Visiting);
    let mut hurt = parent(v, true);
    hurt.downed = true;
    let out = decide_visits(&[
        input(vec![parent(v, true)], 10),
        input(vec![], 10),
        input(
            vec![parent(v, true), parent(Some(ProgramRole::InParty), true)],
            10,
        ),
        input(
            vec![parent(v, true), parent(Some(ProgramRole::Sortie), false)],
            10,
        ),
        input(vec![parent(v, true), parent(None, true)], 10),
        input(vec![parent(v, true), hurt], 10),
    ]);
    assert_eq!(
        out,
        vec![
            VisitOutcome::Cancel(CancelReason::ParentGone),
            VisitOutcome::Cancel(CancelReason::ParentGone),
            VisitOutcome::Cancel(CancelReason::LeftBase),
            VisitOutcome::Cancel(CancelReason::LeftBase),
            VisitOutcome::Cancel(CancelReason::LeftBase),
            VisitOutcome::Cancel(CancelReason::Downed),
        ]
    );
}

// ---- the whole visit, through the tick ----

#[test]
fn a_visit_walks_to_the_bay_pauses_and_then_starts_the_child() {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 2);
    game.breed(a, b, bay).unwrap();

    // Confirm reserves the slot and sends them; there is no child yet.
    assert_eq!(game.program_role(a), Some(ProgramRole::Visiting));
    assert_eq!(game.program_role(b), Some(ProgramRole::Visiting));
    assert_eq!(seeds_in_pack(&game), 1);
    assert!(game.world.get::<BreedReadyAt>(a).is_none());
    assert!(
        game.incubations(bay)[0]
            .child
            .as_ref()
            .unwrap()
            .visit
            .is_some()
    );

    until_paused(&mut game, bay);
    // Paused means standing beside the bay, both of them, on different tiles.
    let bay_at = *game.world.get::<Position>(bay).unwrap();
    let (pa, pb) = (
        *game.world.get::<Position>(a).unwrap(),
        *game.world.get::<Position>(b).unwrap(),
    );
    assert_ne!((pa.x, pa.y), (pb.x, pb.y));
    for p in [pa, pb] {
        assert_eq!(
            (p.x - bay_at.x).abs() + (p.y - bay_at.y).abs(),
            1,
            "beside the bay, on a side"
        );
    }

    finish_visit(&mut game, bay);
    let child = game.world.get::<Incubator>(bay).unwrap().slots[0]
        .clone()
        .unwrap();
    assert!(child.visit.is_none());
    assert!(child.due > game.world.resource::<GameClock>().tick);
    assert_eq!(marked(&mut game), 0);

    for (who, other) in [(a, b), (b, a)] {
        assert_eq!(game.program_role(who), Some(ProgramRole::Staff));
        assert_eq!(
            game.world.get::<BreedReadyAt>(who).unwrap().0,
            child.due - INCUBATION_TICKS + BREEDING_COOLDOWN_TICKS
        );
        let id = *game.world.get::<ProgramId>(other).unwrap();
        assert!(
            memory_defs(&game, who)
                .contains(&("bred_with".to_string(), MemorySubject::Program(id))),
            "{who:?} should remember the other parent"
        );
    }
}

#[test]
fn a_pending_slot_does_not_hatch_however_late_the_clock_is() {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 1);
    game.breed(a, b, bay).unwrap();
    game.world.resource_mut::<GameClock>().tick += INCUBATION_TICKS * 3;
    let pets = game.pet_count();
    game.hatch_incubations();
    assert!(game.world.get::<Incubator>(bay).unwrap().slots[0].is_some());
    assert_eq!(game.pet_count(), pets);
}

// ---- cancels ----

fn mid_visit() -> (Game, Entity, Entity, Entity) {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 1);
    game.breed(a, b, bay).unwrap();
    for _ in 0..3 {
        game.tick();
    }
    (game, a, b, bay)
}

#[test]
fn a_parent_that_cannot_route_to_the_bay_calls_the_visit_off() {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 1);
    // A laid cell with no floor joining it to the base: floor to stand on,
    // and no walk from it.
    game.world
        .resource_mut::<crate::base_grid::BaseGrid>()
        .lay_floor(-20, -20);
    *game.world.get_mut::<Position>(a).unwrap() = Position { x: -20, y: -20 };
    game.breed(a, b, bay).unwrap();
    for _ in 0..(BREEDING_VISIT_STUCK_TICKS * 4) {
        game.tick();
        if marked(&mut game) == 0 {
            break;
        }
    }
    assert_called_off(&mut game, Some(bay), &[a, b], 1);
    assert_eq!(game.program_role(a), Some(ProgramRole::Staff));
    let said: Vec<String> = game.message_log(1000).into_iter().map(|l| l.text).collect();
    assert!(
        said.iter().any(|l| l.contains("couldn't reach the bay")),
        "the log says why: {said:?}"
    );
}

#[test]
fn decide_cancels_for_a_parent_that_cannot_route() {
    let v = Some(ProgramRole::Visiting);
    let mut lost = parent(v, false);
    lost.unreachable = true;
    let out = decide_visits(&[input(vec![parent(v, true), lost], 10)]);
    assert_eq!(out, vec![VisitOutcome::Cancel(CancelReason::Unreachable)]);
}

#[test]
fn a_parent_joining_the_party_calls_the_visit_off() {
    let (mut game, a, b, bay) = mid_visit();
    enlist(&mut game, a);
    game.tick();
    assert_called_off(&mut game, Some(bay), &[b], 1);
}

#[test]
fn a_parent_sent_on_a_sortie_calls_the_visit_off() {
    let (mut game, a, b, bay) = mid_visit();
    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![a]));
    game.tick();
    assert_called_off(&mut game, Some(bay), &[b], 1);
}

#[test]
fn a_parent_going_down_calls_the_visit_off() {
    let (mut game, a, b, bay) = mid_visit();
    game.world.entity_mut(a).insert(Downed);
    game.tick();
    assert_called_off(&mut game, Some(bay), &[b], 1);
}

#[test]
fn a_parent_lost_calls_the_visit_off() {
    let (mut game, a, b, bay) = mid_visit();
    game.world.despawn(a);
    game.tick();
    assert_called_off(&mut game, Some(bay), &[b], 1);
}

#[test]
fn demolishing_the_bay_mid_visit_is_allowed_and_refunds_the_seed() {
    let (mut game, a, b, bay) = mid_visit();
    game.remove_structure(bay).unwrap();
    assert!(game.world.get::<Structure>(bay).is_none());
    assert_called_off(&mut game, None, &[a, b], 1);
}

#[test]
fn a_destroyed_bay_mid_visit_refunds_the_seed() {
    let (mut game, a, b, bay) = mid_visit();
    game.world
        .entity_mut(bay)
        .insert(Durability { hp: 1, max_hp: 1 });
    game.damage_structure(bay, u32::MAX, "The Breeding Bay", "a siege");
    assert!(game.world.get::<Structure>(bay).is_none());
    assert_called_off(&mut game, None, &[a, b], 1);
    let said: Vec<String> = game.message_log(1000).into_iter().map(|l| l.text).collect();
    assert!(
        !said.iter().any(|l| l.contains("lost with its bay")),
        "a pending slot holds no child to lose: {said:?}"
    );
}

// ---- needs hold ----

#[test]
fn a_visiting_parents_needs_hold_and_it_is_never_pulled_away() {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 1);
    let control = game.adopt_program("worm", 0, 0, 1.0).unwrap();
    // One tick seeds every store.
    game.tick();
    let (critical, content) = threshold(&game, &coherence());
    let level = (critical + content) / 2.0;
    for who in [a, b, control] {
        set_reserve(&mut game, who, &coherence(), level);
    }
    game.breed(a, b, bay).unwrap();
    // Every tick of the visit, not only its end: the tick that ends it hands
    // the parents back to the pool, and they drain from then on.
    let mut ticks = 0;
    while pause_left(&game, bay).is_some() {
        for who in [a, b] {
            assert_eq!(
                reserve(&game, who),
                level,
                "a visitor's reserve never drains"
            );
            assert!(game.world.get::<OffShift>(who).is_none());
        }
        game.tick();
        ticks += 1;
        assert!(ticks < 600, "the visit never finished");
    }
    // Long enough that staff in the same state would have drained.
    assert!(
        ticks > BREEDING_VISIT_PAUSE_TICKS,
        "the visit ran {ticks} ticks"
    );
    for who in [a, b] {
        assert!(
            !memory_defs(&game, who)
                .iter()
                .any(|(d, _)| d == "frayed_here" || d == "ran_down")
        );
    }
    // The control is long enough proof: staff over the same ticks do drain.
    assert!(reserve(&game, control) < level);
    assert!(game.world.get::<Needs>(a).is_some());
}

// ---- battle ----

#[test]
fn a_battle_pauses_the_visit_where_it_stands() {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 1);
    game.breed(a, b, bay).unwrap();
    until_paused(&mut game, bay);
    let held = pause_left(&game, bay);
    start_battle_with_a_wild_program(&mut game);
    for _ in 0..5 {
        game.step_breeding_visits();
    }
    assert_eq!(pause_left(&game, bay), held);
}

// ---- refusals ----

#[test]
fn each_new_refusal_names_its_reason() {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 3);
    let [c, d, e, f, g] = [(); 5].map(|_| game.adopt_program("worm", 0, 0, 1.0).unwrap());

    enlist(&mut game, c);
    assert_eq!(game.breed_refusal(c), Some(BreedRefusal::AwayFromBase));

    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![d]));
    assert_eq!(game.breed_refusal(d), Some(BreedRefusal::AwayFromBase));

    game.world.entity_mut(e).insert(UnderStudy { station: bay });
    assert_eq!(game.breed_refusal(e), Some(BreedRefusal::Held));

    game.world.entity_mut(f).insert(Downed);
    assert_eq!(game.breed_refusal(f), Some(BreedRefusal::Recovering));
    assert_eq!(game.breed_refusal(g), None);

    // On a visit, and breed() refuses it before anything is spent.
    game.breed(a, b, bay).unwrap();
    assert_eq!(game.breed_refusal(a), Some(BreedRefusal::OnVisit));
    let seeds = seeds_in_pack(&game);
    assert_eq!(game.breed(a, g, bay), Err(BreedRefusal::OnVisit));
    assert_eq!(seeds_in_pack(&game), seeds);

    for (r, text) in [
        (BreedRefusal::AwayFromBase, "away from base"),
        (BreedRefusal::Held, "held"),
        (BreedRefusal::OnVisit, "with the bay"),
        (BreedRefusal::Recovering, "recovering"),
    ] {
        assert_eq!(r.reason(), text);
    }
}

// ---- save and load ----

#[test]
fn a_visit_saved_mid_walk_resumes_and_completes() {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 1);
    game.breed(a, b, bay).unwrap();
    game.tick();
    assert_eq!(pause_left(&game, bay), Some(BREEDING_VISIT_PAUSE_TICKS));

    let mut loaded = save_and_load(&mut game, "walk");
    assert_eq!(marked(&mut loaded), 2, "both markers came back");
    let bay = loaded_bay(&mut loaded);
    finish_visit(&mut loaded, bay);
    assert_eq!(marked(&mut loaded), 0);
    assert!(loaded.world.get::<Incubator>(bay).unwrap().slots[0].is_some());
}

#[test]
fn a_visit_saved_mid_pause_keeps_its_pause() {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 1);
    game.breed(a, b, bay).unwrap();
    until_paused(&mut game, bay);
    for _ in 0..4 {
        game.tick();
    }
    let left = pause_left(&game, bay).unwrap();

    let mut loaded = save_and_load(&mut game, "pause");
    let bay = loaded_bay(&mut loaded);
    assert_eq!(
        loaded.world.get::<Incubator>(bay).unwrap().slots[0]
            .as_ref()
            .unwrap()
            .visit,
        Some(VisitPause { pause_left: left })
    );
    assert_eq!(marked(&mut loaded), 2);
    finish_visit(&mut loaded, bay);
    assert_eq!(
        loaded
            .world
            .query::<&BreedReadyAt>()
            .iter(&loaded.world)
            .count(),
        2,
        "both parents rest after the loaded visit"
    );
}

#[test]
fn a_pending_slot_with_no_parents_after_a_load_is_refunded() {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 1);
    game.breed(a, b, bay).unwrap();
    // The state a save leaves when a marker's bay no longer resolves.
    for p in [a, b] {
        game.world.entity_mut(p).remove::<BreedingVisit>();
    }
    let mut loaded = save_and_load(&mut game, "orphan");
    let bay = loaded_bay(&mut loaded);
    assert!(loaded.world.get::<Incubator>(bay).unwrap().slots[0].is_some());
    loaded.tick();
    assert_eq!(
        loaded.world.get::<Incubator>(bay).unwrap().slots,
        vec![None]
    );
    assert_eq!(seeds_in_pack(&loaded), 1);
}
