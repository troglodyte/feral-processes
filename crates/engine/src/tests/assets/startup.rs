use crate::tests::support::*;
use crate::*;

/// `build_radius_bonus` and `claims_ground` are gone from `StructureDef` —
/// deleted along with the Heaps that were their only shipped readers (Task
/// 6) and with `resources::Platform`, the last engine-side reader (this
/// task). Neither field's removal would fail a normal `ron::from_str` on a
/// file that still declares one: an unknown key is silently ignored rather
/// than refused, so a stray `build_radius_bonus: 4` left in a `.ron` would
/// parse clean and simply do nothing forever. A text census over the raw
/// shipped files, not a parsed-struct assertion, is the only thing that can
/// catch that.
#[test]
fn no_shipped_structure_declares_a_retired_platform_field() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/structures");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        checked += 1;
        for retired in ["build_radius_bonus", "claims_ground"] {
            assert!(
                !text.contains(retired),
                "{} still declares `{retired}`, a field deleted with resources::Platform — \
                 serde would silently ignore it rather than refuse the file",
                path.display()
            );
        }
    }
    assert!(
        checked > 0,
        "the census must actually walk assets/structures, or this passes vacuously"
    );
}

#[test]
fn game_new_aborts_startup_when_the_item_set_is_missing_the_currency_role() {
    // The economy can't run without a Currency-role item — see
    // `ItemDb::missing_roles` — so `Game::new` must abort before the
    // world is built rather than let play reach `Game::currency()`'s
    // `.expect("validated at startup")` deep in gameplay.
    let dir = assets_dir_missing_currency_item();
    let result = Game::new(900, DifficultyMode::Forgiving, &dir);
    let _ = std::fs::remove_dir_all(&dir);

    // `Game` isn't `Debug` (it wraps a `bevy_ecs::World`), so this can't
    // use `Result::expect_err` / `unwrap_err`.
    let Err(err) = result else {
        panic!("startup should abort rather than run with no item holding the Currency role");
    };
    assert!(
        err.to_string().contains("Currency"),
        "error should name the missing role: {err}"
    );
}

#[test]
fn game_load_aborts_when_the_item_set_is_missing_the_currency_role() {
    // Resuming a save is the other door into the same world, and it
    // reaches the same `Game::currency()` `.expect("validated at
    // startup")` — so an item set that lost its Currency-role holder
    // between saving and loading has to be refused here too, not only
    // in `Game::new`.
    let mut game = Game::new(902, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let path = std::env::temp_dir().join(format!(
        "feral_missing_currency_load_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let dir = assets_dir_missing_currency_item();
    let result = Game::load(&path, &dir);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&path);

    // `Game` isn't `Debug` (it wraps a `bevy_ecs::World`), so this can't
    // use `Result::expect_err` / `unwrap_err`.
    let Err(err) = result else {
        panic!("loading should abort rather than resume with no item holding the Currency role");
    };
    assert!(
        err.to_string().contains("Currency"),
        "error should name the missing role: {err}"
    );
}

/// The moddability guarantee for the two new grid fields: a structure file
/// written before they existed — the common case, since every shipped file
/// predates them too — must still parse, reading both as `0` (draws
/// nothing, supplies nothing) rather than failing to load.
#[test]
fn a_structure_file_without_the_power_fields_still_parses() {
    const NO_POWER_FIELDS_STRUCTURE: &str = r#"(
        id: "no_power_fields_structure",
        name: "No Power Fields Structure",
        glyph: '?',
        color: White,
        build_cost: [],
        work: None,
    )"#;
    let dir = assets_dir_with_extra_structure(
        "no_power_fields_structure",
        "no_power_fields_structure.ron",
        NO_POWER_FIELDS_STRUCTURE,
    );
    let game = Game::new(905, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    let db = game.world.resource::<crate::structures::StructureDb>();
    let def = db
        .get("no_power_fields_structure")
        .expect("the fixture structure loaded");

    assert_eq!(def.power_draw, 0, "an old-format file should draw nothing");
    assert_eq!(
        def.power_supply, 0,
        "an old-format file should supply nothing"
    );
}

/// The moddability guarantee for `StructureDef::turret`: a structure file
/// written before it existed — the common case, since every shipped file
/// but `turret.ron` predates it — must still parse, reading it as `None`
/// rather than failing to load.
#[test]
fn a_structure_file_without_a_turret_still_parses() {
    const NO_TURRET_STRUCTURE: &str = r#"(
        id: "no_turret_structure",
        name: "No Turret Structure",
        glyph: '?',
        color: White,
        build_cost: [],
        work: None,
    )"#;
    let dir = assets_dir_with_extra_structure(
        "no_turret_structure",
        "no_turret_structure.ron",
        NO_TURRET_STRUCTURE,
    );
    let game = Game::new(906, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    let db = game.world.resource::<crate::structures::StructureDb>();
    let def = db
        .get("no_turret_structure")
        .expect("the fixture structure loaded");

    assert!(
        def.turret.is_none(),
        "an old-format file should declare no turret"
    );
}

/// Every shipped file that declares `turret:` declares a legal one — the
/// `checked > 0` guard `no_shipped_structure_declares_a_retired_platform_field`
/// uses, so the census cannot pass by finding nothing.
#[test]
fn every_shipped_turret_declares_positive_damage_and_range() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/structures");
    let mut checked = 0;
    let mut turrets = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        checked += 1;
        let def: crate::structures::StructureDef =
            ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        if let Some(turret) = def.turret {
            turrets += 1;
            assert!(
                turret.damage > 0,
                "{} declares a turret with no damage",
                path.display()
            );
            assert!(
                turret.range > 0,
                "{} declares a turret with no range",
                path.display()
            );
        }
    }
    assert!(
        checked > 0,
        "the census must actually walk assets/structures, or this passes vacuously"
    );
    assert!(
        turrets > 0,
        "the census must find at least one shipped turret, or this passes vacuously"
    );
}

#[test]
fn every_shipped_asset_file_loads_without_a_warning() {
    // A malformed shipped asset is warn-and-skipped like a mod's would
    // be, so it costs the player content silently instead of failing the
    // build. This is the only thing that catches it — a serde attribute
    // missing from `ItemId` once made every asset load fail this way. The
    // whole warning list, not log lines matching one phrase: a dropped rider
    // or a dangling status id is worded differently and costs content too.
    let warnings = crate::game::lifecycle::load_asset_dbs(&test_assets_dir())
        .unwrap()
        .warnings;

    assert!(
        warnings.is_empty(),
        "shipped assets must load without a warning: {warnings:#?}"
    );
}
