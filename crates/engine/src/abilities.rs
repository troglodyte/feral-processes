use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::{BuffKind, FieldBuffKind, FieldScope, StatusKind};
use crate::species::MoveEffect;

pub type AbilityId = String;

/// The ability every companion falls back to when its species declares
/// none. Validated at startup (see `Game::new`) rather than defended at
/// every call site, the same way a missing economy role aborts the load.
pub const FALLBACK_ABILITY_ID: &str = "priority_boost";

/// The ability a new game pre-installs into the player's first routine slot
/// — capturing a program is reached through the Special menu like anything
/// else. Validated at startup the same way `FALLBACK_ABILITY_ID` is.
pub const DECOMPILE_ABILITY_ID: &str = "decompile";

/// Routine slots at `level`, from one constant set. Both public wrappers
/// call this so the companion and player curves cannot drift into two
/// different shapes — only their constants differ.
///
/// The floor of 1 is load-bearing: `COMPANION_ROUTINE_SLOT_BASE` is 0, so a
/// level-1 companion would otherwise have nowhere to put the kit its species
/// grants it at level 1.
fn routine_slots(level: u32, base: u32, per_level: u32, cap: u32) -> usize {
    (base + level / per_level).clamp(1, cap) as usize
}

/// How many routines a companion at `level` can hold — see
/// `tuning::COMPANION_ROUTINE_SLOT_BASE` and friends.
pub fn companion_routine_slots(level: u32) -> usize {
    routine_slots(
        level,
        crate::tuning::COMPANION_ROUTINE_SLOT_BASE,
        crate::tuning::COMPANION_ROUTINE_SLOT_PER_LEVEL,
        crate::tuning::COMPANION_ROUTINE_SLOT_CAP,
    )
}

/// How many routines the player at `level` can hold — see
/// `tuning::PLAYER_ROUTINE_SLOT_BASE` and friends.
pub fn player_routine_slots(level: u32) -> usize {
    routine_slots(
        level,
        crate::tuning::PLAYER_ROUTINE_SLOT_BASE,
        crate::tuning::PLAYER_ROUTINE_SLOT_PER_LEVEL,
        crate::tuning::PLAYER_ROUTINE_SLOT_CAP,
    )
}

/// `level`'s multiplier at `per_level`, clamped at
/// `tuning::ABILITY_SCALE_LEVEL_CAP` because the player has no level
/// ceiling; see `player_routine_slots`, which clamps for the same reason.
fn level_scale(level: u32, per_level: f32) -> f32 {
    let level = level.min(crate::tuning::ABILITY_SCALE_LEVEL_CAP);
    1.0 + level as f32 * per_level
}

/// The multiplier a **stat-point** magnitude is scaled by at `level` — a
/// `Buff` or `FieldBuff` power. See `tuning::ABILITY_STAT_SCALE_PER_LEVEL`
/// for why this is the gentler of the two.
pub fn ability_stat_scale(level: u32) -> f32 {
    level_scale(level, crate::tuning::ABILITY_STAT_SCALE_PER_LEVEL)
}

/// The multiplier an **HP** magnitude is scaled by at `level` — `Damage`,
/// `Drain`, `Heal`, `Debuff`. See `tuning::ABILITY_HP_SCALE_PER_LEVEL`.
pub fn ability_hp_scale(level: u32) -> f32 {
    level_scale(level, crate::tuning::ABILITY_HP_SCALE_PER_LEVEL)
}

/// A stat-point `power` scaled by `ability_stat_scale(level)` and by the
/// invoker's `affinity` for this effect's category, rounded once. Negative
/// powers scale too — a sap is a negative-power buff, and it has to sharpen
/// with level and with affinity the same way a buff does.
///
/// Both factors multiply before the single `round`: rounding after each
/// would drop points that one combined multiply keeps.
pub fn scaled_stat_power(power: i32, level: u32, affinity: f32) -> i32 {
    (power as f32 * ability_stat_scale(level) * affinity).round() as i32
}

/// An HP `power` scaled by `ability_hp_scale(level)` and `affinity`, on the
/// same terms as `scaled_stat_power` — only the per-level rate differs.
pub fn scaled_hp_power(power: i32, level: u32, affinity: f32) -> i32 {
    (power as f32 * ability_hp_scale(level) * affinity).round() as i32
}

/// An authored damage range scaled for its invoker, on the same curve
/// `scaled_hp_power` puts the centre on.
///
/// **The spread scales proportionally rather than staying put**, or a
/// high-level ability becomes deterministic — the band would collapse to a
/// point exactly when the numbers get big enough for the variance to matter.
/// Scaling both ends through the same function is what keeps it proportional
/// without a second formula: `scaled_hp_power` is linear in its input, so the
/// width scales by the same factor as the centre.
pub fn scaled_range(
    range: crate::battle::DamageRange,
    level: u32,
    affinity: f32,
) -> crate::battle::DamageRange {
    crate::battle::DamageRange {
        min: scaled_hp_power(range.min, level, affinity),
        max: scaled_hp_power(range.max, level, affinity),
    }
}

/// The cooldown armed on a combatant right after it runs an ability whose
/// authored value is `cooldown`, floored at `floor` rounds. Called from both
/// `resolve_one_action` (party side, `floor = 0`, so the authored value is
/// untouched — this is what keeps `decompile` spammable) and `wild_retaliate`
/// (hostile side, `floor = tuning::ENEMY_ROUTINE_MIN_COOLDOWN`, so a mod
/// ability declaring no cooldown still can't fire every single round).
///
/// One function rather than the same `+1` written twice: a comment saying
/// two formulas "match" can't keep them in sync if one drifts, so this is
/// the sync.
///
/// The `+1` is armed before the effect resolves and read again at the end of
/// this same round by `tick_ability_cooldowns` — without it, that tick would
/// eat a round the invoker never actually got to wait out.
pub fn armed_cooldown(cooldown: u32, floor: u32) -> u32 {
    cooldown.max(floor) + 1
}

/// Index into `weights` that `roll` selects, treating each weight as the
/// width of a bucket. `roll` is expected in `0..weights.iter().sum()`.
///
/// `None` only when there is genuinely nothing to pick — an empty slice, or
/// every weight zero. An overshooting roll saturates to the last non-zero
/// bucket rather than returning `None`, so a caller that computes its range
/// wrong degrades to a valid pick instead of silently spawning nothing.
///
/// Pure, and takes the roll rather than the RNG, so the distribution can be
/// tested without a `Game`.
pub fn weighted_pick(weights: &[u32], roll: u32) -> Option<usize> {
    let mut remaining = roll;
    let mut last = None;
    for (index, &weight) in weights.iter().enumerate() {
        if weight == 0 {
            continue;
        }
        last = Some(index);
        if remaining < weight {
            return Some(index);
        }
        remaining -= weight;
    }
    last
}

/// Who an ability lands on. Which picker the UI opens for it — if any — is
/// `AbilityTarget::targeting`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityTarget {
    /// One party member the player picks.
    OneAlly,
    /// Every living party member, no picker.
    WholeParty,
    /// The front member of one enemy group the player picks.
    OneEnemyGroupFront,
    /// Every living member of one enemy group the player picks.
    WholeEnemyGroup,
    /// Every living enemy in every group, no picker.
    AllEnemies,
}

