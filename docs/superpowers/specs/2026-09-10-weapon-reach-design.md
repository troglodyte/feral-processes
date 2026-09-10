# A swing that lands on more than one body

**Status:** designed, not implemented. This file lives in `specs/` rather
than `archive/specs/`, which is what says so — the header is the thing that
has rotted before, so read the directory.

A weapon that declares how wide its swing is. Not a routine, not a second
action and not a new key: you attack the way you already attack, and the
swing is wide when the weapon's charge is up.

## Context

A weapon today is `EquipmentStats { atk, mitigation, decompiler, damage,
accuracy, evasion }`, and the only thing it changes about a swing is the
band — `Game::attack_range` takes the weapon's `damage` as an **override**
of the wielder's natural attack, keyed on the weapon carrying a band and not
on the slot being filled. Both swing paths then land on exactly one body:
`party_member_swing` resolves `retarget(group)` to `front_of_group`, and
`tactical_attack` refuses anything past Chebyshev distance 1.

Area already exists, but only for routines. `AbilityShape` (`Single`,
`Line`, `Cone`, `Radius`) and `AbilityRange` sit on `AbilityDef` and are
read **in tactical fights alone**; the abstract model says the same thing in
its own vocabulary through `AbilityTarget::WholeEnemyGroup` and
`AllEnemies`. `AbilityTarget::derived_shape` is the conversion between them,
and `AbilityDef::tactical_shape` is the one door that reconciles an authored
figure with a derived one.

So the feature is not a new mechanic. It is a weapon being allowed to say
one word the routine system already understands, and each combat model
converting that word the way it already converts it.

`ItemDef::grants` is deliberately not the seam: it names a **passive**
routine and is refused at load if it names one that could never fire, and an
invocable AoE is a different feature with a different price (Power, a
picker row, a turn).

## Decisions taken

- **The field is on `ItemDef`, not on `EquipmentStats`.** Three reasons, and
  each is independently sufficient. `EquipmentStats` is scaled four ways by
  `Game::copy_bonus` and a reach must not scale — `damage`'s own rule is
  that *a tier sharpens what an item does and never hands it a stat it never
  had*. `is_empty` and `has_upside` **destructure** the struct on
  `cell_mark`'s rule, and a non-numeric field has no answer for either. And
  the def is already reachable at the swing site: `Equipment::weapon →
  EquippedItem → GearCopy → ItemId → ItemDef`. On the def, the reach is off
  all four scaling axes *by construction* rather than by a rule someone has
  to keep remembering.
- **The weapon authors an `AbilityTarget`, and the shape is derived.**
  `WholeEnemyGroup` gives the abstract model its recipients through
  `ability_recipients` and the battle map its `Radius {
  TACTICAL_GROUP_RADIUS }` through `derived_shape`, with no new derivation
  written. The arrow only points this way: a `Line` has no group meaning, so
  a shape-first field would need a shape→group mapping invented out of
  nothing. A weapon that wants real geometry authors `shape:` as an
  override, exactly as `AbilityDef` does, reconciled through one door.
- **Breadth, never distance.** The reach carries no `AbilityRange`. In the
  abstract model you still name a group; on a board `tactical_attack`'s
  adjacency gate is untouched. This is what holds the aiming UI at **zero
  change in both crates** — no new key, no new `Mode`, no `ALL_MODES` entry
  — and it means a `Radius` is centred on the body you aimed at while a
  `Line` or `Cone` is cast from you toward it, which is `reach::recipients`'
  existing rule that the aim is a bearing for a line and a cone and a
  destination for a blast.
- **The recharge is authored per weapon, in the `.ron`.** Multi-target on
  every swing is a straight throughput multiplier that nothing in the
  balance gate would see. The rate is content, not difficulty, so it lives
  in the item file rather than `tuning.rs`.
- **Friendly fire on a board is inherited, not decided.**
  `reach::recipients` never reads `Hostile`, and the seam names the side
  filter as the trap: one line, reads as an obvious bug fix, breaks nothing
  that compiles, and deletes the reason a shape is worth aiming. A cleaving
  weapon catches your own companions standing beside the target. That is the
  tactical price of the reach, and the abstract model has no equivalent
  because it has no geometry.
- **`Game::copy_power` gains no term.** The rating is *absolute*, priced
  against one reference wearer derived from `balance_sim`'s swept curve, and
  the seam is explicit that the fix for a disagreeing swap picker is not to
  make the column contextual. A reach weapon paid for with a weak band will
  therefore rate below its single-target peer. The answer is a **line on the
  gear page**, not a number: the rating prices the swing, the page says what
  the swing lands on, and the player decides.
