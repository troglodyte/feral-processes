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
- **A tile's con read takes the glyph's own hue whenever it can, and
  `ConRead` is the one place that is decided.** The glyph is the better home
  by a distance — it is the ink the eye lands on scanning a screenful, and
  it costs only the species' authored hue, which answers *what is this*, a
  question asked on a tile the player has already stopped at. Exactly two
  tiles cannot spend it: one drawn as a **sprite**, because art is authored
  near-white and egui *multiplies* the tile colour through it, so a con rung
  repaints the drawing rather than tinting it; and a **boss**, whose magenta
  is the ink. Those pay with a right-angled earmark folded into the
  **top-left** corner — `difficulty_mark_points`, leg `CON_MARK`. **The
  property the type exists for is "never both and never neither"**: a rung
  on the glyph *and* in the corner reads as two different creatures, a rung
  in neither reads as harmless, and two conditions agreeing at two draw
  sites is how either arrives — so `ConRead` returns one value and the
  matrix is swept rather than its interesting corner. **The second trap is
  the gate**: `drew_sprite` is the sprite call's own answer, never
  `sprite.is_some()`. A name the table has nothing under falls back to the
  glyph — the whole of what keeps `assets/sprites/` optional — and that
  glyph is free to carry the rung; read off the name instead, every tile
  whose art failed to load pays a corner for nothing, which is why the
  sprite attempt is **hoisted out of the glyph draw** and `drew_sprite` is a
  `let` and not a `mut` seeded with a default. **The third is the top edge,
  which carries three things**: the rarity bar owns its full width and is
  painted *first*, so the earmark drops below `RARITY_BAR_PX` exactly as
  `nemesis_mark_rect` does, and the nemesis mark is the other neighbour —
  `CON_MARK`'s leg is a fraction of the tile where that mark's inset is
  absolute, so the gap is narrowest at the deepest zoom and the census
  sweeps 24→64px. It is a **`poly` and not a `rect`** on purpose: form, not
  a fifth coloured strip, is what tells two top-edge readings apart. A boss
  wears no corner mark of its own any more — its magenta *is* the fact —
  and the census that used to hold that mark apart from every other mark now
  holds the **hue** apart from every con rung, which is the reading it could
  actually be mistaken for. `EntityView::difficulty` is `None` for anything
  non-hostile, *no reading* rather than one worth nothing, because either
  home on a companion says the player can beat their own program.
  **`staffed_mark_rect` outlived the bottom bar it was extracted to clear**
  and is still worth its own test, because its lift is `Fx::staffed_bob`: a
  resting place that has drifted off the tile is invisible while a machine
  is worked and shows only at rest, and for a stranded mark, which never
  bobs at all.
- **The player's `@` is a role and is read off `is_player`** — `PLAYER`, br
  cyan, which nothing else may take, and never off the `GlyphColor::Cyan` the
  player happens to spawn with. Asserted by **distance** through a real draw,
  the map dimming everything by a vignette; the second assertion, that the
  `@` is nearer the role than the authored hue, is what stops the test
  passing against the colour it replaced.
- **The player's drawn icon is the one sprite drawn untinted, and the
  player tile's fallback is four rungs.** The pixel editor's icon
  (`sprites::DRAWN_ICON_KEY`, `"@drawn"`) sits above `PlayerLook::sprite`
  and above the `@`, and it draws at `Color::new(vig, vig, vig, color.a)` —
  the vignette's value with the hue dropped — where every other sprite in
  the game takes the tile's tint. **The trap is that putting the hue back
  reads as a bug fix.** `assets/sprites/README.md`'s near-white rule exists
  because egui's tint *multiplies*, so art must not carry a hue of its own;
  a reviewer holding that rule sees flat grey among coloured tiles and
  concludes the tint was dropped by omission. It was not: the player's tile
  is the only one that inherits no hue to protect — no species colour, no
  `biome_tint`, no damage dimming, verified in `render/base.rs`'s
  `is_player` arm — and the drawing is the one sprite whose colours the
  player chose by hand, so multiplying it by an indigo swatch turns most of
  it black. What that costs is the Colour step's swatch on this one tile,
  and `App::creation_colour_note` says so on the step where the choice is
  made. `the_drawn_icon_is_drawn_untinted` passes `colour: Some(3)`
  (indigo) rather than the default, so a regression handing the tinted
  `color` through is red on `r != g`. The overdraw rule from the sprite
  seam applies with no exception here: a drawn icon is transparent
  somewhere by construction, so the test asserts the mesh **and** the
  absent `@`. **The player edits 8x8 and the sprite is 16x16** —
  `ICON_GRID` against `ICON_SIZE`, with `ICON_CELL_PIXELS` the one
  expression of the ratio — so each drawn cell fills a 2x2 block of the
  uploaded texture, which under nearest sampling is pixel-identical to a
  native 8x8 one. The trap there is reading the two as one number:
  changing `ICON_GRID` is a screen decision, changing `ICON_SIZE` breaks
  the sprite format `assets/sprites/README.md` calls non-negotiable. The
  full argument is `docs/seams.md`, "The player's drawn icon is the one
  sprite drawn untinted".
