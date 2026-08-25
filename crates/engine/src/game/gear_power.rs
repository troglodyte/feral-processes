//! One absolute power scalar for a gear copy, and its breakdown.
//!
//! **`Game::copy_power` is the one door to a rating**, and every term in it
//! is a *call* into the formula that already governs that axis —
//! `Game::copy_bonus` for what the copy is worth, `Stats::power` for what
//! attack and mitigation buy, `battle::hit_chance` for what accuracy and
//! evasion buy. Nothing here restates any of them. Four screens have already
//! silently dropped the affix by rebuilding `copy_bonus`'s chain by hand;
//! this module would drop it in a fifth place if it re-walked the axes.
//!
//! The rating is **absolute**: every copy is priced against one fixed
//! reference wearer (`tuning`'s power reference block) rather than against
//! whoever is holding it, so the same copy shows the same number on the
//! inventory list, a trader's shelf and the swap picker. The swap picker's
//! *delta* is a different question and may legitimately disagree — gear
//! locks in `EquippedItem::level`, so the worn piece and the candidate are
//! scaled at two different levels.

use crate::Game;
use crate::SpeciesDb;
use crate::components::Stats;
use crate::items::{EquipmentStats, GearCopy};
use crate::tuning::{
    PLAYER_BASE_SPEED, POWER_REFERENCE_ATK, POWER_REFERENCE_DAMAGE, POWER_REFERENCE_LEVEL,
    POWER_REFERENCE_MAX_HP, POWER_REFERENCE_MITIGATION, POWER_REFERENCE_ZONE,
};
use crate::views::ItemPower;

/// The reference wearer with nothing on.
fn bare() -> Stats {
    Stats {
        hp: POWER_REFERENCE_MAX_HP,
        max_hp: POWER_REFERENCE_MAX_HP,
        atk: POWER_REFERENCE_ATK,
        mitigation: POWER_REFERENCE_MITIGATION,
    }
}

/// What the reference wearer swings for, in `Stats::power`'s own attack
/// currency — the throughput an accuracy bonus multiplies.
fn reference_offense() -> f64 {
    POWER_REFERENCE_ATK as f64 + POWER_REFERENCE_DAMAGE.mean()
}

/// What the reference wearer soaks, which is what an evasion bonus
/// multiplies. `Stats::power` minus its attack half, so the split cannot
/// drift from the function it comes out of.
fn reference_soak() -> f64 {
    (bare().power() - POWER_REFERENCE_ATK) as f64
}

/// Rates `mods` against the reference wearer, facing a hostile with
/// `foe_accuracy` and `foe_evasion`.
///
/// Split out from `Game::copy_power` so each of the four terms can be
/// exercised on its own axis — a shipped item mixes them, and a term nothing
/// catches is a term that can be deleted later by accident.
///
/// `None` when the copy pays nothing on any priced axis. That is "there is
/// no answer", not "the answer is zero": `EquipmentStats::decompiler` buys
/// taming rather than combat and gets **no term**, so a Decompiler module
/// rates `None` however large its number.
pub(crate) fn rate(mods: EquipmentStats, foe_accuracy: f64, foe_evasion: f64) -> Option<ItemPower> {
    if mods.atk == 0
        && mods.mitigation == 0
        && mods.accuracy == 0
        && mods.evasion == 0
        && mods.damage == crate::battle::DamageRange::default()
    {
        return None;
    }

    let bare = bare();
    let baseline = bare.power();

    // Each axis moved on its own, so the sum stays a breakdown rather than a
    // single opaque delta. Attack enters `Stats::power` raw; mitigation is
    // priced as the effective HP it buys, which is the whole reason a
    // percentage may not simply be summed into a total.
    let offense_stat = Stats {
        atk: bare.atk + mods.atk,
        ..bare
    }
    .power()
        - baseline;
    let survivability = Stats {
        mitigation: bare.mitigation + mods.mitigation,
        ..bare
    }
    .power()
        - baseline;

    // A weapon **replaces** the natural band rather than adding to it
    // (`Game::attack_range`), so this is a difference and a worse band than
    // the reference wearer's fists is worth a negative offense.
    let band = if mods.damage == crate::battle::DamageRange::default() {
        0
    } else {
        (mods.damage.mean() - POWER_REFERENCE_DAMAGE.mean()).round() as i32
    };

    // Accuracy and evasion are **proportional**. A probability is not a
    // quantity and must never be summed into the total, so each is priced as
    // the fraction it moves the throughput it acts on.
    let hit = |acc: f64, eva: f64| crate::battle::hit_chance(acc, eva);
    let ref_accuracy = crate::battle::accuracy_of(PLAYER_BASE_SPEED, POWER_REFERENCE_LEVEL, 0);
    let ref_evasion = crate::battle::evasion_of(PLAYER_BASE_SPEED, POWER_REFERENCE_LEVEL, 0);

    let accuracy = reference_offense()
        * (hit(ref_accuracy + mods.accuracy as f64, foe_evasion) / hit(ref_accuracy, foe_evasion)
            - 1.0);
    let evasion = reference_soak()
        * (hit(foe_accuracy, ref_evasion) / hit(foe_accuracy, ref_evasion + mods.evasion as f64)
            - 1.0);

    let offense = offense_stat + band;
    let accuracy = accuracy.round() as i32;
    let evasion = evasion.round() as i32;
    Some(ItemPower {
        total: offense + survivability + accuracy + evasion,
        offense,
        survivability,
        accuracy,
        evasion,
    })
}

