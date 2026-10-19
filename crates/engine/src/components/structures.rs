use super::*;

#[derive(Component)]
pub struct Structure {
    pub kind: StructureId,
}

/// A structure's current upgrade tier, starting at 1. Present only on
/// structures whose definition sets `StructureDef::upgrade`.
#[derive(Component, Clone, Copy, Debug)]
pub struct StructureTier(pub u32);

/// A structure that can be worked for `resource`. Carries no deposit pool:
/// a node is not a reserve that gets mined down, it is a tap. What paces it
/// is `Stock::capacity` — it produces until its output buffer is full and
/// then clogs until someone collects.
#[derive(Component)]
pub struct ResourceNode {
    pub resource: ItemId,
    /// Mirrors `RecipeDef::level`. `None` means a completed gather cycle
    /// always yields, same as before this field existed. `Some(level)` gates
    /// each completion behind a level-based percentage chance instead (see
    /// `systems::task_progress_system`) — a harder, chancier variant that a
    /// structure opts into via its `.ron` file rather than something every
    /// worked node does by default.
    pub level: Option<u32>,
}

/// Which recipe a configurable machine (`StructureDef::machine`) is set to,
/// by produced item. `None` is unconfigured: it stands but runs nothing.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct MachineRecipe(pub Option<ItemId>);

/// A structure's local buffers — the whole of a production chain's
/// directionality.
///
/// Neighbours (and the player, collecting) may take from `output`. Nothing
/// outside a machine ever touches its `input`. That asymmetry is why a chain
/// flows one way without belts existing: a machine can only pull from what
/// its upstream has already finished, never reach into what upstream is
/// still working on.
///
/// `BTreeMap`, not `HashMap`, in both directions. Iteration order feeds the
/// pull phase, so a `HashMap` would make two machines competing for one
/// scarce feeder resolve differently between runs — and would make the save
/// encoding differ run to run as well.
///
/// `capacity` bounds `output` *in total*, not per item: it is how much the
/// box holds, so a machine that produces two things cannot dodge clogging by
/// splitting its output across them. `input` has no field here — it is
/// derived per ingredient from the recipe, at `INPUT_STOCK_BATCHES` batches,
/// so a greedy machine cannot drain a shared feeder dry.
#[derive(Component, Clone, Debug, Default)]
pub struct Stock {
    pub input: std::collections::BTreeMap<ItemId, u32>,
    pub output: std::collections::BTreeMap<ItemId, u32>,
    pub capacity: u32,
}

impl Stock {
    pub fn new(capacity: u32) -> Self {
        Self {
            capacity,
            ..Default::default()
        }
    }

    /// How many units are in `output`, across every item in it.
    pub fn output_used(&self) -> u32 {
        self.output.values().sum()
    }

    /// Room left in `output` before it clogs.
    pub fn output_room(&self) -> u32 {
        self.capacity.saturating_sub(self.output_used())
    }

    /// Whether `output` has room to take a payout of up to `worst`. The
    /// payout is capped at `capacity`: a worst case larger than the whole
    /// output would otherwise never fit and clog the machine for good, so an
    /// empty output always takes it.
    pub fn fits_payout(&self, worst: u32) -> bool {
        self.output_room() >= worst.min(self.capacity)
    }
}

