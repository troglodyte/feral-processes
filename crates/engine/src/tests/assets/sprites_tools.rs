use crate::tests::support::*;
use crate::tools::ToolCategory;
use crate::*;

/// `SpeciesDef::sprite_name` and `StructureDef::sprite_name` are the one
/// place the "override or fall back to the id" convention is written — see
/// their doc comments. A reader that re-derives the fallback itself is a
/// second copy of the rule.
#[test]
fn species_sprite_name_falls_back_to_the_id_and_honours_an_override() {
    let game = Game::new(3401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .expect("the shipped roster is not empty");
    assert!(
        species.sprite.is_none(),
        "no shipped species authors a sprite override yet"
    );
    assert_eq!(species.sprite_name(), species.id.as_str());

    species.sprite = Some("custom_species_sprite".to_string());
    assert_eq!(species.sprite_name(), "custom_species_sprite");
}

/// `@` keys the runtime-only drawn-icon slot (`sprites::DRAWN_ICON_KEY`) the
/// player's own pixel editor registers under, never a file — an override
/// naming it must fall back to the id rather than being honoured, or a
/// modded species could draw the player's own drawn icon on its tile.
#[test]
fn species_sprite_name_falls_back_to_the_id_when_the_override_names_the_drawn_icon_slot() {
    let game = Game::new(3406, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .expect("the shipped roster is not empty");

    species.sprite = Some("@drawn".to_string());

    assert_eq!(species.sprite_name(), species.id.as_str());
}

#[test]
fn structure_sprite_name_falls_back_to_the_id_and_honours_an_override() {
    let game = Game::new(3402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Not `.next()`: the Home sorts first by `category()` alone (its own
    // `StructureCategory::Home`) and, as of the Home's own `sprite: Some
    // ("anchor")`, is no longer one of the un-overridden majority this test
    // means to exercise — `.find` reaches past it to a def that still
    // relies on the id-fallback convention.
    let mut structure = game
        .structure_defs()
        .into_iter()
        .find(|s| s.sprite.is_none())
        .expect("some shipped structure still relies on the id-fallback convention");
    assert_eq!(structure.sprite_name(), structure.id.as_str());

    structure.sprite = Some("custom_structure_sprite".to_string());
    assert_eq!(structure.sprite_name(), "custom_structure_sprite");
}

/// The structure half of
/// `species_sprite_name_falls_back_to_the_id_when_the_override_names_the_drawn_icon_slot`.
#[test]
fn structure_sprite_name_falls_back_to_the_id_when_the_override_names_the_drawn_icon_slot() {
    let game = Game::new(3407, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut structure = game
        .structure_defs()
        .into_iter()
        .next()
        .expect("the shipped structure catalogue is not empty");

    structure.sprite = Some("@drawn".to_string());

    assert_eq!(structure.sprite_name(), structure.id.as_str());
}

/// A species or structure file authored before this field existed — every
/// shipped one, today — has no `sprite:` key at all, and `#[serde(default)]`
/// is what keeps that parsing rather than refusing the file.
#[test]
fn a_def_file_without_a_sprite_key_still_parses() {
    const NO_SPRITE_SPECIES: &str = r#"(
    id: "no_sprite_key_species",
    name: "No Sprite Key Species",
    glyph: 'x',
    color: White,
    base_hp: 10,
    base_atk: 1,
    base_mitigation: 0,
    taming_difficulty: 0.1,
    habitats: [OpenGrid],
    moves: [(name: "Poke", power: 1)],
    work_resource: None,
)"#;
    let species_dir = modded_assets_dir(
        "no_sprite_key_species",
        &[],
        &[],
        &[("no_sprite_key_species.ron", NO_SPRITE_SPECIES)],
        &[],
        &[],
    );
    let game = Game::new(3403, DifficultyMode::Forgiving, &species_dir).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| s.id == "no_sprite_key_species")
        .expect("the fixture species loaded");
    assert_eq!(species.sprite, None);
    assert_eq!(species.sprite_name(), "no_sprite_key_species");

    const NO_SPRITE_STRUCTURE: &str = r#"(
        id: "no_sprite_key_structure",
        name: "No Sprite Key Structure",
        glyph: '?',
        color: White,
        build_cost: [],
        work: None,
    )"#;
    let structure_dir = assets_dir_with_extra_structure(
        "no_sprite_key_structure",
        "no_sprite_key_structure.ron",
        NO_SPRITE_STRUCTURE,
    );
    let game = Game::new(3404, DifficultyMode::Forgiving, &structure_dir).unwrap();
    let structure = game
        .structure_defs()
        .into_iter()
        .find(|s| s.id == "no_sprite_key_structure")
        .expect("the fixture structure loaded");
    assert_eq!(structure.sprite, None);
    assert_eq!(structure.sprite_name(), "no_sprite_key_structure");
}

/// The census the spec is built around: every shipped species or structure
/// that authors a `sprite:` override must resolve to a real file under
/// `assets/sprites/`. A def with no override is skipped entirely — a
/// convention-named sprite (`sprite_name()` falling back to the id) that has
/// no file on disk is the ordinary, silent case (`Painter::sprite` draws the
/// glyph instead), and this census must not flag it.
///
/// Not vacuous: `depot_mk2` through `depot_mk6` override to `"depot"` and
/// the Home overrides to `"anchor"` (see `assets/structures/home.ron`), so
/// the structure loop below does walk real overrides against real files —
/// see the task report for how the failure branch was confirmed by hand
/// before those existed, since a census nobody can make fail is not a
/// census.
#[test]
fn every_shipped_sprite_override_resolves_to_a_real_file() {
    let game = Game::new(3405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let sprites_dir = test_assets_dir().join("sprites");

    let species = game.species_defs();
    let structures = game.structure_defs();
    assert!(
        !species.is_empty() && !structures.is_empty(),
        "the census walked no defs at all"
    );

    for def in &species {
        let Some(name) = &def.sprite else { continue };
        let path = sprites_dir.join(format!("{name}.png"));
        let colour = sprites_dir.join(format!("{name}{}.png", crate::icon::FULL_COLOUR_SUFFIX));
        assert!(
            path.exists() || colour.exists(),
            "species {:?} overrides sprite to {:?}, missing at {}",
            def.id,
            name,
            path.display()
        );
    }
    for def in &structures {
        let Some(name) = &def.sprite else { continue };
        let path = sprites_dir.join(format!("{name}.png"));
        let colour = sprites_dir.join(format!("{name}{}.png", crate::icon::FULL_COLOUR_SUFFIX));
        assert!(
            path.exists() || colour.exists(),
            "structure {:?} overrides sprite to {:?}, missing at {}",
            def.id,
            name,
            path.display()
        );
    }
}

/// Every shipped tool's `yields` names a real, loaded item — the same
/// "every reference resolves" rule `the_shipped_species_kits_reference_only_real_abilities`
/// holds abilities to, over `tools::ToolDef::yields` instead.
#[test]
fn every_shipped_tools_yields_resolve_to_real_items() {
    let game = Game::new(4102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    let tools = game.world.resource::<ToolDb>();

    let mut checked = 0;
    for def in tools.all() {
        for (item, _) in &def.yields {
            assert!(
                items.get(item.as_str()).is_some(),
                "tool {:?} yields unknown item {:?}",
                def.id,
                item
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "the census walked no yield entries at all");
}

/// Every non-`Routines` tool declares at least one thing to draw from —
/// an empty pool on any other category is a tool that does nothing when
/// used. See `tools::ToolCategory`'s own doc and `assets/tools/README.md`.
///
/// This passed vacuously through phases 1 and 2, when no `Routines` tool
/// shipped and every tool took the asserted branch. The Routine Reader
/// ships as of phase 3, so the exclusion is now walked for real and a
/// `yields` pool authored onto it would be caught here.
#[test]
fn every_non_routines_tool_has_a_non_empty_yield_pool() {
    let game = Game::new(4103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tools = game.world.resource::<ToolDb>();

    let mut checked = 0;
    for def in tools.all() {
        // Exhaustive, `render/stack.rs`'s `cell_mark` rule: a fifth
        // category shipping tomorrow must pick a side of this match rather
        // than falling through a `_` arm unnoticed.
        match def.category {
            ToolCategory::Materials | ToolCategory::Parts | ToolCategory::Cores => {
                assert!(
                    !def.yields.is_empty(),
                    "tool {:?} is category {:?} and must declare a non-empty yields pool",
                    def.id,
                    def.category
                );
                checked += 1;
            }
            ToolCategory::Routines => {
                assert!(
                    def.yields.is_empty(),
                    "tool {:?} is category Routines and must not declare a yields pool — it \
                     takes the routine branch instead",
                    def.id
                );
            }
            ToolCategory::Gear => {
                assert!(
                    def.yields.is_empty(),
                    "tool {:?} is category Gear and must not declare a yields pool — it rolls \
                     the species' own drop table instead",
                    def.id
                );
            }
            ToolCategory::Image => {
                assert!(
                    def.yields.is_empty(),
                    "tool {:?} is category Image and must not declare a yields pool — it \
                     teaches the downed program's species instead",
                    def.id
                );
            }
        }
    }
    assert!(
        checked > 0,
        "the census walked no non-Routines tools at all"
    );
}

/// Every shipped tool's `yields` weight is a finite, positive number — the
/// same shape `weighted_pick` (and the extraction draw a later phase builds
/// on it) needs to make sense of a pool at all. `ToolDb::load_dir` already
/// refuses this at load, so this census is a second, independent check over
/// the files actually shipped, the way `every_shipped_power_cost_is_finite_
/// and_non_negative` doubles `AbilityDef::non_finite_field`.
#[test]
fn every_shipped_tools_yield_weight_is_finite_and_positive() {
    let game = Game::new(4104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tools = game.world.resource::<ToolDb>();

    let mut checked = 0;
    for def in tools.all() {
        for (item, weight) in &def.yields {
            assert!(
                weight.is_finite() && *weight > 0.0,
                "tool {:?} yield {:?} has weight {weight}, which must be finite and positive",
                def.id,
                item
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "the census walked no yield weights at all");
}

/// `tuning::STARTER_TOOL_ID` must name a tool that actually shipped — the
/// same floor `game_new_aborts_startup_when_the_item_set_is_missing_the_currency_role`
/// holds an economy role to, except a missing starter tool does not abort
/// startup today (nothing forges it into a slot yet — that door is a later
/// phase), so this census is what would have caught a typo before that door
/// opens.
#[test]
fn starter_tool_id_resolves_to_a_shipped_tool() {
    let game = Game::new(4105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tools = game.world.resource::<ToolDb>();
    assert!(
        tools.get(crate::tuning::STARTER_TOOL_ID).is_some(),
        "tuning::STARTER_TOOL_ID {:?} does not name a shipped tool",
        crate::tuning::STARTER_TOOL_ID
    );
}

/// No two shipped tool files collide on `id` — `ToolDb::load_dir` keys a
/// `HashMap` by id, so a duplicate silently loses one file rather than
/// warning, and this is the only thing that would notice. Counts `.ron`
/// files on disk against loaded tools rather than asserting a literal
/// number, so it doesn't need editing every time a tool ships.
#[test]
fn every_shipped_tool_id_is_unique() {
    let game = Game::new(4106, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tools = game.world.resource::<ToolDb>();
    let loaded = tools.all().count();

    let on_disk = std::fs::read_dir(test_assets_dir().join("tools"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|e| e.to_str()) == Some("ron"))
        .count();

    assert!(on_disk > 0, "the census found no shipped tool files at all");
    assert_eq!(
        loaded, on_disk,
        "loaded {loaded} tools against {on_disk} .ron files — a duplicated id silently \
         dropped one"
    );
}

/// Every `unlocks_tools` id across the shipped research `.ron` files names
/// a real, loaded tool — `every_shipped_tools_yields_resolve_to_real_items`'s
/// own rule, turned on the other reference a research node can make to the
/// tool catalogue.
///
/// Parses `assets/research/*.ron` directly rather than reading a loaded
/// `ResearchDb`: `ResearchDb::load_dir` already retains an unknown
/// `unlocks_tools` id away at load time and keeps the node (task 1's own
/// rule), so a census over the *loaded* db would only ever see survivors —
/// a typo could never make it fail. `every_shipped_tool_id_is_unique`'s own
/// precedent for reading the shipped tree off disk instead of through the
/// loader.
#[test]
fn every_researched_tool_unlock_resolves_to_a_shipped_tool() {
    let (tools, tool_warnings) = ToolDb::load_dir(&test_assets_dir().join("tools")).unwrap();
    assert!(
        tool_warnings.is_empty(),
        "shipped tool files must load clean: {tool_warnings:?}"
    );

    let mut checked = 0;
    for entry in std::fs::read_dir(test_assets_dir().join("research")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let def: crate::research::ResearchDef = ron::from_str(&text)
            .unwrap_or_else(|e| panic!("{path:?} must parse as a ResearchDef: {e}"));
        for tool in &def.unlocks_tools {
            assert!(
                tools.get(tool.as_str()).is_some(),
                "{:?} unlocks unknown tool {:?}",
                path,
                tool
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "the census walked no unlocks_tools entries at all"
    );
}

/// Every shipped tool's `forge_cost` names a real, loaded item —
/// `every_shipped_tools_yields_resolve_to_real_items`'s rule again, over the
/// other item reference a tool def carries.
#[test]
fn every_shipped_tool_forge_cost_resolves_to_a_real_item() {
    let game = Game::new(4108, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    let tools = game.world.resource::<ToolDb>();

    let mut checked = 0;
    for def in tools.all() {
        for (item, _) in &def.forge_cost {
            assert!(
                items.get(item.as_str()).is_some(),
                "tool {:?} forge_cost names unknown item {:?}",
                def.id,
                item
            );
            checked += 1;
        }
    }
    assert!(
        checked > 0,
        "the census walked no forge_cost entries at all"
    );
}

/// A tool nothing grants is content nobody can ever forge —
/// `every_shipped_field_routine_can_actually_be_obtained`'s own rule, ported
/// to tools. Every shipped tool other than `STARTER_TOOL_ID` (granted
/// straight into the first slot at `Game::new`, no research required) must
/// be named by some research node's `unlocks_tools`, or it ships with no
/// door into the game at all.
#[test]
fn every_shipped_tool_can_actually_be_obtained() {
    let game = Game::new(4109, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let research = game.world.resource::<crate::research::ResearchDb>();
    let tools = game.world.resource::<ToolDb>();

    let granted: std::collections::HashSet<&str> = research
        .all()
        .flat_map(|node| node.unlocks_tools.iter())
        .map(|id| id.as_str())
        .collect();

    let unreachable: Vec<&str> = tools
        .all()
        .map(|def| def.id.as_str())
        .filter(|id| *id != crate::tuning::STARTER_TOOL_ID)
        .filter(|id| !granted.contains(id))
        .collect();

    assert!(
        unreachable.is_empty(),
        "these tools are in no research node's unlocks_tools and are not the starter tool, \
         so nothing in a real game can ever hand them over: {unreachable:?}"
    );
}

/// The `Routines` branch is unreachable content until some shipped tool is
/// in that category — `every_non_routines_tool_has_a_non_empty_yield_pool`
/// only says what a `Routines` tool is *exempt* from, never that one exists.
#[test]
fn a_shipped_tool_reads_routines() {
    let game = Game::new(4110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tools = game.world.resource::<ToolDb>();
    assert!(
        tools
            .all()
            .any(|def| def.category == ToolCategory::Routines),
        "no shipped tool takes the routine branch"
    );
}

/// A `Gear` category with no tool in it would ship the whole third branch
/// as unreachable content — `every_non_routines_tool_has_a_non_empty_yield_pool`
/// only says what a `Gear` tool is *exempt* from, never that one exists.
#[test]
fn a_shipped_tool_pulls_gear() {
    let game = Game::new(4210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tools = game.world.resource::<ToolDb>();
    assert!(
        tools.all().any(|def| def.category == ToolCategory::Gear),
        "no shipped tool takes the gear branch"
    );
}

/// A `Routines` tool reads no yield pool at all, so a populated one is
/// authored content that silently never runs. Stated over the `Routines`
/// tools alone, rather than as the exclusion arm of the every-category
/// census above: that one fails on the *other* categories too, and a
/// reviewer reading its failure has to work out which half broke.
#[test]
fn a_routines_tool_ships_an_empty_yield_pool() {
    let game = Game::new(4111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tools = game.world.resource::<ToolDb>();
    for def in tools.all().filter(|d| d.category == ToolCategory::Routines) {
        assert!(
            def.yields.is_empty(),
            "{} is a Routines tool with a yield pool that will never be read",
            def.id
        );
    }
}

/// An `Image` category with no tool in it would ship the whole fourth
/// branch as unreachable content (todo #100 Task 5) —
/// `every_non_routines_tool_has_a_non_empty_yield_pool` only says what an
/// `Image` tool is *exempt* from, never that one exists.
#[test]
fn a_shipped_tool_captures_an_image() {
    let game = Game::new(4211, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tools = game.world.resource::<ToolDb>();
    assert!(
        tools.all().any(|def| def.category == ToolCategory::Image),
        "no shipped tool takes the image branch"
    );
}

/// An `Image` tool reads no yield pool at all — `a_routines_tool_ships_an_
/// empty_yield_pool`'s own reason, isolated to its own category so a
/// reviewer reading a failure knows which half broke.
#[test]
fn an_image_tool_ships_an_empty_yield_pool() {
    let game = Game::new(4212, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tools = game.world.resource::<ToolDb>();
    for def in tools.all().filter(|d| d.category == ToolCategory::Image) {
        assert!(
            def.yields.is_empty(),
            "{} is an Image tool with a yield pool that will never be read",
            def.id
        );
    }
}

/// Every shipped species' `Game::rich_in` — authored, or `work_resource`
/// fallen back to — names a real, loaded item. Spec section 7's own census:
/// no shipped species authors `rich_in` yet (decision 5 is that none had
/// to), so this walks the fallback path today and starts checking an
/// authored value for real the day one ships, `every_non_routines_tool_
/// has_a_non_empty_yield_pool`'s own precedent for a census that currently
/// only exercises one of its branches.
#[test]
fn every_species_rich_in_resolves_to_a_real_item() {
    let game = Game::new(4107, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.world.resource::<SpeciesDb>();
    let items = game.world.resource::<crate::items_db::ItemDb>();

    let mut checked = 0;
    for def in species.all() {
        let Some(item) = game.rich_in(&def.id) else {
            continue;
        };
        assert!(
            items.get(item.as_str()).is_some(),
            "species {:?} rich_in (or its work_resource fallback) names unknown item {:?}",
            def.id,
            item
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "the census walked no species with a rich_in at all"
    );
}

/// Every `SettlementKind`, `Specialty` and `Temperament` variant is authored
/// by at least one shipped town.
///
/// **The census `settlements/mod.rs` cites, written.** `Temperament`'s doc
/// comment named `every_temperament_is_authored_somewhere` as what keeps an
/// unreachable variant from shipping, and no such test existed — the
/// catalogue happened to author all three, so the claim was true and
/// unheld. Widened to all three axes, since a `Specialty` nothing authors
/// means `specialty_weights` has an arm no shelf is ever drawn through, and
/// an unauthored `SettlementKind` means one of `settlement_rows`' two row
/// counts is dead.
///
/// `cell_mark`'s rule turned outward: the matches are already exhaustive, so
/// a new variant fails to compile in the *engine*. What it cannot catch is a
/// variant that compiles everywhere and is authored nowhere, which ships as
/// content that exists only in Rust.
#[test]
fn every_settlement_shape_is_authored_somewhere() {
    use crate::settlements::{SettlementDb, SettlementKind, Specialty, Temperament};

    let (db, warnings) = SettlementDb::load_dir(&test_assets_dir().join("settlements"))
        .expect("the catalogue loads");
    assert!(
        warnings.is_empty(),
        "shipped settlements warned: {warnings:?}"
    );
    let defs: Vec<_> = db.iter().collect();
    assert!(!defs.is_empty(), "the census walked an empty catalogue");

    for kind in [SettlementKind::Mainframe, SettlementKind::Server] {
        assert!(
            defs.iter().any(|d| d.kind == kind),
            "no shipped settlement is a {kind:?}, so one of `settlement_rows`' row counts is \
             unreachable content"
        );
    }
    for specialty in [
        Specialty::Gear,
        Specialty::Materials,
        Specialty::Routines,
        Specialty::Programs,
    ] {
        assert!(
            defs.iter().any(|d| d.specialty == specialty),
            "no shipped settlement specializes in {specialty:?}, so `specialty_weights` has an \
             arm no shelf is ever drawn through"
        );
    }
    for temperament in [
        Temperament::Open,
        Temperament::Guarded,
        Temperament::Mercantile,
    ] {
        assert!(
            defs.iter().any(|d| d.temperament == temperament),
            "no shipped settlement is {temperament:?}, so its `buy_mult`/`sell_mult` pair prices \
             nothing"
        );
    }
}
