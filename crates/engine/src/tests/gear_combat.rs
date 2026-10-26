//! A crafted weapon's rider and exploit in combat (ADR 0031, phase 3): the
//! rider arms its status on a landed hit through the one status-arming path,
//! and the exploit raises a swing's damage only against a target already
//! carrying its status.

use super::support::*;
use crate::gear_craft::GearPart;
use crate::gear_craft::weapon::{Frame, WeaponPart};
use crate::resources::Research;
use crate::routine_craft::{StatusKind, Subroutine, Version};
use crate::statuses::StatusId;
use crate::*;

fn edge() -> WeaponPart {
    WeaponPart::Frame(Frame::Edge, Version::V1)
}

fn bleed_id() -> StatusId {
    StatusKind::Bleed.id()
}

/// A game with a bench, a saved design of `list`, and a sturdy target; the
/// player wears the design's weapon. Returns the game, the target and the
/// swinger.
fn armed(seed: u32, list: &[WeaponPart]) -> (Game, Entity, Entity) {
    let (mut game, player) = loadout(seed, list);
    let target = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    {
        let mut stats = game.world.get_mut::<Stats>(target).unwrap();
        stats.hp = 5000;
        stats.max_hp = 5000;
    }
    insert_battle(&mut game, player, vec![target]);
    (game, target, player)
}

/// The bench, the saved design and the player wearing its weapon.
fn loadout(seed: u32, list: &[WeaponPart]) -> (Game, Entity) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_an_extraction_bench(&mut game, 30, 30);
    for part in WeaponPart::all() {
        game.world
            .resource_mut::<Research>()
            .0
            .insert(part.node_id());
    }
    let design: Vec<GearPart> = list.iter().copied().map(GearPart::Weapon).collect();
    let id = game.create_design("Test", design).unwrap();
    let player = game.player_entity();
    equip_weapon(&mut game, player, &format!("crafted_gear/{id}"));
    (game, player)
}

fn carries(game: &Game, target: Entity, status: &StatusId) -> bool {
    game.world
        .get::<StatusEffects>(target)
        .is_some_and(|s| s.active.iter().any(|a| a.id == *status))
}

fn hp(game: &Game, e: Entity) -> i32 {
    game.world.get::<Stats>(e).unwrap().hp
}

/// One abstract-round swing on `stream`: whether it landed, and whether the
/// target ended up carrying bleed.
fn swing(list: &[WeaponPart], stream: u64) -> (bool, bool) {
    let (mut game, target, player) = armed(73_001, list);
    reseed_rng(&mut game, stream);
    let before = hp(&game, target);
    game.party_member_attacks(0, player, 0, player);
    (
        hp(&game, target) < before,
        carries(&game, target, &bleed_id()),
    )
}

#[test]
fn a_rider_arms_on_a_landed_hit_never_on_a_miss_and_only_some_hits() {
    let rider = [edge(), WeaponPart::Rider(StatusKind::Bleed)];
    let (mut landed_armed, mut landed_clean, mut missed) = (0, 0, 0);
    for stream in 0..200u64 {
        let (landed, bleeding) = swing(&rider, stream);
        match (landed, bleeding) {
            (true, true) => landed_armed += 1,
            (true, false) => landed_clean += 1,
            (false, true) => panic!("stream {stream}: a miss armed a status"),
            (false, false) => missed += 1,
        }
    }
    assert!(landed_armed > 0, "the rider never armed on a landed hit");
    assert!(
        landed_clean > 0,
        "the rider armed on every hit: no chance roll"
    );
    assert!(
        missed > 0,
        "no stream missed, so 'never on a miss' proved nothing"
    );
}

#[test]
fn a_weapon_with_no_rider_arms_nothing() {
    for stream in 0..100u64 {
        let (_, bleeding) = swing(&[edge()], stream);
        assert!(!bleeding, "stream {stream}: a plain weapon armed bleed");
    }
}

/// The damage one swing does on `stream`, with or without bleed already on
/// the target.
fn damage(list: &[WeaponPart], pre_armed: bool, stream: u64) -> i32 {
    let (mut game, target, player) = armed(73_002, list);
    if pre_armed {
        game.arm_status(target, &bleed_id(), 3, 1);
    }
    reseed_rng(&mut game, stream);
    let before = hp(&game, target);
    game.party_member_attacks(0, player, 0, player);
    before - hp(&game, target)
}

