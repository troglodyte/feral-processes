//! Decompiling a wild program into a companion, and the catalysts it consumes.

use super::support::*;
use crate::*;

#[test]
fn successful_decompile_removes_wander_ai_so_the_tamed_creature_stops_roaming() {
    let mut game = Game::new(19, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
            WanderAi::default(),
            Position { x: 3, y: 3 },
            Stats {
                hp: 1,
                max_hp: 10,
                atk: 1,
                def: 1,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    // Near-dead target + maxed decompiler skill + plenty of breakers,
    // so the capture-chance clamp (95%) makes a handful of attempts
    // succeed for certain, without needing to control the RNG directly.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::ICE_BREAKER), 50);
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

    for _ in 0..50 {
        if game.world.get::<Tamed>(wild).is_some() {
            break;
        }
        player_decompiles(&mut game);
    }

    assert!(
        game.world.get::<Tamed>(wild).is_some(),
        "creature should have been tamed"
    );
    assert!(game.world.get::<Hostile>(wild).is_none());
    assert!(
        game.world.get::<WanderAi>(wild).is_none(),
        "a tamed creature must stop roaming like a wild one"
    );
}

#[test]
fn decompile_spends_the_highest_potency_catalyst_held_not_the_shipped_one() {
    // The mod case `taming_potency` exists for: a dropped-in catalyst
    // stronger than the shipped ICE Breaker must be the one resolved
    // and consumed, with no Rust change.
    let dir = modded_assets_dir(
        "strong_catalyst",
        &[],
        &[(
            "master_key.ron",
            r#"(id: "master_key", name: "Master Key", taming_potency: Some(0.9))"#,
        )],
        &[],
    );
    let mut game = Game::new(3100, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 1), ("master_key", 1)]);

    player_decompiles(&mut game);

    let inv = game.world.get::<Inventory>(game.player_entity()).unwrap();
    assert_eq!(
        inv.count(&ItemId::from("master_key")),
        0,
        "the strongest catalyst held should be the one spent"
    );
    assert_eq!(
        inv.count(&ItemId::from(ids::ICE_BREAKER)),
        1,
        "the weaker catalyst must be left untouched"
    );
}

#[test]
fn decompiling_with_no_catalyst_is_refused_without_naming_a_shipped_item() {
    let mut game = Game::new(3101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);

    player_decompiles(&mut game);

    assert!(
        game.world.get::<Tamed>(wild).is_none(),
        "a decompile with no catalyst must not tame anything"
    );
    let refusal = game
        .message_log(usize::MAX)
        .into_iter()
        .map(|(_, line)| line)
        .find(|line| line.starts_with("You have no"))
        .expect("the refusal should be logged");
    let shipped_names: Vec<String> = game
        .world
        .resource::<ItemDb>()
        .all()
        .map(|d| d.name.clone())
        .collect();
    for name in shipped_names {
        assert!(
            !refusal.contains(&name),
            "the refusal must not name a specific item, got: {refusal}"
        );
    }
}

#[test]
fn two_catalysts_of_equal_potency_resolve_to_the_first_id_alphabetically() {
    let dir = modded_assets_dir(
        "tied_catalysts",
        &[],
        &[
            (
                "alpha_key.ron",
                r#"(id: "alpha_key", name: "Alpha Key", taming_potency: Some(0.5))"#,
            ),
            (
                "omega_key.ron",
                r#"(id: "omega_key", name: "Omega Key", taming_potency: Some(0.5))"#,
            ),
        ],
        &[],
    );
    let mut game = Game::new(3102, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    start_battle_with_a_wild_program(&mut game);
    // Stocked in reverse so the tie can't be won by inventory order.
    set_inventory(&mut game, &[("omega_key", 1), ("alpha_key", 1)]);

    player_decompiles(&mut game);

    let inv = game.world.get::<Inventory>(game.player_entity()).unwrap();
    assert_eq!(
        inv.count(&ItemId::from("alpha_key")),
        0,
        "a tie should resolve to the first item id alphabetically"
    );
    assert_eq!(inv.count(&ItemId::from("omega_key")), 1);
}

#[test]
fn the_decompile_preview_follows_the_catalyst_held_not_a_fixed_item() {
    let dir = modded_assets_dir(
        "preview_catalyst",
        &[],
        &[(
            "master_key.ron",
            r#"(id: "master_key", name: "Master Key", taming_potency: Some(0.9))"#,
        )],
        &[],
    );
    let mut game = Game::new(3104, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let wild = spawn_wild_on_player_tile(&mut game);

    set_inventory(&mut game, &[(ids::ICE_BREAKER, 1)]);
    let with_shipped = game
        .inspect(wild)
        .unwrap()
        .decompile_chance
        .expect("holding a catalyst should quote odds");
    set_inventory(&mut game, &[("master_key", 1)]);
    let with_mod = game
        .inspect(wild)
        .unwrap()
        .decompile_chance
        .expect("holding a catalyst should quote odds");
    assert!(
        with_mod > with_shipped,
        "a stronger catalyst must preview better odds: {with_mod} vs {with_shipped}"
    );

    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 1)]);
    assert!(
        game.inspect(wild).unwrap().decompile_chance.is_none(),
        "with no catalyst there are no odds to quote — the action is unavailable"
    );
}

#[test]
fn battle_view_offers_no_decompile_odds_without_a_catalyst() {
    let mut game = Game::new(3105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    start_battle_with_a_wild_program(&mut game);
    assert!(
        game.battle_view().unwrap().groups[0]
            .decompile_chance
            .is_some(),
        "the starting kit holds a catalyst"
    );

    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);

    // This is also what gates the engine-emitted de[c]ompile option.
    assert!(
        game.battle_view().unwrap().groups[0]
            .decompile_chance
            .is_none()
    );
}

#[test]
fn the_shipped_ice_breaker_still_tames_for_a_player_holding_only_it() {
    let mut game = Game::new(3103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
    // Maxed Decompiler skill pins capture_chance to its 0.95 clamp, so
    // 50 seeded attempts succeed without the test depending on a
    // particular roll.
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

    let mut attempts = 0;
    for _ in 0..50 {
        if game.world.get::<Tamed>(wild).is_some() {
            break;
        }
        player_decompiles(&mut game);
        attempts += 1;
    }

    assert!(
        game.world.get::<Tamed>(wild).is_some(),
        "the shipped catalyst must still tame exactly as before"
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::ICE_BREAKER)),
        50 - attempts,
        "one ICE Breaker per attempt, same as before"
    );
}
