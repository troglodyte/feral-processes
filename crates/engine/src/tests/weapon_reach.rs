//! A weapon whose swing lands on more than one body.
//!
//! The feature is one `#[serde(default)]` field on `ItemDef` naming an
//! enemy-facing plural `AbilityTarget`, one door that decides whether a
//! given swing is wide (`Game::swing_reach`), and one sweep per combat
//! model built out of the converter that model already has.
//!
//! The halves held apart here are what the loader refuses, what the charge
//! gates, and what each model's sweep lands on.

use super::support::*;
use crate::abilities::{AbilityDb, AbilityShape};
use crate::items_db::ItemDb;
use crate::tactical::TacticalBattle;
use crate::tuning::TACTICAL_GROUP_RADIUS;
use crate::*;

/// `gear_passives.rs`' loader fixture: the reach's refusals live in
/// `ItemDb::load_dir` beside `ungrantable_ability`, so they need a real
/// `AbilityDb` alongside the scratch item tree.
fn load_items(tag: &str, files: &[(&str, &str)]) -> (ItemDb, Vec<String>) {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(dir.join("items")).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join("items").join(name), body).unwrap();
    }
    let (abilities, _) = AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
    ItemDb::load_dir(&dir.join("items"), &abilities).unwrap()
}

/// Armour that cleaves is an authoring mistake, and one that would
/// otherwise be silently inert: nothing but a weapon is ever asked for a
/// reach, so the field would sit in the file doing nothing.
#[test]
fn a_reach_on_a_non_weapon_is_refused_at_load() {
    let (db, warnings) = load_items(
        "reach_non_weapon",
        &[(
            "cleaving_vest.ron",
            r#"(id: "cleaving_vest", name: "Cleaving Vest", description: "d",
                equipment: Some((Armor, (mitigation: 1))),
                reach: Some((target: WholeEnemyGroup, recharge: 2)))"#,
        )],
    );
    assert!(
        db.get("cleaving_vest").is_none(),
        "only a weapon swings, so a reach on anything else must not load"
    );
    assert_eq!(warnings.len(), 1, "the skip warns: {warnings:?}");
}

/// A swing is aimed at the other side. An ally-facing target would resolve
/// through `ability_recipients` to the player's own party.
#[test]
fn a_reach_aimed_at_an_ally_is_refused_at_load() {
    let (db, warnings) = load_items(
        "reach_ally_facing",
        &[(
            "friendly_lance.ron",
            r#"(id: "friendly_lance", name: "Friendly Lance", description: "d",
                equipment: Some((Weapon, (damage: (min: 1, max: 2)))),
                reach: Some((target: WholeParty, recharge: 2)))"#,
        )],
    );
    assert!(
        db.get("friendly_lance").is_none(),
        "a reach lands on the hostile side; an ally-facing one would swing at the party"
    );
    assert_eq!(warnings.len(), 1, "the skip warns: {warnings:?}");
}

/// The same fault in two vocabularies: a reach that reaches exactly one
/// body is not a reach. `OneEnemyGroupFront` says it in the group model's
/// words and `AbilityShape::Single` says it in the board's.
#[test]
fn a_reach_that_reaches_one_body_is_refused_at_load() {
    let (db, warnings) = load_items(
        "reach_single_target",
        &[(
            "narrow_lance.ron",
            r#"(id: "narrow_lance", name: "Narrow Lance", description: "d",
                equipment: Some((Weapon, (damage: (min: 1, max: 2)))),
                reach: Some((target: OneEnemyGroupFront, recharge: 2)))"#,
        )],
    );
    assert!(
        db.get("narrow_lance").is_none(),
        "a single-target reach reaches nobody past the body already being swung at"
    );
    assert_eq!(warnings.len(), 1, "the skip warns: {warnings:?}");

    let (db, warnings) = load_items(
        "reach_single_shape",
        &[(
            "flat_lance.ron",
            r#"(id: "flat_lance", name: "Flat Lance", description: "d",
                equipment: Some((Weapon, (damage: (min: 1, max: 2)))),
                reach: Some((target: WholeEnemyGroup, shape: Some(Single), recharge: 2)))"#,
        )],
    );
    assert!(
        db.get("flat_lance").is_none(),
        "an authored Single shape is the same fault spelled on the board's side"
    );
    assert_eq!(warnings.len(), 1, "the skip warns: {warnings:?}");
}

