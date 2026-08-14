//! Walking the Stack: getting in, moving around inside, and getting back
//! out.
//!
//! Everything here operates on `resources::Locale` and leaves the player's
//! `Position` component alone — see that resource's docs for why. The one
//! exception is `enter_stack`, which pins `Position` to the entrance tile
//! on the way in.

use bevy_ecs::system::SystemParam;

use crate::game::spawning::SpawnEscalation;
use crate::resources::{CurrentStack, Locale, Trace};
use crate::stack::{self, CellKind, Dir};
use crate::tuning::{
    STACK_COLLAPSE_RELINK_TILES, STACK_ENCOUNTER_CHANCE, STACK_FRAMES_MAX, STACK_FRAMES_MIN,
    STACK_LINK_SCATTER_TILES, STACK_MIN_LINK_TILES, STACK_NEAREST_LINK_TILES,
    STACK_TILES_PER_FRAME,
};
use crate::*;

/// The three resources that together say "the party is on the surface",
/// as a set — what leaving the Stack means, in one place.
///
/// `Game::clear_stack` is the door every player action goes through, but a
/// Forgiving reboot is decided inside `difficulty::death_handling_system`,
/// which has queries rather than a `Game` and so cannot call it. Returning
/// the values rather than writing them is what lets both a `&mut World` and
/// a set of `ResMut`s apply the same reset: a fourth resource added to this
/// tuple stops both callers compiling, which is the drift this exists to
/// prevent.
pub(crate) fn surfaced() -> (Locale, CurrentStack, Trace) {
    (Locale::Surface, CurrentStack(None), Trace::default())
}

/// Those same three resources as one system parameter, so a system that
/// has to leave the Stack asks for the set rather than for three separate
/// `ResMut`s it could take two of.
#[derive(SystemParam)]
pub(crate) struct StackLocale<'w> {
    locale: ResMut<'w, Locale>,
    current: ResMut<'w, CurrentStack>,
    trace: ResMut<'w, Trace>,
}

impl StackLocale<'_> {
    pub(crate) fn is_underground(&self) -> bool {
        matches!(*self.locale, Locale::Stack { .. })
    }

    /// The system-side twin of `Game::clear_stack`, sharing its
    /// implementation through `surfaced`.
    pub(crate) fn surface(&mut self) {
        (*self.locale, *self.current, *self.trace) = surfaced();
    }
}

/// Eight-point compass heading for an offset, with north as `-y` to match
/// `stack::Dir`.
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

/// The `Locale::Stack` payload, unpacked — see `Game::stack_pos`. A
/// named struct rather than a five-tuple so call sites read as fields
/// rather than as positions.
#[derive(Clone, Copy)]
pub(crate) struct StackPos {
    pub(crate) depth: u32,
    pub(crate) frames: u32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) facing: Dir,
    pub(crate) entrance: (i32, i32),
}

/// How many frames the stack under a link at `tile` runs before it
/// bottoms out.
///
/// Depth is read off the walk to the link rather than rolled, so it rides
/// the same distance-from-arrival that already scales wild program stats.
/// The two then agree — a far link is deeper *and* fields harder programs
/// — instead of a nearby hole occasionally being the deepest thing in the
/// sector for no reason the player could have seen coming.
///
/// `build_radius` is subtracted for that agreement to survive a base that
/// grows: `distance_from_danger_origin` already makes exactly this
/// correction, so the whole base counts as distance zero. Without it the
/// on-ramp being pushed out to the slab's edge would deepen every stack in
/// the sector as the player built Pillars — a difficulty change caused
/// entirely by a cosmetic one.
pub(crate) fn frames_for(tile: (i32, i32), spawn: (i32, i32), build_radius: i32) -> u32 {
    let distance = (tile.0 - spawn.0).abs().max((tile.1 - spawn.1).abs());
    let extra = ((distance - build_radius).max(0) / STACK_TILES_PER_FRAME) as u32;
    (STACK_FRAMES_MIN + extra).min(STACK_FRAMES_MAX)
}

/// The offset `along` steps clockwise around the Chebyshev ring of radius
/// `band`, starting from its north-west corner. `along` runs `0..8 * band`,
/// which is the ring's perimeter.
///
/// Shared by the two things that place a link on a ring — a zone's own
/// scatter and the replacement a collapse puts down — so the two cannot
/// disagree about what a ring is.
pub(crate) fn ring_offset(band: i32, along: i32) -> (i32, i32) {
    let side = 2 * band;
    match along / side {
        0 => (-band + along % side, -band),
        1 => (band, -band + along % side),
        2 => (band - along % side, band),
        _ => (-band, band - along % side),
    }
}

