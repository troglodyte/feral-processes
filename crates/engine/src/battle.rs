use bevy_ecs::prelude::Entity;

use crate::items::ItemId;
use crate::species::SpeciesId;
use crate::tuning::{
    BACK_SLOT_AGGRO_WEIGHT, DEFEND_AGGRO_WEIGHT, FRONT_SLOT_AGGRO_WEIGHT, FRONT_SLOTS,
    JACK_OUT_BASE_CHANCE, JACK_OUT_CHANCE_MAX, JACK_OUT_CHANCE_MIN, LOW_POWER_ATTACK_THRESHOLD,
    LOW_POWER_MIN_ATTACK_MULTIPLIER, MIN_DAMAGE,
};

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

/// Smallest `k` where `k * k >= n`. Integer throughout: `(n as f64).sqrt().ceil()`
/// can round the wrong way at perfect squares, and both callers need 81 to
/// give exactly 9.
pub(crate) fn ceil_sqrt(n: u32) -> u32 {
    let root = n.isqrt();
    if root * root == n { root } else { root + 1 }
}

/// How many of a group's `n` living members can bring weapons to bear in one
/// round. A hundred-strong swarm cannot all reach the party at once, so it
/// swings ten at a time — which is what makes a swarm an attrition problem
/// rather than an instant wipe. Shared with `crate::balance_sim` so the offline
/// projections and the real round loop cannot drift.
pub(crate) fn attackers_in_group(n: usize) -> usize {
    ceil_sqrt(n as u32) as usize
}

/// Relative weight a roster member at `slot` carries in a wild program's
/// target roll — the player is slot 0 and party members follow in order.
/// Ranks are soft: a back-slot member is hit less often, never zero times.
/// Bracing adds `DEFEND_AGGRO_WEIGHT` on top, which is what makes Defend a
/// party-level play rather than a selfish one.
///
/// Shared with `crate::balance_sim` so the offline projections and the real
/// target roll cannot drift. The sim passes `defending: false` throughout —
/// it models no Defend actions, and its own docs say so.
pub(crate) fn slot_aggro_weight(slot: usize, defending: bool) -> u32 {
    let base = if slot < FRONT_SLOTS {
        FRONT_SLOT_AGGRO_WEIGHT
    } else {
        BACK_SLOT_AGGRO_WEIGHT
    };
    base + if defending { DEFEND_AGGRO_WEIGHT } else { 0 }
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
        /// Index into `Game::actor_abilities` for the acting member —
        /// which of its abilities this is. Always valid by construction;
        /// resolution falls back to the first if a stale index survives a
        /// party change mid-round.
        ability: usize,
        /// Who it lands on, which side depending on the ability — see
        /// `species::SpecialAbility::targeting`.
        target: SpecialTarget,
    },
    Defend,
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
    /// Position in `Game::actor_abilities`, and what
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
    /// `Some(reason)` means render it greyed with the reason shown — same
    /// contract as `ActionOption::unavailable`.
    pub unavailable: Option<String>,
    /// The player's Fatigue this costs to order, whoever runs it. Carried so
    /// the picker can price a routine next to the Fatigue the roster shows,
    /// rather than leaving `unavailable`'s "not enough Fatigue" to quote a
    /// number nothing on screen names.
    pub fatigue_cost: f32,
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
    (move_power + atk - def).max(MIN_DAMAGE)
}

/// Multiplier applied to the player's attack total once their Power drops
/// below `LOW_POWER_ATTACK_THRESHOLD`: full strength at the threshold and
/// above, falling off linearly to half strength at 0 power. A separate,
/// milder penalty from the flat HP drain that already kicks in once power
/// hits exactly 0 (see `systems::needs_tick_system`) — this one's felt in
/// combat well before you're actually starving.
pub fn power_attack_multiplier(hunger: f32) -> f32 {
    if hunger >= LOW_POWER_ATTACK_THRESHOLD {
        1.0
    } else {
        LOW_POWER_MIN_ATTACK_MULTIPLIER
            + (hunger.max(0.0) / LOW_POWER_ATTACK_THRESHOLD)
                * (1.0 - LOW_POWER_MIN_ATTACK_MULTIPLIER)
    }
}

