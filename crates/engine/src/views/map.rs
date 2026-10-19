use super::*;

#[derive(Clone)]
pub struct EntityView {
    /// A configurable machine standing with nothing set to make.
    pub unconfigured: bool,
    pub entity: Entity,
    pub pos: (i32, i32),
    pub glyph: char,
    /// The resolved name a sprite loader looks this entity up by —
    /// `SpeciesDef::sprite_name` for a creature, `StructureDef::sprite_name`
    /// for a structure, `None` for anything that is neither. Always the
    /// *resolved* name (the override when authored, the def's own id
    /// otherwise), never `def.sprite` directly — see `sprite_name`'s doc
    /// comment on both defs for the one place that fallback is written.
    pub sprite: Option<String>,
    /// This entity's **authored** hue — what it is, never how dangerous it
    /// is. See `difficulty` below for the reading that used to replace it.
    pub color: GlyphColor,
    /// How badly this (hostile) entity would beat the player, bucketed by
    /// `game::inspection::difficulty_color` — `None` for everything that is
    /// not hostile, which is what stops the map drawing a con bar under a
    /// companion.
    ///
    /// **Its own reading, and the map decides where it lands.** This used to
    /// *replace* `color` for a hostile, so a tile could say either what a
    /// program is or how dangerous it is, never both — and a boss or a
    /// nemesis gave up the danger read entirely to a reserved hue.
    /// `render/base.rs`'s `ConRead` spends the glyph's own ink on it wherever
    /// that ink is free and folds it into a top-left earmark where it is
    /// not, `rarity` keeps the top edge and identity rides corner marks.
    /// **Not the bottom edge**, which the con bar held once and
    /// `job_progress` holds now.
    pub difficulty: Option<GlyphColor>,
    pub label: String,
    pub is_player: bool,
    /// The player's chosen sprite and colour — `Some` exactly when
    /// `is_player` is true, `None` for every other entity. See
    /// `PlayerLook`.
    pub look: Option<PlayerLook>,
    /// What the player draws instead of `look`/`glyph` while emulating —
    /// see `FormLook`. `None` for every entity that is not the emulating
    /// player, `look`'s own shape.
    pub form: Option<FormLook>,
    pub is_tamed: bool,
    pub is_companion: bool,
    pub is_hostile: bool,
    pub is_structure: bool,
    /// Whether this entity is a fixture that stays drawn (dim) once seen:
    /// a settlement, the base anchor, a nest or a Stack link. Creatures,
    /// caravans, patrols and traps are not; they show only in sight. Read
    /// by `shown_at`.
    pub is_landmark: bool,
    /// Whether this entity is the base anchor — `components::BaseAnchor`,
    /// the permanent door into base space that `Game::new` spawns under the
    /// party.
    ///
    /// It is neither a creature nor a `Structure`, so it is the one map
    /// fixture `is_player`, `is_hostile` and `is_structure` all say nothing
    /// about; without this a frontend had to recognise it by its glyph.
    /// Distinct from `is_home` below, which names the Home *building* — the
    /// two stand in different spaces and only one of them is ever drawn on
    /// the zone map.
    pub is_anchor: bool,
    /// Whether this (structure) entity is the base's Home — the anchor for
    /// the 15-tile build radius, and the one whose removal cascades to
    /// every other structure (see `Game::remove_structure`).
    pub is_home: bool,
    /// This (structure) entity's upgrade tier, or `None` if its def
    /// declares no upgrade path — see `StructureDef::upgrade`. Frontends
    /// use `Some` as "this is upgradeable" when listing candidates.
    pub tier: Option<u32>,
    /// The highest tier this (structure) entity can currently reach, from
    /// `Game::upgrade_ceiling` — `None` alongside a `None` `tier`.
    pub ceiling: Option<u32>,
    /// The highest tier this (structure) entity could *ever* reach, from
    /// `StructureDef::upgrade`'s `max_tier` — `None` alongside a `None`
    /// `tier`. Carried next to `ceiling` because the two together are what
    /// tell a menu row why a structure has stopped climbing: `ceiling` below
    /// `max_tier` means "breach first", `ceiling` equal to it means
    /// "finished". Neither value alone distinguishes those.
    pub max_tier: Option<u32>,
    /// Whether this (structure) entity has a drop pod charge ready to
    /// spend — `components::DropPod`, `tier`'s own shape: `None` for a
    /// structure whose def declares no `drop_pod` at all, `Some` for the
    /// one that does.
    pub pod_charged: Option<bool>,
    pub is_boss: bool,
    /// Whether this (creature) entity has beaten the party or driven them
    /// off — see `components::Nemesis`. Wins the glyph colour in
    /// `difficulty_color` over both the power-ratio con read and the boss
    /// override; see `rarity`'s doc for what that costs.
    pub nemesis: bool,
    /// The name of the town this (creature) entity patrols for — see
    /// `components::TownPatrol`. `None` for everything else.
    ///
    /// **A fourth channel, and it touches none of the other three.** The
    /// authored `color` still says what the program is, `difficulty` still
    /// says how dangerous it is and `rarity` still says how rare — a patrol
    /// mark may not spend any of them, which is the regression the HUD's
    /// separate-readings rule exists to stop.
    ///
    /// The town's *name* and not a bool: the mark raises the question "whose
    /// is it", and a flag draws the mark without answering it.
    pub patrol: Option<String>,
    pub can_work: bool,
    /// Whether this (structure) entity is a trading post (see
    /// `StructureDef::trade`).
    pub can_trade: bool,
    /// Whether this (structure) entity is a Contract Broker (see
    /// `StructureDef::issues_contracts`). The same shape `can_trade` has, and
    /// read the same way — a frontend finds a Broker by scanning for this
    /// rather than by naming the structure id.
    pub issues_contracts: bool,
    /// If this is a structure, the label of the (tamed) entity currently
    /// working it via cronjob, if any.
    pub structure_worker: Option<String>,
    /// Whether a frontend draws the "somebody is on this job" mark on this
    /// (tamed) entity rather than on the far end of its posting.
    ///
    /// True whenever the far end has no glyph to carry it — a `DigSite` has
    /// none at all and a `BuildSite` is not a `Structure`, so a digger and a
    /// builder each wear it for the whole job — and, for a machine's own
    /// worker, only while it is away from its post: at it, the body belongs
    /// *under* the machine's glyph, so a base at rest reads as buildings
    /// with motion the only thing that draws the eye. A guard is never
    /// walked and never drawn, so its structure keeps the mark always.
    ///
    /// `Game::mark_sits_on_the_post` is the rule and `structure_attended`
    /// below is its other half — exactly one mark per posted program, by
    /// construction rather than by two comments agreeing.
    pub wears_job_mark: bool,
    /// Whether this entity's `Position` is a tile the sim actually keeps up
    /// to date — the input `views::drawn_on_surface_map` takes.
    ///
    /// **A drawn program and a marked program are the same set**, plus one
    /// case: `wears_job_mark` above, or an idle base staff member, which
    /// `schedule_base_labour` parks on a tile every tick while it is on no
    /// job at all and so has no mark to wear. A guard keeps whatever tile it
    /// was on when it took the job, and a party companion keeps the tile it
    /// was beaten on — neither is ever written again, so drawing either
    /// would claim it is somewhere it isn't.
    pub position_is_honest: bool,
    /// If this is a structure, whether a posted program is standing at it
    /// right now — a guard (which never moves, so always) or a worker that
    /// has not stepped off on an errand.
    ///
    /// The other half of `wears_job_mark`, and literally its negation: both
    /// are `Game::mark_sits_on_the_post`, so a frontend draws the mark on
    /// the program when the program is drawn and on the structure when it
    /// isn't. **A build site never carries it**, because a `BuildSite` is
    /// not a `Structure` and this field is gated on that — which is why the
    /// builder wears it for the whole job. Distinct from `structure_worker`,
    /// which counts any `Task` wherever its holder happens to be.
    pub structure_attended: bool,
    /// Whether this is a downed program in reach of a Repair Bay right now,
    /// so its Integrity is climbing this tick — see
    /// `Game::recovering_programs`.
    ///
    /// **A fact about the body, not about the Bay**, which is the opposite
    /// end from `structure_attended` above. A Bay runs no job, so it has no
    /// `MachineStatus` to be in, and the program lying in it holds no `Task`,
    /// so nothing in the posting vocabulary describes either end — but only
    /// one of the two is the thing being mended, and that is what the map's
    /// mark is about. Reading it off the Bay also could not survive
    /// `RecoveryDef::radius` growing past zero, where one Bay mends several
    /// bodies at once and a single mark on the building says nothing about
    /// which.
    ///
    /// **Not "is this program `Downed`."** A body lying out of reach of every
    /// Bay — or with no Bay standing at all — is benched, not recovering, and
    /// wears nothing. Derived per call for `structure_attended`'s reason.
    pub recovering: bool,
    /// Whether this is the player with a Refit Bench mending their gear this
    /// tick — derived from `Game::refit_bench_serving`, the predicate the
    /// pass itself acts on, so the mark cannot claim a mend that is not
    /// happening. Never true for a program: the bench serves the player's
    /// stand, and the mark rises off their tile alone.
    pub refitting: bool,
    /// Whether this is a staff program holding a place at an amenity right
    /// now — a Sandbox, a Log Analyzer Bay — so one of its need reserves is
    /// climbing this tick. `recovering`'s shape for the other refill: a fact
    /// about the body, read off the same `Needs::serviced_at` the refill in
    /// `needs_drain_system` wrote, so the mark cannot claim a refill that is
    /// not happening. A body still walking to its amenity, or standing at
    /// one with no place free, wears nothing.
    pub serviced: bool,
    /// If this is a structure, whether its output buffer is full while
    /// nothing in the base can take a load — no depot built, or every depot
    /// already full.
    ///
    /// The dead end a base can sit in indefinitely: `haul_step_system`
    /// starts an errand only when a depot with room exists, so the worker
    /// never leaves and the machine never drains. Deliberately *not* a sixth
    /// `MachineStatus` — that enum is one machine's own state, and this is a
    /// fact about every depot at once, so folding it in would stop the enum
    /// meaning one thing and force a precedence call against all five
    /// existing variants.
    ///
    /// Keyed on room rather than on a depot existing, because a depot that
    /// has filled up is no better than no depot.
    pub output_stranded: bool,
    /// A structure other than a Depot whose finished-output buffer holds
    /// anything. The map's "the items are in here, not in storage" dot.
    /// Finished output only: staged input is never marked, and a banked
    /// output never reaches `Stock::output`, so it never lights this.
    pub holds_output: bool,
    pub hp_fraction: Option<f32>,
    pub level: Option<u32>,
    /// If this is a structure, its current/max raid `Durability`.
    pub durability: Option<(u32, u32)>,
    /// How many fusions deep this (creature) entity's lineage is, 0 to
    /// `MAX_FUSIONS` — see `components::FusionCount`. At `MAX_FUSIONS` it
    /// can no longer be an input to a fusion, which the fuse menus show.
    pub fusions: u32,
    /// This (creature) entity's rare-spawn tier — see `components::Rarity`.
    ///
    /// The map draws it as a bar along the top edge of the tile rather than
    /// by recolouring the glyph, because `color` above is already carrying
    /// `difficulty_color` for a hostile: how dangerous something is and how
    /// rare it is are two readings, and the glyph can only hold one — with
    /// one deliberate exception. A nemesis (see `nemesis` above) spends that
    /// reading on purpose: `difficulty_color` returns a reserved colour for
    /// one regardless of power ratio, because you have already fought this
    /// one and it has since gotten stronger, which makes "can I win this
    /// fight" the least informative thing its tile could say. Rarity still
    /// never gets the glyph — it keeps the bar, nemesis or not.
    pub rarity: Rarity,
    /// Why this (structure) entity is or isn't producing, or `None` for
    /// anything that runs no job and so has no state to be in. Lets the map
    /// colour a machine's outline by what it is doing.
    pub machine_status: Option<MachineStatus>,
    /// How far along the work being done on this cell is, 0.0..=1.0, or
    /// `None` where no work is being done on it at all.
    ///
    /// **A quantity on a channel that only ever carried a yes or a no.**
    /// `machine_status` above says *what state* a machine is in and
    /// `structure_attended` says *whether anyone is on it*; neither says how
    /// far through the job is, which is the one thing a base at work has no
    /// way to show.
    ///
    /// Two kinds of cell answer, and each answer is a *call* rather than a
    /// second formula:
    ///
    /// - a structure whose posted program is turning its production cycle —
    ///   `Task::progress` against `Task::required`. Only the one `TaskKind`
    ///   whose meter is a machine's own cycle counts: a guard's never moves,
    ///   a digger's is the swing cadence rather than the cut (that one is
    ///   `Game::marked_cells`), and a builder's is unused, a site's progress
    ///   living on `BuildSite::progress`.
    /// - a build site — `BuildOrderRow::percent`, which is already the one
    ///   derivation of how far along a request is.
    ///
    /// **One bar to a cell, and a machine's own cycle wins.** A machine with
    /// a pending upgrade carries that site's row in `build` below and is
    /// still producing; the tile draws the machine, and its glyph and
    /// outline are about the machine, so its bar is too. The upgrade's own
    /// figure is on the examine page.
    pub job_progress: Option<f32>,
    /// How full this Depot is, or `None` for anything that is not one — see
    /// `DepotFill`.
    pub depot_fill: Option<DepotFill>,
    /// The build request this entity *is*, or `None` for everything that is
    /// not one — see `views::BuildOrderRow`.
    ///
    /// `Some` here is what tells the renderer to paint a site's frame rather
    /// than a glyph on bare floor, and it is what the examine page reads to
    /// say what the crew is still carrying. Carried on the view rather than
    /// asked for separately so the map and the inspector cannot come to
    /// disagree about which cell is a pending build.
    pub build: Option<BuildOrderRow>,
    /// The orthogonal offsets of neighbours this (structure) entity is
    /// joined to for production — the sides the map leaves un-outlined, so
    /// that a chain draws as one continuous shape and a machine that should
    /// be joined and isn't shows a seam.
    ///
    /// Symmetric, so both walls of a joined pair come down together; the
    /// feeding relation underneath is directional. See
    /// `Game::linked_edges_by_structure`.
    ///
    /// Deliberately a property of the *defs*, not of what is in a buffer
    /// right now: it answers "is this feeder joined to me", not "did a unit
    /// move this tick". A healthy chain drains its feeder within a tick or
    /// two, so a live-transfer marker would be dark most of the time and a
    /// correctly-built line would look identical to a broken one. A missing
    /// join therefore always means the base is laid out wrong.
    pub linked_edges: Vec<(i32, i32)>,
    /// Whether this (creature) entity is a wild program the Alt marker
    /// should flag — `Game::unseen_routine`, computed through the same
    /// candidate function `routine_candidates` calls so the two can never
    /// promise different things. Always false for anything the player owns.
    pub unseen_routine: bool,
}

