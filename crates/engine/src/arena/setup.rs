//! Turning a `Scenario` into a `Game` and the groups it fights.

use std::path::Path;

use super::scenario::{OpponentSpec, PlayerSource, Scenario};
use super::{set_level, spawn_companion};
use crate::battle::EnemyGroup;
use crate::items::ItemId;
use crate::items_db::ItemDb;
use crate::tuning::{MAX_ENEMY_GROUPS, MAX_GROUP_SIZE};
use crate::*;

/// The player the scenario describes, standing ready.
///
/// A save or a template is taken wholesale — the run state is what it is,
/// and that is the "what would happen if I hit this pack right now"
/// question. `Fresh` is the other half, and the only variant that accepts
/// an authored loadout.
pub(crate) fn build_player(scenario: &Scenario, assets_dir: &Path) -> Result<Game, String> {
    match &scenario.player {
        PlayerSource::Save(path) => {
            Game::load(path, assets_dir).map_err(|e| format!("{}: {e}", path.display()))
        }
        PlayerSource::Template(name) => Err(format!(
            "template `{name}` must be resolved to a save by the `arena` bin — \
             the engine cannot see `dev_template`"
        )),
        // Forgiving deliberately: a permadeath loss mid-measurement is a
        // `GameOver` every later rep would inherit.
        PlayerSource::Fresh { level, zone } => {
            let mut game = Game::new(0, DifficultyMode::Forgiving, assets_dir)
                .map_err(|e| format!("{}: {e}", assets_dir.display()))?;
            // Before the equips below: `Game::equip` captures
            // `EquippedItem::level` off the current zone and gear grows by
            // `GEAR_LEVEL_STEP` per level, so equipping first under-scales
            // every weapon — the party's as well as the player's.
            game.world.resource_mut::<ZoneLevel>().0 = *zone;
            let player = game.player_entity();
            set_level(&mut game, player, *level);

            for row in &scenario.inventory {
                known_item(&game, &row.item)?;
                game.add_copies(&GearCopy::plain(row.item.clone()), row.qty);
            }
            for row in &scenario.equip {
                known_item(&game, &row.item)?;
                game.add_copies(&row.copy(), 1);
                let player = game.player_entity();
                game.equip(player, &row.copy())
                    .map_err(|e| format!("equip `{}`: {e}", row.item.as_str()))?;
            }
            for row in &scenario.party {
                let program = spawn_companion(&mut game, &row.species, row.level)
                    .ok_or_else(|| format!("unknown companion species `{}`", row.species))?;
                game.add_companion(program)
                    .map_err(|e| format!("party `{}`: {e}", row.species))?;
                // After the join: `Game::equip` checks the wearer is owned.
                for gear in &row.equip {
                    known_item(&game, &gear.item)?;
                    game.add_copies(&gear.copy(), 1);
                    game.equip(program, &gear.copy()).map_err(|e| {
                        format!("equip `{}` on `{}`: {e}", gear.item.as_str(), row.species)
                    })?;
                }
            }
            Ok(game)
        }
    }
}