- **`balance_sim` gains no term either.** It models the abstract model with
  a mid-grade fitted party that carries no reach weapon, so the curves will
  not move. The shipped reach weapons are priced **below** the single-target
  ladder on raw throughput so they cannot become the standard endgame
  weapon, which is what keeps `Game::level_cap`'s lower bound — a
  correctness bound against the *geared* clear requirement — measured
  against the right weapon. The gap is written into `docs/measurements/`
  rather than modelled.

## 1. The schema

```rust
// items_db.rs, on ItemDef
/// What one swing of this weapon lands on, past the body it is aimed at.
/// Absent on everything that is not a weapon, and refused at load on
/// anything that is not.
#[serde(default)]
pub reach: Option<WeaponReach>,
```

```rust
// items.rs
pub struct WeaponReach {
    /// Enemy-facing and plural: `WholeEnemyGroup` or `AllEnemies`.
    pub target: AbilityTarget,
    /// The battle map's geometry. `None` derives one from `target`, which
    /// is what every shipped reach weapon does.
    #[serde(default)]
    pub shape: Option<AbilityShape>,
    /// Rounds between wide swings, counted on the holding model's own round
    /// counter. Never 0 in shipped content — see §7.
    pub recharge: u32,
}
```

`#[serde(default)]` on the `ItemDef` field, so every existing item file and
every mod keeps parsing untouched. The save stores an `ItemId`
(`EquippedItemSave`) and resolves the def at load, so the reach reaches no
save field and costs **no `SAVE_FORMAT_VERSION` bump**.

### The load refusal

`ItemDef::unreachable_reach(&self) -> Option<String>`, called from
`ItemDb::load_dir` in `ungrantable_ability`'s exact position and shape:
warn, `continue`, never panic. It refuses three things.

- A `reach` on an item with no `equipment: (Weapon, _)`. Armour that cleaves
  is an authoring mistake that would otherwise be silently inert.
- A `target` that is not `WholeEnemyGroup` or `AllEnemies`.
  `AbilityTarget::is_ally_facing` covers two of the three; the third,
  `OneEnemyGroupFront`, is a reach that reaches nobody.
- A `shape` of `AbilityShape::Single`. Same fault as above, spelled in the
  other vocabulary.

## 2. One door for the decision

```rust
// Game
pub(crate) fn swing_reach(&self, actor: Entity) -> Option<WeaponReach>
```

Answers `Some` when `actor` wears a weapon whose def declares a reach **and**
its `ReachCharge` is not holding it back. Both combat models call it and
neither re-derives it.

The **conversion** is each model's own, because they already have converters
that disagree and the seam says so: `ability_recipients` is the abstract
model's, `reach::recipients` is the board's, and folding the second into the
first needs a `SpecialTarget::Cell` variant that every abstract match in
three crates would have to reject.

| model | converter | aim it is given |
| --- | --- | --- |
| abstract | `ability_recipients(actor, reach.target, &SpecialTarget::EnemyGroup { group })` | the group already being swung at |
| tactical | `reach::recipients(shape, from, at)` | the cell of the adjacent body already being swung at |

## 3. The recharge

A battle-scoped `components::ReachCharge { ready_on: u32 }` on the swinger,
inserted on demand and cleared in `Game::clear_battle_status_effects`
alongside `StatusEffects`, `CombatBuff`, `AbilityCooldowns` and `Cloaked` —
the same three-way sweep over the player, every hostile still in the fight
and every party member. That door is reached from `finish_fight`, so both
models get it from one place, and being battle-scoped is what keeps it out
of `save.rs` entirely.

**Armed once per turn, never once per swing.** `attacks_for` loops a
Striker's second swing *inside* the turn, so a per-swing arm would silently
double what the weapon is worth and nothing in `BattleState::planned` or
app-core would ever learn the feature existed. This is
`proc_wielded_routine`'s rule, applied verbatim.

## 4. The sweep

Every body the converter returns takes its own
`resolve_and_apply_attack(actor, body, Swing::plain(range))` — the same call
the primary body takes, with the same band from the same
`Game::attack_range`. No new damage path, so `Game::apply_damage`,
`effective_mitigation`, affinity and every rung of the fumble ladder hold
with no new code.

Three consequences, taken deliberately rather than patched:

- **The fumble ladder rolls per body.** A wide swing is a sloppy swing, and
  the cost needs no new constant.
- **A Recoil rung can kill the swinger mid-sweep**, so the loop breaks on
  `!self.creature_alive(actor)` — the guard `party_member_attacks` already
  carries for the same reason.
- **The reap runs after the sweep, not per body.** `reap_dead_members` and
  `reap_tactical_dead` stay where they are, so a swing that empties a group
  cannot re-letter it half way through its own sweep.

