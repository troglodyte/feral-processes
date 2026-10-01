//! What the `Sulking` rung does: the predicate every behaviour reads, and the
//! freeze-out half of it.

use super::respite::sulk;
use super::support::*;
use crate::components::{Disgruntled, Grievance, Position};
use crate::*;

#[test]
fn sulks_is_true_from_the_mild_rung_up_and_false_below_it() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let who = spawn_tamed(&mut game, 10, 3);
    assert!(!game.sulks(who), "no marker, no sulk");

    sulk(&mut game, who);
    assert!(game.sulks(who));
    for grievance in [Grievance::DownedTools, Grievance::LashingOut] {
        game.world.get_mut::<Disgruntled>(who).unwrap().grievance = grievance;
        assert!(game.sulks(who), "{grievance:?} still sulks");
    }
}

#[test]
fn is_beside_is_chebyshev_one() {
    use crate::situations::is_beside;
    let at = |x, y| Position { x, y };
    assert!(is_beside(at(0, 0), at(1, 1)));
    assert!(is_beside(at(0, 0), at(0, 0)));
    assert!(!is_beside(at(0, 0), at(2, 0)));
    assert!(!is_beside(at(0, 0), at(1, -2)));
}