/// Whether the surface map draws this entity at all — the rule stated once
/// so that what the player can *see* and what the inspector can *name* are
/// the same set.
///
/// It says: everything untamed is drawn, and a tamed program only while its
/// `Position` is one the sim keeps honest — drawing any other would claim
/// it is somewhere it isn't.
///
/// The second parameter widened from "is this worker away from its post" on
/// 2026-08-14, when base staff arrived, and again on 2026-08-27, when it
/// turned out to have been reading `TaskKind::GatherResource` alone: a
/// builder and a digger each held a post the sim walked them along and were
/// drawn nowhere for the whole job, so filing a build request made a program
/// vanish and a structure appear. It is `Game::position_is_honest` now,
/// which is `wears_job_mark` plus idle base staff. A party companion still
/// has neither — its `Position` is the tile it was beaten on, written at
/// capture and never again — so it is still not drawn.
/// `EntityView::position_is_honest` is the value.
///
/// **A pure function shared by two crates rather than a condition written
/// twice.** `render/base.rs` filters the map with it, and
/// `Game::find_target_in_direction` filters its ray with it. They used to
/// disagree: the ray had no such rule, so aiming at a machine hit the worker
/// parked in front of it — a program with no glyph on screen — while the
/// machine's own glyph sat under the cursor. Per `CLAUDE.md`, a claim that
/// two places use the same rule has to be a call, not a comment.
pub fn drawn_on_surface_map(is_tamed: bool, position_is_honest: bool) -> bool {
    !is_tamed || position_is_honest
}

