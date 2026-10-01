# Sulking behaviours (G)

**Status:** design. Sub-project G of `2026-09-16-base-social-roadmap.md`,
building on C (`2026-09-30-bonds-and-social-tab-design`), E1/E2
(`2026-10-01-interactions-design`, `2026-10-01-conversations-design`) and F
(`2026-09-30-situational-thoughts-design`). The roadmap's shared decisions
hold: nothing draws `GameRng`, every derived value stays derived, and an
empty catalogue is a supported install.

**No save change.** Every new value is derived or a `#[serde(default)]`
asset field; `SAVE_FORMAT_VERSION` is untouched, so the release is a patch.

**Out of scope:** tuning (every constant below is a guess awaiting play), a
work-speed effect of morale, and any GUI change.

## 1. Purpose and where this departs from the roadmap

G closes the roadmap's loop — *a sulk → sabotage, seen by a witness → gossip
again* — by giving the `Sulking` rung behaviour the player can see in
conversations, on the map and in the stores.

Two facts about the shipped code reshape the roadmap's §G:

- **A sulker is rarely at a post.** `Game::on_respite`
  (`game/base/morale.rs`) takes any non-stranded `Disgruntled` body off the
  line whenever the base has an amenity. A sulker stays posted only with no
  amenity or a stranded errand. Sabotage "at a post", as the roadmap words
  it, would be green in tests and almost never seen. **Sabotage is done by an
  idle sulker beside a machine** instead (decided 2026-10-01).
- **"Slower beside a rival" already shipped** as F's `BesideRival` thought
  (`assets/thoughts/beside_rival.ron`). Morale moves gather reliability
  (`systems::mining_success_chance` via `morale_shift`), not work speed. G
  adds nothing for it.

## 2. One predicate

`Game::sulks(e) -> bool`: the body carries `Disgruntled` with
`grievance >= Grievance::Sulking`. Every new term in this design reads it,
so a program that has downed tools or is lashing out still talks
spitefully and still spoils output.

`refuses_post` keeps its own `== Sulking` test: a body that has downed
tools refuses every post already, and that ladder is not this design's to
change.

## 3. The slanted voice

**Weights.** `InteractionDef` gains

```rust
#[serde(default = "one")] pub sulking: f32,
```

and `InteractionDef::weight_for` gains a `sulks: bool` argument,
multiplying `sulking` in when it is true. `schema_fault` rejects a value
that is not finite or is negative, as it does for `by_band` and
`by_disposition`. `note_interactions` passes `self.sulks(speaker)`.

Shipped values (guesses):

| def | `sulking` |
| --- | --- |
| `slight`, `insult` | 3.0 |
| `complain` | 2.0 |
| `gossip` | 1.5 |
| `commiserate` | 1.0 (omitted) |
| `small_talk`, `shop_talk` | 0.5 |
| `joke` | 0.3 |
| `compliment`, `thanks` | 0.2 |

`assets/interactions/README.md` documents the field in the same change.

**Negative gossip.** `Game::tellable` gains a `sulker: bool` argument.
When set, a memory is a candidate only if its def's `valence < 0.0`; the
existing strongest-by-absolute-felt-intensity ordering and tie-break are
unchanged within that set. A sulker holding no negative tellable memory has
nothing to tell, and `gossip_ok` is false for that pick, exactly as for a
program holding nothing tellable today.

## 4. Freezing out

**No warmth between rivals.** `note_idling` (`game/base/offshift.rs`) skips
any other body for which `self.bond(worker, id).avoids()`. This applies to
every program, not only sulkers (decided 2026-10-01): two programs at Rival
or Enemy no longer gain `idled_with` from sharing an amenity, so a grudge
does not heal by accident. The comparison is the bond's own signed band, so
a fondness never triggers it.

**A sulker refuses a post beside a rival.** `refuses_post` gains a second
reason, after its existing machine-resentment test and under the same
`== Sulking` gate: some other staff body whose `Position` is within
Chebyshev 1 of the post's `Position`, and which the worker `.avoids()`.
"Beside" is the distance `situations::assess` uses for `BesideRival`, so
the thought and the refusal agree about who is beside whom. `Excavate`
stays exempt.

**Shared resentment test.** The machine half of `refuses_post` is extracted
as `Game::resents_structure(worker, kind: &StructureKind) -> bool`
(`opinion_of(worker, &MemorySubject::Structure(kind)) <
MEMORY_AVOIDANCE_THRESHOLD`), called by both `refuses_post` and sabotage
(§5) — one formula, two callers.

## 5. Petty sabotage

**A new pass, `Game::note_sabotage`,** in the `note_*` block of
`game/turn.rs` after `note_interactions`, gated like it: tick a multiple of
`INTERACTION_PERIOD`, base established. It is not in `drift_idle_staff`
(that would tie sabotage to wandering) nor in `schedule_base_labour` (it is
not a labour decision).

For each base staff body, in `ProgramId` order:

1. **Who.** `self.sulks(body)` and the body has no `Task`.
2. **What.** Candidate machines: structures with a non-empty
   `Stock.output` whose `Position` is within Chebyshev `SABOTAGE_REACH` of
   the body, where either
   - `resents_structure(body, kind)`, or
   - some staff body posted there (its `Task.target` is the machine) is one
     the saboteur `.avoids()`.

   None → skip.
3. **Whether.** `seed = fold(FNV_BASIS, &[now, id, SABOTAGE_SALT])`;
   proceed only if `unit(seed) < SABOTAGE_CHANCE`.
