//! `CharacterChoice` and `Game::new_with` — the foundation the rest of the
//! character-creation feature builds on. `new_and_new_with_default_produce_
//! the_same_player` is the load-bearing one: it is what protects the
//! ~1,600 `Game::new` call sites across the suite from a regression here.

use super::support::*;
use crate::achievements::MainStat;
use crate::species::AffinityClass;
use crate::tuning;
use crate::*;

/// A save path unique to one test, cleaned up on the way out the way every
/// other save/load test in this crate does — see `research.rs`'s
/// `a_save_round_trip_preserves_unlocked_research` for the precedent.
fn save_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("feral_creation_{name}_{}.sav", std::process::id()))
}

fn stats_at(index: MainStat, points: u32) -> [u32; 4] {
    let mut stats = [0u32; 4];
    let i = MainStat::all().iter().position(|s| *s == index).unwrap();
    stats[i] = points;
    stats
}

#[test]
fn new_and_new_with_default_produce_the_same_player() {
    let seed = 90_001;
    let a = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let b = Game::new_with(
        seed,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &CharacterChoice::default(),
    )
    .unwrap();

    let pa = a.player_entity();
    let pb = b.player_entity();

    let sa = a.world.get::<Stats>(pa).unwrap();
    let sb = b.world.get::<Stats>(pb).unwrap();
    assert_eq!(sa.hp, sb.hp);
    assert_eq!(sa.max_hp, sb.max_hp);
    assert_eq!(sa.atk, sb.atk);
    assert_eq!(sa.mitigation, sb.mitigation);

    let ga = a.world.get::<Glyph>(pa).unwrap();
    let gb = b.world.get::<Glyph>(pb).unwrap();
    assert_eq!(ga.ch, gb.ch);
    assert_eq!(ga.color, gb.color);

    assert_eq!(
        a.world.get::<Inventory>(pa).unwrap().items,
        b.world.get::<Inventory>(pb).unwrap().items
    );
    assert_eq!(
        a.world.get::<Routines>(pa).unwrap().0,
        b.world.get::<Routines>(pb).unwrap().0
    );
}

#[test]
fn creation_points_are_additive_over_the_baseline() {
    let points = tuning::CREATION_STAT_POINTS; // Integrity costs 1, so this fits exactly.
    let choice = CharacterChoice {
        stats: stats_at(MainStat::Integrity, points),
        ..CharacterChoice::default()
    };
    assert_eq!(
        choice.cost(),
        Some(points * tuning::CREATION_COST_INTEGRITY)
    );

    let game = Game::new_with(
        90_002,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &choice,
    )
    .unwrap();
    let stats = game.world.get::<Stats>(game.player_entity()).unwrap();

    let expected_max_hp =
        tuning::PLAYER_BASE_STATS.max_hp + (points * tuning::CREATION_GAIN_INTEGRITY) as i32;
    assert_eq!(stats.max_hp, expected_max_hp);
    // A run must not start damaged — `MainStat::Integrity`'s own trap.
    assert_eq!(stats.hp, stats.max_hp);
}

#[test]
fn mitigation_costs_more_than_a_point() {
    let pool = tuning::CREATION_STAT_POINTS;
    // "Spending the whole pool" on an axis priced above 1 buys only as many
    // whole units as the pool covers — the remainder is unspendable.
    let units = pool / tuning::CREATION_COST_DEF;
    let choice = CharacterChoice {
        stats: stats_at(MainStat::Def, units),
        ..CharacterChoice::default()
    };
    assert!(choice.cost().is_some());

    let game = Game::new_with(
        90_003,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &choice,
    )
    .unwrap();
    let stats = game.world.get::<Stats>(game.player_entity()).unwrap();
    let gained = stats.mitigation - tuning::PLAYER_BASE_STATS.mitigation;

    assert_eq!(gained, (pool / tuning::CREATION_COST_DEF) as i32);
    assert_ne!(
        gained, pool as i32,
        "Def costs more than a point per point of mitigation"
    );
}

