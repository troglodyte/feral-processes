# Conversations (E2)

**Status:** design. Sub-project E2 of `2026-09-16-base-social-roadmap.md`
§E, building on `2026-10-01-interactions-design.md` (E1, `v0.14.2`). The
roadmap's shared decisions hold: nothing draws `GameRng`, every derived value
stays derived, and an empty catalogue is a supported install.

**This is the first social sub-project with a save change:** one additive
`#[serde(default)]` field on `CreatureSave`. `SAVE_FORMAT_VERSION` stays 33
(field-named RON; `.claude/rules/seams-screens.md`), so the release is a
patch.

**Out of scope:** G's sulker slant, manifest scrolling, and tuning the
interaction rate.

## 1. Purpose and decisions

E1 moves opinion, but the player sees only memory names: `slighted_by`
appears and a relationship row drops. E2 shows what was said, so a moved
opinion can be read back to its cause.

- **Dialogue.** An exchange is two to four paired lines, spoken by the
  speaker or the listener. It is not a narrated summary.
- **Records, rendered on read.** No generated text is stored. A record names
  its interaction, which exchange was used, and who and what it was about.
  The text is rendered from the current templates whenever the page is drawn.
- **A third manifest tab, TALK.** The manifest does not scroll, and the
  SOCIAL page is already sized to its tightest window
  (`the_tallest_social_page_fits_the_tightest_window`). A box on SOCIAL would
  leave the 32-record ring mostly unread. A tab of its own leaves SOCIAL's
  height census untouched.
- **Exchanges live on `InteractionDef`, not in a separate
  `assets/conversations/` directory.** A modder edits one file per
  interaction, and no template can name a def that does not exist.
- **The topic is a memory the speaker holds.** People talk about what is on
  their mind.
