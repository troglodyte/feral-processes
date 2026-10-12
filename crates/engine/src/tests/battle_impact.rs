//! What each narrated battle line says it did, as an `Impact`: the data a
//! renderer draws floats, flashes and a portrait reaction from.

use super::support::*;
use crate::resources::SwingOutcome;
use crate::*;

fn lines(game: &Game) -> Vec<LogLine> {
    game.world.resource::<MessageLog>().lines.clone()
}

fn swing_line(game: &Game, kind: MessageKind) -> LogLine {
    lines(game)
        .into_iter()
        .find(|l| l.kind == kind && l.outcome.is_some())
        .expect("the swing logged a line")
}

/// One party of one against one wild program with HP to spare, so a round
/// runs to completion either way. Returns the wild's group id.
fn battle(seed: u32, wild_hp: i32) -> (Game, Entity, Entity, crate::battle::GroupId) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.world.get_mut::<Stats>(wild).unwrap().hp = wild_hp;
    game.world.get_mut::<Stats>(wild).unwrap().max_hp = 500;
    insert_battle(&mut game, player, vec![wild]);
    let id = game.world.resource::<BattleState>().groups[0].id;
    (game, player, wild, id)
}

fn hp(game: &Game, e: Entity) -> i32 {
    game.world.get::<Stats>(e).unwrap().hp
}

#[test]
fn a_party_hit_lands_damage_on_the_enemy_group() {
    let (mut game, wild, id) = {
        let (g, _, w, id) = battle(52, 500);
        (g, w, id)
    };
    force_the_next_attack_to_land(&mut game);
    let before = hp(&game, wild);
    player_swings_at_group(&mut game, 0);
    let dealt = before - hp(&game, wild);
    assert!(dealt > 0);
    assert_eq!(
        swing_line(&game, MessageKind::PartyDamage).impact,
        Some(Impact {
            target: ImpactTarget::Group(id),
            kind: ImpactKind::Damage(dealt)
        })
    );
}

#[test]
fn a_party_crit_lands_a_crit_on_the_enemy_group() {
    let (mut game, player, wild, id) = battle(52, 500);
    force_the_next_attack_to_crit(&mut game, player);
    let before = hp(&game, wild);
    player_swings_at_group(&mut game, 0);
    assert_eq!(
        swing_line(&game, MessageKind::PartyDamage).impact,
        Some(Impact {
            target: ImpactTarget::Group(id),
            kind: ImpactKind::Crit(before - hp(&game, wild))
        })
    );
}

#[test]
fn a_party_miss_is_a_miss_mark_on_the_enemy_group() {
    let (mut game, player, wild, id) = battle(52, 500);
    let range = game.attack_range(player, crate::tuning::PLAYER_UNARMED_DAMAGE);
    force_the_next_attack_to_miss_plainly(&mut game, player, wild, battle::Swing::plain(range));
    player_swings_at_group(&mut game, 0);
    let line = swing_line(&game, MessageKind::PartyDamage);
    assert_eq!(line.outcome, Some(SwingOutcome::Miss));
    assert_eq!(
        line.impact,
        Some(Impact {
            target: ImpactTarget::Group(id),
            kind: ImpactKind::Miss
        })
    );
}

#[test]
fn a_party_fumble_is_a_fumble_mark_on_the_enemy_group() {
    let (mut game, player, _, id) = battle(52, 500);
    force_the_next_attack_to_miss(&mut game, player);
    player_swings_at_group(&mut game, 0);
    let line = swing_line(&game, MessageKind::PartyDamage);
    assert_eq!(line.outcome, Some(SwingOutcome::Fumble));
    assert_eq!(
        line.impact,
        Some(Impact {
            target: ImpactTarget::Group(id),
            kind: ImpactKind::Fumble
        })
    );
}

#[test]
fn an_enemy_hit_lands_damage_on_the_party_slot() {
    let game = first_rng_seed_where(
        |seed| {
            let (mut game, player, wild, _) = battle(53, 500);
            reseed_rng(&mut game, seed);
            game.wild_retaliate(wild, 0, player);
            game
        },
        |game| {
            lines(game).iter().any(|l| {
                matches!(l.kind, MessageKind::EnemyAttack | MessageKind::EnemySpecial)
                    && l.outcome == Some(SwingOutcome::Hit)
            })
        },
    );
    let line = lines(&game)
        .into_iter()
        .find(|l| l.outcome == Some(SwingOutcome::Hit))
        .unwrap();
    let Some(Impact {
        target: ImpactTarget::Party(0),
        kind: ImpactKind::Damage(n),
    }) = line.impact
    else {
        panic!("an enemy hit on the player should be Damage on Party(0): {line:?}");
    };
    assert!(n > 0 && line.text.contains(&n.to_string()), "{line:?}");
}

#[test]
fn a_heal_restores_on_the_party_slot_it_landed_on() {
    let (mut game, player, _, _) = battle(54, 500);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 100;
        stats.hp = 90;
    }
    let patch = ability(&game, "hot_patch");
    game.use_ability(&patch, player, "You", &[player]);
    let gained = hp(&game, player) - 90;
    assert!(gained > 0);
    let heal = lines(&game)
        .into_iter()
        .find(|l| l.text.contains("patches"))
        .expect("the heal logged a line");
    assert_eq!(
        heal.impact,
        Some(Impact {
            target: ImpactTarget::Party(0),
            kind: ImpactKind::Heal(gained)
        })
    );
}

#[test]
fn a_killing_swing_and_its_kill_line_are_two_lines_with_two_impacts() {
    let (mut game, player, _, id) = battle(52, 1);
    force_the_next_attack_to_crit(&mut game, player);
    player_swings_at_group(&mut game, 0);
    let all = lines(&game);
    let swing = all
        .iter()
        .position(|l| l.outcome.is_some())
        .expect("the swing line");
    let target = ImpactTarget::Group(id);
    assert!(matches!(
        all[swing].impact,
        Some(Impact { target: t, kind: ImpactKind::Crit(_) }) if t == target
    ));
    assert_eq!(
        all[swing + 1].impact,
        Some(Impact {
            target,
            kind: ImpactKind::Kill
        }),
        "the kill line follows the swing line: {all:?}"
    );
}

#[test]
fn a_round_header_has_no_impact() {
    let (mut game, ..) = battle(52, 500);
    player_attacks(&mut game);
    let headers: Vec<LogLine> = lines(&game)
        .into_iter()
        .filter(|l| l.kind == MessageKind::Round)
        .collect();
    assert!(!headers.is_empty());
    assert!(headers.iter().all(|l| l.impact.is_none()), "{headers:?}");
}

/// Impacts ride existing lines; none is ever added to carry one. The count
/// is pinned from the build before impacts existed.
#[test]
fn a_seeded_round_logs_exactly_the_lines_it_always_did() {
    let (mut game, ..) = battle(52, 500);
    player_attacks(&mut game);
    assert_eq!(lines(&game).len(), 6);
}