/// Why a machine is or isn't producing. Present on a structure that runs a
/// job (`StructureDef::work` or `::machine`) and on one that burns to keep
/// supplying (`StructureDef::power_upkeep`) — absence means "nothing here
/// can stall", which is why a Home never reports a status it could not
/// possibly leave.
///
/// Deliberately not saved **as a component**. It initialises to `Running`
/// and is corrected on the first tick, so a base that loads starved
/// announces it once — which is information the player wants, and costs no
/// save field. `Serialize`/`Deserialize` are derived only so
/// `alerts::AlertKind::MachineStalled` can carry one inside a saved `Alert`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MachineStatus {
    #[default]
    Running,
    /// Input short: upstream is too slow, or isn't adjacent.
    Starved,
    /// Output full: downstream is too slow, or nobody has collected.
    Clogged,
    /// A program is posted but is not standing at the machine — walking to
    /// its post, carrying a load to a depot, or unable to reach the machine
    /// at all. Distinct from `Idle`, which means nobody is posted.
    ///
    /// Wins over both `Clogged` and `Running` whenever the worker is off its
    /// tile: after shedding a load the machine is no longer full, and
    /// without the precedence the pane would claim it is running while
    /// nothing produces.
    Unstaffed,
    /// A program is posted and cannot get here at all — no route to the
    /// machine or to any depot, usually because the base has been built
    /// around it. Strictly more specific than `Unstaffed`, which it wins
    /// over: both mean nobody is at the machine, but this one will not
    /// resolve itself by waiting, so it is the difference between a worker
    /// walking and a worker that needs the player to clear a path.
    ///
    /// Driven by the `Stranded` marker `haul_step_system` maintains.
    Stranded,
    /// No program assigned.
    Idle,
    /// The base's grid can't cover this machine's `power_draw` — see
    /// `game::base::power::ledger` for which machines lose the cut.
    ///
    /// **Top precedence over all five variants above.** Nothing else the
    /// player can do makes a dark machine run: posting a program, clearing a
    /// clog, feeding an input, building a depot and clearing a route are all
    /// wasted moves while it is dark, so this is the reading that has to be
    /// shown. It is the only status whose fix is "supply more grid".
    ///
    /// A sixth variant is allowed here where `views::output_stranded`
    /// refused one, and the difference is worth keeping straight: a base-wide
    /// shortfall is *not* what this says. Under the ledger's cut order one
    /// machine runs while its neighbour two tiles over is dark, so which
    /// machine lost the cut really is that machine's own state — which is the
    /// test that enum is held to.
    ///
    /// Written by `systems::idle_machine_system` and by nothing else;
    /// `task_progress_system`, `assembler_system` and `player_gather_system`
    /// only guard on the same fact via `resources::PowerGrid`, and write no
    /// status of their own.
    Unpowered,
    /// A supplier that burns `StructureDef::power_upkeep` ran its charge to
    /// zero and could not buy another cell — no orthogonally adjacent output
    /// buffer had one. It supplies nothing to the grid until fuel appears
    /// beside it.
    ///
    /// **Split out of `Starved`, which it was for one release.** The two
    /// really are one shape — "the thing it consumes is not there" — and the
    /// reuse was argued for on exactly that. What it missed is that the
    /// *sentence* differs: `Starved` sends the player to look for an upstream
    /// machine and a program to feed it, and a Recharger Node has neither. A
    /// dry supplier is fixed by standing a stocked buffer next to it, which
    /// is not advice `Starved` can give. Sharing the concept was right;
    /// sharing the status made three readouts say the wrong thing.
    ///
    /// **Below `Unpowered`, above the rest.** No structure today both draws
    /// and burns — a Recharger Node has no `power_draw` — so the two cannot
    /// collide yet, and the rule is written here rather than discovered by
    /// whichever structure first does both. `Unpowered` keeps the top for its
    /// own reason: while a machine is dark, nothing else the player fixes
    /// makes it run, and that includes feeding it.
    ///
    /// Written by `systems::burn_grid_upkeep` and by nothing else, which is
    /// already the single writer of a burner's stall.
    Dry,
    /// A defence (`StructureDef::raid_defense` or `::turret`) that spends a
    /// `StructureDef::supply` holds none, so it adds nothing to a sweep or a
    /// siege until a hauler stocks it.
    ///
    /// **Not `Dry`, which means a burner out of Power Cells.** `Dry`'s fix is
    /// "stand a stocked buffer beside it"; this one's is "haul it coils", and
    /// the two sentences must not be shared.
    ///
    /// Written by `systems::idle_machine_system`, the one pass that already
    /// visits every structure, for the defences that carry a status.
    Uncharged,
    /// A configurable machine (`StructureDef::machine`) with no recipe set,
    /// so there is nothing for a program to do at it. Wins over `Idle` and
    /// every worker-dependent status, but not over `Unpowered`.
    ///
    /// Written by `systems::idle_machine_system`, before its worker check.
    Unconfigured,
}

