//! The battle screen's picture window: who it shows and what it looks like.

use super::support::*;
use crate::*;

fn a_game() -> Game {
    Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn a_hostile(game: &mut Game, hp: i32) -> Entity {
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .expect("at least one species");
    game.world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Glyph {
                ch: species.glyph,
                color: GlyphColor::Red,
            },
            Position { x: 3, y: 3 },
            Stats {
                hp,
                max_hp: hp,
                atk: 20,
                mitigation: 1,
            },
            StatusEffects::default(),
        ))
        .id()
}

#[test]
fn a_hostiles_portrait_carries_its_species_sprite_and_glyph() {
    let mut game = a_game();
    let wild = a_hostile(&mut game, 50);
    let def = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap();

    let portrait = game.portrait_of(wild).expect("a creature has a portrait");

    assert_eq!(portrait.sprite.as_deref(), Some(def.sprite_name()));
    assert_eq!(portrait.glyph, def.glyph);
    assert_eq!(portrait.color, GlyphColor::Red);
    assert!(!portrait.drawn_icon, "only the player has a drawn icon");
}

#[test]
fn a_player_with_a_drawn_icon_asks_for_it() {
    let mut game = a_game();
    let player = game.player_entity();
    game.world.get_mut::<PlayerIdentity>(player).unwrap().icon = Some(PlayerIcon::default());

    assert!(game.portrait_of(player).unwrap().drawn_icon);
}

#[test]
fn a_player_without_a_drawn_icon_does_not() {
    let mut game = a_game();
    let player = game.player_entity();
    game.world.get_mut::<PlayerIdentity>(player).unwrap().icon = None;

    assert!(!game.portrait_of(player).unwrap().drawn_icon);
}

/// The map draws an emulating player as the form, never as the drawn icon;
/// the portrait must agree or the two would show different bodies.
#[test]
fn an_emulating_player_shows_the_form_not_the_drawn_icon() {
    let mut game = a_game();
    let player = game.player_entity();
    game.world.get_mut::<PlayerIdentity>(player).unwrap().icon = Some(PlayerIcon::default());
    game.world.entity_mut(player).insert(Emulation {
        species: "drone".into(),
        rounds_left: 3,
    });
    let drone = game.form_look(player).expect("drone is a real species");

    let portrait = game.portrait_of(player).unwrap();

    assert!(!portrait.drawn_icon);
    assert_eq!(portrait.sprite, drone.sprite);
    assert_eq!(portrait.glyph, drone.glyph);
}

fn a_fight(hp: i32) -> (Game, Entity, Entity) {
    let mut game = a_game();
    let player = game.player_entity();
    let wild = a_hostile(&mut game, hp);
    insert_battle(&mut game, player, vec![wild]);
    (game, player, wild)
}

fn shown_at(game: &Game, revealed: usize) -> Option<String> {
    game.battle_view_at(revealed)
        .and_then(|v| v.portrait)
        .map(|p| p.name)
}

#[test]
fn before_any_round_the_window_shows_the_front_hostile() {
    let (game, _, wild) = a_fight(500);
    let portrait = game.battle_view().unwrap().portrait;
    assert_eq!(portrait.map(|p| p.name), Some(game.creature_label(wild)));
}

#[test]
fn each_swing_line_shows_whoever_swung() {
    let (mut game, player, wild) = a_fight(500);
    player_attacks(&mut game);
    let lines: Vec<_> = game
        .battle_log()
        .into_iter()
        .map(|l| (l.outcome, l.text.clone()))
        .collect();
    let mut saw_player = false;
    for (i, (outcome, text)) in lines.iter().enumerate() {
        if outcome.is_none() {
            continue;
        }
        let expected = if text.starts_with("You ") {
            saw_player = true;
            game.creature_label(player)
        } else {
            game.creature_label(wild)
        };
        assert_eq!(shown_at(&game, i + 1), Some(expected), "line {i}: {text}");
    }
    assert!(
        saw_player,
        "the round should narrate the player's own swing"
    );
}

