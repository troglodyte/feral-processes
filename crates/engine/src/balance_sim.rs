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
use crate::components::Stats;
use crate::items::EquipmentStats;
use crate::progression::stats_after_levels;
use crate::resources::ZoneLevel;
use crate::species::{SpeciesDb, SpeciesDef};
use crate::tuning::PLAYER_BASE_STATS;

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
/// unlocks — see `crate::tuning::GEAR_LEVEL_GROWTH`/`Game::equip`, where gear level
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
///
/// Unaffected by `WILD_ABILITY_CHANCE`: that gates a chosen move's *status
/// effect*, not its damage, and this module models damage only.
fn average_move_power(species: &SpeciesDef) -> i32 {
    let total: i32 = species.moves.iter().map(|m| m.power).sum();
    (total as f64 / species.moves.len().max(1) as f64).round() as i32
}

/// The same mean, but over only the moves that reach past the front line —
/// what a back-rank group actually gets to pick from (see
/// `crate::tuning::ENGAGED_GROUPS`). `None` when the species has no ranged move at
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

/// Whether a bare, level-1 player — no companions, no gear — is projected
/// to beat one wild `species` one-on-one at zone 1, even when that
/// individual rolls its best (`MAX_INDIVIDUAL_ROLL` on every stat).
///
/// This is the only fight in this module projected without a party, and
/// it's the one the game actually opens on: `Party` starts empty, so until
/// the player wins something they can decompile, every encounter is solo.
/// The zone-1 opening ring spawns only species this holds for — see
/// `Game::try_spawn_habitat_creature`. Eleven of the fifteen shipped
/// ordinary species fail it — ten of them even on an average roll — which
/// is what made the opening unwinnable.
///
/// Not a claim that the excluded species are unfair — they're what the ring
/// exists to keep out of the player's first few fights, and they're waiting
/// one step further out.
pub fn beatable_by_a_fresh_player(species: &SpeciesDef) -> bool {
    let best_roll = |base: i32| (base as f32 * crate::tuning::MAX_INDIVIDUAL_ROLL).round() as i32;
    let stats = wild_stats_at_zone(species, 1);
    let group = GroupSim {
        stats: Stats {
            hp: best_roll(stats.hp),
            max_hp: best_roll(stats.max_hp),
            atk: best_roll(stats.atk),
            def: best_roll(stats.def),
        },
        count: 1,
        move_power: average_move_power(species),
        ranged_move_power: average_ranged_move_power(species),
    };
    simulate_roster_fight(
        PLAYER_BASE_STATS,
        &[],
        crate::tuning::PLAYER_STRIKE_POWER,
        &[group],
    )
    .player_won
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
    /// Share of incoming fire, from the same `battle::slot_aggro_weight`
    /// `Game::roll_enemy_target` rolls against.
    aggro: f64,
}

/// The swarm one intrusion throws at the player deep in `zone`: a full
/// `MAX_ENEMY_GROUPS` groups, each at the zone's group cap — what
/// `Game::max_group_size` allows once distance growth is fully unlocked.
/// Only the reach rule scores a swarm this size; the progression sweeps
/// project against `full_group_at_zone`.
fn full_pack_at_zone(species: &SpeciesDef, zone: u32) -> Vec<GroupSim> {
    let group = full_group_at_zone(species, zone);
    std::iter::repeat_n(group[0], crate::tuning::MAX_ENEMY_GROUPS).collect()
}

/// The zone the reach rule is measured at: four groups of nine. Deeper and
/// the fight is decided by how long the party takes to chew through the
/// front group against `TURN_CAP` rather than by how much damage comes
/// back, and both versions land on the same level — the valve stops being
/// measurable long before it stops mattering.
const REACH_RULE_ZONE: u32 = 3;

/// The highest level either half of the reach measurement will search to.
const REACH_RULE_MAX_LEVEL: u32 = 200;

/// What the reach rule is worth on a roster, as the level a full pack
/// demands with it and without it. `None` means unclearable inside
/// `REACH_RULE_MAX_LEVEL`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReachRuleVerdict {
    pub with_reach: Option<u32>,
    pub all_engaged: Option<u32>,
}

impl ReachRuleVerdict {
    /// Whether the valve is still open. A pack big enough to fill every
    /// group must be meaningfully easier than the same number of enemies
    /// all standing in melee range — that gap *is* what makes a swarm fight
    /// survivable, and a roster where it closes has had `ENGAGED_GROUPS`
    /// quietly stop buying anything.
    pub fn holds(&self) -> bool {
        match self.with_reach {
            None => false,
            Some(reached) => self.all_engaged.is_none_or(|unreached| reached < unreached),
        }
    }
}

