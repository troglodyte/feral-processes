//! Looking at the world without changing it: the tile and entity views the
//! renderer draws, plus inspect and symlink targeting.

use crate::game::base::hauling::at_station;
use crate::tuning::{
    DIFFICULTY_EASY_MAX, DIFFICULTY_EVEN_MAX, DIFFICULTY_TOUGH_MAX, MAX_COMPANION_REFACTORS,
    MAX_FUSIONS,
};
use crate::views::drawn_on_surface_map;
use crate::*;
use std::collections::HashSet;

/// Which of two things sharing one tile `find_target_in_direction` names —
/// lower wins. The structure, because it is the glyph the map draws there.
///
/// Named constants rather than deriving `Ord` on `InspectTarget` and letting
/// its variant order carry the rule: that would hide a decision about what
/// the player sees inside a declaration order, where a tidy-up reorder flips
/// it silently and nothing reads as having changed.
const STRUCTURE_ON_TILE: u8 = 0;
const CREATURE_ON_TILE: u8 = 1;

impl Game {
    /// Which sector the party is standing in, or `None` for a neutral one.
    ///
    /// Both inputs come off live state the save already carries, which is
    /// why a sector needs no field of its own — see `sectors::for_zone`,
    /// which is the only place the choice is made.
    pub fn sector(&self) -> Option<&crate::sectors::SectorDef> {
        crate::sectors::for_zone(
            self.world.resource::<WorldMap>().seed(),
            self.world.resource::<ZoneLevel>().0,
            self.world.resource::<crate::sectors::SectorDb>(),
        )
    }

    /// This sector's ground and hazard hues, in degrees, falling back to the
    /// neutral pair.
    ///
    /// Two floats and no `Color`: what a hue *looks like* is
    /// `crates/gui/src/render/base.rs`'s business, and the engine shipping a
    /// palette would put the colour table on the wrong side of the drawing
    /// seam. The renderer reads these as the anchors its two bands rotate
    /// about, not as finished colours.
    pub fn sector_hues(&self) -> (f32, f32) {
        self.sector().map_or(
            (
                crate::sectors::NEUTRAL_GROUND_HUE,
                crate::sectors::NEUTRAL_HAZARD_HUE,
            ),
            |def| (def.palette.ground_hue, def.palette.hazard_hue),
        )
    }

    /// The tile grid the map renders — the zone surface, or base space when
    /// the party is out of phase, both through the one renderer.
    ///
    /// `render/base.rs`'s `draw_surface_map` gets its tiles from exactly
    /// this one call, and nothing else about it changed to make base space
    /// drawable: dispatching here on `Game::base_pos` is the entire
    /// relocation, on the gui side. Base space has no `Tile` of its own —
    /// `base_grid::BaseCell` carries no biome — so this synthesises one per
    /// cell: `Floor` → `Biome::Platform`, `Open` → `Biome::Excavated`,
    /// absent (solid) → `Biome::Entropy`, all three walkable exactly as
    /// `Biome::walkable` already says.
    pub fn view_tiles(&mut self, half_w: i32, half_h: i32) -> Vec<Vec<Tile>> {
        if let Some((cx, cy)) = self.base_pos() {
            let grid = self.world.resource::<crate::base_grid::BaseGrid>();
            // Hoisted out of the loop: this runs per tile per frame, and a
            // resource lookup per cell for a value that cannot change inside
            // one call is a cost with nothing to show for it.
            let seed = grid.seed();
            let rock = self.world.resource::<crate::rock::RockDb>();
            let mut rows = Vec::new();
            for ty in -half_h..=half_h {
                let mut row = Vec::new();
                for tx in -half_w..=half_w {
                    let (x, y) = (cx + tx, cy + ty);
                    let biome = match grid.cell(x, y) {
                        Some(crate::base_grid::BaseCell::Floor) => Biome::Platform,
                        Some(crate::base_grid::BaseCell::Open { .. }) => Biome::Excavated,
                        None => Biome::Entropy,
                    };
                    row.push(Tile {
                        biome,
                        walkable: biome.walkable(),
                        // **Exposed faces only.** Colouring every wall would
                        // hand the player a map of everything they will ever
                        // dig; colouring the faces with air against them
                        // makes exposing one the act of prospecting. Rock
                        // deeper than a face draws as the flat hole it has
                        // always been.
                        rock_shade: grid
                            .is_exposed(x, y)
                            .then(|| rock.wall_at(seed, x, y).shade),
                    });
                }
                rows.push(row);
            }
            return rows;
        }
        let center = *self.world.get::<Position>(self.player_entity()).unwrap();
        let mut world_map = self.world.resource_mut::<WorldMap>();
        let mut rows = Vec::new();
        for ty in -half_h..=half_h {
            let mut row = Vec::new();
            for tx in -half_w..=half_w {
                row.push(world_map.tile(center.x + tx, center.y + ty));
            }
            rows.push(row);
        }
        rows
    }

    /// The structure standing on the tile one step in `(dx, dy)`, if any.
    ///
    /// One tile, deliberately, where `find_target_in_direction` below runs a
    /// ray out to `EXAMINE_RANGE_TILES`. Its caller demolishes what it finds,
    /// so any reach at all would let a single keypress take down a structure
    /// across the base; you have to be standing next to what you remove.
    ///
    /// An `EntityView` rather than an `Entity` because the caller has to route
    /// a Home into its confirmation screen, and `view_entities` is where
    /// `is_home` is decided — the demolish menu reads the same field from the
    /// same builder, so the two routes cannot disagree about what a Home is.
    ///
    /// `None` outside base space, not merely underground: a `Structure`
    /// only ever stands in base space (`Structure` is the space tag — see
    /// `find_blocking_structure_at`), so a demolish key pressed anywhere
    /// else is asking about a tile in a different coordinate space
    /// entirely. Underground that tile is the base four frames overhead,
    /// reached the same way `find_target_in_direction` used to reach it
    /// before this guard covered creatures too, because `Position` is
    /// pinned to the surface entrance tile down there; on the open surface
    /// it is whatever the numbers happen to alias against base space's own
    /// origin.
    pub fn adjacent_structure(&mut self, dx: i32, dy: i32) -> Option<EntityView> {
        if !self.in_base() {
            return None;
        }
        let center = self.scan_center();
        let target = (center.x + dx, center.y + dy);
        // A square box just big enough to contain the one tile asked about,
        // rather than a per-axis one: the scan is only a way to reach the
        // shared view builder, and the `pos` filter is what actually selects.
        let reach = dx.abs().max(dy.abs());
        self.view_entities(reach, reach)
            .into_iter()
            .find(|e| e.is_structure && e.pos == target)
    }

    /// Whether `entity` is a tamed program holding a `GatherResource` post
    /// and not currently standing at it — see
    /// `EntityView::worker_away_from_post`, which is this value.
    ///
    /// A function rather than the inline expression it was, because the
    /// frontend's "someone is on this job" mark reads the same answer.
    pub(crate) fn worker_away_from_post(&self, entity: Entity) -> bool {
        self.world.get::<Tamed>(entity).is_some()
            && self.world.get::<Task>(entity).is_some_and(|t| {
                t.kind == TaskKind::GatherResource
                    && self
                        .world
                        .get::<Position>(entity)
                        .zip(self.world.get::<Position>(t.target))
                        .is_some_and(|(pos, station)| !at_station(*pos, *station))
            })
    }

