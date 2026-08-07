//! Turning a `Scenario` into a `Game` and the groups it fights.

use std::path::Path;

use super::scenario::{PlayerSource, Scenario};
use super::{set_level, spawn_companion};
use crate::items::ItemId;
use crate::items_db::ItemDb;
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
            // `EquippedItem::level` off the current zone and gear doubles
            // per level, so equipping first under-scales every weapon.
            game.world.resource_mut::<ZoneLevel>().0 = *zone;
            let player = game.player_entity();
            set_level(&mut game, player, *level);

            for row in &scenario.inventory {
                known_item(&game, &row.item)?;
                game.add_copies(&row.item, 0, row.qty);
            }
            for row in &scenario.equip {
                known_item(&game, &row.item)?;
                game.add_copies(&row.item, row.tier, 1);
                game.equip(&row.item, row.tier)
                    .map_err(|e| format!("equip `{}`: {e}", row.item.as_str()))?;
            }
            for row in &scenario.party {
                let program = spawn_companion(&mut game, &row.species, row.level)
                    .ok_or_else(|| format!("unknown companion species `{}`", row.species))?;
                game.add_companion(program)
                    .map_err(|e| format!("party `{}`: {e}", row.species))?;
            }
            Ok(game)
        }
    }
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
    use crate::arena::scenario::{CompanionSpec, EquipSpec, InventorySpec, OpponentSpec};
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
        }];

        let game = build_player(&s, &test_assets_dir()).unwrap();

        let equipment = game.world.get::<Equipment>(game.player_entity()).unwrap();
        let worn = [&equipment.weapon, &equipment.armor, &equipment.module]
            .into_iter()
            .flatten()
            .find(|e| e.item == item)
            .expect("the requested item is worn");
        // Gear fuses per physical copy, so a tier-blind implementation
        // would quietly measure a different weapon from the one named.
        assert_eq!(worn.fusion_tier, 2);
        assert_eq!(worn.level, 2, "gear locks in the zone it was equipped at");
    }

    #[test]
    fn an_inventory_row_is_countable_in_cargo() {
        let probe = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let item = an_equippable(&probe);
        let before = probe.count_copies(&item, 0);
        let mut s = fresh(1, 1);
        s.inventory = vec![InventorySpec {
            item: item.clone(),
            qty: 5,
        }];

        let game = build_player(&s, &test_assets_dir()).unwrap();

        assert_eq!(game.count_copies(&item, 0), before + 5);
    }

    #[test]
    fn a_party_row_becomes_a_party_member_at_the_requested_level() {
        let mut s = fresh(10, 2);
        s.party = vec![CompanionSpec {
            species: "glitch".into(),
            level: 4,
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
        }];
        let err = build_player(&s, &test_assets_dir())
            .err()
            .expect("should refuse");
        assert!(err.contains("not_a_program"), "{err}");
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
