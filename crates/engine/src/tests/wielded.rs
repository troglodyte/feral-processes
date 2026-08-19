//! Wielding a tamed program as your weapon — the live-computed passive
//! bonus, the wield/unwield doors, and the routine proc.

use super::support::*;
use crate::tuning::WIELDED_PROGRAM_STAT_DIVISOR;
use crate::*;

/// Sets the resource straight, for the tests that predate `wield_program`
/// or deliberately want to bypass its refusals.
fn wield_directly(game: &mut Game, entity: Entity) {
    game.world.insert_resource(WieldedProgram(Some(entity)));
}

#[test]
fn wielding_a_program_raises_the_players_attack_and_defense() {
    let mut game = Game::new(9100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let (atk_before, def_before) = (
        game.effective_atk(player),
        game.effective_mitigation(player),
    );

    let program = spawn_tamed(&mut game, 40, 60);
    game.world.get_mut::<Stats>(program).unwrap().mitigation = 30;
    wield_directly(&mut game, program);

    assert_eq!(
        game.effective_atk(player) - atk_before,
        60 / WIELDED_PROGRAM_STAT_DIVISOR,
        "the wielded program lends a share of its ATK"
    );
    assert_eq!(
        game.effective_mitigation(player) - def_before,
        30 / WIELDED_PROGRAM_STAT_DIVISOR,
        "and a share of its DEF"
    );
}

#[test]
fn the_wielded_bonus_floors_at_one_per_stat() {
    let mut game = Game::new(9101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 1);
    game.world.get_mut::<Stats>(program).unwrap().mitigation = 1;
    wield_directly(&mut game, program);

    assert_eq!(
        game.wielded_stat_bonus(),
        (1, 1),
        "a weak program is still worth something in the hand"
    );
}

#[test]
fn the_wielded_bonus_tracks_the_programs_current_stats() {
    let mut game = Game::new(9102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 20);
    wield_directly(&mut game, program);
    let before = game.wielded_stat_bonus().0;

    game.world.get_mut::<Stats>(program).unwrap().atk = 200;

    assert!(
        game.wielded_stat_bonus().0 > before,
        "the bonus is computed live, not captured at wield time"
    );
}

#[test]
fn a_despawned_wielded_program_lends_nothing() {
    let mut game = Game::new(9103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let program = spawn_tamed(&mut game, 40, 60);
    wield_directly(&mut game, program);
    let armed = game.effective_atk(player);

    game.world.despawn(program);

    assert_eq!(game.wielded_program(), None);
    assert_eq!(game.wielded_stat_bonus(), (0, 0));
    assert!(
        game.effective_atk(player) < armed,
        "the bonus goes with the program, with nothing to clear"
    );
}

#[test]
fn wielding_a_party_member_stands_it_down() {
    let mut game = Game::new(9104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 20);
    game.add_companion(program).unwrap();

    game.wield_program(program).unwrap();

    assert!(
        !game.world.resource::<Party>().0.contains(&program),
        "a weapon is not a combatant"
    );
    assert_eq!(game.wielded_program(), Some(program));
}

#[test]
fn adding_a_wielded_program_to_the_party_unwields_it() {
    let mut game = Game::new(9105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 20);
    game.wield_program(program).unwrap();

    game.add_companion(program).unwrap();

    assert_eq!(game.wielded_program(), None, "the other door");
    assert!(game.world.resource::<Party>().0.contains(&program));
}

#[test]
fn wielding_returns_the_worn_weapon_and_removes_its_bonus() {
    let mut game = Game::new(9106, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let bare_atk = game.world.get::<Stats>(player).unwrap().atk;
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    game.equip(
        game.player_entity(),
        &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0),
    )
    .unwrap();
    assert!(game.world.get::<Stats>(player).unwrap().atk > bare_atk);

    let program = spawn_tamed(&mut game, 40, 20);
    game.wield_program(program).unwrap();

    assert_eq!(
        game.player_status().weapon,
        None,
        "the slot holds an item or a program, never both"
    );
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().atk,
        bare_atk,
        "the item's delta comes off; the program's bonus never touches Stats"
    );
    assert!(
        game.player_status()
            .inventory
            .iter()
            .any(|r| r.copy.item == ItemId::from(ids::OVERCLOCK_CORE) && r.qty == 1),
        "the displaced item goes back to cargo rather than being destroyed"
    );
}

#[test]
fn a_wield_refused_in_battle_changes_nothing() {
    let mut game = Game::new(9107, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    game.equip(
        game.player_entity(),
        &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0),
    )
    .unwrap();
    let program = spawn_tamed(&mut game, 40, 20);
    game.add_companion(program).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);

    assert!(game.wield_program(program).is_err());

    assert!(
        game.world.resource::<Party>().0.contains(&program),
        "the refusal resolves before anything moves"
    );
    assert!(game.player_status().weapon.is_some());
    assert_eq!(game.wielded_program(), None);
}

