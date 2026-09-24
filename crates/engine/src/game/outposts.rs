//! Founding an outpost — `crate::outposts` holds the record and the pure
//! def/growth math; this is `&mut Game` work, `game/route.rs`'s split.

use bevy_ecs::prelude::Entity;
use rand::RngExt;

use crate::Game;
use crate::ProgramRole;
use crate::base_ledger::LootSource;
use crate::components::{
    Carrying, CarryingProgram, Creature, Downed, Experience, Inventory, Perks, Position, PostedAt,
    ProgramId, Tamed, Task,
};
use crate::game::base::transfer::{Moved, TransferBasket};
use crate::outposts::{Outpost, OutpostDb};
use crate::perks::Perk;
use crate::resources::{GameRng, Outposts};
use crate::species::SpeciesDb;
use crate::systems::mining_success_chance;
use crate::tuning::{
    DEFAULT_BASE_INT, MAX_OUTPOSTS, OUTPOST_CREW_CAP, OUTPOST_CYCLE_TICKS, OUTPOST_DECAY_PER_TICK,
    OUTPOST_GROWTH_PER_CREW, OUTPOST_MAX_INTEGRITY, OUTPOST_MIN_ANCHOR_DISTANCE,
    OUTPOST_MIN_SPACING, OUTPOST_STOCK_CAP, OUTPOST_TIER_CREW,
};
use crate::views::{OutpostCrewRow, OutpostMark, OutpostReport, OutpostYieldRow, TransferRow};
use crate::world::{Biome, WorldMap};

impl Game {
    /// Founds an outpost at `tile` — design spec §9's refusal ladder.
    /// `Game::use_item` is the one caller, and does not take a unit out of
    /// `Inventory` until this returns `Ok`.
    pub fn found_outpost(&mut self, tile: (i32, i32)) -> Result<(), String> {
        self.require_surface()?;
        if self.world.resource::<OutpostDb>().def().is_none() {
            return Err("No outpost design is known.".into());
        }
        let (x, y) = tile;
        let map_tile = self.world.resource_mut::<WorldMap>().tile(x, y);
        if !map_tile.walkable {
            return Err("Nothing would hold an outpost there.".into());
        }
        if map_tile.biome == Biome::Platform {
            return Err("The base's own ground holds no outpost.".into());
        }
        let biome = map_tile.biome;
        if let Some((ax, ay)) = self.anchor_position()
            && (x - ax).abs().max((y - ay).abs()) < OUTPOST_MIN_ANCHOR_DISTANCE
        {
            return Err("That's still within reach of the base. Walk further out.".into());
        }
        // The Stack link, nest, settlement and trap checks reuse the
        // existing occupancy queries `place_trap` already draws on, rather
        // than restating what "something stands here" means a fifth time.
        // The trap check matters beyond "something stands there": `move_
        // player`'s outpost-bump arm comes before its trap arm, so a trap
        // under a founded outpost would be permanently unreachable.
        if self.find_nest_at(x, y).is_some()
            || self.find_surface_link_at(x, y).is_some()
            || self.find_settlement_at(x, y).is_some()
            || self.find_trap_at(x, y).is_some()
        {
            return Err("Something already stands there.".into());
        }
        if self
            .world
            .resource::<Outposts>()
            .0
            .keys()
            .any(|&(ox, oy)| (x - ox).abs().max((y - oy).abs()) < OUTPOST_MIN_SPACING)
        {
            return Err("Another outpost stands too close to build here.".into());
        }
        if self.world.resource::<Outposts>().0.len() >= MAX_OUTPOSTS {
            return Err(format!(
                "You already have {MAX_OUTPOSTS} outposts standing. That's the limit."
            ));
        }
        self.world
            .resource_mut::<Outposts>()
            .0
            .insert(tile, Outpost::new(biome, OUTPOST_MAX_INTEGRITY));
        Ok(())
    }

    /// Every program posted at the outpost standing at `tile`, sorted by
    /// `ProgramId` — `assembler_system`'s reason: bevy's query iteration
    /// order is not stable, and `Game::run_outposts`'s crew roll has to
    /// visit the same order every run for a seeded test to pin it.
    pub fn outpost_crew(&mut self, tile: (i32, i32)) -> Vec<Entity> {
        let mut crew: Vec<(u32, Entity)> = self
            .world
            .query::<(Entity, &PostedAt, &ProgramId)>()
            .iter(&self.world)
            .filter(|(_, posted, _)| posted.0 == tile)
            .map(|(entity, _, id)| (id.0, entity))
            .collect();
        crew.sort_by_key(|&(id, _)| id);
        crew.into_iter().map(|(_, entity)| entity).collect()
    }

    /// Posts `creature` at the outpost standing at `tile` — design spec §6.
    /// Every refusal lands before anything changes.
    pub fn post_to_outpost(&mut self, tile: (i32, i32), creature: Entity) -> Result<(), String> {
        let player = self.player_entity();
        let owner = self
            .world
            .get::<Tamed>(creature)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != player {
            return Err("You don't control that program.".into());
        }
        // One check for "in the party", "wielded", "away on a sortie",
        // "posted at another outpost" and "pinned in a pen" — `role_of`'s
        // whole point, `pin_subject`'s own phrasing one role over.
        if self.program_role(creature) != Some(ProgramRole::Staff) {
            return Err(
                "Only a program on the base staff can be posted at an outpost — bring it home \
                 first."
                    .into(),
            );
        }
        if self.world.get::<Downed>(creature).is_some() {
            return Err("That program is down and needs a Repair Bay first.".into());
        }
        // Refused rather than freed: unlike a pin, posting has nowhere for
        // the load to land, and `hauling`'s own rule is that a body holding
        // `Carrying` is never freed for exactly this reason. `CarryingProgram`
        // is `is_on_shift`'s pairing — a carried kill is a program the player
        // cannot get back, so it is refused for the same reason.
        if self.world.get::<Carrying>(creature).is_some() {
            return Err("That program is carrying a load — it has to deliver that first.".into());
        }
        if self.world.get::<CarryingProgram>(creature).is_some() {
            return Err(
                "That program is carrying a downed program — it has to deliver that first.".into(),
            );
        }
        if !self.world.resource::<Outposts>().0.contains_key(&tile) {
            return Err("No outpost stands there.".into());
        }
        if !self.standing_at_outpost(tile) {
            return Err("You need to be standing at the outpost to post someone there.".into());
        }
        if self.outpost_crew(tile).len() >= OUTPOST_CREW_CAP {
            return Err(format!(
                "That outpost's crew is full ({OUTPOST_CREW_CAP}/{OUTPOST_CREW_CAP})."
            ));
        }
        // `pin_subject`'s own order: the marker first, the stale job after
        // — a posted program is `Staff` until this insert lands, so
        // `role_of` never sees the `Task` on its own and the job has to be
        // freed here or it never is.
        self.world
            .entity_mut(creature)
            .insert(PostedAt(tile))
            .remove::<Task>();
        let name = self.creature_label(creature);
        self.log(format!("{name} is posted at the outpost."));
        Ok(())
    }

