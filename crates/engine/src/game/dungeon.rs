//! Walking a dungeon: getting in, moving around inside, and getting back
//! out.
//!
//! Everything here operates on `resources::Locale` and leaves the player's
//! `Position` component alone — see that resource's docs for why. The one
//! exception is `enter_dungeon`, which pins `Position` to the entrance tile
//! on the way in.

use crate::dungeon::{self, CellKind, Dir};
use crate::resources::{CurrentDungeon, DungeonMemory, LevelMemory, Locale};
use crate::tuning::{
    DUNGEON_CACHE_CREDITS, DUNGEON_CACHE_DEPTH_GROWTH, DUNGEON_CACHE_FRAGMENT_CHANCE,
    DUNGEON_DEPTH_STAT_GROWTH, DUNGEON_ENCOUNTER_CHANCE, DUNGEON_ENTRANCE_SCATTER_TILES,
    DUNGEON_FLOORS_MAX, DUNGEON_FLOORS_MIN, DUNGEON_MIN_ENTRANCE_TILES,
    DUNGEON_NEAREST_ENTRANCE_TILES, DUNGEON_TILES_PER_FLOOR,
};
use crate::*;

/// How far ahead the first-person view reaches, in cells. Four is enough
/// corridor to read a junction coming without the far wall shrinking to
/// nothing.
pub const DUNGEON_VIEW_DEPTH: usize = 4;

/// Cells visible either side of the party's line of sight. One gives the
/// three-wide cone a classic blobber shows — the corridor you are in, plus
/// whatever opens off it.
pub const DUNGEON_VIEW_HALF_WIDTH: usize = 1;

/// Eight-point compass heading for an offset, with north as `-y` to match
/// `dungeon::Dir`.
///
/// A direction counts as diagonal when neither axis dominates the other by
/// more than half — a strict "whichever is larger" split would call a
/// near-45° bearing due east and send the player off at an angle.
pub(crate) fn bearing(dx: i32, dy: i32) -> &'static str {
    let (ax, ay) = (dx.abs(), dy.abs());
    let horizontal = ax * 2 > ay;
    let vertical = ay * 2 > ax;
    match (vertical.then_some(dy < 0), horizontal.then_some(dx > 0)) {
        (Some(true), None) => "north",
        (Some(false), None) => "south",
        (None, Some(true)) => "east",
        (None, Some(false)) => "west",
        (Some(true), Some(true)) => "north-east",
        (Some(true), Some(false)) => "north-west",
        (Some(false), Some(true)) => "south-east",
        (Some(false), Some(false)) => "south-west",
        // Only reachable at dx == dy == 0, where every direction is as good
        // as any other.
        (None, None) => "here",
    }
}

/// The world coordinates of the view cone from `(x, y)` facing `facing`,
/// indexed `[ahead][lateral]` — the same shape and order
/// `views::DungeonView::cells` carries.
///
/// The first-person view and the map's record of what has been seen are both
/// filled by walking this, so the map cannot mark a cell the view never
/// showed and the view cannot show one the map won't remember.
fn view_cone(x: i32, y: i32, facing: Dir) -> Vec<Vec<(i32, i32)>> {
    let (fx, fy) = facing.delta();
    let (rx, ry) = facing.right_delta();
    let span = DUNGEON_VIEW_HALF_WIDTH as i32;

    (0..DUNGEON_VIEW_DEPTH as i32)
        .map(|ahead| {
            (-span..=span)
                .map(|lateral| (x + fx * ahead + rx * lateral, y + fy * ahead + ry * lateral))
                .collect()
        })
        .collect()
}

/// The `Locale::Dungeon` payload, unpacked — see `Game::dungeon_pos`. A
/// named struct rather than a five-tuple so call sites read as fields
/// rather than as positions.
#[derive(Clone, Copy)]
struct DungeonPos {
    depth: u32,
    floors: u32,
    x: i32,
    y: i32,
    facing: Dir,
    entrance: (i32, i32),
}

/// How many levels the shaft under a breach at `tile` runs before it
/// bottoms out.
///
/// Depth is read off the walk to the breach rather than rolled, so it rides
/// the same distance-from-arrival that already scales wild program stats.
/// The two then agree — a far breach is deeper *and* fields harder programs
/// — instead of a nearby hole occasionally being the deepest thing in the
/// sector for no reason the player could have seen coming.
pub(crate) fn breach_floors(tile: (i32, i32), spawn: (i32, i32)) -> u32 {
    let distance = (tile.0 - spawn.0).abs().max((tile.1 - spawn.1).abs());
    let extra = (distance / DUNGEON_TILES_PER_FLOOR).max(0) as u32;
    (DUNGEON_FLOORS_MIN + extra).min(DUNGEON_FLOORS_MAX)
}

