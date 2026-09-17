//! Playing a whole fight out with no pacing — the engine half of `[R]`.

use crate::tactical::TacticalBattle;
use crate::tuning::AUTO_RESOLVE_ROUND_CAP;
use crate::*;

/// What `Game::auto_resolve_battle` came back with.
///
/// Two states because the loop's own stop rule
/// (`!has_active_battle() || is_game_over().is_some()`) and its round cap
/// answer different questions: `Finished` says the fight is over and the
/// results are the caller's to show; `Stalled` says the cap was reached
/// with the fight still open, which `[R]`'s caller reads as "couldn't
/// settle it — finish by hand."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoResolve {
    Finished,
    Stalled,
}

impl Game {
    /// Plays the open fight out to its end with no pacing, whichever combat
    /// model holds it, and reports whether it closed.
    ///
    /// `auto_resolve_battle_with` with a hook that does nothing — see there
    /// for the loop itself. This door is every caller but one: the arena's
    /// `[R]` half needs to hear each round as it happens, and is the reason
    /// the hook exists at all.
    pub fn auto_resolve_battle(&mut self) -> AutoResolve {
        self.auto_resolve_battle_with(|_| {})
    }

    /// `auto_resolve_battle`, calling `after_round` once for every round
    /// that actually resolved — including the one that ends the fight.
    ///
    /// **Which model is open is read off `TacticalBattle`'s presence, not
    /// passed in**: a `TacticalBattle` resource means a battle map, and its
    /// absence with a fight open means the group model. Each step is that
    /// model's own existing driver — `tactical_drive_turn` for a battle
    /// map, `battle_auto_round` (this file's sibling in `combat_round.rs`)
    /// for the group model — so a fight `[R]` plays out is the fight `[A]`
    /// would have played out, never a third decision-maker invented for
    /// this door.
    ///
    /// **The stop rule is never the player's HP.** A Forgiving defeat is
    /// rebooted inside the round that lands it, so by the time this loop
    /// could look the player is alive again; `is_game_over` is the
    /// Permadeath half, and `battle_resolve_round` (through
    /// `battle_auto_round`) does nothing forever once that is set. This is
    /// `arena::run::run_rep`'s own stop rule, reused rather than restated.
    ///
    /// **The round is read only right after that stop check passes.** A
    /// fight that just closed has no round — `fight_round()` answers
    /// `None` — and reading that as round 0 would corrupt the cap
    /// arithmetic (a `u32` underflow against `start`) instead of simply
    /// never being reached, because the stop check above already returned.
    ///
    /// **The hook fires on a round, never on a step**, which is not the
    /// same thing: `battle_auto_round` reports a step *as* a round, so the
    /// two agree for the group model, but `tactical_drive_turn` reports a
    /// step for every body's *turn*, and a turn is not a round. Gating the
    /// call on `fight_round()` actually moving (or the fight ending inside
    /// the step) is `run_tactical_rep`'s own rule for the same reason: a
    /// `Watch` counts one call as one round, and the arena's published
    /// numbers rest on this loop counting the same way whether the fight
    /// was paced by hand or played out here in one call.
    pub fn auto_resolve_battle_with(&mut self, mut after_round: impl FnMut(&Game)) -> AutoResolve {
        let Some(start) = self.fight_round() else {
            return AutoResolve::Finished;
        };
        loop {
            if !self.has_active_battle() || self.is_game_over().is_some() {
                return AutoResolve::Finished;
            }
            let round = self
                .fight_round()
                .expect("has_active_battle just confirmed a fight is open");
            if round - start >= AUTO_RESOLVE_ROUND_CAP {
                return AutoResolve::Stalled;
            }
            let stepped = if self.world.get_resource::<TacticalBattle>().is_some() {
                self.tactical_drive_turn()
            } else {
                self.battle_auto_round()
            };
            if !stepped {
                // A step can report nothing happened either because the
                // fight ended inside it or because nothing was left to
                // drive with the fight still open (an unreachable target,
                // say) — the two cases this return can't tell apart from
                // in here, and doesn't need to.
                return if !self.has_active_battle() || self.is_game_over().is_some() {
                    AutoResolve::Finished
                } else {
                    AutoResolve::Stalled
                };
            }
            if !self.has_active_battle() || self.fight_round() != Some(round) {
                after_round(self);
            }
        }
    }
}