#[test]
fn an_exploit_raises_damage_only_when_the_status_is_present() {
    let exploit = [edge(), WeaponPart::Exploit(StatusKind::Bleed)];
    let plain = [edge()];
    let stream = (0..512u64)
        .find(|s| damage(&plain, false, *s) >= 3)
        .expect("a stream that lands");
    let base = damage(&plain, false, stream);
    assert_eq!(
        damage(&plain, true, stream),
        base,
        "a weapon with no exploit ignores the status"
    );
    let without = damage(&exploit, false, stream);
    let with = damage(&exploit, true, stream);
    assert!(
        with > without,
        "exploit did nothing against bleed: {with} vs {without}"
    );
    // The accuracy cost shifts the stream's outcome, so the absent-status
    // swing is compared against the same weapon, not the plain one: what it
    // may not do is gain damage with nothing to exploit.
    let other = StatusKind::Exposed;
    let (mut game, target, player) = armed(73_003, &exploit);
    game.arm_status(target, &other.id(), 3, 0);
    reseed_rng(&mut game, stream);
    let before = hp(&game, target);
    game.party_member_attacks(0, player, 0, player);
    assert_eq!(
        before - hp(&game, target),
        without,
        "a different status is not the exploit's"
    );
}

/// A routine puts Exposed on the target, and a weapon exploiting Exposed
/// then hits harder: the two halves of the synergy, through real doors.
#[test]
fn a_routine_applied_exposed_feeds_an_exposed_exploit() {
    let exploit = [edge(), WeaponPart::Exploit(StatusKind::Exposed)];
    let stream = (0..512u64)
        .find(|s| damage(&exploit, false, *s) >= 3)
        .expect("a stream that lands");
    let (mut game, target, player) = armed(73_002, &exploit);
    reseed_rng(&mut game, stream);
    let before = hp(&game, target);
    game.party_member_attacks(0, player, 0, player);
    let cold = before - hp(&game, target);

    let (mut game, target, player) = armed(73_002, &exploit);
    let routine = mint_parts(
        &mut game,
        &[
            Subroutine::Core(crate::routine_craft::Core::Debuff, Version::V1),
            Subroutine::Status(StatusKind::Exposed),
        ],
    );
    let ability = game
        .world
        .resource::<AbilityDb>()
        .get(&routine)
        .cloned()
        .unwrap();
    game.use_ability(&ability, player, "You", &[target]);
    assert!(
        carries(&game, target, &StatusKind::Exposed.id()),
        "the routine did not expose the target"
    );
    reseed_rng(&mut game, stream);
    let before = hp(&game, target);
    game.party_member_attacks(0, player, 0, player);
    let exposed = before - hp(&game, target);
    assert!(exposed > cold, "{exposed} is not above {cold}");
}

/// The same exploit on a battle map, through `tactical_attack`.
#[test]
fn an_exploit_works_on_the_battle_map_too() {
    let exploit = [edge(), WeaponPart::Exploit(StatusKind::Bleed)];
    let on_map = |pre_armed: bool, stream: u64| {
        let (mut game, player) = loadout(73_004, &exploit);
        let pack = crate::tests::tactical::tactical_fight(&mut game, 1, 5000);
        let target = pack[0];
        game.world
            .resource_mut::<crate::tactical::TacticalBattle>()
            .set_initiative(vec![player, target]);
        let beside = {
            let battle = game.world.resource::<crate::tactical::TacticalBattle>();
            let (px, py) = battle.cell_of(player).expect("the player is seated");
            (px + 1, py)
        };
        assert!(
            game.world
                .resource_mut::<crate::tactical::TacticalBattle>()
                .move_to(target, beside)
        );
        if pre_armed {
            game.arm_status(target, &bleed_id(), 3, 1);
        }
        reseed_rng(&mut game, stream);
        let before = hp(&game, target);
        assert!(game.tactical_attack(target));
        before - hp(&game, target)
    };
    let stream = (0..512u64)
        .find(|s| on_map(false, *s) >= 3)
        .expect("a stream that lands");
    assert!(on_map(true, stream) > on_map(false, stream));
}