impl MachineStatus {
    /// Every status, for `GlyphColor::ALL`'s reason: the renderer's colour
    /// table is exhaustive, but the census holding the palette's reserved
    /// colours off machine states is not, unless it walks this.
    pub const ALL: [MachineStatus; 10] = [
        MachineStatus::Running,
        MachineStatus::Starved,
        MachineStatus::Clogged,
        MachineStatus::Unstaffed,
        MachineStatus::Stranded,
        MachineStatus::Idle,
        MachineStatus::Unpowered,
        MachineStatus::Dry,
        MachineStatus::Uncharged,
        MachineStatus::Unconfigured,
    ];

    /// Why a machine in this state is not producing, as a screen words it.
    /// `Running` has nothing to say.
    pub fn reason(self) -> &'static str {
        match self {
            MachineStatus::Running => "",
            MachineStatus::Starved => "nothing is feeding it",
            MachineStatus::Clogged => "its output buffer is full",
            MachineStatus::Unstaffed => "its program is away",
            MachineStatus::Stranded => "its program can't reach it",
            MachineStatus::Idle => "no program is assigned",
            MachineStatus::Unpowered => "the grid is short",
            MachineStatus::Dry => "no Power Cells beside it",
            MachineStatus::Uncharged => "its supply has run out",
            MachineStatus::Unconfigured => "nothing is set to make",
        }
    }

    /// The name a `telemetry::Record::MachineStall` carries.
    ///
    /// Its own match rather than `{:?}`, `cell_mark`'s rule turned at the
    /// wire: the derived form is a debug aid nobody promised to keep, and
    /// this string is what an analysis script written months after a run
    /// greps for.
    pub fn as_str(self) -> &'static str {
        match self {
            MachineStatus::Running => "running",
            MachineStatus::Starved => "starved",
            MachineStatus::Clogged => "clogged",
            MachineStatus::Unstaffed => "unstaffed",
            MachineStatus::Stranded => "stranded",
            MachineStatus::Idle => "idle",
            MachineStatus::Unpowered => "unpowered",
            MachineStatus::Dry => "dry",
            MachineStatus::Uncharged => "uncharged",
            MachineStatus::Unconfigured => "unconfigured",
        }
    }
}

/// Integrity left on the Patch Routine a Repair Bay is working through: a
/// supplied heal tick draws a patch when this is zero and refills it to
/// `tuning::REST_INTEGRITY_PER_PATCH`, then pays it down by what it restores.
///
/// Not saved, `MachineStatus`' rule: a reload forfeits at most the part-used
/// patch, and a save field for it would buy nothing a player could see.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PatchCredit(pub u32);

/// How many ticks of charge a `StructureDef::power_upkeep` supplier has left
/// before it must buy another Power Cell — see `systems::power_grid_system`,
/// which is the only writer.
///
/// Present only on a supplier that declares the upkeep, and **absence reads
/// as dry**: `game::base::power::ledger` counts a burner's `power_supply`
/// only through this component, so a supplier standing without one supplies
/// nothing. That is deliberately the strict direction — both writers of a
/// structure's component list have to insert it, and a fixture that
/// hand-spawns a bare `Structure` should read as a base that never wired the
/// thing up rather than as one running on free power.
///
/// Saved, unlike `MachineStatus`: this one is not recomputed from the world
/// on the next tick, it *is* the state.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerFuel {
    pub ticks_left: u32,
}

/// Whether a `StructureDef::drop_pod` terminal has a charge ready to spend —
/// see `structures::DropPodDef`.
///
/// Present exactly on a structure whose def declares `drop_pod`,
/// `PowerFuel`'s rule one field family up: `Game::spawn_structure` is the one
/// writer, and a hand-spawned fixture without one reads as a terminal that
/// was never wired up rather than as a free pod. A freshly raised terminal
/// is charged; spending it is `Game::spend_pod`, which also files the
/// `BuildGoal::Recharge` request that flips this back.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropPod {
    pub charged: bool,
}

