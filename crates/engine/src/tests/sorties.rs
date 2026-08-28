//! Sorties: the role, the catalogue, the board, dispatch, the trip and the
//! return.

use bevy_ecs::prelude::Entity;

use super::support::{scratch_assets_dir, stand_in_base, test_assets_dir};
use crate::Game;
use crate::components::{Glyph, GlyphColor, Position, Structure};
use crate::game::party::ProgramRole;
use crate::game::sortie::{SortieReach, SortieRefusal};
use crate::resources::DifficultyMode;
use crate::resources::{Sortie, Sorties};
use crate::sorties::SortieDb;

// ------------------------------------------------------------- the role

/// A program named by an in-flight sortie is `Sortie`, not `Staff` — and
/// `Staff` stays the leftover rather than becoming something assigned.
#[test]
fn a_dispatched_program_is_not_staff() {
    let mut game = Game::new(4200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let program = game
        .adopt_program("scrapper", 4, 4, 1.0)
        .expect("test roster program");

    assert_eq!(game.program_role(program), Some(ProgramRole::Staff));

    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![program]));

    assert_eq!(
        game.program_role(program),
        Some(ProgramRole::Sortie),
        "a program named by an in-flight sortie has left the labour pool"
    );
}

/// The map and the examine ray both drop an away program, and neither
/// needed a new rule: `position_is_honest` tests for `Staff` exactly.
#[test]
fn a_dispatched_program_leaves_the_map() {
    let mut game = Game::new(4201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let program = game
        .adopt_program("scrapper", 4, 4, 1.0)
        .expect("test roster program");

    assert!(game.position_is_honest(program), "idle staff are drawn");

    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![program]));

    assert!(
        !game.position_is_honest(program),
        "an away program must not claim a tile it is not standing on"
    );
}

/// The base's labour pool is what the scheduler, the drift pass and the
/// entropy pass all read, and an away program is out of it by omission
/// rather than by a check at each of the three.
#[test]
fn a_dispatched_program_leaves_the_labour_pool() {
    let mut game = Game::new(4202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let program = game
        .adopt_program("scrapper", 4, 4, 1.0)
        .expect("test roster program");

    assert!(game.base_staff().contains(&program));

    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![program]));

    assert!(
        !game.base_staff().contains(&program),
        "an away program is not staff, so nothing in the base can post it"
    );
}

// ------------------------------------------------------ the catalogue

/// An absent directory is a supported install: it loads empty, warns about
/// nothing, and the feature is simply absent. `NeedDb` and `MemoryDb`'s
/// property, and the reason nothing may ever gate on the db being
/// non-empty.
#[test]
fn an_absent_catalogue_loads_empty_and_quiet() {
    let (db, warnings) = SortieDb::load_dir(std::path::Path::new("/nonexistent/sorties"))
        .expect("an absent directory is not an error");
    assert!(db.is_empty());
    assert!(warnings.is_empty(), "an absent directory is not a fault");
}

/// A malformed file is skipped with a warning, never a panic that takes
/// startup down with it.
#[test]
fn a_malformed_site_is_skipped_with_a_warning() {
    let scratch = scratch_assets_dir("sortie_malformed");
    let dir = scratch.join("sorties");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("broken.ron"), "(this is not ron").unwrap();
    std::fs::write(
        dir.join("good.ron"),
        r#"(
    id: "good",
    name: "A Good Site",
    description: "Fine.",
    risk: 0,
    battles_min: 4,
    battles_max: 6,
)"#,
    )
    .unwrap();

    let (db, warnings) = SortieDb::load_dir(&dir).unwrap();
    assert_eq!(db.iter().count(), 1, "the good file still loads");
    assert_eq!(warnings.len(), 1, "the broken one is reported, not fatal");
}

/// Sorted by id, because every caller walks it and an unsorted walk is how
/// a derived board stops being reproducible.
#[test]
fn the_catalogue_iterates_in_id_order() {
    let (db, _) = SortieDb::load_dir(&test_assets_dir().join("sorties")).unwrap();
    let ids: Vec<&str> = db.iter().map(|d| d.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted);
}

