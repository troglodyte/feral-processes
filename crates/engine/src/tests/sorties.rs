//! Sorties: the role, the catalogue, the board, dispatch, the trip and the
//! return.

use bevy_ecs::prelude::{Entity, With};

use super::support::{scratch_assets_dir, stand_in_base, test_assets_dir};
use crate::Game;
use crate::components::{DownedPrograms, Glyph, GlyphColor, Position, Structure};
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

/// A rest repairs **neither** a dispatched squad nor the staff it left
/// behind. Both roles are away from the player: a squad is provisioned out
/// of its own dispatch (`SORTIE_PROVISION_HEAL_FRACTION`, paid between
/// battles) and the base's pool mends at a Repair Bay.
///
/// The roles are asserted *before* the rest deliberately. `Staff` is what
/// `party::role_of` leaves over, so a fixture that failed to dispatch would
/// leave both programs staff and this would pass for the wrong reason —
/// which is exactly the shape of vacuous test the assertions below cannot
/// tell apart on their own.
#[test]
fn a_rest_repairs_neither_a_dispatched_squad_nor_the_staff_left_behind() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4613, 4, 500);
    assert!(game.dispatch_sortie(&site, &staff[..2]).is_ok());
    for &e in &staff {
        game.world
            .get_mut::<crate::components::Stats>(e)
            .unwrap()
            .hp = 1;
    }
    assert_eq!(game.program_role(staff[0]), Some(ProgramRole::Sortie));
    assert_eq!(game.program_role(staff[2]), Some(ProgramRole::Staff));

    game.rest().unwrap();

    let hp = |g: &Game, e: Entity| g.world.get::<crate::components::Stats>(e).unwrap().hp;
    assert_eq!(
        hp(&game, staff[0]),
        1,
        "a squad in another sector is not repaired by a rest taken at home"
    );
    assert_eq!(
        hp(&game, staff[2]),
        1,
        "and neither are the bodies still working the base"
    );
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

// ------------------------------------------------------------- the trip

/// A base with a squad already away, plus the members it sent.
fn a_dispatched_sortie(seed: u32, mode: DifficultyMode) -> (Game, Vec<Entity>) {
    let mut game = Game::new(seed, mode, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);
    let depot = deploy(&mut game, "depot", 0, 1);
    let currency = game.currency();
    game.world
        .entity_mut(depot)
        .insert(crate::components::Stock {
            output: [(currency, 5_000)].into_iter().collect(),
            capacity: 9_999,
            ..Default::default()
        });
    let staff: Vec<Entity> = (0..5)
        .map(|i| {
            game.adopt_program("scrapper", 4 + i, 4, 1.0)
                .expect("test roster program")
        })
        .collect();
    let site = game.sortie_board().expect("a Relay stands")[0].id.clone();
    let squad = staff[..3].to_vec();
    game.dispatch_sortie(&site, &squad)
        .expect("a legal dispatch");
    (game, squad)
}

/// A sortie's kill leaves a downed program on the player through
/// `Game::leave_downed_program` — the same call an ordinary kill makes, not
/// a copy of it (`game/combat_rewards.rs`'s own reason for splitting that
/// function out in the first place: a sortie paying through a drifted
/// second copy is exactly the trap `Perk::Teardown` used to fall into).
///
/// Swept across seeds, `nest_orphans_across`'s reason nearby: whether a
/// battle this tick lands any kill at all depends on the habitat draw and
/// the fight's own rolls, so a single seed proves only its own outcome.
#[test]
fn a_sortie_kill_leaves_a_downed_program_on_the_player() {
    let found = (5000..5020).any(|seed| {
        let (mut game, _) = a_dispatched_sortie(seed, DifficultyMode::Forgiving);
        let player = game.player_entity();
        let before = game.world.get::<DownedPrograms>(player).unwrap().0.len();
        let total = game.world.resource::<Sorties>().0[0].ticks_total;
        for _ in 0..(total - 1) {
            game.wait();
        }
        let after = game.world.get::<DownedPrograms>(player).unwrap().0.len();
        after > before
    });
    assert!(
        found,
        "no seed in the sweep left a downed program on the player after a sortie kill"
    );
}

