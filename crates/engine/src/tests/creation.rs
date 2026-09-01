//! `CharacterChoice` and `Game::new_with` — the foundation the rest of the
//! character-creation feature builds on. `new_and_new_with_default_produce_
//! the_same_player` is the load-bearing one: it is what protects the
//! ~1,600 `Game::new` call sites across the suite from a regression here.

use super::support::*;
use crate::achievements::MainStat;
use crate::classes::PlayerClass;
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

/// The default choice is today's player, and the sprite is the half of
/// that which nothing else asserts: a run created with no wizard at all
/// must still carry the name `assets/sprites/player.png` is loaded under,
/// or the map falls back to a bare `@` for every existing save, every
/// `dev-saves/` template, and anyone who skips the Look step.
#[test]
fn the_default_choice_keeps_the_shipped_player_sprite() {
    let game = Game::new(90_014, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let identity = game
        .world
        .get::<PlayerIdentity>(game.player_entity())
        .unwrap();
    assert_eq!(identity.sprite, DEFAULT_PLAYER_SPRITE);
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

/// **Def is priced like the other three axes now**, and this is what says
/// the whole pool put on it actually reaches `Stats::mitigation`.
///
/// It replaces `mitigation_costs_more_than_a_point`, which held the axis at
/// `CREATION_COST_DEF = 3` on the argument that pricing it flat would make
/// it dominant. The instrument said otherwise — a unit is one percentage
/// point, and `docs/measurements/2026-09-01-creation-stat-pool-exchange-
/// rates.md` measured the whole pool spent on Def as byte-identical to
/// spending nothing.
///
/// The ceiling is asserted alongside, because that is the bound a pool
/// retune could actually cross: mitigation is capped at
/// `MAX_MITIGATION_PERCENT` in the damage path, and creation must not be
/// able to open a run anywhere near it.
#[test]
fn the_whole_pool_on_def_reaches_mitigation() {
    let pool = tuning::CREATION_STAT_POINTS;
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

    assert_eq!(
        gained, units as i32,
        "every unit bought is a point of mitigation"
    );
    assert!(
        stats.mitigation < tuning::MAX_MITIGATION_PERCENT / 2,
        "a creation spend opens on {} mitigation against a {} cap — the pool \
         has outgrown what this axis may be allowed to buy",
        stats.mitigation,
        tuning::MAX_MITIGATION_PERCENT
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
        class: Some(PlayerClass::Saboteur),
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
    assert_eq!(identity.class, Some(PlayerClass::Saboteur));
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
        class: Some(PlayerClass::Striker),
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
    assert_eq!(
        identity.sprite, DEFAULT_PLAYER_SPRITE,
        "a save with no sprite key must still draw the shipped player art, \
         not the empty name the renderer reads as no sprite"
    );
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
        assert_eq!(a.trade, b.trade, "the trade summary drifted");
    }

    for class in PlayerClass::ALL.iter().copied().map(Some).chain([None]) {
        assert_eq!(
            game.starter_routine_rows(class),
            catalogue.starter_rows(class),
            "the starter pool priced differently for {class:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The starting-kit step. `creation_shelf` is the one derivation of what may
// be picked; `items` non-empty is what replaces the class kit.
// ---------------------------------------------------------------------------

/// The shelf as the wizard and the run both see it, off the real assets.
fn shelf() -> Vec<views::StartingItemRow> {
    let game = Game::new(90_101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.creation_shelf_rows()
}

fn carried(game: &Game, id: &str) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&ItemId::from(id))
}

fn game_with_items(seed: u32, items: Vec<(ItemId, u32)>) -> Game {
    Game::new_with(
        seed,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &CharacterChoice {
            items,
            ..Default::default()
        },
    )
    .unwrap()
}

/// The three exclusions, each for its own reason: Credits are the allowance
/// itself, Portal Fragments are what the Stack exists to pay you, and a
/// banked item is not cargo. `EconomyRole::Currency` — Core Fragments —
/// deliberately stays, because all five shipped class kits open with 4-6 of
/// them and this shelf *replaces* that kit.
#[test]
fn the_shelf_offers_no_currency_the_run_is_meant_to_earn() {
    let rows = shelf();
    assert!(!rows.is_empty(), "the shipped item set stocked no shelf");
    for banned in [ids::CREDITS, ids::PORTAL_FRAGMENT, ids::RESEARCH_DATA] {
        assert!(
            !rows.iter().any(|r| r.id.as_str() == banned),
            "{banned} is on the creation shelf: {:?}",
            rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>()
        );
    }
    assert!(
        rows.iter().any(|r| r.id.as_str() == ids::CORE_FRAGMENT),
        "Core Fragments must stay on the shelf — every class kit opens with them"
    );
}

/// The ceiling is what keeps the step inside the wizard's no-scroll
/// promise, so it is a property of the shelf and not of the screen alone.
#[test]
fn the_shelf_offers_nothing_above_its_ceiling() {
    for row in shelf() {
        assert!(
            row.price <= tuning::CREATION_SHELF_MAX_VALUE,
            "{} is priced {} over the {} ceiling",
            row.id.as_str(),
            row.price,
            tuning::CREATION_SHELF_MAX_VALUE
        );
        assert!(row.price > 0, "{} is free", row.id.as_str());
    }
}

/// Cheapest first, then by id. `ItemDb` keys by `String` in a `HashMap`, so
/// without this the wizard's rows — and the digit shortcuts over them —
/// would land in a different order every run.
#[test]
fn the_shelf_is_ordered_and_bounded() {
    let rows = shelf();
    let keys: Vec<(u32, &str)> = rows.iter().map(|r| (r.price, r.id.as_str())).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "the shelf is not in (price, id) order");
    assert!(
        rows.len() <= tuning::CREATION_SHELF_ROWS,
        "{} rows over the {} cap",
        rows.len(),
        tuning::CREATION_SHELF_ROWS
    );
}

/// The whole point of the step: what you pick is what you start with, and
/// the class kit does not arrive alongside it.
#[test]
fn a_picked_kit_replaces_the_class_kit() {
    let game = game_with_items(90_102, vec![(ids::POWER_CELL.into(), 4)]);
    assert_eq!(carried(&game, ids::POWER_CELL), 4);
    // The `None`-class fallback kit's own four items, none of which were
    // picked. Power Cell is in that kit too, which is why it is asserted
    // as an exact 4 above rather than merely present.
    assert_eq!(carried(&game, ids::ICE_BREAKER), 0);
    assert_eq!(carried(&game, ids::CORE_FRAGMENT), 0);
    assert_eq!(carried(&game, ids::OUTLET), 0);
}

/// Walking the step without spending anything is not a naked run — it is
/// the pre-wizard game. This is the half that keeps
/// `CharacterChoice::default()` producing today's player and an empty
/// `assets/classes/` a supported install.
#[test]
fn an_empty_basket_keeps_the_class_kit() {
    let game = game_with_items(90_103, vec![]);
    assert_eq!(carried(&game, ids::ICE_BREAKER), 3);
    assert_eq!(carried(&game, ids::POWER_CELL), 3);
    assert_eq!(carried(&game, ids::CORE_FRAGMENT), 5);
    assert_eq!(carried(&game, ids::OUTLET), 2);
}

/// What the basket did not spend arrives as Credits — and only when the
/// basket was used at all, or today's kitted player would silently gain an
/// allowance they never chose.
#[test]
fn the_unspent_allowance_arrives_as_credits() {
    let picked = game_with_items(90_104, vec![(ids::POWER_CELL.into(), 4)]);
    assert_eq!(
        carried(&picked, ids::CREDITS),
        tuning::CREATION_CREDITS - 4,
        "Power Cell is 1 Credit, so four of them leave the rest"
    );

    let untouched = game_with_items(90_105, vec![]);
    assert_eq!(
        carried(&untouched, ids::CREDITS),
        0,
        "an untouched basket must not hand out the allowance"
    );
}

/// `apply_creation_stats`' rule on the other axis: an overspend applies
/// nothing rather than a clamped basket, and falls back to the class kit so
/// the run still starts equipped.
#[test]
fn an_overspent_basket_falls_back_to_the_kit() {
    let game = game_with_items(
        90_106,
        vec![(ids::POWER_CELL.into(), tuning::CREATION_CREDITS + 1)],
    );
    assert_eq!(carried(&game, ids::POWER_CELL), 3, "the kit's own three");
    assert_eq!(carried(&game, ids::CORE_FRAGMENT), 5);
    assert_eq!(carried(&game, ids::CREDITS), 0);
}

/// A pick naming an item the shelf does not offer is refused the same way
/// an overspend is — the shelf is the rule, and a hand-built
/// `CharacterChoice` must not be a way around it.
#[test]
fn a_basket_off_the_shelf_falls_back_to_the_kit() {
    let game = game_with_items(90_107, vec![(ids::PORTAL_FRAGMENT.into(), 1)]);
    assert_eq!(carried(&game, ids::PORTAL_FRAGMENT), 0);
    assert_eq!(
        carried(&game, ids::CORE_FRAGMENT),
        5,
        "the kit arrived instead"
    );
}

/// The wizard prices a shelf before any `Game` exists, so the catalogue and
/// the run must be the same derivation — a preview that disagreed with what
/// the run granted would be worse than no preview.
#[test]
fn the_catalogue_and_the_run_offer_the_same_shelf() {
    let catalogue = CreationCatalogue::load(&test_assets_dir()).unwrap();
    let rows = catalogue.shelf_rows();
    let expected = shelf();
    assert_eq!(rows.len(), expected.len());
    for (a, b) in rows.iter().zip(expected.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.price, b.price);
        assert_eq!(a.name, b.name);
    }
}

/// **The manifest is where the name and the class the player chose are read
/// back**, and neither had a reader before this: the sheet said "You" and
/// named no class, so a run's two most identifying facts were visible only
/// on the wizard screen that asked for them and in the save list.
///
/// Both halves are asserted against a *classless, nameless* control in the
/// same test — the default `CharacterChoice`, which is what every one of
/// the ~1,600 `Game::new` call sites builds and what every save from
/// before creation shipped carries. That arm is the one that would break
/// if the fallback were dropped.
#[test]
fn the_manifest_reads_back_the_name_and_class() {
    let choice = CharacterChoice {
        name: "Kestrel".to_string(),
        class: Some(PlayerClass::Leech),
        ..CharacterChoice::default()
    };
    let game = Game::new_with(
        90_010,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &choice,
    )
    .unwrap();
    let view = game.manifest(game.player_entity()).expect("a player sheet");
    assert_eq!(view.name, "Kestrel");
    let ManifestSubject::Player(player) = &view.subject else {
        panic!("the player's own entity produced a program sheet");
    };
    let class = player.class.as_ref().expect("the chosen class resolved");
    assert!(!class.name.is_empty(), "the class had no display name");
    assert!(
        !class.bonuses.is_empty(),
        "a class with a spread must say what it is worth: {class:?}"
    );

    // The classless, nameless run every `Game::new` produces.
    let plain = Game::new(90_011, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let view = plain
        .manifest(plain.player_entity())
        .expect("a player sheet");
    assert_eq!(view.name, "You", "a nameless run still reads as You");
    let ManifestSubject::Player(player) = &view.subject else {
        panic!("the player's own entity produced a program sheet");
    };
    assert!(player.class.is_none(), "a classless run must name no class");
}

/// **The perk basket is applied by replaying the purchase**, which is the
/// one thing this feature could get wrong invisibly: three of the nineteen
/// perks grant a `Stats` gain at purchase (`perks::purchase_stat_gain`),
/// and a hand-built `Perks` component would ship those three doing nothing
/// while every other perk looked fine.
///
/// So the perk bought here is a `StatGain` one, and the assertion is on
/// `Stats` rather than on the unlocked list.
#[test]
fn a_creation_perk_is_bought_through_the_same_door_the_run_uses() {
    let plain = Game::new(90_020, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let before = *plain.world.get::<Stats>(plain.player_entity()).unwrap();

    let choice = CharacterChoice {
        perks: vec![(crate::perks::Perk::Buffer, 1)],
        ..CharacterChoice::at_creation()
    };
    let game = Game::new_with(
        90_020,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &choice,
    )
    .unwrap();
    let player = game.player_entity();
    let perks = game.world.get::<Perks>(player).expect("a perks component");
    assert_eq!(perks.level(crate::perks::Perk::Buffer), 1);
    // **What the screen did not spend arrives with the run** — the one
    // creation allowance that survives the door, and the reason this step
    // is not a gate like the other two.
    let cost = game
        .perk_defs()
        .into_iter()
        .find(|def| def.id == crate::perks::Perk::Buffer)
        .expect("a shipped perk")
        .cost;
    assert_eq!(perks.points, tuning::CREATION_PERK_POINTS - cost);

    let after = *game.world.get::<Stats>(player).unwrap();
    assert!(
        after.max_hp > before.max_hp,
        "a StatGain perk bought at creation must reach Stats: {} vs {}",
        after.max_hp,
        before.max_hp
    );
    assert_eq!(after.hp, after.max_hp, "a run must not start damaged");
}

/// **An overspent basket buys nothing**, `apply_creation_stats`' rule on
/// the third budget — and the allowance is granted all the same, since it
/// is not what was overspent.
///
/// The allowance riding `CharacterChoice::perk_points` is what keeps that
/// safe: `Game::new` builds `default()`, whose allowance is zero, so none
/// of its ~1,600 call sites opens holding points —
/// `attention::unspent_perk_points_ask_to_be_spent` would read that as a
/// run needing the player, and `a_calm_base_needs_nothing` is what caught
/// it when the constant was read here directly.
#[test]
fn an_overspent_perk_basket_applies_nothing() {
    let choice = CharacterChoice {
        // Four levels of a 3-point perk is 12 against an allowance of 4.
        perks: vec![(crate::perks::Perk::Buffer, 4)],
        ..CharacterChoice::at_creation()
    };
    let game = Game::new_with(
        90_021,
        DifficultyMode::Forgiving,
        &test_assets_dir(),
        &choice,
    )
    .unwrap();
    let perks = game
        .world
        .get::<Perks>(game.player_entity())
        .expect("a perks component");
    assert_eq!(perks.level(crate::perks::Perk::Buffer), 0);
    assert_eq!(
        perks.points,
        tuning::CREATION_PERK_POINTS,
        "the allowance is not what was overspent, so it is still granted"
    );
}

/// **A class row is a sentence a player can act on**, not a row of sigils.
/// It read `"+Damage  -Healing"`, which needs the reader to already know
/// the game has five affinity axes and that a class raises one by trading
/// another away — on the second screen of a new game, before they have
/// seen a fight.
///
/// Asserted over the *shipped* classes, so a mod's spread is free to
/// produce a shape this does not describe, and the words are read out of
/// `AffinityKind::label` rather than restated here.
#[test]
fn a_class_row_says_what_it_trades_in_words() {
    let game = Game::new(90_030, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let rows = game.class_rows();
    assert!(!rows.is_empty(), "the shipped classes loaded");
    for row in rows {
        assert!(
            row.trade.starts_with("Bonus to ") && row.trade.contains(" at the expense of "),
            "{} reads {:?}, which is not a sentence",
            row.name,
            row.trade
        );
        assert!(
            !row.trade.contains('+') && !row.trade.contains('-'),
            "{} still carries a sigil: {:?}",
            row.name,
            row.trade
        );
    }
}