/// How well the program spent on this machine's build put it together —
/// `game::base::building::build_quality`'s answer, baked in the tick the
/// structure was raised and never moving again except on an upgrade, which
/// overwrites it with the new program's.
///
/// **Absent means 1.0.** Three things rely on that and none of them writes
/// one: the Home, which costs no program at all; every hand-spawned test
/// fixture; and every structure in a pre-feature save, whose
/// `StructureSave::build_quality` defaults neutral. So no reader may treat
/// the component's absence as an error.
#[derive(Component, Clone, Copy, Debug)]
pub struct BuildQuality(pub f32);

/// A structure's remaining health against raids (see `Game::raid_check`).
/// Every deployed structure gets one, sized from its
/// `StructureDef::durability`; reaching 0 destroys the structure.
#[derive(Component, Clone, Copy, Debug)]
pub struct Durability {
    pub hp: u32,
    pub max_hp: u32,
}

/// A cell of base-space rock the player has started on: the one
/// representation of rock-in-progress, carrying `Durability` on the same
/// entity the way a nest does.
///
/// **Spawned lazily** — by the first swing at a wall, or by marking one —
/// which is why `base_grid::BaseCell` gains no `Rock` variant: absent from
/// `BaseGrid` still means solid and untouched, and a parallel cell variant
/// would have to be kept in step with this entity. It despawns when the cell
/// opens, unless it is marked: a mark outlives the cut, because marked solid
/// means *cut it* and marked `Open` means *floor it*, one verb running a wall
/// all the way to finished floor.
///
/// **Its `Position` is in base space, not on the zone surface.** That widens
/// the space rule this repo otherwise holds — a `Structure` is the space tag,
/// with posted programs the one prior exception — so anything reading a
/// `Position` off one of these must already know which locale it is in. See
/// `seam:a-digsite-is-the-second-non-structure-entity-standing-in`; 0.13.0
/// shipped two fixes for exactly this bug class, and a wrong-space read is
/// silent.
#[derive(Component, Clone, Debug, Default)]
pub struct DigSite {
    /// Whether the player marked this cell for a crew. Set by the mark verb
    /// (slice 2, phase B) and saved, because a plan you drew has to survive
    /// a reload.
    pub marked: bool,
    /// Whether the crew has already said it cannot reach this one. Written
    /// by the dig scheduler (phase C) and read nowhere else: the
    /// announcement follows `set_machine_status` and fires only on
    /// transition, because entering a state is news and staying in it is not.
    pub announced_stuck: bool,
    /// Whether the crew has already said it is short for this one — Core
    /// Fragments to floor the cell, or Blank Substrate to finish it, the two
    /// wordings of `DigDryReason`. `announced_stuck`'s rule and `announced_stuck`'s reason —
    /// entering a state is news, staying in it is not — but a second field
    /// rather than a shared one, because the two leave the player different
    /// errands: no route is a wall to cut, no stock is a shelf to fill.
    ///
    /// Neither is saved: `save::DigSiteSave` writes the mark and the meter,
    /// so a reload says both again, which is right — the run that was told
    /// is over.
    pub announced_dry: bool,
    /// What kind of mark this is, on top of cut-or-tile: `None` is today's
    /// plain mark, `Some` is a finish or a strip over already-laid floor.
    /// `Option<FinishOrder>` rather than a fourth `DigSite` variant of its
    /// own — this is orthogonal to `marked`, not a replacement for it, so a
    /// site can be unmarked and still carry the finish it was last asked
    /// for while it holds no chip progress to keep it alive.
    pub finish: Option<FinishOrder>,
}