/// Odds that a jack-out attempt actually gets the party clear, from the
/// summed `Stats::power` of your side (`ours` — the player plus every living
/// party member) against theirs (`theirs` — every living enemy in every
/// group). Scales linearly with that ratio off `JACK_OUT_BASE_CHANCE`, then
/// clamps, so no escape is hopeless and none is certain.
///
/// `luck` is passed in rather than drawn here to keep this pure and
/// testable; the caller rolls it from `JACK_OUT_LUCK_MIN..=JACK_OUT_LUCK_MAX`
/// fresh on every attempt. Applied before the clamp, so a lucky roll can
/// never carry the chance past the ceiling.
///
/// Both totals use `max_hp` rather than current HP, making the odds a
/// property of the matchup rather than of how the fight is going: the ratio
/// you face when a pack engages is the ratio you keep, improving as you kill
/// and worsening as companions fall.
pub fn jack_out_chance(ours: i32, theirs: i32, luck: f64) -> f64 {
    // `theirs` at 0 means the battle should already have ended; guarding
    // rather than dividing means a stale call reads as "trivially escapable"
    // instead of producing an infinity.
    let ratio = ours.max(0) as f64 / theirs.max(1) as f64;
    (JACK_OUT_BASE_CHANCE * ratio * luck).clamp(JACK_OUT_CHANCE_MIN, JACK_OUT_CHANCE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Only the caller of `jack_out_chance` draws against these; the function
    // itself takes luck as a parameter, so they aren't imported at module
    // scope.
    use crate::tuning::{JACK_OUT_LUCK_MAX, JACK_OUT_LUCK_MIN};

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

    #[test]
    fn jack_out_at_an_even_matchup_is_the_base_chance() {
        assert!((jack_out_chance(300, 300, 1.0) - JACK_OUT_BASE_CHANCE).abs() < f64::EPSILON);
    }

    #[test]
    fn jack_out_scales_with_the_power_ratio() {
        let outgunned = jack_out_chance(100, 300, 1.0);
        let even = jack_out_chance(300, 300, 1.0);
        let dominant = jack_out_chance(500, 300, 1.0);
        assert!(outgunned < even, "being outgunned must hurt your odds");
        assert!(dominant > even, "outclassing them must help");
    }

    #[test]
    fn jack_out_clamps_to_the_floor_against_an_overwhelming_pack() {
        assert_eq!(jack_out_chance(10, 100_000, 1.0), JACK_OUT_CHANCE_MIN);
    }

    #[test]
    fn jack_out_clamps_to_the_ceiling_against_a_trivial_pack() {
        assert_eq!(jack_out_chance(100_000, 10, 1.0), JACK_OUT_CHANCE_MAX);
    }

    #[test]
    fn jack_out_luck_shifts_the_odds_in_both_directions() {
        let unlucky = jack_out_chance(300, 300, JACK_OUT_LUCK_MIN);
        let neutral = jack_out_chance(300, 300, 1.0);
        let lucky = jack_out_chance(300, 300, JACK_OUT_LUCK_MAX);
        assert!(unlucky < neutral);
        assert!(lucky > neutral);
    }

    #[test]
    fn jack_out_luck_cannot_push_the_chance_outside_the_clamp() {
        // Even maximum luck on a hopeless matchup stays inside the bounds —
        // the clamp is applied last, not to the pre-luck value.
        let lucky_but_hopeless = jack_out_chance(1, 100_000, JACK_OUT_LUCK_MAX);
        assert!((JACK_OUT_CHANCE_MIN..=JACK_OUT_CHANCE_MAX).contains(&lucky_but_hopeless));
    }

    #[test]
    fn jack_out_against_no_living_enemies_does_not_divide_by_zero() {
        let chance = jack_out_chance(300, 0, 1.0);
        assert!(chance.is_finite());
        assert_eq!(chance, JACK_OUT_CHANCE_MAX);
    }
}