impl Game {
    /// Finds a `SurfaceLink` at `(x, y)`, if any — checked in
    /// `move_player` before the generic blocking-structure check, so walking
    /// onto one descends instead of just bumping into it.
    pub(crate) fn find_surface_link_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<SurfaceLink>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// Spawns `STACK_LINKS_PER_ZONE` links scattered around the
    /// player's arrival point, on walkable ground outside the base platform.
    /// Called once per zone, alongside `spawn_initial_creatures`.
    ///
    /// Platform tiles are skipped rather than merely unlikely: the base is
    /// the one safe ground in the game, and a hole down into the Stack in
    /// the middle of it would undo that.
    ///
    /// Draws from a locally seeded RNG rather than `resources::GameRng`, for
    /// two reasons. Where the entrances are is world generation, and world
    /// generation should be a function of the seed rather than of however
    /// many rolls happened to precede it — the same argument
    /// `stack::generate` makes. And drawing from the shared stream here
    /// would shift every later roll in the run, which is not a private
    /// matter: it silently rewrites the outcome of every seeded test in the
    /// suite. `ENTRANCE_SALT` keeps this stream off the one the frames
    /// themselves are carved from, so entrance placement and maze layout
    /// don't correlate.
    pub(crate) fn spawn_surface_links(&mut self, count: usize) {
        const ENTRANCE_SALT: u64 = 0xD07E_5A17;

        let origin = *self.world.get::<Position>(self.player_entity()).unwrap();
        let seed = self.world.resource::<WorldMap>().seed();
        let zone = self.world.resource::<ZoneLevel>().0;
        let mut rng = StdRng::seed_from_u64(((seed as u64) << 32) ^ zone as u64 ^ ENTRANCE_SALT);

        // The on-ramp is drawn from the ring just outside the slab rather
        // than from a box centred on the player, because the slab can eat
        // that box outright: the largest `|dx| + |dy|` in it is twice
        // `STACK_NEAREST_LINK_TILES`, and `Platform::covers` spares a tile
        // only while that exceeds `2 * radius - PLATFORM_CORNER_CUT`. Since
        // `reach` widens only once one link is down and the attempt budget
        // is shared across all three, an on-ramp that can never land does
        // not cost the zone its first link — it costs the zone every link,
        // which ends the run's access to Portal Fragments and reads as a bad
        // seed rather than as a bug.
        let inner = self.world.resource::<Platform>().radius + 1;

        let mut placed = 0;
        let mut attempts = 0;
        while placed < count && attempts < count * 40 {
            attempts += 1;
            // Every link is drawn from a band of Chebyshev rings starting
            // one tile outside the slab — the on-ramp from a narrow band,
            // the rest from a wide one. Both are measured *outward from the
            // base* rather than from the arrival point, which is what makes
            // them independent of how big the base has grown; a box centred
            // on the player is a box the slab can eat, and the whole zone
            // goes with it.
            //
            // The band is walked rather than rejection-sampled because every
            // attempt has to produce a candidate: the budget is shared, so
            // wasted draws come out of the links that follow.
            let width = if placed == 0 {
                STACK_NEAREST_LINK_TILES
            } else {
                STACK_LINK_SCATTER_TILES
            };
            let band = rng.random_range(inner..=inner + width);
            let (dx, dy) = ring_offset(band, rng.random_range(0..8 * band));
            if dx.abs().max(dy.abs()) < STACK_MIN_LINK_TILES {
                continue;
            }
            let (x, y) = (origin.x + dx, origin.y + dy);
            if !self.link_site_free(x, y) {
                continue;
            }
            self.spawn_entrance_at(x, y);
            placed += 1;
        }
        self.announce_surface_links(origin);
    }

