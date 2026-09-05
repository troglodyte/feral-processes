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
  argument. **`Game::dispatch_reach` measures the base, never the distance to
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

- **A route is one record with a `standing` flag, and a one-off is the flag
  turned off.** `routes::Route` carries the manifest, the destination, the
  leg, the countdown and `standing: bool` — there is no `RouteKind` and no
  second type, `WorkOrder`'s shape and its reason: the record stores what was
  asked for, never how it will be done. The two kinds differ at exactly one
  moment, the arrival home, and everything before it — refusals, countdown,
  the sale, predation, the save form, the hub row — is shared. **`sever_route`
  clears `standing` and nothing else**: the trip in flight keeps its cargo,
  arrives, sells and pays, so there is no refund path and no half-delivered
  state. A `Severed` leg variant is what would have needed all three.
- **A route and a squad leave through the same door.** `Game::dispatch_reach`
  → `DispatchReach {NoRelay, OffBase, AtRelay}` gates both, both on
  `StructureDef::dispatches_sorties`. A second `runs_routes` flag was
  rejected: it buys a second building, a second reach function, a second base
  menu row and a second screen to express one idea, and it weakens the seam
  above rather than reusing it — a mod's second dispatch structure gets
  routes for free exactly as it gets sorties. The `SortieReach` →
  `DispatchReach` rename is the cost and is compiler-checked, so unlike the
  two load-bearing renames it could not half-convert.
- **`Game::route_quote` is the one derivation the picker's preview and the
  sale at the far end share**, reached from the screen through
  `route_manifest_quote`, which resolves the destination's own `Temperament`
  so no screen holds one. `extraction_yield`'s argument on a second feature.
  The trap is that the formula is short enough for a second copy to compile,
  pass review and drift the first time a temperament multiplier moves — and
  a quoted figure that differs from the granted one is the feature lying on
  the one screen whose whole job is to answer what a manifest is worth before
  stock is spent that cannot be got back.
- **Predation is a named query plus pure geometry, and neither half touches
  `Game`.** `Standing::preys_on_routes` answers at `Hostile` alone —
  `refuses_service`'s rule, the arm the relations module's doc comment named
  before it existed — and `routes::settlements_near_route` answers which
  known towns lie within `ROUTE_PREDATION_RADIUS` of the **segment** anchor →
  destination. Radius-from-the-destination is the cheap version and is wrong
  in the way that matters: it makes the town you have to walk *past*
  irrelevant. **Predation is also the only thing in `run_routes` that may
  draw `GameRng`**, asserted by a test, or a route in flight would shift the
  seeded stream for every other test in the suite.

- **A sortie banks its downed programs and delivers them in `return_sortie`;
  an off-screen battle never writes the player's store.** A kill six screens
  away appearing in the pack the instant it lands makes the trip telemetry
  rather than travel, and lets the store's cap be consumed by something the
  player was not present for. The delivery loop **stops at the first
  refusal** rather than trying each remaining program: once the store is
  full it stays full, and `message_history` condenses repeats, so a test
  counting log entries cannot tell "said once" from "said eight times" — the
  `break` is what holds it. The roll is shared, not copied:
  `leave_downed_program` split into `downed_program_for` (the roll) and
  `push_downed_program` (the store), and the sortie calls the former,
  because a drifted second copy is exactly the trap `Perk::Teardown` fell
  into on the old material-drop path.
