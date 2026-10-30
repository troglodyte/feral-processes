//! The asset databases a `Game` is built from.

use crate::*;

/// Every asset database a `Game` needs, plus the per-file warnings the loads
/// accumulated for the caller to push into the message log.
pub(crate) struct AssetDbs {
    pub(super) abilities: AbilityDb,
    pub(super) tools: ToolDb,
    pub(super) classes: crate::classes::ClassDb,
    pub(super) achievements: crate::achievements::AchievementDb,
    pub(super) contracts: crate::contracts::ContractDb,
    pub(super) descriptions: crate::descriptions::DescriptionDb,
    pub(super) memories: crate::memories::MemoryDb,
    pub(super) statuses: crate::statuses::StatusDb,
    pub(super) implants: crate::implants::ImplantDb,
    pub(super) phase_keys: crate::phase_keys::PhaseKeyDb,
    pub(super) thoughts: crate::situations::ThoughtDb,
    pub(super) interactions: crate::interactions::InteractionDb,
    pub(super) needs: crate::needs::NeedDb,
    pub(super) attributes: crate::attributes::AttributeDb,
    pub(super) sorties: crate::sorties::SortieDb,
    pub(super) caravans: crate::caravans::CaravanDb,
    pub(super) nemesis: crate::nemesis::NemesisDb,
    pub(super) species: SpeciesDb,
    pub(super) structures: StructureDb,
    pub(super) research: ResearchDb,
    pub(super) rock: crate::rock::RockDb,
    pub(super) floors: crate::floors::FloorDb,
    pub(super) crops: crate::crops::CropDb,
    pub(super) rooms: crate::rooms::RoomDb,
    pub(super) props: crate::tactical::props::PropDb,
    pub(super) items: ItemDb,
    pub(super) perks: PerkDb,
    pub(super) talents: crate::talents::TalentDb,
    pub(super) affixes: AffixDb,
    pub(super) settlements: crate::settlements::SettlementDb,
    pub(super) outposts: crate::outposts::OutpostDb,
    pub(super) policy: crate::resources::EnemyPolicy,
    pub(super) behaviours: crate::resources::Behaviours,
    pub(crate) warnings: Vec<String>,
}

