//! What a fight pays out — loot, experience, and how it spreads across the party.

use super::support::*;
use crate::tuning::STACK_BOSS_PORTAL_FRAGMENT_DROP;
use crate::*;

/// A bare creature of `species` on the player's tile, ready for
/// `award_loot`. Every loot test here wants the same thing: an entity that
/// carries a `Creature` and nothing else that could pay out on its own.
fn corpse_of(game: &mut Game, species: &str) -> Entity {
    game.world
        .spawn((
            Creature {
                species: species.to_string(),
            },
            Position { x: 0, y: 0 },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 1,
            },
        ))
        .id()
}

/// Puts the party `depth` frames down without generating a frame to walk.
/// `award_loot`'s fragment branch reads the locale and its depth and
/// nothing else — `mark_lair_cleared` finds no `CurrentStack` and no-ops,
/// which is what a test about the payout wants.
fn stand_in_the_stack(game: &mut Game, depth: u32) {
    game.world.insert_resource(Locale::Stack {
        depth,
        frames: 6,
        x: 1,
        y: 1,
        facing: crate::stack::Dir::North,
        entrance: (0, 0),
    });
}

fn a_boss(game: &Game) -> SpeciesDef {
    game.species_defs()
        .into_iter()
        .find(|s| s.is_boss)
        .expect("at least one boss species should exist in assets/species for this test")
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
        .any(|e| e.kind == MessageKind::Loot);
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
fn defeating_a_boss_in_the_stack_guarantees_a_cache_of_portal_fragments() {
    let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let boss = a_boss(&game);
    stand_in_the_stack(&mut game, 1);

    let wild = corpse_of(&mut game, &boss.id);
    game.award_loot(wild);

    let qty = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::PORTAL_FRAGMENT));
    assert!(
        STACK_BOSS_PORTAL_FRAGMENT_DROP.contains(&qty),
        "a depth-1 lair boss should pay a cache in {STACK_BOSS_PORTAL_FRAGMENT_DROP:?}, got {qty}"
    );
}

#[test]
fn a_deeper_lair_boss_pays_more_portal_fragments() {
    let paid_at = |depth: u32| {
        // Same seed either side, so both runs consume GameRng identically
        // right up to the payout — depth multiplies the roll rather than
        // changing how many draws are made, which makes this a comparison
        // of one roll scaled two ways.
        let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let boss = a_boss(&game);
        stand_in_the_stack(&mut game, depth);
        let wild = corpse_of(&mut game, &boss.id);
        game.award_loot(wild);
        game.world
            .get::<Inventory>(game.player_entity())
            .unwrap()
            .count(&ItemId::from(ids::PORTAL_FRAGMENT))
    };

    let shallow = paid_at(1);
    let deep = paid_at(3);
    assert_eq!(
        deep,
        shallow * 3,
        "depth is the lever on the one faucet that pays the breaching currency, so the \
         bottom of a stack has to be worth the walk back up (depth 1 paid {shallow}, \
         depth 3 paid {deep})"
    );
}

#[test]
fn a_boss_defeated_on_the_surface_pays_no_portal_fragments() {
    let mut game = Game::new(53, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let boss = a_boss(&game);
    assert!(
        !game.is_underground(),
        "test premise: a fresh game starts on the surface"
    );

    let wild = corpse_of(&mut game, &boss.id);
    game.award_loot(wild);

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::PORTAL_FRAGMENT)),
        0,
        "progress toward the next zone is bought underground or not at all — a surface \
         boss pays in gear instead"
    );
}

#[test]
fn an_ordinary_kill_pays_no_portal_fragments() {
    // Underground, where the payout does exist, so this measures the
    // `is_boss` half of the gate rather than passing for free on the
    // locale half. A lair's escort past zone 1 is exactly this case.
    let mut game = Game::new(54, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ordinary = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is mostly ordinary species");
    stand_in_the_stack(&mut game, 3);

    let wild = corpse_of(&mut game, &ordinary.id);
    game.award_loot(wild);

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::PORTAL_FRAGMENT)),
        0,
        "only the thing at the bottom of a stack pays fragments; everything else it \
         brought with it does not"
    );
}

/// The rule is universal, not an opening-zone gate: a run eight sectors
/// deep still buys its next portal underground. Swept rather than asserted
/// once because a zone-scaled payout is the obvious thing for a later
/// retune to reach for, and reintroducing one on the *surface* branch would
/// quietly restore the grind-to-breach route this change closed.
#[test]
fn no_zone_lets_a_surface_boss_pay_the_breaching_currency() {
    let mut game = Game::new(58, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = a_boss(&game);
    let player = game.player_entity();

    for zone in 1..=8 {
        set_zone(&mut game, zone);
        let wild = corpse_of(&mut game, &boss.id);
        game.award_loot(wild);
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::PORTAL_FRAGMENT)),
            0,
            "a boss killed on the surface in zone {zone} paid the breaching currency — \
             every zone is breached out of the Stack, not just the first"
        );
    }
}