/// A site whose battle range is inverted or empty is a content fault and is
/// refused at load, the way `field_buff_duration_mismatch` refuses its
/// corners — a `battles_max` below `battles_min` would silently roll an
/// empty range at board time, far from the file that caused it.
#[test]
fn an_inverted_battle_range_is_refused_at_load() {
    let scratch = scratch_assets_dir("sortie_inverted");
    let dir = scratch.join("sorties");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("backwards.ron"),
        r#"(
    id: "backwards",
    name: "Backwards",
    description: "Bad.",
    risk: 1,
    battles_min: 9,
    battles_max: 3,
)"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("empty.ron"),
        r#"(
    id: "empty",
    name: "Empty",
    description: "Also bad.",
    risk: 0,
    battles_min: 0,
    battles_max: 3,
)"#,
    )
    .unwrap();

    let (db, warnings) = SortieDb::load_dir(&dir).unwrap();
    assert!(db.is_empty());
    assert_eq!(warnings.len(), 2);
}

// ------------------------------------------------- the risk offset

/// A risk offset reaches the same window `depth` does, so a sortie can ask
/// for tougher opposition without the caller re-deriving the biome rules.
#[test]
fn a_risk_offset_raises_the_habitat_window() {
    let mut game = Game::new(4300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(crate::resources::ZoneLevel(1));
    // Well clear of the opening ring, which filters the pool down to what a
    // fresh player can beat and would mask the window moving at all.
    let (x, y) = (400, 400);

    let Some((base, _)) = game.habitat_pools(x, y, None, 0) else {
        panic!("this tile should be habitable");
    };
    let Some((raised, _)) = game.habitat_pools(x, y, None, 4) else {
        panic!("a raised window should still resolve");
    };

    assert_ne!(
        base, raised,
        "a four-step offset must move the window, or the parameter is inert"
    );
}

/// Zero is exactly today's behaviour, which is what lets every existing
/// caller pass it and nothing move. Asserted against a hand-rolled window at
/// the same step rather than against itself — comparing one call to another
/// passes against an offset that does nothing at all.
#[test]
fn a_zero_offset_is_todays_window() {
    let mut game = Game::new(4301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(crate::resources::ZoneLevel(3));
    let (x, y) = (400, 400);

    let step = game.danger_steps(None);
    let biome = game
        .world
        .resource_mut::<crate::world::WorldMap>()
        .tile(x, y)
        .biome;
    let expected: Vec<String> = game
        .world
        .resource::<crate::species::SpeciesDb>()
        .windowed_matches(biome, step)
        .into_iter()
        .map(|s| s.id.clone())
        .collect();

    let (ordinary, _) = game
        .habitat_pools(x, y, None, 0)
        .expect("this tile should be habitable");
    assert_eq!(ordinary, expected);
}

// ------------------------------------------------------------ the Relay

/// Stands a Home and a Relay up in base space and puts the party on the
/// laid floor beside them, `deploy_broker`'s shape. A Relay without a Home
/// does not survive a save the test later loads.
fn deploy_relay(game: &mut Game) {
    game.lay_starting_pocket();
    deploy(game, "home", 0, 0);
    deploy(game, "relay", 1, 0);
    super::support::stand_in_base_at(game, 1, 1);
}

/// A structure of `kind` standing at `(x, y)` in base space.
fn deploy(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    game.world
        .spawn((
            Structure {
                kind: kind.to_string(),
            },
            Position { x, y },
            Glyph {
                ch: 'K',
                color: GlyphColor::Magenta,
            },
        ))
        .id()
}

/// Three states rather than two booleans, `NoPost::BoxedIn`'s rule: "no
/// Relay built" and "not standing in base" leave the player different
/// errands, and a screen that cannot tell them apart says the wrong
/// sentence.
#[test]
fn sortie_reach_reports_the_three_states() {
    let mut game = Game::new(4400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.sortie_reach(), SortieReach::NoRelay);

    deploy_relay(&mut game);
    assert_eq!(game.sortie_reach(), SortieReach::AtRelay);

    // Out of base space entirely — on the open grid, where no floor can
    // answer for the party.
    game.world
        .insert_resource(crate::resources::Locale::Surface);
    assert_eq!(game.sortie_reach(), SortieReach::OffBase);
}

/// It measures the base and not the distance to the mast: a Relay stands on
/// laid floor by construction, so its own tile says nothing the base does
/// not. The far edge of the pocket is the case the rule exists for.
#[test]
fn the_far_side_of_the_base_still_reaches_the_relay() {
    let mut game = Game::new(4401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);
    let edge = crate::tuning::STARTING_POCKET_RADIUS;
    assert!(edge - 1 > 2, "the far edge must be beyond arm's length");
    super::support::stand_in_base_at(&mut game, edge, 0);
    assert_eq!(game.sortie_reach(), SortieReach::AtRelay);
}

// ------------------------------------------------------------ the board

/// Derived, never stored: reloading reproduces the identical board, because
/// the inputs are identical and there is no stored roll to reroll.
#[test]
fn the_board_survives_a_save_and_load_unchanged() {
    let scratch = scratch_assets_dir("sortie_board_roundtrip");
    let mut game = Game::new(4500, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);

    let before = game.sortie_board().expect("a Relay stands");
    assert!(!before.is_empty(), "the shipped catalogue offers something");

    std::fs::create_dir_all(&*scratch).unwrap();
    let path = scratch.join("save.bin");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();

    assert_eq!(
        loaded.sortie_board().expect("a Relay stands"),
        before,
        "a reload must not reroll the board"
    );
}

/// Drawing the board spends no `GameRng`. A draw would not survive a reload
/// and would shift every later roll in the run — `stack::generate`'s rule.
/// Asserted by comparing the stream, since a test that only checks the board
/// is stable passes against a board that draws and discards.
#[test]
fn drawing_the_board_spends_no_rng() {
    let mut game = Game::new(4501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);

    fn peek(g: &mut Game) -> u64 {
        use rand::RngExt;
        g.world
            .resource_mut::<crate::resources::GameRng>()
            .0
            .random()
    }

    super::support::reseed_rng(&mut game, 77);
    let without = peek(&mut game);

    super::support::reseed_rng(&mut game, 77);
    let _ = game.sortie_board();
    let with = peek(&mut game);

    assert_eq!(without, with, "the board must not touch the run's stream");
}

/// It rotates on its own as the epoch advances — which is what makes "no
/// save-scumming" a property rather than a lockout.
#[test]
fn the_board_rotates_with_the_clock() {
    let mut game = Game::new(4502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);

    let first = game.sortie_board().unwrap();
    // Several epochs on rather than one: the shipped catalogue is four
    // sites into three slots, so a single step can legitimately land on the
    // same set. What is asserted is that the board is not frozen.
    let turned = (1..=12).any(|epoch| {
        game.world
            .resource_mut::<crate::resources::GameClock>()
            .tick = crate::tuning::SORTIE_BOARD_ROTATION_TICKS * epoch;
        game.sortie_board().unwrap() != first
    });
    assert!(turned, "twelve epochs on, the offers have turned over");
}

/// The screen and the trip quote the same number, `BuildOrderRow`'s rule
/// that every figure is a call.
#[test]
fn a_row_quotes_the_duration_the_trip_will_run() {
    let mut game = Game::new(4503, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);

    for row in game.sortie_board().unwrap() {
        assert_eq!(row.ticks, Game::sortie_duration(row.risk, row.battles));
    }
}

/// A row's battle count is inside its own authored range, so the offer can
/// be quoted before it is signed for and the trip cannot run a different
/// number of fights than the board said.
#[test]
fn a_rows_battle_count_is_inside_the_sites_range() {
    let mut game = Game::new(4505, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);
    let db = SortieDb::load_dir(&test_assets_dir().join("sorties"))
        .unwrap()
        .0;

    for row in game.sortie_board().unwrap() {
        let def = db.get(&row.id).expect("a board row names a shipped site");
        assert!(
            (def.battles_min..=def.battles_max).contains(&row.battles),
            "{} was offered at {} fights, outside {}..={}",
            row.id,
            row.battles,
            def.battles_min,
            def.battles_max
        );
    }
}

/// One epoch's board never offers the same site twice — it is drawn without
/// replacement, which a test comparing only lengths would not see.
#[test]
fn a_board_never_offers_one_site_twice() {
    let mut game = Game::new(4506, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);
    for epoch in 0..40u64 {
        game.world
            .resource_mut::<crate::resources::GameClock>()
            .tick = crate::tuning::SORTIE_BOARD_ROTATION_TICKS * epoch;
        let rows = game.sortie_board().unwrap();
        let mut ids: Vec<String> = rows.iter().map(|r| r.id.0.clone()).collect();
        let offered = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), offered, "epoch {epoch} offered a duplicate site");
    }
}

