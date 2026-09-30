# Bonds and the SOCIAL tab

**Status:** design. Sub-projects C and D of
`2026-09-16-base-social-roadmap.md`, specced together so a bond is visible
the day it exists. The roadmap's shared decisions all hold here: nothing
draws `GameRng`, every derived value stays derived, the empty memory
catalogue is a supported install, and there is **no save change and no
`SAVE_FORMAT_VERSION` bump**.

A and B (handles, `mood`/`stack_decay`) are built. E (conversations), F
(situational thoughts) and G (sulking behaviours) are out of scope.

## 1. Decisions and why

- **Four bands plus neutral**: Enemy, Rival, Neutral, Friend, Close. Two
  rungs each side gives the SOCIAL page something to say about degree, and
  lets grief scale without a second axis.
- **One departure hook, called from every door** (approach A). There are
  six doors, not the roadmap's three (§4). A per-tick roster diff was
  rejected: it cannot tell a death from a sale, fires on a build commit
  that is later refunded, and scans the roster every tick.
- **The hook takes a `ProgramId` and a name, not an `Entity`.** The build
  door fires in `consume_site`, where the program exists only as a
  `CreatureSave` snapshot.
- **A build spend grieves on completion, not on commit.** `commit_program`
  is reversible through `refund_program`; grieving at commit would need an
  un-grief.
- **A Forgiving battle "death" is not a departure.** `bench_or_dissolve`'s
  Forgiving arm leaves the program `Downed` on the roster.
- **Grief differs by departure kind**: fell, let go, fused — plus one
  relief def for a rival or enemy leaving.
- **SOCIAL is an `App` field, not a `Mode`** (roadmap D), switched with
  `Tab`, the key hint in the header's tab strip rather than the footer —
  the footer has a recorded overflow and a pinned width census.
- **The conversation log is not drawn in D.** E adds its region; until then
  the relationships list takes the height rather than an empty box.
- **Departed programs stay on the relationships list**, tagged `(gone)`,
  named by the memory's stored `subject_name`.

## 2. The band — `crates/engine/src/bonds.rs` (new)

```rust
pub enum Bond { Enemy, Rival, Neutral, Friend, Close }
pub fn band(opinion: f32) -> Bond
```

`settlements::relations::band`'s pattern: a pure function over `tuning.rs`
constants, and every consequence a named query on `Bond` with an
exhaustive match and no `_` arm, so a new rung fails to compile until each
query answers.

Thresholds, placeholders until play (`balance_sim` gates none of this):

| Constant | Value | Reasoning |
|---|---|---|
| `BOND_ENEMY_AT` | `-15.0` | two fresh `turned_on_me` strikes (−14) plus a witness |
| `BOND_RIVAL_AT` | `-3.0` | `MEMORY_AVOIDANCE_THRESHOLD`'s value — the same "worth avoiding" |
| `BOND_FRIEND_AT` | `8.0` | two strikes of `idled_with` or `bonded_in_battle` |
| `BOND_CLOSE_AT` | `20.0` | a full `bonded_in_battle` stack |

Negative thresholds are `<=`, positive `>=`; everything between is
Neutral. The fresh range today is about −28 to +36 before disposition's
±40%.

Named queries (the ones this spec reads): `Bond::avoids()` (Enemy, Rival),
`Bond::grieves()` (Friend, Close), `Bond::relieved()` (Enemy, Rival),
`Bond::label()`.

`Game::bond(holder: Entity, about: ProgramId) -> Bond` is
`band(opinion_of(holder, &MemorySubject::Program(about)))`. Derived on
read, saved nowhere.

## 3. Avoidance and witnessing

**Avoidance.** `drift_idle_staff` (`game/base/work_orders.rs`) gains a
rejection beside the `BaseTile` one: decline a tile 8-adjacent to a staff
body whose `ProgramId` this worker holds at a band where `avoids()`. Signed,
so a fondness never triggers it; a rejected candidate leaves the body where
it stands, as today. Posting is untouched — `schedule_base_labour` gains no
memory term.

**Witnessing.** `tantrum.rs`'s `close_brawl` writes `turned_on_me` on the
victim. It also writes a new `saw_turn_on` (subject: the aggressor) on every
other staff body within `BOND_WITNESS_REACH` (2, placeholder) tiles (Chebyshev) of the
victim, excluding the aggressor and the victim. The aggressor's `ProgramId`
read is hoisted above the branch. `saw_turn_on` is weaker than the victim's
memory: valence −3.0, strike cap 3.

## 4. Departures

```rust
pub enum Departure { Fell, LetGo, Fused }
fn note_departure(&mut self, id: ProgramId, name: &str, how: Departure)
```

For every other owned program holding a memory about `id`: read its band
via `bond`, then write the matching def with subject `Program(id)` and the
given name as `subject_name` (a sibling of `remember` that takes the name,
since `remembered_name` cannot resolve a despawned subject).

| Departure | Holder band | Def | Valence | Half-life |
|---|---|---|---|---|
| Fell | `grieves()` | `lost_in_battle` | −8.0 | 6000 |
| LetGo | `grieves()` | `let_go` | −3.0 | 3000 |
| Fused | `grieves()` | `became_part_of` | −2.0 | 3000 |
| any | `relieved()` | `rid_of` | +3.0 | 3000 |

