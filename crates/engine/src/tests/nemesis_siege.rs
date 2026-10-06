//! A nemesis's band: gathering, following, the wild-population exemptions
//! and the save round trip. The march and the siege itself are tested
//! beside the code that opens them.

use super::support::*;
use crate::components::{Besieger, NemesisFollower, NemesisHome, NemesisMuster};
use crate::tuning::{
    NEMESIS_BAND_MAX, NEMESIS_FOLLOW_DISTANCE, NEMESIS_RECRUIT_INTERVAL, NEMESIS_RECRUIT_RADIUS,
    WILD_CREATURE_CAP,
};
use crate::*;

fn chebyshev(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

/// The left end of a stretch of open ground far from the player, where no
/// seeded wild population stands and a habitat species can spawn. A test
/// that needs room to move picks its cells from here rather than guessing
/// at a map it does not own.
fn open_ground(game: &mut Game) -> (i32, i32) {
    for x in (300..400).step_by(40) {
        for y in (300..700).step_by(7) {
            let clear = (0..=40).all(|dx| {
                game.world
                    .resource_mut::<crate::world::WorldMap>()
                    .tile(x + dx, y)
                    .open_to_hostiles()
            });
            if clear && game.habitat_pools(x, y, None, 0).is_some() {
                return (x, y);
            }
        }
    }
    panic!("no open ground found for the band tests");
}

fn nemesis_at(game: &mut Game, x: i32, y: i32) -> Entity {
    let leader = game.spawn_wild_creature("construct", x, y).unwrap();
    game.world.entity_mut(leader).insert(Nemesis(1));
    leader
}

fn wild_at(game: &mut Game, x: i32, y: i32) -> Entity {
    game.spawn_wild_creature("construct", x, y).unwrap()
}

fn muster_for(game: &mut Game, ticks: u32) {
    for _ in 0..ticks {
        game.nemesis_muster();
    }
}

fn followers_of(game: &mut Game, leader: Entity) -> Vec<Entity> {
    game.world
        .query::<(Entity, &NemesisFollower)>()
        .iter(&game.world)
        .filter(|(_, f)| f.0 == leader)
        .map(|(e, _)| e)
        .collect()
}

fn new_game() -> Game {
    Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

#[test]
fn a_nemesis_recruits_the_nearest_eligible_wild_body_after_the_interval() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let leader = nemesis_at(&mut game, x, y);
    let far = wild_at(&mut game, x + 8, y);
    let near = wild_at(&mut game, x + 3, y);

    muster_for(&mut game, NEMESIS_RECRUIT_INTERVAL - 1);
    assert!(followers_of(&mut game, leader).is_empty(), "too early");

    muster_for(&mut game, 1);
    assert_eq!(followers_of(&mut game, leader), vec![near]);
    assert!(game.world.get::<NemesisFollower>(far).is_none());
    assert_eq!(
        game.world.get::<NemesisMuster>(leader).unwrap().ticks,
        0,
        "a recruit resets the muster"
    );
}

#[test]
fn a_nemesis_refuses_every_excluded_kind_and_spawns_a_body_instead() {
    type Tag = fn(&mut Game, Entity, Entity);
    let tags: [(&str, Tag); 5] = [
        ("another nemesis", |g, e, _| {
            g.world.entity_mut(e).insert(Nemesis(1));
        }),
        ("another's follower", |g, e, other| {
            g.world.entity_mut(e).insert(NemesisFollower(other));
        }),
        ("a nest guardian", |g, e, other| {
            g.world
                .entity_mut(e)
                .insert(crate::components::NestGuardian { nest: other });
        }),
        ("a boss", |g, e, _| {
            g.world.entity_mut(e).insert(Boss);
        }),
        ("a besieger", |g, e, _| {
            g.world.entity_mut(e).insert(Besieger);
        }),
    ];
    for (name, tag) in tags {
        let mut game = new_game();
        let (x, y) = open_ground(&mut game);
        let leader = nemesis_at(&mut game, x, y);
        let other = nemesis_at(&mut game, x, y + 100);
        let excluded = wild_at(&mut game, x + 2, y);
        tag(&mut game, excluded, other);

        muster_for(&mut game, NEMESIS_RECRUIT_INTERVAL);

        let band = followers_of(&mut game, leader);
        assert_eq!(band.len(), 1, "{name}: a body is still gathered");
        assert_ne!(band[0], excluded, "{name} must not be recruited");
        let at = *game.world.get::<Position>(band[0]).unwrap();
        assert_eq!((at.x, at.y), (x, y), "{name}: the recruit is a fresh spawn");
    }
}

#[test]
fn a_nemesis_with_nobody_in_range_spawns_a_body_at_its_own_cell() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let leader = nemesis_at(&mut game, x, y);
    // In the world, but past the radius.
    let _beyond = wild_at(&mut game, x + NEMESIS_RECRUIT_RADIUS + 3, y);

    muster_for(&mut game, NEMESIS_RECRUIT_INTERVAL);

    let band = followers_of(&mut game, leader);
    assert_eq!(band.len(), 1);
    let pos = *game.world.get::<Position>(band[0]).unwrap();
    assert_eq!((pos.x, pos.y), (x, y));
    assert!(game.world.get::<Hostile>(band[0]).is_some());
}

