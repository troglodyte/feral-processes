# The HUD

- **`Game::attention` is the one derivation of what needs the player, and
  three surfaces read the same call.** `draw_playing_base` calls it once and
  hands the slice to the status bar's badge and to `hud::column`'s tab
  markers and two collapsed bars; neither has a `Game` to re-derive from,
  which is what makes "a closed pane cannot hide an actionable state" a
  construction rather than three sites agreeing. `AttentionRow::kind` exists
  so `hud::column::tab_of` routes a row into a pane by an **exhaustive
  match** — off the keycap or the prose instead, a `_ =>` arm ships a
  condition with no marker anywhere. **The trap is the pack**: there is no
  pack capacity — `components::Inventory` is an unbounded `Vec` — so the row
  is the *roster's* capacity, and restoring a `pack full` row invents a limit
  the simulation does not have. Two smaller corrections that read as
  arbitrary and are not: the chips name the **top-level map key** (`b`, `p`),
  the handoff's `k` and `m` being walk-west and the Excavation plan here; and
  **threat rows sort first**, because the badge shows only the leading row.
- **`1`/`2`/`3` are bound once, in `handle_playing_key`'s top match**, which
  runs before the hand-off to `handle_stack_key` — the same place `f` sits.
  What is load-bearing is `the_digits_work_underground`, since a key the
  Stack path never sees falls through its `_ => {}` with no refusal and
  nothing in the log, which is how `r` shipped broken.
- **`draw_info_column` owns the column's chrome and returns the open pane's
  body rect.** The column **does not scroll**, so that rect's height is a
  layout constraint.
- **A pane body is rows, and `hud::panes::fitting_rows` is the one place
  what does not fit is counted** — `strip::fitting`'s rule turned ninety
  degrees, written once rather than once per pane. The three builders take
  **no `Painter`**, which is what lets a census read the rows a pane *would*
  draw. **The trap is that `+N more` is itself a row**: spend the whole
  budget on content and the overflow notice draws past the bottom edge,
  which is the exact silence the count exists to break, so the reserve is
  subtracted before the walk and only in the branch that needs it.
- **`PaneData` takes `roster`/`carrying` and not a `&PlayerStatus`**: those
  are every field of it the panes read, and `PlayerStatus` derives nothing,
  so a wider dependency is a census nobody writes. The vitals strip carries
  the bars and the four stats (on the *log* pane's top border — see the last
  bullet), the status bar the zone/position/stock; only the roster, the
  routines and the pack needed homes in the column.
- **The buff-tag ceiling moved with the buffs.** CREW calls `buff_entries`
  rather than restating a buff row, `TagStyle::OwnLine` because the column
  cannot widen. `no_column_row_overflows_the_column` measures a row's left
  run and right tail **joined**, `caravan.rs`' rule — `draw_line`
  right-aligns the tail into the width the left run starts in, so measuring
  the head alone budgets for a narrower row than is drawn.
  `the_tallest_column_pane_fits_its_column` asserts overflow is **rare**
  rather than that nothing is silent.
- **A collapsed bar carries `panes::summary` when calm, and a condition
  outranks it.** Built from the same `PaneData` as the open pane's rows, so
  a bar and the pane it stands for cannot disagree. Exhaustive on `InfoTab`,
  `rows`' rule. PACK summarises to **units carried, never a fraction** —
  there is no capacity to be a denominator.
- **A content hue is authored, and `hud::palette::glyph` is the one table it
  is drawn from** — `render/mod.rs::glyph_color` is a call to it, shared with
  the six popups that draw a program's glyph, since a program reading as one
  colour on the grid and another on its own sheet is the failure to avoid. It
  is a **hue table, not a role table**: a role table cannot know what an asset
  file's hue *is*, and would flatten `difficulty_color`'s four rungs into one
  red. The reservations are held by census instead — br yellow is unreachable
  from content at all, and br red from exactly one hue. Brown and orange
  stand **outside the handoff's sixteen** deliberately: there is no brown in
  it, and the ladder needs a rung between a caution and a kill. **The trap is
  that channel distance cannot see hue** — the table's other red is *further*
  from br red than the orange that replaced it (0.278 against 0.271) and
  still wrong, being the same hue at a different lightness, which the
  vignette eats. The ladder census catches it only because it asserts a
  **margin** (0.10, against a shipped step of 0.12).