/// What the player sees of one surface tile — see `Game::sight_at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sight {
    Unseen,
    Remembered,
    InSight,
}

/// Whether a thing is drawn or addressable at `sight`.
///
/// Party members always show; landmarks (settlements, the base anchor,
/// nests, Stack links) show once seen; everything else shows only in sight.
/// The rule, so `shown_at` and `Game::find_target_in_direction`'s raw
/// queries cannot drift.
pub fn shown_in(sight: Sight, in_party: bool, is_landmark: bool) -> bool {
    in_party
        || match sight {
            Sight::InSight => true,
            Sight::Remembered => is_landmark,
            Sight::Unseen => false,
        }
}

/// [`shown_in`] for a built view. Outposts are not entities and are always
/// drawn from `outpost_marks`, so they never come through here.
pub fn shown_at(sight: Sight, view: &EntityView) -> bool {
    shown_in(sight, view.is_player || view.is_companion, view.is_landmark)
}

/// One cell of the Excavation plan — see `Game::marked_cells`.
///
/// **A struct rather than the bare coordinate it used to be**, because a
/// mark now carries how far through the wall the crew is and the renderer
/// draws that beside the wash. A tuple would have left the two figures
/// telling apart only by position.
///
/// `cut` is the fraction of the wall already taken down, 0.0..=1.0, and
/// `None` on a cell that is no longer a wall: a marked `Open` cell is the
/// other half of the one dig verb — floor it — and has no cut left to be
/// part-way through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DigMark {
    /// **Base-space** coordinates.
    pub pos: (i32, i32),
    pub cut: Option<f32>,
}