4. **Which.** The machine is `candidates[index(fold(seed, &[1, SALT]), n)]`
   in entity order; the item is the output entry at
   `index(fold(seed, &[2, SALT]), m)` in `ItemId` order.
5. **Effect.** `hauling::take_from(stock, item, 1)` and
   `note_consumed(item, 1, ConsumeSource::Sabotage)` — a new
   `ConsumeSource` variant with its `as_str` arm (`"sabotage"`). One base-log
   line through `log_base` (`MessageKind::Info`; `Tantrum` is documented as
   one program rounding on another, which this is not):
   `"{saboteur} spoiled a {item} at the {machine}."`, names from
   `creature_label` and the item and structure defs.
6. **Witnesses.** Every other base staff body within Chebyshev
   `BOND_WITNESS_REACH` of the machine gets `saw_sabotage` about
   `MemorySubject::Program(saboteur_id)`, through `Game::remember` —
   `close_brawl`'s witness loop, measured from the machine.

Posting, carrying, power and `MachineStatus` are untouched; a machine
emptied by sabotage simply has less to haul.

**The new memory def,** `assets/memories/saw_sabotage.ron`:

| field | value |
| --- | --- |
| `subject` | `Program` |
| `valence` | `-3.0` |
| `half_life` | `4000` (as `idled_with`) |
| `strike_cap` | `3` |
| `known_for` | `"petty sabotage"` |
| `spreads_as` | `"heard_ill_of"` |

`known_for` must fit the SOCIAL page's width census; `spreads_as` satisfies
the existing rule (resolves, same subject kind, no onward chain, stacked
hearsay below one firsthand strike). `MEMORY_TRIGGERS` gains a row naming
`note_sabotage`.

**New constants** in `tuning.rs`: `SABOTAGE_REACH = 1`,
`SABOTAGE_CHANCE = 0.25`, `SABOTAGE_SALT` (a fresh odd 64-bit word). A
const assert keeps `SABOTAGE_CHANCE` in `(0, 1]`.

## 6. Invariants and failure modes

- **No `GameRng`.** Every choice in §3–§5 is a fold of
  `(tick, participants, salt)`.
- **Empty catalogues.** With `assets/memories/` empty, sabotage still
  spoils and logs; the witness `remember` is a no-op, as every
  `remember` on a missing def already is. With `assets/interactions/`
  empty, `note_interactions` already skips; nothing new depends on it.
- **A machine with no output** is never a candidate, so `take_from` is
  never asked for a unit that is not there.
- **Save/load.** Nothing new is saved; a sulker reloaded mid-sulk behaves
  identically because `Disgruntled.grievance` is already saved.

## 7. Testing

Test-first, one failing reproducer per behaviour:

- **Voice.** `weight_for` with `sulks` multiplies `sulking`; a sulking
  speaker's pick over a fixed seed sweep lands on slights/insults more often
  than the same speaker unsulking (deterministic: same seeds both sides).
- **Gossip.** A sulker holding a stronger positive and a weaker negative
  tellable memory tells the negative one; holding only positive ones, it
  tells nothing.
- **Freeze-out, idling.** Two rivals finishing a need at one amenity write
  no `idled_with`; a neutral pair still does.
- **Freeze-out, posting.** A sulker refuses a post with an avoided body
  beside it and accepts the same post once that body has moved; a
  non-sulker accepts it.
- **Sabotage.** An idle sulker beside a resented machine with output, on a
  tick whose seed rolls under the chance, loses one unit to a `Sabotage`
  consume, writes one base-log line, and gives `saw_sabotage` to a witness
  in reach and not to one out of reach.
- **Reachability.** The same in a base **with** an amenity, where the
  sulker is on respite — the case the roadmap's wording would have missed.
- **Negatives.** No sabotage by a non-sulker, by a sulker with a `Task`,
  at a machine with empty output, or at a machine neither resented nor
  worked by a rival.
- **RNG.** `rng_unadvanced_by` across a tick on which `note_sabotage`
  spoils.
- **Schema and census.** `schema_fault` rejects a negative or non-finite
  `sulking`; the `MEMORY_TRIGGERS` census, the `spreads_as` rule and the
  `known_for` width census pass with `saw_sabotage` shipped.
- `balance_sim` is unaffected (it models no base); the full workspace suite
  is the final gate.

## 8. Files

- `crates/engine/src/game/base/morale.rs` — `sulks`, `resents_structure`,
  `refuses_post`'s second reason.
- `crates/engine/src/interactions.rs` — `sulking` field, `weight_for`,
  `schema_fault`.
- `crates/engine/src/game/memories.rs` — `note_interactions` passes `sulks`;
  `tellable(…, sulker)`; `note_sabotage` (or a new `game/base/sabotage.rs`
  if it outgrows a function).
- `crates/engine/src/game/base/offshift.rs` — `note_idling` filter.
- `crates/engine/src/base_ledger.rs` — `ConsumeSource::Sabotage`.
- `crates/engine/src/game/turn.rs` — call `note_sabotage`.
- `crates/engine/src/tuning.rs` — three constants and an assert.
- `assets/interactions/*.ron`, `assets/interactions/README.md`,
  `assets/memories/saw_sabotage.ron`, `assets/memories/README.md` if it
  lists defs.
- `crates/engine/src/tests/` — the tests in §7 and the census row.
