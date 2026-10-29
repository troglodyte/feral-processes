//! The base's own verbs on the field: the dig, floor, finish and stockpile
//! marks, the dig crew that works them, rock describing and mining mode, and
//! the cosmetic transit walk. The base has no space of its own (ADR 0032);
//! everything here is in field coordinates.

use crate::base_grid::BaseGrid;
use crate::components::Fleeing;
use crate::*;

/// The cells a body walks through to get from `from` to `to` inside base
/// space — `from` first, `to` last — or nothing at all when there is no walk
/// between them.
///
/// Cosmetic, and its callers are the sortie departure/arrival cue and a
/// caravan route's — `Game::queue_transit_walk`. Two things about it are
/// deliberate.
///
/// It pathfinds on laid floor **alone and admits no blocked set**,
/// unlike `hauling::post_field`, which refuses tiles other things stand on.
/// The anchor is where the Home stands, so a walk that refused occupied
/// tiles could never reach its own destination. A walker ghosting past a
/// machine for a fraction of a second is invisible; one ghosting through rock
/// reads as a rendering fault, which is the whole reason a path is computed
/// rather than the two ends being interpolated between.
///
/// And a walk that does not exist is **nothing**, never a straight line: a
/// body whose tile cannot be stood on has no walk to show.
pub(crate) fn transit_path(grid: &BaseGrid, from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    if !grid.is_floor(from.0, from.1) || !grid.is_floor(to.0, to.1) {
        return Vec::new();
    }
    if from == to {
        return vec![from];
    }
    let reach = crate::tuning::haul_walk_radius(grid.radius());
    let field =
        crate::game::pursuit::walk_field(to, reach, |p| grid.is_floor(p.0, p.1).then_some(1));
    let Some(&start) = field.get(&from) else {
        return Vec::new();
    };
    let mut path = vec![from];
    let (mut here, mut cost) = (from, start);
    while here != to {
        // `min` over the whole tuple is a total order, `find_target_in_direction`'s
        // rule: cost first and the coordinate as the tie-break, so a body
        // offered two equally short ways round a machine takes the same one
        // every run.
        let Some((next, x, y)) = crate::world::NEIGHBOURS
            .iter()
            .map(|(dx, dy)| (here.0 + dx, here.1 + dy))
            .filter_map(|n| field.get(&n).map(|&c| (c, n.0, n.1)))
            .min()
            .filter(|&(next, ..)| next < cost)
        else {
            return Vec::new();
        };
        (here, cost) = ((x, y), next);
        path.push(here);
    }
    path
}

impl Game {
    /// Queues one `TransitCue` for a walk between two field cells, or
    /// queues nothing when there is no walk between them — `transit_path`'s
    /// own "a walk that does not exist is nothing" rule, carried through
    /// rather than re-checked here.
    ///
    /// The entity-free half of what `sortie::queue_squad_walk` used to do
    /// whole: a sortie's cue reads a member's own glyph and tile off its
    /// components first, and a caravan route's cargo has neither, so this
    /// is the part both actually share.
    pub(crate) fn queue_transit_walk(
        &mut self,
        glyph: char,
        color: crate::components::GlyphColor,
        from: (i32, i32),
        to: (i32, i32),
    ) {
        let path = transit_path(self.world.resource::<BaseGrid>(), from, to);
        if path.len() < 2 {
            return;
        }
        self.world
            .resource_mut::<crate::resources::TransitQueue>()
            .push(crate::resources::TransitCue { glyph, color, path });
    }
}

impl Game {
    /// Whether the run has a base at all — a Home standing.
    ///
    /// The one thing a frontend has to be able to ask about the base without
    /// being in it, and it exists because of what the answer gates: the
    /// anchor refuses entry without a Home, and deploying one is the single
    /// build permitted from the open grid (`Game::place_structure`). A build
    /// menu that hid its Deploy row off the base would hide the only way
    /// onto it, so the row asks this.
    ///
    pub fn has_home(&self) -> bool {
        self.has_structure(HOME_STRUCTURE_ID)
    }

    /// Moves the anchor — the base's one fixed point — onto field
    /// `(x, y)`.
    ///
    /// **One writer, two callers with nothing else in common**: founding a
    /// Home, which stands the door on the tile the player chose to found
    /// from, and `Game::enter_next_zone`, which carries a base that travels
    /// to the next sector's arrival point. Both *move* the entity rather
    /// than despawning and respawning it, so `resources::AnchorEntity` names
    /// the same anchor for the whole run — and written once because the
    /// three lines that do it read as trivial right up until one of them
    /// grows a second thing to keep in step.
    pub(crate) fn move_anchor_to(&mut self, x: i32, y: i32) {
        let anchor = self.world.resource::<AnchorEntity>().0;
        if let Some(mut pos) = self.world.get_mut::<Position>(anchor) {
            pos.x = x;
            pos.y = y;
        }
    }