impl AbilityTarget {
    /// What this lands on, as the inspect page says it — a taxonomy label
    /// rather than authored content, so it lives here beside the variants
    /// and not in a `.ron`. Same call `AffinityKind::label` makes.
    ///
    /// **Exhaustive on purpose.** As a `_ =>` arm a sixth targeting mode
    /// would ship reading as one of the five that already exist, which is
    /// the trap `render/stack.rs::cell_mark` records.
    pub fn phrase(self) -> &'static str {
        match self {
            AbilityTarget::OneAlly => "one party member",
            AbilityTarget::WholeParty => "the whole party",
            AbilityTarget::OneEnemyGroupFront => "the front of one hostile group",
            AbilityTarget::WholeEnemyGroup => "one whole hostile group",
            AbilityTarget::AllEnemies => "every hostile",
        }
    }
}

/// The category an ability's magnitude belongs to, for affinity purposes —
/// one per `AbilityEffect` variant that *has* a magnitude. A invoker's
/// affinity for a category multiplies every magnitude in it (see
/// `Game::ability_affinity`).
/// The serde derives are for `talents::TalentNode::Affinity`, which names a
/// category in a `.ron` file.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum AffinityKind {
    Damage,
    Heal,
    Buff,
    Debuff,
    Drain,
}

impl AffinityKind {
    /// Display label, for the manifest screen. A taxonomy label rather than
    /// authored content, so it lives here and not in a `.ron` — same call
    /// as `Dir::label`.
    pub fn label(self) -> &'static str {
        match self {
            AffinityKind::Damage => "Damage",
            AffinityKind::Heal => "Healing",
            AffinityKind::Buff => "Buffs",
            AffinityKind::Debuff => "Debuffs",
            AffinityKind::Drain => "Drain",
        }
    }

    /// The perk that raises the player's affinity in this category.
    pub fn perk(self) -> crate::perks::Perk {
        match self {
            AffinityKind::Damage => crate::perks::Perk::DamageAffinity,
            AffinityKind::Heal => crate::perks::Perk::HealAffinity,
            AffinityKind::Buff => crate::perks::Perk::BuffAffinity,
            AffinityKind::Debuff => crate::perks::Perk::DebuffAffinity,
            AffinityKind::Drain => crate::perks::Perk::DrainAffinity,
        }
    }

    /// The affinity perk's per-level rate for this category — see
    /// `tuning::AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED`'s doc for why `Damage`
    /// and `Drain` need a different (higher) number than the other three.
    /// One lookup here rather than a match at the call site in
    /// `Game::ability_affinity`.
    pub fn perk_bonus_per_level(self) -> f32 {
        match self {
            AffinityKind::Damage | AffinityKind::Drain => {
                crate::tuning::AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED
            }
            AffinityKind::Heal | AffinityKind::Buff | AffinityKind::Debuff => {
                crate::tuning::AFFINITY_PERK_BONUS_PER_LEVEL
            }
        }
    }
}

/// What an ability does to each of its recipients.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AbilityEffect {
    /// Direct damage through `battle::resolve_attack`, so it scales with the
    /// user's ATK exactly as a `MoveDef` does, plus an optional status rider
    /// — the same shape a move already has.
    Damage {
        /// The centre of the damage band — see `spread`.
        power: i32,
        /// Half-width of the damage band around `power`. `#[serde(default)]`
        /// at 0 is a degenerate band, which is exactly the single
        /// deterministic number every ability dealt before ranges existed —
        /// so none of the shipped ability files needed editing and a mod
        /// gains damage ranges for free.
        #[serde(default)]
        spread: i32,
        #[serde(default)]
        status: Option<MoveEffect>,
    },
    Heal {
        /// The centre of the restored band — see `spread`.
        power: i32,
        /// Half-width of the band around `power`, on exactly the terms
        /// `Damage::spread` states: `#[serde(default)]` at 0 is a degenerate
        /// band, which is the single deterministic figure every heal restored
        /// before ranges reached this variant.
        ///
        /// Rolled through `battle::DamageRange` rather than a second formula,
        /// so the low end is floored at 0 and both ends scale with the invoker
        /// through `scaled_range` — a heal band widens with level instead of
        /// collapsing to a point, the same reason damage bands do.
        #[serde(default)]
        spread: i32,
    },
    Buff {
        kind: BuffKind,
        power: i32,
        duration: u32,
    },
    Debuff {
        kind: StatusKind,
        power: i32,
        duration: u32,
    },
    /// Damage through `battle::resolve_attack`, then the user is healed for
    /// `heal_fraction` of the damage it actually dealt, capped at its own
    /// maximum Integrity.
    ///
    /// Deliberately excluded from `scaled_hp_power`: the heal rides the damage,
    /// which already rides the user's ATK, so this scales with level without
    /// being scaled.
    Drain {
        /// The centre of the damage band — see `Damage::spread`.
        power: i32,
        /// Half-width of the damage band. See `Damage::spread`.
        #[serde(default)]
        spread: i32,
        /// Clamped to `0.0..=1.0` at load — see `AbilityDb::load_dir`. Bounded
        /// there rather than at use, so a `heal_fraction: 5.0` mod is a
        /// bounded ability instead of a bounded surprise inside a formula.
        heal_fraction: f32,
    },
    /// Clears each recipient's active status condition. Carries no fields.
    Cleanse,
    /// Spends a taming catalyst and rolls `taming::capture_chance` against
    /// the target group's front program — see `Game::attempt_decompile`.
    /// Carries no numbers of its own: the whole formula is `taming`'s, and
    /// duplicating any of it here would be a second copy to drift.
    Decompile,
    /// Arms a `components::ActiveFieldBuff` outside battle rather than
    /// resolving against a recipient in one. This is the field-only marker:
    /// there is no separate `field_routine: bool` on `AbilityDef`, an ability
    /// carrying this effect *is* field-only, and `AbilityDb::load_dir`
    /// rejects a `target` its `kind`'s `FieldScope` can't reach. What running
    /// costs lives on `AbilityDef::power_cost` like every other effect's —
    /// this variant carried its own `power_cost` field until the two cost
    /// fields were folded into one. `cooldown` is dead here, since battle
    /// round throttling does not apply outside a battle.
    FieldBuff {
        kind: FieldBuffKind,
        power: i32,
        /// Turns the armed buff lasts. `#[serde(default)]` to 0, which means
        /// **absent rather than instant**: the kinds that run until the party
        /// rests (`FieldBuffKind::runs_until_rest`) have no lifetime to
        /// author and must leave this off, and the two that do must set it.
        /// `field_buff_duration_mismatch` refuses both mistakes at load, so a
        /// 0 reaching `ActiveFieldBuff::remaining` is always an until-rest
        /// buff whose count nothing reads — never a counted buff that expires
        /// the turn it was run.
        #[serde(default)]
        duration: u32,
        /// How many turns pass between firings. `1` — the default, and what
        /// every buff did before this existed — means every turn. Only the
        /// two over-time kinds (`Regen` and `Trickle`) have a
        /// per-tick effect for it to space out; the rest are read on demand
        /// and ignore it.
        ///
        /// A separate knob from `power` because they are not
        /// interchangeable: halving the rate by doubling the interval leaves
        /// the same total across the duration but changes what the routine
        /// is *for*, and `power` is what affinity scaling multiplies.
        #[serde(default = "every_turn")]
        interval: u32,
    },
    /// Steps the party through exactly one solid cell along their current
    /// facing, landing on the open cell beyond — see `Game::phase_landing`.
    ///
    /// Carries no fields, and specifically no depth: one wall is a rule of
    /// the mechanic rather than an authored magnitude, because a two-deep
    /// run from the frame edge cuts a diagonal across the whole maze. A mod
    /// gets the routine, not a tunneller.
    Phase,
    /// Moves the party to any cell of the current frame the player points
    /// at, and kills them if that cell is solid — see `Game::wild_jump`.
    /// Carries no fields; the gamble is the whole mechanic.
    Jump,
}

