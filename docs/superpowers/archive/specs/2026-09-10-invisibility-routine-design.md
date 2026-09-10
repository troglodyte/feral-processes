# A body nothing can address

**Status:** implemented. This file lives in `archive/specs/`, which is what
says so — the header is the thing that has rotted before, so read the
directory.

A battle routine that makes one body untargetable until it does something
aggressive. It is not a stat, not a status condition and not a buff — it
changes *who a picker may name*, and nothing else.

## Context

The game resolves fights through two models. The abstract one lines the wild
side up in groups and lets the player name a **group**; `ability_recipients`
turns `OneEnemyGroupFront` into `Game::front_of_group`, and the wild side
picks a party **slot** through `roll_enemy_target` (the baseline) or
`living_targets` (the trained policy). The tactical one puts bodies on a
board and names them individually — `Game::tactical_attack` for a swing,
`reach::recipients` for a routine's shape.

So "cannot be singly targeted" is not one rule in one place. It is a filter
applied at each of the five doors that *name* a body, and deliberately at
none of the doors that *resolve* against one. That split is the whole design:
an area routine keeps hitting a cloaked body because `reach::recipients`
never learns the word, and the abstract model's `WholeEnemyGroup` and
`AllEnemies` keep hitting it because they resolve a membership list rather
than picking a front.

The state is battle-scoped. `CombatBuff`, `StatusEffects` and
`AbilityCooldowns` are all wiped in `game/combat_teardown.rs` and appear
nowhere in `save.rs`, so a fourth component alongside them costs **no
`SAVE_FORMAT_VERSION` bump and no save field**.

## Decisions taken

- **Its own component, not a `BuffKind` and not a `StatusKind`.**
  `CombatBuff` holds exactly one wanted buff and bracing overwrites it, so a
  cloaked body that braced would uncloak itself and an ally's Rally would
  strip your cloak; `BuffKind` is also a stat axis folded into
  `combatant_profile`, and this moves no stat. `StatusEffects` is documented
  as conditions inflicted on you, always unwanted, and `Cleanse` would strip
  your own cloak.
- **A round cap on top of the break rule.** Pure until-you-attack has an edge
  where a fully cloaked group leaves `front_of_group` with nothing to hand a
  single-target attack. The cap is the ceiling; §4's never-empty rule is what
  stops a wasted round underneath it.
- **Breaks on the body's own aggression *and* on taking damage.** Not on
  moving, not on bracing, not on healing or buffing an ally, and not on a
  swing that missed it.
- **A cloaked body is still a wall.** `reach::movement_field` refuses a
  pass-through/destination split by design. A cell the player cannot path
  into is a tell, and a tell is the right amount of information.
- **Player-side content only.** No species declares it and it carries no
  `wild_weight`, so `roll_wild_routine` is untouched and **no seeded fight in
  the suite moves.** The engine rules are still written on the entity rather
  than on the side, so a mod that puts it on a species gets coherent
  targeting for free.
- **Nothing is hidden from the player's view.** A cloaked body draws faded,
  whichever side it is on. "The enemy approaches unseen" — omitting a hostile
  from `TacticalView::bodies`, masking its rung in the turn strip — is
  deliberately deferred; see "Open, deliberately".

## 1. The component

```rust
/// Battle-scoped: a body no picker may name until it acts.
///
/// Not a `CombatBuff`, which holds one wanted buff at a time and would be
/// clobbered by bracing, and not a `StatusEffects` entry, which is reserved
/// for unwanted conditions and would be stripped by `Cleanse`.
#[derive(Component, Clone, Copy, Debug)]
pub struct Cloaked {
    /// Battle rounds left, ticked down in `tick_combatant_upkeep`.
    pub remaining: u32,
}
```

Cleared in `combat_teardown.rs` beside `CombatBuff` and `AbilityCooldowns`,
on all three of the player, the hostiles and the companions.

### The upkeep tick

`Game::tick_combatant_upkeep` currently repeats the same three-call block
three times — once per hostile, once for the player, once per companion.
Adding a fourth call means writing it three more times, so extract the block
into one per-body `tick_one_combatant(entity, &label)` and call *that* three
times. This is the "targeted improvement in code you are working in" case,
not a drive-by refactor: the drift it heads off is a cloak that expires for
companions and never for the player.

## 2. The effect

```rust
Cloak {
    /// Battle rounds. Not scaled by the invoker — `Trickle`'s reason: this
    /// is a count against a fixed ceiling rather than a magnitude, so a
    /// level term would swamp whatever the file authors.
    duration: u32,
}
```

`AbilityDb::load_dir` refuses, with the logged warning every other malformed
file gets:

- any `target` but `OneAlly` or `WholeParty` — there is no mechanic to cloak
  an enemy, exactly as the creature-scoped `FieldBuff` kinds are refused an
  enemy-facing target;