/// Duration reads the risk **offset**, never the absolute band — so a deep
/// sector does not silently make every trip enormous.
#[test]
fn the_sector_does_not_lengthen_a_trip() {
    assert_eq!(
        Game::sortie_duration(0, 6),
        crate::tuning::SORTIE_TRAVEL_BASE_TICKS + crate::tuning::SORTIE_TICKS_PER_BATTLE * 6
    );
}

/// No board without a Relay, and no panic either.
#[test]
fn no_relay_means_no_board() {
    let mut game = Game::new(4504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.sortie_board().is_none());
}

// --------------------------------------------------------- dispatch

/// A base with a Home, a Relay and a Depot holding `provisions` of the
/// build currency, plus `bodies` idle staff on the roster.
///
/// Returns the game, the id of the first site on the board, and the staff.
fn a_base_ready_to_dispatch(
    seed: u32,
    bodies: usize,
    provisions: u32,
) -> (Game, crate::sorties::SortieId, Vec<Entity>) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);
    let depot = deploy(&mut game, "depot", 0, 1);
    let currency = game.currency();
    game.world
        .entity_mut(depot)
        .insert(crate::components::Stock {
            output: [(currency, provisions)].into_iter().collect(),
            capacity: 9_999,
            ..Default::default()
        });
    let staff: Vec<Entity> = (0..bodies)
        .map(|i| {
            game.adopt_program("scrapper", 4 + i as i32, 4, 1.0)
                .expect("test roster program")
        })
        .collect();
    let site = game.sortie_board().expect("a Relay stands")[0].id.clone();
    (game, site, staff)
}

