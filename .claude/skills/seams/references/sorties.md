# Sorties

- **`ProgramRole` has a fourth variant, and a sortie's five consequences are
  omissions rather than checks.** `Sortie` sits **between `InParty` and
  `Staff`**, keeping `Staff` as what is left over. `schedule_base_labour`,
  `drift_idle_staff`, `base_entropy_system`, `needs_drain_system` and
  `position_is_honest` all narrow through `party::role_of` and all want
  `Staff` exactly, so an away program leaves the labour pool, the drift, the
  occupancy set, the needs drain and the surface map in one edit. Needs
  **freeze**, exactly as a party member's do. Widening `role_of`
  deliberately fails to compile at all three appliers; **`party::Roles` is
  the `SystemParam` bundling the four resources that decide a role**, a thin
  adapter and never a second copy, so a fifth source is one edit rather than
  three signatures.
- **A sortie battle is spawn, fight and despawn inside a single call, and
  that is the whole feature's load-bearing decision.** No bevy system runs
  mid-method, so the opposition is never observed by the map, the examine
  ray, `cull_to_cap` or `ensure_local_population` — which keeps this out of
  the "which space is this?" bug class rather than teaching four systems a
  new space. **The despawn is unconditional and covers every hostile, living
  or dead**, and is the last thing the method does. **The trap is that
  narrowing it to corpses passes an entity-count test whose squad won**: the
  fixture has to drop its own bodies so the battle ends with survivors
  standing. `run_sorties` is a `Game` method for `run_dig_crew`'s reason and
  carries its guard for `nest_aggro_tick`'s.
- **The fights are real by construction, and the trained policy is
  deliberately not used.** `resolve_and_apply_attack` and `use_ability` are
  both `BattleState`-free, so the ladder, the bands, mitigation, affinity,
  Power and cooldowns are the ones a fight in front of the player uses. The
  policy's *selection* reads `BattleState` and exists to make fights against
  **the player** interesting, so off-screen it would model an absent
  audience. Both sides run one stated rule instead — highest-priority
  affordable Special off cooldown, else a basic attack, targeting the front
  — with `field_only`, passives and `Decompile` excluded. A bonus derived
  from a routine loadout was **considered and rejected**: it is a second
  model of what routines are worth, and the Relay screen and the battle
  screen would price the same three Specials differently.
- **A Relay is identified by `StructureDef::dispatches_sorties`, not by its
  id.** The research gate really is pure data, but `has_relay` is not —
  naming `"relay"` in Rust puts content in the engine and makes a mod's
  second dispatch structure impossible. `issues_contracts`' shape and
  argument. **`Game::sortie_reach` measures the base, never the distance to
  the mast** — `base_pos` then `BaseGrid::is_floor`. Three states,
  `NoPost::BoxedIn`'s rule.
- **The board is derived and the whole record travels.** Recomputed from the
  world seed, `ZoneLevel` and the clock epoch, the Broker board's rule: no
  save field, nothing to scum, rotates on its own, and **no `GameRng` draw**
  — asserted by comparing the stream, since a stable board passes against
  one that draws and discards. Each draw folds its own seed through
  `derive::index`, never `%`, and is taken **without replacement**. An empty
  catalogue is an empty `Vec` and **not** `None`, which means no Relay.
  `Sortie` and `SortieSave` store the whole resolved `SortieDef` —
  `ActiveContract`'s rule reaching the save format.
- **`sortie_duration` reads the risk *offset* and has no term for the
  squad.** Against the absolute band every trip in a deep sector grows for
  no reason the player could name; with a strength term the feature becomes
  a throughput multiplier that scales with itself. One computation, quoted
  by the board and run by the countdown — `BuildOrderRow`'s rule.
- **Membership rides `CreatureSave::sortie_index`; `SortieSave` carries no
  member list.** `party_slot`'s precedent — entity ids are not stable across
  a save. A **named struct, never a positional tuple**. Both fields are
  additive behind `#[serde(default)]`, so **no `SAVE_FORMAT_VERSION` bump**,
  and the test packs the stripped RON back into a **real save**: a round trip
  alone leaves a `#[serde(skip)]` green. A record whose members all failed to
  load is **dropped**, not restored empty. `Sorties` is **not** wiped by
  `enter_next_zone`: the base travels and so does what it sent out.
- **Every dispatch refusal lands before anything is spent**,
  `commit_caravan_basket`'s rule, asserted **per refusal** — a single test
  over one of nine passes against eight paths that never spend anyway.
  Provisioning is charged in `Game::currency()`, role-derived rather than
  named in Rust and the same figure the stock strip shows.
- **A squad's departure is a cue the engine queues and forgets, and it is
  base space's.** `dispatch_sortie` and `return_sortie` each queue one
  `resources::TransitCue` per body — a glyph and the cells it walks — drained
  by `Game::take_transits`, `take_effects`' counterpart. **Nothing in the
  simulation stands on those cells**: giving an away program a live
  `Position` is the one thing `ProgramRole::Sortie`'s five omissions exist to
  avoid. Departure and arrival are the **same cue with its ends swapped**, so
  there is no direction field. A separate value from `VisualEffect`, whose
  whole shape is one *tile*. `base_space::transit_path` walks
  `BaseGrid::walkable` **alone with no blocked set** — the door is where the
  Home stands — and **a walk that does not exist is nothing, never a straight
  line**: a body on unwalkable base space is the ordinary state of a program
  adopted on the surface that has not drifted, so a fixture that skips
  standing its bodies in the pocket proves nothing. The draw is behind
  **`show_effects`**, the raid flash's gate. A cue is drained on the frame it
  is queued, so a return nobody is home to see is dropped.
- **`award_companion_xp` and `roll_work_resource_drop` are extractions, not
  copies.** The first holds the growth roll, the cap, the XP buff, the tally
  and the routine unlocks; the second holds a `Perk::Teardown` term added to
  the roll rather than drawn for. The drop **reports rather than grants** —
  that is the whole of what differs. The rest of `award_loot` — Trace, the
  `Terminate` feat, boss records, fragments — is deliberately **not** shared:
  those belong to a fight the player was in.