- `duration: 0` — a routine that spends Power and does nothing.

Priced in `power_cost` and throttled by `cooldown` like every other battle
effect. It is not field-only and not passive, so `AbilityEffect::field_only`
and `is_passive` both answer as they do for `Damage`.

### The break census

```rust
/// Whether running this ends the invoker's own cloak.
///
/// Exhaustive rather than `_ => false` — `cell_mark`'s rule. A new effect
/// kind must state whether it is aggressive, or it ships as a routine you
/// can run from inside a cloak for free.
pub fn breaks_cloak(&self) -> bool
```

`true` for `Damage`, `Drain`, `Debuff`, `Decompile`. `false` for `Heal`,
`Buff`, `Cleanse`, `Cloak`, and every field-only variant (`FieldBuff`,
`Phase`, `Jump`, `Symlink`), which cannot be run in a battle at all.

## 3. One door, three callers

`Game::break_cloak(entity)` is the only place an action removes the
component. It logs once, on the transition only — `set_machine_status`'s
rule, so that three callers cannot come to disagree about whether entering a
state is news.

| Caller | Whose | What it covers |
| --- | --- | --- |
| `resolve_and_apply_attack(attacker, …)` | the attacker's | every creature-vs-creature swing, in both models and in a sortie — so basic attacks *and* the `Damage`/`Drain` routines that route through it are one hook |
| `use_ability` | the actor's | gated on `breaks_cloak()`; this is what catches `Debuff` and `Decompile`, which deal no damage and so reach no other hook |
| `apply_damage(target, …)` | the target's | "an area attack connected" |

`resolve_and_apply_attack` and `use_ability` will both fire for a `Damage`
routine. `break_cloak` is idempotent and logs on the transition, so the
second is a no-op rather than a duplicate line. Both call it **after** the
action resolves, so the swing's own line is logged before the reveal.

Everything not on that list is an **omission**, and the omissions are the
feature: moving, bracing, `Heal`, `Buff` and `Cleanse` all leave a cloak
standing.

**A miss is the asymmetric case, and deliberately so.** It still breaks the
*attacker's* cloak — `resolve_and_apply_attack` is entered whatever the roll
says, and committing to a swing is the aggressive act — but leaves the
*defender's* standing, because only landed damage reaches `apply_damage`. So
you cannot flush a cloak out by swinging at where you guess it is.

## 4. The five doors that name a body

**Abstract model.**

- `Game::front_of_group` — skips cloaked members, so `OneEnemyGroupFront`
  cannot reach one.
- `Game::roll_enemy_target` — cloaked party members leave the weighted pool.
- `Game::living_targets` — the same, for the trained policy.

**Tactical model.**

- `Game::tactical_attack` — refuses a cloaked target, alongside the
  adjacency check it already makes, returning `false` the same way.
- `Game::tactical_sides` — cloaked bodies leave the acting body's `targets`
  list, which is what keeps them out of `best_aim`'s scoring and out of
  `swing_at_best_neighbour`.

**Untouched, deliberately:** `reach::recipients` (so a blast that covers the
cell still hits it — collateral, not aim), `reach::movement_field` (a cloaked
body stays a wall), and the abstract model's `WholeEnemyGroup`/`AllEnemies`
arms of `ability_recipients` (they resolve a membership list, not a pick).

### The never-empty rule

**A cloak filter may never empty the pool it filters.** Where every candidate
at one of the five doors is cloaked, the filter is skipped for that pick and
the door answers as it would have without cloaks.

Without it, `roll_enemy_target`'s `total == 0` fallback returns the player
specifically — an all-cloaked party would find the player taking every blow,
which reads as a bug and not as a mechanic — and `front_of_group` answering
`None` would refuse the player's single-target attacks outright for the
rounds until the cap expires. The round cap is the ceiling on the situation;
this is what makes the rounds underneath it playable.

## 5. What the player sees

**Tactical.** `view::TacticalBody` gains `cloaked: bool`, and
`render/tactical.rs` multiplies that body's draw colour's alpha by
`CLOAKED_ALPHA`. `paint::Color` already carries alpha and `Painter::sprite`'s
tint multiplies, so a cloaked body's glyph *and* its sprite both fade through
one multiply at the draw site — no sixteenth `Painter` operation, and no
change to `paint.rs` at all.

**Abstract.** A third block in `Game::active_buffs`' existing per-holder loop,
producing an `ActiveBuffView` beside the `FieldBuff` and `CombatBuff` ones.
`Cloaked` carries no invocation-time name, so the label is fixed the way
`CombatBuff`'s is — `name: "Cloaked"`, `remaining: format!("{}t", …)`, and
`magnitude` an em dash, `PowerCell::Unrated`'s convention for *no answer*
rather than a bad one.

