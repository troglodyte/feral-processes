//! What a fight pays out — loot, experience, and how it spreads across the party.

use super::support::*;
use crate::items::DownedProgram;
use crate::tuning::{
    PARTY_XP_DIVISOR, QUALITY_DROP_BASE, QUALITY_MAX, QUALITY_MIN, QUALITY_SPREAD, QUALITY_STEP,
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
                mitigation: 1,
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

// `teardown_adds_flat_salvage_to_a_kill_without_rerolling` lived here until
// program extraction retired the `work_resource` drop it measured —
// `Perk::Teardown`'s term now belongs in `Game::extraction_yield` (a later
// phase), and `perks.rs`'s own census
// (`every_perk_has_a_query_that_answers_what_it_is_worth`) is what still
// holds `salvage_bonus` to being worth something in the meantime.

// `award_loot_grants_the_species_work_resource` and
// `award_loot_grants_nothing_for_species_without_a_work_resource` lived here
// until program extraction retired the direct `work_resource` drop both
// measured.
// `a_kill_leaves_exactly_one_downed_program_carrying_species_level_and_rarity`
// (below) is what replaces the first; the second's premise — a species with
// a `work_resource` versus one without — no longer distinguishes any
// observable behaviour, since a kill grants neither species one directly
// any more.

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
    enlist(&mut game, in_party);
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
    enlist(&mut game, companion);

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
                    mitigation: 10,
                },
                Tamed { owner: player },
                PowerReserve::default(),
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
    enlist(&mut game, baseline);
    enlist(&mut game, boosted);

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
    enlist(&mut game, companion);

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
                mitigation: 0,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    // Asked of the engine rather than hardcoded: this test is about the
    // party's *share*, and pinning the kill's own value here would make it
    // fail every time the XP curve is retuned.
    let paid = game.kill_xp(wild);

    // Forced: there is no XP to share unless the strike lands the kill.
    force_the_next_attack_to_land(&mut game);
    player_attacks(&mut game);

    assert_eq!(
        game.world.get::<Experience>(companion).unwrap().xp,
        paid / PARTY_XP_DIVISOR,
        "a party member should gain half of whatever the kill paid ({paid})"
    );
}

