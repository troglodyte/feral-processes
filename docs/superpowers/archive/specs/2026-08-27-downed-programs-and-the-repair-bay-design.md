# Downed programs and the Repair Bay

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

A program that dies fighting beside you is destroyed. Under Forgiving that is
the harshest consequence in the game — harsher than the player's own death,
which is a reboot and a mild XP setback — and it is the one loss a player
cannot undo by playing better afterwards. This makes a Forgiving death of an
owned program **recoverable**, at the cost of a structure, a walk home and a
number of ticks; and it makes an expedition **committing**, by moving party
assignment into base space.

Permadeath is untouched throughout. Losing programs for good is what that
mode is.

## The thesis

Three changes, one premise: *the expedition is the unit of risk, and the base
is where you recover from it.*

1. A Forgiving death benches a program instead of destroying it.
2. A benched program is repaired only by a structure built for it.
3. Who is in your party is decided at base, not in the field.

Together these mean a party wipe four frames down is a walk home rather than
a lost run, and also that you cannot paper over a wipe by swapping in fresh
bodies mid-expedition.

## Part 1 — The downed state

### What changes

`Game::end_battle` (`game/combat_teardown.rs:283`) collects dead `Party`
members and calls `dissolve_tamed_program` on each. That call becomes a
branch on `DifficultyMode`:

- **Permadeath** — dissolve, exactly as today.
- **Forgiving** — the program survives at 1 HP, gains a `Downed` marker, and
  is retained out of `Party`.

The same branch applies at the **raid defender** site
(`game/base/upkeep.rs:413`), where a program destroyed defending a structure
goes through the same function. One rule everywhere: any Forgiving death of
an owned program is recoverable. The `Task` removal already there stays.

`strip_gear` still runs on both arms. Gear is the player's property and the
program was only wearing it; a benched program holding the party's best
weapon would be a second, invisible cost.

### Why `end_battle` and nowhere else

`BattleState::planned` indexes `Party` positionally and nothing may leave
`Party` mid-battle — deferred removal is the whole reason `end_battle`
exists. The bench is a removal, so it belongs there and not at the point of
death.

### Why no new role

`party::role_of` derives `Wielded` / `InParty` / `Staff` from `Party` and
`WieldedProgram`, and `Staff` is what is left over. A program out of `Party`
is therefore already staff, with no marker to assign and no verb to assign
it. `Downed` is not a fourth role — it is a *condition* on a staff member,
the same shape `OffShift` already has.

### The scheduler

`schedule_base_labour` (`game/base/work_orders.rs:873`) filters its `staff`
list down to `on_shift` before deciding postings. `Downed` joins that filter
on the same terms `OffShift` sits on today, and **without** the
`Carrying` exception — an off-shift body may still be holding goods, a body
that just died in a fight underground is not.

`drift_idle_staff` keeps the whole list, as it does for off-shift bodies:
it is what walks them somewhere.

### Cost, and the way out

A downed program still occupies a roster slot against `pet_capacity`. That
is the cost of a wipe and it is deliberate. It is not a dead end: both
`sell_companion` and `routines::extract_routine` operate on a downed program
and free the slot for value. The refusal text when a downed program is
offered to the party screen should say so, so the player is not left
guessing.

### Save

One additive `#[serde(default)]` bool on `CreatureSave`. **No
`SAVE_FORMAT_VERSION` bump** — the save is field-named RON and an additive
field behind a default costs nothing.

Note the standing trap: a RON round-trip test cannot catch a field that
never gets written. This needs a save-then-load test that asserts a downed
program comes back downed, not only that the RON round-trips.

## Part 2 — The Repair Bay

### The schema addition

`StructureDef` grows:

```rust
pub repair: Option<RepairDef>,   // #[serde(default)]

pub struct RepairDef {
    /// HP restored per tick to a downed program within `radius`.
    pub per_tick: i32,   // HP is i32 in `Stats`, so this is too
    /// Chebyshev distance in tiles.
    pub radius: i32,
}
```

This is deliberately the **third member of an existing family**, not a new
shape: `PowerRegenDef` is a rate and a Chebyshev radius aimed at the
player's Power; `ServiceDef` is a rate and a radius aimed at a program's
needs; `RepairDef` is a rate and a radius aimed at a downed program's HP.
Anything a reviewer wants to know about how it behaves is answered by
reading the other two.

`assets/structures/README.md` gains its section in the same change — the
schema docs are the reference for anyone modding, and that obligation is
not optional.

### The system

`repair_system` is `systems::power_regen_system` with the scan centred on
each downed program rather than on the party's base coordinates. It:

- restores `per_tick` HP, clamped to `max_hp`;
- removes `Downed` when HP reaches `max_hp`;
- logs the recovery once, on the transition — `set_machine_status`'s rule
  that entering a state is news and staying in it is not.

`per_tick` is mod-supplied, so it is clamped rather than trusted — but it is
an `i32`, matching `Stats::hp`, so it needs only **half** of
`power_regen_system`'s clamp: a negative value is floored at zero. There is
no non-finite case to guard, which is exactly why the integer type is worth
taking over `PowerRegenDef`'s `f32`. A field named "repair" must never
damage.

