use super::*;
use crate::abilities::{AbilityDb, PassiveTrigger};
use crate::*;

/// Plans `action` for the player and a plain attack for every
/// companion, then resolves the round. The one-line stand-in for the
/// old single-action entry points, for tests that care about what a
/// round *does* rather than how it gets planned — the planning API has
/// its own tests.
pub(crate) fn resolve_round_with(game: &mut Game, action: BattleAction) {
    let slots = game
        .world
        .get_resource::<BattleState>()
        .map(|b| b.planned.len())
        .unwrap_or(0);
    if game.battle_set_action(0, action).is_err() {
        return;
    }
    for slot in 1..slots {
        let _ = game.battle_set_action(slot, BattleAction::Attack { group: 0 });
    }
    game.battle_resolve_round();
}

pub(crate) fn player_attacks(game: &mut Game) {
    resolve_round_with(game, BattleAction::Attack { group: 0 });
}

/// Plans the player's decompile as the Special it now is, and resolves the
/// round.
pub(crate) fn player_decompiles(game: &mut Game) {
    let index = game
        .battle_special_options(0)
        .into_iter()
        .find(|o| o.name.to_lowercase().contains("decompile"))
        .expect("the player starts with decompile installed")
        .index;
    resolve_round_with(
        game,
        BattleAction::Special {
            ability: index,
            target: crate::battle::SpecialTarget::EnemyGroup { group: 0 },
            image: None,
        },
    );
}

/// Resolves a round in which `companion` uses its Special (the rally or
/// species ability that commanding it used to trigger) and everyone
/// else braces. Defend deals no damage, so anything that happens to the
/// enemy in such a round is attributable to the Special alone.
pub(crate) fn companion_uses_special(
    game: &mut Game,
    companion: Entity,
    ability: usize,
    target: battle::SpecialTarget,
) {
    companion_acts(
        game,
        companion,
        BattleAction::Special {
            ability,
            target,
            image: None,
        },
    );
}

/// Resolves a round in which `companion` takes `action` and everyone else
/// braces. A no-op if `companion` is not in the party.
pub(crate) fn companion_acts(game: &mut Game, companion: Entity, action: BattleAction) {
    let slot = game
        .world
        .resource::<Party>()
        .0
        .iter()
        .position(|&e| e == companion)
        .map(|i| i + 1);
    let Some(slot) = slot else {
        return;
    };
    let slots = game
        .world
        .get_resource::<BattleState>()
        .map(|b| b.planned.len())
        .unwrap_or(0);
    for other in 0..slots {
        let planned = if other == slot {
            action.clone()
        } else {
            BattleAction::Defend
        };
        let _ = game.battle_set_action(other, planned);
    }
    game.battle_resolve_round();
}

/// Starts a battle against `enemies`, partitioned into species groups
/// exactly as `start_battle` does. Tests that use this build their
/// combatants by hand to pin down the precise stats a case needs, which
/// `start_battle`'s own spawn path can't express — this keeps them
/// saying "these programs are in the fight" without restating the
/// partition, and without the opening log line.
pub(crate) fn insert_battle(game: &mut Game, player: Entity, enemies: Vec<Entity>) {
    let groups = game.group_pack(enemies);
    insert_battle_with_groups(game, player, groups);
}

/// `insert_battle` with the partition supplied rather than rolled. The
/// group *ceiling* is a zone reading, so `group_pack` collapses a
/// two-species pack to one group wherever the fixture happens to stand —
/// a test about how groups shift needs to state its own.
pub(crate) fn insert_battle_with_groups(
    game: &mut Game,
    player: Entity,
    groups: Vec<crate::battle::EnemyGroup>,
) {
    let slots = game.world.resource::<Party>().0.len() + 1;
    game.world.insert_resource(BattleState {
        player,
        round_targets: groups.iter().map(|g| g.members.clone()).collect(),
        groups,
        round: 1,
        planned: vec![None; slots],
        finished: false,
        player_won: false,
        decompile_attempts: std::collections::HashMap::new(),
        rewards: BattleRewards::default(),
        lair: None,
        // A hand-built fight is not weighed: `begin_battle` is what judges a
        // pack, and a fixture that wants the verdict sets it itself.
        outmatched: false,
        dead_mans_switch_spent: false,
    });
}

