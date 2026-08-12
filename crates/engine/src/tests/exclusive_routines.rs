//! The exclusive pool: routines nobody can learn, and the two doors they
//! come through.
//!
//! The invariant everything here defends is that **an exclusive routine
//! never enters `KnownRoutines`**. Knowledge is the only thing in this game
//! that duplicates — you learn a routine once and etch blanks with it
//! forever — so every one of these tests is ultimately asking the same
//! question from a different direction: did anything leak one into the
//! known set?

use super::support::*;
use crate::abilities::{AbilityDb, PassiveTrigger};
use crate::components::{
    ActiveFieldBuff, BuffSource, FieldBuff, FieldBuffKind, Routines, Stats,
};
use crate::items::ItemId;
use crate::*;

/// Every exclusive routine the shipped assets declare. A census rather than
/// a hard-coded list, so adding a seventh puts it under all of these
/// automatically — and so a change that accidentally emptied the pool fails
/// loudly here instead of turning every test below into a vacuous pass over
/// an empty loop.
fn exclusive_ids(game: &Game) -> Vec<String> {
    let ids: Vec<String> = game
        .world
        .resource::<AbilityDb>()
        .exclusive_pool()
        .into_iter()
        .map(|def| def.id.clone())
        .collect();
    assert!(
        !ids.is_empty(),
        "the shipped exclusive pool is empty; every test in this file would pass vacuously"
    );
    ids
}

/// The whole gate, stated directly. If this passes and everything else in
/// the file fails, the pool is still sound; if this fails, nothing else
/// matters.
#[test]
fn nothing_a_new_game_ships_with_teaches_an_exclusive_routine() {
    let game = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let granted: std::collections::HashSet<&str> = game
        .world
        .resource::<crate::research::ResearchDb>()
        .all()
        .flat_map(|node| node.unlocks_abilities.iter())
        .map(|id| id.as_str())
        .chain(
            game.world
                .resource::<SpeciesDb>()
                .all()
                .flat_map(|s| s.abilities.iter())
                .map(|a| a.id.as_str()),
        )
        .collect();

    for id in exclusive_ids(&game) {
        assert!(
            !granted.contains(id.as_str()),
            "{id} is granted by a research node or a species kit, which would \
             teach it — an exclusive routine has no path into KnownRoutines"
        );
        assert!(
            !game.knows_routine(&id),
            "{id} is known from the very first turn"
        );
    }
}

