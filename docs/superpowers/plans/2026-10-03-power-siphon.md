# Power Siphon — implementation plan

Spec (source of truth): `docs/superpowers/specs/2026-10-03-power-siphon-design.md`.
Branch `power-siphon`. Phases are sequential, one subagent dispatch each (sonnet; opus only for
the final review). Every phase: tests first (red, then green), then
`cargo test -p feral-processes-engine <filter>`, `cargo clippy --workspace --all-targets`,
`cargo fmt`, one commit. **No push. No CHANGELOG, no version bump** (landing does that).
Mutation-check each new test against its fix once the fix is committed.
Read first: `.claude/rules/seams-base.md`, `seams-memories.md`, `content-schema.md`
(and `seams-screens.md` for phase 5). Anchors below were verified against the tree at
commit `79c36246`; line numbers drift, grep the symbol.

## Decisions the spec left to the plan

| Item | Decision | Evidence |
|---|---|---|
| Glyph | `'G'` | Glyphs in use across `assets/structures`: `+ % ! M # V Y C W E ^ D K L $ y r X = T U * S A I B R / & z Q b O P s`. `G`, `H`, `J`, `N`, `F`, `Z` are free; `G` reads as "grid". |
| Interact key | `P` (UPPERCASE), at a siphon orthogonally adjacent to the party in base space | Map-screen uppercase keys already bound in `app-core/src/app/playing.rs`: `D`(219) `L`(328) `N`(338) `F`(537, rig) `Z`(752, hidden). `T`, `W` are hidden keys (`crates/engine/EASTER_EGGS.md`). `P` is unbound on the map (outposts/level_up use it only inside their own modes). Shape copies `F`/`open_rig_tool` and `Game::adjacent_teardown_rigs` (`game/extraction.rs:1018`), not the `b` group menu that `Study a program` uses. |
| `power_regen` gating | **No.** `is_fuelled` is untouched; occupancy gates `supply` only | A siphon has no `power_regen`, and `power_regen_system` (`systems.rs:2197`) skips defs without it before it asks `is_fuelled`. |
| Save bump | **No `SAVE_FORMAT_VERSION` bump** (stays 34) | The save is field-named RON (`save.rs:1973` `ron::ser::to_string_pretty`; bincode died at 0.8.0, `save.rs:1846`). `save.rs:638-700` states the rule for each sibling additive tether (`study_station`, `outpost`): `#[serde(default)]` is enough, "no bump". The `serde(default)` caveat in the docs applies to the pre-0.8 positional format only. |
| `SIPHON_GRUDGE_PERIOD` | `500` ticks (2x `MEMORY_POSTING_PERIOD` = 250, `tuning.rs:4585`) | "Builds over a long hold": a stretch memory, one strike per period. |
| `siphoned` memory | `valence: -6.0`, `strike_cap: 9`, `half_life: 2500`, `subject: BaseTile`, `mood` default | See derivation below. |
| `SIPHON_RELEASE_INTEGRITY_LOSS` | `0.75` (f32 fraction of `max_hp`), new HP floored at 1 | Spec section 5. |

### Memory numbers (derivation)

Morale ladder (`tuning.rs`): `MORALE_SULKS_AT = -8` (4896), `MORALE_DOWNS_TOOLS_AT = -50` (4929 area),
`MORALE_RECOVERED_AT = -6` (4929), `MORALE_LASHES_OUT_AT = -75`. Disposition scale
`DISPOSITION_MEMORY_SWING = 0.40` (`tuning.rs:4875`), so Steady x1.0, Abrasive x1.4 (strongest), Amiable x0.6.
`unslotted` (-22 x 4 = -88) is the precedent for a stretch grudge that crosses the line on its own.

- Capped grudge = -6 x 9 = **-54**. Steady crosses `MORALE_SULKS_AT` after 2 strikes (-12) and
  `MORALE_DOWNS_TOOLS_AT` at the cap (-54): "builds to low mood", one strike alone (-6) does not sulk.
  Amiable at the cap -32.4 (sulks, never downs tools); Abrasive -75.6 (just past lashing out).
  It therefore crosses the tantrum line on its own, which is what justifies the by-name exemption.