/// The player has no level ceiling, while their party members stop at
/// `crate::tuning::TALENT_START_LEVEL` — one big XP award should push
/// the player past that ceiling and leave the companion pinned to it.
/// **One ceiling over the party.** This test used to say the opposite — the
/// player levelled forever and a companion stopped at the creature cap — and
/// it is the behaviour `Game::level_cap` replaced. What is left to pin is
/// that neither side has a ceiling of its own any more.
#[test]
fn the_player_and_a_companion_share_one_ceiling() {
    let mut game = Game::new(105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(3));
    let cap = game.level_cap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);

    // Party members earn half the player's award (PARTY_XP_DIVISOR),
    // so this is far past the cap for both of them.
    game.award_player_xp(player, 1_000_000);

    let player_level = game.world.get::<Experience>(player).unwrap().level;
    let companion_level = game.world.get::<Experience>(companion).unwrap().level;
    assert_eq!(
        player_level, cap,
        "the player is capped now, and at the zone's number"
    );
    assert_eq!(
        companion_level, player_level,
        "and a companion stops at exactly the same level, with no ring in sight"
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
    // Uniform, tiny HP, so every member is worth the same and the total
    // stays well under xp_for_level(1) — a level-up mid-fight would grow the
    // player's power and change what the later members pay.
    for &m in &members {
        let mut stats = game.world.get_mut::<Stats>(m).unwrap();
        stats.max_hp = 3;
        stats.hp = 3;
    }
    game.start_battle(members.clone());
    let per_kill = game.kill_xp(members[0]);
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
        per_kill * members.len() as u32,
        "every vanquished member should pay its own way"
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
///
/// It reads the ledger rather than counting it empty, because a compile
/// does write rows there now: a copy carries the quality it rolled, and
/// only one that came out exactly at spec is plain enough to stack.
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
        // A hand-compile's ticks burn Power and
        // `tuning::HAND_CRAFT_POWER_FLOOR` refuses a batch past the reserve,
        // so the reserve is restocked between them exactly as the pack is —
        // twenty in a row is enough to reach it at any multiplier — and the
        // compile is unwrapped rather than discarded so a refusal cannot
        // hide as a missing copy.
        fill_power(&mut game);
        game.craft(&recipe.result, 1, false).unwrap();
    }

    assert_eq!(
        held_any(&game, &recipe.result),
        20,
        "twenty compiles, twenty copies, wherever the quality axis put them"
    );
    let ledger = game.world.get::<GearCopies>(player).unwrap();
    assert!(
        ledger
            .copies
            .iter()
            .all(|(copy, _)| copy.rarity == Rarity::Ordinary && copy.affixes.is_empty()),
        "crafting must never produce a rare or affixed copy — the ledger rows \
         a compile writes are there for its quality alone"
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
            .any(|c| !c.affixes.is_empty() && c.rarity == Rarity::Ordinary),
        "an affix must be reachable without a rare tier: {copies:?}"
    );
    assert!(
        copies.iter().filter(|c| !c.affixes.is_empty()).count() > 1,
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
        rarity: Rarity::Gold,
        tier: 0,
        affixes: vec![affix.id.clone()],
        ..GearCopy::plain(weapon.clone())
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
            rarity: Rarity::Ordinary,
            tier: 0,
            affixes: vec![affix.id.clone()],
            ..GearCopy::plain(weapon.clone())
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

/// The direct resource grant `award_loot` made per kill before program
/// extraction retired it (`Game::leave_downed_program` replaced it — see
/// `docs/superpowers/specs/2026-09-04-program-extraction-design.md` section
/// 5). Kept here as a manual stand-in so the salvage-tally tests below can
/// still drive `record_drop`'s own merging, formatting and timing on a
/// real, guaranteed multi-kill drop — the same way
/// `a_rare_copy_is_tallied_apart_from_a_plain_one` already drives it
/// directly rather than through a fight, and for the same reason: no seed
/// reliably reproduces a specific quantity through the kill path any more,
/// and these tests were never about that quantity.
fn drop_a_resource(game: &mut Game, resource: &ItemId, qty: u32) {
    let landed = game.grant_loot(resource.clone(), qty, LootSource::Kill);
    game.record_drop(GearCopy::plain(resource.clone()), landed);
}

/// A pack that actually drops something, in one group, with stats that make
/// `finish_member` the whole of the fight.
///
/// Modelled on `support::battle_with_a_pack_of`, which takes the first
/// species in the db rather than one carrying a `work_resource` — a pack
/// that drops nothing has no salvage to consolidate. The zone and the
/// distance are that fixture's and are load-bearing for the same reason: a
/// group's size ceiling at a zone-1 spawn point is one member, so a pack
/// asked for here would silently arrive as a single program.
///
/// The species still carries a `work_resource` even though the kill path no
/// longer pays it automatically: callers that care about the drop use
/// `drop_a_resource` alongside their own `finish_member` calls, and this
/// keeps the returned `ItemId` meaningful for them to grant.
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
                        mitigation: 0,
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
        // Before `finish_member`, matching where `award_loot` used to grant
        // it — `finish_member`'s own last kill ends the battle, and a drop
        // granted after that point is "outside a battle" and gets its own,
        // separate immediate tally instead of joining this one.
        drop_a_resource(&mut game, &resource, 2);
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
    // 1 HP apiece keeps the fight's total comfortably under
    // `XP_PER_LEVEL_STEP`, which keeps the plain no-level wording under test
    // here rather than a level-up. The total itself is summed off the engine
    // as the kills happen, so the wording is what this pins, not the curve.
    let (_, members) = battle_with_a_dropping_pack(&mut game, 3, 1);
    let mut total = 0;
    for &m in &members {
        total += game.kill_xp(m);
        game.finish_member(0, 0, player);
    }

    let lines = log_texts(&game);
    assert_eq!(
        lines.iter().filter(|t| t.contains(" XP")).count(),
        1,
        "expected one XP line for the whole fight: {lines:#?}"
    );
    assert!(
        lines
            .iter()
            .any(|t| *t == format!("  You gain {total} XP.")),
        "expected the three kills summed into one line, indented under the \
         `Experience:` header: {lines:#?}"
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
    // Heavy enough that the first kill clears `xp_for_level(1)` on its own,
    // and light enough that it clears only the one level — the wording under
    // test here is a single announced level, not a jump.
    let (_, members) = battle_with_a_dropping_pack(&mut game, 3, 100);
    let mut total = game.kill_xp(members[0]);

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

    // Summed as they land: a level-up grows the player's power, so the later
    // members genuinely pay less than the first.
    for &m in &members[1..] {
        total += game.kill_xp(m);
        game.finish_member(0, 0, player);
    }
    let level = game.world.get::<Experience>(player).unwrap().level;
    let end = log_texts(&game);
    assert!(
        level > 2,
        "the fixture must reach a second level, or the wording below is the \
         single-level one already asserted above"
    );
    assert!(
        end.iter()
            .any(|t| *t == format!("  You gain {total} XP, reaching level {level}.")),
        "expected one XP line summing the fight: {end:#?}"
    );
    assert!(
        end.iter().any(|t| t.starts_with("    Max HP")),
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
    drop_a_resource(&mut game, &resource, 2);
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
///
/// A boss's gear drop is the guaranteed-drop premise this test needs since
/// program extraction retired the ordinary species' direct resource grant
/// (`pay_surface_boss_gear` rolls with replacement from a pool that falls
/// back to "the best gear there is" rather than ever landing empty).
#[test]
fn a_drop_outside_a_battle_is_announced_at_once() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let boss = a_boss(&game);
    let corpse = corpse_of(&mut game, &boss.id);

    game.award_loot(corpse);

    let lines = log_texts(&game);
    assert!(lines.iter().any(|t| *t == "Salvage:"), "{lines:#?}");
    assert!(
        lines
            .iter()
            .any(|t| t.starts_with("  ") && *t != "Salvage:"),
        "expected at least one drop row under the header, announced immediately \
         rather than deferred: {lines:#?}"
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
        rarity: Rarity::Gold,
        tier: 0,
        affixes: Vec::new(),
        ..GearCopy::plain(weapon.clone())
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

/// Overwrites a creature's whole stat block, so a `kill_xp` assertion rests
/// on a known power rather than on whichever species a fixture happened to
/// reach for.
fn set_stats(game: &mut Game, entity: Entity, max_hp: i32, atk: i32, def: i32) {
    let mut stats = game.world.get_mut::<Stats>(entity).unwrap();
    stats.max_hp = max_hp;
    stats.hp = max_hp;
    stats.atk = atk;
    stats.mitigation = def;
}

/// The wiring, not the formula: `progression::kill_xp`'s own tests cover the
/// curve, and this asserts that a real kill actually goes through it. It
/// would pass against the old flat `max_hp` award only if the challenge
/// factor were exactly 1, which `DIFFICULTY_EASY_MAX` puts well away from a
/// fresh player's ratio against a starter program.
#[test]
fn a_kill_pays_its_challenge_rather_than_the_victims_hp_bar() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let victim = spawn_wild_on_player_tile(&mut game);
    // A starter program's shape, set explicitly rather than taken from the
    // fixture's arbitrary first species: the assertion is about where this
    // sits against the player's power, so that has to be the known quantity.
    set_stats(&mut game, victim, 40, 4, 1);
    let bar = game.world.get::<Stats>(victim).unwrap().max_hp as u32;

    let earned = game.kill_xp(victim);

    assert!(
        earned < bar,
        "a starter program reads green against a fresh player, so it must pay \
         less than its {bar}-point bar, got {earned}"
    );
    assert!(earned > 0, "and the floor keeps it from paying nothing");
}

/// The point of the change: the same program is worth less to a party that
/// has outgrown it. Mutation-checked by flattening the factor to a constant,
/// which fails this while leaving the test above passing.
#[test]
fn the_same_program_pays_less_once_the_player_has_outgrown_it() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let victim = spawn_wild_on_player_tile(&mut game);
    // Deliberately heavy enough that neither reading lands on a clamp — two
    // floored values would compare equal and pass this vacuously.
    set_stats(&mut game, victim, 120, 10, 5);
    let fresh = game.kill_xp(victim);

    let player = game.player_entity();
    let mut stats = game.world.get_mut::<Stats>(player).unwrap();
    stats.max_hp *= 4;
    stats.hp = stats.max_hp;

    let grown = game.kill_xp(victim);
    assert!(
        grown < fresh,
        "a four-times stronger party should earn less from the same drone, \
         got {grown} against {fresh}"
    );
}

/// A Stack guardian's HP is already inflated by depth, so without the
/// ceiling it would earn a multiplier on top of an inflated bar — the double
/// count behind "four depth-3 fights were worth five levels".
#[test]
fn an_overwhelming_program_pays_no_more_than_the_ceiling() {
    let mut game = Game::new(43, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let victim = spawn_wild_on_player_tile(&mut game);
    set_stats(&mut game, victim, 2_000, 100, 100);
    let bar = game.world.get::<Stats>(victim).unwrap().max_hp as u32;

    let earned = game.kill_xp(victim);

    assert_eq!(
        earned,
        (bar as f64 * crate::tuning::XP_CHALLENGE_CEIL).round() as u32,
        "however far out of its depth, a kill pays its bar times the ceiling"
    );
}

/// The payout gate used to read `SpeciesDef::is_boss` directly, so a rolled
/// boss would have died underground paying nothing. Asserted on a species
/// that is deliberately **not** apex.
#[test]
fn a_rolled_boss_pays_the_stack_boss_cache() {
    let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ordinary = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");
    stand_in_the_stack(&mut game, 1);

    let wild = corpse_of(&mut game, &ordinary.id);
    game.world.entity_mut(wild).insert(Boss);
    game.award_loot(wild);

    let qty = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::PORTAL_FRAGMENT));
    assert!(
        STACK_BOSS_PORTAL_FRAGMENT_DROP.contains(&qty),
        "a rolled boss killed at depth 1 should pay a cache in \
         {STACK_BOSS_PORTAL_FRAGMENT_DROP:?}, got {qty}"
    );
}

#[test]
fn a_lair_guardian_drops_a_privilege_ring() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let boss = a_boss(&game);
    stand_in_the_stack(&mut game, 1);

    let wild = corpse_of(&mut game, &boss.id);
    game.award_loot(wild);

    assert!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::PRIVILEGE_RING))
            >= 1,
        "the only source of a companion's level ceiling is a lair guardian"
    );
}