/// Sums what the base's shelves hold, so a refusal can be shown to have
/// spent nothing.
fn stock_total(game: &Game) -> u32 {
    game.base_stock().iter().map(|r| r.qty).sum()
}

/// Every refusal lands before anything is spent. Asserted per refusal: a
/// single test over one of them passes against eight paths where seven
/// spend.
#[test]
fn every_refusal_spends_nothing() {
    // (name, builder) — each builds a base whose dispatch must be refused.
    #[allow(clippy::type_complexity)]
    let cases: Vec<(
        &str,
        Box<dyn Fn() -> (Game, crate::sorties::SortieId, Vec<Entity>)>,
    )> = vec![
        (
            "off base",
            Box::new(|| {
                let (mut game, site, staff) = a_base_ready_to_dispatch(4600, 4, 500);
                game.world
                    .insert_resource(crate::resources::Locale::Surface);
                (game, site, staff[..2].to_vec())
            }),
        ),
        (
            "not on the board",
            Box::new(|| {
                let (game, _, staff) = a_base_ready_to_dispatch(4601, 4, 500);
                (
                    game,
                    crate::sorties::SortieId::from("no_such_site"),
                    staff[..2].to_vec(),
                )
            }),
        ),
        (
            "empty squad",
            Box::new(|| {
                let (game, site, _) = a_base_ready_to_dispatch(4602, 4, 500);
                (game, site, Vec::new())
            }),
        ),
        (
            "a duplicate body",
            Box::new(|| {
                let (game, site, staff) = a_base_ready_to_dispatch(4603, 4, 500);
                (game, site, vec![staff[0], staff[0]])
            }),
        ),
        (
            "a party member",
            Box::new(|| {
                let (mut game, site, staff) = a_base_ready_to_dispatch(4604, 4, 500);
                super::support::enlist(&mut game, staff[0]);
                (game, site, vec![staff[0], staff[1]])
            }),
        ),
        (
            "a downed body",
            Box::new(|| {
                let (mut game, site, staff) = a_base_ready_to_dispatch(4605, 4, 500);
                game.world
                    .entity_mut(staff[0])
                    .insert(crate::components::Downed);
                (game, site, vec![staff[0], staff[1]])
            }),
        ),
        (
            "a wounded body",
            Box::new(|| {
                let (mut game, site, staff) = a_base_ready_to_dispatch(4606, 4, 500);
                let mut stats = game
                    .world
                    .get_mut::<crate::components::Stats>(staff[0])
                    .unwrap();
                stats.hp = 1;
                (game, site, vec![staff[0], staff[1]])
            }),
        ),
        (
            "the whole roster",
            Box::new(|| {
                let (game, site, staff) = a_base_ready_to_dispatch(4607, 2, 500);
                (game, site, staff)
            }),
        ),
        (
            "nothing to provision with",
            Box::new(|| {
                let (game, site, staff) = a_base_ready_to_dispatch(4608, 4, 0);
                (game, site, staff[..2].to_vec())
            }),
        ),
    ];

    for (name, build) in cases {
        let (mut game, site, members) = build();
        let before = stock_total(&game);
        let away = game.world.resource::<Sorties>().0.len();
        assert!(
            game.dispatch_sortie(&site, &members).is_err(),
            "{name} should have been refused"
        );
        assert_eq!(stock_total(&game), before, "{name} spent something");
        assert_eq!(
            game.world.resource::<Sorties>().0.len(),
            away,
            "{name} filed a record anyway"
        );
    }
}