    /// Returns `creature` from outpost duty to ordinary base staff — design
    /// spec §6.
    ///
    /// **`Position` is deliberately left untouched.** A recalled program
    /// keeps whatever base-space `Position` it held before being posted, and
    /// `drift_idle_staff` walks it from there on its very next beat: if that
    /// tile is still laid floor, `wander_step` continues from it directly.
    /// `game::base::work_orders::entry_tile` — a ring around the Home — is
    /// only the fallback for the *off-floor* case, the same arrival path a
    /// program downed in the Stack takes; an ordinary recall does not go
    /// through it at all.
    pub fn recall_from_outpost(&mut self, creature: Entity) -> Result<(), String> {
        if self.world.get::<PostedAt>(creature).is_none() {
            return Err("That program isn't posted at an outpost.".into());
        }
        self.world.entity_mut(creature).remove::<PostedAt>();
        let name = self.creature_label(creature);
        self.log(format!("{name} is recalled to base staff."));
        Ok(())
    }

    /// Restores the outpost at `tile` to full integrity — design spec §8.
    /// Pays `OutpostDef::repair_cost` from the pack, every line refused
    /// before a unit moves — a research bill's own rule (`Game::
    /// select_research`'s reason one door over).
    pub fn repair_outpost(&mut self, tile: (i32, i32)) -> Result<(), String> {
        if !self.world.resource::<Outposts>().0.contains_key(&tile) {
            return Err("No outpost stands there.".into());
        }
        if !self.standing_at_outpost(tile) {
            return Err("You need to be standing at the outpost to repair it.".into());
        }
        if self.world.resource::<Outposts>().0[&tile].integrity == OUTPOST_MAX_INTEGRITY {
            return Err("The outpost is already at full integrity.".into());
        }
        let Some(def) = self.world.resource::<OutpostDb>().def().cloned() else {
            return Err("No outpost design is known.".into());
        };
        let player = self.player_entity();
        for (item, need) in &def.repair_cost {
            let held = self
                .world
                .get::<Inventory>(player)
                .map(|inv| inv.count(item))
                .unwrap_or(0);
            if held < *need {
                return Err(format!(
                    "Not enough {} to repair it: need {need}, have {held}.",
                    self.item_name(item)
                ));
            }
        }
        for (item, need) in &def.repair_cost {
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take(item.clone(), *need);
        }
        self.world
            .resource_mut::<Outposts>()
            .0
            .get_mut(&tile)
            .unwrap()
            .integrity = OUTPOST_MAX_INTEGRITY;
        self.log("The outpost is repaired.".to_string());
        Ok(())
    }

    /// Posts an alert the moment `trend` first differs from what was last
    /// announced for this outpost — design correction 10. Latched on
    /// `Outpost::announced` rather than re-checked every tick against the
    /// board's own collapse rule, which is what keeps a whole `Trend::Stale`
    /// stretch from re-posting on every single tick it holds.
    ///
    /// **Entering `Stale` or `Declining` posts; every other trend (including
    /// leaving one of those two) only updates the latch.** `reason` is
    /// `Trend::reason` itself — the screen's status line and the alert
    /// board cannot disagree about why, `Trend::reason`'s own doc.
    fn announce_outpost_trend(
        &mut self,
        tile: (i32, i32),
        trend: crate::outposts::Trend,
        crew: usize,
        tier: usize,
    ) {
        let already = self
            .world
            .resource::<Outposts>()
            .0
            .get(&tile)
            .and_then(|o| o.announced);
        if already == Some(trend) {
            return;
        }
        {
            let mut outposts = self.world.resource_mut::<Outposts>();
            outposts.0.get_mut(&tile).unwrap().announced = Some(trend);
        }
        let kind = match trend {
            crate::outposts::Trend::Stale => crate::alerts::AlertKind::OutpostStale,
            crate::outposts::Trend::Declining => crate::alerts::AlertKind::OutpostDeclining,
            crate::outposts::Trend::Growing | crate::outposts::Trend::Stable => return,
        };
        let outpost = self.world.resource::<Outposts>().0[&tile].clone();
        let reason = trend.reason(&outpost, crew, tier);
        let subject = format!("outpost@{},{}", tile.0, tile.1);
        let text = format!("Outpost at ({}, {}): {reason}", tile.0, tile.1);
        self.post_alert(kind, subject, text);
    }

    /// Re-seeds `Outpost::announced` to the trend it derives to right now —
    /// design correction 10. The one caller is `Game::load`, right after
    /// `attach_outpost_crew` (trend reads crew count): without this, a
    /// freshly loaded save's `announced: None` would differ from a trend
    /// that was already true when the run was saved, and the very next tick
    /// would post an alert for a state the player already knows about.
    pub(crate) fn reseed_outpost_announcements(&mut self) {
        let tiles: Vec<(i32, i32)> = self
            .world
            .resource::<Outposts>()
            .0
            .keys()
            .copied()
            .collect();
        for tile in tiles {
            let crew = self.outpost_crew(tile).len();
            let mut outposts = self.world.resource_mut::<Outposts>();
            let outpost = outposts.0.get_mut(&tile).unwrap();
            let tier = crate::outposts::tier(outpost, crew);
            let trend = crate::outposts::trend(outpost, crew, tier, OUTPOST_STOCK_CAP);
            outpost.announced = Some(trend);
        }
    }

    /// Whether the player is standing at (or diagonally beside) `tile` —
    /// the shared "at the outpost" reach `post_to_outpost` and
    /// `take_from_outpost` both refuse without.
    fn standing_at_outpost(&self, tile: (i32, i32)) -> bool {
        let player = self.player_entity();
        let Some(pos) = self.world.get::<Position>(player) else {
            return false;
        };
        (pos.x - tile.0).abs().max((pos.y - tile.1).abs()) <= 1
    }

    /// One tick of every outpost — design spec §5, called from `tick_inner`
    /// beside `run_routes`.
    ///
    /// Ticked in `BTreeMap` key `(x, y)` order (`resources::Outposts`' own
    /// order), the traps precedent, and this is the one early return that
    /// makes the whole feature draw no `resources::GameRng` while no
    /// outpost stands.
    pub(crate) fn run_outposts(&mut self) {
        if self.world.resource::<Outposts>().0.is_empty() {
            return;
        }
        let tiles: Vec<(i32, i32)> = self
            .world
            .resource::<Outposts>()
            .0
            .keys()
            .copied()
            .collect();
        for tile in tiles {
            let crew = self.outpost_crew(tile);
            self.tick_one_outpost(tile, &crew);
        }
    }