/// **The load-bearing test of the feature.** A battle spawns its
/// opposition, fights it and despawns it inside one call, so no bevy system
/// ever observes it. A hostile that outlives its battle is a defect, not a
/// tuning question.
#[test]
fn a_battle_leaves_no_hostile_behind() {
    let (mut game, _) = a_dispatched_sortie(4700, DifficultyMode::Forgiving);
    assert_eq!(game.world.resource::<Sorties>().0[0].battles_done, 0);

    // Far enough for several battles to have fired, and short of the return.
    let total = game.world.resource::<Sorties>().0[0].ticks_total;
    for _ in 0..(total - 1) {
        game.wait();
    }

    let record = &game.world.resource::<Sorties>().0[0];
    assert!(
        record.battles_done > 0 || record.aborted,
        "the trip must actually have fought something, or this proves nothing"
    );

    // Ambient world spawns run on every tick, so the count is compared over
    // *hostiles at the sentinel* rather than over the whole world.
    let mut query = game
        .world
        .query_filtered::<&Position, With<crate::components::Hostile>>();
    let at_sentinel = query
        .iter(&game.world)
        .filter(|p| p.x.abs() > 1_000_000 || p.y.abs() > 1_000_000)
        .count();
    assert_eq!(
        at_sentinel, 0,
        "a sortie battle must spawn and despawn inside one tick"
    );
}

/// The same rule from the other side: with the ambient spawner unable to
/// interfere, the world holds exactly as many entities after a run of
/// off-screen battles as before them.
#[test]
fn the_entity_count_is_unchanged_across_a_battle() {
    let (mut game, squad) = a_dispatched_sortie(4701, DifficultyMode::Forgiving);
    // The squad drops on the first blow, so the battle ends with hostiles
    // still standing. Without that the opposition dies to a man and a
    // despawn that only cleared *corpses* would pass this — which is
    // exactly what a mutation run found.
    for &member in &squad {
        game.world
            .get_mut::<crate::components::Stats>(member)
            .unwrap()
            .hp = 1;
    }
    let before = game.world.iter_entities().count();
    game.world.resource_mut::<Sorties>().0[0].ticks_elapsed =
        game.world.resource::<Sorties>().0[0].ticks_total / 2;

    // One battle's worth of ticks, resolved directly rather than through
    // `wait`, so ambient spawning and the caravan never enter the count.
    game.run_sorties();
    let record = game.world.resource::<Sorties>().0[0].clone();
    assert!(record.battles_done > 0, "a battle must have fired");
    assert!(
        record.aborted,
        "the squad must have dropped, leaving survivors on the other side"
    );

    assert_eq!(
        game.world.iter_entities().count(),
        before,
        "spawn, fight and despawn all happened inside one call"
    );
}

/// The trip aborts on the first casualty — remaining battles are skipped,
/// the loot so far is kept, and the return travel still runs. It does not
/// come home early.
#[test]
fn the_first_casualty_aborts_but_does_not_shorten_the_trip() {
    let (mut game, squad) = a_dispatched_sortie(4702, DifficultyMode::Forgiving);
    let total = game.world.resource::<Sorties>().0[0].ticks_total;

    // Put the squad one blow from dropping, so the first battle takes one.
    for &member in &squad {
        game.world
            .get_mut::<crate::components::Stats>(member)
            .unwrap()
            .hp = 1;
    }
    game.world.resource_mut::<Sorties>().0[0].ticks_elapsed = total / 2;
    game.run_sorties();

    let record = game.world.resource::<Sorties>().0[0].clone();
    assert!(record.aborted, "a casualty aborts the trip");
    assert!(
        record.battles_done < record.battles_total,
        "the remaining battles are skipped"
    );

    // It is still away, and stays away until the countdown runs out.
    let elapsed = record.ticks_elapsed;
    for _ in 0..(total - elapsed - 1) {
        game.run_sorties();
        assert_eq!(
            game.world.resource::<Sorties>().0.len(),
            1,
            "an aborted trip does not teleport home"
        );
    }
    game.run_sorties();
    assert!(
        game.world.resource::<Sorties>().0.is_empty(),
        "it comes home on the tick it was always going to"
    );
    assert_eq!(
        game.world.resource::<Sorties>().0.len(),
        0,
        "and the record is gone"
    );
}

