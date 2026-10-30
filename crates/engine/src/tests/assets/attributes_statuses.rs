use super::zones_weapons::ZONE_MATERIALS;
use crate::tests::support::*;
use crate::*;

/// Every shipped attribute authors all seven fields, and the two prose
/// fields are actually prose.
///
/// The cost of the catalogue being data, paid: nothing in Rust enumerates
/// the shipped five, so without this an attribute could ship with an empty
/// `meaning` and the dossier page would draw a blank line under it.
#[test]
fn every_shipped_attribute_says_what_it_means() {
    let game = Game::new(905, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let defs = game.attribute_defs();
    assert_eq!(defs.len(), 6, "the shipped catalogue is six attributes");
    for def in &defs {
        assert!(!def.name.is_empty(), "{} has no name", def.id);
        assert!(!def.legacy.is_empty(), "{} has no legacy name", def.id);
        assert!(
            def.short.len() >= 10,
            "{}'s gloss is too short to say anything: {:?}",
            def.id,
            def.short
        );
        assert!(
            def.meaning.split_whitespace().count() >= 15,
            "{}'s meaning is a phrase, not prose: {:?}",
            def.id,
            def.meaning
        );
        assert!(
            def.spread < def.base,
            "{}: a spread at or above the base can mint a negative attribute",
            def.id
        );
    }
}

/// The gloss may not promise a mechanic. Decision 2 of the spec: the
/// sentence saying what an attribute *does* lands with the mechanic that
/// does it, and until then a claim the player tests and finds false is
/// worse than no claim.
#[test]
fn no_shipped_attribute_claims_an_effect() {
    let game = Game::new(906, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.attribute_defs() {
        let prose = format!("{} {}", def.short, def.meaning).to_lowercase();
        for claim in [
            "increases",
            "reduces",
            "improves",
            "raises",
            "lowers",
            "bonus",
            "chance to",
            "per point",
            "rises over",
            "grows over",
        ] {
            assert!(
                !prose.contains(claim),
                "{} promises a mechanic ({claim:?}) that nothing implements: {prose}",
                def.id
            );
        }
    }
}

/// Every shipped species authors every shipped attribute.
///
/// The field is `#[serde(default)]`, so nothing in the compiler or the
/// loader notices a species that authors none — it simply mints the
/// catalogue's own bases and reads identically to every other species,
/// which is the `AbilityDef::spread` failure: a field authored nowhere,
/// with nobody to find out.
#[test]
fn every_shipped_species_authors_every_attribute() {
    let game = Game::new(907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ids: Vec<String> = game
        .attribute_defs()
        .iter()
        .map(|d| d.id.as_str().to_string())
        .collect();
    for species in game.species_defs() {
        // The test fixture is registered into the db at `Game::new` and is
        // deliberately blank — `the_generic_fixture_species_is_blank_and_
        // unshipped` is the rule it exists for.
        if species.id == GENERIC_SPECIES_ID {
            continue;
        }
        for id in &ids {
            assert!(
                species.attributes.contains_key(id),
                "species {} authors no {id}",
                species.id
            );
        }
        for authored in species.attributes.keys() {
            assert!(
                ids.contains(authored),
                "species {} authors {authored:?}, which no attribute def defines",
                species.id
            );
        }
    }
}

/// And every shipped class, which is the player's only authored source —
/// the player carries no `Creature` and no species.
#[test]
fn every_shipped_class_authors_every_attribute() {
    let game = Game::new(908, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ids: Vec<String> = game
        .attribute_defs()
        .iter()
        .map(|d| d.id.as_str().to_string())
        .collect();
    let classes = game.class_defs();
    assert_eq!(classes.len(), 8, "the shipped catalogue is eight classes");
    for class in classes {
        for id in &ids {
            assert!(
                class.attributes.contains_key(id),
                "class {} authors no {id}",
                class.name
            );
        }
        for authored in class.attributes.keys() {
            assert!(
                ids.contains(authored),
                "class {} authors {authored:?}, which no attribute def defines",
                class.name
            );
        }
    }
}

/// The outpost content censuses from the 2026-09-23 outposts plan's Phase
/// 6: the yield table, the item ids it names, the zone material at the top
/// tier, and the founding kit's research gate.
mod outpost_content {
    use super::*;
    use crate::outposts::OutpostDb;
    use crate::world::Biome;

    /// Every biome `WorldMap::classify` can actually produce. `Platform`,
    /// `Excavated` and `Entropy` are base-space rendering-only variants —
    /// `Biome::walkable`'s doc and `classify_never_produces_the_platform_
    /// biome` — so an outpost can never stand on one and this list leaves
    /// them out on purpose.
    const SURFACE_BIOMES: [Biome; 6] = [
        Biome::DataVoid,
        Biome::Deadlock,
        Biome::NullSector,
        Biome::Backplane,
        Biome::OpenGrid,
        Biome::BlackIce,
    ];

    fn shipped_def() -> crate::outposts::OutpostDef {
        let (db, warnings) =
            OutpostDb::load_dir(&test_assets_dir().join("outposts")).expect("the dir reads");
        assert!(warnings.is_empty(), "{warnings:?}");
        db.def().expect("v1 ships one outpost def").clone()
    }

    /// Every shipped biome has an explicit row at tier 1 — the plan's own
    /// wording, stricter than the engine's own fallback: a new biome
    /// silently inheriting another one's row at the very first tier would
    /// never be noticed at the keyboard.
    #[test]
    fn every_shipped_biome_has_a_tier_1_row() {
        let def = shipped_def();
        let tier1 = &def.tiers[0].yields;
        for biome in SURFACE_BIOMES {
            assert!(
                tier1.contains_key(&biome),
                "{biome:?} has no tier-1 row in the shipped outpost def"
            );
        }
    }

    /// Every item id the shipped def names anywhere actually exists —
    /// `OutpostDef` names items in Rust nowhere else, so a typo here would
    /// otherwise surface only as a silent draw of nothing at that biome.
    #[test]
    fn every_item_the_shipped_def_names_exists() {
        let game = Game::new(9500, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = shipped_def();
        let mut checked = 0;
        for tier in &def.tiers {
            for items in tier.yields.values() {
                for item in items {
                    assert!(
                        game.item_defs().iter().any(|d| d.id == *item),
                        "{item:?} is named by the outpost def but no such item exists"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "the def named no items at all");
    }

    /// Tier 3 (index 2) — the "complex" row — names at least one
    /// `ZONE_MATERIALS` entry, `every_zone_gated_gear_recipe_asks_for_a_
    /// zone_material`'s rule one content directory over: the top of an
    /// outpost's ladder has to cost something a breach actually gates.
    #[test]
    fn tier_3_names_a_zone_material() {
        let def = shipped_def();
        let tier3 = &def.tiers[2].yields;
        let names_one = tier3
            .values()
            .any(|items| items.iter().any(|id| ZONE_MATERIALS.contains(&id.as_str())));
        assert!(
            names_one,
            "tier 3 names no zone material — {:?}",
            tier3.values().collect::<Vec<_>>()
        );
    }

    /// The founding kit exists, and it is reachable only by research — never
    /// its own `craftable` field, which `Game::craft_recipes` offers from
    /// turn one with no gate at all.
    #[test]
    fn the_kit_exists_and_is_gated_by_research() {
        let game = Game::new(9501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = shipped_def();
        let kit = def.kit.clone();
        let items = game.item_defs();
        let item = items
            .iter()
            .find(|d| d.id == kit)
            .unwrap_or_else(|| panic!("the outpost def names kit {kit:?}, which does not exist"));
        assert!(
            item.craftable.is_none(),
            "{kit:?} carries its own `craftable` field, which is always available and defeats \
             the research gate"
        );
        let gate = game
            .world
            .resource::<crate::research::ResearchDb>()
            .all()
            .find(|node| node.unlocks_recipes.iter().any(|r| r.result == kit));
        assert!(
            gate.is_some(),
            "no research node's `unlocks_recipes` names {kit:?} — it cannot be made at all"
        );
    }
}

/// A shipped-content rule, `MEMORY_TRIGGERS`' kind: a `Trigger` with no
/// shipped def is a situation the game detects and never feels.
#[test]
fn every_thought_trigger_has_a_shipped_def() {
    let (db, warnings) =
        crate::situations::ThoughtDb::load_dir(&test_assets_dir().join("thoughts")).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    for trigger in crate::situations::Trigger::ALL {
        let def = db
            .get(trigger)
            .unwrap_or_else(|| panic!("{trigger:?} has no def in assets/thoughts/"));
        assert_ne!(def.intensity, 0.0, "{trigger:?} would be worth nothing");
    }
}

/// Every status id a shipped ability or species move names, read from the
/// raw directories: `Game::new` drops a rider naming an unknown status, so a
/// census over the loaded db could never see a dangling one.
fn shipped_status_references() -> Vec<(String, StatusId)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&dir.join("abilities")).unwrap();
    let (species, _) = SpeciesDb::load_dir(&dir.join("species"), &abilities).unwrap();
    let mut refs = Vec::new();
    for def in abilities.all() {
        match &def.effect {
            crate::abilities::AbilityEffect::Debuff { kind, .. } => {
                refs.push((format!("ability {}", def.id), kind.clone()));
            }
            crate::abilities::AbilityEffect::Damage {
                status: Some(rider),
                ..
            } => refs.push((format!("ability {}", def.id), rider.kind.clone())),
            _ => {}
        }
    }
    for status in crate::routine_craft::StatusKind::ALL {
        refs.push((format!("Status part {}", status.name()), status.id()));
    }
    for def in species.all() {
        for mv in &def.moves {
            if let Some(effect) = &mv.effect {
                refs.push((
                    format!("species {} move {}", def.id, mv.name),
                    effect.kind.clone(),
                ));
            }
        }
    }
    refs
}

fn shipped_statuses() -> StatusDb {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/statuses");
    let (db, warnings) = StatusDb::load_dir(&dir).unwrap();
    assert!(
        warnings.is_empty(),
        "statuses should load cleanly: {warnings:?}"
    );
    db
}

#[test]
fn every_status_id_in_the_shipped_assets_resolves() {
    let statuses = shipped_statuses();
    let refs = shipped_status_references();
    assert!(!refs.is_empty(), "the census must actually find references");
    for (who, id) in refs {
        assert!(
            statuses.contains(&id),
            "{who} names unknown status {:?}",
            id.0
        );
    }
}

#[test]
fn every_shipped_status_is_armed_by_something() {
    let statuses = shipped_statuses();
    let mut referenced: std::collections::HashSet<StatusId> = shipped_status_references()
        .into_iter()
        .map(|(_, id)| id)
        .collect();
    for id in [
        crate::tuning::FUMBLE_EXPOSED_STATUS,
        crate::tuning::FUMBLE_CRASH_STATUS,
    ] {
        referenced.insert(StatusId::from(id));
    }
    for id in statuses.ids() {
        assert!(
            referenced.contains(id),
            "status {:?} ships but no move, ability or fumble rung arms it",
            id.0
        );
    }
    for id in ["poison", "throttled", "locked"] {
        assert!(statuses.contains(&StatusId::from(id)), "{id} should ship");
    }
}

/// The decompiler chain: four shipped nodes, each one rung past the last, all
/// reachable from `routine_fabrication`. Reach and area climb separately, so
/// each is checked strictly increasing along its own half.
#[test]
fn the_decompiler_research_chain_is_loaded_reachable_and_strictly_increasing() {
    let game = Game::new(4120, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::research::ResearchDb>();
    let chain = [
        "remote_decompile",
        "long_range_decompile",
        "broadcast_decompile",
        "wide_broadcast_decompile",
    ];
    let mut previous: Option<&str> = None;
    let (mut range, mut radius) = (0, 0);
    for id in chain {
        let def = db
            .get(id)
            .unwrap_or_else(|| panic!("{id} should be shipped"));
        let up = def.decompiler.as_ref().expect("a decompiler node");
        let expected_parent = previous.unwrap_or("routine_fabrication");
        assert_eq!(def.requires, vec![expected_parent.to_string()], "{id}");
        if let Some(r) = up.range {
            assert!(r > range, "{id} must raise the range past {range}");
            range = r;
        }
        if let Some(r) = up.radius {
            assert!(r > radius, "{id} must raise the radius past {radius}");
            radius = r;
        }
        previous = Some(id);
    }
    assert!(range > 0 && radius > 0, "both halves are covered");
    let shipped = db.all().filter(|d| d.decompiler.is_some()).count();
    assert_eq!(shipped, chain.len(), "no stray decompiler node");
}

#[test]
fn six_implants_ship_and_every_implant_links_to_its_item_both_ways() {
    let game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let implants = game.world.resource::<crate::implants::ImplantDb>();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    let ids = implants.ids();
    assert_eq!(ids.len(), 6, "the first set: {ids:?}");
    for id in &ids {
        let linked: Vec<_> = items
            .all()
            .filter(|item| item.implant.as_ref() == Some(*id))
            .collect();
        assert_eq!(
            linked.len(),
            1,
            "implant {id:?} must have exactly one item installing it"
        );
    }
    for item in items.all().filter(|item| item.implant.is_some()) {
        let target = item.implant.as_ref().unwrap();
        assert!(
            implants.get(target).is_some(),
            "item {:?} installs implant {target:?}, which does not exist",
            item.id.0
        );
    }
    let (_, warnings) =
        crate::implants::ImplantDb::load_dir(&test_assets_dir().join("implants")).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn every_shipped_rejection_status_exists() {
    let game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let statuses = game.world.resource::<StatusDb>();
    for id in tuning::REJECTION_STATUSES {
        assert!(statuses.contains(&StatusId::from(id)), "{id}");
    }
    let implants = game.world.resource::<crate::implants::ImplantDb>();
    for id in implants.ids() {
        if let Some(crate::implants::ImplantDownside::BattleStartStatus(status, _)) =
            &implants.get(id).unwrap().downside
        {
            assert!(statuses.contains(status), "{id:?} arms unknown {status:?}");
        }
    }
}

/// Every shipped room def parses, every `room_tags` value is some room's
/// `requires` tag, every required tag is carried by some structure, and
/// `door` is set on the Door alone.
#[test]
fn rooms_and_structure_room_fields_agree() {
    use crate::rooms::RoomDb;

    let (rooms, warnings) = RoomDb::load_dir(&test_assets_dir().join("rooms")).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(rooms.iter().count() > 0, "the census must walk real rooms");
    let (structures, _) = StructureDb::load_dir(&test_assets_dir().join("structures")).unwrap();
    let required: Vec<&str> = rooms
        .iter()
        .flat_map(|r| r.requires.iter().map(|t| t.tag.as_str()))
        .collect();
    let carried: Vec<&str> = structures
        .all()
        .flat_map(|s| s.room_tags.iter().map(String::as_str))
        .collect();
    for def in structures.all() {
        for tag in &def.room_tags {
            assert!(
                required.contains(&tag.as_str()),
                "{} carries room tag {tag:?}, which no room requires",
                def.id
            );
        }
        assert_eq!(def.door, def.id == "door", "{}: door flag", def.id);
    }
    for tag in required {
        assert!(
            carried.contains(&tag),
            "no structure carries room tag {tag:?}"
        );
    }
}

/// A structure file written before incubation existed still parses, as a
/// building with no incubation slots.
#[test]
fn a_structure_file_without_incubation_slots_still_parses() {
    const PLAIN: &str = r#"(
        id: "plain_structure",
        name: "Plain Structure",
        glyph: '?',
        color: White,
        build_cost: [],
        work: None,
    )"#;
    let dir = assets_dir_with_extra_structure("plain_incubation", "plain_structure.ron", PLAIN);
    let game = Game::new(907, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let db = game.world.resource::<crate::structures::StructureDb>();
    assert_eq!(db.get("plain_structure").unwrap().incubation_slots, 0);
}

/// `#[serde(default)]` lets every file omit `incubation_slots`, which is also
/// what lets the Breeding Bay forget it and ship a feature that never
/// unlocks. Only the bay incubates, and it holds exactly one child.
#[test]
fn the_breeding_bay_is_the_one_structure_that_incubates() {
    let game = Game::new(908, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::structures::StructureDb>();
    let incubators: Vec<(&str, u32)> = db
        .all()
        .filter(|d| d.incubation_slots > 0)
        .map(|d| (d.id.as_str(), d.incubation_slots))
        .collect();
    assert_eq!(incubators, vec![("breeding_bay", 1)]);
}

/// The seed is the cost of a breeding and drops only from wild species a
/// player can actually fight: a typo or a hybrid in its table would ship an
/// item nothing ever drops.
#[test]
fn the_breeding_seed_drops_only_from_real_wild_species() {
    let game = Game::new(909, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let seed = game
        .item_defs()
        .into_iter()
        .find(|d| d.id.as_str() == "breeding_seed")
        .expect("breeding_seed ships");
    let sources = seed.droppable.expect("the seed has a drop table");
    assert!(!sources.is_empty());
    let species = game.species_defs();
    for (id, chance) in sources {
        let def = species
            .iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("the seed drops from unknown species {id}"));
        assert!(
            !def.is_boss && !def.is_hybrid() && !def.habitats.is_empty(),
            "{id}"
        );
        assert!((0.05..=0.10).contains(&chance), "{id}: {chance}");
    }
}

/// The setting is the Phase-Manifold Basin, not the Grid. The Power Grid, the
/// base's supply, keeps its name, so the census forbids only the bare setting
/// sense and requires the power sense to survive. Text is whitespace-folded so
/// a phrase wrapped across lines in a help page is still caught.
#[test]
fn no_shipped_asset_calls_the_setting_the_grid() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut files = Vec::new();
    walk(&root, &mut files);
    let mut power_grid_seen = false;
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let folded = text.split_whitespace().collect::<Vec<_>>().join(" ");
        power_grid_seen |= folded.contains("Power Grid");
        for phrase in ["the Grid", "The Grid", "a Grid"] {
            assert!(
                !folded.contains(phrase),
                "{} says `{phrase}`; the setting is the Phase-Manifold Basin",
                path.display()
            );
        }
    }
    assert!(
        power_grid_seen,
        "the Power Grid terms must survive the rename"
    );
}

/// Every shipped finish parses, and every `FloorShade` is worn by at least
/// one of them — a shade the census in gui guards but no file paints is a
/// colour the brush picker never offers.
#[test]
fn every_floor_shade_has_a_shipped_finish_and_every_file_parses() {
    use crate::floors::{FloorDb, FloorShade};

    let (floors, warnings) = FloorDb::load_dir(&test_assets_dir().join("floors")).unwrap();
    assert!(
        warnings.is_empty(),
        "shipped floor files skipped: {warnings:?}"
    );
    for shade in FloorShade::ALL {
        assert!(
            floors.iter().any(|d| d.shade == shade),
            "no shipped finish wears {shade:?}"
        );
    }
}
