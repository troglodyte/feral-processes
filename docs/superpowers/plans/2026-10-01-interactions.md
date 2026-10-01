# Plan: interactions and gossip (E1)

Spec: `docs/superpowers/specs/2026-10-01-interactions-design.md` (the
argument; this file is only the order of work). Branch `interactions`, primary
checkout. TDD per task, a commit per green step, `cargo fmt` + `cargo clippy
--workspace --all-targets` before each commit. Read
`.claude/rules/seams-memories.md`, `seams-base.md` and `content-schema.md`
before Phase 1. No save change, no `SAVE_FORMAT_VERSION` bump, no `GameRng`.

Iterate with `cargo test -p feral-processes-engine <name>`. A phase gate is its
named tests plus clippy; the full suite and `balance_sim` run once, at the end.

## Phase 1 — data (engine)

**T1. Derives.** `Bond` (`bonds.rs:12`) gains `Serialize, Deserialize`;
`Disposition` (`disposition.rs:46`) gains `PartialOrd, Ord`. No test; clippy.

**T2. `MemoryDef::spreads_as: Option<String>`** (`memories.rs:~94`,
`#[serde(default)]`). Validate in `MemoryDb::load_dir` after all defs load
(needs the target): drop with a warning if the def is not `Program`-subject,
the target is missing, not `Program`-subject, or itself has `spreads_as`.
Tests: each of the four rejections warns and drops; a valid one survives.