The primary body is swung at first and is never dropped from the recipient
list — a blast centred on it contains it, and a line or cone cast toward it
passes through it.

## 5. What the player sees

The gear inspect page (`Game::gear_detail`, the one derivation, opened with
`[I]` from every list that names gear) gains **one row**: what the swing
lands on and how often. Every figure on that page is a call, so the row
reads its two facts off the same `WeaponReach` the swing does.

That page has **zero headroom and no scroll** —
`the_tallest_gear_page_fits_its_popup` in gui's `render/inventory.rs`
is what says it fits, and `GEAR_AFFIX_ROW_CAP` is where the
affix rows were bought from when they landed. The row has to be bought the
same way, and the census re-run against the tallest shipped reach weapon.

A wide swing also wants to read as one in the log. The existing
`party_swing_line` is one wording for both models by design; the sweep logs
one line per body through it, which is what an area routine already does.
No new line shape.

## 6. Content

Two weapons, not a ladder.

| | target | recharge | band | source |
| --- | --- | --- | --- | --- |
| mid-zone craftable | `WholeEnemyGroup` | short | below the single-target ladder | recipe, gated on its zone's material |
| late rare drop | `AllEnemies` | long | below the single-target ladder | drop, rare tier |

Enough to prove both target arms and both ends of the recharge. A per-zone
ladder is a second change, after the mechanic has been played — and neither
of these has been played, which is the standing gap a green suite does not
close.

## 7. Tests

Engine, unless noted.

1. `a_weapon_reach_lands_on_every_member_of_the_group` — abstract model, a
   group of three, one swing, three bodies damaged.
2. `a_weapon_reach_lands_on_every_body_in_the_blast` — tactical, bodies
   placed around the target, all inside the radius take damage.
3. `a_weapon_reach_catches_a_companion_standing_beside_the_target` — the
   friendly-fire rule, held explicitly so the side filter cannot be added as
   a tidy-up. Bodies carrying no components beyond what the sweep needs,
   following `recipients`' own test.
4. `a_recharging_reach_swings_narrow_until_it_is_ready` — swing, swing
   again next round, second lands on one body.
5. `a_reach_arms_once_a_turn_and_not_once_a_swing` — a wielder with
   `attacks_for` > 1 gets one wide swing, not two.
6. `a_reach_charge_does_not_survive_the_fight` — armed, fight ends, next
   fight's first swing is wide.
7. `a_fumble_that_kills_the_swinger_stops_the_sweep`.
8. `a_reach_on_a_non_weapon_is_refused_at_load` / `_on_an_ally_facing_target_`
   / `_on_a_single_shape_` — three refusals, asserted **separately**: one
   test over one path passes against the other two.
9. `tests/assets.rs`: `every_shipped_reach_weapon_authors_a_recharge` and
   `every_shipped_reach_weapon_is_enemy_facing`.
10. The existing `every_weapon_authors_a_range_and_nothing_else_does` must
    still pass untouched — the reach is on `ItemDef`, not on the stats.
11. gui: `the_tallest_gear_page_fits_its_popup`, re-run against a reach
    weapon.
12. `cargo test -p feral-processes-engine balance_sim` — the curves must
    **not** move. A curve that moves means the shipped weapons are not
    priced below the ladder.

## 8. Documentation owed

- `assets/items/README.md` — the `reach` field, its three refusals, and the
  note that the shape is derived unless authored.
- `CHANGELOG.md` — its own `## X.Y.Z` section at the merge.
- The three seam writes, in order: the argument to the memory graph as
  `seam:<slug>`, the trap to `.claude/skills/seams/references/items.md`, the
  one-sentence rule to `CLAUDE.md` under **Items, gear and economy**.
- `docs/measurements/` — the throughput gap between the shipped reach
  weapons and the single-target ladder, with the commands that produced it.

## Open, deliberately

- **No hostile ever swings wide.** Nothing inserts `Equipment` on a wild
  program, so the mechanic is player-side by omission rather than by a gate.
  The engine rules are written on the entity, so a mod that equips a hostile
  gets a coherent sweep for free.
- **A companion wearing one works.** `Game::equip` inserts `Equipment` on
  any wearer, so `swing_reach` answers for a companion exactly as it does
  for the player. Deliberate, and it is most of what makes the mid-zone
  craftable worth owning.
- **No range.** A reach that reached *further* is a different weapon class
  and a different aiming problem. Not designed here.
- **`copy_power` still rates a reach weapon by its band alone.** Accepted;
  see the decision above. If the swap picker turns out to mislead badly
  enough in play to matter, the fix is a measurement first.
