# Species classes: role as an axis independent of tier

Status: approved, unstarted. 2026-08-10.

## The problem

The 17 shipped species vary along **one axis wearing seventeen names**. HP, ATK,
DEF and `growth_multiplier` climb together — 36/3/1 (glitch) through 200/19/17
(wintermute), growth stepping 1.0 → 1.25 → 1.5 → 2.0 in lockstep. A player
meeting a rootkit after a crawler learns "bigger", and nothing else.

Two measurements taken while scoping this:

- **8 of 17 species grant no abilities at all**; 11 of 17 are neutral on every
  `Affinities` axis. The manifest's AFFINITIES box is hidden when the list is
  empty, so most of the roster shows no such box. Every species has exactly two
  moves.
- **The identity of a posted program affects nothing.** Across extraction,
  assembly, hauling and upkeep, the only creature-side input to the entire base
  economy is one read of `Stats::def` mitigating sweep damage
  (`game/upkeep.rs:295`). Every program is an interchangeable pair of hands.

There are buried role hints — sentinel is a real tank, proxy is glass, zero_day
a striker, sub_process a healer — but each is unsystematic and confounded with
tier, so the role cannot be read without already knowing the ladder.

## The design

**Five classes, emergent and unnamed.** No `role` field on `SpeciesDef`. A class
is a bundle of three things that must agree, and a census test enforces the
agreement by deriving the class from the affinities and checking the other two
against it:

| Class | Affinity axis | Stat shape | Kit |
|---|---|---|---|
| Striker | damage ↑ heal ↓ | ATK high, DEF/HP low | `Damage` |
| Bastion | buff ↑ damage ↓ | HP/DEF high, ATK low, slow | `Buff` |
| Medic | heal ↑ damage ↓ | HP mid, ATK low, fast | `Heal` + `Cleanse` |
| Saboteur | debuff ↑ heal ↓ | ATK mid, fast, DEF low | `Debuff` |
| Leech | drain ↑ buff ↓ | HP high, ATK mid, slow | `Drain` |

The taxonomy is read off the data rather than imposed on it: the six species
that already carry non-neutral affinities land on exactly these five axes
(scrapper and zero_day on damage, sentinel buff, sub_process heal, cipher
debuff, rootkit drain). The other eleven are unassigned, which is the work.

**Stat shape is expressed as proportions of the tier's budget, not as absolute
numbers.** That is the mechanism that makes role independent of tier: today a
tier-3 striker out-tanks a tier-1 tank on raw HP, so "tanky" is unreadable. "Low
DEF for its size" is readable at any tier, and it is already on the manifest.

### The grid

15 non-boss species × 3 non-boss tiers × 5 roles. Bold cells are forced — by
shipped affinities, or by `base_roster_growth_multiplier_rises_with_difficulty_
tier`, which pins `virus == 1.5` **by name**. That pin makes virus the tier-3
Medic and forces **construct down to tier 1** as the Bastion, which happens to
preserve both `sprite > construct` on speed and the melee-only census.

| tier | Striker | Bastion | Medic | Saboteur | Leech |
|---|---|---|---|---|---|
| 1.0 | glitch | **construct** ↓ | **sub_process** | sprite | drone |
| 1.25 | **scrapper** | crawler | proxy | trojan | worm |
| 1.5 | **zero_day** | **sentinel** | **virus** | **cipher** | **rootkit** |

Non-bold cells are free choices, assigned by flavour with stats to follow.
Adding to the tier-1 opening ring is safe; removing from it is not
(`the_shipped_roster_has_species_on_both_sides_of_the_opening_ring`, whose
fallback empties the ring while leaving it looking intact). `can_nest` is pinned
to crawler/scrapper/trojan/worm — no conflict.

### Two work dials, both species data

`SpeciesDef` gains `base_int`; `base_speed` gains a second meaning as worker
rate. Neither goes in `Stats`, and that is a decision rather than a shortcut:

- `Stats` has no `Default` and no struct-update syntax anywhere, so a new field
  breaks **99 construction sites** and forces `SAVE_FORMAT_VERSION` 25→26,
  invalidating every `.bin` under `saves/` including the dev saves.
- More importantly it would **grow on level-up**, so a level-20 Striker would
  out-think a level-1 Medic — reintroducing the exact tier-confound this work
  exists to remove. A class is legible only if its numbers are fixed to the
  species.

`base_int` becomes a third term in `systems::mining_success_chance`, exactly the
way `KEEN_SCAVENGER_BONUS_PER_LEVEL` was added, and governs extraction
**success** — the fizzle at `resolve_gather_cycle`. `base_speed` governs worker
**rate**; its sole reader today is `roll_initiative`, so the wiring is nearly
free. The cost is that initiative and work rate can no longer be tuned apart,
which is accepted deliberately: "the sprite is quick" meaning quick in both
halves is the coherence this design is for.

### Class base jobs — three of five, on purpose

