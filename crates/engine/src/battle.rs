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
    Attack {
        group: usize,
    },
    Special {
        /// Index into `Game::companion_abilities` for the acting member —
        /// which of its abilities this is. Always valid by construction;
        /// resolution falls back to the first if a stale index survives a
        /// party change mid-round.
        ability: usize,
        /// Who it lands on, which side depending on the ability — see
        /// `species::SpecialAbility::targeting`.
        target: SpecialTarget,
    },
    Defend,
    Decompile {
        group: usize,
    },
    UseItem {
        item: ItemId,
    },
}

/// Which picker the UI opens after an ability is chosen — see
/// `abilities::AbilityTarget::targeting`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialTargeting {
    /// Lands on a party member: the player or any companion.
    Ally,
    /// Lands on an enemy group.
    Enemy,
    /// Needs no choice at all — it resolves the moment it is picked.
    None,
}

/// Who a `BattleAction::Special` lands on. A buff or heal picks a party
/// member; a debuff picks an enemy group — so unlike every other targeted
/// action, a Special's target isn't always a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialTarget {
    /// A party slot, indexed as `battle::Actor::Party` — slot 0 is the
    /// player.
    Ally {
        slot: usize,
    },
    EnemyGroup {
        group: usize,
    },
    /// Every living party member. Carries no index because the player makes
    /// no choice — see `SpecialTargeting::None`.
    WholeParty,
    /// Every living enemy in every group. Same no-choice rationale.
    AllEnemies,
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
    /// Pick one of the acting member's special abilities, *then* whoever it
    /// lands on — the only two-step collection, because which ability is
    /// used and who it is aimed at are independent choices. Which picker
    /// comes second depends on the ability chosen: a buff or heal lists the
    /// party, a debuff lists enemy groups (see `SpecialOption::targeting`).
    SpecialAbility,
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

/// One row of the special-ability picker — which of a companion's abilities
/// a Special will spend. Renderers draw this verbatim, same contract as
/// `ActionOption`; see `Game::battle_special_options`.
#[derive(Debug, Clone)]
pub struct SpecialOption {
    /// Position in `Game::companion_abilities`, and what
    /// `BattleAction::Special::ability` is set to.
    pub index: usize,
    /// e.g. "Heal"
    pub name: String,
    /// e.g. "Heal: 8 HP"
    pub detail: String,
    /// Which picker follows this choice — an ally list or an enemy group
    /// list. Carried here so neither renderer has to know which abilities
    /// are buffs.
    pub targeting: SpecialTargeting,
    /// For a `SpecialTargeting::None` ability, which side it sweeps —
    /// carried here so neither renderer has to know what any ability does.
    /// Meaningless (and always `false`) for abilities that open a picker.
    pub sweeps_party: bool,
}

/// One row of the ally picker — who a party-facing Special lands on. Same
/// verbatim-render contract as `ActionOption`; see
/// `Game::battle_ally_options`.
#[derive(Debug, Clone)]
pub struct AllyOption {
    /// Party slot, and what `SpecialTarget::Ally` is set to.
    pub slot: usize,
    /// e.g. "You" or the companion's display name.
    pub name: String,
    /// e.g. "12/30 HP"
    pub detail: String,
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