/// Spawns `count` hostile members of one species into a single group and
/// starts a battle against them, so back-rank indices actually exist.
/// Stats are set by hand rather than rolled, because these tests assert on
/// exact HP.
///
/// Placed deep and far on purpose: a group's size ceiling is the local
/// `max_group_size`, which at a zone-1 spawn point is one member — there
/// would be no back rank to test. The hand-set stats are what make the move
/// free, since nothing here reads the distance or zone scaling it implies.
pub(crate) fn battle_with_a_pack_of(game: &mut Game, count: usize, hp: i32) -> Vec<Entity> {
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .expect("at least one species");
    game.world.resource_mut::<ZoneLevel>().0 = 3;
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let (x, y) = (spawn.x + 500, spawn.y);
    let members: Vec<Entity> = (0..count)
        .map(|i| {
            game.world
                .spawn((
                    Creature {
                        species: species.id.clone(),
                    },
                    Hostile,
                    Position { x: x + i as i32, y },
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
    members
}

/// The id of a shipped passive with `trigger`, or a panic naming what was
/// looked for — an assertion about a fixture, not about the code.
pub(crate) fn passive_id(game: &Game, trigger: PassiveTrigger) -> String {
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
pub(crate) fn battle_with_a_passive_holder(seed: u32, passive: &str) -> Game {
    battle_with_a_passive_holder_in(&test_assets_dir(), seed, Some(passive))
}

/// The same shape against an arbitrary asset install, and with the routine
/// optional — the two things a gear grant needs that an installed one does
/// not. `None` is the control half every test of a passive needs: an
/// otherwise identical battle where nothing is armed.
pub(crate) fn battle_with_a_passive_holder_in(
    assets: &std::path::Path,
    seed: u32,
    passive: Option<&str>,
) -> Game {
    battle_with_a_passive_holder_prepared(assets, seed, passive, |_| {})
}

/// The same again with a hook that runs once the party is assembled and
/// before the fight opens. Equipping is what needs it — `Game::equip`
/// refuses mid-battle, so gear a battle is supposed to test has to go on
/// while there is still no battle.
pub(crate) fn battle_with_a_passive_holder_prepared(
    assets: &std::path::Path,
    seed: u32,
    passive: Option<&str>,
    prepare: impl FnOnce(&mut Game),
) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, assets).unwrap();
    let player = game.player_entity();
    set_level(&mut game, player, 40);
    if let Some(passive) = passive {
        install_routine_for_test(&mut game, player, passive);
    }

    let ally = spawn_tamed(&mut game, 30, 3);
    enlist(&mut game, ally);
    prepare(&mut game);

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
pub(crate) const HOSTILE_SWEEP: &str = "bus_fault";

/// Plans every party slot to defend and resolves the round. Defending
/// rather than attacking so the only damage the enemy takes in the round is
/// the passive's — which is what makes the two-round comparison in
/// `a_dropped_ally_sets_off_the_deadman_and_a_quiet_round_does_not` mean
/// what it says.
pub(crate) fn resolve_a_planned_round(game: &mut Game) {
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

pub(crate) fn total_enemy_hp(game: &Game) -> i32 {
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

/// Puts one plain copy of `item` in cargo and wears it on `wearer`. The
/// two halves are one helper because a copy nobody is carrying cannot be
/// equipped, and a test that forgot the first half reads as the feature not
/// working.
pub(crate) fn wear(game: &mut Game, wearer: Entity, item: &str) {
    let copy = crate::items::GearCopy::plain(crate::items::ItemId(item.into()));
    game.add_copies(&copy, 1);
    game.equip(wearer, &copy)
        .unwrap_or_else(|e| panic!("equipping {item}: {e}"));
}

/// Hangs a plain copy of `item` in `wearer`'s weapon slot directly.
///
/// `wear`'s blunt sibling, and the two are not interchangeable. That one
/// goes through `Game::equip`, which refuses mid-fight and refuses a wearer
/// the player does not own; this one is for the cases that door exists to
/// stop — arming a hostile, or arming anybody once a fight is already open.
/// `gear_bonus` reads `Equipment` live, so what is worn here is worth
/// exactly what an equipped copy is worth.
pub(crate) fn equip_weapon(game: &mut Game, wearer: Entity, item: &str) {
    let worn = crate::components::EquippedItem {
        copy: crate::items::GearCopy::plain(crate::items::ItemId(item.into())),
        level: 1,
    };
    let mut body = game.world.entity_mut(wearer);
    body.insert_if_new(crate::components::Equipment::default());
    body.get_mut::<crate::components::Equipment>()
        .expect("just inserted")
        .weapon = Some(worn);
}

/// `equip_weapon`'s twin for the Armor slot.
pub(crate) fn equip_armor(game: &mut Game, wearer: Entity, item: &str) {
    let worn = crate::components::EquippedItem {
        copy: crate::items::GearCopy::plain(crate::items::ItemId(item.into())),
        level: 1,
    };
    let mut body = game.world.entity_mut(wearer);
    body.insert_if_new(crate::components::Equipment::default());
    body.get_mut::<crate::components::Equipment>()
        .expect("just inserted")
        .armor = Some(worn);
}

/// Reseeds `resources::GameRng` so the next `battle::resolve_attack` lands a
/// plain hit — not a crit, not a fumble — whoever is swinging at whom.
///
/// **Matchup-independent by construction, which is why it is reliable.**
/// `resolve_attack` reads one `f64` and bands it: below the attacker's crit
/// band is a crit, below the hit chance is a hit, at or above `1 -` the
/// fumble band is a fumble, and the rest is a miss. The band starts at the
/// widest crit band any body in the game carries (Entropy can widen the
/// player's past `CRIT_CHANCE`), and `hit_chance` is clamped to at least
/// `HIT_CHANCE_MIN`, which `tuning/` asserts sits above every crit band, so
/// a first draw in it is a plain hit against *every* pairing of combatants.
/// No test using this has to know what it is fighting.
///
/// Starting at the widest band present rather than at `CRIT_CHANCE_MAX`
/// keeps the band — and so the seed it picks — unchanged for every game
/// whose bodies sit at or below base Entropy.
///
/// Only the next roll. A round that resolves several attacks gets one forced
/// swing and then the stream as it falls — a test needing more than that
/// wants `first_rng_seed_where`.
///
/// A test fixture rather than a production hook, deliberately: nothing in the
/// engine may reach for a way to make a swing land.
pub(crate) fn force_the_next_attack_to_land(game: &mut Game) {
    let widest_crit = game
        .world
        .query::<&crate::components::Derived>()
        .iter(&game.world)
        .map(|d| d.crit)
        .fold(crate::tuning::CRIT_CHANCE, f64::max);
    seed_the_next_roll_into(game, widest_crit..crate::tuning::HIT_CHANCE_MIN);
}

/// `attacker`'s next attack crits, whoever it is aimed at.
///
/// Takes the attacker because a crit band is derived (Entropy) and can be
/// narrower than `CRIT_CHANCE`. The defender is not needed: the band is
/// clamped to the hit chance, which never falls below `HIT_CHANCE_MIN`.
pub(crate) fn force_the_next_attack_to_crit(game: &mut Game, attacker: Entity) {
    let crit = game
        .combatant_profile(attacker, battle::Swing::default())
        .crit
        .min(crate::tuning::HIT_CHANCE_MIN);
    assert!(crit > 0.0, "this attacker has no crit band to force");
    seed_the_next_roll_into(game, 0.0..crit);
}

/// Reseeds `resources::GameRng` so `attacker`'s next `swing` at `defender`
/// resolves as a plain miss — not a fumble.
///
/// **Not matchup-independent**, unlike its three siblings above: the
/// plain-miss band is `[h, 1 - FUMBLE_CHANCE)`, and `h` can clamp all the way
/// up to `HIT_CHANCE_MAX`, closing that band to nothing for a lopsided
/// pairing. So this reads the pairing's actual `h` off `combatant_profile`
/// — the same call `resolve_and_apply_attack` makes — rather than assuming a
/// fixed band, and asserts the band is non-empty before scanning for a seed
/// in it, so a fixture that picked too lopsided a pairing fails with a
/// reason instead of spinning through 100,000 seeds for nothing.
pub(crate) fn force_the_next_attack_to_miss_plainly(
    game: &mut Game,
    attacker: Entity,
    defender: Entity,
    swing: battle::Swing,
) {
    let accuracy = game.combatant_profile(attacker, swing).accuracy;
    let evasion = game
        .combatant_profile(
            defender,
            battle::Swing::plain(battle::DamageRange::centred(0, 0)),
        )
        .evasion;
    let band = battle::hit_chance(accuracy, evasion)..(1.0 - crate::tuning::FUMBLE_CHANCE);
    assert!(
        !band.is_empty(),
        "plain-miss band is empty for this pairing (h={}, fumble starts at {}); \
         pick a less lopsided matchup",
        band.start,
        band.end
    );
    seed_the_next_roll_into(game, band);
}

/// The mirror of `force_the_next_attack_to_land`: the next attack fumbles,
/// and so deals nothing to its target.
///
/// Aimed at the fumble band rather than the plain-miss one because the
/// plain-miss band is empty when `h` reaches its ceiling. The fumble band is
/// `min(attacker's fumble, 1 - h)`, and `h` is clamped to at most
/// `HIT_CHANCE_MAX`, so capping the attacker's band at `1 - HIT_CHANCE_MAX`
/// gives a band that fumbles against every defender. Takes the attacker
/// because a fumble band is derived (Entropy).
pub(crate) fn force_the_next_attack_to_miss(game: &mut Game, attacker: Entity) {
    let fumble = game
        .combatant_profile(attacker, battle::Swing::default())
        .fumble
        .min(1.0 - crate::tuning::HIT_CHANCE_MAX);
    assert!(fumble > 0.0, "this attacker has no fumble band to force");
    seed_the_next_roll_into(game, (1.0 - fumble)..1.0);
}

/// Sets up `BattleState::round_targets` the way `Game::battle_resolve_round`
/// does at the top of a round, then resolves the player's own swing at
/// `group` directly, skipping `Game::roll_initiative` and every other
/// actor's turn.
///
/// **The reason this exists rather than going through
/// `resolve_round_with`**: `roll_initiative` draws once per living actor,
/// through `Rng::random_range`, before anyone's `resolve_attack` roll — so a
/// `force_the_next_attack_to_*` fixture's forced draw lands on an initiative
/// die instead of on the attack it was aimed at, and the actual swing rolls
/// whatever the stream happens to hold next. A one-sided fixture (one party
/// member, one enemy) still burns two draws this way before the swing that
/// matters.
pub(crate) fn player_swings_at_group(game: &mut Game, group: usize) {
    let player = game.player_entity();
    if let Some(mut battle) = game.world.get_resource_mut::<BattleState>() {
        battle.round_targets = battle.groups.iter().map(|g| g.members.clone()).collect();
    }
    game.party_member_attacks(0, player, group, player);
}

/// Scans for a `GameRng` seed whose first `f64` draw falls in `band` and
/// installs it. Scanned rather than mocked: `resolve_attack` takes a real
/// `Rng`, and a fake returning scripted values would stop measuring how much
/// of the stream a swing actually spends.
fn seed_the_next_roll_into(game: &mut Game, band: std::ops::Range<f64>) {
    use rand::{RngExt, SeedableRng};
    for seed in 0..100_000u64 {
        let mut candidate = rand::rngs::StdRng::seed_from_u64(seed);
        let roll: f64 = candidate.random();
        if band.contains(&roll) {
            game.world
                .insert_resource(GameRng(rand::rngs::StdRng::seed_from_u64(seed)));
            return;
        }
    }
    panic!("no seed produced a first roll inside {band:?}");
}

/// Builds and runs a whole scenario once per `GameRng` seed, returning the
/// first game where `found` holds.
///
/// For a fixture that needs *several* swings to land, which no single forced
/// roll can arrange — every matchup has a miss chance by design, so a
/// multi-round scenario cannot be made deterministic, only chosen. The world
/// is rebuilt each time so the run is identical apart from the stream, and
/// the same seed always wins, so this is deterministic rather than flaky.
///
/// Prefer it to pinning one magic seed: a pinned seed breaks silently the
/// next time anything upstream changes how much `GameRng` gets consumed.
#[allow(dead_code)]
pub(crate) fn first_rng_seed_where(
    mut build: impl FnMut(u64) -> Game,
    mut found: impl FnMut(&Game) -> bool,
) -> Game {
    for seed in 0..512u64 {
        let game = build(seed);
        if found(&game) {
            return game;
        }
    }
    panic!("no seed in 0..512 produced the outcome under test");
}

/// Installs `seed` as the run's `GameRng`, so a scenario can be replayed
/// against a chosen stream. Pairs with `first_rng_seed_where`.
pub(crate) fn reseed_rng(game: &mut Game, seed: u64) {
    game.world
        .insert_resource(GameRng(rand::SeedableRng::seed_from_u64(seed)));
}

/// Takes everything the adjacent structures are offering, through the one
/// door there now is — the shape `Game::collect_adjacent` used to be, kept
/// here rather than in the engine because "select everything, then commit"
/// is a fixture's convenience and not a verb the game has.
pub(crate) fn take_everything_adjacent(game: &mut Game) -> Vec<(ItemId, u32)> {
    let mut taken = Vec::new();
    for near in game.adjacent_structures() {
        let at = TransferAt::Structure(near.entity);
        let all: Vec<(ItemId, u32)> = game
            .transfer_offer_at(at)
            .into_iter()
            .filter(|r| r.on_shelves > 0)
            .map(|r| (r.item, r.on_shelves))
            .collect();
        taken.extend(
            game.transfer_items_at(at, &TransferBasket::items(&all, &[]))
                .0,
        );
    }
    taken
}
