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

// ---- March, siege, aftermath ----

use crate::tactical::TacticalBattle;
use crate::tuning::{
    BASE_ESTABLISHED_STAFF, BASE_ESTABLISHED_STRUCTURES, NEMESIS_MARCH_DELAY, SIEGE_MIN_ZONE,
};

/// A full band whose march countdown has run out, on open ground.
fn marching_band(game: &mut Game) -> (Entity, Vec<Entity>) {
    let (x, y) = open_ground(game);
    let leader = nemesis_at(game, x, y);
    let mut band = Vec::new();
    for i in 0..NEMESIS_BAND_MAX as i32 {
        let f = wild_at(game, x + i, y + 1);
        game.world.entity_mut(f).insert(NemesisFollower(leader));
        band.push(f);
    }
    game.world.entity_mut(leader).insert(NemesisMuster {
        ticks: NEMESIS_MARCH_DELAY,
    });
    (leader, band)
}

/// `siege.rs::establish_base`: enough staff and one raidable structure that
/// `base_is_established` and `nothing_to_besiege` both pass, on a floored
/// pocket so a siege board can be built.
fn established_base(game: &mut Game) {
    set_zone(game, SIEGE_MIN_ZONE.max(2));
    game.lay_starting_pocket();
    for _ in 0..BASE_ESTABLISHED_STAFF {
        spawn_tamed(game, 10, 3);
    }
    for i in 0..BASE_ESTABLISHED_STRUCTURES {
        let mut e = game.world.spawn((
            Structure {
                kind: "test_structure".to_string(),
            },
            Position {
                x: 30 + i as i32,
                y: 30,
            },
        ));
        if i == 0 {
            e.insert(Durability { hp: 30, max_hp: 30 });
        }
    }
}

fn has_siege_tags(game: &Game, e: Entity) -> bool {
    game.world.get::<Besieger>(e).is_some()
}

#[test]
fn a_full_band_marches_once_its_delay_has_run() {
    let mut game = new_game();
    established_base(&mut game);
    let (leader, band) = marching_band(&mut game);
    game.world.get_mut::<NemesisMuster>(leader).unwrap().ticks = NEMESIS_MARCH_DELAY - 1;
    game.nemesis_march_check();
    assert!(!has_siege_tags(&game, leader), "one tick early");

    game.world.get_mut::<NemesisMuster>(leader).unwrap().ticks = NEMESIS_MARCH_DELAY;
    let before = *game.world.get::<Position>(leader).unwrap();
    game.nemesis_march_check();
    // Away from the base, the march resolves at once and the aftermath has
    // already run: the leader is home alone, the followers are gone.
    for f in band {
        assert!(game.world.get_entity(f).is_err(), "followers are spent");
    }
    let after = *game.world.get::<Position>(leader).unwrap();
    assert_eq!((after.x, after.y), (before.x, before.y));
    assert_eq!(game.world.get::<Nemesis>(leader).unwrap().0, 2, "grudge +1");
    assert_eq!(game.world.get::<NemesisMuster>(leader).unwrap().ticks, 0);
    assert!(game.world.get::<NemesisHome>(leader).is_none());
    assert!(!has_siege_tags(&game, leader));
}

#[test]
fn an_away_march_resolves_the_siege_off_screen() {
    let mut game = new_game();
    established_base(&mut game);
    let (_leader, _band) = marching_band(&mut game);
    game.nemesis_march_check();
    let alerts: Vec<String> = game
        .world
        .resource::<crate::alerts::AlertBoard>()
        .alerts
        .iter()
        .map(|a| a.text.clone())
        .collect();
    assert!(
        alerts.iter().any(|t| t.contains("while you're away")),
        "{alerts:?}"
    );
}

