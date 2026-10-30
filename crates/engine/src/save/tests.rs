use super::*;

/// A pre-stacking save wrote one optional `affix`; this build writes a
/// list. The singular field is kept, load-only, and lifted — so a save
/// written before affixes stacked keeps the one it had without a
/// `SAVE_FORMAT_VERSION` bump.
///
/// Parsed as a RON fragment rather than round-tripped through a whole
/// save for `a_v29_save_still_loads_its_gear_after_the_new_field_lands`'
/// reason: the fragment is what an old file on disk actually contains,
/// and building one by hand is the only way to have a file this build
/// never wrote.
#[test]
fn a_pre_stacking_save_lifts_its_singular_affix() {
    let old: GearCopySave = ron::from_str(
        r#"(item: "kinetic_edge", tier: 1, rarity: Silver, affix: Some("of_static"))"#,
    )
    .expect("a pre-stacking row must still load");
    assert_eq!(
        affixes_from_save(old.affix, old.affixes),
        vec![AffixId::from("of_static")],
        "the singular field must be lifted into the list"
    );
}

/// The write side fills `affixes` and leaves the legacy field empty, so
/// this is the shape every save from here on carries.
#[test]
fn a_stacked_save_loads_every_affix_it_names() {
    let new: GearCopySave =
        ron::from_str(r#"(item: "kinetic_edge", tier: 1, affixes: ["hardened", "of_static"])"#)
            .expect("the stacked row shape must load");
    assert!(
        new.affix.is_none(),
        "a save written by this build carries no singular field"
    );
    assert_eq!(
        affixes_from_save(new.affix, new.affixes),
        vec![AffixId::from("hardened"), AffixId::from("of_static")],
        "both affixes must survive the load"
    );
}

/// Neither key: every copy in every save written before affixes existed
/// at all. Both fields default, and the copy reads as unaffixed.
#[test]
fn a_pre_affix_save_loads_unaffixed() {
    let ancient: GearCopySave =
        ron::from_str(r#"(item: "shim_blade", tier: 0)"#).expect("a pre-affix row must load");
    assert!(
        affixes_from_save(ancient.affix, ancient.affixes).is_empty(),
        "a copy that never had an affix must load with none"
    );
}

/// The one save surface gear rarity cannot widen in place.
/// `PlayerSave::fused_gear` ships as a positional 3-tuple
/// (`dev-saves/extraction.ron:63` is literally `("scrap_ward", 3, 1)`),
/// and RON parses a `(` in a struct position as the start of *named*
/// fields — it raises `ExpectedIdentifier` rather than falling through
/// to serde's `visit_seq`, so converting the row to a named struct with
/// defaulted trailing fields does **not** load an old save. Measured,
/// not assumed.
///
/// So the two shapes coexist for one release instead: `fused_gear` stays
/// exactly as it was and is read-only, `gear_copies` is the new store,
/// and `Game::load` drains the first into the second. `fused_gear` is
/// skipped when empty, so a save written from here on carries only the
/// new field — and since nothing sets `deny_unknown_fields`, the legacy
/// field can be deleted outright a release later with no bump.
#[test]
fn a_v29_save_still_loads_its_gear_after_the_new_field_lands() {
    #[derive(Debug, Serialize, Deserialize)]
    struct GearCopyProbe {
        item: ItemId,
        tier: u32,
        qty: u32,
        #[serde(default)]
        rarity: Rarity,
        #[serde(default)]
        affix: Option<String>,
    }

    #[derive(Debug, Default, Serialize, Deserialize)]
    struct PlayerProbe {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fused_gear: Vec<(ItemId, u32, u32)>,
        #[serde(default)]
        gear_copies: Vec<GearCopyProbe>,
    }

    // A v29 player: legacy rows, and no `gear_copies` field at all.
    let old: PlayerProbe = ron::from_str(r#"(fused_gear: [("scrap_ward", 3, 1)])"#)
        .expect("a v29 save must still load");
    assert_eq!(old.fused_gear, vec![(ItemId::from("scrap_ward"), 3, 1)]);
    assert!(
        old.gear_copies.is_empty(),
        "the new store defaults empty so load can drain the legacy rows into it"
    );

    // A save written after this change: new rows, no legacy field.
    let new: PlayerProbe = ron::from_str(
        r#"(gear_copies: [(item: "kinetic_edge", tier: 1, qty: 2, rarity: Silver)])"#,
    )
    .expect("the new row shape must load");
    assert_eq!(new.gear_copies[0].rarity, Rarity::Silver);
    assert_eq!(new.gear_copies[0].qty, 2);
    assert!(
        new.gear_copies[0].affix.is_none(),
        "affix lands in a later phase and must default until then"
    );

    // Nothing carries `deny_unknown_fields`, so dropping `fused_gear` in
    // a later release is not a format break either.
    let dropped: GearCopyProbe =
        ron::from_str(r#"(item: "shim_blade", tier: 1, qty: 1, retired_field: 7)"#)
            .expect("an unknown field must be ignored, not refused");
    assert_eq!(dropped.item, ItemId::from("shim_blade"));

    // And a fresh save carries no legacy field at all.
    let written = ron::ser::to_string_pretty(
        &PlayerProbe {
            gear_copies: vec![GearCopyProbe {
                item: ItemId::from("shim_blade"),
                tier: 0,
                qty: 1,
                rarity: Rarity::Gold,
                affix: None,
            }],
            ..Default::default()
        },
        ron::ser::PrettyConfig::default(),
    )
    .unwrap();
    assert!(
        !written.contains("fused_gear"),
        "an empty legacy field must not be written back out: {written}"
    );
}

fn sample_data() -> SaveData {
    SaveData {
        seed: 1,
        base_ledger: Default::default(),
        tick: 0,
        difficulty: DifficultyMode::Forgiving,
        game_over: None,
        player: PlayerSave {
            position: (0, 0),
            hp: 30,
            power: 100.0,
            stat_points: 0,
            inventory: Vec::new(),
            level: 1,
            xp: 0,
            xp_to_next: 20,
            weapon: None,
            weapon_level: 1,
            weapon_fusion_tier: 0,
            weapon_rarity: Rarity::Ordinary,
            weapon_affix: None,
            weapon_affixes: Vec::new(),
            weapon_quality: crate::tuning::QUALITY_DEFAULT,
            weapon_wear: 0,
            armor: None,
            armor_level: 1,
            armor_fusion_tier: 0,
            armor_rarity: Rarity::Ordinary,
            armor_affix: None,
            armor_affixes: Vec::new(),
            armor_quality: crate::tuning::QUALITY_DEFAULT,
            armor_wear: 0,
            module: None,
            module_level: 1,
            module_fusion_tier: 0,
            module_rarity: Rarity::Ordinary,
            module_affix: None,
            module_affixes: Vec::new(),
            module_quality: crate::tuning::QUALITY_DEFAULT,
            module_wear: 0,
            fused_gear: Vec::new(),
            gear_copies: Vec::new(),
            downed_programs: Vec::new(),
            tools: Vec::new(),
            implants: Vec::new(),
            phase_keys: crate::components::PhaseKeys::default(),
            perk_points: 0,
            unlocked_perks: Vec::new(),
            bought_stats: crate::components::BoughtStats::default(),
            tutorial_seeded: true,
            routines: Vec::new(),
            field_buffs: Vec::new(),
            sorties: Vec::new(),
            routes: Vec::new(),
            name: String::new(),
            class: None,
            glyph: '@',
            sprite: String::new(),
            colour: None,
            icon: None,
            attributes: Default::default(),
        },
        creatures: Vec::new(),
        structures: Vec::new(),
        floor_items: Vec::new(),
        floor_piles: Vec::new(),
        nests: Vec::new(),
        traps: Vec::new(),
        dig_sites: Vec::new(),
        farm_plots: Vec::new(),
        ruins: Vec::new(),
        build_sites: Vec::new(),
        caravans: Vec::new(),
        caravan_memory: CaravanMemorySave::default(),
        tile_overrides: Vec::new(),
        base_grid: crate::base_grid::BaseGrid::default(),
        mining: false,
        enemy_strength: crate::resources::EnemyStrength::default(),
        free_builds: crate::resources::FreeBuilds::default(),
        run_tally: crate::resources::RunTally::default(),
        anchor: None,
        zone: 1,
        spawn_point: (0, 0),
        buyback: Vec::new(),
        buyback_shelves: Vec::new(),
        researched: Vec::new(),
        active_research: None,
        research_progress: Vec::new(),
        discovered_research: Vec::new(),
        study_progress: 0,
        known_tools: Vec::new(),
        emulation_images: Vec::new(),
        crafted_routines: Vec::new(),
        unslotted_routines: Vec::new(),
        next_crafted_id: 0,
        ftue_played: Vec::new(),
        gear_designs: Vec::new(),
        next_gear_design_id: 0,
        known_affixes: Vec::new(),
        link_sites: Vec::new(),
        locale: crate::resources::Locale::Surface,
        stack_memory: crate::resources::StackMemory::default(),
        stack_memory_tiered: true,
        entered_tiers: crate::resources::EnteredTiers::default(),
        populated_chunks: crate::resources::PopulatedChunks::default(),
        explored_chunks: crate::resources::ExploredChunks::default(),
        seen_tiles: crate::resources::SeenTiles::default(),
        settlements: crate::resources::Settlements::default(),
        standings: crate::resources::Standings::default(),
        compass: crate::resources::CompassBearing::default(),
        trace: 0,
        raid_pressure: crate::resources::RaidPressure::default(),
        siege_pressure: crate::resources::SiegePressure::default(),
        siege: None,
        contracts: Vec::new(),
        contracts_done: Vec::new(),
        work_orders: Vec::new(),
        alerts: Vec::new(),
        next_program_id: crate::resources::NextProgramId::START.0,
        outposts: Vec::new(),
        outpost_routes: Vec::new(),
    }
}

/// A minimal tamed program, for the fixtures that need the save to hold
/// a creature at all.
fn sample_creature() -> CreatureSave {
    CreatureSave {
        species: "scrapper".to_string(),
        position: (1, 1),
        hp: 10,
        max_hp: 10,
        atk: 3,
        mitigation: 1,
        tamed: true,
        power: crate::components::POWER_MAX,
        level: 1,
        xp: 0,
        xp_to_next: 20,
        cronjob: None,
        party_slot: None,
        sortie_index: None,
        wielded: false,
        zone: 1,
        custom_name: None,
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
        assembly_roll: 1.0,
        extraction_roll: 1.0,
        fusions: 0,
        refactors: 0,
        purchased_tiers: 0,
        ring: 0,
        talents: Vec::new(),
        bought_stats: crate::components::BoughtStats::default(),
        routines: Vec::new(),
        field_buffs: Vec::new(),
        nest_position: None,
        patrol_position: None,
        study_station: None,
        outpost: None,
        siphon: None,
        jailed: None,
        pursuing: false,
        carrying: None,
        carrying_program: None,
        tidying: None,
        clearing: false,
        rarity: Rarity::Ordinary,
        boss: false,
        nemesis_grudges: 0,
        nemesis_muster_ticks: 0,
        nemesis_home: None,
        nemesis_band: None,
        equipment: Vec::new(),
        program_id: 1,
        disposition: None,
        disgruntled: None,
        disgruntled_stranded: false,
        memories: Vec::new(),
        conversations: Vec::new(),
        needs: Default::default(),
        attributes: Default::default(),
        off_shift: None,
        staff: false,
        downed: false,
        siege_cell: None,
        siege_order: None,
        besieger: false,
        stolen_from: None,
        off_duties: Vec::new(),
        staff_rank: None,
        drop_trooper: false,
        base: None,
        stat_points: 0,
        hold_points: false,
        generation: 0,
        breed_ready_at: 0,
    }
}

/// The savetool's whole premise: a save dumped to RON, then packed back,
/// must be the same save. Byte identity of the bincode encoding is the
/// strictest form of that and catches a field silently dropped by the
/// text encoding, which a field-by-field assertion would miss.
#[test]
fn a_save_survives_a_round_trip_through_ron_unchanged() {
    let mut data = sample_data();
    data.player.inventory = vec![(ItemId::from("core_fragment"), 3)];
    data.player.power = 62.5;
    data.tile_overrides = vec![(
        (4, -7),
        Tile {
            biome: crate::world::Biome::Platform,
            walkable: true,
            rock_shade: None,
        },
    )];
    data.zone = 3;
    // `StackMemory` is a map keyed by a *tuple* (`FrameKey`), which is
    // exactly where a text encoding tends to give up, and `Locale` is a
    // struct-variant enum. Both are in the round trip deliberately.
    data.locale = crate::resources::Locale::Stack {
        depth: 2,
        frames: 4,
        x: 9,
        y: 11,
        facing: crate::stack::Dir::West,
        entrance: (4, -7),
    };
    // A `BTreeSet` of tuples, which is the same place a text encoding
    // tends to give up.
    // A newtype over an `Option<enum>` with a tuple payload, and the
    // other half of the compass' save coverage: the real save/load test
    // in `tests::compass` catches a field `Game::save` never fills,
    // while this catches one the text encoding drops.
    data.compass =
        crate::resources::CompassBearing(Some(crate::settlements::CompassTarget::Link((4, -7))));
    data.populated_chunks.0.insert((3, -2));
    data.populated_chunks.0.insert((-14, 9));
    data.explored_chunks.0.insert((5, -6));
    data.stack_memory.0.insert(
        ((4, -7), 2),
        crate::resources::FrameMemory {
            seen: [(1, 1), (1, 2)].into_iter().collect(),
            looted: [(3, 3)].into_iter().collect(),
            opened: Default::default(),
            cleared: true,
            fights: [(5, 5)].into_iter().collect(),
            jacked: [(7, 2)].into_iter().collect(),
            adopted: [(9, 4)].into_iter().collect(),
            bought: [0, 4].into_iter().collect(),
        },
    );

    let before = bincode::serde::encode_to_vec(&data, bincode::config::standard()).unwrap();
    let text = to_ron(&data).unwrap();
    let parsed = from_ron(&text).unwrap();
    let after = bincode::serde::encode_to_vec(&parsed, bincode::config::standard()).unwrap();

    assert_eq!(
        before, after,
        "a RON round trip must not change a single byte of the save"
    );
}

/// The save on disk is the same field-named RON `savetool dump` prints,
/// behind one line naming the version. Positional encodings are why
/// every bump from v19 to v28 broke saves for a change that only ever
/// *added* a field — see `SAVE_FORMAT_VERSION`.
#[test]
fn a_save_file_is_field_named_text() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_text_{}.bin",
        std::process::id()
    ));
    save_to_file(&path, &sample_data()).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let loaded = load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        text.starts_with(&format!("{SAVE_FORMAT_VERSION}\n")),
        "the version is the first line, so it can be read without parsing the rest"
    );
    assert!(
        text.contains("seed:"),
        "the payload names its fields: {}",
        &text[..text.len().min(120)]
    );
    assert_eq!(loaded.seed, sample_data().seed);
}

