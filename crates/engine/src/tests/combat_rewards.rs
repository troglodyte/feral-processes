//! What a fight pays out — loot, experience, and how it spreads across the party.

use super::support::*;
use crate::tuning::{
    STACK_BOSS_PORTAL_FRAGMENT_DROP, SURFACE_BOSS_LOOT_DROPS, SURFACE_BOSS_LOOT_RARITY_FLOOR,
};
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

/// The battle log is where a player meets a dropped item for the first
/// time, and every screen that lists one — inventory, trade — puts its
/// category beside the name. A drop line that named it alone was the one
/// place the player had to already know what a "Hardened Shell" was.
#[test]
fn a_drop_line_tags_the_items_category() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = a_boss(&game);

    let wild = corpse_of(&mut game, &boss.id);
    game.award_loot(wild);

    // Every row of the salvage tally but its header — a surface boss pays
    // gear alongside its species' own rolls, so the equipment tags are in
    // here among whatever materials it also dropped.
    let drops: Vec<String> = game
        .message_log(40)
        .into_iter()
        .filter(|l| l.kind == MessageKind::Loot && l.text.starts_with("  "))
        .map(|l| l.text)
        .collect();
    assert!(
        !drops.is_empty(),
        "a surface boss should have spilled gear to tag, got: {:?}",
        game.message_log(40)
    );
    assert!(
        drops
            .iter()
            .any(|line| ["[WEP]", "[ARM]", "[MOD]"].iter().any(|t| line.contains(t))),
        "a gear drop should carry its equipment category tag, got: {drops:?}"
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

    // `held_any`, not `held`: a surface boss's gear carries
    // `SURFACE_BOSS_LOOT_RARITY_FLOOR`, so every copy lands in `GearCopies`
    // and counting the plain store alone would read as paying nothing.
    let before: u32 = band.iter().map(|id| held_any(&game, id)).sum();
    let wild = corpse_of(&mut game, &boss.id);
    game.award_loot(wild);
    let after: u32 = band.iter().map(|id| held_any(&game, id)).sum();

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
        .any(|e| e.kind == MessageKind::LevelUp && e.text.contains("reaching level"));
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

/// Over enough kills a dropped weapon comes up rare. Seeded, so this is a
/// fact about the build rather than a probability.
///
/// Deliberately asserts on `GearCopies` rather than on a log line: the store
/// a copy lands in *is* the mechanism (`GearCopy::is_plain` picks it), so a
/// rare drop that somehow landed in `Inventory` would read as ordinary
/// everywhere and this is the assertion that catches it.
#[test]
fn a_dropped_weapon_can_roll_a_rare_tier() {
    let mut game = Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let weapon = game
        .item_defs()
        .into_iter()
        .find(|d| d.equipment.is_some())
        .expect("shipped assets include equippable gear")
        .id;

    for _ in 0..2000 {
        game.grant_gear_drop(weapon.clone(), Rarity::Ordinary);
    }

    let special: Vec<GearCopy> = game
        .world
        .get::<GearCopies>(player)
        .expect("the player carries the special-copy store")
        .copies
        .iter()
        .map(|(copy, _)| copy.clone())
        .collect();
    assert!(
        special.iter().any(|c| c.rarity != Rarity::Ordinary),
        "2000 drops rolled no rare tier at all — is the roll wired up?"
    );
    // The store rule, not the rarity rule: a copy is here because
    // `is_plain` said no, and an *ordinary* copy carrying an affix is a
    // perfectly good reason for that. Asserting "everything here is rare"
    // was right while rarity was the only special property and became wrong
    // the moment affixes landed — which is why this now asks the predicate.
    assert!(
        special.iter().all(|c| !c.is_plain()),
        "a plain copy must live in Inventory, not the special store: {special:?}"
    );
}

/// **The floor is what makes a surface boss worth fighting**, so this pins
/// that no boss drop is ever ordinary — see
/// `SURFACE_BOSS_LOOT_RARITY_FLOOR`. Nothing else in the game guarantees a
/// tier.
#[test]
fn a_surface_boss_never_drops_an_ordinary_copy() {
    let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = a_boss(&game).id;
    let boss = corpse_of(&mut game, &species);

    game.award_loot(boss);

    // Counted against the floor rather than asserting that *nothing*
    // ordinary arrived: `award_loot` also rolls the boss species' own
    // `equipment_drop` table, and that one is an ordinary drop like any
    // other. Only `pay_surface_boss_gear` carries the floor.
    let floored: u32 = game
        .world
        .get::<GearCopies>(player)
        .map(|g| {
            g.copies
                .iter()
                .filter(|(copy, _)| copy.rarity >= SURFACE_BOSS_LOOT_RARITY_FLOOR)
                .map(|(_, qty)| *qty)
                .sum()
        })
        .unwrap_or(0);
    assert!(
        floored >= SURFACE_BOSS_LOOT_DROPS,
        "a boss owes {SURFACE_BOSS_LOOT_DROPS} copies at or above \
         {SURFACE_BOSS_LOOT_RARITY_FLOOR:?}, got {floored}"
    );
}

/// Materials take `grant_gear_drop`'s early return, and that return must
/// spend **no** `GameRng` draw. Every kill in the game drops a work
/// resource, so a draw here would shift the shared stream on essentially
/// every fight — the failure mode `roll_rarity`'s doc describes, where a
/// seeded combat test three files away quietly changes its answer.
#[test]
fn a_material_drop_spends_no_rarity_roll() {
    let material = {
        let probe = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        probe
            .item_defs()
            .into_iter()
            .find(|d| d.equipment.is_none())
            .expect("shipped assets include non-equippable items")
            .id
    };
    assert!(
        rng_unadvanced_by(9, |g| {
            g.grant_gear_drop(material.clone(), Rarity::Ordinary);
        }),
        "dropping a material must not consume a rarity roll"
    );
}

/// The absence that makes the whole feature worth having: gear you *make*
/// is never rare, so gear you *find* is categorically better. An omission
/// is invisible without a test naming it — the same reason
/// `an_arena_fight_writes_no_save` exists.
#[test]
fn crafted_gear_is_never_rare() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let recipe = game
        .craft_recipes()
        .into_iter()
        .find(|r| game.equipment_of(&r.result).is_some())
        .expect("some equippable is craftable with no bench");

    for (item, qty) in &recipe.cost {
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(item.clone(), qty * 20);
    }
    for _ in 0..20 {
        let _ = game.craft(&recipe.result, 1);
    }

    assert!(
        game.count_copies(&GearCopy::plain(recipe.result.clone())) > 0,
        "the crafted copies should be in the plain store"
    );
    assert_eq!(
        game.world
            .get::<GearCopies>(player)
            .map(|g| g.total())
            .unwrap_or(0),
        0,
        "crafting must never produce a rare copy"
    );
}

