// ─────────────────────────────────────────────────────────────────────────
// Combat
// ─────────────────────────────────────────────────────────────────────────

/// Relative weight each party member carries in a wild program's target
/// roll. Ranks are *soft*: everyone stays targetable, slot order only
/// changes the odds — a back-slot member is hit
/// `FRONT_SLOT_AGGRO_WEIGHT / BACK_SLOT_AGGRO_WEIGHT` times less often than
/// a front-slot one, never zero times. Bracing (see `Game::begin_defend`)
/// adds `DEFEND_AGGRO_WEIGHT` on top, which is what makes Defend a
/// party-level play rather than a selfish one.
pub const FRONT_SLOT_AGGRO_WEIGHT: u32 = 3;
pub const BACK_SLOT_AGGRO_WEIGHT: u32 = 1;
/// Raised from 4 on 2026-08-09, and the reason is the whole argument for
/// why this number is not free to drift back down.
///
/// These weights enter `Game::choose_wild_action` as `ln(weight)`, so what
/// Defend actually buys is `ln(7/3) = +0.85` of score — and the trained
/// policy's `est_damage_frac` term is worth about `-1.0` against a bracing
/// target, because *reducing incoming damage is what bracing is*. At 4 the
/// two cancelled and came out the wrong way round: bracing drew 40% of the
/// fire against 44% not bracing, so Defend was quietly counterproductive.
///
/// This is not fixable by pinning a feature — a damage-aware policy will
/// always have a reason to walk past the tank — nor by
/// `ENEMY_POLICY_TEMPERATURE`, which divides the prior and the learned term
/// alike and so cannot flip the sign. The prior has to be big enough to
/// win, and 7 is the smallest value that clears
/// `bracing_still_draws_more_fire_under_the_shipped_weights` with margin;
/// 6 flips the sign back but only by 0.02, which is inside the noise.
///
/// Bracing is therefore a stronger taunt than it was against the old
/// random-rolling enemy. That is the intended reading: Defend's old value
/// was an artifact of nobody on the other side thinking about it.
pub const DEFEND_AGGRO_WEIGHT: u32 = 7;

/// How many party slots count as the front line for `FRONT_SLOT_AGGRO_WEIGHT`
/// — the player plus the first two companions.
pub const FRONT_SLOTS: usize = 3;

/// Initiative baseline for a species whose `.ron` file omits `base_speed` —
/// the midpoint of the shipped roster's range, so an un-annotated mod
/// species is neither free initiative nor dead weight.
pub const DEFAULT_BASE_SPEED: i32 = 10;

/// The player's initiative baseline. A shade above `DEFAULT_BASE_SPEED`: the
/// player acts first against an average opponent, but loses the roll to
/// anything genuinely fast.
pub const PLAYER_BASE_SPEED: i32 = 11;

/// Each round every combatant rolls `base_speed + rng(0..=INITIATIVE_DIE)`
/// and acts in descending order. Sized so a 4-point speed gap still loses
/// the roll sometimes — order should be a tendency, not a lookup table.
pub const INITIATIVE_DIE: i32 = 10;

/// How often a wild program reaches for the status effect on the move it
/// just used — see `Game::wild_retaliate`.
///
/// Wild programs used to attempt their move's effect every single turn, so
/// a species with a nasty stun was that stun on repeat. At 20% a moveset
/// reads as something a program *can* bring to bear rather than all it does.
///
/// Gates the effect only; the move still lands its full damage. That is
/// deliberate — it changes how a fight feels without touching the damage
/// curves `balance_sim` projects, so retuning it does not move them.
///
/// Composes with each move's own `effect.chance` (`.ron` data, 0.3-0.5 on
/// the shipped roster) rather than replacing it, so an effect actually
/// lands on roughly 6-10% of wild attacks.
pub const WILD_ABILITY_CHANCE: f64 = 0.2;