/// A save written before the mining toggle existed carries no `mining`
/// field at all, and must load with the player's bump **disarmed** —
/// both because that is the new default and because a save that never
/// expressed a preference must not arm a tool that destroys terrain.
///
/// Deleting the line rather than trusting the derive: `#[serde(default)]`
/// on a `bool` is exactly the shape that reads as obviously right and
/// silently isn't if the attribute is ever dropped in a refactor.
/// A save written before the enemy-strength knob existed carries no
/// `enemy_strength` field, and must load on the floor — the band it was
/// played on, since the ladder has no rung below `Standard`.
///
/// Deleting the line rather than trusting the derive, the mining test's
/// reason: a `#[serde(default)]` dropped in a refactor reads as
/// obviously fine and silently is not.
#[test]
fn a_save_without_the_enemy_strength_field_loads_at_standard() {
    let mut data = sample_data();
    data.enemy_strength = crate::resources::EnemyStrength::Critical;
    let text = to_ron(&data).unwrap();
    assert!(
        text.contains("enemy_strength: Critical"),
        "the fixture must actually write the field to be a real test"
    );
    let older = text.replace("enemy_strength: Critical,", "");
    let loaded = from_ron(&older).expect("an absent field must still parse");
    assert_eq!(
        loaded.enemy_strength,
        crate::resources::EnemyStrength::Standard
    );
}

