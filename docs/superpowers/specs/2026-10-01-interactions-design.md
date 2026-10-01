# Interactions and gossip (E1)

**Status:** design. Sub-project E1 of `2026-09-16-base-social-roadmap.md`
§E. The roadmap's shared decisions hold: nothing draws `GameRng`, every
derived value stays derived, an empty catalogue is a supported install, and
there is **no save change and no `SAVE_FORMAT_VERSION` bump**. Interactions
write ordinary memories; Sociability is derived on read.

**E2 is out of scope:** conversation templates, the per-program record ring
(the one save field E needs), the SOCIAL log section and the base-map speech
mark. So is G's sulker slant. E1 is complete without E2: its effect is read
on the memories page and the SOCIAL tab's relationship rows, which already
render any memory about a program.

## 1. Decisions and why

- **Mechanics first.** An interaction exists to move opinion. Text that says
  what was said is E2, and reads through to E1's records.
- **A timed pass, not an event.** `note_idling` (`game/base/offshift.rs`) is
  event-driven and amenity-bound; riding it would leave the rate uncapped and
  silence everyone not resting. A period also keeps `strikes` meaning
  "stretches", the argument on `MEMORY_POSTING_PERIOD`.
- **`INTERACTION_PERIOD` = `MEMORY_POSTING_PERIOD` (250).** The roadmap's
  rule: the period must not be shorter than posting, or hearsay overtakes the
  firsthand memories it copies and inverts the eviction order B rests on.
  Equal is allowed. A test pins `INTERACTION_PERIOD >= MEMORY_POSTING_PERIOD`.
- **Data, not code.** `assets/interactions/`, per CLAUDE.md's moddability
  rule, though the roadmap allowed a Rust table.
- **Any pair can slight; weights decide how often.** Rare between friends,
  common between rivals and from an `Abrasive` speaker. Opinion feeds back
  through the band, so the pass can start a feud or a friendship.
- **Hearsay is one hop.** Only firsthand memories carry `spreads_as`; a
  census forbids it on a hearsay def, so a grudge cannot echo around the base
  and amplify.

## 2. Data

### `InteractionDef` — `assets/interactions/<id>.ron`

| field | type | default | meaning |
| --- | --- | --- | --- |
| `id` | `String` | — | file stem by convention; duplicate → warn, first wins |
| `name` | `String` | — | for the README table and E2; not shown in E1 |
| `listener_memory` | `String` | — | `MemoryDef` id written on the listener, subject = speaker. Ignored when `gossip` |
| `speaker_memory` | `Option<String>` | `None` | written on the speaker, subject = listener |
| `weight` | `f32` | — | base pick weight; finite, `>= 0` |
| `by_disposition` | `BTreeMap<Disposition, f32>` | empty | multiplier on the **speaker's** disposition; absent key = 1.0 |
| `by_band` | `BTreeMap<Bond, f32>` | empty | multiplier on the band of the speaker's opinion of the listener; absent = 1.0 |
| `gossip` | `bool` | `false` | see §5 |

`Bond` gains `Serialize, Deserialize`; `Disposition` gains `PartialOrd, Ord`
(neither is saved by position, so neither derive changes a save).

### `InteractionDb` — `crates/engine/src/interactions.rs` (new)

`InteractionDb::load_dir` copies `ThoughtDb::load_dir`: a missing directory
is an empty db, files sorted, a malformed file or a non-finite / negative
weight or multiplier is skipped with a warning, duplicate id warns and the
first wins. A def whose `listener_memory`/`speaker_memory` does not resolve in
`MemoryDb` is skipped with a warning at load (the dangling-name rule from
`load_dir`, not a panic). Loaded beside `ThoughtDb` in `game/lifecycle.rs`.

### `MemoryDef::spreads_as: Option<String>`

`#[serde(default)]`. The hearsay def this memory becomes when told to someone
else. Valid only on a def whose `subject` is `Program`; the target must also
be `Program`-subject. Checked in the `MemoryDef` validity pass
(`memories.rs`); a bad one is dropped with a warning.

## 3. Sociability — `crates/engine/src/sociability.rs` (new)