impl AbilityEffect {
    /// Whether this effect runs outside battle only. `FieldBuff` was the
    /// original field-only marker and the two Stack movement effects joined
    /// it, which is why this is a predicate rather than a `matches!` at each
    /// of the four sites that need it: `Game::field_routines` (which builds
    /// the invocation list), `Game::battle_special_options` and
    /// `Game::wild_routine_ready` (which exclude it from both sides of a
    /// fight), and `use_ability`'s `unreachable!` arm (which is only
    /// unreachable *because* the other three agree with this one).
    pub fn field_only(&self) -> bool {
        matches!(
            self,
            AbilityEffect::FieldBuff { .. } | AbilityEffect::Phase | AbilityEffect::Jump
        )
    }

    /// Which affinity category this effect's magnitude falls under, or
    /// `None` for the two variants that have no magnitude to scale.
    /// `Decompile` is deliberately `None` rather than a category of its own:
    /// the `Decompiler` stat and `Perk::ExploitFocus` already move those
    /// odds, and a third multiplier there is a fourth spelling of the same
    /// thing.
    pub fn affinity_kind(&self) -> Option<AffinityKind> {
        match self {
            AbilityEffect::Damage { .. } => Some(AffinityKind::Damage),
            AbilityEffect::Heal { .. } => Some(AffinityKind::Heal),
            AbilityEffect::Buff { .. } => Some(AffinityKind::Buff),
            AbilityEffect::Debuff { .. } => Some(AffinityKind::Debuff),
            AbilityEffect::Drain { .. } => Some(AffinityKind::Drain),
            // The three that move nothing measurable. `Phase` and `Jump`
            // carry no magnitude at all — how far they reach is fixed by the
            // mechanic — so there is nothing here for an affinity to scale.
            AbilityEffect::Cleanse
            | AbilityEffect::Decompile
            | AbilityEffect::Phase
            | AbilityEffect::Jump => None,
            AbilityEffect::FieldBuff { kind, .. } => kind.affinity_kind(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AbilityDef {
    pub id: AbilityId,
    pub name: String,
    /// The one-line detail the ability picker shows. Authored rather than
    /// computed from `effect`, so a modder controls exactly how their
    /// ability reads.
    pub description: String,
    pub target: AbilityTarget,
    pub effect: AbilityEffect,
    /// Battle rounds before this ability can be used again by the same
    /// combatant, and the whole price of a Special — nothing in a battle
    /// charges a need. `#[serde(default)]` — 0 means usable every round,
    /// which for a battle effect means entirely unthrottled.
    #[serde(default)]
    pub cooldown: u32,
    /// Power spent running this routine, by *whoever runs it* — the invoker
    /// pays, so a companion's Special draws on the companion's reserve. Read
    /// through `routine_power_cost`, never directly, so the refusal in
    /// `Game::ability_unavailable` and the charge in `Game::spend_power`
    /// cannot disagree about the price.
    ///
    /// **Defaults to 0.0, not to a flat fee.** This field used to reach only
    /// `Phase` and `Jump`, and carried a nonzero default left over from a
    /// mechanic — commanding a companion — that stopped charging on
    /// 2026-08-08. Widening its reach to every routine in the game while
    /// keeping that default would silently price every ability a mod ships.
    /// Free-by-default is the only safe default once a field's audience
    /// widens; a mod that means to charge says so.
    ///
    /// `AbilityEffect::FieldBuff` reads it like everything else — the
    /// separate `power_cost` it carried inside the variant was folded in
    /// here. `Game::proc_wielded_routine` is the one exemption, and a
    /// deliberate one: its 25% proc rate is the whole of its price (see
    /// `tuning::WIELDED_ROUTINE_PROC_CHANCE`).
    #[serde(default)]
    pub power_cost: f32,
    /// How likely this ability is to be found already installed on a wild
    /// program — see `Game::spawn_wild_creature`. Relative within the pool,
    /// not a probability: weight 12 is twice as likely as weight 6, and the
    /// pool is normalised at pick time.
    ///
    /// `#[serde(default)]` to 0, which means "never spawns wild". Defaulting
    /// to exclusion is what keeps `priority_boost` and `decompile` — and
    /// every other ability reachable through a species or a research node —
    /// out of the pool without this module having to name them.
    /// Flat Accuracy this routine adds to its invoker's own, for the roll it
    /// makes and nothing else.
    ///
    /// **Read only by the effects that roll to hit** — `Damage` and `Drain`.
    /// Every other effect lands without a roll, so authoring this on one is
    /// inert; the census in `tests/assets.rs` is what holds the shipped
    /// roster to authoring it exactly where it means something.
    ///
    /// **Flat, not scaled by level**, unlike every magnitude beside it. A
    /// hostile's Evasion grows with the *zone* while the invoker's Accuracy
    /// grows with their *level*, and a player levels far faster than zones
    /// advance — so to-hit is already a solved problem late and an unsolved
    /// one early. A bonus that scaled would be largest exactly where it is
    /// least needed.
    #[serde(default)]
    pub accuracy: i32,
    #[serde(default)]
    pub wild_weight: u32,
    /// Marks this routine **exclusive**: it never enters `KnownRoutines`, no
    /// research node or species may grant it, and no blank Routine Disk can
    /// be etched with it. Its etched disk is reachable exactly two ways — a
    /// boss drop (`boss_drop`) or a Stack trader's rare shelf row.
    ///
    /// Opt-in exclusion, the same idiom `wild_weight` uses: the default is
    /// ordinary, so the pool is defined by the files that ask to be in it
    /// rather than by this module listing them. Knowledge is the only thing
    /// in this game that duplicates — you learn a routine once and etch it
    /// forever — so keeping these out of `KnownRoutines` is the whole gate,
    /// and `Game::etch_disk`, `Game::unlock_research` and
    /// `Game::extract_routine` are the three places that honour it.
    #[serde(default)]
    pub exclusive: bool,

    /// Whether this attack reaches past the front line. Read by **the basic
    /// attack path only** — `Game::basic_attacks_that_reach` — because that
    /// is the only place reach has ever been decided: a group standing
    /// behind `tuning::ENGAGED_GROUPS` can use only its ranged attacks and
    /// idles if it has none. A Special has never been gated on reach and
    /// still is not, so honouring this in `use_ability` would silently stop
    /// back-row hostiles running what they run today.
    ///
    /// `#[serde(default)]` to false, matching `MoveDef::ranged`, so a
    /// converted attack and an authored ability agree on melee-by-default.
    #[serde(default)]
    pub ranged: bool,
    /// Which species drop this routine's etched disk, each with its own
    /// 0.0-1.0 chance. Becomes the synthesised disk item's
    /// `ItemDef::droppable` (see `ItemDb::synthesise_etched_disks`), which
    /// is why the boss-drop path needs no engine code of its own:
    /// `Game::equipment_drops_for` already merges every item that names the
    /// dead species, and `award_loot` already rolls them.
    ///
    /// Nothing here requires the named species to be a boss. What makes
    /// these boss drops is that only bosses are named — the shipped set is
    /// checked by `every_exclusive_routine_is_dropped_by_a_boss`.
    #[serde(default)]
    pub boss_drop: Option<Vec<(crate::species::SpeciesId, f32)>>,
    /// Fires on an event rather than being chosen on a turn. `None` — the
    /// default, and what every ability shipped before this existed is —
    /// means the routine is offered as a Special and runs when picked.
    ///
    /// A field beside `effect` rather than an `AbilityEffect` variant
    /// because the axis is genuinely orthogonal: *when* a routine runs says
    /// nothing about *what* it does, and a passive should be free to Damage,
    /// Heal or Cleanse. As a variant this would need either one arm per
    /// effect it can pair with or a recursive `Passive { trigger, effect:
    /// Box<AbilityEffect> }`, which would force a delegating arm into every
    /// match on the enum. `cooldown`, `power_cost` and `wild_weight` are
    /// all orthogonal modifiers carried here for the same reason.
    #[serde(default)]
    pub triggers: Option<PassiveTrigger>,
}

/// What makes a passive routine fire.
///
/// A small closed set rather than a general event name, because each
/// variant is a specific point in `game::combat_round` that has to call
/// `Game::fire_passives`. A trigger nothing fires would be an authored
/// routine that silently never runs — the failure mode
/// `decompile_target_mismatch` refuses at load rather than allow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PassiveTrigger {
    /// A member of the holder's own group was dropped this round.
    ///
    /// A poor basis for anything a player *chooses* to carry: a dropped
    /// companion is dissolved and despawned at `end_battle` with no revive
    /// at any difficulty, so a routine that pays out here only ever pays a
    /// player who has already lost more than the payout is worth. It suits
    /// `deadman` — an exclusive last-stand routine is meant to be the thing
    /// you never want to see fire — and `AllyWounded` is what gear reaching
    /// for "the party is in trouble" actually wants.
    AllyDropped,
    /// A living member of the holder's own group crossed below
    /// `tuning::WOUNDED_INTEGRITY_FRACTION` this round.
    ///
    /// The recoverable crisis, and the one a player can be glad of: they
    /// wanted to survive the hit, and something they were carrying answered.
    /// *Crossed*, not *is under* — a member held at 20% for six rounds is
    /// one crisis and not six, which is a stronger rule than the cooldown
    /// alone would give.
    AllyWounded,
    /// A status condition landed on the holder this round.
    Afflicted,
    /// The round opened. Unlike the other two this is not a fact about the
    /// holder at all — nothing has to have *happened* — which is what makes
    /// it the one trigger a piece of gear can carry and be worth wearing.
    RoundStart,
}

impl PassiveTrigger {
    /// What makes this fire, as the inspect page says it. `AllyWounded`
    /// quotes `tuning::WOUNDED_INTEGRITY_FRACTION` rather than spelling the
    /// threshold out in prose, so retuning the crisis point cannot leave the
    /// page describing the old one — the reason `item_grant` reads the
    /// ability rather than the item's own text.
    ///
    /// Exhaustive for `AbilityTarget::phrase`'s reason.
    pub fn phrase(self) -> String {
        match self {
            PassiveTrigger::AllyDropped => "Fires when a party member is dropped".to_string(),
            PassiveTrigger::AllyWounded => format!(
                "Fires when a party member is driven below {:.0}% Integrity",
                crate::tuning::WOUNDED_INTEGRITY_FRACTION * 100.0
            ),
            PassiveTrigger::Afflicted => {
                "Fires when a status condition lands on the holder".to_string()
            }
            PassiveTrigger::RoundStart => "Fires at the start of every round".to_string(),
        }
    }
}

/// What running `def` actually costs its invoker: the authored
/// `power_cost` scaled by `tuning::ROUTINE_POWER_COST_MULTIPLIER`.
///
/// **This must stay the one expression for a routine's price.** Two call
/// sites reading `def.power_cost * MULTIPLIER` independently is exactly the
/// drift a shared doc comment cannot prevent — and here the two sites are a
/// refusal (`Game::ability_unavailable`) and a charge (`Game::spend_power`),
/// which disagreeing means a routine the picker offers and the invocation cannot
/// pay for, or one charged more than the row quoted.
pub(crate) fn routine_power_cost(def: &AbilityDef) -> f32 {
    def.power_cost * crate::tuning::ROUTINE_POWER_COST_MULTIPLIER
}

/// `AbilityEffect::FieldBuff::interval`'s default, and the only value that
/// existed before it did — a buff with no authored cadence fires every turn.
/// Never `0`: the cadence is a modulus, and a mod shipping `interval: 0`
/// would divide by it.
pub(crate) fn every_turn() -> u32 {
    1
}

impl AbilityDef {
    /// What an attack hits for and what it may leave behind — `(0, None)`
    /// for any ability that is not direct damage.
    ///
    /// One accessor rather than a `power` and an `effect` field mirroring
    /// `MoveDef`: the numbers live in `AbilityEffect::Damage` and reading
    /// them out is the only thing the basic-attack path ever wanted from a
    /// move. Returns the rider **by value** so a caller that rolls it away
    /// for the turn — see `Game::wild_attack` — does so on its own copy
    /// rather than mutating a definition shared by every program of the
    /// species.
    pub(crate) fn attack_parts(
        &self,
    ) -> (
        crate::battle::DamageRange,
        Option<crate::species::MoveEffect>,
    ) {
        match &self.effect {
            AbilityEffect::Damage {
                power,
                spread,
                status,
            } => (
                crate::battle::DamageRange::centred(*power, *spread),
                status.clone(),
            ),
            _ => (crate::battle::DamageRange::default(), None),
        }
    }

    /// Names the first field holding a NaN or infinity, if any. RON accepts
    /// bare `NaN`/`inf` literals and they survive every clamp downstream —
    /// cheaper to refuse the file at load than to defend every read. Same
    /// rationale as `ItemDef::non_finite_field`.
    fn non_finite_field(&self) -> Option<&'static str> {
        if !self.power_cost.is_finite() {
            return Some("power_cost");
        }
        if let AbilityEffect::Damage {
            status: Some(status),
            ..
        } = &self.effect
            && !status.chance.is_finite()
        {
            return Some("effect.status.chance");
        }
        if let AbilityEffect::Drain { heal_fraction, .. } = &self.effect
            && !heal_fraction.is_finite()
        {
            return Some("effect.heal_fraction");
        }
        None
    }

    /// `Decompile` is resolved by group index in `Game::attempt_decompile`,
    /// which only ever runs when the planned target is a
    /// `battle::SpecialTarget::EnemyGroup` — the shape `AbilityTarget`'s
    /// `Enemy` targeting produces. Any other `target` would still arm the
    /// cooldown and spend Power in `resolve_one_action`, then find no
    /// group index to act on and silently do nothing: the exact
    /// "wastes-the-round" failure mode this branch refuses loudly for
    /// everywhere else it can reach. Caught here instead, the same way
    /// `non_finite_field` catches a bad number before it reaches a formula.
    fn decompile_target_mismatch(&self) -> Option<&'static str> {
        if matches!(self.effect, AbilityEffect::Decompile)
            && self.target.targeting() != crate::battle::SpecialTargeting::Enemy
        {
            return Some(
                "effect: Decompile requires target: OneEnemyGroupFront or WholeEnemyGroup",
            );
        }
        None
    }

    /// A `FieldBuff` effect paired with a `target` its `kind`'s
    /// `FieldScope` can't reach. A `Run`-scoped kind always lands on the
    /// player (`FieldBuffKind::scope`, `Game::arm_field_buff`), so anything
    /// but `WholeParty` is a stated target the invocation never actually honours.
    /// A `Creature`-scoped kind may aim at a party member — `OneAlly` or
    /// `WholeParty` — but never an enemy: there is no mechanic to aim a
    /// field buff at a hostile. Caught here for the same reason
    /// `decompile_target_mismatch` is: refused loudly at load rather than
    /// silently doing nothing (or nothing coherent) at invocation time.
    fn field_buff_target_mismatch(&self) -> Option<&'static str> {
        let AbilityEffect::FieldBuff { kind, .. } = &self.effect else {
            return None;
        };
        match (kind.scope(), self.target) {
            (FieldScope::Run, AbilityTarget::WholeParty) => None,
            (FieldScope::Run, _) => {
                Some("effect: a Run-scoped FieldBuff requires target: WholeParty")
            }
            (FieldScope::Creature, AbilityTarget::OneAlly | AbilityTarget::WholeParty) => None,
            (FieldScope::Creature, _) => {
                Some("effect: a Creature-scoped FieldBuff requires target: OneAlly or WholeParty")
            }
        }
    }

