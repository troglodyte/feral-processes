//! What a fight pays out — loot, experience, and how it spreads across the party.

use super::support::*;
use crate::*;

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

    let before = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&resource);
    game.award_loot(wild);
    let after = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&resource);

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
        .find(|s| s.work_resource.is_none())
        .expect("at least one species should have no work_resource for this test");

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

    // Portal Fragments are a universal drop, and gear arrives on its own
    // `droppable` channel — count neither, so this only measures whether
    // the absent `work_resource` stayed silent.
    let count_resources = |game: &Game| -> u32 {
        let gear_ids: Vec<ItemId> = game
            .world
            .resource::<ItemDb>()
            .all()
            .filter(|d| d.equipment.is_some())
            .map(|d| d.id.clone())
            .collect();
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .items
            .iter()
            .filter(|(item, _)| {
                *item != ItemId::from(ids::PORTAL_FRAGMENT) && !gear_ids.contains(item)
            })
            .map(|(_, q)| *q)
            .sum()
    };
    let before = count_resources(&game);
    game.award_loot(wild);
    let after = count_resources(&game);

    assert_eq!(
        before, after,
        "no-resource species shouldn't add anything besides a possible portal fragment"
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

    game.award_loot(wild);

    let qty = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::PORTAL_FRAGMENT));
    assert!(
        BOSS_PORTAL_FRAGMENT_DROP.contains(&qty),
        "boss kill should guarantee a portal fragment cache in {BOSS_PORTAL_FRAGMENT_DROP:?}, got {qty}"
    );
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
fn higher_growth_multiplier_species_out_grows_a_baseline_one_via_award_party_xp() {
    let mut game = Game::new(419, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game.species_defs();
    let baseline_id = species
        .iter()
        .find(|s| s.growth_multiplier == progression::BASELINE_GROWTH_MULTIPLIER)
        .expect("base roster should have at least one baseline-growth species")
        .id
        .clone();
    let boosted_id = species
        .iter()
        .find(|s| s.growth_multiplier > progression::BASELINE_GROWTH_MULTIPLIER)
        .expect("base roster should have at least one higher-growth species")
        .id
        .clone();

    let spawn = |game: &mut Game, species: String| {
        game.world
            .spawn((
                Creature { species },
                Position { x: 3, y: 3 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 10,
                    def: 10,
                },
                Tamed { owner: player },
                Experience {
                    level: 1,
                    xp: 0,
                    xp_to_next: 1,
                },
            ))
            .id()
    };
    let baseline = spawn(&mut game, baseline_id);
    let boosted = spawn(&mut game, boosted_id);
    game.add_companion(baseline).unwrap();
    game.add_companion(boosted).unwrap();

    // xp_to_next is rigged to 1 above, so any non-zero party XP levels
    // both companions up exactly once.
    game.award_player_xp(player, 2);

    let baseline_hp = game.world.get::<Stats>(baseline).unwrap().max_hp;
    let boosted_hp = game.world.get::<Stats>(boosted).unwrap().max_hp;
    assert!(
        boosted_hp > baseline_hp,
        "a higher growth_multiplier species should out-grow a baseline one: {boosted_hp} vs {baseline_hp}"
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
    insert_battle(&mut game, player, vec![wild]);

    player_attacks(&mut game);

    assert_eq!(
        game.world.get::<Experience>(companion).unwrap().xp,
        5,
        "killing a 10-max-HP wild program should award the party member half its max HP as XP"
    );
}

/// The player has no level ceiling, while their party members stop at
/// `progression::CREATURE_MAX_LEVEL` — one big XP award should push
/// the player past that ceiling and leave the companion pinned to it.
#[test]
fn player_levels_past_the_creature_cap_but_companions_dont() {
    let mut game = Game::new(105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();

    // Party members earn half the player's award (PARTY_XP_DIVISOR),
    // so this is far past the cap for both of them.
    game.award_player_xp(player, 1_000_000);

    let player_level = game.world.get::<Experience>(player).unwrap().level;
    let companion_level = game.world.get::<Experience>(companion).unwrap().level;
    assert!(
        player_level > progression::CREATURE_MAX_LEVEL,
        "the player should keep leveling past the creature ceiling, got {player_level}"
    );
    assert_eq!(
        companion_level,
        progression::CREATURE_MAX_LEVEL,
        "a companion should still stop at the creature ceiling"
    );
}

/// Each member of a group pays its own XP when it dies — five kills pay
/// five times, not once. A swarm's whole reward curve rests on this, and
/// `finish_member` is reached from every death path (attack, ability,
/// status tick), so it is worth pinning independently of any of them.
#[test]
fn every_member_of_a_group_pays_its_own_xp_when_it_dies() {
    let mut game = Game::new(88, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 6;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    // Deep and far, so the per-group ceiling this fixture will live under
    // once Task 5 lands is 100 rather than 1.
    let (x, y) = (spawn.x + DISTANCE_STAT_STEP_TILES * 8, spawn.y);
    let player = game.player_entity();

    let members: Vec<Entity> = (0..5)
        .map(|i| game.spawn_wild_creature("glitch", x, y + i).unwrap())
        .collect();
    // Uniform, tiny HP: XP awarded per kill is the victim's max_hp, and
    // 5 x 3 stays under xp_for_level(1) = 20 so no level-up spends the
    // total being measured.
    for &m in &members {
        let mut stats = game.world.get_mut::<Stats>(m).unwrap();
        stats.max_hp = 3;
        stats.hp = 3;
    }
    game.start_battle(members.clone());
    let before = game.world.get::<Experience>(player).unwrap().xp;

    for _ in 0..members.len() {
        game.finish_member(0, 0, player);
    }

    let exp = game.world.get::<Experience>(player).unwrap();
    assert_eq!(
        exp.level, 1,
        "the fixture must not level up, or the XP total below measures nothing"
    );
    assert_eq!(
        exp.xp - before,
        3 * members.len() as u32,
        "every vanquished member should pay its own max_hp in XP"
    );
}