- Full cap takes 9 x 500 = 4500 ticks held. While held, every strike resets the clock, so decay is
  irrelevant to the hold; `half_life` only sets the way back.
- Way back: from -54 to `MORALE_RECOVERED_AT` (-6) is log2(9) = 3.17 half-lives, about 7900 ticks at 2500.
  Longer than `unslotted` (2000) because this one is the player's deliberate price.
- Worst single grudge excluding `siphoned` stays what it is; `siphoned` is exempted by name from
  `worst_single_grudge` (`tests/disposition.rs:310`) next to `unslotted`.

## Spec conflicts / gaps found

1. **Spec 7 says "the interact key"; there is no per-structure key today.** Study uses the base group menu.
   Plan follows the spec with a rig-style adjacent key (`P`) and a new `Mode::Siphon`.
2. **Destruction is a second path.** `remove_structure` (`building.rs:1349`) is not the only despawn:
   `damage_structure` (`upkeep.rs:1104`) despawns a structure a raid destroys. Without a release there a
   program is held by a dangling `Entity` forever. Plan releases (with the full release cost) on both, as
   `release_study_station` already is on both (`building.rs:1346`, `upkeep.rs:1102`). Spec only names deconstruct.
3. **Exemption wording.** The disposition rule being exempted is the "one memory may not down tools / lash
   out" rule (`tests/disposition.rs:336, 895`), not the sulk rule; the numbers above are chosen to need it.

## Phase 1 — data, `siphons` field, ledger supply

Files:
- `crates/engine/src/structures.rs` (beside `studies`, ~659): `#[serde(default)] pub siphons: bool` with a doc comment (why: data decides which structure holds a program).
- `assets/structures/power_siphon.ron`: spec text, `glyph: 'G'`, `power_supply: 4`, `siphons: true`, no `max_deployed`, description ends "Walk up to it and press P to load or release." (copy the shape of `line_driver.ron`).
- `assets/structures/README.md`: document `siphons` (final phase may polish; write the row now).
- `crates/engine/src/components.rs` (next to `UnderStudy`, ~2211): `#[derive(Component, Clone, Copy, Debug)] pub struct Siphoned { pub siphon: Entity }` with doc: held program, pointing at its siphon, supply is counted off this.
- `crates/engine/src/game/base/power.rs` `ledger` (~140-155): before the entity loop collect `occupied: HashSet<Entity>` from `world.iter_entities()` filtered on `Siphoned`; in the loop `if is_fuelled(..) && (!def.siphons || occupied.contains(&entity_ref.id()))`. Update `ledger`'s and `is_fuelled`'s docs (the one-predicate sentence stays true: `is_fuelled` is unchanged).

Tests (new file `crates/engine/src/tests/siphon.rs`, `mod siphon;` in `tests/mod.rs` ~82; reuse `spawn_structure_at` `support.rs:1092`, `spawn_tamed` 1987, `place_home` 1661; pattern from `tests/power.rs`):
- `an_empty_siphon_supplies_nothing_and_draws_nothing`: ledger supply equals the Home-only baseline.
- `an_occupied_siphon_adds_its_authored_supply`: insert `Siphoned` by hand; supply rises by `StructureDef::power_supply` read from the db (not a literal 4), and `Game::base_power` agrees (second caller).
- `releasing_the_marker_drops_the_supply_again`: remove `Siphoned`, back to baseline.
- `a_siphon_with_a_dangling_marker_counts_nothing_extra`: marker pointing at a different entity does not light the siphon.
- Census: shipped `power_siphon` parses, `siphons` true, `power_supply > 0`, no `power_upkeep`, no `work` (extend the existing structure census in `tests/assets.rs` if a matching table exists, else assert here).

Gate: `cargo test -p feral-processes-engine siphon power assets`. Commit "Power Siphon: data and ledger supply".

## Phase 2 — role, siphon/release API, every `UnderStudy` site ruled