    /// Whether `entity`'s `Position` is a tile the sim keeps up to date —
    /// see `EntityView::position_is_honest`, which is this value, and
    /// `views::drawn_on_surface_map`, which is what consumes it.
    ///
    /// Wider than `worker_away_from_post` above by exactly one case: an
    /// idle base staff member, which `schedule_base_labour` parks on a ring
    /// around the Home every tick. A guard and a party companion keep
    /// whatever tile they were on when they took the job, and neither is
    /// written again.
    pub(crate) fn position_is_honest(&self, entity: Entity) -> bool {
        if self.world.get::<Tamed>(entity).is_none() {
            return true;
        }
        self.worker_away_from_post(entity)
            || (self.program_role(entity) == Some(ProgramRole::Staff)
                && self.world.get::<Task>(entity).is_none())
    }

    /// Whether `entity`'s `Position` is a cell of base space rather than a
    /// tile of the zone surface — the space tag every map-facing view
    /// filters on.
    ///
    /// **The party's own things stand in base space and everything else
    /// stands on the surface.** A `Structure` only ever stands on the
    /// base's floor, which is what makes it *the* space tag (see
    /// `find_blocking_structure_at`, and `BaseAnchor`'s own doc for why the
    /// anchor is deliberately not one). A `Tamed` program is posted at a
    /// machine, hauling between two, or parked around the Home by
    /// `schedule_base_labour`. Everything else with a `Glyph` — a wild
    /// program, a nest, a Stack entrance, the anchor — is a fixture of the
    /// zone map.
    ///
    /// The two answers are `Position`s in different coordinate systems that
    /// freely alias onto each other, which is the whole reason this exists:
    /// base space's origin and the zone spawn point are both usually
    /// `(0, 0)`.
    ///
    /// A party companion and a posted guard are the two tamed programs
    /// whose `Position` is never written again, so theirs is the tile they
    /// were beaten on — out on the surface, or four frames down. Answering
    /// "base space" for them is still the right answer to the question
    /// actually being asked: a companion is standing beside you rather than
    /// where it was caught, and `position_is_honest` is what says so.
    /// Between them the two rules leave a stale tile drawn in neither space.
    ///
    /// The player is in both spaces at once and has no answer here — its
    /// callers hold it out, and `scan_center` is where its base-space cell
    /// comes from.
    /// A caravan is the third arm, and the only one that is **per-stage
    /// rather than per-entity**: it is the first entity besides the player
    /// that changes spaces, so the answer is read off `Caravan::stage` and
    /// not off the component being there at all. Testing for the component
    /// would pin a trader in one space for its whole visit, and the wrong
    /// half of that journey would be drawn on a coordinate that aliases onto
    /// a plausible tile in the other space.
    pub(crate) fn stands_in_base_space(&self, entity: Entity) -> bool {
        if let Some(caravan) = self.world.get::<Caravan>(entity) {
            return caravan.stage.in_base_space();
        }
        self.world.get::<Structure>(entity).is_some() || self.world.get::<Tamed>(entity).is_some()
    }

    /// The first creature or structure along the row or column the player is
    /// facing — the read-only "look in a direction" counterpart to
    /// `move_player`. `(dx, dy)` is one of the four cardinal unit vectors.
    /// Ignores terrain walkability (this never moves anything, just looks),
    /// and never matches the player.
    ///
    /// **A ray exactly one tile wide, and it used to be a 90° cone.** The
    /// cone counted anything leaning toward the chosen axis at least as much
    /// as away from it, on the reasoning that a strict line would rarely
    /// coincide with a wandering creature's exact row. What that traded away
    /// was any relationship between the key pressed and the answer: paired
    /// with the 40-tile reach the caller passed — app-core's `MENU_SCAN_
    /// RADIUS`, a *menu window*, against a map pane of roughly 16x9 — an
    /// eastward press could name a program forty tiles east *and* forty
    /// north, well off screen in both. The reach is now `EXAMINE_RANGE_TILES`
    /// and the shape is the line the player is looking down. Missing the
    /// creature one tile off your row is the price, and it is the right one:
    /// you can step or turn, and now what `x` names is what was in front of
    /// you.
    ///
    /// **Both kinds are gathered in one walk, and that is what makes
    /// "nearest wins" answerable.** Two functions and a caller choosing
    /// between them would have to re-derive distance to compare, putting the
    /// ray rule in two places; the returned variant is the answer this walk
    /// already computed, so a caller never has to ask a second time what it
    /// just found.
    ///
    /// **The order is total, which is what makes the answer stable.** Bevy's
    /// query iteration order is not, so `min_by_key` — which returns the
    /// *first* of several equal minima — would let a tie resolve differently
    /// between runs or after a reload. `(step, kind, entity)` has no equal
    /// minima to be first among. Same trap as `assembler_system`'s `(x, y)`
    /// sort, reached from the other side.
    ///
    /// A tile holding both names the **structure**, because that is the glyph
    /// the map draws there. For the same reason the walk skips any program
    /// the map does not draw (`views::drawn_on_surface_map`): a worker at its
    /// post stands orthogonally *beside* its machine, so without that filter
    /// aiming at a machine hit an invisible program one tile in front of it.
    ///
    /// The ray is transparent to everything that is neither a `Creature` nor
    /// a `Structure` — nests, surface links and zone portals all draw a glyph
    /// and are passed straight through, so aiming at one reports whatever
    /// lies beyond it. That is a known gap rather than a decision; see
    /// `TODO.md`.
    ///
    /// **Nothing is found underground, and that is the whole function's
    /// guard rather than one scan's.** `Position` stays pinned to the
    /// surface entrance tile while the party is in the Stack, so an
    /// unguarded scan reports the base four frames overhead as being off to
    /// your east — and, before this guard covered creatures too, opened a
    /// manifest for a wild program up there as lying "that way". The guard
    /// lives here rather than at the call site for the reason
    /// `require_surface` exists.
    ///
    /// This takes no action and moves nothing, so `require_surface` does not
    /// apply and never would have caught it. The test for whether a
    /// `Position` reader needs the guard is not "does it act" but "does it
    /// claim something about where the party is" — see `CLAUDE.md`'s
    /// load-bearing-seams entry. Underground, `x` describes the cell instead
    /// (`Game::describe_view_direction`), which is a claim about the frame
    /// the party is actually in.
    pub fn find_target_in_direction(
        &mut self,
        dx: i32,
        dy: i32,
        max_range: i32,
    ) -> Option<InspectTarget> {
        if self.is_underground() {
            return None;
        }
        let in_base = self.in_base();
        let start = self.scan_center();
        // `(dx, dy)` is a cardinal unit vector, so a tile is on the ray
        // exactly when its offset *is* `step` copies of it — which rules out
        // the off-axis tile, the player's own tile and everything behind
        // them in one condition.
        let on_ray = |pos: &Position| -> Option<i32> {
            let (ddx, ddy) = (pos.x - start.x, pos.y - start.y);
            let step = ddx * dx + ddy * dy;
            (step >= 1 && step <= max_range && ddx == dx * step && ddy == dy * step).then_some(step)
        };

        // Both queries below run over raw `Position`s, so both are gated on
        // `Game::stands_in_base_space` agreeing with where the party is —
        // the same one condition `view_entities` selects on, because the
        // rule here is that examine names only what the map draws and two
        // spellings of one rule is how that lapses. A ray aimed across the
        // open grid could otherwise name a base structure, or a program
        // parked beside the Home, at whatever surface tile their base-space
        // cell aliased onto — commonly near `(0, 0)`, since base space's own
        // origin and the zone spawn point usually share it — and a ray aimed
        // from inside the base could name a wild program out on the actual
        // zone surface, the "rays across the zone surface" bug named for
        // this function.
        //
        // `drawn_on_surface_map` is still asked of every creature that
        // survives the space test, and neither subsumes the other: this one
        // says which space a `Position` is a tile of, that one says whether
        // it is a tile the sim keeps up to date.
        let mut candidates: Vec<(i32, u8, Entity)> = Vec::new();
        if in_base {
            let mut structures = self.world.query::<(Entity, &Position, &Structure)>();
            candidates.extend(
                structures
                    .iter(&self.world)
                    .filter_map(|(e, p, _)| on_ray(p).map(|step| (step, STRUCTURE_ON_TILE, e))),
            );
        }
        let creatures_on_ray: Vec<(i32, Entity)> = {
            let mut creatures = self.world.query::<(Entity, &Position, &Creature)>();
            creatures
                .iter(&self.world)
                .filter_map(|(e, p, _)| on_ray(p).map(|step| (step, e)))
                .collect()
        };
        candidates.extend(creatures_on_ray.into_iter().filter_map(|(step, e)| {
            if self.stands_in_base_space(e) != in_base {
                return None;
            }
            let tamed = self.world.get::<Tamed>(e).is_some();
            drawn_on_surface_map(tamed, self.position_is_honest(e)).then_some((
                step,
                CREATURE_ON_TILE,
                e,
            ))
        }));

        candidates.into_iter().min().map(|(_, rank, entity)| {
            if rank == STRUCTURE_ON_TILE {
                InspectTarget::Structure(entity)
            } else {
                InspectTarget::Creature(entity)
            }
        })
    }