/// One farm plot: the recipe every one of its `FarmTile` cells grows.
///
/// A plot entity apart from its cells because "one box is one plot, one
/// recipe" must be a single write, and a crew's interaction needs one `Copy`
/// handle for the plot. It carries no `Position` — the cells do.
#[derive(Component, Clone, Debug)]
pub struct FarmPlot {
    /// `None` until the player picks a crop.
    pub recipe: Option<crate::crops::CropId>,
}

/// One cell of a plot, in **base space** like a `DigSite`: a task targets one
/// `Entity`, so the crew's target is the cell. The renderer draws it from
/// `Game::farm_tiles`, so it carries no `Glyph`.
#[derive(Component, Clone, Copy, Debug)]
pub struct FarmTile {
    pub plot: Entity,
    pub growth: CropGrowth,
}

/// Where one farm cell is in its crop's life.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CropGrowth {
    #[default]
    Fallow,
    Growing {
        elapsed: u32,
    },
    Ripe,
}

/// What a finish mark asks the crew to do — `Game::toggle_mark_box`'s brush,
/// carried on the `DigSite` it marks.
///
/// A `FloorId` rather than an index into `FloorDb`, `DigSite` losing `Copy`
/// with it: `FloorDb::load_dir` re-sorts on every load, and a mod adding or
/// removing a finish would silently repoint an index a save had already
/// written to a different finish entirely.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinishOrder {
    Apply(FloorId),
    Strip,
}

/// The character a pending build site draws.
///
/// **A caret, and orange.** The renderer paints its own dark frame around
/// the cell and bounces this on top, but the glyph is what the examine ray
/// reads and what any surface that draws a `Glyph` without knowing what it
/// is will show — so it has to mean something on its own rather than be a
/// placeholder the renderer is expected to override.
pub const BUILD_SITE_GLYPH: char = '^';

/// A structure the player has requested and the base has not stood up yet:
/// the resolved bill of materials, what has been carried to the spot so
/// far, and how far along the raising of it is.
///
/// **Its `Position` is in base space**, on the cell the finished structure
/// will occupy — `DigSite`'s rule and `DigSite`'s warning. It is the third
/// non-`Structure` entity to carry a base-space `Position`, so anything
/// reading one off it must already know which locale it is in.
///
/// **Everything a future build-order screen needs is here or derived from
/// here**, which is why the cost is carried rather than looked up: the
/// screen wants a percentage and a shortfall, and both fall out of `cost`
/// against `delivered` with no second walk of the world. It is also why the
/// cost is *resolved at filing* rather than re-read from the
/// `StructureDef` each tick — `ActiveContract`'s rule. A zone-portal's
/// build cost grows with `ZoneLevel`, so a request filed in zone 3 and
/// finished in zone 4 would otherwise silently change price underneath the
/// crew already carrying to it.
///
/// **What is not here is who is building it.** That is a `Task` with
/// `TaskKind::Construct` pointing at this entity, exactly as a digger's
/// posting is, so "one builder at a time" is a property of the scheduler
/// naming a site once rather than a count stored here. A second builder is
/// a scheduler change, not a save-format change.
/// What a `BuildSite` does when its meter fills — see `BuildSite::goal`.
///
/// **The axis of change is what a finished site does, and nothing else.**
/// The bill, the fetching, the walk, the delivery, both announcement
/// latches, the reachability check above the truncation and the refund on
/// cancel are identical for a deploy and an upgrade, so exactly one step
/// branches on this: `raise_one_tick`'s completion. A second component with
/// its own crew pass would have to copy all four rules build orders
/// established, and the copy that drifts is the one nobody runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildGoal {
    /// Raise the structure named by `BuildSite::structure` on this cell.
    #[default]
    New,
    /// Advance the structure **already standing on this cell** to `to_tier`.
    ///
    /// The machine is named by tile and never by `Entity`: it is resolved by
    /// position at completion, so there is nothing to dangle when it is
    /// destroyed underneath the request and no load-order dependency between
    /// structures and sites in the save.
    Upgrade { to_tier: u32 },
    /// Recharge the drop pod **already standing on this cell** — see
    /// `components::DropPod` and `structures::DropPodDef`.
    ///
    /// `Upgrade`'s path exactly: named by tile and never by `Entity`, for the
    /// same reason. `BuildSite::program` is always `None` on one of these —
    /// a recharge runs no job, so it costs no program, the Shield/shelf
    /// rule — which is why `Game::spend_pod` files the site directly rather
    /// than through `commit_for_build`.
    Recharge,
}

