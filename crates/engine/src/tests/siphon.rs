//! The Power Siphon's ledger half: a siphon supplies its authored
//! `power_supply` only while some program holds a `Siphoned` pointing at it.
//! Every figure is read from the shipped def, never a literal.

use super::support::*;
use crate::components::{Durability, Needs, Siphoned};
use crate::game::base::power::ledger;
use crate::game::base::siphon::siphon_release_hp;
use crate::needs::NeedDb;
use crate::resources::{GameOver, Party, Sortie, Sorties, WieldedProgram};
use crate::structures::StructureDb;
use crate::tuning::SIPHON_RELEASE_INTEGRITY_LOSS;
use crate::*;

const SIPHON: &str = "power_siphon";

fn game() -> Game {
    Game::new(6101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn supply(game: &Game) -> u32 {
    let db = game.world.resource::<StructureDb>();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    ledger(&game.world, db, items).supply
}

fn authored_supply(game: &Game) -> u32 {
    game.world
        .resource::<StructureDb>()
        .get(SIPHON)
        .expect("the shipped siphon loads")
        .power_supply
}

#[test]
fn an_empty_siphon_supplies_nothing_and_draws_nothing() {
    let mut game = game();
    let baseline = supply(&game);
    let (draw_before, _) = game.base_power();
    spawn_structure_at(&mut game, SIPHON, 3, 3);

    assert_eq!(supply(&game), baseline);
    let (draw, supply_now) = game.base_power();
    assert_eq!(draw, draw_before);
    assert_eq!(supply_now, baseline);
}

#[test]
fn an_occupied_siphon_adds_its_authored_supply() {
    let mut game = game();
    let baseline = supply(&game);
    let siphon = spawn_structure_at(&mut game, SIPHON, 3, 3);
    let program = spawn_tamed(&mut game, 20, 5);
    game.world.entity_mut(program).insert(Siphoned { siphon });
    let authored = authored_supply(&game);
    assert!(authored > 0);

    assert_eq!(supply(&game), baseline + authored);
    assert_eq!(game.base_power().1, baseline + authored);
}

#[test]
fn releasing_the_marker_drops_the_supply_again() {
    let mut game = game();
    let baseline = supply(&game);
    let siphon = spawn_structure_at(&mut game, SIPHON, 3, 3);
    let program = spawn_tamed(&mut game, 20, 5);
    game.world.entity_mut(program).insert(Siphoned { siphon });
    game.world.entity_mut(program).remove::<Siphoned>();

    assert_eq!(supply(&game), baseline);
}

#[test]
fn a_siphon_with_a_dangling_marker_counts_nothing_extra() {
    let mut game = game();
    let baseline = supply(&game);
    let _empty = spawn_structure_at(&mut game, SIPHON, 3, 3);
    let elsewhere = spawn_structure_at(&mut game, "data_cache", 5, 5);
    let program = spawn_tamed(&mut game, 20, 5);
    game.world
        .entity_mut(program)
        .insert(Siphoned { siphon: elsewhere });

    assert_eq!(supply(&game), baseline);
}

#[test]
fn the_shipped_siphon_is_a_pure_supplier() {
    let game = game();
    let def = game
        .world
        .resource::<StructureDb>()
        .get(SIPHON)
        .expect("the shipped siphon loads");
    assert!(def.siphons);
    assert!(def.power_supply > 0);
    assert!(def.power_upkeep.is_none());
    assert!(def.work.is_none());
}

// ---------------------------------------------------------------------
// Phase 2: holding, release, the role and every census site
// ---------------------------------------------------------------------

/// A base with one empty siphon and one staff program standing clear of it.
fn base_with_a_siphon() -> (Game, Entity, Entity) {
    let mut game = game();
    stand_in_base(&mut game);
    let siphon = spawn_structure_at(&mut game, SIPHON, 3, 3);
    let program = spawn_tamed(&mut game, 100, 5);
    game.world.get_mut::<Position>(program).unwrap().x = 6;
    (game, siphon, program)
}

fn held(game: &Game, program: Entity) -> bool {
    game.world.get::<Siphoned>(program).is_some()
}

fn hp(game: &Game, program: Entity) -> i32 {
    game.world.get::<Stats>(program).unwrap().hp
}

fn expected_after_release(hp: i32, max_hp: i32) -> i32 {
    (hp - (max_hp as f32 * SIPHON_RELEASE_INTEGRITY_LOSS).ceil() as i32).max(1)
}

/// Asserts `siphon_program` refused with `fragment` in its message and wrote
/// nothing: no marker, the log untouched, and the `Task` (when one was
/// there) still in place.
fn assert_refused_untouched(game: &mut Game, program: Entity, siphon: Entity, fragment: &str) {
    let log_before = game.message_log(200).len();
    let had_task = game.world.get::<Task>(program).is_some();
    let err = game
        .siphon_program(program, siphon)
        .expect_err("this hold must be refused");
    assert!(err.contains(fragment), "unexpected error: {err}");
    assert!(!held(game, program), "a refusal must write no marker");
    assert_eq!(game.message_log(200).len(), log_before, "nothing is logged");
    assert_eq!(game.world.get::<Task>(program).is_some(), had_task);
}

#[test]
fn siphoning_a_staff_program_holds_it_and_frees_its_post() {
    let (mut game, siphon, program) = base_with_a_siphon();
    let target = spawn_structure_at(&mut game, "data_cache", 8, 8);
    game.world.entity_mut(program).insert(Task {
        kind: TaskKind::GatherResource,
        target,
        progress: 0,
        required: 10,
    });

    game.siphon_program(program, siphon).unwrap();

    assert_eq!(game.program_role(program), Some(ProgramRole::Siphoned));
    assert_eq!(game.siphon_holder(siphon), Some(program));
    assert!(game.world.get::<Task>(program).is_none());
    assert!(!game.base_staff().contains(&program));
}

#[test]
fn a_siphon_refuses_in_a_battle_or_a_finished_run_and_writes_nothing() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world.resource_mut::<GameOver>().reason = Some("done".into());
    assert_refused_untouched(&mut game, program, siphon, "right now");
    game.world.resource_mut::<GameOver>().reason = None;

    let enemy = spawn_wild_without_routine(&mut game, "scrapper", 5, 5);
    let player = game.player_entity();
    insert_battle(&mut game, player, vec![enemy]);
    assert_refused_untouched(&mut game, program, siphon, "right now");
}

#[test]
fn a_siphon_refuses_a_program_that_is_not_yours_and_writes_nothing() {
    let (mut game, siphon, _) = base_with_a_siphon();
    let wild = spawn_wild_without_routine(&mut game, "scrapper", 5, 5);
    assert_refused_untouched(&mut game, wild, siphon, "compiled under your control");

    let other = spawn_tamed(&mut game, 10, 3);
    let stranger = game.world.spawn_empty().id();
    game.world.get_mut::<Tamed>(other).unwrap().owner = stranger;
    assert_refused_untouched(&mut game, other, siphon, "don't control");
}

#[test]
fn a_siphon_refuses_every_program_that_is_not_plain_staff_and_writes_nothing() {
    // Partied.
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world.resource_mut::<Party>().0.push(program);
    assert_refused_untouched(&mut game, program, siphon, "bring it home");

    // Wielded.
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world.insert_resource(WieldedProgram(Some(program)));
    assert_refused_untouched(&mut game, program, siphon, "bring it home");

    // On a sortie.
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![program]));
    assert_refused_untouched(&mut game, program, siphon, "bring it home");

    // Posted at an outpost.
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world
        .entity_mut(program)
        .insert(components::PostedAt((5, 5)));
    assert_refused_untouched(&mut game, program, siphon, "bring it home");

    // Pinned for study.
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world
        .entity_mut(program)
        .insert(components::UnderStudy { station: siphon });
    assert_refused_untouched(&mut game, program, siphon, "bring it home");

    // Already held, by another siphon.
    let (mut game, siphon, program) = base_with_a_siphon();
    let second = spawn_structure_at(&mut game, SIPHON, 9, 9);
    game.siphon_program(program, second).unwrap();
    let log_before = game.message_log(200).len();
    let err = game.siphon_program(program, siphon).unwrap_err();
    assert!(err.contains("bring it home"), "unexpected error: {err}");
    assert_eq!(game.siphon_holder(siphon), None);
    assert_eq!(game.siphon_holder(second), Some(program));
    assert_eq!(game.message_log(200).len(), log_before);
}