- **Its own log, visible nowhere else.** Not `MessageLog`, not a
  `MessageSource` (the roadmap's rejection stands). The TALK tab is the only
  reader.
- **A speech mark on the base map** gives the social layer a sign on screen
  without opening a page.

## 2. Content schema

### `InteractionDef::exchanges: Vec<Vec<Line>>`

`#[serde(default)]`. Each inner `Vec` is one exchange.

```ron
exchanges: [
    [
        (by: Speaker, text: "Your last cycle on {topic} was sloppy."),
        (by: Listener, text: "Noted, {speaker}."),
    ],
],
```

`Line { by: Role, text: String }`, where `enum Role { Speaker, Listener }`.

Slots: `{speaker}`, `{listener}` and `{topic}`. Nothing else is a slot.

**Load-time validation** (in `InteractionDb::load_dir`, warn and skip, never
panic):

- An exchange with an unknown slot, or with fewer than 2 or more than
  `CONVERSATION_MAX_LINES` (4) lines, is dropped with a warning.
- The def itself still loads, with its remaining exchanges.

**A def with no exchanges is valid** (an older mod). Its record renders as
one narrated fallback line, `"{speaker} and {listener}: {name}"`. The def's
`name` was reserved for E2 in E1's schema.

`assets/interactions/README.md` documents `exchanges`, `Line`, the slots and
the line cap.

## 3. Topic

When an interaction fires (inside `note_interactions`, where `speaker_id`,
`listener_id`, `def` and `tellable` are already in scope):

- **Gossip:** the topic is the subject of the told memory, the `about`
  program from `tellable()`.
- **Otherwise:** the candidates are the speaker's `Memories` entries whose
  subject is not `Nothing` and is neither the speaker nor the listener. They
  are de-duplicated by subject and taken in subject order. One is picked by
  `index(fold(seed, [2, INTERACTION_SALT]), len)`. With no candidates there
  is no topic.
- The topic is stored as `(MemorySubject, String)`. The name is stamped at
  write time through the same helper memories use (`remembered_name`), so a
  departed or renamed subject stays readable.

## 4. Exchange pick

- Filter `def.exchanges` to those that use `{topic}` only when a topic
  exists.
- Pick by `index(fold(seed, [3, INTERACTION_SALT]), len)`.
- If none qualify, or the def has none, store `exchange: None` and render
  the fallback.

## 5. Record and ring

```rust
pub struct ConversationRecord {
    pub tick: u64,
    pub interaction: String,          // InteractionDef id
    pub exchange: Option<u8>,
    pub role: Role,                   // this program's side
    pub other: ProgramId,
    pub other_name: String,           // stamped; live name wins on read
    pub topic: Option<(MemorySubject, String)>,
}
```

- **Component `Conversations(VecDeque<ConversationRecord>)`**, newest first,
  capped at `CONVERSATION_RING` (32, `tuning.rs`). Pushing past the cap drops
  the oldest.
- **Both participants get a record**, with mirrored `role` and `other`. The
  records are collected beside E1's `writes` and applied after the loop, the
  `note_*` pattern.
- **Save:** `CreatureSave` gains `#[serde(default)] conversations:
  Vec<ConversationRecord>`; the record derives `Serialize`/`Deserialize`
  directly, as `MemorySubject` already does, so no mirror type. It is written in `lifecycle.rs` beside `memories:` and
  restored beside them.
- `tick` is kept for ordering and tests only. It is never shown, per the
  rule against player-facing tick vocabulary.

## 6. Reading: `Game::conversations`

```rust
pub struct ExchangeView { pub lines: Vec<SpokenLine> }
pub struct SpokenLine { pub who: String, pub text: String }
```

`Game::conversations(e) -> Option<Vec<ExchangeView>>` returns newest first.
It returns `None` for a program the player does not own, as `Game::social`
does.

**Rendering happens in the engine**, so the gui draws only strings.

**Names:**
- This program is resolved live.
- `other` is resolved live through `program_entity` and `creature_short_label`,
  so a rename reads through. The stamped `other_name` is used only when the
  program is gone.
- The topic uses its stamped name. A `Program` topic that is still live
  resolves live, like `other`.

**Missing data degrades and never panics.** An `interaction` id or an
`exchange` index that no longer resolves (a mod removed or reordered it)
renders the fallback line, using the id if the def itself is gone.

## 7. The TALK tab

- **`ManifestTab`** gains `Talk`. Tab cycles STATS → SOCIAL → TALK → STATS
  for owned programs only, and resets to STATS where it does today.
- **Renderer:** `crates/gui/src/render/talk.rs`, through `Painter` and
  `manifest_layout`.
  - One full-width section, CONVERSATIONS.
  - Each exchange is a group of `who: text` rows. Long lines wrap, never
    truncate.
  - The page shows **as many whole exchanges as fit**, newest first. It never
    shows part of an exchange.
  - With no conversations, a note says so, so no section is drawn empty.
- **The tab strip** gains TALK.
  `the_widest_title_and_the_strip_fit_the_header` covers it.

## 8. Speech mark

- **`SpeechCue { cell: (i32, i32) }`** goes into `SpeechQueue`, a resource
  modelled on `TransitQueue`: capped at `EFFECT_QUEUE_CAP`, oldest dropped,
  cosmetic and never saved. `note_interactions` pushes the speaker's cell for
  each interaction that fires.
- **`Game::take_speech()`** drains it. The gui drains it every frame whether
  or not it draws (as at `lib.rs` for transits). It draws a short-lived
  glyph over the cell, only when `base_pos().is_some()`.
- The glyph and its lifetime are gui constants, chosen in the plan.

## 9. Content

Each of the ten shipped interactions gets two or three exchanges in a terse
machine voice. These use `{topic}`:
- every gossip exchange;
- at least one exchange each for small talk, shop talk, complain and
  commiserate.

Every def keeps at least one topic-free exchange, except gossip, which
always has a topic.

## 10. Tests (intent)

**Engine:**
- **Loader:** an unknown slot, too few lines, or too many lines each drop
  the exchange with a warning, while the def loads; a def with no
  `exchanges` loads.
- **Pass:** an interaction that fires writes one record on each side with
  mirrored role and other; nothing is written when no interaction fires; the
  ring is capped at 32 and evicts the oldest; the same state gives the same
  records.
- **Topic:** never the speaker or the listener; gossip's topic is the told
  subject; no candidates gives no topic and only topic-free exchanges.
- **Render:**
  - slots fill;
  - a rename reads through for a live `other`;
  - a departed `other` uses the stamped name;
  - a missing def or exchange index falls back without panicking.
- **Save:** a save then load round trip through `Game` keeps the records
  (a RON round trip alone cannot catch a skipped field). A save without the
  field loads with empty rings.
- **Speech:** one cue per fired interaction, drained by `take_speech`.

**Census (`tests/assets.rs`):**
- every shipped exchange's slots are known and its line count is in range;
- every shipped def has at least one exchange;
- gossip's exchanges all use `{topic}`;
- the interaction count (10) is unchanged.

**gui:**
- the longest rendered shipped line, with the longest possible name, wraps
  within the TALK page at the tightest window;
- the page never shows part of an exchange;
- the empty state renders;
- the tab strip fits;
- the speech mark draws in base space and never on the surface (mirroring
  the transit test).

**app-core:** Tab cycles three tabs for an owned program, stays on STATS for
others, and resets as today.

`balance_sim` must not move: nothing here is on a combat path.

## 11. Known risk, not addressed

- **The voice is unplayed.** Templates are authored blind, as E1's weights
  were.
- **Save growth:** up to 32 records per staff program. These are small
  structs, and staff counts are small.
- **The rate is E1's.** A quiet base shows a sparse TALK page. That is a
  tuning question for after play.
