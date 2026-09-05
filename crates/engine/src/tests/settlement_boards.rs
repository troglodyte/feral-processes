//! A town's own job board — Phase 5.
//!
//! Everything here is about `issuer` being the whole vocabulary: it decides
//! whose board a job came off, where it may be delivered, and whose standing
//! finishing or abandoning it moves. The Broker's own board is `None` and is
//! asserted to be untouched by any of it.

use super::support::*;
use crate::contracts::Objective;
use crate::items::ids;
use crate::resources::{ActiveContracts, Standings};
use crate::settlements::relations::Standing;
use crate::settlements::{SettlementKey, Specialty};
use crate::tuning::*;
use crate::*;

fn game() -> Game {
    Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A town the party is standing next to, so `settlement_reach` answers yes.
fn town_next_to_player(game: &mut Game) -> SettlementKey {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let key = SettlementKey { rx: 0, ry: 0 };
    place_settlement(game, key, pos.x + 1, pos.y);
    key
}

fn set_standing(game: &mut Game, key: SettlementKey, standing: i32) {
    game.world
        .resource_mut::<Standings>()
        .0
        .entry(key)
        .or_default()
        .standing = standing;
}

/// A Home and a Broker standing, with the party on the base's floor —
/// `tests/contracts.rs`'s `deploy_broker`, which is private to that module.
/// Spends no tick, so a seeded board below it is unmoved.
fn stand_up_broker(game: &mut Game) {
    game.lay_starting_pocket();
    for (kind, x) in [("home", 0), ("contract_broker", 1)] {
        game.world.spawn((
            crate::components::Structure {
                kind: kind.to_string(),
            },
            Position { x, y: 0 },
            crate::components::Glyph {
                ch: '!',
                color: crate::components::GlyphColor::Yellow,
            },
        ));
    }
    stand_in_base_at(game, 1, 1);
}

/// The held contract with `id`, by id and never by index — a new run is
/// already holding its first onboarding mission at index 0.
fn held(game: &Game, id: &str) -> crate::resources::ActiveContract {
    game.world
        .resource::<ActiveContracts>()
        .active
        .iter()
        .find(|c| c.def.id.as_str() == id)
        .unwrap_or_else(|| panic!("{id} is not held"))
        .clone()
}

fn holds(game: &Game, id: &str) -> bool {
    game.world
        .resource::<ActiveContracts>()
        .active
        .iter()
        .any(|c| c.def.id.as_str() == id)
}

fn set_specialty(game: &mut Game, key: SettlementKey, specialty: Specialty) {
    game.world
        .resource_mut::<crate::resources::Settlements>()
        .0
        .get_mut(&key)
        .unwrap()
        .def
        .specialty = specialty;
}

// ---------------------------------------------------------------------------
// The censuses
// ---------------------------------------------------------------------------

/// The other axis of `Specialty::of_objective`, which is exhaustive on
/// `Objective`: every specialty must be *reachable* through it, or a town
/// ships with a first tier nothing can ever fall into and a board that
/// silently ignores its own character.
#[test]
fn every_specialty_is_courted_by_an_objective() {
    let objectives = [
        Objective::Terminate {
            species: None,
            count: 1,
        },
        Objective::Deliver {
            item: ids::CREDITS.into(),
            count: 1,
        },
        Objective::Hold {
            item: ids::CREDITS.into(),
            count: 1,
        },
        Objective::Descend { depth: 1 },
        Objective::Breach { zone: 2 },
        Objective::Build {
            structure: "home".to_string(),
        },
        Objective::Perform {
            deed: crate::contracts::Deed::Examined,
        },
    ];
    for specialty in [
        Specialty::Gear,
        Specialty::Materials,
        Specialty::Routines,
        Specialty::Programs,
    ] {
        assert!(
            objectives
                .iter()
                .any(|objective| specialty.favours(objective)),
            "{} is courted by no objective",
            specialty.label()
        );
    }
}

// ---------------------------------------------------------------------------
// Reading a board
// ---------------------------------------------------------------------------

#[test]
fn there_is_no_town_board_without_a_town_to_read_it_at() {
    let mut game = game();
    assert!(
        game.settlement_board(SettlementKey { rx: 9, ry: 9 })
            .is_none()
    );
}

#[test]
fn a_town_you_are_not_standing_at_posts_no_board() {
    let mut game = game();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let key = SettlementKey { rx: 0, ry: 0 };
    place_settlement(&mut game, key, pos.x + 40, pos.y + 40);
    assert!(game.settlement_board(key).is_none());
}

/// `settlement_view`'s rule: a closed counter is not a missing one.
#[test]
fn a_hostile_town_posts_an_empty_board_rather_than_none() {
    let mut game = game();
    let key = town_next_to_player(&mut game);
    set_standing(&mut game, key, SETTLEMENT_MIN_STANDING);
    assert_eq!(game.standing_band(key), Standing::Hostile);
    assert_eq!(game.settlement_board(key), Some(Vec::new()));
}

#[test]
fn a_town_posts_as_many_jobs_as_its_band_allows() {
    let mut game = game();
    let key = town_next_to_player(&mut game);

    set_standing(&mut game, key, 0);
    let neutral = game.settlement_board(key).unwrap().len();

    set_standing(&mut game, key, SETTLEMENT_ALLIED_STANDING);
    let allied = game.settlement_board(key).unwrap().len();

    assert_eq!(neutral, SETTLEMENT_NEUTRAL_BOARD_SLOTS);
    assert_eq!(allied, SETTLEMENT_ALLIED_BOARD_SLOTS);
    assert!(allied > neutral);
}

/// The ranking, not a filter: the first tier is the town's specialty, so a
/// board with anything favoured on offer leads with it.
#[test]
fn a_towns_specialty_leads_its_board() {
    let mut game = game();
    let key = town_next_to_player(&mut game);
    set_standing(&mut game, key, SETTLEMENT_ALLIED_STANDING);

    for specialty in [Specialty::Programs, Specialty::Materials] {
        set_specialty(&mut game, key, specialty);
        let board = game.settlement_board_defs_for_test(key).unwrap();
        let held: Vec<Objective> = board.iter().map(|def| def.objective.clone()).collect();
        // Whatever the pool held, everything favoured sits above everything
        // that is not — that is what "first tier" means and it is checkable
        // without knowing which jobs the seed rolled.
        let first_unfavoured = held.iter().position(|o| !specialty.favours(o));
        if let Some(cut) = first_unfavoured {
            assert!(
                held[cut..].iter().all(|o| !specialty.favours(o)),
                "{} posted a favoured job below an unfavoured one",
                specialty.label()
            );
        }
    }
}

/// Two towns in one sector must not post the same three jobs — the whole
/// reason the key is folded into the seed.
#[test]
fn two_towns_in_one_sector_post_different_work() {
    let mut game = game();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let east = SettlementKey { rx: 0, ry: 0 };
    let west = SettlementKey { rx: 1, ry: 0 };
    place_settlement(&mut game, east, pos.x + 1, pos.y);
    place_settlement(&mut game, west, pos.x - 1, pos.y);
    set_standing(&mut game, east, SETTLEMENT_ALLIED_STANDING);
    set_standing(&mut game, west, SETTLEMENT_ALLIED_STANDING);

    let east_ids: Vec<_> = game
        .settlement_board(east)
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    let west_ids: Vec<_> = game
        .settlement_board(west)
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert_ne!(east_ids, west_ids);
}

/// Onboarding is the Broker's errand: a tutorial mission must never reach a
/// town board, where `ensure_tutorial_held` would hand it straight back out.
#[test]
fn a_town_never_posts_an_onboarding_mission() {
    let mut game = game();
    let key = town_next_to_player(&mut game);
    set_standing(&mut game, key, SETTLEMENT_ALLIED_STANDING);
    for row in game.settlement_board(key).unwrap() {
        assert!(!row.tutorial, "{} reached a town board", row.name);
    }
}

/// The board is derived and rotates on its own — no `GameRng` draw, no save
/// field. Reading it twice in the same epoch answers the same thing.
#[test]
fn a_towns_board_is_stable_within_its_epoch() {
    let mut game = game();
    let key = town_next_to_player(&mut game);
    let first: Vec<_> = game
        .settlement_board(key)
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    let again: Vec<_> = game
        .settlement_board(key)
        .unwrap()
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(first, again);
}

// ---------------------------------------------------------------------------
// Signing, delivering, finishing
// ---------------------------------------------------------------------------

#[test]
fn a_job_signed_at_a_town_remembers_who_posted_it() {
    let mut game = game();
    let key = town_next_to_player(&mut game);
    let id = game.settlement_board(key).unwrap()[0].id.clone();

    assert_eq!(game.accept_contract(&id, Some(key)), Ok(()));
    assert_eq!(held(&game, id.as_str()).issuer, Some(key));
}

/// The Broker's half is untouched by any of this: what the run hands itself
/// is still its own, and `None` is what says so.
#[test]
fn the_runs_own_contracts_have_no_issuer() {
    let game = game();
    assert!(!game.world.resource::<ActiveContracts>().active.is_empty());
    for held in &game.world.resource::<ActiveContracts>().active {
        assert_eq!(held.issuer, None, "{} came from a town", held.def.name);
    }
}

#[test]
fn a_town_you_are_not_standing_at_cannot_be_signed_with() {
    let mut game = game();
    let key = town_next_to_player(&mut game);
    let id = game.settlement_board(key).unwrap()[0].id.clone();

    // Walk out of reach and the offer is no longer on any counter.
    let mut pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    pos.x += 20;
    *game
        .world
        .get_mut::<Position>(game.player_entity())
        .unwrap() = pos;
    assert_eq!(
        game.accept_contract(&id, Some(key)),
        Err(crate::game::contracts::ContractRefusal::NotOffered)
    );
    assert!(!holds(&game, id.as_str()));
}

#[test]
fn finishing_a_towns_job_raises_its_standing() {
    let mut game = game();
    let key = town_next_to_player(&mut game);
    let id = game.settlement_board(key).unwrap()[0].id.clone();
    game.accept_contract(&id, Some(key)).unwrap();
    let before = game.standing(key);
    let idx = game
        .world
        .resource::<ActiveContracts>()
        .active
        .iter()
        .position(|c| c.def.id == id)
        .unwrap();

    game.complete_contract(idx);

    assert_eq!(game.standing(key), before + SETTLEMENT_CONTRACT_STANDING);
    assert!(!holds(&game, id.as_str()));
}

#[test]
fn handing_a_towns_job_back_costs_standing() {
    let mut game = game();
    let key = town_next_to_player(&mut game);
    let id = game.settlement_board(key).unwrap()[0].id.clone();
    game.accept_contract(&id, Some(key)).unwrap();
    let before = game.standing(key);

    assert!(game.abandon_contract(&id));

    assert_eq!(game.standing(key), before + SETTLEMENT_ABANDON_STANDING);
    assert!(SETTLEMENT_ABANDON_STANDING < 0);
}

/// The whole rule of the delivery half: a job is delivered where it was
/// signed. The party cannot hand a town's cargo to the Broker even standing
/// at the desk, and cannot hand the Broker's to a town.
///
/// Two positions rather than one, because the two reaches are mutually
/// exclusive by construction — `settlement_reach` is false in base space and
/// `broker_reach` is `AtBroker` only there. Standing at each in turn is what
/// makes the refusal the **issuer**'s rather than the reach's, which is what
/// this test would otherwise prove nothing about.
#[test]
fn a_job_is_delivered_where_it_was_signed() {
    let mut game = game();
    let key = town_next_to_player(&mut game);
    let item: crate::items::ItemId = ids::CORE_FRAGMENT.into();

    let hold = |game: &mut Game, id: &str, issuer: Option<SettlementKey>| {
        let accepted_tick = game.current_tick();
        game.world.resource_mut::<ActiveContracts>().active.push(
            crate::resources::ActiveContract {
                def: crate::contracts::ContractDef {
                    id: crate::contracts::ContractId::from(id),
                    name: id.to_string(),
                    description: String::new(),
                    objective: Objective::Deliver {
                        item: ids::CORE_FRAGMENT.into(),
                        count: 2,
                    },
                    reward: Vec::new(),
                    min_zone: 0,
                    repeatable: false,
                    starter: false,
                    tutorial: None,
                },
                progress: 0,
                accepted_tick,
                issuer,
            },
        );
    };
    hold(&mut game, "town_job", Some(key));
    hold(&mut game, "broker_job", None);

    let player = game.player_entity();
    let carried = |game: &Game| {
        game.world
            .get::<crate::components::Inventory>(player)
            .unwrap()
            .count(&item)
    };
    // On top of whatever the run started with — a new player is not
    // empty-handed, and a hardcoded figure here reads as a leak.
    let start = carried(&game);
    game.world
        .get_mut::<crate::components::Inventory>(player)
        .unwrap()
        .add(item.clone(), 8);

    let town_job = crate::contracts::ContractId::from("town_job");
    let broker_job = crate::contracts::ContractId::from("broker_job");

    // Standing at the town: its own job takes cargo, the Broker's does not.
    assert_eq!(
        game.deliver_to_contract(&broker_job, Some(key)),
        Err(crate::game::contracts::ContractRefusal::NotOffered)
    );
    assert_eq!(game.deliver_to_contract(&town_job, Some(key)), Ok(2));
    assert_eq!(carried(&game), start + 6);
    // Filling it completed it, and completion paid the town.
    assert_eq!(game.standing(key), SETTLEMENT_CONTRACT_STANDING);

    // Now at the Broker's desk, which really is reachable — so the town job's
    // refusal there can only be the issuer.
    hold(&mut game, "town_job", Some(key));
    stand_up_broker(&mut game);
    assert_eq!(
        game.broker_reach(),
        crate::game::contracts::BrokerReach::AtBroker
    );
    assert_eq!(
        game.deliver_to_contract(&town_job, None),
        Err(crate::game::contracts::ContractRefusal::NotOffered)
    );
    assert_eq!(carried(&game), start + 6);
    assert_eq!(game.deliver_to_contract(&broker_job, None), Ok(2));
    assert_eq!(carried(&game), start + 4);
}

/// The additive-field property: a save written before towns posted work
/// loads every held contract as the Broker's, which is what it was.
#[test]
fn a_save_from_before_town_jobs_loads_its_contracts_as_the_brokers() {
    let scratch = scratch_assets_dir("settlement_boards_save");
    std::fs::create_dir_all(&*scratch).unwrap();
    let path = scratch.join("save.bin");
    let mut game = game();
    let key = town_next_to_player(&mut game);
    let id = game.settlement_board(key).unwrap()[0].id.clone();
    game.accept_contract(&id, Some(key)).unwrap();
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(held(&loaded, id.as_str()).issuer, Some(key));

    // And with the field stripped, exactly as a pre-Phase-5 file has it.
    // Legal only because the save is field-named RON.
    //
    // Whole *block*, not the one line `strip_field_from_save` drops: pretty
    // RON breaks `Some((rx: .., ry: ..))` across four lines, and a line
    // filter leaves the tail behind as a parse error — which would make this
    // test fail for a reason that has nothing to do with the property.
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(
        raw.contains("issuer: Some("),
        "the current build has to be writing an issuer, or this proves nothing"
    );
    let mut older = String::new();
    let mut depth = 0usize;
    for line in raw.lines() {
        if depth == 0 && !line.trim_start().starts_with("issuer:") {
            older.push_str(line);
            older.push('\n');
            continue;
        }
        depth += line.matches('(').count();
        depth -= line.matches(')').count().min(depth);
    }
    assert!(
        !older.contains("issuer:"),
        "the key has to actually be gone"
    );
    std::fs::write(&path, older).unwrap();
    let old = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(held(&old, id.as_str()).issuer, None);
}