impl Game {
    /// Finds a `DungeonEntrance` at `(x, y)`, if any — checked in
    /// `move_player` before the generic blocking-structure check, so walking
    /// onto one descends instead of just bumping into it.
    pub(crate) fn find_dungeon_entrance_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<DungeonEntrance>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// Spawns `DUNGEON_ENTRANCES_PER_ZONE` breaches scattered around the
    /// player's arrival point, on walkable ground outside the base platform.
    /// Called once per zone, alongside `spawn_initial_creatures`.
    ///
    /// Platform tiles are skipped rather than merely unlikely: the base is
    /// the one safe ground in the game, and a hole down into a dungeon in
    /// the middle of it would undo that.
    ///
    /// Draws from a locally seeded RNG rather than `resources::GameRng`, for
    /// two reasons. Where the entrances are is world generation, and world
    /// generation should be a function of the seed rather than of however
    /// many rolls happened to precede it — the same argument
    /// `dungeon::generate` makes. And drawing from the shared stream here
    /// would shift every later roll in the run, which is not a private
    /// matter: it silently rewrites the outcome of every seeded test in the
    /// suite. `ENTRANCE_SALT` keeps this stream off the one the levels
    /// themselves are carved from, so entrance placement and maze layout
    /// don't correlate.
    pub(crate) fn spawn_dungeon_entrances(&mut self, count: usize) {
        const ENTRANCE_SALT: u64 = 0xD07E_5A17;

        let origin = *self.world.get::<Position>(self.player_entity()).unwrap();
        let seed = self.world.resource::<WorldMap>().seed();
        let zone = self.world.resource::<ZoneLevel>().0;
        let mut rng = StdRng::seed_from_u64(((seed as u64) << 32) ^ zone as u64 ^ ENTRANCE_SALT);

        let mut placed = 0;
        let mut attempts = 0;
        while placed < count && attempts < count * 40 {
            attempts += 1;
            // The first one lands inside the opening viewport; the rest
            // scatter. See `DUNGEON_NEAREST_ENTRANCE_TILES`.
            let reach = if placed == 0 {
                DUNGEON_NEAREST_ENTRANCE_TILES
            } else {
                DUNGEON_ENTRANCE_SCATTER_TILES
            };
            let (dx, dy) = (
                rng.random_range(-reach..=reach),
                rng.random_range(-reach..=reach),
            );
            if dx.abs().max(dy.abs()) < DUNGEON_MIN_ENTRANCE_TILES {
                continue;
            }
            let (x, y) = (origin.x + dx, origin.y + dy);
            let tile = self.world.resource_mut::<WorldMap>().tile(x, y);
            if !tile.walkable || tile.biome == Biome::Platform {
                continue;
            }
            if self.find_dungeon_entrance_at(x, y).is_some()
                || self.find_blocking_structure_at(x, y).is_some()
            {
                continue;
            }
            self.spawn_entrance_at(x, y);
            placed += 1;
        }
        self.announce_dungeon_entrances(origin);
    }

    /// Logs what the arrival scan picks up: how many breaches are in the
    /// sector and where the nearest one lies.
    ///
    /// Without this the layer is invisible. Three unmarked holes scattered
    /// across an unbounded procedural field, in a game that has never had a
    /// reason to reward wandering, is not something a player finds — they
    /// have to be told the breaches are there before looking for one is a
    /// choice rather than an accident.
    fn announce_dungeon_entrances(&mut self, origin: Position) {
        let mut query = self
            .world
            .query_filtered::<&Position, With<DungeonEntrance>>();
        let mut found: Vec<(i32, i32, i32)> = query
            .iter(&self.world)
            .map(|p| {
                let (dx, dy) = (p.x - origin.x, p.y - origin.y);
                (dx.abs().max(dy.abs()), dx, dy)
            })
            .collect();
        found.sort();

        let Some(&(distance, dx, dy)) = found.first() else {
            return;
        };
        self.log(format!(
            "Deep scan: {} breach{} in this sector. Nearest bears {} at {distance} tiles.",
            found.len(),
            if found.len() == 1 { "" } else { "es" },
            bearing(dx, dy),
        ));
    }

    /// Rebuilds the entrances a save recorded — see
    /// `save::SaveData::dungeon_entrances`.
    pub(crate) fn restore_dungeon_entrances(&mut self, tiles: Vec<(i32, i32)>) {
        for (x, y) in tiles {
            self.spawn_entrance_at(x, y);
        }
    }

