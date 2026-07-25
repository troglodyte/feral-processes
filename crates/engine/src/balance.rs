//! Offline balance projections for level/zone scaling: pure, deterministic
//! simulations of the player and their party fighting a zone-scaled *pack*,
//! partitioned into species groups the way a real intrusion is, decoupled
//! from the ECS so they run as fast regression tests.
//!
//! `simulate_roster_fight` mirrors `Game::battle_resolve_round`: everyone on
//! both sides acts each round, the reach rule keeps back groups from
//! swinging, and incoming damage is spread across the roster by the same
//! aggro weights `Game::roll_enemy_target` rolls. No RNG lives here — mean
//! move power and expected damage share stand in for the real rolls, so
//! these are projections rather than samples.
//!
//! See `grind_only_zone_scaling_grows_predictably`,
//! `geared_zone_scaling_grows_predictably_and_beats_grind_only`, and
//! `a_full_party_survives_a_full_group_at_each_zone` for the actual
//! regression checks this module exists to support.

use crate::battle::compute_damage;
use crate::components::{PLAYER_BASE_STATS, Stats};
use crate::items::EquipmentStats;
use crate::progression::stats_after_levels;
use crate::resources::{BASE_PET_CAPACITY, ZoneLevel};
use crate::species::{SpeciesDb, SpeciesDef};

/// Rounds to let a simulated fight run before scoring it a loss. The cap
/// exists to catch a genuine stalemate — defense permanently outpacing
/// attack, short of `compute_damage`'s floor of 1 — and nothing else.
///
/// It was 300 back when a zone's whole pack was twelve programs. At swarm
/// scale that stopped being "generously above any realistic fight length":
/// only a group's front member is targetable and overkill is discarded, so
/// clearing a full-size group is a 100+ round fight *by construction* — a
/// zone-5 group of 81 needs over 800. At 300 the cap was scoring ordinary
/// deep-zone wins as stalemates, which is the one thing it must not do.
///
/// Power and Fatigue decay still aren't modelled here, so this is not a
/// stand-in for "how long a fight the player can actually sustain" — that
/// gap predates this constant's retuning and is not closed by it.
const TURN_CAP: u32 = 2000;

/// A companion levels at half the player's XP rate (`PARTY_XP_DIVISOR` in
/// `crate::lib`). XP cost per level grows linearly with level
/// (`xp_for_level`), so cumulative XP to reach a level grows with its
/// *square* — half the XP rate therefore lands a companion at roughly
/// `1/sqrt(2)` of the player's level over the same grinding time, not half
/// the level.
fn companion_level_for_player_level(player_level: u32) -> u32 {
    ((player_level as f64) / std::f64::consts::SQRT_2)
        .round()
        .max(1.0) as u32
}

/// `species`' `Stats` as scaled for a wild spawn in `zone`, per
/// `ZoneLevel::stat_multiplier`.
fn wild_stats_at_zone(species: &SpeciesDef, zone: u32) -> Stats {
    let mult = ZoneLevel(zone).stat_multiplier();
    Stats {
        hp: species.base_hp * mult,
        max_hp: species.base_hp * mult,
        atk: species.base_atk * mult,
        def: species.base_def * mult,
    }
}

/// Best-in-slot Weapon + Armor bonus (no fusion) at the gear level `zone`
/// unlocks — see `items::GEAR_LEVEL_GROWTH`/`Game::equip`, where gear level
/// is capped by `ZoneLevel`. Takes the two items' base `EquipmentStats`
/// (the strongest shipped weapon/armor, resolved from `ItemDb` by the
/// caller) and applies the real `scaled_for_level` scaling, so this tracks
/// any future item rebalance. Modules are skipped: their bonus is
/// `decompiler`, not combat ATK/DEF.
fn best_case_gear_bonus(zone: u32, weapon: EquipmentStats, armor: EquipmentStats) -> (i32, i32) {
    let weapon = weapon.scaled_for_level(zone);
    let armor = armor.scaled_for_level(zone);
    (weapon.atk, armor.def)
}

/// A companion tamed from `species` while breached into `zone` — it starts
/// with zone-scaled base stats (a tamed creature keeps whatever stats it
/// spawned with) and is then leveled to `level` on top of that, mirroring
/// how `Experience::default()` plus `progression::add_xp` actually grows a
/// tamed creature.
fn companion_stats(species: &SpeciesDef, caught_zone: u32, level: u32) -> Stats {
    stats_after_levels(
        wild_stats_at_zone(species, caught_zone),
        level.saturating_sub(1),
        species.growth_multiplier,
    )
}