#[test]
fn a_save_without_the_run_tally_field_loads_at_zero() {
    let mut data = sample_data();
    data.run_tally.foe_levels = 77;
    let text = to_ron(&data).unwrap();
    assert!(
        text.contains("run_tally:"),
        "the fixture must actually write the field to be a real test"
    );
    let start = text.find("run_tally:").unwrap();
    let end = start + text[start..].find("),").unwrap() + 2;
    let older = format!("{}{}", &text[..start], &text[end..]);
    let loaded = from_ron(&older).expect("an absent field must still parse");
    assert_eq!(loaded.run_tally, crate::resources::RunTally::default());
}

#[test]
fn a_save_without_the_mining_field_loads_disarmed() {
    let mut data = sample_data();
    data.mining = true;
    let text = to_ron(&data).unwrap();
    assert!(
        text.contains("mining: true"),
        "the fixture must actually write the field to be a real test"
    );
    let older = text.replace("mining: true,", "");
    let loaded = from_ron(&older).expect("an absent field must still parse");
    assert!(!loaded.mining);
}

/// The whole point of the format. A field added behind
/// `#[serde(default)]` must load out of a file written before it
/// existed, with no migration code and no version bump — which is what
/// every single bump in this file's history would have needed.
#[test]
fn a_save_file_written_before_a_defaulted_field_existed_still_loads() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_older_{}.bin",
        std::process::id()
    ));
    let mut data = sample_data();
    data.creatures.push(sample_creature());
    save_to_file(&path, &data).unwrap();

    // Exactly what an older build would have written: the same file
    // with a since-added key absent. Derived from the real one rather
    // than hand-written, so the fixture cannot drift.
    let text = std::fs::read_to_string(&path).unwrap();
    let older: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("equipment: ["))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !older.contains("equipment: ["),
        "the key has to actually be gone for this to prove anything"
    );
    std::fs::write(&path, &older).unwrap();

    let loaded = match load_from_file(&path) {
        Ok(loaded) => loaded,
        Err(e) => panic!("an older file must still load: {e}"),
    };
    let _ = std::fs::remove_file(&path);
    assert!(loaded.creatures[0].equipment.is_empty());
}