    /// Every `DigSite` in the field, by the tile it stands on.
    ///
    /// The lookup `toggle_mark_box` walks its box against. Built once per
    /// press rather than per cell: bevy's `World::query_filtered` constructs
    /// a fresh `QueryState` and scans every entity on each call, so asking
    /// tile by tile made one committed 25x25 box cost 625 of those, times a
    /// linear scan that grows with the sites already standing.
    ///
    /// **The keys are in the field**, so this must never be indexed with a
    /// zone-surface coordinate — see `components::DigSite`.
    fn dig_sites_by_tile(&mut self) -> HashMap<(i32, i32), Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<DigSite>>();
        query
            .iter(&self.world)
            .map(|(e, p)| ((p.x, p.y), e))
            .collect()
    }

    /// The `DigSite` standing on field `(x, y)`, if the player has
    /// started on that wall or marked it.
    ///
    /// For a single tile. Anything walking a region wants
    /// `dig_sites_by_tile` instead, for the reason stated there.
    ///
    /// **Its `Position` is in the field**, so this must never be reached
    /// for with a zone-surface coordinate — see `components::DigSite`.
    pub(crate) fn dig_site_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<DigSite>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// What the first wall the player can see in `(dx, dy)` is made of, out
    /// to `range` — the examine ray's field answer.
    ///
    /// **Reads the same faces the map colours, and that is the point.** If
    /// the map hid a kind while `x` named it, the hiding would be decorative
    /// and prospecting would be dead on arrival. Here the agreement is
    /// structural rather than a shared predicate: the ray stops at the
    /// *first* solid cell, whose predecessor is walkable by construction, so
    /// every cell this can reach is an exposed face already.
    /// `the_map_and_the_examine_ray_agree_about_a_wall` pins it anyway,
    /// because "by construction" is exactly the kind of claim that stops
    /// being true when someone lets the ray run through rock.
    ///
    /// **A finish or pile underfoot is named first.** The cell one step along
    /// `(dx, dy)` — the ray's own first stop — is checked for a finish
    /// before the ray runs at all, since a finished floor is walkable and
    /// the ray would otherwise step straight over it looking for the wall
    /// beyond. A finish with no rock in range still answers `Some`, so
    /// examining a fully floored, fully finished room says something.
    pub fn describe_base_rock(&self, dx: i32, dy: i32, range: i32) -> Option<String> {
        let (bx, by) = self.field_pos()?;
        let finish_name = {
            let grid = self.world.resource::<BaseGrid>();
            grid.finish_at(bx + dx, by + dy).cloned()
        }
        .and_then(|id| {
            self.world
                .resource::<crate::floors::FloorDb>()
                .get(&id)
                .map(|def| def.name.clone())
        });

        // Underfoot, in the order the cell is read: its finish, then what
        // lies on it. Both are properties of the ray's first stop.
        let underfoot: Vec<String> = finish_name
            .map(|name| format!("{name} underfoot"))
            .into_iter()
            .chain(self.describe_floor_item(bx + dx, by + dy))
            .chain(
                self.world
                    .resource::<BaseGrid>()
                    .is_stockpile(bx + dx, by + dy)
                    .then(|| "a stockpile".to_string()),
            )
            .collect();
        // Capitalised here so a floor item that leads the line reads like the
        // other examine lines; a finish name already is.
        let underfoot = (!underfoot.is_empty()).then(|| {
            let line = underfoot.join("; ");
            let mut chars = line.chars();
            chars.next().map_or(line.clone(), |c| {
                c.to_uppercase().chain(chars).collect::<String>()
            })
        });

        let (mut x, mut y) = (bx, by);
        for _ in 0..range {
            x += dx;
            y += dy;
            if !self.ground().walkable(x, y) {
                let biome = self.world.resource::<WorldMap>().peek(x, y).biome;
                let def = self.world.resource::<crate::rock::RockDb>().kind_at(biome);
                let rock_line = format!(
                    "{}. It takes at least {} swings to cut through.",
                    def.name, def.min_swings
                );
                return Some(match &underfoot {
                    Some(line) => format!("{line}; {rock_line}"),
                    None => rock_line,
                });
            }
        }
        underfoot.map(|line| format!("{line}."))
    }

    /// Whether the player's step into solid rock cuts it. See
    /// `resources::MiningMode`.
    pub fn mining(&self) -> bool {
        self.world.resource::<crate::resources::MiningMode>().0
    }

    /// Arms or disarms the player's own bump, and returns what it now is.
    ///
    /// Costs no turn and draws nothing: picking a tool up is not an action,
    /// and the caller `return`s rather than falling through to the tick.
    pub fn toggle_mining(&mut self) -> bool {
        let mut mode = self.world.resource_mut::<crate::resources::MiningMode>();
        mode.0 = !mode.0;
        let armed = mode.0;
        if armed {
            self.log("Cutting tools out. Walking into entropy will cut it.");
        } else {
            self.log("Cutting tools away. Walking into entropy will not cut it.");
        }
        armed
    }

    /// What the rock at `(x, y)` is: how much it absorbs, the most one swing
    /// may take off it, and how bright its face is drawn — keyed by the
    /// field tile's biome (`RockDb::kind_at`).
    ///
    /// Answers for solid and open cells alike: a kind is a property of the
    /// place, and the place does not stop being made of something once it
    /// has been cut.
    pub(crate) fn wall_at(&self, x: i32, y: i32) -> crate::rock::Wall {
        let biome = self.world.resource::<WorldMap>().peek(x, y).biome;
        self.world.resource::<crate::rock::RockDb>().wall_at(biome)
    }

    /// One tick of cutting at `(x, y)`, for the ear.
    ///
    /// **The unit is a tick of work, not a swing**, and that is what makes
    /// the player's bump and a posted digger sound like the same activity: a
    /// bump spends one tick and lands a swing, while a crew spends
    /// `BASE_DIG_TICKS_PER_SWING` of them for the same swing. Cueing the
    /// swing alone left a crew chipping once every six real seconds with the
    /// cell's own progress bar moving every tick, which read as the sound
    /// being broken rather than as the crew being unhurried.
    ///
    /// Two callers, because the two sides reach a tick of cutting
    /// differently — `strike_rock` for the tick a swing lands on,
    /// `run_dig_crew` for the eleven that only move the meter. Exactly one
    /// of them fires per tick; `the_tick_a_crew_swing_lands_cues_once` is
    /// the seam between them.
    fn cue_mining(&mut self, x: i32, y: i32) {
        self.world
            .resource_mut::<crate::resources::EffectQueue>()
            .push((x, y), EffectKind::Mine);
    }

    /// One swing of `swinger`'s damage at the rock cell `(x, y)`, spawning
    /// the `DigSite` that records progress if this is the first. Returns the
    /// site when the swing broke the cell and `None` while it still stands.
    ///
    /// Shared by the field strike and the field strike
    /// (`Game::strike_field_rock`), so rock wears down by one rule in both.
    pub(crate) fn swing_at_rock(
        &mut self,
        swinger: Entity,
        x: i32,
        y: i32,
        wall: crate::rock::Wall,
    ) -> Option<Entity> {
        let by_player = swinger == self.player_entity();
        // Capped, and this is the whole of the fix for a developed player
        // demolishing their own base by clipping a corner. `swing_damage`
        // grows all run against rock that does not, so uncapped every cell
        // eventually fell to one bump. The cap is per kind and
        // level-independent: levelling still cuts the swing count down to
        // the kind's floor, it simply cannot reach one.
        let dmg = self.swing_damage(swinger).min(wall.swing_cap);
        let site = self.dig_site_at(x, y).unwrap_or_else(|| {
            self.world
                .spawn((
                    DigSite::default(),
                    Durability {
                        hp: wall.durability,
                        max_hp: wall.durability,
                    },
                    Position { x, y },
                ))
                .id()
        });
        let mut durability = self.world.get_mut::<Durability>(site)?;
        durability.hp = durability.hp.saturating_sub(dmg);
        let still_standing = durability.hp > 0;
        // Above the break/no-break split deliberately: the swing that opens
        // a cell is still one tick of cutting, and the break has its own
        // line to announce itself with.
        self.cue_mining(x, y);
        if still_standing {
            if by_player {
                self.log(format!("You cut into the entropy for {dmg} damage."));
            }
            return None;
        }
        Some(site)
    }

    /// The fragment a broken rock cell may return, at `ROCK_FRAGMENT_CHANCE`.
    pub(crate) fn roll_rock_fragment(&mut self, by_player: bool) {
        // A live action, not world generation, so `GameRng` is the right
        // stream to draw from: nothing here has to be reproduced by a
        // reload.
        let paid = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0
                .random_bool(crate::tuning::ROCK_FRAGMENT_CHANCE.clamp(0.0, 1.0) as f64)
        };
        if paid {
            let item = ItemId::from(crate::items::ids::CORE_FRAGMENT);
            let landed = self.grant_loot(item.clone(), 1, LootSource::Rock);
            if landed > 0 {
                let line = format!("A {} shakes loose from the cut.", self.item_name(&item));
                if by_player {
                    self.log_kind(MessageKind::Loot, line);
                } else {
                    self.log_base_kind(MessageKind::Loot, line);
                }
            }
        }
    }

    /// Marks — or clears — every cell in the inclusive box spanned by `a`
    /// and `b`.
    ///
    /// **The anchor decides which of the two it does**: a box whose `a` cell
    /// is already marked clears, an unmarked one marks. That is the whole of
    /// why there is no second erase verb — settled decision 4 — and it is
    /// also why the anchor is read *before* anything in the box is written.
    /// Clearing ignores `brush` entirely: whatever kind of mark a cell in the
    /// box carries, clearing removes it.
    ///
    /// The box is normalised rather than assumed ordered: a plan drawn
    /// up-left is the same plan drawn down-right, and a cursor the player
    /// dragged backwards is the ordinary case rather than the corner one.
    ///
    /// `brush` is the excavate screen's current tool: `None` is the plain
    /// cut-or-tile brush, which takes solid and open cells and skips laid
    /// floor exactly as before this feature; `Some` is a finish or a strip,
    /// which take laid floor and skip everything else — `set_mark`'s table.
    pub fn toggle_mark_box(&mut self, a: (i32, i32), b: (i32, i32), brush: Option<&FinishOrder>) {
        let mut sites = self.dig_sites_by_tile();
        let marking = !self.is_marked_in(&sites, a.0, a.1);
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        // Floor must join the base, so a pass can unlock cells an earlier
        // cell in the box needed first; repeat until a pass marks nothing new.
        loop {
            let before = sites.len();
            for y in y0..=y1 {
                for x in x0..=x1 {
                    self.set_mark(&mut sites, x, y, marking, brush);
                }
            }
            if !(marking && brush.is_none()) || sites.len() == before {
                break;
            }
        }
    }

    /// Zones — or un-zones — every laid-floor cell in the inclusive box
    /// spanned by `a` and `b`.
    ///
    /// **The anchor decides**, the rule `toggle_mark_box` states: a box whose
    /// `a` cell is already a stockpile clears, otherwise it marks. Marking
    /// skips anything that is not laid floor. The box is normalised.
    pub fn toggle_stockpile_box(&mut self, a: (i32, i32), b: (i32, i32)) {
        let mut grid = self.world.resource_mut::<BaseGrid>();
        let marking = !grid.is_stockpile(a.0, a.1);
        for y in a.1.min(b.1)..=a.1.max(b.1) {
            for x in a.0.min(b.0)..=a.0.max(b.0) {
                grid.set_stockpile(x, y, marking);
            }
        }
    }

    /// Every stockpile cell, in `(x, y)` order; empty outside the field.
    pub fn stockpile_cells(&self) -> Vec<(i32, i32)> {
        if self.is_underground() {
            return Vec::new();
        }
        self.world
            .resource::<BaseGrid>()
            .stockpile_cells()
            .collect()
    }

    /// Every marked cell and how far through it the crew is, in `(x, y)`
    /// order — see `views::DigMark`.
    ///
    /// Sorted rather than handed back in query order for the reason `Stock`
    /// keys by `BTreeMap`: bevy's iteration order is not stable, and the
    /// renderer draws these in the order it gets them. By `pos` alone, since
    /// the meter beside it is an `f32` and has no business in an ordering.
    ///
    /// **The meter is the wall's own `Durability`, not the digger's `Task`.**
    /// A digger's `Task::progress` is the swing *cadence* and resets to zero
    /// every swing, so a bar drawn off it would sweep full and empty again
    /// between chips while the cut itself barely moved.
    pub fn marked_cells(&self) -> Vec<DigMark> {
        self.marked_sites()
            .into_iter()
            .map(|(mark, _)| mark)
            .collect()
    }

    /// The marks that are plain digs: cut it, or tile it. `marked_cells`
    /// less the finish marks, which `floor_marks` lists instead, so the
    /// Digging and Floors screens never both show a cell.
    pub fn dig_marks(&self) -> Vec<DigMark> {
        self.marked_sites()
            .into_iter()
            .filter(|(_, finish)| finish.is_none())
            .map(|(mark, _)| mark)
            .collect()
    }

    /// Cells marked to be finished, or stripped, in `(x, y)` order.
    pub fn floor_marks(&self) -> Vec<FloorMark> {
        let floors = self.world.resource::<crate::floors::FloorDb>();
        self.marked_sites()
            .into_iter()
            .filter_map(|(mark, finish)| {
                let finish = finish?;
                Some(FloorMark {
                    pos: mark.pos,
                    finish: match finish {
                        FinishOrder::Strip => None,
                        FinishOrder::Apply(id) => Some(
                            floors
                                .get(&id)
                                .map_or_else(|| id.0.clone(), |d| d.name.clone()),
                        ),
                    },
                })
            })
            .collect()
    }

    /// Every marked cell with the finish it carries: the one list the three
    /// readers above filter, so a mark is a dig or a floor by one test.
    fn marked_sites(&self) -> Vec<(DigMark, Option<FinishOrder>)> {
        let ground = self.ground();
        let mut marks: Vec<(DigMark, Option<FinishOrder>)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let site = e.get::<DigSite>().filter(|d| d.marked)?;
                let p = e.get::<Position>()?;
                let dur = e.get::<Durability>()?;
                Some((
                    DigMark {
                        pos: (p.x, p.y),
                        // Only a cell that still *is* a wall has a wall left
                        // to report. A marked `Open` cell is the other half
                        // of the one dig verb - floor it - and carries its
                        // old wall's `max_hp` against an `hp` of zero, which
                        // would read as a finished bar sitting on a job that
                        // has not started.
                        cut: (!ground.walkable(p.x, p.y) && dur.max_hp > 0)
                            .then(|| 1.0 - dur.hp as f32 / dur.max_hp as f32),
                    },
                    site.finish.clone(),
                ))
            })
            .collect();
        marks.sort_unstable_by_key(|(m, _)| m.pos);
        marks
    }

    /// Whether field `(x, y)` is marked. A cell with no `DigSite` is
    /// not, which is what makes an untouched wall the unmarked case without
    /// storing anything for it.
    fn is_marked_in(&self, sites: &HashMap<(i32, i32), Entity>, x: i32, y: i32) -> bool {
        sites
            .get(&(x, y))
            .and_then(|&site| self.world.get::<DigSite>(site))
            .is_some_and(|d| d.marked)
    }

    /// Writes one cell's mark, spawning or retiring its `DigSite` as needed.
    ///
    /// The durability a *marked* site is born with is the cell's own state:
    /// a solid cell has the whole wall left to cut, an already-open or
    /// already-floored one has none — its mark means floor it, or finish it.
    /// Clearing retires the site unless it is still holding chip progress, so
    /// an unmarked wall the player had started on does not heal.
    ///
    /// **`brush` decides both which cells this pass may touch and what it
    /// writes**, exactly the design's table: `None` marks solid and open
    /// cells for cutting or tiling and skips laid floor, `Some(Apply(id))`
    /// marks floor that does not already wear `id` and skips everything
    /// else, `Some(Strip)` marks floor that wears *some* finish and skips a
    /// bare one. A cell the brush has nothing to say about is left exactly
    /// as it was — no site spawned, no existing one touched.
    fn set_mark(
        &mut self,
        sites: &mut HashMap<(i32, i32), Entity>,
        x: i32,
        y: i32,
        marked: bool,
        brush: Option<&FinishOrder>,
    ) {
        let solid = !self.ground().walkable(x, y);
        // A dig mark is a floor request, so it is refused wherever floor is.
        if marked && brush.is_none() && self.floor_conflict(x, y).is_some() {
            return;
        }
        let grid = self.world.resource::<BaseGrid>();
        let floor = grid.is_floor(x, y);

        // The order to write onto the mark, or `None` to leave this cell
        // untouched — decided from the *committed* grid, never from a
        // pending `DigSite`, so replacing a finish always targets what is
        // actually laid rather than a job still queued behind it.
        let order = if marked {
            match brush {
                None => {
                    if floor {
                        return;
                    }
                    // A dig mark on a blocker is the dig request itself, so
                    // only adjacency can refuse it here.
                    let pending = self.pending_floor();
                    if crate::game::base::building::floor_request_refusal(
                        (x, y),
                        self.ground(),
                        &pending,
                        true,
                    )
                    .is_some()
                    {
                        return;
                    }
                    None
                }
                Some(FinishOrder::Apply(id)) => {
                    if !floor || grid.finish_at(x, y) == Some(id) {
                        return;
                    }
                    Some(FinishOrder::Apply(id.clone()))
                }
                Some(FinishOrder::Strip) => {
                    if !floor || grid.finish_at(x, y).is_none() {
                        return;
                    }
                    Some(FinishOrder::Strip)
                }
            }
        } else {
            None
        };

        match sites.get(&(x, y)).copied() {
            Some(site) => {
                if let Some(mut dig) = self.world.get_mut::<DigSite>(site) {
                    dig.marked = marked;
                    // Overwrites whatever the site's `finish` carried —
                    // clearing writes `None` through `order` above, and
                    // marking with a different brush replaces it, so a site
                    // never keeps a stale order for a kind it is no longer
                    // asked to be.
                    dig.finish = order;
                }
                // An unmarked site earns its keep by holding chip progress,
                // and *both* ends of the meter hold none. A full meter is a
                // wall nobody has touched; a spent one on a cell that is
                // still solid is stale — nothing in play produces that state
                // any more, since open ground never reverts, but a leftover
                // from an older save should not linger forever either.
                // Keeping either leaves an entity drawn nowhere, wanted by
                // nobody, and written to every save from then on. A finish
                // or strip mark never holds chip progress — there is no rock
                // left to cut — so clearing one always despawns it.
                let holds_progress = self
                    .world
                    .get::<Durability>(site)
                    .is_some_and(|d| d.hp > 0 && d.hp < d.max_hp);
                let keep = marked || (solid && holds_progress);
                if !keep {
                    self.world.despawn(site);
                    sites.remove(&(x, y));
                }
            }
            None if marked => {
                let max_hp = self.wall_at(x, y).durability;
                let site = self.world.spawn((
                    DigSite {
                        marked: true,
                        announced_stuck: false,
                        announced_dry: false,
                        finish: order,
                    },
                    Durability {
                        hp: if solid { max_hp } else { 0 },
                        max_hp,
                    },
                    Position { x, y },
                ));
                sites.insert((x, y), site.id());
            }
            None => {}
        }
    }

    /// Lays floor over field `(x, y)` and retires the cell's `DigSite`
    /// with it.
    ///
    /// The one place a field cell becomes `Floor` in play — `lay_tile`
    /// today, the crew's flooring job next — because a finished cell has
    /// nothing left to record and a mark that outlived the tile would be a
    /// job no one could ever complete. Founding writes the starting 3x3
    /// directly instead: it runs before anything can have dug.
    pub(crate) fn floor_cell(&mut self, x: i32, y: i32) {
        self.world.resource_mut::<BaseGrid>().lay_floor(x, y);
        if let Some(site) = self.dig_site_at(x, y) {
            self.world.despawn(site);
        }
    }

    /// One tick of the dig crew: every program posted to a `DigSite` walks
    /// to it, cuts it, and floors what the cut opened.
    ///
    /// **A `&mut Game` method called from `tick_inner`, not a bevy system**,
    /// and for a stronger version of the reason `schedule_base_labour` is
    /// one: a cycle here ends in `Game::strike_rock` or `Game::floor_cell`,
    /// which are *the* place rock takes damage and *the* place a base cell
    /// becomes floor. A system could reach neither, and would have had to
    /// keep a second copy of the swing damage, the fragment roll, the
    /// tile spend and the cell write — four formulas with one door
    /// each, which is exactly the drift `balance_sim` has already been
    /// bitten by four times. It runs immediately after
    /// `schedule_base_labour`, so a body posted this tick digs this tick.
    ///
    /// The walk is `hauling::step_to_post` — the same walk a posted worker
    /// takes to a machine, not a second one — because `haul_step_system`
    /// cannot move a digger: it resolves every destination through a
    /// `Structure` query, and a dig site is the one `Task` target that is
    /// not a structure.
    ///
    /// **A digger that loses its route drops the job** rather than standing
    /// in the dark forever. That is what keeps the stall announcement
    /// honest: `schedule_base_labour` is the only thing that announces one,
    /// and it only ever looks at sites nobody is posted to.
    pub(crate) fn run_dig_crew(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        // Sorted by tile for the reason `assembler_system` sorts its
        // machines: bevy's iteration order is not stable, and two diggers
        // whose swings land in a different order between runs would make the
        // same base save differently.
        let mut diggers: Vec<(i32, i32, Entity)> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Task, &Position), Without<Fleeing>>();
            query
                .iter(&self.world)
                .filter(|(_, task, _)| task.kind == TaskKind::Excavate)
                .map(|(e, _, p)| (p.x, p.y, e))
                .collect()
        };
        diggers.sort_unstable();
        let blocked = self.blocked_tiles();
        let pocket_radius = self.world.resource::<BaseGrid>().radius();
        for (.., worker) in diggers {
            let Some(site) = self.world.get::<Task>(worker).map(|t| t.target) else {
                continue;
            };
            // The site is gone — cut and floored by somebody else — or its
            // mark was cleared while this body was working. Either way there
            // is nothing left to dig here, and the scheduler will find it
            // something else.
            //
            // The cleared mark is the half that has to be said separately:
            // `set_mark` deliberately *keeps* an unmarked site that still
            // holds chip progress, so a cancelled plan does not look like a
            // despawned entity. `dig_wants` stops listing it, but
            // `schedule_base_labour` never takes a body off a post it has
            // nowhere better to send — so nothing upstream frees this one.
            let marked = self.world.get::<DigSite>(site).is_some_and(|d| d.marked);
            let Some(target) = self.world.get::<Position>(site).copied().filter(|_| marked) else {
                self.world.entity_mut(worker).remove::<Task>();
                continue;
            };
            let from = self
                .world
                .get::<Position>(worker)
                .copied()
                .unwrap_or(Position { x: 0, y: 0 });
            // A `DigSite` is always footprint 1 — a mark names one cell —
            // so `side` here is never anything but the default.
            if !crate::game::base::hauling::at_station(from, target, 1) {
                let step = {
                    crate::game::base::hauling::step_to_post(
                        self.world.resource::<BaseGrid>(),
                        from,
                        target,
                        1,
                        &blocked,
                        pocket_radius,
                    )
                };
                match step {
                    Ok(Some(next)) => self.take_base_step(worker, next),
                    // Nowhere better to stand: the field admits the tile the
                    // worker is already on and nothing closer. It waits.
                    Ok(None) => {}
                    Err(_) => {
                        self.world.entity_mut(worker).remove::<Task>();
                    }
                }
                continue;
            }
            let Some(mut task) = self.world.get_mut::<Task>(worker) else {
                continue;
            };
            task.progress += 1;
            let landed = task.progress >= task.required;
            if landed {
                task.progress = 0;
            }
            // Read per tick rather than once the cycle completes, because
            // which half of the one dig verb this is decides whether the
            // tick makes a sound: cutting is mining, flooring and finishing
            // are not.
            let cutting = !self.ground().walkable(target.x, target.y);
            if cutting && !landed {
                self.cue_mining(target.x, target.y);
            }
            if !landed {
                continue;
            }
            // Which of the three things this cycle lands is decided by the
            // site's own `finish`, not by anything about the cell: a finish
            // or strip mark never reaches `strike_rock` or `crew_lays_tile`,
            // because it only ever sits on laid floor and there is no rock
            // to cut and no tile to lay. Absent one, marked solid means cut
            // it and marked `Open` means floor it — the mark outlives the
            // cut either way.
            let finish = self
                .world
                .get::<DigSite>(site)
                .and_then(|d| d.finish.clone());
            match finish {
                Some(order) => self.crew_finishes(site, target.x, target.y, order),
                None if cutting => {
                    // The crew digs outward from the floor, a mark at a time:
                    // a blocker no floor or pending floor touches waits until
                    // one does (plan, resolved questions 1 and 5).
                    let pending = self.pending_floor();
                    if crate::game::field_mining::minable(
                        (target.x, target.y),
                        self.ground(),
                        &pending,
                    ) {
                        self.strike_field_rock(worker, target.x, target.y);
                    }
                }
                None => self.crew_lays_tile(target.x, target.y),
            }
        }
    }

    /// The crew's half of a finish or a strip mark. `Apply` spends
    /// `tuning::FLOOR_FINISH_COST` and paints the shade on; `Strip` clears
    /// whatever is there and charges nothing. Both despawn the site — a
    /// finish job is done in one landing, unlike cutting, which keeps its
    /// site across the cut and the tile.
    fn crew_finishes(&mut self, site: Entity, x: i32, y: i32, order: FinishOrder) {
        match order {
            FinishOrder::Apply(id) => {
                let grid = self.world.resource::<BaseGrid>();
                // The cell stopped being floor, or a second digger already
                // painted this exact finish here first: either way there is
                // nothing left to do, and no charge for not doing it —
                // `BaseGrid::revert`'s own caution about "a finish implies
                // floor" reaching a cell mid-job.
                if !grid.is_floor(x, y) || grid.finish_at(x, y) == Some(&id) {
                    self.world.despawn(site);
                    return;
                }
                // A mod removed this finish since the order was written —
                // `restore_dig_sites` already drops one of these at load,
                // but a live `DigSite` can still carry one from a save
                // written before that check existed. `BaseGrid::set_finish`
                // does not validate the id, so this must, or a name
                // nothing can display gets painted onto the grid and
                // charged for.
                if self
                    .world
                    .resource::<crate::floors::FloorDb>()
                    .get(&id)
                    .is_none()
                {
                    self.world.despawn(site);
                    self.log_base(format!(
                        "The finish planned for ({x}, {y}) is no longer installed — the order is dropped."
                    ));
                    return;
                }
                let substrate = ItemId::from(crate::items::ids::BLANK_SUBSTRATE);
                if !self.spend_from_stores(
                    &substrate,
                    crate::tuning::FLOOR_FINISH_COST,
                    crate::base_ledger::ConsumeSource::Base,
                ) {
                    // `dig_wants` already judged this workable a moment ago
                    // — the one way this can still fail is two finish jobs
                    // landing on the same tick against a single surviving
                    // batch, `crew_lays_tile`'s own race. The mark stays,
                    // and the next `dig_wants` will see the shortage and
                    // announce it.
                    return;
                }
                let name = self
                    .world
                    .resource::<crate::floors::FloorDb>()
                    .get(&id)
                    .map(|def| def.name.clone())
                    .unwrap_or_else(|| id.as_str().to_string());
                self.world.resource_mut::<BaseGrid>().set_finish(x, y, id);
                self.world.despawn(site);
                self.log_base(format!(
                    "Your crew finishes the floor at ({x}, {y}) with {name}."
                ));
            }
            FinishOrder::Strip => {
                let removed = self.world.resource_mut::<BaseGrid>().clear_finish(x, y);
                self.world.despawn(site);
                if removed {
                    self.log_base(format!(
                        "Your crew strips the finish from the floor at ({x}, {y})."
                    ));
                }
            }
        }
    }

    /// The crew's half of `Game::lay_tile`: `tuning::FLOOR_TILE_COST` Core
    /// Fragments, and the cell is floor.
    ///
    /// **Out of the base's own stores and never the player's pack.** The
    /// crew is base labour, and the store it spends from is the one the base
    /// fills — `stock::spend_from_base` over the same buffers the stock block
    /// counts, so a strip showing fragments is a strip saying the crew can
    /// pave. Core Fragments are the player's currency and cross a breach in
    /// the pack, so a pack fallback would spend a marked corridor's worth
    /// while the party is away, with no dry line because the pack covered
    /// it. `Game::lay_tile` pays from the pack alone — a player verb, paid
    /// the way every other player verb is. A finish still falls back on the
    /// pack (`spend_from_stores`): Blank Substrate is nobody's currency.
    ///
    /// **A dry attempt here is silent**, `run_build_crew`'s `Errand::Dry`
    /// rule. `Game::dig_wants` decides whether a body is even posted to a
    /// floor job — it will not post one to a site it just judged dry, and
    /// it is the one place that both sets and clears `DigSite::announced_dry`
    /// now, asked every tick regardless of whose turn it is to swing. So a
    /// site reaching this function has already been judged workable a
    /// moment ago; the one way this can still fail is two floor jobs
    /// finishing on the same tick against a single surviving unit, a race
    /// `drop_dry_dig_wants`' budget cannot see because it only reads
    /// how much exists right now, not enough for every job about to ask.
    /// That race needs no second announcement — `dig_wants` will see the
    /// shortage itself on the very next tick — and the mark stays either
    /// way, so the job is simply retried.
    fn crew_lays_tile(&mut self, x: i32, y: i32) {
        // Something claimed the cell since the mark went down — a town grew
        // over it, an outpost was raised: the request is dropped, unpaid.
        if self.floor_conflict(x, y).is_some() {
            if let Some(site) = self.dig_site_at(x, y) {
                self.world.despawn(site);
            }
            return;
        }
        let fragment = ItemId::from(crate::items::ids::CORE_FRAGMENT);
        let cost = crate::tuning::FLOOR_TILE_COST;
        if crate::game::base::work_orders::base_holding(self, &fragment) < cost {
            return;
        }
        crate::game::base::stock::spend_from_base(
            self,
            &fragment,
            cost,
            crate::base_ledger::ConsumeSource::Base,
        );
        self.floor_cell(x, y);
        self.log_base("Your crew lays a VectorStasis Tile, and the cell reads as floor.");
    }

    /// Spends `count` of `item`, base stores before the player's pack,
    /// refusing — and moving nothing — unless `stores_hold` already covers
    /// the whole count. The floor-finish arm above and the Refit Bench call
    /// it; a tile never does, since the crew pays for tiles out of the base
    /// alone.
    pub(crate) fn spend_from_stores(
        &mut self,
        item: &ItemId,
        count: u32,
        source: crate::base_ledger::ConsumeSource,
    ) -> bool {
        if self.stores_hold(item) < count {
            return false;
        }
        let from_base = crate::game::base::stock::spend_from_base(self, item, count, source);
        let remaining = count - from_base;
        if remaining > 0 {
            let player = self.player_entity();
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take(item.clone(), remaining);
            self.note_consumed(item, remaining, source);
        }
        true
    }

    /// Whether `(x, y)` refuses a step outright: ground nothing can stand
    /// on, or a barrier. `Game::move_player` and `Game::travel_step` both ask
    /// this rather than `Game::blocked_tiles`, which also refuses every
    /// structure's own anchor — a machine is walked over, only a barrier or
    /// rock actually stopping the party.
    pub(crate) fn base_step_blocked(&mut self, x: i32, y: i32) -> bool {
        !self.ground().walkable(x, y) || self.barrier_at(x, y).is_some()
    }

    /// A bump into a minable field blocker from the player's cell: a swing
    /// when the tools are out and the cell is straight ahead, a free refusal
    /// otherwise.
    pub(crate) fn bump_rock(&mut self, (nx, ny): (i32, i32), diagonal: bool) {
        // Armed first, and this is the whole of the toggle. Slice 2 made
        // every bump a swing, which meant walking a corridor at a
        // developed level took the corridor's corners out with you. A
        // disarmed bump is refused for exactly as slice 1 refused it —
        // free, no tick, nothing damaged — so the cost of the safety is
        // a keypress and not a turn.
        if !self.mining() {
            self.log(
                "You put a shoulder to the entropy and stop short.                      (Press n to cut.)",
            );
            return;
        }
        // Straight ahead only: a diagonal cut can open a cell that
        // touches the base at a corner alone, and the siege board fills
        // four ways, so a room dug that way would stand out of every
        // raid's reach. Refused as the disarmed bump is — free, no tick.
        if diagonal {
            self.log("You can only cut rock straight ahead.");
            return;
        }
        let player = self.player_entity();
        self.strike_field_rock(player, nx, ny);
        self.tick();
    }
}
