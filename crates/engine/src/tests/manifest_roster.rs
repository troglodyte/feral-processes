//! The manifest's rail: who is on it and in what order, what each row asks
//! of the player, and where each subject ranks.

use super::support::*;
use crate::components::{CustomName, HoldPoints, ProgramId, StatPoints};
use crate::*;

fn game() -> Game {
    Game::new(9301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn species(game: &Game) -> String {
    game.species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .to_string()
}

fn named_program(game: &mut Game, name: &str) -> Entity {
    let s = species(game);
    let program = game.adopt_program(&s, 70, 70, 1.0).unwrap();
    game.world
        .entity_mut(program)
        .insert(CustomName(name.to_string()));
    program
}

fn put_in_party(game: &mut Game, programs: &[Entity]) {
    game.world.resource_mut::<Party>().0 = programs.to_vec();
}

#[test]
fn the_rail_leads_with_you_then_the_party_in_slot_order_then_the_rest_a_to_z() {
    let mut game = game();
    let player = game.player_entity();
    let zed = named_program(&mut game, "zed");
    let apple = named_program(&mut game, "Apple");
    let mango = named_program(&mut game, "mango");
    let slot0 = named_program(&mut game, "yak");
    let slot1 = named_program(&mut game, "bee");
    put_in_party(&mut game, &[slot0, slot1]);
    assert_eq!(
        game.manifest_subjects(),
        vec![player, slot0, slot1, apple, mango, zed],
        "case-insensitive A-Z behind the party, however the names are cased"
    );
}

#[test]
fn equal_names_fall_back_to_program_id() {
    let mut game = game();
    let player = game.player_entity();
    let first = named_program(&mut game, "twin");
    let second = named_program(&mut game, "twin");
    let id = |g: &Game, e| g.world.get::<ProgramId>(e).copied().unwrap();
    assert!(id(&game, first) < id(&game, second));
    assert_eq!(game.manifest_subjects(), vec![player, first, second]);
}

#[test]
fn the_rail_has_the_same_members_as_the_roster() {
    let mut game = game();
    for name in ["c", "a", "b"] {
        named_program(&mut game, name);
    }
    let mut rail = game.manifest_subjects();
    rail.remove(0);
    let mut owned: Vec<Entity> = game.owned_pets().into_iter().map(|p| p.entity).collect();
    rail.sort();
    owned.sort();
    assert_eq!(rail, owned);
}

#[test]
fn owned_pets_keeps_its_role_order() {
    let mut game = game();
    let a = named_program(&mut game, "a");
    let b = named_program(&mut game, "b");
    put_in_party(&mut game, &[b]);
    let order: Vec<Entity> = game.owned_pets().into_iter().map(|p| p.entity).collect();
    assert_eq!(order, vec![b, a], "the party still leads the roster screen");
}

fn bank_points(game: &mut Game, program: Entity, hold: bool, points: u32) {
    game.world.get_mut::<HoldPoints>(program).unwrap().0 = hold;
    game.world.get_mut::<StatPoints>(program).unwrap().0 = points;
}

#[test]
fn banked_points_are_flagged_and_the_flag_clears_once_spent() {
    let mut game = game();
    let program = named_program(&mut game, "a");
    put_in_party(&mut game, &[program]);
    bank_points(&mut game, program, true, 3);
    assert_eq!(game.call_to_act(program), Some(CallToAct::DevPoints(3)));
    bank_points(&mut game, program, true, 0);
    assert_eq!(game.call_to_act(program), None);
}

#[test]
fn points_that_are_not_held_are_not_a_call_to_act() {
    let mut game = game();
    let program = named_program(&mut game, "a");
    put_in_party(&mut game, &[program]);
    bank_points(&mut game, program, false, 3);
    assert_eq!(game.call_to_act(program), None);
}

#[test]
fn the_attention_count_equals_the_dev_points_on_the_rail() {
    let mut game = game();
    let a = named_program(&mut game, "a");
    let b = named_program(&mut game, "b");
    let c = named_program(&mut game, "c");
    bank_points(&mut game, a, true, 2);
    bank_points(&mut game, b, false, 2);
    bank_points(&mut game, c, true, 1);
    let flagged = game
        .manifest_subjects()
        .into_iter()
        .filter(|e| matches!(game.call_to_act(*e), Some(CallToAct::DevPoints(_))))
        .count();
    assert_eq!(flagged, 2);
    let row = game
        .attention()
        .into_iter()
        .find(|r| r.kind == AttentionKind::ProgramPoints)
        .unwrap();
    assert_eq!(row.text, "2 programs have points to spend (Manifest)");
    bank_points(&mut game, a, true, 0);
    bank_points(&mut game, c, true, 0);
    assert!(
        game.attention()
            .iter()
            .all(|r| r.kind != AttentionKind::ProgramPoints)
    );
}

#[test]
fn an_idle_staff_program_is_flagged_idle() {
    let mut game = game();
    let program = named_program(&mut game, "a");
    assert_eq!(game.program_activity(program), "idle");
    assert_eq!(game.call_to_act(program), Some(CallToAct::Idle));
}

#[test]
fn a_party_member_is_not_idle() {
    let mut game = game();
    let program = named_program(&mut game, "a");
    put_in_party(&mut game, &[program]);
    assert_eq!(game.call_to_act(program), None);
}

#[test]
fn a_player_with_unspent_perk_points_is_flagged() {
    let mut game = game();
    let player = game.player_entity();
    assert_eq!(game.call_to_act(player), None);
    game.world.get_mut::<Perks>(player).unwrap().points = 5;
    assert_eq!(game.call_to_act(player), Some(CallToAct::PerkPoints(5)));
}

#[test]
fn the_roster_rows_follow_the_rail_with_party_and_call_to_act_filled() {
    let mut game = game();
    let a = named_program(&mut game, "a");
    let b = named_program(&mut game, "b");
    put_in_party(&mut game, &[b]);
    bank_points(&mut game, a, true, 2);
    let rail = game.manifest_subjects();
    let rows = game.manifest_roster();
    assert_eq!(rows.iter().map(|r| r.entity).collect::<Vec<_>>(), rail);
    let by = |e| rows.iter().find(|r| r.entity == e).unwrap();
    assert!(!by(game.player_entity()).in_party);
    assert!(by(b).in_party);
    assert!(!by(a).in_party);
    assert_eq!(by(a).needs_you, Some(CallToAct::DevPoints(2)));
    assert_eq!(by(b).needs_you, None);
    assert_eq!(by(b).level, 1);
    assert!(by(b).name.to_lowercase().contains('b'));
}

#[test]
fn the_player_and_two_programs_give_each_stat_its_expected_roster_size() {
    let mut game = game();
    let a = named_program(&mut game, "a");
    named_program(&mut game, "b");
    let ranks = game.subject_ranks(a).unwrap();
    let of = |stat| ranks.rows.iter().find(|r| r.stat == stat).map(|r| r.of);
    assert_eq!(of(RankedStat::Damage), Some(3));
    assert_eq!(of(RankedStat::Attack), Some(3));
    assert_eq!(of(RankedStat::Mitigation), Some(3));
    assert_eq!(of(RankedStat::Power), Some(3));
    assert_eq!(of(RankedStat::Potential), Some(2), "programs only");
    assert_eq!(of(RankedStat::WorkAptitude), Some(2), "programs only");
}

#[test]
fn the_player_is_not_ranked_on_program_only_stats() {
    let mut game = game();
    named_program(&mut game, "a");
    let player = game.player_entity();
    let ranks = game.subject_ranks(player).unwrap();
    assert!(
        ranks
            .rows
            .iter()
            .all(|r| !matches!(r.stat, RankedStat::Potential | RankedStat::WorkAptitude))
    );
    assert_eq!(ranks.rows.len(), 4);
}

#[test]
fn a_body_off_the_rail_has_no_ranks() {
    let mut game = game();
    let s = species(&game);
    let wild = game
        .spawn_wild_creature_scaled(&s, 60, 60, 1.0, false)
        .unwrap();
    assert!(game.subject_ranks(wild).is_none());
}