/// The narrow refusal the wield ordering exists for: `unequip` is the last
/// step that can still fail, and it fails on a worn item whose `ItemDef` has
/// gone (a mod removed it between sessions). Standing the program down first
/// would empty a party slot for a wield that never happened.
#[test]
fn a_wield_refused_by_an_unequippable_weapon_leaves_the_party_alone() {
    let mut game = Game::new(9123, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Equipment>(player)
        .unwrap()
        .weapon
        .replace(EquippedItem {
            copy: gear(
                &ItemId::from("an_item_this_asset_set_has_never_heard_of"),
                0,
            ),
            level: 1,
        });
    let program = spawn_tamed(&mut game, 40, 20);
    game.add_companion(program).unwrap();

    assert!(game.wield_program(program).is_err());

    assert!(
        game.world.resource::<Party>().0.contains(&program),
        "the party slot survives a wield that could not complete"
    );
    assert_eq!(game.wielded_program(), None);
}

#[test]
fn wielding_costs_one_turn_whether_or_not_it_displaces_a_weapon() {
    let mut bare = Game::new(9108, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut bare, 40, 20);
    let before = bare.world.resource::<GameClock>().tick;
    bare.wield_program(program).unwrap();
    let bare_cost = bare.world.resource::<GameClock>().tick - before;

    let mut armed = Game::new(9108, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = armed.player_entity();
    armed
        .world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    armed
        .equip(
            armed.player_entity(),
            &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0),
        )
        .unwrap();
    let program = spawn_tamed(&mut armed, 40, 20);
    let before = armed.world.resource::<GameClock>().tick;
    armed.wield_program(program).unwrap();
    let armed_cost = armed.world.resource::<GameClock>().tick - before;

    assert_eq!(bare_cost, 1);
    assert_eq!(
        armed_cost, bare_cost,
        "one player action is one tick, whether or not a weapon was displaced"
    );
}

#[test]
fn unwield_program_clears_the_bonus() {
    let mut game = Game::new(9109, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 60);
    game.wield_program(program).unwrap();
    assert_ne!(game.wielded_stat_bonus(), (0, 0));

    game.unwield_program().unwrap();

    assert_eq!(game.wielded_program(), None);
    assert_eq!(game.wielded_stat_bonus(), (0, 0));
    assert!(game.unwield_program().is_err(), "nothing to put down twice");
}

#[test]
fn selling_the_wielded_program_ends_the_wield() {
    let mut game = Game::new(9110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 60);
    game.wield_program(program).unwrap();

    game.dissolve_tamed_program(program);

    assert_eq!(game.wielded_program(), None);
    assert_eq!(
        game.wielded_stat_bonus(),
        (0, 0),
        "asserted without `dissolve_tamed_program` knowing this feature exists"
    );
}

#[test]
fn fusing_away_the_wielded_program_ends_the_wield() {
    let mut game = Game::new(9111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 60);
    let other = spawn_tamed(&mut game, 30, 30);
    game.wield_program(program).unwrap();

    game.fuse_companions(program, other, None).unwrap();

    assert_eq!(game.wielded_program(), None);
    assert_eq!(
        game.wielded_stat_bonus(),
        (0, 0),
        "the second destruction path is covered by the same omission"
    );
}

/// Installs exactly `ids` into `entity`'s routine slots, replacing whatever
/// `install_innate_routines` put there — the proc rolls uniformly over the
/// whole kit, so a test asserting *which* routine fired needs a kit of one.
fn install_only(game: &mut Game, entity: Entity, ids: &[&str]) {
    game.world
        .entity_mut(entity)
        .insert(Routines(ids.iter().map(|id| id.to_string()).collect()));
}

/// Resolves one round per `GameRng` seed until `found` reports a hit,
/// rebuilding the world each time so the run is identical apart from the
/// stream. Deterministic — the same seed always wins — and self-healing:
/// pinning one magic seed instead would break silently the next time
/// anything upstream of the proc changes how much `GameRng` it consumes.
fn first_seed_where(
    mut build: impl FnMut(u64) -> Game,
    mut found: impl FnMut(&Game) -> bool,
) -> Game {
    for seed in 0..64u64 {
        let mut game = build(seed);
        player_attacks(&mut game);
        if found(&game) {
            return game;
        }
    }
    panic!("no seed in 0..64 produced the outcome under test");
}

/// Every line the log currently holds, for tests reading a specific one out.
fn log_text(game: &Game) -> Vec<String> {
    game.world
        .resource::<MessageLog>()
        .lines
        .iter()
        .map(|e| e.text.clone())
        .collect()
}

/// The damage figure out of the one line matching `needle`, or `None`.
fn damage_in(game: &Game, needle: &str) -> Option<i32> {
    log_text(game)
        .iter()
        .find(|l| l.contains(needle))
        .and_then(|l| {
            let rest = l.split(" for ").nth(1)?;
            rest.split(' ').next()?.parse().ok()
        })
}

#[test]
fn no_wieldable_routine_is_field_only_or_decompile() {
    let mut game = Game::new(9112, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let every: Vec<String> = game
        .world
        .resource::<AbilityDb>()
        .all()
        .map(|d| d.id.clone())
        .collect();
    let program = spawn_tamed(&mut game, 40, 20);
    game.world
        .entity_mut(program)
        .insert(Routines(every.clone()));

    let wieldable = game.wieldable_routines(program);

    assert!(
        !wieldable.is_empty(),
        "the shipped set must leave something a weapon can fire"
    );
    for def in &wieldable {
        assert!(
            !def.effect.field_only(),
            "{} is field-only and has no battle recipient",
            def.id
        );
        assert!(
            !matches!(def.effect, AbilityEffect::Decompile),
            "{} would spend an ICE Breaker the player never authorised",
            def.id
        );
    }
    assert!(
        wieldable.len() < every.len(),
        "the shipped set contains at least one excluded routine, or this census proves nothing"
    );
}

/// The wielded program, named so its proc line is unmistakable in a log
/// that also carries the player's own strike and the enemy's reply.
const BLADE: &str = "Blade";

/// A player wielding a one-routine `BLADE` against a single wild program
/// too tough to die to the strike, with `GameRng` reset to `rng_seed` so
/// the proc roll is the only thing that varies between runs.
fn armed_battle(seed: u32, rng_seed: u64, routine: &str, enemy_hp: i32) -> Game {
    armed_battle_with_companions(seed, rng_seed, routine, enemy_hp, 0)
}

/// `armed_battle` with `companions` programs standing in the battle line
/// beside the player — they must join before the fight starts, since
/// `add_companion` refuses mid-battle.
fn armed_battle_with_companions(
    seed: u32,
    rng_seed: u64,
    routine: &str,
    enemy_hp: i32,
    companions: usize,
) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let program = spawn_tamed(&mut game, 60, 90);
    install_only(&mut game, program, &[routine]);
    game.world
        .entity_mut(program)
        .insert(CustomName(BLADE.to_string()));
    game.wield_program(program).unwrap();
    for _ in 0..companions {
        let companion = spawn_tamed(&mut game, 60, 20);
        install_only(&mut game, companion, &[]);
        game.add_companion(companion).unwrap();
    }
    let wild = spawn_wild_on_player_tile(&mut game);
    {
        let mut stats = game.world.get_mut::<Stats>(wild).unwrap();
        stats.hp = enemy_hp;
        stats.max_hp = enemy_hp.max(stats.max_hp);
    }
    insert_battle(&mut game, player, vec![wild]);
    game.world
        .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(rng_seed)));
    game
}

