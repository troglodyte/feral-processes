//! Rewinding the battle roster to what a given log line described.
//!
//! `battle_resolve_round` resolves the whole round at once while a frontend
//! scrolls the narration in over a second or two, so the roster has to be
//! able to answer "what did this look like when line N landed" rather than
//! only "what does it look like now".

use super::support::*;
use crate::*;

/// A fight the player cannot finish in one round: one enemy with far too
/// much HP to kill and enough ATK to hit back, so the round narrates a
/// strike *and* a retaliation and the battle is still live afterwards.
fn a_fight_that_survives_a_round() -> (Game, Entity) {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 3, y: 3 },
            Stats {
                hp: 500,
                max_hp: 500,
                atk: 20,
                mitigation: 1,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    (game, wild)
}

#[test]
fn the_opening_line_of_a_round_shows_the_roster_untouched() {
    let (mut game, _) = a_fight_that_survives_a_round();
    let player = game.player_entity();
    let player_hp_before = game.world.get::<Stats>(player).unwrap().hp;

    player_attacks(&mut game);

    // Line 0 is the "── round N ──" header, so one revealed line is the
    // round announced and nothing yet resolved.
    let view = game.battle_view_at(1).expect("the battle is still live");
    assert_eq!(
        view.groups[0].front_hp, 500,
        "the enemy should still be untouched while only the round header is on screen"
    );
    assert_eq!(
        view.party[0].hp, player_hp_before,
        "the player should still be untouched too"
    );
}

/// A frontend reports zero revealed lines for the whole gap between a
/// round resolving and the next frame it draws (see `App::revealed_count`).
/// Falling back to live rows there would flash the finished round on screen
/// for one frame before the narration started scrolling.
#[test]
fn no_revealed_lines_shows_the_roster_as_the_round_opened() {
    let (mut game, _) = a_fight_that_survives_a_round();

    player_attacks(&mut game);

    let view = game.battle_view_at(0).expect("the battle is still live");
    assert_eq!(view.groups[0].front_hp, 500);
}

#[test]
fn the_last_line_of_a_round_shows_the_live_roster() {
    let (mut game, _) = a_fight_that_survives_a_round();

    player_attacks(&mut game);

    let lines = game.battle_log().len();
    let live = game.battle_view().expect("the battle is still live");
    let rewound = game
        .battle_view_at(lines)
        .expect("the battle is still live");
    assert_eq!(rewound.groups[0].front_hp, live.groups[0].front_hp);
    assert_eq!(rewound.party[0].hp, live.party[0].hp);
    assert!(
        live.groups[0].front_hp < 500,
        "the fixture landed no damage, so this asserts nothing"
    );
}

#[test]
fn a_rewound_roster_never_heals_as_the_narration_advances() {
    let (mut game, _) = a_fight_that_survives_a_round();

    player_attacks(&mut game);

    let lines = game.battle_log().len();
    let mut previous = 500;
    for revealed in 1..=lines {
        let view = game
            .battle_view_at(revealed)
            .expect("the battle is still live");
        let hp = view.groups[0].front_hp;
        assert!(
            hp <= previous,
            "enemy HP rose from {previous} to {hp} at line {revealed}"
        );
        previous = hp;
    }
}

#[test]
fn a_group_keeps_its_dead_front_member_until_the_line_announcing_it() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // One HP apiece, so a landed strike kills the front member and promotes
    // the one behind it. Forced, because the strike can miss now and a
    // fixture that killed nobody asserts nothing.
    battle_with_a_pack_of(&mut game, 3, 1);

    force_the_next_attack_to_land(&mut game);
    player_attacks(&mut game);
    assert!(
        game.has_active_battle(),
        "a three-member pack should survive one kill"
    );

    let opening = game.battle_view_at(1).expect("the battle is still live");
    assert_eq!(
        opening.groups[0].count, 3,
        "the pack should still read as three while only the round header is on screen"
    );
    let live = game.battle_view().expect("the battle is still live");
    assert_eq!(
        live.groups[0].count, 2,
        "the fixture killed nobody, so this asserts nothing"
    );
}

/// `end_battle` removes `BattleState`, so `battle_view` goes `None` the
/// instant a fight ends — but the battle screen stays up while its results
/// scroll in, and still has to say what the fight cost. That roster is
/// captured on the way out.
#[test]
fn the_roster_outlives_the_fight_so_the_screen_can_stay_up() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = spawn_wild_on_player_tile(&mut game);
    game.world.get_mut::<Stats>(wild).unwrap().hp = 1;
    insert_battle(&mut game, player, vec![wild]);

    player_attacks(&mut game);

    assert!(
        !game.has_active_battle(),
        "the fixture did not end the fight"
    );
    assert!(
        game.battle_view().is_none(),
        "the live view should be gone with the battle"
    );
    let closing = game.battle_result_view().expect("a fight has been fought");
    assert_eq!(
        closing.party.len(),
        1,
        "the player should be on the closing roster"
    );
    assert_eq!(closing.party[0].name, "You");
    assert!(
        closing.groups.is_empty(),
        "a win empties the last group before `end_battle` runs, and the \
         hostile pane clearing is what winning looks like"
    );
    assert!(
        closing.active_slot.is_none() && closing.options.is_empty(),
        "nothing is choosing anything once the fight is over"
    );
}

/// Captured at the *top* of `end_battle`, before `dissolve_tamed_program`
/// drops the dead from `Party` and despawns them. A companion that died
/// winning the fight is exactly what the player needs the page to tell them,
/// and it is unrecoverable one line later.
#[test]
fn a_companion_that_died_is_still_on_the_closing_roster() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.world.resource_mut::<Party>().0.push(companion);
    let wild = spawn_wild_on_player_tile(&mut game);
    game.world.get_mut::<Stats>(wild).unwrap().hp = 1;
    insert_battle(&mut game, player, vec![wild]);
    game.world.get_mut::<Stats>(companion).unwrap().hp = 0;

    player_attacks(&mut game);

    assert!(
        !game.has_active_battle(),
        "the fixture did not end the fight"
    );
    let closing = game.battle_result_view().expect("a fight has been fought");
    assert_eq!(
        closing.party.len(),
        2,
        "the fallen companion was dropped before the roster was captured"
    );
    assert_eq!(closing.party[1].hp, 0);
    assert!(
        game.world.get::<Stats>(companion).is_none(),
        "the companion should still have been dissolved — this asserts the \
         capture is a copy, not a live read"
    );
}

#[test]
fn a_fresh_round_forgets_the_previous_rounds_frames() {
    let (mut game, _) = a_fight_that_survives_a_round();

    player_attacks(&mut game);
    let after_one = game
        .battle_view_at(1)
        .expect("the battle is still live")
        .groups[0]
        .front_hp;
    player_attacks(&mut game);
    let after_two = game
        .battle_view_at(1)
        .expect("the battle is still live")
        .groups[0]
        .front_hp;

    assert!(
        after_two < after_one,
        "round two's opening line should show the damage round one already did, \
         not replay round one's frames"
    );
}