    /// A `FieldBuff` whose `duration` contradicts what its `kind` does with
    /// one. Both directions are refused rather than quietly resolved, and
    /// neither is a tidiness check:
    ///
    /// - A kind that **runs until rest** never reads `duration`. Authoring
    ///   one states a lifetime the game will not honour — the mod author's
    ///   90-turn shield is permanent, and nothing tells them so. This is the
    ///   case `field_only_dead_fields` merely warns about for `cooldown`;
    ///   refused here instead, because a dead `cooldown` leaves a routine
    ///   that still works as written and a dead `duration` does not.
    /// - A kind that **counts down** with no `duration` arms at 0 and expires
    ///   on the tick it was run. That was silently possible before this
    ///   field defaulted, and it is a routine that spends Power for nothing.
    fn field_buff_duration_mismatch(&self) -> Option<&'static str> {
        let AbilityEffect::FieldBuff { kind, duration, .. } = &self.effect else {
            return None;
        };
        match (kind.runs_until_rest(), *duration) {
            (true, 0) | (false, 1..) => None,
            (true, _) => {
                Some("effect: this FieldBuff kind runs until the party rests and takes no duration")
            }
            (false, 0) => {
                Some("effect: this FieldBuff kind counts turns down and needs a duration")
            }
        }
    }

    /// A `Phase` or `Jump` effect paired with a `target` other than
    /// `WholeParty`. Both move the party as a body — there is no mechanic to
    /// phase one companion through a wall and leave the rest behind — so any
    /// other target is a stated aim the invocation never honours. Refused at load
    /// for the same reason `field_buff_target_mismatch` refuses its own
    /// mismatches rather than quietly doing something else at invocation time.
    fn movement_target_mismatch(&self) -> Option<&'static str> {
        if !matches!(self.effect, AbilityEffect::Phase | AbilityEffect::Jump) {
            return None;
        }
        (self.target != AbilityTarget::WholeParty)
            .then_some("effect: Phase and Jump require target: WholeParty")
    }

    /// A `triggers` set on a **field-only** effect. A `Phase` cannot fire
    /// when an ally drops: every `PassiveTrigger` names a moment inside a
    /// battle, and a field-only effect is by definition one that runs
    /// outside one. Refused rather than warned, unlike
    /// `field_only_dead_fields` — a dead `cooldown` still leaves a routine
    /// that works, where this leaves one that can never fire at all.
    fn passive_field_mismatch(&self) -> Option<&'static str> {
        (self.triggers.is_some() && self.effect.field_only())
            .then_some("triggers: a field-only effect has no battle moment to fire in")
    }

    /// `exclusive` set together with `wild_weight > 0`. Both fields claim to
    /// name this routine's *only* source — one says a wild carrier, the
    /// other says a boss drop or a Stack trader — and they cannot both be
    /// the only one. Refused at load because there is no way to pick a
    /// winner that isn't this module inventing a precedence rule the file
    /// never asked for.
    fn exclusive_source_conflict(&self) -> Option<&'static str> {
        (self.exclusive && self.wild_weight > 0)
            .then_some("exclusive: a routine cannot be both hunt-only (wild_weight) and exclusive")
    }

    /// Names `cooldown` if it's set on a **field-only** ability, where it
    /// does nothing: cooldown throttles re-use within a battle, and a field
    /// ability runs outside one. Not a load failure — the def still loads —
    /// just something worth a modder knowing rather than silently swallowing.
    ///
    /// `power_cost` is not checked here, because there is nothing dead
    /// about it any more: every routine is priced in it, field-only ones
    /// included. That is what folding the two cost fields into one bought —
    /// the exemption this function used to carry, for a field whose nonzero
    /// default made "the author wrote this" indistinguishable from "the
    /// author never touched it", has nothing left to except.
    /// `assets/abilities/README.md` tells a modder directly which fields
    /// apply to which field-only effect.
    fn field_only_dead_fields(&self) -> Option<&'static str> {
        if !self.effect.field_only() {
            return None;
        }
        (self.cooldown != 0).then_some("cooldown")
    }

    /// Whether this routine fires on a trigger rather than being chosen.
    ///
    /// A predicate rather than a `triggers.is_some()` at each site, for
    /// `AbilityEffect::field_only`'s reason: the four places that need it
    /// must agree. `Game::battle_special_options`, `Game::field_routines`
    /// and `Game::wild_routine_ready` all exclude these — a passive in a
    /// menu would be a row that either does nothing when picked or spends a
    /// turn doing what it was going to do free — and `Game::fire_passives`
    /// is what runs them instead.
    pub fn is_passive(&self) -> bool {
        self.triggers.is_some()
    }

    /// Bounds a `Drain`'s `heal_fraction` to `0.0..=1.0`. Applied at load so
    /// every reader downstream can treat it as a fraction, rather than each
    /// one re-clamping. Runs after `non_finite_field`, which has already
    /// refused a NaN — `clamp` would panic on one.
    fn clamp_ranges(&mut self) {
        if let AbilityEffect::Drain { heal_fraction, .. } = &mut self.effect {
            *heal_fraction = heal_fraction.clamp(0.0, 1.0);
        }
    }
}

