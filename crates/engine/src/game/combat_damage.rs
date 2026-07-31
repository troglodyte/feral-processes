//! Lowering and raising a creature's HP, and what follows when it reaches
//! zero.
//!
//! `apply_damage` is the only code path in the engine that lowers HP — see
//! its doc comment. Keeping the mitigation it applies, the death line it
//! emits and the reap that clears the fallen out of their group in one file
//! is what makes that claim checkable by reading rather than by grepping.

use crate::*;

impl Game {
    /// Applies `dmg` to `target`, floored at 0.
    ///
    /// Death is detected here rather than at the six call sites because this
    /// is the only path that lowers a creature's HP — every other write to
    /// `Stats::hp` is a heal, one of the two full-heals, or
    /// `needs_tick_system`, which is `With<Player>`. A seventh caller added
    /// later inherits the check for free; six separate checks would not.
    ///
    /// Only party members are announced. A hostile reaching 0 is reported by
    /// `finish_member`, and the player by `difficulty::death_handling_system`.
    pub(crate) fn apply_damage(&mut self, target: Entity, dmg: i32) {
        let dmg = self.mitigate_incoming_damage(target, dmg);
        let killed = {
            let Some(mut stats) = self.world.get_mut::<Stats>(target) else {
                return;
            };
            let was_alive = stats.hp > 0;
            stats.hp = (stats.hp - dmg).max(0);
            was_alive && stats.hp == 0
        };
        if killed && self.world.resource::<Party>().0.contains(&target) {
            self.announce_program_death(target);
        }
    }

    /// Raises `target`'s HP by `amount`, capped at its maximum, and returns
    /// how much actually landed — zero on a full-health target.
    ///
    /// The return value is what battle logs print. Printing the requested
    /// figure instead let a heal claim twenty points on a target with three
    /// to spare, which reads as the heal having been wasted by the game
    /// rather than by the player's timing.
    pub(crate) fn restore_hp(&mut self, target: Entity, amount: i32) -> i32 {
        let Some(mut stats) = self.world.get_mut::<Stats>(target) else {
            return 0;
        };
        let before = stats.hp;
        stats.hp = (stats.hp + amount).min(stats.max_hp);
        stats.hp - before
    }

    /// Cuts `dmg` by `target`'s running `Mitigation` field buff power
    /// (percentage points), read off `target` itself since the kind is
    /// `FieldScope::Creature` — it protects whoever carries it, not the
    /// whole party.
    ///
    /// Rounds once, in the same expression as the percentage cut, rather
    /// than rounding the reduction and then subtracting it — two roundings
    /// can discard a point the combined operation keeps.
    ///
    /// Floors at 1 so a landed hit stays a hit under heavy mitigation, but
    /// only when there was a hit to protect: `dmg <= 0` (already a miss, or
    /// no buff at all) passes through untouched rather than being raised to 1.
    fn mitigate_incoming_damage(&self, target: Entity, dmg: i32) -> i32 {
        let power = self.field_buff_power(target, FieldBuffKind::Mitigation);
        if power <= 0 || dmg <= 0 {
            return dmg;
        }
        let reduced = (dmg as f32 * (1.0 - power as f32 / 100.0)).round() as i32;
        reduced.max(1)
    }

    /// The `Outcome` line for a party member killed in battle: what died and
    /// what died with it.
    ///
    /// Emitted the moment its HP reaches 0, while the entity itself lives on
    /// until `end_battle` reaps it — see that method for why the removal has
    /// to wait.
    fn announce_program_death(&mut self, program: Entity) {
        let name = self.creature_label(program);
        let routines: Vec<String> = self
            .extractable_routines(program)
            .into_iter()
            .map(|def| def.name)
            .collect();
        let line = if routines.is_empty() {
            format!("{name} crashes and is deleted for good.")
        } else {
            format!(
                "{name} crashes and is deleted for good, taking {} with it.",
                routines.join(", ")
            )
        };
        self.log_kind(MessageKind::Outcome, line);
    }

    pub(crate) fn creature_alive(&self, e: Entity) -> bool {
        self.world
            .get::<Stats>(e)
            .map(|s| s.hp > 0)
            .unwrap_or(false)
    }

    /// Clears out any group whose front died to a status tick, awarding
    /// loot and XP exactly as a direct kill would. Walks back to front so
    /// removing a group can't shift a later one out from under the loop.
    /// Returns whether that ended the battle.
    pub(crate) fn reap_dead_members(&mut self, player: Entity) -> bool {
        let mut group = self.living_group_count();
        while group > 0 {
            group -= 1;
            let mut index = self
                .world
                .get_resource::<BattleState>()
                .and_then(|b| b.groups.get(group))
                .map(|g| g.members.len())
                .unwrap_or(0);
            while index > 0 {
                index -= 1;
                let alive = self
                    .world
                    .get_resource::<BattleState>()
                    .and_then(|b| b.groups.get(group))
                    .and_then(|g| g.members.get(index))
                    .is_some_and(|&e| self.creature_alive(e));
                if alive {
                    continue;
                }
                if self.finish_member(group, index, player) {
                    return true;
                }
            }
        }
        false
    }
}