/// The accepted case, and the one-door rule that comes with it: a shipped
/// weapon authors no `shape:`, so a reader taking `self.shape` directly
/// resolves every one of them to nothing.
#[test]
fn a_reach_weapon_loads_and_derives_its_shape() {
    let (db, warnings) = load_items(
        "reach_ok",
        &[(
            "wide_lance.ron",
            r#"(id: "wide_lance", name: "Wide Lance", description: "d",
                equipment: Some((Weapon, (damage: (min: 1, max: 2)))),
                reach: Some((target: WholeEnemyGroup, recharge: 3)))"#,
        )],
    );
    assert!(warnings.is_empty(), "nothing to warn about: {warnings:?}");
    let reach = db
        .get("wide_lance")
        .expect("a well-formed reach weapon loads")
        .reach
        .expect("the field survives the round trip");
    assert_eq!(reach.recharge, 3);
    assert_eq!(
        reach.tactical_shape(),
        AbilityShape::Radius {
            radius: TACTICAL_GROUP_RADIUS
        },
        "an unauthored shape is derived from the target, `AbilityDef::tactical_shape`'s rule"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// The one door, and the charge that gates it.
// ─────────────────────────────────────────────────────────────────────────

/// A weapon carrying a reach and a band, and one carrying only the band —
/// the control half, so a `swing_reach` that answered `Some` for everything
/// could not pass.
const REACH_WEAPON: (&str, &str) = (
    "wide_lance.ron",
    r#"(id: "wide_lance", name: "Wide Lance", description: "A lance that sweeps.",
        value: Some(40),
        equipment: Some((Weapon, (damage: (min: 4, max: 6)))),
        reach: Some((target: WholeEnemyGroup, recharge: 3)))"#,
);

const NARROW_WEAPON: (&str, &str) = (
    "plain_lance.ron",
    r#"(id: "plain_lance", name: "Plain Lance", description: "A lance.",
        value: Some(40),
        equipment: Some((Weapon, (damage: (min: 4, max: 6)))))"#,
);

/// A run against a scratch install carrying the two fixture weapons above.
/// Shipped content ships its own reach weapons, but a test about the
/// *mechanism* must not move when those are retuned.
fn install(tag: &str) -> (ScratchAssets, Game) {
    let dir = modded_assets_dir(tag, &[], &[REACH_WEAPON, NARROW_WEAPON], &[], &[], &[]);
    let game = Game::new(9_100, DifficultyMode::Forgiving, &dir).unwrap();
    (dir, game)
}

/// A hostile standing on the player's tile, in a fight with them —
/// `cloak.rs`' `abstract_fight`.
fn abstract_fight(game: &mut Game) -> Entity {
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    let wild = spawn_wild_without_routine(game, "scrapper", pos.x, pos.y);
    insert_battle(game, player, vec![wild]);
    wild
}

/// The baseline. Without it a `swing_reach` that answered `Some` for every
/// armed body would pass every other test in this file.
#[test]
fn a_weapon_with_no_reach_answers_none() {
    let (_dir, mut game) = install("reach_door_none");
    let player = game.player_entity();
    wear(&mut game, player, "plain_lance");
    abstract_fight(&mut game);

    assert!(
        game.swing_reach(player).is_none(),
        "an ordinary weapon swings at one body"
    );
}

/// The recharge is what stops a wide swing being a straight throughput
/// multiplier, so the gap between arming and being ready is the feature.
#[test]
fn a_reach_is_unavailable_until_its_charge_is_ready() {
    let (_dir, mut game) = install("reach_door_charge");
    let player = game.player_entity();
    wear(&mut game, player, "wide_lance");
    abstract_fight(&mut game);

    assert!(
        game.swing_reach(player).is_some(),
        "an unarmed charge is a ready one"
    );
    let opened = game.world.resource::<BattleState>().round;
    game.arm_reach_charge(player, 3);

    for spent in 0..3 {
        game.world.resource_mut::<BattleState>().round = opened + spent;
        assert!(
            game.swing_reach(player).is_none(),
            "round {} is inside the recharge and must swing narrow",
            opened + spent
        );
    }
    game.world.resource_mut::<BattleState>().round = opened + 3;
    assert!(
        game.swing_reach(player).is_some(),
        "the charge is ready on the round it was armed for"
    );
}