#[test]
fn a_march_holds_under_every_siege_hold() {
    type Setup = fn(&mut Game);
    let cases: [(&str, Setup); 5] = [
        ("no base", |g| {
            // Remove the staff: not established.
            let staff: Vec<Entity> = g
                .world
                .query_filtered::<Entity, With<Tamed>>()
                .iter(&g.world)
                .collect();
            for s in staff {
                g.world.despawn(s);
            }
        }),
        ("nothing to besiege", |g| {
            let d: Vec<Entity> = g
                .world
                .query_filtered::<Entity, With<Durability>>()
                .iter(&g.world)
                .collect();
            for e in d {
                g.world.entity_mut(e).remove::<Durability>();
            }
        }),
        ("a fight running", |g| {
            let player = g.player_entity();
            let w = g.spawn_wild_creature("construct", 3, 3).unwrap();
            insert_battle(g, player, vec![w]);
        }),
        ("a siege already running", |g| {
            let w = g.spawn_wild_creature("construct", 3, 3).unwrap();
            g.world.entity_mut(w).insert(Besieger);
        }),
        ("sieges off", |g| g.dev_set_sieges(false)),
    ];
    for (name, setup) in cases {
        let mut game = new_game();
        established_base(&mut game);
        let (leader, band) = marching_band(&mut game);
        setup(&mut game);
        game.nemesis_march_check();
        assert!(!has_siege_tags(&game, leader), "{name}: the leader holds");
        for f in band {
            assert!(game.world.get_entity(f).is_ok(), "{name}: band intact");
        }
        assert!(game.world.get::<NemesisHome>(leader).is_none(), "{name}");
    }
}

#[test]
fn a_band_that_is_short_does_not_march_and_a_refill_waits_its_delay() {
    let mut game = new_game();
    established_base(&mut game);
    let (leader, band) = marching_band(&mut game);
    game.world.despawn(band[0]);
    game.nemesis_march_check();
    assert!(!has_siege_tags(&game, leader), "short band holds");

    // The refill resets the countdown, so the new full band waits again.
    game.nemesis_muster();
    assert_eq!(followers_of(&mut game, leader).len(), NEMESIS_BAND_MAX);
    game.nemesis_march_check();
    assert!(game.world.get_entity(band[1]).is_ok(), "waits its delay");
    assert!(!has_siege_tags(&game, leader));
}

#[test]
fn the_marching_nemesis_is_chosen_by_what_it_is_not_by_entity_order() {
    let mut game = new_game();
    established_base(&mut game);
    let (x, y) = open_ground(&mut game);
    // Spawned in the opposite order to their cells: the higher entity sits
    // at the lower cell, which the save-stable key puts first.
    let mk = |game: &mut Game, x: i32, y: i32| {
        let l = nemesis_at(game, x, y);
        for i in 0..NEMESIS_BAND_MAX as i32 {
            let f = wild_at(game, x + i, y + 1);
            game.world.entity_mut(f).insert(NemesisFollower(l));
        }
        game.world.entity_mut(l).insert(NemesisMuster {
            ticks: NEMESIS_MARCH_DELAY,
        });
        l
    };
    let later_cell = mk(&mut game, x + 20, y);
    let earlier_cell = mk(&mut game, x, y);
    game.world.insert_resource(Locale::Base { x: 0, y: 0 });
    game.nemesis_march_check();
    assert!(has_siege_tags(&game, earlier_cell), "lower cell goes first");
    assert!(!has_siege_tags(&game, later_cell));
}

#[test]
fn a_march_at_home_seats_the_real_band_and_logs_the_taunt() {
    let mut game = new_game();
    established_base(&mut game);
    stand_in_base_at(&mut game, 0, 0);
    let (leader, band) = marching_band(&mut game);
    let home = *game.world.get::<Position>(leader).unwrap();

    game.nemesis_march_check();

    assert!(game.in_tactical_battle());
    let battle = game.world.resource::<TacticalBattle>();
    assert_eq!(
        battle.siege_pack as usize,
        1 + NEMESIS_BAND_MAX,
        "the band and its leader"
    );
    for e in std::iter::once(leader).chain(band) {
        assert!(battle.cell_of(e).is_some(), "seated");
        assert!(game.world.get::<Besieger>(e).is_some());
    }
    let h = game.world.get::<NemesisHome>(leader).unwrap().0;
    assert_eq!((h.x, h.y), (home.x, home.y));
    let label = game.creature_label(leader);
    let logged = game
        .world
        .resource::<crate::resources::MessageLog>()
        .recent(50)
        .iter()
        .any(|m| m.text.starts_with(&label));
    assert!(logged, "the taunt is logged");
}