/// Party and wielded programs are refused by name, so seconding one is an
/// explicit act rather than a side effect of a dispatch screen.
#[test]
fn a_party_member_cannot_be_dispatched() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4610, 4, 500);
    super::support::enlist(&mut game, staff[0]);
    assert!(matches!(
        game.dispatch_sortie(&site, &[staff[0], staff[1]]),
        Err(SortieRefusal::NotStaff(_))
    ));
}

/// A hurt program is refused: sending one on a twenty-fight trip is the
/// mistake the abort rule cannot save you from, because it fires on the
/// first battle.
#[test]
fn a_wounded_program_is_refused() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4611, 4, 500);
    let max_hp = game
        .world
        .get::<crate::components::Stats>(staff[0])
        .unwrap()
        .max_hp;
    // Just under the threshold, so the test is about the threshold and not
    // about a body at 1 HP.
    let hurt = (max_hp as f32 * crate::tuning::SORTIE_MIN_HP_FRACTION) as i32 - 1;
    game.world
        .get_mut::<crate::components::Stats>(staff[0])
        .unwrap()
        .hp = hurt;
    assert!(matches!(
        game.dispatch_sortie(&site, &[staff[0], staff[1]]),
        Err(SortieRefusal::Wounded(_))
    ));

    // And one unit of Integrity higher is fine, or the threshold is not
    // where the test says it is.
    game.world
        .get_mut::<crate::components::Stats>(staff[0])
        .unwrap()
        .hp = hurt + 1;
    assert!(game.dispatch_sortie(&site, &[staff[0], staff[1]]).is_ok());
}

/// The base is never emptied. Production stops dead and a sweep lands on an
/// empty base — the same category of guard as `max_deployed`.
#[test]
fn a_dispatch_may_not_empty_the_roster() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4612, 3, 500);
    assert_eq!(
        game.dispatch_sortie(&site, &staff),
        Err(SortieRefusal::WouldEmptyTheBase)
    );
    // One short of everyone is allowed, or the guard is off by one and the
    // feature is unusable on a small roster.
    assert!(game.dispatch_sortie(&site, &staff[..2]).is_ok());
}

/// A successful dispatch charges the provisioning and takes the bodies off
/// the labour pool in the same call.
#[test]
fn a_dispatch_charges_and_takes_the_bodies() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4613, 4, 500);
    let squad = &staff[..2];
    let battles = game
        .sortie_board()
        .unwrap()
        .iter()
        .find(|r| r.id == site)
        .unwrap()
        .battles;
    let cost: u32 = game
        .sortie_provision_cost(battles, squad.len())
        .iter()
        .map(|(_, q)| q)
        .sum();
    assert!(cost > 0, "the provisioning must actually cost something");

    let before = stock_total(&game);
    game.dispatch_sortie(&site, squad)
        .expect("a legal dispatch");

    assert_eq!(stock_total(&game), before - cost);
    for &member in squad {
        assert_eq!(game.program_role(member), Some(ProgramRole::Sortie));
    }
    for &staying in &staff[2..] {
        assert_eq!(game.program_role(staying), Some(ProgramRole::Staff));
    }
}

/// The record stores the whole resolved site, never the id: a board that
/// rotates while the squad is out must not be able to rewrite the trip.
#[test]
fn the_record_outlives_the_board_that_offered_it() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4614, 4, 500);
    game.dispatch_sortie(&site, &staff[..2])
        .expect("a legal dispatch");
    let signed = game.world.resource::<Sorties>().0[0].clone();

    // Roll the board on several epochs; whatever it now offers, the trip in
    // flight is unchanged.
    game.world
        .resource_mut::<crate::resources::GameClock>()
        .tick += crate::tuning::SORTIE_BOARD_ROTATION_TICKS * 5;
    let now = game.world.resource::<Sorties>().0[0].clone();
    assert_eq!(now.site, signed.site);
    assert_eq!(now.battles_total, signed.battles_total);
    assert_eq!(now.ticks_total, signed.ticks_total);
}