`enum Sociability { Reserved, Sociable, Chatty }` with
`Sociability::of(ProgramId)`:
`derive::index(derive::fold(derive::fold(FNV_BASIS, &[SOCIABILITY_SALT]), &[id]), 3)`.
Not a Component, never saved, like `handles::of`. The salt keeps it
independent of `Disposition::seed` (which is unsalted). Each variant maps to a
chance multiplier in `tuning.rs` (`SOCIABILITY_RESERVED`/`_SOCIABLE`/
`_CHATTY`, starting 0.5 / 1.0 / 1.5). Its name shows as one line in the
SOCIAL tab's KNOWN FOR section (`SocialView` gains `sociability: &'static
str`), so the player can see why one program talks more.

## 4. The pass — `Game::note_interactions` (`game/memories.rs`)

Called from `game/turn.rs` after `note_unslotted`. Order with the other
`note_*` passes matters at the memory cap; this goes last so a pass's
firsthand memories are written before its hearsay competes with them.

1. Return unless `tick % INTERACTION_PERIOD == 0`, `base_is_established()`,
   and `InteractionDb` is non-empty.
2. Collect idle staff: base staff bodies with no `Task`, with position and
   `ProgramId`. Sort by `ProgramId`.
3. Pair greedily: each unpaired program in order takes the nearest unpaired
   program within Chebyshev `BOND_WITNESS_REACH` (ties → lower `ProgramId`).
   A program is in at most one interaction per pass. The first of the pair is
   the candidate speaker.
4. Roll: `seed = fold(FNV_BASIS, &[tick, speaker_id, listener_id])`;
   speak iff `unit(seed) < INTERACTION_CHANCE × sociability(speaker)`.
   `INTERACTION_CHANCE` starts at 0.5. No `GameRng`.
5. Pick: weight of each def = `weight × by_disposition[speaker] ×
   by_band[game.bond(speaker, listener)]`; a gossip def weighs 0 when no
   tellable memory exists (§5). Weighted pick from `fold(seed, 1)`. All
   weights 0 → nothing happens.
6. Collect `(who, def, subject)` writes into a `Vec`, then `remember` each,
   the `note_*` pattern.

Pure helpers carry the logic so tests need no `Game`: `pair_idle(&[(ProgramId,
IVec2)]) -> Vec<(ProgramId, ProgramId)>` and `pick(&InteractionDb, speaker:
Disposition, band: Bond, gossip_ok: bool, seed) -> Option<&InteractionDef>`.

## 5. Gossip

A def with `gossip: true` ignores `listener_memory`. It is eligible only when
the speaker holds a memory whose def has `spreads_as` and whose subject is a
`ProgramId` that is neither the listener nor the speaker — **never to the
subject about itself**, else a program is handed a grudge against itself.
The speaker tells its strongest such memory by felt intensity (ties → lower
subject id). The listener is given the `spreads_as` def about that subject.
`speaker_memory`, if set, is written as usual.

Gossip about a departed program is allowed; the stamped `subject_name` keeps
it readable, as memories already do.

## 6. Content (full set)

New memory defs, all `subject: Program`:

| memory | valence | written by |
| --- | --- | --- |
| `chatted_with` | +1.5 | small talk (both sides) |
| `talked_shop_with` | +2.0 | shop talk (both sides) |
| `laughed_with` | +2.5 | joke (both) |
| `complimented_by` | +3.0 | compliment (listener) |
| `thanked_by` | +2.0 | thanks (listener) |
| `commiserated_with` | +2.0 | commiserate (both) |
| `complained_at_by` | -1.5 | complain (listener) |
| `slighted_by` | -2.5 | slight (listener) |
| `insulted_by` | -4.0 | insult (listener) |
| `heard_well_of` | +1.5 | gossip, hearsay |
| `heard_ill_of` | -2.0 | gossip, hearsay |

Ten interactions: `small_talk`, `shop_talk`, `joke`, `compliment`, `thanks`,
`commiserate`, `complain`, `slight`, `insult`, `gossip`. Weights and
multipliers are authored in the plan, guided by: positive defs
favoured by `Friend`/`Close` and `Amiable`; `slight`/`insult` by
`Rival`/`Enemy` and `Abrasive`, with `insult` near zero above `Neutral`;
`commiserate`/`complain` lean `Languid`; `shop_talk` leans `Dogged`.

`spreads_as` on existing firsthand defs: `turned_on_me` → `heard_ill_of`,
`saw_turn_on` → `heard_ill_of`, `idled_with` → `heard_well_of`,
`bonded_in_battle` → `heard_well_of` — all four `Program`-subject. (`mauled_by`
is `Species`-subject, so it cannot spread.) New interaction memories carry no
`spreads_as`: what was said to me stays mine.

`assets/interactions/README.md`: deletable directory, RON example, field
table, the gossip rule. `assets/memories/README.md` gains `spreads_as`.

## 7. Tests (intent)

- **Pass:** nothing off-period or before `base_is_established`; a lone idle
  program does nothing; a busy (`Task`) program is never paired; one
  interaction per program per pass; same seed state → same writes.
- **Pairing (`pair_idle`):** nearest-in-reach, out-of-reach unpaired,
  deterministic tie-breaks, odd count leaves one out.
- **Pick:** band and disposition tilt the distribution over fixed seeds
  (slight beats compliment between rivals, and the reverse for close);
  gossip weight 0 with nothing tellable; all-zero → `None`.
- **Gossip:** never told to its subject; never about the speaker; the
  strongest tellable memory wins; hearsay is never retold (end-to-end over
  two passes).
- **Sociability:** stable per id; all three variants occur over 1000 ids;
  independent of `Disposition` (not identical partition).
- **Loader:** malformed, duplicate, non-finite weight, dangling memory id —
  each skipped with a warning, no panic; missing dir is empty.
- **Census (`tests/assets.rs`):** shipped interactions load with no
  warnings; every `spreads_as` resolves to a `Program`-subject def with no
  `spreads_as` of its own; `|hearsay valence| < |source valence|` for every
  source; `INTERACTION_PERIOD >= MEMORY_POSTING_PERIOD`; `MEMORY_TRIGGERS`
  and the every-def-is-written census learn the new memories.
- **SOCIAL tab:** the sociability line renders; the tallest-page height test
  still passes.
- `balance_sim` must not move: E1 has no combat path.

## 8. Known risk, not addressed

- **Rate is unmeasured.** 0.5 × sociability per idle pair per 250 ticks is a
  guess, as `FIGHT_CONDITION_WEIGHT` was. Tune after play, not before.
- **Memory pressure.** Twelve slots per program; a chatty base may evict
  older weak memories faster. Hearsay and small talk are weak, so they go
  first, which is the intended order — but it is unplayed.
- **No player-facing explanation of a moved opinion** until E2's log; E1
  shows only the memory names.

## 9. Deviations in implementation

- The fold order is `[id, SALT]` rather than §3's `[SALT]` then `[id]`: with
  the varying word last, adjacent ids give correlated outputs.
- The §4 seed gains a trailing `INTERACTION_SALT`, for the same reason and to
  keep it apart from other folds of the same words.
- The pick folds `[1, INTERACTION_SALT]`.
- The pass runs after `note_low_power`, still last.
- The gossip def carries a `listener_memory` that is ignored (the field is
  required by the schema; gossip's hearsay def comes from `spreads_as`).