/// One rule, two meanings: Forgiving benches and keeps the roster slot,
/// Permadeath dissolves.
#[test]
fn a_casualty_is_benched_under_forgiving_and_dissolved_under_permadeath() {
    for (mode, benched) in [
        (DifficultyMode::Forgiving, true),
        (DifficultyMode::Permadeath, false),
    ] {
        let (mut game, squad) = a_dispatched_sortie(4703, mode);
        let roster_before = game.pet_count();
        for &member in &squad {
            game.world
                .get_mut::<crate::components::Stats>(member)
                .unwrap()
                .hp = 1;
        }
        let total = game.world.resource::<Sorties>().0[0].ticks_total;
        game.world.resource_mut::<Sorties>().0[0].ticks_elapsed = total / 2;
        game.run_sorties();

        let record = game.world.resource::<Sorties>().0[0].clone();
        assert!(record.aborted, "{mode:?}: somebody should have dropped");
        assert_eq!(record.casualties.len(), 1);

        let alive: usize = squad
            .iter()
            .filter(|&&e| game.world.get::<crate::components::Stats>(e).is_some())
            .count();
        if benched {
            assert_eq!(alive, squad.len(), "Forgiving benches rather than deletes");
            assert_eq!(
                game.pet_count(),
                roster_before,
                "a benched program keeps its roster slot"
            );
            let downed = squad
                .iter()
                .filter(|&&e| game.world.get::<crate::components::Downed>(e).is_some())
                .count();
            assert_eq!(downed, 1, "the casualty comes home Downed");
        } else {
            assert_eq!(alive, squad.len() - 1, "Permadeath dissolves");
            assert_eq!(game.pet_count(), roster_before - 1, "the slot is freed");
        }
    }
}

/// Provisions restore Integrity between battles, which is the single dial
/// that decides whether a twenty-fight trip is survivable.
#[test]
fn provisions_restore_integrity_between_battles() {
    let (mut game, squad) = a_dispatched_sortie(4704, DifficultyMode::Forgiving);
    let total = game.world.resource::<Sorties>().0[0].ticks_total;
    // Deep bars, hurt to half, so the squad cannot drop in one battle and
    // the heal (a fraction of `max_hp`) is comfortably larger than anything
    // a sector-1 pack can take off them. Both halves matter: at their real
    // Integrity the trip aborts on the first battle and no heal ever runs.
    for &member in &squad {
        let mut stats = game
            .world
            .get_mut::<crate::components::Stats>(member)
            .unwrap();
        stats.max_hp = 4_000;
        stats.hp = 2_000;
    }
    let hurt: Vec<i32> = squad
        .iter()
        .map(|&e| game.world.get::<crate::components::Stats>(e).unwrap().hp)
        .collect();

    game.world.resource_mut::<Sorties>().0[0].ticks_elapsed = total / 2;
    game.run_sorties();
    assert!(!game.world.resource::<Sorties>().0[0].aborted);

    // Somebody is better off than the damage they took would leave them —
    // the heal is the only thing in the loop that raises Integrity.
    let after: Vec<i32> = squad
        .iter()
        .map(|&e| game.world.get::<crate::components::Stats>(e).unwrap().hp)
        .collect();
    let heal = {
        let max_hp = game
            .world
            .get::<crate::components::Stats>(squad[0])
            .unwrap()
            .max_hp;
        (max_hp as f32 * crate::tuning::SORTIE_PROVISION_HEAL_FRACTION) as i32
    };
    assert!(heal > 0, "the provisioning heal must be worth something");
    assert!(
        after.iter().zip(&hurt).any(|(a, b)| a > b),
        "provisions must put Integrity back: {hurt:?} -> {after:?}"
    );
}

/// Power does not recover in the field, so Specials taper across a trip.
/// This is what earns the lower yield rather than tuning it.
#[test]
fn power_does_not_recover_in_the_field() {
    let (mut game, squad) = a_dispatched_sortie(4705, DifficultyMode::Forgiving);
    // Spend a member's reserve down, then run the trip out. Nothing in the
    // loop may put it back.
    let member = squad[0];
    game.spend_power(member, 999.0);
    let drained = game
        .world
        .get::<crate::components::PowerReserve>(member)
        .unwrap()
        .get();

    let total = game.world.resource::<Sorties>().0[0].ticks_total;
    game.world.resource_mut::<Sorties>().0[0].ticks_elapsed = total / 2;
    for _ in 0..(total / 2) {
        if game.world.resource::<Sorties>().0.is_empty() {
            break;
        }
        game.run_sorties();
    }

    let after = game
        .world
        .get::<crate::components::PowerReserve>(member)
        .unwrap()
        .get();
    assert!(
        after <= drained,
        "a reserve must not refill in the field: {drained} -> {after}"
    );
}