/// One farm cell as the base map draws it — see `Game::farm_tiles`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FarmTileView {
    /// **Base-space** coordinates.
    pub pos: (i32, i32),
    pub plot: Entity,
    /// The recipe's glyph and colour; `None` when the plot is unsown.
    pub crop: Option<(String, GlyphColor)>,
    pub state: FarmTileState,
}

/// How far a farm cell has grown, as a look rather than a count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FarmTileState {
    Fallow,
    /// `stage` is in `0..stages`.
    Growing {
        stage: u8,
        stages: u8,
    },
    Ripe,
}

/// One room with a role, for the base map's Alt overlay — see
/// `Game::view_rooms`. Rooms with no role (the commons, bare space) are not
/// listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomView {
    /// The role's display name, e.g. "quarters".
    pub role: String,
    pub band: RoomBand,
    /// **Base-space** cells, sorted.
    pub cells: Vec<(i32, i32)>,
    /// The cell of `cells` nearest the room's centre, where a label goes.
    pub label_at: (i32, i32),
    /// The role's overlay colour, as authored in its `RoomDef`.
    pub tint: (u8, u8, u8),
}

/// One floor item as the base map draws it — see `Game::floor_items`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FloorItemView {
    /// **Base-space** coordinates.
    pub pos: (i32, i32),
    pub look: FloorLook,
    /// Display name, for the examine line.
    pub name: String,
}

