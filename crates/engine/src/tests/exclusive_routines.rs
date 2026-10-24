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
use crate::components::{ActiveFieldBuff, BuffSource, FieldBuff, FieldBuffKind, Routines, Stats};
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
        .filter_map(|node| node.teaches.as_deref())
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

/// Every exclusive routine names at least one real boss, takes apart into
/// parts the boss can pay, and the chances are probabilities.
///
/// The declaration half. `killing_a_boss_can_drop_its_signature_parts`
/// below is the half that actually runs `award_loot`.
#[test]
fn a_boss_drops_the_parts_its_abilities_claim() {
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

    // And what the boss pays is the routine's parts, never a disk.
    for (ability, _) in &claims {
        let def = game.world.resource::<AbilityDb>().get(ability).unwrap();
        assert!(
            crate::routine_craft::decompose(def).is_some_and(|parts| !parts.is_empty()),
            "{ability} takes apart into nothing, so its boss has nothing to drop"
        );
    }
}

/// The other half: `award_loot` actually pays the parts out.
///
/// The 0.35 chance is forced to certainty with a `DropBoost` field buff
/// rather than by repeating the kill until it lands — `equipment_drops_for`
/// multiplies the whole table by the boost and then clamps at 1.0, so a
/// large enough boost makes every declared drop certain. That keeps this a
/// test of the wiring rather than of the RNG, which is what the declaration
/// half above already can't reach.
#[test]
fn killing_a_boss_can_drop_its_signature_parts() {
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
    let parts = {
        let def = game.world.resource::<AbilityDb>().get(&wanted).unwrap();
        crate::routine_craft::decompose(def).unwrap()
    };
    assert!(
        parts.iter().all(|p| game.parts_held(p) == 0),
        "the parts were in cargo before the boss even died"
    );

    game.award_loot(corpse, 0.0);

    for (part, count) in crate::routine_craft::parts_used(&parts) {
        assert!(
            game.parts_held(&part) >= count,
            "killing {boss} paid no {} for {wanted}, so the boss door is shut",
            part.label()
        );
    }
    assert!(
        !game.knows_routine(&wanted),
        "a boss drop taught the routine as well as dropping it"
    );
}

/// Extraction is the one way an installed exclusive routine can be moved,
/// and it hands back its *parts*, never the disk or the knowledge.
#[test]
fn extracting_an_exclusive_routine_returns_its_parts_and_teaches_nothing() {
    let mut game = Game::new(9005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let id = exclusive_ids(&game)[0].clone();
    build_extraction_bench(&mut game);

    let pet = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, pet, 4);
    install_routine_for_test(&mut game, pet, &id);

    let index = game
        .extractable_routines(pet)
        .into_iter()
        .position(|row| row.ability == id)
        .expect("the routine is installed, so it is extractable");
    let parts = game.parts_of_routine(&id);
    game.extract_routine(pet, index).unwrap();

    for (part, count) in crate::routine_craft::parts_used(&parts) {
        assert!(game.parts_held(&part) >= count, "{}", part.label());
    }
    assert!(
        !game.knows_routine(&id),
        "extraction taught an exclusive routine — the duplication hole is open"
    );
    assert!(
        game.owned_pets().iter().all(|p| p.entity != pet),
        "the program survived an extraction"
    );
}

/// An ordinary routine takes apart the same way: parts.
#[test]
fn extracting_an_ordinary_routine_yields_parts() {
    let (mut game, medic) = game_with_two_ability_companion();
    build_extraction_bench(&mut game);
    let row = game.extractable_routines(medic).into_iter().next().unwrap();
    let ability = row.ability.clone();
    assert!(
        !game
            .world
            .resource::<AbilityDb>()
            .get(&ability)
            .is_some_and(|def| def.exclusive),
        "this fixture's kit went exclusive; pick another"
    );

    let parts = game.parts_of_routine(&ability);
    game.extract_routine(medic, 0).unwrap();

    for (part, count) in crate::routine_craft::parts_used(&parts) {
        assert!(game.parts_held(&part) >= count, "{}", part.label());
    }
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

    install_routine_for_test(&mut game, player, &passive);
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
fn a_passive_never_appears_in_the_field_routine_list() {
    let mut game = Game::new(9007, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    set_level(&mut game, player, 40);
    let passive = passive_id(&game, PassiveTrigger::Afflicted);

    install_routine_for_test(&mut game, player, &passive);

    let listed: Vec<String> = game
        .field_routines()
        .into_iter()
        .map(|row| row.ability)
        .collect();
    assert!(
        !listed.contains(&passive),
        "{passive} is runnable from the map: {listed:?}"
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
    // **Summed over a fixed seed sweep**, because both the hostile swing that
    // drops the ally and the passive's own answering shot roll to hit now. A
    // single pair of rounds is a coin flip on both; the totals are
    // deterministic and still collapse to equal if the passive never fires.
    let mut killed_round = 0;
    let mut quiet_round = 0;
    let mut ally_actually_dropped = 0;
    for seed in 0..24u64 {
        let mut game = battle_with_a_passive_holder(9008, &passive);
        let doomed = game.owned_pets()[0].entity;
        game.world.get_mut::<Stats>(doomed).unwrap().hp = 1;
        reseed_rng(&mut game, seed);
        let enemy_hp_before = total_enemy_hp(&game);
        resolve_a_planned_round(&mut game);
        if game.creature_alive(doomed) {
            // The hostile missed this seed, so this round is not one where
            // the trigger could have fired — it belongs in neither total.
            continue;
        }
        ally_actually_dropped += 1;
        killed_round += enemy_hp_before - total_enemy_hp(&game);

        // Identical round on the identical stream, but the ally survives.
        let mut quiet = battle_with_a_passive_holder(9008, &passive);
        let survivor = quiet.owned_pets()[0].entity;
        reseed_rng(&mut quiet, seed);
        let enemy_hp_before = total_enemy_hp(&quiet);
        resolve_a_planned_round(&mut quiet);
        assert!(
            quiet.creature_alive(survivor),
            "the control round lost its ally too, so it controls for nothing"
        );
        quiet_round += enemy_hp_before - total_enemy_hp(&quiet);
    }
    assert!(
        ally_actually_dropped > 0,
        "the fixture's hostile routine never dropped the ally, so nothing triggered"
    );

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
    // The hostile's swing that drops the ally rolls to hit, so the stream is
    // chosen such that it connects; the claim is about the cooldown a fired
    // passive leaves behind, not about that swing's luck.
    let doomed_at = std::cell::Cell::new(None);
    let game = first_rng_seed_where(
        |seed| {
            let mut game = battle_with_a_passive_holder(9010, &passive);
            let doomed = game.owned_pets()[0].entity;
            doomed_at.set(Some(doomed));
            game.world.get_mut::<Stats>(doomed).unwrap().hp = 1;
            reseed_rng(&mut game, seed);
            resolve_a_planned_round(&mut game);
            game
        },
        |game| !game.creature_alive(doomed_at.get().unwrap()),
    );
    let player = game.player_entity();

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

// ---------------------------------------------------------------- helpers

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
    unlock_research_chain(game, "automation");
    assert!(
        game.can_extract_routines(),
        "the bench went up but extraction is still refused"
    );
}