/// The half that matters: the gate is `is_boss_creature` **and** underground,
/// and a test of the first half alone passes against a drop wired into the
/// wrong branch of `award_loot`.
#[test]
fn a_boss_defeated_on_the_surface_drops_no_privilege_ring() {
    let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
            .count(&ItemId::from(ids::PRIVILEGE_RING)),
        0,
        "a developed companion is bought by descending, not by clearing the surface"
    );
}

/// **The world does not make good gear; your base does.** A field drop
/// rolls off `QUALITY_DROP_BASE`, below the crafting floor Phase 3 will
/// add, so an average drop loses to an average craft — which is the whole
/// design intent the axis exists to express.
///
/// The band is asserted rather than a single sample because the roll is the
/// point: a drop that always landed on its floor would satisfy any bound
/// test and still be the flat 100 this replaces.
#[test]
fn a_dropped_weapon_rolls_its_quality_off_the_drop_floor() {
    let mut game = Game::new(4402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let weapon = game
        .item_defs()
        .into_iter()
        .find(|d| d.equipment.is_some())
        .expect("shipped assets include equippable gear")
        .id;

    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..200 {
        let copy = game.grant_gear_drop(weapon.clone(), Rarity::Ordinary);
        assert!(
            (QUALITY_DROP_BASE..=QUALITY_DROP_BASE + QUALITY_SPREAD).contains(&copy.quality),
            "a drop rolls its spread off the drop floor, got {}",
            copy.quality
        );
        assert_eq!(
            copy.quality % QUALITY_STEP,
            0,
            "the spread is drawn in steps, never drawn fine and rounded: {}",
            copy.quality
        );
        seen.insert(copy.quality);
    }
    assert!(
        seen.len() > 1,
        "every drop rolled {seen:?} — the spread is not being drawn"
    );
}

/// The clamp is the band and both of its ends are reachable, so it is
/// asserted at both. A floor above the ceiling is what Phase 3's developed
/// base produces (`QUALITY_BASE` + bench + perk + care already exceeds
/// `QUALITY_MAX`), and it must saturate rather than wrap a `u8`.
#[test]
fn the_quality_roll_clamps_at_both_ends_of_the_band() {
    let mut game = Game::new(4403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    for _ in 0..50 {
        assert_eq!(game.roll_quality(QUALITY_MAX), QUALITY_MAX);
        assert!(game.roll_quality(0) >= QUALITY_MIN);
    }
}

/// **A name is what lets two otherwise identical copies be told apart**,
/// which is the whole point of a fourth axis — five compiles of one blade
/// are five rows in the ledger and the player has to be able to pick the
/// good one.
///
/// A copy at spec shows **no** figure, the call `Rarity::label` makes for
/// `Ordinary`. Everything in every existing save is at `QUALITY_DEFAULT`,
/// so nothing already on screen gets wider.
#[test]
fn a_name_carries_the_quality_only_when_it_is_off_spec() {
    let game = Game::new(4404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let at_spec = GearCopy::plain(whip);
    let bare = game.copy_name(&at_spec);

    assert!(
        !bare.contains('%'),
        "a copy compiled to spec names no figure: {bare}"
    );

    let poor = GearCopy {
        quality: 85,
        ..at_spec.clone()
    };
    assert_eq!(game.copy_name(&poor), format!("{bare} (85%)"));

    // The figure goes last, after the rare tier's word and the affix's
    // decoration — one segment appended to a name already built, so the
    // three axes cannot come to fight over the order.
    let decorated = GearCopy {
        rarity: Rarity::Gold,
        quality: 130,
        ..at_spec
    };
    let name = game.copy_name(&decorated);
    assert!(name.ends_with(" (130%)"), "{name}");
    assert!(
        name.starts_with(Rarity::Gold.label().expect("Gold reads as a word")),
        "{name}"
    );
}

// ---------------------------------------------------------------------------
// The results screen's reading order: the final blows, the outcome, the
// salvage, the XP.
//
// `retain_outcomes_since_battle` runs when the player *leaves* the results
// screen rather than inside the round that ends the fight — see
// `Game::prune_battle_narration`. Pruned at `end_battle` the decisive
// round's blow-by-blow was deleted before a frontend had revealed a line of
// it, so the fight appeared to jump from the kill straight to the salvage.
// ---------------------------------------------------------------------------

/// A one-round fight against a single 1-HP program, resolved through the
/// real round loop so the narration actually exists. `finish_member` on its
/// own writes the payout without ever opening a round.
///
/// Killed through the real `award_loot` rather than `finish_member` called
/// directly, so there is no seam to slip a manual `drop_a_resource` call
/// into the way the `battle_with_a_dropping_pack` tests do — the `Boss`
/// insert is what guarantees this kill still has something to salvage now
/// that an ordinary kill's direct resource grant is gone:
/// `pay_surface_boss_gear` rolls with replacement from a pool that falls
/// back to "the best gear there is" rather than ever landing empty, and
/// bosshood affects no stat this hand-built 1-HP body reads.
fn win_a_fight_in_one_round(game: &mut Game) {
    let (_, members) = battle_with_a_dropping_pack(game, 1, 1);
    game.world.entity_mut(members[0]).insert(Boss);
    // `insert_battle` stands a `BattleState` up without going through
    // `begin_battle`, so the log has no battle mark and the prune would find
    // nothing to slice against — it would pass by doing nothing at all.
    game.world.resource_mut::<MessageLog>().open_battle();
    force_the_next_attack_to_land(game);
    player_attacks(game);
    assert!(
        game.world.get_resource::<BattleState>().is_none(),
        "the fixture should have finished the fight in one round"
    );
}

#[test]
fn the_decisive_rounds_narration_survives_the_fight() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    win_a_fight_in_one_round(&mut game);

    let log = game.battle_log();
    assert!(
        log.iter().any(|l| l.kind == MessageKind::PartyDamage),
        "the final round's blow-by-blow should still be readable once the \
         fight is over: {:#?}",
        log.iter().map(|l| (&l.text, l.kind)).collect::<Vec<_>>()
    );
}

#[test]
fn leaving_the_results_screen_prunes_the_narration() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    win_a_fight_in_one_round(&mut game);

    game.prune_battle_narration();

    let log = game.battle_log();
    assert!(
        !log.iter().any(|l| l.kind == MessageKind::PartyDamage),
        "the blow-by-blow should not follow the player onto the map: {:#?}",
        log.iter().map(|l| (&l.text, l.kind)).collect::<Vec<_>>()
    );
    assert!(
        log.iter().any(|l| l.kind == MessageKind::Loot),
        "the salvage tally should survive the prune: {:#?}",
        log.iter().map(|l| (&l.text, l.kind)).collect::<Vec<_>>()
    );
}

/// The win is the one ending that had no line of its own — a jack-out and a
/// flatline both announce themselves a line higher, at their own sites.
#[test]
fn a_won_fight_says_so_between_the_final_blow_and_the_salvage() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    win_a_fight_in_one_round(&mut game);

    let lines = log_texts(&game);
    let won = lines
        .iter()
        .position(|t| t == "You won!")
        .unwrap_or_else(|| panic!("a won fight never said so: {lines:#?}"));
    let kill = lines
        .iter()
        .position(|t| t.contains("crashes and deletes itself"))
        .expect("the kill line");
    let salvage = lines
        .iter()
        .position(|t| t == "Salvage:")
        .expect("the salvage tally");
    assert!(
        kill < won && won < salvage,
        "the headline should sit between the final blow and the salvage: {lines:#?}"
    );
}