/// What the player chose to set on the floor. A program is named by its
/// index in the downed store, since two programs can look alike.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PutDownChoice {
    Item(ItemId),
    Gear(GearCopy),
    Program(usize),
}

/// One row of the put-down picker — see `Game::put_down_options`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PutDownOption {
    pub label: String,
    pub choice: PutDownChoice,
}

/// What a floor item draws as. Gear uses its item's `floor_glyph()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloorLook {
    Item(char),
    Program,
}

/// What one cell of the first-person view cone contains — see
/// `StackView::cells`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackCellView {
    /// Solid, and what everything past the edge of the frame reads as. The
    /// renderer draws this as a wall face.
    Rock,
    Floor,
    LinkUp,
    LinkDown,
    /// An unopened cache. One that has already been emptied comes through
    /// as `Floor` — still drawing it would send the party back down a dead
    /// end they have already walked.
    Cache,
    /// The stack's lair, still held. A cleared one comes through as `Floor`.
    Lair,
    /// A doorway. Walkable, but the view stops here — a door is drawn as a
    /// face, not as more corridor. A sealed door already burned open comes
    /// through as this.
    Door,
    /// A door that still wants an access shard.
    SealedDoor,
    /// An unused debug port. A spent one comes through as `Floor`, on the
    /// same argument as an emptied cache — the frame it maps is already
    /// mapped, so advertising it again is a walk for nothing.
    Breakpoint,
    /// A hole down to the next frame. Never spent, so it always draws.
    Fault,
    /// Rotten substrate, which costs HP to cross. Always drawn: unlike a
    /// cache this is a warning rather than a reward, and one the party needs
    /// every time they consider the route, not once.
    Corruption,
    /// A program waiting to be adopted. One already taken comes through as
    /// `Floor`, on the same argument as an emptied cache — the dead end has
    /// nothing left in it, so advertising it is a walk for nothing.
    Orphan,
    /// Somebody selling things. One whose every row has been bought comes
    /// through as `Floor` — see `Game::market_live`.
    Market,
}