#[test]
fn a_siphon_refuses_a_structure_that_is_not_a_siphon_and_writes_nothing() {
    let (mut game, _, program) = base_with_a_siphon();
    let cache = spawn_structure_at(&mut game, "data_cache", 8, 8);
    assert_refused_untouched(&mut game, program, cache, "can't hold");
    let gone = game.world.spawn_empty().id();
    assert_refused_untouched(&mut game, program, gone, "can't hold");
}

#[test]
fn a_siphon_refuses_an_occupied_siphon_and_writes_nothing() {
    let (mut game, siphon, first) = base_with_a_siphon();
    game.siphon_program(first, siphon).unwrap();
    let second = spawn_tamed(&mut game, 10, 3);
    assert_refused_untouched(&mut game, second, siphon, "already holding");
    assert_eq!(game.siphon_holder(siphon), Some(first));
}

#[test]
fn a_siphon_refuses_outside_base_space_and_writes_nothing() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world.insert_resource(Locale::Surface);
    assert_refused_untouched(&mut game, program, siphon, "back at the base");
}

#[test]
fn a_held_program_is_never_handed_a_task_and_never_moves() {
    let mut game = game();
    stand_in_base(&mut game);
    place_home(&mut game);
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);
    spawn_machine_at(&mut game, "lathe", 3, 0);
    spawn_machine_at(&mut game, "disk_press", 4, 0);
    let siphon = spawn_structure_at(&mut game, SIPHON, 5, 5);
    let held_one = spawn_tamed(&mut game, 100, 5);
    let crew = [
        spawn_tamed(&mut game, 100, 5),
        spawn_tamed(&mut game, 100, 5),
    ];
    game.siphon_program(held_one, siphon).unwrap();
    let start = *game.world.get::<Position>(held_one).unwrap();
    game.queue_work_order(WorkOrder::batch(ItemId::from("routine_disk"), 30))
        .unwrap();

    let mut crew_worked = false;
    for _ in 0..60 {
        game.tick();
        crew_worked |= crew.iter().any(|c| game.world.get::<Task>(*c).is_some());
        assert!(game.world.get::<Task>(held_one).is_none());
        assert_eq!(*game.world.get::<Position>(held_one).unwrap(), start);
    }
    assert!(crew_worked, "the work must have been there to hand out");
    let _ = mine;
}