/// The same guarantee for slice 2's own added field, and the reason
/// `SAVE_FORMAT_VERSION` stayed at 32 for it: every `dev-saves/`
/// template predates `dig_sites` and must load with no dig sites, which
/// is exactly the base those runs had.
#[test]
fn a_save_written_before_dig_sites_existed_still_loads() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_no_dig_{}.bin",
        std::process::id()
    ));
    save_to_file(&path, &sample_data()).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let older: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("dig_sites: ["))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !older.contains("dig_sites:"),
        "the key has to actually be gone for this to prove anything"
    );
    std::fs::write(&path, &older).unwrap();

    let loaded = match load_from_file(&path) {
        Ok(loaded) => loaded,
        Err(e) => panic!("a file written before dig sites must still load: {e}"),
    };
    let _ = std::fs::remove_file(&path);
    assert!(
        loaded.dig_sites.is_empty(),
        "and comes back with no walls half-cut"
    );
}

/// The same demand of the caravan field: a run that predates it must
/// load with no trader standing, which is exactly what that run had.
#[test]
fn a_save_written_before_caravans_existed_still_loads() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_no_caravan_{}.bin",
        std::process::id()
    ));
    save_to_file(&path, &sample_data()).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let older: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("caravans: ["))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !older.contains("caravans:"),
        "the key has to actually be gone for this to prove anything"
    );
    std::fs::write(&path, &older).unwrap();

    let loaded = match load_from_file(&path) {
        Ok(loaded) => loaded,
        Err(e) => panic!("a file written before caravans must still load: {e}"),
    };
    let _ = std::fs::remove_file(&path);
    assert!(loaded.caravans.is_empty(), "and nobody is standing there");
}