**T3. `crates/engine/src/interactions.rs`** (`pub mod interactions;`).
`InteractionDef` per spec §2 table; `InteractionDb::load_dir(&Path, &MemoryDb)
-> io::Result<(Self, Vec<String>)>`, shape of `ThoughtDb::load_dir`
(`situations.rs:87`): absent dir empty, sorted files, malformed skipped,
non-finite/negative `weight` or any multiplier skipped, duplicate id → first
wins + warning, unresolved `listener_memory`/`speaker_memory` skipped (a
`gossip` def's `listener_memory` is not resolved). `iter()` in id order.
Tests: one per skip rule + absent dir.

**T4. Wire loading** — `game/lifecycle.rs`: bundle field beside `thoughts`
(~:3371), load after `MemoryDb` (~:3502, same absent-is-silent comment), both
`insert_resource` sites (grep `ThoughtDb`). Also the gui's
`render/party.rs:1359` test fixture only if it builds a full asset set.

## Phase 2 — pure pieces (engine)

**T5. Tuning** (`tuning.rs`, near `MEMORY_POSTING_PERIOD` :4578):
`INTERACTION_PERIOD: u64 = MEMORY_POSTING_PERIOD`, `INTERACTION_CHANCE: f64 =
0.5`, `SOCIABILITY_RESERVED/_SOCIABLE/_CHATTY: f64 = 0.5/1.0/1.5`, salts
`SOCIABILITY_SALT`, `INTERACTION_SALT` (any distinct odd u64s), plus `const _:
() = assert!(INTERACTION_PERIOD >= MEMORY_POSTING_PERIOD);` with the spec's
one-line reason.

**T6. `derive::unit(seed) -> f64`** in `[0,1)` from the top 53 bits. **Read
`derive.rs:72-95` first:** a word folded *last* that differs only in low bits
never reaches the high bits. So every fold this feature reads ends on a
constant salt word: roll = `fold(FNV_BASIS, &[tick, speaker, listener,
INTERACTION_SALT])` (deviation from spec §4's word order, same inputs). Test:
`unit` in range; adjacent listener ids give decorrelated rolls (copy the shape
of the bit-63 test at `derive.rs:~151`).

**T7. `crates/engine/src/sociability.rs`** — `Sociability { Reserved,
Sociable, Chatty }`, `of(ProgramId)`, `chance_mult() -> f64`, `name() ->
&'static str`. Seed ends on the salt per T6 (`fold(FNV_BASIS, &[id,
SOCIABILITY_SALT])`). Tests: stable per id; all three occur over 1000 ids,
each ≥ 20%; partition differs from `Disposition::seed`'s (some id pair same
disposition, different sociability, and vice versa).

**T8. `interactions::pair_idle(&[(ProgramId, IVec2)]) -> Vec<(ProgramId,
ProgramId)>`** — sort by id, greedy nearest within Chebyshev
`BOND_WITNESS_REACH`, ties → lower id. Tests: spec §7 Pairing bullet.

**T9. `interactions::pick(&InteractionDb, Disposition, Bond, gossip_ok, seed)
-> Option<&InteractionDef>`** — weighted pick from `fold(seed, &[1,
INTERACTION_SALT])`. Tests: over 200 fixed seeds against the shipped-shaped
fixture, `slight`+`insult` outnumber `compliment` for `Rival`/`Abrasive` and
the reverse for `Close`/`Amiable`; gossip never picked when `!gossip_ok`;
all-zero → `None`; empty db → `None`.

## Phase 3 — the pass (engine)

**T10. `Game::note_interactions`** in `game/memories.rs`; call in
`game/turn.rs` *after* `note_low_power` (:351) — last of the `note_*` block,
with a comment giving the spec's ordering reason. Steps per spec §4: gate
(`tick.is_multiple_of(INTERACTION_PERIOD)`, `base_is_established`, db
non-empty) → idle staff (base staff, no `Task`; copy `note_idling`'s staff
query in `game/base/offshift.rs`) → `pair_idle` → roll × `Sociability` →
`pick` with `self.bond(speaker, listener_id)` (:672) → collect writes, then
`remember` each.

**T11. Gossip** inside the same pass: `tellable(speaker) -> Option<(def
spreads_as, ProgramId)>` = strongest by `Disposition::felt` intensity among
the speaker's memories whose def has `spreads_as` and whose subject is a
program ≠ listener ≠ speaker; ties → lower id. `gossip_ok =
tellable.is_some()`. Departed subjects allowed — use `remember_named` with the
memory's stamped `subject_name` when the subject has no live body.

Tests (new `crates/engine/src/tests/interactions.rs`, start from a
`dev-saves/` template with an established base if one exists, else the
`tests/situations.rs` fixture; insert a hand-built one-def `InteractionDb`.
No test-only chance override: pick a tick whose roll passes by a pure search
over `unit`, then step the game to it):
- off-period and pre-established → no writes; lone idle → none; `Task` body
  never paired; one interaction per program per pass; two runs from a cloned
  state write the same memories.
- gossip: never told to the subject; never about the speaker; strongest wins;
  over two passes, the hearsay a listener received is not retold (hearsay defs
  have no `spreads_as`).
- Mutation-check each: revert the guard, see the test fail.

## Phase 4 — content (assets)

**T12. Memory defs**, `assets/memories/*.ron`, all `subject: Program`,
`strike_cap: 3`, no `known_for`, blurbs short (`no_memory_row_overflows_its_popup`
is the gate). Valence per spec §6 table; `half_life`: interaction defs 2000,
hearsay 1500. Add `spreads_as` to `turned_on_me`, `saw_turn_on` →
`heard_ill_of`; `idled_with`, `bonded_in_battle` → `heard_well_of`.

**T13. Interaction defs**, `assets/interactions/*.ron` (`by_disposition` keys
omitted = 1.0; Steady always 1.0):

| id | listener / speaker memory | weight | by_band (Enemy, Rival, Neutral, Friend, Close) | by_disposition |
| --- | --- | --- | --- | --- |
| small_talk | chatted_with / chatted_with | 3.0 | 0.3, 0.6, 1, 1.3, 1.5 | Amiable 1.3 |
| shop_talk | talked_shop_with / same | 2.0 | 0.3, 0.6, 1, 1.2, 1.3 | Dogged 2.0 |
| joke | laughed_with / same | 1.5 | 0.1, 0.4, 1, 1.5, 2.0 | Amiable 1.5, Languid 0.7 |
| compliment | complimented_by / — | 1.0 | 0.05, 0.2, 1, 1.5, 2.0 | Amiable 1.8, Abrasive 0.3 |
| thanks | thanked_by / — | 1.0 | 0.1, 0.4, 1, 1.4, 1.6 | Amiable 1.4 |
| commiserate | commiserated_with / same | 1.0 | 0.2, 0.5, 1, 1.5, 1.5 | Languid 2.0 |
| complain | complained_at_by / — | 1.0 | 1.5, 1.3, 1, 0.7, 0.5 | Languid 2.0, Abrasive 1.3 |
| slight | slighted_by / — | 0.6 | 3.0, 2.5, 1, 0.3, 0.1 | Abrasive 2.5, Amiable 0.3 |
| insult | insulted_by / — | 0.3 | 4.0, 3.0, 0.3, 0.02, 0.01 | Abrasive 3.0, Amiable 0.1 |
| gossip | (ignored) / — | 1.0 | 1, 1, 1, 1.3, 1.5 | Amiable 1.2 |

Check T9's direction test against these numbers; adjust the table, not the
test, if a tilt fails.

**T14. READMEs.** New `assets/interactions/README.md` (deletable dir, RON
example, field table, gossip rule incl. never-about-itself and one hop).
`assets/memories/README.md` gains `spreads_as` with its validity rules.

**T15. Census** (`crates/engine/src/tests/assets.rs`): shipped interactions
load with zero warnings; every `spreads_as` resolves to a `Program`-subject def
with no `spreads_as`; `|hearsay valence| < |source valence|`; `MEMORY_TRIGGERS`
gains a row per new def (11) naming `note_interactions`; whatever census
asserts every def is written learns them.

## Phase 5 — SOCIAL tab (engine view + gui)

**T16.** `SocialView` (`views.rs:3657`) gains `sociability: &'static str`,
filled in `Game::social` (`game/memories.rs:877`) from
`Sociability::of(id).name()`. `render/social.rs::social_sections` puts it as a
line in the KNOWN FOR section; update the two test constructors there
(:157, :181). Tests: the line renders; the tallest-page height test still
passes.

## End gate

`cargo test --workspace`, `cargo test -p feral-processes-engine balance_sim`
(must not move), clippy clean. Then the final whole-branch review (opus,
diff as a file), CHANGELOG `## X.Y.Z` entry and INDEX row at landing. Not
played at the keyboard — say so in the report.