/// A jack-out is not a win, and nothing may claim otherwise on the way out.
#[test]
fn a_jack_out_does_not_claim_a_win() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    start_battle_with_a_wild_program(&mut game);
    flee_until_clear(&mut game);

    let lines = log_texts(&game);
    assert!(
        !lines.iter().any(|t| t == "You won!"),
        "running away reported a win: {lines:#?}"
    );
}

/// The experience block is a header and indented rows, the shape salvage
/// already has.
#[test]
fn the_xp_block_is_headed_and_indented() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    win_a_fight_in_one_round(&mut game);

    let lines = log_texts(&game);
    let header = lines
        .iter()
        .position(|t| t == "Experience:")
        .unwrap_or_else(|| panic!("the XP block lost its header: {lines:#?}"));
    assert!(
        lines[header + 1].starts_with("  ") && lines[header + 1].contains("XP"),
        "the header should be followed by an indented XP row: {lines:#?}"
    );
}

/// ...and it is never a header over nothing. A fight the party jacked out of
/// before landing a kill pays no XP at all.
#[test]
fn a_fight_that_paid_no_xp_writes_no_experience_header() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    start_battle_with_a_wild_program(&mut game);
    flee_until_clear(&mut game);

    let lines = log_texts(&game);
    assert!(
        !lines.iter().any(|t| t == "Experience:"),
        "a fight that earned nothing still headed an empty block: {lines:#?}"
    );
}