#[test]
fn a_held_program_does_not_drain_needs() {
    let (mut game, siphon, held_one) = base_with_a_siphon();
    let control = spawn_tamed(&mut game, 100, 5);
    game.siphon_program(held_one, siphon).unwrap();
    let ids: Vec<_> = game
        .world
        .resource::<NeedDb>()
        .iter()
        .map(|d| d.id.clone())
        .collect();
    assert!(!ids.is_empty(), "the shipped catalogue has needs");
    game.world.resource_scope(|world, db: Mut<NeedDb>| {
        for e in [held_one, control] {
            world.get_mut::<Needs>(e).unwrap().seed_missing(&db);
        }
    });
    let read = |game: &Game, e: Entity| -> Vec<Option<f32>> {
        let needs = game.world.get::<Needs>(e).unwrap();
        ids.iter().map(|id| needs.get(id)).collect()
    };
    let before = read(&game, held_one);
    let control_before = read(&game, control);

    for _ in 0..20 {
        game.tick();
    }

    assert_eq!(read(&game, held_one), before);
    assert_ne!(read(&game, control), control_before, "staff do drain");
}

#[test]
fn release_hp_is_three_quarters_of_max_and_never_below_one() {
    assert_eq!(siphon_release_hp(100, 100), 25);
    assert_eq!(siphon_release_hp(80, 100), 5);
    assert_eq!(siphon_release_hp(75, 100), 1);
    assert_eq!(siphon_release_hp(30, 100), 1);
    assert_eq!(siphon_release_hp(1, 100), 1);
    assert_eq!(siphon_release_hp(1, 1), 1);
}