    /// Whether a link may stand on `(x, y)` — the one statement of what a
    /// link needs from the ground under it, shared by a zone's scatter and
    /// by the replacement `collapse_stack` puts down.
    ///
    /// Platform tiles are refused rather than merely unlikely: the base is
    /// the one safe ground in the game, and a hole down into the Stack in
    /// the middle of it would undo that.
    ///
    /// A nest is checked before an entrance in `move_player`, so a link
    /// sharing a nest's tile is a link that can never be walked into — the
    /// bump attacks the nest instead, forever. Nests are already down by the
    /// time a zone scatters its links, in both `Game::new` and
    /// `enter_next_zone`.
    fn link_site_free(&mut self, x: i32, y: i32) -> bool {
        let tile = self.world.resource_mut::<WorldMap>().tile(x, y);
        tile.walkable
            && tile.biome != Biome::Platform
            && self.find_surface_link_at(x, y).is_none()
            && self.find_blocking_structure_at(x, y).is_none()
            && self.find_nest_at(x, y).is_none()
    }

    /// Logs what the arrival scan picks up: how many links are in the
    /// sector and where the nearest one lies.
    ///
    /// Without this the layer is invisible. Three unmarked holes scattered
    /// across an unbounded procedural field, in a game that has never had a
    /// reason to reward wandering, is not something a player finds — they
    /// have to be told the links are there before looking for one is a
    /// choice rather than an accident.
    fn announce_surface_links(&mut self, origin: Position) {
        let mut query = self.world.query_filtered::<&Position, With<SurfaceLink>>();
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
            "Deep scan: {} link{} in this sector. Nearest bears {} at {distance} tiles.",
            found.len(),
            if found.len() == 1 { "" } else { "s" },
            bearing(dx, dy),
        ));
    }

    /// Rebuilds the entrances a save recorded — see
    /// `save::SaveData::link_sites`.
    pub(crate) fn restore_surface_links(&mut self, tiles: Vec<(i32, i32)>) {
        for (x, y) in tiles {
            self.spawn_entrance_at(x, y);
        }
    }

    /// The one place an entrance entity is built, so a fresh zone's links
    /// and a reloaded save's cannot end up looking like different things.
    ///
    /// `>` rather than anything more decorative because every other glyph on
    /// the map is spoken for — `%` is both the Static Field biome and a
    /// species — and because it already reads as "the way down" to anyone
    /// who has played a roguelike. It is the same mark the Stack view puts
    /// on a link down.
    fn spawn_entrance_at(&mut self, x: i32, y: i32) {
        self.world.spawn((
            SurfaceLink,
            Position { x, y },
            Glyph {
                ch: '>',
                color: GlyphColor::Magenta,
            },
        ));
    }

    pub fn is_underground(&self) -> bool {
        matches!(self.world.resource::<Locale>(), Locale::Stack { .. })
    }

    /// `Err` if the party is underground — for actions that reach into the
    /// zone map through the player's `Position`.
    ///
    /// While underground that `Position` is pinned to the entrance tile
    /// (see `resources::Locale`), so these would otherwise operate on a tile
    /// out in the wild that the player is nowhere near: deploying a
    /// structure beside the link, or — worst of all — `use_symlink`
    /// teleporting the pinned entrance somewhere else and changing where
    /// climbing back up puts you.
    ///
    /// Party and inventory management deliberately isn't on this list.
    /// Fusing programs, installing routines, crafting, equipping and
    /// spending perk points all work fine four frames down, and stopping to
    /// sort your gear in the Stack is a thing the genre expects.
    pub(crate) fn require_surface(&self) -> Result<(), String> {
        if self.is_underground() {
            return Err("Not down here — that needs open grid.".into());
        }
        Ok(())
    }

    /// Descends into the Stack reached through the entrance standing at
    /// `(x, y)`, which the player is stepping onto. The entrance itself
    /// survives — unlike a zone portal, it is a place you can come back to.
    pub(crate) fn enter_stack(&mut self, x: i32, y: i32) {
        let player = self.player_entity();
        if let Some(mut pos) = self.world.get_mut::<Position>(player) {
            pos.x = x;
            pos.y = y;
        }
        let frames = self.frames_at((x, y));
        self.descend_to(1, frames, (x, y));
        self.log(format!(
            "You drop through the link. The signal above you thins to nothing. \
             This stack sounds {frames} frames deep."
        ));
    }

    /// How deep the stack under the link at `tile` runs — see
    /// `frames_for`, which this feeds the zone's arrival point and the width
    /// of the safe ground around it.
    ///
    /// The radius is the platform's or nothing, the same branch
    /// `distance_from_danger_origin` takes: with no Home deployed there is no
    /// safe territory to measure from and the walk is the whole distance.
    pub(crate) fn frames_at(&self, tile: (i32, i32)) -> u32 {
        let spawn = self.world.resource::<ZoneSpawnPoint>();
        let platform = self.world.resource::<Platform>();
        let radius = if platform.center.is_some() {
            platform.radius
        } else {
            0
        };
        frames_for(tile, (spawn.x, spawn.y), radius)
    }

    pub(crate) fn frame_spec(
        &self,
        depth: u32,
        frames: u32,
        entrance: (i32, i32),
    ) -> stack::FrameSpec {
        stack::FrameSpec {
            world_seed: self.world.resource::<WorldMap>().seed(),
            entrance,
            depth,
            frames,
        }
    }

    /// Generates the frame for `depth` and puts the party on the cell
    /// `landing` picks out of it, facing north.
    ///
    /// One function for all three ways into a frame — walking down a link,
    /// climbing up one, and falling through a fault — because they differ in
    /// exactly one thing and agree on the rest: generate, install the frame,
    /// rewrite the locale, remember what is now in view. Three copies of that
    /// spine is three places to fix an ordering bug in.
    ///
    /// `landing` takes the generated frame rather than a bare cell because
    /// two of the three callers cannot name their landing cell until the
    /// frame exists — an ascent lands on `link_down`, and a fall is placed by
    /// distance. Each caller's rule therefore stays at its own call site.
    fn enter_frame(
        &mut self,
        depth: u32,
        frames: u32,
        entrance: (i32, i32),
        landing: impl FnOnce(&stack::Frame) -> (i32, i32),
    ) {
        let level = stack::generate(self.frame_spec(depth, frames, entrance));
        let (x, y) = landing(&level);
        self.world.insert_resource(CurrentStack(Some(level)));
        self.world.insert_resource(Locale::Stack {
            depth,
            frames,
            x,
            y,
            facing: Dir::North,
            entrance,
        });
        self.remember_view();
        // After the view, so the frame's own line reads as the thing you
        // notice once you are standing in it. `enter_frame` is the one spine
        // every descent, ascent and fall goes through, which is why the line
        // fires here and not at each of them.
        if let Some(pos) = self.stack_pos()
            && let Some(line) = self.arrival_line(pos)
        {
            self.log(line);
        }
    }

    /// Arrives on the frame's entry cell — walking in from the surface, and
    /// every descent by link after it.
    ///
    /// `pub(crate)` for `arena::encounter`, which descends for real rather
    /// than faking a locale: it is the second caller, not a second way in.
    /// Both still go through `enter_frame`.
    pub(crate) fn descend_to(&mut self, depth: u32, frames: u32, entrance: (i32, i32)) {
        self.enter_frame(depth, frames, entrance, |level| level.entry);
    }

    /// Puts the party back on the zone map. The player's `Position` was
    /// pinned to the entrance tile the whole time, so there is nothing to
    /// restore.
    ///
    /// Unnarrated, because the link is no longer the only way out —
    /// `use_symlink` and `collapse_stack` each leave by their own route and
    /// say so in their own words.
    /// Trace resets here and nowhere else. That is safe for caches and seals
    /// — both are one-shot per stack and recorded in `FrameMemory`, so
    /// climbing out to shed Trace means returning to a stack with less left
    /// to take. The lair is the known exception: it stays un-spent until
    /// killed, so a looted-out party can climb out and walk their remembered
    /// map back down to meet the guardian in a lower band. The price is the
    /// walk — a descent through emptied frames, at no reward — and the spec
    /// records that as accepted rather than as a hole to plug.
    pub(crate) fn clear_stack(&mut self) {
        let (locale, stack, trace) = surfaced();
        self.world.insert_resource(locale);
        self.world.insert_resource(stack);
        self.world.insert_resource(trace);
    }

    /// Ends the stack reached through `entrance`: the party is thrown out,
    /// the way in caves in behind them, the run's record of the place goes
    /// with it, and the ground opens somewhere else nearby.
    ///
    /// Called from `end_battle` against `BattleState::cleared_lair`, so the
    /// party may already be on the surface by the time this runs — a
    /// Forgiving reboot on the round the guardian fell gets there first.
    /// Everything here is therefore written to work from either side, and
    /// the ejection is the one part that checks.
    ///
    /// **The site is found before the old link comes down**, and that
    /// ordering is the whole of the failure case. A zone with no link left
    /// is a run that can never breach again, since `award_loot` underground
    /// is the game's only source of Portal Fragments; that reads to a player
    /// as a bad seed rather than as a bug. So a search that finds nowhere
    /// legal skips the collapse entirely and leaves the stack standing —
    /// `FrameMemory::cleared` is what holds the guardian down in that
    /// branch, and it is the only branch where that record still has work to
    /// do.
    ///
    /// The trade keeps the sector's link count flat, so nothing downstream
    /// has to reason about a zone running dry.
    pub(crate) fn collapse_stack(&mut self, entrance: (i32, i32)) {
        let Some((nx, ny)) = self.replacement_link_site(entrance) else {
            return;
        };

        if self.stack_pos().is_some_and(|pos| pos.entrance == entrance) {
            self.clear_stack();
            self.log_kind(
                MessageKind::Outcome,
                "The stack folds in on itself. You are flung up through \
                 collapsing frames and land hard on open grid.",
            );
        }

        if let Some(link) = self.find_surface_link_at(entrance.0, entrance.1) {
            self.world.despawn(link);
        }
        // Keyed by `(link tile, depth)`, so this is every frame of the stack
        // that just fell and nothing else. What it holds is the run's record
        // of a place that no longer exists — see `resources::StackMemory`.
        self.world
            .resource_mut::<StackMemory>()
            .0
            .retain(|&(tile, _), _| tile != entrance);

        self.spawn_entrance_at(nx, ny);
        let (dx, dy) = (nx - entrance.0, ny - entrance.1);
        self.log_kind(
            MessageKind::Outcome,
            format!(
                "The ground answers somewhere else: a new link opens {} at {} tiles.",
                bearing(dx, dy),
                dx.abs().max(dy.abs()),
            ),
        );
    }

    /// The nearest legal tile to `from` a replacement link may stand on, or
    /// `None` if there is none inside `STACK_COLLAPSE_RELINK_TILES`.
    ///
    /// Rings are walked outward and the first legal tile wins, so the
    /// replacement opens as close to the collapse as the ground allows —
    /// the reading being that this ground shifted, not that a hole appeared
    /// across the sector. Where on a ring the walk *starts* is rolled, or
    /// every collapse in the game would open its replacement due north-west.
    ///
    /// The roll is a local `StdRng` rather than `resources::GameRng` for the
    /// reason `spawn_surface_links` gives: a draw from the shared stream
    /// shifts every later roll in the run, which silently rewrites the
    /// outcome of every seeded test in the suite. Salted off the collapsed
    /// tile so two stacks finished in one zone do not answer alike. Nothing
    /// needs this to be reproducible from the seed alone — unlike a frame,
    /// the tile is world state and rides out in `SaveData::link_sites`.
    fn replacement_link_site(&mut self, from: (i32, i32)) -> Option<(i32, i32)> {
        const RELINK_SALT: u64 = 0xC0A1_5E5D;

        let seed = self.world.resource::<WorldMap>().seed();
        let zone = self.world.resource::<ZoneLevel>().0;
        let mut rng = StdRng::seed_from_u64(
            ((seed as u64) << 32)
                ^ ((from.0 as u32 as u64) << 16)
                ^ (from.1 as u32 as u64)
                ^ zone as u64
                ^ RELINK_SALT,
        );

        for band in STACK_MIN_LINK_TILES..=STACK_COLLAPSE_RELINK_TILES {
            let perimeter = 8 * band;
            let start = rng.random_range(0..perimeter);
            for step in 0..perimeter {
                let (dx, dy) = ring_offset(band, (start + step) % perimeter);
                let (x, y) = (from.0 + dx, from.1 + dy);
                if self.link_site_free(x, y) {
                    return Some((x, y));
                }
            }
        }
        None
    }

    /// Climbs out through the link the party walked in through.
    fn leave_stack(&mut self) {
        self.clear_stack();
        self.log("You surface through the link, back onto open grid.".to_string());
    }

    /// The party's current cell and facing, or `None` on the surface.
    pub(crate) fn stack_pos(&self) -> Option<StackPos> {
        match *self.world.resource::<Locale>() {
            Locale::Surface => None,
            Locale::Stack {
                depth,
                frames,
                x,
                y,
                facing,
                entrance,
            } => Some(StackPos {
                depth,
                frames,
                x,
                y,
                facing,
                entrance,
            }),
        }
    }

    /// The party's cell in the frame they are standing in, or `None` on the
    /// surface. The public slice of `stack_pos` app-core needs to open a
    /// cell cursor where the party is standing.
    pub fn stack_pos_xy(&self) -> Option<(i32, i32)> {
        self.stack_pos().map(|pos| (pos.x, pos.y))
    }

    /// The current frame's `(width, height)`, or `None` on the surface —
    /// what a cell cursor clamps itself against, so an out-of-bounds
    /// coordinate is unreachable rather than lethal.
    pub fn frame_bounds(&self) -> Option<(i32, i32)> {
        self.world
            .resource::<CurrentStack>()
            .0
            .as_ref()
            .map(|level| (level.width, level.height))
    }

    /// Whether a Stack action can run at all: underground, alive, and not
    /// mid-intrusion. Mirrors the guard at the top of `move_player`.
    fn can_act_underground(&self) -> bool {
        self.is_underground() && self.is_game_over().is_none() && !self.has_active_battle()
    }

    fn set_facing(&mut self, dir: Dir) {
        if let Locale::Stack { facing, .. } = &mut *self.world.resource_mut::<Locale>() {
            *facing = dir;
        }
        self.remember_view();
    }

    pub fn turn_left(&mut self) {
        if !self.can_act_underground() {
            return;
        }
        let Some(pos) = self.stack_pos() else {
            return;
        };
        self.set_facing(pos.facing.turn_left());
        self.tick();
    }

    pub fn turn_right(&mut self) {
        if !self.can_act_underground() {
            return;
        }
        let Some(pos) = self.stack_pos() else {
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
        let Some(pos) = self.stack_pos() else {
            return;
        };
        let (dx, dy) = pos.facing.delta();
        let (nx, ny) = (pos.x + dx * sign, pos.y + dy * sign);

        let target = self
            .world
            .resource::<CurrentStack>()
            .0
            .as_ref()
            .map(|level| level.cell(nx, ny))
            .unwrap_or(CellKind::Rock);

        let walkable = target.walkable();
        // A sealed door opens by being walked into, and the record of that
        // has to be written before `remember_view` below draws the frame
        // with the party standing in the doorway.
        if walkable && target == CellKind::SealedDoor {
            self.force_seal(pos, (nx, ny));
        }

        if walkable && let Locale::Stack { x, y, .. } = &mut *self.world.resource_mut::<Locale>() {
            *x = nx;
            *y = ny;
        }
        // Before anything below can interrupt, so a corridor the party
        // walked into is on their map even if something jumps them the
        // moment they enter it. Unconditional because a blocked step still
        // faces the party at what they shoved into.
        self.remember_view();
        // Only a step that actually covered ground arrives anywhere —
        // shoving at a wall is not travel, the same call `move_player`
        // makes about an ambush.
        if walkable {
            self.arrive();
        }
        self.tick();
    }

    /// Everything arriving on a cell means, in the order it means it.
    ///
    /// There are three ways to arrive — a step, a phase through a wall, and
    /// a wild jump — and the same argument `enter_frame` makes about there
    /// being one way *into a frame* applies here to landing on a cell: they
    /// differ in how the party got there and agree completely on what
    /// happens next. Three copies of this tail is three places to fix an
    /// ordering bug in, and the ordering is the whole of it:
    ///
    /// - **Corruption first**, because it is a property of arriving rather
    ///   than something the cell offers, and if it kills the party the four
    ///   below all refuse on their own `is_game_over` checks.
    /// - **The fault before the encounter roll**, so a party that fell rolls
    ///   for an ambush in the frame they landed in rather than the one they
    ///   left. Landing somewhere strange and being jumped there is the right
    ///   reading of a fall.
    ///
    /// Every one of these is about the cell the party is standing on *now*:
    /// a lair roused without having arrived is a second full-HP guardian
    /// conjured by a party that jacked out of the first.
    ///
    /// Deliberately does **not** call `remember_view`. Every caller does
    /// that itself first, because a corridor the party moved into has to be
    /// on their map even if something jumps them the moment they enter it —
    /// and `step` calls it unconditionally, including on a blocked step,
    /// which is a rule about facing rather than about arriving.
    pub(crate) fn arrive(&mut self) {
        self.bleed_corruption();
        self.open_cache();
        self.rouse_lair();
        self.trip_breakpoint();
        self.take_fault();
        self.maybe_stack_encounter();
    }

    /// What Stack depth multiplies wild program stats by — `1.0` on the
    /// surface, so `spawn_wild_creature` can fold it in unconditionally.
    ///
    /// Compounds with `ZoneLevel::stat_multiplier` and
    /// `distance_stat_multiplier` rather than replacing them: a link far
    /// out in a deep zone is a nastier hole than one beside your base, and
    /// going down makes either worse.
    /// The depth term alone is `stack::depth_stat_multiplier`, which a
    /// market's quote is priced off: a price that moved because the party
    /// had been noisy would change while the player was reading it.
    pub(crate) fn stack_depth_multiplier(&self) -> f32 {
        match self.stack_pos() {
            None => 1.0,
            Some(pos) => stack::depth_stat_multiplier(pos.depth) * self.trace_stat_mult(),
        }
    }

    /// Everything an underground spawn escalates by, gathered in one place
    /// so the two callers that conjure a pack down here — an ambush and a
    /// lair guardian — cannot disagree about it.
    pub(crate) fn stack_escalation(&self, depth: u32) -> SpawnEscalation {
        SpawnEscalation {
            stat_mult: self.stack_depth_multiplier(),
            group_mult: self.trace_group_mult(),
            depth: Some(depth),
        }
    }

    /// The pack a Stack ambush fields: one species pick per group the depth
    /// has earned (`Game::max_enemy_groups`), each rolled to its own size,
    /// all tagged `StackSpawn` for `end_battle` to sweep.
    ///
    /// A pick *per group* rather than one pick scaled up, because
    /// `group_pack` groups by **species**: a single draw is a single group
    /// however many the curve allows, so raising the count alone would have
    /// been the same shape of no-op that scaling only the spawn was for
    /// `TRACE_GROUP_MULT`. Repeat picks are deliberately not filtered out —
    /// a thin biome may offer only one species, and two draws of it making
    /// one wider group is the honest answer rather than a reason to refuse
    /// the second.
    pub(crate) fn stack_encounter_pack(&mut self) -> Vec<Entity> {
        let Some(pos) = self.stack_pos() else {
            return Vec::new();
        };
        // Spawned onto the link tile itself: the party's `Position` is
        // pinned there, and a Stack pack is resolved immediately rather
        // than left to roam, so where on the surface it stands never matters.
        let (ex, ey) = pos.entrance;
        let esc = self.stack_escalation(pos.depth);
        let groups = self.max_enemy_groups(esc.depth);
        let mut pack = Vec::new();
        for _ in 0..groups {
            let Some((species, _)) = self.pick_habitat_species(ex, ey, false) else {
                continue;
            };
            pack.extend(self.spawn_pack(&species, false, ex, ey, esc));
        }
        for &member in &pack {
            self.world.entity_mut(member).insert(StackSpawn);
        }
        pack
    }

    /// Rolls `STACK_ENCOUNTER_CHANCE` for an encounter after a step, and
    /// starts the fight if it hits.
    ///
    /// The pack is drawn from the biome of the **entrance tile** — the
    /// surface terrain the link opens in. The Stack has no biome of its
    /// own, and rather than invent one, this reads the frame as the
    /// substrate beneath the ground above it: descend under a Mainframe
    /// sector and Mainframe programs are what live down there. It costs no
    /// new content and it gives the player a reason to care which link
    /// they picked.
    ///
    /// Never a boss, for the same reason `maybe_ambush` refuses one: a fight
    /// you never saw coming should not also be the hardest fight available.
    fn maybe_stack_encounter(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        if self.stack_pos().is_none() {
            return;
        }
        let mult = self.trace_encounter_mult();
        let encountered = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0
                .random_bool((STACK_ENCOUNTER_CHANCE * mult).clamp(0.0, 1.0))
        };
        if !encountered {
            return;
        }

        let pack = self.stack_encounter_pack();
        if pack.is_empty() {
            return;
        }
        self.remember_fight();
        self.log("Something moves in the dark ahead.".to_string());
        self.start_battle(pack);
    }

    /// The cell the party is standing on, or `None` on the surface.
    pub(crate) fn cell_underfoot(&self) -> Option<CellKind> {
        let pos = self.stack_pos()?;
        self.world
            .resource::<CurrentStack>()
            .0
            .as_ref()
            .map(|level| level.cell(pos.x, pos.y))
    }

    /// Goes down a frame, if the party is standing on a way down.
    ///
    /// Separate from `ascend` rather than one key that does whichever the
    /// cell offers: descending is a commitment — the frame below is harder
    /// and the way back is a walk — and a single key would sometimes take it
    /// when the player meant to leave.
    pub fn descend(&mut self) {
        if !self.can_act_underground() {
            return;
        }
        let (Some(pos), Some(cell)) = (self.stack_pos(), self.cell_underfoot()) else {
            return;
        };
        if cell != CellKind::LinkDown {
            // The bottom frame is generated without a link down at all, so
            // this is where a finished stack reports itself. Saying so beats
            // "there's no way down here" on the one cell where the player
            // might reasonably keep looking for one.
            if pos.depth >= pos.frames {
                self.log("Solid bedrock. This stack bottoms out here.".to_string());
            } else {
                self.log("There's no way down here.".to_string());
            }
            return;
        }
        self.descend_to(pos.depth + 1, pos.frames, pos.entrance);
        self.log(format!(
            "You descend to frame {} of {}.",
            pos.depth + 1,
            pos.frames
        ));
        self.tick();
    }

    /// Goes up a frame, if the party is standing on a way up. From depth 1
    /// that is the link itself, and climbing it surfaces.
    pub fn ascend(&mut self) {
        if !self.can_act_underground() {
            return;
        }
        let (Some(pos), Some(cell)) = (self.stack_pos(), self.cell_underfoot()) else {
            return;
        };
        if cell != CellKind::LinkUp {
            self.log("There's no way up here.".to_string());
            return;
        }
        if pos.depth == 1 {
            self.leave_stack();
        } else {
            // Climbing lands on the frame above's link *down*, not its
            // entry — otherwise every ascent would teleport the party back to
            // that frame's entrance and undo the walk they just made.
            self.ascend_to(pos.depth - 1, pos.frames, pos.entrance);
            self.log(format!("You climb back to frame {}.", pos.depth - 1));
        }
        self.tick();
    }

    /// Whether the cell underfoot offers a way down, and a way up — what the
    /// renderer greys the two link prompts from, so it never advertises a
    /// key that would only log a refusal.
    pub fn links_available(&self) -> (bool, bool) {
        match self.cell_underfoot() {
            Some(CellKind::LinkDown) => (true, false),
            Some(CellKind::LinkUp) => (false, true),
            _ => (false, false),
        }
    }

    fn ascend_to(&mut self, depth: u32, frames: u32, entrance: (i32, i32)) {
        self.enter_frame(depth, frames, entrance, |level| {
            // Infallible: you can only climb *to* a frame you already climbed
            // *from*, so it is not the bottom of the stack and has a way down.
            level
                .link_down
                .expect("a frame climbed up into must have the link that was climbed")
        });
    }

    /// Drops the party through a fault onto the frame below, landing in its
    /// far half rather than on its entry — see `stack::fault_landing`.
    ///
    /// The fall is a descent the party did not choose, so it is narrated as
    /// one rather than passing silently, and it costs no Trace: falling
    /// through a rotten floor is clumsy, not loud, and Trace is a meter for
    /// what the party takes rather than for what happens to them.
    pub(crate) fn fall_to(&mut self, depth: u32, frames: u32, entrance: (i32, i32)) {
        let spec = self.frame_spec(depth, frames, entrance);
        self.enter_frame(depth, frames, entrance, |level| {
            stack::fault_landing(level, spec).unwrap_or(level.entry)
        });
        self.log_kind(
            MessageKind::Outcome,
            format!("The floor gives way. You come down hard on frame {depth}."),
        );
    }

    /// Restores a saved Stack position, regenerating the frame from the
    /// world seed and the saved spec rather than reading it off disk — see
    /// `resources::CurrentStack`.
    pub(crate) fn restore_locale(&mut self, mut locale: Locale) {
        if let Locale::Stack {
            depth,
            frames,
            entrance,
            ref mut x,
            ref mut y,
            ..
        } = locale
        {
            let level = stack::generate(self.frame_spec(depth, frames, entrance));
            // A frame is regenerated, not saved, so a change to the generator
            // moves the walls under an old save — and a party restored into
            // rock is stuck in it. Rock is the one kind that is both
            // unwalkable and sight-blocking, so they cannot step out, the
            // first-person view fills with wall and the map truncates to
            // their own row. The entry is the one cell every layout
            // guarantees, and it is where they came in.
            if !level.walkable(*x, *y) {
                (*x, *y) = level.entry;
            }
            self.world.insert_resource(CurrentStack(Some(level)));
        }
        self.world.insert_resource(locale);
        // Silent: this is a reload, and every sighting in this view was
        // already announced in the session that saved it.
        self.remember_view_silent();
    }

    pub(crate) fn locale(&self) -> Locale {
        *self.world.resource::<Locale>()
    }
}