/// A returned program is staff again by omission — nothing writes a role
/// anywhere, which is the whole of why the fourth variant was worth having.
#[test]
fn a_returned_program_is_staff_again() {
    let (mut game, squad) = a_dispatched_sortie(4706, DifficultyMode::Forgiving);
    assert_eq!(game.program_role(squad[0]), Some(ProgramRole::Sortie));

    let total = game.world.resource::<Sorties>().0[0].ticks_total;
    game.world.resource_mut::<Sorties>().0[0].ticks_elapsed = total - 1;
    game.run_sorties();

    assert!(game.world.resource::<Sorties>().0.is_empty());
    for &member in &squad {
        if game.world.get::<crate::components::Stats>(member).is_none() {
            continue;
        }
        assert_ne!(
            game.program_role(member),
            Some(ProgramRole::Sortie),
            "the record is gone, so nothing can still call it away"
        );
    }
}

// ------------------------------------------- return, report and save

/// An in-flight sortie survives a save and load: the same members, the same
/// site, the same countdown.
#[test]
fn an_in_flight_sortie_survives_a_save_and_load() {
    let scratch = scratch_assets_dir("sortie_inflight_roundtrip");
    std::fs::create_dir_all(&*scratch).unwrap();
    let (mut game, squad) = a_dispatched_sortie(4800, DifficultyMode::Forgiving);
    for _ in 0..40 {
        game.wait();
    }
    let before = game.world.resource::<Sorties>().0[0].clone();
    assert!(before.ticks_elapsed > 0, "the countdown must have moved");
    let names: Vec<String> = squad.iter().map(|&e| game.creature_label(e)).collect();

    let path = scratch.join("save.bin");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let after = &loaded.world.resource::<Sorties>().0[0];
    assert_eq!(after.site, before.site, "the whole resolved site travels");
    assert_eq!(after.risk, before.risk);
    assert_eq!(after.ticks_total, before.ticks_total);
    assert_eq!(after.ticks_elapsed, before.ticks_elapsed);
    assert_eq!(after.battles_total, before.battles_total);
    assert_eq!(after.battles_done, before.battles_done);
    assert_eq!(after.kills, before.kills);
    assert_eq!(after.xp, before.xp);
    assert_eq!(after.loot, before.loot);
    assert_eq!(after.aborted, before.aborted);
    assert_eq!(after.casualties, before.casualties);

    let reloaded: Vec<String> = after
        .members
        .iter()
        .map(|&e| loaded.creature_label(e))
        .collect();
    assert_eq!(reloaded, names, "the same bodies are still away");
}

/// Membership rides `CreatureSave`, `party_slot`'s precedent — entity ids
/// are not stable across a save, which is exactly why the party does it
/// this way. Asserted through the *role*, since that is what every consumer
/// of membership actually reads.
#[test]
fn membership_is_restored_from_the_creature_side() {
    let scratch = scratch_assets_dir("sortie_membership");
    std::fs::create_dir_all(&*scratch).unwrap();
    let (mut game, _) = a_dispatched_sortie(4801, DifficultyMode::Forgiving);
    let path = scratch.join("save.bin");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let away: Vec<Entity> = loaded.world.resource::<Sorties>().0[0].members.clone();
    assert_eq!(away.len(), 3);
    for member in away {
        assert_eq!(
            loaded.program_role(member),
            Some(ProgramRole::Sortie),
            "a restored member is out of the labour pool again"
        );
    }
    // And the bodies that stayed are staff, or the index was read off by one.
    assert_eq!(loaded.base_staff().len(), 2);
}