/// The field `tools` differs from every other `#[serde(default)]` above
/// it: a save written before program extraction shipped paid its
/// material income through the direct kill drop this branch retired
/// (`combat_rewards.rs`), so loading with an *empty* loadout — the
/// plain `Vec::default()` reading — would leave that save with no tool
/// to extract with and no way to ever get one, forever. The absent key
/// must land the starter tool instead, as a migration.
#[test]
fn a_save_written_before_tools_existed_loads_with_the_starter_tool() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_no_tools_{}.bin",
        std::process::id()
    ));
    save_to_file(&path, &sample_data()).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let older: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("tools: ["))
        .collect::<Vec<_>>()
        .join("\n");
    // A bare `.contains("tools:")` also matches `known_tools:` — a
    // sibling field this same phase adds — and would fail the premise
    // check for the wrong reason. The line-anchored check the removal
    // filter itself uses is what actually proves the key is gone.
    assert!(
        !older
            .lines()
            .any(|line| line.trim_start().starts_with("tools: [")),
        "the key has to actually be gone for this to prove anything"
    );
    std::fs::write(&path, &older).unwrap();

    let loaded = match load_from_file(&path) {
        Ok(loaded) => loaded,
        Err(e) => panic!("a file written before tools existed must still load: {e}"),
    };
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        loaded.player.tools,
        vec![ToolId(crate::tuning::STARTER_TOOL_ID.to_string())],
        "an absent tools key must land the starter tool, or this save's \
         material income is gone for good"
    );
}

