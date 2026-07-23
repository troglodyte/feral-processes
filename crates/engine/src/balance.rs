//! Offline balance projections for level/zone scaling: pure, deterministic
//! simulations of the player (plus a full party) fighting zone-scaled wild
//! species, decoupled from the ECS so they run as fast regression tests.
//! See the `zone_scaling_stays_survivable_with_a_full_party` test for the
//! actual regression check this module exists to support.

use crate::battle::compute_damage;
use crate::components::{PLAYER_BASE_STATS, Stats};
use crate::items::EquipmentStats;
use crate::progression::stats_after_levels;
use crate::resources::{MAX_PARTY_SIZE, ZoneLevel};
use crate::species::{SpeciesDb, SpeciesDef};

/// Rounds to let a simulated fight run before scoring it a loss —
/// generously above any realistic fight length, so it only fires on a
/// genuine stalemate (defense permanently outpacing attack, short of
/// `compute_damage`'s floor of 1).
const TURN_CAP: u32 = 300;

/// Round cadence for commanding a companion to rally instead of attacking:
/// matches the real default rally's `RALLY_DURATION` (3 rounds, see
/// `crate::RALLY_DURATION`) so the buff is refreshed right as it expires.
const RALLY_CADENCE: u32 = 4;

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

pub struct BattleOutcome {
    pub player_won: bool,
    pub turns: u32,
    pub player_hp_fraction: f32,
}