impl AbilityTarget {
    /// Which picker the UI opens after this ability is chosen. `None` means
    /// it resolves immediately — there is nothing left for the player to
    /// choose.
    pub fn targeting(self) -> crate::battle::SpecialTargeting {
        use crate::battle::SpecialTargeting;
        match self {
            AbilityTarget::OneAlly => SpecialTargeting::Ally,
            AbilityTarget::OneEnemyGroupFront | AbilityTarget::WholeEnemyGroup => {
                SpecialTargeting::Enemy
            }
            AbilityTarget::WholeParty | AbilityTarget::AllEnemies => SpecialTargeting::None,
        }
    }
}

#[derive(Resource, Default)]
pub struct AbilityDb {
    abilities: HashMap<AbilityId, AbilityDef>,
}

impl AbilityDb {
    /// Loads every `*.ron` ability in `dir`. A malformed file is skipped
    /// with a returned warning rather than aborting the load, same as
    /// `ItemDb::load_dir`.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = AbilityDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<AbilityDef>(&text) {
                Ok(mut def) => {
                    if let Some(field) = def.non_finite_field() {
                        warnings.push(format!(
                            "skipped invalid ability file {path:?}: {field} is not a finite number"
                        ));
                        continue;
                    }
                    if let Some(reason) = def.decompile_target_mismatch() {
                        warnings.push(format!("skipped invalid ability file {path:?}: {reason}"));
                        continue;
                    }
                    if let Some(reason) = def.field_buff_target_mismatch() {
                        warnings.push(format!("skipped invalid ability file {path:?}: {reason}"));
                        continue;
                    }
                    if let Some(reason) = def.field_buff_duration_mismatch() {
                        warnings.push(format!("skipped invalid ability file {path:?}: {reason}"));
                        continue;
                    }
                    if let Some(reason) = def.movement_target_mismatch() {
                        warnings.push(format!("skipped invalid ability file {path:?}: {reason}"));
                        continue;
                    }
                    if let Some(reason) = def.passive_field_mismatch() {
                        warnings.push(format!("skipped invalid ability file {path:?}: {reason}"));
                        continue;
                    }
                    if let Some(reason) = def.exclusive_source_conflict() {
                        warnings.push(format!("skipped invalid ability file {path:?}: {reason}"));
                        continue;
                    }
                    def.clamp_ranges();
                    if let Some(dead) = def.field_only_dead_fields() {
                        warnings.push(format!(
                            "ability file {path:?} sets {dead}, which has no effect on a field-only ability"
                        ));
                    }
                    db.abilities.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid ability file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&AbilityDef> {
        self.abilities.get(id)
    }

    /// Every loaded ability, by id. `HashMap` iteration order is randomized
    /// per instance, so without this the picker's numbering would shuffle
    /// between sessions even though nothing about the files changed.
    pub fn all(&self) -> impl Iterator<Item = &AbilityDef> {
        let mut defs: Vec<&AbilityDef> = self.abilities.values().collect();
        defs.sort_by(|a, b| a.id.cmp(&b.id));
        defs.into_iter()
    }

    /// Every ability that can be found on a wild program, paired with its
    /// weight, ordered by id.
    ///
    /// Ordered for the same reason `all()` is: `HashMap` iteration is
    /// randomised per instance, so a weighted walk over an unordered pool
    /// would not be reproducible from a seed — and every wild spawn in this
    /// game is.
    pub fn wild_pool(&self) -> Vec<(&AbilityDef, u32)> {
        self.all()
            .filter(|d| d.wild_weight > 0)
            .map(|d| (d, d.wild_weight))
            .collect()
    }

    /// Every routine nobody can learn — the boss-drop and Stack-trader pool,
    /// ordered by id.
    ///
    /// Ordered for `wild_pool`'s reason: a Stack market's shelf is drawn
    /// from this with a seeded `StdRng` and has to survive a save and load,
    /// so a `HashMap`-ordered pool would put a different routine on the
    /// shelf every time the game was reopened.
    pub fn exclusive_pool(&self) -> Vec<&AbilityDef> {
        self.all().filter(|d| d.exclusive).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `files` as `.ron` into a fresh temp dir and loads an
    /// `AbilityDb` from it.
    fn load(tag: &str, files: &[(&str, &str)]) -> (AbilityDb, Vec<String>) {
        let dir =
            std::env::temp_dir().join(format!("feral_abilities_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(format!("{name}.ron")), body).unwrap();
        }
        let result = AbilityDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    const VALID: &str = r#"(
        id: "test_sweep",
        name: "Test Sweep",
        description: "Damage 6 to one group.",
        target: WholeEnemyGroup,
        effect: Damage(power: 6),
    )"#;

    /// A kind that runs until the party rests never reads `duration`, so a
    /// file authoring one has stated a lifetime the game will not honour.
    /// Refused rather than warned: the modder's 90-turn shield would be
    /// permanent and nothing would tell them.
    #[test]
    fn an_until_rest_field_buff_kind_may_not_author_a_duration() {
        let (db, warnings) = load(
            "until_rest_duration",
            &[(
                "test_timed_shell",
                r#"(
        id: "test_timed_shell",
        name: "Test Timed Shell",
        description: "d",
        target: OneAlly,
        effect: FieldBuff(kind: Mitigation, power: 12, duration: 90),
    )"#,
            )],
        );
        assert!(db.get("test_timed_shell").is_none());
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("runs until the party rests")),
            "{warnings:?}"
        );
    }