/// The etch picker lists what a blank can be burnt with. An exclusive
/// routine appearing here would be a row that always refuses.
#[test]
fn an_exclusive_routine_never_reaches_the_etch_picker() {
    let mut game = Game::new(9002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ids = exclusive_ids(&game);

    // Force the failure mode this is really guarding: something reaching
    // past the gate and writing one into the known set anyway. The picker
    // must still refuse to offer it.
    for id in &ids {
        game.world
            .resource_mut::<crate::resources::KnownRoutines>()
            .0
            .insert(id.clone());
    }

    let offered: Vec<String> = game
        .etchable_routines()
        .into_iter()
        .map(|row| row.ability)
        .collect();
    for id in &ids {
        assert!(
            !offered.contains(id),
            "{id} is offered in the etch picker even though nothing may write one"
        );
    }
}

/// And the refusal says *why*. Without the explicit branch the
/// `knows_routine` check below it would already refuse every exclusive
/// routine — with "you don't know that routine", which is true and tells
/// the player nothing about why they never will.
#[test]
fn etching_an_exclusive_routine_is_refused_with_a_reason() {
    let mut game = Game::new(9003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let id = exclusive_ids(&game)[0].clone();
    give_disks(&mut game, 4);

    let err = game.etch_disk(&id).unwrap_err();
    assert!(
        err.contains("already etched"),
        "the refusal must explain that the disk comes pre-written: {err}"
    );
    assert!(
        !err.contains("don't know"),
        "the generic refusal leaked through instead of the specific one: {err}"
    );
    assert_eq!(game.blank_disks_held(), 4, "a refusal burnt a blank");

    // And it stays refused even if something has taught it, which is the
    // branch order this pins: the exclusive check runs first.
    game.world
        .resource_mut::<crate::resources::KnownRoutines>()
        .0
        .insert(id.clone());
    let err = game.etch_disk(&id).unwrap_err();
    assert!(
        err.contains("already etched"),
        "a taught exclusive routine became etchable: {err}"
    );
    assert_eq!(game.etched_disks_of(&id), 0, "and it produced a disk anyway");
}

/// Every exclusive routine names at least one real boss, that boss's claim
/// survives onto the synthesised disk, and the chances are probabilities.
///
/// The declaration half. `killing_a_boss_can_drop_its_signature_disk`
/// below is the half that actually runs `award_loot`.
#[test]
fn a_boss_drops_the_disks_its_abilities_claim() {
    let game = Game::new(9004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<AbilityDb>();

    let mut claims: Vec<(String, String)> = Vec::new();
    for def in db.exclusive_pool() {
        let sources = def
            .boss_drop
            .as_ref()
            .unwrap_or_else(|| panic!("{} names no boss to drop it", def.id));
        assert!(
            !sources.is_empty(),
            "{} declares an empty boss_drop, which is no door at all",
            def.id
        );
        for (species, chance) in sources {
            assert!(
                (0.0..=1.0).contains(chance),
                "{} drops off {species} at {chance}, which is not a probability",
                def.id
            );
            claims.push((def.id.clone(), species.clone()));
        }
    }

    // Every named species must actually be a boss. Nothing in the engine
    // requires it — `droppable` is species-agnostic — so this is the only
    // thing holding "exclusive routines come off bosses" as a fact rather
    // than an intention.
    let species_db = game.world.resource::<SpeciesDb>();
    for (ability, species) in &claims {
        let def = species_db
            .get(species)
            .unwrap_or_else(|| panic!("{ability} names {species}, which does not ship"));
        assert!(
            def.is_boss,
            "{ability} drops off {species}, which is not a boss"
        );
    }

    // And the synthesised disk carries the claim through to the item db,
    // which is what `equipment_drops_for` actually reads.
    for (ability, species) in &claims {
        let disk = ItemId::etched(ability);
        let item = game
            .world
            .resource::<ItemDb>()
            .get(disk.as_str())
            .unwrap_or_else(|| panic!("no etched disk was synthesised for {ability}"));
        let sources = item
            .droppable
            .as_ref()
            .unwrap_or_else(|| panic!("{}'s droppable did not carry across", disk));
        assert!(
            sources.iter().any(|(id, _)| id == species),
            "{disk} does not list {species} as a source"
        );
    }
}

/// The other half: `award_loot` actually pays the disk out.
///
/// The 0.35 chance is forced to certainty with a `DropBoost` field buff
/// rather than by repeating the kill until it lands — `equipment_drops_for`
/// multiplies the whole table by the boost and then clamps at 1.0, so a
/// large enough boost makes every declared drop certain. That keeps this a
/// test of the wiring rather than of the RNG, which is what the declaration
/// half above already can't reach.
#[test]
fn killing_a_boss_can_drop_its_signature_disk() {
    let mut game = Game::new(9012, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (boss, wanted) = {
        let db = game.world.resource::<AbilityDb>();
        let def = db
            .exclusive_pool()
            .into_iter()
            .find(|d| d.boss_drop.is_some())
            .expect("some exclusive routine names a boss");
        let (species, _) = def.boss_drop.as_ref().unwrap()[0].clone();
        (species, def.id.clone())
    };

    // A boost big enough that 0.35 clamps to certainty, so this asserts on
    // the drop table rather than on a roll.
    let player = game.player_entity();
    game.world.entity_mut(player).insert(FieldBuff {
        active: vec![ActiveFieldBuff {
            kind: FieldBuffKind::DropBoost,
            name: "test harness".to_string(),
            power: 900,
            remaining: 999,
            interval: 1,
            source: BuffSource::Routine,
        }],
    });

    let pos = *game.world.get::<Position>(player).unwrap();
    let corpse = game
        .spawn_wild_creature(&boss, pos.x + 1, pos.y)
        .expect("the boss species ships with the game");
    assert_eq!(
        game.etched_disks_of(&wanted),
        0,
        "the disk was in cargo before the boss even died"
    );

    game.award_loot(corpse);

    assert_eq!(
        game.etched_disks_of(&wanted),
        1,
        "killing {boss} paid no {wanted} disk, so the boss door is shut"
    );
    assert!(
        !game.knows_routine(&wanted),
        "a boss drop taught the routine as well as dropping it"
    );
}

/// Extraction is the one way an installed exclusive routine can be moved,
/// and it must hand back the *disk* rather than the knowledge — otherwise
/// buy one, install it, break the program down, and you can etch that
/// routine forever.
#[test]
fn extracting_an_exclusive_routine_returns_the_disk_and_teaches_nothing() {
    let mut game = Game::new(9005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let id = exclusive_ids(&game)[0].clone();
    build_extraction_bench(&mut game);

    let pet = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, pet, 4);
    give_etched_disks(&mut game, &id, 1);
    game.install_disk(pet, &id).unwrap();
    assert_eq!(game.etched_disks_of(&id), 0, "the install spent the disk");

    let index = game
        .extractable_routines(pet)
        .into_iter()
        .position(|row| row.ability == id)
        .expect("the routine is installed, so it is extractable");
    game.extract_routine(pet, index).unwrap();

    assert_eq!(
        game.etched_disks_of(&id),
        1,
        "breaking the program down must pry the disk back out"
    );
    assert!(
        !game.knows_routine(&id),
        "extraction taught an exclusive routine — the duplication hole is open"
    );
    assert!(
        game.owned_pets().iter().all(|p| p.entity != pet),
        "the program survived an extraction"
    );
}

/// The ordinary branch is untouched by the one above it: an ordinary
/// routine still teaches and still hands back no disk. Without this, the
/// exclusive branch could have swallowed both cases and every test of the
/// old behaviour would still pass through `install_routine_for_test`.
#[test]
fn extracting_an_ordinary_routine_still_teaches_and_yields_no_disk() {
    let (mut game, medic) = game_with_two_ability_companion();
    build_extraction_bench(&mut game);
    let row = game.extractable_routines(medic).into_iter().next().unwrap();
    let ability = row.ability.clone();
    assert!(
        !game.routine_is_exclusive(&ability),
        "this fixture's kit went exclusive; pick another"
    );

    game.extract_routine(medic, 0).unwrap();

    assert!(game.knows_routine(&ability), "the ordinary branch stopped teaching");
    assert_eq!(
        game.etched_disks_of(&ability),
        0,
        "the ordinary branch started handing out disks"
    );
}

/// A passive occupies a slot but is never a row anyone can pick. In the
/// Special menu it would be a choice that either does nothing or spends a
/// turn on what it was going to do free.
#[test]
fn a_passive_never_appears_in_the_special_menu() {
    let mut game = Game::new(9006, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    set_level(&mut game, player, 40);
    let passive = passive_id(&game, PassiveTrigger::AllyDropped);

    give_etched_disks(&mut game, &passive, 1);
    game.install_disk(player, &passive).unwrap();
    assert!(
        game.world
            .get::<Routines>(player)
            .unwrap()
            .0
            .contains(&passive),
        "the fixture failed to install the passive at all"
    );

    start_battle_with_a_wild_program(&mut game);
    let offered: Vec<String> = game
        .battle_special_options(0)
        .into_iter()
        .map(|row| row.name)
        .collect();
    let name = game.ability_display_name(&passive);
    assert!(
        !offered.contains(&name),
        "{name} is offered as a Special: {offered:?}"
    );
}

/// The other half: a passive is not a field routine either. `long_winter`
/// is field-only and *not* passive, so this also shows the two flags are
/// read independently rather than one standing in for the other.
#[test]
fn a_passive_never_appears_in_the_field_cast_list() {
    let mut game = Game::new(9007, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    set_level(&mut game, player, 40);
    let passive = passive_id(&game, PassiveTrigger::Afflicted);

    give_etched_disks(&mut game, &passive, 1);
    game.install_disk(player, &passive).unwrap();

    let listed: Vec<String> = game
        .field_routines()
        .into_iter()
        .map(|row| row.ability)
        .collect();
    assert!(
        !listed.contains(&passive),
        "{passive} is castable from the map: {listed:?}"
    );
}

/// `AllyDropped` fires, and fires because an ally actually dropped —
/// checked by running an identical round in which nobody dies and finding
/// the passive silent. Without that second half this would pass against a
/// passive that fired every single round.
#[test]
fn a_dropped_ally_sets_off_the_deadman_and_a_quiet_round_does_not() {
    let passive = {
        let probe = Game::new(9008, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        passive_id(&probe, PassiveTrigger::AllyDropped)
    };

    // The ally must die *inside* the round, not before it —
    // `battle_resolve_round` snapshots the living party before anyone acts,
    // so an ally already on the floor never "dropped". A hostile carrying a
    // whole-party routine is the deterministic way to do it: for a hostile,
    // `AllEnemies` resolves to `living_party()`, so it certainly connects
    // rather than rolling `roll_enemy_target`.
    let mut game = battle_with_a_passive_holder(9008, &passive);
    let doomed = game.owned_pets()[0].entity;
    game.world.get_mut::<Stats>(doomed).unwrap().hp = 1;
    let enemy_hp_before = total_enemy_hp(&game);
    resolve_a_planned_round(&mut game);
    assert!(
        !game.creature_alive(doomed),
        "the fixture's hostile routine failed to drop the ally, so nothing triggered"
    );
    let killed_round = enemy_hp_before - total_enemy_hp(&game);

    // Identical round, but the ally survives the same hit.
    let mut quiet = battle_with_a_passive_holder(9008, &passive);
    let survivor = quiet.owned_pets()[0].entity;
    let enemy_hp_before = total_enemy_hp(&quiet);
    resolve_a_planned_round(&mut quiet);
    assert!(
        quiet.creature_alive(survivor),
        "the control round lost its ally too, so it controls for nothing"
    );
    let quiet_round = enemy_hp_before - total_enemy_hp(&quiet);

    assert!(
        killed_round > quiet_round,
        "losing an ally landed no more damage than a quiet round \
         ({killed_round} vs {quiet_round}) — the passive did not fire"
    );
}

/// A passive honours its cooldown like anything else, which for
/// `AllyDropped` is what stops a party being wiped from firing it every
/// round of the wipe.
#[test]
fn a_fired_passive_goes_on_cooldown() {
    let passive = {
        let probe = Game::new(9009, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        passive_id(&probe, PassiveTrigger::AllyDropped)
    };
    let mut game = battle_with_a_passive_holder(9009, &passive);
    let player = game.player_entity();
    let doomed = game.owned_pets()[0].entity;

    game.world.get_mut::<Stats>(doomed).unwrap().hp = 1;
    resolve_a_planned_round(&mut game);
    assert!(
        !game.creature_alive(doomed),
        "the ally survived, so the passive was never triggered"
    );

    let remaining = game
        .world
        .get::<crate::components::AbilityCooldowns>(player)
        .and_then(|c| c.0.get(&passive).copied())
        .unwrap_or(0);
    assert!(
        remaining > 0,
        "the passive fired without arming its cooldown, so a wipe would set it off every round"
    );
}

/// Every derived disk resolves through the ordinary item lookups. A disk
/// that displayed as its raw id, or priced at the default, would be an
/// item the trade and cargo screens cannot describe.
#[test]
fn every_synthesised_disk_has_a_name_and_a_price() {
    let game = Game::new(9010, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let abilities: Vec<(String, String, bool)> = game
        .world
        .resource::<AbilityDb>()
        .all()
        .map(|def| (def.id.clone(), def.name.clone(), def.exclusive))
        .collect();

    for (id, name, exclusive) in abilities {
        let disk = ItemId::etched(&id);
        assert_eq!(
            disk.etched_ability(),
            Some(id.as_str()),
            "the id round trip does not close for {id}"
        );
        let shown = game.item_name(&disk);
        assert!(
            shown.contains(&name),
            "{disk} displays as {shown:?}, which does not name its routine"
        );
        let expected = if exclusive {
            crate::tuning::ETCHED_DISK_EXCLUSIVE_VALUE
        } else {
            crate::tuning::ETCHED_DISK_VALUE
        };
        assert_eq!(game.item_value(&disk), expected, "{disk} is priced wrong");
    }
}

/// A disk cannot be manufactured, cached, or worn — the three defaults that
/// keep an exclusive routine off every path except a boss and a trader.
/// Left as defaults in `synthesise_etched_disks`, which is exactly the kind
/// of load-bearing absence a test has to state out loud.
#[test]
fn no_etched_disk_is_craftable_cacheable_or_equipment() {
    let game = Game::new(9011, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let disks: Vec<&crate::items_db::ItemDef> = game
        .world
        .resource::<ItemDb>()
        .all()
        .filter(|def| def.id.etched_ability().is_some())
        .collect();
    assert!(!disks.is_empty(), "no etched disks were synthesised at all");

    for def in disks {
        assert!(
            def.craftable.is_none(),
            "{} can be crafted, which routes around both doors",
            def.id
        );
        assert!(
            def.cache_drop.is_none(),
            "{} can turn up in a Stack cache, which is a third door",
            def.id
        );
        assert!(
            def.equipment.is_none(),
            "{} is equipment, so surface_boss_loot would hand it out",
            def.id
        );
    }
}

// ---------------------------------------------------------------- helpers

/// The id of a shipped passive with `trigger`, or a panic naming what was
/// looked for — an assertion about a fixture, not about the code.
fn passive_id(game: &Game, trigger: PassiveTrigger) -> String {
    game.world
        .resource::<AbilityDb>()
        .all()
        .find(|def| def.triggers == Some(trigger))
        .unwrap_or_else(|| panic!("no shipped ability triggers on {trigger:?}"))
        .id
        .clone()
}

/// A battle with the player holding `passive` and one companion fielded
/// beside them — the smallest shape in which an ally can drop.
fn battle_with_a_passive_holder(seed: u32, passive: &str) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    set_level(&mut game, player, 40);
    give_etched_disks(&mut game, passive, 1);
    game.install_disk(player, passive).unwrap();

    let ally = spawn_tamed(&mut game, 30, 3);
    game.add_companion(ally).unwrap();

    // Enough Integrity on the hostile that a round cannot end the battle
    // before the passives are reached, and a whole-party routine so the
    // damage it deals lands on every party member rather than on a rolled
    // target — see the comment in the test that uses this.
    let wild = spawn_wild_on_player_tile(&mut game);
    {
        let mut stats = game.world.get_mut::<Stats>(wild).unwrap();
        stats.max_hp = 40_000;
        stats.hp = 40_000;
    }
    game.world
        .entity_mut(wild)
        .insert(Routines(vec![HOSTILE_SWEEP.to_string()]));
    game.start_battle(vec![wild]);
    game
}

/// A shipped `AllEnemies` damage routine, which on a hostile resolves to
/// the whole living party. Named here so the two passive tests share the
/// one fixture assumption rather than each picking their own.
const HOSTILE_SWEEP: &str = "bus_fault";

/// Plans every party slot to defend and resolves the round. Defending
/// rather than attacking so the only damage the enemy takes in the round is
/// the passive's — which is what makes the two-round comparison in
/// `a_dropped_ally_sets_off_the_deadman_and_a_quiet_round_does_not` mean
/// what it says.
fn resolve_a_planned_round(game: &mut Game) {
    let slots = game
        .world
        .get_resource::<BattleState>()
        .map(|b| b.planned.len())
        .unwrap_or(0);
    for slot in 0..slots {
        // Unwrapped rather than ignored: an unplanned slot leaves
        // `battle_round_ready` false, `battle_resolve_round` returns without
        // doing anything, and every assertion downstream compares two
        // untouched worlds and passes. Every member is alive at plan time in
        // both fixtures, so a failure here is the fixture breaking.
        game.battle_set_action(slot, BattleAction::Defend)
            .unwrap_or_else(|e| panic!("planning slot {slot}: {e}"));
    }
    game.battle_resolve_round();
}

fn total_enemy_hp(game: &Game) -> i32 {
    let Some(battle) = game.world.get_resource::<BattleState>() else {
        return 0;
    };
    battle
        .groups
        .iter()
        .flat_map(|g| g.members.iter())
        .filter_map(|&e| game.world.get::<Stats>(e))
        .map(|s| s.hp.max(0))
        .sum()
}

/// Stands whichever structure `Game::can_extract_routines` looks for, so an
/// extraction test is testing extraction rather than construction.
fn build_extraction_bench(game: &mut Game) {
    let bench = game
        .world
        .resource::<StructureDb>()
        .all()
        .find(|def| def.extracts_routines)
        .map(|def| def.id.clone())
        .expect("some shipped structure extracts routines");
    spawn_structure_at(game, &bench, 30, 30);
    assert!(
        game.can_extract_routines(),
        "the bench went up but extraction is still refused"
    );
}