/// A deterministic stand-in for the real move selection
/// (`Game::wild_retaliate` picks uniformly at random among `species.moves`)
/// — the mean power across the moveset.
fn average_move_power(species: &SpeciesDef) -> i32 {
    let total: i32 = species.moves.iter().map(|m| m.power).sum();
    (total as f64 / species.moves.len().max(1) as f64).round() as i32
}

/// The same mean, but over only the moves that reach past the front line —
/// what a back-rank group actually gets to pick from (see
/// `crate::ENGAGED_GROUPS`). `None` when the species has no ranged move at
/// all, which is the case that leaves a back group inert.
fn average_ranged_move_power(species: &SpeciesDef) -> Option<i32> {
    let ranged: Vec<i32> = species
        .moves
        .iter()
        .filter(|m| m.ranged)
        .map(|m| m.power)
        .collect();
    if ranged.is_empty() {
        return None;
    }
    let total: i32 = ranged.iter().sum();
    Some((total as f64 / ranged.len() as f64).round() as i32)
}

/// The strongest non-boss species (by flat `base_hp+base_atk+base_def`)
/// across every habitat — the toughest *ordinary* encounter a player must
/// be able to survive to keep progressing. Bosses are excluded: they're
/// rare, hand-tuned per-file rather than zone-scaled (see
/// `SpeciesDef::is_boss`), and not something every run is required to
/// fight to advance.
pub fn toughest_ordinary_species(db: &SpeciesDb) -> &SpeciesDef {
    db.all()
        .filter(|s| !s.is_boss)
        .max_by_key(|s| s.base_hp + s.base_atk + s.base_def)
        .expect("species db should have at least one ordinary species")
}

/// The median non-boss species by the same flat stat total — the party
/// these projections assume. A player tames what the habitat gives them,
/// so three copies of the strongest creature in the game is a best case,
/// not a baseline; a mid-grade party is what the survivability sweeps
/// need to hold for. `SpeciesDb::all` is sorted by id and the sort below
/// is stable, so ties resolve deterministically.
pub fn median_ordinary_species(db: &SpeciesDb) -> &SpeciesDef {
    let mut ordinary: Vec<&SpeciesDef> = db.all().filter(|s| !s.is_boss).collect();
    assert!(
        !ordinary.is_empty(),
        "species db should have at least one ordinary species"
    );
    ordinary.sort_by_key(|s| s.base_hp + s.base_atk + s.base_def);
    ordinary[ordinary.len() / 2]
}

/// Ticks of a single worked, tiered node needed to fund one Portal at
/// `zone`, routed through the Market's `portal_fragment` buy price.
///
/// Deliberately arithmetic rather than a live sim: this is the check that
/// the base economy keeps pace with the doubling curve, which is the whole
/// reason the travelling-base work happened. See
/// `docs/superpowers/specs/2026-07-24-travelling-base-design.md`.
///
/// Mirrors the real payout in `systems::task_progress_system` (tier ×
/// `ZoneLevel::stat_multiplier`) and the real reliability curve in
/// `systems::mining_success_chance`, so a rebalance of either shows up here.
pub fn ticks_to_afford_portal(
    zone: u32,
    tier: u32,
    ticks_per_unit: u32,
    portal_fragment_rate: u32,
    market_price: u32,
) -> f64 {
    let payout = (tier * ZoneLevel(zone).stat_multiplier() as u32) as f64;
    let success = (0.4 + tier as f64 * 0.1).min(1.0);
    let per_tick = payout * success / ticks_per_unit as f64;
    // Priced through the same helper the game charges with (see
    // `crate::zone_portal_cost`), and fragments are bought with the
    // currency the base actually produces.
    let needed = (crate::zone_portal_cost(portal_fragment_rate, zone) * market_price) as f64;
    needed / per_tick
}

pub struct BattleOutcome {
    pub player_won: bool,
    pub turns: u32,
    pub player_hp_fraction: f32,
}

/// One enemy group in a projection: `count` identical members of a species,
/// of which only the front one can be hit (see `battle::EnemyGroup`).
#[derive(Clone, Copy)]
pub struct GroupSim {
    pub stats: Stats,
    pub count: u32,
    /// Mean power of every move, used while this group is in melee range.
    pub move_power: i32,
    /// Mean power of its *ranged* moves — `None` for a melee-only species,
    /// which can do nothing at all from the back rank.
    pub ranged_move_power: Option<i32>,
}