impl std::fmt::Display for ReachRuleVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let level = |l: Option<u32>| match l {
            Some(l) => l.to_string(),
            None => format!("none up to {REACH_RULE_MAX_LEVEL}"),
        };
        write!(
            f,
            "zone {REACH_RULE_ZONE} full pack demands level {} with the reach rule, {} without",
            level(self.with_reach),
            level(self.all_engaged)
        )
    }
}

/// Measures what the reach rule is worth on `db`'s roster.
///
/// Compared by the level each version demands rather than by HP left: past
/// a certain depth both versions are losses at a given level, and "0% vs
/// 0%" measures nothing.
///
/// Shipping code because the roster tuner post-checks a winning proposal
/// against it. That check cannot be a rejection — it runs a level search
/// per call and would be paid on every candidate — so it runs once on the
/// winner and is *reported*, which is why this returns the two levels
/// rather than a bool: a human reading a proposal's diff needs to see how
/// close it came, not merely that it passed.
/// `the_reach_rule_measurably_softens_a_full_pack` asserts `holds()`.
pub fn reach_rule_verdict(db: &SpeciesDb) -> ReachRuleVerdict {
    let toughest = toughest_ordinary_species(db);
    let party = median_ordinary_species(db);
    let companion_power = average_move_power(party);

    let with_reach = full_pack_at_zone(toughest, REACH_RULE_ZONE);
    // The same pack with every group in melee range — the fight this would
    // be without the reach rule.
    let all_engaged: Vec<GroupSim> = with_reach
        .iter()
        .map(|g| GroupSim {
            ranged_move_power: Some(g.move_power),
            ..*g
        })
        .collect();

    let level_needed = |groups: &[GroupSim]| {
        (1..=REACH_RULE_MAX_LEVEL).find(|&level| {
            let player = stats_after_levels(
                PLAYER_BASE_STATS,
                level - 1,
                crate::tuning::BASELINE_GROWTH_MULTIPLIER,
            );
            let companions: Vec<Stats> = (0..crate::tuning::MAX_PARTY_SIZE)
                .map(|_| {
                    companion_stats(
                        party,
                        REACH_RULE_ZONE,
                        companion_level_for_player_level(level),
                    )
                })
                .collect();
            simulate_roster_fight(player, &companions, companion_power, groups).player_won
        })
    };

    ReachRuleVerdict {
        with_reach: level_needed(&with_reach),
        all_engaged: level_needed(&all_engaged),
    }
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
    let mut roster: Vec<Fighter> = std::iter::once((player, crate::tuning::PLAYER_STRIKE_POWER))
        .chain(companions.iter().map(|c| (*c, companion_move_power)))
        .enumerate()
        .map(|(slot, (stats, move_power))| Fighter {
            hp: stats.hp as f64,
            max_hp: stats.max_hp as f64,
            atk: stats.atk,
            def: stats.def,
            move_power,
            aggro: crate::battle::slot_aggro_weight(slot, false) as f64,
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
            let power = if idx < crate::tuning::ENGAGED_GROUPS {
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
/// party of `companion_count` (all tamed from `party_species` while
/// breached into `zone` and leveled per `companion_level_for_player_level`)
/// beats a **full-size group** of `wild_species` scaled to `zone` in
/// `simulate_roster_fight`. `None` means scaling has broken down outright —
/// not just a long grind, but no level up to `max_level` clears it.
///
/// The unit is one full-size *group* — every member of one species, at the
/// zone's cap. A lone creature is no contest at any level and would report
/// level 1 everywhere; the full four-group swarm is not something the sim
/// can score, because its intended answer is AoE and no ability is
/// modelled here.
///
/// `companion_count` is a parameter, not a hardcoded constant, because
/// different callers are asking different questions with it: the
/// progression sweeps want `BASE_PET_CAPACITY`, the party most players are
/// actually fielding (fielding `MAX_PARTY_SIZE` takes Data Caches — see
/// `Game::pet_capacity` — so it is an achievement rather than the
/// baseline those sweeps describe, and modelling it there would report
/// that early zones need level 1, which says nothing about the curve);
/// the full-roster ratio test wants `MAX_PARTY_SIZE` specifically, because
/// that ratio is the thing under test.
///
/// `party_species` is deliberately separate from `wild_species`: the party
/// a player actually fields is whatever they tamed along the way, not a
/// mirror of the toughest thing they have to fight. Pass
/// `median_ordinary_species` for the realistic baseline.
///
/// `with_gear` adds `best_case_gear_bonus(zone, gear.0, gear.1)` to the
/// player's ATK/DEF (companions never carry equipment — see
/// `components::Equipment`, only ever fetched for the player entity) — set
/// it to `false` for a gear-free, pure-grind floor, `true` for the
/// fully-intended progression path where the player re-equips
/// zone-appropriate gear as they go. `gear` is `(weapon, armor)`, the base
/// `EquipmentStats` of the strongest shipped gear, resolved from `ItemDb`
/// by the caller (see `best_gear_stats`, which already returns them paired
/// this way) — kept as one tuple parameter rather than two so this
/// function stays under clippy's argument-count lint; ignored when
/// `with_gear` is `false`.
pub fn min_level_to_clear_zone(
    wild_species: &SpeciesDef,
    party_species: &SpeciesDef,
    zone: u32,
    max_level: u32,
    companion_count: usize,
    with_gear: bool,
    gear: (EquipmentStats, EquipmentStats),
) -> Option<(u32, BattleOutcome)> {
    let groups = full_group_at_zone(wild_species, zone);
    let companion_move_power = average_move_power(party_species);
    let (gear_atk, gear_def) = if with_gear {
        best_case_gear_bonus(zone, gear.0, gear.1)
    } else {
        (0, 0)
    };
    for level in 1..=max_level {
        let mut player = stats_after_levels(
            PLAYER_BASE_STATS,
            level - 1,
            crate::tuning::BASELINE_GROWTH_MULTIPLIER,
        );
        player.atk += gear_atk;
        player.def += gear_def;
        let companion_level = companion_level_for_player_level(level);
        let companions: Vec<Stats> = (0..companion_count)
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
    use crate::tuning::{BASE_PET_CAPACITY, MAX_PARTY_SIZE};
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

    /// The opening ring filters the shipped roster with
    /// `beatable_by_a_fresh_player`, so that predicate has to keep some
    /// species and turn others away. All-pass and it's inert — the ring
    /// stops doing anything and a zone-1 opening goes back to being
    /// unwinnable. All-fail and every ring falls back to its biome's
    /// unfiltered pool, which is the same thing by a different route.
    ///
    /// A rebalance of the shipped `.ron` stats is exactly what would push
    /// it to either end, which is why this reads the real assets.
    #[test]
    fn the_shipped_roster_has_species_on_both_sides_of_the_opening_ring() {
        let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        let (gentle, tough): (Vec<&SpeciesDef>, Vec<&SpeciesDef>) =
            db.all().partition(|s| beatable_by_a_fresh_player(s));
        eprintln!(
            "[ring] a fresh player beats {} of {} shipped species solo: {:?}",
            gentle.len(),
            gentle.len() + tough.len(),
            gentle.iter().map(|s| &s.id).collect::<Vec<_>>()
        );

        assert!(
            !gentle.is_empty(),
            "nothing a bare level-1 player can beat ships at all, so the ring would \
             fall back to the full pool everywhere and the opening stays unwinnable"
        );
        assert!(
            !tough.is_empty(),
            "every shipped species is beatable solo, which makes the ring inert — \
             either the roster got soft or the projection stopped measuring the \
             fight the player actually opens on"
        );
        for boss in db.all().filter(|s| s.is_boss) {
            assert!(
                !beatable_by_a_fresh_player(boss),
                "{} is projected as a fair solo fight for a level-1 player, which \
                 would let a boss spawn inside the opening ring",
                boss.id
            );
        }
    }

    /// How deep to sweep the gear-free grind baseline. Zones are unbounded,
    /// so what this depth is really asserting is that the curve *has no end*
    /// — and that is a claim only a linear curve can make.
    ///
    /// It used to stop at 5, because it had to: wild stats doubled every
    /// zone against the player's flat per-level growth, so the level needed
    /// to keep pace doubled too (measured: 1, 15, 30, 63, 131 for zones 1-5)
    /// and zone 6 alone wanted more than `MAX_LEVEL_SEARCHED`. That was
    /// written up as a cliff that was "real and expected". It was neither:
    /// it was a geometric quantity racing a linear one, which has an end
    /// wherever you put the coefficients, and past that end
    /// `battle::compute_damage`'s subtractive rule floors every swing at
    /// `MIN_DAMAGE` so no amount of levelling helps at all.
    ///
    /// With `ZONE_STAT_STEP` linear the measured curve is 1, 15, 24, 32, 47,
    /// 61, 76, 90, 106, 121 for zones 1-10 — about 14 levels a zone,
    /// indefinitely. Sweeping to 10 is therefore the gate itself: a return
    /// to a compounding curve cannot reach zone 10 inside
    /// `MAX_LEVEL_SEARCHED` and fails here rather than shipping.
    const MAX_GRIND_ONLY_ZONE_SWEPT: u32 = 10;
    /// How deep to sweep the fully-geared scenario. `GEAR_LEVEL_STEP`
    /// matches `ZoneLevel::stat_multiplier`'s per-zone step (see
    /// `crate::tuning::GEAR_LEVEL_STEP`), so gear neither overtakes deep
    /// zones the way the old 2.5x factor did nor collapses to "level 1
    /// clears everything" (measured: 1, 10, 18, 31, 43, 56, 70, 83, 97, 112
    /// geared vs. 1, 15, 24, 32, 47, 61, 76, 90, 106, 121 gear-free for
    /// zones 1-10).
    ///
    /// Gear's advantage narrows with depth rather than holding at a
    /// constant fraction, because a full-size group makes the fight about
    /// how long the party takes to chew through it — a per-hit ATK/DEF
    /// bonus moves that less and less as the bodies multiply.
    const MAX_GEARED_ZONE_SWEPT: u32 = 10;
    const MAX_LEVEL_SEARCHED: u32 = 200;

    /// How much the largest per-zone *step* in the level curve may exceed
    /// the smallest, past `GROWTH_GUARD_FIRST_MEASURED_PAIR`.
    ///
    /// The guard beside this one (`LEVEL_GROWTH_GUARD_MULTIPLIER`) bounds
    /// the *ratio* between consecutive requirements, which is a one-sided
    /// check on steepness: a curve that gets gentler passes it trivially,
    /// and so does a compounding curve with a small enough base. This one
    /// bounds the shape instead. On a linear curve the steps are roughly
    /// constant; on a compounding one they grow without limit, so a single
    /// figure catches the whole class rather than one retune of it.
    ///
    /// Set at 3 against a shipped spread of 8 to 16 — real margin for the
    /// integer search's lumpiness, and nowhere near the 5x a geometric
    /// curve reaches within five zones, let alone ten.
    const LINEAR_STEP_GUARD_MULTIPLIER: u32 = 3;

    /// Multiplier and flat slack the "no cliff" guard allows a zone's level
    /// requirement to grow by over the previous zone's. Growth is
    /// geometric in stats *and* group size, not the old fixed-size-pack
    /// game's flat ~2x-per-zone, so the shipped curve runs 1.9x-2.3x from
    /// zone 2 down, and up to 4.0x on the geared sweep. `* 6 + 10` is
    /// exactly double the multiplier and slack this guard used to gate on —
    /// real margin against every shipped pair, including the ones that used
    /// to sit exactly on the wire, while still catching a jump twice as
    /// sharp as anything that ships today.
    const LEVEL_GROWTH_GUARD_MULTIPLIER: u32 = 6;
    const LEVEL_GROWTH_GUARD_SLACK: u32 = 10;

    /// The zone 1 -> 2 pair is exempt from the growth guard, and only from
    /// that half of it — the monotonicity check still covers every pair.
    ///
    /// Zone 1 fields one program at a time (`zone_group_cap(1)` is 1, and
    /// `max_enemy_groups` allows one group), so "needs level 1" measures an
    /// intentionally empty tutorial zone rather than a difficulty. Six times
    /// almost nothing is still almost nothing: the guard hands zone 2 a
    /// ceiling of 16 whatever the roster actually says, and the shipped
    /// curve has sat one level under that wire. That is a property of the
    /// floor, not of the curve — every pair from zone 2 on runs at ~2x
    /// against a ceiling in the hundreds.
    ///
    /// Exempting it costs nothing this guard was catching: a real zone-2
    /// cliff shows up in the 2 -> 3 step it feeds, which is still gated.
    const GROWTH_GUARD_FIRST_MEASURED_PAIR: usize = 1;

    /// The party-size change compounds three ways: `party_stat_bonus`
    /// feeds a share of every companion's ATK/DEF into the player's own
    /// effective stats, so 3 -> 5 raises the player's *passive* stats as
    /// well as adding two more attackers and two more bodies to absorb
    /// hits. The group-size increase is the counterweight. This test is the
    /// only evidence that ratio is survivable before anyone plays it — it
    /// fields `MAX_PARTY_SIZE` companions, unlike the progression sweeps
    /// below, which field `BASE_PET_CAPACITY`; the full-roster ratio is
    /// exactly what this test exists to check.
    ///
    /// Deliberately fought against the *toughest* ordinary species with a
    /// *median* party — a player tames what the habitat gives them, and has
    /// to survive the worst thing it spawns.
    ///
    /// Swept through `MAX_GEARED_ZONE_SWEPT` zones, matching the range the
    /// geared progression sweep below covers.
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

        for zone in 1..=MAX_GEARED_ZONE_SWEPT {
            let Some((level, outcome)) = min_level_to_clear_zone(
                toughest,
                party,
                zone,
                MAX_LEVEL_SEARCHED,
                MAX_PARTY_SIZE,
                true,
                (weapon, armor),
            ) else {
                panic!(
                    "zone {zone}: a full party of {MAX_PARTY_SIZE} can't clear a full group of \
                     {}s at any level up to {MAX_LEVEL_SEARCHED} — the group/party ratio is off",
                    toughest.name
                );
            };
            eprintln!(
                "[roster] zone {zone}: full party of {MAX_PARTY_SIZE} {}s vs group of {} {}s \
                 needs level {level} ({} rounds, {:.0}% player HP left)",
                party.name,
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
    /// The measurement is `reach_rule_verdict`, which is shipping code
    /// because the roster tuner post-checks a winning proposal against it.
    /// Asserting through it rather than restating it is what keeps the two
    /// from drifting, and the drifting copy would be the tuner's — the one
    /// nobody runs.
    #[test]
    fn the_reach_rule_measurably_softens_a_full_pack() {
        let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        let verdict = reach_rule_verdict(&db);
        eprintln!("{verdict}");
        assert!(
            verdict.holds(),
            "the reach rule must lower the level a full pack demands, or ENGAGED_GROUPS is \
             buying nothing: {verdict}"
        );
    }

    /// Pure-grind floor: no gear equipped, ever. Confirms the level
    /// required to clear a zone with a full (leveled, zone-caught) party
    /// grows roughly geometrically with zone depth — expected, since wild
    /// stats double per zone against flat linear player growth — and
    /// catches any *sharper* blowup than that as a regression.
    /// Asserts the level curve rises in roughly constant steps rather than
    /// compounding — the property that makes a zone ceiling fundable at all.
    ///
    /// Separate from the ratio guard, which only bounds steepness and so is
    /// blind to the shape: 1, 2, 4, 8, 16 and 1, 2, 3, 4, 5 both satisfy
    /// "never more than 6x the previous", and only one of them ends.
    fn assert_steps_stay_flat(required_levels: &[u32], what: &str) {
        let steps: Vec<u32> = required_levels
            .windows(2)
            .skip(GROWTH_GUARD_FIRST_MEASURED_PAIR)
            .map(|w| w[1] - w[0])
            .collect();
        let (smallest, largest) = (
            *steps.iter().min().expect("a swept curve has steps"),
            *steps.iter().max().expect("a swept curve has steps"),
        );
        assert!(
            largest <= smallest * LINEAR_STEP_GUARD_MULTIPLIER,
            "{what} level curve steps run {smallest}..{largest} — a step that \
             grows with the zone is a compounding curve, which has a zone \
             past which no reachable level clears it: {required_levels:?} \
             (steps {steps:?})"
        );
    }

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
                BASE_PET_CAPACITY,
                false,
                (weapon, armor),
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
                BASE_PET_CAPACITY,
                party.name,
                outcome.turns,
                outcome.player_hp_fraction * 100.0
            );
            required_levels.push(level);
        }

        for (i, pair) in required_levels.windows(2).enumerate() {
            let (prev, next) = (pair[0], pair[1]);
            assert!(
                next >= prev,
                "deeper zones should never require a *lower* level to clear: {required_levels:?}"
            );
            if i < GROWTH_GUARD_FIRST_MEASURED_PAIR {
                continue;
            }
            assert!(
                next <= prev * LEVEL_GROWTH_GUARD_MULTIPLIER + LEVEL_GROWTH_GUARD_SLACK,
                "level requirement jumped from {prev} to {next} one zone deeper — sharper than \
                 the shipped curve's growth in stats and group size ever produces: \
                 {required_levels:?}"
            );
        }
        assert_steps_stay_flat(&required_levels, "gear-free");
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
                BASE_PET_CAPACITY,
                true,
                (weapon, armor),
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
                BASE_PET_CAPACITY,
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
                BASE_PET_CAPACITY,
                false,
                (weapon, armor),
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
                next <= prev * LEVEL_GROWTH_GUARD_MULTIPLIER + LEVEL_GROWTH_GUARD_SLACK,
                "geared level requirement jumped from {prev} to {next} one zone deeper — \
                 sharper than the shipped curve's growth in stats and group size ever \
                 produces: {required_levels:?}"
            );
        }
        assert_steps_stay_flat(&required_levels, "geared");
    }
}
