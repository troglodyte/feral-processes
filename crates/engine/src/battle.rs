use bevy_ecs::prelude::Entity;
use rand::RngExt;
use serde::{Deserialize, Serialize};

use crate::items::ItemId;
use crate::species::SpeciesId;
use crate::tuning::{
    ACCURACY_PER_LEVEL, ACCURACY_PER_SPEED, BACK_SLOT_AGGRO_WEIGHT, CRIT_CHANCE,
    CRIT_ROLL_MULTIPLIER, DEFEND_AGGRO_WEIGHT, EVASION_PER_LEVEL, EVASION_PER_SPEED,
    FRONT_SLOT_AGGRO_WEIGHT, FRONT_SLOTS, FUMBLE_CHANCE, FUMBLE_RECOIL_FRACTION,
    FUMBLE_RUNG_THRESHOLDS, HIT_CHANCE_MAX, HIT_CHANCE_MIN, JACK_OUT_BASE_CHANCE,
    JACK_OUT_CHANCE_MAX, JACK_OUT_CHANCE_MIN, LOW_POWER_ATTACK_THRESHOLD,
    LOW_POWER_MIN_ATTACK_MULTIPLIER,
};

/// The band one attack rolls its damage from, inclusive at both ends.
///
/// **Two constructors on purpose.** Items author `(min, max)` directly and
/// never convert to anything else; abilities and moves author a centre and a
/// spread, because `species::basic_attack_ability` converts a `MoveDef` into
/// an `AbilityDef` and a centre-and-spread pair survives that losslessly
/// where a `(min, max)` pair would round on odd widths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageRange {
    pub min: i32,
    pub max: i32,
}

impl DamageRange {
    /// A range `spread` either side of `power`, floored at 0 on the low end.
    /// A `spread` of 0 is a degenerate range — exactly the deterministic
    /// behaviour every ability had before ranges existed, which is why
    /// `AbilityEffect::Damage`'s new `spread` field defaults to it and none
    /// of the shipped ability files needed editing.
    pub fn centred(power: i32, spread: i32) -> Self {
        let spread = spread.max(0);
        DamageRange {
            min: (power - spread).max(0),
            max: (power + spread).max(0),
        }
    }

    /// The mean of a uniform draw over this range. `expected_damage` — and
    /// so `balance_sim` — is built on this rather than on a re-derived
    /// midpoint.
    pub fn mean(self) -> f64 {
        (self.min as f64 + self.max as f64) / 2.0
    }

    /// One uniform draw from the range.
    ///
    /// Written as an offset from `min` rather than `random_range(min..=max)`
    /// so a degenerate range still consumes exactly one draw. Draw counts
    /// have to be a property of the outcome and not of the weapon, or every
    /// seeded run's RNG stream would shift with the party's loadout.
    pub fn roll(self, rng: &mut impl rand::Rng) -> i32 {
        let width = (self.max - self.min).max(0);
        self.min + rng.random_range(0..=width)
    }
}

/// Odds one attack lands, from the attacker's Accuracy against the
/// defender's Evasion.
///
/// **The ratio form is load-bearing and a difference form must not replace
/// it.** The ratio is scale-free: doubling both sides leaves the result at
/// 0.5, so a zone that scales everything by its tier multiplier changes
/// nothing about hit rates and the "every difficulty curve must be linear"
/// hazard cannot reappear on this axis at all. `base + k * (acc - eva)`
/// makes hit rate depend on absolute scale, so deep zones drift silently
/// toward always-hit or always-miss.
///
/// Two identical combatants get exactly 0.5 by construction, before the
/// clamp — the baseline every constant in this section is read against.
pub fn hit_chance(accuracy: f64, evasion: f64) -> f64 {
    let acc = accuracy.max(0.0);
    let eva = evasion.max(0.0);
    let total = acc + eva;
    // Two combatants with nothing at all is an even matchup, not an
    // infinity. Reachable from a mod species authoring `base_speed: 0`.
    if total <= 0.0 {
        return 0.5f64.clamp(HIT_CHANCE_MIN, HIT_CHANCE_MAX);
    }
    (acc / total).clamp(HIT_CHANCE_MIN, HIT_CHANCE_MAX)
}

