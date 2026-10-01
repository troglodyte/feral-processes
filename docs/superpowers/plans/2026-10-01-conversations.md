# Plan: conversations (E2)

Spec: `docs/superpowers/specs/2026-10-01-conversations-design.md` (the
argument; this file is only the order of work). Branch `conversations`,
primary checkout. TDD per task, a commit per green step, `cargo fmt` +
`cargo clippy --workspace --all-targets` before each commit. Read
`.claude/rules/seams-memories.md`, `seams-screens.md`, `content-schema.md`
and `drawing-seam.md` before Phase 1. One additive `#[serde(default)]` save
field, no `SAVE_FORMAT_VERSION` bump (stays 33), no `GameRng`.

Iterate with `cargo test -p <crate> <name>`. A phase gate is its named tests
plus clippy; the full suite and `balance_sim` run once, at the end. Phases
are sized for one subagent each; 1→2→3 are serial, 4 needs 1, 5 needs 3, 6
needs 2.

## Phase 1 — schema and loader (engine)

**T1. Types** in `interactions.rs`: `enum Role { Speaker, Listener }`,
`struct Line { by: Role, text: String }` (both `Serialize, Deserialize,
Clone, Copy`-where-possible, `PartialEq`), `InteractionDef::exchanges:
Vec<Vec<Line>>` `#[serde(default)]` (struct at `:25`).
`CONVERSATION_MAX_LINES: usize = 4` and `CONVERSATION_RING: usize = 32` in
`tuning.rs` beside `INTERACTION_SALT` (`:4603`).

**T2. Validation** in `InteractionDb::load_dir` (`:85`): per exchange, drop
with a warning naming file + index if line count ∉ `2..=MAX` or any `{…}`
token is not `speaker|listener|topic`. Def still loads. One pure
`fn slots(text) -> impl Iterator<&str>` shared by the loader, the renderer
(T7) and the census (T10) — the CLAUDE.md "a call, not a copy" rule.
Tests: unknown slot / 1 line / 5 lines each drop + warn, def loads; no
`exchanges` key loads.

## Phase 2 — record and pass (engine)

**T3. `ConversationRecord`** (spec §5, `Serialize, Deserialize, Clone,
PartialEq, Debug`) and component `Conversations(VecDeque<…>)` with
`push_front` capping at `CONVERSATION_RING`. Put both in `interactions.rs`
unless `components.rs` holds every component (check; follow it). Test: 33
pushes keep 32, oldest gone.

**T4. Topic** — pure `fn topic(memories, speaker, listener, seed) ->
Option<MemorySubject>` per spec §3 (dedupe by subject, subject order,
`index(fold(seed, &[2, INTERACTION_SALT]), len)`); gossip overrides with
`tellable`'s subject. Name stamped via `remembered_name`
(`game/memories.rs:915`). Tests: never speaker/listener; no candidates →
`None`.

**T5. Exchange pick** — pure `fn pick_exchange(def, has_topic, seed) ->
Option<u8>` per spec §4 (`[3, INTERACTION_SALT]`). Tests: no topic → only
topic-free indices; none qualify → `None`.

**T6. Wire into `note_interactions`** (`game/memories.rs:484`): collect a
mirrored record pair beside E1's `writes`, apply after the loop (insert
`Conversations` if absent — owned staff only). Tests in
`tests/interactions.rs`, reusing E1's fixture and tick-search: one record
each side, mirrored `role`/`other`; none when nothing fires; gossip topic =
told subject; cloned state → identical records. Mutation-check each.

## Phase 3 — read, render, save (engine)

**T7. `Game::conversations(e) -> Option<Vec<ExchangeView>>`** beside
`Game::social` (`:1013`); `ExchangeView`/`SpokenLine` in `views.rs` beside
`SocialView`. Names per spec §6 via `program_entity` (`:924`) +
`creature_short_label` (`game/party.rs:469`), stamped fallback. Missing
def/index → fallback line `"{speaker} and {listener}: {name}"` (id if def
gone). Tests: slots fill; rename reads through; departed uses stamp; missing
def and bad index fall back; unowned → `None`.

**T8. Save.** `CreatureSave` (`save.rs:454`) gains `#[serde(default)]
conversations: Vec<ConversationRecord>` beside `memories` (`:797`), doc
comment in `memories`' style. Write at `lifecycle.rs:~2524`, restore at
`:~2143`. ~27 `CreatureSave {` literals need the field
(`rg -n "CreatureSave \{" crates`; 16 in `app-core/src/tests/support.rs`).
Tests: save→load through `Game` keeps records (not a RON round trip); a
save without the key loads empty.

## Phase 4 — content (assets)

**T9. Exchanges** for all ten `assets/interactions/*.ron`, two or three
each, terse machine voice, per spec §9 (gossip all `{topic}`; small_talk,
shop_talk, complain, commiserate each ≥1 `{topic}`; every non-gossip def ≥1
topic-free). `assets/interactions/README.md`: `exchanges`, `Line`, `Role`,
the three slots, line cap, the fallback.

**T10. Census** (`tests/assets.rs`): shipped interactions load with zero
warnings; every def has ≥1 exchange; every gossip exchange uses `{topic}`;
count still 10 (slots/line range are covered by zero warnings — say so in a
comment rather than re-checking).

## Phase 5 — TALK tab (app-core + gui)

**T11. app-core.** `ManifestTab::Talk` (`app-core/src/lib.rs:1221`); cycle
in `app/inspection.rs:318`: Stats→Social→Talk→Stats. Resets at `:382/:418`
unchanged. Extend `tests/social_tab.rs`: three-tab cycle owned, Stats-only
for others, reset.

**T12. `crates/gui/src/render/talk.rs`** via `Painter` +
`manifest_layout`, dispatched in `render/manifest.rs:82` like social. One
CONVERSATIONS section; `who: text` rows, wrapping; whole exchanges newest
first until the next would not fit; empty note. Tab strip in
`render/social.rs:76` gains TALK (move the strip to `manifest_layout` if
talk.rs would otherwise import from social.rs). Tests: longest shipped line
× longest name wraps inside the tightest window; never a partial exchange;
empty state; `the_widest_title_and_the_strip_fit_the_header` still passes.

## Phase 6 — speech mark (engine + gui)

**T13. Engine.** `SpeechCue { cell: (i32, i32) }` + `SpeechQueue` in
`resources.rs` modelled on `TransitQueue` (`:817`); `init_resource` where
`TransitQueue` is; push the speaker's cell in `note_interactions`.
`Game::take_speech()` beside `take_transits` (`game/base/upkeep.rs:211`).
Test: one cue per fired interaction, drained empty.

**T14. gui.** Both `take_transits` sites (`gui/src/lib.rs:838`, `:1719`)
also drain speech, passed to `Fx::begin_frame` (`fx.rs:866`; always
consumed even when disabled). Constants in `fx.rs`: `SPEECH_GLYPH = '"'`
(in both DejaVu and UNSCII), `SPEECH_SECONDS = 1.5`. Draw only when
`base_pos().is_some()`. Test: copy
`a_departure_walk_draws_in_base_space_and_never_on_the_surface`
(`lib.rs:1827`).

## End gate

`cargo test --workspace`; `cargo test -p feral-processes-engine
balance_sim` (must not move); clippy clean. Final whole-branch review (opus,
diff as a file). At landing: CHANGELOG `## 0.14.3` (patch: additive save
field), INDEX row, tag. Not played at the keyboard — say so in the report.