#[test]
fn an_overspent_choice_is_refused() {
    // One point over the pool at Atk's 1-for-1 rate — cheapest possible
    // overspend.
    let overspent = CharacterChoice {
        stats: stats_at(MainStat::Atk, tuning::CREATION_STAT_POINTS + 1),
        ..CharacterChoice::default()
    };
    assert_eq!(overspent.cost(), None);

    let game = Game::new_with(
        90_004,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &overspent,
    )
    .unwrap();
    let stats = game.world.get::<Stats>(game.player_entity()).unwrap();
    assert_eq!(
        stats.atk,
        tuning::PLAYER_BASE_STATS.atk,
        "an overspent choice must fall back to no spend, not a clamped one"
    );
}

/// **Through a real file, not a RON string round trip.** `ron::from_str(&
/// ron::to_string(x))` cannot catch a field that `Game::save` simply never
/// writes — only `Game::save` followed by a fresh `Game::load` exercises the
/// actual write path.
#[test]
fn a_created_player_round_trips_through_a_real_save() {
    let choice = CharacterChoice {
        name: "Zephyr".to_string(),
        class: Some(AffinityClass::Saboteur),
        glyph: 'Z',
        sprite: "hero".to_string(),
        colour: Some(3),
        ..CharacterChoice::default()
    };
    let mut game = Game::new_with(
        90_010,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &choice,
    )
    .unwrap();

    let path = save_path("round_trip");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let player = loaded.player_entity();
    assert_eq!(loaded.world.get::<Glyph>(player).unwrap().ch, 'Z');
    let identity = loaded.world.get::<PlayerIdentity>(player).unwrap();
    assert_eq!(identity.class, Some(AffinityClass::Saboteur));
    assert_eq!(identity.sprite, "hero");
    assert_eq!(identity.colour, Some(3));
    assert_eq!(loaded.world.get::<CustomName>(player).unwrap().0, "Zephyr");
}

/// The points and the kit are receipts, the shape a `Stat` talent already
/// has: `Game::load` must restore the numbers a creation choice produced
/// without running the choice through `apply_character_choice` a second
/// time. Verified by mutation — see the task report for the transcript
/// that shows this test failing when `Game::load` is made to re-run it.
#[test]
fn loading_does_not_re_apply_the_choice() {
    let choice = CharacterChoice {
        class: Some(AffinityClass::Striker),
        stats: stats_at(MainStat::Atk, tuning::CREATION_STAT_POINTS),
        ..CharacterChoice::default()
    };
    let mut game = Game::new_with(
        90_011,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &choice,
    )
    .unwrap();
    let player = game.player_entity();
    let stats_before = *game.world.get::<Stats>(player).unwrap();
    let inventory_before = game.world.get::<Inventory>(player).unwrap().items.clone();

    let path = save_path("no_reapply");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let loaded_player = loaded.player_entity();
    let stats_after = *loaded.world.get::<Stats>(loaded_player).unwrap();
    let inventory_after = loaded
        .world
        .get::<Inventory>(loaded_player)
        .unwrap()
        .items
        .clone();

    assert_eq!(stats_after.hp, stats_before.hp);
    assert_eq!(stats_after.max_hp, stats_before.max_hp);
    assert_eq!(
        stats_after.atk, stats_before.atk,
        "a reapplied choice would double the Atk spend"
    );
    assert_eq!(stats_after.mitigation, stats_before.mitigation);
    assert_eq!(
        inventory_after, inventory_before,
        "a reapplied choice would double the class kit"
    );
}