/// Affixes are rolled independently of the rare tier, so most affixed
/// drops are *ordinary* copies — which is the point: rarity is the chase
/// and affixes are the variety, and gating one behind the other would leave
/// the 96.5% of drops that roll no tier exactly as featureless as before.
#[test]
fn a_dropped_weapon_can_roll_an_affix_without_a_rare_tier() {
    let mut game = Game::new(515, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let weapon = game
        .item_defs()
        .into_iter()
        .find(|d| d.equipment.is_some())
        .expect("shipped assets include equippable gear")
        .id;

    for _ in 0..400 {
        game.grant_gear_drop(weapon.clone(), Rarity::Ordinary);
    }

    let copies: Vec<GearCopy> = game
        .world
        .get::<GearCopies>(player)
        .expect("the player carries the special-copy store")
        .copies
        .iter()
        .map(|(copy, _)| copy.clone())
        .collect();
    assert!(
        copies
            .iter()
            .any(|c| c.affix.is_some() && c.rarity == Rarity::Ordinary),
        "an affix must be reachable without a rare tier: {copies:?}"
    );
    assert!(
        copies.iter().filter(|c| c.affix.is_some()).count() > 1,
        "400 drops at GEAR_AFFIX_CHANCE should yield several affixes"
    );
}

/// Every affix the copy names must resolve, and the generated name must
/// actually contain the affix's word — a name that silently dropped it
/// would leave the player a stat bonus with no visible source, which is the
/// exact fault `AffixDef::fault` refuses a file for.
#[test]
fn an_affixed_copys_name_carries_both_its_word_and_its_tier() {
    let game = Game::new(516, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    let slot = game.equipment_of(&weapon).expect("it is equippable").0;
    let affix = game
        .affix_defs()
        .into_iter()
        .find(|d| d.fits(slot))
        .expect("the shipped set has a weapon-eligible affix");

    let plain = GearCopy::plain(weapon.clone());
    let dressed = GearCopy {
        item: weapon.clone(),
        rarity: Rarity::Gold,
        tier: 0,
        affix: Some(affix.id.clone()),
    };

    let bare = game.copy_name(&plain);
    let full = game.copy_name(&dressed);
    let word = affix
        .prefix
        .clone()
        .or_else(|| affix.suffix.clone())
        .expect("a loaded affix has one or the other");

    assert_eq!(
        bare,
        game.item_name(&weapon),
        "a plain copy is just its name"
    );
    assert!(
        full.contains(&word),
        "{full:?} lost the affix word {word:?}"
    );
    assert!(
        full.contains(Rarity::Gold.label().unwrap()),
        "{full:?} lost the rare tier"
    );
    assert!(
        full.contains(game.item_name(&weapon)),
        "{full:?} lost the item's own name"
    );
}

/// An affix is worth stats, not just a name, and it is added to the base
/// *before* scaling — so it grows with gear level and both tiers rather
/// than dwindling into irrelevance over a run.
#[test]
fn an_affix_is_worth_more_than_its_name() {
    let mut game = Game::new(517, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    let slot = game.equipment_of(&weapon).expect("it is equippable").0;
    let affix = game
        .affix_defs()
        .into_iter()
        .find(|d| d.fits(slot) && d.stats.atk > 0)
        .expect("the shipped set has a weapon affix granting ATK");

    let worn_atk = |game: &mut Game, copy: GearCopy| {
        game.add_copies(&copy, 1);
        game.equip(player, &copy).unwrap();
        let atk = game.world.get::<Stats>(player).unwrap().atk;
        game.unequip(player, EquipmentSlot::Weapon).unwrap();
        game.take_copies(&copy, 1);
        atk
    };

    let plain = worn_atk(&mut game, GearCopy::plain(weapon.clone()));
    let affixed = worn_atk(
        &mut game,
        GearCopy {
            item: weapon.clone(),
            rarity: Rarity::Ordinary,
            tier: 0,
            affix: Some(affix.id.clone()),
        },
    );
    assert!(
        affixed > plain,
        "an affix granting +{} ATK changed nothing worn ({plain} -> {affixed})",
        affix.stats.atk
    );

    // And unequipping it leaves nothing behind — the same symmetry rarity
    // has, reached by a third property. `worn_atk` unequips before
    // returning, so by here the player is back to base either way; if the
    // affix leaked into `Stats`, the two passes above would have started
    // from different footings and this would not hold.
    let bare_again = worn_atk(&mut game, GearCopy::plain(weapon.clone()));
    assert_eq!(
        bare_again, plain,
        "wearing and removing an affixed copy shifted the player's base ATK"
    );
}

// ---------------------------------------------------------------------------
// Consolidating a fight's payout into its closing lines.
//
// The awards themselves still land per kill — a level-up full-heals inside
// `progression::add_xp` and the killing blow is usually the level, so moving
// *when* they are granted would move fight outcomes. Only the announcement
// waits.
// ---------------------------------------------------------------------------

/// A pack that actually drops something, in one group, with stats that make
/// `finish_member` the whole of the fight.
///
/// Modelled on `support::battle_with_a_pack_of`, which takes the first
/// species in the db rather than one carrying a `work_resource` — a pack
/// that drops nothing has no salvage to consolidate. The zone and the
/// distance are that fixture's and are load-bearing for the same reason: a
/// group's size ceiling at a zone-1 spawn point is one member, so a pack
/// asked for here would silently arrive as a single program.
fn battle_with_a_dropping_pack(game: &mut Game, count: usize, hp: i32) -> (ItemId, Vec<Entity>) {
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| s.work_resource.is_some() && !s.is_boss)
        .expect("at least one non-boss species should carry a work_resource");
    let resource = species.work_resource.clone().unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let members: Vec<Entity> = (0..count)
        .map(|i| {
            game.world
                .spawn((
                    Creature {
                        species: species.id.clone(),
                    },
                    Hostile,
                    Position {
                        x: spawn.x + 500 + i as i32,
                        y: spawn.y,
                    },
                    Stats {
                        hp,
                        max_hp: hp,
                        atk: 0,
                        def: 0,
                    },
                    StatusEffects::default(),
                ))
                .id()
        })
        .collect();
    insert_battle(game, player, members.clone());
    assert_eq!(
        game.world.resource::<BattleState>().groups[0].members.len(),
        count,
        "the pack was capped on the way in, so this fight has fewer kills than it asks for"
    );
    (resource, members)
}

fn log_texts(game: &Game) -> Vec<String> {
    game.message_log(crate::MESSAGE_LOG_CAP)
        .into_iter()
        .map(|l| l.text)
        .collect()
}

/// Three kills, one salvage tally — and it carries the whole fight's haul,
/// not the last kill's.
#[test]
fn a_fight_reports_its_salvage_once() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let (resource, members) = battle_with_a_dropping_pack(&mut game, 3, 1);

    let before = held(&game, &resource);
    for _ in 0..members.len() {
        game.finish_member(0, 0, player);
    }
    let gained = held(&game, &resource) - before;
    assert!(gained >= 3, "three kills should each have paid a drop");

    let lines = log_texts(&game);
    assert_eq!(
        lines.iter().filter(|t| *t == "Salvage:").count(),
        1,
        "expected exactly one salvage tally: {lines:#?}"
    );
    let row = format!("  {gained} {}", game.item_name_tagged(&resource));
    assert!(
        lines.contains(&row),
        "expected the tally to merge all three drops into {row:?}: {lines:#?}"
    );
}

/// ...and none of it reaches the log while the fight is still running.
#[test]
fn nothing_is_salvaged_until_the_fight_ends() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    battle_with_a_dropping_pack(&mut game, 3, 1);

    game.finish_member(0, 0, player);
    assert!(
        game.world.get_resource::<BattleState>().is_some(),
        "the fixture should leave two programs standing"
    );
    let loot: Vec<String> = game
        .message_log(crate::MESSAGE_LOG_CAP)
        .into_iter()
        .filter(|l| l.kind == MessageKind::Loot)
        .map(|l| l.text)
        .collect();
    assert!(
        loot.is_empty(),
        "a kill announced its drop mid-fight: {loot:#?}"
    );
}

