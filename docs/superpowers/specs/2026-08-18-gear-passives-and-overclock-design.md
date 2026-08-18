# Gear passives, and the Overclock axis they will carry

**Status: APPROVED 2026-08-18. Feature B (gear passives) is the change to
implement. Feature C (Overclock) is decided but deliberately unbuilt — it is
recorded here as an appendix so B does not foreclose it.**

Read `INDEX.md`'s warning about `**Status:**` headers before trusting any
other spec's; this one is accurate as of the date above and will rot the
same way.

Supersedes `2026-08-17-item-synergy-burnout-parked.md`, which established
that there was nothing to stack and that **burn-out is not a governor you
add before the content, it is what you trade for opening one specific
closure**. That file's six-closure survey is still the reference for *why*
the engine is anti-synergy by construction; this file picks the closures and
designs against them.

## What was chosen, and what that inverts

Two of the parked spec's three shapes, sequenced:

- **B — gear grants a worn passive.** Opens a new axis rather than a
  closure: gear gets side effects for the first time, through the effect
  vocabulary that already exists as data.
- **C — a compounding multiplier with burn-out.** Opens closure 1 (stat
  stacking is linear), which is the only place in this engine where anything
  can genuinely compound.

B lands alone and is played before C is built. C's shape is settled here so
B's one new data field and one new trigger are the doors C will come
through, rather than something C has to widen later.

**Backlash is denominated in HP**, and the reason is structural rather than
a preference: `tuning::MIN_DAMAGE` is 1 and `Game::mitigate_incoming_damage`
floors its result at 1, so no stack of anything can make a combatant
untouchable. An HP-denominated burn cannot be dodged by stacking harder. The
damage floor is the governor's backstop and it is already in the engine.

## Two findings that reshaped the parked reading

**1. "Multiplicative" is only dangerous in one direction.** Mitigation today
*sums*: `long_winter` 25 + `patch_routine` 10 is 35% off incoming damage.
Composed multiplicatively that is `1 - (0.75 * 0.90)` = 32.5% — *less*.
Multiplicative composition on the defensive axis is sub-additive, a
diminishing-returns rule that can never reach 100%, not a compounding one.
So the compounding risk lives entirely on the **outgoing** side. A defensive
multiplier is the failure the standing linear-curve invariant exists to
prevent; an offensive one only shortens fights.

**Do not "add synergy" by making `Mitigation` multiplicative.** It is a nerf
wearing the feature's clothes.

**2. A gear passive can never arm a field buff, and that is a feature.**
`AbilityEffect::FieldBuff` *is* the field-only marker, and
`AbilityDef::passive_field_mismatch` already refuses a `triggers` on a
field-only effect. So B's vocabulary is `Damage`, `Heal`, `Buff`, `Debuff`, `Drain` and
`Cleanse` — meaning **B touches none of the six closures**. Every one of them
stays shut until C deliberately opens one.

## B — the seam

**The data.** `ItemDef::grants: Option<AbilityId>` on
`crates/engine/src/items_db.rs`, `#[serde(default)]` like every other
optional field, so existing mods keep parsing. It names an ability that
already exists in `assets/abilities/` — no new effect vocabulary, and every
`AbilityEffect` variant and `FieldBuffKind` a modder already has works here
for free. `assets/items/README.md` documents it in the same change.

`ItemDef` is asset data, not save data. **B changes no save format.**

**The firing.** `Game::ready_passives` (`game/passives.rs:90`) reads exactly
one source today: the holder's `Routines`. It gains a second — the wearer's
`Equipment`, three slots, each resolved through `ItemDef::grants` and
`AbilityDb::get`. Installed routines first so today's slot order is
untouched, gear after.

That is the whole hook. `fire_passives` above it does not change, cooldowns
arm exactly as they do now, and `use_ability` sees a holder like any other.

**Derived, never written into `Routines`.** Unequipping ends the passive by
omission — nothing has to remember to clear it, which is the same argument
`Game::wielded_stat_bonus` makes by being computed live. Writing the id into
`Routines` instead would fight the slot bound (a level-1 program's single
slot is already occupied), would need an unequip path subtracting exactly
what the equip added, and would put an ability id in the save that came from
an item file a mod is free to delete.

**Stacking rule: an ability fires once per source per round; the cooldown is
per id.**

- A gear grant *and* an installed routine of the same id both fire — twice
  in that round. This is deliberate and is the feature's cross-source stack.
- Duplicate *routines* are already impossible: `install_disk`
  (`game/routines.rs:290`) refuses with "X already runs Y". There is no
  guard to write.
- Two gear slots naming the same ability fire **once**, by symmetry with
  that refusal. Stacking is a reward for combining different things, not for
  wearing two of the same.
- `AbilityCooldowns` is keyed by `(holder, AbilityId)`, so after a
  double-fire the id goes on cooldown once and both sources return together.

**Load-time refusal.** `grants` must name an ability that exists, is
`is_passive()`, and is not field-only. Each failure skips the file with a
logged warning, never a panic — the posture `SpeciesDb::load_dir` and
friends already take. This is the same reasoning as
`passive_field_mismatch` itself: an authored thing that would silently never
fire is refused at load rather than shipped.

**Who gets it.** Gear is wearable by the player *or any owned program*
(`Game::check_wearer`), and `fire_passives` already walks holders in party
order — so companion gear passives arrive at no extra cost, and are the case
most worth seeing on screen.

**One new trigger: `RoundStart`.** `PassiveTrigger` has two variants and
both are reactive to misfortune; gear that acts only when an ally dies is
thin. `RoundStart` fires once at the top of `battle_resolve_round` for every
living party member. Its call site needs no new per-round state, unlike a
"was struck" trigger, which would want a `landed_this_round`-style flag.
`cooldown` is what throttles it, and `cooldown: 0` firing every round is the
intended shape. It is also the trigger C ramps on, so B ships it regardless
of whether B's own content needs it.

