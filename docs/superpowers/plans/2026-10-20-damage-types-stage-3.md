# Damage types — Stage 3 plan (side effects, one release per type)

Spec: `docs/superpowers/specs/2026-10-19-damage-types-design.md`, § Side
effects and § Stage 3. Stages 1–2 (`v0.19.20`, `v0.19.21`) built the typed
path and tagged the gear. This stage adds the five side effects in spec order
(**Noise, Radiation, Magnetic, Thermal, Voltage**), and each one ships as its
own patch release. The work is in the engine crate only. There are no `.ron`
field changes and no save-format bump, because the new statuses are saved by
id like any other.

Paths are under `crates/engine/src/` unless stated. Read
`.claude/rules/seams-combat.md`, `seams-tactical.md` and `seams-sorties.md`
before starting.

## Shared decisions (all phases)

- **One hook.** `resolve_and_apply_attack` (`game/combat_damage.rs:272`) calls
  `self.type_side_effect(defender, ty, landed)` right after
  `apply_damage_typed`. It only runs when `landed > 0`, the swing has a type,
  and the defender is still above 0 HP, so a dead target never gets a
  "jammed" line. Every real attack on both battle models and in sorties
  already passes through this function (spec, § Where the attacker's type
  comes from). Nothing is hooked into `apply_damage_typed`, so status ticks,
  recoil and arcs never trigger a side effect.
- **Functional core.** A pure function
  `damage::side_effect(ty, resist_pct) -> Option<SideEffect>` decides what
  happens. It returns `None` when `side_effect_blocked` says so.
  `SideEffect` is concrete: `Arm { id, rounds, power }` or `Arc`. Magnetic
  has no variant because it acts inside mitigation. The shell
  (`type_side_effect`) reads the resistance through `resist_against(gear.resist, ty)`,
  then arms the status and logs it through `log_status_landing`, or applies
  the arc. No RNG draws anywhere.
- **Built-in statuses.** `damage::statuses() -> Vec<StatusDef>` builds the
  defs in Rust. `StatusDb::with_builtins(self) -> Self` adds them (a new
  `pub(crate)`; the `#[cfg(test)] insert` stays as it is). Both load sites
  call it: `game/lifecycle.rs:3904` (before `drop_unknown_status_riders`)
  and `game/creation.rs:392`. If an asset file uses a built-in id, the
  built-in wins and a warning is logged. A census in `damage/` checks that
  no shipped `assets/statuses/*.ron` uses a built-in id.
  `assets/statuses/README.md` gets one paragraph naming the built-ins.
- **Constants** go in `tuning/damage.rs`. The values are sensible, not tuned,
  and balance is a separate effort.
- **Threshold reach.** Shipped armour is at 30%, under
  `SIDE_EFFECT_BLOCK_THRESHOLD` (50), so no shipped gear blocks anything yet.
  Tests for blocking use test-built armour at 50%. Each CHANGELOG entry
  restates this.
- **Wording** is plain language, with no tick vocabulary and no occult terms.

## Per-phase gate (every phase)

TDD, with a commit for each green step. The tests cover the pure decide
function, plus engine tests on **both models**: one abstract swing (party
swing into a wild, or a wild into the party) and one tactical attack
(`tactical/turn.rs`). Each phase also needs:

- a landed hit that applies the effect;
- a target at 50% resistance that blocks it;
- a miss that applies nothing;
- a mutation check on the hook line.

The existing suite and `balance_sim` must pass **unchanged**, because
untyped fights and untyped mitigation paths must not move. Run fmt and
clippy (`--all-targets`), then `cargo test --workspace`. Release with the
deploy skill. After the deploy lands, fast-forward the worktree branch onto
`origin/main` before starting the next phase.

## Phase N — Noise: jammed (release 0.19.22)

1. New `StatusBehaviour::AccuracyCut(i32)`. `combatant_profile`
   (`game/combat_damage.rs:134`) scales `accuracy` by
   `100 - status_sum(AccuracyCut)` clamped to 0..=100, the same way
   `EvasionCut` scales evasion. Add it to the closed enum's doc and to
   `assets/statuses/README.md`.
2. Built-in status `jammed`. Name "Jammed", tag `JAM`, `Refresh`,
   `[AccuracyCut(NOISE_JAM_ACCURACY_CUT)]`. Inflict line: "{target}'s
   targeting is jammed!". Expire line: "{target}'s targeting clears.".
   Constants: `NOISE_JAM_ACCURACY_CUT = 30`, `NOISE_JAM_ROUNDS = 2`.
3. This phase also builds the shared hook, `SideEffect`, `with_builtins` and
   the census.
4. Extra test: a jammed attacker's `combatant_profile.accuracy` is cut, and a
   save→load round trip keeps `jammed` on the wild (the spec asked the
   builder to confirm the save path).

## Phase R — Radiation: corrupted (release 0.19.23)

1. Built-in status `corrupted`. Name "Corrupted", tag `COR`, `Refresh`,
   `[DamagePerRound]`, power `RADIATION_CORRUPTION_POWER = 2`, rounds
   `RADIATION_CORRUPTION_ROUNDS = 3`. Inflict line: "{target}'s data starts
   corrupting!". Tick line: "{target} loses {n} Integrity to corruption.".
   Expire line: "{target}'s corruption is purged.".
