//! What a crafted weapon's exploit does to one swing: the pure half of the
//! combat hook. The caller owns the gathering (does the target carry the
//! status?) and the applying; `Game::apply_damage*` stay the only damage
//! path and see only the number this returns.

/// `rolled` with an exploit of `percent` applied: the bonus is rounded up,
/// so a payoff that has something to exploit is never worth nothing on a
/// small band. A hit that did no damage gains none.
pub fn exploited(rolled: i32, percent: u32) -> i32 {
    if rolled <= 0 {
        return rolled;
    }
    rolled + (rolled * percent as i32 + 99) / 100
}

#[cfg(test)]
mod tests {
    use super::exploited;

    #[test]
    fn the_bonus_rounds_up_and_a_miss_gains_nothing() {
        assert_eq!(exploited(10, 20), 12);
        assert_eq!(exploited(3, 20), 4, "0.6 rounds up to one");
        assert_eq!(exploited(0, 40), 0);
        assert_eq!(exploited(-2, 40), -2);
    }
}
