---
paths:
  - "**/*settlement*"
  - "**/settlements/**"
  - "**/environment*"
  - "**/zone*"
  - "**/world*"
  - "**/outpost*"
  - "**/travel*"
  - "**/pursuit*"
  - "**/contracts*"
  - "crates/app-core/src/app/**"
---

# Load-bearing seams: The ground

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **`Game::terrain_at` is the one door onto what terrain does to you, and
  `Game::environment_biome_at` is the single definition of its gates (zone
  1, `Biome::Platform`).** Two copies of that check is how neutral zone 1
  lapses.
- **`EnvironmentEffect`'s fold is additive on every term except the ambush
  multiplier, which multiplies.** A reader that takes only the attrition
  terms compiles clean and silently drops drag and the multiplier.
- **Which `StaticEvent` is live is derived from `(seed, zone, biome,
  epoch)` and never stored.** No save field, no `SAVE_FORMAT_VERSION`
  bump, and no `resources::GameRng` draw.
- **A breach raises a tier now, and does not rebuild the world.**
  `Game::enter_next_zone` mints no new map, moves nobody and spends no
  currency — it raises `ZoneLevel`, clears `StackMemory` and
  `PopulatedChunks`, and re-stocks ground already walked at the new tier.
- **A breach's re-tier is two calls and either alone is inert** —
  `enter_next_zone` clears `PopulatedChunks` *and* calls
  `Game::clear_local_wild`, which keeps a `NestGuardian` and a `Nemesis`.
- **Where a settlement stands is derived off `(world seed, region)`, never
  stored.** `settlements::placement::settlement_at` is `rock::RockDb::
  kind_at`'s rule for the zone surface; `resources::Settlements` records
  the resolved tile and def once a town is found, never the candidate cell
  the derivation answered with.
- **A settlement is entered by walking into it, and the footprint admits
  nobody.** The bump is the fourth arm of `move_player`'s ladder; it queues
  `resources::PendingVisit` and leaves the player's `Position` unchanged,
  and `Game::take_settlement_visit` is a drain, not a getter, or the screen
  it opens would reopen on the next keypress.
- **A settlement's footprint is a square derived from kind and vitality on
  every read, `Game::sync_settlement_footprint` is its one writer, and the
  centre cell's `SettlementCentre` is the entity a tether names.**
- **A settlement's buyback keys through a minted `"settlement/<id>"`
  string, not a widened `ShelfKey`.** `StructureId` is a bare `String`, so
  the mint plus its tile fits with no type change and no save bump.
- **A settlement's `Temperament` prices both directions off a neutral
  middle, and Mercantile is not their average.** It competes on the buy
  side and takes its margin on the sell side instead.
- **A town's opinion is written through one door, `Game::adjust_standing`,
  and the band under it is derived on every read.** `resources::Standings`
  holds the signed number; `relations::band` says what it means, and the
  door holds the clamp and the only-on-a-crossing announcement.
- **A consequence of standing is a named query on the band, never a table
  of effects** — `Standing::refuses_service`, exhaustive, `perks.rs`'s seam.
- **A town refusing service answers with a *closed* view, never `None`**,
  and the gate is applied again at `Game::commit_settlement_basket` because
  only a commit can spend.
- **`Relation::trade_credits` is a remainder, not a total**, or trade
  volume becomes a per-basket rounding rule.
- **A contract is delivered where it was signed, and
  `ActiveContract::issuer` is the whole vocabulary** — `None` is the run's
  own Broker, `Some(key)` the town that posted it.
- **A town's garrison is one term in `Game::total_raid_defense`, and the
  clamp is on the settlement half alone** — `SETTLEMENT_GARRISON_MAX`, held
  strictly below `RAID_DAMAGE`.
- **An aid radius is a fraction of `placement::REGION_TILES`, never a flat
  number** — both flat ones were dead, and a dead radius reads at the
  keyboard exactly like a weak one.
- **A gifted program's species is derived from `(world seed, region, gifts
  taken)`; choosing it spends no `GameRng` draw and adopting it spends what
  every adoption does.**
- **A relay landing is `Game::free_tile_outside` — band `radius + 1`, the
  displacement search's own filter — and queues the visit cue only if it
  lands in reach.**
- **A relay trip's tick loop breaks on a fight, and both travel keys owe
  `after_world_action`** — the charge is at most the quote, never equal to it
  on an interrupted trip.
- **The town page's aid rows are engine sentences, `AID_LINES` the census** —
  the width gate lives in gui and cannot build a `Game` to ask.
- **Every aid row is a *call* to the door that honours it, reach included** —
  `Game::town_garrisons` is the garrison's shared half, and the two verbs are
  gated on `settlement_reach` exactly as their doors are.
- **A town's board is `Game::board_defs` with four things changed** — the
  reach that gates it, the seed it draws with, the slot count
  (`Standing::job_slots`) and which tier goes first (`Specialty`, a ranking
  and never a filter).
- **A Hostile town fields a patrol, `components::TownPatrol` is the second
  tether, and `pursuit_tick` sizes its one shared field off the *maximum* of
  the two leashes.**
- **A patrol is provoked by proximity and stood down by the band re-read
  every tick, and only the provocation half carries the surface guard** —
  `Game::patrol_aggro_tick`.
- **Fielding one is a roll with a mean rather than a countdown, and its
  range is measured to the *party* where `raiding_towns` measures to the
  anchor.**
- **Killing a patrol member charges that town alone by key, never
  `credit_nearby_settlements`, and `SETTLEMENT_PATROL_KILL_STANDING` is
  bounded by what clearing one nest pays.**
- **A patrol's mark is the tile's bottom-right corner and spends none of the
  identity, danger or rarity channels**, `EntityView::patrol` carrying the
  town's name rather than a flag.
- **A patrol's tether saves by the town's tile and is resolved *after*
  `restore_settlements`**, `pending_cronjobs`' deferral.
- **An outpost is a `resources::Outposts` record with no entity, never a
  `Structure`** — `resources::Settlements`' own precedent one tile over.
- **`Visit` is the second visit extension point after `Settlement`, and
  `resources::PendingVisit` widened rather than gaining a second field.**
- **Walking is spent by the clock: `update_realtime` owns the step, and
  `handle_key` never ticks on an unpaused map arrow** — a paused arrow is
  still turn-based, and drag ground is paid as idle clock ticks
  (`drag_ticks_owed`) rather than spent inline.