/// How sharply a wild program acts on the trained battle policy — the
/// softmax temperature `Game::choose_wild_action` samples its
/// `(move, target)` pair at.
///
/// 1.0 is the trained distribution as-is. Below 1.0 sharpens it, and 0 (or
/// less) is argmax — always the best-scoring pair, which reads as an enemy
/// that never makes a mistake. A large value flattens it back towards the
/// uniform move roll and slot-weighted target roll the game had before
/// policies existed, so raising this is how a policy that trained too well
/// gets dialled back without retraining.
///
/// This is the **only** shipping control over how hard the learned enemy
/// plays, and it is deliberately not backed by `balance_sim`: that gate is
/// RNG-free and models no abilities, so a policy that makes real fights
/// substantially harder moves none of its curves. The arena
/// (`dev-arenas/`, `FERAL_DEV_ARENA=1`) is the instrument here.
///
/// Inert with no `assets/policies/enemy_battle.ron` installed: with no
/// weights there is nothing to sample and the baseline rolls run instead.
pub const ENEMY_POLICY_TEMPERATURE: f32 = 1.0;

/// Mitigation granted for the round by the Defend action, in **percentage
/// points** — the unit `Stats::mitigation` carries. Re-authored from the flat
/// 6 points of subtractive absorption a brace used to add: the value had to
/// move with the unit, or a brace would have silently become a rounding
/// error. Well under `MAX_MITIGATION_PERCENT` so a brace is a real cut
/// without approaching immunity on its own.
pub const DEFEND_MITIGATION_BONUS: i32 = 20;

/// Scales every `AbilityDef::power_cost` in the game, applied wherever one is
/// read — so `Game::ability_unavailable`'s refusal and `Game::spend_power`'s
/// charge move together and cannot disagree. `Phase` and `Jump` are covered
/// too; one knob, no exemptions.
///
/// **This is the lever for the whole routine-cost curve, and the reason it exists
/// is that the shipped numbers' *scale* is inherited while their *ordering*
/// is trusted.** The 55 files carrying a cost were priced against a Fatigue
/// pool that refilled at 0.08 a tick — cheap and renewable. They are now
/// spent out of an irreplaceable underground reserve. The relative ordering
/// between abilities is worth keeping; the absolute scale almost certainly is
/// not, and 1.0 is the starting point rather than a measured answer.
///
/// Ungated by `balance_sim`, which models no abilities at all. Tuning happens
/// in play; the instruments that can see this are `dev-arenas/` and a
/// session. Two levels with different costs: the whole curve moves by editing
/// this and rebuilding, a single ability by editing its `.ron` and
/// restarting — no rebuild, which is the loop that matters mid-session.
pub const ROUTINE_POWER_COST_MULTIPLIER: f32 = 1.0;

/// Below this Power ("Power" is the player-facing label for `PowerReserve.hunger`)
/// threshold, the player's own attacks start losing effectiveness — see
/// `battle::power_attack_multiplier`.
pub const LOW_POWER_ATTACK_THRESHOLD: f32 = 50.0;

/// The Integrity fraction a party member has to cross *downward* in one
/// round to count as wounded — what fires `PassiveTrigger::AllyWounded`.
///
/// A third rather than a half, because half is where an ordinary exchange
/// puts somebody most fights: a threshold that common makes the trigger
/// indistinguishable from `RoundStart` on anything with a cooldown, and
/// stops the moment reading as a crisis at all. Low enough to mean trouble,
/// high enough that the answer still has a round or two to matter — under a
/// quarter, most of what could fire has already stopped helping.
pub const WOUNDED_INTEGRITY_FRACTION: f32 = 0.33;

/// What the player's attack total falls to at zero Power. Between zero and
/// `LOW_POWER_ATTACK_THRESHOLD` the multiplier interpolates linearly from
/// this up to full strength — see `battle::power_attack_multiplier`.
pub const LOW_POWER_MIN_ATTACK_MULTIPLIER: f32 = 0.5;

/// Divisor on the wielded program's ATK and DEF when totalling the bonus it
/// lends the player (see `Game::wielded_stat_bonus`), floored at 1 per stat.
///
/// Was deliberately kept independent of the party's own passive divisor
/// rather than expressed in terms of it, on the grounds that the party buff
/// was a candidate for removal. It was removed on 2026-08-19 and this
/// survived unchanged, which is the whole of what that independence bought.
pub const WIELDED_PROGRAM_STAT_DIVISOR: i32 = 10;

/// Chance that a player strike also fires one of the wielded program's
/// installed routines (see `Game::proc_wielded_routine`). The proc costs
/// nothing — no Power, no cooldown — so this rate is the whole of its price,
/// and it is the one carve-out from `ROUTINE_POWER_COST_MULTIPLIER` reaching
/// every routine. Unguarded by any test: `balance_sim` models no abilities at all,
/// so neither this nor the magnitudes it fires can move a curve.
pub const WIELDED_ROUTINE_PROC_CHANCE: f64 = 0.25;

