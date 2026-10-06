# Seams: the player's implants

- **Implant effects are read only at a short list of sites, always through
  `ImplantDb`, and a def that is missing contributes nothing.** The sites:
  `implant_stats` (`game/derived.rs`), `power_multiplier` / `load_of`
  (`systems.rs` hunger, `game/crafting.rs`), the `RoutineSlots` hook
  (`game/combat.rs`), `CaptureOdds` (`game/unlocks.rs`), the Trace rise
  (`game/trace.rs`), `DropBoost` and `XpBoost` (`game/combat_rewards.rs`),
  `roll_implant_battle_start` (`game/combat.rs`, `tactical/turn.rs`) and
  `dead_mans_switch` (`game/combat_damage.rs`). **The trap**: `Implants` holds
  bare ids, so a new reader that unwraps `db.get` or matches ids itself
  panics or misprices on a save that names a removed or modded def.
  `implant_defs` drops the `None`, which is also what lets the player still
  remove an implant whose def is gone.
- **A player with no implants draws nothing from `GameRng`.**
  `roll_implant_battle_start` rolls only when something can land (over the
  Load cap, or an installed `BattleStartStatus` downside). **The trap**: an
  unconditional roll shifts the RNG stream for every save, which moves every
  seeded test and `balance_sim` number with no implant in sight.
- **`DeadMansSwitch` hooks `apply_damage`, not `lower_hp`.** It runs after
  mitigation and before the HP comes off, only for the player, only in an open
  battle, once per battle, and it costs Power. **The trap**: hooking
  `lower_hp` would also save the player from `kill_outright` (dying inside
  rock) and every non-battle source, which are meant to kill.