/// The save format is not bumped: the fields are additive behind
/// `#[serde(default)]`, so a save written before sorties loads with none.
#[test]
fn a_pre_sortie_save_loads_with_no_sorties() {
    let scratch = scratch_assets_dir("sortie_pre_save");
    std::fs::create_dir_all(&*scratch).unwrap();
    let (mut game, _) = a_dispatched_sortie(4802, DifficultyMode::Forgiving);
    let path = scratch.join("save.bin");
    game.save(&path).unwrap();

    // Emptied first so both keys serialise on one line, then stripped
    // outright — which is what a file written before they existed looks
    // like. Packed back into a **real save** rather than left as RON: a
    // round trip alone leaves a `#[serde(skip)]` green while the save a
    // player actually reloads has lost the field.
    let mut data = crate::save::load_from_file(&path).unwrap();
    data.player.sorties.clear();
    for c in &mut data.creatures {
        c.sortie_index = None;
    }
    let text = crate::save::to_ron(&data).unwrap();
    let stripped: String = text
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("sorties:") && !t.starts_with("sortie_index:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stripped.lines().count() < text.lines().count(),
        "the keys must have been there to strip, or this proves nothing"
    );
    let old_path = scratch.join("old.bin");
    let stripped_data = crate::save::from_ron(&stripped).expect("a pre-sortie save still parses");
    crate::save::save_to_file(&old_path, &stripped_data).unwrap();

    let loaded = Game::load(&old_path, &test_assets_dir()).unwrap();
    assert!(
        loaded.world.resource::<Sorties>().0.is_empty(),
        "a pre-sortie save has nobody away"
    );
    assert_eq!(loaded.base_staff().len(), 5, "and everyone is staff again");
}

/// Loot lands in depots; what does not fit is logged rather than dropped in
/// silence — `return_to_depots`' existing rule.
#[test]
fn overflow_loot_is_logged_rather_than_lost() {
    let (mut game, _) = a_dispatched_sortie(4803, DifficultyMode::Forgiving);
    let scrap = crate::items::ItemId::from(crate::items::ids::CORE_FRAGMENT);
    {
        let record = &mut game.world.resource_mut::<Sorties>().0[0];
        record.loot = vec![(scrap.clone(), 100_000)];
        record.battles_done = record.battles_total;
        record.ticks_elapsed = record.ticks_total - 1;
    }

    let before = game.message_history(400).len();
    game.run_sorties();

    assert!(game.world.resource::<Sorties>().0.is_empty());
    let said: Vec<String> = game.message_history(400)[before..]
        .iter()
        .map(|l| l.text.clone())
        .collect();
    assert!(
        said.iter().any(|l| l.contains("no shelf to stand on")),
        "what did not fit must be said out loud: {said:?}"
    );
}

/// Loot that does fit lands on a Depot shelf, so the base is actually paid.
#[test]
fn returned_loot_lands_on_a_shelf() {
    let (mut game, _) = a_dispatched_sortie(4804, DifficultyMode::Forgiving);
    let scrap = crate::items::ItemId::from(crate::items::ids::CORE_FRAGMENT);
    let before = game
        .base_stock()
        .iter()
        .find(|r| r.item == scrap)
        .map(|r| r.qty)
        .unwrap_or(0);
    {
        let record = &mut game.world.resource_mut::<Sorties>().0[0];
        record.loot = vec![(scrap.clone(), 7)];
        // Every battle already fought, or the last tick fires all of them at
        // once and the haul under test is buried in what they dropped.
        record.battles_done = record.battles_total;
        record.ticks_elapsed = record.ticks_total - 1;
    }
    game.run_sorties();

    let after = game
        .base_stock()
        .iter()
        .find(|r| r.item == scrap)
        .map(|r| r.qty)
        .unwrap_or(0);
    assert_eq!(after, before + 7);
}

/// The report is derived off the record and evicts nothing — a screen that
/// rewrote what it draws would make the trip depend on whether anyone
/// looked.
#[test]
fn the_report_reads_the_record_without_changing_it() {
    let (mut game, squad) = a_dispatched_sortie(4805, DifficultyMode::Forgiving);
    for _ in 0..50 {
        game.wait();
    }
    let before = game.world.resource::<Sorties>().0[0].clone();

    let reports = game.sortie_reports();
    assert_eq!(reports.len(), 1);
    let report = &reports[0];
    assert_eq!(report.site, before.site.name);
    assert_eq!(report.members.len(), squad.len() - before.casualties.len());
    assert_eq!(report.kills, before.kills);
    assert_eq!(report.xp, before.xp);
    assert_eq!(report.battles_total, before.battles_total);
    assert_eq!(report.ticks_left, before.ticks_total - before.ticks_elapsed);

    let after = game.world.resource::<Sorties>().0[0].clone();
    assert_eq!(after.ticks_elapsed, before.ticks_elapsed);
    assert_eq!(after.battles_done, before.battles_done);
}

// --------------------------------------------------------- the censuses

