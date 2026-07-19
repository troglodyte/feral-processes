/// Damage always deals at least 1, so battles can't stall out on high-defense
/// matchups.
pub fn compute_damage(atk: i32, def: i32, move_power: i32) -> i32 {
    (move_power + atk - def).max(1)
}

/// Below this Power ("Power" is the player-facing label for `Needs.hunger`)
/// threshold, the player's own attacks start losing effectiveness — see
/// `power_attack_multiplier`.
pub const LOW_POWER_ATTACK_THRESHOLD: f32 = 50.0;

/// Multiplier applied to the player's attack total once their Power drops
/// below `LOW_POWER_ATTACK_THRESHOLD`: full strength at the threshold and
/// above, falling off linearly to half strength at 0 power. A separate,
/// milder penalty from the flat HP drain that already kicks in once power
/// hits exactly 0 (see `systems::needs_decay_system`) — this one's felt in
/// combat well before you're actually starving.
pub fn power_attack_multiplier(hunger: f32) -> f32 {
    if hunger >= LOW_POWER_ATTACK_THRESHOLD {
        1.0
    } else {
        0.5 + (hunger.max(0.0) / LOW_POWER_ATTACK_THRESHOLD) * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_scales_with_power_and_attack() {
        let low = compute_damage(4, 2, 5);
        let high = compute_damage(9, 2, 5);
        assert!(high > low);
    }

    #[test]
    fn damage_never_drops_below_one() {
        assert_eq!(compute_damage(1, 50, 2), 1);
    }

    #[test]
    fn power_attack_multiplier_is_full_strength_at_and_above_the_threshold() {
        assert_eq!(power_attack_multiplier(50.0), 1.0);
        assert_eq!(power_attack_multiplier(100.0), 1.0);
    }

    #[test]
    fn power_attack_multiplier_falls_off_linearly_below_the_threshold() {
        assert!((power_attack_multiplier(25.0) - 0.75).abs() < f32::EPSILON);
        assert!((power_attack_multiplier(0.0) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn power_attack_multiplier_never_drops_below_half() {
        assert_eq!(power_attack_multiplier(-10.0), 0.5);
    }
}