/// A siege at home, with the player taken off the board so it can be ended.
fn home_siege(game: &mut Game) -> (Entity, Vec<Entity>) {
    established_base(game);
    stand_in_base_at(game, 0, 0);
    let (leader, band) = marching_band(game);
    game.nemesis_march_check();
    assert!(game.in_tactical_battle(), "the siege opened");
    (leader, band)
}

#[test]
fn a_leader_that_outlives_the_siege_goes_home_alone_with_a_grudge() {
    let mut game = new_game();
    let (leader, band) = home_siege(&mut game);
    let home = game.world.get::<NemesisHome>(leader).unwrap().0;
    let grudge = game.world.get::<Nemesis>(leader).unwrap().0;
    // A jack-out: the player leaves the board mid-siege.
    let player = game.player_entity();
    game.world.resource_mut::<TacticalBattle>().remove(player);
    game.end_tactical_battle(None);

    for f in band {
        assert!(game.world.get_entity(f).is_err(), "followers despawn");
    }
    assert!(game.world.get_entity(leader).is_ok(), "the leader survives");
    let pos = *game.world.get::<Position>(leader).unwrap();
    assert_eq!((pos.x, pos.y), (home.x, home.y));
    assert_eq!(game.world.get::<Nemesis>(leader).unwrap().0, grudge + 1);
    // `end_tactical_battle` owes the round's tick, so the reset muster has
    // already counted once.
    assert!(game.world.get::<NemesisMuster>(leader).unwrap().ticks <= 1);
    assert!(game.world.get::<NemesisHome>(leader).is_none());
    assert!(!has_siege_tags(&game, leader));
}

#[test]
fn a_downed_leader_is_gone_and_troubles_the_base_no_more() {
    let mut game = new_game();
    let (leader, _band) = home_siege(&mut game);
    let player = game.player_entity();
    game.finish_hostile_with_overkill(leader, player, 0.0);
    assert!(game.world.get_entity(leader).is_err());
    let said = game
        .world
        .resource::<crate::alerts::AlertBoard>()
        .alerts
        .iter()
        .any(|a| a.text.contains("will trouble you no more"));
    assert!(said);
}

#[test]
fn a_leader_that_withdraws_through_the_door_is_kept() {
    let mut game = new_game();
    let (leader, _band) = home_siege(&mut game);
    game.besieger_leaves(leader, false);
    assert!(game.world.get_entity(leader).is_ok());
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .cell_of(leader)
            .is_none(),
        "off the board"
    );
}