/// The opponents the scenario names, spawned for real, plus any warnings
/// for the caller to print.
///
/// Every member goes through `spawn_wild_creature_scaled` on the player's
/// own tile, so the zone multiplier, the potential roll, wild routines and
/// the `Hostile`/`WanderAi`/`StatusEffects` bundle all apply exactly as a
/// map spawn's would. `depth_mult` is 1.0: Stack depth is not a scenario
/// knob, and every arena fight is a surface fight.
///
/// **One group per entry, never merged by species.** Order is the
/// formation — `ENGAGED_GROUPS` is 2, so two entries naming the same
/// species is how you place the same program both in reach and out of it,
/// and merging them would silently delete that lever.
///
/// The ceilings a real fight is capped at are *warned* about rather than
/// applied: explicit authoring is the point of a tester and "what if zone 1
/// threw nine at me" is a legitimate question. `MAX_ENEMY_GROUPS` and
/// `MAX_GROUP_SIZE` are the exception and hard-error, because past those
/// the fight is not one the game can represent at all.
pub(crate) fn build_opponents(
    game: &mut Game,
    opponents: &[OpponentSpec],
) -> Result<(Vec<EnemyGroup>, Vec<String>), String> {
    if opponents.len() > MAX_ENEMY_GROUPS {
        return Err(format!(
            "{} opponent entries, but a battle holds at most MAX_ENEMY_GROUPS ({MAX_ENEMY_GROUPS})",
            opponents.len()
        ));
    }
    for row in opponents {
        if row.count == 0 {
            return Err(format!("`{}` has a count of 0", row.species));
        }
        if row.count > MAX_GROUP_SIZE {
            return Err(format!(
                "`{}` asks for {}, but a group holds at most MAX_GROUP_SIZE ({MAX_GROUP_SIZE})",
                row.species, row.count
            ));
        }
    }

    let zone = game.world.resource::<ZoneLevel>().0;
    let size_ceiling = game.group_size_ceiling();
    let group_ceiling = game.enemy_group_ceiling();
    let mut warnings = Vec::new();
    if opponents.len() > group_ceiling {
        warnings.push(format!(
            "{} groups asked for; zone {zone} would field at most {group_ceiling}",
            opponents.len()
        ));
    }

    let pos = *game
        .world
        .get::<Position>(game.player_entity())
        .ok_or_else(|| "the player has no position to spawn around".to_string())?;
    let mut groups = Vec::with_capacity(opponents.len());
    for row in opponents {
        if row.count as usize > size_ceiling {
            warnings.push(format!(
                "`{}` x{} asked for; zone {zone} would field at most {size_ceiling}",
                row.species, row.count
            ));
        }
        let mut members = Vec::with_capacity(row.count as usize);
        for _ in 0..row.count {
            let member = game
                .spawn_wild_creature_scaled(&row.species, pos.x, pos.y, 1.0)
                .ok_or_else(|| format!("unknown opponent species `{}`", row.species))?;
            members.push(member);
        }
        groups.push(EnemyGroup {
            species: row.species.clone(),
            members,
        });
    }
    Ok((groups, warnings))
}