/// A copy carries a list of affixes now, and a name has room for two words.
/// `copy_name` takes the **first prefix** and the **first suffix** in the
/// copy's own sorted order and appends `+N` for the rest — so what the
/// player reads is the two decorations they can act on plus an honest count
/// of what is not shown.
///
/// `+N` sits after the decoration and before the quality figure, which stays
/// last for the reason it already is: one segment appended to a name already
/// built, so the axes cannot come to fight over the order.
#[test]
fn a_multi_affix_copy_names_two_and_counts_the_rest() {
    let game = Game::new(4210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let defs = game.affix_defs();
    let prefix = defs
        .iter()
        .find(|a| a.prefix.is_some())
        .expect("the shipped set has a prefix affix");
    let suffix = defs
        .iter()
        .find(|a| a.suffix.is_some())
        .expect("the shipped set has a suffix affix");
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    let base = game.item_name(&weapon).to_string();

    // Two affixes, one of each shape: both named, and nothing counted.
    let two = GearCopy::with_affixes(
        weapon.clone(),
        Rarity::Ordinary,
        0,
        vec![prefix.id.clone(), suffix.id.clone()],
        crate::tuning::QUALITY_DEFAULT,
    );
    assert_eq!(
        game.copy_name(&two),
        format!(
            "{} {base} {}",
            prefix.prefix.as_ref().unwrap(),
            suffix.suffix.as_ref().unwrap()
        ),
        "one prefix and one suffix name both and count nothing"
    );

    // A third affix has nowhere to go in the name, so it is counted.
    let three = GearCopy::with_affixes(
        weapon.clone(),
        Rarity::Ordinary,
        0,
        vec![prefix.id.clone(), suffix.id.clone(), prefix.id.clone()],
        crate::tuning::QUALITY_DEFAULT,
    );
    assert_eq!(
        game.copy_name(&three),
        format!(
            "{} {base} {} +1",
            prefix.prefix.as_ref().unwrap(),
            suffix.suffix.as_ref().unwrap()
        ),
        "the third affix must be counted"
    );

    // And the count sits ahead of the quality figure, which stays last.
    let off_spec = GearCopy::with_affixes(
        weapon,
        Rarity::Ordinary,
        0,
        vec![prefix.id.clone(), suffix.id.clone(), prefix.id.clone()],
        85,
    );
    assert!(
        game.copy_name(&off_spec).ends_with(" +1 (85%)"),
        "{}",
        game.copy_name(&off_spec)
    );
}

// --- Downed programs ----------------------------------------------------
//
// A Forgiving death benches an owned program instead of destroying it.
// `Game::bench_or_dissolve` is the one door both death sites take, so these
// tests drive `end_battle` rather than the door directly: the point is that
// the *site* asks the door, not that the door works in isolation.

/// Stands a companion in a fight and kills it, returning the entity so the
/// caller can ask what became of it. The kill is a direct HP write rather
/// than a resolved swing — what is under test is teardown, and a fixture
/// that has to land a lethal blow first is a fixture about `resolve_attack`.
fn a_companion_killed_in_battle(game: &mut Game) -> Entity {
    let player = game.player_entity();
    let companion = spawn_tamed(game, 10, 3);
    enlist(game, companion);
    let enemy = spawn_wild_on_player_tile(game);
    insert_battle(game, player, vec![enemy]);
    game.world.get_mut::<Stats>(companion).unwrap().hp = 0;
    companion
}

#[test]
fn a_forgiving_death_benches_a_companion_rather_than_destroying_it() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = a_companion_killed_in_battle(&mut game);

    game.end_battle(player, None);

    let stats = game
        .world
        .get::<Stats>(companion)
        .expect("a Forgiving death must leave the program in the world");
    assert_eq!(stats.hp, 1, "a benched program is downed, not healthy");
    assert!(
        game.world
            .get::<crate::components::Downed>(companion)
            .is_some(),
        "the program should be marked Downed"
    );
    assert!(
        game.world.get::<Tamed>(companion).is_some(),
        "a benched program is still the player's"
    );
    assert!(
        !game.world.resource::<Party>().0.contains(&companion),
        "a benched program leaves the battle party"
    );
}