    /// The one place an entrance entity is built, so a fresh zone's breaches
    /// and a reloaded save's cannot end up looking like different things.
    ///
    /// `>` rather than anything more decorative because every other glyph on
    /// the map is spoken for — `%` is both the Static Field biome and a
    /// species — and because it already reads as "the way down" to anyone
    /// who has played a roguelike. It is the same mark the dungeon view puts
    /// on a staircase.
    fn spawn_entrance_at(&mut self, x: i32, y: i32) {
        self.world.spawn((
            DungeonEntrance,
            Position { x, y },
            Glyph {
                ch: '>',
                color: GlyphColor::Magenta,
            },
        ));
    }

    pub fn is_underground(&self) -> bool {
        matches!(self.world.resource::<Locale>(), Locale::Dungeon { .. })
    }

    /// `Err` if the party is underground — for actions that reach into the
    /// zone map through the player's `Position`.
    ///
    /// While underground that `Position` is pinned to the dungeon entrance
    /// (see `resources::Locale`), so these would otherwise operate on a tile
    /// out in the wild that the player is nowhere near: deploying a
    /// structure beside the breach, or — worst of all — `use_symlink`
    /// teleporting the pinned entrance somewhere else and changing where
    /// climbing back up puts you.
    ///
    /// Party and inventory management deliberately isn't on this list.
    /// Fusing programs, installing routines, crafting, equipping and
    /// spending perk points all work fine four levels down, and stopping to
    /// sort your gear in a dungeon is a thing the genre expects.
    pub(crate) fn require_surface(&self) -> Result<(), String> {
        if self.is_underground() {
            return Err("Not down here — that needs open grid.".into());
        }
        Ok(())
    }

    /// Descends into the dungeon reached through the entrance standing at
    /// `(x, y)`, which the player is stepping onto. The entrance itself
    /// survives — unlike a zone portal, it is a place you can come back to.
    pub(crate) fn enter_dungeon(&mut self, x: i32, y: i32) {
        let player = self.player_entity();
        if let Some(mut pos) = self.world.get_mut::<Position>(player) {
            pos.x = x;
            pos.y = y;
        }
        let floors = self.breach_floors_at((x, y));
        self.descend_to(1, floors, (x, y));
        self.log(format!(
            "You drop through the breach. The signal above you thins to nothing. \
             The shaft sounds {floors} levels deep."
        ));
    }

    /// How deep the shaft under the breach at `tile` runs — see
    /// `breach_floors`, which this feeds the zone's arrival point.
    pub(crate) fn breach_floors_at(&self, tile: (i32, i32)) -> u32 {
        let spawn = self.world.resource::<ZoneSpawnPoint>();
        breach_floors(tile, (spawn.x, spawn.y))
    }

    fn level_spec(&self, depth: u32, floors: u32, entrance: (i32, i32)) -> dungeon::LevelSpec {
        dungeon::LevelSpec {
            world_seed: self.world.resource::<WorldMap>().seed(),
            entrance,
            depth,
            floors,
        }
    }

    /// Generates the level for `depth` and puts the party on its entry cell
    /// facing north. Shared by the way in and every flight of stairs down.
    fn descend_to(&mut self, depth: u32, floors: u32, entrance: (i32, i32)) {
        let level = dungeon::generate(self.level_spec(depth, floors, entrance));
        let entry = level.entry;
        self.world.insert_resource(CurrentDungeon(Some(level)));
        self.world.insert_resource(Locale::Dungeon {
            depth,
            floors,
            x: entry.0,
            y: entry.1,
            facing: Dir::North,
            entrance,
        });
        self.remember_view();
    }

    /// Puts the party back on the zone map. The player's `Position` was
    /// pinned to the entrance tile the whole time, so there is nothing to
    /// restore.
    ///
    /// Unnarrated, because the breach is no longer the only way out —
    /// `use_symlink` leaves by its own route and says so in its own words.
    pub(crate) fn clear_dungeon(&mut self) {
        self.world.insert_resource(Locale::Surface);
        self.world.insert_resource(CurrentDungeon(None));
    }

    /// Climbs out through the breach the party walked in through.
    fn leave_dungeon(&mut self) {
        self.clear_dungeon();
        self.log("You surface through the breach, back onto open grid.".to_string());
    }

    /// Records everything the party can see from where they are standing.
    ///
    /// Called from every place that moves the party or turns them, plus the
    /// load path — anywhere the view changes, the map has to change with it,
    /// or the player is told they never looked down a corridor they are
    /// currently staring at.
    fn remember_view(&mut self) {
        let Some(pos) = self.dungeon_pos() else {
            return;
        };
        let Some(level) = self.world.resource::<CurrentDungeon>().0.clone() else {
            return;
        };

        let mut seen = Vec::new();
        for (ahead, row) in view_cone(pos.x, pos.y, pos.facing).into_iter().enumerate() {
            // The party's own cell can never stop their view out of it. That
            // is not hypothetical: a door both blocks sight and is walkable —
            // the only cell that is both — so standing in a doorway would
            // otherwise blind the party to the corridor they are standing in.
            //
            // The wall that stops the view is itself in plain sight, so the
            // row is recorded before the break, not after the check.
            let blocked = ahead > 0
                && row
                    .get(DUNGEON_VIEW_HALF_WIDTH)
                    .is_some_and(|&(cx, cy)| level.cell(cx, cy).blocks_sight());
            seen.extend(row);
            if blocked {
                break;
            }
        }

        let memory = self.level_memory_mut(pos);
        memory.seen.extend(seen);
    }