#[test]
fn a_band_stops_at_its_maximum() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let leader = nemesis_at(&mut game, x, y);

    muster_for(
        &mut game,
        NEMESIS_RECRUIT_INTERVAL * (NEMESIS_BAND_MAX as u32 + 2),
    );

    assert_eq!(followers_of(&mut game, leader).len(), NEMESIS_BAND_MAX);
    let alerts = game
        .world
        .resource::<crate::alerts::AlertBoard>()
        .alerts
        .iter()
        .filter(|a| a.subject.starts_with("nemesis-band"))
        .count();
    assert_eq!(alerts, 1, "the band is announced once");
}

#[test]
fn a_nemesis_in_a_fight_or_pursuing_does_not_muster() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let leader = nemesis_at(&mut game, x, y);
    game.world.entity_mut(leader).insert(Pursuing);
    muster_for(&mut game, NEMESIS_RECRUIT_INTERVAL * 2);
    assert!(followers_of(&mut game, leader).is_empty());
}

#[test]
fn a_follower_closes_in_on_a_leader_that_has_moved_away() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let leader = nemesis_at(&mut game, x, y);
    let follower = wild_at(&mut game, x + 1, y);
    game.world
        .entity_mut(follower)
        .insert(NemesisFollower(leader));
    game.world.get_mut::<Position>(leader).unwrap().x = x + 20;

    muster_for(&mut game, 30);

    let l = *game.world.get::<Position>(leader).unwrap();
    let f = *game.world.get::<Position>(follower).unwrap();
    assert!(chebyshev(l, f) <= NEMESIS_FOLLOW_DISTANCE, "{l:?} {f:?}");
}

#[test]
fn a_follower_whose_leader_is_gone_or_no_longer_a_nemesis_is_released() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let gone = nemesis_at(&mut game, x, y);
    let demoted = nemesis_at(&mut game, x, y + 10);
    let a = wild_at(&mut game, x, y);
    let b = wild_at(&mut game, x, y + 10);
    game.world.entity_mut(a).insert(NemesisFollower(gone));
    game.world.entity_mut(b).insert(NemesisFollower(demoted));
    game.world.despawn(gone);
    game.world.entity_mut(demoted).remove::<Nemesis>();

    game.nemesis_muster();

    assert!(game.world.get::<NemesisFollower>(a).is_none());
    assert!(game.world.get::<NemesisFollower>(b).is_none());
}

#[test]
fn cull_to_cap_spares_a_nemesis_and_its_followers() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let leader = nemesis_at(&mut game, x, y);
    let follower = wild_at(&mut game, x + 1, y);
    game.world
        .entity_mut(follower)
        .insert(NemesisFollower(leader));
    let stock = wild_at(&mut game, x + 2, y);

    // This much demand evicts everything evictable.
    game.cull_to_cap(WILD_CREATURE_CAP * 100);
    assert!(
        game.world.get_entity(stock).is_err(),
        "stock far from the player is evicted"
    );
    assert!(
        game.world.get_entity(leader).is_ok(),
        "the nemesis is exempt"
    );
    assert!(
        game.world.get_entity(follower).is_ok(),
        "its follower is exempt"
    );
}