    /// One outpost's beat: growth first, then — only every
    /// `OUTPOST_CYCLE_TICKS` — a production roll per crew member.
    fn tick_one_outpost(&mut self, tile: (i32, i32), crew: &[Entity]) {
        // Dark: integrity zero, Phase 5's raid-to-zero outcome. Growth and
        // production both stop; the record stays on the map to be repaired.
        if self.world.resource::<Outposts>().0[&tile].integrity == 0 {
            return;
        }
        let crew_count = crew.len();
        // Step 1: `stale_ticks` and the growth delta the current `Trend`
        // buys — both are the record's own state, mutated once up front so
        // the production step below reads a tier already caught up with
        // this tick's growth.
        let (tier_now, trend_now) = {
            let mut outposts = self.world.resource_mut::<Outposts>();
            let outpost = outposts.0.get_mut(&tile).unwrap();
            let stock_total: u32 = outpost.stock.values().sum();
            if stock_total >= OUTPOST_STOCK_CAP {
                outpost.stale_ticks += 1;
            } else {
                outpost.stale_ticks = 0;
            }
            let tier = crate::outposts::tier(outpost, crew_count);
            let trend = crate::outposts::trend(outpost, crew_count, tier, OUTPOST_STOCK_CAP);
            match trend {
                crate::outposts::Trend::Growing => {
                    // Crew counted from the tier-1 floor upward, so the
                    // minimum crew that avoids `Trend::Declining` still
                    // grows at the base rate rather than nothing at all —
                    // `tuning::OUTPOST_GROWTH_PER_CREW`'s own doc.
                    let credited =
                        (crew_count as u32 + 1).saturating_sub(OUTPOST_TIER_CREW[0] as u32);
                    outpost.growth += OUTPOST_GROWTH_PER_CREW * credited;
                }
                crate::outposts::Trend::Declining => {
                    outpost.growth = outpost.growth.saturating_sub(OUTPOST_DECAY_PER_TICK);
                }
                crate::outposts::Trend::Stale | crate::outposts::Trend::Stable => {}
            }
            (tier, trend)
        };
        self.announce_outpost_trend(tile, trend_now, crew_count, tier_now);

        // Step 2: advance toward the next cycle. Nothing below runs — no
        // crew roll, no `GameRng` draw — until the cycle actually elapses,
        // and an unstaffed outpost has nobody to roll regardless.
        let ran_a_cycle = {
            let mut outposts = self.world.resource_mut::<Outposts>();
            let outpost = outposts.0.get_mut(&tile).unwrap();
            outpost.cycle_progress += 1;
            if outpost.cycle_progress >= OUTPOST_CYCLE_TICKS {
                outpost.cycle_progress = 0;
                true
            } else {
                false
            }
        };
        if !ran_a_cycle || crew.is_empty() {
            return;
        }
        let Some(def) = self.world.resource::<OutpostDb>().def().cloned() else {
            return;
        };
        let biome = self.world.resource::<Outposts>().0[&tile].biome;
        let union = crate::outposts::yields(&def, tier_now, biome);
        if union.is_empty() {
            return;
        }
        // The player's own perk, read once outside the loop —
        // `resolve_gather_cycle`'s own reason: the perk belongs to whoever
        // is running the cycle, not to each worker in it.
        let player = self.player_entity();
        let keen_scavenger_level = self
            .world
            .get::<Perks>(player)
            .map(|p| p.level(Perk::KeenScavenger))
            .unwrap_or(0);
        // v1 applies no class-based yield bonus at an outpost — correction
        // 6 of the outposts plan: that bonus lives in `resolve_gather_cycle`
        // and is bound to a node, and morale/need strain are away crew's own
        // two omissions (`Sortie`'s shape), so both terms pass as `0.0`.
        for &member in crew {
            let species_id = self
                .world
                .get::<Creature>(member)
                .map(|c| c.species.clone());
            let base_int = species_id
                .as_deref()
                .and_then(|id| self.world.resource::<SpeciesDb>().get(id))
                .map(|s| s.base_int)
                .unwrap_or(DEFAULT_BASE_INT);
            let level = tier_now as u32 + 1;
            let chance = mining_success_chance(level, keen_scavenger_level, base_int, 0.0, 0.0);
            let ok = self.world.resource_mut::<GameRng>().0.random_bool(chance);
            if !ok {
                continue;
            }
            // One draw to pick the item, and none when the union has a
            // single entry — the outposts plan's own test for this.
            let item = if union.len() == 1 {
                union[0].clone()
            } else {
                let index = self
                    .world
                    .resource_mut::<GameRng>()
                    .0
                    .random_range(0..union.len());
                union[index].clone()
            };
            let landed = {
                let mut outposts = self.world.resource_mut::<Outposts>();
                let outpost = outposts.0.get_mut(&tile).unwrap();
                let stock_total: u32 = outpost.stock.values().sum();
                if stock_total < OUTPOST_STOCK_CAP {
                    *outpost.stock.entry(item.clone()).or_insert(0) += 1;
                    1
                } else {
                    0
                }
            };
            self.report_base(
                crate::base_ledger::Event::Extract {
                    item: item.clone(),
                    rolled: 1,
                    landed,
                    ok: true,
                },
                move |tick, zone, event| {
                    let crate::base_ledger::Event::Extract {
                        item,
                        rolled,
                        landed,
                        ok,
                    } = event
                    else {
                        unreachable!("an outpost cycle emits an Extract event")
                    };
                    crate::telemetry::Record::Extract {
                        tick,
                        zone,
                        machine: tile,
                        kind: "outpost".to_string(),
                        tier: tier_now as u32 + 1,
                        worker_species: species_id.clone(),
                        item: item.0.clone(),
                        rolled: *rolled,
                        landed: *landed,
                        ok: *ok,
                    }
                },
            );
        }
    }