2. Healing still works, so no `HealBlock`. Test: a heal on a corrupted body
   lands.
3. The only change in this phase is a `SideEffect` arm. If it needs more
   than that, the shared hook is wrong; fix the hook and don't add a
   special case.

## Phase M — Magnetic: ignores deflection (release 0.19.24)

1. Pure function `damage::ignores_deflection(ty, resist_pct) -> bool`: true
   for Magnetic below the threshold.
   `mitigate_incoming_damage_typed` (`game/combat_damage.rs:543`) passes
   deflection `0` when it is true. `damage::side_effect` returns `None` for
   Magnetic.
2. **`resisted` must keep measuring only the type cut.** Today it compares
   the untyped mitigation with the typed one, and the deflection skip would
   leak into that comparison. Change the baseline to the typed path with the
   type's resistance zeroed. Use one private helper that takes a
   resistance override. Do not write a second copy of the formula.
   Test: a Magnetic hit into deflection armour with no Magnetic resistance
   gives `resisted == None` and lands `deflection` more damage than an
   untyped hit.
3. No new log line (spec: "only if unclear"). This is recorded in the
   CHANGELOG. The final review may still ask for one.

## Phase T — Thermal: heat (release 0.19.25)

1. New `StatusBehaviour::DamageTakenPercent(i32)`, which acts per stack.
   `mitigate_incoming_damage_typed` scales `dmg` up by
   `100 + status_sum(DamageTakenPercent)` before the general cut. When the
   sum is 0, it returns early on the existing path, so the numbers are
   byte-identical. Because the scale sits in that function, it covers every
   source: ticks, arcs, abilities and props. Add it to the enum and README.
2. Built-in status `heat`. Name "Overheating", tag `HOT`,
   `Stack { max: THERMAL_HEAT_MAX_STACKS = 3 }`,
   `[DamageTakenPercent(THERMAL_HEAT_DAMAGE_TAKEN_PCT = 10)]`, rounds
   `THERMAL_HEAT_ROUNDS = 3`. Inflict line: "{target} heats up!". Expire
   line: "{target} cools down.".
3. Tests:
   - a second Thermal hit adds a stack;
   - a hot target takes more damage from an untyped hit and from a bleed
     tick;
   - the hit that applies heat is not boosted by its own heat.

## Phase V — Voltage: arc (release 0.19.26)

1. Pure function `damage::arc_damage(landed) -> i32`:
   `landed * VOLTAGE_ARC_SHARE_PCT / 100`, with `VOLTAGE_ARC_SHARE_PCT = 50`.
   A result of 0 means no arc. `SideEffect::Arc` is returned for Voltage.
2. Gather function `Game::arc_neighbour(defender) -> Option<Entity>`, with
   no draws. It always picks from the defender's own side:
   - **Tactical** (the defender is on the active board): bodies whose
     footprint touches a cell in the ring around the defender's footprint.
     Use `tactical::reach::recipients_in_cells`. Keep only the same side,
     drop the defender, and take the lowest stable id.
   - **Abstract enemy:** the next living member after the defender in its
     group order, wrapping round.
   - **Abstract party:** the next occupied slot in slot order, wrapping
     round.
   - **Sortie:** read where `game/sortie.rs:703`'s `targets` comes from
     first. If it is the group order, the abstract branch already covers
     sorties. If not, that site passes its order to the same function as an
     override. Use one function with an optional order and no second picker.
3. Apply the arc with
   `apply_damage_typed(neighbour, arc, Some(Voltage))`. This is not a second
   `resolve_and_apply_attack`, so the arc cannot miss, crit, fumble or
   recoil. It does not chain, and the neighbour's own resistance cuts it.
   Log line: "The charge arcs to {name} for {n}." with `resisted_note`
   appended. A kill from an arc goes through `lower_hp`'s existing death
   path.
4. Tests:
   - each branch picks the documented neighbour;
   - no neighbour means no arc;
   - a lone target gets no arc;
   - the arc does not chain;
   - the arc respects the neighbour's resistance;
   - a 50%-resistant **primary** target blocks the arc.
5. Fallback, per the spec: if the tactical or sortie branch gets messy, ship
   Voltage as a plain typed hit. In that case ship only the pure function
   and the abstract branches, and move the rest to a follow-up release.
   **Ask the user before taking this fallback.**

## Finish (after Phase V)

- The final review is opus, over the whole Stage 3 diff
  (`git diff v0.19.21..HEAD`, written to a file). It checks the shared hook
  against the anti-goals: no untyped change, no parallel status system, no
  RNG draws.
- Set the spec status to "Stage 3 shipped" and update the `INDEX.md` line.

## Dispatch notes

- Use one sonnet subagent per phase, run one after another (they all write
  to `combat_damage.rs`). Deploy between phases.
- Use opus only for the final review. Per-task reviews are off unless the
  user asks for them.
- Every dispatch prompt forbids push and forbids `git checkout` of
  uncommitted work, and stages explicit paths only.
- Work in the worktree branch `worktree-damage-types-3`. Ask the user before
  creating it.