/// A combatant's Accuracy. **Derived, never stored** — not a `Stats` field,
/// not a save field, so it cannot drift from its inputs.
///
/// `gear_accuracy` is `EquipmentStats::accuracy` summed over worn slots,
/// which unlike `atk`/`mitigation` is *not* baked into `Stats` by
/// `Game::apply_equipment_delta` and so must be passed in live.
pub fn accuracy_of(base_speed: i32, level: u32, gear_accuracy: i32) -> f64 {
    (base_speed as f64 * ACCURACY_PER_SPEED
        + level as f64 * ACCURACY_PER_LEVEL
        + gear_accuracy as f64)
        .max(0.0)
}

/// A combatant's Evasion. Same derived-never-stored contract as
/// `accuracy_of`; see its doc for `gear_evasion`.
pub fn evasion_of(base_speed: i32, level: u32, gear_evasion: i32) -> f64 {
    (base_speed as f64 * EVASION_PER_SPEED + level as f64 * EVASION_PER_LEVEL + gear_evasion as f64)
        .max(0.0)
}

/// Everything one side brings to a single attack roll, resolved by the
/// caller and handed in flat.
///
/// A struct rather than four parameters for the same reason
/// `Game::copy_bonus` takes a whole `GearCopy`: a fifth axis added later is
/// then not forgettable at a call site.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Combatant {
    pub accuracy: f64,
    pub evasion: f64,
    /// Flat damage added to every landed roll. Never feeds the to-hit roll —
    /// see `accuracy_of`.
    pub atk: i32,
    pub range: DamageRange,
}

/// How badly an attack went wrong. **Rungs replace rather than stack** — a
/// cumulative top rung is a run-ender. Which rung comes from how deep into
/// the fumble band the roll fell, so it needs no second draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FumbleRung {
    /// Evasion cut by `EXPOSED_EVASION_PERCENT` until the fumbler's next
    /// turn.
    Exposed,
    /// `FUMBLE_RECOIL_FRACTION` of a fresh roll of the fumbler's own range,
    /// dealt to the fumbler.
    Recoil { dmg: i32 },
    /// The target takes a free swing at the fumbler, for `dmg`. Zero when
    /// the free swing missed.
    Opening { dmg: i32 },
    /// The fumbler loses their next action.
    Crash,
}

/// What one attack did. The caller branches on this: a miss must skip a
/// Drain's heal and a rider's status, which is why the branch cannot live
/// inside `Game::apply_damage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackOutcome {
    Fumble(FumbleRung),
    Miss,
    Hit { dmg: i32 },
    Crit { dmg: i32 },
}

impl AttackOutcome {
    /// Damage aimed at the *defender*. Zero for a miss and for every fumble
    /// rung — a Recoil hurts the fumbler and an Opening's riposte lands on
    /// the fumbler, so neither is defender-facing damage.
    pub fn damage_to_defender(self) -> i32 {
        match self {
            AttackOutcome::Hit { dmg } | AttackOutcome::Crit { dmg } => dmg,
            AttackOutcome::Miss | AttackOutcome::Fumble(_) => 0,
        }
    }
}

/// Resolves one creature-versus-creature attack.
///
/// **One draw, four bands.** A single `r ∈ [0, 1)` decides the outcome, in
/// this order: crit (clamped to at most the hit chance), hit, fumble
/// (clamped to at most `1 - hit chance`), miss. One draw rather than three
/// bounds the RNG-stream shift and makes crit and fumble mutually exclusive
/// *by construction* rather than by a check.
///
/// A structure has no speed and cannot dodge, so `Game::attack_nest` does
/// not come through here — only creature-versus-creature attacks do.
pub fn resolve_attack(
    attacker: Combatant,
    defender: Combatant,
    rng: &mut impl rand::Rng,
) -> AttackOutcome {
    resolve_attack_inner(attacker, defender, rng, true)
}