#[test]
fn the_round_header_shows_the_front_hostile() {
    let (mut game, _, wild) = a_fight(500);
    player_attacks(&mut game);
    assert_eq!(shown_at(&game, 1), Some(game.creature_label(wild)));
}

/// Once the round is over nobody is acting, so the live view goes back to
/// the first in line rather than freezing on whoever swung last.
#[test]
fn after_the_round_the_live_view_returns_to_the_front_hostile() {
    let (mut game, _, wild) = a_fight(500);
    player_attacks(&mut game);
    assert_eq!(
        game.battle_view().unwrap().portrait.map(|p| p.name),
        Some(game.creature_label(wild))
    );
}

/// A stalled turn still belongs to the stalled body: its one line is not a
/// swing, but it is that body's turn being narrated.
#[test]
fn a_stunned_players_stall_line_shows_the_player() {
    let (mut game, player, _) = a_fight(500);
    game.world.get_mut::<StatusEffects>(player).unwrap().active = vec![ActiveStatus {
        id: crate::statuses::StatusId::from("stun"),
        remaining: 1,
        power: 0,
        stacks: 1,
        landed_this_round: false,
    }];
    player_attacks(&mut game);
    let stall = game
        .battle_log()
        .iter()
        .position(|l| l.text.contains("stalls out"))
        .expect("the stun should narrate a lost turn");
    assert_eq!(
        shown_at(&game, stall + 1),
        Some(game.creature_label(player))
    );
}

#[test]
fn a_cloaked_front_member_is_not_the_one_shown() {
    let mut game = a_game();
    let player = game.player_entity();
    let cloaked = a_hostile(&mut game, 500);
    let visible = a_hostile(&mut game, 500);
    // Same species, so the colour is what tells the two portraits apart.
    game.world.get_mut::<Glyph>(visible).unwrap().color = GlyphColor::Cyan;
    let species = game.world.get::<Creature>(cloaked).unwrap().species.clone();
    insert_battle_with_groups(
        &mut game,
        player,
        vec![crate::battle::EnemyGroup {
            species,
            members: vec![cloaked, visible],
        }],
    );
    game.world
        .entity_mut(cloaked)
        .insert(Cloaked { remaining: 3 });

    let portrait = game.battle_view().unwrap().portrait.unwrap();

    assert_eq!(portrait.color, GlyphColor::Cyan);
}

#[test]
fn a_won_fight_leaves_the_window_empty() {
    let (mut game, _, _) = a_fight(1);
    player_attacks(&mut game);
    assert!(game.battle_view().is_none(), "the fight should be over");
    let result = game.battle_result_view().expect("the screen stays up");
    assert_eq!(result.portrait, None);
}

#[test]
fn a_jacked_out_fight_still_shows_what_you_ran_from() {
    let (mut game, _, wild) = a_fight(500);
    let label = game.creature_label(wild);
    let mut fled = false;
    for _ in 0..50 {
        if game.battle_flee() && game.battle_view().is_none() {
            fled = true;
            break;
        }
    }
    assert!(fled, "fleeing should eventually succeed");
    let result = game.battle_result_view().expect("the screen stays up");
    assert_eq!(result.portrait.map(|p| p.name), Some(label));
}

/// A hostile can kill itself on its own turn (a Recoil fumble) and is
/// despawned while its turn is still being narrated; the window falls back
/// to the first in line rather than showing a nameless `?`.
#[test]
fn a_despawned_actor_falls_back_to_the_front_hostile() {
    let (mut game, _, wild) = a_fight(500);
    let gone = a_hostile(&mut game, 5);
    game.world.despawn(gone);
    game.world.resource_mut::<BattleTimeline>().acting = Some(gone);

    assert_eq!(game.portrait_of(gone), None);
    assert_eq!(
        game.battle_view().unwrap().portrait.map(|p| p.name),
        Some(game.creature_label(wild))
    );
}