/// `discovered_research` and `study_progress` are additive behind
/// `#[serde(default)]`, so a save written before discovery shipped must
/// load with nothing found and nothing banked. For a run in progress
/// that means the discoverable half of its tree hides itself and is
/// studied back out, which is the new rule correctly applied rather
/// than a migration.
#[test]
fn a_save_written_before_discovery_loads_with_nothing_discovered() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_no_discovery_{}.bin",
        std::process::id()
    ));
    // An empty list writes as a single `discovered_research: []` line,
    // so the removal below can be line-anchored the way the tools
    // migration beside it is — a populated list is written across
    // several lines and cutting only its first would leave the rest
    // behind as a parse error rather than an absent key.
    let mut data = sample_data();
    data.study_progress = 5;
    save_to_file(&path, &data).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let older: String = text
        .lines()
        .filter(|line| {
            let t = line.trim_start();
            !t.starts_with("discovered_research:") && !t.starts_with("study_progress:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    // Line-anchored, for the reason the tools migration beside this one
    // takes the same care: a bare `.contains` would be satisfied by a
    // substring of some other field and pass for the wrong reason.
    assert!(
        older.lines().count() < text.lines().count(),
        "both keys have to actually be gone for this to prove anything"
    );
    std::fs::write(&path, &older).unwrap();

    let loaded = match load_from_file(&path) {
        Ok(loaded) => loaded,
        Err(e) => panic!("a file written before discovery existed must still load: {e}"),
    };
    let _ = std::fs::remove_file(&path);
    assert!(loaded.discovered_research.is_empty());
    assert_eq!(loaded.study_progress, 0);
}

/// The other half of the same default, proven in the same file rather
/// than assumed from the doc comment: `starter_tools` must fire on an
/// *absent* key only. `Game::save` always writes `tools`, so a save
/// this branch itself wrote with an explicitly empty loadout (standing
/// in for a future `uninstall_tool`) must load empty, not get the
/// starter tool pushed back in.
#[test]
fn a_save_with_an_explicitly_empty_tools_list_loads_empty() {
    let mut data = sample_data();
    data.player.tools = Vec::new();
    let text = to_ron(&data).unwrap();
    // Line-anchored rather than a bare `.contains`, for the reason the
    // migration test above takes the same care: `known_tools: []` also
    // contains the substring `"tools: []"` and would satisfy a loose
    // check regardless of what `player.tools` actually serialized to.
    assert!(
        text.lines()
            .any(|line| line.trim_start().starts_with("tools: []")),
        "the fixture must actually write the empty list to be a real test"
    );

    let loaded = from_ron(&text).expect("an explicit empty list must still parse");
    assert!(
        loaded.player.tools.is_empty(),
        "a stored empty loadout must not be overwritten by the starter-tool default"
    );
}

/// The biome rename is not a save-format break, and this is what says
/// so: a save written before it holds `Deadlock` in its tile overlay
/// and must still load, while a save written now says `Deadlock`. The
/// `serde(alias)` is the whole mechanism, so removing it fails here
/// rather than in a player's next session.
#[test]
fn a_save_holding_the_old_cold_biome_name_still_loads() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_biome_{}.bin",
        std::process::id()
    ));
    let mut data = sample_data();
    data.tile_overrides = vec![(
        (4, -7),
        Tile {
            biome: crate::world::Biome::Deadlock,
            walkable: true,
            rock_shade: None,
        },
    )];
    save_to_file(&path, &data).unwrap();

    // Derived from the real file rather than hand-written, so the
    // fixture cannot drift from what the writer actually emits.
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("Deadlock"),
        "a save written now must use the new name"
    );
    std::fs::write(&path, text.replace("Deadlock", "StaticField")).unwrap();

    let loaded = match load_from_file(&path) {
        Ok(loaded) => loaded,
        Err(e) => panic!("a save written before the rename must still load: {e}"),
    };
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        loaded.tile_overrides[0].1.biome,
        crate::world::Biome::Deadlock
    );
}