/// A `PlayerSave` written before character creation existed carries none of
/// the five new keys — reproduced by stripping them out of a fresh save's
/// own RON, `nemesis.rs`'s
/// `a_save_written_without_the_nemesis_field_loads_to_an_unmarked_creature`
/// precedent, rather than hand-maintaining a second save fixture.
#[test]
fn an_old_save_without_the_fields_still_loads() {
    let mut game = Game::new(90_012, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let path = save_path("no_fields");
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let fields = ["name:", "class:", "glyph:", "sprite:", "colour:"];
    for field in fields {
        assert!(
            text.contains(field),
            "a fresh save must carry `{field}`, or stripping it below proves nothing"
        );
    }
    // Scoped to the `player: (...)` block alone — RON's pretty-printer
    // omits struct names, and a bare `name:`/`class:` line-prefix match
    // also hits `ContractDef::name` and `systems::CycleModifiers::class`
    // elsewhere in the same file, so this finds the player block's own
    // matching close paren by depth rather than the next line that happens
    // to start with `)`.
    let marker = "player: (";
    let block_start = text.find(marker).expect("save must contain a player block");
    let open_paren = block_start + marker.len() - 1;
    let mut depth = 0i32;
    let mut block_end = open_paren;
    for (i, byte) in text.as_bytes()[open_paren..].iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    block_end = open_paren + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let cleaned_block: String = text[open_paren..=block_end]
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !fields.iter().any(|field| trimmed.starts_with(field))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let stripped = format!(
        "{}{cleaned_block}{}",
        &text[..open_paren],
        &text[block_end + 1..]
    );
    std::fs::write(&path, stripped).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let player = loaded.player_entity();
    assert_eq!(loaded.world.get::<Glyph>(player).unwrap().ch, '@');
    let identity = loaded.world.get::<PlayerIdentity>(player).unwrap();
    assert_eq!(identity.class, None);
    assert_eq!(identity.sprite, "");
    assert_eq!(identity.colour, None);
    assert!(
        loaded.world.get::<CustomName>(player).is_none(),
        "an absent name field must not install an empty override"
    );
}

/// The player's `EntityView.look` is `Some`; a wild creature standing right
/// beside it is `None` — `views::PlayerLook`'s whole reason to be an
/// `Option` rather than two bare fields on every row.
#[test]
fn the_player_view_carries_its_look_and_nothing_else_does() {
    let choice = CharacterChoice {
        sprite: "hero".to_string(),
        colour: Some(3),
        ..CharacterChoice::default()
    };
    let mut game = Game::new_with(
        90_013,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &choice,
    )
    .unwrap();
    let spawn = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.spawn_wild_creature("construct", spawn.x + 2, spawn.y)
        .unwrap();

    let views = game.view_entities(20, 20);
    let mut saw_player = false;
    for view in &views {
        if view.is_player {
            saw_player = true;
            let look = view
                .look
                .as_ref()
                .expect("the player's view must carry a look");
            assert_eq!(look.sprite, "hero");
            assert_eq!(look.colour, Some(3));
        } else {
            assert!(
                view.look.is_none(),
                "only the player's row may carry a look"
            );
        }
    }
    assert!(saw_player, "the player must appear in its own view");
}

/// **The anti-drift gate for the wizard's catalogue.** `CreationCatalogue`
/// exists because the wizard runs before any `Game` does, and the failure
/// it opens is a preview that disagrees with the run it is previewing. Both
/// halves are asserted against the real `assets/` — the class rows *and*
/// the class-priced starter rows — for every shipped class, because the
/// pricing is the half a second copy of the formula would silently get
/// wrong.
#[test]
fn the_creation_catalogue_agrees_with_the_game() {
    let catalogue = CreationCatalogue::load(&test_assets_dir()).unwrap();
    let game = Game::new(90_017, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let from_game = game.class_rows();
    let from_catalogue = catalogue.class_rows();
    assert!(
        !from_game.is_empty(),
        "the shipped assets must carry classes, or this test proves nothing"
    );
    assert_eq!(from_game.len(), from_catalogue.len());
    for (a, b) in from_game.iter().zip(from_catalogue.iter()) {
        assert_eq!(a.class, b.class);
        assert_eq!(a.name, b.name);
        assert_eq!(a.axes, b.axes, "the spread summary drifted");
        assert_eq!(a.kit, b.kit, "the kit summary drifted");
    }

    for class in AffinityClass::ALL.iter().copied().map(Some).chain([None]) {
        assert_eq!(
            game.starter_routine_rows(class),
            catalogue.starter_rows(class),
            "the starter pool priced differently for {class:?}"
        );
    }
}