/// A scenario is authored, not scavenged: a typo should stop the run rather
/// than quietly leave the player one item short of what was measured.
fn known_item(game: &Game, item: &ItemId) -> Result<(), String> {
    if game.world.resource::<ItemDb>().get(item.as_str()).is_none() {
        return Err(format!("unknown item `{}`", item.as_str()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::scenario::{CompanionSpec, EquipSpec, InventorySpec};
    use crate::tests::support::test_assets_dir;

    fn scenario(player: PlayerSource) -> Scenario {
        Scenario {
            player,
            opponents: vec![OpponentSpec {
                species: "glitch".into(),
                count: 1,
            }],
            ..Scenario::default()
        }
    }

    fn fresh(level: u32, zone: u32) -> Scenario {
        scenario(PlayerSource::Fresh { level, zone })
    }

    /// The one weapon every run starts able to name, so the equip tests
    /// assert about tiers rather than about the shipped catalogue.
    fn an_equippable(game: &Game) -> ItemId {
        game.world
            .resource::<ItemDb>()
            .all()
            .find(|d| d.equipment.is_some())
            .map(|d| d.id.clone())
            .expect("the roster ships equippable gear")
    }

    #[test]
    fn a_fresh_player_arrives_at_the_requested_level_and_zone() {
        let game = build_player(&fresh(20, 3), &test_assets_dir()).unwrap();
        assert_eq!(
            game.world
                .get::<Experience>(game.player_entity())
                .unwrap()
                .level,
            20
        );
        assert_eq!(game.world.resource::<ZoneLevel>().0, 3);
    }

    #[test]
    fn an_equip_row_lands_in_the_players_equipment_at_the_requested_tier() {
        let probe = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let item = an_equippable(&probe);
        let mut s = fresh(5, 2);
        s.equip = vec![EquipSpec {
            item: item.clone(),
            tier: 2,
            rarity: Rarity::Ordinary,
        }];

        let game = build_player(&s, &test_assets_dir()).unwrap();

        let equipment = game.world.get::<Equipment>(game.player_entity()).unwrap();
        let worn = [&equipment.weapon, &equipment.armor, &equipment.module]
            .into_iter()
            .flatten()
            .find(|e| e.copy.item == item)
            .expect("the requested item is worn");
        // Gear fuses per physical copy, so a tier-blind implementation
        // would quietly measure a different weapon from the one named.
        assert_eq!(worn.copy.tier, 2);
        assert_eq!(worn.level, 2, "gear locks in the zone it was equipped at");
    }

    #[test]
    fn an_inventory_row_is_countable_in_cargo() {
        let probe = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let item = an_equippable(&probe);
        let before = probe.count_copies(&GearCopy::plain(item.clone()));
        let mut s = fresh(1, 1);
        s.inventory = vec![InventorySpec {
            item: item.clone(),
            qty: 5,
        }];

        let game = build_player(&s, &test_assets_dir()).unwrap();

        assert_eq!(
            game.count_copies(&GearCopy::plain(item.clone())),
            before + 5
        );
    }

    /// Every item worn by `wearer`, so a test can ask *who* is wearing a
    /// row rather than only whether it was equipped at all.
    fn worn(game: &Game, wearer: Entity) -> Vec<ItemId> {
        game.world
            .get::<Equipment>(wearer)
            .map(|e| {
                [&e.weapon, &e.armor, &e.module]
                    .into_iter()
                    .flatten()
                    .map(|w| w.copy.item.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_companion_equip_row_lands_on_that_companion_and_not_the_player() {
        let probe = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let item = an_equippable(&probe);
        let mut s = fresh(10, 2);
        s.party = vec![CompanionSpec {
            species: "glitch".into(),
            level: 4,
            equip: vec![EquipSpec {
                item: item.clone(),
                tier: 0,
                rarity: Rarity::Ordinary,
            }],
        }];

        let game = build_player(&s, &test_assets_dir()).unwrap();

        let program = game.world.resource::<Party>().0[0];
        assert!(worn(&game, program).contains(&item));
        // The failure mode this exists for: `Game::equip` takes a wearer,
        // so an implementation copied from the player's loop above equips
        // the player and the scenario reads as a geared party.
        assert!(
            !worn(&game, game.player_entity()).contains(&item),
            "the player is wearing the companion's gear"
        );
    }

    #[test]
    fn a_companions_gear_bonus_reaches_its_stats() {
        let probe = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let item = an_equippable(&probe);

        // Against `Game::gear_bonus` rather than the item's base mods: gear
        // scales by the zone it was equipped at, so the base numbers are
        // not what lands in `Stats` anywhere but zone 1.
        let member = |equip: Vec<EquipSpec>| {
            let mut s = fresh(10, 2);
            s.party = vec![CompanionSpec {
                species: "glitch".into(),
                level: 4,
                equip,
            }];
            let game = build_player(&s, &test_assets_dir()).unwrap();
            let program = game.world.resource::<Party>().0[0];
            let stats = game.world.get::<Stats>(program).unwrap();
            ((stats.atk, stats.def), game.gear_bonus(program))
        };

        let (bare, no_bonus) = member(Vec::new());
        let (geared, bonus) = member(vec![EquipSpec {
            item,
            tier: 0,
            rarity: Rarity::Ordinary,
        }]);
        assert_eq!((no_bonus.atk, no_bonus.def), (0, 0));
        // A zero-bonus item would make the deltas below trivially equal.
        assert!(
            bonus.atk != 0 || bonus.def != 0,
            "picked an equippable worth nothing"
        );
        assert_eq!(geared.0 - bare.0, bonus.atk);
        assert_eq!(geared.1 - bare.1, bonus.def);
    }

    #[test]
    fn an_unknown_item_in_a_companions_gear_is_an_err_naming_it() {
        let mut s = fresh(1, 1);
        s.party = vec![CompanionSpec {
            species: "glitch".into(),
            level: 1,
            equip: vec![EquipSpec {
                item: ItemId("not_an_item".into()),
                tier: 0,
                rarity: Rarity::Ordinary,
            }],
        }];

        let err = build_player(&s, &test_assets_dir())
            .err()
            .expect("should refuse");
        assert!(err.contains("not_an_item"), "{err}");
    }

    #[test]
    fn a_party_row_becomes_a_party_member_at_the_requested_level() {
        let mut s = fresh(10, 2);
        s.party = vec![CompanionSpec {
            species: "glitch".into(),
            level: 4,
            equip: Vec::new(),
        }];

        let game = build_player(&s, &test_assets_dir()).unwrap();

        let party = &game.world.resource::<Party>().0;
        assert_eq!(party.len(), 1);
        assert_eq!(game.world.get::<Experience>(party[0]).unwrap().level, 4);
        assert_eq!(
            game.world.get::<Creature>(party[0]).unwrap().species,
            "glitch"
        );
    }

    #[test]
    fn an_unknown_item_id_is_an_err_naming_it() {
        let mut s = fresh(1, 1);
        s.equip = vec![EquipSpec {
            item: ItemId("not_an_item".into()),
            tier: 0,
            rarity: Rarity::Ordinary,
        }];
        let err = build_player(&s, &test_assets_dir())
            .err()
            .expect("should refuse");
        assert!(err.contains("not_an_item"), "{err}");
    }

    #[test]
    fn an_unknown_companion_species_is_an_err_naming_it() {
        let mut s = fresh(1, 1);
        s.party = vec![CompanionSpec {
            species: "not_a_program".into(),
            level: 1,
            ..Default::default()
        }];
        let err = build_player(&s, &test_assets_dir())
            .err()
            .expect("should refuse");
        assert!(err.contains("not_a_program"), "{err}");
    }

    fn arena(zone: u32) -> Game {
        build_player(&fresh(1, zone), &test_assets_dir()).unwrap()
    }

    fn against(counts: &[(&str, u32)]) -> Vec<OpponentSpec> {
        counts
            .iter()
            .map(|(species, count)| OpponentSpec {
                species: (*species).into(),
                count: *count,
            })
            .collect()
    }

    #[test]
    fn a_group_is_built_at_the_size_asked_for_past_the_zones_ceiling() {
        let mut game = arena(1);
        assert_eq!(
            game.group_size_ceiling(),
            1,
            "zone 1 fields one program; without that this asserts nothing"
        );

        let (groups, _) = build_opponents(&mut game, &against(&[("glitch", 9)])).unwrap();

        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0].members.len(),
            9,
            "the truncation `group_pack` would apply must not happen here"
        );
    }

    #[test]
    fn exceeding_the_zones_ceiling_warns_with_the_ask_the_ceiling_and_the_zone() {
        let mut game = arena(1);
        let (_, warnings) = build_opponents(&mut game, &against(&[("glitch", 9)])).unwrap();

        assert_eq!(warnings.len(), 1, "{warnings:?}");
        let w = &warnings[0];
        assert!(w.contains('9'), "the ask: {w}");
        assert!(w.contains('1'), "the ceiling and the zone: {w}");
        assert!(w.contains("glitch"), "which entry: {w}");
    }

    #[test]
    fn a_composition_inside_the_zones_ceilings_warns_about_nothing() {
        let mut game = arena(1);
        let (_, warnings) = build_opponents(&mut game, &against(&[("glitch", 1)])).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn two_entries_naming_one_species_stay_two_groups() {
        let mut game = arena(3);
        let (groups, _) =
            build_opponents(&mut game, &against(&[("glitch", 1), ("glitch", 1)])).unwrap();

        // Merging them would delete the one lever `ENGAGED_GROUPS` gives a
        // scenario: the same program, one in melee reach and one behind it.
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].species, "glitch");
        assert_eq!(groups[1].species, "glitch");
        assert_ne!(groups[0].members[0], groups[1].members[0]);
    }

    #[test]
    fn an_unknown_opponent_species_is_an_err_naming_it() {
        let mut game = arena(1);
        let err = build_opponents(&mut game, &against(&[("not_a_program", 1)])).unwrap_err();
        assert!(err.contains("not_a_program"), "{err}");
    }

    #[test]
    fn more_entries_than_max_enemy_groups_is_an_err() {
        let mut game = arena(6);
        let rows = against(&[
            ("glitch", 1),
            ("glitch", 1),
            ("glitch", 1),
            ("glitch", 1),
            ("glitch", 1),
        ]);
        assert!(rows.len() > MAX_ENEMY_GROUPS);
        let err = build_opponents(&mut game, &rows).unwrap_err();
        assert!(err.contains("MAX_ENEMY_GROUPS"), "{err}");
    }

    #[test]
    fn an_entry_with_a_count_of_zero_is_an_err() {
        let mut game = arena(1);
        let err = build_opponents(&mut game, &against(&[("glitch", 0)])).unwrap_err();
        assert!(err.contains("glitch"), "{err}");
    }

    #[test]
    fn an_entry_past_max_group_size_is_an_err() {
        let mut game = arena(6);
        let err =
            build_opponents(&mut game, &against(&[("glitch", MAX_GROUP_SIZE + 1)])).unwrap_err();
        assert!(err.contains("MAX_GROUP_SIZE"), "{err}");
    }

    #[test]
    fn a_template_player_is_an_err_pointing_at_the_bin() {
        let s = scenario(PlayerSource::Template("extraction".into()));
        let err = build_player(&s, &test_assets_dir())
            .err()
            .expect("should refuse");
        assert!(err.contains("arena"), "{err}");
        assert!(err.contains("extraction"), "{err}");
    }
}