    /// The other direction. `duration` defaults to 0 now, and a counting kind
    /// armed at 0 expires on the tick it was run — a routine that spends
    /// Power for nothing.
    #[test]
    fn a_counting_field_buff_kind_must_author_a_duration() {
        let (db, warnings) = load(
            "no_duration",
            &[(
                "test_endless_regen",
                r#"(
        id: "test_endless_regen",
        name: "Test Endless Regen",
        description: "d",
        target: OneAlly,
        effect: FieldBuff(kind: Regen, power: 2),
    )"#,
            )],
        );
        assert!(db.get("test_endless_regen").is_none());
        assert!(
            warnings.iter().any(|w| w.contains("needs a duration")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_valid_def_loads_with_defaulted_optional_fields() {
        let (db, warnings) = load("valid", &[("test_sweep", VALID)]);
        let def = db.get("test_sweep").expect("valid ability should load");
        assert_eq!(def.name, "Test Sweep");
        assert_eq!(def.target, AbilityTarget::WholeEnemyGroup);
        assert_eq!(def.cooldown, 0, "cooldown defaults to none");
        assert_eq!(
            def.power_cost, 0.0,
            "an ability declaring no cost is free; free-by-default is what \
             stops a widened field silently pricing every mod's abilities"
        );
        assert!(warnings.is_empty(), "a valid def warns about nothing");
    }

    /// Regression for M11: a `Decompile` effect is resolved by group index
    /// in `Game::attempt_decompile`, which only runs for a
    /// `SpecialTarget::EnemyGroup` — the shape only `OneEnemyGroupFront` and
    /// `WholeEnemyGroup` targeting produces. Pairing it with anything else
    /// would arm the cooldown and then silently waste the
    /// round, so it must be refused at load time instead.
    #[test]
    fn a_decompile_effect_paired_with_a_non_group_target_is_skipped() {
        let mismatched = r#"(
            id: "test_bad_decompile",
            name: "Bad Decompile",
            description: "d",
            target: AllEnemies,
            effect: Decompile,
        )"#;
        let (db, warnings) = load(
            "bad_decompile",
            &[("test_sweep", VALID), ("bad", mismatched)],
        );
        assert!(db.get("test_sweep").is_some(), "the valid file still loads");
        assert!(
            db.get("test_bad_decompile").is_none(),
            "the mismatched pairing must not load"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Decompile"), "{}", warnings[0]);
    }

    /// `Trickle` is `Run`-scoped (`FieldBuffKind::scope`) — it always lands
    /// on the player, so authoring `OneAlly` states a target the invocation never
    /// actually reaches.
    #[test]
    fn a_run_scoped_field_buff_targeting_one_ally_is_skipped() {
        let mismatched = r#"(
            id: "test_bad_trickle",
            name: "Bad Trickle",
            description: "d",
            target: OneAlly,
            power_cost: 5.0,
            effect: FieldBuff(kind: Trickle, power: 4, duration: 20),
        )"#;
        let (db, warnings) = load(
            "bad_run_scope",
            &[("test_sweep", VALID), ("bad", mismatched)],
        );
        assert!(db.get("test_sweep").is_some(), "the valid file still loads");
        assert!(
            db.get("test_bad_trickle").is_none(),
            "a Run-scoped kind paired with anything but WholeParty must not load"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Run-scoped"), "{}", warnings[0]);
    }

    /// `Regen` is `Creature`-scoped and may aim at `OneAlly` or
    /// `WholeParty` — never an enemy target, since there is no mechanic to
    /// aim a field buff at a hostile.
    #[test]
    fn a_creature_scoped_field_buff_targeting_an_enemy_group_is_skipped() {
        let mismatched = r#"(
            id: "test_bad_regen",
            name: "Bad Regen",
            description: "d",
            target: WholeEnemyGroup,
            power_cost: 5.0,
            effect: FieldBuff(kind: Regen, power: 2, duration: 20),
        )"#;
        let (db, warnings) = load(
            "bad_creature_scope",
            &[("test_sweep", VALID), ("bad", mismatched)],
        );
        assert!(db.get("test_sweep").is_some(), "the valid file still loads");
        assert!(
            db.get("test_bad_regen").is_none(),
            "a Creature-scoped kind can't be aimed at an enemy"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Creature-scoped"), "{}", warnings[0]);
    }

    #[test]
    fn a_creature_scoped_field_buff_targeting_one_ally_loads_clean() {
        let good = r#"(
            id: "test_good_regen",
            name: "Good Regen",
            description: "d",
            target: OneAlly,
            power_cost: 5.0,
            effect: FieldBuff(kind: Regen, power: 2, duration: 20),
        )"#;
        let (db, warnings) = load("good_creature_scope", &[("good", good)]);
        assert!(
            db.get("test_good_regen").is_some(),
            "OneAlly is a legal target for a Creature-scoped kind"
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_field_buff_with_a_non_finite_power_cost_is_skipped() {
        let bad = r#"(
            id: "test_bad_power_cost",
            name: "Bad Power Cost",
            description: "d",
            target: OneAlly,
            power_cost: NaN,
            effect: FieldBuff(kind: Regen, power: 2, duration: 20),
        )"#;
        let (db, warnings) = load("bad_power_cost", &[("test_sweep", VALID), ("bad", bad)]);
        assert!(db.get("test_sweep").is_some(), "the valid file still loads");
        assert!(
            db.get("test_bad_power_cost").is_none(),
            "a NaN power_cost must not reach the invocation formula"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("power_cost"), "{}", warnings[0]);
    }

    /// `cooldown` throttles re-use within a battle; a field ability runs
    /// outside one, so `cooldown` is dead weight the loader should point out
    /// rather than silently accept. `cooldown` defaults to 0, so any nonzero
    /// value here is unambiguous authorial intent.
    #[test]
    fn a_field_buff_declaring_a_cooldown_loads_with_a_warning() {
        let noisy = r#"(
            id: "test_noisy_regen",
            name: "Noisy Regen",
            description: "d",
            target: OneAlly,
            cooldown: 3,
            power_cost: 5.0,
            effect: FieldBuff(kind: Regen, power: 2, duration: 20),
        )"#;
        let (db, warnings) = load("noisy_field_buff", &[("noisy", noisy)]);
        assert!(
            db.get("test_noisy_regen").is_some(),
            "a dead field is a warning, not a load failure"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("cooldown"), "{}", warnings[0]);
    }

    #[test]
    fn a_malformed_file_is_skipped_with_a_warning_and_the_rest_still_load() {
        let (db, warnings) = load(
            "malformed",
            &[("test_sweep", VALID), ("broken", "(this is not ron")],
        );
        assert!(
            db.get("test_sweep").is_some(),
            "one bad mod file must not take the others down"
        );
        assert_eq!(warnings.len(), 1, "exactly the bad file should warn");
        assert!(warnings[0].contains("broken"));
    }

    #[test]
    fn all_is_ordered_by_id() {
        let b =
            r#"(id: "b", name: "B", description: "d", target: OneAlly, effect: Heal(power: 1))"#;
        let a =
            r#"(id: "a", name: "A", description: "d", target: OneAlly, effect: Heal(power: 1))"#;
        let (db, _) = load("order", &[("b", b), ("a", a)]);
        let ids: Vec<&str> = db.all().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["a", "b"],
            "HashMap order is randomized per instance; the menu must not be"
        );
    }

