//! Fixtures shared by the app-core tests.

use std::sync::atomic::{AtomicU32, Ordering};

use feral_processes_engine::affixes::AffixId;
use feral_processes_engine::components::Rarity;
use feral_processes_engine::resources::Locale;
use feral_processes_engine::save::{self, CreatureSave};
use feral_processes_engine::stack::{Dir, FrameSpec, generate};

use crate::*;

pub(crate) fn test_assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// The shipped `dev-arenas/`, for fixtures that never open the arena. A
/// test that *saves* a scenario takes a scratch copy instead — see
/// `tests/arena.rs::app_with_scratch_arenas` — since this one is source.
pub(crate) fn arenas_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dev-arenas")
}

/// A scratch path no other fixture call can be using.
///
/// Keying these on `(fixture, seed)` alone was not enough: the test binary
/// runs its cases as concurrent threads, so two tests that reach for the
/// same fixture with the same seed shared one file and raced — one loading
/// what the other had half-written. That is not hypothetical, it is how
/// `a_full_party_is_asked_slot_by_slot_and_only_then_resolves` failed in the
/// suite while passing alone, and `app_at_a_trading_post(921, ..)` had the
/// same collision waiting in two other tests.
///
/// A counter rather than a timestamp or a random suffix, deliberately: it is
/// unique across the process without making the run depend on a clock or on
/// RNG nobody seeded.
fn scratch_path(fixture: &str, seed: u32) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "feral_processes_appcore_{fixture}_{seed}_{unique}.sav"
    ))
}

pub(crate) fn test_app(seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let saves_dir = std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}_saves"));
    let history_path =
        std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}.log"));
    let profile_path =
        std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}_profile.ron"));
    let _ = std::fs::remove_file(&profile_path);
    let mut app = App::new(
        assets_dir.clone(),
        saves_dir,
        history_path,
        profile_path,
        arenas_dir(),
        std::env::temp_dir().join(format!(
            "feral_processes_appcore_test_{seed}_telemetry.jsonl"
        )),
    );
    app.game = Game::new(seed, DifficultyMode::Forgiving, &assets_dir).ok();
    app.mode = Mode::Playing;
    app
}