/// A save from a build that wrote bincode is not text at all, so the
/// version line is missing entirely. It has to be refused by version,
/// the way any other unreadable save is, rather than surfacing a parse
/// error about a byte offset.
#[test]
fn a_save_from_a_binary_format_build_is_refused_by_version() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_binary_{}.bin",
        std::process::id()
    ));
    let mut bytes = 28u32.to_le_bytes().to_vec();
    bytes
        .extend(bincode::serde::encode_to_vec(sample_data(), bincode::config::standard()).unwrap());
    std::fs::write(&path, bytes).unwrap();

    let err = match load_from_file(&path) {
        Ok(_) => panic!("a binary save must not decode as text"),
        Err(e) => e.to_string(),
    };
    let _ = std::fs::remove_file(&path);
    assert!(
        err.contains("older version of the game"),
        "the refusal should name the cause, not a byte offset: {err}"
    );
}

/// The Fatigue meter is gone, so nothing on disk may claim one. Asserted
/// against the serialized text and against a payload that omits the key,
/// because a field merely left in the struct and ignored would pass a
/// struct-level check and still cost the next property a version bump.
#[test]
fn a_save_neither_writes_nor_requires_a_fatigue_field() {
    let ron = to_ron(&sample_data()).unwrap();
    assert!(
        !ron.contains("fatigue"),
        "a save must not carry a Fatigue meter:\n{ron}"
    );
    assert!(
        from_ron(&ron).is_ok(),
        "and a payload written without one has to load"
    );
}

#[test]
fn a_save_round_trips_through_the_current_version() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_roundtrip_{}.bin",
        std::process::id()
    ));
    save_to_file(&path, &sample_data()).unwrap();
    let loaded = load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(loaded.seed, 1);
}