/// Deterministic turn-based simulation of a player (commanding up to
/// `companions.len()` party members) fighting one wild creature, mirroring
/// `Game::battle_attack` / `battle_command_companion` / `wild_retaliate`
/// closely enough for balance projections:
///
/// - The player attacks with the real fixed move power (5) every round,
///   except every `RALLY_CADENCE`th round, when it instead commands its
///   strongest companion for the generic ATK rally (a third of that
///   companion's ATK, for 3 rounds) — the fallback every companion species
///   without a `special_ability` gets.
/// - The wild creature retaliates against the player every round with its
///   `average_move_power` — a simplification of the real random move pick
///   and the real chance to hit a companion instead
///   (`COMPANION_RETALIATION_CHANCE`). Always targeting the player is the
///   conservative case: in the real game some hits land on a companion
///   instead, which only helps the player's own HP hold out longer.
///
/// Runs for at most `TURN_CAP` rounds; a fight that hasn't resolved by then
/// is scored as a loss — a stalemate that long isn't survivable in
/// practice (Power/Fatigue would run out first).
pub fn simulate_battle(
    mut player: Stats,
    companions: &[Stats],
    mut wild: Stats,
    wild_move_power: i32,
) -> BattleOutcome {
    let strongest_companion_atk = companions.iter().map(|c| c.atk).max().unwrap_or(0);
    let mut rally_power = 0;
    let mut rally_rounds_left = 0u32;

    for turn in 1..=TURN_CAP {
        if !companions.is_empty() && turn % RALLY_CADENCE == 0 {
            rally_power = (strongest_companion_atk / 3).max(1);
            rally_rounds_left = 3;
        } else {
            let effective_atk = player.atk
                + if rally_rounds_left > 0 {
                    rally_power
                } else {
                    0
                };
            let dmg = compute_damage(effective_atk, wild.def, 5);
            wild.hp -= dmg;
            if wild.hp <= 0 {
                return BattleOutcome {
                    player_won: true,
                    turns: turn,
                    player_hp_fraction: (player.hp as f32 / player.max_hp as f32).max(0.0),
                };
            }
        }
        rally_rounds_left = rally_rounds_left.saturating_sub(1);

        let dmg = compute_damage(wild.atk, player.def, wild_move_power);
        player.hp -= dmg;
        if player.hp <= 0 {
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
        player_hp_fraction: (player.hp as f32 / player.max_hp as f32).max(0.0),
    }
}

/// Searches player levels `1..=max_level` for the lowest one at which a
/// full party (`MAX_PARTY_SIZE` companions, all tamed from `species` while
/// breached into `zone` and leveled per
/// `companion_level_for_player_level`) beats `species` scaled to `zone` in
/// `simulate_battle`. `None` means scaling has broken down outright — not
/// just a long grind, but no level up to `max_level` clears it.
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
    species: &SpeciesDef,
    zone: u32,
    max_level: u32,
    with_gear: bool,
    weapon: EquipmentStats,
    armor: EquipmentStats,
) -> Option<(u32, BattleOutcome)> {
    let wild = wild_stats_at_zone(species, zone);
    let move_power = average_move_power(species);
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
        let companions: Vec<Stats> = (0..MAX_PARTY_SIZE)
            .map(|_| companion_stats(species, zone, companion_level))
            .collect();
        let outcome = simulate_battle(player, &companions, wild, move_power);
        if outcome.player_won {
            return Some((level, outcome));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn species_assets_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/species")
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

    /// How deep to sweep the gear-free grind baseline. Zones are actually
    /// unbounded, but wild stats double every zone
    /// (`ZoneLevel::stat_multiplier`) against the player's flat linear
    /// per-level growth, so the level needed to keep pace roughly doubles
    /// too (confirmed empirically: 7, 15, 29, 57, 111 for zones 1-5) —
    /// zone 6 alone needs north of `MAX_LEVEL_SEARCHED`. That cliff is
    /// real and expected without gear; this sweep stays inside the range
    /// where a pure-grind path is still supposed to work, so a regression
    /// in the *shape* of that curve (not its eventual end) still gets
    /// caught.
    const MAX_GRIND_ONLY_ZONE_SWEPT: u32 = 5;
    /// How deep to sweep the fully-geared scenario. `GEAR_LEVEL_GROWTH`
    /// now matches `ZoneLevel::stat_multiplier`'s doubling-per-zone base
    /// (see `items::GEAR_LEVEL_GROWTH`'s doc comment), so gear no longer
    /// overtakes and trivializes deep zones the way the old 2.5x factor
    /// did — it consistently roughly halves the level a zone needs
    /// (confirmed empirically: 4/8/16/28/53/109 geared vs. 7/15/29/57/111
    /// gear-free for zones 1-6), rather than collapsing to "level 1 clears
    /// everything" past a certain depth. It's still the same doubling
    /// curve shape, though, so it hits `MAX_LEVEL_SEARCHED` one zone later
    /// than the gear-free sweep.
    const MAX_GEARED_ZONE_SWEPT: u32 = 6;
    const MAX_LEVEL_SEARCHED: u32 = 200;

    /// Pure-grind floor: no gear equipped, ever. Confirms the level
    /// required to clear a zone with a full (leveled, zone-caught) party
    /// grows roughly geometrically with zone depth — expected, since wild
    /// stats double per zone against flat linear player growth — and
    /// catches any *sharper* blowup than that as a regression.
    #[test]
    fn grind_only_zone_scaling_grows_predictably() {
        let (db, warnings) = SpeciesDb::load_dir(&species_assets_dir()).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should all load cleanly: {warnings:?}"
        );
        let toughest = toughest_ordinary_species(&db);
        let (weapon, armor) = best_gear_stats();

        let mut required_levels = Vec::new();
        for zone in 1..=MAX_GRIND_ONLY_ZONE_SWEPT {
            let Some((level, outcome)) =
                min_level_to_clear_zone(toughest, zone, MAX_LEVEL_SEARCHED, false, weapon, armor)
            else {
                panic!(
                    "zone {zone} ({}) isn't clearable by level {MAX_LEVEL_SEARCHED} on pure grind \
                     with a full party — the curve got steeper than expected",
                    toughest.name
                );
            };
            eprintln!(
                "[no gear] zone {zone} vs {}: needs level {level} ({} turns, {:.0}% player HP left)",
                toughest.name,
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
        let (db, warnings) = SpeciesDb::load_dir(&species_assets_dir()).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should all load cleanly: {warnings:?}"
        );
        let toughest = toughest_ordinary_species(&db);
        let (weapon, armor) = best_gear_stats();

        let mut required_levels = Vec::new();
        for zone in 1..=MAX_GEARED_ZONE_SWEPT {
            let Some((geared_level, outcome)) =
                min_level_to_clear_zone(toughest, zone, MAX_LEVEL_SEARCHED, true, weapon, armor)
            else {
                panic!(
                    "zone {zone} ({}) isn't clearable by level {MAX_LEVEL_SEARCHED} even fully \
                     geared with a full party — that's a real lockout",
                    toughest.name
                );
            };
            eprintln!(
                "[geared] zone {zone} vs {}: needs level {geared_level} ({} turns, {:.0}% player \
                 HP left)",
                toughest.name,
                outcome.turns,
                outcome.player_hp_fraction * 100.0
            );
            // Only compare against the gear-free requirement where that's
            // itself known (zone <= MAX_GRIND_ONLY_ZONE_SWEPT) — beyond
            // that range gear-free is already established as unclearable
            // within MAX_LEVEL_SEARCHED (see the other test), so gear
            // being strictly *required* there is expected, not a failure.
            if let Some((grind_only_level, _)) =
                min_level_to_clear_zone(toughest, zone, MAX_LEVEL_SEARCHED, false, weapon, armor)
            {
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
            assert!(
                next <= prev * 3 + 5,
                "geared level requirement jumped from {prev} to {next} one zone deeper — the \
                 scaling curve has a cliff: {required_levels:?}"
            );
        }
    }
}