/// The player's XP is one line for the fight, carrying the sum.
#[test]
fn the_xp_line_carries_the_whole_fights_total() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // 1 HP apiece: a kill pays the victim's `max_hp` in XP, so this is three
    // XP for the fight — comfortably under `XP_PER_LEVEL_STEP`, which keeps
    // the plain no-level wording under test here rather than a level-up.
    let (_, members) = battle_with_a_dropping_pack(&mut game, 3, 1);
    for _ in 0..members.len() {
        game.finish_member(0, 0, player);
    }

    let lines = log_texts(&game);
    assert_eq!(
        lines.iter().filter(|t| t.contains(" XP")).count(),
        1,
        "expected one XP line for the whole fight: {lines:#?}"
    );
    assert!(
        lines.iter().any(|t| t == "You gain 3 XP."),
        "expected the three kills summed into one line: {lines:#?}"
    );
}

/// A level reached mid-fight is announced when it happens — the HP bar snaps
/// to full at that moment (`progression::add_xp` heals on a level), and a
/// player watching it needs the cause on screen then, not after the fight.
/// The XP total and the stat block still wait for the tally.
#[test]
fn reaching_a_level_is_announced_while_the_fight_runs() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // 30 HP apiece against `XP_PER_LEVEL_STEP` of 20: the first kill levels.
    battle_with_a_dropping_pack(&mut game, 3, 30);

    game.finish_member(0, 0, player);
    let mid = log_texts(&game);
    assert!(
        mid.iter().any(|t| t == "You reach level 2!"),
        "a mid-fight level went unannounced: {mid:#?}"
    );
    assert!(
        !mid.iter().any(|t| t.contains(" XP")),
        "the XP total should wait for the tally: {mid:#?}"
    );
    assert!(
        !mid.iter().any(|t| t.starts_with("  Max HP")),
        "the stat block should wait for the tally: {mid:#?}"
    );

    game.finish_member(0, 0, player);
    game.finish_member(0, 0, player);
    let end = log_texts(&game);
    assert!(
        end.iter().any(|t| t == "You gain 90 XP, reaching level 3."),
        "expected one XP line summing the fight: {end:#?}"
    );
    assert!(
        end.iter().any(|t| t.starts_with("  Max HP")),
        "expected the tally to carry the fight's stat block: {end:#?}"
    );
}