/// The party's first-person view of the frame around them — see
/// `Game::stack_view`.
///
/// `cells` is already rotated into **view space**: `cells[ahead][lateral]`,
/// where `ahead` counts cells away from the party (0 is the cell they stand
/// in) and `lateral` runs left to right across the cone, with
/// `STACK_VIEW_HALF_WIDTH` in the middle. The engine does that rotation so
/// the renderer only ever draws a forward-facing corridor and never learns
/// which way north is — the same contract `ActionOption` has, where the
/// renderer draws verbatim and authors nothing.
pub struct StackView {
    pub depth: u32,
    /// How many frames this stack runs in total, so the renderer can show
    /// "2 / 4" and the player can tell a long descent from a short one
    /// without counting frames.
    pub frames: u32,
    /// The party's world facing; `.label()` for display. The cells are
    /// already rotated into view space, so the renderer needs it only for
    /// what depends on the world's axes, such as a light's direction.
    pub facing: crate::stack::Dir,
    /// The frame's lighting seed, derived per frame and never stored; the
    /// engine attaches no meaning to it.
    pub light_seed: u64,
    /// `TraceBand::label` — how loud the party has been in this stack, and
    /// the only form they ever see the meter in. A reading, like `facing`,
    /// not a number the renderer does arithmetic on.
    ///
    /// Not decoration: escalating ambushes with no visible cause are
    /// experienced as bad luck rather than as consequence, and without this
    /// the whole phase is a difficulty curve nobody can see.
    pub trace: &'static str,
    pub position: (i32, i32),
    pub cells: Vec<Vec<StackCellView>>,
    /// What the party is standing on, worded for a prompt — e.g. "A link
    /// leads down". `None` on plain floor.
    pub standing_on: Option<String>,
}

/// One cell of `FrameMapView`.
///
/// `Unknown` is a cell the party has never had in view. It is deliberately
/// indistinguishable from solid rock the party *has* seen only in that both
/// are unwalkable — the renderer draws them differently, because "I have not
/// been here" and "there is nothing here" are the two things a mapper most
/// needs to tell apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameMapCell {
    Unknown,
    Rock,
    Floor,
    LinkUp,
    LinkDown,
    /// A cache the party has seen and not yet emptied. An emptied one maps
    /// as `Floor`, so the map answers "where is there still something" and
    /// not merely "where was there once".
    Cache,
    /// The stack's lair, still held. A cleared one maps as `Floor`.
    Lair,
    /// A doorway, or a seal already burned open.
    Door,
    /// A door that still wants an access shard — worth marking, since it is
    /// the one thing on a map that tells you where to come back to.
    SealedDoor,
    /// A debug port the party has seen and not yet used. A spent one maps as
    /// `Floor`, like an emptied cache.
    Breakpoint,
    /// A hole down to the next frame.
    Fault,
    /// Rotten substrate. The one cell kind the map draws as a warning rather
    /// than as a destination, and the reason mapping a frame is worth more
    /// than knowing which cells are walkable.
    Corruption,
    /// A market with something still on the shelf. A bought-out one maps as
    /// `Floor`, like an emptied cache.
    Market,
    /// A program the party has seen and not yet adopted. An adopted one maps
    /// as `Floor`, like an emptied cache.
    Orphan,
}

/// A landmark pinned to a mapped cell, over and above what the layout says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameMapMark {
    /// Where the party is standing, and which way they are looking.
    Party,
    /// A corridor the party was jumped in.
    Fight,
}