/// `ReachCharge` is battle-scoped exactly as `Cloaked`, `CombatBuff` and
/// `AbilityCooldowns` are, which is what keeps it out of `save.rs`. Left
/// set, it would follow the wielder out of one fight and hold their first
/// swing of the next one narrow.
#[test]
fn a_reach_charge_does_not_survive_the_fight() {
    let (_dir, mut game) = install("reach_door_teardown");
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 200, 1);
    enlist(&mut game, pet);
    wear(&mut game, player, "wide_lance");
    let wild = abstract_fight(&mut game);

    game.arm_reach_charge(player, 9);
    game.arm_reach_charge(pet, 9);
    game.arm_reach_charge(wild, 9);
    game.clear_battle_status_effects(player, Some(wild));

    for (who, label) in [
        (player, "the player"),
        (pet, "a companion"),
        (wild, "a hostile"),
    ] {
        assert!(
            game.world
                .get::<crate::components::ReachCharge>(who)
                .is_none(),
            "{label}'s charge survived the fight"
        );
    }

    // The next fight's first swing is wide, which is what the removal buys.
    game.world.remove_resource::<BattleState>();
    abstract_fight(&mut game);
    assert!(
        game.swing_reach(player).is_some(),
        "a fresh fight opens with the charge ready"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// The abstract model's sweep.
// ─────────────────────────────────────────────────────────────────────────

/// A single hostile group `count` deep, with the fight opened around it and
/// enough Integrity on every body that nothing dies mid-turn.
///
/// **`insert_battle_with_groups`, never `insert_battle`.** `group_pack`
/// reads the local `max_group_size`, which at a zone-1 spawn point is one
/// member — the rolled partition would drop the bodies this fixture exists
/// to stand behind the front.
fn group_of(game: &mut Game, count: usize) -> Vec<Entity> {
    let player = game.player_entity();
    let at = *game.world.get::<Position>(player).unwrap();
    let members: Vec<Entity> = (0..count)
        .map(|_| {
            let e = spawn_wild_without_routine(game, "scrapper", at.x, at.y);
            let mut stats = game.world.get_mut::<Stats>(e).unwrap();
            stats.max_hp = 100_000;
            stats.hp = 100_000;
            e
        })
        .collect();
    let species = game
        .world
        .get::<Creature>(members[0])
        .unwrap()
        .species
        .clone();
    insert_battle_with_groups(
        game,
        player,
        vec![crate::battle::EnemyGroup {
            species,
            members: members.clone(),
        }],
    );
    members
}

/// The open fight's first group, read back off `BattleState` rather than
/// carried out of the fixture — `first_rng_seed_where` rebuilds the world
/// per seed, so the entities a closure minted are not the ones under test.
fn group_members(game: &Game) -> Vec<Entity> {
    game.world.resource::<BattleState>().groups[0]
        .members
        .clone()
}

/// How many bodies `who` is standing at less than full Integrity.
fn wounded(game: &Game, bodies: &[Entity]) -> usize {
    bodies
        .iter()
        .filter(|&&e| {
            let s = game.world.get::<Stats>(e).unwrap();
            s.hp < s.max_hp
        })
        .count()
}

/// Every swing the player has taken this turn, counted off the one thing
/// every outcome of one shares: the move's own name. A crit, a hit, a miss
/// and all four fumble rungs each write exactly one line naming it, so this
/// is an exact count of swings resolved and not an estimate of how many
/// landed.
fn player_swings_logged(game: &Game) -> usize {
    game.world
        .resource::<MessageLog>()
        .lines
        .iter()
        .filter(|l| l.text.contains("data strike"))
        .count()
}

/// The feature. One swing, and every member of the group takes its own
/// `resolve_and_apply_attack` — so mitigation, affinity and the fumble
/// ladder hold for each of them with no new damage path.
///
/// Seeded through `first_rng_seed_where` rather than forced: no matchup in
/// the game is a guaranteed landing (`HIT_CHANCE_MAX`), so three swings
/// that all land is a stream to choose and not one to force.
#[test]
fn a_weapon_reach_lands_on_every_member_of_the_group() {
    let dir = modded_assets_dir(
        "reach_group_sweep",
        &[],
        &[REACH_WEAPON, NARROW_WEAPON],
        &[],
        &[],
        &[],
    );
    let game = first_rng_seed_where(
        |seed| {
            let mut game = Game::new(9_101, DifficultyMode::Forgiving, &dir).unwrap();
            let player = game.player_entity();
            wear(&mut game, player, "wide_lance");
            group_of(&mut game, 3);
            reseed_rng(&mut game, seed);
            player_swings_at_group(&mut game, 0);
            game
        },
        |game| wounded(game, &group_members(game)) == 3,
    );

    let bodies = group_members(&game);
    assert_eq!(bodies.len(), 3, "the group is still three deep");
    assert_eq!(
        wounded(&game, &bodies),
        3,
        "one wide swing lands on every member of the group"
    );
}

/// `attacks_for` loops a Striker's second swing *inside* the turn, so a
/// charge armed per swing silently doubles what the weapon is worth and
/// nothing in `BattleState::planned` or app-core ever learns the feature
/// exists — `proc_wielded_routine`'s rule, applied verbatim.
///
/// Counted in swings rather than in damage: a Striker's turn against a
/// three-deep group is three swings then one if the charge is armed once,
/// and three then three if it is armed per swing.
#[test]
fn a_reach_arms_once_a_turn_and_not_once_a_swing() {
    let dir = modded_assets_dir(
        "reach_once_a_turn",
        &[],
        &[REACH_WEAPON, NARROW_WEAPON],
        &[],
        &[],
        &[],
    );
    // A stream in which the turn is neither cut short nor turned around: a
    // Crash rung ends the turn and a Recoil rung can kill the swinger, and
    // both would leave a *shorter* count that the per-swing bug also
    // produces. The criterion is orthogonal to what is being counted.
    let game = first_rng_seed_where(
        |seed| {
            let mut game = Game::new(9_102, DifficultyMode::Forgiving, &dir).unwrap();
            let player = game.player_entity();
            game.world.get_mut::<Experience>(player).unwrap().level =
                crate::tuning::EXTRA_ATTACK_LEVEL;
            game.world.get_mut::<PlayerIdentity>(player).unwrap().class =
                Some(crate::classes::PlayerClass::Striker);
            wear(&mut game, player, "wide_lance");
            group_of(&mut game, 3);
            reseed_rng(&mut game, seed);
            player_swings_at_group(&mut game, 0);
            game
        },
        |game| {
            let player = game.player_entity();
            !game.is_stunned(player) && game.creature_alive(player)
        },
    );
    let player = game.player_entity();
    assert_eq!(
        game.attacks_for(player),
        2,
        "the fixture wants a wielder that swings twice a turn"
    );
    assert_eq!(
        player_swings_logged(&game),
        4,
        "three bodies on the wide swing and one on the narrow one that follows"
    );
}

/// The recharge, measured where it is felt. Round 1 sweeps the group; round
/// 2 is an ordinary swing at the front and the bodies behind it are
/// untouched.
#[test]
fn a_recharging_reach_swings_narrow_until_it_is_ready() {
    let dir = modded_assets_dir(
        "reach_recharge",
        &[],
        &[REACH_WEAPON, NARROW_WEAPON],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(9_103, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    wear(&mut game, player, "wide_lance");
    let bodies = group_of(&mut game, 3);

    player_swings_at_group(&mut game, 0);
    let after_the_wide_swing: Vec<i32> = bodies
        .iter()
        .map(|&e| game.world.get::<Stats>(e).unwrap().hp)
        .collect();

    // The next round, inside the fixture weapon's recharge of 3.
    game.world.resource_mut::<BattleState>().round += 1;
    player_swings_at_group(&mut game, 0);

    for (index, body) in bodies.iter().enumerate().skip(1) {
        assert_eq!(
            game.world.get::<Stats>(*body).unwrap().hp,
            after_the_wide_swing[index],
            "body {index} stands behind the front and the narrow swing cannot reach it"
        );
    }
}

/// A Recoil rung damages the fumbler, so the swinger really can die on its
/// own first body — the guard `party_member_attacks` already carries, one
/// level down.
///
/// Asserted directly rather than by forcing a fatal fumble: the rung is one
/// band of one roll, and a deterministic fatal one is a stream to hunt for
/// rather than one to force. A swinger that is already down is exactly the
/// state a Recoil leaves behind mid-sweep.
///
/// **The first body is not guarded and must not be.** `party_member_
/// attacks` spells its own guard `swing > 0 &&` — a body's opening swing
/// lands whatever its own Integrity says — so the sweep breaks between
/// bodies and never before the first one, or the narrow path stops
/// behaving as it always has.
#[test]
fn a_dead_swinger_stops_the_sweep() {
    let dir = modded_assets_dir(
        "reach_dead_swinger",
        &[],
        &[REACH_WEAPON, NARROW_WEAPON],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(9_104, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    wear(&mut game, player, "wide_lance");
    let bodies = group_of(&mut game, 3);

    game.world.get_mut::<Stats>(player).unwrap().hp = 0;
    player_swings_at_group(&mut game, 0);

    assert_eq!(
        wounded(&game, &bodies[1..]),
        0,
        "the sweep stops at the body the swinger died on"
    );
}

// ─────────────────────────────────────────────────────────────────────────
// The battle map's sweep.
// ─────────────────────────────────────────────────────────────────────────

/// A tactical fight with the player wearing `weapon`, the whole party and
/// pack placed by hand, and the player holding the turn.
///
/// The bodies are laid out in a row so the geometry is stated rather than
/// deployed: the player, the body they aim at, one body a cell further on
/// (inside the derived `Radius { 1 }` around the aim) and one two cells
/// further still (outside it).
struct Board {
    game: Game,
    player: Entity,
    target: Entity,
    beside: Entity,
    away: Entity,
    pet: Entity,
}

fn tactical_board(tag: &str, weapon: &str) -> (ScratchAssets, Board) {
    let dir = modded_assets_dir(tag, &[], &[REACH_WEAPON, NARROW_WEAPON], &[], &[], &[]);
    let mut game = Game::new(9_200, DifficultyMode::Forgiving, &dir).unwrap();
    let mut profile = game.profile().clone();
    profile.tactical_battles = true;
    game.install_profile(profile);

    let player = game.player_entity();
    wear(&mut game, player, weapon);
    let pet = spawn_tamed(&mut game, 10_000, 1);
    enlist(&mut game, pet);

    let at = *game.world.get::<Position>(player).unwrap();
    let pack: Vec<Entity> = (0..3)
        .map(|i| {
            let e = spawn_wild_without_routine(&mut game, "scrapper", at.x + 1 + i, at.y);
            let mut stats = game.world.get_mut::<Stats>(e).unwrap();
            stats.max_hp = 10_000;
            stats.hp = 10_000;
            e
        })
        .collect();
    game.open_tactical_battle(pack.clone());

    // A row along y, with the player at x and the aim at x + 1. `Radius { 1 }`
    // around the aim covers x..=x + 2, so `beside` is caught and `away` is
    // not — and the companion is placed inside it deliberately.
    let (x, y) = (4, 4);
    let placements = [
        (player, (x, y)),
        (pack[0], (x + 1, y)),
        (pack[1], (x + 2, y)),
        (pack[2], (x + 4, y)),
        (pet, (x + 1, y + 1)),
    ];
    for (body, cell) in placements {
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(body, cell),
            "the fixture could not stand a body on {cell:?}"
        );
    }
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0], pack[1], pack[2], pet]);

    let board = Board {
        game,
        player,
        target: pack[0],
        beside: pack[1],
        away: pack[2],
        pet,
    };
    (dir, board)
}

fn hurt(game: &Game, body: Entity) -> bool {
    let s = game.world.get::<Stats>(body).unwrap();
    s.hp < s.max_hp
}

/// The feature on a board. The shape is the one derived from the weapon's
/// `target`, centred on the cell of the body already being swung at —
/// `reach::recipients`' existing rule that an aim is a destination for a
/// blast.
///
/// Seeded through `first_rng_seed_where` for the group sweep's reason: no
/// matchup is a guaranteed landing.
#[test]
fn a_weapon_reach_lands_on_every_body_in_the_blast() {
    let (_dir, mut board) = tactical_board("reach_blast", "wide_lance");
    let seed = (0..512u64)
        .find(|&seed| {
            let mut probe = tactical_board("reach_blast_probe", "wide_lance").1;
            reseed_rng(&mut probe.game, seed);
            probe.game.tactical_attack(probe.target);
            hurt(&probe.game, probe.target) && hurt(&probe.game, probe.beside)
        })
        .expect("some stream in 0..512 lands both swings");
    reseed_rng(&mut board.game, seed);

    assert!(
        board.game.tactical_attack(board.target),
        "the swing was refused"
    );
    assert!(
        hurt(&board.game, board.target),
        "the body aimed at takes the swing it always took"
    );
    assert!(
        hurt(&board.game, board.beside),
        "the body inside the blast takes its own"
    );
}

/// **The friendly-fire rule, held explicitly.** `reach::recipients` never
/// reads `Hostile`, and a side filter added here is one line that reads as
/// an obvious bug fix, breaks nothing that compiles, and deletes the reason
/// a shape is worth aiming at all. A cleaving weapon catches your own
/// companion standing beside the target. That is the tactical price of the
/// reach.
#[test]
fn a_weapon_reach_catches_a_companion_standing_beside_the_target() {
    let seed = (0..512u64)
        .find(|&seed| {
            let mut probe = tactical_board("reach_ff_probe", "wide_lance").1;
            reseed_rng(&mut probe.game, seed);
            probe.game.tactical_attack(probe.target);
            hurt(&probe.game, probe.pet)
        })
        .expect("some stream in 0..512 lands the swing on the companion");

    let (_dir, mut board) = tactical_board("reach_friendly_fire", "wide_lance");
    reseed_rng(&mut board.game, seed);
    board.game.tactical_attack(board.target);
    assert!(
        hurt(&board.game, board.pet),
        "a companion inside the blast is not filtered out of it"
    );
}

/// The negative half, or the two above pass against "hit everything on the
/// board". The blast is a shape and not a side.
#[test]
fn a_body_outside_the_shape_is_untouched() {
    let (_dir, mut board) = tactical_board("reach_outside", "wide_lance");
    assert!(
        board.game.tactical_attack(board.target),
        "the swing was refused"
    );
    assert!(
        !hurt(&board.game, board.away),
        "a body two cells past the aim stands outside the derived radius"
    );
    assert!(
        !hurt(&board.game, board.player),
        "the swinger is not a recipient of its own cleave"
    );
}

/// A weapon with no reach swings exactly where it always did, on a board as
/// in front of a group.
#[test]
fn a_plain_weapon_swings_at_one_body_on_a_board() {
    let (_dir, mut board) = tactical_board("reach_board_narrow", "plain_lance");
    board.game.tactical_attack(board.target);
    assert!(
        !hurt(&board.game, board.beside),
        "an ordinary weapon reaches nobody beside the body it is aimed at"
    );
    assert!(
        !hurt(&board.game, board.pet),
        "and catches no companion either"
    );
}

/// A wide swing can empty the last group and kill its own swinger in the
/// **same turn** — one body's blow lands the killing hit, a later body's
/// fumble puts the swinger down on a Recoil rung. The group model then
/// closes the fight inside `reap_dead_members`, and
/// `battle_resolve_round` still owes the round's upkeep.
///
/// A narrow swing cannot reach that state: a fumble deals the defender no
/// damage, so the swing that empties a group is never the swing that kills
/// the swinger. This is the state the arena found on the second rep of
/// `dev-arenas/measure-reach-broadcast-storm.ron`.
///
/// Asserted on `tick_round_status_effects` directly, because getting there
/// through a fight means hunting a stream for a fatal fumble on the right
/// body — the state is what matters and it is exactly reproducible.
#[test]
fn the_rounds_upkeep_survives_a_swing_that_ended_the_fight_and_the_swinger() {
    let (_dir, mut game) = install("reach_upkeep_after_the_end");
    let player = game.player_entity();
    let wild = abstract_fight(&mut game);

    // What a sweep that emptied the roster leaves behind: no fight, and a
    // swinger who did not outlive it.
    game.world.remove_resource::<BattleState>();
    game.world.despawn(wild);
    game.world.get_mut::<Stats>(player).unwrap().hp = 0;

    game.tick_round_status_effects(player);
}