    /// The `B` roster's row for one structure, for the inspector's detail
    /// screen. Deliberately *the same call* the roster makes rather than a
    /// second builder beside it: per `CLAUDE.md` a doc comment claiming to
    /// mirror another formula has to be a call, and a detail screen that
    /// disagreed with the roster about the same machine is exactly the drift
    /// that rule exists to stop. Building every row to return one is O(n)
    /// over a base's worth of structures, once per keypress.
    pub fn structure_manifest(&mut self, entity: Entity) -> Option<StructureReport> {
        self.structure_report()
            .into_iter()
            .find(|r| r.entity == entity)
    }

    /// Puts a scan's results in the order every menu built from one shows
    /// them: by name, then by position.
    ///
    /// Both scans need it and neither may differ from the other, since the
    /// cronjob picker and the symlink picker are lists of the same base. The
    /// position tiebreak is not cosmetic — bevy's query iteration order is
    /// not stable, so two Mining Nodes with nothing else to separate them
    /// would otherwise swap rows between openings of the same menu, and a
    /// list nobody can learn the shape of is worse than an unsorted one.
    fn sort_by_label(views: &mut [EntityView]) {
        views.sort_by(|a, b| (&a.label, a.pos).cmp(&(&b.label, b.pos)));
    }

    /// Display label for any entity — species name for a creature,
    /// structure name for a structure, `"You"` otherwise. Shared by
    /// `view_entities` for both an entity's own label and cross-references
    /// (a worker's assigned structure, a structure's assigned worker).
    pub(crate) fn entity_label(&self, entity: Entity) -> String {
        if let Some(name) = self.creature_name(entity) {
            self.zone_tagged_name(entity, name)
        } else if let Some(s) = self.world.get::<Structure>(entity) {
            self.world
                .resource::<StructureDb>()
                .get(&s.kind)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| s.kind.clone())
        } else if let Some(nest) = self.world.get::<Nest>(entity) {
            let species_name = self
                .world
                .resource::<SpeciesDb>()
                .get(&nest.species)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| nest.species.clone());
            format!("{species_name} Nest")
        } else if self.world.get::<BaseAnchor>(entity).is_some() {
            // Unreachable before this task: nothing rendered base space, so
            // nothing ever asked `view_entities` for a name near the anchor.
            // `Game::view_tiles` makes that reachable, and the fall-through
            // below said "You" for it — the same body the player's own
            // entity gets, on an entity that is never the player.
            "The Anchor".to_string()
        } else if self.world.get::<SurfaceLink>(entity).is_some() {
            "Stack Entrance".to_string()
        } else if let Some(dig) = self.world.get::<DigSite>(entity) {
            // A dig site is named by its tile, because it has nothing else:
            // no def, no species, and no glyph. Without this arm it falls
            // through to `"You"` below, and a posted digger's manifest row
            // reads as the player standing at their own post.
            //
            // *What* it is called comes off the mark and the cell under it,
            // for `toggle_mark_box`'s reason: one verb, and the cell decides
            // which half of the job it means. An unmarked site is one the
            // player walked away from mid-swing — it appears in no plan, so
            // calling it marked is a claim about the plan that is false.
            let marked = dig.marked;
            let at = self.world.get::<Position>(entity).copied();
            let solid = at.is_none_or(|p| {
                self.world
                    .resource::<crate::base_grid::BaseGrid>()
                    .is_solid(p.x, p.y)
            });
            let what = match (marked, solid) {
                (true, true) => "Marked Wall",
                (true, false) => "Marked Floor",
                (false, _) => "Chipped Wall",
            };
            match at {
                Some(p) => format!("{what} ({}, {})", p.x, p.y),
                None => what.to_string(),
            }
        } else {
            "You".to_string()
        }
    }

    /// The tile every scan around the party is centered on, **in the
    /// coordinate space the things being scanned live in.**
    ///
    /// In base space that is the party's base cell, not their `Position`:
    /// `Position` is pinned to the anchor tile on the zone surface while
    /// they are out of phase, and every `Structure` stands in base space. A
    /// scan centered on the pinned tile would list the base only when the
    /// anchor happened to sit at the same numbers base space's origin does
    /// — which the zone spawn point usually does, so it would read as
    /// working and quietly stop the day a sector spawned somewhere else.
    fn scan_center(&self) -> Position {
        match self.base_pos() {
            Some((x, y)) => Position { x, y },
            None => *self.world.get::<Position>(self.player_entity()).unwrap(),
        }
    }

    /// Every entity within `half_w`/`half_h` of `scan_center`, in whichever
    /// space the party currently occupies.
    ///
    /// **Closes the cross-space read generally**, the same ruling
    /// `find_blocking_structure_at` acts on, and it is one condition rather
    /// than a gate per kind: an entity is selected exactly when
    /// `Game::stands_in_base_space` agrees with where the party is. That is
    /// symmetric on purpose, because the aliasing is. Standing on the
    /// surface tile that numerically matched a base-space Market's cell
    /// made `[S]ell` appear on the inventory screen
    /// (`app_core::traders_in_range`) and the trade calls it led to then
    /// refused via `require_base`; base space has no wildlife, ever, so a
    /// wild program's surface tile aliasing inward drew a monster nobody
    /// was fighting; and in the other direction the base's own roster —
    /// parked in base-space coordinates by `schedule_base_labour`, which is
    /// exactly what makes `position_is_honest` true of it — drew scattered
    /// across the open grid.
    ///
    /// **A kind-by-kind gate is what let that last one through**, since
    /// `Tamed` is the one space tag that had no gate at all and
    /// `views::drawn_on_surface_map` answers a different question: whether
    /// a program's `Position` means anything, not which space it means it
    /// in. Both filters are needed and neither subsumes the other.
    ///
    /// The player is held out of the space test — it is in both spaces at
    /// once — and read through `scan_center` instead, which is the base's
    /// half of that.
    pub fn view_entities(&mut self, half_w: i32, half_h: i32) -> Vec<EntityView> {
        let in_base = self.in_base();
        let center = self.scan_center();
        let player = self.player_entity();
        let mut query = self.world.query::<(Entity, &Position, &Glyph)>();
        let hits: Vec<(Entity, Position, Glyph)> = query
            .iter(&self.world)
            // The player is read through `scan_center` rather than off its
            // own `Position`, which stays pinned to the anchor tile on the
            // zone surface for as long as the party is out of phase (see
            // `resources::Locale`). This is the map's only source of the
            // `@` it draws, so reading the pinned tile drew the player at
            // whatever base-space cell the anchor's surface coordinates
            // aliased onto and left it there however far the party walked.
            // A no-op on the surface, where `scan_center` *is* that tile.
            .map(|(e, p, g)| (e, if e == player { center } else { *p }, *g))
            .filter(|(_, p, _)| {
                (p.x - center.x).abs() <= half_w && (p.y - center.y).abs() <= half_h
            })
            .collect();
        let hits: Vec<(Entity, Position, Glyph)> = hits
            .into_iter()
            .filter(|(e, _, _)| *e == player || self.stands_in_base_space(*e) == in_base)
            .collect();
        self.build_views(hits)
    }

    /// Every tamed program the player owns, wherever it happens to be
    /// standing — the roster, not a window onto the map.
    ///
    /// A companion's `Position` is the tile it was beaten on and is never
    /// written again (see `worker_away_from_post` below), so a distance
    /// filter over it hides programs by where they were *captured* rather
    /// than by where they are. `owned_pets` made this move already, for the
    /// fusion picker; the posting menus need the same list, and neither
    /// `assign_cronjob` nor `assign_guard` has a distance requirement on the
    /// program to justify one. Shares `build_views` with `view_entities`
    /// rather than repeating it: the two differ only in which entities they
    /// select, exactly as `pursuit_field` differs from `walk_field`.
    pub fn owned_program_views(&mut self) -> Vec<EntityView> {
        let player = self.player_entity();
        let mut query = self.world.query::<(Entity, &Position, &Glyph, &Tamed)>();
        let hits: Vec<(Entity, Position, Glyph)> = query
            .iter(&self.world)
            .filter(|(_, _, _, t)| t.owner == player)
            .map(|(e, p, g, _)| (e, *p, *g))
            .collect();
        self.build_views(hits)
    }

    /// The `EntityView` for each of `hits`, whatever selected them.
    fn build_views(&mut self, hits: Vec<(Entity, Position, Glyph)>) -> Vec<EntityView> {
        let worker_by_structure: HashMap<Entity, Entity> = {
            let mut tasks = self.world.query::<(Entity, &Task)>();
            tasks
                .iter(&self.world)
                .map(|(worker, task)| (task.target, worker))
                .collect()
        };
        // Structures with a posted program standing at them right now.
        //
        // Separate from the map above, which is keyed by target and so
        // collapses a machine's worker and its guard into whichever the
        // query reached last — a machine whose worker has stepped out is
        // still attended by its guard, and has to survive that pairing.
        let attended: HashSet<Entity> = {
            let mut tasks = self.world.query::<(Entity, &Task)>();
            let posted: Vec<(Entity, Entity, TaskKind)> = tasks
                .iter(&self.world)
                .map(|(holder, task)| (holder, task.target, task.kind))
                .collect();
            posted
                .into_iter()
                .filter(|&(holder, target, kind)| match kind {
                    // Nothing ever walks a guard to what it guards, so it is
                    // standing wherever it was when assigned — and is never
                    // drawn, which makes "at its post" the only useful
                    // answer for it.
                    TaskKind::Guard => true,
                    // A dig site carries no `Glyph`, so nothing ever selects
                    // one into `hits` and no view asks this set about it.
                    // The set is structures with somebody standing at them,
                    // and an excavation is not one.
                    TaskKind::Excavate => false,
                    TaskKind::GatherResource => {
                        match (
                            self.world.get::<Position>(holder),
                            self.world.get::<Position>(target),
                        ) {
                            (Some(w), Some(s)) => at_station(*w, *s),
                            _ => false,
                        }
                    }
                })
                .map(|(_, target, _)| target)
                .collect()
        };

        // Whether anywhere in the base can still take a load. Base-wide, and
        // rebuilt per call for the reason `haul_step_system` rebuilds its own
        // depot list every tick: a demolished or newly-filled depot has to
        // stop counting without anything having to notice it changed.
        let anywhere_to_unload = {
            let mut stores = self.world.query::<(&Structure, &Stock)>();
            let rooms: Vec<(StructureId, u32)> = stores
                .iter(&self.world)
                .map(|(s, stock)| (s.kind.clone(), stock.output_room()))
                .collect();
            let db = self.world.resource::<StructureDb>();
            rooms
                .iter()
                .any(|(kind, room)| *room > 0 && db.get(kind).is_some_and(|d| d.stores))
        };

        let player_power = self.player_power();
        let mut linked_edges = self.linked_edges_by_structure();

        let mut views: Vec<EntityView> = hits
            .into_iter()
            .map(|(entity, pos, glyph)| {
                let is_player = self.world.get::<Player>(entity).is_some();
                let is_tamed = self.world.get::<Tamed>(entity).is_some();
                let is_companion = self.world.resource::<Party>().0.contains(&entity);
                let is_hostile = self.world.get::<Hostile>(entity).is_some();
                let is_structure = self.world.get::<Structure>(entity).is_some();
                let is_home = self
                    .world
                    .get::<Structure>(entity)
                    .is_some_and(|s| s.kind == HOME_STRUCTURE_ID);
                let is_boss = self.is_boss_creature(entity);
                let is_nemesis = self.world.get::<Nemesis>(entity).is_some();
                let tier = self.world.get::<StructureTier>(entity).map(|t| t.0);
                let (ceiling, max_tier) = match self.entity_upgrade_ceiling(entity) {
                    Some((c, m)) => (Some(c), Some(m)),
                    None => (None, None),
                };
                let can_work = self.accepts_a_program(entity);
                let machine_status = self.world.get::<MachineStatus>(entity).copied();
                let can_trade = self.trade_options(entity).is_some();
                let issues_contracts = self.issues_contracts(entity);
                let structure_worker = if is_structure {
                    worker_by_structure
                        .get(&entity)
                        .map(|&worker| self.entity_label(worker))
                } else {
                    None
                };
                let worker_away_from_post = self.worker_away_from_post(entity);
                let position_is_honest = self.position_is_honest(entity);
                let structure_attended = is_structure && attended.contains(&entity);
                let output_stranded = is_structure
                    && !anywhere_to_unload
                    && self
                        .world
                        .get::<Stock>(entity)
                        .is_some_and(|s| s.output_room() == 0);
                let stats = self.world.get::<Stats>(entity);
                let hp_fraction = stats.map(|s| s.hp_fraction());
                // Hostile wild programs are recolored by difficulty relative
                // to the player's current power, rather than shown in their
                // species' authored color — see `difficulty_color`. Everyone
                // and everything else (the player, tamed/companion programs,
                // structures) keeps its normal glyph color.
                let color = if is_hostile {
                    stats
                        .map(|s| difficulty_color(s.power(), player_power, is_boss, is_nemesis))
                        .unwrap_or(glyph.color)
                } else {
                    glyph.color
                };
                let level = self.world.get::<Experience>(entity).map(|e| e.level);
                let durability = self
                    .world
                    .get::<Durability>(entity)
                    .map(|d| (d.hp, d.max_hp));
                let label = self.entity_label(entity);
                EntityView {
                    entity,
                    pos: (pos.x, pos.y),
                    glyph: glyph.ch,
                    color,
                    label,
                    is_player,
                    is_tamed,
                    is_companion,
                    is_hostile,
                    is_structure,
                    is_home,
                    tier,
                    ceiling,
                    max_tier,
                    is_boss,
                    nemesis: is_nemesis,
                    can_work,
                    can_trade,
                    issues_contracts,
                    structure_worker,
                    worker_away_from_post,
                    position_is_honest,
                    structure_attended,
                    output_stranded,
                    hp_fraction,
                    level,
                    durability,
                    fusions: self.fusion_count(entity),
                    rarity: self.rarity_of(entity),
                    machine_status,
                    linked_edges: linked_edges.remove(&entity).unwrap_or_default(),
                }
            })
            .collect();
        Self::sort_by_label(&mut views);
        views
    }

    /// For each structure, the orthogonal offsets of the neighbours it is
    /// joined to for production — the sides the map leaves un-outlined so a
    /// chain draws as one continuous shape.
    ///
    /// **Symmetric, though the feeding relation is not.** A Refinery names
    /// the Mining Node beside it; the Mining Node names nobody, because it
    /// has no recipe to want anything. Both walls between a joined pair have
    /// to go or the single remaining line reads as a rendering fault rather
    /// than as a join, so every link found is recorded from both ends.
    ///
    /// Reads the same `assembly_recipe` and walks the same `ORTHOGONAL` as
    /// `systems::assembler_system`'s pull phase, so a join can never be drawn
    /// where the pull phase would refuse to take. The one deliberate
    /// difference is documented on `EntityView::linked_edges`: this asks what
    /// a neighbour *makes*, not what is in its buffer this instant.
    ///
    /// Computed for the whole base in one pass rather than per structure:
    /// `view_entities` runs every frame, and asking each machine to re-scan
    /// every structure in the zone would be quadratic in the size of a base
    /// for a picture that only changes when something is built.
    pub(crate) fn linked_edges_by_structure(&mut self) -> HashMap<Entity, Vec<(i32, i32)>> {
        let mut query = self.world.query::<(Entity, &Position, &Structure)>();
        let placed: Vec<(Entity, Position, StructureId)> = query
            .iter(&self.world)
            .map(|(e, p, s)| (e, *p, s.kind.clone()))
            .collect();
        let by_tile: HashMap<(i32, i32), (Entity, &StructureId)> = placed
            .iter()
            .map(|(e, p, k)| ((p.x, p.y), (*e, k)))
            .collect();

        let db = self.world.resource::<StructureDb>();
        let items = self.world.resource::<ItemDb>();
        let mut edges: HashMap<Entity, Vec<(i32, i32)>> = HashMap::new();
        for (entity, pos, kind) in &placed {
            let Some(recipe) = db
                .get(kind)
                .and_then(|def| crate::systems::assembly_recipe(def, items))
            else {
                continue;
            };
            for (dx, dy) in crate::game::base::collect::ORTHOGONAL {
                let Some((neighbour, neighbour_kind)) = by_tile.get(&(pos.x + dx, pos.y + dy))
                else {
                    continue;
                };
                let feeds = db
                    .get(neighbour_kind)
                    .and_then(crate::systems::produced_item)
                    .is_some_and(|made| recipe.iter().any(|(want, _)| want == made));
                if !feeds {
                    continue;
                }
                edges.entry(*entity).or_default().push((dx, dy));
                edges.entry(*neighbour).or_default().push((-dx, -dy));
            }
        }
        for dirs in edges.values_mut() {
            dirs.sort();
            dirs.dedup();
        }
        edges
    }

    /// Every structure in the zone and every program assigned to it, for the
    /// roster screen.
    ///
    /// Deliberately unbounded where `view_entities` takes a radius: the base
    /// sits within `MAX_BUILD_DISTANCE_FROM_HOME` of its Home, but the player
    /// wanders, and a roster that thinned out as they walked away would be
    /// worse than none. There is no zone-local trimming to worry about
    /// either way — a `Structure` is base-space, not zone surface, and a
    /// breach leaves it exactly where it was (see `enter_next_zone`).
    ///
    /// Ordered Home first, then grouped by def id, then nearest first inside
    /// a group. Sorting here rather than in the frontend keeps one order for
    /// every consumer.
    pub fn structure_report(&mut self) -> Vec<StructureReport> {
        // `Game::scan_center`, not the player's `Position` — every
        // `Structure` this reports on lives in base space, and `Position`
        // stays pinned to the anchor tile for the whole of a base visit.
        // `Mode::StructureAssign` only ever opens while `in_base()`
        // (`app-core`'s `menus.rs`), which is exactly when `center` and the
        // player's real `Position` disagree.
        let center = self.scan_center();
        let mut structures = self.world.query::<(Entity, &Structure, &Position)>();
        let found: Vec<(Entity, StructureId, Position)> = structures
            .iter(&self.world)
            .map(|(e, s, p)| (e, s.kind.clone(), *p))
            .collect();

        // Grouped by target rather than mapped from it: a cronjob worker and
        // a guard can be posted on the same structure, and the roster exists
        // to show both.
        let mut assignees_by_structure: HashMap<Entity, Vec<Assignee>> = HashMap::new();
        let mut tasks = self.world.query::<(Entity, &Task)>();
        let posted: Vec<(Entity, Entity, TaskKind, u32, u32)> = tasks
            .iter(&self.world)
            .map(|(worker, task)| (worker, task.target, task.kind, task.progress, task.required))
            .collect();
        for (worker, target, kind, progress, required) in posted {
            assignees_by_structure
                .entry(target)
                .or_default()
                .push(Assignee {
                    entity: worker,
                    label: self.entity_label(worker),
                    kind,
                    progress,
                    required,
                    level: self.world.get::<Experience>(worker).map(|e| e.level),
                    hp: self.world.get::<Stats>(worker).map(|s| (s.hp, s.max_hp)),
                });
        }

        let mut report: Vec<StructureReport> = found
            .into_iter()
            .map(|(entity, kind, pos)| {
                let workable = self.accepts_a_program(entity);
                let named = |map: Option<&std::collections::BTreeMap<ItemId, u32>>| {
                    map.map(|m| {
                        m.iter()
                            .map(|(item, n)| (self.item_name(item).to_string(), *n))
                            .collect()
                    })
                    .unwrap_or_default()
                };
                let stock = self.world.get::<Stock>(entity);
                StructureReport {
                    input: named(stock.map(|s| &s.input)),
                    output: named(stock.map(|s| &s.output)),
                    output_capacity: stock.map(|s| s.capacity).unwrap_or(0),
                    status: self.world.get::<MachineStatus>(entity).copied(),
                    entity,
                    is_home: kind == HOME_STRUCTURE_ID,
                    kind,
                    label: self.entity_label(entity),
                    pos: (pos.x, pos.y),
                    distance: (pos.x - center.x).abs().max((pos.y - center.y).abs()),
                    tier: self.world.get::<StructureTier>(entity).map(|t| t.0),
                    durability: self
                        .world
                        .get::<Durability>(entity)
                        .map(|d| (d.hp, d.max_hp)),
                    workable,
                    player_adjacent: at_station(center, pos),
                    assignees: assignees_by_structure.remove(&entity).unwrap_or_default(),
                }
            })
            .collect();
        report.sort_by(|a, b| {
            b.is_home
                .cmp(&a.is_home)
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.distance.cmp(&b.distance))
                .then_with(|| a.pos.cmp(&b.pos))
        });
        report
    }

    /// The base's `Grid` header, as `(draw, supply)`.
    ///
    /// Calls `game::base::power::ledger` directly rather than reading
    /// `resources::PowerGrid` — the per-tick cache `systems::power_grid_system`
    /// fills at the head of the base chain. That keeps this correct on the
    /// very first frame after a load, before any tick has run and while the
    /// resource still holds its `Default`. Iterating a base's handful of
    /// structures once per frame is not a cost worth optimising ahead of
    /// evidence.
    ///
    /// `PowerLedger` itself stays engine-internal — its `dark` set holds
    /// `Entity`, which is not the renderer's business. A machine's own dark
    /// state reaches the gui the same way any other status does, through
    /// `EntityView::machine_status` and `MachineStatus::Unpowered`.
    pub fn base_power(&self) -> (u32, u32) {
        let db = self.world.resource::<StructureDb>();
        let ledger = crate::game::base::power::ledger(&self.world, db);
        (ledger.draw, ledger.supply)
    }

    /// A species' affinities, or `None` if no such species loaded.
    pub fn species_affinities(&self, id: &str) -> Option<Affinities> {
        self.world
            .resource::<SpeciesDb>()
            .get(id)
            .map(|s| s.affinities)
    }

    /// Everything known about one subject, for the manifest screen. Works on
    /// the player and on any creature — wild, owned, or in the party.
    /// Read-only: looking a program over never triggers an intrusion.
    ///
    /// `None` for anything that is neither (a structure, a nest, a despawned
    /// entity), or for a creature whose species failed to resolve.
    pub fn manifest(&self, entity: Entity) -> Option<ManifestView> {
        if self.world.get::<Player>(entity).is_some() {
            return self.player_manifest(entity);
        }
        self.program_manifest(entity)
    }

    fn player_manifest(&self, entity: Entity) -> Option<ManifestView> {
        let stats = self.world.get::<Stats>(entity)?;
        let needs = self.world.get::<PowerReserve>(entity)?;
        let pos = self.world.get::<Position>(entity)?;
        let inv = self.world.get::<Inventory>(entity)?;
        let exp = self.world.get::<Experience>(entity)?;
        let glyph = self.world.get::<Glyph>(entity)?;
        // The same calls `player_status` makes, so the sidebar and the sheet
        // cannot show different numbers for the same player.
        let atk = self.effective_atk(entity);
        let mitigation = self.effective_mitigation(entity);
        let perks = self.world.get::<Perks>(entity);
        Some(ManifestView {
            entity,
            name: "You".to_string(),
            glyph: glyph.ch,
            color: glyph.color,
            level: Some(exp.level),
            xp: Some((exp.xp, exp.xp_to_next)),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk,
            mitigation,
            damage: self.damage_range_label(self.natural_range_of(entity)),
            // The same scalar `Stats::power` computes, over the player's
            // *effective* numbers rather than their raw ones.
            power: Stats {
                atk,
                mitigation,
                ..*stats
            }
            .power(),
            accuracy: self.manifest_accuracy(entity),
            evasion: self.manifest_evasion(entity),
            status_effect: self.status_label(entity),
            routines: self.routine_view(entity),
            equipment: self.worn_slots(entity),
            subject: ManifestSubject::Player(PlayerManifest {
                power: needs.get(),
                decompiler: self
                    .world
                    .get::<Decompiler>(entity)
                    .map(|d| d.skill)
                    .unwrap_or(0),
                perk_points: perks.map(|p| p.points).unwrap_or(0),
                perks: perks
                    .map(|p| {
                        let db = self.world.resource::<PerkDb>();
                        Perk::all()
                            .into_iter()
                            .map(|perk| (perk, p.level(perk)))
                            .filter(|(_, level)| *level > 0)
                            .filter_map(|(perk, level)| Some((db.get(perk)?.name.clone(), level)))
                            .collect()
                    })
                    .unwrap_or_default(),
                position: (pos.x, pos.y),
                zone: self.world.resource::<ZoneLevel>().0,
                pet_count: self.pet_count(),
                pet_capacity: self.pet_capacity(),
                cargo_used: inv.cargo_used(self.world.resource::<ItemDb>()),
                party: self.party_info(),
                credits: self.banked(&crate::items::ids::CREDITS.into()),
                portal_fragments: self.banked(&crate::items::ids::PORTAL_FRAGMENT.into()),
                difficulty: *self.world.resource::<DifficultyMode>(),
                cycle: self.current_tick(),
                active_contracts: self.active_contracts().len(),
            }),
        })
    }

    /// The Accuracy this combatant brings to an attack roll, through the
    /// one derivation `battle::resolve_attack` consults. A call and not a
    /// copy: the sheet exists to say what the fight will do, so a second
    /// expression of the formula here is the drift this repo has already
    /// paid for four times.
    fn manifest_accuracy(&self, entity: Entity) -> f32 {
        crate::battle::accuracy_of(
            self.combat_speed(entity),
            self.ability_user_level(entity),
            self.gear_bonus(entity).accuracy,
        ) as f32
    }

    /// See `manifest_accuracy`.
    fn manifest_evasion(&self, entity: Entity) -> f32 {
        crate::battle::evasion_of(
            self.combat_speed(entity),
            self.ability_user_level(entity),
            self.gear_bonus(entity).evasion,
        ) as f32
    }

    /// Every occupied equipment slot on `wearer`, in `EquipmentSlot::ALL`
    /// order. Empty for anything with no `Equipment` component, which is
    /// what a wild program — and an owned one that has never been geared —
    /// looks like.
    fn worn_slots(&self, wearer: Entity) -> Vec<ManifestEquipSlot> {
        let Some(equipment) = self.world.get::<Equipment>(wearer) else {
            return Vec::new();
        };
        EquipmentSlot::ALL
            .into_iter()
            .filter_map(|slot| self.manifest_equip_slot(slot, equipment.get(slot)?))
            .collect()
    }

    /// One worn item as the manifest lists it. `None` if the item's
    /// definition has gone missing (a mod removed since the save was
    /// written), which drops the row rather than failing the whole sheet.
    fn manifest_equip_slot(
        &self,
        slot: EquipmentSlot,
        worn: EquippedItem,
    ) -> Option<ManifestEquipSlot> {
        // Through `worn_bonus` rather than scaling here, so the sheet cannot
        // quote a figure the wearer's `Stats` disagree with — this was a
        // second copy of the chain, and the copy nobody runs is the one that
        // drifts.
        let mods = self.worn_bonus(&worn)?;
        Some(ManifestEquipSlot {
            slot: slot.label().to_string(),
            item_name: self.copy_name(&worn.copy),
            gear_level: worn.level,
            fusion_tier: worn.copy.tier,
            atk: mods.atk,
            mitigation: mods.mitigation,
            decompiler: mods.decompiler,
        })
    }

    fn program_manifest(&self, entity: Entity) -> Option<ManifestView> {
        let creature = self.world.get::<Creature>(entity)?;
        let species = self.world.resource::<SpeciesDb>().get(&creature.species)?;
        let stats = self.world.get::<Stats>(entity)?;
        let exp = self.world.get::<Experience>(entity);
        let is_tamed = self.world.get::<Tamed>(entity).is_some();
        let custom = self.world.get::<CustomName>(entity).map(|c| c.0.clone());
        let bonuses = self.player_decompiler_bonuses();
        Some(ManifestView {
            entity,
            name: match &custom {
                Some(name) => name.clone(),
                None => self.zone_tagged_name(entity, species.name.clone()),
            },
            glyph: species.glyph,
            color: species.color,
            level: exp.map(|e| e.level),
            xp: exp.map(|e| (e.xp, e.xp_to_next)),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk: stats.atk,
            mitigation: stats.mitigation,
            damage: self.damage_range_label(self.natural_range_of(entity)),
            power: stats.power(),
            accuracy: self.manifest_accuracy(entity),
            evasion: self.manifest_evasion(entity),
            status_effect: self.status_label(entity),
            routines: self.routine_view(entity),
            equipment: self.worn_slots(entity),
            subject: ManifestSubject::Program(Box::new(ProgramManifest {
                species_name: custom
                    .is_some()
                    .then(|| self.zone_tagged_name(entity, species.name.clone())),
                is_hostile: self.world.get::<Hostile>(entity).is_some(),
                is_tamed,
                is_companion: self.world.resource::<Party>().0.contains(&entity),
                is_boss: self.is_boss_creature(entity),
                activity: is_tamed.then(|| self.program_activity(entity)),
                post: self.program_post(entity),
                potential: self
                    .world
                    .get::<Potential>(entity)
                    .map(|p| ManifestPotential {
                        hp_roll: p.hp_roll,
                        atk_roll: p.atk_roll,
                        def_roll: p.def_roll,
                        growth_roll: p.growth_roll,
                        percent: p.quality_percent(),
                        label: p.quality_label().to_string(),
                    }),
                fusions: self.fusion_count(entity),
                max_fusions: MAX_FUSIONS,
                rarity: self.rarity_of(entity),
                refactors: self.refactor_count(entity),
                max_refactors: MAX_COMPANION_REFACTORS,
                ring: self.world.get::<KernelRing>(entity).map_or(0, |r| r.0),
                max_ring: crate::tuning::KERNEL_RING_MAX,
                level_cap: self.companion_level_cap(entity),
                talents_spent: self.talent_points(entity).spent,
                talents_earned: self.talent_points(entity).earned,
                zone_tier: self.zone_tier(entity),
                player_zone: self.world.resource::<ZoneLevel>().0,
                habitats: species.habitats.clone(),
                moves: species.moves.clone(),
                work_resource: species.work_resource.clone(),
                taming_difficulty: species.taming_difficulty,
                decompile_chance: self
                    .taming_catalyst()
                    .zip(self.target_resistance(entity))
                    .map(|((_, potency), resistance)| {
                        taming::capture_chance(potency, resistance, bonuses)
                    }),
                growth_multiplier: species.growth_multiplier,
                base_speed: species.base_speed,
                base_int: species.base_int,
                affinities: species.affinities.non_neutral(),
                base_job: species.affinity_class(),
            })),
        })
    }

    /// What `creature` is worth at a post — see `views::WorkProfile`.
    ///
    /// `None` when the species is not in the db, which in play means a mod
    /// that failed to load. Quoting the roster's baseline numbers for it
    /// would be inventing them, and the caller can say "unknown" far better
    /// than this can guess.
    ///
    /// The manifest reads the same three off a `&SpeciesDef` it already has
    /// in hand rather than calling this, and that is not a copy worth
    /// closing: these are field reads, not a formula, so there is nothing
    /// here that can drift out of step with them.
    pub fn work_profile(&self, creature: Entity) -> Option<WorkProfile> {
        let species = &self.world.get::<Creature>(creature)?.species;
        let def = self.world.resource::<SpeciesDb>().get(species)?;
        Some(WorkProfile {
            speed: def.base_speed,
            analysis: def.base_int,
            class: def.affinity_class(),
        })
    }

    /// Every deployed structure that's a symlink target (its def has
    /// `teleport_cost` set), anywhere on the map — unlike `view_entities`,
    /// this isn't limited to a scan radius, since the whole point of a
    /// symlink is reaching it from far away.
    pub fn symlink_targets(&mut self) -> Vec<EntityView> {
        let mut query = self
            .world
            .query::<(Entity, &Position, &Glyph, &Structure)>();
        let hits: Vec<(Entity, Position, Glyph, StructureId)> = query
            .iter(&self.world)
            .map(|(e, p, g, s)| (e, *p, *g, s.kind.clone()))
            .collect();

        let db = self.world.resource::<StructureDb>();
        let mut views: Vec<EntityView> = hits
            .into_iter()
            .filter(|(_, _, _, kind)| db.get(kind).is_some_and(|d| d.teleport_cost.is_some()))
            .map(|(entity, pos, glyph, kind)| {
                let bounds = self.entity_upgrade_ceiling(entity);
                EntityView {
                    entity,
                    pos: (pos.x, pos.y),
                    glyph: glyph.ch,
                    color: glyph.color,
                    label: self.entity_label(entity),
                    is_player: false,
                    is_tamed: false,
                    is_companion: false,
                    is_hostile: false,
                    is_structure: true,
                    machine_status: None,
                    linked_edges: Vec::new(),
                    is_home: kind == HOME_STRUCTURE_ID,
                    tier: self.world.get::<StructureTier>(entity).map(|t| t.0),
                    ceiling: bounds.map(|(c, _)| c),
                    max_tier: bounds.map(|(_, m)| m),
                    is_boss: false,
                    // Structures only, so there is no creature here to have
                    // beaten the party.
                    nemesis: false,
                    can_work: false,
                    can_trade: false,
                    issues_contracts: false,
                    structure_worker: None,
                    worker_away_from_post: false,
                    position_is_honest: true,
                    structure_attended: false,
                    output_stranded: false,
                    hp_fraction: None,
                    level: None,
                    durability: self
                        .world
                        .get::<Durability>(entity)
                        .map(|d| (d.hp, d.max_hp)),
                    fusions: 0,
                    // Structures only, so there is no creature here to have
                    // rolled a tier.
                    rarity: Rarity::Ordinary,
                }
            })
            .collect();
        Self::sort_by_label(&mut views);
        views
    }

    /// The item cost to symlink to `target`, if it's a symlink-capable
    /// structure — used both by `use_symlink` itself and by the renderer to
    /// show the cost before the player commits to it.
    pub fn symlink_cost(&self, target: Entity) -> Option<Vec<(ItemId, u32)>> {
        let kind = self.world.get::<Structure>(target)?.kind.clone();
        self.world
            .resource::<StructureDb>()
            .get(&kind)
            .and_then(|d| d.teleport_cost.clone())
    }

    /// "Use symlink" — instantly teleports the player to `target` (a
    /// symlink-capable structure from `symlink_targets`), paying its
    /// `teleport_cost` from inventory.
    pub fn use_symlink(&mut self, target: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if self.world.get::<Structure>(target).is_none() {
            return Err("That's not a structure.".to_string());
        }
        let cost = self
            .symlink_cost(target)
            .ok_or_else(|| "That structure has no symlink.".to_string())?;
        // Where a symlink puts you down: the anchor, not the structure's own
        // tile. Every `Structure` stands in base space, so its `Position` is
        // in a coordinate space the player's `Position` is not — writing one
        // across to the other would drop the party on whatever happens to be
        // at those numbers out on the zone surface. The anchor is the base's
        // one presence there, and it is where a trip home ends now.
        //
        // Read before the cost is taken, with the other refusals: a symlink
        // that cannot land must not have been paid for.
        let target_pos = self
            .anchor_position()
            .map(|(x, y)| Position { x, y })
            .ok_or_else(|| "There's no anchor in this sector to link to.".to_string())?;
        let player = self.player_entity();
        {
            let inv = self.world.get::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                if inv.count(item) < *qty {
                    return Err(format!("Not enough {}.", self.item_name(item)));
                }
            }
        }
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                inv.take(item.clone(), *qty);
            }
        }
        let name = self.entity_label(target);
        // Only once every check above has passed: a symlink that is refused
        // must not have surfaced the party on its way to refusing. Doing it
        // here also keeps `Position` from ever being written while
        // `Locale::Stack` is live, which is what `require_surface` guards
        // the other actions against — underground it *is* the entrance tile.
        let surfaced = self.is_underground();
        if surfaced {
            self.clear_stack();
        }
        {
            let mut pos = self.world.get_mut::<Position>(player).unwrap();
            pos.x = target_pos.x;
            pos.y = target_pos.y;
        }
        self.log(if surfaced {
            format!("The symlink hauls you up out of the stack and drops you at {name}'s anchor.")
        } else {
            format!("You use a symlink and teleport to {name}'s anchor.")
        });
        self.tick();
        Ok(())
    }

    /// The denominator of every `power_ratio` reading. The `unwrap` is the
    /// player entity always carrying `Stats` — the same invariant the map
    /// coloring has always relied on here.
    pub(crate) fn player_power(&self) -> i32 {
        self.world
            .get::<Stats>(self.player_entity())
            .unwrap()
            .power()
    }
}

