use crate::items::ItemId;

/// How effective an item is as a base multiplier in [`capture_chance`].
pub fn item_potency(item: ItemId) -> f32 {
    match item {
        ItemId::IceBreaker => 0.55,
        ItemId::CoreFragment
        | ItemId::PowerCell
        | ItemId::OverclockCore
        | ItemId::FirewallPlating
        | ItemId::NeuralAmplifier
        | ItemId::PortalFragment
        | ItemId::MonofilamentWhip
        | ItemId::AblativePlating
        | ItemId::CortexHack => 0.0,
    }
}

/// Percentage-point bonus to decompile chance per point of the player's
/// `Decompiler` stat (see `components::Decompiler`).
const DECOMPILER_SKILL_BONUS: f32 = 0.03;

/// ICE-breaking odds: weaker (lower `hp_fraction`) and easier-compiled
/// species are more likely to be decompiled; stronger breakers help; a more
/// practiced player (`decompiler_skill`) adds a flat bonus on top.
pub fn capture_chance(hp_fraction: f32, item_potency: f32, taming_difficulty: f32, decompiler_skill: i32) -> f32 {
    let base = item_potency * (1.05 - hp_fraction * 0.65) * (1.0 - taming_difficulty * 0.6);
    let skill_bonus = decompiler_skill as f32 * DECOMPILER_SKILL_BONUS;
    (base + skill_bonus).clamp(0.05, 0.95)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weaker_prey_is_easier_to_tame() {
        let full_hp = capture_chance(1.0, 0.55, 0.2, 0);
        let low_hp = capture_chance(0.1, 0.55, 0.2, 0);
        assert!(low_hp > full_hp);
    }

    #[test]
    fn harder_species_resist_taming() {
        let easy = capture_chance(0.5, 0.55, 0.1, 0);
        let hard = capture_chance(0.5, 0.55, 0.9, 0);
        assert!(hard < easy);
    }

    #[test]
    fn higher_decompiler_skill_improves_odds() {
        let unskilled = capture_chance(0.5, 0.55, 0.5, 0);
        let skilled = capture_chance(0.5, 0.55, 0.5, 10);
        assert!(skilled > unskilled);
    }

    #[test]
    fn chance_is_always_within_bounds() {
        for hp in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for diff in [0.0, 0.5, 1.0] {
                for skill in [0, 5, 50] {
                    let c = capture_chance(hp, 0.55, diff, skill);
                    assert!((0.05..=0.95).contains(&c), "out of bounds: {c}");
                }
            }
        }
    }
}