### Getting there

`drift_idle_staff` (`game/base/work_orders.rs:1464`) already has the
fall-through this needs: a body with an errand walks it (`step_off_shift`),
everyone else wanders. A `Downed` arm goes **above** the off-shift arm —
repair outranks an amenity — and walks the program toward the nearest
structure declaring `repair`, reusing `hauling::step_to_post` the way
`step_off_shift` does.

The existing `entry_tile` behaviour is load-bearing here and needs no
change: a program beaten on the surface carries that surface tile as its
`Position`, and `drift_idle_staff` is already what gives such a body a
base-space cell before it walks anywhere. A program downed in the Stack
arrives home by the same route a freshly tamed one does.

`offshift::Amenities` is the precedent for gathering the serving structures
once per beat rather than re-scanning per body.

### The shipped structure

One file in `assets/structures/`, modelled on `recharger_node.ron` — passive,
no `work` block, no posted worker, and **no research gate**. `build_cost`
must be affordable in zone 1, because the player answered that a downed
program is stuck until a Bay stands, and a gate you cannot afford in the
zone where you first need it is a dead run rather than pressure.

It should declare a modest `power_draw`, consistent with the other passive
structures, and no `power_supply`.

`tuning.rs` carries nothing here — `per_tick` and `radius` are authored data
like every other structure's numbers. What does belong in `tuning.rs` is
nothing at all for this part, and that is the correct answer: do not
duplicate `.ron` values into it.

## Part 3 — Party assignment requires base

`Game::add_companion` (`game/party.rs:448`) gains `self.require_base()?`,
joining that guard's existing caller list.

**The remove side is not symmetrical, and that is the trap here.**
`Game::remove_companion` (`game/party.rs:609`) returns `()`, not a `Result`,
and it is not only a player verb: `wield_program` calls it internally
(`game/party.rs:577`) to stand a member down before taking it as a weapon.
Putting `require_base` inside it would refuse wielding in the field as a
side effect, through a function the player never asked for.

So `remove_companion` stays **guard-free by construction** — the mover, not
the verb — and a new guarded public verb wraps it for the party screen. This
is `take_from_adjacent` / `give_to_adjacent`'s shape exactly: those are
guard-free, log-free and tick-free on purpose, so the caller owns the
refusal. The one app-core call site (`app/party.rs:139`) moves to the new
verb; `wield_program` keeps calling the mover.

App-core is smaller than expected: `Locality` in `app/group_menu.rs` is
already a three-state enum with a `Base` variant, so the party row changes
its `locality` field and nothing else. That table is the *only* source of
which rows show, and `locality` is a field in it precisely so it can be kept
in step with the engine's `require_base` caller list.

`docs/seams.md` carries the guard table and gains a row for each of the two
new callers. The table is the thing that keeps the two lists honest.

### The consequence, stated deliberately

Losing your party in the Stack now means walking out alone. There is no
swapping in a fresh program four frames down. This is the point of the
change, and it is only survivable because Part 1 means the programs you lost
are waiting at home rather than gone.

## Testing

Every test below carries a mutation check: delete the fix, watch the test
fail, restore. A test that passes with the fix removed is not coverage, and
this repo has shipped two of those.

**Part 1**
- A companion dying in a Forgiving battle is alive, downed, out of `Party`
  and still owned afterwards.
- The same death under Permadeath still despawns it.
- A raid defender's death takes the same two arms.
- Gear is stripped on both arms.
- `schedule_base_labour` does not post a downed program, and `LabourDemand`'s
  shortfall reflects its absence.
- A downed program survives a save/load **as downed** — a save-then-load
  test, not a RON round-trip, since a skipped field leaves that one green.

**Part 2**
- A downed program in range of a Bay recovers and loses `Downed` at full HP.
- One out of range does not.
- A negative `per_tick` does not damage.
- A downed program walks toward a Bay rather than wandering, including one
  whose `Position` is still a surface tile.
- With no Bay standing, a downed program stays downed indefinitely.
- The shipped Bay's `build_cost` is affordable at zone 1 — a census over the
  real assets, in `tests/assets.rs`.

**Part 3**
- `add_companion` and the new stand-down verb refuse outside base space, on
  the surface and in the Stack alike.
- **Wielding a program still works outside base space** — the regression this
  split exists to prevent, and the one a naive guard on `remove_companion`
  would cause silently.
- The party row is absent from the group menu outside base space.
- The `require_base` caller list and the `docs/seams.md` guard table agree.

**Gates**
- `cargo test --workspace`
- `cargo test -p feral-processes-engine balance_sim` — no balance constant
  moves here, so the curves must not either. A movement means something was
  changed that should not have been.
- `cargo clippy --workspace`, `cargo fmt`

## Out of scope

The **zone level cap** (one cap over the player and every companion, Kernel
Rings converted from buying levels to unlocking talent tiers, overflow XP
redirected to Perk Points at a sublinear rate) is a separate feature and gets
its own spec. It shares this one's thesis — power should be horizontal — but
it moves every `balance_sim` curve and needs its own measurement pass. It is
not blocked by this work and does not block it.
