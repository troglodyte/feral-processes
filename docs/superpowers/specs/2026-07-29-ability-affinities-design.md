# Ability affinities

## Problem

Two members of one species are mechanically identical in the one dimension
that matters most in a fight: what their routines are *good at*. A Cipher and
a Scrapper both run `hot_patch` for exactly the same number of points at the
same level. `SpeciesDef::growth_multiplier` is the only per-species
progression knob, and it is a tier number — 1.0 for Easy, 1.25 Medium, 1.5
Hard, 2.0 boss — that scales all three stats uniformly. Nothing anywhere says
"this program is a healer".

The player has the same flatness from the other direction. The seven perks
buy foraging, hunger, decompiling, crafting, ATK, DEF and max Integrity.
None of them touch the abilities the player actually spends rounds on.

**Affinities** are a per-category multiplier on ability magnitudes: a healing
affinity of 1.1 makes every heal that program runs 10% stronger. Species
carry them as data; the player buys them as perks.

## What exists

Verified against source, not remembered.

- `Game::use_ability` (`crates/engine/src/game/combat_round.rs:631`) is the
  single place all seven `AbilityEffect` variants resolve, for **both** sides
  of a fight — it took over the hostile path too, so a species trait applied
  here reaches wild carriers with no extra work.
- Every authored magnitude passes through one function:
  `abilities::scaled_power(power, level)` = `power × ability_power_scale(level)`,
  which is `1.0 + level × ABILITY_POWER_SCALE_PER_LEVEL` (0.15), clamped at
  `ABILITY_POWER_SCALE_LEVEL_CAP` (40) for a 7x ceiling. Three call sites in
  `combat_round.rs` (Buff, Heal, Debuff) plus tests.
- `AbilityEffect` has seven variants. Five carry a magnitude (`Damage`,
  `Heal`, `Buff`, `Debuff`, `Drain`); `Cleanse` and `Decompile` carry none.
- `Drain::heal_fraction` is *deliberately* outside `scaled_power`
  (`abilities.rs:169`) — the heal rides the damage, which already rides ATK.
- Species live on a `Creature { species: SpeciesId }` component. The player
  is a bare `Player` marker with **no species** — `progression` passes
  `BASELINE_GROWTH_MULTIPLIER` for exactly that reason.
- `ability_user_level` (`game/combat.rs:623`) reads `Experience`, falling
  back to `ZoneLevel` for a wild program, which has no `Experience`.
- `PerkDef.id` is the `Perk` enum variant, parsed by RON as a variant name
  (`id: Attacker`). `PerkDb::load_dir`'s doc (`perks.rs:111`) is explicit
  that a file naming a variant this build lacks is rejected as an unknown
  variant. **A perk file cannot bring a perk into existence.**
- `Potential` (`components.rs:465`) is an existing per-individual roll —
  hp/atk/def/growth, range `MIN_INDIVIDUAL_ROLL`..`MAX_INDIVIDUAL_ROLL`
  (0.8–1.2), persisted, averaged on fusion, displayed via `quality_label`.
- `balance_sim` **models no abilities at all** — stated in its own doc
  comments at lines 263 and 699.

## Design

### Categories

An `AffinityKind` per magnitude-carrying effect: `Damage`, `Heal`, `Buff`,
`Debuff`, `Drain`. Lives in `abilities.rs` beside `AbilityEffect`, with
`AbilityEffect::affinity_kind() -> Option<AffinityKind>` as the one mapping.
`Cleanse` returns `None` (no number to scale). `Decompile` returns `None`
deliberately: the `Decompiler` stat and `Perk::ExploitFocus` already occupy
that axis, and a third multiplier there is a fourth spelling of the same
thing.

A sixth category is not addable by a modder — but it is not addable by us
either without a new `AbilityEffect` variant, which is Rust on both paths.

### Species side — fully data

`SpeciesDef` gains one `#[serde(default)]` field, so every existing species
file and every third-party mod keeps parsing untouched:

```ron
affinities: (heal: 1.4, damage: 0.85),
```

A struct of five `f32`s, not a map — the categories are a closed set.
**Both** levels of default are required and it is easy to supply only one:
`#[serde(default)]` on the `affinities` field itself, so a file omitting it
entirely still parses, *and* a per-field default of 1.0, so the partial form
above works without naming all five.

Two load-time guards:

- **Finite check.** `SpeciesDef` has **no** `non_finite_field` today — that
  mechanism exists on `AbilityDef` and `ItemDef` only, so this adds it rather
  than following it. RON accepts bare `NaN`/`inf`, and `f32::clamp` returns
  NaN for a NaN input, so a clamp alone does not contain it: the check is
  required, and the file is refused at load with a warning.

  Scoped to `affinities`. `taming_difficulty` and `growth_multiplier` are
  also unchecked floats on the same struct, and closing that hole is a
  one-line extension — but it could newly reject a species file that loads
  today, which is a behaviour change beyond this feature. Noted, not done.