    /// The name a caravan destination row or an in-flight report uses for
    /// the outpost standing at `tile` — `def.name` disambiguated by its
    /// tile, since v1 ships one def for every outpost. Falls back to a
    /// generic label when no outpost def is installed at all, which a
    /// caller only reaches for a route already dispatched to a tile whose
    /// def has since been removed.
    pub fn outpost_destination_name(&self, tile: (i32, i32)) -> String {
        let def_name = self
            .world
            .resource::<OutpostDb>()
            .def()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "Outpost".to_string());
        format!("{def_name} ({}, {})", tile.0, tile.1)
    }

    /// `Mode::OutpostVisit`'s one derivation — design spec §9. `None` when
    /// no outpost stands at `tile`, or the def that founded it has since
    /// been removed from `assets/outposts/`.
    pub fn outpost_report(&mut self, tile: (i32, i32)) -> Option<OutpostReport> {
        let outpost = self.world.resource::<Outposts>().0.get(&tile)?.clone();
        let def = self.world.resource::<OutpostDb>().def()?.clone();
        let crew_entities = self.outpost_crew(tile);
        let crew_count = crew_entities.len();
        let tier = crate::outposts::tier(&outpost, crew_count);
        let trend = crate::outposts::trend(&outpost, crew_count, tier, OUTPOST_STOCK_CAP);
        let reason = trend.reason(&outpost, crew_count, tier);
        let growth_fill = crate::outposts::growth_fill(&outpost, tier);
        let crew = crew_entities
            .into_iter()
            .map(|entity| {
                let name = self.creature_label(entity);
                let species = self
                    .world
                    .get::<Creature>(entity)
                    .map(|c| self.species_name(&c.species).to_string())
                    .unwrap_or_default();
                let level = self
                    .world
                    .get::<Experience>(entity)
                    .map(|e| e.level)
                    .unwrap_or(0);
                OutpostCrewRow {
                    name,
                    species,
                    level,
                }
            })
            .collect();
        let yields = crate::outposts::yields_with_tier(&def, tier, outpost.biome)
            .into_iter()
            .map(|(item, first_tier)| {
                let name = self.item_name(&item).to_string();
                OutpostYieldRow {
                    item,
                    name,
                    tier: first_tier + 1,
                }
            })
            .collect();
        let next_tier_requirement = (tier + 1 < OUTPOST_TIER_CREW.len()).then(|| {
            format!(
                "Tier {} needs {} crew",
                tier + 2,
                OUTPOST_TIER_CREW[tier + 1]
            )
        });
        let stock: u32 = outpost.stock.values().sum();
        Some(OutpostReport {
            tile,
            name: def.name.clone(),
            biome: outpost.biome,
            tier,
            tier_label: crate::outposts::tier_label(tier),
            growth_fill,
            trend,
            reason,
            integrity: outpost.integrity,
            max_integrity: OUTPOST_MAX_INTEGRITY,
            stock,
            stock_cap: OUTPOST_STOCK_CAP,
            route: self.outpost_route_line(tile),
            crew,
            yields,
            next_tier_requirement,
            dark: outpost.integrity == 0,
        })
    }

    /// Every outpost's tile mark for the surface map — design spec §9's
    /// growth bar and tier pips. `views::DigMark`'s counterpart: a record
    /// with no entity has no other way onto `Game::view_entities_at`'s
    /// list, so the map needs a pass of its own to find it at all.
    pub fn outpost_marks(&mut self) -> Vec<OutpostMark> {
        let Some(def) = self.world.resource::<OutpostDb>().def().cloned() else {
            return Vec::new();
        };
        let tiles: Vec<(i32, i32)> = self
            .world
            .resource::<Outposts>()
            .0
            .keys()
            .copied()
            .collect();
        tiles
            .into_iter()
            .filter_map(|tile| {
                let outpost = self.world.resource::<Outposts>().0.get(&tile)?.clone();
                let crew = self.outpost_crew(tile).len();
                let tier = crate::outposts::tier(&outpost, crew);
                let trend = crate::outposts::trend(&outpost, crew, tier, OUTPOST_STOCK_CAP);
                Some(OutpostMark {
                    tile,
                    glyph: def.glyph,
                    tier,
                    fill: crate::outposts::growth_fill(&outpost, tier),
                    trend,
                    dark: outpost.integrity == 0,
                })
            })
            .collect()
    }

    /// Every item sitting in the outpost's stock, as `Mode::Transfer`'s take
    /// side lists it — design correction 11. Take-only, so every row is
    /// `can_put: 0`: nothing may be put back into an outpost by hand.
    pub fn outpost_transfer_offer(&self, tile: (i32, i32)) -> Vec<TransferRow> {
        let Some(outpost) = self.world.resource::<Outposts>().0.get(&tile) else {
            return Vec::new();
        };
        let player = self.player_entity();
        let carried = self.world.get::<Inventory>(player);
        outpost
            .stock
            .iter()
            .filter(|(_, qty)| **qty > 0)
            .map(|(item, qty)| TransferRow {
                item: item.clone(),
                on_shelves: *qty,
                carried: carried.map(|inv| inv.count(item)).unwrap_or(0),
                can_put: 0,
            })
            .collect()
    }

    /// Moves stock out of the outpost at `tile` into the pack — the take
    /// side of `Mode::Transfer`'s one basket, design correction 11's engine
    /// door. Every refusal lands before anything is spent.
    ///
    /// `basket.give` and `.carriers` are silently ignored rather than
    /// refused: `outpost_transfer_offer`'s rows are all `can_put: 0`, so
    /// nothing in app-core can ever populate either — there is no partner
    /// error to invent for an empty list.
    ///
    /// Bounded by the stock actually standing, `TransferBasket`'s own
    /// clamp-not-refuse convention (`take_from_adjacent`), and reported per
    /// unit through `Game::grant_loot` with `LootSource::Outpost` —
    /// correction 5's second, later event, distinct from the cycle's own
    /// `base_ledger::Event::Extract`.
    pub fn take_from_outpost(
        &mut self,
        tile: (i32, i32),
        basket: &TransferBasket,
    ) -> Result<Moved, String> {
        if !self.world.resource::<Outposts>().0.contains_key(&tile) {
            return Err("No outpost stands there.".into());
        }
        if !self.standing_at_outpost(tile) {
            return Err("You need to be standing at the outpost to take its stock.".into());
        }
        let mut moved = Moved::new();
        for (item, want) in &basket.take {
            let have = self
                .world
                .resource::<Outposts>()
                .0
                .get(&tile)
                .and_then(|o| o.stock.get(item))
                .copied()
                .unwrap_or(0);
            let qty = (*want).min(have);
            if qty == 0 {
                continue;
            }
            {
                let mut outposts = self.world.resource_mut::<Outposts>();
                let outpost = outposts.0.get_mut(&tile).unwrap();
                let remaining = outpost.stock.get_mut(item).unwrap();
                *remaining -= qty;
                if *remaining == 0 {
                    outpost.stock.remove(item);
                }
            }
            self.grant_loot(item.clone(), qty, LootSource::Outpost);
            moved.push((item.clone(), qty));
        }
        Ok(moved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DifficultyMode;
    use crate::components::{Inventory, Position};
    use crate::tests::support::test_assets_dir;

    fn game(seed: u32) -> Game {
        Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
    }

    fn kit_id(game: &Game) -> crate::items::ItemId {
        game.world
            .resource::<OutpostDb>()
            .def()
            .expect("assets/outposts/outpost.ron ships a def")
            .kit
            .clone()
    }

    /// Deterministically finds a legal founding tile at least `min_distance`
    /// Chebyshev tiles from the anchor and at least `avoid_dist` from every
    /// tile in `avoid` — walking outward ring by ring rather than trusting a
    /// single hardcoded offset to be walkable, unoccupied ground for every
    /// seed a test picks. Panics if none turns up within a generous search
    /// radius, which would mean the constants really do make the ground
    /// unreachable (`a-gated-consequence-can-be-green-and-unreachable`).
    fn open_tile_at_least(
        game: &mut Game,
        min_distance: i32,
        avoid: &[(i32, i32)],
        avoid_dist: i32,
    ) -> (i32, i32) {
        let (ax, ay) = game.anchor_position().unwrap();
        for radius in min_distance..min_distance + 500 {
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    let (x, y) = (ax + dx, ay + dy);
                    let tile = game.world.resource_mut::<WorldMap>().tile(x, y);
                    if !tile.walkable || tile.biome == Biome::Platform {
                        continue;
                    }
                    if game.find_nest_at(x, y).is_some()
                        || game.find_surface_link_at(x, y).is_some()
                        || game.find_settlement_at(x, y).is_some()
                    {
                        continue;
                    }
                    if avoid
                        .iter()
                        .any(|&(ox, oy)| (x - ox).abs().max((y - oy).abs()) < avoid_dist)
                    {
                        continue;
                    }
                    return (x, y);
                }
            }
        }
        panic!("no open founding tile found within the search radius");
    }

    /// The shipped def loads — every test below relies on it, so a broken
    /// `assets/outposts/outpost.ron` fails loudly here rather than as a
    /// mysterious refusal in every other test.
    #[test]
    fn the_shipped_outpost_def_loads() {
        let game = game(1);
        assert!(game.world.resource::<OutpostDb>().def().is_some());
    }

    #[test]
    fn founding_far_from_the_anchor_on_open_ground_succeeds() {
        let mut game = game(2);
        // `OUTPOST_MIN_ANCHOR_DISTANCE` must be reachable at all — a
        // constant that refused every tile in a real, generated world would
        // be green and useless (`a-gated-consequence-can-be-green-and-
        // unreachable`).
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        game.found_outpost(tile).unwrap();
        assert!(game.world.resource::<Outposts>().0.contains_key(&tile));
    }

    #[test]
    fn founded_outpost_reads_the_biome_at_the_tile() {
        let mut game = game(2);
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        let expected_biome = game
            .world
            .resource_mut::<WorldMap>()
            .tile(tile.0, tile.1)
            .biome;
        game.found_outpost(tile).unwrap();
        let record = &game.world.resource::<Outposts>().0[&tile];
        assert_eq!(record.biome, expected_biome);
        assert_eq!(record.growth, 0);
        assert_eq!(record.integrity, OUTPOST_MAX_INTEGRITY);
        assert!(record.stock.is_empty());
        assert_eq!(record.stale_ticks, 0);
        assert_eq!(record.cycle_progress, 0);
        assert!(record.announced.is_none());
    }

    #[test]
    fn refuses_underground() {
        let mut game = game(3);
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        // `enter_stack` drops the party down wherever they are standing —
        // no real link entity is needed to exercise `require_surface`.
        let player = game.player_entity();
        let pos = *game.world.get::<Position>(player).unwrap();
        game.enter_stack(pos.x, pos.y);
        assert!(game.is_underground());
        assert!(game.found_outpost(tile).is_err());
        // Deleted-fix check: without `require_surface`, this same call
        // would succeed underground, which is exactly the bug this guards.
    }

    #[test]
    fn refuses_when_the_kit_is_still_in_the_pack_on_every_refusal() {
        let mut game = game(4);
        // The player starts standing at the anchor itself, well inside
        // `OUTPOST_MIN_ANCHOR_DISTANCE` — that refusal is enough on its own.
        let id = kit_id(&game);
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(id.clone(), 1);
        // Too close to the anchor — a refusal that leaves the kit untouched.
        game.use_item(&id);
        assert_eq!(
            game.world.get::<Inventory>(player).unwrap().count(&id),
            1,
            "a refused founding must not spend the kit"
        );
    }

    #[test]
    fn using_the_kit_with_none_in_the_pack_refuses_without_founding() {
        let mut game = game(5);
        // Walk the player somewhere `found_outpost` would otherwise accept,
        // so the only thing left to refuse this is the empty pack —
        // founding at the anchor itself would refuse for a different
        // reason and leave this guard untested.
        let target = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        let player = game.player_entity();
        {
            let mut pos = game.world.get_mut::<Position>(player).unwrap();
            pos.x = target.0;
            pos.y = target.1;
        }
        let id = kit_id(&game);
        // The pack starts empty of the kit — nothing to spend.
        game.use_item(&id);
        assert!(game.world.resource::<Outposts>().0.is_empty());
    }

    #[test]
    fn successful_use_spends_one_kit_and_founds_the_outpost() {
        let mut game = game(6);
        let target = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        let id = kit_id(&game);
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(id.clone(), 2);
        // The player starts standing on the anchor itself, which is too
        // close to found on — walk them out first.
        {
            let mut pos = game.world.get_mut::<Position>(player).unwrap();
            pos.x = target.0;
            pos.y = target.1;
        }
        game.use_item(&id);
        assert_eq!(game.world.get::<Inventory>(player).unwrap().count(&id), 1);
        assert!(game.world.resource::<Outposts>().0.contains_key(&target));
    }

    #[test]
    fn refuses_within_min_anchor_distance() {
        let mut game = game(7);
        // The anchor's own tile is guaranteed open ground (the party
        // started there) and is distance 0 from itself — well inside the
        // minimum, with no dependence on this seed's generated terrain
        // elsewhere. Deleted-fix check: without the distance guard this
        // same call succeeds, since nothing else about the tile refuses it.
        let tile = game.anchor_position().unwrap();
        assert!(game.found_outpost(tile).is_err());
    }

    #[test]
    fn refuses_on_platform_biome() {
        let mut game = game(8);
        let (ax, ay) = game.anchor_position().unwrap();
        let tile = (ax + OUTPOST_MIN_ANCHOR_DISTANCE, ay);
        game.world.resource_mut::<WorldMap>().set_override(
            tile.0,
            tile.1,
            crate::world::Tile {
                biome: Biome::Platform,
                walkable: true,
                rock_shade: None,
            },
        );
        assert_eq!(
            game.found_outpost(tile),
            Err("The base's own ground holds no outpost.".to_string())
        );
    }

    #[test]
    fn refuses_when_a_settlement_stands_on_the_tile() {
        let mut game = game(13);
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        let key = crate::settlements::SettlementKey { rx: 0, ry: 0 };
        game.world.spawn((
            crate::components::Settlement { key },
            Position {
                x: tile.0,
                y: tile.1,
            },
        ));
        assert!(game.found_outpost(tile).is_err());
        // Deleted-fix check: without the occupancy guard this same call
        // would succeed on top of the settlement, since nothing else about
        // an ordinary open tile refuses it.
    }

    /// `move_player`'s outpost-bump arm comes before its trap arm — a trap
    /// under a founded outpost would be permanently unreachable, since the
    /// bump always wins the tile first.
    #[test]
    fn refuses_when_a_trap_stands_on_the_tile() {
        let mut game = game(14);
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        game.world.spawn((
            crate::components::Trap {
                item: crate::items::ItemId::from("core_fragment"),
                next_roll: 10,
                caught: None,
            },
            Position {
                x: tile.0,
                y: tile.1,
            },
        ));
        assert!(game.found_outpost(tile).is_err());
        // Deleted-fix check: without the trap occupancy guard this same call
        // would succeed on top of the trap, since nothing else about an
        // ordinary open tile refuses it.
    }

    #[test]
    fn refuses_when_the_outpost_cap_is_reached() {
        let mut game = game(9);
        let mut founded = Vec::new();
        for _ in 0..MAX_OUTPOSTS {
            let tile = open_tile_at_least(
                &mut game,
                OUTPOST_MIN_ANCHOR_DISTANCE,
                &founded,
                OUTPOST_MIN_SPACING,
            );
            game.found_outpost(tile).unwrap();
            founded.push(tile);
        }
        // A tile that clears every other check — reachable, unoccupied and
        // properly spaced — still refuses once the cap is reached.
        let one_more = open_tile_at_least(
            &mut game,
            OUTPOST_MIN_ANCHOR_DISTANCE,
            &founded,
            OUTPOST_MIN_SPACING,
        );
        assert!(game.found_outpost(one_more).is_err());
    }

    #[test]
    fn refuses_within_min_spacing_of_another_outpost() {
        let mut game = game(10);
        let first = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        game.found_outpost(first).unwrap();
        // The same tile again is distance 0 from `first` — guaranteed to
        // trip the spacing refusal alone, since it already passed every
        // other check once.
        assert!(game.found_outpost(first).is_err());
        // Reachability: a tile spaced far enough away must still succeed.
        let far_enough = open_tile_at_least(
            &mut game,
            OUTPOST_MIN_ANCHOR_DISTANCE,
            &[first],
            OUTPOST_MIN_SPACING,
        );
        game.found_outpost(far_enough).unwrap();
    }

    #[test]
    fn refuses_without_a_loaded_outpost_def() {
        let mut game = game(11);
        // Simulates a deleted `assets/outposts/` — see `OutpostDb`'s doc.
        game.world.insert_resource(OutpostDb::default());
        let (ax, ay) = game.anchor_position().unwrap();
        let tile = (ax + OUTPOST_MIN_ANCHOR_DISTANCE, ay);
        assert_eq!(
            game.found_outpost(tile),
            Err("No outpost design is known.".to_string())
        );
    }

    // -------------------------------------------------------------------
    // Crew: `post_to_outpost`, `recall_from_outpost`, `outpost_crew`
    // -------------------------------------------------------------------

    /// A bare base-staff program — `spawn_tamed`'s shape in
    /// `tests::support`, restated because that module's helpers are
    /// `pub(super)` to `crate::tests` and this file's own inline tests are
    /// a sibling module tree, `found_outpost`'s tests' own precedent.
    fn staff(game: &mut Game) -> Entity {
        let species = game.species_defs().into_iter().next().unwrap();
        let parts = game.roster_parts();
        game.world
            .spawn((
                crate::components::Creature {
                    species: species.id.clone(),
                },
                Position { x: 3, y: 3 },
                crate::components::Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 3,
                    mitigation: 1,
                },
                parts,
            ))
            .id()
    }

    /// Founds an outpost far enough from the anchor and stands the player on
    /// its tile, so `post_to_outpost`'s adjacency check passes without a
    /// maze walk. Returns the tile.
    fn founded_outpost_with_player_standing_there(game: &mut Game) -> (i32, i32) {
        let tile = open_tile_at_least(game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        game.found_outpost(tile).unwrap();
        let player = game.player_entity();
        let mut pos = game.world.get_mut::<Position>(player).unwrap();
        pos.x = tile.0;
        pos.y = tile.1;
        tile
    }

    #[test]
    fn posting_a_staff_program_marks_it_outpost_and_frees_its_task() {
        let mut game = game(20);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        let program = staff(&mut game);
        let target = game.world.spawn_empty().id();
        game.world
            .entity_mut(program)
            .insert(crate::components::Task {
                kind: crate::components::TaskKind::GatherResource,
                target,
                progress: 1,
                required: 10,
            });

        game.post_to_outpost(tile, program).unwrap();

        assert_eq!(game.program_role(program), Some(ProgramRole::Outpost));
        assert!(
            game.world.get::<crate::components::Task>(program).is_none(),
            "posting must free a stale job the same way pinning does"
        );
        assert_eq!(game.outpost_crew(tile), vec![program]);
    }

    #[test]
    fn post_to_outpost_refuses_a_program_not_on_staff() {
        let mut game = game(21);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        let program = staff(&mut game);
        // `add_companion` refuses outside base space (`require_base`), so
        // the party join has to happen with the locale swapped in and back
        // out — `tests::support::stand_in_base`'s effect, restated for
        // `staff`'s own reason above.
        game.world
            .insert_resource(crate::resources::Locale::Base { x: 0, y: 0 });
        game.add_companion(program).unwrap();
        game.world
            .insert_resource(crate::resources::Locale::Surface);

        let err = game
            .post_to_outpost(tile, program)
            .expect_err("a partied program isn't staff");
        assert!(err.contains("base staff"), "unexpected error: {err}");
        assert!(game.outpost_crew(tile).is_empty());
        // Deleted-fix check: without the role check this succeeds, leaving
        // the program both `InParty` and `PostedAt` at once.
    }

    #[test]
    fn post_to_outpost_refuses_a_downed_program() {
        let mut game = game(22);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        let program = staff(&mut game);
        game.world
            .entity_mut(program)
            .insert(crate::components::Downed);

        assert!(game.post_to_outpost(tile, program).is_err());
        assert!(game.outpost_crew(tile).is_empty());
    }

    #[test]
    fn post_to_outpost_refuses_a_carrying_program() {
        let mut game = game(23);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        let program = staff(&mut game);
        let id = kit_id(&game);
        game.world
            .entity_mut(program)
            .insert(crate::components::Carrying { item: id, qty: 1 });

        assert!(game.post_to_outpost(tile, program).is_err());
        assert!(game.outpost_crew(tile).is_empty());
        // Deleted-fix check: without this guard the post succeeds and
        // despawns nothing, but the carried load is stranded on a body that
        // no longer walks the base at all.
    }

    /// `is_on_shift`'s pairing (I3): a body walking a downed program to a
    /// rack is exactly as un-freeable as one holding `Carrying`, and for the
    /// same reason — a carried kill is a program the player cannot get back.
    #[test]
    fn post_to_outpost_refuses_a_program_carrying_a_program() {
        let mut game = game(26);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        let program = staff(&mut game);
        let species = game.species_defs()[0].id.clone();
        game.world
            .entity_mut(program)
            .insert(crate::components::CarryingProgram(
                crate::items::DownedProgram {
                    species,
                    level: 1,
                    rarity: Default::default(),
                    boss: false,
                    condition: 100,
                    carried: None,
                },
            ));

        assert!(game.post_to_outpost(tile, program).is_err());
        assert!(game.outpost_crew(tile).is_empty());
        // Deleted-fix check: without this guard the post succeeds and the
        // carried program is stranded on a body that no longer walks the
        // base at all.
    }

    #[test]
    fn post_to_outpost_refuses_without_a_record_at_the_tile() {
        let mut game = game(24);
        let (ax, ay) = game.anchor_position().unwrap();
        let program = staff(&mut game);
        let no_outpost_here = (ax + 500, ay + 500);

        let err = game
            .post_to_outpost(no_outpost_here, program)
            .expect_err("no outpost stands there");
        assert!(err.contains("No outpost"), "unexpected error: {err}");
    }

    #[test]
    fn post_to_outpost_refuses_when_the_player_is_too_far() {
        let mut game = game(25);
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        game.found_outpost(tile).unwrap();
        // The player never moved onto the tile this time — still standing
        // at the anchor, well outside adjacency.
        let program = staff(&mut game);

        let err = game
            .post_to_outpost(tile, program)
            .expect_err("the player must be standing at the outpost");
        assert!(err.contains("standing"), "unexpected error: {err}");
        // Deleted-fix check: without the distance check this succeeds from
        // anywhere on the map, which is exactly the bug this guards.
    }

    #[test]
    fn post_to_outpost_refuses_when_the_crew_is_full() {
        let mut game = game(26);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        for _ in 0..crate::tuning::OUTPOST_CREW_CAP {
            let program = staff(&mut game);
            game.post_to_outpost(tile, program).unwrap();
        }
        let one_more = staff(&mut game);
        let err = game
            .post_to_outpost(tile, one_more)
            .expect_err("the crew cap must be reachable and then hold");
        assert!(err.contains("full"), "unexpected error: {err}");
        assert_eq!(
            game.outpost_crew(tile).len(),
            crate::tuning::OUTPOST_CREW_CAP
        );
    }

    #[test]
    fn outpost_crew_is_sorted_by_program_id() {
        let mut game = game(27);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        // Spawned in this order, so a query-order bug would list them
        // reversed rather than merely differently.
        let first = staff(&mut game);
        let second = staff(&mut game);
        game.post_to_outpost(tile, second).unwrap();
        game.post_to_outpost(tile, first).unwrap();

        assert_eq!(game.outpost_crew(tile), vec![first, second]);
    }

    #[test]
    fn recall_from_outpost_returns_the_program_to_staff() {
        let mut game = game(28);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        let program = staff(&mut game);
        game.post_to_outpost(tile, program).unwrap();

        game.recall_from_outpost(program).unwrap();

        assert_eq!(game.program_role(program), Some(ProgramRole::Staff));
        assert!(game.outpost_crew(tile).is_empty());
    }

    #[test]
    fn recall_from_outpost_refuses_a_program_that_is_not_posted() {
        let mut game = game(29);
        let program = staff(&mut game);
        assert!(game.recall_from_outpost(program).is_err());
    }

    // -- Phase 5: repair_outpost ----------------------------------------

    /// Writes a temp `assets/outposts/outpost.ron` naming `cost` as
    /// `repair_cost`, loads it and returns the `OutpostDb` — the shipped def
    /// ships an empty `repair_cost` (Phase 6 fills it in), so a test of the
    /// refusal-then-spend ladder needs one of its own with real lines in it.
    fn outpost_db_with_repair_cost(cost: &[(&str, u32)]) -> OutpostDb {
        let dir = std::env::temp_dir().join(format!(
            "feral_outpost_repair_cost_{}_{}",
            std::process::id(),
            cost.len()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let lines: String = cost
            .iter()
            .map(|(id, qty)| format!("(\"{id}\", {qty}), "))
            .collect();
        let ron = format!(
            "OutpostDef(name: \"Outpost\", glyph: '#', kit: \"outpost_kit\", \
             repair_cost: [{lines}], tiers: [(yields: {{Deadlock: [\"raw_trace\"]}})])"
        );
        std::fs::write(dir.join("outpost.ron"), ron).unwrap();
        let (db, warnings) = OutpostDb::load_dir(&dir).unwrap();
        assert!(
            warnings.is_empty(),
            "the fixture's own RON must parse cleanly"
        );
        std::fs::remove_dir_all(&dir).unwrap();
        db
    }

    #[test]
    fn repair_outpost_refuses_without_a_record_at_the_tile() {
        let mut game = game(60);
        let (ax, ay) = game.anchor_position().unwrap();
        assert!(game.repair_outpost((ax + 999, ay + 999)).is_err());
    }

    #[test]
    fn repair_outpost_refuses_when_the_player_is_too_far() {
        let mut game = game(61);
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        game.found_outpost(tile).unwrap();
        game.world
            .resource_mut::<Outposts>()
            .0
            .get_mut(&tile)
            .unwrap()
            .integrity = 10;
        let err = game
            .repair_outpost(tile)
            .expect_err("the player must be standing at the outpost");
        assert!(err.contains("standing"), "unexpected error: {err}");
    }

    #[test]
    fn repair_outpost_refuses_at_full_integrity() {
        let mut game = game(62);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        assert!(game.repair_outpost(tile).is_err());
    }

    #[test]
    fn repair_outpost_refuses_without_a_loaded_outpost_def() {
        let mut game = game(63);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        game.world
            .resource_mut::<Outposts>()
            .0
            .get_mut(&tile)
            .unwrap()
            .integrity = 10;
        game.world.insert_resource(OutpostDb::default());
        assert_eq!(
            game.repair_outpost(tile),
            Err("No outpost design is known.".to_string())
        );
    }

    /// Every refusal lands before anything is spent, asserted per refusal —
    /// `every_refusal_leaves_stock_and_routes_exactly_as_they_were`'s shape.
    /// **The deleted-fix check for this ladder**: a version that spends
    /// each line as it checks it, rather than checking every line before
    /// spending any, would leave `raw_trace` gone here even though the bill
    /// as a whole was refused for want of `static_mesh`.
    #[test]
    fn repair_outpost_refuses_the_whole_bill_before_a_unit_moves() {
        let mut game = game(64);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        game.world
            .resource_mut::<Outposts>()
            .0
            .get_mut(&tile)
            .unwrap()
            .integrity = 10;
        game.world.insert_resource(outpost_db_with_repair_cost(&[
            ("raw_trace", 3),
            ("static_mesh", 2),
        ]));
        let player = game.player_entity();
        let raw_trace = crate::items::ItemId("raw_trace".to_string());
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(raw_trace.clone(), 3);
        // static_mesh is short by the whole amount — the bill must refuse
        // before raw_trace, which alone would have been affordable, is spent.

        let err = game.repair_outpost(tile).expect_err("static_mesh is short");
        let static_mesh = crate::items::ItemId("static_mesh".to_string());
        let static_mesh_name = game.item_name(&static_mesh).to_string();
        assert!(err.contains(&static_mesh_name), "unexpected error: {err}");
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&raw_trace),
            3,
            "a refused bill must not spend the lines that were individually affordable"
        );
        assert_eq!(
            game.world.resource::<Outposts>().0[&tile].integrity,
            10,
            "a refused repair must not touch integrity"
        );
    }

    #[test]
    fn repair_outpost_spends_the_cost_and_restores_full_integrity() {
        let mut game = game(65);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        game.world
            .resource_mut::<Outposts>()
            .0
            .get_mut(&tile)
            .unwrap()
            .integrity = 10;
        game.world
            .insert_resource(outpost_db_with_repair_cost(&[("raw_trace", 3)]));
        let player = game.player_entity();
        let raw_trace = crate::items::ItemId("raw_trace".to_string());
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(raw_trace.clone(), 3);

        game.repair_outpost(tile).unwrap();

        assert_eq!(
            game.world.resource::<Outposts>().0[&tile].integrity,
            OUTPOST_MAX_INTEGRITY
        );
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&raw_trace),
            0,
            "the cost is spent in full"
        );
    }

    // -- Task 5: visit, report, marks, transfer take ------------------------

    #[test]
    fn walking_onto_an_outpost_queues_a_visit_and_leaves_position_unchanged() {
        let mut game = game(30);
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        game.found_outpost(tile).unwrap();
        let player = game.player_entity();
        // Stand one step away and bump into it, `settlement_east_of_player`'s
        // shape — a directed step rather than teleporting onto the tile,
        // since the bump is the arm under test.
        {
            let mut pos = game.world.get_mut::<Position>(player).unwrap();
            pos.x = tile.0 - 1;
            pos.y = tile.1;
        }
        let before = *game.world.get::<Position>(player).unwrap();

        game.move_player(1, 0);

        let after = *game.world.get::<Position>(player).unwrap();
        assert_eq!(
            before, after,
            "an outpost admits nobody — the bump must not move the player"
        );
        assert_eq!(
            game.take_visit(),
            Some(crate::resources::Visit::Outpost(tile)),
            "the bump must name the outpost it landed on"
        );
        assert_eq!(
            game.take_visit(),
            None,
            "the drain must answer None on the second call"
        );
    }

    #[test]
    fn outpost_report_is_none_without_a_record_at_the_tile() {
        let mut game = game(31);
        let (ax, ay) = game.anchor_position().unwrap();
        assert!(game.outpost_report((ax + 999, ay + 999)).is_none());
    }

    #[test]
    fn outpost_report_reflects_a_hand_built_record() {
        let mut game = game(32);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        {
            let mut outposts = game.world.resource_mut::<Outposts>();
            let outpost = outposts.0.get_mut(&tile).unwrap();
            outpost.growth = crate::tuning::OUTPOST_TIER_GROWTH[1];
            outpost.integrity = 40;
            outpost
                .stock
                .insert(crate::items::ItemId("raw_trace".to_string()), 5);
        }
        let a = staff(&mut game);
        let b = staff(&mut game);
        game.post_to_outpost(tile, a).unwrap();
        game.post_to_outpost(tile, b).unwrap();

        let report = game.outpost_report(tile).expect("the record exists");

        assert_eq!(report.tile, tile);
        assert_eq!(report.integrity, 40);
        assert_eq!(report.max_integrity, OUTPOST_MAX_INTEGRITY);
        assert_eq!(report.stock, 5);
        assert_eq!(report.stock_cap, OUTPOST_STOCK_CAP);
        assert_eq!(report.crew.len(), 2);
        assert!(report.route.is_none(), "no caravan route runs to this tile");
        assert!(!report.yields.is_empty(), "a founded outpost yields tier 1");
        assert_eq!(report.tier_label, crate::outposts::tier_label(report.tier));
        assert!(!report.dark, "integrity 40 is not zero");
    }

    #[test]
    fn outpost_report_is_dark_at_zero_integrity() {
        let mut game = game(33);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        game.world
            .resource_mut::<Outposts>()
            .0
            .get_mut(&tile)
            .unwrap()
            .integrity = 0;

        let report = game.outpost_report(tile).unwrap();
        assert!(report.dark);
    }

    /// `outpost_report`'s `route` field reads `Game::outpost_route_line` —
    /// a route to this tile is filled in and a stalled one names it, rather
    /// than a route runs directly through `dispatch_outpost_route` (that
    /// door's own fixture and refusal ladder live in `tests::routes`).
    #[test]
    fn outpost_report_route_reflects_a_route_bound_to_this_tile() {
        let mut game = game(40);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        assert!(game.outpost_report(tile).unwrap().route.is_none());

        game.world
            .resource_mut::<crate::resources::Routes>()
            .0
            .push(crate::routes::Route {
                destination: crate::routes::RouteEnd::Outpost(tile),
                cargo: Vec::new(),
                standing: true,
                stalled: false,
                leg: crate::routes::RouteLeg::Outbound,
                ticks_total: 100,
                ticks_elapsed: 40,
                proceeds: 0,
            });
        let line = game.outpost_report(tile).unwrap().route;
        assert!(line.is_some_and(|l| l.contains("60")), "60 ticks remain");

        game.world
            .resource_mut::<crate::resources::Routes>()
            .0
            .get_mut(0)
            .unwrap()
            .stalled = true;
        let line = game.outpost_report(tile).unwrap().route;
        assert!(
            line.clone().is_some_and(|l| l.contains("can't reach")),
            "a stalled route must say so: {line:?}"
        );
    }

    #[test]
    fn outpost_marks_reports_one_mark_per_outpost() {
        let mut game = game(34);
        let tile = founded_outpost_with_player_standing_there(&mut game);

        let marks = game.outpost_marks();

        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].tile, tile);
        assert_eq!(marks[0].tier, 0);
        assert!(!marks[0].dark);
    }

    #[test]
    fn outpost_marks_is_empty_with_no_outposts() {
        let mut game = game(35);
        assert!(game.outpost_marks().is_empty());
    }

    #[test]
    fn outpost_transfer_offer_rows_never_allow_a_put() {
        let mut game = game(36);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        let item = crate::items::ItemId("raw_trace".to_string());
        game.world
            .resource_mut::<Outposts>()
            .0
            .get_mut(&tile)
            .unwrap()
            .stock
            .insert(item.clone(), 9);

        let rows = game.outpost_transfer_offer(tile);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].item, item);
        assert_eq!(rows[0].on_shelves, 9);
        assert_eq!(rows[0].can_put, 0);
    }

    #[test]
    fn take_from_outpost_refuses_without_a_record_at_the_tile() {
        let mut game = game(37);
        let (ax, ay) = game.anchor_position().unwrap();
        let basket =
            TransferBasket::items(&[(crate::items::ItemId("raw_trace".to_string()), 1)], &[]);
        assert!(
            game.take_from_outpost((ax + 999, ay + 999), &basket)
                .is_err()
        );
    }

    #[test]
    fn take_from_outpost_refuses_when_the_player_is_too_far() {
        let mut game = game(38);
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        game.found_outpost(tile).unwrap();
        let item = crate::items::ItemId("raw_trace".to_string());
        game.world
            .resource_mut::<Outposts>()
            .0
            .get_mut(&tile)
            .unwrap()
            .stock
            .insert(item.clone(), 9);
        let basket = TransferBasket::items(&[(item, 1)], &[]);

        let err = game
            .take_from_outpost(tile, &basket)
            .expect_err("standing far away must refuse");
        assert!(err.contains("standing"), "unexpected error: {err}");
    }

    #[test]
    fn take_from_outpost_clamps_to_the_stock_actually_standing_and_reports_it() {
        let mut game = game(39);
        let tile = founded_outpost_with_player_standing_there(&mut game);
        let item = crate::items::ItemId("raw_trace".to_string());
        game.world
            .resource_mut::<Outposts>()
            .0
            .get_mut(&tile)
            .unwrap()
            .stock
            .insert(item.clone(), 3);
        game.world
            .resource_mut::<crate::resources::BattleTelemetry>()
            .on = true;
        let player = game.player_entity();
        let before = game
            .world
            .get::<Inventory>(player)
            .map(|i| i.count(&item))
            .unwrap_or(0);
        let basket = TransferBasket::items(&[(item.clone(), 10)], &[]);

        let moved = game.take_from_outpost(tile, &basket).unwrap();

        assert_eq!(moved, vec![(item.clone(), 3)], "clamped to what stood");
        assert_eq!(
            game.world
                .resource::<Outposts>()
                .0
                .get(&tile)
                .unwrap()
                .stock
                .get(&item),
            None,
            "an emptied item is removed from the map rather than left at zero"
        );
        let after = game.world.get::<Inventory>(player).unwrap().count(&item);
        assert_eq!(after, before + 3);
        let records = &game
            .world
            .resource::<crate::resources::BattleTelemetry>()
            .records;
        assert!(
            records.iter().any(|r| matches!(
                r,
                crate::telemetry::Record::Acquire { item: i, qty, source, .. }
                    if i.as_str() == "raw_trace" && *qty == 3 && source.as_str() == "outpost"
            )),
            "a hand take must report through Game::grant_loot with LootSource::Outpost: {records:?}"
        );
    }
}