/// `allow_fumble: false` is the Opening rung's non-recursion guard — see
/// `fumble_rung`.
fn resolve_attack_inner(
    attacker: Combatant,
    defender: Combatant,
    rng: &mut impl rand::Rng,
    allow_fumble: bool,
) -> AttackOutcome {
    let h = hit_chance(attacker.accuracy, defender.evasion);
    let crit = CRIT_CHANCE.min(h);
    let fumble = if allow_fumble {
        FUMBLE_CHANCE.min(1.0 - h)
    } else {
        0.0
    };
    let r: f64 = rng.random();
    if r < crit {
        let rolled = attacker.range.roll(rng);
        return AttackOutcome::Crit {
            dmg: rolled * CRIT_ROLL_MULTIPLIER + attacker.atk,
        };
    }
    if r < h {
        let rolled = attacker.range.roll(rng);
        return AttackOutcome::Hit {
            dmg: rolled + attacker.atk,
        };
    }
    if fumble > 0.0 && r >= 1.0 - fumble {
        let depth = (r - (1.0 - fumble)) / fumble;
        return AttackOutcome::Fumble(fumble_rung(depth, attacker, defender, rng));
    }
    AttackOutcome::Miss
}

/// Which rung a fumble at `depth` into the band lands on. `depth` is in
/// `[0, 1)` and derived from the single band roll, so severity costs no
/// second draw.
fn fumble_rung(
    depth: f64,
    attacker: Combatant,
    defender: Combatant,
    rng: &mut impl rand::Rng,
) -> FumbleRung {
    let [exposed, recoil, opening] = FUMBLE_RUNG_THRESHOLDS;
    if depth < exposed {
        return FumbleRung::Exposed;
    }
    if depth < recoil {
        let rolled = attacker.range.roll(rng);
        return FumbleRung::Recoil {
            dmg: ((rolled as f32) * FUMBLE_RECOIL_FRACTION).round().max(1.0) as i32,
        };
    }
    if depth < opening {
        // **The free swing must not itself fumble.** A fumbled riposte
        // resolves as a plain miss. This is a hard rule, not a convention:
        // without it one bad roll chains into an unbounded exchange, and the
        // deepest rung stops being the run-ender the ladder is shaped to
        // avoid. `the_opening_rung_does_not_recurse` pins it.
        let riposte = resolve_attack_inner(defender, attacker, rng, false);
        return FumbleRung::Opening {
            dmg: riposte.damage_to_defender(),
        };
    }
    FumbleRung::Crash
}