- **Clamp** to `AFFINITY_MIN`..`AFFINITY_MAX` at load, once, not on every
  read. `SpeciesDb::load_dir` already binds `Ok(mut def)` and mutates it (it
  retains known abilities), so the clamp goes there.

### Player side — perk

Five new `Perk` variants, **appended** to the enum. `Perk`'s variant order is
save format (bincode encodes enums positionally and
`PlayerSave::unlocked_perks` holds indices), so appending is mandatory and
`SAVE_FORMAT_VERSION` does not move.

Five new `assets/perks/*.ron` files carry name, description and cost, exactly
as the seven existing perks do. Cost 2, matching `Attacker`.

The magnitude is **one shared const**, `AFFINITY_PERK_BONUS_PER_LEVEL`, in
`tuning.rs` at 0.03 — the same figure as `EXPLOIT_FOCUS_HP_PENALTY_REDUCTION_PER_LEVEL`,
the closest existing analogue. Five identical consts would be five things to
keep in sync. A perk's affinity is `1.0 + 0.03 × level`.

`Perk::affinity_kind() -> Option<AffinityKind>` gives one generic hook in the
read path rather than five bespoke ones. This is the shared shape being
honoured in code, where it genuinely is shared — `PerkDef` still has no
`effect` field, because the magnitude is a difficulty knob and those live in
`tuning.rs`.

### Resolution

One function, `Game::ability_affinity(actor, effect) -> f32`, mirroring how
`ability_user_level` already resolves off the actor:

1. `effect.affinity_kind()` is `None` → `1.0`, and no lookup happens.
2. Actor has `Perks` (the player) → `1.0 + AFFINITY_PERK_BONUS_PER_LEVEL ×
   player_perk_level(perk_for(kind))`.
3. Actor has `Creature` → that species' affinity for the kind.
4. Neither → `1.0`.

The player has no species and a companion has no `Perks`, so **the two
sources can never stack.** There is no stacking rule to design and no
double-dip to price. This is a consequence of the two scoping decisions, and
it is why the perk is scoped to the player's own casts: a party-wide perk
would multiply against the companion's own affinity and reintroduce exactly
that problem.

`use_ability` resolves affinity **once per cast**, next to `level`, for the
reason already commented there: re-reading it inside the recipient loop
invites keying it off the recipient instead of the caster.

`scaled_power` takes affinity as a third argument rather than the caller
multiplying afterwards — `(power × scale).round() × affinity).round()` rounds
twice and loses points that one combined multiply keeps.

### What this buys, beyond a flat multiplier

An affinity applies to whatever is *installed* in a program's routine slots,
not to what its species natively grants — and a routine can be popped out and
installed on a different species entirely (`m` in the routine panel). So a
species with a strong heal affinity and no innate heal is not a contradiction:
it is a reason to spend a researched or extracted heal routine on *that*
program rather than another. That interaction with the existing routine
system is the actual mechanic; the multiplier is just how it is spelled.

### Damage

Damage affinity scales the **authored power only**, before it enters
`battle::compute_damage(effective_atk, def, power)`.

This is consistent with the other four categories, which have no ATK term,
and keeps one code path. The cost is that it is a *small* lever: damage is
`power + ATK − DEF`, so at a high level the authored power is a minority of
the total and a 1.1 damage affinity may be worth a point or two. The
compensation is authoring range — `AFFINITY_MAX` is set wide enough (2.0)
that a species meant to be a damage specialist can be written as one, and
balance lives in the `.ron` rather than in the formula.

Drain's damage power scales; its `heal_fraction` does not, or drain
double-dips through the same multiplier twice.

### Clamp ceiling, stated plainly

Affinity multiplies the *already level-scaled* power, so the ceilings
compound:

- **Companion**: capped at level 12 → 2.8x from level, × 2.0 affinity = 5.6x
  authored power. Reachable, and it is the modder's choice — that is the
  moddability contract.
- **Player**: uncapped level → 7x from level, but affinity comes only from
  perks at 0.03/level, so 2.0 needs ~33 perk levels ≈ 66 Perk Points at 1 per
  level-up. Distant, not unreachable.

`AFFINITY_MIN` 0.5 / `AFFINITY_MAX` 2.0 is a wider band than the existing
`MIN_INDIVIDUAL_ROLL`..`MAX_INDIVIDUAL_ROLL` (0.8–1.2), deliberately, per the
damage decision above.

### UI

Species affinities go on the **manifest** screen, whose `ProgramManifest`
view (`views.rs:500`) already carries the species-level numbers —
`growth_multiplier`, `base_speed`, `taming_difficulty` — alongside
`ManifestPotential`, the existing per-individual block. Non-neutral
affinities only; a species with all five at 1.0 shows nothing rather than
five 1.00s.