Strike cap 1 on all four. Values are placeholders. Holders are enumerated
with `owned_pets()`; a despawned subject and a departing co-parent are not
holders. Base-wide and instant, not proximity.

**The six doors**, each calling `note_departure`:

| Door | Where | Departure |
|---|---|---|
| Sold | `sell_companion` → `dissolve_tamed_program` | LetGo |
| Routine extracted | `extract_routine` → `dissolve_tamed_program` | LetGo |
| Permadeath death | `bench_or_dissolve`'s Permadeath arm (six callers: battle teardown, siege board, siege off-screen, outpost raid, raid defence, sortie) | Fell |
| Fused | `fuse_companions`, both parents | Fused |
| Spent on a build | `consume_site`, from the site's `CreatureSave` | LetGo |
| Spent in the study | `settle_research` | LetGo |

`dissolve_tamed_program` takes the departure from its caller rather than
guessing it. Its doc comment ("the one way a tamed program permanently
leaves play") is false and is corrected to point at `note_departure` and
this table. `note_departure`'s doc lists the doors.

## 5. Known for

`MemoryDef` gains `known_for: Option<String>`, `#[serde(default)]`.

`Game::known_for(e) -> Vec<String>` (at most two): sum, by def, every
*other* owned program's `Program(this)` memories using the opinion read;
take the defs carrying `known_for`, heaviest by magnitude first, distinct
phrases. The subject's own store is never read — reputation is what others
think.

| Def | `known_for` |
|---|---|
| `turned_on_me`, `saw_turn_on` | "a brawler" |
| `bonded_in_battle` | "steady in a fight" |
| `idled_with` | "good company" |

## 6. The SOCIAL tab

**app-core.** `App::manifest_tab: ManifestTab { Stats, Social }`. `Tab`
switches it in `handle_manifest_key` when the subject is an owned program;
←/→ paging keeps it; `leave_manifest` resets it to `Stats`. No new `Mode`,
so `ALL_MODES` and the transition table are unchanged. Existing keys (`w`,
`D`, `R`) work from either tab.

**Engine view.** `Game::social(e) -> Option<SocialView>`, `None` for
anything but an owned program:

```rust
pub struct SocialView {
    pub relationships: Vec<RelationshipRow>, // strongest |opinion| first
    pub known_for: Vec<String>,
}
pub struct RelationshipRow { pub name: String, pub bond: Bond, pub opinion: f32, pub gone: bool }
```

Rows are the holder's memories grouped by `Program` subject. `name` is the
live short label, or the stored `subject_name` when the subject is gone.

**gui.** The tab strip `[Tab] STATS · SOCIAL`, open tab highlighted, sits
right-aligned on `draw_header`'s title line, drawn only when `social` is
`Some`. SOCIAL replaces the section grid with a relationships box (`name`,
band label, signed figure) capped at what fits, then the known-for line.
Only `render/` through `Painter`; a new `render/social.rs`.

## 7. Content

- Five new defs in `assets/memories/`: `saw_turn_on`, `lost_in_battle`,
  `let_go`, `became_part_of`, `rid_of`.
- `known_for:` on the four defs in §5.
- `assets/memories/README.md`: `known_for`, and the departure defs.
- `CHANGELOG.md` at release.

## 8. Tests (intent)

Engine:
- `band` thresholds are half-open at all four edges.
- Avoidance: a rival's neighbouring tile is declined; a friend's is not; a
  declined drift leaves the body in place.
- Witnessing: a staff body in reach gets `saw_turn_on`; one out of reach,
  the aggressor and the victim do not.
- Each of the six doors: a friend gets its departure's def, a rival gets
  `rid_of`, a neutral holder gets nothing.
- A committed-then-refunded program writes nothing; a finished build
  writes `let_go`.
- A Forgiving battle death writes nothing.
- Fusion: the co-parent is not written as a holder.
- `known_for` reads only other programs' memories.
- No `GameRng` draw across a tick with a brawl and a departure (the
  `run_routes` stream-unmoved pattern).
- With `assets/memories/` empty, departures and the SOCIAL view work and
  are empty.

Censuses:
- `MEMORY_TRIGGERS` gains the five defs.
- Every shipped `known_for` fits the SOCIAL page width.
- The tallest SOCIAL page fits the frame, measured through the renderer.
- No relationship row is cut, measured through `paint::with_painter`.
- The widest title plus the tab strip fits the header at the narrowest
  window.

app-core: `Tab` toggles, paging keeps the tab, leaving resets it, `Tab`
does nothing on the player's or a wild creature's page.

## 9. Known risk, not addressed

`MEMORY_CAP_PER_PROGRAM` is 12, shared with every other memory. On a large
roster bond memories are evicted by weakness, and a band quietly decays to
Neutral. Left for play rather than retuned blind.

## 10. Phasing

1. `bonds.rs`, `Game::bond`, avoidance (engine).
2. Witnessing and `saw_turn_on`.
3. `note_departure`, the four defs, the six doors.
4. `known_for`, `Game::social` (engine).
5. app-core tab state and keys.
6. gui tab strip, SOCIAL page, censuses.
7. README, dev-save capture, CHANGELOG at release.