#[test]
fn a_full_health_release_leaves_at_most_a_quarter_whatever_the_max_hp() {
    // Truncating the loss left 10/38 and 13/49, past the quoted quarter.
    assert_eq!(siphon_release_hp(38, 38), 9);
    assert_eq!(siphon_release_hp(49, 49), 12);
    assert_eq!(siphon_release_hp(1, 1), 1);
    assert_eq!(siphon_release_hp(100, 100), 25);
    for max_hp in 4..300 {
        let left = siphon_release_hp(max_hp, max_hp);
        assert!(
            left * 4 <= max_hp,
            "{left} of {max_hp} is more than a quarter"
        );
    }
}

#[test]
fn release_takes_75_percent_of_max_hp_and_never_below_one() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.siphon_program(program, siphon).unwrap();
    game.release_siphoned(program).unwrap();
    assert_eq!(hp(&game, program), expected_after_release(100, 100));
    assert_eq!(hp(&game, program), 25);

    // Already hurt past the price: floored at 1, not killed.
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world.get_mut::<Stats>(program).unwrap().hp = 40;
    game.siphon_program(program, siphon).unwrap();
    game.release_siphoned(program).unwrap();
    assert_eq!(hp(&game, program), 1);

    // Already at 1.
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world.get_mut::<Stats>(program).unwrap().hp = 1;
    game.siphon_program(program, siphon).unwrap();
    game.release_siphoned(program).unwrap();
    assert_eq!(hp(&game, program), 1);
}

#[test]
fn releasing_refuses_a_program_that_is_not_held_or_a_battle_and_changes_nothing() {
    let (mut game, siphon, program) = base_with_a_siphon();
    let err = game.release_siphoned(program).unwrap_err();
    assert!(err.contains("isn't held"), "unexpected error: {err}");
    assert_eq!(hp(&game, program), 100);

    game.siphon_program(program, siphon).unwrap();
    game.world.resource_mut::<GameOver>().reason = Some("done".into());
    assert!(game.release_siphoned(program).is_err());
    assert!(held(&game, program), "a refused release keeps the hold");
    assert_eq!(hp(&game, program), 100);
}

#[test]
fn release_returns_the_program_to_staff_and_it_is_postable_again() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.siphon_program(program, siphon).unwrap();
    game.release_siphoned(program).unwrap();

    assert_eq!(game.program_role(program), Some(ProgramRole::Staff));
    assert!(game.base_staff().contains(&program));
    assert_eq!(game.siphon_holder(siphon), None);
    game.siphon_program(program, siphon)
        .expect("the siphon is free to hold again");
}

#[test]
fn deconstructing_an_occupied_siphon_releases_its_program_hurt() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.siphon_program(program, siphon).unwrap();

    game.remove_structure(siphon).unwrap();

    assert!(!held(&game, program));
    assert_eq!(game.program_role(program), Some(ProgramRole::Staff));
    assert_eq!(hp(&game, program), expected_after_release(100, 100));
}

#[test]
fn a_raid_destroying_the_siphon_releases_its_program_hurt() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world
        .entity_mut(siphon)
        .insert(Durability { hp: 5, max_hp: 5 });
    game.siphon_program(program, siphon).unwrap();

    game.damage_structure(siphon, 99, "Power Siphon", "a raid");

    assert!(game.world.get_entity(siphon).is_err(), "the siphon is gone");
    assert!(!held(&game, program));
    assert_eq!(hp(&game, program), expected_after_release(100, 100));
}

#[test]
fn a_raid_that_only_damages_the_siphon_keeps_its_program_held() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world
        .entity_mut(siphon)
        .insert(Durability { hp: 50, max_hp: 50 });
    game.siphon_program(program, siphon).unwrap();

    game.damage_structure(siphon, 1, "Power Siphon", "a raid");

    assert!(held(&game, program));
    assert_eq!(hp(&game, program), 100);
}