/// Deleting `assets/sorties/` restores the pre-sortie game rather than
/// breaking one — `NeedDb` and `MemoryDb`'s property. Never gate a system or
/// a screen on the catalogue being non-empty.
#[test]
fn an_empty_catalogue_is_a_supported_install() {
    let scratch = scratch_assets_dir("sortie_empty");
    std::fs::create_dir_all(&*scratch).unwrap();
    super::support::copy_shipped_assets(&scratch, &[]);
    assert!(
        !scratch.join("sorties").exists(),
        "this install must genuinely have no catalogue, or the test is vacuous"
    );

    let mut game = Game::new(4900, DifficultyMode::Forgiving, &scratch).unwrap();
    deploy_relay(&mut game);
    assert_eq!(
        game.sortie_board(),
        Some(Vec::new()),
        "an empty catalogue is an empty board, not the absence of a Relay"
    );
    // And the game keeps running. `run_sorties` has nothing to do and the
    // rest of the base carries on.
    for _ in 0..200 {
        game.wait();
    }
}

/// Every shipped site's risk is inside a window `habitat_pools` can actually
/// serve — a site nothing can be drawn for is an offer that fights nothing
/// when it is taken.
#[test]
fn every_shipped_site_can_be_populated() {
    let (db, _) = SortieDb::load_dir(&test_assets_dir().join("sorties")).unwrap();
    assert!(!db.is_empty(), "the shipped catalogue must not be empty");
    let sites: Vec<crate::sorties::SortieDef> = db.iter().cloned().collect();

    // Across the run's whole span, not just sector 1: a risk offset is
    // applied on top of the sector's own step, and the top band never exits,
    // so a site that empties late would empty silently.
    for zone in [1u32, 4, 8, 12] {
        let mut game = Game::new(4901, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world
            .insert_resource(crate::resources::ZoneLevel(zone));
        let (ax, ay) = game.anchor_position().expect("a run has an anchor");
        for site in &sites {
            let pools = game.habitat_pools(ax, ay, None, site.risk);
            let Some((candidates, _)) = pools else {
                panic!("{} has no habitat at all in sector {zone}", site.id);
            };
            assert!(
                !candidates.is_empty(),
                "{} draws an empty pool in sector {zone}",
                site.id
            );
        }
    }
}

/// A sortie kill pays less than the same kill taken with the player in the
/// fight. `balance_sim` models no abilities and no base production, so it
/// cannot gate this — the assertion lives here, over the real assets.
///
/// It pins the **lever**, not the rate: what actually earns the lower yield
/// is Power not recovering in the field and no rest out there, neither of
/// which is a number this can read. What it catches is a retune that takes
/// the multiplier to 1.0 or above, and the rounding corner where a cheap
/// kill would pay the same either way.
#[test]
fn a_sortie_kill_pays_less_than_fighting_it_yourself() {
    assert!(
        crate::tuning::SORTIE_XP_MULTIPLIER < 1.0,
        "the whole point of the lever is that it is below 1"
    );

    let mut game = Game::new(4902, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species: Vec<String> = game
        .world
        .resource::<crate::species::SpeciesDb>()
        .all()
        .map(|s| s.id.clone())
        .collect();
    assert!(!species.is_empty());

    let mut priced = 0;
    for id in species {
        let Some(victim) = game.spawn_wild_creature(&id, 300, 300) else {
            continue;
        };
        let full = game.kill_xp(victim);
        game.world.entity_mut(victim).despawn();
        if full == 0 {
            continue;
        }
        let paid = (full as f32 * crate::tuning::SORTIE_XP_MULTIPLIER) as u32;
        assert!(
            paid < full,
            "{id} pays {paid} off-screen against {full} in a fight — the lever is not biting"
        );
        priced += 1;
    }
    assert!(priced > 0, "no species priced, so this asserted nothing");
}

/// Two trips in flight, and the first coming home must not cost the second
/// a tick.
///
/// The record behind a returning one slides into its index, so the walk has
/// to read whether it stepped rather than compare `index` to the new
/// length — which only catches a removal at the tail, and so lost the
/// second trip a tick in silence every time the first came home.
#[test]
fn a_returning_trip_does_not_skip_the_one_behind_it() {
    let mut game = Game::new(4903, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    deploy_relay(&mut game);
    let depot = deploy(&mut game, "depot", 0, 1);
    let currency = game.currency();
    game.world
        .entity_mut(depot)
        .insert(crate::components::Stock {
            output: [(currency, 5_000)].into_iter().collect(),
            capacity: 9_999,
            ..Default::default()
        });
    let staff: Vec<Entity> = (0..6)
        .map(|i| {
            game.adopt_program("scrapper", 4 + i, 4, 1.0)
                .expect("test roster program")
        })
        .collect();
    let board = game.sortie_board().expect("a Relay stands");
    assert!(board.len() >= 2, "this needs two sites to sign for");
    game.dispatch_sortie(&board[0].id, &staff[..2]).unwrap();
    game.dispatch_sortie(&board[1].id, &staff[2..4]).unwrap();

    // The first is one tick from home; the second is barely out. Both are
    // parked before any battle is due, so nothing but the countdown moves.
    {
        let records = &mut game.world.resource_mut::<Sorties>().0;
        records[0].ticks_elapsed = records[0].ticks_total - 1;
        records[0].battles_done = records[0].battles_total;
        records[1].ticks_elapsed = 0;
        records[1].battles_done = records[1].battles_total;
    }
    game.run_sorties();

    let left = game.world.resource::<Sorties>().0.clone();
    assert_eq!(left.len(), 1, "the first came home");
    assert_eq!(
        left[0].ticks_elapsed, 1,
        "the second must still have been advanced this tick"
    );
}

// ------------------------------------------------------------- the walk

/// Stands every body of `squad` on a floored cell of the starting pocket,
/// clear of the Home, the Relay and the Depot.
///
/// `adopt_program` writes the tile a program was beaten on, which is a
/// *surface* coordinate and is solid rock read as base space more often than
/// not — `entry_tile` is what normally gives a tamed program a base cell, on
/// its first drift. A fixture that skips that stands its bodies inside the
/// wall, where a walk is refused by design and the test proves nothing.
fn stand_squad_in_the_pocket(game: &mut Game, squad: &[Entity]) {
    for (i, &member) in squad.iter().enumerate() {
        game.world.entity_mut(member).insert(Position {
            x: 2,
            y: -1 - i as i32,
        });
    }
}

/// Every cell of `path` is walkable base space, and each step is one
/// Chebyshev move from the last — a walker interpolating between two cells
/// that are not neighbours slides, and one crossing a solid cell walks
/// through the wall.
fn assert_walkable_and_contiguous(game: &Game, path: &[(i32, i32)]) {
    let grid = game.world.resource::<crate::base_grid::BaseGrid>();
    for &(x, y) in path {
        assert!(grid.walkable(x, y), "({x}, {y}) is not walkable base space");
    }
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        assert!(
            (a.0 - b.0).abs() <= 1 && (a.1 - b.1).abs() <= 1 && a != b,
            "{a:?} and {b:?} are not neighbouring cells"
        );
    }
}

/// A dispatch queues one walk per member, from the tile it was standing on
/// to base space's one door.
#[test]
fn a_dispatch_queues_one_walk_out_per_member() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4620, 4, 500);
    let squad = &staff[..2];
    stand_squad_in_the_pocket(&mut game, squad);
    let from: Vec<(i32, i32)> = squad
        .iter()
        .map(|&e| {
            let p = game.world.get::<Position>(e).unwrap();
            (p.x, p.y)
        })
        .collect();

    game.dispatch_sortie(&site, squad)
        .expect("a legal dispatch");

    let walks = game.take_transits();
    assert_eq!(walks.len(), squad.len(), "one walk per body sent out");
    for (walk, start) in walks.iter().zip(&from) {
        assert_eq!(
            walk.path.first(),
            Some(start),
            "a body sets off from its own tile"
        );
        assert_eq!(
            walk.path.last(),
            Some(&crate::game::base_space::BASE_EXIT_CELL),
            "base space has one door and the walk ends at it"
        );
        assert_walkable_and_contiguous(&game, &walk.path);
    }
}