/// How outmatched the player is by one creature, as its `Stats::power` over
/// theirs — the single reading two systems share. `difficulty_color` buckets
/// it into the con colors drawn on the map, and `Game::target_resistance`
/// hands it to `taming::capture_chance`, whose two power ramps are bounded by
/// the same `DIFFICULTY_*` thresholds this is bucketed against. One function
/// rather than two divisions, so the color on a program and the decompile
/// odds against it can never come to different conclusions about which of the
/// two is stronger.
///
/// `player_power` is floored at 1 rather than guarded by the caller: the one
/// value that would divide by zero is a dead player, and every caller here
/// runs while they are alive.
pub(crate) fn power_ratio(creature_power: i32, player_power: i32) -> f64 {
    creature_power as f64 / player_power.max(1) as f64
}

/// Old-school "con"-style map coloring for a hostile wild program, relative
/// to the player's current `Stats::power`. A nemesis is always Blue
/// regardless of the ratio, checked *before* the boss override so a
/// creature that is both draws as a nemesis — see `views::EntityView::rarity`
/// for the argument that spending the con read here is deliberate. Blue was
/// the one `GlyphColor` variant nothing else on the map was painted — Cyan
/// was tried first and rejected, because it is the player's own glyph colour
/// (`lifecycle.rs`'s `Game::new`/`load`), and a nemesis tile has no business
/// reading as the player. A boss that isn't a nemesis is always Magenta
/// regardless of the ratio; everything else runs Green (easy) → Yellow
/// (even) → Orange (tough) → Red (hard) as `creature_power` grows past
/// `player_power`. Pulled out of `view_entities` so the bucketing is
/// unit-testable without spinning up a `Game`.
pub(crate) fn difficulty_color(
    creature_power: i32,
    player_power: i32,
    is_boss: bool,
    is_nemesis: bool,
) -> GlyphColor {
    if is_nemesis {
        return GlyphColor::Blue;
    }
    if is_boss {
        return GlyphColor::Magenta;
    }
    let ratio = power_ratio(creature_power, player_power);
    if ratio <= DIFFICULTY_EASY_MAX {
        GlyphColor::Green
    } else if ratio <= DIFFICULTY_EVEN_MAX {
        GlyphColor::Yellow
    } else if ratio <= DIFFICULTY_TOUGH_MAX {
        GlyphColor::Orange
    } else {
        GlyphColor::Red
    }
}