#[test]
fn a_save_written_with_a_different_version_is_rejected_cleanly_instead_of_corrupting() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_badversion_{}.bin",
        std::process::id()
    ));
    // A save at a version this build does not read, written the way a
    // build at that version would have: the same text framing, a
    // different number on the first line.
    std::fs::write(&path, format!("999\n{}", to_ron(&sample_data()).unwrap())).unwrap();

    let Err(err) = load_from_file(&path) else {
        panic!("loading a mismatched-version save should fail, not succeed");
    };
    let _ = std::fs::remove_file(&path);
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("incompatible save version"),
        "error should clearly say the save is from an incompatible version, got: {err}"
    );
}

/// There is no migration path (see `SAVE_FORMAT_VERSION`'s docs), so a
/// save written under the immediately preceding version must be refused
/// exactly like any other mismatch rather than silently decoded into
/// garbage — the adjacent version being the one where a near-miss decode
/// is most plausible.
///
/// Written relative to the constant rather than against a hardcoded pair
/// so it keeps testing the adjacent case across every future bump. It
/// last named 14 -> 15 (adding `field_buffs`) and had gone stale by the
/// time the constant reached 16.
#[test]
fn a_save_written_at_the_previous_version_is_refused() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_prev_version_{}.bin",
        std::process::id()
    ));
    std::fs::write(
        &path,
        format!(
            "{}\n{}",
            SAVE_FORMAT_VERSION - 1,
            to_ron(&sample_data()).unwrap()
        ),
    )
    .unwrap();

    let Err(err) = load_from_file(&path) else {
        panic!("a save one version back should not load");
    };
    let _ = std::fs::remove_file(&path);
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("incompatible save version"),
        "error should clearly say the save is from an incompatible version, got: {err}"
    );
}

/// `dev-saves/extraction.ron` deserializes by field name — RON is
/// self-describing, unlike the positional bincode save (see
/// `SAVE_FORMAT_VERSION`'s docs) — so a `SaveData` field rename that
/// forgets to update the template's keys breaks `--template extraction`
/// at load. The launcher's `dev_template` tests do cover this file too
/// (`every_checked_in_template_still_loads` enumerates every template
/// and parses it), but this test still earns its place: it lives in the
/// crate that owns `SaveData`, so `cargo test -p feral-processes-engine`
/// catches RON-key drift without building the launcher.
#[test]
fn the_extraction_template_parses_into_save_data() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dev-saves/extraction.ron");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    if let Err(e) = from_ron(&text) {
        panic!("dev-saves/extraction.ron should parse into SaveData: {e}");
    }
}

/// A save carrying a Stack position and a live Trace, written and read
/// back through the real file round trip rather than the RON one above.
///
/// This used to pin `SAVE_FORMAT_VERSION` to 15, which was the whole
/// claim phase 1 of the Stack work rested on: renaming types moves no
/// encoded byte. Phase 2 adds `trace`, which genuinely does, so the pin
/// is gone rather than merely retargeted at 16 — a hardcoded version in
/// a test taxes every future phase that legitimately bumps it, and
/// phases 3 and 4 are both expected to. What is worth asserting is that
/// the *payload* survives, which is what the bump exists to protect.
#[test]
fn a_stack_position_and_its_trace_survive_a_binary_save_round_trip() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_stack_roundtrip_{}.bin",
        std::process::id()
    ));
    let mut data = sample_data();
    data.locale = crate::resources::Locale::Stack {
        depth: 2,
        frames: 4,
        x: 9,
        y: 11,
        facing: crate::stack::Dir::West,
        entrance: (4, -7),
    };
    data.trace = 123;
    save_to_file(&path, &data).unwrap();
    let loaded = load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        loaded.locale, data.locale,
        "the Stack position did not survive the round trip"
    );
    assert_eq!(
        loaded.trace, 123,
        "Trace did not survive the round trip — saving mid-dive would \
         be a free reset"
    );
}

#[test]
fn a_truncated_file_fails_cleanly_instead_of_panicking() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_save_truncated_{}.bin",
        std::process::id()
    ));
    std::fs::write(&path, [1, 2]).unwrap();
    let Err(err) = load_from_file(&path) else {
        panic!("loading a truncated save should fail, not succeed");
    };
    let _ = std::fs::remove_file(&path);
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
}