#[test]
fn a_boss_defeated_on_the_surface_pays_gear_from_its_zones_band() {
    let mut game = Game::new(55, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = a_boss(&game);
    let band = game.surface_boss_loot();
    assert!(!band.is_empty(), "zone 1 should have a band to draw from");

    let before: u32 = band.iter().map(|id| held(&game, id)).sum();
    let wild = corpse_of(&mut game, &boss.id);
    game.award_loot(wild);
    let after: u32 = band.iter().map(|id| held(&game, id)).sum();

    assert!(
        after > before,
        "a surface boss pays power where a Stack boss pays progression, and the band it \
         draws from is {band:?}"
    );
}

#[test]
fn the_surface_boss_band_climbs_the_value_ladder_with_the_zone() {
    let mut game = Game::new(56, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let value = |game: &Game, id: &ItemId| game.item_value(id);

    let mut previous_best = 0;
    for zone in 1..=5 {
        set_zone(&mut game, zone);
        let band = game.surface_boss_loot();
        assert!(
            !band.is_empty(),
            "zone {zone}'s band selects nothing from the shipped ladder, so every boss in \
             it would fall back to the top tier"
        );
        let best = band.iter().map(|id| value(&game, id)).max().unwrap();
        assert!(
            best >= previous_best,
            "the band must not walk back down the ladder: zone {zone} tops out at {best} \
             where the zone before it reached {previous_best}"
        );
        previous_best = best;
    }
    assert!(
        previous_best >= 80,
        "by zone 5 a boss should be paying the premium tier, not still handing out \
         standard gear (best was {previous_best})"
    );
}

#[test]
fn a_zone_past_the_top_of_the_ladder_still_pays_the_best_gear_there_is() {
    let mut game = Game::new(57, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // The ceiling climbs forever and the shipped ladder stops at 120, so
    // far enough out the band's own floor rises past every item there is.
    set_zone(&mut game, 40);

    let band = game.surface_boss_loot();
    assert!(
        !band.is_empty(),
        "a band that empties must fall back to the best gear rather than paying nothing"
    );
    let best_in_game = game
        .world
        .resource::<ItemDb>()
        .all()
        .filter(|d| d.equipment.is_some())
        .map(|d| game.item_value(&d.id))
        .max()
        .unwrap();
    assert!(
        band.iter().all(|id| game.item_value(id) == best_in_game),
        "the fallback is the top of the ladder and nothing below it, got {band:?}"
    );
}

#[test]
fn a_running_drop_boost_field_buff_scales_every_equipment_drop_chance() {
    let mut game = Game::new(38, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !game.equipment_drops_for(s).is_empty())
        .expect("at least one species should have equipment drops for this test");

    let before = game.equipment_drops_for(&species);
    assert!(!before.is_empty());

    let player = game.player_entity();
    game.world.entity_mut(player).insert(FieldBuff {
        active: vec![ActiveFieldBuff {
            kind: FieldBuffKind::DropBoost,
            name: "Test Drop Boost".to_string(),
            power: 50,
            remaining: 10,
            interval: 1,
            source: BuffSource::Routine,
        }],
    });

    let after = game.equipment_drops_for(&species);

    assert_eq!(after.len(), before.len());
    for ((before_item, before_chance), (after_item, after_chance)) in
        before.iter().zip(after.iter())
    {
        assert_eq!(
            before_item, after_item,
            "a DropBoost must not reorder drops"
        );
        assert!(
            (after_chance - before_chance * 1.5).abs() < 1e-6,
            "a 50% DropBoost should scale every drop chance by 1.5x: \
             {after_chance} vs {before_chance}"
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
fn a_running_xp_boost_field_buff_raises_player_xp_gain() {
    let mut game = Game::new(361, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    game.world.entity_mut(player).insert(FieldBuff {
        active: vec![ActiveFieldBuff {
            kind: FieldBuffKind::XpBoost,
            name: "Test XP Boost".to_string(),
            power: 50,
            remaining: 10,
            interval: 1,
            source: BuffSource::Routine,
        }],
    });

    game.award_player_xp(player, 10);

    assert_eq!(
        game.world.get::<Experience>(player).unwrap().xp,
        15,
        "a 50% XpBoost should turn a 10 XP award into 15"
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
        .find(|s| s.growth_multiplier == crate::tuning::BASELINE_GROWTH_MULTIPLIER)
        .expect("base roster should have at least one baseline-growth species")
        .id
        .clone();
    let boosted_id = species
        .iter()
        .find(|s| s.growth_multiplier > crate::tuning::BASELINE_GROWTH_MULTIPLIER)
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
        .any(|e| e.kind == MessageKind::LevelUp && e.text.contains("reach level"));
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
/// `crate::tuning::CREATURE_MAX_LEVEL` — one big XP award should push
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
        player_level > crate::tuning::CREATURE_MAX_LEVEL,
        "the player should keep leveling past the creature ceiling, got {player_level}"
    );
    assert_eq!(
        companion_level,
        crate::tuning::CREATURE_MAX_LEVEL,
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
    // Deep, so the per-group ceiling this fixture lives under is 46 rather
    // than 1.
    let (x, y) = (spawn.x + 500, spawn.y);
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
