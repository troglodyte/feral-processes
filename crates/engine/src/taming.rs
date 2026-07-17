use crate::items::ItemId;

/// How effective an item is as a base multiplier in [`capture_chance`].
pub fn item_potency(item: ItemId) -> f32 {
    match item {
        ItemId::IceBreaker => 0.55,
        ItemId::CoreFragment | ItemId::PowerCell => 0.0,
    }
}

/// ICE-breaking odds: weaker (lower `hp_fraction`) and easier-compiled
/// species are more likely to be decrypted; stronger breakers help.
pub fn capture_chance(hp_fraction: f32, item_potency: f32, taming_difficulty: f32) -> f32 {
    let base = item_potency * (1.05 - hp_fraction * 0.65) * (1.0 - taming_difficulty * 0.6);
    base.clamp(0.05, 0.95)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weaker_prey_is_easier_to_tame() {
        let full_hp = capture_chance(1.0, 0.55, 0.2);
        let low_hp = capture_chance(0.1, 0.55, 0.2);
        assert!(low_hp > full_hp);
    }

    #[test]
    fn harder_species_resist_taming() {
        let easy = capture_chance(0.5, 0.55, 0.1);
        let hard = capture_chance(0.5, 0.55, 0.9);
        assert!(hard < easy);
    }

    #[test]
    fn chance_is_always_within_bounds() {
        for hp in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for diff in [0.0, 0.5, 1.0] {
                let c = capture_chance(hp, 0.55, diff);
                assert!((0.05..=0.95).contains(&c), "out of bounds: {c}");
            }
        }
    }
}