The five perks need no renderer work: the picker is `PerkDb::catalogue()`
driven off `Perk::all()`, so they appear from their files.

Without the manifest change the feature is imperceptible, which is why it is
in scope rather than a follow-up.

### Save format

**No bump.** Species affinities are data, regenerated from the `.ron` on
every load. Appending `Perk` variants leaves every existing index valid.

## Rejected

- **Per-individual affinity rolls on `Potential`.** Genuinely cheap — the
  roll, persistence, fusion averaging and display machinery all exist — and
  it is the stronger character-variety mechanic. Rejected for this pass
  because it is a save-format change, it would distort `quality_percent`
  (which averages exactly four rolls), and it needs a fusion combination
  rule. Fixed-per-species is the smaller thing that delivers the feature, and
  it does not block adding rolls on top later.
- **Affinity effect in the perk `.ron`.** Would make `per_level` data, which
  contradicts the tuning rule — a per-level perk magnitude is a difficulty
  knob, same category as `ATTACKER_BONUS_PER_LEVEL`. Moving only `kind` into
  the file buys nothing while `id` still parses as an enum variant.
- **`Perk` as a string id** so affinity perks are pure data. `unlocked_perks`
  holds bincode enum indices, so this is a save-format refactor, and the
  categories are fixed at five by the effect taxonomy — there would be
  nothing left for a modder to add.
- **Scaling final damage rather than authored power.** Bigger felt effect,
  but a second code path for one of five categories, and it multiplies the
  ATK contribution the affinity has no claim on.
- **A `Decompile` affinity.** Third multiplier on an axis that already has
  two.

## Testing

TDD, failing test first, per category of risk:

1. `scaled_power` at affinity 1.0 reproduces the current two-argument
   result — the regression guard on the signature change.
2. A heal cast by a species with `heal: 1.5` restores 1.5x, rounded once.
3. A species file with no `affinities` field parses and resolves 1.0 for all
   five — the `#[serde(default)]` contract that keeps mods working.
4. A non-finite affinity disqualifies that species file with a warning while
   the rest of the directory still loads. Assert on NaN specifically, not
   just infinity — NaN is the case a clamp would silently pass through.
5. An out-of-range affinity is clamped at load, and a read returns the
   clamped value.
6. A player affinity perk scales the player's own ability **and does not
   scale a companion's ability the player commanded** — the scoping decision,
   asserted directly.
7. A wild carrier of a species with a damage affinity has it applied when it
   retaliates, via the `wild_retaliate` path into `use_ability`.
8. `Drain`'s `heal_fraction` is unaffected; only its damage power scales.
9. `Cleanse` and `Decompile` resolve to no category and take no multiplier.
10. A fusion inherits the higher-level parent's species affinities. This
    falls out of `fuse_companions` already taking that parent's species — the
    test pins the behaviour rather than adding code.
11. `Perk::all()` still yields the original seven in their original order,
    guarding the save-format ordering.

Fixtures come from `crates/engine/src/tests/support.rs` — `spawn_tamed`,
`spawn_wild_on_player_tile`, `insert_battle`, `resolve_round_with`,
`test_assets_dir` — before writing any new one.

## Gates

```sh
cargo test -p feral-processes-engine balance_sim   # expect NO curve movement
cargo test --workspace
cargo clippy --workspace
cargo fmt
```

`balance_sim` not moving is the *expected* result, not a pass: it models no
abilities, so it cannot see this feature. **Affinities ship with no automated
balance coverage.** Every magnitude here — 0.03 per perk level, the 0.5–2.0
clamp, and whatever the shipped species files claim — is
arithmetic-plausible and unplayed, the same state the ability set and the
dungeon tuning are in. Say so rather than implying the green suite is
evidence.

## Files

Engine: `abilities.rs` (`AffinityKind`, `affinity_kind`, `scaled_power`),
`species.rs` (field, finite check, clamp), `perks.rs` (five appended
variants, `affinity_kind`), `game/combat.rs` (`ability_affinity`),
`game/combat_round.rs` (thread it through `use_ability`), `tuning.rs` (three
consts), `views.rs` (manifest field).

GUI: `render/manifest.rs`.

Assets: five `assets/perks/*.ron`; affinities on the shipped species.

For the species pass, the rule rather than a table: give a non-neutral
affinity to species with a clear role in the existing roster tiering, at most
one strength and at most one weakness each, and leave the rest neutral. A
roster where every species has five non-1.0 numbers is a roster where
affinity means nothing. Exact values are a plan-time decision made against
the files, not guessed here.

Docs: `assets/species/README.md` and `assets/perks/README.md` in the same
change, per the schema rule; plus root `README.md` and `CHANGELOG.md`.