#[test]
fn a_permadeath_death_still_destroys_a_companion() {
    let mut game = Game::new(9, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = a_companion_killed_in_battle(&mut game);

    game.end_battle(player, None);

    assert!(
        game.world.get::<Stats>(companion).is_none(),
        "Permadeath is unchanged: the program is gone"
    );
}

#[test]
fn gear_comes_back_to_the_player_on_both_arms_of_a_death() {
    for mode in [DifficultyMode::Forgiving, DifficultyMode::Permadeath] {
        let mut game = Game::new(9, mode, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let weapon = game
            .world
            .resource::<ItemDb>()
            .all()
            .find(|d| d.equipment.is_some())
            .map(|d| d.id.clone())
            .expect("at least one equippable item");
        // Geared before the fight opens: `equip` refuses mid-battle, so a
        // fixture that wears its loadout after `insert_battle` is testing
        // that refusal rather than the teardown.
        let companion = spawn_tamed(&mut game, 10, 3);
        enlist(&mut game, companion);
        wear(&mut game, companion, &weapon.0);
        assert_eq!(
            held_any(&game, &weapon),
            0,
            "the copy should be worn, not carried, before the death"
        );
        let enemy = spawn_wild_on_player_tile(&mut game);
        insert_battle(&mut game, player, vec![enemy]);
        game.world.get_mut::<Stats>(companion).unwrap().hp = 0;

        game.end_battle(player, None);

        assert_eq!(
            held_any(&game, &weapon),
            1,
            "gear is the player's property on both arms: {mode:?}"
        );
    }
}

/// A real save and load, not a RON round trip. A field that is written but
/// never read back leaves a round-trip test green — this repo has shipped
/// that shape before.
#[test]
fn a_downed_program_loads_back_downed() {
    let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    a_companion_killed_in_battle(&mut game);
    game.end_battle(player, None);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_downed_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    // Re-fetched from `loaded`: `Entity` identity is private to the `World`
    // that allocated it.
    let mut query = loaded
        .world
        .query_filtered::<Entity, (With<Tamed>, With<crate::components::Downed>)>();
    assert_eq!(
        query.iter(&loaded.world).count(),
        1,
        "the benched program should load back benched"
    );
}

// ---------------------------------------------------------------------------
// Program extraction, phase 1: a kill leaves a downed program instead of
// paying its species' `work_resource` directly — see
// `docs/superpowers/specs/2026-09-04-program-extraction-design.md` section
// 5 and `Game::leave_downed_program`. `tests/extraction.rs` covers the
// condition roll's own formula in isolation; these exercise it wired up to
// a real kill.
// ---------------------------------------------------------------------------

#[test]
fn a_kill_leaves_exactly_one_downed_program_carrying_species_level_and_rarity() {
    let mut game = Game::new(910, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");

    let wild = corpse_of(&mut game, &species.id);
    game.world.entity_mut(wild).insert(Rarity::Gold);

    game.award_loot(wild);

    let held = &game.world.get::<DownedPrograms>(player).unwrap().0;
    assert_eq!(
        held.len(),
        1,
        "a kill should leave exactly one downed program: {held:?}"
    );
    let program = &held[0];
    assert_eq!(program.species, species.id, "the species must carry over");
    assert_eq!(
        program.level, 1,
        "a fresh player is level 1, and level has no other source on a wild Creature"
    );
    assert_eq!(program.rarity, Rarity::Gold, "the rarity must carry over");
    assert!(
        !program.boss,
        "an ordinary kill must not carry the boss flag"
    );
}

#[test]
fn a_boss_kills_program_is_at_or_above_both_floors() {
    let mut game = Game::new(911, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");

    let wild = corpse_of(&mut game, &species.id);
    game.world.entity_mut(wild).insert(Boss);
    // No `Rarity` component: `Ordinary` is the default, so a floor above
    // it — not a lucky roll — is what has to lift this program's rarity.

    game.award_loot(wild);

    let held = &game.world.get::<DownedPrograms>(player).unwrap().0;
    let program = &held[0];
    assert!(program.boss, "test premise: the kill must be a boss");
    assert!(
        program.condition >= crate::tuning::BOSS_CONDITION_FLOOR,
        "a boss's condition must be at or above BOSS_CONDITION_FLOOR: {}",
        program.condition
    );
    assert!(
        program.rarity >= crate::tuning::BOSS_RARITY_FLOOR,
        "a boss's rarity must be at or above BOSS_RARITY_FLOOR: {:?}",
        program.rarity
    );
}

#[test]
fn a_full_store_refuses_the_drop_logs_and_destroys_nothing() {
    let mut game = Game::new(912, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");

    let filler = DownedProgram {
        species: species.id.clone(),
        level: 1,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 50,
    };
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 =
        vec![filler.clone(); tuning::MAX_DOWNED_PROGRAMS];

    let wild = corpse_of(&mut game, &species.id);
    let granted = game.leave_downed_program(wild);

    assert!(!granted, "a full store must refuse the drop");
    let held = &game.world.get::<DownedPrograms>(player).unwrap().0;
    assert_eq!(
        held.len(),
        tuning::MAX_DOWNED_PROGRAMS,
        "a refusal must not grow the store"
    );
    assert!(
        held.iter().all(|p| *p == filler),
        "a refusal must destroy nothing already held: {held:?}"
    );

    let log = game.message_log(20);
    assert!(
        log.iter().any(|l| l.text.contains("No room")),
        "the refusal must be logged: {log:?}"
    );
}
