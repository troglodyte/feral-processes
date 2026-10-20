# Damage types — Stage 2 plan (typed gear and moves, visible)

Spec: `docs/superpowers/specs/2026-10-19-damage-types-design.md`, § Stage 2
and § Display. Stage 1 (`v0.19.20`) built the fields and the typed path; this
stage fills them in assets and shows them. No new fields, so no schema or
save-format change. Engine crate plus `assets/`; gui only if a width test
fails.

Paths are under `crates/engine/src/` unless they start with `assets/`.

## Decisions taken here (the spec left them to the builder)

- **One label for a band and its type.** `Game::damage_label(range, ty)`
  (beside `damage_range_label`, `game/crafting.rs:874`) gives `5–10 Thermal`
  or, untyped, exactly what `damage_range_label` gives today.
  `stat_summary` (`game/catalog.rs:633`) and the two body labels
  (`game/inspection.rs:2392,2525`, using `natural_profile_of`, which carries
  the type) call it. No third formatter.
- **Resistance in `stat_summary`:** after the scalar stats, one part per
  non-zero type in `DamageType::ALL` order: `+30% Thermal resist`. Signed
  like the rest, so a swap delta still reads.
- **The log names the type only when resistance cut the hit.** An uncut
  typed hit logs exactly as today. That keeps every existing log-string test
  green, and it is the one fact the player can act on.
- **The outcome carries what was resisted.** `AttackOutcome::Hit` and `Crit`
  gain `resisted: Option<Resisted>`, where `damage::Resisted { ty, amount }`
  is `Some` only when `amount > 0`. `battle::resolve_attack` builds `None`.
  `resolve_and_apply_attack` (`game/combat_damage.rs:272`) fills it in: it
  computes `amount = mitigate_typed(defender, rolled, None) -
  mitigate_typed(defender, rolled, ty)` before `apply_damage_typed`. Both
  calls are `&self` reads with no draw. It is the outcome that carries this,
  not a wrapper struct, as the spec says.
- **One suffix formatter:** `damage::resisted_note(Option<Resisted>) ->
  String`. It gives `" (3 stopped by Thermal resist)"`, or `""`. Every hit
  line appends it.
- **Resistance values:** 30% on each typed armour piece. That is below
  `SIDE_EFFECT_BLOCK_THRESHOLD` (50), so in Stage 2 no gear blocks a side
  effect. Stage 3 or 4 decides how the threshold is reached; this is noted
  in the CHANGELOG, not tuned here.
- **Previews stay untyped:** `charge.rs`, `gear_power.rs` and
  `prop_fight.rs` (the spec allows it). `copy_power` does not rate
  resistance.

## The sort

Weapons:

| Type | Items |
|---|---|
| Voltage | `arc_lance`, `interrupt_coil` |
| Thermal | `plasma_router`, `overclock_core` |
| Noise | `broadcast_storm`, `scatter_lance` |
| Magnetic | `siege_compiler` |
| Untyped (blades) | `monofilament_whip`, `kinetic_edge`, `shim_blade`, `shiv_routine`, `ragged_edge`, `recursion_blade`, `crash_handler`, `black_ice_pick` |

Armour, `resist: (<type>: 30)`:

| Type | Items |
|---|---|
| Thermal | `ablative_plating` |
| Noise | `static_mesh` |
| Radiation | `parity_weave` |
| Magnetic | `null_weave`, `nullsteel_plate` |
| Voltage | `hardened_shell` |
| Untyped | the rest |

Species moves (move name → type; any move not listed stays untyped):

- **Voltage:** scrapper Shock, glitch Flicker, sprite Pulse, polymorph Phase
  Burst, wintermute Cascade Logic.
- **Noise:** botnet Flood, drone Buzz, glitch Static Burst, zero_day Static
  Burst, proxy Spoof, sub_process Signal Spam.
- **Thermal:** sub_process Fork Bomb, overseer Purge.
- **Magnetic:** construct Lockdown, sentinel Lockout, cipher Encrypt,
  wintermute Absolute Authority.