#[derive(Component, Clone, Debug)]
pub struct BuildSite {
    /// Which structure this becomes. A `StructureId` and never a resolved
    /// `StructureDef`, unlike `cost` above: the def is content and may be
    /// edited or modded between the request and the raising, and the two
    /// things a stale def would get wrong — the price and the glyph — are
    /// respectively carried here already and wanted *fresh*.
    pub structure: StructureId,
    /// The bill of materials as `Game::structure_build_cost` priced it the
    /// moment the request was filed.
    pub cost: Vec<(ItemId, u32)>,
    /// What has actually been carried here and set down, which is a subset
    /// of `cost` by construction — `BuildSite::deliver` clamps against the
    /// outstanding amount, so nothing can be over-delivered and no entry
    /// can name an item the bill does not.
    ///
    /// These units have already left the shelf they came from. They are
    /// **owed back** on a cancel, which is `Game::cancel_build_request`'s
    /// whole job.
    pub delivered: Vec<(ItemId, u32)>,
    /// Ticks of construction done, against `required_ticks`. Only ever
    /// advanced by a builder standing at the site with the materials all
    /// in, so it is not a second progress meter racing the delivery one.
    pub progress: u32,
    /// Whether the crew has already said there is nothing anywhere to fetch
    /// for this one. `DigSite::announced_dry`'s rule and its reason:
    /// entering a state is news, staying in it is not.
    pub announced_dry: bool,
    /// Whether the crew has already said it cannot reach this one.
    /// `DigSite::announced_stuck`'s counterpart, and neither latch is
    /// saved — a reload should say both again, because the run that was
    /// told is over.
    pub announced_stuck: bool,
    /// What raising this one does — stand a new structure up, or advance
    /// the one already on this cell a tier. See `BuildGoal`.
    pub goal: BuildGoal,
    /// The tamed program committed to this order, retired from the roster
    /// the moment the request was filed.
    ///
    /// A value and never an `Entity`: entity ids are not stable across a
    /// save round trip, which is why `HopperEntry` carries a
    /// `DownedProgram` rather than a reference to one. A whole
    /// `CreatureSave` rather than a narrower record because a refund owes
    /// the player back the program they had — gear, name, refactors and all
    /// — and a second description of a creature is one that drifts.
    ///
    /// `None` only for a Home, the one exempt structure, and for the
    /// hand-spawned sites in test fixtures.
    pub program: Option<crate::save::CreatureSave>,
    /// A rebuild of a structure that runs a job, filed after its wreck with
    /// no program committed yet. Not workable and never announced dry, which
    /// is about materials: it waits on the player, via
    /// `Game::commit_rebuild_program`. Saved.
    pub awaiting_program: bool,
    /// What a rebuilt machine is set to when it is raised: carried from the
    /// wreck (`resources::Ruin::recipe`), `None` for every fresh build.
    /// Saved.
    pub recipe: Option<ItemId>,
}

impl BuildSite {
    /// A fresh request for `structure` against an already-resolved `cost`.
    pub fn new(structure: StructureId, cost: Vec<(ItemId, u32)>) -> Self {
        Self::filed(structure, cost, BuildGoal::New)
    }

    /// A request to advance the structure already standing on this cell to
    /// `to_tier`, against an already-resolved `cost`.
    pub fn upgrade(structure: StructureId, cost: Vec<(ItemId, u32)>, to_tier: u32) -> Self {
        Self::filed(structure, cost, BuildGoal::Upgrade { to_tier })
    }

