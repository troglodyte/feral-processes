//! The mood step a base staff program wears on the map and is counted under
//! on the HUD — `views::mood_step`, `Game::mood_step`, `Game::staff_moods`.
//!
//! No test here ticks the game: `update_disgruntled` would read the bare
//! fixtures' zero morale and clear every marker these tests insert.

use super::support::*;
use crate::components::{Disgruntled, Grievance};
use crate::resources::Party;
use crate::*;

/// A base with `n` staff standing in it, each confirmed to be staff — a
/// fixture whose bodies quietly read as party would make every count below
/// vacuous.
fn staff_of(game: &mut Game, n: usize) -> Vec<Entity> {
    stand_in_base(game);
    let bodies: Vec<Entity> = (0..n)
        .map(|i| spawn_tamed_on_map(game, -2 - i as i32, 0))
        .collect();
    for &b in &bodies {
        assert_eq!(game.program_role(b), Some(ProgramRole::Staff));
    }
    bodies
}

fn sour(game: &mut Game, who: Entity, grievance: Grievance) {
    game.world.entity_mut(who).insert(Disgruntled {
        grievance,
        stranded: false,
        told: false,
    });
}

fn fresh_game() -> Game {
    Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn view_of(game: &mut Game, who: Entity) -> EntityView {
    game.view_entities(30, 30)
        .into_iter()
        .find(|v| v.entity == who)
        .expect("the body is in view")
}

#[test]
fn mood_step_boundaries() {
    use MoodStep::*;
    assert_eq!(mood_step(None, 0.0), Ok);
    assert_eq!(mood_step(None, -4.0), Ok);
    assert_eq!(mood_step(None, -4.01), Slipping);
    assert_eq!(mood_step(None, -7.0), Slipping);
    // No marker means not yet acting, however deep the number.
    assert_eq!(mood_step(None, -60.0), Slipping);
    // Inside the hysteresis band a held marker still reads as sulking.
    assert_eq!(mood_step(Some(Grievance::Sulking), -7.0), Sulking);
    assert_eq!(mood_step(Some(Grievance::Sulking), 0.0), Sulking);
    assert_eq!(mood_step(Some(Grievance::DownedTools), 0.0), DownedTools);
    assert_eq!(mood_step(Some(Grievance::LashingOut), -90.0), LashingOut);
}

#[test]
fn staff_moods_counts_every_base_staff_body() {
    let mut game = fresh_game();
    // More than the CREW pane's twelve rows: the counts are not a fold over
    // what the pane has room for.
    let staff = staff_of(&mut game, 14);
    for &b in &staff[0..5] {
        sour(&mut game, b, Grievance::Sulking);
    }
    for &b in &staff[5..9] {
        sour(&mut game, b, Grievance::DownedTools);
    }
    for &b in &staff[9..13] {
        sour(&mut game, b, Grievance::LashingOut);
    }
    assert_eq!(
        game.staff_moods(),
        MoodCounts {
            slipping: 0,
            sulking: 5,
            downed_tools: 4,
            lashing_out: 4,
        }
    );
}

#[test]
fn non_staff_bodies_read_ok() {
    let mut game = fresh_game();
    let staff = staff_of(&mut game, 2);
    let member = staff[1];
    game.world.resource_mut::<Party>().0.push(member);
    assert_ne!(game.program_role(member), Some(ProgramRole::Staff));
    sour(&mut game, member, Grievance::LashingOut);
    assert_eq!(game.mood_step(member), MoodStep::Ok);
    assert_eq!(view_of(&mut game, member).mood, MoodStep::Ok);

    let player = game.player_entity();
    assert_eq!(game.mood_step(player), MoodStep::Ok);
    assert_eq!(view_of(&mut game, player).mood, MoodStep::Ok);
    assert!(game.staff_moods().is_calm());
}

#[test]
fn entity_view_carries_the_staff_step() {
    let mut game = fresh_game();
    let staff = staff_of(&mut game, 1);
    sour(&mut game, staff[0], Grievance::Sulking);
    assert_eq!(game.mood_step(staff[0]), MoodStep::Sulking);
    assert_eq!(view_of(&mut game, staff[0]).mood, MoodStep::Sulking);
}