/// You keep what you killed before you ran. `end_battle` is the one teardown
/// for a win and a jack-out alike, so the tally is paid out either way.
#[test]
fn jacking_out_still_reports_what_was_killed() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let (resource, _) = battle_with_a_dropping_pack(&mut game, 3, 1);

    let before = held(&game, &resource);
    game.finish_member(0, 0, player);
    let gained = held(&game, &resource) - before;
    flee_until_clear(&mut game);

    let lines = log_texts(&game);
    assert!(
        lines.iter().any(|t| *t == "Salvage:"),
        "fleeing dropped the kill's payout on the floor: {lines:#?}"
    );
    let row = format!("  {gained} {}", game.item_name_tagged(&resource));
    assert!(
        lines.contains(&row),
        "expected {row:?} in the tally after a jack-out: {lines:#?}"
    );
}

/// With no battle to hold the tally, a drop is announced where it happens —
/// through the same formatter, so the two paths cannot come to word it
/// differently. Nothing in the game reaches `award_loot` outside a fight
/// today; this is what stops the next thing that does from paying silently.
#[test]
fn a_drop_outside_a_battle_is_announced_at_once() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| s.work_resource.is_some() && !s.is_boss)
        .expect("at least one non-boss species should carry a work_resource");
    let resource = species.work_resource.clone().unwrap();
    let corpse = corpse_of(&mut game, &species.id);

    let before = held(&game, &resource);
    game.award_loot(corpse);
    let gained = held(&game, &resource) - before;

    let lines = log_texts(&game);
    assert!(lines.iter().any(|t| *t == "Salvage:"), "{lines:#?}");
    assert!(
        lines.contains(&format!("  {gained} {}", game.item_name_tagged(&resource))),
        "{lines:#?}"
    );
}

