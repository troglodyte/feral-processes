//! Throwing a consumable at the wild group instead of using it.
//!
//! Bound to a key no screen in the game names — see
//! `crates/engine/EASTER_EGGS.md`, and the design argument in
//! `docs/superpowers/archive/specs/2026-08-06-easter-eggs-design.md`.

use crate::resources::BattleState;
use crate::tuning::THROWN_ITEM_DAMAGE;
use crate::*;

impl Game {
    /// Spends one unit of `item` bouncing it off the front of the wild
    /// group.
    ///
    /// **Resolves immediately, and is not a `BattleAction`.** An action
    /// kind would have to appear in `battle_action_options`, which is the
    /// list both renderers build the round prompt from — the key would be
    /// printed on screen and stop being a secret. That is the whole reason
    /// this is not modelled as an action, and the reason it costs no round.
    ///
    /// **A throw cannot take a target below 1 HP.** Not squeamishness: a
    /// kill resolving from outside the round loop would end a battle right
    /// beside `BattleState::planned`'s positional indexing into `Party` and
    /// the deferred removal `end_battle` exists to do. Clamping here makes
    /// that state unreachable rather than merely unlikely, the same way a
    /// lethal Wild Jump never writes `Locale`. Mitigation inside
    /// `apply_damage` only ever reduces the figure further, so clamping at
    /// the call site is sufficient.
    ///
    /// The damage goes through `apply_damage` rather than writing
    /// `Stats::hp`, because that is the one path in the engine that lowers
    /// a creature's HP and anything watching damage has to see this too.
    pub fn throw_item(&mut self, item: &ItemId) -> Result<(), String> {
        if !self.has_active_battle() {
            return Err("Nothing to throw it at.".into());
        }
        let target = self
            .front_of_the_wild_side()
            .ok_or_else(|| "Nothing to throw it at.".to_string())?;
        let player = self.player_entity();
        let spent = self
            .world
            .get_mut::<Inventory>(player)
            .ok_or_else(|| "You aren't carrying that.".to_string())?
            .take(item.clone(), 1);
        if spent == 0 {
            return Err("You aren't carrying that.".into());
        }

        // Past every refusal: the item is gone and something gets hit.
        let hp = self.world.get::<Stats>(target).map(|s| s.hp).unwrap_or(0);
        let damage = THROWN_ITEM_DAMAGE.min(hp - 1).max(0);
        self.apply_damage(target, damage);

        let name = self.item_name(item);
        let who = self.creature_label(target);
        self.log_kind(
            MessageKind::Outcome,
            format!("You throw a {name} at {who}. It bounces off."),
        );
        Ok(())
    }

    /// The front living program of the first wild group that still has one
    /// — the same target an ordinary attack on group A would find.
    fn front_of_the_wild_side(&self) -> Option<Entity> {
        self.world
            .get_resource::<BattleState>()?
            .groups
            .iter()
            .find_map(|group| {
                group
                    .members
                    .iter()
                    .copied()
                    .find(|&e| self.creature_alive(e))
            })
    }
}