fn front_enemy(game: &Game) -> Entity {
    game.world
        .resource::<BattleState>()
        .groups
        .first()
        .expect("the fight is still running")
        .members[0]
}

#[test]
fn a_proc_scales_off_the_wielded_programs_stats() {
    // The program is deliberately stronger and higher-level than the player,
    // so a proc scaled off the wrong actor lands on a visibly wrong number.
    let game = first_seed_where(
        |rng_seed| {
            let mut game = armed_battle(9113, rng_seed, "kernel_panic", 9999);
            let program = game.wielded_program().unwrap();
            set_level(&mut game, program, 8);
            game
        },
        |g| damage_in(g, "Blade hits").is_some(),
    );

    let program = game.wielded_program().unwrap();
    let ability = ability(&game, "kernel_panic");
    let AbilityEffect::Damage { power, .. } = ability.effect else {
        panic!("kernel_panic is a Damage ability");
    };
    // The target's mitigation goes through the engine's own
    // `mitigate_incoming_damage` rather than being restated as a subtractive
    // term here — a second copy of that formula in a test is exactly what
    // drifts, and the logged figure is what actually landed.
    // A landed hit is the band's roll plus the attacker's flat ATK.
    // `kernel_panic` authors no `spread`, so the band is degenerate and rolls
    // its scaled centre exactly — which is what makes this predictable at all.
    let raw = abilities::scaled_hp_power(
        power,
        game.ability_user_level(program),
        game.ability_affinity(program, &ability.effect),
    ) + game.world.get::<Stats>(program).unwrap().atk;
    let expected = game.mitigate_incoming_damage(front_enemy(&game), raw);

    assert!(
        game.world.get::<Experience>(program).unwrap().level
            > game
                .world
                .get::<Experience>(game.player_entity())
                .unwrap()
                .level,
        "the two levels have to differ or this asserts nothing"
    );
    assert_eq!(
        damage_in(&game, "Blade hits"),
        Some(expected),
        "the proc's level, affinity and ATK all come off the program in your hand"
    );
}