- **`ICON_PALETTE` is exactly fifteen entries because it is the player
  icon's save format, and `SPRITE_PALETTE` is a separate constant for that
  reason.** `PlayerIcon::encode` writes one lowercase hex digit per cell
  (`char::from_digit(index, 16)`); a hex digit has sixteen values and index
  `0` is already spent on transparent, so fifteen is the whole remaining
  budget, not a stylistic count. **Nothing in the compiler holds this** —
  `PlayerIcon::set`'s bound check (`index as usize > ICON_PALETTE.len()`)
  accepts a sixteenth entry just as happily as a fifteenth, and `Canvas`
  (shared with the dev sprite editor's own canvas) carries no palette or
  range guard at all. The one thing that notices is a pinned test,
  `the_palette_has_room_for_exactly_fifteen_colours_because_one_hex_digit_encodes_them`,
  and a test phrased as an assertion of a specific number reads as "update
  it" rather than "this is load-bearing" to a careless green-the-suite
  pass. **The trap is the "fix" for what happens next, not the growth
  itself.** A sixteenth colour compiles clean and only panics at
  `encode`'s `.expect("palette index fits one hex digit")` the first time a
  player paints with it and saves — loud, not silent, as the code stands.
  Silence is one edit further: replace that `.expect` with a wrap or a mask
  to make the panic go away, and index `16` becomes the hex digit `'0'`,
  which `decode` reads back as transparent forever after — no compile
  error, no failing test, the player's paint job already gone by the time
  anyone would look. This is why the dev sprite editor's wider palette is
  `SPRITE_PALETTE`, a second constant that never enters `PlayerIcon`'s
  codec at all, rather than a longer `ICON_PALETTE` — merging the two back
  into one is the DRY move this seam exists to refuse. Full argument in
  `docs/seams.md`, "`ICON_PALETTE` is fixed at fifteen entries by the save
  format, and `SPRITE_PALETTE` is why it stays that way".
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
  nemesis mark and boss mark the table's cyan and magenta (the spawn ring
  this used to name is gone). **The Excavation plan changed colour rather
  than moving**: its washes sat 0.11 from
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
  `map_pane`'s bottom border carries nothing, and the compass tried it for
  one release before moving inside the pane (below). The filter header is the
  body's first row again, at `m.small()` and **above** the refusal, and it
  costs a row through its own `LOG_FILTER_ROWS` — `LOG_TEXT_ROWS` stays the
  *message* count, or SPACE grows the pane by five rows instead of four.
  `base.rs`'s `log_capacity` subtracts that same constant, never a second
  literal.

- **A pane whose border carries a strip starts its body at
  `layout::strip_inset`, not at `m.inset`.** The quad `border_strip` centres
  on its line reaches `strip_clearance` *inward* as well as outward, and the
  strips are painted after the body — so at 1280x720 the vitals hung over
  `[pane.y, pane.y+9]` and took 4.32px off the top of the 13px filter row
  beneath them. `m.inset` is measured from the first pixel the body owns, and
  on such a border that pixel is the bottom of the quad, so the two **add**;
  the larger simply winning is the obvious form and still leaves ~2px of the
  ascenders covered, because a row's baseline sits only `m.font_size/2` below
  the body's top while its ink rises a full ascent above that baseline. **The
  pane must buy the height** — `log_h` spends `strip_inset * 2.0` — or
  `LOG_TEXT_ROWS` silently stops meaning four, which is the mutation
  `the_pane_draws_four_message_rows_collapsed_and_eight_expanded` exists for.
  `base.rs`'s `log_capacity` came into step for free and is asserted by that
  same test; against the old height it under-asked by one in both states. The
  keybar's end was the same fault latent — the floor sat 6.67px inside its
  quad and nothing was clipped only because the row lattice never landed
  within an ascent of it. **The general form is the deliverable**:
  `nothing_the_log_pane_paints_covers_text_it_already_drew` walks every text
  the pane draws, in both states, and asserts nothing painted afterwards
  overlaps its **ink** by an area — three releases have now shipped this bug
  because the test named one rectangle instead.

- **The compass is a block drawn *inside* the map pane, and it was a border
  strip for exactly one release.** The trap is that "put a readout on the
  map pane's bottom border" is a one-line change that costs two layout
  changes, both invisible until something is painted over. A strip centres
  its quad *on* its border line, so it reaches `strip_clearance` into the
  pane: the map then has to buy a band it can never draw tiles in, and
  buying it only while a destination is selected re-lays the entire tile
  grid on the keypress that selects one — which at the keyboard reads as
  the camera lurching, not as a strip appearing. Second, `map_pane`'s
  bottom border and `log_pane`'s top border face each other across
  `pane_gap`, and the vitals already reach *up* into it, so a strip
  reaching *down* needs `pane_gap` to hold two clearances instead of one;
  at one they overlap exactly and the log pane's fill — painted later —
  cuts the compass in half. **A block inside the pane costs none of that**:
  it overlays tiles that are still drawn, so `regions` never learns the
  compass exists, and `base.rs::the_map_is_the_same_size_whether_or_not_a
  _destination_is_picked` compares the map's background fill between the
  two states to hold it. What the block *does* inherit is the body rule
  above — it starts at `layout::strip_inset` below the pane's top edge,
  because THREAT rides that border and its quad hangs down into the pane.
  A block at `m.inset` is painted straight through the lower half of
  THREAT's glyphs, which is the vitals/filter-header collision again in a
  third place.