#[test]
fn demolishing_the_home_releases_every_held_program() {
    let mut game = game();
    place_home(&mut game);
    stand_in_base(&mut game);
    let a = spawn_structure_at(&mut game, SIPHON, 3, 3);
    let b = spawn_structure_at(&mut game, SIPHON, 6, 6);
    let first = spawn_tamed(&mut game, 100, 5);
    let second = spawn_tamed(&mut game, 80, 5);
    game.siphon_program(first, a).unwrap();
    game.siphon_program(second, b).unwrap();
    let home = game
        .world
        .query::<(Entity, &Structure)>()
        .iter(&game.world)
        .find(|(_, s)| s.kind == "home")
        .map(|(e, _)| e)
        .expect("the Home stands");

    game.remove_structure(home).unwrap();

    for (program, before) in [(first, 100), (second, 80)] {
        assert!(!held(&game, program));
        assert_eq!(hp(&game, program), expected_after_release(before, before));
    }
}

#[test]
fn a_held_program_does_not_occupy_ground_and_is_not_drawn() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.siphon_program(program, siphon).unwrap();
    let at = *game.world.get::<Position>(program).unwrap();

    assert!(!crate::game::party::walks_the_base(
        game.program_role(program),
        None
    ));
    assert!(
        !game
            .base_bodies()
            .into_iter()
            .any(|(e, p)| e == program || (p.x, p.y) == (at.x, at.y)),
        "a held body blocks no cell"
    );
    assert!(!game.position_is_honest(program));
}

#[test]
fn a_held_program_cannot_be_pinned_for_study_or_dispatched() {
    let (mut game, siphon, program) = base_with_a_siphon();
    let station = spawn_structure_at(&mut game, "data_cache", 8, 8);
    game.siphon_program(program, siphon).unwrap();

    let err = game.pin_subject(program, station).unwrap_err();
    assert!(err.contains("base staff"), "unexpected error: {err}");
    assert!(game.world.get::<components::UnderStudy>(program).is_none());

    let (mut game, siphon, program) = base_with_a_siphon();
    super::routes::deploy_relay(&mut game);
    game.siphon_program(program, siphon).unwrap();
    let site = game.sortie_board().expect("a Relay stands")[0].id.clone();
    let before = game.world.resource::<Sorties>().0.len();
    assert!(matches!(
        game.dispatch_sortie(&site, &[program]),
        Err(crate::game::sortie::SortieRefusal::NotStaff(_))
    ));
    assert_eq!(game.world.resource::<Sorties>().0.len(), before);
    assert!(held(&game, program), "refusal leaves the hold alone");
}

#[test]
fn roster_rank_places_siphoned_between_under_study_and_staff() {
    assert!(ProgramRole::UnderStudy.roster_rank() < ProgramRole::Siphoned.roster_rank());
    assert!(ProgramRole::Siphoned.roster_rank() < ProgramRole::Staff.roster_rank());
}

#[test]
fn a_held_program_is_not_offered_for_a_build_spend() {
    let (mut game, siphon, program) = base_with_a_siphon();
    // A lone program is never offered (the roster may not be emptied).
    spawn_tamed(&mut game, 10, 3);
    let before = game
        .programs_for_build(1)
        .iter()
        .any(|p| p.entity == program);
    assert!(before, "a staff program is offered");
    game.siphon_program(program, siphon).unwrap();
    assert!(
        !game
            .programs_for_build(1)
            .iter()
            .any(|p| p.entity == program),
        "a held program must not be offered"
    );
}

fn assert_held_refusal(err: &str) {
    assert!(err.contains("Power Siphon"), "unexpected error: {err}");
    assert!(err.contains("Release it first"), "unexpected error: {err}");
}

#[test]
fn add_companion_refuses_a_held_program() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.siphon_program(program, siphon).unwrap();
    assert_held_refusal(&game.add_companion(program).unwrap_err());
    assert!(!game.world.resource::<Party>().0.contains(&program));
    assert_eq!(game.program_role(program), Some(ProgramRole::Siphoned));
}