Striker and Saboteur get no base job. Posting one is a waste, and with
`BASE_PET_CAPACITY = 3` that asymmetry is the point: every program at a machine
is one absent from the party, so roster composition becomes a real cost rather
than a formality.

- **Bastion** → `TaskKind::Guard` becomes meaningful. Today the entire payoff of
  that task kind is one `worker.def` read.
- **Medic** → repairs structure `Durability`. No creature can repair today; the
  Patch Node is the only source in the game.
- **Leech** → extraction **yield**, deliberately distinct from `base_int`'s
  effect on extraction **success**, since the two are trivially conflatable.

Medic repair **reuses `TaskKind::Guard`** rather than adding a variant. A new
kind costs a menu mode, a `group_menu` row with its `surface_only` flag, an
`Assignee` widening, two detachment strings and a save round-trip — for a
distinction the player does not need. Reusing Guard makes the existing menu mean
"post a program here", with *what it does* decided by class, which is the
emergence this design is after. `displace_task_holder` already gives
one-post-per-structure for free.

## What this costs that isn't obvious

**The Leech class has no ability it is legally allowed to have.** All three Drain
abilities are in the hunt-only wild pool, and `no_species_or_research_file_
grants_a_wild_only_ability` forbids a species file from naming any of the 28
wild-only files. Grantable today: Striker 2, Medic 2, Bastion 3 (one being the
mandatory fallback), Saboteur 3, **Leech 0** — nine, against fifteen species
needing two slots each. So the kit work is **authoring ~15-20 new ability files**,
including an entire new non-wild Drain family, not wiring up existing content.

**`support.rs::generic_species` is defined as "first species by id with no
declared abilities"**, which today resolves to construct, and `spawn_tamed` is
built on it across **233 call sites**. Completing the grid panics the suite at
startup. It has to move to a hand-written def under `modded_assets_dir` before
any species gains a kit.

**The first kit entry sits at level 2, not 1.** `FALLBACK_ABILITY_ID`
(`priority_boost`) exists to fill an empty kit and goes dead the moment every
species grants something at level 1. Holding the first entry back keeps it
reachable and obtainable by extraction, and makes a fresh capture read as
generic before it reads as a class.

**The manifest SPECIES box is already at exactly `MAX_SECTION_ROWS = 6`.** A 7th
row silently truncates to "+N more" — nothing fails, the data just vanishes. The
answer is a new WORK box with the existing Speed row moved into it, which drops
SPECIES to 5 and leaves headroom for the base-job row. The fixture edit lands in
the same commit as the renderer edit; `manifest.rs:381-387` records the exact
regression where it didn't.

**`balance_sim`'s median species changes identity.** `median_ordinary_species`
sorts by stat sum and takes index 7 of 15 — today scrapper (112), trojan (104)
after construct drops. So the party baseline for every progression sweep changes
species. Worse, the median can land on a Bastion or Medic, and the sim models no
healing and no buffs, so a Medic-shaped median is *strictly worse there* for a
reason that isn't real. Record the moved curve; do not retune the gate to sit
still.

**Phase 2 inverts a documented seam.** CLAUDE.md states an assembler's rate comes
from its def's `ticks_per_unit`, not from `Task::required`. Baking worker speed
into `required` at assignment and having `assembler_system` read it inverts that
— deliberately, in the same commit as the amendment, on the evidence that
`upgrade_structure` never touches `ticks_per_unit` so nothing changes a machine's
rate after assignment. The known cost: a cronjob in an existing save keeps its
old rate until reassigned.

## Sequence

Eight releases, ordered so the first already changes how the game plays.

1. `base_int` + the manifest WORK box.
2. `base_speed` as worker rate, baked into `Task::required` at assignment.
3. Stat shapes, construct's tier move, affinities normalised. **The `balance_sim` phase.**
4a. Author the missing ability content — referenced by nothing yet, zero Rust.
4b. Attach the kits (after `generic_species` moves).
5a/5b/5c. Leech yield, Bastion guard, Medic repair.

## What this is blind to

`balance_sim` gates the stat shapes for HP/ATK/DEF and nothing else — it models
no abilities and no initiative, so a Bastion's real survivability and a Striker's
real burst are invisible to it. Those go to the arena, and a new
`dev-arenas/class-mirror.ron` staging one of each class against a common pack is
the instrument this design actually wants.

Kits only fire through the Special menu, so `FERAL_DEV_ARENA=1` and the `[R]`
arena is the only way to see one in an authored fight. And there is no headless
equivalent for whether the base *feels* like it has roles, which is the whole
point of phase 5 — that needs `--template chains` and `--template extraction`
played by hand.

## Corrections found while scoping

Two stale doc claims to fix in passing, in the phases that touch their files:

- `game/combat_rewards.rs:56-62` claims `work_resource`'s only non-loot reader is
  the inspection view. It misses `grant_nest_cache` (`game/zone.rs:87`), added
  later.
- `assets/species/README.md:81` describes a cronjob-assignability gate on
  `work_resource` that `Game::accepts_a_program` does not implement.