#[test]
fn a_siege_in_progress_keeps_the_leaders_home_across_a_real_save() {
    let mut game = new_game();
    let (leader, _band) = home_siege(&mut game);
    let home = game.world.get::<NemesisHome>(leader).unwrap().0;

    let path = std::env::temp_dir().join(format!("feral_nemesis_siege_{}.sav", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(loaded.in_tactical_battle(), "the siege resumes");
    let l = loaded
        .world
        .query_filtered::<Entity, With<NemesisHome>>()
        .iter(&loaded.world)
        .next()
        .expect("the leader kept its home");
    let h = loaded.world.get::<NemesisHome>(l).unwrap().0;
    assert_eq!((h.x, h.y), (home.x, home.y));
    assert!(loaded.world.get::<Besieger>(l).is_some());
    assert_eq!(followers_of(&mut loaded, l).len(), NEMESIS_BAND_MAX);

    // And the resumed siege still ends the same way.
    let player = loaded.player_entity();
    loaded.world.resource_mut::<TacticalBattle>().remove(player);
    loaded.end_tactical_battle(None);
    assert!(loaded.world.get_entity(l).is_ok());
    assert!(loaded.world.get::<NemesisHome>(l).is_none());
}

#[test]
fn a_map_fight_with_a_nemesis_pulls_in_its_trailing_band() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let leader = nemesis_at(&mut game, x, y);
    let mut band = Vec::new();
    for i in 0..NEMESIS_BAND_MAX as i32 {
        let f = wild_at(&mut game, x + 1 + i.min(1), y + 1);
        game.world.entity_mut(f).insert(NemesisFollower(leader));
        band.push(f);
    }
    // Deep enough that the zone's group ceiling holds the whole band.
    set_zone(&mut game, 8);
    let pack = game.gather_pack(leader);
    for f in band {
        assert!(pack.contains(&f), "the band joins the fight");
    }
}

// ---- Review fixes ----

/// Decompiles `target` outright: a skilled player, plenty of catalysts, and
/// the roll retried until it lands.
fn decompile(game: &mut Game, target: Entity) {
    let player = game.player_entity();
    game.world
        .get_mut::<crate::components::Decompiler>(player)
        .unwrap()
        .skill = 50;
    set_inventory(game, &[(ids::ICE_BREAKER, 50)]);
    for _ in 0..50 {
        if game.decompile_body(target, player) {
            return;
        }
    }
    panic!("the decompile never landed");
}

#[test]
fn a_decompiled_follower_leaves_the_band_and_survives_a_march() {
    let mut game = new_game();
    established_base(&mut game);
    let (leader, band) = marching_band(&mut game);
    let captured = band[0];

    decompile(&mut game, captured);
    assert!(game.world.get::<NemesisFollower>(captured).is_none());

    game.nemesis_muster();
    assert!(
        !followers_of(&mut game, leader).contains(&captured),
        "a roster program is not in the band"
    );
    // The muster above recruited a replacement, so the band is full.
    assert_eq!(followers_of(&mut game, leader).len(), NEMESIS_BAND_MAX);
    game.world.entity_mut(leader).insert(NemesisMuster {
        ticks: NEMESIS_MARCH_DELAY,
    });
    game.nemesis_march_check();
    assert!(
        game.world.get_entity(captured).is_ok(),
        "the march must not spend a program the player owns"
    );
    assert!(
        game.world.get_entity(band[1]).is_err(),
        "the march did fire"
    );
    assert!(game.world.get::<Tamed>(captured).is_some());
}

#[test]
fn a_tamed_body_is_never_a_band_member_even_if_still_tagged() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let leader = nemesis_at(&mut game, x, y);
    let pet = wild_at(&mut game, x + 8, y);
    let game_player = game.player_entity();
    game.world
        .entity_mut(pet)
        .insert((NemesisFollower(leader), Tamed { owner: game_player }));
    assert!(game.nemesis_band(leader).is_empty());
    game.nemesis_muster();
    let at = *game.world.get::<Position>(pet).unwrap();
    assert_eq!((at.x, at.y), (x + 8, y), "not walked to the leader");
}

#[test]
fn a_tamed_nemesis_releases_its_band_and_stops_recruiting() {
    let mut game = new_game();
    let (x, y) = open_ground(&mut game);
    let leader = nemesis_at(&mut game, x, y);
    let mut band = Vec::new();
    for i in 0..2 {
        let f = wild_at(&mut game, x + i, y + 1);
        game.world.entity_mut(f).insert(NemesisFollower(leader));
        band.push(f);
    }

    decompile(&mut game, leader);
    muster_for(&mut game, NEMESIS_RECRUIT_INTERVAL * 2);

    for f in band {
        assert!(game.world.get::<NemesisFollower>(f).is_none(), "released");
    }
    assert!(
        game.world
            .query::<&NemesisFollower>()
            .iter(&game.world)
            .next()
            .is_none(),
        "a roster program recruits nobody"
    );
    assert!(game.world.get_entity(leader).is_ok());
}