/// Loads every asset directory and refuses an item set that leaves any
/// economy role unfilled. Both `Game::new` and `Game::load` must go through
/// here: `Game::currency`/`research_currency`/`craft_currency`/`trade_currency` each
/// `.expect("validated at startup")`, so a door into the world that skipped
/// this check would turn a modder's incomplete item set into a panic mid-play
/// instead of a startup error.
pub(crate) fn load_asset_dbs(assets_dir: &Path) -> std::io::Result<AssetDbs> {
    let (mut abilities, mut warnings) = AbilityDb::load_dir(&assets_dir.join("abilities"))?;
    // Absent-is-silent: an empty catalogue arms nothing. Loaded straight after
    // `abilities` and before anything derived from them (etched disks, basic
    // attacks), so a rider naming a status that does not exist is dropped
    // before a copy of it is made.
    let (statuses, status_warnings) =
        crate::statuses::StatusDb::load_dir(&assets_dir.join("statuses"))?;
    warnings.extend(status_warnings);
    let (statuses, builtin_warnings) = statuses.with_builtins();
    warnings.extend(builtin_warnings);
    warnings.extend(abilities.drop_unknown_status_riders(&statuses));
    for (name, id) in [
        ("exposed fumble rung", crate::tuning::FUMBLE_EXPOSED_STATUS),
        ("crash fumble rung", crate::tuning::FUMBLE_CRASH_STATUS),
    ] {
        if !statuses.contains(&crate::statuses::StatusId::from(id)) {
            warnings.push(format!(
                "the {name} arms status {id:?}, which is not defined — it will do nothing"
            ));
        }
    }
    // Absent-is-silent: an empty catalogue leaves nothing to install.
    let (implants, implant_warnings) =
        crate::implants::ImplantDb::load_dir(&assets_dir.join("implants"))?;
    warnings.extend(implant_warnings);
    let (mut phase_keys, phase_key_warnings) =
        crate::phase_keys::PhaseKeyDb::load_dir(&assets_dir.join("phase_keys"))?;
    warnings.extend(phase_key_warnings);
    let (ending, ending_warnings) = crate::story::EndingText::load_dir(&assets_dir.join("story"))?;
    warnings.extend(ending_warnings);
    phase_keys.ending = ending;
    // Same absent-is-silent rule as `AffixDb` — see `ToolDb::load_dir`. An
    // empty catalogue leaves nothing to forge or install, which is the
    // pre-extraction game.
    let (tools, tools_warnings) = ToolDb::load_dir(&assets_dir.join("tools"))?;
    warnings.extend(tools_warnings);
    let (mut species, species_warnings) =
        SpeciesDb::load_dir(&assets_dir.join("species"), &abilities)?;
    warnings.extend(species_warnings);
    warnings.extend(species.drop_unknown_status_riders(&statuses));
    // The blank fixture companion joins every test-built db, rather than
    // being registered onto one `Game`, because `Game::load` rebuilds the
    // db from the asset directory: a species registered after `new` is
    // gone by the time the load path looks a saved creature's id up, and
    // that creature is dropped. Both doors come through here, so both see
    // it. See `tests::support::generic_species` for why the fixture is not
    // a shipped species.
    #[cfg(test)]
    species.insert(crate::tests::support::generic_species());
    let (mut structures, structure_warnings) =
        StructureDb::load_dir(&assets_dir.join("structures"))?;
    warnings.extend(structure_warnings);
    let (mut items, item_warnings) = ItemDb::load_dir(&assets_dir.join("items"), &abilities)?;
    warnings.extend(item_warnings);
    warnings.extend(items.synthesise_parts());
    warnings.extend(items.synthesise_weapon_parts());
    // Every carrier is derived from a loaded `ToolDef`, and `tools` finished
    // loading before `items` did.
    warnings.extend(items.synthesise_tool_carriers(&tools));
    warnings.extend(structures.validate_machines(&items));
    structures.compute_levels(&items);
    let (perks, perk_warnings) = PerkDb::load_dir(&assets_dir.join("perks"))?;
    warnings.extend(perk_warnings);
    // Same absent-is-silent rule again — see `ClassDb`'s own doc. An empty
    // catalogue leaves the creation screen's class step with nothing to
    // offer and every axis resolving neutral, which is the pre-class game.
    let (classes, class_warnings) = crate::classes::ClassDb::load_dir(&assets_dir.join("classes"))?;
    warnings.extend(class_warnings);
    // Not absent-is-silent, unlike `AffixDb` and `SectorDb`: every level a
    // Kernel Ring buys is spent in one of these trees, so a missing directory
    // is an install fault rather than a supported state. The `?` surfaces it
    // the way a missing abilities directory does.
    let (talents, talent_warnings) =
        crate::talents::TalentDb::load_dir(&assets_dir.join("talents"))?;
    warnings.extend(talent_warnings);
    // An absent directory is silent and leaves the db empty, which is the
    // pre-affix game — see `AffixDb`.
    let (affixes, affix_warnings) = AffixDb::load_dir(&assets_dir.join("affixes"), &items)?;
    warnings.extend(affix_warnings);
    // After `affixes`: every research-only affix is a node in the Affixes
    // tree, derived from `AffixDb` (`affix_tree::synthesise_nodes`). Nothing
    // between the old position and this one reads research.
    let (research, research_warnings) =
        ResearchDb::load_dir(&assets_dir.join("research"), &structures, &tools, &affixes)?;
    warnings.extend(research_warnings);
    warnings.extend(structures.unknown_research_references(&items, &research));
    // A file, not a directory, and an absent one is silent — see
    // `policy::load_file`. Nothing downstream branches on whether it loaded;
    // `Game::choose_wild_action` reads the resource and falls back.
    // Absent is the pre-settlement game, `MemoryDb`'s rule: no settlements
    // derive anywhere and nothing draws one.
    let (settlements, settlement_warnings) =
        crate::settlements::SettlementDb::load_dir(&assets_dir.join("settlements"))?;
    warnings.extend(settlement_warnings);
    // Same absent-is-silent rule again — see `OutpostDb`'s own doc. An
    // empty catalogue leaves the kit refusing with a message, which is the
    // pre-outpost game.
    let (outposts, outpost_warnings) =
        crate::outposts::OutpostDb::load_dir(&assets_dir.join("outposts"))?;
    warnings.extend(outpost_warnings);
    let (policy, policy_warnings) =
        crate::policy::load_file(&assets_dir.join("policies/enemy_battle.ron"))?;
    warnings.extend(policy_warnings);
    // `default` must exist, so unlike its siblings an absent or empty
    // directory is a startup error rather than an empty catalogue.
    let (behaviours, behaviour_warnings) =
        crate::behaviours::BehaviourDb::load_dir(&assets_dir.join("behaviours"))?;
    warnings.extend(behaviour_warnings);
    warnings.extend(behaviours.unknown_species_references(&species));
    let (achievements, achievement_warnings) =
        crate::achievements::AchievementDb::load_dir(&assets_dir.join("achievements"))?;
    warnings.extend(achievement_warnings);
    let (descriptions, description_warnings) =
        crate::descriptions::DescriptionDb::load_dir(&assets_dir.join("descriptions"))?;
    warnings.extend(description_warnings);
    // Same absent-is-silent rule as `SectorDb` and `AffixDb` — see
    // `NemesisDb`'s own doc.
    let (nemesis, nemesis_warnings) =
        crate::nemesis::NemesisDb::load_dir(&assets_dir.join("nemesis"))?;
    warnings.extend(nemesis_warnings);
    let (contracts, contract_warnings) =
        crate::contracts::ContractDb::load_dir(&assets_dir.join("contracts"))?;
    warnings.extend(contract_warnings);
    // Same absent-is-silent rule again — see `MemoryDb`'s own doc. An empty
    // catalogue makes `Game::remember` a no-op and every reader zero, which
    // is the pre-memory game.
    let (memories, memory_warnings) =
        crate::memories::MemoryDb::load_dir(&assets_dir.join("memories"))?;
    warnings.extend(memory_warnings);
    // Same absent-is-silent rule again — see `ThoughtDb`'s own doc. An empty
    // catalogue fires no thought, so every program's situational term is zero
    // and the game is the pre-situation game.
    let (thoughts, thought_warnings) =
        crate::situations::ThoughtDb::load_dir(&assets_dir.join("thoughts"))?;
    warnings.extend(thought_warnings);
    // Same absent-is-silent rule again — see `InteractionDb`'s own doc. An
    // empty catalogue means nobody talks. Loaded after `MemoryDb` because a
    // def naming a memory that does not exist is skipped.
    let (interactions, interaction_warnings) =
        crate::interactions::InteractionDb::load_dir(&assets_dir.join("interactions"), &memories)?;
    warnings.extend(interaction_warnings);
    // Same absent-is-silent rule again — see `NeedDb`'s own doc. An empty
    // catalogue seeds nothing, drains nothing and takes nobody off a post,
    // which is the pre-needs game.
    let (needs, need_warnings) = crate::needs::NeedDb::load_dir(&assets_dir.join("needs"))?;
    warnings.extend(need_warnings);
    // Same absent-is-silent rule again — see `AttributeDb`'s own doc. An empty
    // catalogue mints nothing and leaves the dossier page with no rows, which
    // is the pre-attribute game.
    let (attributes, attribute_warnings) =
        crate::attributes::AttributeDb::load_dir(&assets_dir.join("attributes"))?;
    warnings.extend(attribute_warnings);
    // Same absent-is-silent rule again — see `SortieDb`'s own doc. An empty
    // catalogue leaves `Game::sortie_board` with nothing to offer, which is
    // the pre-sortie game.
    let (sorties, sortie_warnings) =
        crate::sorties::SortieDb::load_dir(&assets_dir.join("sorties"))?;
    warnings.extend(sortie_warnings);
    // Same absent-is-silent rule again — see `CaravanDb`'s own doc. An empty
    // catalogue leaves `Game::scheduled_visit` with nothing to pick, which is
    // the pre-caravan game.
    let (caravans, caravan_warnings) =
        crate::caravans::CaravanDb::load_dir(&assets_dir.join("caravans"))?;
    warnings.extend(caravan_warnings);
    // Same absent-is-silent rule a third time. An empty catalogue makes base
    // space one uniform kind of rock — the pre-kinds game — while keeping
    // the swing floor, which is a bug fix and not content.
    let (rock, rock_warnings) = crate::rock::RockDb::load_dir(&assets_dir.join("rock"))?;
    warnings.extend(rock_warnings);
    // Same absent-is-silent rule again — see `MemoryDb`'s own doc. An empty
    // catalogue offers no brush and leaves every laid tile plain, which is
    // the pre-finish game.
    let (floors, floor_warnings) = crate::floors::FloorDb::load_dir(&assets_dir.join("floors"))?;
    warnings.extend(floor_warnings);
    // Absent is silent: no crop means a farm plot has nothing to sow.
    let (mut crops, crop_warnings) = crate::crops::CropDb::load_dir(&assets_dir.join("crops"))?;
    warnings.extend(crop_warnings);
    let (rooms, room_warnings) = crate::rooms::RoomDb::load_dir(&assets_dir.join("rooms"))?;
    warnings.extend(room_warnings);
    // Absent is silent: an empty catalogue is the pre-prop battle map.
    let (props, prop_warnings) =
        crate::tactical::props::PropDb::load_dir(&assets_dir.join("battle-props"))?;
    warnings.extend(prop_warnings);
    let missing = items.missing_roles();
    if !missing.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "item set is missing required economy role(s): {}",
                missing.join(", ")
            ),
        ));
    }
    for required in [
        abilities::FALLBACK_ABILITY_ID,
        abilities::DECOMPILE_ABILITY_ID,
    ] {
        if abilities.get(required).is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "ability set is missing the mandatory ability {required:?} — the game \
                     pre-installs it and cannot start without it"
                ),
            ));
        }
    }
    warnings.extend(crops.drop_unknown_products(&items));
    Ok(AssetDbs {
        abilities,
        tools,
        classes,
        talents,
        achievements,
        contracts,
        descriptions,
        memories,
        statuses,
        implants,
        phase_keys,
        thoughts,
        interactions,
        needs,
        attributes,
        sorties,
        caravans,
        nemesis,
        species,
        structures,
        research,
        rock,
        floors,
        crops,
        rooms,
        props,
        items,
        perks,
        affixes,
        settlements,
        outposts,
        policy: crate::resources::EnemyPolicy(policy),
        behaviours: crate::resources::Behaviours(behaviours),
        warnings,
    })
}