#[test]
fn a_proc_lands_on_top_of_the_strike() {
    let game = first_seed_where(
        |rng_seed| armed_battle(9114, rng_seed, "kernel_panic", 9999),
        |g| damage_in(g, "Blade hits").is_some(),
    );

    let strike = damage_in(&game, "data strike").expect("the player still strikes");
    let proc = damage_in(&game, "Blade hits").unwrap();

    assert_eq!(
        9999 - game.world.get::<Stats>(front_enemy(&game)).unwrap().hp,
        strike + proc,
        "the routine lands on top of the attack, not instead of it"
    );
}

#[test]
fn no_proc_fires_when_the_strike_ended_the_battle() {
    // One enemy on one HP: the strike always kills, so the fight is over
    // before the proc could roll however the stream happens to fall.
    for rng_seed in 0..64u64 {
        let mut game = armed_battle(9115, rng_seed, "kernel_panic", 1);

        player_attacks(&mut game);

        assert!(
            damage_in(&game, "Blade hits").is_none(),
            "a routine must never resolve after the fight is already won (seed {rng_seed})"
        );
    }
}

#[test]
fn a_companion_attack_never_procs() {
    for rng_seed in 0..64u64 {
        let mut game = armed_battle_with_companions(9116, rng_seed, "kernel_panic", 9999, 1);

        player_attacks(&mut game);

        assert!(
            log_text(&game)
                .iter()
                .filter(|l| l.contains("Blade hits"))
                .count()
                <= 1,
            "two attackers, at most one proc — only slot 0 carries the weapon (seed {rng_seed})"
        );
    }
}

#[test]
fn nothing_procs_when_no_program_is_wielded() {
    for rng_seed in 0..64u64 {
        let mut game = Game::new(9117, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let wild = spawn_wild_on_player_tile(&mut game);
        {
            let mut stats = game.world.get_mut::<Stats>(wild).unwrap();
            stats.hp = 9999;
            stats.max_hp = 9999;
        }
        insert_battle(&mut game, player, vec![wild]);
        game.world
            .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(rng_seed)));

        player_attacks(&mut game);

        // At most one, not exactly one: a wild move can stun the player out
        // of its own turn. Two would be the strike plus a leaked proc.
        assert!(
            game.world
                .resource::<MessageLog>()
                .lines
                .iter()
                .filter(|e| e.kind == MessageKind::PartyDamage)
                .count()
                <= 1,
            "an empty weapon hand adds nothing to the player's side of the round (seed {rng_seed})"
        );
    }
}