/// One member of the player's side, tracked with fractional HP so incoming
/// damage can be spread across the roster by aggro weight rather than
/// sampled. See `simulate_roster_fight`.
#[derive(Clone, Copy)]
struct Fighter {
    hp: f64,
    max_hp: f64,
    atk: i32,
    def: i32,
    move_power: i32,
    /// Share of incoming fire, matching `Game::roll_enemy_target`'s weights.
    aggro: f64,
}

/// The swarm one intrusion throws at the player deep in `zone`: a full
/// `MAX_ENEMY_GROUPS` groups, each at the zone's group cap — what
/// `Game::max_group_size` allows once distance growth is fully unlocked.
/// Only the reach-rule test scores a swarm this size; the progression
/// sweeps project against `full_group_at_zone`.
#[cfg(test)]
fn full_pack_at_zone(species: &SpeciesDef, zone: u32) -> Vec<GroupSim> {
    let group = full_group_at_zone(species, zone);
    std::iter::repeat_n(group[0], crate::MAX_ENEMY_GROUPS).collect()
}

/// One species group at `zone`'s cap — the unit `min_level_to_clear_zone`
/// projects against. The four-group swarm is the reach rule's test case
/// (`the_reach_rule_measurably_softens_a_full_pack`), not the progression
/// baseline: the sim models no abilities, and AoE is what a four-group
/// swarm is answered with.
fn full_group_at_zone(species: &SpeciesDef, zone: u32) -> Vec<GroupSim> {
    vec![GroupSim {
        stats: wild_stats_at_zone(species, zone),
        count: crate::game::spawning::zone_group_cap(zone),
        move_power: average_move_power(species),
        ranged_move_power: average_ranged_move_power(species),
    }]
}

/// Deterministic simulation of the roster round loop: the player and every
/// party member act each round, then every enemy that can reach the party
/// does. Mirrors `Game::battle_resolve_round` closely enough for balance
/// projections:
///
/// - **Everyone attacks.** Companions deal damage now rather than only
///   granting the player a buff, so the party's damage is the sum across
///   the roster. The player uses the flat `PLAYER_STRIKE_POWER`; companions
///   use their species' `average_move_power`.
/// - **The party focuses the front group,** which is what a player does and
///   what the reach rule rewards. Each fighter's hit lands on that group's
///   front member and any overkill is discarded — the real battle can only
///   ever address the front of a group, so one action removes at most one
///   member however hard it lands.
/// - **Only `battle::attackers_in_group` of a group swing back,** the same
///   rule the real round loop applies in `Game::roll_initiative`.
/// - **Reach is enforced.** Groups past `ENGAGED_GROUPS` only act if their
///   species has a ranged move, and then at its `ranged_move_power`.
/// - **Incoming damage is spread by aggro weight** rather than all landing
///   on the player. Deterministic expectation, not a sample: each hit is
///   divided across the living roster in the same proportions
///   `Game::roll_enemy_target` rolls, which is why HP is tracked as `f64`.
///   Assuming every hit lands on the player would overstate the damage a
///   full roster takes by roughly 4x and drive the tuning badly wrong.
///
/// Initiative is not modelled: over a fight of any length, who goes first
/// each round averages out, and modelling it would need RNG this module
/// deliberately has none of.
///
/// Runs for at most `TURN_CAP` rounds; a fight that hasn't resolved by then
/// is scored as a loss — a stalemate that long isn't survivable in practice
/// (Power/Fatigue would run out first).
pub fn simulate_roster_fight(
    player: Stats,
    companions: &[Stats],
    companion_move_power: i32,
    groups: &[GroupSim],
) -> BattleOutcome {
    let mut roster: Vec<Fighter> = std::iter::once((player, crate::PLAYER_STRIKE_POWER))
        .chain(companions.iter().map(|c| (*c, companion_move_power)))
        .enumerate()
        .map(|(slot, (stats, move_power))| Fighter {
            hp: stats.hp as f64,
            max_hp: stats.max_hp as f64,
            atk: stats.atk,
            def: stats.def,
            move_power,
            aggro: if slot < crate::FRONT_SLOTS {
                crate::FRONT_SLOT_AGGRO_WEIGHT as f64
            } else {
                crate::BACK_SLOT_AGGRO_WEIGHT as f64
            },
        })
        .collect();

    // Remaining HP of the front member of each still-standing group, plus
    // how many members are still behind it.
    let mut groups: Vec<(GroupSim, i32, u32)> = groups
        .iter()
        .filter(|g| g.count > 0)
        .map(|g| (*g, g.stats.hp, g.count))
        .collect();

    let player_hp_fraction = |roster: &[Fighter]| (roster[0].hp / roster[0].max_hp).max(0.0) as f32;

    for turn in 1..=TURN_CAP {
        // Focus fire on the front group, one fighter at a time, discarding
        // overkill. Only a group's front member is targetable in the real
        // battle, so a single action kills at most one member — pooling the
        // roster's damage would let a big group evaporate at a rate nothing
        // in the game can reproduce.
        for fighter in &roster {
            if fighter.hp <= 0.0 || groups.is_empty() {
                continue;
            }
            let dealt = compute_damage(fighter.atk, groups[0].0.stats.def, fighter.move_power);
            let (group, front_hp, remaining) = &mut groups[0];
            *front_hp -= dealt;
            if *front_hp <= 0 {
                *remaining -= 1;
                if *remaining == 0 {
                    groups.remove(0);
                } else {
                    *front_hp = group.stats.hp;
                }
            }
        }
        if groups.is_empty() {
            return BattleOutcome {
                player_won: true,
                turns: turn,
                player_hp_fraction: player_hp_fraction(&roster),
            };
        }

        let total_aggro: f64 = roster.iter().filter(|f| f.hp > 0.0).map(|f| f.aggro).sum();
        if total_aggro <= 0.0 {
            return BattleOutcome {
                player_won: false,
                turns: turn,
                player_hp_fraction: 0.0,
            };
        }
        for (idx, (group, _, remaining)) in groups.iter().enumerate() {
            let power = if idx < crate::ENGAGED_GROUPS {
                group.move_power
            } else {
                match group.ranged_move_power {
                    Some(power) => power,
                    // Melee-only and out of reach: this group does nothing.
                    None => continue,
                }
            };
            for _ in 0..crate::battle::attackers_in_group(*remaining as usize) {
                for fighter in roster.iter_mut().filter(|f| f.hp > 0.0) {
                    let dealt = compute_damage(group.stats.atk, fighter.def, power) as f64;
                    fighter.hp -= dealt * fighter.aggro / total_aggro;
                }
            }
        }
        if roster[0].hp <= 0.0 {
            return BattleOutcome {
                player_won: false,
                turns: turn,
                player_hp_fraction: 0.0,
            };
        }
    }
    BattleOutcome {
        player_won: false,
        turns: TURN_CAP,
        player_hp_fraction: player_hp_fraction(&roster),
    }
}