    #[test]
    fn the_shipped_set_loads_clean() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join("abilities");
        let ron_file_count = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("ron"))
            .count();
        let (db, warnings) = AbilityDb::load_dir(&dir).unwrap();
        assert!(
            warnings.is_empty(),
            "the shipped set must not warn: {warnings:?}"
        );
        // Counted against the directory rather than a hardcoded number, so
        // shipping a new file never requires hand-updating this assertion.
        // What it still catches that no `tests::assets` coverage check
        // would: two files sharing an `id` silently overwrite one another in
        // `db.abilities.insert` (a `HashMap`, no warning on either side), so
        // the loaded count would fall below the file count with nothing
        // else to notice the collision.
        assert_eq!(
            db.all().count(),
            ron_file_count,
            "every .ron file in assets/abilities should have loaded as a distinct ability"
        );
        assert!(
            db.get(FALLBACK_ABILITY_ID).is_some(),
            "the fallback ability must ship, or every companion loses its Special"
        );
    }

    #[test]
    fn companion_slots_grow_one_per_level_up_to_the_cap() {
        // A slot a level against `CREATURE_MAX_LEVEL` of 6 lands a companion
        // on the same six slots it used to reach at level 12 — the ceiling
        // moved in level units and stayed put in power.
        let expected = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];
        for (level, slots) in expected {
            assert_eq!(
                companion_routine_slots(level),
                slots,
                "companion level {level}"
            );
        }
        assert_eq!(
            companion_routine_slots(50),
            crate::tuning::COMPANION_ROUTINE_SLOT_CAP as usize,
            "past the cap a companion stops gaining slots"
        );
    }

    #[test]
    fn player_slots_grow_one_per_five_levels_so_the_first_free_one_lands_at_5() {
        assert_eq!(
            player_routine_slots(1),
            1,
            "the starting slot holds decompile"
        );
        assert_eq!(player_routine_slots(4), 1, "still nothing free at 4");
        assert_eq!(
            player_routine_slots(5),
            2,
            "the first free slot arrives at 5"
        );
        assert_eq!(player_routine_slots(24), 5);
        assert_eq!(player_routine_slots(25), 6);
        assert_eq!(
            player_routine_slots(9_999),
            crate::tuning::PLAYER_ROUTINE_SLOT_CAP as usize,
            "the player has no level cap, so only this clamp bounds their slots"
        );
    }

    #[test]
    fn wild_weight_defaults_to_zero_so_an_ability_opts_in_rather_than_out() {
        let (db, _) = load("wild_default", &[("test_sweep", VALID)]);
        let def = db.get("test_sweep").expect("valid ability should load");
        assert_eq!(
            def.wild_weight, 0,
            "an ability that says nothing must never spawn wild"
        );
    }

    #[test]
    fn wild_pool_holds_only_the_opted_in_abilities_ordered_by_id() {
        let common = r#"(id: "zebra", name: "Zebra", description: "d",
            target: OneAlly, effect: Heal(power: 1), cooldown: 1, wild_weight: 4)"#;
        let rare = r#"(id: "apple", name: "Apple", description: "d",
            target: OneAlly, effect: Heal(power: 1), cooldown: 1, wild_weight: 1)"#;
        let (db, _) = load(
            "wild_pool",
            &[("test_sweep", VALID), ("zebra", common), ("apple", rare)],
        );
        let pool: Vec<(&str, u32)> = db
            .wild_pool()
            .into_iter()
            .map(|(d, w)| (d.id.as_str(), w))
            .collect();
        assert_eq!(
            pool,
            vec![("apple", 1), ("zebra", 4)],
            "weight-0 abilities are excluded, and HashMap order must not leak into a seeded roll"
        );
    }

    #[test]
    fn weighted_pick_is_proportional_to_the_weights() {
        let weights = [1, 3, 1];
        // Roll 0 lands in the first bucket; 1..=3 in the second; 4 in the third.
        assert_eq!(weighted_pick(&weights, 0), Some(0));
        assert_eq!(weighted_pick(&weights, 1), Some(1));
        assert_eq!(weighted_pick(&weights, 3), Some(1));
        assert_eq!(weighted_pick(&weights, 4), Some(2));
    }

    #[test]
    fn weighted_pick_handles_an_empty_pool_and_an_overshooting_roll() {
        assert_eq!(weighted_pick(&[], 0), None, "nothing to pick from");
        assert_eq!(weighted_pick(&[0, 0], 0), None, "all weights excluded");
        assert_eq!(
            weighted_pick(&[2, 3], 99),
            Some(1),
            "an overshooting roll saturates to the last real bucket, never panics"
        );
    }

    #[test]
    fn a_drain_with_a_non_finite_heal_fraction_is_skipped() {
        let bad = r#"(id: "test_bad_drain", name: "Bad Drain", description: "d",
            target: OneEnemyGroupFront, cooldown: 1,
            effect: Drain(power: 8, heal_fraction: NaN))"#;
        let (db, warnings) = load("bad_drain", &[("test_sweep", VALID), ("bad", bad)]);
        assert!(db.get("test_sweep").is_some(), "the valid file still loads");
        assert!(
            db.get("test_bad_drain").is_none(),
            "a NaN heal fraction must not reach the formula"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("heal_fraction"), "{}", warnings[0]);
    }

    #[test]
    fn an_out_of_range_heal_fraction_is_clamped_rather_than_refused() {
        let greedy = r#"(id: "test_greedy", name: "Greedy", description: "d",
            target: OneEnemyGroupFront, cooldown: 1,
            effect: Drain(power: 8, heal_fraction: 5.0))"#;
        let (db, warnings) = load("greedy_drain", &[("greedy", greedy)]);
        let def = db.get("test_greedy").expect("clamped, not skipped");
        let AbilityEffect::Drain { heal_fraction, .. } = def.effect else {
            panic!("expected a Drain effect");
        };
        assert_eq!(
            heal_fraction, 1.0,
            "a mod asking for 500% lifesteal gets 100%, bounded at load not at use"
        );
        assert!(warnings.is_empty(), "clamping is not a load failure");
    }

    #[test]
    fn a_cleanse_needs_no_fields() {
        let cleanse = r#"(id: "test_cleanse", name: "Cleanse", description: "d",
            target: WholeParty, cooldown: 1, effect: Cleanse)"#;
        let (db, warnings) = load("cleanse", &[("cleanse", cleanse)]);
        assert!(db.get("test_cleanse").is_some(), "{warnings:?}");
    }

    #[test]
    fn both_ability_scales_grow_per_level_and_stop_at_the_shared_cap() {
        assert_eq!(ability_stat_scale(0), 1.0, "no level, no bonus");
        assert_eq!(ability_hp_scale(0), 1.0, "no level, no bonus");
        assert!(
            (ability_stat_scale(crate::tuning::CREATURE_MAX_LEVEL) - 2.8).abs() < 1e-5,
            "a companion at its level cap runs stat routines at 2.8x — the same \
             figure it reached at level 12 before `HP_PER_LEVEL`'s K = 2 halved \
             the cap and doubled the rate"
        );
        assert!(
            ability_hp_scale(10) > ability_stat_scale(10),
            "an HP magnitude has Integrity's curve to keep pace with, not ATK's"
        );
        for scale in [ability_stat_scale as fn(u32) -> f32, ability_hp_scale] {
            assert_eq!(
                scale(9_999),
                scale(crate::tuning::ABILITY_SCALE_LEVEL_CAP),
                "the player has no level cap, so this clamp is the only bound"
            );
        }
    }

    #[test]
    fn scaled_power_scales_negative_magnitudes_too() {
        assert_eq!(
            scaled_stat_power(-4, 20, crate::tuning::AFFINITY_NEUTRAL),
            -28,
            "a sap must sharpen with level the same way a buff does"
        );
        assert_eq!(scaled_stat_power(0, 20, crate::tuning::AFFINITY_NEUTRAL), 0);
    }

    #[test]
    fn neutral_affinity_leaves_scaled_power_unchanged() {
        // The regression guard on the signature change: at 1.0 this must be
        // exactly the level-only result the three call sites produced before.
        let level_only = (8.0 * ability_stat_scale(20)).round() as i32;
        assert_eq!(
            scaled_stat_power(8, 20, crate::tuning::AFFINITY_NEUTRAL),
            level_only
        );
    }

    #[test]
    fn affinity_multiplies_on_top_of_the_level_scale() {
        // One combined multiply, not two rounds of rounding: 8 * 1.3 * 1.5
        // is 15.6 -> 16, where rounding twice gives 10 * 1.5 = 15.
        assert_eq!(scaled_stat_power(8, 1, 1.5), 16);
    }

    #[test]
    fn affinity_scales_negative_magnitudes_too() {
        // A sap is a negative-power buff (see scaled_stat_power's doc); an affinity
        // has to sharpen it, not flip or flatten it.
        assert_eq!(
            scaled_stat_power(-4, 20, 1.5),
            -(scaled_stat_power(4, 20, 1.5))
        );
    }

    #[test]
    fn only_magnitude_carrying_effects_have_an_affinity_category() {
        use crate::components::{BuffKind, StatusKind};
        assert_eq!(
            AbilityEffect::Heal {
                power: 8,
                spread: 0
            }
            .affinity_kind(),
            Some(AffinityKind::Heal)
        );
        assert_eq!(
            AbilityEffect::Damage {
                power: 6,
                spread: 0,
                status: None
            }
            .affinity_kind(),
            Some(AffinityKind::Damage)
        );
        assert_eq!(
            AbilityEffect::Buff {
                kind: BuffKind::Atk,
                power: 3,
                duration: 3
            }
            .affinity_kind(),
            Some(AffinityKind::Buff)
        );
        assert_eq!(
            AbilityEffect::Debuff {
                kind: StatusKind::Stun,
                power: 0,
                duration: 1
            }
            .affinity_kind(),
            Some(AffinityKind::Debuff)
        );
        assert_eq!(
            AbilityEffect::Drain {
                power: 10,
                spread: 0,
                heal_fraction: 0.5
            }
            .affinity_kind(),
            Some(AffinityKind::Drain)
        );
        // Cleanse has no number to scale; Decompile's axis is already occupied
        // by the Decompiler stat and Perk::ExploitFocus.
        assert_eq!(AbilityEffect::Cleanse.affinity_kind(), None);
        assert_eq!(AbilityEffect::Decompile.affinity_kind(), None);
    }

    #[test]
    fn armed_cooldown_floors_a_zero_authored_value_but_leaves_a_real_one_alone() {
        assert_eq!(
            armed_cooldown(0, crate::tuning::ENEMY_ROUTINE_MIN_COOLDOWN),
            2,
            "a mod ability declaring no cooldown is still floored, plus the +1"
        );
        assert_eq!(
            armed_cooldown(3, crate::tuning::ENEMY_ROUTINE_MIN_COOLDOWN),
            4,
            "a real cooldown above the floor is untouched but for the +1"
        );
        assert_eq!(
            armed_cooldown(0, 0),
            1,
            "the party side's floor is 0 — only the +1 applies, which is what \
             leaves `decompile` spammable"
        );
    }
}