    /// A request to recharge the drop pod already standing on this cell,
    /// against its `structures::DropPodDef::recharge_cost`. `Self::upgrade`'s
    /// shape — `filed` already defaults `program` to `None`, which is what
    /// makes this one cost no program on its own.
    pub fn recharge(structure: StructureId, cost: Vec<(ItemId, u32)>) -> Self {
        Self::filed(structure, cost, BuildGoal::Recharge)
    }

    fn filed(structure: StructureId, cost: Vec<(ItemId, u32)>, goal: BuildGoal) -> Self {
        Self {
            structure,
            cost,
            delivered: Vec::new(),
            progress: 0,
            announced_dry: false,
            announced_stuck: false,
            goal,
            program: None,
            awaiting_program: false,
            recipe: None,
        }
    }

    /// How many units of material the whole structure costs.
    pub fn total_materials(&self) -> u32 {
        self.cost.iter().map(|(_, qty)| qty).sum()
    }

    /// How long raising this one takes, in ticks —
    /// `tuning::BUILD_TICKS_PER_MATERIAL` per unit of material.
    ///
    /// **Derived, never stored**, so retuning the rate moves every site in
    /// every save at once rather than only the ones filed afterwards. See
    /// that constant for the argument.
    ///
    /// Floored at one tick: a modded structure costing nothing at all would
    /// otherwise have a `required` of zero and complete on the tick it was
    /// filed, which reads as the request being ignored rather than as an
    /// instant build.
    pub fn required_ticks(&self) -> u32 {
        (self.total_materials() * crate::tuning::BUILD_TICKS_PER_MATERIAL).max(1)
    }

    /// How much of `item` has been set down here.
    pub fn delivered_of(&self, item: &ItemId) -> u32 {
        self.delivered
            .iter()
            .find(|(id, _)| id == item)
            .map(|(_, qty)| *qty)
            .unwrap_or(0)
    }

    /// What the site still needs, in `cost` order and skipping anything
    /// already satisfied. Empty exactly when the materials are all in.
    ///
    /// In `cost` order rather than sorted, so a crew fetches a structure's
    /// ingredients in the order its `.ron` file lists them and a base
    /// raising the same structure twice does it the same way both times.
    pub fn outstanding(&self) -> Vec<(ItemId, u32)> {
        self.cost
            .iter()
            .filter_map(|(item, want)| {
                let short = want.saturating_sub(self.delivered_of(item));
                (short > 0).then(|| (item.clone(), short))
            })
            .collect()
    }

    /// Whether everything the structure costs is standing on the cell.
    pub fn materials_ready(&self) -> bool {
        self.outstanding().is_empty()
    }

    /// Sets `qty` of `item` down here, clamped to what is still outstanding,
    /// and reports how much actually landed.
    ///
    /// **Clamped rather than refused**, so a builder that fetched a full
    /// carry of five against a shortfall of two sets two down and keeps
    /// three — the caller is told the figure and owns what happens to the
    /// remainder. Refusing the lot would strand the load.
    pub fn deliver(&mut self, item: &ItemId, qty: u32) -> u32 {
        let want = self
            .cost
            .iter()
            .find(|(id, _)| id == item)
            .map(|(_, qty)| *qty)
            .unwrap_or(0);
        let room = want.saturating_sub(self.delivered_of(item));
        let landed = qty.min(room);
        if landed == 0 {
            return 0;
        }
        match self.delivered.iter_mut().find(|(id, _)| id == item) {
            Some((_, held)) => *held += landed,
            None => self.delivered.push((item.clone(), landed)),
        }
        landed
    }
}

/// The char an armed trap wears on the map, and the char a sprung one wears.
///
/// Two named constants beside the struct rather than literals at the spawn
/// and the capture, because there is no other channel left to say which
/// state a trap is in: the tile's four corners, both edges and the
/// background wash are all claimed, so the distinction rides the centre
/// glyph's own char. A capture site and a spawn site that disagreed about
/// which char means which would read as the map lying.
pub const TRAP_GLYPH_ARMED: char = '^';

