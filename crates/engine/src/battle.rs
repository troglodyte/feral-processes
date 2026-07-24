use bevy_ecs::prelude::Entity;

use crate::items::ItemId;
use crate::species::SpeciesId;

/// One species' worth of the wild pack in an active intrusion.
/// `members[0]` is the front — the only member that takes hits and the only
/// one whose HP the roster shows. Emptying a group removes it from
/// `resources::BattleState::groups`, which promotes whatever sat behind it.
#[derive(Debug, Clone)]
pub struct EnemyGroup {
    pub species: SpeciesId,
    pub members: Vec<Entity>,
}

impl EnemyGroup {
    pub fn front(&self) -> Option<Entity> {
        self.members.first().copied()
    }
}

/// One combatant in an initiative order — an index rather than an `Entity`,
/// so a resolution walk can survive members dying mid-round and can address
/// a party slot that has since emptied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    /// Slot 0 is the player; 1.. are party members in roster order.
    Party(usize),
    Enemy {
        group: usize,
        slot: usize,
    },
}

/// What a party member is doing this round. Adding a variant here plus an
/// arm in `Game::resolve_one_action` and a rule in
/// `Game::battle_action_options` is the *entire* cost of a new battle action
/// — no renderer changes, by design.
#[derive(Debug, Clone, PartialEq)]
pub enum BattleAction {
    Attack { group: usize },
    Special { group: usize },
    Defend,
    Decompile { group: usize },
    UseItem { item: ItemId },
}

/// The menu-facing identity of an action, without its parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Attack,
    Special,
    Defend,
    Decompile,
    UseItem,
}

/// What the UI must collect before an `ActionKind` becomes a
/// `BattleAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSpec {
    None,
    EnemyGroup,
    InventoryItem,
}

/// One row of a party member's action menu. Renderers draw this verbatim and
/// never author an action string of their own.
#[derive(Debug, Clone)]
pub struct ActionOption {
    pub kind: ActionKind,
    /// Hotkey the engine assigns, so the two renderers cannot drift.
    pub key: char,
    /// e.g. "[a]ttack"
    pub label: String,
    /// e.g. "Rally: +3 ATK for 3 rounds"
    pub detail: String,
    pub target: TargetSpec,
    /// `Some(reason)` means render it greyed with the reason shown.
    pub unavailable: Option<String>,
}

/// A command that applies to the whole party at once rather than to the slot
/// currently choosing. Deliberately not an `ActionOption`: these never become
/// one slot's `BattleAction`, so sharing `ActionKind` would force meaningless
/// arms into `action_from` and `Game::resolve_one_action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyCommandKind {
    AllAttack,
    AllDefend,
    JackOut,
}

/// One party-level entry in the battle action bar. Renderers draw this
/// verbatim, exactly as they do `ActionOption`.
#[derive(Debug, Clone)]
pub struct PartyCommand {
    pub kind: PartyCommandKind,
    /// Uppercase for the party-wide pair, so shift reads as "everyone does
    /// this" against the lowercase per-slot keys.
    pub key: char,
    pub label: String,
    /// Whether the UI must collect an enemy group before this can run.
    /// All-attack sets it only while more than one group is alive.
    pub needs_target: bool,
}

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