/// Two copies that differ get a row each, and two of the same get one row
/// carrying both.
///
/// Driven through `record_drop` rather than through a fight, because no seed
/// reliably drops one plain and one Gold copy of the same weapon —
/// and the merge key is the whole point: keyed on the item alone, an
/// Gold copy would be tallied as another ordinary one and the row
/// colour a player reads the tier off would be a lie.
#[test]
fn a_rare_copy_is_tallied_apart_from_a_plain_one() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    battle_with_a_dropping_pack(&mut game, 1, 1);
    let weapon = game
        .world
        .resource::<ItemDb>()
        .all()
        .find(|d| d.equipment.is_some())
        .map(|d| d.id.clone())
        .expect("at least one equippable item");
    let plain = GearCopy::plain(weapon.clone());
    let rare = GearCopy {
        item: weapon.clone(),
        rarity: Rarity::Gold,
        tier: 0,
        affix: None,
    };

    game.record_drop(plain.clone(), 1);
    game.record_drop(rare.clone(), 1);
    game.record_drop(plain.clone(), 2);
    game.end_battle(player, None);

    let lines = log_texts(&game);
    let plain_row = format!("  3 {}", game.drop_label(&plain));
    let rare_row = format!("  1 {}", game.drop_label(&rare));
    assert!(
        lines.contains(&plain_row),
        "expected the two plain drops merged into {plain_row:?}: {lines:#?}"
    );
    assert!(
        lines.contains(&rare_row),
        "expected the rare copy on its own row as {rare_row:?}: {lines:#?}"
    );
}