/// The party's own map of the frame they are standing in — everything they
/// have had in view, and nothing they haven't.
///
/// A whole-frame grid rather than a window, because the point of a map is
/// seeing the shape of the parts you have walked all at once. The corner
/// inset can be zoomed into a window around the party, but it crops this on
/// its way to the screen rather than asking the engine for less.
#[derive(Clone)]
pub struct FrameMapView {
    pub depth: u32,
    pub frames: u32,
    pub width: i32,
    pub height: i32,
    /// Row-major, `height` rows of `width` cells.
    pub cells: Vec<Vec<FrameMapCell>>,
    /// Landmarks by cell, in a stable order.
    pub marks: Vec<((i32, i32), FrameMapMark)>,
    /// `Dir::label` for the party's heading — the map is drawn north-up, so
    /// unlike `StackView` this is a real bearing rather than a readout.
    pub facing: &'static str,
    /// The surface tile of the link this stack hangs from, so the map can
    /// say which of a sector's links it belongs to.
    pub entrance: (i32, i32),
    /// How much of the frame's walkable area has been seen, 0.0 to 1.0.
    ///
    /// Counted from what the party has actually walked even when `revealed`
    /// is drawing the rest, so the one figure worth having while hunting an
    /// unwalked wing keeps telling the truth.
    pub explored: f32,
    /// Whether `FERAL_DEV_REVEAL` drew this, rather than the party's own
    /// memory. Carried so a screen can say so — a map that silently knows
    /// more than the party is a debugging session that outlives its
    /// session.
    pub revealed: bool,
}

/// One cell of `WorldMapView`: one 32x32 chunk.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorldMapCell {
    /// Not revealed. The map says nothing about it, marks included.
    Unknown,
    /// Revealed, with the biome sampled at the chunk's centre tile.
    Explored(Biome),
}

/// What a `WorldMapMark` is, with the facts the detail panel needs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorldMapMarkKind {
    Home,
    Town {
        standing: crate::settlements::Standing,
        kind: crate::settlements::SettlementKind,
        /// Mainframes only.
        vitality: Option<crate::settlements::Vitality>,
        /// Mainframes only: which way commerce is heading.
        trend: Option<crate::settlements::growth::TownTrend>,
        /// Servers only: how near growth is, in words.
        outlook: Option<crate::settlements::growth::GrowthOutlook>,
        sends_raiders: bool,
        fields_patrols: bool,
        preys_on_routes: bool,
        refuses_service: bool,
        allows_standing_route: bool,
    },
    Outpost {
        trend: Trend,
        dark: bool,
    },
    StackLink,
    Nest,
}

/// A place on the world map. `target` is `None` for a nest, which is
/// map-only.
#[derive(Clone, PartialEq, Debug)]
pub struct WorldMapMark {
    pub chunk: (i32, i32),
    pub kind: WorldMapMarkKind,
    pub label: String,
    pub target: Option<crate::settlements::CompassTarget>,
}

/// One dispatched route, drawn as a single straight segment between chunks.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WorldMapRoute {
    pub from_chunk: (i32, i32),
    pub to_chunk: (i32, i32),
    /// Towns that prey on this route, from `Game::route_predators`.
    pub preyed_by: Vec<crate::settlements::SettlementKey>,
}

/// The surface at chunk scale: what the party has revealed, and the places
/// in it. `Game::world_map` builds it; it writes nothing.
#[derive(Clone, Debug)]
pub struct WorldMapView {
    /// The chunk at the grid's centre.
    pub center: (i32, i32),
    /// The grid is `2 * radius + 1` cells square.
    pub radius: i32,
    /// `cells[row][col]`, north-up; `cells[0][0]` is chunk
    /// `(center.0 - radius, center.1 - radius)`.
    pub cells: Vec<Vec<WorldMapCell>>,
    /// Revealed marks inside the grid, nearest the party first.
    pub marks: Vec<WorldMapMark>,
    /// Every dispatched route, whether or not it is inside the grid.
    pub routes: Vec<WorldMapRoute>,
    /// The party's own chunk.
    pub party: (i32, i32),
}

/// One cell marked for a floor finish, or for stripping one - see
/// `Game::floor_marks`. A plain dig mark is a `DigMark` and never one of
/// these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FloorMark {
    /// **Base-space** coordinates.
    pub pos: (i32, i32),
    /// The finish to lay, or `None` for a strip.
    pub finish: Option<String>,
}