/// The queue is drained, not accumulated: a cue mid-flight has nothing to
/// say to the frame after the one that drew it.
#[test]
fn taking_the_walks_drains_them() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4621, 4, 500);
    stand_squad_in_the_pocket(&mut game, &staff);
    game.dispatch_sortie(&site, &staff[..2])
        .expect("a legal dispatch");

    assert!(!game.take_transits().is_empty());
    assert!(game.take_transits().is_empty(), "the queue drains on take");
}

/// The walk follows the dug ground rather than the straight line to the
/// door. A body down a corridor that bends has to come back along it, and a
/// straight interpolation would take it through the rock in between.
#[test]
fn a_walk_bends_around_solid_rock() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4622, 4, 500);
    let member = staff[0];
    // A corridor east out of the pocket and then north, ending at a cell
    // whose straight line back to the door crosses unmined rock.
    let corridor: Vec<(i32, i32)> = (3..=8)
        .map(|x| (x, 3))
        .chain((-2..3).map(|y| (8, y)))
        .collect();
    {
        let mut grid = game.world.resource_mut::<crate::base_grid::BaseGrid>();
        for &(x, y) in &corridor {
            grid.lay_floor(x, y);
        }
    }
    let end = (8, -2);
    game.world
        .entity_mut(member)
        .insert(Position { x: end.0, y: end.1 });
    stand_squad_in_the_pocket(&mut game, &staff[1..]);

    game.dispatch_sortie(&site, &[member])
        .expect("a legal dispatch");

    let walks = game.take_transits();
    assert_eq!(walks.len(), 1);
    let path = &walks[0].path;
    assert_eq!(path.first(), Some(&end));
    assert_eq!(path.last(), Some(&crate::game::base_space::BASE_EXIT_CELL));
    assert_walkable_and_contiguous(&game, path);
    let straight = end.0.abs().max(end.1.abs()) as usize + 1;
    assert!(
        path.len() > straight,
        "a walk that is no longer than the straight line went through the wall: {path:?}"
    );
}