That row lands in the map's status column, which cannot grow: it holds 38.5
monospace cells and the widest shipped buff row already spends all but 3.8 of
them. The row must therefore be **measured**, not eyeballed — see §7.

## 6. Content

**`assets/abilities/detach.ron`** — id `detach`, name `"Detach Single"`
(`<Family> <Scope>`, held by
`every_shipped_ability_name_ends_in_the_scope_it_targets`), `target: OneAlly`,
`effect: Cloak(duration: 3)`, with a `cooldown` and `power_cost` in the band
its neighbours use.

Not `null_route`: that id is taken by the mass stun `kernel_privileges`
teaches. Not the stealth family either — `stealth_protocol` is the
encounter-damping field buff.

**`assets/research/process_detachment.ron`** — id `process_detachment`, name
"Process Detachment", `unlocks_abilities: ["detach"]`. A new node rather than
a home in an existing one: `deep_analysis` is the stealth-flavoured node but
its own description scopes it to "field routines that bend the odds of a
whole run rather than a single fight", which a battle routine would falsify,
and `segmentation` and `paging` are about Depots. `requires: ["self_exec"]`,
the node that already teaches a battle routine, and a bill naming only
`routine_disk` and `logic_wafer` — both in `self_exec`'s own bill, so the
"a research node's material bill may only name what that node's own
prerequisites can make" seam holds.

Because `target: OneAlly` reaches slot 0, the player can cloak themself or a
companion. A companion carries it only if the player etches a disk and
installs it, which falls out of the existing routine-slot machinery with no
extra work.

## 7. Tests

Engine, in `crates/engine/src/tests/`:

1. A cloaked party member is not returned by `roll_enemy_target` over many
   seeded rolls, and *is* returned once its cloak is broken.
2. The same for `living_targets` with a policy installed.
3. `front_of_group` skips a cloaked member and answers the one behind it.
4. **Never-empty:** with every living party member cloaked,
   `roll_enemy_target` still answers a real weighted pick and not the
   `total == 0` fallback; with every member of a group cloaked,
   `front_of_group` answers a member.
5. Each of the three `break_cloak` callers, one test apiece: a basic swing
   (attacker), a `Debuff` routine (actor, no damage dealt), an `AllEnemies`
   `Damage` routine landing on a cloaked body (target).
6. Each omission, one test apiece: a tactical step, `begin_defend`, a `Heal`
   on an ally, and a swing that **missed** a cloaked body all leave it
   cloaked.
7. The cap expires the cloak with no action taken, in both models — the
   tactical half is what proves `tick_one_combatant` is reached from the
   board's own round.
8. `tactical_attack` refuses a cloaked target; an area routine whose shape
   covers its cell still hits it, and breaks it.
9. A cloaked body still blocks `reach::movement_field`.
10. Load validation: an enemy-facing `target` and a `duration: 0` are each
    skipped with a warning, and the rest of the directory still loads.
11. `breaks_cloak()` is exhaustive over `AbilityEffect` — a compile-time
    property, so the test is the census in `tests/assets.rs` that every
    shipped ability's effect answers it.

gui:

12. The cloaked buff row **fits the status column**, measured through
    `paint::with_painter` — the memory note is that popup row width is
    testable headlessly, and this row is exactly the case that column has
    silently overflowed before.
13. A cloaked `TacticalBody` draws at reduced alpha, and an uncloaked one
    does not.

Gates: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`, and
`cargo test -p feral-processes-engine balance_sim` — `balance_sim` models no
abilities, so the expectation is **no curve moves**, asserted rather than
assumed.

Because no species and no spawn table changes, no seeded fight outside these
tests should move. A moved seeded test is a signal that a filter landed on a
resolve door rather than a naming door.

## 8. Documentation owed

- `assets/abilities/README.md` — the `Cloak` variant, in the effect list,
  with the two load-time refusals.
- `assets/research/README.md` — only if the new node needs a field it does
  not already document; it should not.
- `CHANGELOG.md` and a version bump at the merge, per one release per change.
- CLAUDE.md gets the seam's one sentence, the `seams` skill gets the trap,
  and the memory graph gets the argument — the three writes, in that order.

## Open, deliberately

**A hostile that approaches unseen.** The engine rules here are written on
the entity, so a modded hostile cloak already cannot be singly targeted; what
it does not get is the presentation — omitting it from `TacticalView::bodies`
so no renderer can leak it, and masking its rung in the turn-order strip so
the strip's `active` index stays valid. That is the larger half of the
original design and the whole of its test surface, and it is worth building
only once the targeting half has been played.

**The wild side's walk scoring.** `cell_score` measures distance to the
*enemy* side even when the acting body intends a helpful routine, so a wild
carrier of `bastion` or `checksum_repair` charges the player before healing
its packmate. Pre-existing, out of scope here, and named so the next person
to meet it knows it was seen.