#[test]
fn a_jack_out_marks_a_follower_as_its_own_leader() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let leader = nemesis_at(&mut game, x, y);
    let wild = start_battle_with_a_wild_program(&mut game);
    game.world.entity_mut(wild).insert(NemesisFollower(leader));

    let mut landed = false;
    for _ in 0..200 {
        if game.battle_flee() {
            landed = true;
            break;
        }
        if !game.has_active_battle() {
            break;
        }
    }
    assert!(landed, "the escape roll never landed");

    assert!(game.world.get::<Nemesis>(wild).is_some());
    assert!(
        game.world.get::<NemesisFollower>(wild).is_none(),
        "a newly marked follower leads itself"
    );
}

#[test]
fn a_band_survives_a_real_save_and_load_with_followers_on_the_right_leader() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let a = nemesis_at(&mut game, x, y);
    let b = nemesis_at(&mut game, x + 20, y);
    for (leader, dx, n) in [(a, 0, 3), (b, 20, 1)] {
        for i in 0..n {
            let f = wild_at(&mut game, x + dx + i, y + 1);
            game.world.entity_mut(f).insert(NemesisFollower(leader));
        }
    }
    game.world
        .entity_mut(a)
        .insert(NemesisMuster { ticks: 123 });
    game.world
        .entity_mut(a)
        .insert(NemesisHome(Position { x: x - 5, y: y + 9 }));
    nemesis_at(&mut game, x, y + 50);

    let path = std::env::temp_dir().join(format!("feral_nemesis_band_{}.sav", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let find = |g: &mut Game, px: i32, py: i32| {
        g.world
            .query_filtered::<(Entity, &Position), With<Nemesis>>()
            .iter(&g.world)
            .find(|(_, p)| p.x == px && p.y == py)
            .map(|(e, _)| e)
            .unwrap()
    };
    let la = find(&mut loaded, x, y);
    let lb = find(&mut loaded, x + 20, y);
    let llone = find(&mut loaded, x, y + 50);
    assert_eq!(followers_of(&mut loaded, la).len(), 3);
    assert_eq!(followers_of(&mut loaded, lb).len(), 1);
    assert!(followers_of(&mut loaded, llone).is_empty());
    assert_eq!(loaded.world.get::<NemesisMuster>(la).unwrap().ticks, 123);
    let home = loaded.world.get::<NemesisHome>(la).unwrap().0;
    assert_eq!((home.x, home.y), (x - 5, y + 9));
    assert!(loaded.world.get::<NemesisHome>(lb).is_none());
    for f in followers_of(&mut loaded, la) {
        assert!(loaded.world.get::<Hostile>(f).is_some());
        assert!(loaded.world.get::<Nemesis>(f).is_none());
    }
}

#[test]
fn a_save_without_band_fields_loads_a_nemesis_with_no_band() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let _ = nemesis_at(&mut game, x, y);
    let path =
        std::env::temp_dir().join(format!("feral_nemesis_band_old_{}.sav", std::process::id()));
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("nemesis_muster_ticks"));
    let stripped: String = text
        .lines()
        .filter(|l| {
            let l = l.trim_start();
            !l.starts_with("nemesis_muster_ticks")
                && !l.starts_with("nemesis_home")
                && !l.starts_with("nemesis_band")
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, stripped).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        loaded
            .world
            .query::<&NemesisFollower>()
            .iter(&loaded.world)
            .count(),
        0
    );
}

#[test]
fn a_breach_sweep_keeps_a_nemesis_and_its_followers() {
    let mut game = new_game();
    let player = *game.world.get::<Position>(game.player_entity()).unwrap();
    let leader = nemesis_at(&mut game, player.x + 1, player.y);
    let follower = wild_at(&mut game, player.x + 2, player.y);
    game.world
        .entity_mut(follower)
        .insert(NemesisFollower(leader));
    let stock = wild_at(&mut game, player.x + 3, player.y);

    game.clear_local_wild();

    assert!(game.world.get_entity(stock).is_err());
    assert!(game.world.get_entity(leader).is_ok());
    assert!(game.world.get_entity(follower).is_ok());
}
