pub mod abilities;
pub mod achievements;
pub mod affixes;
pub mod arena;
pub mod balance_sim;
pub mod base_grid;
pub mod battle;
pub mod caravans;
pub mod components;
pub mod contracts;
pub(crate) mod derive;
pub mod descriptions;
pub mod difficulty;
pub mod environment;
mod game;
pub mod help;
pub mod items;
pub mod items_db;
pub mod memories;
pub mod nemesis;
pub mod perks;
pub mod policy;
pub mod progression;
pub mod research;
pub mod resources;
pub mod rock;
pub mod save;
pub mod sectors;
pub mod species;
pub mod stack;
pub mod structures;
pub mod systems;
pub mod talents;
pub mod taming;
pub mod telemetry;
pub mod text;
pub mod tuning;
pub mod views;
pub mod world;

use crate::tuning::{
    BASE_PET_CAPACITY, MAX_INDIVIDUAL_ROLL, MAX_PARTY_SIZE, MIN_INDIVIDUAL_ROLL,
    ZONE_PORTAL_COST_GROWTH_PERCENT,
};
use std::collections::HashMap;
use std::path::Path;

pub use bevy_ecs::prelude::Entity;
use bevy_ecs::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use abilities::{AbilityDb, AbilityDef, AbilityEffect, AbilityTarget};
use affixes::{AffixDb, AffixId};
use battle::{
    ActionKind, ActionOption, AllyOption, BattleAction, EnemyGroup, PartyCommand, PartyCommandKind,
    SpecialOption, TargetSpec,
};
use components::{
    AbilityCooldowns, ActiveBuff, ActiveFieldBuff, ActiveStatus, BaseAnchor, Boss, BuffKind,
    BuffSource, Caravan, CaravanStage, Carrying, CombatBuff, Creature, CustomName, Decompiler,
    DigSite, Durability, Equipment, EquippedItem, Experience, FieldBuff, FieldBuffKind,
    FusionCount, GearCopies, Glyph, GlyphColor, Hostile, Inventory, KernelRing, MachineStatus,
    Memories, Memory, Nemesis, Nest, NestGuardian, POWER_MAX, Perks, Player, Position, Potential,
    PowerReserve, ProgramId, PurchasedTiers, Pursuing, Rarity, Refactors, ResourceNode, Routines,
    StackSpawn, StandingJob, Stats, StatusEffects, StatusKind, Stock, Stranded, Structure,
    StructureTier, SurfaceLink, Talents, Tamed, Task, TaskKind, Temporary, WanderAi, ZonePortal,
};
pub use game::base::work_orders::{OrderPriority, WorkOrder};
pub use game::caravan::CaravanReach;
pub use game::contracts::{BrokerReach, ContractRefusal};
pub use game::party::ProgramRole;
pub use game::stack_view::ExamineDir;
use items::{EquipmentSlot, EquipmentStats, GearCopy, ItemCategory, ItemId, ids};
use items_db::{ItemDb, ItemDef};
pub use perks::{Perk, PerkDb, PerkDef};
use research::{ResearchDb, ResearchDef};
pub use research::{ResearchId, ResearchRecipe};
use resources::{
    AnchorEntity, BattleRewards, BattleState, BattleTimeline, BuybackLedger, ClosingRoster,
    CurrentStack, EffectQueue, GameClock, GameOver, GameRng, KnownRoutines, Locale, MessageLog,
    Party, PlayerEntity, Research, RosterFrame, StackMemory, WieldedProgram, XpTally, ZoneLevel,
    ZoneSpawnPoint,
};
pub use resources::{
    DifficultyMode, EffectKind, LabourDemand, LogEntry, LogLine, MESSAGE_LOG_CAP, MessageKind,
    MessageSource, SlotShift, VisualEffect, condense,
};
use species::{Affinities, SpeciesDb, SpeciesDef, SpeciesId};
use structures::{StructureDb, StructureDef, StructureId, TradeDef};
pub use views::*;
use world::{Biome, Tile, WorldMap};

/// Longest name a player can give a fused program (see
/// `Game::fuse_companions`) — enforced by truncation, not rejection, so a
/// too-long name just gets shortened rather than failing the fusion.
pub const MAX_CUSTOM_NAME_LEN: usize = 12;

/// How many characters the `standing_on` row will take, descriptive clause
/// and key-prompt suffix together.
///
/// That row is centred, drawn at `Metrics::font_size` and **unwrapped** —
/// nothing clips it, so an over-long line runs off the pane rather than
/// eliding. 48 leaves headroom over the longest literal the row carried
/// before the bank existed ("Rotten substrate  — moving on costs", 35) while
/// still keeping the clause a phrase rather than a sentence.
///
/// Proved in pixels by `crates/gui`'s
/// `the_longest_underfoot_line_fits_the_stack_pane`, at the narrowest
/// window size the UI supports. A number asserted in one place and repeated
/// in a doc comment somewhere else is how this measurement rots.
pub const MAX_UNDERFOOT_LINE: usize = 48;

/// The quantity a zone-portal structure costing `base_qty` of an item
/// charges at `zone`.
pub(crate) fn zone_portal_cost(base_qty: u32, zone: u32) -> u32 {
    base_qty + base_qty * ZONE_PORTAL_COST_GROWTH_PERCENT * zone.saturating_sub(1) / 100
}

/// `StructureDef::id` of the one structure `Game::place_structure` will
/// let you deploy before any other — everything else requires a Home
/// already standing somewhere. Also what pins the build menu's ordering
/// (see `StructureDb::all`).
const HOME_STRUCTURE_ID: &str = "home";

/// The entire public API surface the renderer talks to via app-core. Its
/// methods live in the `game` module, split by topic; the renderer never
/// touches the ECS `World` directly.
pub struct Game {
    world: World,
    schedule: Schedule,
}

#[cfg(test)]
<<<<<<< Updated upstream
mod tests;
=======
mod tests {
    use super::*;
    use std::path::Path;

