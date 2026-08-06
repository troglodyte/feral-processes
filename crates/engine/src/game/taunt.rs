//! Saying something at the wild group.
//!
//! Bound to a key no screen in the game names — see
//! `crates/engine/EASTER_EGGS.md`, and the design argument in
//! `docs/superpowers/specs/2026-08-06-easter-eggs-design.md`.
//!
//! Pure flavour: no turn, no round, no effect on the fight. Nudging
//! anything real — `capture_chance` was the obvious candidate — would put a
//! hidden key on taming, which is an axis `balance_sim` models nothing of.

use crate::resources::TauntCount;
use crate::*;

/// What a program with nothing authored says. The key must never silently
/// do nothing, and every shipped species carries no lines until somebody
/// writes them, so this is the common case rather than the edge one.
const GENERIC_TAUNTS: [&str; 3] = [
    "emits a burst of noise on a frequency nothing out there is listening on.",
    "cycles its fans up to a whine and holds it there.",
    "prints something unrepeatable to a console that isn't attached.",
];

impl Game {
    /// Goads the party's front rank into saying something at the wild
    /// group.
    ///
    /// Logged as `MessageKind::Info` on purpose:
    /// `MessageLog::retain_outcomes_since_battle` keeps only `Outcome`,
    /// `Loot`, `LevelUp` and `Raid`, so a taunt is pruned when the fight
    /// ends and does not follow the player back onto the map. Trash talk
    /// belongs to the fight it was said in.
    pub fn taunt(&mut self) -> Result<(), String> {
        if !self.has_active_battle() {
            return Err("Nothing out there to say it to.".into());
        }
        // The front living companion, or the player if the party is empty
        // — `living_party` is in slot order with the player at slot 0, so
        // "the first one who isn't you" is the front rank.
        let player = self.player_entity();
        let speaker = self
            .living_party()
            .into_iter()
            .find(|&e| e != player)
            .unwrap_or(player);

        let lines = self.taunt_lines(speaker);
        // Defaulted in place rather than inserted by the constructors, so
        // there is exactly one place that knows this counter exists and no
        // chance of it drifting into the save struct. See `TauntCount`.
        let count = self.world.get_resource::<TauntCount>().map_or(0, |c| c.0);
        self.world
            .insert_resource(TauntCount(count.wrapping_add(1)));
        let line = &lines[count as usize % lines.len()];

        let name = if speaker == player {
            "You".to_string()
        } else {
            self.creature_label(speaker)
        };
        self.log_kind(MessageKind::Info, format!("{name} {line}"));
        Ok(())
    }

    /// What `speaker` has to say, falling back to the generic lines for a
    /// species that authors none — and for the player, who has no species
    /// at all.
    fn taunt_lines(&self, speaker: Entity) -> Vec<String> {
        self.world
            .get::<Creature>(speaker)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|def| def.taunts.clone())
            .filter(|lines| !lines.is_empty())
            .unwrap_or_else(|| GENERIC_TAUNTS.iter().map(|l| l.to_string()).collect())
    }
}