/// Damage a consumable does when thrown at the wild group rather than used
/// (`Game::throw_item`).
///
/// One point, and meant to stay a ruinous rate. The throw costs no round —
/// it cannot, since an `ActionKind` would have to appear in
/// `battle_action_options`, which both renderers build the prompt from —
/// so this is free damage, and the only thing keeping it from being worth
/// doing is how bad the exchange is. The consumables are finite and cost
/// Credits; a point apiece is a joke, not a strategy.
///
/// Never lethal regardless of this number: `throw_item` clamps so the
/// target keeps at least 1 HP. See its doc comment for why.
pub const THROWN_ITEM_DAMAGE: i32 = 1;

/// Chance that a *successful* jack-out still costs the player a parting
/// counter-strike. Whether the escape happens at all is a separate roll —
/// see `battle::jack_out_chance`.
pub const FLEE_COUNTERATTACK_CHANCE: f64 = 0.5;

/// Coefficients of `battle::jack_out_chance`. The base is the escape chance
/// at an even matchup — where your side's summed `Stats::power` equals
/// theirs — before the luck roll, so running from a fair fight usually
/// works. The chance scales linearly with that power ratio: outnumbered and
/// outgunned, you are much likelier to be pinned.
pub const JACK_OUT_BASE_CHANCE: f64 = 0.6;

/// Uniform random multiplier drawn fresh on every jack-out attempt, so the
/// odds are never a lookup: a hopeless-looking escape sometimes works and a
/// favourable one sometimes doesn't. Same spread as the per-creature stat
/// roll below, and for the same reason — enough wobble to matter, not
/// enough to overturn the matchup.
pub const JACK_OUT_LUCK_MIN: f64 = 0.8;
pub const JACK_OUT_LUCK_MAX: f64 = 1.2;

/// Hard bounds on the final jack-out chance, applied after the ratio and
/// the luck roll. Mirrors `CAPTURE_CHANCE_MIN`/`MAX`: no escape is ever
/// hopeless and none is ever certain. The floor is what keeps an ambush by
/// an overwhelming pack survivable — expensive, since every failed attempt
/// costs a full enemy volley, but never a guaranteed death.
pub const JACK_OUT_CHANCE_MIN: f64 = 0.10;
pub const JACK_OUT_CHANCE_MAX: f64 = 0.95;

/// Uniform random-roll range applied independently to each of a newly
/// created creature's stats (baked into `Stats` at spawn) and to its
/// growth rate (`Potential::growth_roll`) — see `Game::roll_potential`.
/// The "same species, different stats" mechanic; doesn't apply to the
/// player, who has no species.
pub const MIN_INDIVIDUAL_ROLL: f32 = 0.8;
pub const MAX_INDIVIDUAL_ROLL: f32 = 1.2;

/// How far a bred child's roll may drift either side of its better parent's,
/// uniformly (`breeding::inherit`).
pub const BREEDING_MUTATION: f32 = 0.04;
/// How far each generation lifts the ceiling on a bred roll above
/// `MAX_INDIVIDUAL_ROLL` — a gen-3 child may roll up to 1.29.
pub const BREEDING_GEN_STEP: f32 = 0.03;
/// The ceiling no generation passes, whatever the lineage.
pub const BREEDING_ROLL_HARD_CAP: f32 = 1.4;
/// The most a lineage lifts an attribute's top above the wild range, however
/// many generations deep.
pub const BREEDING_ATTRIBUTE_HARD_CAP: u32 = 3;

/// Rounds `Game::auto_resolve_battle` will drive a fight for, in either
/// combat model, before giving up and reporting `AutoResolve::Stalled` with
/// the fight still open.
///
/// **A stalemate guard, never a difficulty lever.** Neither live model has
/// a round cap of its own — a fight with no reachable target just hands
/// turns on forever — so this exists to bound the loop, not to shape how
/// fights play out; a real fight ends far short of it. The longest single
/// authored set-piece on record is `class-mirror` at 21 rounds
/// (`docs/measurements/2026-08-19-combat-model-slice-1.md`), and the arena's
/// own `ROUND_CAP` (`arena::run`) sits at 2000 for the same "stalemate, not
/// a real bound" reason. 200 is comfortably above anything a player's
/// fight can reach and comfortably below flooding the log with a stall.
pub const AUTO_RESOLVE_ROUND_CAP: u32 = 200;