- **Radiation:** crawler Corrupt, virus Corrupt, zero_day Fray.

The builder may move an entry that reads wrong. Every change goes in the
CHANGELOG list.

## Phase A — display plumbing (no asset edits)

One subagent, TDD, a commit per green step. Tests build typed defs on the
test's own `ItemDb`/`SpeciesDb`, as Stage 1's tests do. **Gate:** existing
tests pass. The only edits allowed to them are mechanical: `resisted: None`
in the 7 `AttackOutcome::{Hit,Crit} { dmg: N }` literals, and `{ dmg, .. }`
in patterns.

1. `damage_label` and `stat_summary` resistance parts. Tests: a typed band,
   an untyped band unchanged, an armour with two resistances in `ALL`
   order, and an untyped weapon's summary byte-identical to today.
2. `Resisted`, `resisted_note`, the new outcome field, and the fill in
   `resolve_and_apply_attack`. Pure tests on `resisted_note`. Engine tests:
   a typed hit into matching armour carries `Some` with the right amount; an
   untyped hit and a typed hit into no resistance carry `None`.
3. Append the note at every hit line: `party_swing_line`
   (`game/combat_round.rs:698`; it covers tactical through `turn.rs:548,769`),
   the ability line (`combat_round.rs:1802`) and the wild lines
   (`game/combat_enemy.rs:314,342`). Find the full set with
   `rg 'AttackOutcome::(Hit|Crit)' --glob '!tests/**'`. `resources.rs:431,470`
   only map to fx, so they just get `..`. Test: one log assertion per path
   (abstract party, wild, ability, tactical). Mutation-check each one by
   dropping its append.

## Phase B — tag the assets

One subagent, after A lands on the branch.

1. Edit the `.ron` files per the sort.
   - `damage_type: Some(Voltage)` goes inside the weapon's `EquipmentStats`
     tuple.
   - `resist: (thermal: 30)` goes inside the armour's tuple.
   - `damage_type: Some(…)` goes on the move tuple.
   - Check the spelling against `assets/items/README.md` and
     `assets/species/README.md`, which Stage 1 wrote.
2. A census in `tests/assets.rs`: every `DamageType` that a shipped weapon
   or move carries has at least one shipped armour resisting it. That holds
   the sort to the spec's counter-play intent. A load test that the tagged
   `arc_lance` parses as Voltage proves the spelling reaches the def.
3. Run the full gate. Expected fallout is real-asset tests where a party
   member wears one of the six typed armours against a typed wild move. The
   numbers there move by design. Update each assertion only after reading
   why it moved, and list every one in the handback.
4. `cargo test -p feral-processes-engine balance_sim`. Its party wears no
   typed armour, so the curves should not move. If one does, update it and
   record the before and after in the CHANGELOG entry (spec, § Stage 2).
5. Run the gui width tests (`cargo test -p feral-processes-gui`). The
   longest new summary is a typed weapon with affixes, and the swap row's
   stat column is the narrow one (the `seams-items` rule). If a width test
   fails, shorten the wording in `damage_label`/`stat_summary`. Never clip
   in the renderer.
6. READMEs: no schema change. Add one line each to `assets/items/README.md`
   and `assets/species/README.md` pointing at the census, so a new typed
   weapon knows it needs counter-armour.

## Finish

- `cargo fmt`, `cargo clippy --workspace --all-targets` clean, then the
  full gate.
- Screenshot check: open an inventory with a typed weapon and a typed
  armour via `--template` + `--screenshot`, and Read the PNG.
- The final review is opus, over the whole-branch diff written to a file.
- Release as a patch: "Weapons, armour and program moves now have damage
  types". The CHANGELOG lists the sort and states that no gear yet reaches
  the side-effect threshold. Use the deploy skill.
- Set the spec status to "Stage 2 shipped" and update the `INDEX.md` line.

## Dispatch notes

Sonnet for A and B; opus for the final review only. Every dispatch prompt
forbids push and `git checkout` of uncommitted work, and stages explicit
paths. Work in a worktree branch `worktree-damage-types-2`, and ask the user
before creating it.