/// A game where the player owns `count` programs parked well outside
/// `MENU_SCAN_RADIUS` — a cronjob worker left at a far-flung node, say.
/// Built by editing a save and reloading it, since the engine deliberately
/// exposes no way to hand-place a tamed program from outside the crate.
pub(crate) fn app_owning_distant_programs(seed: u32, count: i32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("distant", seed);
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    for i in 0..count {
        data.creatures.push(CreatureSave {
            species: species.clone(),
            position: (px + MENU_SCAN_RADIUS + 10 + i, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            def: 2,
            tamed: true,
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
            fusions: 0,
            refactors: 0,
            purchased_tiers: 0,
            routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
            field_buffs: Vec::new(),
            nest_position: None,
            pursuing: false,
            carrying: None,
            rarity: Default::default(),
            equipment: Vec::new(),
            staff: false,
        });
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app
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
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    // Only the row, because only the row can be seen from here — a wider
    // sweep would hide a ray that had been widened back into a cone.
    data.creatures
        .retain(|c| !(c.position.1 == py && c.position.0 > px));
    data.creatures.push(CreatureSave {
        species,
        position: (px + east, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        def: 2,
        tamed: false,
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
        fusions: 0,
        refactors: 0,
        purchased_tiers: 0,
        routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
        field_buffs: Vec::new(),
        nest_position: None,
        pursuing: false,
        carrying: None,
        rarity: Default::default(),
        equipment: Vec::new(),
        staff: false,
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

/// A game where the player owns one tamed program carrying `routines` and
/// has a Compiler standing, so the extraction flow has both of its
/// preconditions. Built by editing a save and reloading it, for the same
/// reason `app_owning_distant_programs` is.
pub(crate) fn app_owning_a_program_and_a_compiler(seed: u32, routines: &[&str]) -> App {
    app_owning_a_program_and_a_compiler_with_cargo(seed, routines, &[])
}

/// The same, plus exactly `cargo` in the player's inventory — the refactor
/// flow needs a program *and* something to spend on it, and the second half
/// is the one that changes during a run.
pub(crate) fn app_owning_a_program_and_a_compiler_with_cargo(
    seed: u32,
    routines: &[&str],
    cargo: &[(&str, u32)],
) -> App {
    app_owning_a_program_and_a_compiler_deep(seed, routines, cargo, false)
}

/// The same again, optionally four frames down. `underground` is a parameter
/// rather than a second fixture because the point of the Stack variant is
/// that *nothing else about the game differs* — a row that changes has
/// changed because of the locale and not because two fixtures drifted.
pub(crate) fn app_owning_a_program_and_a_compiler_deep(
    seed: u32,
    routines: &[&str],
    cargo: &[(&str, u32)],
    underground: bool,
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("extract", seed);
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    // Extended, never assigned over: replacing the whole inventory would
    // silently delete the starting kit `Game::new` grants, and the next test
    // written against this fixture would fail for a reason with nothing to do
    // with what it was testing.
    data.player
        .inventory
        .extend(cargo.iter().map(|(item, qty)| (ItemId::from(*item), *qty)));
    let (px, py) = data.player.position;
    data.creatures.push(CreatureSave {
        species,
        position: (px + 1, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        def: 1,
        tamed: true,
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
        fusions: 0,
        refactors: 0,
        purchased_tiers: 0,
        routines: routines.iter().map(|r| r.to_string()).collect(),
        field_buffs: Vec::new(),
        nest_position: None,
        pursuing: false,
        carrying: None,
        rarity: Default::default(),
        equipment: Vec::new(),
        staff: false,
    });
    data.structures.push(save::StructureSave {
        kind: "compiler".to_string(),
        position: (px + 30, py + 30),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
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

/// A game where the player stands next to a Black Market — the shipped
/// trader that buys programs as well as items — holding exactly
/// `inventory`, and owning one tamed program so the trader's program rows
/// are populated too. Built by editing a save and reloading it, for the
/// same reason `app_owning_a_program_and_a_compiler` is: staging a trading
/// post through the build flow needs a Home, build clearance and 16 Core
/// Fragments, and the player starts with 5.
pub(crate) fn app_at_a_trading_post(seed: u32, inventory: &[(&str, u32)]) -> App {
    app_at_trading_posts(seed, inventory, 1)
}

/// `app_at_a_trading_post` with `posts` traders in range instead of one —
/// the case where "sell this" is no longer a complete instruction.
pub(crate) fn app_at_trading_posts(seed: u32, inventory: &[(&str, u32)], posts: i32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("market", seed);
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory = inventory
        .iter()
        .map(|(item, qty)| (ItemId::from(*item), *qty))
        .collect();
    let (px, py) = data.player.position;
    data.creatures.push(CreatureSave {
        species,
        position: (px + 2, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        def: 2,
        tamed: true,
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
        fusions: 0,
        refactors: 0,
        purchased_tiers: 0,
        routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
        field_buffs: Vec::new(),
        nest_position: None,
        pursuing: false,
        carrying: None,
        rarity: Default::default(),
        equipment: Vec::new(),
        staff: false,
    });
    for n in 0..posts {
        data.structures.push(save::StructureSave {
            kind: "market".to_string(),
            // Spread along -y so none lands on the player, the program at
            // `px + 2` or each other.
            position: (px + 1, py - n),
            durability: None,
            tier: None,
            stock_input: Vec::new(),
            stock_output: Vec::new(),
            standing_work: false,
            standing_guard: false,
        });
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A game where the player has `routines` installed (in place of the
/// default `decompile`) and `hunger` set to a chosen level, so a field-cast
/// test can pin affordability on either side of a routine's `power_cost`
/// exactly. Built by editing a save and reloading it, for the same reason
/// `app_owning_a_program_and_a_compiler` is.
pub(crate) fn app_with_player_routines(seed: u32, routines: &[&str], hunger: f32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("field", seed);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines = routines.iter().map(|r| r.to_string()).collect();
    data.player.hunger = hunger;
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// An app whose player carries `routines` and is standing on depth 1 of a
/// Stack frame, at the frame's own entry cell facing north.
///
/// Written into the save rather than walked to: `Game::enter_stack` is
/// `pub(crate)` in the engine and app-core is outside that boundary, and a
/// fixture that hunted the zone for a surface link would be a test about
/// world generation.
pub(crate) fn app_underground_with_routines(seed: u32, routines: &[&str]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("field_stack", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines = routines.iter().map(|r| r.to_string()).collect();
    data.player.fatigue = 100.0;
    data.locale = feral_processes_engine::resources::Locale::Stack {
        depth: 1,
        frames: 2,
        // Overwritten below by the frame's own entry — the save only has to
        // put the party *somewhere* legal for the frame to regenerate.
        x: 1,
        y: 1,
        facing: feral_processes_engine::stack::Dir::North,
        entrance: data.player.position,
    };
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// Same as `app_with_player_routines`, but the player also owns one program
/// (parked next to them) and one wild, unowned creature is nearby too — for
/// asserting the field-cast ally picker (`Mode::FieldCastAlly`) offers the
/// former and never the latter. Full Power, since affordability isn't what
/// these tests are checking.
pub(crate) fn app_with_owned_and_wild_neighbors(seed: u32, routines: &[&str]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("field_own", seed);
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines = routines.iter().map(|r| r.to_string()).collect();
    data.player.hunger = 100.0;
    let (px, py) = data.player.position;
    for (offset, tamed) in [(1, true), (2, false)] {
        data.creatures.push(CreatureSave {
            species: species.clone(),
            position: (px + offset, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            def: 2,
            tamed,
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
            fusions: 0,
            refactors: 0,
            purchased_tiers: 0,
            routines: Vec::new(),
            field_buffs: Vec::new(),
            nest_position: None,
            pursuing: false,
            carrying: None,
            rarity: Default::default(),
            equipment: Vec::new(),
            staff: false,
        });
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A game where the player has one program standing in the active `Party`,
/// so a battle opens with two planning slots rather than the single one
/// every other battle fixture here produces. Built by editing a save and
/// reloading it, for the reason `app_owning_a_program_and_a_compiler` is:
/// nothing outside the engine can hand-place a tamed program, and
/// `party_slot` is the save field that puts one on the roster.
pub(crate) fn app_with_companions_in_the_party(seed: u32, count: u32) -> App {
    app_with_companions_and_cargo(seed, count, &[])
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
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    // Extended rather than assigned over, for the reason
    // `app_owning_a_program_and_a_compiler_deep` gives.
    data.player.inventory.extend(
        extra_cargo
            .iter()
            .map(|(item, qty)| (ItemId::from(*item), *qty)),
    );
    let (px, py) = data.player.position;
    for slot in 0..count {
        data.creatures.push(CreatureSave {
            species: species.clone(),
            position: (px, py),
            hp: 30,
            max_hp: 30,
            atk: 3,
            def: 1,
            tamed: true,
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
            fusions: 0,
            refactors: 0,
            purchased_tiers: 0,
            routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
            field_buffs: Vec::new(),
            nest_position: None,
            pursuing: false,
            carrying: None,
            rarity: Default::default(),
            equipment: Vec::new(),
            staff: false,
        });
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// An `App` standing on the entry cell of Stack frame 1.
///
/// Built by editing a save and reloading it, the same trick
/// `app_owning_distant_programs` uses: the engine deliberately exposes no
/// way to drop the player into the Stack from outside the crate, since on a
/// real run that only ever happens by walking onto an entrance.
pub(crate) fn app_underground(seed: u32) -> App {
    // Counted rather than keyed on the seed alone: tests run in parallel and
    // several share a seed, so a seed-named scratch file has two of them
    // reading and deleting the same path.
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path =
        std::env::temp_dir().join(format!("feral_processes_appcore_stack_{seed}_{unique}.sav"));
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let spec = FrameSpec {
        world_seed: data.seed,
        entrance: data.player.position,
        depth: 1,
        frames: 2,
    };
    let entry = generate(spec).entry;
    data.locale = Locale::Stack {
        depth: spec.depth,
        frames: spec.frames,
        x: entry.0,
        y: entry.1,
        facing: Dir::North,
        entrance: spec.entrance,
    };
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A game where the player wears `weapon` — an item id plus the gear level
/// it was equipped at (see `components::EquippedItem`) — carries
/// `inventory`, and stands `zone` sectors deep.
///
/// Written straight into the save rather than staged through `Game::equip`,
/// which always stamps gear with the *current* zone level. The gap between
/// the level your weapon remembers and the one the zone would grant it now
/// is exactly what the swap screen's delta column reports, so there is no
/// other way to set it up. `Stats` is deliberately left at its unequipped
/// value: `equip_swap_rows` reads the recorded gear level and the item
/// catalogue, never the player's current attack.
pub(crate) fn app_wearing_weapon(
    seed: u32,
    weapon: Option<(&str, u32)>,
    inventory: &[(&str, u32)],
    zone: u32,
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("gear", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory = inventory
        .iter()
        .map(|(item, qty)| (ItemId::from(*item), *qty))
        .collect();
    if let Some((item, level)) = weapon {
        data.player.weapon = Some(ItemId::from(item));
        data.player.weapon_level = level;
    }
    data.zone = zone;
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A game `zone` sectors deep, wearing nothing, carrying one copy of `item`
/// with `affix` on it.
///
/// Written into the save rather than staged through play because nothing
/// public grants an affixed copy: `Game::grant_gear_drop` is the only way one
/// enters the game and it is the engine's own, rolled off `GameRng`. An empty
/// slot is the point — with nothing worn, what the swap picker prints for a
/// candidate is what equipping it must actually grant, with no outgoing item's
/// figure in between.
pub(crate) fn app_carrying_affixed_gear(seed: u32, item: &str, affix: &str, zone: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("affixed", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.zone = zone;
    data.player.gear_copies = vec![(affixed_gear(item, affix), 1)];
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// An ordinary, unfused copy of `item` wearing `affix` — the affixed twin of
/// `gear` above.
pub(crate) fn affixed_gear(item: &str, affix: &str) -> GearCopy {
    GearCopy {
        item: ItemId::from(item),
        rarity: Rarity::Ordinary,
        tier: 0,
        affix: Some(AffixId::from(affix)),
    }
}

/// Opens a screen the way a player now has to: through its group menu.
///
/// Tests that used to press one retired key go through this instead of
/// hard-coding a row number — the rows are filtered by what is currently
/// possible (see `App::base_menu_rows`), so a row's position depends on the
/// fixture and pinning it would make these tests break for the wrong reason.
pub(crate) fn open_via_menu(app: &mut App, group: char, label: &str) {
    app.handle_key(GameKey::Char(group));
    let rows = match group {
        'b' => app.base_menu_rows(),
        'p' => app.party_menu_rows(),
        other => panic!("{other} is not a group menu key"),
    };
    let labels: Vec<_> = rows.iter().map(|r| r.label).collect();
    let idx = rows
        .iter()
        .position(|r| r.label == label)
        .unwrap_or_else(|| panic!("{label:?} is not offered right now; rows: {labels:?}"));
    app.handle_key(GameKey::Char(menu_shortcut(idx)));
}

/// A base the player is standing inside: a Home one tile north and an
/// ordinary Mining Node one tile east, with nothing to the south or west.
///
/// Built by editing a save for the same reason `app_owning_a_program_and_a_
/// compiler` is — deploying a second structure through the build flow needs
/// materials the player does not start with, and the layout is the whole
/// point here: the direct demolish key has to tell a Home from anything else
/// and both from an empty tile, which is three neighbours of one tile.
pub(crate) fn app_inside_a_small_base(seed: u32, underground: bool) -> App {
    app_inside_a_small_base_with_programs(seed, underground, 0)
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
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    for (kind, position) in [("home", (px, py - 1)), ("mining_node", (px + 1, py))] {
        data.structures.push(save::StructureSave {
            kind: kind.to_string(),
            position,
            durability: None,
            tier: None,
            stock_input: Vec::new(),
            stock_output: Vec::new(),
            standing_work: false,
            standing_guard: false,
        });
    }
    for _ in 0..programs {
        data.creatures.push(CreatureSave {
            species: species.clone(),
            position: (px, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            def: 1,
            tamed: true,
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
            fusions: 0,
            refactors: 0,
            purchased_tiers: 0,
            routines: Vec::new(),
            field_buffs: Vec::new(),
            nest_position: None,
            pursuing: false,
            carrying: None,
            rarity: Default::default(),
            equipment: Vec::new(),
            staff: false,
        });
    }
    if underground {
        data.locale = Locale::Stack {
            depth: 1,
            frames: 2,
            x: 1,
            y: 1,
            facing: Dir::North,
            entrance: data.player.position,
        };
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A carried copy of `item` at fusion `tier`, ordinary rare tier — the
/// app-core twin of the engine's test helper of the same name, and here for
/// the same reason: the screens and handlers now name a whole
/// `items::GearCopy`, and almost every test means the plain one.
pub(crate) fn gear(item: &ItemId, tier: u32) -> GearCopy {
    GearCopy {
        item: item.clone(),
        rarity: Rarity::Ordinary,
        tier,
        affix: None,
    }
}

/// A game with a Contract Broker one tile east of the player, so the
/// contracts screen has a board on it. `underground` drops the party into a
/// Stack frame afterwards, which is what makes the offers half go away while
/// the active half stays.
///
/// Built by editing a save for the reason `app_at_trading_posts` is: the
/// engine exposes no way to hand-place a structure from outside the crate.
pub(crate) fn app_at_a_contract_broker(seed: u32, underground: bool) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("broker", seed);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    data.structures.push(save::StructureSave {
        kind: "contract_broker".to_string(),
        position: (px + 1, py),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
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
    app
}