// ─────────────────────────────────────────────────────────────────────────
// Combat resolution: to-hit, crit, fumble, mitigation
// ─────────────────────────────────────────────────────────────────────────

/// Accuracy and Evasion are **derived, never stored** — see
/// `battle::accuracy_of`/`evasion_of`. Both come off `SpeciesDef::base_speed`
/// (range 6..=14 across the shipped roster) plus entity level plus gear, so a
/// fast program both hits and dodges well. `atk` is deliberately absent from
/// both: feeding it to-hit *and* damage compounds quadratically and is the
/// most likely thing to break `balance_sim`'s curves.
pub const ACCURACY_PER_SPEED: f64 = 1.0;
/// See `ACCURACY_PER_SPEED`. Levelling buys accuracy; it never buys mitigation.
pub const ACCURACY_PER_LEVEL: f64 = 0.5;
/// See `ACCURACY_PER_SPEED`.
pub const EVASION_PER_SPEED: f64 = 1.0;
/// See `ACCURACY_PER_LEVEL`.
pub const EVASION_PER_LEVEL: f64 = 0.5;

/// What the attacker's Accuracy is multiplied by inside
/// `battle::hit_chance`, before the ratio against the defender's Evasion.
///
/// **A multiplier and not an addend, because the ratio form is scale-free
/// and must stay that way.** `k*acc / (k*acc + eva)` survives doubling both
/// sides exactly as `acc / (acc + eva)` does, so a zone that scales
/// everything by its tier multiplier still changes no hit rate. A flat
/// `+n` on accuracy would wash out as levels grow, which is the same
/// scale-dependence the difference form was rejected for.
///
/// 1.4 puts an even matchup at 0.583 rather than 0.5. The old 0.5 baseline
/// was the whole of "routines miss too often": measured against the shipped
/// roster, a player with no accuracy gear sat at 0.44-0.64 for the first ten
/// levels and both apex species — the lair guardians — are the fastest
/// things in the game and so the hardest to hit at every level. A basic
/// attack shrugs a miss off; a routine has already spent its Power and armed
/// its cooldown by the time the roll happens, which is why the same rate
/// reads as a routine problem.
///
/// **Necessarily symmetric.** `hit_chance` is a pure function of two
/// numbers and cannot know which side is the player, so hostiles take the
/// same edge — notably lifting them off `HIT_CHANCE_MIN`, which a
/// high-level player had pinned them to. The player's *asymmetric* edge is
/// the flat accuracy sources instead, summed by `Game::accuracy_bonus`.
pub const ATTACKER_ACCURACY_ADVANTAGE: f64 = 1.4;

/// Bounds on `battle::hit_chance`. The floor is what keeps
/// `balance_sim`'s `TURN_CAP` meaningful as stalemate detection rather than
/// as a fight-length cap: expected damage stays strictly positive, so a
/// timeout is a genuine stalemate.
pub const HIT_CHANCE_MIN: f64 = 0.25;
/// See `HIT_CHANCE_MIN`. Below 1.0 so no matchup is a guaranteed landing.
pub const HIT_CHANCE_MAX: f64 = 0.95;

/// The crit rate of every body without derived stats, and the player's at
/// base Entropy (`progression::DerivedBase::player`). Clamped to at most the
/// hit chance inside `battle::resolve_attack`, so a crit is always a hit.
/// Gear crit is deferred — a `crit` field on `EquipmentStats` that nothing
/// authors is an unused feature flag.
pub const CRIT_CHANCE: f64 = 0.08;
/// What a crit multiplies. The **rolled portion only** — doubling the total
/// would scale crits with levelling and with every `atk` source in the game.
pub const CRIT_ROLL_MULTIPLIER: i32 = 2;

/// The fumble rate of every body without derived stats, and the player's at
/// base Entropy. Clamped to at most `1 - hit_chance`, so a fumble is always a
/// miss.
pub const FUMBLE_CHANCE: f64 = 0.05;