    fn test_assets_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    /// Deploys a Home just off the player's current position (`dx`, `dy`
    /// relative, so it doesn't collide with whatever the caller places
    /// next) — `place_structure` refuses anything else until a Home
    /// exists, so most structure-placement tests need this first.
    fn place_home(game: &mut Game, dx: i32, dy: i32) {
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::CoreFragment, 5);
        game.place_structure("home", dx, dy).unwrap();
    }

    #[test]
    fn award_loot_grants_the_species_work_resource() {
        let mut game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| s.work_resource.is_some())
            .expect("at least one species should have a work_resource for this test");
        let resource = species.work_resource.unwrap();

        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position { x: 0, y: 0 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        let before = game.world.get::<Inventory>(player).unwrap().count(resource);
        game.award_loot(player, wild);
        let after = game.world.get::<Inventory>(player).unwrap().count(resource);

        assert!(
            after > before,
            "defeating the program should have granted {resource:?}"
        );
        let tagged = game
            .message_log(10)
            .into_iter()
            .any(|(kind, _)| kind == MessageKind::Loot);
        assert!(
            tagged,
            "a resource drop should log a MessageKind::Loot line, got: {:?}",
            game.message_log(10)
        );
    }

    #[test]
    fn award_loot_grants_nothing_for_species_without_a_work_resource() {
        let mut game = Game::new(2, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| s.work_resource.is_none() && s.equipment_drop.is_none())
            .expect("at least one species should have neither a work_resource nor an equipment_drop for this test");

        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position { x: 0, y: 0 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        // Portal Fragments are a universal drop independent of species, so
        // count everything *except* those to check the species-specific
        // channels stayed silent.
        let count_non_portal = |game: &Game| -> u32 {
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .items
                .iter()
                .filter(|(item, _)| *item != ItemId::PortalFragment)
                .map(|(_, q)| *q)
                .sum()
        };
        let before = count_non_portal(&game);
        game.award_loot(player, wild);
        let after = count_non_portal(&game);

        assert_eq!(
            before, after,
            "no-resource species shouldn't add anything besides a possible portal fragment"
        );
    }

    #[test]
    fn inspect_reports_species_detail_without_starting_a_battle() {
        let mut game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
                Position { x: 5, y: 5 },
                Stats {
                    hp: species.base_hp,
                    max_hp: species.base_hp,
                    atk: species.base_atk,
                    def: species.base_def,
                },
            ))
            .id();

        let view = game
            .inspect(wild)
            .expect("wild creature should be inspectable");
        assert_eq!(view.name, species.name);
        assert!(view.is_hostile);
        assert!(!view.is_tamed);
        assert_eq!(view.max_hp, species.base_hp);
        assert!((0.0..=1.0).contains(&view.decompile_chance));
        assert!(
            !game.has_active_battle(),
            "inspecting must not trigger an intrusion"
        );
    }

    #[test]
    fn inspect_returns_none_for_non_creature_entities() {
        let game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        assert!(game.inspect(player).is_none());
    }

    #[test]
    fn use_symlink_teleports_the_player_to_the_structure_and_charges_the_cost() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.teleport_cost.is_some())
            .expect("a symlink-capable structure (Home) should exist");
        let cost = def.teleport_cost.clone().unwrap();

        let home = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 50, y: 50 },
                Glyph {
                    ch: def.glyph,
                    color: def.color,
                },
            ))
            .id();

        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                inv.add(*item, *qty);
            }
        }
        let before: Vec<u32> = cost
            .iter()
            .map(|(item, _)| game.world.get::<Inventory>(player).unwrap().count(*item))
            .collect();

        let targets = game.symlink_targets();
        assert!(
            targets.iter().any(|t| t.entity == home),
            "Home should be a symlink target"
        );

        game.use_symlink(home).unwrap();

        let pos = *game.world.get::<Position>(player).unwrap();
        assert_eq!(
            pos,
            Position { x: 50, y: 50 },
            "symlink should teleport the player onto the structure"
        );
        for ((item, qty), before) in cost.iter().zip(before) {
            let after = game.world.get::<Inventory>(player).unwrap().count(*item);
            assert_eq!(
                after,
                before - qty,
                "the teleport cost should be fully consumed"
            );
        }
    }

    #[test]
    fn use_symlink_fails_without_enough_of_the_cost() {
        let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.teleport_cost.is_some())
            .expect("a symlink-capable structure (Home) should exist");

        let home = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 20, y: 20 },
                Glyph {
                    ch: def.glyph,
                    color: def.color,
                },
            ))
            .id();

        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
        }

        let before_pos = *game.world.get::<Position>(player).unwrap();
        assert!(game.use_symlink(home).is_err());
        let after_pos = *game.world.get::<Position>(player).unwrap();
        assert_eq!(
            before_pos, after_pos,
            "a failed symlink shouldn't move the player"
        );
    }

    #[test]
    fn place_structure_rejects_anything_but_home_until_a_home_exists() {
        let mut game = Game::new(300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::CoreFragment, 20);

        assert!(
            game.place_structure("armory", 1, 0).is_err(),
            "nothing should be buildable before a Home exists"
        );
        assert_eq!(
            game.view_entities(10, 10)
                .into_iter()
                .filter(|e| e.is_structure)
                .count(),
            0,
            "the rejected placement shouldn't have spawned anything"
        );

        game.place_structure("home", -1, 0).unwrap();
        game.place_structure("armory", 1, 0).unwrap();
        assert_eq!(
            game.view_entities(10, 10)
                .into_iter()
                .filter(|e| e.is_structure)
                .count(),
            2,
            "once a Home exists, other structures should be buildable"
        );
    }

    #[test]
    fn place_structure_rejects_a_second_home() {
        let mut game = Game::new(301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game, -1, 0);

        let err = game
            .place_structure("home", 1, 0)
            .expect_err("a second Home shouldn't be buildable while one already exists");
        assert!(err.contains("already deployed"), "unexpected error: {err}");
    }

    #[test]
    fn place_structure_rejects_building_beyond_max_distance_from_home() {
        let mut game = Game::new(302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::CoreFragment, 20);
        place_home(&mut game, 0, 1);

        // Walk far enough away that the next placement lands outside the
        // 15-tile build radius from Home.
        game.world.get_mut::<Position>(player).unwrap().x += 20;
        let err = game
            .place_structure("armory", 1, 0)
            .expect_err("structures more than 15 tiles from Home shouldn't be buildable");
        assert!(err.contains("Too far from Home"), "unexpected error: {err}");

        // Walking back within range should make it buildable again.
        game.world.get_mut::<Position>(player).unwrap().x -= 20;
        game.place_structure("armory", 1, 0)
            .expect("building back within 15 tiles of Home should succeed");
    }

    #[test]
    fn remove_structure_refunds_a_percentage_of_its_build_cost() {
        let mut game = Game::new(303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::CoreFragment, 20);
        place_home(&mut game, -1, 0);
        game.place_structure("armory", 1, 0).unwrap();
        let armory = game
            .view_entities(10, 10)
            .into_iter()
            .find(|e| e.is_structure && !e.is_home)
            .unwrap()
            .entity;

        let before = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::CoreFragment);
        game.remove_structure(armory).unwrap();
        let after = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::CoreFragment);

        assert!(
            after > before,
            "demolishing a structure should refund some of its build cost"
        );
        assert_eq!(
            game.view_entities(10, 10)
                .into_iter()
                .filter(|e| e.is_structure)
                .count(),
            1,
            "only the Home should remain after demolishing the armory"
        );
    }

    #[test]
    fn removing_home_cascades_to_destroy_every_other_structure_and_refunds_each() {
        let mut game = Game::new(304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::CoreFragment, 50);
        place_home(&mut game, -1, 0);
        game.place_structure("armory", 1, 0).unwrap();
        game.place_structure("fabricator", 0, 1).unwrap();
        let home = game
            .view_entities(10, 10)
            .into_iter()
            .find(|e| e.is_home)
            .unwrap()
            .entity;

        let before = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::CoreFragment);
        game.remove_structure(home).unwrap();
        let after = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::CoreFragment);

        assert_eq!(
            game.view_entities(10, 10)
                .into_iter()
                .filter(|e| e.is_structure)
                .count(),
            0,
            "removing Home should cascade to remove every other structure too"
        );
        assert!(
            after > before,
            "the cascade should refund a share of every demolished structure's cost, including Home's own"
        );
    }

    #[test]
    fn armory_and_fabricator_are_not_cronjob_workable() {
        let game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        for id in ["armory", "fabricator"] {
            let def = game
                .structure_defs()
                .into_iter()
                .find(|d| d.id == id)
                .unwrap_or_else(|| panic!("{id}.ron should load as a structure"));
            assert!(
                def.work.is_none(),
                "{id} should unlock crafting instead of being cronjob-workable"
            );
        }
    }

    #[test]
    fn building_an_armory_unlocks_firewall_plating_crafting_for_portal_fragments() {
        let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(
            game.craft_recipes()
                .iter()
                .all(|r| r.result != ItemId::FirewallPlating),
            "Firewall Plating shouldn't be craftable before an Armory is built"
        );

        place_home(&mut game, -1, 0);
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::CoreFragment, 18);
        game.place_structure("armory", 1, 0).unwrap();

        let recipe = game
            .craft_recipes()
            .into_iter()
            .find(|r| r.result == ItemId::FirewallPlating)
            .expect("building an Armory should unlock Firewall Plating crafting");
        assert_eq!(
            recipe.cost,
            vec![(ItemId::PortalFragment, FIREWALL_PLATING_PORTAL_COST)]
        );

        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::PortalFragment, 10);
        game.craft(ItemId::FirewallPlating, 1).unwrap();
        assert_eq!(
            game.world
                .get::<Inventory>(game.player_entity())
                .unwrap()
                .count(ItemId::FirewallPlating),
            1
        );
    }

    #[test]
    fn cronjob_assignment_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(6, DifficultyMode::Forgiving, &assets).unwrap();

        let structure_def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.work.is_some())
            .expect("at least one workable structure should exist");
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: structure_def.id.clone(),
                },
                Position { x: 3, y: 3 },
                ResourceNode {
                    resource: structure_def.work.as_ref().unwrap().produces,
                    amount: 20,
                    capacity: 20,
                    level: None,
                },
            ))
            .id();

        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let player = game.player_entity();
        game.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position { x: 3, y: 4 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                def: 1,
            },
            Tamed { owner: player },
            Experience::default(),
            Task {
                kind: TaskKind::GatherResource,
                target: structure,
                progress: 3,
                required: 6,
            },
        ));

        let path = std::env::temp_dir().join(format!(
            "feral_processes_cronjob_test_{}_{}.bin",
            std::process::id(),
            6
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let mut query = loaded.world.query::<&Task>();
        let task = query
            .iter(&loaded.world)
            .next()
            .expect("restored creature should still have its cronjob task");
        assert_eq!(task.progress, 3);
        assert_eq!(task.required, 6);
        let target_pos = loaded
            .world
            .get::<Position>(task.target)
            .expect("task target should resolve to a structure entity");
        assert_eq!((target_pos.x, target_pos.y), (3, 3));
    }

    #[test]
    fn a_mined_out_node_refills_instead_of_stalling_the_cronjob() {
        let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: ItemId::CoreFragment,
                    amount: 1,
                    capacity: 2,
                    level: None,
                },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: 1,
        });

        // First tick mines the last unit down to 0.
        game.tick();
        assert_eq!(game.world.get::<ResourceNode>(structure).unwrap().amount, 0);

        // The node refills to capacity on the next tick rather than
        // leaving the assigned creature permanently idle.
        game.tick();
        assert_eq!(game.world.get::<ResourceNode>(structure).unwrap().amount, 1);
        assert!(
            game.world.get::<Task>(worker).is_some(),
            "the cronjob should keep running once the node refills"
        );
    }

    #[test]
    fn cronjob_work_grants_no_more_xp_once_the_worker_hits_the_work_level_cap() {
        let mut game = Game::new(301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.world.get_mut::<Experience>(worker).unwrap().level = systems::WORK_XP_LEVEL_CAP;
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: ItemId::CoreFragment,
                    amount: 5,
                    capacity: 5,
                    level: None,
                },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: 1,
        });

        for _ in 0..3 {
            game.tick();
        }

        let exp = game.world.get::<Experience>(worker).unwrap();
        assert_eq!(
            exp.level,
            systems::WORK_XP_LEVEL_CAP,
            "a capped worker shouldn't level further from cronjob work"
        );
        assert_eq!(exp.xp, 0, "a capped worker shouldn't earn any work XP at all");
    }

    #[test]
    fn cronjob_work_still_grants_xp_below_the_work_level_cap() {
        let mut game = Game::new(302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        assert!(
            game.world.get::<Experience>(worker).unwrap().level < systems::WORK_XP_LEVEL_CAP,
            "a freshly tamed program should start well under the work level cap"
        );
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: ItemId::CoreFragment,
                    amount: 5,
                    capacity: 5,
                    level: None,
                },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: 1,
        });

        game.tick();

        let xp = game.world.get::<Experience>(worker).unwrap().xp;
        assert!(xp > 0, "a worker under the cap should still earn work XP");
    }

    #[test]
    fn a_leveled_node_doesnt_always_yield_on_a_completed_cycle() {
        let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: ItemId::CoreFragment,
                    amount: 20,
                    capacity: 20,
                    level: Some(1),
                },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: 1,
        });

        let player = game.player_entity();
        let starting_fragments = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::CoreFragment);

        for _ in 0..40 {
            game.tick();
        }

        let gained = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::CoreFragment)
            - starting_fragments;
        assert!(
            gained < 40,
            "a level-1 node succeeding on every single one of 40 cycles is implausible at ~50% odds, got {gained}"
        );
    }

    #[test]
    fn player_decompiler_skill_grows_on_level_up_and_survives_save_load() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();

        assert_eq!(
            game.player_status().decompiler,
            0,
            "should start with no decompiler skill"
        );

        game.award_player_xp(player, 20);
        assert_eq!(
            game.player_status().level,
            2,
            "20 xp should be enough to reach level 2"
        );
        assert_eq!(
            game.player_status().decompiler,
            DECOMPILER_SKILL_PER_LEVEL,
            "one level gained should grant one point of decompiler skill"
        );

        let path = std::env::temp_dir().join(format!(
            "feral_processes_decompiler_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            loaded.player_status().decompiler,
            DECOMPILER_SKILL_PER_LEVEL,
            "decompiler skill should survive a save/load round trip"
        );
    }

    #[test]
    fn equip_grants_stat_bonus_and_removes_item_from_inventory() {
        let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::OverclockCore, 1);
        let atk_before = game.player_status().atk;

        game.equip(ItemId::OverclockCore).unwrap();

        let status = game.player_status();
        assert_eq!(
            status.atk,
            atk_before + 3,
            "weapon should grant its Attack bonus"
        );
        assert_eq!(
            status.weapon,
            Some(EquippedItem {
                item: ItemId::OverclockCore,
                level: 1,
                fusion_tier: 0
            })
        );
        assert!(
            status
                .inventory
                .iter()
                .all(|(i, _)| *i != ItemId::OverclockCore),
            "equipped item should leave the inventory stack"
        );
    }

    #[test]
    fn equipping_gear_in_a_deeper_zone_scales_its_bonus_100_percent_per_level() {
        let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.resource_mut::<ZoneLevel>().0 = 3;
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::OverclockCore, 1);
        let atk_before = game.player_status().atk;

        game.equip(ItemId::OverclockCore).unwrap();

        let status = game.player_status();
        // Base +3 ATK, scaled 2x per level above 1: level 3 = 3 * 2^2 = 12.
        assert_eq!(
            status.atk,
            atk_before + 12,
            "gear equipped at zone level 3 should be scaled 2x per level"
        );
        assert_eq!(
            status.weapon,
            Some(EquippedItem {
                item: ItemId::OverclockCore,
                level: 3,
                fusion_tier: 0
            })
        );

        game.unequip(EquipmentSlot::Weapon).unwrap();
        assert_eq!(
            game.player_status().atk,
            atk_before,
            "unequipping should remove exactly the level-scaled bonus that was granted"
        );
    }

    #[test]
    fn equipping_the_same_slot_again_swaps_without_double_counting_the_bonus() {
        let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::OverclockCore, 2);
        let atk_before = game.player_status().atk;

        game.equip(ItemId::OverclockCore).unwrap();
        assert_eq!(game.player_status().atk, atk_before + 3);

        // Equipping into an already-occupied slot swaps the old item back
        // to inventory and must not stack the bonus a second time.
        game.equip(ItemId::OverclockCore).unwrap();
        let status = game.player_status();
        assert_eq!(
            status.atk,
            atk_before + 3,
            "re-equipping must not double the bonus"
        );
        assert_eq!(
            status
                .inventory
                .iter()
                .find(|(i, _)| *i == ItemId::OverclockCore)
                .map(|(_, q)| *q),
            Some(1),
            "the swapped-out copy should return to inventory"
        );
    }

    #[test]
    fn unequip_removes_bonus_and_returns_item_to_inventory() {
        let mut game = Game::new(10, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::FirewallPlating, 1);
        let def_before = game.player_status().def;
        game.equip(ItemId::FirewallPlating).unwrap();
        assert_eq!(game.player_status().def, def_before + 3);

        game.unequip(EquipmentSlot::Armor).unwrap();

        let status = game.player_status();
        assert_eq!(status.def, def_before, "unequip should remove the bonus");
        assert_eq!(status.armor, None);
        assert_eq!(
            status
                .inventory
                .iter()
                .find(|(i, _)| *i == ItemId::FirewallPlating)
                .map(|(_, q)| *q),
            Some(1)
        );
    }

    #[test]
    fn unequip_errors_on_an_empty_slot() {
        let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(game.unequip(EquipmentSlot::Weapon).is_err());
    }

    #[test]
    fn fuse_item_consumes_two_copies_and_raises_the_fusion_tier() {
        let mut game = Game::new(200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::OverclockCore, 3);

        game.fuse_item(ItemId::OverclockCore).unwrap();

        assert_eq!(game.item_fusion_tier(ItemId::OverclockCore), 1);
        assert_eq!(
            game.player_status()
                .inventory
                .iter()
                .find(|(i, _)| *i == ItemId::OverclockCore)
                .map(|(_, q)| *q),
            Some(1),
            "fusing should consume 2 of the 3 copies"
        );
    }

    #[test]
    fn fuse_item_bonus_scales_the_equipped_stat_bonus() {
        let mut game = Game::new(201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Ablative Plating's base is +4 def, so a 10%/tier bonus is visible
        // (unlike a +3 item, where 10% rounds away to nothing at tier 1).
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::AblativePlating, 6);

        let def_before = game.player_status().def;
        game.equip(ItemId::AblativePlating).unwrap();
        assert_eq!(
            game.player_status().def,
            def_before + 4,
            "unfused equip should grant the plain base bonus"
        );
        game.unequip(EquipmentSlot::Armor).unwrap();

        game.fuse_item(ItemId::AblativePlating).unwrap();
        game.fuse_item(ItemId::AblativePlating).unwrap();
        assert_eq!(game.item_fusion_tier(ItemId::AblativePlating), 2);

        game.equip(ItemId::AblativePlating).unwrap();
        assert_eq!(
            game.player_status().def,
            def_before + 5,
            "tier 2 is +20%: 4 * 1.2 = 4.8, rounds to 5"
        );
    }

    #[test]
    fn fuse_item_rejects_non_equipment_and_insufficient_stock() {
        let mut game = Game::new(202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        assert!(
            game.fuse_item(ItemId::CoreFragment).is_err(),
            "plain resources aren't equipment and can't be fused"
        );

        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::OverclockCore, 1);
        assert!(
            game.fuse_item(ItemId::OverclockCore).is_err(),
            "fusing needs 2 copies, only 1 is available"
        );
        assert_eq!(
            game.player_status()
                .inventory
                .iter()
                .find(|(i, _)| *i == ItemId::OverclockCore)
                .map(|(_, q)| *q),
            Some(1),
            "a failed fuse should not consume the lone copy"
        );
    }

    #[test]
    fn item_fusion_tier_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(203, DifficultyMode::Forgiving, &assets).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::OverclockCore, 2);
        game.fuse_item(ItemId::OverclockCore).unwrap();

        let path = std::env::temp_dir().join(format!(
            "feral_processes_fusion_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.item_fusion_tier(ItemId::OverclockCore), 1);
    }

    #[test]
    fn erase_item_removes_the_full_stack() {
        let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::NeuralAmplifier, 3);

        game.erase_item(ItemId::NeuralAmplifier, 3).unwrap();
        assert!(
            game.player_status()
                .inventory
                .iter()
                .all(|(i, _)| *i != ItemId::NeuralAmplifier)
        );

        assert!(
            game.erase_item(ItemId::NeuralAmplifier, 1).is_err(),
            "erasing from an empty stack should error"
        );
    }

    #[test]
    fn equipped_gear_and_its_bonus_survive_save_and_load() {
        let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::NeuralAmplifier, 1);
        game.equip(ItemId::NeuralAmplifier).unwrap();
        let decompiler_after_equip = game.player_status().decompiler;

        let path = std::env::temp_dir().join(format!(
            "feral_processes_equipment_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        let status = loaded.player_status();
        assert_eq!(
            status.module,
            Some(EquippedItem {
                item: ItemId::NeuralAmplifier,
                level: 1,
                fusion_tier: 0
            })
        );
        assert_eq!(status.decompiler, decompiler_after_equip);
    }

    /// The initial world spawns 14 wild creatures scattered around the
    /// player, so directional-inspect tests clear whatever landed along
    /// their search ray first — otherwise they'd be at the mercy of the
    /// seed's RNG instead of testing the method itself.
    fn clear_creatures_east_of_player(game: &mut Game, start: Position, range: i32) {
        // Matches the same 90° eastward cone `find_creature_in_direction`
        // itself uses, not just the exact row — otherwise a wild creature
        // that merely leans east (without being exactly on the player's
        // row) would survive the cleanup and make the test flaky.
        let stale: Vec<Entity> = {
            let mut query = game.world.query::<(Entity, &Position, &Creature)>();
            query
                .iter(&game.world)
                .filter(|(_, pos, _)| {
                    let (ddx, ddy) = (pos.x - start.x, pos.y - start.y);
                    ddx > 0 && ddx >= ddy.abs() && ddx <= range
                })
                .map(|(e, ..)| e)
                .collect()
        };
        for e in stale {
            game.world.despawn(e);
        }
    }

    #[test]
    fn find_creature_in_direction_finds_the_nearest_match_along_the_line() {
        let mut game = Game::new(14, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let start = *game.world.get::<Position>(player).unwrap();
        let species = game.species_defs().into_iter().next().unwrap();
        clear_creatures_east_of_player(&mut game, start, 10);

        assert!(game.find_creature_in_direction(1, 0, 10).is_none());

        let far = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position {
                    x: start.x + 5,
                    y: start.y,
                },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        let near = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position {
                    x: start.x + 2,
                    y: start.y,
                },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        let found = game.find_creature_in_direction(1, 0, 10);
        assert_eq!(
            found,
            Some(near),
            "the nearer creature along the ray should win"
        );
        assert_ne!(found, Some(far));
    }

    #[test]
    fn find_creature_in_direction_respects_max_range() {
        let mut game = Game::new(15, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let start = *game.world.get::<Position>(player).unwrap();
        let species = game.species_defs().into_iter().next().unwrap();
        clear_creatures_east_of_player(&mut game, start, 10);
        game.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: start.x + 10,
                y: start.y,
            },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 1,
            },
        ));

        assert!(
            game.find_creature_in_direction(1, 0, 5).is_none(),
            "creature is out of range"
        );
        assert!(
            game.find_creature_in_direction(1, 0, 10).is_some(),
            "creature should be within range"
        );
    }

    #[test]
    fn find_creature_in_direction_matches_a_90_degree_cone_not_just_the_exact_row() {
        let mut game = Game::new(17, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let start = *game.world.get::<Position>(player).unwrap();
        let species = game.species_defs().into_iter().next().unwrap();
        clear_creatures_east_of_player(&mut game, start, 10);

        // Leans east more than north/south (ddx=4 >= |ddy|=3) — inside the cone.
        let diagonal_ish = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position {
                    x: start.x + 4,
                    y: start.y - 3,
                },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        assert_eq!(
            game.find_creature_in_direction(1, 0, 10),
            Some(diagonal_ish)
        );

        // Leans north more than east (ddy=-8, ddx=2) — outside the eastward cone.
        game.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: start.x + 2,
                y: start.y - 8,
            },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 1,
            },
        ));
        assert_eq!(
            game.find_creature_in_direction(1, 0, 10),
            Some(diagonal_ish),
            "a creature that leans mostly north shouldn't win the eastward search"
        );
    }

    #[test]
    fn player_status_power_matches_max_hp_plus_atk_plus_def() {
        let game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let status = game.player_status();
        assert_eq!(status.power, status.max_hp + status.atk + status.def);
    }

    #[test]
    fn wait_advances_one_tick_without_moving() {
        let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let pos_before = *game.world.get::<Position>(player).unwrap();
        let tick_before = game.world.resource::<GameClock>().tick;

        game.wait();

        let pos_after = *game.world.get::<Position>(player).unwrap();
        let tick_after = game.world.resource::<GameClock>().tick;
        assert_eq!(pos_after, pos_before, "waiting shouldn't move the player");
        assert_eq!(
            tick_after,
            tick_before + 1,
            "waiting should advance exactly one tick"
        );
    }

    #[test]
    fn current_tick_matches_the_internal_game_clock() {
        let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert_eq!(
            game.current_tick(),
            0,
            "a fresh game should start at tick 0"
        );

        game.wait();
        game.wait();

        assert_eq!(
            game.current_tick(),
            2,
            "current_tick should track GameClock exactly"
        );
    }

    #[test]
    fn idle_tick_advances_the_clock_outside_battle_but_not_during_one() {
        let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

        game.idle_tick();
        assert_eq!(game.current_tick(), 1, "idle_tick should advance the clock with no battle active");

        let player = game.player_entity();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![player],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        game.idle_tick();
        assert_eq!(
            game.current_tick(),
            1,
            "idle_tick should be a no-op while a battle is active"
        );
    }

    #[test]
    fn rest_fully_heals_and_restores_fatigue() {
        let mut game = Game::new(18, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut stats = game.world.get_mut::<Stats>(player).unwrap();
            stats.hp = 1;
        }
        {
            let mut needs = game.world.get_mut::<Needs>(player).unwrap();
            needs.fatigue = 10.0;
        }

        game.rest();

        let stats = *game.world.get::<Stats>(player).unwrap();
        let needs = *game.world.get::<Needs>(player).unwrap();
        assert_eq!(stats.hp, stats.max_hp, "rest should fully heal Integrity");
        assert_eq!(needs.fatigue, 100.0, "rest should fully restore Fatigue");
    }

    #[test]
    fn rest_also_fully_heals_the_active_companion() {
        let mut game = Game::new(29, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();
        {
            let mut stats = game.world.get_mut::<Stats>(companion).unwrap();
            stats.hp = 1;
        }

        game.rest();

        let stats = *game.world.get::<Stats>(companion).unwrap();
        assert_eq!(
            stats.hp, stats.max_hp,
            "rest should fully heal the active companion too"
        );
    }

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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        // Near-dead target + maxed decompiler skill + plenty of breakers,
        // so the capture-chance clamp (95%) makes a handful of attempts
        // succeed for certain, without needing to control the RNG directly.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::IceBreaker, 50);
        game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

        for _ in 0..50 {
            if game.world.get::<Tamed>(wild).is_some() {
                break;
            }
            game.battle_decompile();
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
    fn raid_check_never_targets_a_nest_even_as_the_only_durability_holder() {
        let mut game = Game::new(600, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Strip every other Durability holder so a Nest would be the only
        // possible target if it weren't explicitly excluded.
        let existing: Vec<Entity> = {
            let mut query = game.world.query_filtered::<Entity, With<Durability>>();
            query.iter(&game.world).collect()
        };
        for e in existing {
            game.world.despawn(e);
        }
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 10, y: 10 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();

        for _ in 0..500 {
            game.raid_check();
        }

        assert_eq!(
            game.world.get::<Durability>(nest).unwrap().hp,
            NEST_DURABILITY,
            "a Nest must never take raid damage, even when it's the only Durability holder"
        );
    }

    #[test]
    fn entering_a_zone_portal_despawns_nests_left_behind_in_the_old_zone() {
        let mut game = Game::new(602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 10, y: 10 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();

        let player = game.player_entity();
        let ppos = *game.world.get::<Position>(player).unwrap();
        game.world.spawn((
            Structure {
                kind: "portal".to_string(),
            },
            Position {
                x: ppos.x + 1,
                y: ppos.y,
            },
        ));

        game.move_player(1, 0);

        // Note: `enter_next_zone` spawns fresh initial creatures for the new
        // zone, which can legitimately include brand-new nests — so this
        // must check the specific entity spawned above, not just count all
        // `Nest` entities in the world.
        assert!(
            game.world.get_entity(nest).is_err(),
            "a Nest left behind in the old zone must be despawned on zone transition, not just its guardians"
        );
    }

    #[test]
    fn temp_probe_find_seed_with_pre_existing_nest() {
        for seed in 0u32..5000 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let mut query = game.world.query_filtered::<Entity, With<Nest>>();
            let count = query.iter(&game.world).count();
            if count > 0 {
                panic!("seed {seed} produces {count} pre-existing nest(s) from Game::new alone");
            }
        }
    }

    #[test]
    fn spawn_nest_creates_a_tethered_guardian_cluster() {
        let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

        // `Game::new` runs initial habitat-spawn rolls that can themselves
        // occasionally create a Nest (now that species like scrapper have
        // can_nest: true), before this test's own explicit spawn_nest call
        // ever runs. So capture the pre-existing nests and only assert
        // about the newly-created one, not a world-wide count.
        let pre_existing_nests: std::collections::HashSet<Entity> = {
            let mut query = game.world.query_filtered::<Entity, With<Nest>>();
            query.iter(&game.world).collect()
        };
        game.spawn_nest("scrapper", 30, 30);

        let nests: Vec<(Entity, Position)> = {
            let mut query = game.world.query::<(Entity, &Nest, &Position)>();
            query
                .iter(&game.world)
                .filter(|(e, _, _)| !pre_existing_nests.contains(e))
                .map(|(e, _, p)| (e, *p))
                .collect()
        };
        assert_eq!(nests.len(), 1, "spawn_nest should create exactly one new Nest entity");
        let (nest, nest_pos) = nests[0];
        assert_eq!(nest_pos, Position { x: 30, y: 30 });
        assert_eq!(
            game.world.get::<Durability>(nest).unwrap().hp,
            NEST_DURABILITY
        );

        let guardians: Vec<Position> = {
            let mut query = game.world.query::<(&NestGuardian, &Position)>();
            query
                .iter(&game.world)
                .filter(|(g, _)| g.nest == nest)
                .map(|(_, p)| *p)
                .collect()
        };
        assert!(
            guardians.len() >= NEST_GUARDIAN_MIN as usize
                && guardians.len() <= NEST_GUARDIAN_MAX as usize,
            "expected {}..={} guardians, got {}",
            NEST_GUARDIAN_MIN,
            NEST_GUARDIAN_MAX,
            guardians.len()
        );
        for pos in guardians {
            let dist = (pos.x - 30).abs().max((pos.y - 30).abs());
            assert!(
                dist <= NEST_TETHER_RADIUS,
                "guardian spawned {dist} tiles from its nest, past the {NEST_TETHER_RADIUS}-tile tether"
            );
        }
    }

    #[test]
    fn guardian_never_wanders_beyond_the_nest_tether_radius() {
        let mut game = Game::new(602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.spawn_nest("scrapper", 40, 40);

        let (nest, nest_pos) = {
            let mut query = game.world.query::<(Entity, &Nest, &Position)>();
            let (e, _, p) = query.iter(&game.world).next().expect("nest should exist");
            (e, *p)
        };
        let guardians: Vec<Entity> = {
            let mut query = game.world.query::<(Entity, &NestGuardian)>();
            query
                .iter(&game.world)
                .filter(|(_, g)| g.nest == nest)
                .map(|(e, _)| e)
                .collect()
        };
        assert!(!guardians.is_empty());

        for _ in 0..200 {
            game.tick();
            for &guardian in &guardians {
                let pos = *game.world.get::<Position>(guardian).unwrap();
                let dist = (pos.x - nest_pos.x).abs().max((pos.y - nest_pos.y).abs());
                assert!(
                    dist <= NEST_TETHER_RADIUS,
                    "guardian wandered {dist} tiles from its nest, past the {NEST_TETHER_RADIUS}-tile tether"
                );
            }
        }
    }

    #[test]
    fn craft_consumes_cost_and_grants_the_result() {
        let mut game = Game::new(20, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
            inv.add(ItemId::CoreFragment, ICE_BREAKER_CORE_COST);
        }

        game.craft(ItemId::IceBreaker, 1).unwrap();

        let inv = game.world.get::<Inventory>(player).unwrap();
        assert_eq!(
            inv.count(ItemId::CoreFragment),
            0,
            "cost should be fully consumed"
        );
        assert_eq!(
            inv.count(ItemId::IceBreaker),
            1,
            "the recipe's result should be granted"
        );
    }

    #[test]
    fn craft_multiple_scales_cost_and_result() {
        let mut game = Game::new(30, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
            inv.add(ItemId::CoreFragment, ICE_BREAKER_CORE_COST * 3);
        }

        game.craft(ItemId::IceBreaker, 3).unwrap();

        let inv = game.world.get::<Inventory>(player).unwrap();
        assert_eq!(
            inv.count(ItemId::CoreFragment),
            0,
            "cost should scale with quantity"
        );
        assert_eq!(
            inv.count(ItemId::IceBreaker),
            3,
            "quantity units should be granted"
        );
    }

    #[test]
    fn max_craftable_floors_to_the_cheapest_affordable_whole_unit() {
        let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
            // ICE_BREAKER_CORE_COST per unit; 7 fragments afford 2 whole
            // units with 1 left over, not 3.
            inv.add(ItemId::CoreFragment, ICE_BREAKER_CORE_COST * 2 + 1);
        }

        assert_eq!(game.max_craftable(ItemId::IceBreaker), 2);
    }

    #[test]
    fn max_craftable_is_zero_with_no_recipe_or_no_resources() {
        let mut game = Game::new(32, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .items
            .clear();

        assert_eq!(
            game.max_craftable(ItemId::IceBreaker),
            0,
            "no resources at all"
        );
        assert_eq!(
            game.max_craftable(ItemId::CoreFragment),
            0,
            "no recipe exists for this item"
        );
    }

    #[test]
    fn craft_fails_without_enough_of_the_cost() {
        let mut game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
        }

        assert!(game.craft(ItemId::IceBreaker, 1).is_err());
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(ItemId::IceBreaker),
            0
        );
    }

    #[test]
    fn craft_rejects_a_result_with_no_recipe() {
        let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(game.craft(ItemId::CoreFragment, 1).is_err());
    }

    #[test]
    fn structure_defs_order_pins_home_mining_node_compiler_first_and_is_stable_across_sessions() {
        // StructureDb is backed by a HashMap, whose iteration order is
        // randomized per-instance — without an explicit sort, the build
        // menu's [1], [2], ... numbering would shuffle between sessions
        // even though the mod files never changed. Multiple seeds (each a
        // fresh StructureDb/HashMap instance) should all agree.
        let seeds = [40, 41, 42, 43];
        let mut orders = Vec::new();
        for seed in seeds {
            let game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let ids: Vec<String> = game.structure_defs().into_iter().map(|d| d.id).collect();
            assert_eq!(
                &ids[..3],
                ["home", "mining_node", "compiler"],
                "the three starter structures should always lead the build menu"
            );
            let mut rest_sorted = ids[3..].to_vec();
            rest_sorted.sort();
            assert_eq!(
                ids[3..],
                rest_sorted[..],
                "everything after the pinned three should still be alphabetical"
            );
            orders.push(ids);
        }
        assert!(
            orders.windows(2).all(|w| w[0] == w[1]),
            "structure order should be identical across fresh sessions, got {orders:?}"
        );
    }

    #[test]
    fn species_defs_order_is_sorted_by_id_and_stable_across_sessions() {
        let seeds = [44, 45, 46, 47];
        let mut orders = Vec::new();
        for seed in seeds {
            let game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let ids: Vec<String> = game.species_defs().into_iter().map(|d| d.id).collect();
            let mut sorted = ids.clone();
            sorted.sort();
            assert_eq!(ids, sorted, "species_defs() should already be sorted by id");
            orders.push(ids);
        }
        assert!(
            orders.windows(2).all(|w| w[0] == w[1]),
            "species order should be identical across fresh sessions, got {orders:?}"
        );
    }

    #[test]
    fn battle_flee_applies_the_same_mild_xp_setback_as_a_death() {
        let mut game = Game::new(33, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Experience>(player).unwrap().xp = 10;
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
                Position { x: 3, y: 3 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 0,
                    def: 1,
                },
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_flee();

        assert_eq!(
            game.world.get::<Experience>(player).unwrap().xp,
            8,
            "fleeing should dock the same 20% setback as a death"
        );
        assert!(!game.has_active_battle(), "fleeing should end the battle");
    }

    fn spawn_tamed(game: &mut Game, hp: i32, atk: i32) -> Entity {
        let player = game.player_entity();
        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        game.world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position { x: 3, y: 3 },
                Stats {
                    hp,
                    max_hp: hp,
                    atk,
                    def: 1,
                },
                Tamed { owner: player },
                Experience::default(),
            ))
            .id()
    }

    #[test]
    fn set_companion_rejects_a_wild_creature() {
        let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
                Position { x: 3, y: 3 },
                Stats {
                    hp: 5,
                    max_hp: 5,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        assert!(game.add_companion(wild).is_err());
        assert!(game.player_status().companions.is_empty());
    }

    #[test]
    fn set_companion_clears_any_active_cronjob_task() {
        let mut game = Game::new(24, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 2,
            required: 5,
        });

        game.add_companion(worker).unwrap();

        assert!(
            game.world.get::<Task>(worker).is_none(),
            "companion duty should cancel the cronjob"
        );
        assert_eq!(
            game.player_status().companions.first().map(|c| c.hp),
            Some(10)
        );
    }

    #[test]
    fn assigning_cronjob_to_the_active_companion_clears_companion_status() {
        let assets = test_assets_dir();
        let mut game = Game::new(25, DifficultyMode::Forgiving, &assets).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.add_companion(worker).unwrap();
        assert!(!game.player_status().companions.is_empty());

        let structure_def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.work.is_some())
            .expect("at least one workable structure should exist");
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: structure_def.id.clone(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: structure_def.work.as_ref().unwrap().produces,
                    amount: 20,
                    capacity: 20,
                    level: None,
                },
            ))
            .id();

        game.assign_cronjob(worker, structure).unwrap();

        assert!(
            game.player_status().companions.is_empty(),
            "running a cronjob should stand the companion down"
        );
        assert!(game.world.get::<Task>(worker).is_some());
    }

    #[test]
    fn clear_companion_reverts_to_no_companion() {
        let mut game = Game::new(26, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.add_companion(worker).unwrap();
        assert!(!game.player_status().companions.is_empty());

        game.remove_companion(worker);

        assert!(game.player_status().companions.is_empty());
    }

    #[test]
    fn owned_pets_reports_every_owned_creature_regardless_of_location_or_job() {
        let mut game = Game::new(34, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();

        let far_worker = spawn_tamed(&mut game, 12, 4);
        game.world
            .entity_mut(far_worker)
            .insert(Position { x: 500, y: 500 });
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 500, y: 501 },
            ))
            .id();
        game.world.entity_mut(far_worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 1,
            required: 5,
        });

        let idle = spawn_tamed(&mut game, 5, 2);
        game.world
            .entity_mut(idle)
            .insert(Position { x: 999, y: 999 });

        let pets = game.owned_pets();
        assert_eq!(
            pets.len(),
            3,
            "every owned tamed creature should be reported, wherever it is"
        );

        let companion_info = pets.iter().find(|p| p.entity == companion).unwrap();
        assert!(companion_info.is_companion);
        assert_eq!(companion_info.job_structure, None);

        let worker_info = pets.iter().find(|p| p.entity == far_worker).unwrap();
        assert!(!worker_info.is_companion);
        assert!(
            worker_info.job_structure.is_some(),
            "a far-off cronjob worker should still be reported"
        );
        assert_eq!(worker_info.hp, 12);
        assert_eq!(worker_info.atk, 4);

        let idle_info = pets.iter().find(|p| p.entity == idle).unwrap();
        assert!(!idle_info.is_companion);
        assert_eq!(idle_info.job_structure, None);
    }

    #[test]
    fn battle_command_companion_rallies_the_player_instead_of_attacking() {
        let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 10, 20);
        game.add_companion(companion).unwrap();

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
                Position { x: 5, y: 5 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 1,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_command_companion(companion);

        let wild_hp = game.world.get::<Stats>(wild).unwrap().hp;
        assert_eq!(
            wild_hp, 100,
            "commanding a companion should never damage the wild creature directly"
        );
        let buff = game.world.get::<PlayerBuff>(player).unwrap().active;
        assert!(
            buff.is_some_and(|b| b.kind == BuffKind::Atk),
            "commanding a companion with no special ability should rally (ATK buff) the player"
        );
    }

    /// Sets up a single-round battle with one companion (stunned or not)
    /// and returns how much the player's fatigue dropped from commanding
    /// it. Shared by the two fatigue-cost tests below.
    fn fatigue_spent_commanding_companion(seed: u32, stunned: bool) -> f32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 10, 20);
        game.add_companion(companion).unwrap();
        if stunned {
            game.world.entity_mut(companion).insert(StatusEffects {
                active: Some(ActiveStatus {
                    kind: StatusKind::Stun,
                    remaining: 1,
                    power: 0,
                }),
            });
        }

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
                Position { x: 5, y: 5 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 0,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        let fatigue_before = game.world.get::<Needs>(player).unwrap().fatigue;
        game.battle_command_companion(companion);
        let fatigue_after = game.world.get::<Needs>(player).unwrap().fatigue;
        fatigue_before - fatigue_after
    }

    #[test]
    fn commanding_a_companion_in_battle_costs_more_fatigue_than_a_stunned_one() {
        // Both paths advance the clock by one tick (`battle_command_companion`
        // always ticks at the end), so both pay the same small natural
        // fatigue decay regardless — comparing the two deltas rather than
        // asserting an absolute number isolates just the companion-command
        // cost from that shared per-tick decay.
        let active = fatigue_spent_commanding_companion(84, false);
        let stunned = fatigue_spent_commanding_companion(85, true);
        assert!(
            (active - stunned - COMPANION_COMMAND_FATIGUE_COST).abs() < 0.001,
            "commanding an active companion should cost exactly {COMPANION_COMMAND_FATIGUE_COST} \
             more fatigue than commanding a stunned one, which doesn't actually act: \
             active spent {active}, stunned spent {stunned}"
        );
    }

    #[test]
    fn an_atk_buff_increases_damage_dealt_and_expires_after_its_duration() {
        let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<PlayerBuff>(player).unwrap().active = Some(ActiveBuff {
            kind: BuffKind::Atk,
            remaining: 1,
            power: 50,
        });

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
                Position { x: 5, y: 5 },
                Stats {
                    hp: 10_000,
                    max_hp: 10_000,
                    atk: 0,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_attack();

        let wild_hp = game.world.get::<Stats>(wild).unwrap().hp;
        assert!(
            wild_hp < 10_000 - 50,
            "a +50 ATK buff should meaningfully increase damage dealt"
        );
        assert!(
            game.world
                .get::<PlayerBuff>(player)
                .unwrap()
                .active
                .is_none(),
            "a 1-round buff should expire once the round it covered ticks down"
        );
    }

    #[test]
    fn special_ability_heal_restores_player_hp_and_debuff_afflicts_the_wild_creature() {
        let mut game = Game::new(19, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Stats>(player).unwrap().hp = 5;

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
                Position { x: 5, y: 5 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 1,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();

        game.use_special_ability(&SpecialAbility::Heal { power: 8 }, "TestBot", player, wild);
        let hp = game.world.get::<Stats>(player).unwrap().hp;
        assert_eq!(
            hp, 13,
            "Heal should restore the player's HP by its power, capped at max_hp"
        );

        game.use_special_ability(
            &SpecialAbility::Debuff {
                kind: StatusKind::Bleed,
                power: 4,
                duration: 2,
            },
            "TestBot",
            player,
            wild,
        );
        let active = game.world.get::<StatusEffects>(wild).unwrap().active;
        assert!(
            active.is_some_and(|a| a.kind == StatusKind::Bleed && a.power == 4 && a.remaining == 2),
            "Debuff should inflict the given status condition on the wild creature"
        );
    }

    #[test]
    fn companion_ability_label_shows_special_ability_or_a_computed_attack_rally() {
        let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let all_species = game.species_defs();
        let no_ability_species = all_species
            .iter()
            .find(|s| s.special_ability.is_none())
            .expect("at least one species with no special ability")
            .id
            .clone();

        let plain = game
            .world
            .spawn((
                Creature { species: no_ability_species },
                Position { x: 3, y: 3 },
                Stats { hp: 10, max_hp: 10, atk: 30, def: 1 },
                Tamed { owner: player },
                Experience::default(),
            ))
            .id();
        game.add_companion(plain).unwrap();
        let plain_ability = game.player_status().companions[0].ability.clone();
        assert_eq!(
            plain_ability,
            format!("Attack Rally: +{} ATK for {RALLY_DURATION} rounds", (30_i32 / 3).max(1)),
            "a species with no special_ability should show the computed default rally"
        );

        if let Some((species_id, expected)) = all_species
            .iter()
            .find_map(|s| s.special_ability.clone().map(|a| (s.id.clone(), a)))
        {
            let with_ability = game
                .world
                .spawn((
                    Creature { species: species_id },
                    Position { x: 3, y: 3 },
                    Stats { hp: 10, max_hp: 10, atk: 5, def: 1 },
                    Tamed { owner: player },
                    Experience::default(),
                ))
                .id();
            game.add_companion(with_ability).unwrap();
            let shown = game
                .player_status()
                .companions
                .iter()
                .find(|c| c.entity == with_ability)
                .unwrap()
                .ability
                .clone();
            assert_eq!(
                shown,
                expected.display_label(),
                "a species with a special_ability should show its own label, not the generic rally"
            );
        }
    }

    #[test]
    fn award_player_xp_also_grants_party_members_half_as_much() {
        let mut game = Game::new(36, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let in_party = spawn_tamed(&mut game, 10, 3);
        game.add_companion(in_party).unwrap();
        let not_in_party = spawn_tamed(&mut game, 10, 3);

        game.award_player_xp(player, 10);

        assert_eq!(
            game.world.get::<Experience>(in_party).unwrap().xp,
            5,
            "a party member should gain half the player's XP"
        );
        assert_eq!(
            game.world.get::<Experience>(not_in_party).unwrap().xp,
            0,
            "a tamed program outside the party shouldn't gain any XP from a kill"
        );
    }

    #[test]
    fn award_player_xp_can_level_up_a_party_member_independently_of_the_player() {
        let mut game = Game::new(37, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.world
            .get_mut::<Experience>(companion)
            .unwrap()
            .xp_to_next = 5;
        game.add_companion(companion).unwrap();

        game.award_player_xp(player, 10);

        let exp = game.world.get::<Experience>(companion).unwrap();
        assert_eq!(
            exp.level, 2,
            "5 XP against a 5-XP requirement should level the companion up"
        );
    }

    #[test]
    fn player_level_up_message_is_tagged_message_kind_level_up() {
        let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Experience>(player).unwrap().xp_to_next = 5;

        game.award_player_xp(player, 5);

        let tagged = game
            .message_log(10)
            .into_iter()
            .any(|(kind, text)| kind == MessageKind::LevelUp && text.contains("reach level"));
        assert!(
            tagged,
            "leveling up should log a MessageKind::LevelUp line, got: {:?}",
            game.message_log(10)
        );
    }

    #[test]
    fn killing_a_wild_creature_in_battle_awards_the_active_companion_half_xp() {
        let mut game = Game::new(38, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();

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
                Position { x: 5, y: 5 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_attack();

        assert_eq!(
            game.world.get::<Experience>(companion).unwrap().xp,
            5,
            "killing a 10-max-HP wild program should award the party member half its max HP as XP"
        );
    }

    /// `wild_retaliate` rolls per-call whether a companion soaks the hit, so
    /// this drives it across many seeds and checks both outcomes occur —
    /// proof the roll is live, not that any single call behaves one way.
    #[test]
    fn wild_retaliation_can_land_on_either_the_player_or_the_companion() {
        let species_id = {
            let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.species_defs()
                .into_iter()
                .next()
                .expect("at least one species")
                .id
                .clone()
        };

        let mut companion_hit = false;
        let mut player_hit = false;

        for seed in 0..60u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let player = game.player_entity();
            let companion = spawn_tamed(&mut game, 1000, 1);
            game.add_companion(companion).unwrap();
            let player_hp_before = game.world.get::<Stats>(player).unwrap().hp;

            let wild = game
                .world
                .spawn((
                    Creature {
                        species: species_id.clone(),
                    },
                    Hostile,
                    Position { x: 5, y: 5 },
                    Stats {
                        hp: 1000,
                        max_hp: 1000,
                        atk: 5,
                        def: 0,
                    },
                ))
                .id();
            game.world.insert_resource(BattleState {
                player,
                wild_creatures: vec![wild],
                log: Vec::new(),
                finished: false,
                player_won: false,
            });

            game.battle_attack();

            let companion_hp = game.world.get::<Stats>(companion).unwrap().hp;
            let player_hp_after = game.world.get::<Stats>(player).unwrap().hp;
            if companion_hp < 1000 {
                companion_hit = true;
            }
            if player_hp_after < player_hp_before {
                player_hit = true;
            }
            if companion_hit && player_hit {
                break;
            }
        }

        assert!(
            companion_hit,
            "across 60 battles, the companion should have taken at least one hit"
        );
        assert!(
            player_hit,
            "across 60 battles, the player should have taken at least one hit"
        );
    }

    #[test]
    fn effective_def_excludes_the_players_party_bonus_when_a_companion_is_the_target() {
        // `wild_retaliate` calls `effective_def` on whichever entity got
        // hit — the player, or (per the test above) a companion. The
        // player's passive party bonus (see `party_stat_bonus`) must only
        // ever land on the player, never get double-applied to a
        // companion's own defense just because it's a party member too.
        let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 30);
        game.world.get_mut::<Stats>(a).unwrap().def = 20;
        game.add_companion(a).unwrap();
        // A second party member gives the *player's* bonus a nonzero,
        // easy-to-notice value if it ever leaked onto `a`.
        let b = spawn_tamed(&mut game, 10, 200);
        game.add_companion(b).unwrap();

        let raw_def = game.world.get::<Stats>(a).unwrap().def;
        assert_eq!(
            game.effective_def(a),
            raw_def,
            "a companion's effective DEF as a retaliation target must be its own raw Stats, \
             not inflated by the player's party bonus"
        );
    }

    #[test]
    fn a_knocked_out_companion_stands_down() {
        let species_id = {
            let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.species_defs()
                .into_iter()
                .next()
                .expect("at least one species")
                .id
                .clone()
        };

        // The companion-targeting roll is 30% per call; a 1-HP companion is
        // guaranteed to hit 0 the moment it's targeted (damage is always
        // >= 1). Across 60 seeds the odds of never once rolling the
        // companion are astronomically small, so this deterministically
        // exercises the knockout path without needing to fake the RNG.
        for seed in 0..60u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let player = game.player_entity();
            let companion = spawn_tamed(&mut game, 1, 1);
            game.add_companion(companion).unwrap();

            let wild = game
                .world
                .spawn((
                    Creature {
                        species: species_id.clone(),
                    },
                    Hostile,
                    Position { x: 5, y: 5 },
                    Stats {
                        hp: 1000,
                        max_hp: 1000,
                        atk: 50,
                        def: 0,
                    },
                ))
                .id();
            game.world.insert_resource(BattleState {
                player,
                wild_creatures: vec![wild],
                log: Vec::new(),
                finished: false,
                player_won: false,
            });

            game.wild_retaliate(wild, player);
            if game.world.get::<Stats>(companion).unwrap().hp == 0 {
                assert!(
                    game.player_status().companions.is_empty(),
                    "0 HP should have stood the companion down"
                );
                return;
            }
        }
        panic!("companion was never targeted across 60 seeds — retaliation roll may be broken");
    }

    #[test]
    fn companion_status_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(28, DifficultyMode::Forgiving, &assets).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.add_companion(worker).unwrap();

        let path = std::env::temp_dir().join(format!(
            "feral_processes_companion_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let status = loaded.player_status();
        assert!(
            !status.companions.is_empty(),
            "the active companion should survive a save/load round trip"
        );
    }

    #[test]
    fn party_accepts_up_to_max_party_size_and_rejects_beyond_that() {
        let mut game = Game::new(70, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let members: Vec<Entity> = (0..MAX_PARTY_SIZE)
            .map(|_| spawn_tamed(&mut game, 10, 3))
            .collect();
        for &m in &members {
            game.add_companion(m).unwrap();
        }
        assert_eq!(game.player_status().companions.len(), MAX_PARTY_SIZE);

        let one_too_many = spawn_tamed(&mut game, 10, 3);
        assert!(
            game.add_companion(one_too_many).is_err(),
            "adding a 4th member to a full 3-slot party should fail"
        );
        assert_eq!(game.player_status().companions.len(), MAX_PARTY_SIZE);
    }

    #[test]
    fn adding_the_same_companion_twice_is_rejected() {
        let mut game = Game::new(71, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();
        assert!(
            game.add_companion(companion).is_err(),
            "a program already in the party can't be added again"
        );
        assert_eq!(game.player_status().companions.len(), 1);
    }

    #[test]
    fn removing_one_party_member_leaves_the_others_active() {
        let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.add_companion(a).unwrap();
        game.add_companion(b).unwrap();

        game.remove_companion(a);

        assert_eq!(game.player_status().companions.len(), 1);
        assert!(
            game.player_status()
                .companions
                .first()
                .is_some_and(|c| c.hp == 10)
        );
        assert!(!game.world.resource::<Party>().0.contains(&a));
        assert!(game.world.resource::<Party>().0.contains(&b));
    }

    #[test]
    fn party_members_grant_a_passive_ten_percent_atk_def_bonus_that_stacks_updates_live_and_disappears_on_removal() {
        let mut game = Game::new(75, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let base_atk = game.player_status().atk;
        let base_def = game.player_status().def;

        // `spawn_tamed` fixes def at 1, so 10% of it floors to 0 and should
        // clamp up to the stated minimum of 1 rather than contributing 0.
        let a = spawn_tamed(&mut game, 10, 30);
        game.add_companion(a).unwrap();
        let status = game.player_status();
        assert_eq!(status.atk, base_atk + 3, "10% of a's 30 ATK is 3");
        assert_eq!(status.def, base_def + 1, "10% of a's 1 DEF floors to 0, minimum 1 applies");

        // A second party member's bonus stacks on top of the first's.
        let b = spawn_tamed(&mut game, 10, 50);
        game.add_companion(b).unwrap();
        let status = game.player_status();
        assert_eq!(status.atk, base_atk + 3 + 5, "10% of b's 50 ATK is 5, stacked with a's");
        assert_eq!(status.def, base_def + 1 + 1);

        // The bonus is computed live from each companion's current Stats,
        // not baked in at add_companion time — a level-up (simulated here
        // by mutating Stats directly, same as `progression::add_xp` would)
        // should be reflected immediately with no extra bookkeeping.
        game.world.get_mut::<Stats>(a).unwrap().atk = 60;
        let status = game.player_status();
        assert_eq!(status.atk, base_atk + 6 + 5, "a's stronger ATK should raise its contribution");

        game.remove_companion(a);
        game.remove_companion(b);
        let status = game.player_status();
        assert_eq!(status.atk, base_atk, "bonus should vanish once every companion leaves the party");
        assert_eq!(status.def, base_def);
    }

    #[test]
    fn dropping_below_half_power_weakens_the_players_attack() {
        let mut game = Game::new(76, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let full_atk = game.player_status().atk;

        // At and above the threshold, no penalty at all.
        game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
        assert_eq!(game.player_status().atk, full_atk, "50 power is still full strength");

        // Below it, a linear falloff — checked at a couple of points rather
        // than re-deriving the formula, since `battle::power_attack_multiplier`
        // already has its own dedicated unit tests for the exact curve.
        game.world.get_mut::<Needs>(player).unwrap().hunger = 25.0;
        let quarter_power_atk = game.player_status().atk;
        assert!(
            quarter_power_atk < full_atk,
            "attack should be weaker at 25 power than at full power"
        );

        game.world.get_mut::<Needs>(player).unwrap().hunger = 0.0;
        let zero_power_atk = game.player_status().atk;
        assert!(
            zero_power_atk < quarter_power_atk,
            "attack should keep weakening as power keeps dropping"
        );
        assert_eq!(
            zero_power_atk,
            (full_atk as f32 * 0.5).round() as i32,
            "the penalty floors at half strength, even fully starved"
        );
    }

    #[test]
    fn battle_command_companion_rejects_a_program_not_in_the_party() {
        let mut game = Game::new(73, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let not_in_party = spawn_tamed(&mut game, 10, 20);

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
                Position { x: 5, y: 5 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 1,
                    def: 0,
                },
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_command_companion(not_in_party);

        let wild_hp = game.world.get::<Stats>(wild).unwrap().hp;
        assert_eq!(
            wild_hp, 100,
            "a program outside the active party shouldn't be able to act in battle"
        );
    }

    #[test]
    fn rest_heals_every_party_member() {
        let mut game = Game::new(74, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.add_companion(a).unwrap();
        game.add_companion(b).unwrap();
        for e in [a, b] {
            game.world.get_mut::<Stats>(e).unwrap().hp = 1;
        }

        game.rest();

        assert_eq!(game.world.get::<Stats>(a).unwrap().hp, 10);
        assert_eq!(game.world.get::<Stats>(b).unwrap().hp, 10);
    }

    #[test]
    fn fuse_companions_combines_stats_and_keeps_the_higher_level_species() {
        let mut game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game.species_defs();
        let species_a = species[0].id.clone();
        let species_b = species[1 % species.len()].id.clone();

        let a = game
            .world
            .spawn((
                Creature { species: species_a },
                Position { x: 3, y: 3 },
                Stats {
                    hp: 20,
                    max_hp: 20,
                    atk: 10,
                    def: 4,
                },
                Tamed { owner: player },
                Experience {
                    level: 5,
                    xp: 3,
                    xp_to_next: 100,
                },
            ))
            .id();
        let b = game
            .world
            .spawn((
                Creature {
                    species: species_b.clone(),
                },
                Position { x: 4, y: 4 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 6,
                    def: 2,
                },
                Tamed { owner: player },
                Experience {
                    level: 2,
                    xp: 1,
                    xp_to_next: 40,
                },
            ))
            .id();

        game.fuse_companions(a, b, None).unwrap();

        assert!(
            game.world.get::<Creature>(a).is_none(),
            "the first input should be consumed"
        );
        assert!(
            game.world.get::<Creature>(b).is_none(),
            "the second input should be consumed"
        );

        let mut query = game
            .world
            .query::<(&Creature, &Stats, &Experience, &Tamed)>();
        let (creature, stats, exp, _) = query
            .iter(&game.world)
            .find(|(_, _, _, t)| t.owner == player)
            .expect("a fused creature should exist");
        assert_eq!(
            exp.level, 5,
            "fusion should keep the higher level (ties favor `a`)"
        );
        assert_eq!(exp.xp, 0);
        assert_eq!(exp.xp_to_next, progression::xp_for_level(5));
        assert_eq!(
            stats.max_hp,
            20 + 10 / 2,
            "fused HP should be higher + lower/2"
        );
        assert_eq!(stats.atk, 10 + 6 / 2);
        assert_eq!(stats.def, 4 + 2 / 2);
        assert_ne!(
            creature.species, species_b,
            "the lower-level input's species shouldn't win the tie"
        );
    }

    #[test]
    fn fuse_companions_applies_a_custom_name_truncated_to_the_max_length() {
        let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.fuse_companions(a, b, Some("Way Too Long A Name".to_string())).unwrap();

        let fused = game.owned_pets();
        assert_eq!(fused.len(), 1, "fusing two owned programs should leave exactly one");
        // PetInfo::name is zone-tagged (every fused program gets
        // `ZonePortal(1)`, always shown per `entity_label`'s own test
        // coverage), so strip that " 1" suffix before checking the
        // truncated custom name itself.
        let base_name = fused[0]
            .name
            .strip_suffix(" 1")
            .expect("a freshly fused program should be zone-tagged");
        assert_eq!(
            base_name.chars().count(),
            MAX_CUSTOM_NAME_LEN,
            "an overlong custom name should be truncated, not rejected"
        );
        assert!(
            "Way Too Long A Name".starts_with(base_name),
            "the truncated name should be a prefix of what was typed, got {base_name:?}"
        );
    }

    #[test]
    fn fuse_companions_with_no_name_or_blank_name_keeps_the_species_name() {
        let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // `spawn_tamed` always uses this same species (`species_defs()` is
        // stably sorted), and fusing two same-level, same-species programs
        // keeps it — capturing it directly here avoids having to pick the
        // fused entity back out of a world that also has 14 unrelated wild
        // creatures in it from `Game::new`.
        let species_name = game.species_defs().into_iter().next().unwrap().name;
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.fuse_companions(a, b, None).unwrap();
        let no_name = game.owned_pets();
        assert_eq!(no_name.len(), 1);
        // Every fused program gets `ZonePortal(1)` (see `fuse_companions`),
        // which `creature_label`/`PetInfo::name` always zone-tags — even at
        // zone 1, per `entity_label`'s own test coverage — so the expected
        // fallback name carries that same " 1" suffix, not the bare species name.
        let expected_default_name = format!("{species_name} 1");
        assert_eq!(
            no_name[0].name, expected_default_name,
            "no name given should fall back to the (zone-tagged) species name"
        );

        let c = spawn_tamed(&mut game, 10, 3);
        let d = spawn_tamed(&mut game, 10, 3);
        game.fuse_companions(c, d, Some("   ".to_string())).unwrap();
        let pets = game.owned_pets();
        let blank_named = pets.iter().find(|p| p.entity != no_name[0].entity).unwrap();
        assert_eq!(
            blank_named.name, expected_default_name,
            "an all-whitespace name should also fall back to the species name, not become blank"
        );
    }

    #[test]
    fn a_fused_programs_custom_name_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(92, DifficultyMode::Forgiving, &assets).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.fuse_companions(a, b, Some("Zappy".to_string())).unwrap();

        let path = std::env::temp_dir().join(format!(
            "feral_processes_fuse_name_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let pets = loaded.owned_pets();
        assert_eq!(pets.len(), 1);
        // Zone-tagged the same as any other fused program — see the
        // truncation test above for why " 1" is expected here too.
        assert_eq!(
            pets[0].name, "Zappy 1",
            "a custom name should survive a save/load round trip"
        );
    }

    #[test]
    fn fuse_companions_rejects_fusing_a_program_with_itself() {
        let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        assert!(game.fuse_companions(a, a, None).is_err());
    }

    #[test]
    fn fuse_companions_rejects_a_wild_creature() {
        let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let species = game.species_defs().into_iter().next().unwrap();
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 5,
                    max_hp: 5,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        assert!(game.fuse_companions(a, wild, None).is_err());
        assert!(
            game.world.get::<Creature>(a).is_some(),
            "a failed fusion shouldn't consume either input"
        );
        assert!(game.world.get::<Creature>(wild).is_some());
    }

    #[test]
    fn fuse_companions_removes_fused_members_from_the_active_party() {
        let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.add_companion(a).unwrap();
        game.add_companion(b).unwrap();

        game.fuse_companions(a, b, None).unwrap();

        assert!(!game.world.resource::<Party>().0.contains(&a));
        assert!(!game.world.resource::<Party>().0.contains(&b));
    }

    #[test]
    fn sell_item_pays_out_core_fragments_at_the_structures_sell_rate() {
        let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.trade.is_some())
            .expect("a trading structure (Black Market) should exist");
        let market = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 5, y: 5 },
            ))
            .id();

        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::FirewallPlating, 3);
        let cf_before = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::CoreFragment);

        game.sell_item(market, ItemId::FirewallPlating, 2).unwrap();

        let inv = game.world.get::<Inventory>(player).unwrap();
        assert_eq!(
            inv.count(ItemId::FirewallPlating),
            1,
            "only the sold quantity should leave the inventory"
        );
        let sell_rate = def.trade.as_ref().unwrap().sell_rate;
        assert_eq!(inv.count(ItemId::CoreFragment), cf_before + sell_rate * 2);
    }

    #[test]
    fn sell_item_rejects_core_fragments_and_items_you_dont_have() {
        let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.trade.is_some())
            .unwrap();
        let market = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 5, y: 5 },
            ))
            .id();

        assert!(game.sell_item(market, ItemId::CoreFragment, 1).is_err());
        assert!(
            game.sell_item(market, ItemId::NeuralAmplifier, 1).is_err(),
            "can't sell what you don't have"
        );
    }

    #[test]
    fn buy_item_charges_core_fragments_and_grants_the_item() {
        let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.trade.is_some())
            .unwrap();
        let (buy_item, unit_cost) = def.trade.as_ref().unwrap().buy[0];
        let market = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 5, y: 5 },
            ))
            .id();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
            inv.add(ItemId::CoreFragment, unit_cost * 2);
        }

        game.buy_item(market, buy_item, 2).unwrap();

        let inv = game.world.get::<Inventory>(player).unwrap();
        assert_eq!(
            inv.count(ItemId::CoreFragment),
            0,
            "the full cost should be charged"
        );
        assert_eq!(inv.count(buy_item), 2);
    }

    #[test]
    fn buy_item_fails_without_enough_core_fragments_or_for_an_unlisted_item() {
        let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.trade.is_some())
            .unwrap();
        let (buy_item, _) = def.trade.as_ref().unwrap().buy[0];
        let market = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 5, y: 5 },
            ))
            .id();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .items
            .clear();

        assert!(
            game.buy_item(market, buy_item, 1).is_err(),
            "no Core Fragments should fail the purchase"
        );
        assert!(
            game.buy_item(market, ItemId::CoreFragment, 1).is_err(),
            "an item not on the buy list shouldn't be purchasable"
        );
    }

    #[test]
    fn damage_structure_destroys_it_and_clears_its_cronjob_at_zero_durability() {
        let mut game = Game::new(100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 10, max_hp: 30 },
            ))
            .id();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 1,
            required: 5,
        });

        game.damage_structure(structure, 10, "Mining Node");

        assert!(
            game.world.get::<Structure>(structure).is_none(),
            "0 durability should destroy the structure"
        );
        assert!(
            game.world.get::<Task>(worker).is_none(),
            "the destroyed structure's cronjob should be cleared"
        );
    }

    #[test]
    fn damage_structure_just_reduces_durability_when_it_survives() {
        let mut game = Game::new(101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 20, max_hp: 30 },
            ))
            .id();

        game.damage_structure(structure, 10, "Mining Node");

        assert_eq!(game.world.get::<Durability>(structure).unwrap().hp, 10);
        assert!(
            game.world.get::<Structure>(structure).is_some(),
            "a structure with remaining durability should survive"
        );
    }

    #[test]
    fn raid_check_can_damage_an_undefended_structure() {
        // RAID_CHANCE_PER_TICK is a per-call roll; drive many seeds until it
        // fires at least once, same pattern as the wild-retaliation test.
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();

            game.raid_check();

            let Some(durability) = game.world.get::<Durability>(structure) else {
                // Destroyed outright (30 durability, RAID_DAMAGE 10 — shouldn't
                // happen in one hit, but tolerate it rather than assume).
                return;
            };
            if durability.hp < 30 {
                return;
            }
        }
        panic!(
            "raid_check never damaged the structure across 300 seeds — the raid roll may be broken"
        );
    }

    #[test]
    fn raid_damage_message_is_tagged_message_kind_raid() {
        // Same seed-hunting pattern as raid_check_can_damage_an_undefended_structure
        // — RAID_CHANCE_PER_TICK is a per-call roll, so drive seeds until it fires.
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.world.spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ));

            game.raid_check();

            let tagged = game
                .message_log(10)
                .into_iter()
                .any(|(kind, _)| kind == MessageKind::Raid);
            if tagged {
                return;
            }
        }
        panic!(
            "raid_check never logged a MessageKind::Raid line across 300 seeds — the raid roll may be broken"
        );
    }

    #[test]
    fn turret_structure_loads_with_no_work_and_a_raid_defense_bonus() {
        let game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "turret")
            .expect("turret.ron should load as a structure");
        assert!(
            def.work.is_none(),
            "a turret defends passively, not via cronjob work"
        );
        assert!(
            def.raid_defense > 0,
            "a turret should contribute a nonzero raid_defense bonus"
        );
    }

    #[test]
    fn deployed_turrets_reduce_raid_damage_to_an_undefended_structure() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let turret_defense = game
                .structure_defs()
                .into_iter()
                .find(|d| d.id == "turret")
                .unwrap()
                .raid_defense;
            game.world.spawn((
                Structure {
                    kind: "turret".to_string(),
                },
                Position { x: 1, y: 1 },
            ));
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();

            game.raid_check();

            let Some(durability) = game.world.get::<Durability>(structure) else {
                return;
            };
            if durability.hp < 30 {
                assert_eq!(
                    durability.hp,
                    30 - (RAID_DAMAGE - turret_defense),
                    "a raid on an undefended structure should be reduced by the deployed turret's raid_defense"
                );
                return;
            }
        }
        panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
    }

    #[test]
    fn assign_guard_defends_a_structure_with_no_work_recipe() {
        let mut game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "home".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();
        let worker = spawn_tamed(&mut game, 50, 3);

        game.assign_guard(worker, structure).unwrap();

        let task = game
            .world
            .get::<Task>(worker)
            .expect("guarding should assign a Task");
        assert_eq!(task.kind, TaskKind::Guard);
        assert_eq!(task.target, structure);
    }

    #[test]
    fn a_guard_task_never_produces_resources_even_on_a_workable_node() {
        let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: ItemId::CoreFragment,
                    amount: 5,
                    capacity: 5,
                    level: None,
                },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::Guard,
            target: structure,
            progress: 0,
            required: 1,
        });

        for _ in 0..10 {
            game.tick();
        }

        assert_eq!(
            game.world.get::<ResourceNode>(structure).unwrap().amount,
            5,
            "a guard shouldn't advance the node's gather cycle at all"
        );
    }

    #[test]
    fn guard_assignment_on_a_non_resource_structure_survives_save_and_load() {
        let mut game = Game::new(6, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "home".to_string(),
                },
                Position { x: 3, y: 3 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.assign_guard(worker, structure).unwrap();

        let path = std::env::temp_dir().join(format!(
            "feral_processes_guard_test_{}_{}.bin",
            std::process::id(),
            6
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        let mut query = loaded.world.query::<&Task>();
        let task = query
            .iter(&loaded.world)
            .next()
            .expect("restored creature should still have its guard assignment");
        assert_eq!(task.kind, TaskKind::Guard);
        let target_pos = loaded
            .world
            .get::<Position>(task.target)
            .expect("guard task target should resolve to the structure entity");
        assert_eq!((target_pos.x, target_pos.y), (3, 3));
    }

    #[test]
    fn raid_check_defended_by_a_worker_reduces_structure_damage_and_hurts_the_worker() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();
            let worker = spawn_tamed(&mut game, 50, 3);
            game.world.get_mut::<Stats>(worker).unwrap().def = 100; // fully mitigates RAID_DAMAGE
            game.world.entity_mut(worker).insert(Task {
                kind: TaskKind::GatherResource,
                target: structure,
                progress: 0,
                required: 5,
            });

            game.raid_check();

            let worker_hp = game.world.get::<Stats>(worker).unwrap().hp;
            if worker_hp < 50 {
                // The raid rolled this attempt: the structure should be
                // untouched (fully mitigated) and the worker should have
                // taken the defender's cost.
                assert_eq!(
                    game.world.get::<Durability>(structure).unwrap().hp,
                    30,
                    "a worker with overwhelming Defense should fully mitigate the raid"
                );
                assert_eq!(worker_hp, 50 - RAID_DEFENDER_DAMAGE);
                return;
            }
        }
        panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
    }

    #[test]
    fn structure_regen_heals_damaged_structures_over_time() {
        let mut game = Game::new(102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 10, max_hp: 30 },
            ))
            .id();
        game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;

        game.structure_regen();

        assert_eq!(
            game.world.get::<Durability>(structure).unwrap().hp,
            10 + STRUCTURE_REGEN_AMOUNT
        );
    }

    #[test]
    fn structure_regen_does_not_exceed_max_durability() {
        let mut game = Game::new(103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 29, max_hp: 30 },
            ))
            .id();
        game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;

        game.structure_regen();

        assert_eq!(game.world.get::<Durability>(structure).unwrap().hp, 30);
    }

    #[test]
    fn structures_survive_save_and_load_with_their_durability() {
        let assets = test_assets_dir();
        let mut game = Game::new(104, DifficultyMode::Forgiving, &assets).unwrap();
        let structure_def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "mining_node")
            .unwrap();
        game.world.spawn((
            Structure {
                kind: structure_def.id.clone(),
            },
            Position { x: 5, y: 5 },
            Durability {
                hp: 12,
                max_hp: structure_def.durability,
            },
        ));

        let path = std::env::temp_dir().join(format!(
            "feral_processes_structure_durability_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let mut query = loaded.world.query::<&Durability>();
        let durability = query
            .iter(&loaded.world)
            .next()
            .expect("the structure should survive a save/load round trip");
        assert_eq!(durability.hp, 12);
        assert_eq!(durability.max_hp, structure_def.durability);
    }

    #[test]
    fn difficulty_color_buckets_relative_power_into_con_colors() {
        assert_eq!(
            difficulty_color(50, 100, false),
            GlyphColor::Green,
            "much weaker than the player"
        );
        assert_eq!(
            difficulty_color(100, 100, false),
            GlyphColor::Yellow,
            "an even match"
        );
        assert_eq!(
            difficulty_color(140, 100, false),
            GlyphColor::Orange,
            "notably tougher"
        );
        assert_eq!(
            difficulty_color(200, 100, false),
            GlyphColor::Red,
            "far stronger than the player"
        );
    }

    #[test]
    fn difficulty_color_is_always_magenta_for_a_boss_regardless_of_power() {
        assert_eq!(difficulty_color(1, 1000, true), GlyphColor::Magenta);
        assert_eq!(difficulty_color(1000, 1, true), GlyphColor::Magenta);
    }

    #[test]
    fn difficulty_color_never_divides_by_zero_player_power() {
        assert_eq!(difficulty_color(10, 0, false), GlyphColor::Red);
    }

    #[test]
    fn forage_chance_applies_keen_scavenger_per_level_but_never_boosts_a_zero_chance_biome() {
        assert_eq!(forage_chance(Biome::OpenGrid, 0), 0.6);
        assert_eq!(
            forage_chance(Biome::OpenGrid, 1),
            0.6 + KEEN_SCAVENGER_BONUS_PER_LEVEL
        );
        assert_eq!(
            forage_chance(Biome::OpenGrid, 3),
            0.6 + KEEN_SCAVENGER_BONUS_PER_LEVEL * 3.0
        );
        assert_eq!(
            forage_chance(Biome::DataVoid, 1),
            0.0,
            "an unwalkable biome's 0% chance shouldn't be boosted into a nonzero one"
        );
    }

    #[test]
    fn unlock_perk_spends_points_and_can_be_bought_repeatedly() {
        let mut game = Game::new(110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 5;

        game.unlock_perk(Perk::KeenScavenger).unwrap();

        let status = game.player_status();
        assert_eq!(status.perk_points, 5 - Perk::KeenScavenger.cost());
        assert_eq!(status.unlocked_perks, vec![Perk::KeenScavenger]);
        assert_eq!(game.player_perk_level(Perk::KeenScavenger), 1);

        game.unlock_perk(Perk::KeenScavenger).unwrap();
        assert_eq!(
            game.player_perk_level(Perk::KeenScavenger),
            2,
            "buying the same perk again should stack another level, not be rejected"
        );
        assert_eq!(
            status.perk_points - Perk::KeenScavenger.cost(),
            game.player_status().perk_points
        );
    }

    #[test]
    fn unlock_perk_rejects_without_enough_points() {
        let mut game = Game::new(111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 0;

        assert!(game.unlock_perk(Perk::ExploitFocus).is_err());
        assert_eq!(game.player_perk_level(Perk::ExploitFocus), 0);
    }

    #[test]
    fn exploit_focus_boosts_effective_decompiler_skill_per_level() {
        let mut game = Game::new(112, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
                Position { x: 3, y: 3 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        let before = game.inspect(wild).unwrap().decompile_chance;

        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::ExploitFocus).unwrap();
        let after_one = game.inspect(wild).unwrap().decompile_chance;
        game.unlock_perk(Perk::ExploitFocus).unwrap();
        let after_two = game.inspect(wild).unwrap().decompile_chance;

        assert!(
            after_one > before,
            "Exploit Focus should raise the decompile chance shown for the same target"
        );
        assert!(
            after_two > after_one,
            "a second level of Exploit Focus should raise it further still"
        );
    }

    #[test]
    fn lean_compiler_discounts_craft_cost_per_level_but_never_below_one_each() {
        let mut game = Game::new(113, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let base_cost = game.craft_cost(ItemId::PowerCell);
        assert_eq!(
            base_cost,
            vec![(ItemId::CoreFragment, POWER_CELL_CORE_COST)]
        );

        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::LeanCompiler).unwrap();
        let discounted = game.craft_cost(ItemId::PowerCell);
        assert_eq!(
            discounted,
            vec![(
                ItemId::CoreFragment,
                POWER_CELL_CORE_COST - LEAN_COMPILER_DISCOUNT_PER_LEVEL
            )]
        );

        for _ in 0..10 {
            game.world.get_mut::<Perks>(player).unwrap().points = 10;
            let _ = game.unlock_perk(Perk::LeanCompiler);
        }
        let floored = game.craft_cost(ItemId::PowerCell);
        assert_eq!(
            floored,
            vec![(ItemId::CoreFragment, 1)],
            "the discount should never drop the cost below 1"
        );
    }

    #[test]
    fn perk_state_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(114, DifficultyMode::Forgiving, &assets).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::LowPowerMode).unwrap();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::LowPowerMode).unwrap();
        let points_after_unlock = game.player_status().perk_points;

        let path = std::env::temp_dir().join(format!(
            "feral_processes_perk_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let status = loaded.player_status();
        assert_eq!(status.perk_points, points_after_unlock);
        assert_eq!(
            status.unlocked_perks,
            vec![Perk::LowPowerMode, Perk::LowPowerMode]
        );
        assert_eq!(loaded.player_perk_level(Perk::LowPowerMode), 2);
    }

    #[test]
    fn attacker_perk_adds_permanent_atk_per_level() {
        let mut game = Game::new(115, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        let base_atk = game.player_status().atk;

        game.unlock_perk(Perk::Attacker).unwrap();
        assert_eq!(
            game.player_status().atk,
            base_atk + ATTACKER_BONUS_PER_LEVEL
        );

        game.unlock_perk(Perk::Attacker).unwrap();
        assert_eq!(
            game.player_status().atk,
            base_atk + ATTACKER_BONUS_PER_LEVEL * 2
        );
    }

    #[test]
    fn defender_perk_adds_permanent_def_per_level() {
        let mut game = Game::new(116, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        let base_def = game.player_status().def;

        game.unlock_perk(Perk::Defender).unwrap();
        assert_eq!(
            game.player_status().def,
            base_def + DEFENDER_BONUS_PER_LEVEL
        );
    }

    #[test]
    fn buffer_perk_adds_percent_max_hp_per_level_floored_and_fully_heals() {
        let mut game = Game::new(117, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        let base_max_hp = game.player_status().max_hp;
        {
            let mut stats = game.world.get_mut::<Stats>(player).unwrap();
            stats.hp = 1;
        }

        game.unlock_perk(Perk::Buffer).unwrap();
        let status = game.player_status();
        // 1% of the starting max HP rounds to well under the floor, so the
        // minimum bonus is what actually applies here.
        assert_eq!(status.max_hp, base_max_hp + BUFFER_MIN_BONUS_PER_LEVEL);
        assert_eq!(
            status.hp, status.max_hp,
            "buying Buffer should fully heal, like a level-up does"
        );
    }

    #[test]
    fn buffer_perk_scales_past_the_floor_at_high_max_hp() {
        let mut game = Game::new(118, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        {
            let mut stats = game.world.get_mut::<Stats>(player).unwrap();
            stats.max_hp = 2000;
            stats.hp = 2000;
        }

        game.unlock_perk(Perk::Buffer).unwrap();
        let status = game.player_status();
        assert_eq!(
            status.max_hp, 2020,
            "1% of 2000 is 20, above the floor, so that's what should apply"
        );
    }

    #[test]
    fn entering_a_zone_portal_increments_zone_and_doubles_wild_stats() {
        let mut game = Game::new(40, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert_eq!(game.player_status().zone, 1);
        let player = game.player_entity();
        let ppos = *game.world.get::<Position>(player).unwrap();

        game.world.spawn((
            Structure {
                kind: "portal".to_string(),
            },
            Position {
                x: ppos.x + 1,
                y: ppos.y,
            },
        ));

        game.move_player(1, 0);

        assert_eq!(
            game.player_status().zone,
            2,
            "walking onto a zone portal should advance the zone level"
        );

        let species_db = game.species_defs();
        let mut query = game
            .world
            .query_filtered::<(&Creature, &Stats, &Position), With<Hostile>>();
        let results: Vec<_> = query
            .iter(&game.world)
            .map(|(c, s, p)| (c.species.clone(), s.max_hp, *p))
            .collect();
        assert!(
            !results.is_empty(),
            "zone 2 should have spawned wild creatures"
        );
        for (species_id, max_hp, _pos) in results {
            let species = species_db.iter().find(|s| s.id == species_id).unwrap();
            // Zone 2 doubles base stats at minimum (`ZoneLevel::stat_multiplier`);
            // `distance_stat_multiplier` can scale it up further (capped at
            // `MAX_DISTANCE_STAT_MULTIPLIER`) depending how far from the
            // zone's entry point it spawned. Checked as a range rather than
            // an exact figure since `WanderAi` may have already moved this
            // creature from its spawn position by the time this runs.
            assert!(
                max_hp >= species.base_hp * 2,
                "zone 2 wild creatures should have at least doubled stats"
            );
            assert!(
                (max_hp as f32) <= (species.base_hp as f32) * 2.0 * MAX_DISTANCE_STAT_MULTIPLIER,
                "zone 2 wild creatures shouldn't exceed the zone doubling times the distance cap"
            );
        }
    }

    #[test]
    fn distance_stat_multiplier_grows_with_distance_from_the_zone_spawn_point_and_caps() {
        let game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let spawn = *game.world.resource::<ZoneSpawnPoint>();

        assert_eq!(
            game.distance_stat_multiplier(spawn.x, spawn.y),
            1.0,
            "right at the spawn point, distance shouldn't add any scaling"
        );
        assert_eq!(
            game.distance_stat_multiplier(spawn.x + DISTANCE_STAT_STEP_TILES - 1, spawn.y),
            1.0,
            "just short of a full step away should still read as no scaling"
        );
        assert!(
            (game.distance_stat_multiplier(spawn.x + DISTANCE_STAT_STEP_TILES, spawn.y) - 1.25)
                .abs()
                < f32::EPSILON,
            "one full step away should add one step of bonus"
        );
        assert!(
            (game.distance_stat_multiplier(spawn.x + DISTANCE_STAT_STEP_TILES * 2, spawn.y) - 1.5)
                .abs()
                < f32::EPSILON,
            "two full steps away should add two steps of bonus"
        );
        assert_eq!(
            game.distance_stat_multiplier(spawn.x + 10_000, spawn.y),
            MAX_DISTANCE_STAT_MULTIPLIER,
            "far enough away should cap rather than grow without bound"
        );
    }

    #[test]
    fn max_pack_size_grows_with_zone_and_distance_and_caps_per_zone() {
        let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let spawn = *game.world.resource::<ZoneSpawnPoint>();

        assert_eq!(
            game.max_pack_size(spawn.x, spawn.y),
            1,
            "right at spawn, packs should always be solo"
        );
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES - 1, spawn.y),
            1,
            "just short of a full step away should still be solo"
        );
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES, spawn.y),
            2,
            "one full step away should allow a packmate, and zone 1's cap is 2"
        );
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES * 10, spawn.y),
            2,
            "zone 1's cap of 2 should hold even far past the first step"
        );

        game.world.resource_mut::<ZoneLevel>().0 = 2;
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES, spawn.y),
            2,
            "zone 2 grows the same way per step, just with a higher cap"
        );
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES * 2, spawn.y),
            3,
            "two steps away should reach zone 2's cap of 3"
        );
    }

    #[test]
    fn defeating_the_front_pack_member_continues_the_battle_against_the_next_one() {
        let species_id = {
            let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.species_defs()
                .into_iter()
                .next()
                .expect("at least one species")
                .id
                .clone()
        };
        let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut stats = game.world.get_mut::<Stats>(player).unwrap();
            stats.atk = 1000; // guarantees a one-shot kill on the front target below
        }
        let front = game
            .world
            .spawn((
                Creature {
                    species: species_id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 0,
                },
            ))
            .id();
        let second = game
            .world
            .spawn((
                Creature {
                    species: species_id.clone(),
                },
                Hostile,
                Position { x: 6, y: 5 },
                Stats {
                    hp: 500,
                    max_hp: 500,
                    atk: 1,
                    def: 0,
                },
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![front, second],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_attack();

        assert!(
            game.has_active_battle(),
            "a pack member is still alive, so the fight should continue rather than end"
        );
        let view = game
            .battle_view()
            .expect("battle should still be active with the second member up front");
        assert_eq!(
            view.pack_remaining, 0,
            "only the second (surviving) member should remain, now as the front"
        );
        assert_eq!(
            view.wild_hp, 500,
            "the new front should be the untouched second pack member"
        );
    }

    #[test]
    fn gather_pack_pulls_in_nearby_hostiles_and_caps_at_max_pack_size() {
        let species_id = {
            let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.species_defs()
                .into_iter()
                .next()
                .expect("at least one species")
                .id
                .clone()
        };
        let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let spawn = *game.world.resource::<ZoneSpawnPoint>();
        // Far enough out that zone 1's pack cap (2) is fully unlocked.
        let (ax, ay) = (spawn.x + PACK_SIZE_STEP_TILES * 5, spawn.y);
        let spawn_hostile = |game: &mut Game, x: i32, y: i32| {
            game.world
                .spawn((
                    Creature {
                        species: species_id.clone(),
                    },
                    Hostile,
                    Position { x, y },
                    Stats {
                        hp: 10,
                        max_hp: 10,
                        atk: 1,
                        def: 0,
                    },
                ))
                .id()
        };
        let anchor = spawn_hostile(&mut game, ax, ay);
        for i in 1..=3 {
            spawn_hostile(&mut game, ax + i, ay);
        }

        let pack = game.gather_pack(anchor);

        assert_eq!(
            pack[0], anchor,
            "the creature actually bumped into should always be the pack's front"
        );
        assert!(
            pack.len() <= 2,
            "zone 1's pack cap is 2 even with 3 other Hostiles nearby, got {}",
            pack.len()
        );
        assert!(
            pack.len() >= 2,
            "at least one nearby Hostile should have joined the anchor"
        );
    }

    #[test]
    fn a_creatures_display_label_is_tagged_with_its_spawn_zone() {
        let mut game = Game::new(50, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let species = game.species_defs().into_iter().next().unwrap();

        let zone1 = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 3, y: 3 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
                ZonePortal(1),
            ))
            .id();
        let zone2 = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 4, y: 4 },
                Stats {
                    hp: 2,
                    max_hp: 2,
                    atk: 2,
                    def: 2,
                },
                ZonePortal(2),
            ))
            .id();

        assert_eq!(game.entity_label(zone1), format!("{} 1", species.name));
        assert_eq!(game.entity_label(zone2), format!("{} 2", species.name));
        assert_eq!(
            game.inspect(zone2).unwrap().name,
            format!("{} 2", species.name)
        );
    }

    #[test]
    fn defeating_a_boss_guarantees_a_cache_of_portal_fragments() {
        let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let boss = game
            .species_defs()
            .into_iter()
            .find(|s| s.is_boss)
            .expect("at least one boss species should exist in assets/species for this test");

        let wild = game
            .world
            .spawn((
                Creature {
                    species: boss.id.clone(),
                },
                Position { x: 0, y: 0 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        game.award_loot(player, wild);

        let qty = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::PortalFragment);
        assert!(
            BOSS_PORTAL_FRAGMENT_DROP.contains(&qty),
            "boss kill should guarantee a portal fragment cache in {BOSS_PORTAL_FRAGMENT_DROP:?}, got {qty}"
        );
    }

    #[test]
    fn boss_creatures_are_flagged_in_entity_and_inspect_views() {
        let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let boss = game
            .species_defs()
            .into_iter()
            .find(|s| s.is_boss)
            .expect("at least one boss species should exist in assets/species for this test");
        let normal = game
            .species_defs()
            .into_iter()
            .find(|s| !s.is_boss)
            .expect("at least one non-boss species should exist");

        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        let boss_entity = game
            .world
            .spawn((
                Creature {
                    species: boss.id.clone(),
                },
                Hostile,
                Position {
                    x: player_pos.x + 1,
                    y: player_pos.y,
                },
                Glyph {
                    ch: boss.glyph,
                    color: boss.color,
                },
                Stats {
                    hp: boss.base_hp,
                    max_hp: boss.base_hp,
                    atk: boss.base_atk,
                    def: boss.base_def,
                },
            ))
            .id();
        game.world.spawn((
            Creature {
                species: normal.id.clone(),
            },
            Hostile,
            Position {
                x: player_pos.x - 1,
                y: player_pos.y,
            },
            Glyph {
                ch: normal.glyph,
                color: normal.color,
            },
            Stats {
                hp: normal.base_hp,
                max_hp: normal.base_hp,
                atk: normal.base_atk,
                def: normal.base_def,
            },
        ));

        let views = game.view_entities(5, 5);
        let boss_view = views.iter().find(|v| v.entity == boss_entity).unwrap();
        assert!(
            boss_view.is_boss,
            "the boss creature's EntityView should be flagged is_boss"
        );
        let normal_views: Vec<_> = views
            .iter()
            .filter(|v| v.entity != boss_entity && v.is_hostile)
            .collect();
        assert!(
            normal_views.iter().all(|v| !v.is_boss),
            "non-boss creatures shouldn't be flagged is_boss"
        );

        assert!(
            game.inspect(boss_entity).unwrap().is_boss,
            "InspectView should also flag a boss creature"
        );
    }

    #[test]
    fn view_entities_colors_hostiles_by_difficulty_and_leaves_others_alone() {
        let mut game = Game::new(53, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let player_pos = *game.world.get::<Position>(player).unwrap();
        game.world.get_mut::<Stats>(player).unwrap().atk = 0;
        game.world.get_mut::<Stats>(player).unwrap().def = 0;
        game.world.get_mut::<Stats>(player).unwrap().max_hp = 100;
        game.world.get_mut::<Stats>(player).unwrap().hp = 100;
        // Player power is now 100. An easy hostile is well under that; a
        // hard one is well over it.
        let easy = game
            .world
            .spawn((
                Creature {
                    species: "does_not_matter".to_string(),
                },
                Hostile,
                Position {
                    x: player_pos.x + 1,
                    y: player_pos.y,
                },
                Glyph {
                    ch: 'e',
                    color: GlyphColor::Cyan,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        let hard = game
            .world
            .spawn((
                Creature {
                    species: "does_not_matter".to_string(),
                },
                Hostile,
                Position {
                    x: player_pos.x - 1,
                    y: player_pos.y,
                },
                Glyph {
                    ch: 'h',
                    color: GlyphColor::Cyan,
                },
                Stats {
                    hp: 300,
                    max_hp: 300,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        let tamed_worker = spawn_tamed(&mut game, 10, 3);
        game.world.entity_mut(tamed_worker).insert(Position {
            x: player_pos.x,
            y: player_pos.y + 1,
        });
        game.world.entity_mut(tamed_worker).insert(Glyph {
            ch: 't',
            color: GlyphColor::Cyan,
        });

        let views = game.view_entities(5, 5);
        let easy_view = views.iter().find(|v| v.entity == easy).unwrap();
        let hard_view = views.iter().find(|v| v.entity == hard).unwrap();
        let tamed_view = views.iter().find(|v| v.entity == tamed_worker).unwrap();

        assert_eq!(
            easy_view.color,
            GlyphColor::Green,
            "a much weaker hostile should read Green"
        );
        assert_eq!(
            hard_view.color,
            GlyphColor::Red,
            "a much stronger hostile should read Red"
        );
        assert_eq!(
            tamed_view.color,
            GlyphColor::Cyan,
            "a non-hostile entity should keep its own glyph color, not be difficulty-colored"
        );
    }

    #[test]
    fn stunned_player_loses_their_turn_but_wild_still_retaliates_and_stun_clears() {
        let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Deliberately effect-free, so the wild creature's own retaliation
        // can't re-apply (and thus reset the clock on) the status this test
        // is tracking.
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
            .expect("at least one species with no status-effect moves should exist for this test");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 50,
                    max_hp: 50,
                    atk: 3,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
            kind: StatusKind::Stun,
            remaining: 1,
            power: 0,
        });

        let wild_hp_before = game.world.get::<Stats>(wild).unwrap().hp;
        game.battle_attack();
        let wild_hp_after = game.world.get::<Stats>(wild).unwrap().hp;

        assert_eq!(
            wild_hp_before, wild_hp_after,
            "a stunned player shouldn't deal any attack damage"
        );
        assert!(
            game.world
                .get::<StatusEffects>(player)
                .unwrap()
                .active
                .is_none(),
            "the stun should clear after its one round elapses"
        );
    }

    #[test]
    fn bleed_status_deals_extra_damage_each_round_and_expires_after_its_duration() {
        let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Deliberately effect-free, so the wild creature's own retaliation
        // can't re-apply (and thus reset the clock on) the status this test
        // is tracking.
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
            .expect("at least one species with no status-effect moves should exist for this test");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 0,
                    def: 0,
                },
                StatusEffects {
                    active: Some(ActiveStatus {
                        kind: StatusKind::Bleed,
                        remaining: 2,
                        power: 5,
                    }),
                },
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        let player_atk = game.world.get::<Stats>(player).unwrap().atk;
        let expected_attack_dmg = battle::compute_damage(player_atk, 0, 5);

        let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
        game.battle_attack();
        let hp_after = game.world.get::<Stats>(wild).unwrap().hp;
        assert_eq!(
            hp_before - hp_after,
            expected_attack_dmg + 5,
            "wild should take its attack damage plus one round of bleed"
        );
        assert_eq!(
            game.world
                .get::<StatusEffects>(wild)
                .unwrap()
                .active
                .unwrap()
                .remaining,
            1
        );

        let hp_before2 = game.world.get::<Stats>(wild).unwrap().hp;
        game.battle_attack();
        let hp_after2 = game.world.get::<Stats>(wild).unwrap().hp;
        assert_eq!(
            hp_before2 - hp_after2,
            expected_attack_dmg + 5,
            "the second bleed round should also tick"
        );
        assert!(
            game.world
                .get::<StatusEffects>(wild)
                .unwrap()
                .active
                .is_none(),
            "bleed should clear once its duration elapses"
        );
    }

    #[test]
    fn status_effects_are_cleared_once_the_battle_ends() {
        let mut game = Game::new(63, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Deliberately effect-free, so the wild creature's own retaliation
        // can't re-apply (and thus reset the clock on) the status this test
        // is tracking.
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
            .expect("at least one species with no status-effect moves should exist for this test");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
            kind: StatusKind::Bleed,
            remaining: 5,
            power: 1,
        });

        // 1 HP wild creature dies to the player's first attack, ending the battle.
        game.battle_attack();

        assert!(
            !game.has_active_battle(),
            "the wild creature's death should end the battle"
        );
        assert!(
            game.world
                .get::<StatusEffects>(player)
                .unwrap()
                .active
                .is_none(),
            "leftover status effects should be cleared once the battle ends, however it ends"
        );
    }

    #[test]
    fn zone_transition_carries_tamed_companions_but_leaves_structures_and_wild_creatures_behind() {
        let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let ppos = *game.world.get::<Position>(player).unwrap();

        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();

        let species = game.species_defs().into_iter().next().unwrap();
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position {
                    x: ppos.x + 3,
                    y: ppos.y,
                },
                Stats {
                    hp: 5,
                    max_hp: 5,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        let home = game
            .world
            .spawn((
                Structure {
                    kind: "home".to_string(),
                },
                Position {
                    x: ppos.x + 5,
                    y: ppos.y,
                },
            ))
            .id();

        game.world.spawn((
            Structure {
                kind: "portal".to_string(),
            },
            Position {
                x: ppos.x + 1,
                y: ppos.y,
            },
        ));

        game.move_player(1, 0);

        assert_eq!(game.player_status().zone, 2);
        assert!(
            game.world.get::<Tamed>(companion).is_some(),
            "the companion should still be tamed after breaching"
        );
        assert!(
            game.world.get::<Creature>(wild).is_none(),
            "wild creatures should be left behind, not carried through the portal"
        );
        assert!(
            game.world.get::<Structure>(home).is_none(),
            "structures should be left behind when breaching a zone"
        );
        let companion_pos = *game.world.get::<Position>(companion).unwrap();
        let player_pos = *game.world.get::<Position>(player).unwrap();
        assert_eq!(
            companion_pos, player_pos,
            "the companion should travel with the player into the new zone"
        );
    }

    #[test]
    fn portal_build_cost_scales_with_current_zone_level() {
        let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        place_home(&mut game, -1, 0);

        // Zone 1: base rate from portal.ron is 10 PortalFragment * zone 1 = 10.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::PortalFragment, 10);
        game.place_structure("portal", 1, 0).unwrap();
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(ItemId::PortalFragment),
            0,
            "zone 1 portal should cost the base rate"
        );

        game.move_player(1, 0);
        assert_eq!(game.player_status().zone, 2);
        // Zone transitions leave structures behind (see
        // `zone_transition_carries_tamed_companions_but_leaves_structures_and_wild_creatures_behind`),
        // so the new zone needs its own Home before anything else.
        place_home(&mut game, -1, 0);

        // Zone 2: cost should now be doubled (10 * zone level 2 = 20).
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::PortalFragment, 19);
        assert!(
            game.place_structure("portal", 1, 0).is_err(),
            "19 fragments shouldn't be enough for a zone-2 portal"
        );
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::PortalFragment, 1);
        game.place_structure("portal", 1, 0).unwrap();
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(ItemId::PortalFragment),
            0,
            "zone 2 portal should cost double the base rate"
        );
    }

    #[test]
    fn zone_level_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
        let player = game.player_entity();
        let ppos = *game.world.get::<Position>(player).unwrap();
        game.world.spawn((
            Structure {
                kind: "portal".to_string(),
            },
            Position {
                x: ppos.x + 1,
                y: ppos.y,
            },
        ));
        game.move_player(1, 0);
        assert_eq!(game.player_status().zone, 2);

        let path = std::env::temp_dir().join(format!(
            "feral_processes_zone_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            loaded.player_status().zone,
            2,
            "zone level should survive a save/load round trip"
        );
    }

    /// Regression test for a nearly-empty zone: `find_walkable_start`
    /// always re-centers a freshly generated zone's spawn box near world
    /// origin, and the terrain noise there has roughly the same period as
    /// that box — so a blind, one-attempt-per-slot spawn (the previous
    /// behavior of `spawn_initial_creatures`) could land almost all 14
    /// rolls on an unwalkable or habitat-mismatched tile for an unlucky
    /// seed, leaving the new zone feeling all but abandoned. Sweeps a
    /// range of seeds (rather than trusting one lucky one) to confirm the
    /// retry-until-`count` fix reliably delivers the full population.
    #[test]
    fn zone_transition_reliably_populates_the_new_zone_regardless_of_seed() {
        for seed in 0u32..20 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let player = game.player_entity();
            let ppos = *game.world.get::<Position>(player).unwrap();
            // The zone-1 starting spawn can, for some seeds, happen to
            // place a wild creature right on the tile the portal is about
            // to go on — clear it so the walk onto the portal deterministically
            // enters the portal rather than picking a fight instead.
            let blockers: Vec<Entity> = {
                let mut query = game.world.query_filtered::<(Entity, &Position), With<Hostile>>();
                query
                    .iter(&game.world)
                    .filter(|(_, p)| p.x == ppos.x + 1 && p.y == ppos.y)
                    .map(|(e, _)| e)
                    .collect()
            };
            for e in blockers {
                game.world.despawn(e);
            }
            game.world.spawn((
                Structure {
                    kind: "portal".to_string(),
                },
                Position {
                    x: ppos.x + 1,
                    y: ppos.y,
                },
            ));
            game.move_player(1, 0);
            assert_eq!(game.player_status().zone, 2, "seed {seed}: portal should advance the zone");

            let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
            let count = query.iter(&game.world).count();
            assert!(
                count >= 14,
                "seed {seed}: zone 2 should have spawned at least the 14 requested wild \
                 creatures, found {count}"
            );
        }
    }

    #[test]
    fn bumping_a_nest_damages_it_and_destroying_it_frees_its_guardians() {
        let mut game = Game::new(603, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Position>(player).unwrap().x = 49;
        game.world.get_mut::<Position>(player).unwrap().y = 50;

        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 50, y: 50 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability { hp: 5, max_hp: 5 },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                WanderAi::default(),
                NestGuardian { nest },
                Position { x: 52, y: 52 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        // Player's base ATK (6) vs. 0 defense, move_power 5 → well over 5
        // damage, so one bump is enough to destroy a 5-HP nest.
        game.move_player(1, 0);

        assert!(
            game.world.get::<Nest>(nest).is_none(),
            "nest should be destroyed by one bump"
        );
        assert!(
            game.world.get::<NestGuardian>(guardian).is_none(),
            "guardian should lose its NestGuardian tether once the nest is destroyed"
        );
    }

    #[test]
    fn bumping_a_nest_with_high_hp_damages_it_without_destroying_it() {
        let mut game = Game::new(604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Position>(player).unwrap().x = 49;
        game.world.get_mut::<Position>(player).unwrap().y = 50;

        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 50, y: 50 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability { hp: 50, max_hp: 50 },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                WanderAi::default(),
                NestGuardian { nest },
                Position { x: 52, y: 52 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        // Player's base ATK (6) vs. 0 defense, move_power 5 → 11 damage,
        // well short of the nest's 50 HP, so one bump only dents it.
        game.move_player(1, 0);

        assert!(
            game.world.get::<Nest>(nest).is_some(),
            "nest should survive a single bump when it has 50 HP"
        );
        let hp = game.world.get::<Durability>(nest).unwrap().hp;
        assert!(hp < 50, "nest HP should have decreased from the bump, got {hp}");
        assert!(hp > 0, "nest HP should still be positive, got {hp}");
        assert!(
            game.world.get::<NestGuardian>(guardian).is_some(),
            "guardian should keep its NestGuardian tether while the nest survives"
        );
    }

    #[test]
    fn killing_a_guardian_respawns_a_replacement_after_exactly_the_respawn_delay() {
        let mut game = Game::new(604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 60, y: 60 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                NestGuardian { nest },
                Position { x: 61, y: 61 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![guardian],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_attack();

        // battle_attack's own kill-resolution path (finish_front_pack_member
        // returning true, the pack now empty) already calls self.tick() once
        // internally before returning — that tick already ran
        // nest_respawn_tick and decremented the entry we just pushed. So the
        // value observed here is NEST_RESPAWN_TICKS - 1, not the full delay.
        assert_eq!(
            game.world.get::<Nest>(nest).unwrap().pending_respawns,
            vec![NEST_RESPAWN_TICKS - 1],
            "killing a guardian should queue one respawn"
        );

        let guardian_count = |game: &mut Game| -> usize {
            let mut query = game.world.query::<&NestGuardian>();
            query.iter(&game.world).filter(|g| g.nest == nest).count()
        };

        for _ in 0..(NEST_RESPAWN_TICKS - 2) {
            game.tick();
        }
        assert_eq!(
            guardian_count(&mut game),
            0,
            "no replacement should spawn before its delay elapses"
        );

        game.tick();
        assert_eq!(
            guardian_count(&mut game),
            1,
            "a replacement should spawn exactly when its delay elapses"
        );
    }

    #[test]
    fn taming_a_guardian_also_queues_a_respawn() {
        let mut game = Game::new(605, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 70, y: 70 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                WanderAi::default(),
                NestGuardian { nest },
                Position { x: 71, y: 71 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![guardian],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::IceBreaker, 50);
        game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

        for _ in 0..50 {
            if game.world.get::<Tamed>(guardian).is_some() {
                break;
            }
            game.battle_decompile();
        }

        assert!(game.world.get::<Tamed>(guardian).is_some());
        assert!(
            game.world.get::<NestGuardian>(guardian).is_none(),
            "a tamed creature should lose its nest tether"
        );
        // Same off-by-one as the kill test above: battle_decompile's
        // success path also calls self.tick() once internally before
        // returning, which already decremented the entry we just pushed.
        assert_eq!(
            game.world.get::<Nest>(nest).unwrap().pending_respawns,
            vec![NEST_RESPAWN_TICKS - 1],
            "taming a guardian should also queue one respawn"
        );
    }

    #[test]
    fn killing_a_guardian_whose_nest_is_already_gone_queues_nothing() {
        let mut game = Game::new(606, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // A dangling nest Entity — never actually spawned, standing in
        // for "the nest was destroyed before this guardian died."
        let gone_nest = game.world.spawn_empty().id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                NestGuardian { nest: gone_nest },
                Position { x: 80, y: 80 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![guardian],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        // Should not panic even though `gone_nest` has no Nest component.
        game.battle_attack();

        for _ in 0..(NEST_RESPAWN_TICKS + 5) {
            game.tick();
        }
        // Nothing to assert beyond "didn't panic" — there's no Nest left
        // to have queued anything on, and no new guardian entity for a
        // nonexistent nest.
    }

    #[test]
    fn nest_respawn_tick_spawns_one_guardian_per_ready_entry_not_one_per_nest() {
        let mut game = Game::new(607, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    // Two entries reach 0 on the same tick, and a third
                    // untouched entry that should survive, decremented but
                    // not fired — this proves nest_respawn_tick spawns once
                    // per ready entry, not once per nest.
                    pending_respawns: vec![1, 1, 5],
                },
                Position { x: 90, y: 90 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();

        let guardian_count = |game: &mut Game| -> usize {
            let mut query = game.world.query::<&NestGuardian>();
            query.iter(&game.world).filter(|g| g.nest == nest).count()
        };
        assert_eq!(guardian_count(&mut game), 0, "no guardians before the tick");

        game.tick();

        assert_eq!(
            guardian_count(&mut game),
            2,
            "both entries reaching 0 on the same tick should each spawn a guardian"
        );
        assert_eq!(
            game.world.get::<Nest>(nest).unwrap().pending_respawns,
            vec![4],
            "the two fired entries should be removed and the untouched entry decremented once"
        );
    }
}
>>>>>>> Stashed changes