#[test]
fn wield_program_refuses_a_held_program() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.siphon_program(program, siphon).unwrap();
    assert_held_refusal(&game.wield_program(program).unwrap_err());
    assert_eq!(game.wielded_program(), None);
    assert_eq!(game.program_role(program), Some(ProgramRole::Siphoned));
}

#[test]
fn fuse_companions_refuses_a_held_program_as_either_input() {
    let (mut game, siphon, program) = base_with_a_siphon();
    unlock_research_chain(&mut game, "program_refactoring");
    let other = spawn_tamed(&mut game, 10, 3);
    game.siphon_program(program, siphon).unwrap();

    assert_held_refusal(&game.fuse_companions(program, other, None).unwrap_err());
    assert_held_refusal(&game.fuse_companions(other, program, None).unwrap_err());
    assert!(game.world.get::<Creature>(program).is_some());
    assert!(game.world.get::<Creature>(other).is_some());
}

#[test]
fn sell_companion_refuses_a_held_program() {
    let (mut game, siphon, program) = base_with_a_siphon();
    let market = spawn_market(&mut game);
    game.siphon_program(program, siphon).unwrap();
    assert_held_refusal(&game.sell_companion(market, program).unwrap_err());
    assert!(game.world.get::<Stats>(program).is_some(), "not sold");
    assert!(held(&game, program));
}

#[test]
fn open_kernel_ring_refuses_a_held_program_and_spends_nothing() {
    let (mut game, siphon, program) = base_with_a_siphon();
    let player = game.player_entity();
    let ring = ItemId::from(crate::items::ids::PRIVILEGE_RING);
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ring.clone(), 4);
    game.siphon_program(program, siphon).unwrap();

    assert_held_refusal(&game.open_kernel_ring(program).unwrap_err());
    assert_eq!(game.world.get::<Inventory>(player).unwrap().count(&ring), 4);
}

#[test]
fn extract_routine_refuses_a_held_program() {
    let (mut game, siphon, program) = base_with_a_siphon();
    let prize = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .wild_pool()
        .into_iter()
        .map(|(def, _)| def.id.clone())
        .next()
        .expect("some shipped ability is wild-poolable");
    game.world
        .get_mut::<components::Routines>(program)
        .unwrap()
        .0 = vec![prize];
    spawn_structure_at(&mut game, "compiler", 30, 30);
    game.siphon_program(program, siphon).unwrap();

    assert_held_refusal(&game.extract_routine(program, 0).unwrap_err());
    assert!(
        game.world.get::<Creature>(program).is_some(),
        "not consumed"
    );
}

#[test]
fn committing_a_held_program_is_refused() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.siphon_program(program, siphon).unwrap();
    assert!(game.commit_program(program).is_none());
    assert!(game.world.get_entity(program).is_ok());
}

#[test]
fn a_held_program_is_not_healed_by_a_rest() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world.get_mut::<Stats>(program).unwrap().hp = 10;
    game.siphon_program(program, siphon).unwrap();

    game.rest().unwrap();

    assert_eq!(hp(&game, program), 10);
}

#[test]
fn the_roster_files_a_held_program_under_its_own_role() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.siphon_program(program, siphon).unwrap();
    let row = game
        .owned_pets()
        .into_iter()
        .find(|p| p.entity == program)
        .expect("a held program is still on the roster");
    assert_eq!(row.role, ProgramRole::Siphoned);
}

// ---------------------------------------------------------------------
// Phase 3: the grudge
// ---------------------------------------------------------------------

use crate::components::{Disgruntled, Memories, MemorySubject};
use crate::disposition::Disposition;
use crate::memories::MemoryDb;
use crate::tuning::{
    MORALE_DOWNS_TOOLS_AT, MORALE_RECOVERED_AT, MORALE_SULKS_AT, SIPHON_GRUDGE_PERIOD,
};

fn wind_to(game: &mut Game, tick: u64) {
    game.world.resource_mut::<GameClock>().tick = tick;
}