#[test]
fn program_activity_names_the_wielded_program() {
    let mut game = Game::new(9118, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 20);
    assert_eq!(game.program_activity(program), "idle");

    game.wield_program(program).unwrap();

    assert_eq!(game.program_activity(program), "equipped as weapon");
}

#[test]
fn selling_your_weapon_warns_you_first() {
    let mut game = Game::new(9119, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 20);
    game.wield_program(program).unwrap();

    assert!(
        game.sale_detachments(program)
            .iter()
            .any(|d| d.contains("weapon")),
        "selling the thing in your hand warns you the way selling a member does"
    );
}

#[test]
fn pet_info_flags_the_wielded_program() {
    let mut game = Game::new(9120, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 20);
    let _other = spawn_tamed(&mut game, 30, 10);
    game.wield_program(program).unwrap();

    let rows = game.owned_pets();

    assert_eq!(
        rows.iter().filter(|p| p.wielded).count(),
        1,
        "exactly one row carries the tag"
    );
    assert!(rows.iter().find(|p| p.entity == program).unwrap().wielded);
}

#[test]
fn player_status_shows_the_wielded_program_in_place_of_a_weapon() {
    let mut game = Game::new(9121, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    game.equip(
        game.player_entity(),
        &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0),
    )
    .unwrap();
    let program = spawn_tamed(&mut game, 40, 60);
    set_level(&mut game, program, 4);
    game.wield_program(program).unwrap();

    let status = game.player_status();

    assert_eq!(status.weapon, None, "the two are mutually exclusive");
    let wielded = status.wielded.expect("the weapon line names the program");
    assert_eq!(wielded.name, game.creature_label(program));
    assert_eq!(wielded.level, 4);
    assert_eq!(wielded.bonus, game.wielded_stat_bonus());
}

#[test]
fn a_wielded_program_survives_a_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(9122, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    let program = spawn_tamed(&mut game, 40, 60);
    game.world
        .entity_mut(program)
        .insert(CustomName(BLADE.to_string()));
    let _decoy = spawn_tamed(&mut game, 30, 10);
    game.wield_program(program).unwrap();
    let armed_atk = game.effective_atk(player);
    let bonus = game.wielded_stat_bonus();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_wielded_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    // Entity ids are not stable across the round trip, so the program is
    // identified by the name it was given.
    let restored = loaded
        .wielded_program()
        .expect("the weapon comes back in hand");
    // `contains` rather than equality: a restored creature carries a
    // `ZonePortal`, so its label picks up the zone tag the hand-spawned
    // fixture never had.
    assert!(loaded.creature_label(restored).contains(BLADE));
    assert_eq!(loaded.wielded_stat_bonus(), bonus);
    assert_eq!(
        loaded.effective_atk(loaded.player_entity()),
        armed_atk,
        "and the bonus is back in the player's attack"
    );
}

/// The proc is the one carve-out from "every routine call costs Power".
/// `tuning::WIELDED_ROUTINE_PROC_CHANCE` states that the 25% rate *is* the
/// routine's whole price, and charging the program's reserve on top would
/// quietly degrade a feature that is already an easter egg.
///
/// This is why the charge lives at the `BattleAction::Special` site rather
/// than in `use_ability` — that function is also the path the proc and
/// hostile casts take.
#[test]
fn a_proc_charges_neither_the_player_nor_the_program() {
    let game = first_seed_where(
        |rng_seed| armed_battle(9115, rng_seed, "kernel_panic", 9999),
        |g| damage_in(g, "Blade hits").is_some(),
    );
    let program = game.wielded_program().unwrap();
    let cost = abilities::routine_power_cost(&ability(&game, "kernel_panic"));
    assert!(
        cost > 0.0,
        "kernel_panic has to cost something to prove this"
    );

    assert_eq!(
        game.world.get::<PowerReserve>(program).map(|r| r.get()),
        Some(POWER_MAX),
        "a wielded program's reserve is never drawn on"
    );
    // The player attacked, which costs nothing beyond the round's own drain
    // — anything approaching `cost` would mean the proc billed them.
    let player_power = game
        .world
        .get::<PowerReserve>(game.player_entity())
        .unwrap()
        .get();
    assert!(
        POWER_MAX - player_power < cost,
        "the player was charged {} for a proc that is meant to be free",
        POWER_MAX - player_power
    );
}