New `crates/engine/src/game/base/siphon.rs` (`pub(crate) mod siphon;` in `game/base/mod.rs` near `study`, line 31), modelled on `study.rs:233-360`:
- `pub fn siphon_program(&mut self, program: Entity, siphon: Entity) -> Result<(), String>`. Refusals, **all before any write**, in this order, each with its own test: game over or `has_active_battle` ("Can't do that right now."); not `Tamed` or owner != player; `program_role != Some(Staff)` (partied, wielded, sortie, outpost, studying, already held: one sentence, "bring it home first"); `siphon` has no `Structure` or its def has `siphons == false`; siphon already occupied (any `Siphoned` pointing at it); player not in base (`self.require_base()?`, as `remove_structure`). Write: `insert(Siphoned { siphon })`, `.remove::<Task>().remove::<Carrying>()` (copy `pin_subject`'s reason: a posted program is `Staff`; `return_carried_program` first as `building.rs:~1313` does so a carried kill is put back). Log line.
- `pub fn release_siphoned(&mut self, program: Entity) -> Result<(), String>`: refuses game over/battle and "not held"; removes `Siphoned`; applies the cost via a pure fn `siphon_release_hp(hp: i32, max_hp: i32) -> i32` (`max(1, hp - (max_hp as f32 * SIPHON_RELEASE_INTEGRITY_LOSS) as i32)`), called by the API and asserted directly. Log "comes out hurt".
- `pub fn siphon_holder(&self, siphon: Entity) -> Option<Entity>`, `pub fn adjacent_siphons(&self) -> Vec<Entity>` (copy `adjacent_teardown_rigs`, `game/extraction.rs:1018`, filter `def.siphons`), both used by phase 5.
- `pub(crate) fn release_siphon_at(&mut self, structure: Entity)`: `release_study_station`'s shape (`study.rs:479`): finds the holder, releases it **with the cost**, logs "the Power Siphon is gone". Called at `building.rs:1346` area (`remove_structure`, before `despawn`) and `upkeep.rs:1102` area (`damage_structure`).
- `tuning.rs`: `pub const SIPHON_RELEASE_INTEGRITY_LOSS: f32 = 0.75;` with a `const _` assert in `(0, 1)`, documented why (the price is the program; never kills).

Role wiring, `crates/engine/src/game/party.rs`: new `ProgramRole::Siphoned` between `UnderStudy` and `Staff` (doc says why: held in a machine, out of the pool); `roster_rank` Siphoned 5, Staff 6 (113); `RoleMarkers` gains `siphoned: bool` (121); `role_of` returns it after the `under_study` check (158); `Roles` gains `siphoned: Query<(), With<Siphoned>>` and `of` passes it (211-229); `Game::program_role` marker (1057-1064). No other `RoleMarkers`/`role_of` caller exists (grepped).

**`UnderStudy` census, one ruling each** (exhaustive matches are compile errors; the rest compile silently and each gets a named test):

| Site | Ruling |
|---|---|
| `party.rs:181-191` `walks_the_base` | Siphoned does **not** walk or block ground (Sortie-like): the body stays at its stale `Position`, excluded from `base_bodies`. Test it. |
| `party.rs:839` `programs_for_build` filter | Exclude (a spend would despawn a body the siphon still counts). |
| `party.rs:982` `add_companion` | Refuse: "held in a Power Siphon. Release it first." |
| `party.rs:1135` `wield_program` | Refuse, same sentence. |
| `party.rs:1398` `fuse_companions` (both halves) | Refuse. |
| `party.rs:1779` `commit_program` | Return `None` (a snapshot never carries it). |
| `trade.rs:~723` `sell_companion` | Refuse "release it first" (selling would vanish it from the siphon with nothing said). |
| `refactor.rs:~257` `open_kernel_ring`, `routines.rs:~744` extract routine | Refuse; read both for the exact existing `PostedAt` refusal and copy its shape. |
| `trade.rs:617` `detach_from_play` / `bench_or_dissolve` | Read the callers; if a Siphoned program can reach it (raid benching), strip `Siphoned` there like `PostedAt`, else leave it and say why in the test name. |
| `turn.rs:1552-1572` rest-repair exhaustive match | New arm on the `false` side: a held program is not beside the player, gets no free heal. |
| `work_orders.rs:961` `walkers` and `:2379` step arm | **Not** widened to Siphoned: it never walks and never gets a `Task`. `base_staff` (`party.rs:1083`) and the scheduler exclude it through `role_of` for free. |
| `inspection.rs:325` `position_is_honest` | No edit: falls through to false (not Staff, no job mark), so the map does not draw it. Assert. |
| `study.rs` `pin_subject` | No edit: `!= Staff` already refuses; assert. `dispatch_sortie`, `post_to_outpost`, `needs_drain_system` also filter `== Staff`; assert one (dispatch) and note the rest. |
| `lifecycle.rs:1247, 2441` | Save side, phase 4. |
| gui `render/party.rs:301` `role_heading`, `launcher/dev_template.rs:583`, `gui/src/render/base.rs:5883-6130` fixtures | gui: phase 5; the other two need no edit (compile tells). |

Tests (`tests/siphon.rs`): one per refusal in `siphon_program` asserting **nothing written** (no `Siphoned`, `Task` kept where it applies, no log) — not owned, not Staff via each of partied / wielded / sortie / outpost / pinned / already-held, non-siphon structure, occupied siphon, in battle, game over. Then: `a_held_program_is_never_handed_a_task_and_never_moves` (N ticks with other staff and work available: no `Task`, `Position` unchanged); `a_held_program_does_not_drain_needs`; `release_takes_75_percent_of_max_hp_and_never_below_one` (incl. a program already at 1 HP and `siphon_release_hp` pure cases); `release_returns_the_program_to_staff_and_it_is_postable_again`; `deconstructing_an_occupied_siphon_releases_its_program_hurt` (via `remove_structure`); `a_raid_destroying_the_siphon_releases_its_program_hurt` (via `damage_structure`); `demolishing_the_home_releases_every_held_program`; one test per refusal added in the census table (add_companion, wield, fuse, sell, kernel ring, routine extract, build spend, commit_program).

Gate: `cargo test -p feral-processes-engine siphon party study building`; clippy; fmt. Commit "Power Siphon: holding, release, role".

## Phase 3 — the grudge

- `assets/memories/siphoned.ron`: id `siphoned`, name "Held in a Siphon", blurb "Pinned in the wall and left to run the base." (measured by the census: keep the blurb's length near `frayed_here`'s), `valence: -6.0`, `half_life: 2500`, `subject: BaseTile`, `strike_cap: 9`. A header comment with the derivation above (comment style of `unslotted.ron`).
- `tuning.rs`: `pub const SIPHON_GRUDGE_PERIOD: u64 = 500;` documented as a stretch memory's write period and why not `MEMORY_POSTING_PERIOD`.
- `crates/engine/src/game/memories.rs`: `pub(crate) fn note_siphoned(&mut self)` beside `note_unslotted` (346): period gate on `GameClock::tick % SIPHON_GRUDGE_PERIOD`; collect every `(program, siphon)` first (`remember` takes `&mut self`), then `remember(program, "siphoned", MemorySubject::BaseTile { x, y })` at the siphon's `Position` (the 250-line `note_postings` shows the BaseTile shape). Call it from `game/turn.rs` right after `note_unslotted()` (~348) with a one-line comment.
- `crates/engine/src/tests/assets.rs` `MEMORY_TRIGGERS` (~2960): add `("siphoned", K::BaseTile)` with the `Game::note_siphoned` comment.
- `crates/engine/src/tests/disposition.rs:310` `worst_single_grudge`: exempt `siphoned` beside `unslotted` (`matches!(id, "unslotted" | "siphoned")`), and extend the doc paragraph with the reason (preventable: release it).
- `assets/memories/README.md`: add the `siphoned` row to the shipped-kinds table (~147) and rewrite the "`unslotted` is the one kind that crosses the tantrum line on its own" paragraph (198) to name both and both exemptions.

Tests (`tests/siphon.rs`, memory-reading helper `entries` as in `tests/tantrums.rs`):
- `a_held_program_remembers_the_siphon_on_the_grudge_period_only`: one strike per `SIPHON_GRUDGE_PERIOD`, none off-period, none for an empty siphon.
- `after_a_capped_hold_morale_is_past_sulks_and_downs_tools_for_a_steady_program`: hold `SIPHON_GRUDGE_PERIOD * strike_cap` ticks, set `Disposition::Steady`, assert `game.morale(p) <= MORALE_SULKS_AT` and `<= MORALE_DOWNS_TOOLS_AT`; also derive the bound from `MemoryDb` so a retune moves it.
- `one_period_of_holding_does_not_yet_sulk` (the "builds" half; Steady, 1 strike > `MORALE_SULKS_AT`).
- `after_release_morale_climbs_back_as_the_grudge_decays`: release, advance `4 * half_life` ticks, morale `> MORALE_RECOVERED_AT`; and a Disgruntled marker latches on the released Staff program (the grudge has a visible consequence).
- `the_disposition_exemption_names_siphoned` and the existing disposition tests stay green.
- Census: `siphoned.ron` parses; the memories-page width census (`seams-memories.md`) passes.

Gate: `cargo test -p feral-processes-engine siphon memories disposition tantrums assets`. Commit "Power Siphon: the grudge".

## Phase 4 — save

- `crates/engine/src/save.rs` (after `outpost`, ~675): `#[serde(default)] pub siphon: Option<(i32, i32)>` with the doc style of `study_station`: named by tile, resolved after `restore_structures`, silent drop if the tile resolves to no `siphons` structure, **additive behind `serde(default)`, no `SAVE_FORMAT_VERSION` bump (save is RON)**. Update the `CreatureSave` literal at `save.rs:~2335` (`siphon: None`) and any other literal the compiler finds (`rg "study_station: None"`).
- `crates/engine/src/game/lifecycle.rs`: writer next to `study_station` (2441-2447): `Siphoned.siphon` to its `Position`; `CreatureRestore` gains `pending_siphon: Vec<(Entity, (i32,i32))>` (217, 268, 1733); the `if let Some(slot)/else if` chain at 2266-2284 gains `else if let Some(tile) = c.siphon` **after `study_station` and `outpost`, before `cronjob`** (a held program cannot also be posted; precedence comment as the siblings); a new `attach_siphoned(pending, &structure_positions)` beside `attach_pinned_subjects` (call at ~1773, body modelled on `study.rs` attach, lifecycle.rs:1240-1250): resolve the tile to a `siphons` structure, insert `Siphoned`, drop silently otherwise, and drop a second claimant on one siphon.
- `crates/engine/src/game/party.rs:1860-1927` (snapshot restore destructures `CreatureRestore` exhaustively): add `pending_siphon` and `drop(pending_siphon)` with the same "revalidated by hand" comment (the compiler forces this).
- `crates/engine/src/tests/save_roundtrip.rs` (pattern: 1060-1110 for `UnderStudy`):
  - `a_siphoned_program_stays_held_through_save_and_load`: save, load, `program_role == Siphoned`, supply identical to before, `Siphoned.siphon` resolves to the same tile; the grudge memory survives too.
  - `an_old_save_with_no_siphon_key_loads` (RON text without the key, `ron-round-trip-cannot-catch-a-skipped-field`: assert the load path, not only the round trip).
  - `a_save_naming_a_tile_with_no_siphon_drops_the_hold_silently` (program loads as Staff, no panic).
  - `a_siphon_naming_two_programs_holds_only_one` (hand-edited save).
  - Snapshot (`commit_program`) never carries a held program: covered by the phase 2 test; assert the snapshot restore path compiles/drops.
- No `dev-saves/` template needed; capture one only if phase 5 wants a screenshot state: `cargo run --bin savetool -- capture saves/save.bin siphon` (only if useful, and then name it in `dev-saves/README.md`).

Gate: `cargo test -p feral-processes-engine save siphon`; clippy; fmt. Commit "Power Siphon: save".

## Phase 5 — app-core and gui

app-core (`crates/app-core`):
- `src/lib.rs`: `Mode::Siphon` beside `PinSubject` (~1638) with a doc comment; add it to the "never layers over a fight" arm (~2304, with `RigTool`) and to `Mode::ALL`-style lists (`all-modes-length-is-a-semantic-merge-conflict`: grep `PinSubject` for every list, count in `all_modes` tests).
- `src/app/siphon.rs` (new, `mod siphon;` in `app/mod.rs`; modelled on `app/rig_tool.rs:39` `open_rig_tool` and `app/building.rs:443` `handle_pin_subject_key`): `open_siphon(&mut self) -> bool` uses `game.adjacent_siphons()`; none: `self.refuse("There is no Power Siphon here.")`, return false (the `after_world_action` rule in `open_rig_tool`'s doc). Empty siphon: picker over `game.base_staff()`. Occupied: one row "Release", whose confirm text says it comes out hurt. `handle_siphon_key`: Esc closes; lowercase row selectors only (`lowercase-letters-are-row-selectors`; the action key `P` is UPPERCASE and only on the map). Dispatch in `app/input.rs:~250`. State: the siphon `Entity` in a field like `rig_tool` (lib.rs:~2930).
- `src/app/playing.rs` near `GameKey::Char('F') => self.open_rig_tool()` (537, base block only): `GameKey::Char('P') => self.open_siphon()`. Comment why `P` (the free-key evidence above).
- `assets/help/20-controls.md` line ~38 area: add `- P — load a staff program into the Power Siphon beside you, or release the one in it.` (the Controls page is the only key list; check `no_shipped_help_page_names_a_hidden_key` and the help README line grammar).

gui (`crates/gui`):
- `src/render/party.rs:301` `role_heading`: `ProgramRole::Siphoned => "Held in Power Siphon"`; the sample roster at ~785 gains a `pet("cc", ProgramRole::Siphoned)` entry if that table is a census.
- `src/render/building.rs` (beside `draw_pin_subject` 710): `draw_siphon(game, selected, refusal, painter, m)`, same `item_row`/`text_row` helpers; the release row quotes the cost: "Release {name} (it comes out at 25% Integrity or less)" computed from `siphon_release_hp`'s inputs, not a typed 25. Wire at `render/mod.rs:1125` and the mode list at `render/mod.rs:1633`.
- Nothing in `crates/gui` touches `World` (architectural rule); every read goes through `Game` methods from phase 2.

Tests (`crates/app-core/src/tests/`, pattern `tests/building.rs:1717` region): `p_at_no_siphon_refuses_and_logs`; `p_at_an_empty_siphon_opens_the_picker_and_a_pick_holds_the_program`; `p_at_an_occupied_siphon_offers_release_and_confirming_releases`; `esc_leaves_the_siphon_screen_writing_nothing`; `p_is_unbound_in_the_stack_block` (stack keys unchanged); the `all_modes` census; gui: a headless row-width census for `draw_siphon` rows (`popup-row-width-is-testable-headlessly`) and the roster heading test.

Gate: `cargo test -p feral-processes-app-core`, `cargo test -p feral-processes-gui`, clippy, fmt. Smoke: `cargo run -- --template stack --keys "..." --screenshot out.png` is optional (needs DISPLAY; Read the PNG) — only if a base template exists with a siphon; otherwise say it was not run. Commit "Power Siphon: interaction and roster".

## Phase 6 — final gate

1. `cargo test --workspace` (full gate), `cargo test -p feral-processes-engine balance_sim` (expected unmoved: no combat, species or item tuning changed; report the result either way), `cargo clippy --workspace --all-targets`, `cargo fmt --check`.
2. Docs: `assets/structures/README.md` (`siphons` row: default false, what it needs: a program `Siphoned`, supply counts only while occupied), `assets/memories/README.md` (done in phase 3, re-read it), `docs/superpowers/INDEX.md` entry for this feature per its own convention. **Not** the manual, README, TODO, CHANGELOG.
3. Seam bookkeeping: if a rule moved, `.claude/rules/seams-base.md` gets one sentence ("a held program is `ProgramRole::Siphoned` and supply is counted off the marker, so deleting the marker is the whole release") and the argument goes to the memory graph as `seam:<slug>` plus the `seams` skill, in that order (three writes, `seams` skill documents it).
4. Whole-branch review, **opus**, diff handed over as a file (`git diff origin/main...power-siphon > <scratchpad>/siphon.diff`, never pasted). Review brief: the spec, this plan, the three conflict notes, and the rule list "every `UnderStudy` site has a ruling and a test"; mutation-check each test in `tests/siphon.rs` against its fix; check the dangling-marker paths (deconstruct, raid destruction, Home demolition, save of a held program whose siphon moved). A review's fix needs its own review.
5. Do not tag, bump, push or merge; landing is a separate step.