fn siphoned_entries(game: &Game, who: Entity) -> Vec<(MemorySubject, u32)> {
    game.world
        .get::<Memories>(who)
        .map(|m| {
            m.0.iter()
                .filter(|m| m.def.as_str() == "siphoned")
                .map(|m| (m.subject.clone(), m.strikes))
                .collect()
        })
        .unwrap_or_default()
}

fn grudge_def(game: &Game) -> (f32, u32, u64) {
    let def = game
        .world
        .resource::<MemoryDb>()
        .get(&crate::memories::MemoryId::from("siphoned"))
        .expect("the shipped catalogue defines siphoned");
    (def.valence, def.strike_cap, def.half_life)
}

/// A held, Steady program, and the clock walked through `periods` grudge
/// periods the way `tick_inner` would reach them.
fn held_for(periods: u64) -> (Game, Entity, Entity) {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world.entity_mut(program).insert(Disposition::Steady);
    game.siphon_program(program, siphon).unwrap();
    for n in 1..=periods {
        wind_to(&mut game, n * SIPHON_GRUDGE_PERIOD);
        game.note_siphoned();
    }
    (game, siphon, program)
}

#[test]
fn a_held_program_remembers_the_siphon_on_the_grudge_period_only() {
    let (mut game, siphon, program) = base_with_a_siphon();
    let spot = *game.world.get::<Position>(siphon).unwrap();
    let idle = spawn_tamed(&mut game, 100, 5);

    // Empty siphon: nothing to remember, even on the period.
    wind_to(&mut game, SIPHON_GRUDGE_PERIOD);
    game.note_siphoned();
    assert!(siphoned_entries(&game, program).is_empty());

    game.siphon_program(program, siphon).unwrap();
    wind_to(&mut game, SIPHON_GRUDGE_PERIOD + 1);
    game.note_siphoned();
    assert!(siphoned_entries(&game, program).is_empty(), "off-period");

    wind_to(&mut game, 2 * SIPHON_GRUDGE_PERIOD);
    game.note_siphoned();
    assert_eq!(
        siphoned_entries(&game, program),
        vec![(
            MemorySubject::BaseTile {
                x: spot.x,
                y: spot.y
            },
            1
        )]
    );

    wind_to(&mut game, 3 * SIPHON_GRUDGE_PERIOD);
    game.note_siphoned();
    assert_eq!(siphoned_entries(&game, program).len(), 1, "one record");
    assert_eq!(
        siphoned_entries(&game, program)[0].1,
        2,
        "one strike per period"
    );
    assert!(
        siphoned_entries(&game, idle).is_empty(),
        "staff are not held"
    );
}

#[test]
fn one_period_of_holding_does_not_yet_sulk() {
    let (game, _, program) = held_for(1);
    assert!(game.morale(program) > MORALE_SULKS_AT);
    assert!(game.morale(program) < 0.0, "but it is a grudge");
}

#[test]
fn after_a_capped_hold_morale_is_past_sulks_and_downs_tools_for_a_steady_program() {
    let (_, cap, _) = grudge_def(&game());
    let (game, _, program) = held_for(cap as u64);
    let (valence, cap, _) = grudge_def(&game);
    let felt = Disposition::Steady.felt(valence * cap as f32);
    assert!(
        felt <= MORALE_DOWNS_TOOLS_AT,
        "the shipped numbers cross the line"
    );
    assert!(game.morale(program) <= MORALE_SULKS_AT);
    assert!(game.morale(program) <= MORALE_DOWNS_TOOLS_AT);
}

#[test]
fn after_release_morale_climbs_back_as_the_grudge_decays() {
    let (_, cap, _) = grudge_def(&game());
    let (mut game, _, program) = held_for(cap as u64);
    let (_, _, half_life) = grudge_def(&game);

    game.release_siphoned(program).unwrap();
    assert_eq!(game.program_role(program), Some(ProgramRole::Staff));
    game.update_disgruntled(&[program]);
    assert!(
        game.world.get::<Disgruntled>(program).is_some(),
        "the grudge has a visible consequence"
    );

    let now = game.current_tick();
    wind_to(&mut game, now + 4 * half_life);
    assert!(game.morale(program) > MORALE_RECOVERED_AT);
    game.update_disgruntled(&[program]);
    assert!(game.world.get::<Disgruntled>(program).is_none());
}

