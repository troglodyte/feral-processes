# A server growing into a mainframe

The last deferred item from `2026-09-04-settlements-design.md` ("Deferred by
decision: settlement 'aid' rewards …, settlements growing from server to
mainframe"). The aid half was claimed and shipped by
`2026-09-05-settlement-aid-design.md`; raids and patrols by
`2026-09-06-town-raids-and-hostile-patrols-design.md`. This is the third and
last, and after it the settlements path has no unclaimed deferrals.

**One branch, one release.** The feature is one latch and one scalar; splitting
it would ship a growth with nothing to grow into or a dwindle with nothing that
had grown.

## Context

Today a settlement's `kind` is authored. `SettlementDef::kind` comes out of a
`.ron` file, `placement::settlement_at` picks a def per region off a fold of the
world seed, and `KnownSettlement` records the resolved def verbatim. That makes
`kind` a property of the map: permanent, save-stable, and unable to disagree
with the ground. It is the rule `settlements/mod.rs:11` states — *"Where a
settlement stands is derived, never stored … there is no spawn, no despawn, and
nothing in the save that could disagree with the ground."*

Growth is the first thing that breaks it. A town that becomes a city is state
with a history, and it is why this is a spec rather than an edit.

`kind` is read in exactly four places, which is what makes the change tractable:

| Site | What it reads |
|---|---|
| `settlement_market.rs:113` | `settlement_rows(def.kind)` — 6 rows or 14 |
| `settlement_market.rs:114` | `settlement_bonus_share(def.kind)` — 15% or 35% |
| `spawning.rs:948`, `:1003` | `kind.glyph()` — the `s` or `M` on the map |
| `inspection.rs:838` | `def.kind.label()` — the town page's second line |

## Decisions taken

Settled in brainstorming on 2026-09-06; recorded so they are not relitigated.

| Question | Decision |
|---|---|
| What drives growth | **Both**: a randomized world clock, with trade volume as a strong accelerant. |
| Whether growth reverses | **Never.** `Server → Mainframe` is a one-way latch. |
| What a starved city loses | **Options, not status.** A dwindled Mainframe still draws `M` and still says "Mainframe"; its shelf thins. |
| What starves it | **Both**: neglect sets the drift, Hostile standing accelerates it. |
| The dwindle's floor | A Server's six rows. A starved city is never *worse* than a town. |
| Whether a Server dwindles | **No.** It has nowhere to fall, and its commerce is doing the growth job. |
| `SETTLEMENT_RAID_RADIUS` vs `_GARRISON_RADIUS` | **Stay equal.** See "The radius call, closed" below. |

## The radius call, closed

`2026-09-06-town-raids-and-hostile-patrols-design.md` left one question open:
whether the two radii should diverge so hostility reaches more worlds than aid.
**Decided 2026-09-06: they stay equal.** A town near enough to help is exactly
a town near enough to hurt, which is a symmetry worth having over an asymmetry
whose only argument is that the world should be meaner. Both remain
`REGION_TILES / 2`, and both remain their own constant — the reason that spec
gave for splitting them stands: retuning the friendly reach must not silently
retune the hostile one.

That closes the last open design question on the settlements path. What remains
open there is tuning, which no instrument in this repo can check.

---

## 1. The model: one latch, one scalar

Two axes, both hanging off `settlements::Relation` in `resources::Standings`.
That resource is already the per-`SettlementKey` mutable half of a settlement,
already `BTreeMap`-ordered for a stable save encoding, and already documented as
tolerating a record for a town the party has never walked to — which routes
needed and growth needs for the same reason.

```rust
pub struct Relation {
    pub standing: i32,
    // … existing fields …

    /// Whether this town has grown into a Mainframe. **One-way**: set once
    /// by `settlement_growth_tick` and never cleared.
    #[serde(default)]
    pub grown: bool,
    /// How the town is doing. **Signed, and 0 is Steady** — see §3.
    #[serde(default)]
    pub commerce: i32,
    /// The last decay epoch folded in, so the drift is settled lazily
    /// rather than per-tick. `static_epoch`'s shape.
    #[serde(default)]
    pub commerce_epoch: u64,
}
```

Plus a fourth the implementation plan surfaced: `commerce_credits: u32`, the
sub-threshold remainder. `Relation::trade_credits` exists for exactly this
reason on the standing axis — without somewhere to keep what is left over, a
player who trades in ten small baskets feeds a town nothing while one who trades
the same volume in a single basket feeds it the lot, which makes the mover a
rounding rule rather than a volume rule. Commerce needs its own remainder rather
than sharing that one, because the two thresholds differ.

Four additive fields behind `#[serde(default)]`, so **no `SAVE_FORMAT_VERSION`
bump**. A RON round-trip test cannot catch a skipped field, so this needs a real
save → load → assert test on all three, not a round-trip.

Commerce is **signed and defaults to 0**, and that is the load-bearing choice.
An unsigned counter starting at zero would band every authored Mainframe —
Tally Yard, Kernel Reach — as Starved in every fresh world, because nobody has
traded there yet. Signed, 0 means "as it was found", and the bands read as a
drift from that.

## 2. The clock: derived, not stored

A **due tick** is folded out of `(seed, key)` with a fifth salt beside
`placement.rs`'s four, reusing that module's `fold`/`salted` (raised to
`pub(super)`) and `derive::index`. Permanent, per-town, and never stored —
`placement.rs`'s three prohibitions apply unchanged: no `GameRng`, no `StdRng`
sequence, and never `%`.

```
due_tick(seed, key) = SETTLEMENT_GROWTH_DUE_MIN
                    + derive::index(salted(region_seed(seed, key), GROWTH_SALT), span)
```

A Server has grown when:

```
now >= due_tick(seed, key) − commerce × SETTLEMENT_COMMERCE_PULL_TICKS
```

Commerce being signed means the expression does two jobs. Positive commerce
pulls the date earlier — trade with a town and it prospers sooner, which is the
"big impact" half. Negative commerce pushes it later, so a neglected or Hostile
Server **stalls**, and the hostility-delays-growth behaviour arrives as a
consequence of the arithmetic rather than as a second rule.

The pass walks `resources::Settlements` — **materialized towns only**, which
is `town_garrisons`' and `raiding_towns`' rule and the same reason: a town whose
tile has never been resolved has no entity to repaint and no name the log could
use.

**The inequality is evaluated in exactly one place.** Once it holds, `grown`
latches, and every reader — shelf, glyph, page — asks
`Game::settlement_kind(key)`, which answers
`def.kind == Mainframe || relation.grown` and nothing more. That is what keeps a
decaying commerce from un-growing a city, and it is why the four call sites in
"Context" cannot drift apart: they get an effective kind, never a `def.kind`.

**No `GameRng` draw anywhere in this feature.** The world clock is a fold, not a
roll. The RNG stream does not shift, so no existing seeded test moves — which is
worth stating because a settlements change that *did* shift it would read as
thirty unrelated failures.

### Discovery is not an event; change is

`ensure_local_settlements` evaluates the inequality **silently** when it
materializes a town. Walk into a fresh region ten thousand ticks into a run and
the city was simply always there — the clock has been running whether or not
anyone was watching, which is the whole point of an ambient half.

Only a town already known as a Server announces its flip. Without this rule the
first tick after entering a region would fire "Kernel Reach has grown into a
Mainframe" about a place the party has never seen, which is both false and
absurd.

## 3. Dwindling

Commerce **bands on read and is never stored** — `relations::band`'s rule
exactly, and for its reason: a retune of the thresholds re-bands every existing
save rather than leaving towns filed under a boundary that has moved.

| Band | Condition | Rows | Bonus share |
|---|---|---|---|
| Thriving | `>= SETTLEMENT_COMMERCE_THRIVING` | `SETTLEMENT_MAINFRAME_ROWS` (14) | 35% |
| Steady | otherwise (**including 0**) | `SETTLEMENT_STEADY_ROWS` (10) | 25% |
| Starved | `<= SETTLEMENT_COMMERCE_STARVED` | `SETTLEMENT_SERVER_ROWS` (6) | 15% |

A **Server ignores the band entirely** and always draws `SETTLEMENT_SERVER_ROWS`
at `SETTLEMENT_SERVER_BONUS_SHARE`. It has nowhere to fall to, and its commerce
is already spoken for by §2.

The floor is the Server's row count on purpose. A city whose shelf fell below a
town's would make the `M` on the map a lie, and the label is the one thing this
feature promised never to take away.

### The drift

Settled by `settlement_growth_tick`, lazily against an epoch —
`static_epoch`'s shape, so a fast-forward cannot be outrun and no per-tick
arithmetic runs over every town:

```
epoch = now / SETTLEMENT_COMMERCE_DECAY_TICKS
if epoch > relation.commerce_epoch:
    elapsed = epoch − relation.commerce_epoch
    rate    = SETTLEMENT_COMMERCE_DECAY
            + (SETTLEMENT_COMMERCE_HOSTILE_DECAY if band(standing) == Hostile else 0)
    relation.commerce = clamp(relation.commerce − rate × elapsed)
    relation.commerce_epoch = epoch
```

Hostility reads the *current* band, not a history: a town you angered and then
repaired stops falling faster the moment it stops being Hostile.

Trade feeds it at `credit_trade_volume` — the one door that already exists for
"the party moved Credits through this town", and which route deliveries and
counter sales both already call. One more line there, `credit_trade`'s
remainder-not-total shape reused so ten small baskets and one large basket feed
commerce identically.

## 4. Where the code goes

- **`settlements/growth.rs`** (new). The due-tick fold, the commerce bands, and
  the row/share mapping. Its own module on `placement`/`relations`/`catalogue`'s
  precedent — growth is neither placement nor relations, and putting it in
  either would make that module answer two questions.
- **`game/settlement_growth.rs`** (new). `Game::settlement_growth_tick` and
  `Game::settlement_kind`. Called from `turn.rs:173`, immediately after
  `ensure_local_settlements()` and beside `maybe_field_patrol()`, for that
  call's own stated reason: it reads the towns that pass resolves.
- **`settlement_market.rs:113-114`** — `settlement_rows` and
  `settlement_bonus_share` take an effective kind and a band.
- **`spawning.rs:948` and `:1003`** — **both**, and `:1003` is the one that
  matters. That is the load path: it respawns every known settlement's entity
  from `settlement.def.kind`, so a grown town left reading the authored kind
  there would draw `M` all run and come back from a save drawn `s`.
- **`inspection.rs:838`** — `views::SettlementView` gains
  `vitality: Option<&'static str>`, `None` for a Server, which has no band to
  report. The `kind` line already says "Mainframe".
- **`notifications.rs`** — one new `NotificationKind` for a first growth. The
  `..._kind_is_fired_by_a_named_site` census in `tests/notifications.rs`
  requires a named firing site, so the variant cannot ship unreachable.
- A message-log line on every flip; the notification only for a town with
  `KnownSettlement.visited == true`. A notification takes the screen, and a
  place the party has never stood in has not earned that.

`Standings` will now hold a record for every town ever materialized rather than
only every town dealt with. That is harmless — `Relation::default()` is standing
0, which bands Neutral, so no consequence changes — and it is bounded by how far
the party has explored.

## 5. Tuning

Twelve constants, and none of them checkable here. See "Open, deliberately".

| Constant | Shape | Why |
|---|---|---|
| `SETTLEMENT_GROWTH_DUE_MIN` / `_MAX` | `u64` ticks | The ambient clock's span. **The one number that can make this feature ship dead** — see below. |
| `SETTLEMENT_GROWTH_SALT` | `u64` | A fifth salt, `placement.rs`'s four's rule: one fold, salted per question. |
| `SETTLEMENT_COMMERCE_CREDITS_PER_POINT` | `u32` | What a Credit of trade is worth in commerce. `SETTLEMENT_TRADE_CREDITS_PER_POINT`'s shape. |
| `SETTLEMENT_COMMERCE_PULL_TICKS` | `u64` | Ticks the due date moves per commerce point. The "big impact" knob. |
| `SETTLEMENT_COMMERCE_DECAY_TICKS` | `u64` | The drift's epoch length. |
| `SETTLEMENT_COMMERCE_DECAY` | `i32` | Points lost per epoch to neglect. |
| `SETTLEMENT_COMMERCE_HOSTILE_DECAY` | `i32` | Extra points per epoch while Hostile. |
| `SETTLEMENT_COMMERCE_MIN` / `_MAX` | `i32` | The clamp every writer goes through, `relations::clamp`'s rule. |
| `SETTLEMENT_COMMERCE_THRIVING` / `_STARVED` | `i32` | The two band thresholds. |
| `SETTLEMENT_STEADY_ROWS` | `u32` | 10 — between the shipped 6 and 14. |
| `SETTLEMENT_STEADY_BONUS_SHARE` | `u32` | 25 — between the shipped 15 and 35. |

### `const _` assertions

Compile-time, `SETTLEMENT_GARRISON_MAX`'s precedent, because closing any of
these by retune must fail the **build** and not merely the suite:

- `SETTLEMENT_COMMERCE_STARVED < 0 && 0 < SETTLEMENT_COMMERCE_THRIVING`. This is
  the one that matters most: a retune taking `THRIVING` to 0 would band every
  untouched authored Mainframe as Thriving, and one taking `STARVED` to 0 would
  band every one of them Starved. §1's whole argument for a signed scalar is
  this assertion.
- `SETTLEMENT_SERVER_ROWS <= SETTLEMENT_STEADY_ROWS <= SETTLEMENT_MAINFRAME_ROWS`,
  and the same for the three bonus shares. A dwindle that fell below the floor
  would make the label a lie.
- `SETTLEMENT_GROWTH_DUE_MIN < SETTLEMENT_GROWTH_DUE_MAX`. The span feeds
  `derive::index`, which needs a non-empty range.
- `SETTLEMENT_COMMERCE_MAX as u64 * SETTLEMENT_COMMERCE_PULL_TICKS <
  SETTLEMENT_GROWTH_DUE_MIN`. Trade can bring a city forward a long way but can
  never make one exist at tick zero.

## 6. Tests

- **The fold.** A region answers the same due tick every time; a different seed
  lays different dates; the dates spread across the authored span rather than
  clustering — `placement.rs`'s four derivation tests, one axis over.
- **The latch is one-way.** Grow a town, drive its commerce to
  `SETTLEMENT_COMMERCE_MIN`, run the pass: still a Mainframe.
- **A dwindled Mainframe never draws fewer rows than a Server**, at
  `SETTLEMENT_COMMERCE_MIN`.
- **A Server's shelf ignores commerce.** Six rows at every band.
- **The effective kind is read through one door.** A census walk asserting no
  `def.kind` survives at the four sites — the drift this feature is most likely
  to grow back.
- **Load draws the grown glyph.** Grow a town, save, load, assert the respawned
  entity's `Glyph.ch` is `M`. This is `spawning.rs:1003`'s test and the one that
  fails without §4's second bullet.
- **Save → load → all three fields.** Not a RON round-trip; a real save.
- **Discovery is silent, change is not.** Materializing a town already past its
  due tick logs nothing; a known Server crossing its date logs and notifies.
- **The notification fires only for a visited town.**
- **Trade pulls the date earlier**, measured as a flip that happens after
  crediting commerce and would not have happened without it — the fix removed
  and the test still passing is the failure mode here.
- **Hostile decays faster than Neutral**, over the same elapsed epochs.
- **Repairing standing out of Hostile restores the slower rate.**
- **The RNG stream does not move.** The workspace suite is the assertion; this
  is noted so a reviewer knows it was considered rather than discovered.

## 7. The measurement this needs

**`SETTLEMENT_GROWTH_DUE_MIN`/`_MAX` cannot be guessed in this file.** The
longest horizon anywhere in `tuning.rs` today is
`SETTLEMENT_BOARD_ROTATION_TICKS` at 1800 ticks. A due tick has to be several
multiples of that or growth is instant and carries no weight; if it is five or
ten multiples it lands past the end of a real session and **the ambient half
ships dead** — green, correct, and never once seen.

So the plan carries a task before the constants are picked: measure how many
ticks a run actually reaches, from `dev-saves/` templates and whatever session
evidence exists, and write it to `docs/measurements/`. The aid reach
measurement (`docs/measurements/2026-09-05-settlement-aid-reach.md`) is the
precedent — it caught a flat radius that found a town in 1.6% of worlds, which
is exactly this failure one axis over.

## Open, deliberately

- **Every constant here is a guess no instrument in this repo can check**, with
  the one exception §7 carves out. `balance_sim` models no towns, no shelves and
  no clock. The aid spec recorded seven such figures, the raids spec thirteen,
  and this adds twelve. Does a city thinning out read as consequence or as the
  game taking something away? Answerable only at the keyboard.
- **Whether a grown Mainframe should change anything but its shelf.** A city
  might reasonably post more contracts, field a larger garrison, or send heavier
  raiders. Every one of those is a new query on an existing exhaustive match and
  none is blocked by this design — but each is a separate consequence with its
  own tuning, and shipping four at once would leave none of them legible.
- **Whether the world should grow towns the party will never see.** It does not,
  today: a region nobody materializes is never evaluated. The clock is honest
  about this — a town is *found* grown rather than *observed* growing — but a
  world that reported its own history would be a different feature.
- **Nothing here will have been played.** The settlements path has landed seven
  phases and a green suite is not evidence of play.

## Before implementation

1. Branch. One branch, per the repo's release-per-change rule.
2. **Read the `seams` reference files for what this touches** — this spec did
   not read them at design time: `references/screens.md` (the town page, the
   market shelf, saves), `references/notifications.md` (the new kind and its
   census).
3. Run §7's measurement **before** picking `SETTLEMENT_GROWTH_DUE_MIN`/`_MAX`.
4. TDD, failing test first.
5. Version bump, `CHANGELOG.md` section, annotated tag at the merge.
