/// Damage always deals at least 1, so battles can't stall out on high-defense
/// matchups.
pub fn compute_damage(atk: i32, def: i32, move_power: i32) -> i32 {
    (move_power + atk - def).max(1)
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
}