impl Game {
    /// What one carried copy is worth in combat, absolutely — see the module
    /// doc for why absolute and not relative to the holder.
    ///
    /// `None` for anything with no combat axis at all: a consumable, a
    /// material, a Decompiler module. The caller draws an em dash, which
    /// says "no answer" where a `0` would claim the piece was rated and
    /// found worthless.
    pub fn copy_power(&self, copy: &GearCopy) -> Option<ItemPower> {
        // Scaled at the reference *zone*, not the reference level: gear level
        // is the zone (`Game::equip` caps it there), and rating a copy at a
        // level no copy can ever reach would flatter every piece equally.
        let mods = self.copy_bonus(copy, POWER_REFERENCE_ZONE)?;
        let median =
            crate::balance_sim::median_ordinary_species(self.world.resource::<SpeciesDb>());
        rate(
            mods,
            crate::battle::accuracy_of(median.base_speed, POWER_REFERENCE_ZONE, 0),
            crate::battle::evasion_of(median.base_speed, POWER_REFERENCE_ZONE, 0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A nominal hostile in the region the shipped roster sits in, so the
    /// proportional terms are exercised against a real matchup rather than
    /// against parity.
    const FOE_ACC: f64 = 12.5;
    const FOE_EVA: f64 = 12.5;

    fn rated(mods: EquipmentStats) -> ItemPower {
        rate(mods, FOE_ACC, FOE_EVA).expect("a copy paying on one axis rates")
    }

    #[test]
    fn an_atk_only_copy_rates_its_atk() {
        let power = rated(EquipmentStats {
            atk: 9,
            ..Default::default()
        });
        assert_eq!(power.offense, 9);
        assert_eq!(power.total, 9);
    }

    /// Mitigation is percentage points. Summed into the total as its raw
    /// number it would be meaningless; priced as the effective HP it buys
    /// against a real HP pool it is worth far more than its face value.
    #[test]
    fn a_mitigation_only_copy_rates_more_than_its_raw_number() {
        let mods = EquipmentStats {
            mitigation: 5,
            ..Default::default()
        };
        let power = rated(mods);
        assert!(
            power.survivability > mods.mitigation,
            "5 mitigation points rated {} — that is the raw number, not the soak it buys",
            power.survivability
        );
        assert_eq!(power.total, power.survivability);
    }

    /// A weapon **overrides** the natural attack. A band worse than the
    /// reference wearer's fists is a downgrade, and nothing else in the
    /// formula can say so.
    #[test]
    fn a_weapon_below_the_reference_band_rates_negative_offense() {
        let power = rated(EquipmentStats {
            damage: crate::battle::DamageRange { min: 1, max: 2 },
            ..Default::default()
        });
        assert!(
            power.offense < 0,
            "a 1-2 band against the reference {:?} rated {} — the band is being added, not \
             substituted",
            POWER_REFERENCE_DAMAGE,
            power.offense
        );
    }

    #[test]
    fn an_accuracy_only_copy_rates_on_the_offense_it_multiplies() {
        let mods = EquipmentStats {
            accuracy: 5,
            ..Default::default()
        };
        let power = rated(mods);
        assert!(power.accuracy > 0, "accuracy rated {}", power.accuracy);
        assert_ne!(
            power.accuracy, mods.accuracy,
            "the accuracy term is its raw number — a probability is being summed as a quantity"
        );
        assert_eq!(power.total, power.accuracy);
        // Twice the accuracy is worth more, but not twice as much: the term
        // is a proportion of a fixed throughput, not a multiple of the stat.
        let doubled = rated(EquipmentStats {
            accuracy: 10,
            ..Default::default()
        });
        assert!(doubled.accuracy > power.accuracy);
        assert!(doubled.accuracy < power.accuracy * 2);
    }

    #[test]
    fn an_evasion_only_copy_rates_on_the_soak_it_multiplies() {
        let mods = EquipmentStats {
            evasion: 5,
            ..Default::default()
        };
        let power = rated(mods);
        assert!(power.evasion > 0, "evasion rated {}", power.evasion);
        assert_ne!(power.evasion, mods.evasion);
        assert_eq!(power.total, power.evasion);
    }

    /// **The rating is absolute, and that is what makes one figure mean the
    /// same thing on six screens.** It takes no wearer and reads no
    /// `ZoneLevel`, so the same copy rates the same in zone 1 and in zone 10.
    ///
    /// The regression to head off is someone "fixing" the column to be
    /// contextual because it disagrees with the swap picker's delta — those
    /// two answer different questions. The delta is a property of the *swap*
    /// (gear locks in `EquippedItem::level`, so the worn piece and the
    /// candidate are scaled at two different levels); this is a property of
    /// the *copy*.
    #[test]
    fn a_copys_rating_does_not_move_with_the_zone() {
        let mut game = Game::new(
            934,
            crate::DifficultyMode::Forgiving,
            &crate::tests::support::test_assets_dir(),
        )
        .unwrap();
        let copy = GearCopy::plain(crate::items::ItemId::from(
            crate::items::ids::MONOFILAMENT_WHIP,
        ));
        let at_zone_one = game.copy_power(&copy).expect("a weapon rates");
        game.world.resource_mut::<crate::resources::ZoneLevel>().0 = 10;
        assert_eq!(game.copy_power(&copy), Some(at_zone_one));
    }

    /// A Decompiler module buys taming, not combat. There is no answer here,
    /// which is a different thing from an answer of zero.
    #[test]
    fn a_decompiler_only_copy_rates_nothing() {
        assert!(
            rate(
                EquipmentStats {
                    decompiler: 40,
                    ..Default::default()
                },
                FOE_ACC,
                FOE_EVA,
            )
            .is_none()
        );
    }
}