- **The player's `@` is a role and is read off `is_player`** — `PLAYER`, br
  cyan, which nothing else may take, and never off the `GlyphColor::Cyan` the
  player happens to spawn with. Asserted by **distance** through a real draw,
  the map dimming everything by a vignette; the second assertion, that the
  `@` is nearer the role than the authored hue, is what stops the test
  passing against the colour it replaced.
- **A machine's stall asks for attention and never reads as a threat.**
  `Clogged`/`Stranded`/`Unpowered` take `ATTENTION` — waiting fixes none of
  them, and it is the colour `Game::attention` already spends on them —
  while `Starved`/`Unstaffed` keep the dimmer `WARN`. The two-band split is
  the information; flattening it loses the difference between a job for the
  player and a machine waiting its turn. The negative half walks
  `MachineStatus::ALL`, `GlyphColor::ALL`'s reason: an exhaustive match
  cannot miss a new variant and a hand-written census can.

- **The map's overlays take palette roles too, and `fx.rs` is why the palette
  is `pub(crate)`** — a raid's flash is painted by the effects layer, and a
  structure taking a hit is what the `THREAT` reservation exists for. Staffed
  mark `HEALTHY`, stranded mark `ATTENTION` (the role its glyph already
  wears; the *blink* is what separates it from staffed), cursor `EMPHASIS`,
  spawn ring and nemesis mark the table's magenta and cyan. **The Excavation
  plan changed colour rather than moving**: its washes sat 0.11 from
  `ATTENTION`, so `palette::PLAN` is br blue — a plan is the player having
  acted, not the base asking them to — and the three washes are built off
  that one constant by alpha. **The build slab's two greys stay out**: the
  palette has no role for "unfinished construction", and reaching for a pane
  border's grey is addressing a value.

- **The expanded log pane is an overlay, and `map_pane` is derived from the
  *collapsed* log at every window size.** Paying for SPACE's four extra rows
  out of the map's height re-lays the whole grid out, so the tiles move under
  a key whose only job is to show more of what has already been read. The
  log's **bottom** edge is the fixed one and it grows upward. **The trap is
  that what makes an overlay free is draw order and nothing states it**:
  `draw_playing_base` calls `hud::log_frame::draw_log_pane` last and the pane
  fills opaquely, so grouping the two framed panes together — a plausible
  tidy — puts the map's fill over the expanded rows, with nothing failing to
  compile and the collapsed pane still drawing correctly.
  `the_expanded_log_pane_draws_over_the_map` locates both fills in paint
  order. The overlay used to cost the vitals, which rode the map's bottom
  border and were covered outright while the log was open; they ride the
  **log** pane's top border now and travel with it, so the expanded state
  costs nothing. Don't restore the old "price" paragraph on the strength of
  this seam's title — the overlay is unchanged, there is just nothing
  underneath it left to lose.

- **One strip to a border, and the vitals get the contested one.**
  `border_strip` centres its quad *on* its line, so it reaches
  `size/2 + pad/2` past it on **both** sides — `STRIP_CLEARANCE_RATIO` is
  that same expression, and what it never covered is two strips reaching for
  each other across the gap between two panes. The map pane's bottom vitals
  and the log pane's top filter header did exactly that: at 1280x720 the
  filter's opaque quad covered `[B, B+9]` of a 21px strip — the lower half of
  the vitals glyphs, baseline included — because `draw_log_pane` runs after
  `draw_map_frame`. **The trap is that a clearance test naming one rect is
  vacuous here**: both of the tests that should have caught it compared the
  vitals quad against `log_pane.y`, the log pane's *body fill*, and the
  arithmetic held while the strip was cut. Ask instead whether **anything
  painted after** the strip lands on it, and count an overlap as an area
  rather than a touch — a pane edge exactly on the quad's is the clearance
  holding. The vitals took the border because they are the readout that must
  never be covered and because the expanded pane would otherwise erase them;
  `map_pane`'s bottom border now carries nothing. The filter header is the
  body's first row again, at `m.small()` and **above** the refusal, and it
  costs a row through its own `LOG_FILTER_ROWS` — `LOG_TEXT_ROWS` stays the
  *message* count, or SPACE grows the pane by five rows instead of four.
  `base.rs`'s `log_capacity` subtracts that same constant, never a second
  literal.