    /// The memory of the level the party is standing in, created empty on
    /// first sight of it.
    fn level_memory_mut(&mut self, pos: DungeonPos) -> &mut LevelMemory {
        self.world
            .resource_mut::<DungeonMemory>()
            .into_inner()
            .0
            .entry((pos.entrance, pos.depth))
            .or_default()
    }

    /// Empties the cache the party is standing on, if there is one and it
    /// has not already been emptied.
    ///
    /// Payout is a depth-scaled pile of Credits, a chance at a portal
    /// fragment, and whatever the item set declares — see
    /// `ItemDef::cache_drop`, which is how a mod adds to what caches hold
    /// without touching this function.
    ///
    /// Credits rather than Core Fragments deliberately: they are the one
    /// currency that survives a breach, so a dungeon run banks something the
    /// next sector can still spend.
    fn open_cache(&mut self) {
        let Some(pos) = self.dungeon_pos() else {
            return;
        };
        if self.cell_underfoot() != Some(CellKind::Cache) {
            return;
        }
        if self
            .world
            .resource::<DungeonMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.looted.contains(&(pos.x, pos.y)))
        {
            return;
        }
        self.level_memory_mut(pos).looted.insert((pos.x, pos.y));

        let depth_mult = DUNGEON_CACHE_DEPTH_GROWTH.powi(pos.depth as i32 - 1);
        let credits = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(DUNGEON_CACHE_CREDITS)
        };
        let credits = ((credits as f32) * depth_mult).round() as u32;

        self.log_kind(MessageKind::Loot, "A cache, still sealed. You crack it.");
        let landed = self.grant_loot(self.trade_currency(), credits);
        if landed > 0 {
            self.log_kind(
                MessageKind::Loot,
                format!("{landed} credits, skimmed off some long-dead process."),
            );
        }

        let fragment_roll = {
            let chance = (DUNGEON_CACHE_FRAGMENT_CHANCE * pos.depth as f64).min(1.0);
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance)
        };
        if fragment_roll && self.grant_loot(self.craft_currency(), 1) > 0 {
            self.log_kind(
                MessageKind::Loot,
                "A portal fragment, wedged in the casing.",
            );
        }

        // Sorted by id so a seeded run consumes its rolls in the same order
        // however the item files happen to load — the same guarantee
        // `equipment_drops_for` makes.
        let mut table: Vec<(ItemId, f32)> = self
            .world
            .resource::<ItemDb>()
            .all()
            .filter_map(|def| def.cache_drop.map(|chance| (def.id.clone(), chance)))
            .collect();
        table.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

        for (item, chance) in table {
            let roll = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(chance.clamp(0.0, 1.0) as f64)
            };
            if roll && self.grant_loot(item.clone(), 1) > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("Also inside: a {}.", self.item_name(&item)),
                );
            }
        }
    }

    /// Whether the party may walk into `cell`, opening a sealed door on the
    /// way if they are carrying something that opens it.
    ///
    /// Spends the shard rather than keeping it, and records the door as open
    /// so the way back out is free — a one-way vault that charged you again
    /// on the return trip would be a tax on having gone in.
    fn pass_seal(&mut self, pos: DungeonPos, cell: (i32, i32)) -> bool {
        let already_open = self
            .world
            .resource::<DungeonMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.opened.contains(&cell));
        if already_open {
            return true;
        }

        let shard = ItemId::from(ids::ACCESS_SHARD);
        let player = self.player_entity();
        let spent = self
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(shard, 1);
        if spent == 0 {
            self.log("Sealed. The lock wants authorization you don't have.".to_string());
            return false;
        }
        self.level_memory_mut(pos).opened.insert(cell);
        self.log_kind(
            MessageKind::Outcome,
            "You burn an access shard. The seal releases.",
        );
        true
    }

    /// Starts the boss fight if the party has just walked into an uncleared
    /// lair.
    ///
    /// The species is drawn from the breach tile's biome, like every other
    /// dungeon encounter, but from the boss pool rather than the ordinary
    /// one, and from an RNG seeded off the level spec rather than off
    /// `GameRng`: which thing guards a shaft is a property of the shaft, not
    /// of how many rolls happened first, so leaving and coming back cannot
    /// reroll it into something easier.
    ///
    /// Not every biome fields a boss — no shipped Static Field species does
    /// — and there the lair falls back to the toughest ordinary program the
    /// biome has, which at the bottom of a deep shaft is no small thing.
    fn rouse_lair(&mut self) {
        if self.has_active_battle() || self.is_game_over().is_some() {
            return;
        }
        let Some(pos) = self.dungeon_pos() else {
            return;
        };
        if self.cell_underfoot() != Some(CellKind::Lair) || self.lair_cleared(pos) {
            return;
        }

        let (ex, ey) = pos.entrance;
        let Some((species, is_boss)) = self.pick_lair_species(pos) else {
            return;
        };
        let pack = self.spawn_pack(&species, is_boss, ex, ey);
        if pack.is_empty() {
            return;
        }
        for &member in &pack {
            self.world.entity_mut(member).insert(DungeonSpawn);
        }
        self.remember_fight();
        self.log_kind(
            MessageKind::Outcome,
            "The shaft opens out. Something very large is already awake.",
        );
        self.start_battle(pack);
    }

    fn pick_lair_species(&mut self, pos: DungeonPos) -> Option<(String, bool)> {
        let (ex, ey) = pos.entrance;
        let biome = self.world.resource_mut::<WorldMap>().tile(ex, ey).biome;
        let spec = self.level_spec(pos.depth, pos.floors, pos.entrance);

        let bosses: Vec<String> = self
            .world
            .resource::<SpeciesDb>()
            .boss_habitat_matches(biome)
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
        if !bosses.is_empty() {
            // Salted off the level's own stream so the choice of guardian
            // doesn't correlate with the shape of the room it stands in.
            const LAIR_SALT: u64 = 0x1A19_B055;
            let mut rng = StdRng::seed_from_u64(spec.rng_seed() ^ LAIR_SALT);
            return Some((bosses[rng.random_range(0..bosses.len())].clone(), true));
        }

        let db = self.world.resource::<SpeciesDb>();
        db.habitat_matches(biome)
            .into_iter()
            .max_by_key(|s| s.base_hp + s.base_atk + s.base_def)
            .map(|s| (s.id.clone(), false))
    }

    /// Whether the sealed door on `cell` has already been burned open.
    fn seal_open(&self, pos: DungeonPos, cell: (i32, i32)) -> bool {
        self.world
            .resource::<DungeonMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.opened.contains(&cell))
    }

    fn lair_cleared(&self, pos: DungeonPos) -> bool {
        self.world
            .resource::<DungeonMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.cleared)
    }

    /// Records that this shaft's guardian is down, so its lair does not
    /// refill. Called from `award_loot`, which is the one place that knows a
    /// hostile actually died rather than merely being fled from.
    pub(crate) fn mark_lair_cleared(&mut self) {
        let Some(pos) = self.dungeon_pos() else {
            return;
        };
        if self.cell_underfoot() != Some(CellKind::Lair) {
            return;
        }
        self.level_memory_mut(pos).cleared = true;
    }

    /// Whether the cache on `cell` of the level the party is in is still
    /// unopened — what both views use to stop advertising an empty one.
    fn cache_unopened(&self, pos: DungeonPos, cell: (i32, i32)) -> bool {
        !self
            .world
            .resource::<DungeonMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.looted.contains(&cell))
    }

    /// Marks the cell the party is standing on as somewhere a fight started,
    /// for the map to pin.
    pub(crate) fn remember_fight(&mut self) {
        let Some(pos) = self.dungeon_pos() else {
            return;
        };
        self.level_memory_mut(pos).fights.insert((pos.x, pos.y));
    }

    /// The party's current cell and facing, or `None` on the surface.
    fn dungeon_pos(&self) -> Option<DungeonPos> {
        match *self.world.resource::<Locale>() {
            Locale::Surface => None,
            Locale::Dungeon {
                depth,
                floors,
                x,
                y,
                facing,
                entrance,
            } => Some(DungeonPos {
                depth,
                floors,
                x,
                y,
                facing,
                entrance,
            }),
        }
    }

    /// Whether a dungeon action can run at all: underground, alive, and not
    /// mid-intrusion. Mirrors the guard at the top of `move_player`.
    fn can_act_underground(&self) -> bool {
        self.is_underground() && self.is_game_over().is_none() && !self.has_active_battle()
    }

    fn set_facing(&mut self, dir: Dir) {
        if let Locale::Dungeon { facing, .. } = &mut *self.world.resource_mut::<Locale>() {
            *facing = dir;
        }
        self.remember_view();
    }

    pub fn turn_left(&mut self) {
        if !self.can_act_underground() {
            return;
        }
        let Some(pos) = self.dungeon_pos() else {
            return;
        };
        self.set_facing(pos.facing.turn_left());
        self.tick();
    }

    pub fn turn_right(&mut self) {
        if !self.can_act_underground() {
            return;
        }
        let Some(pos) = self.dungeon_pos() else {
            return;
        };
        self.set_facing(pos.facing.turn_right());
        self.tick();
    }

    pub fn step_forward(&mut self) {
        self.step(1);
    }

    /// Backs up without turning round — the party keeps facing the way it
    /// was, which is how you retreat down a corridor you have already read.
    pub fn step_back(&mut self) {
        self.step(-1);
    }

    /// `sign` is +1 to walk forward along the facing, -1 to back up along it.
    /// Ticks the surface sim either way, and ticks it even when the step is
    /// blocked by rock: shoving at a wall still passes time, exactly as a
    /// blocked step does on the surface.
    fn step(&mut self, sign: i32) {
        if !self.can_act_underground() {
            return;
        }
        let Some(pos) = self.dungeon_pos() else {
            return;
        };
        let (dx, dy) = pos.facing.delta();
        let (nx, ny) = (pos.x + dx * sign, pos.y + dy * sign);

        let target = self
            .world
            .resource::<CurrentDungeon>()
            .0
            .as_ref()
            .map(|level| level.cell(nx, ny))
            .unwrap_or(CellKind::Rock);

        // A sealed door is walkable as far as the level is concerned — the
        // generator has to see through it — so the lock is checked here.
        let walkable =
            target.walkable() && (target != CellKind::SealedDoor || self.pass_seal(pos, (nx, ny)));

        if walkable && let Locale::Dungeon { x, y, .. } = &mut *self.world.resource_mut::<Locale>()
        {
            *x = nx;
            *y = ny;
        }
        // Before the encounter roll, so a corridor the party walked into is
        // on their map even if something jumps them the moment they enter it,
        // and so a cache is the party's before anything can interrupt them
        // reaching for it.
        self.remember_view();
        self.open_cache();
        self.rouse_lair();
        // Only a step that actually covered ground draws an encounter —
        // shoving at a wall is not travel, the same call `move_player`
        // makes about an ambush.
        if walkable {
            self.maybe_dungeon_encounter();
        }
        self.tick();
    }

    /// What dungeon depth multiplies wild program stats by — `1.0` on the
    /// surface, so `spawn_wild_creature` can fold it in unconditionally.
    ///
    /// Compounds with `ZoneLevel::stat_multiplier` and
    /// `distance_stat_multiplier` rather than replacing them: a breach far
    /// out in a deep zone is a nastier hole than one beside your base, and
    /// going down makes either worse.
    pub(crate) fn dungeon_depth_multiplier(&self) -> f32 {
        match self.dungeon_pos() {
            None => 1.0,
            Some(pos) => DUNGEON_DEPTH_STAT_GROWTH.powi(pos.depth as i32 - 1),
        }
    }

    /// Rolls `DUNGEON_ENCOUNTER_CHANCE` for an encounter after a step, and
    /// starts the fight if it hits.
    ///
    /// The pack is drawn from the biome of the **entrance tile** — the
    /// surface terrain the breach opens in. A dungeon has no biome of its
    /// own, and rather than invent one, this reads the level as the
    /// substrate beneath the ground above it: descend under a Mainframe
    /// sector and Mainframe programs are what live down there. It costs no
    /// new content and it gives the player a reason to care which breach
    /// they picked.
    ///
    /// Never a boss, for the same reason `maybe_ambush` refuses one: a fight
    /// you never saw coming should not also be the hardest fight available.
    fn maybe_dungeon_encounter(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let Some(pos) = self.dungeon_pos() else {
            return;
        };
        let encountered = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(DUNGEON_ENCOUNTER_CHANCE)
        };
        if !encountered {
            return;
        }

        let (ex, ey) = pos.entrance;
        let Some((species, _)) = self.pick_habitat_species(ex, ey, false) else {
            return;
        };
        // Spawned onto the breach tile itself: the party's `Position` is
        // pinned there, and a dungeon pack is resolved immediately rather
        // than left to roam, so where on the surface it stands never matters.
        let pack = self.spawn_pack(&species, false, ex, ey);
        if pack.is_empty() {
            return;
        }
        for &member in &pack {
            self.world.entity_mut(member).insert(DungeonSpawn);
        }
        self.remember_fight();
        self.log("Something moves in the dark ahead.".to_string());
        self.start_battle(pack);
    }

    /// The cell the party is standing on, or `None` on the surface.
    fn cell_underfoot(&self) -> Option<CellKind> {
        let pos = self.dungeon_pos()?;
        self.world
            .resource::<CurrentDungeon>()
            .0
            .as_ref()
            .map(|level| level.cell(pos.x, pos.y))
    }

    /// Goes down a level, if the party is standing on a way down.
    ///
    /// Separate from `ascend` rather than one key that does whichever the
    /// cell offers: descending is a commitment — the level below is harder
    /// and the way back is a walk — and a single key would sometimes take it
    /// when the player meant to leave.
    pub fn descend(&mut self) {
        if !self.can_act_underground() {
            return;
        }
        let (Some(pos), Some(cell)) = (self.dungeon_pos(), self.cell_underfoot()) else {
            return;
        };
        if cell != CellKind::StairsDown {
            // The bottom level is generated without stairs down at all, so
            // this is where a finished shaft reports itself. Saying so beats
            // "there's no way down here" on the one cell where the player
            // might reasonably keep looking for one.
            if pos.depth >= pos.floors {
                self.log("Solid bedrock. This shaft bottoms out here.".to_string());
            } else {
                self.log("There's no way down here.".to_string());
            }
            return;
        }
        self.descend_to(pos.depth + 1, pos.floors, pos.entrance);
        self.log(format!(
            "You descend to dungeon level {} of {}.",
            pos.depth + 1,
            pos.floors
        ));
        self.tick();
    }

    /// Goes up a level, if the party is standing on a way up. From depth 1
    /// that is the breach itself, and climbing it surfaces.
    pub fn ascend(&mut self) {
        if !self.can_act_underground() {
            return;
        }
        let (Some(pos), Some(cell)) = (self.dungeon_pos(), self.cell_underfoot()) else {
            return;
        };
        if cell != CellKind::StairsUp {
            self.log("There's no way up here.".to_string());
            return;
        }
        if pos.depth == 1 {
            self.leave_dungeon();
        } else {
            // Climbing lands on the level above's stairs *down*, not its
            // entry — otherwise every ascent would teleport the party back to
            // that level's entrance and undo the walk they just made.
            self.ascend_to(pos.depth - 1, pos.floors, pos.entrance);
            self.log(format!(
                "You climb back to dungeon level {}.",
                pos.depth - 1
            ));
        }
        self.tick();
    }

    /// Whether the cell underfoot offers a way down, and a way up — what the
    /// renderer greys the two stair prompts from, so it never advertises a
    /// key that would only log a refusal.
    pub fn stairs_available(&self) -> (bool, bool) {
        match self.cell_underfoot() {
            Some(CellKind::StairsDown) => (true, false),
            Some(CellKind::StairsUp) => (false, true),
            _ => (false, false),
        }
    }

    fn ascend_to(&mut self, depth: u32, floors: u32, entrance: (i32, i32)) {
        let level = dungeon::generate(self.level_spec(depth, floors, entrance));
        // Infallible: you can only climb *to* a level you already climbed
        // *from*, so it is not the bottom of the shaft and has a way down.
        let landing = level
            .stairs_down
            .expect("a level climbed up into must have the stairs that were climbed");
        self.world.insert_resource(CurrentDungeon(Some(level)));
        self.world.insert_resource(Locale::Dungeon {
            depth,
            floors,
            x: landing.0,
            y: landing.1,
            facing: Dir::North,
            entrance,
        });
        self.remember_view();
    }

    /// Restores a saved dungeon position, regenerating the level from the
    /// world seed and the saved spec rather than reading it off disk — see
    /// `resources::CurrentDungeon`.
    pub(crate) fn restore_locale(&mut self, locale: Locale) {
        if let Locale::Dungeon {
            depth,
            floors,
            entrance,
            ..
        } = locale
        {
            let level = dungeon::generate(self.level_spec(depth, floors, entrance));
            self.world.insert_resource(CurrentDungeon(Some(level)));
        }
        self.world.insert_resource(locale);
        self.remember_view();
    }

    pub(crate) fn locale(&self) -> Locale {
        *self.world.resource::<Locale>()
    }

    /// The party's map of the level they are in — see
    /// `views::DungeonMapView`. `None` on the surface.
    ///
    /// Drawn from `DungeonMemory` rather than from the level, so it shows
    /// what has been seen and not what is there. The level is consulted only
    /// to say what each *remembered* cell holds.
    pub fn dungeon_map(&self) -> Option<DungeonMapView> {
        let pos = self.dungeon_pos()?;
        let level = self.world.resource::<CurrentDungeon>().0.as_ref()?;
        let memory = self.world.resource::<DungeonMemory>();
        let seen = memory
            .0
            .get(&(pos.entrance, pos.depth))
            .map(|m| &m.seen)
            .cloned()
            .unwrap_or_default();

        let cells = (0..level.height)
            .map(|y| {
                (0..level.width)
                    .map(|x| {
                        if !seen.contains(&(x, y)) {
                            return DungeonMapCell::Unknown;
                        }
                        match level.cell(x, y) {
                            CellKind::Rock => DungeonMapCell::Rock,
                            CellKind::Floor => DungeonMapCell::Floor,
                            CellKind::StairsUp => DungeonMapCell::StairsUp,
                            CellKind::StairsDown => DungeonMapCell::StairsDown,
                            CellKind::Cache if self.cache_unopened(pos, (x, y)) => {
                                DungeonMapCell::Cache
                            }
                            CellKind::Cache => DungeonMapCell::Floor,
                            CellKind::Lair if !self.lair_cleared(pos) => DungeonMapCell::Lair,
                            CellKind::Lair => DungeonMapCell::Floor,
                            CellKind::Door => DungeonMapCell::Door,
                            CellKind::SealedDoor if self.seal_open(pos, (x, y)) => {
                                DungeonMapCell::Door
                            }
                            CellKind::SealedDoor => DungeonMapCell::SealedDoor,
                        }
                    })
                    .collect()
            })
            .collect();

        let mut marks: Vec<((i32, i32), DungeonMapMark)> = memory
            .0
            .get(&(pos.entrance, pos.depth))
            .map(|m| {
                m.fights
                    .iter()
                    .map(|&c| (c, DungeonMapMark::Fight))
                    .collect()
            })
            .unwrap_or_default();
        // Last, so a fight marker on the party's own cell doesn't hide them.
        marks.push(((pos.x, pos.y), DungeonMapMark::Party));

        let walkable = (0..level.height)
            .flat_map(|y| (0..level.width).map(move |x| (x, y)))
            .filter(|&(x, y)| level.walkable(x, y))
            .count();
        let walked = seen.iter().filter(|&&(x, y)| level.walkable(x, y)).count();

        Some(DungeonMapView {
            depth: pos.depth,
            floors: pos.floors,
            width: level.width,
            height: level.height,
            cells,
            marks,
            facing: pos.facing.label(),
            entrance: pos.entrance,
            explored: if walkable == 0 {
                0.0
            } else {
                walked as f32 / walkable as f32
            },
        })
    }

    /// The first-person view of the cells around the party, already rotated
    /// into view space — see `views::DungeonView`. `None` on the surface.
    pub fn dungeon_view(&self) -> Option<DungeonView> {
        let pos = self.dungeon_pos()?;
        let DungeonPos {
            depth,
            floors,
            x,
            y,
            facing,
            ..
        } = pos;
        let level = self.world.resource::<CurrentDungeon>().0.as_ref()?;

        let cells = view_cone(x, y, facing)
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|(cx, cy)| match level.cell(cx, cy) {
                        CellKind::Rock => DungeonCellView::Rock,
                        CellKind::Floor => DungeonCellView::Floor,
                        CellKind::StairsUp => DungeonCellView::StairsUp,
                        CellKind::StairsDown => DungeonCellView::StairsDown,
                        // An emptied cache is just an alcove. Still drawing
                        // one would send the player back down a dead end
                        // they have already walked.
                        CellKind::Cache if self.cache_unopened(pos, (cx, cy)) => {
                            DungeonCellView::Cache
                        }
                        CellKind::Cache => DungeonCellView::Floor,
                        CellKind::Lair if !self.lair_cleared(pos) => DungeonCellView::Lair,
                        CellKind::Lair => DungeonCellView::Floor,
                        CellKind::Door => DungeonCellView::Door,
                        CellKind::SealedDoor if self.seal_open(pos, (cx, cy)) => {
                            DungeonCellView::Door
                        }
                        CellKind::SealedDoor => DungeonCellView::SealedDoor,
                    })
                    .collect()
            })
            .collect();

        let standing_on = match level.cell(x, y) {
            CellKind::StairsDown => Some("Stairs lead down  [>] descend".to_string()),
            CellKind::StairsUp if depth == 1 => Some("The breach out  [<] surface".to_string()),
            CellKind::StairsUp => Some("Stairs lead up  [<] climb".to_string()),
            // Emptied on arrival rather than on a key, so this reports what
            // already happened rather than offering a choice.
            CellKind::Cache => Some("An empty casing".to_string()),
            CellKind::Lair => Some("The lair, and nothing left holding it".to_string()),
            CellKind::Door | CellKind::SealedDoor => Some("A doorway".to_string()),
            _ => None,
        };

        Some(DungeonView {
            depth,
            floors,
            facing: facing.label(),
            position: (x, y),
            cells,
            standing_on,
        })
    }
}