/// Searches player levels `1..=max_level` for the lowest one at which a
/// full party (`MAX_PARTY_SIZE` companions, all tamed from `party_species`
/// while breached into `zone` and leveled per
/// `companion_level_for_player_level`) beats a **full pack** of
/// `wild_species` scaled to `zone` in `simulate_roster_fight`. `None` means
/// scaling has broken down outright — not just a long grind, but no level
/// up to `max_level` clears it.
///
/// The unit is one full-size *group* — every member of one species, at the
/// zone's cap. A lone creature is no contest at any level and would report
/// level 1 everywhere; the full four-group swarm is not something the sim
/// can score, because its intended answer is AoE and no ability is
/// modelled here.
///
/// The party is `BASE_PET_CAPACITY` strong, not `MAX_PARTY_SIZE`. Fielding
/// a full roster takes Data Caches (see `Game::pet_capacity`), so it is an
/// achievement rather than the baseline these progression sweeps are
/// supposed to describe — and modelling the ceiling here would report that
/// early zones need level 1, which says nothing about the curve. The full
/// roster is projected separately, against a full pack.
///
/// `party_species` is deliberately separate from `wild_species`: the party
/// a player actually fields is whatever they tamed along the way, not a
/// mirror of the toughest thing they have to fight. Pass
/// `median_ordinary_species` for the realistic baseline.
///
/// `with_gear` adds `best_case_gear_bonus(zone, weapon, armor)` to the
/// player's ATK/DEF (companions never carry equipment — see
/// `components::Equipment`, only ever fetched for the player entity) — set
/// it to `false` for a gear-free, pure-grind floor, `true` for the
/// fully-intended progression path where the player re-equips
/// zone-appropriate gear as they go. `weapon`/`armor` are the base
/// `EquipmentStats` of the strongest shipped gear, resolved from `ItemDb`
/// by the caller; ignored when `with_gear` is `false`.
pub fn min_level_to_clear_zone(
    wild_species: &SpeciesDef,
    party_species: &SpeciesDef,
    zone: u32,
    max_level: u32,
    with_gear: bool,
    weapon: EquipmentStats,
    armor: EquipmentStats,
) -> Option<(u32, BattleOutcome)> {
    let groups = full_group_at_zone(wild_species, zone);
    let companion_move_power = average_move_power(party_species);
    let (gear_atk, gear_def) = if with_gear {
        best_case_gear_bonus(zone, weapon, armor)
    } else {
        (0, 0)
    };
    for level in 1..=max_level {
        let mut player = stats_after_levels(
            PLAYER_BASE_STATS,
            level - 1,
            crate::progression::BASELINE_GROWTH_MULTIPLIER,
        );
        player.atk += gear_atk;
        player.def += gear_def;
        let companion_level = companion_level_for_player_level(level);
        let companions: Vec<Stats> = (0..BASE_PET_CAPACITY)
            .map(|_| companion_stats(party_species, zone, companion_level))
            .collect();
        let outcome = simulate_roster_fight(player, &companions, companion_move_power, &groups);
        if outcome.player_won {
            return Some((level, outcome));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::MAX_PARTY_SIZE;
    use std::path::Path;

    fn species_assets_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/species")
    }

    /// The shipped ability set, which `SpeciesDb::load_dir` validates
    /// species kits against.
    fn shipped_abilities() -> crate::abilities::AbilityDb {
        crate::abilities::AbilityDb::load_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/abilities"),
        )
        .unwrap()
        .0
    }

    /// Base `EquipmentStats` of the strongest shipped weapon/armor, resolved
    /// from the item db the same way `Game` does — passed into
    /// `min_level_to_clear_zone` for the geared sweep.
    fn best_gear_stats() -> (EquipmentStats, EquipmentStats) {
        use crate::items::ids;
        use crate::items_db::ItemDb;
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/items");
        let (db, _) = ItemDb::load_dir(&dir).unwrap();
        let weapon = db.get(ids::MONOFILAMENT_WHIP).unwrap().equipment.unwrap().1;
        let armor = db.get(ids::ABLATIVE_PLATING).unwrap().equipment.unwrap().1;
        (weapon, armor)
    }

    /// The shipped economy numbers `ticks_to_afford_portal` needs, read from
    /// the real structure assets rather than hardcoded, so a rebalance of
    /// any `.ron` file shows up in these projections instead of silently
    /// invalidating them.
    fn shipped_economy() -> (u32, u32, u32) {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/structures");
        let (db, _) = crate::structures::StructureDb::load_dir(&dir).unwrap();

        let node = db.get("mining_node").expect("mining_node.ron should load");
        let ticks_per_unit = node.work.as_ref().unwrap().ticks_per_unit;

        let portal = db.get("portal").expect("portal.ron should load");
        let portal_fragment_rate = portal
            .build_cost
            .iter()
            .find(|(item, _)| item.as_str() == crate::items::ids::PORTAL_FRAGMENT)
            .map(|(_, qty)| *qty)
            .expect("a portal is bought with portal fragments");

        let market = db.get("market").expect("black_market.ron should load");
        let market_price = market
            .trade
            .as_ref()
            .unwrap()
            .buy
            .iter()
            .find(|(item, _)| item.as_str() == crate::items::ids::PORTAL_FRAGMENT)
            .map(|(_, cost)| *cost)
            .expect("the market should sell portal fragments");

        (ticks_per_unit, portal_fragment_rate, market_price)
    }

    /// The check this whole change exists to satisfy: a base that's been
    /// invested in must out-earn its own rising portal cost as it goes
    /// deeper. If this fails, settling is still a losing move and the
    /// travelling-base work did not achieve its goal — report the numbers
    /// rather than relaxing the assertion.
    #[test]
    fn a_tiered_base_funds_deeper_portals_faster_than_a_fresh_one_funds_shallow_ones() {
        let (ticks, rate, price) = shipped_economy();

        let shallow = ticks_to_afford_portal(1, 1, ticks, rate, price);
        let deep = ticks_to_afford_portal(4, 3, ticks, rate, price);

        eprintln!("zone 1 Mk1: {shallow:.0} ticks/portal; zone 4 Mk3: {deep:.0} ticks/portal");
        assert!(
            deep < shallow,
            "a tiered base at depth must out-earn its own rising portal cost, or settling \
             is still a losing move: zone 1 Mk1 = {shallow:.0} ticks, zone 4 Mk3 = {deep:.0}"
        );
    }

    /// At a fixed tier, funding time must not blow up with depth — the
    /// payout doubles per zone while the portal cost only grows linearly
    /// with it, so deeper zones should get *easier* to fund, not harder.
    #[test]
    fn base_income_outpaces_portal_cost_across_every_zone() {
        let (ticks, rate, price) = shipped_economy();

        let mut previous = f64::MAX;
        for zone in 1..=6 {
            let t = ticks_to_afford_portal(zone, 3, ticks, rate, price);
            eprintln!("[Mk3] zone {zone}: {t:.1} ticks/portal");
            assert!(
                t <= previous,
                "portal funding time must not grow with depth — it did at zone {zone} \
                 ({t:.1} ticks vs {previous:.1} one zone shallower)"
            );
            previous = t;
        }
    }

    /// Pins the *shape* of the un-upgraded curve. Payout scales
    /// exponentially with depth (`2^(zone-1)`) while a Portal's cost grows
    /// by only half its base rate per zone (see
    /// `crate::ZONE_PORTAL_COST_GROWTH_PERCENT`), so the ratio is 1/1 at
    /// zone 1, 2/1.5 at zone 2, 4/2 at zone 3: every step pays off, the
    /// first one included.
    ///
    /// Under the old `build_cost × zone` ramp the first breach was exactly
    /// break-even, and upgrading was the only thing that turned it into a
    /// gain. Softening the ramp is what bought that first step — deliberate,
    /// since currency no longer survives the breach that spends it.
    #[test]
    fn an_unupgraded_base_gains_ground_from_its_very_first_breach() {
        let (ticks, rate, price) = shipped_economy();
        let at = |zone| ticks_to_afford_portal(zone, 1, ticks, rate, price);

        assert!(
            at(2) < at(1),
            "the first breach must pay for itself even without an upgrade (payout x2, \
             cost x1.5): {:.0} -> {:.0}",
            at(1),
            at(2)
        );
        assert!(
            at(3) < at(2),
            "and exponential payout keeps pulling ahead of the linear cost ramp: \
             {:.0} -> {:.0}",
            at(2),
            at(3)
        );
    }

    /// How deep to sweep the gear-free grind baseline. Zones are actually
    /// unbounded, and two things compound with depth: wild stats double
    /// every zone (`ZoneLevel::stat_multiplier`) and a group triples
    /// (`zone_group_cap`), both against the player's flat linear per-level
    /// growth. So the level needed to keep pace grows geometrically too
    /// (confirmed empirically: 1, 8, 29, 61, 138 for zones 1-5) — zone 6
    /// alone needs north of `MAX_LEVEL_SEARCHED`. That cliff is real and
    /// expected; this sweep stays inside the range where a pure-grind path
    /// is still supposed to work, so a regression in the *shape* of that
    /// curve (not its eventual end) still gets caught.
    const MAX_GRIND_ONLY_ZONE_SWEPT: u32 = 5;
    /// How deep to sweep the fully-geared scenario. `GEAR_LEVEL_GROWTH`
    /// matches `ZoneLevel::stat_multiplier`'s doubling-per-zone base (see
    /// `items::GEAR_LEVEL_GROWTH`'s doc comment), so gear neither overtakes
    /// deep zones the way the old 2.5x factor did nor collapses to "level 1
    /// clears everything" (confirmed empirically: 1, 5, 20, 45, 127 geared
    /// vs. 1, 8, 29, 61, 138 gear-free for zones 1-5).
    ///
    /// Gear's advantage narrows with depth rather than holding at a
    /// constant fraction, because a full-size group makes the fight about
    /// how long the party takes to chew through it — a per-hit ATK/DEF
    /// bonus moves that less and less as the bodies multiply.
    ///
    /// Stops at 5, not 6, for the same reason the gear-free sweep does: a
    /// zone 6 group is `MAX_GROUP_SIZE` members, which needs more than
    /// `MAX_LEVEL_SEARCHED` even fully geared. That is not a lockout in the
    /// game — this sim models no abilities, and a swarm that size is
    /// answered with AoE (`cascade_overflow`, `broadcast_storm`) rather
    /// than with more levels — so sweeping it would only pin a number the
    /// real game never asks the player to reach.
    const MAX_GEARED_ZONE_SWEPT: u32 = 5;
    const MAX_LEVEL_SEARCHED: u32 = 200;

    /// The party-size change compounds three ways: `party_stat_bonus`
    /// feeds a share of every companion's ATK/DEF into the player's own
    /// effective stats, so 3 -> 5 raises the player's *passive* stats as
    /// well as adding two more attackers and two more bodies to absorb
    /// hits. The group-size increase is the counterweight. This test is the
    /// only evidence that ratio is survivable before anyone plays it.
    ///
    /// Deliberately fought against the *toughest* ordinary species with a
    /// *median* party — a player tames what the habitat gives them, and has
    /// to survive the worst thing it spawns.
    #[test]
    fn a_full_party_survives_a_full_group_at_each_zone() {
        let (db, warnings) =
            SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should load cleanly: {warnings:?}"
        );
        let toughest = toughest_ordinary_species(&db);
        let party = median_ordinary_species(&db);
        let (weapon, armor) = best_gear_stats();

        for zone in 1..=5u32 {
            let Some((level, outcome)) = min_level_to_clear_zone(
                toughest,
                party,
                zone,
                MAX_LEVEL_SEARCHED,
                true,
                weapon,
                armor,
            ) else {
                panic!(
                    "zone {zone}: a full party can't clear a full group of {}s at any level up \
                     to {MAX_LEVEL_SEARCHED} — the group/party ratio is off",
                    toughest.name
                );
            };
            eprintln!(
                "[roster] zone {zone}: group of {} {}s needs level {level} ({} rounds, {:.0}% \
                 player HP left)",
                crate::game::spawning::zone_group_cap(zone),
                toughest.name,
                outcome.turns,
                outcome.player_hp_fraction * 100.0
            );
            assert!(
                outcome.turns > 2,
                "zone {zone}: won in {} rounds, which means the fight is trivial — a roster \
                 battle the player never gets to make a second decision in isn't one",
                outcome.turns
            );
        }
    }

    /// The reach rule has to actually be doing work. A pack big enough to
    /// fill every group must be meaningfully easier than the same number of
    /// enemies all standing in melee range — that gap *is* the valve that
    /// makes a swarm fight survivable.
    ///
    /// Zone 3 (four groups of 9) rather than deeper: past that the fight is
    /// decided by how long the party takes to chew through the front group
    /// against `TURN_CAP`, not by how much damage comes back, and both
    /// versions land on the same level — the valve stops being measurable
    /// long before it stops mattering.
    #[test]
    fn the_reach_rule_measurably_softens_a_full_pack() {
        let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        let toughest = toughest_ordinary_species(&db);
        let party = median_ordinary_species(&db);
        let companion_power = average_move_power(party);
        let zone = 3;

        let with_reach = full_pack_at_zone(toughest, zone);
        // The same pack with every group in melee range — the fight this
        // would be without the reach rule.
        let all_engaged: Vec<GroupSim> = with_reach
            .iter()
            .map(|g| GroupSim {
                ranged_move_power: Some(g.move_power),
                ..*g
            })
            .collect();

        // Compared by the level each version demands rather than by HP
        // left: past a certain depth both versions are losses at a given
        // level, and "0% vs 0%" measures nothing.
        let level_needed = |groups: &[GroupSim]| {
            (1..=MAX_LEVEL_SEARCHED).find(|&level| {
                let player = stats_after_levels(
                    PLAYER_BASE_STATS,
                    level - 1,
                    crate::progression::BASELINE_GROWTH_MULTIPLIER,
                );
                let companions: Vec<Stats> = (0..MAX_PARTY_SIZE)
                    .map(|_| companion_stats(party, zone, companion_level_for_player_level(level)))
                    .collect();
                simulate_roster_fight(player, &companions, companion_power, groups).player_won
            })
        };

        let reached = level_needed(&with_reach).expect("the reach case must be clearable");
        let unreached = level_needed(&all_engaged);
        eprintln!(
            "zone {zone} pack of {}: level {reached} with the reach rule, {} without",
            toughest.name,
            unreached
                .map(|l| l.to_string())
                .unwrap_or_else(|| format!("none up to {MAX_LEVEL_SEARCHED}")),
        );
        assert!(
            unreached.is_none_or(|unreached| reached < unreached),
            "the reach rule must lower the level a full pack demands, or ENGAGED_GROUPS is \
             buying nothing: {reached} with it, {unreached:?} without"
        );
    }

    /// Pure-grind floor: no gear equipped, ever. Confirms the level
    /// required to clear a zone with a full (leveled, zone-caught) party
    /// grows roughly geometrically with zone depth — expected, since wild
    /// stats double per zone against flat linear player growth — and
    /// catches any *sharper* blowup than that as a regression.
    #[test]
    fn grind_only_zone_scaling_grows_predictably() {
        let (db, warnings) =
            SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should all load cleanly: {warnings:?}"
        );
        let toughest = toughest_ordinary_species(&db);
        let party = median_ordinary_species(&db);
        let (weapon, armor) = best_gear_stats();

        let mut required_levels = Vec::new();
        for zone in 1..=MAX_GRIND_ONLY_ZONE_SWEPT {
            let Some((level, outcome)) = min_level_to_clear_zone(
                toughest,
                party,
                zone,
                MAX_LEVEL_SEARCHED,
                false,
                weapon,
                armor,
            ) else {
                panic!(
                    "zone {zone} ({}) isn't clearable by level {MAX_LEVEL_SEARCHED} on pure grind \
                     with a full party of {}s — the curve got steeper than expected",
                    toughest.name, party.name
                );
            };
            eprintln!(
                "[no gear] zone {zone} vs {}, party of {} {}s: needs level {level} ({} turns, \
                 {:.0}% player HP left)",
                toughest.name,
                MAX_PARTY_SIZE,
                party.name,
                outcome.turns,
                outcome.player_hp_fraction * 100.0
            );
            required_levels.push(level);
        }

        for pair in required_levels.windows(2) {
            let (prev, next) = (pair[0], pair[1]);
            assert!(
                next >= prev,
                "deeper zones should never require a *lower* level to clear: {required_levels:?}"
            );
            assert!(
                next <= prev * 3 + 5,
                "level requirement jumped from {prev} to {next} one zone deeper — the scaling \
                 curve has a cliff sharper than the expected ~2x-per-zone growth: \
                 {required_levels:?}"
            );
        }
    }

    /// Fully-geared scenario: the player re-equips best-in-slot Weapon +
    /// Armor at the gear level the current zone unlocks
    /// (`best_case_gear_bonus`), every zone. Since `GEAR_LEVEL_GROWTH` was
    /// brought down to match `ZoneLevel::stat_multiplier`'s doubling base,
    /// this now grows just as predictably as the gear-free sweep — no
    /// longer collapsing to "level 1 clears it" a few zones in, the way
    /// the old 2.5x growth did. Gear should still meaningfully lower the
    /// level a zone needs (that's the point of gearing up at all), so this
    /// also checks it stays under the gear-free requirement at every zone.
    #[test]
    fn geared_zone_scaling_grows_predictably_and_beats_grind_only() {
        let (db, warnings) =
            SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should all load cleanly: {warnings:?}"
        );
        let toughest = toughest_ordinary_species(&db);
        let party = median_ordinary_species(&db);
        let (weapon, armor) = best_gear_stats();

        let mut required_levels = Vec::new();
        for zone in 1..=MAX_GEARED_ZONE_SWEPT {
            let Some((geared_level, outcome)) = min_level_to_clear_zone(
                toughest,
                party,
                zone,
                MAX_LEVEL_SEARCHED,
                true,
                weapon,
                armor,
            ) else {
                panic!(
                    "zone {zone} ({}) isn't clearable by level {MAX_LEVEL_SEARCHED} even fully \
                     geared with a full party of {}s — that's a real lockout",
                    toughest.name, party.name
                );
            };
            eprintln!(
                "[geared] zone {zone} vs {}, party of {} {}s: needs level {geared_level} ({} \
                 turns, {:.0}% player HP left)",
                toughest.name,
                MAX_PARTY_SIZE,
                party.name,
                outcome.turns,
                outcome.player_hp_fraction * 100.0
            );
            // Only compare against the gear-free requirement where that's
            // itself known (zone <= MAX_GRIND_ONLY_ZONE_SWEPT) — beyond
            // that range gear-free is already established as unclearable
            // within MAX_LEVEL_SEARCHED (see the other test), so gear
            // being strictly *required* there is expected, not a failure.
            if let Some((grind_only_level, _)) = min_level_to_clear_zone(
                toughest,
                party,
                zone,
                MAX_LEVEL_SEARCHED,
                false,
                weapon,
                armor,
            ) {
                assert!(
                    geared_level <= grind_only_level,
                    "gear should never require a *higher* level than going without it: zone \
                     {zone} needed {geared_level} geared vs. {grind_only_level} gear-free"
                );
            }
            required_levels.push(geared_level);
        }

        for pair in required_levels.windows(2) {
            let (prev, next) = (pair[0], pair[1]);
            assert!(
                next >= prev,
                "deeper zones should never require a *lower* level to clear, geared: \
                 {required_levels:?}"
            );
            // A zone that comes out at level 1 is reporting the floor of the
            // search, not a measurement — best-in-slot gear genuinely
            // trivializes zone 1 — so the growth ratio has nothing real to
            // compare against there. Every pair after that is a measurement.
            if prev == 1 {
                continue;
            }
            assert!(
                next <= prev * 3 + 5,
                "geared level requirement jumped from {prev} to {next} one zone deeper — the \
                 scaling curve has a cliff: {required_levels:?}"
            );
        }
    }
}