/// The wiring, not the function: `tick` itself must reach `note_siphoned`.
#[test]
fn ticking_through_a_grudge_period_strikes_the_held_program() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.siphon_program(program, siphon).unwrap();
    wind_to(&mut game, SIPHON_GRUDGE_PERIOD - 2);
    for _ in 0..4 {
        game.tick();
    }
    assert_eq!(
        siphoned_entries(&game, program).len(),
        1,
        "a tick crossing the period writes one strike"
    );
}

#[test]
fn the_release_quote_is_the_charged_price() {
    let left = ((1.0 - SIPHON_RELEASE_INTEGRITY_LOSS) * 100.0).round() as u32;
    assert_eq!(Game::siphon_release_ceiling_percent(), left);
}

/// A sulking program siphoned beside an amenity is frozen mid-errand. The
/// marker it carried must not outlive the hold: `note_respites` would keep
/// writing `unwound_at` for it, topping the fondness up for the whole hold.
#[test]
fn a_held_program_that_was_sulking_collects_no_respite_and_downs_tools() {
    let (mut game, siphon, program) = base_with_a_siphon();
    place_home(&mut game);
    spawn_structure_at(&mut game, "defrag_bay", 4, 0);
    {
        let mut at = game.world.get_mut::<Position>(program).unwrap();
        at.x = 3;
        at.y = 0;
    }
    game.world.entity_mut(program).insert(Disposition::Steady);
    super::respite::sulk(&mut game, program);
    game.siphon_program(program, siphon).unwrap();
    assert!(
        game.world.get::<Disgruntled>(program).is_none(),
        "a held program is out of the role system, mood marker included"
    );

    let (_, cap, _) = grudge_def(&game);
    let start = game.current_tick();
    for _ in 0..SIPHON_GRUDGE_PERIOD * cap as u64 {
        game.tick();
    }
    assert!(game.current_tick() >= start + SIPHON_GRUDGE_PERIOD * cap as u64);
    assert!(held(&game, program));
    let unwound = game
        .world
        .get::<Memories>(program)
        .map(|m| m.0.iter().any(|m| m.def.as_str() == "unwound_at"))
        .unwrap_or(false);
    assert!(!unwound, "a held program takes no respite");
    assert!(game.morale(program) <= MORALE_DOWNS_TOOLS_AT);
}

#[test]
fn holding_a_program_that_carries_a_kill_puts_the_kill_back() {
    let (mut game, siphon, program) = base_with_a_siphon();
    let player = game.player_entity();
    let shelved = game
        .world
        .get::<components::DownedPrograms>(player)
        .unwrap()
        .0
        .len();
    game.world
        .entity_mut(program)
        .insert(components::CarryingProgram(crate::items::DownedProgram {
            species: "scrapper".to_string(),
            level: 5,
            rarity: crate::components::Rarity::Ordinary,
            boss: false,
            condition: 70,
            carried: None,
        }));

    game.siphon_program(program, siphon).unwrap();

    assert!(
        game.world
            .get::<components::CarryingProgram>(program)
            .is_none()
    );
    assert_eq!(
        game.world
            .get::<components::DownedPrograms>(player)
            .unwrap()
            .0
            .len(),
        shelved + 1,
        "the kill comes back to the pack rather than vanishing"
    );
}

#[test]
fn a_temporary_siphon_expiring_releases_its_program_hurt() {
    let (mut game, siphon, program) = base_with_a_siphon();
    game.world
        .entity_mut(siphon)
        .insert(components::Temporary { ticks_remaining: 1 });
    game.siphon_program(program, siphon).unwrap();

    game.age_temporary_structures();

    assert!(
        game.world.get_entity(siphon).is_err(),
        "the siphon burned out"
    );
    assert!(!held(&game, program), "no marker to a dead entity");
    assert_eq!(game.program_role(program), Some(ProgramRole::Staff));
    assert_eq!(hp(&game, program), 25);
}