/// See `TRAP_GLYPH_ARMED`. `'U'` is authored by no shipped structure or
/// species and appears nowhere else in the engine as a glyph.
pub const TRAP_GLYPH_SPRUNG: char = 'U';

/// A device the player left standing on the zone surface, which catches one
/// non-boss program over game-time and holds it until collected. Carries
/// `Position` and `Glyph` alongside this, the way a `Nest` does, and no
/// `Durability`: a trap is walked into, not attacked.
///
/// `Game::run_traps` is the one writer of `next_roll`, `caught` and the
/// entity's `Glyph::ch` past the spawn.
#[derive(Component, Clone)]
pub struct Trap {
    /// Which item def this was placed from. The rarity ceiling is resolved
    /// from the def on every read rather than copied here — `restore_nests`'
    /// precedent and against `ActiveContract`'s: a trap is a device standing
    /// in the world, not an agreement already signed, so a retuned `.ron`
    /// retunes one already on the ground and a deleted one drops it.
    pub item: crate::items::ItemId,
    /// Ticks until the next capture roll. The countdown is what keeps fifty
    /// traps off the seeded RNG stream — see `Game::run_traps`.
    pub next_roll: u32,
    pub caught: Option<crate::items::DownedProgram>,
}

/// A stationary spawner for a wild species. Present on the nest entity
/// itself, which also carries `Position`, `Glyph`, and `Durability` (all
/// reused as-is — a nest is destroyed the same way a structure is, just
/// via a direct bump-attack instead of a raid).
#[derive(Component, Clone, Debug)]
pub struct Nest {
    pub species: SpeciesId,
    /// Ticks remaining until each queued replacement guardian spawns —
    /// one entry per guardian currently missing from the nest's original
    /// count (see `systems`-adjacent `Game::nest_respawn_tick`). Emptied
    /// naturally once every missing guardian is back.
    pub pending_respawns: Vec<u32>,
}

#[cfg(test)]
mod machine_status_tests {
    use super::*;

    /// `MachineStatus::ALL` is hand-written, and a hand-written list of an
    /// enum's variants is the kind that ships one short: the census in
    /// `render/base.rs` that keeps reserved colours off machine states walks
    /// it, so a variant missing here is a variant the census silently stops
    /// covering.
    ///
    /// The closure is the guard. It is exhaustive over the enum, so adding a
    /// variant stops this file compiling until someone comes here — and once
    /// they are here the length assertion below is what tells them `ALL`
    /// needs the same variant. Neither half works alone: the match alone
    /// passes if you add an arm and forget the list, and the length alone
    /// passes if you add a variant and never look.
    #[test]
    fn all_lists_every_machine_status() {
        let arms = |s: MachineStatus| match s {
            MachineStatus::Running => 0,
            MachineStatus::Starved => 1,
            MachineStatus::Clogged => 2,
            MachineStatus::Unstaffed => 3,
            MachineStatus::Stranded => 4,
            MachineStatus::Idle => 5,
            MachineStatus::Unpowered => 6,
            MachineStatus::Dry => 7,
            MachineStatus::Uncharged => 8,
            MachineStatus::Unconfigured => 9,
        };
        assert_eq!(
            MachineStatus::ALL.len(),
            10,
            "the match above has 10 arms; ALL must list all 10"
        );
        let mut seen: Vec<usize> = MachineStatus::ALL.iter().map(|s| arms(*s)).collect();
        seen.sort_unstable();
        assert_eq!(
            seen,
            (0..10).collect::<Vec<_>>(),
            "ALL repeats one status and omits another"
        );
    }

    /// The wire name a `telemetry::Record::MachineStall` carries. An analysis
    /// script greps these, so two statuses sharing one string silently merges
    /// two different stalls in every run ever recorded.
    #[test]
    fn every_status_has_its_own_wire_name() {
        let mut names: Vec<&str> = MachineStatus::ALL.iter().map(|s| s.as_str()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "two statuses share a wire name");
    }
}