/// The mean of `resolve_attack`'s defender-facing damage, RNG-free.
///
/// **`balance_sim` calls this; it does not keep a copy.** `CLAUDE.md`
/// records four occasions where a `balance_sim` doc comment promised it
/// mirrored a real formula while being an independent copy that drifted —
/// worst of all a mining-reliability curve that would have let the balance
/// gate pass against a game that no longer existed. Follow
/// `attackers_in_group` and `slot_aggro_weight`.
///
/// Deliberately excludes the fumble ladder: Recoil and Opening both land on
/// the *attacker*, so neither is defender-facing damage, and the projection
/// is therefore a mild overestimate of an attacker's net output. Named here
/// rather than silently, in the same spirit as `TURN_CAP`'s note that Power
/// decay is unmodelled.
pub fn expected_damage(attacker: Combatant, defender: Combatant) -> f64 {
    let h = hit_chance(attacker.accuracy, defender.evasion);
    let crit = CRIT_CHANCE.min(h);
    let plain = h - crit;
    let mean = attacker.range.mean();
    let atk = attacker.atk as f64;
    plain * (mean + atk) + crit * (mean * CRIT_ROLL_MULTIPLIER as f64 + atk)
}

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
    /// PowerReserve no choice at all — it resolves the moment it is picked.
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
    /// Battle rounds this locks itself away for once spent — the whole price
    /// of a Special, since neither side pays a need for one. Carried so the
    /// picker can say what a ready routine will cost before it is spent,
    /// rather than only reporting it through `unavailable` once it is too
    /// late to choose differently. 0 means no cooldown at all.
    pub cooldown: u32,
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

    #[test]
    fn a_centred_range_of_zero_spread_is_the_power_exactly() {
        let range = DamageRange::centred(8, 0);
        assert_eq!(range.min, 8);
        assert_eq!(range.max, 8);
    }

    #[test]
    fn a_centred_range_widens_symmetrically_around_its_power() {
        let range = DamageRange::centred(10, 3);
        assert_eq!(range.min, 7);
        assert_eq!(range.max, 13);
        assert_eq!(range.mean(), 10.0);
    }

    #[test]
    fn a_centred_range_never_reaches_below_zero() {
        // A low-power ability with a wide spread must not roll negative
        // damage into `apply_damage`, which would read as a heal.
        let range = DamageRange::centred(2, 5);
        assert_eq!(range.min, 0);
        assert_eq!(range.max, 7);
    }

    #[test]
    fn a_degenerate_range_still_spends_exactly_one_draw() {
        // Draw counts must be a property of the *outcome*, not of which
        // weapon swung: a spread-0 ability and a wide weapon have to cost
        // the same, or the RNG stream shifts with the loadout.
        use rand::SeedableRng;
        let mut wide = rand::rngs::StdRng::seed_from_u64(7);
        let mut narrow = rand::rngs::StdRng::seed_from_u64(7);
        let _ = DamageRange { min: 4, max: 9 }.roll(&mut wide);
        let _ = DamageRange { min: 6, max: 6 }.roll(&mut narrow);
        let after_wide: u64 = wide.random();
        let after_narrow: u64 = narrow.random();
        assert_eq!(
            after_wide, after_narrow,
            "both ranges must leave the stream in the same place"
        );
    }

    #[test]
    fn a_roll_stays_inside_its_range() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(11);
        let range = DamageRange { min: 4, max: 9 };
        for _ in 0..500 {
            let rolled = range.roll(&mut rng);
            assert!((4..=9).contains(&rolled), "rolled {rolled} outside 4..=9");
        }
    }

    #[test]
    fn two_identical_combatants_hit_each_other_half_the_time() {
        // The baseline every tuning number in this section is read against.
        assert!((hit_chance(12.0, 12.0) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn hit_chance_is_scale_free() {
        // The whole reason the ratio form is load-bearing: a zone that
        // scales everything by its tier multiplier must change nothing
        // about hit rates.
        let base = hit_chance(14.0, 6.0);
        assert!((hit_chance(28.0, 12.0) - base).abs() < 1e-12);
        assert!((hit_chance(140.0, 60.0) - base).abs() < 1e-12);
    }

    #[test]
    fn hit_chance_clamps_at_both_ends() {
        assert_eq!(hit_chance(1000.0, 1.0), HIT_CHANCE_MAX);
        assert_eq!(hit_chance(1.0, 1000.0), HIT_CHANCE_MIN);
    }

    #[test]
    fn hit_chance_survives_two_combatants_with_nothing_at_all() {
        // Reachable through a mod species authoring base_speed 0 at level 1
        // with no gear. An even matchup, not a divide by zero.
        assert!((hit_chance(0.0, 0.0) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn accuracy_and_evasion_both_grow_with_speed_level_and_gear() {
        assert!(accuracy_of(14, 1, 0) > accuracy_of(6, 1, 0));
        assert!(accuracy_of(10, 8, 0) > accuracy_of(10, 1, 0));
        assert!(accuracy_of(10, 1, 3) > accuracy_of(10, 1, 0));
        assert!(evasion_of(14, 1, 0) > evasion_of(6, 1, 0));
        assert!(evasion_of(10, 8, 0) > evasion_of(10, 1, 0));
        assert!(evasion_of(10, 1, 3) > evasion_of(10, 1, 0));
    }

    #[test]
    fn a_negative_gear_axis_cannot_push_the_pair_below_zero() {
        // A drawback affix is folded into the base, so a copy can carry a
        // negative on an axis its item never had.
        assert!(accuracy_of(6, 1, -100) >= 0.0);
        assert!(evasion_of(6, 1, -100) >= 0.0);
    }

    fn combatant(accuracy: f64, evasion: f64, atk: i32, range: DamageRange) -> Combatant {
        Combatant {
            accuracy,
            evasion,
            atk,
            range,
        }
    }

    /// A `StdRng` seeded so its first `f64` draw lands in `band`. Scanned
    /// rather than mocked: `resolve_attack` takes `impl Rng`, and a fake
    /// that returns scripted values would stop measuring the thing the
    /// draw-count tests exist for.
    fn rng_whose_first_roll_is_in(band: std::ops::Range<f64>) -> rand::rngs::StdRng {
        use rand::SeedableRng;
        for seed in 0..100_000u64 {
            let mut candidate = rand::rngs::StdRng::seed_from_u64(seed);
            let r: f64 = candidate.random();
            if band.contains(&r) {
                return rand::rngs::StdRng::seed_from_u64(seed);
            }
        }
        panic!("no seed produced a first roll inside {band:?}");
    }

    /// A `StdRng` that counts the primitive draws it is asked for.
    ///
    /// Counting primitives rather than comparing stream positions is the
    /// only measure that works here: a `f64` draw takes a `u64` from the
    /// stream and a `random_range` over a small integer band takes a `u32`,
    /// so the stream never realigns on a single word size. It delegates to a
    /// real `StdRng` rather than scripting values, because a fake would stop
    /// measuring the thing these tests exist for.
    struct CountingRng {
        inner: rand::rngs::StdRng,
        draws: usize,
    }

    impl CountingRng {
        fn seeded(seed: u64) -> Self {
            use rand::SeedableRng;
            CountingRng {
                inner: rand::rngs::StdRng::seed_from_u64(seed),
                draws: 0,
            }
        }
    }

    // `rand_core` blanket-implements `Rng` for any infallible `TryRng`, so
    // this is the only impl needed and every draw goes through it.
    impl rand::TryRng for CountingRng {
        type Error = std::convert::Infallible;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            use rand::Rng;
            self.draws += 1;
            Ok(self.inner.next_u32())
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            use rand::Rng;
            self.draws += 1;
            Ok(self.inner.next_u64())
        }

        fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
            use rand::Rng;
            self.draws += 1;
            self.inner.fill_bytes(dst);
            Ok(())
        }
    }

    /// How many primitive draws `f` spends against a `StdRng` on `seed`.
    fn draws_spent(seed: u64, f: impl Fn(&mut CountingRng)) -> usize {
        let mut rng = CountingRng::seeded(seed);
        f(&mut rng);
        rng.draws
    }

    /// First seed whose `resolve_attack` satisfies `want`.
    fn seed_producing(
        attacker: Combatant,
        defender: Combatant,
        want: impl Fn(&AttackOutcome) -> bool,
    ) -> u64 {
        use rand::SeedableRng;
        for seed in 0..500_000u64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let outcome = resolve_attack(attacker, defender, &mut rng);
            if want(&outcome) {
                return seed;
            }
        }
        panic!("no seed produced the wanted outcome");
    }

    #[test]
    fn a_roll_below_the_crit_chance_is_a_crit_that_doubles_only_the_rolled_part() {
        let attacker = combatant(12.0, 12.0, 10, DamageRange { min: 4, max: 4 });
        let defender = combatant(12.0, 12.0, 0, DamageRange::default());
        let mut rng = rng_whose_first_roll_is_in(0.0..CRIT_CHANCE);
        let outcome = resolve_attack(attacker, defender, &mut rng);
        // 4 rolled, doubled to 8, plus a flat atk of 10 that is NOT doubled.
        assert_eq!(outcome, AttackOutcome::Crit { dmg: 18 });
    }

    #[test]
    fn a_roll_between_the_crit_chance_and_the_hit_chance_is_a_plain_hit() {
        let attacker = combatant(12.0, 12.0, 10, DamageRange { min: 4, max: 4 });
        let defender = combatant(12.0, 12.0, 0, DamageRange::default());
        let mut rng = rng_whose_first_roll_is_in(CRIT_CHANCE..0.5);
        let outcome = resolve_attack(attacker, defender, &mut rng);
        assert_eq!(outcome, AttackOutcome::Hit { dmg: 14 });
    }

    #[test]
    fn a_roll_between_the_hit_chance_and_the_fumble_band_is_a_plain_miss() {
        let attacker = combatant(12.0, 12.0, 10, DamageRange { min: 4, max: 4 });
        let defender = combatant(12.0, 12.0, 0, DamageRange::default());
        let mut rng = rng_whose_first_roll_is_in(0.5..(1.0 - FUMBLE_CHANCE));
        assert_eq!(
            resolve_attack(attacker, defender, &mut rng),
            AttackOutcome::Miss
        );
    }

    #[test]
    fn a_roll_at_the_top_of_the_range_is_a_fumble() {
        let attacker = combatant(12.0, 12.0, 10, DamageRange { min: 4, max: 4 });
        let defender = combatant(12.0, 12.0, 3, DamageRange { min: 2, max: 2 });
        let mut rng = rng_whose_first_roll_is_in((1.0 - FUMBLE_CHANCE)..1.0);
        assert!(matches!(
            resolve_attack(attacker, defender, &mut rng),
            AttackOutcome::Fumble(_)
        ));
    }

    #[test]
    fn crit_and_fumble_are_mutually_exclusive_by_construction() {
        // Not sampled: the bands are read off one draw in a fixed order, so
        // no value of `r` can satisfy both. Sweeping `r` across the whole
        // unit interval is the exhaustive statement of that.
        let attacker = combatant(12.0, 12.0, 0, DamageRange::default());
        let defender = combatant(12.0, 12.0, 0, DamageRange::default());
        let h = hit_chance(attacker.accuracy, defender.evasion);
        let crit = CRIT_CHANCE.min(h);
        let fumble = FUMBLE_CHANCE.min(1.0 - h);
        for step in 0..10_000 {
            let r = step as f64 / 10_000.0;
            assert!(
                !(r < crit && r >= 1.0 - fumble),
                "r = {r} fell in both the crit and the fumble band"
            );
        }
    }

    #[test]
    fn a_crit_can_never_exceed_the_hit_chance() {
        // A hopeless matchup floors at HIT_CHANCE_MIN, which is above
        // CRIT_CHANCE — so squeeze it the other way: a hit chance clamped
        // low must still not let the crit band overhang it.
        let h = HIT_CHANCE_MIN;
        assert!(CRIT_CHANCE.min(h) <= h);
    }

    #[test]
    fn the_opening_rung_does_not_recurse() {
        // A free swing that itself fumbles resolves as a plain miss, which
        // is what bounds an Opening's cost: the band roll, the riposte's own
        // band roll, and at most one range roll if the riposte landed.
        //
        // The bound is what makes this test non-vacuous. The *type* already
        // forbids a `Fumble` nested inside an `Opening`, so classifying the
        // outcome proves nothing — flip `allow_fumble` back to `true` and
        // the outer outcome is still `Opening`, only the draws it spent
        // getting there are unbounded.
        let attacker = combatant(12.0, 12.0, 4, DamageRange { min: 2, max: 6 });
        let defender = combatant(12.0, 12.0, 4, DamageRange { min: 2, max: 6 });
        let mut openings = 0;
        for seed in 0..20_000u64 {
            let mut rng = CountingRng::seeded(seed);
            if let AttackOutcome::Fumble(FumbleRung::Opening { dmg }) =
                resolve_attack(attacker, defender, &mut rng)
            {
                openings += 1;
                assert!(dmg >= 0, "an Opening riposte cannot heal the fumbler");
                assert!(
                    rng.draws <= 3,
                    "seed {seed} spent {} draws on one Opening — the riposte fumbled \
                     into a nested exchange",
                    rng.draws
                );
            }
        }
        assert!(openings > 0, "no seed reached the Opening rung");
    }

    #[test]
    fn draw_counts_are_pinned_per_outcome() {
        // Asserting the exact count is what stops crit or fumble silently
        // becoming an extra draw and shifting every seeded run's stream.
        let attacker = combatant(12.0, 12.0, 5, DamageRange { min: 2, max: 6 });
        let defender = combatant(12.0, 12.0, 5, DamageRange { min: 2, max: 6 });

        let miss_seed = seed_producing(attacker, defender, |o| *o == AttackOutcome::Miss);
        assert_eq!(
            draws_spent(miss_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            1,
            "a miss costs one draw"
        );

        let hit_seed = seed_producing(attacker, defender, |o| {
            matches!(o, AttackOutcome::Hit { .. })
        });
        assert_eq!(
            draws_spent(hit_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            2,
            "a hit costs the band roll plus one weapon roll"
        );

        let crit_seed = seed_producing(attacker, defender, |o| {
            matches!(o, AttackOutcome::Crit { .. })
        });
        assert_eq!(
            draws_spent(crit_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            2,
            "a crit costs the same as a hit — the doubling is arithmetic"
        );

        let exposed_seed = seed_producing(attacker, defender, |o| {
            *o == AttackOutcome::Fumble(FumbleRung::Exposed)
        });
        assert_eq!(
            draws_spent(exposed_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            1,
            "Exposed spends nothing beyond the band roll"
        );

        let recoil_seed = seed_producing(attacker, defender, |o| {
            matches!(o, AttackOutcome::Fumble(FumbleRung::Recoil { .. }))
        });
        assert_eq!(
            draws_spent(recoil_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            2,
            "Recoil adds one fresh roll of the fumbler's own range"
        );

        let crash_seed = seed_producing(attacker, defender, |o| {
            *o == AttackOutcome::Fumble(FumbleRung::Crash)
        });
        assert_eq!(
            draws_spent(crash_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            1,
            "Crash spends nothing beyond the band roll"
        );
    }

    #[test]
    fn expected_damage_is_the_mean_of_the_same_arithmetic() {
        // The property that lets `balance_sim` *call* this rather than keep
        // a copy: averaging a large sample of the real roll must converge on
        // it. Seeded, so it is deterministic.
        use rand::SeedableRng;
        let attacker = combatant(12.0, 12.0, 5, DamageRange { min: 2, max: 6 });
        let defender = combatant(12.0, 12.0, 5, DamageRange { min: 2, max: 6 });
        let mut rng = rand::rngs::StdRng::seed_from_u64(4);
        let n = 200_000;
        let total: i64 = (0..n)
            .map(|_| resolve_attack(attacker, defender, &mut rng).damage_to_defender() as i64)
            .sum();
        let sampled = total as f64 / n as f64;
        let projected = expected_damage(attacker, defender);
        assert!(
            (sampled - projected).abs() < 0.1,
            "sampled {sampled}, projected {projected}"
        );
    }
}
