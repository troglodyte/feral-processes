//! Fixtures shared by the app-core tests: creatures.

use feral_processes_engine::resources::Locale;
use feral_processes_engine::save::{self, CreatureSave};
use feral_processes_engine::stack::Dir;

use super::*;
use crate::*;

/// The shared body: one program per name `pick` returns. It takes the
/// `Game` because the count-based caller wants whichever species the roster
/// happens to list first, and that is not knowable until one is loaded.
pub(super) fn distant_programs(seed: u32, pick: impl FnOnce(&Game) -> Vec<String>) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("distant", seed);
    let game = app.game.as_mut().unwrap();
    let species = pick(game);
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    for (i, species) in species.iter().enumerate() {
        let i = i as i32;
        data.creatures.push(CreatureSave {
            sortie_index: None,
            boss: false,
            species: species.clone(),
            position: (px + MENU_SCAN_RADIUS + 10 + i, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            mitigation: 2,
            tamed: true,
            power: 100.0,
            level: 1,
            xp: 0,
            xp_to_next: 10,
            cronjob: None,
            party_slot: None,
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
            bought_stats: Default::default(),
            routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
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
            rarity: Default::default(),
            nemesis_grudges: 0,
            nemesis_muster_ticks: 0,
            nemesis_home: None,
            nemesis_band: None,
            equipment: Vec::new(),
            program_id: 0,
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
            base: tamed_base(10, 3, 2),
            stat_points: 0,
            hold_points: false,
            generation: 0,
            breed_ready_at: 0,
        });
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

/// A minimal hostile `CreatureSave` at `position` — the level-1 scrapper
/// shape `place_wild_program_east` and `box_in_player_with_hostiles` both
/// need, factored out so a travel test boxing the player in with eight of
/// them doesn't retype the ~40-field literal eight times.
pub(super) fn wild_creature_save(species: String, position: (i32, i32)) -> CreatureSave {
    CreatureSave {
        sortie_index: None,
        boss: false,
        species,
        position,
        hp: 10,
        max_hp: 10,
        atk: 3,
        mitigation: 2,
        tamed: false,
        power: 100.0,
        level: 1,
        xp: 0,
        xp_to_next: 10,
        cronjob: None,
        party_slot: None,
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
        bought_stats: Default::default(),
        routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
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
        rarity: Default::default(),
        nemesis_grudges: 0,
        nemesis_muster_ticks: 0,
        nemesis_home: None,
        nemesis_band: None,
        equipment: Vec::new(),
        program_id: 0,
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

/// Puts one wild program exactly `east` tiles due east of the player and
/// clears the rest of that row, then returns it.
///
/// `Game::find_target_in_direction` reads the row and nothing beside it, so
/// an inspect test has to *own* that row rather than hope the seed left it
/// clear — and the clearing is what makes the returned entity the answer
/// rather than merely *an* answer. Built by editing a save and reloading it,
/// for the same reason `app_owning_distant_programs` is: the engine exposes
/// no way to hand-place a creature from outside the crate.
pub(crate) fn place_wild_program_east(app: &mut App, east: i32) -> Entity {
    let assets_dir = test_assets_dir();
    let path = scratch_path("wild_east", east as u32);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    // Only the row, because only the row can be seen from here — a wider
    // sweep would hide a ray that had been widened back into a cone.
    data.creatures
        .retain(|c| !(c.position.1 == py && c.position.0 > px));
    data.creatures.push(CreatureSave {
        sortie_index: None,
        boss: false,
        species,
        position: (px + east, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        mitigation: 2,
        tamed: false,
        power: 100.0,
        level: 1,
        xp: 0,
        xp_to_next: 10,
        cronjob: None,
        party_slot: None,
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
        bought_stats: Default::default(),
        routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
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
        rarity: Default::default(),
        nemesis_grudges: 0,
        nemesis_muster_ticks: 0,
        nemesis_home: None,
        nemesis_band: None,
        equipment: Vec::new(),
        program_id: 0,
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
    });
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);

    let game = app.game.as_mut().unwrap();
    game.view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .find(|e| e.pos == (px + east, py) && e.is_hostile)
        .expect("the program was just placed on the eastward row")
        .entity
}

/// A `Pursuing` `NestGuardian` sharing the player's own tile — the
/// `Game::pursuit_tick` fixture `travel-on-the-clock`'s idle-tick battle
/// tests need: the "already adjacent" arm of that function calls
/// `start_battle` with no pathing, so a plain `Game::idle_tick()` (no
/// keypress, no queued walk) opens a fight on the very first tick this
/// runs under. The nest shares the guardian's tile too — `pursuit_tick`'s
/// leash check only asks that the nest's own position stay within
/// `NEST_AGGRO_LEASH_RADIUS`, which distance zero trivially satisfies —
/// and needs no `Nest` component beyond what `Game::load` builds from the
/// `NestSave` entry.
///
/// Call `clear_the_area_around_player` first: this pushes straight into
/// `data.creatures`/`data.nests` without clearing either, so a caller that
/// wants the guardian to be the *only* thing `pursuit_tick` can see has to
/// clear the area itself, before this places anything.
pub(crate) fn place_pursuing_guardian_adjacent(app: &mut App, dx: i32, dy: i32) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("pursuing_guardian", 0);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    let pos = (px + dx, py + dy);
    data.nests.push(save::NestSave {
        species: species.clone(),
        position: pos,
        durability: 10,
        pending_respawns: Vec::new(),
    });
    data.creatures.push(CreatureSave {
        nest_position: Some(pos),
        pursuing: true,
        ..wild_creature_save(species, pos)
    });
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}

/// `n` unposted staff programs and an outpost east of the player — more
/// than one so a picker's highlight can differ from its first row.
pub(crate) fn place_outpost_with_staff_programs_east_of_player(
    app: &mut App,
    n: i32,
) -> (i32, i32) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("outpost_staff", 0);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    let target = (px + 1, py);
    data.creatures.retain(|c| c.position != target);
    data.nests.retain(|n| n.position != target);
    data.link_sites.retain(|&site| site != target);
    data.settlements.0.retain(|_, s| s.tile != target);
    data.outposts.push(save::OutpostSave {
        tile: target,
        biome: feral_processes_engine::world::Biome::Deadlock,
        growth: 0,
        integrity: feral_processes_engine::tuning::OUTPOST_MAX_INTEGRITY,
        stock: Vec::new(),
        stale_ticks: 0,
        cycle_progress: 0,
    });
    for offset in 0..n {
        data.creatures.push(CreatureSave {
            sortie_index: None,
            boss: false,
            species: species.clone(),
            position: (px + 50 + offset, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            mitigation: 2,
            tamed: true,
            power: 100.0,
            level: 1,
            xp: 0,
            xp_to_next: 10,
            cronjob: None,
            party_slot: None,
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
            bought_stats: Default::default(),
            routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
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
            rarity: Default::default(),
            nemesis_grudges: 0,
            nemesis_muster_ticks: 0,
            nemesis_home: None,
            nemesis_band: None,
            equipment: Vec::new(),
            program_id: 1 + offset as u32,
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
            base: tamed_base(10, 3, 2),
            stat_points: 0,
            hold_points: false,
            generation: 0,
            breed_ready_at: 0,
        });
    }

    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    target
}

/// An outpost east of the player already crewed by `n` posted programs,
/// with `stock` on its shelf — the `c`/`C` key-collision test's fixture
/// (I2): `outpost: Some(target)` on each `CreatureSave` posts it directly
/// on load, `game/lifecycle/spawn_saved.rs`'s `pending_outpost_crew` path, rather than
/// driving `Mode::OutpostPost`'s picker once per body.
pub(crate) fn place_outpost_with_crew_and_stock(
    app: &mut App,
    n: u32,
    stock: &[(&str, u32)],
) -> (i32, i32) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("outpost_crew_stock", 0);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    let target = (px + 1, py);
    data.creatures.retain(|c| c.position != target);
    data.nests.retain(|n| n.position != target);
    data.link_sites.retain(|&site| site != target);
    data.settlements.0.retain(|_, s| s.tile != target);
    data.outposts.push(save::OutpostSave {
        tile: target,
        biome: feral_processes_engine::world::Biome::Deadlock,
        growth: 0,
        integrity: feral_processes_engine::tuning::OUTPOST_MAX_INTEGRITY,
        stock: stock
            .iter()
            .map(|(id, qty)| (feral_processes_engine::items::ItemId(id.to_string()), *qty))
            .collect(),
        stale_ticks: 0,
        cycle_progress: 0,
    });
    for i in 0..n {
        data.creatures.push(CreatureSave {
            sortie_index: None,
            boss: false,
            species: species.clone(),
            position: (px + 50 + i as i32, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            mitigation: 2,
            tamed: true,
            power: 100.0,
            level: 1,
            xp: 0,
            xp_to_next: 10,
            cronjob: None,
            party_slot: None,
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
            bought_stats: Default::default(),
            routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
            field_buffs: Vec::new(),
            nest_position: None,
            patrol_position: None,
            study_station: None,
            outpost: Some(target),
            siphon: None,
            jailed: None,
            pursuing: false,
            carrying: None,
            carrying_program: None,
            tidying: None,
            clearing: false,
            rarity: Default::default(),
            nemesis_grudges: 0,
            nemesis_muster_ticks: 0,
            nemesis_home: None,
            nemesis_band: None,
            equipment: Vec::new(),
            program_id: 1 + i,
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
            base: tamed_base(10, 3, 2),
            stat_points: 0,
            hold_points: false,
            generation: 0,
            breed_ready_at: 0,
        });
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    target
}

/// `place_settlement_east_of_player`, plus a `Pursuing` nest guardian
/// standing one tile *north* of the player — for the regression that
/// `pursuit_tick` runs inside the same `tick()` `Game::move_player`'s
/// settlement arm drives (`turn.rs:236`, right after `nest_respawn_tick`),
/// so a provoked guardian already adjacent to the player can start a battle
/// in the very tick that queues the settlement visit.
///
/// The guardian sits north rather than on the settlement's own tile east of
/// the player: `find_wild_creature_at` is the bump ladder's *first* arm and
/// would intercept the press before `find_settlement_at` (the fourth) is
/// ever reached if the guardian sat there instead. North also keeps it off
/// `pursuit_tick`'s field walk entirely — it's already adjacent, so the
/// first per-pursuer check (`chebyshev(pos, player_pos) <= 1`) fires before
/// any step is taken, with no dependency on `pursuit_field`'s route.
///
/// The nest sits on the guardian's own tile — `pursuit_tick`'s leash
/// check only compares `chebyshev` distance between the two, so co-locating
/// them costs nothing and needs no second tile cleared.
pub(crate) fn place_settlement_and_a_pursuing_guardian(
    app: &mut App,
) -> (
    feral_processes_engine::settlements::SettlementKey,
    (i32, i32),
) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("settlement_battle", 0);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    // Two east for `place_settlement_east_of_player`'s reason.
    let target = (px + 2, py);
    let guardian_pos = (px, py - 1);
    let near = |(x, y): (i32, i32)| (x - target.0).abs().max((y - target.1).abs()) <= 2;
    data.creatures
        .retain(|c| !near(c.position) && c.position != guardian_pos);
    data.nests
        .retain(|n| !near(n.position) && n.position != guardian_pos);
    data.link_sites.retain(|&site| !near(site));
    let key = feral_processes_engine::settlements::SettlementKey { rx: 0, ry: 0 };
    data.settlements.0.insert(
        key,
        feral_processes_engine::resources::KnownSettlement {
            tile: target,
            def: generic_settlement_def(),
            visited: false,
        },
    );
    data.nests.push(save::NestSave {
        species: "scrapper".to_string(),
        position: guardian_pos,
        durability: 20,
        pending_respawns: Vec::new(),
    });
    data.creatures.push(CreatureSave {
        sortie_index: None,
        boss: false,
        species: "scrapper".to_string(),
        position: guardian_pos,
        hp: 10,
        max_hp: 10,
        atk: 1,
        mitigation: 1,
        tamed: false,
        power: 100.0,
        level: 1,
        xp: 0,
        xp_to_next: 10,
        cronjob: None,
        party_slot: None,
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
        bought_stats: Default::default(),
        routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
        field_buffs: Vec::new(),
        nest_position: Some(guardian_pos),
        patrol_position: None,
        study_station: None,
        outpost: None,
        siphon: None,
        jailed: None,
        pursuing: true,
        carrying: None,
        carrying_program: None,
        tidying: None,
        clearing: false,
        rarity: Default::default(),
        nemesis_grudges: 0,
        nemesis_muster_ticks: 0,
        nemesis_home: None,
        nemesis_band: None,
        equipment: Vec::new(),
        program_id: 0,
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
    });
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    (key, target)
}

/// The same again, optionally four frames down. `underground` is a parameter
/// rather than a second fixture because the point of the Stack variant is
/// that *nothing else about the game differs* — a row that changes has
/// changed because of the locale and not because two fixtures drifted.
pub(crate) fn app_owning_a_program_and_an_extractor_deep(
    seed: u32,
    routines: &[&str],
    cargo: &[(&str, u32)],
    underground: bool,
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("extract", seed);
    found_the_base(&mut app);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    // Extended, never assigned over: replacing the whole inventory would
    // silently delete the starting kit `Game::new` grants, and the next test
    // written against this fixture would fail for a reason with nothing to do
    // with what it was testing.
    //
    // **Merged row by row rather than pushed**, because a save's inventory
    // is a `Vec` that `Game::load` restores verbatim and
    // `Inventory::count` reads the *first* row keyed to an item. A second
    // row for something the starting kit already carries is invisible: the
    // fixture that stocked 200 Core Fragments beside the kit's 5 compiled
    // against 5.
    for (item, qty) in cargo {
        let id = ItemId::from(*item);
        match data.player.inventory.iter_mut().find(|(i, _)| *i == id) {
            Some((_, have)) => *have += qty,
            None => data.player.inventory.push((id, *qty)),
        }
    }
    let (px, py) = data.player.position;
    data.creatures.push(CreatureSave {
        sortie_index: None,
        boss: false,
        species,
        position: (px + 1, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        mitigation: 1,
        tamed: true,
        power: 100.0,
        level: 1,
        xp: 0,
        xp_to_next: 20,
        cronjob: None,
        party_slot: None,
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
        bought_stats: Default::default(),
        routines: routines.iter().map(|r| r.to_string()).collect(),
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
        rarity: Default::default(),
        nemesis_grudges: 0,
        nemesis_muster_ticks: 0,
        nemesis_home: None,
        nemesis_band: None,
        equipment: Vec::new(),
        program_id: 0,
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
        base: tamed_base(10, 3, 1),
        stat_points: 0,
        hold_points: false,
        generation: 0,
        breed_ready_at: 0,
    });
    // The Workbench is the extraction bench, and counts once Automation is done.
    if !data.researched.iter().any(|r| r == "automation") {
        data.researched.push("automation".to_string());
    }
    data.structures.push(save::StructureSave {
        recipe: Some("core_fragment".into()),
        kind: "extractor".to_string(),
        // Base space, two cells east of the Home the fixture just founded.
        position: (2, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        incubating: Vec::new(),
        study_yield: None,
    });
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: "workbench".to_string(),
        // Far from the party: extraction is ownership, not touch.
        position: (40, 40),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        incubating: Vec::new(),
        study_yield: None,
    });
    if underground {
        data.locale = Locale::Stack {
            depth: 1,
            frames: 2,
            x: 1,
            y: 1,
            facing: feral_processes_engine::stack::Dir::North,
            entrance: data.player.position,
        };
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// `app_owning_a_program_and_a_research_station`'s fixture with the standing
/// structure's kind chosen — one program on the staff and one `kind` at
/// base-space `(2, 0)`, so `stand_beside_the_extractor` puts the party next to it.
pub(crate) fn app_owning_a_program_and_a_station_of(label: &str, kind: &str, seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path(label, seed);
    found_the_base(&mut app);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    data.creatures.push(CreatureSave {
        sortie_index: None,
        boss: false,
        species,
        // Inside the starting pocket's floored box around the
        // origin, unlike a further offset that reads as "somewhere in the
        // base" but sits on unlaid rock with no route to anywhere —
        // `pin_subject`'s "no route to the pen" refusal is what a fixture
        // placed past the pocket's edge actually gets.
        position: (px + 1, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        mitigation: 1,
        tamed: true,
        power: 100.0,
        level: 1,
        xp: 0,
        xp_to_next: 20,
        cronjob: None,
        party_slot: None,
        wielded: false,
        zone: 1,
        custom_name: Some("Study Candidate".to_string()),
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
        bought_stats: Default::default(),
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
        rarity: Default::default(),
        nemesis_grudges: 0,
        nemesis_muster_ticks: 0,
        nemesis_home: None,
        nemesis_band: None,
        equipment: Vec::new(),
        program_id: 0,
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
        base: tamed_base(10, 3, 1),
        stat_points: 0,
        hold_points: false,
        generation: 0,
        breed_ready_at: 0,
    });
    // Footprint 2, clear of both the Home at (0, 0) and the program planted
    // at `px + 5`.
    data.structures.push(save::StructureSave {
        recipe: None,
        kind: kind.to_string(),
        position: (2, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        incubating: Vec::new(),
        study_yield: None,
    });
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// The upgrade-picker counterpart to `app_owning_a_program_and_an_extractor`:
/// a base with an Extractor and exactly **one** tamed program, caught at
/// `program_zone`, with the run itself breached to `breach_zone` first
/// (`Game::warp_to_zone` — the real breach `tests::achievements`'s
/// `breach_and_tick` drives, not a stand-in for it).
///
/// `app_owning_a_program_and_an_extractor`'s own program sits at zone 1 and
/// its base is never breached past zone 1 — fine for testing that the
/// picker opens and computes `to_tier`, but a Mk2 upgrade confirmed against
/// that fixture is refused by `upgrade_ceiling` (zone 1 caps every
/// structure at Mk1) before the program-depth or last-program rules ever
/// run. This fixture exists for a test that needs to drive a confirm past
/// the ceiling and into those rules — see
/// `the_upgrade_pickers_confirm_surfaces_the_engines_refusal`.
///
/// Built the same way `app_owning_a_program_and_an_extractor_deep` is (a save
/// edited by hand, then reloaded — "the engine exposes no way to hand-place
/// a structure from outside the crate", `app_at_a_contract_broker`'s
/// reason), because a hand-placed Extractor costs no program to stand up,
/// where `Game::place_structure` would spend the one program this fixture
/// needs to keep.
pub(crate) fn app_owning_one_deep_program_and_an_extractor(
    seed: u32,
    breach_zone: u32,
    program_zone: u32,
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("deep_extractor", seed);
    found_the_base(&mut app);
    let game = app.game.as_mut().unwrap();
    game.warp_to_zone(breach_zone).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    data.creatures.push(CreatureSave {
        sortie_index: None,
        boss: false,
        species,
        position: (px + 1, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        mitigation: 1,
        tamed: true,
        power: 100.0,
        level: 1,
        xp: 0,
        xp_to_next: 20,
        cronjob: None,
        party_slot: None,
        wielded: false,
        zone: program_zone,
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
        bought_stats: Default::default(),
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
        rarity: Default::default(),
        nemesis_grudges: 0,
        nemesis_muster_ticks: 0,
        nemesis_home: None,
        nemesis_band: None,
        equipment: Vec::new(),
        program_id: 0,
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
        base: tamed_base(10, 3, 1),
        stat_points: 0,
        hold_points: false,
        generation: 0,
        breed_ready_at: 0,
    });
    data.structures.push(save::StructureSave {
        recipe: Some("core_fragment".into()),
        kind: "extractor".to_string(),
        // Base space, two cells east of the Home the fixture just founded.
        position: (2, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        incubating: Vec::new(),
        study_yield: None,
    });
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// `app_at_a_trading_post` with `posts` traders in range instead of one —
/// the case where "sell this" is no longer a complete instruction.
pub(crate) fn app_at_trading_posts(seed: u32, inventory: &[(&str, u32)], posts: i32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("market", seed);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory = inventory
        .iter()
        .map(|(item, qty)| (ItemId::from(*item), *qty))
        .collect();
    let (px, py) = data.player.position;
    data.creatures.push(CreatureSave {
        sortie_index: None,
        boss: false,
        species,
        position: (px + 2, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        mitigation: 2,
        tamed: true,
        power: 100.0,
        level: 1,
        xp: 0,
        xp_to_next: 10,
        cronjob: None,
        party_slot: None,
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
        bought_stats: Default::default(),
        routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
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
        rarity: Default::default(),
        nemesis_grudges: 0,
        nemesis_muster_ticks: 0,
        nemesis_home: None,
        nemesis_band: None,
        equipment: Vec::new(),
        program_id: 0,
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
        base: tamed_base(10, 3, 2),
        stat_points: 0,
        hold_points: false,
        generation: 0,
        breed_ready_at: 0,
    });
    for n in 0..posts {
        data.structures.push(save::StructureSave {
            recipe: None,
            kind: "market".to_string(),
            // Base space, spread along -y so none lands on the Home at the
            // origin, the player, or each other.
            position: (1, -n),
            durability: None,
            tier: None,
            stock_input: Vec::new(),
            stock_output: Vec::new(),
            standing_work: false,
            standing_guard: false,
            denied_items: Vec::new(),
            power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
            build_quality: 1.0,
            racked: Vec::new(),
            hopper: Vec::new(),
            hopper_progress: 0,
            standing_tool: None,
            pod_charged: None,
            incubating: Vec::new(),
            study_yield: None,
        });
    }
    // A trader is a deployed `Structure`, and every structure stands in base
    // space — so a player at a counter is a player out of phase, and buying
    // and selling are `Game::require_base`.
    stand_save_on_field(&mut data, 0, 0);
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// Same as `app_with_player_routines`, but the player also owns one program
/// (parked next to them) and one wild, unowned creature is nearby too — for
/// asserting the field-routine ally picker (`Mode::FieldRoutineAlly`) offers the
/// former and never the latter. Full Power, since affordability isn't what
/// these tests are checking.
pub(crate) fn app_with_owned_and_wild_neighbors(seed: u32, routines: &[&str]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("field_own", seed);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines = expand_routines(&mut data, routines);
    data.player.power = 100.0;
    let (px, py) = data.player.position;
    for (offset, tamed) in [(1, true), (2, false)] {
        data.creatures.push(CreatureSave {
            sortie_index: None,
            boss: false,
            species: species.clone(),
            position: (px + offset, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            mitigation: 2,
            tamed,
            power: 100.0,
            level: 1,
            xp: 0,
            xp_to_next: 10,
            cronjob: None,
            party_slot: None,
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
            bought_stats: Default::default(),
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
            rarity: Default::default(),
            nemesis_grudges: 0,
            nemesis_muster_ticks: 0,
            nemesis_home: None,
            nemesis_band: None,
            equipment: Vec::new(),
            program_id: 0,
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
            base: if tamed { tamed_base(10, 3, 2) } else { None },
            stat_points: 0,
            hold_points: false,
            generation: 0,
            breed_ready_at: 0,
        });
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// `app_with_companions_in_the_party` with gear in the player's cargo, which
/// is where a companion's loadout comes from and returns to.
pub(crate) fn app_with_companions_and_cargo(
    seed: u32,
    count: u32,
    extra_cargo: &[(&str, u32)],
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("party", seed);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    // Extended rather than assigned over, for the reason
    // `app_owning_a_program_and_an_extractor_deep` gives.
    data.player.inventory.extend(
        extra_cargo
            .iter()
            .map(|(item, qty)| (ItemId::from(*item), *qty)),
    );
    let (px, py) = data.player.position;
    for slot in 0..count {
        data.creatures.push(CreatureSave {
            sortie_index: None,
            boss: false,
            species: species.clone(),
            position: (px, py),
            hp: 30,
            max_hp: 30,
            atk: 3,
            mitigation: 1,
            tamed: true,
            power: 100.0,
            level: 1,
            xp: 0,
            xp_to_next: 20,
            cronjob: None,
            party_slot: Some(slot),
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
            bought_stats: Default::default(),
            routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
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
            rarity: Default::default(),
            nemesis_grudges: 0,
            nemesis_muster_ticks: 0,
            nemesis_home: None,
            nemesis_band: None,
            equipment: Vec::new(),
            program_id: 0,
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
            base: tamed_base(30, 3, 1),
            stat_points: 0,
            hold_points: false,
            generation: 0,
            breed_ready_at: 0,
        });
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// The same base with `programs` tamed programs standing on the player's own
/// tile — what staffing the node from the roster needs, since the roster
/// shows a base and the picker offers a roster, and neither existing fixture
/// had both halves.
pub(crate) fn app_inside_a_small_base_with_programs(
    seed: u32,
    underground: bool,
    programs: usize,
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("small_base", seed);
    found_the_base(&mut app);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    // The Home is already standing on base space's origin, laid down by
    // `found_the_base`; this is the machine beside it.
    data.structures.push(save::StructureSave {
        recipe: Some("core_fragment".into()),
        kind: "extractor".to_string(),
        position: (1, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        incubating: Vec::new(),
        study_yield: None,
    });
    for _ in 0..programs {
        data.creatures.push(CreatureSave {
            sortie_index: None,
            boss: false,
            species: species.clone(),
            position: (px, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            mitigation: 1,
            tamed: true,
            power: 100.0,
            level: 1,
            xp: 0,
            xp_to_next: 20,
            cronjob: None,
            party_slot: None,
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
            bought_stats: Default::default(),
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
            rarity: Default::default(),
            nemesis_grudges: 0,
            nemesis_muster_ticks: 0,
            nemesis_home: None,
            nemesis_band: None,
            equipment: Vec::new(),
            program_id: 0,
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
            base: tamed_base(10, 3, 1),
            stat_points: 0,
            hold_points: false,
            generation: 0,
            breed_ready_at: 0,
        });
    }
    let locale = if underground {
        Locale::Stack {
            depth: 1,
            frames: 2,
            x: 1,
            y: 1,
            facing: Dir::North,
            entrance: data.player.position,
        }
    } else {
        // On the Home's tile, the machine one cell east.
        data.player.position = TEST_HOME_CELL;
        Locale::Surface
    };
    data.locale = locale;
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// The same, with the two build rolls set — the other thing about a program
/// no public `Game` method can write, and the axis `Game::build_candidates`
/// sorts on.
pub(crate) fn tame_program_at_zone_with_build_rolls(
    app: &mut App,
    zone: u32,
    assembly: f32,
    extraction: f32,
) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("tame_at_zone", zone);
    let game = app.game.as_mut().unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id
        .clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    data.creatures.push(CreatureSave {
        sortie_index: None,
        boss: false,
        species,
        position: (px, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        mitigation: 2,
        tamed: true,
        power: 100.0,
        level: 1,
        xp: 0,
        xp_to_next: 10,
        cronjob: None,
        party_slot: None,
        wielded: false,
        zone,
        custom_name: None,
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
        assembly_roll: assembly,
        extraction_roll: extraction,
        fusions: 0,
        refactors: 0,
        purchased_tiers: 0,
        ring: 0,
        talents: Vec::new(),
        bought_stats: Default::default(),
        routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
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
        rarity: Default::default(),
        nemesis_grudges: 0,
        nemesis_muster_ticks: 0,
        nemesis_home: None,
        nemesis_band: None,
        equipment: Vec::new(),
        program_id: 0,
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
        base: tamed_base(10, 3, 2),
        stat_points: 0,
        hold_points: false,
        generation: 0,
        breed_ready_at: 0,
    });
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}