Adding a `PassiveTrigger` variant means adding its call site in the same
change — `game/passives.rs`'s module doc states this as a rule, and it is
what stops an authored routine that silently never runs.

## B — the content

**Three new items, one per slot. Not retrofits.** Adding `grants` to an
existing item silently changes what a copy already sitting in a player's
save is worth, against a `value` priced for a stat line that no longer
describes it. A new item is priced correctly from the start, and is exactly
the path a modder takes — which is the path worth exercising.

- **Weapon** — a `RoundStart` `Damage` at low power, `cooldown: 2`. Chip
  damage that reads as the weapon acting on its own.
- **Armor** — a `RoundStart` `Buff { kind: Def }` on the wearer,
  `cooldown: 3`. A brace that comes up by itself.
- **Module** — grants **`watchdog`**, the `Afflicted` `Cleanse` passive that
  already ships. No new ability file, and it proves the reuse path: naming
  an ability that already exists is the common modding case.

Two new ability files, three new item files, zero new engine vocabulary.

**Pricing.** Each needs a `value` above what its stat line alone justifies,
and an item's price is bounded twice — the craftable-versus-ingredients
ceiling and the `work.produces` rate bound — both asserted over the real
assets. Ship all three drop-only (`droppable` / `cache_drop`) and leave
crafting to a later pass, so no recipe has to clear the first bound in this
change.

## B — visibility

A player must be able to see what an item grants, or the feature is a
mystery. The item's authored `description` is not enough: it is free text a
mod controls and cannot be trusted to stay in step with `grants`.

The engine exposes the granted ability's **name and description** on the
existing item-inspection view, and the renderer draws one row. Deriving it
in `views` rather than in the renderer is the standing rule for read-only
screens — a per-row transform folded into gui opens a screen on a row that
is not drawn.

This is what makes B a two-crate change and is the reason it earns a spec
under the process-weight rule rather than being done inline.

## B — tests

TDD, failing first, in `crates/engine/src/tests/`.

1. **Fires while worn, does not while stripped.** One test, both halves.
   The unequipped half is what stops it passing with the hook deleted.
2. **Cross-source double-fire.** An item grant plus an installed routine of
   the same id fire twice in one round, and the id is on cooldown once
   afterwards.
3. **Two gear slots granting the same ability fire once.**
4. **Load refusal, three ways.** `grants` naming a nonexistent id, a
   non-passive ability, and a field-only one — each skips the file with a
   warning and no panic.
5. **A companion's gear passive fires**, in party order.
6. **Cooldown is shared plumbing.** A gear passive on cooldown does not
   re-fire the next round.
7. **Save then load, not merely a RON round trip.** The passive is still
   live after a reload with nothing new persisted, which is what proves it
   is derived. A round trip alone cannot catch this class of thing.
8. **The granted ability is named on the inspection view**, engine-side.

**Gates.** `cargo test --workspace`, `cargo clippy --workspace`,
`cargo fmt`. `balance_sim` will pass and **that is not evidence** — it
models no abilities, so B is entirely ungated there. The instruments are a
`dev-arenas/` scenario with the gear equipped (`equip` is top-level in a
scenario, never inside `Fresh(...)`) and a session under
`FERAL_DEV_ARENA=1`, which is the only way a companion's passive is seen
firing in an authored fight.

## Appendix — C, decided and unbuilt

Not part of this change. Recorded so the decisions are not re-derived.

**Where the multiplier lands.** `Game::effective_atk`
(`game/combat_round.rs:1067`) is the single funnel for attack and already
carries a multiplicative term — `battle::power_attack_multiplier`, the
low-Power penalty. Overclock is that term's mirror image at the same site,
so C adds no new formula and leaves `battle::compute_damage` untouched.
Structural note: `effective_atk` early-returns for non-players *before* the
multiplier, so Overclock must sit above that return — companion gear is a
first-class case.

**ATK, not final damage.** Multiplying the result steps over DEF entirely,
which is the runaway. Multiplying the ATK term keeps the subtractive
structure — a high-DEF enemy still resists — while stack count still
compounds, since 1.5x twice is 2.25x. The ceiling is ATK's share of the
damage.

**Its own component, not a `BuffKind`.** `CombatBuff` holds at most one buff
and a fresh one overwrites, so an Overclock authored as a `BuffKind` would
clobber a Rally, or be clobbered by bracing, and could never ramp. C adds a
battle-scoped `Overclock { stacks }`, cleared at `end_battle` with
everything else and never persisted. **So C changes no save format either**,
which retires the parked spec's open question 1: the counter resets per
battle, and per-battle is free.

**Burn-out.** Each round Overclock runs, the holder takes damage growing
faster than the benefit — benefit linear in ATK's share, cost superlinear in
stacks — so there is a stack count past which pushing is strictly losing.
Routed through `Game::apply_damage`, the only path that lowers HP, so every
check that must see all damage sees this too.

**What C must not do:**

- Do not make `Mitigation` multiplicative expecting compounding — it is
  sub-additive, so that is a nerf, not a synergy.
- Do not denominate backlash in Trace. `Game::raise_trace` returns silently
  unless underground, so it would be a mechanic that evaporates on open
  grid.
- Do not expect `balance_sim` to gate any of it. It models no abilities at
  all. C ships on `dev-arenas/` and a played session, or it ships unmeasured.

**What B leaves open for C**, the only coupling: `RoundStart` is the trigger
Overclock ramps on, and `ItemDef::grants` is the door Overclock gear comes
through. Nothing else in B constrains C.