/// Ceiling on a derived crit band, so Entropy cannot buy crits as the
/// default swing. The floor is zero.
pub const CRIT_CHANCE_MAX: f64 = 0.20;
// Below the hit floor, so a crit band never reaches it and a draw between
// the widest band in play and `HIT_CHANCE_MIN` is a plain hit for every
// pairing — the band test fixtures force a landing into.
const _: () = assert!(CRIT_CHANCE_MAX < HIT_CHANCE_MIN);
/// Ceiling on a derived fumble band. The floor is zero. Set where the
/// shipped Entropy reaches `CRIT_CHANCE_MAX` (100: 0.05 + 60 * 0.001), so no
/// point of Entropy buys fumble alone. Pinned against the real asset by
/// `entropy_reaches_both_caps_at_the_same_point`.
pub const FUMBLE_CHANCE_MAX: f64 = 0.11;

/// Where the four fumble rungs divide, against `d` — how deep into the
/// fumble band the roll fell, in `[0, 1)`. Weighted so the deep rungs are
/// rare: Exposed below the first, Recoil below the second, Opening below the
/// third, Crash above it. Rungs **replace** rather than stack; a cumulative
/// top rung is a run-ender.
pub const FUMBLE_RUNG_THRESHOLDS: [f64; 3] = [0.55, 0.85, 0.97];
/// Fraction of a fresh roll of the fumbler's own damage range that the
/// Recoil rung deals to the fumbler.
pub const FUMBLE_RECOIL_FRACTION: f32 = 0.5;
/// The status the Exposed rung arms on the fumbler. What it does (the
/// evasion cut) is that status's own definition in `assets/statuses/`.
pub const FUMBLE_EXPOSED_STATUS: &str = "exposed";
/// The status the Crash rung arms on the fumbler.
pub const FUMBLE_CRASH_STATUS: &str = "stun";
/// Percentage points of evasion a defender in partial cover gains against
/// the body shooting at them.
///
/// Read by `combatant_profile`'s own caller `defender_profile_against`, and
/// so kept here beside the Exposed rung rather than with the tactical grid's
/// constants: the two are the same kind of late multiplicative percentage on
/// the same number, one up and one down, and a reader comparing their
/// magnitudes should not have to go looking.
pub const COVER_EVASION_PERCENT: i32 = 50;

/// How long the Exposed rung lasts. One round, because
/// `ActiveStatus::landed_this_round` already exempts the round a condition
/// lands in — so a duration of 1 is exactly "until the fumbler's next turn",
/// which is the rung's wording.
pub const EXPOSED_DURATION_ROUNDS: u32 = 1;
/// How long the Crash rung's stun lasts. One round, for the same reason
/// `EXPOSED_DURATION_ROUNDS` is: it costs the fumbler their next action and
/// nothing beyond it.
pub const CRASH_DURATION_ROUNDS: u32 = 1;

/// The level a Striker's second swing arrives at. Above `ZONE_LEVEL_CAP_FLOOR`
/// on purpose: zone 1 caps at 6, so the second swing is unreachable before the
/// first breach and lands partway through sector 2. It is a threshold and not
/// a ramp — a Striker has one swing or two, never a fraction of a third.
pub const EXTRA_ATTACK_LEVEL: u32 = 8;
/// The ceiling on swings per actor per round, permanently. Every rung above
/// two multiplies a throughput curve that `balance_sim` fits by hand, and the
/// level cap is a correctness bound rather than a difficulty knob.
pub const MAX_ATTACKS_PER_ROUND: u32 = 2;

/// Ceiling on total mitigation, strictly below 100. Load-bearing twice: it
/// stops the damage path reaching immunity, and it is what keeps
/// `Stats::power`'s effective-HP denominator away from zero.
pub const MAX_MITIGATION_PERCENT: i32 = 75;

/// Ceiling on a wearer's total Deflection, the flat per-hit reduction
/// (`EquipmentStats::deflection`). Applied to the summed gear bonus, not per
/// piece. Unlike the percentage cap this is not what prevents immunity — the
/// floor of 1 in `Game::mitigate_incoming_damage` does that — it is what keeps
/// a stack of small affixes from erasing every hit under a boss-tier one.
pub const DEFLECTION_MAX: i32 = 6;

/// The player's damage range with no weapon equipped. Replaces
/// the flat `PLAYER_STRIKE_POWER` that used to be the player's one basic
/// strike; a weapon **overrides** this rather than adding to it.
pub const PLAYER_UNARMED_DAMAGE: crate::battle::DamageRange =
    crate::battle::DamageRange { min: 3, max: 7 };