/// A body whose tile is not walkable base space gets no walk at all. An
/// adopted program's `Position` is the surface tile it was beaten on until
/// its first drift, so this is reachable in play and not a corner case.
#[test]
fn a_body_standing_in_rock_walks_nowhere() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4623, 4, 500);
    let member = staff[0];
    game.world
        .entity_mut(member)
        .insert(Position { x: 40, y: 40 });
    stand_squad_in_the_pocket(&mut game, &staff[1..]);

    game.dispatch_sortie(&site, &[member])
        .expect("a legal dispatch");

    assert!(
        game.take_transits().is_empty(),
        "no walk can be drawn out of solid rock"
    );
}

/// The return is the same cue with its ends swapped — in through the door
/// and back to the tile the body left from. Direction needs no field.
#[test]
fn a_return_walks_them_back_in_through_the_door() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4624, 4, 500);
    let squad = &staff[..2];
    stand_squad_in_the_pocket(&mut game, squad);
    let from: Vec<(i32, i32)> = squad
        .iter()
        .map(|&e| {
            let p = game.world.get::<Position>(e).unwrap();
            (p.x, p.y)
        })
        .collect();
    game.dispatch_sortie(&site, squad)
        .expect("a legal dispatch");
    let _ = game.take_transits();

    {
        let record = &mut game.world.resource_mut::<Sorties>().0[0];
        record.ticks_elapsed = record.ticks_total - 1;
        record.battles_done = record.battles_total;
    }
    game.run_sorties();

    let walks = game.take_transits();
    assert_eq!(walks.len(), squad.len(), "one walk per body coming home");
    for (walk, home) in walks.iter().zip(&from) {
        assert_eq!(
            walk.path.first(),
            Some(&crate::game::base_space::BASE_EXIT_CELL),
            "a returning body comes in through the door"
        );
        assert_eq!(walk.path.last(), Some(home), "and back to its own tile");
        assert_walkable_and_contiguous(&game, &walk.path);
    }
}

/// A program that did not come back does not walk in. Under Permadeath its
/// entity is gone by the time the report is drawn, so this falls out of
/// there being no tile to walk to rather than out of a check.
#[test]
fn a_casualty_does_not_walk_home() {
    let (mut game, site, staff) = a_base_ready_to_dispatch(4625, 4, 500);
    let squad = &staff[..2];
    stand_squad_in_the_pocket(&mut game, squad);
    game.dispatch_sortie(&site, squad)
        .expect("a legal dispatch");
    let _ = game.take_transits();

    game.world.entity_mut(squad[0]).remove::<Position>();
    {
        let record = &mut game.world.resource_mut::<Sorties>().0[0];
        record.ticks_elapsed = record.ticks_total - 1;
        record.battles_done = record.battles_total;
    }
    game.run_sorties();

    assert_eq!(
        game.take_transits().len(),
        1,
        "only the body that came home walks in"
    );
}
