use super::*;

#[derive(Component, Default)]
pub struct WanderAi {
    pub cooldown: u32,
}

/// Player-only skill at breaching a program's defences — raises decompile odds
/// independent of the target's HP or species difficulty, and grows on
/// player level-up (see `award_player_xp`). Creatures never attempt a
/// decompile themselves, so this never appears on them.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Decompiler {
    pub skill: i32,
}

#[derive(Component)]
pub struct Tamed {
    pub owner: Entity,
}

/// A standing instruction on a **structure**: keep this one worked, or
/// keep it guarded, whether or not any work order asks for it.
///
/// Standing jobs sit at the lowest priority in
/// `game::base::work_orders::schedule_base_labour`'s want list, so they are
/// filled only by a body no order needs and yield that body the moment one
/// does. They are what keeps a Research Node running at all — a banked
/// payout reaches no output buffer, so no order can ever be placed against
/// research — and the only way a guard post survives the sweep that makes
/// it worth having.
///
/// On the structure entity rather than in a resource keyed by tile, which
/// is the deliberate opposite of `resources::BuybackLedger`. A shelf
/// outlives its building on purpose; a job order must not — a Shield
/// rebuilt on the footprint of a demolished one should not inherit a
/// standing guard nobody asked for.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StandingJob {
    pub work: bool,
    pub guard: bool,
}

/// What a Depot refuses to take in — see `Game::depot_accepts`.
///
/// The **denied** set, so an absent or empty component is a Depot that
/// accepts everything: that is what every Depot standing before this
/// shipped had, and what a newly built one has. Storing the allowed set
/// instead would make a fresh Depot refuse the whole catalogue until it
/// was configured.
///
/// On the structure entity for `StandingJob`'s reason rather than in a
/// resource keyed by tile: a filter is an instruction to *this* building,
/// and a Depot rebuilt on a demolished one's footprint must not inherit a
/// rule nobody set on it.
///
/// It says what may come **in**. A denied item already on the shelf stays
/// there until somebody takes it out — nothing ejects stock — and the take
/// side of every door reads the shelf without asking this at all.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct DepotFilter {
    pub denied: std::collections::BTreeSet<ItemId>,
}

/// Which `duties::Duty` columns a base-staff program is barred from — the
/// Base staff work table's whole state. `DepotFilter`'s precedent again: the
/// **unchecked** set, so an absent component (every program before this
/// shipped, and every newly tamed one) reads as every column checked rather
/// than none. `duties::duty_admits` is the one reader.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct Duties {
    pub off: std::collections::BTreeSet<crate::duties::Duty>,
}

/// A base-staff program's position in the scheduler's pick order — the
/// table order the Base staff screen draws and `schedule_base_labour` fills
/// wants against. Minted by every roster door (`roster_parts`) at
/// `max + 1`; `fuse_companions` overrides the fresh rank with the dominant
/// parent's instead, since fusion keeps that parent's place in line.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StaffRank(pub u32);

/// A program the player has said may be called into a tactical fight by a
/// drop pod — `Game::drop_troopers` is the one reader of who is eligible,
/// `Game::set_drop_trooper` the one writer. **Opt-in, so not a `Duty`**:
/// `Duties` is a denied set whose absence means every column is checked,
/// and as a fifth `Duty` every program — and every save written before
/// this — would be a trooper by default. It coexists with any duty,
/// because a trooper keeps working while it waits.
#[derive(Component, Clone, Copy, Debug)]
pub struct DropTrooper;

/// The abilities installed on this entity, in menu order — the player's and
/// every companion's entire kit. Length is bounded by
/// `Game::routine_slots`; position is what `BattleAction::Special::ability`
/// indexes.
///
/// A companion's species kit is *pre-installed* here rather than read from
/// `SpeciesDef` at menu time, which is what lets an innate ability be popped
/// out and plugged into a different program.
#[derive(Component, Default, Clone)]
pub struct Routines(pub Vec<AbilityId>);

/// How many breedings deep a program's lineage runs. Absent reads as 0:
/// wild, tamed, bought and fused programs are all generation 0, and only a
/// hatched child carries one.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Generation(pub u32);

/// The tick a parent is next fit to breed. Absent reads as ready.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BreedReadyAt(pub u64);

/// A Breeding Bay's incubating children, one `Option` per slot — see
/// `StructureDef::incubation_slots`. A child is a record here, not an entity,
/// until `Game::hatch_incubations` seats it on the roster.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Incubator {
    pub slots: Vec<Option<crate::breeding::Incubation>>,
}

impl Incubator {
    pub fn with_slots(count: u32) -> Self {
        Incubator {
            slots: vec![None; count as usize],
        }
    }

    pub fn is_occupied(&self) -> bool {
        self.slots.iter().any(Option::is_some)
    }
}

/// A Quarantine Rack's shelf of downed programs — see `StructureDef::racks`
/// and `Game::rack_slots`.
///
/// A `Vec`, `DownedPrograms`' own reason: two equal-comparing programs are
/// still two separate kills. Unbounded by the type; the ceiling is
/// `slots * tier`, derived per read rather than carried here, so an upgrade
/// takes effect the tick it lands.
#[derive(Component, Default, Clone, Debug)]
pub struct Racked(pub Vec<DownedProgram>);

/// Serde is here for `MemorySubject::Activity`, which saves this enum
/// directly rather than through a mirror — see that enum's doc comment for
/// why it does not follow `save::CronjobKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind {
    GatherResource,
    /// Posted to defend a structure against raids (see `Game::raid_check`)
    /// without also working it — see `Game::assign_guard`. Unlike
    /// `GatherResource`, `task_progress_system` ignores this kind entirely;
    /// a guard doesn't produce anything even if its target happens to have
    /// a `ResourceNode`.
    Guard,
    /// Posted to a marked `DigSite` in base space: cut the wall, then floor
    /// what the cut opened — see `Game::run_dig_crew`.
    ///
    /// **The one task kind whose `target` is not a `Structure`**, which is
    /// why neither `task_progress_system` nor `haul_step_system` touches it:
    /// each resolves its target through a query a dig site cannot answer —
    /// `haul_step_system` requires a `Structure`, `task_progress_system` a
    /// `ResourceNode` and a `Stock`. Its whole cycle is `&mut Game` work
    /// instead, for the
    /// reason `schedule_base_labour` is — it cuts through `Game::strike_rock`
    /// and floors through `Game::floor_cell` rather than keeping a second
    /// copy of either.
    Excavate,
    /// Posted to a `BuildSite` in base space: fetch what the structure
    /// costs, set it down on the cell, then raise it — see
    /// `Game::run_build_crew`.
    ///
    /// **The second task kind whose `target` is not a `Structure`**, and it
    /// inherits every consequence `Excavate` documents above: neither
    /// `task_progress_system` nor `haul_step_system` can touch it, because
    /// each resolves its target through a query a build site cannot answer.
    /// Its cycle is `&mut Game` work for the same reason a digger's is —
    /// completing one ends in `Game::spawn_structure`, the one place a
    /// structure's component list is written.
    ///
    /// Unlike every other kind, a body holding this one may be **carrying**
    /// a load it fetched for the site. `schedule_base_labour`'s
    /// never-free-a-`Carrying`-holder rule keeps the scheduler from freeing
    /// it mid-trip, because those units have already left the shelf they
    /// came off and the builder is the one walking them to the site. Paths
    /// that free the body anyway (displacement, sale) set the load on the
    /// floor (`Game::drop_load`) for a hauler to bring home.
    Construct,
    /// Posted to a `FarmTile`: dimple a fallow one or bring in a ripe one's
    /// harvest — see `Game::run_farm_crew`.
    ///
    /// **The third kind whose `target` is not a `Structure`**, with every
    /// consequence `Excavate` documents. The job rides in the variant so the
    /// crew step can tell the work a body was posted for from the tile's
    /// state now, which is how a recipe change mid-labour drops it.
    Tend(FarmJob),
}

/// What a body posted to a farm tile is there to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FarmJob {
    /// Sow a fallow tile.
    Dimple,
    /// Bring in a ripe tile's crop.
    Harvest,
}

impl TaskKind {
    pub const ALL: [TaskKind; 6] = [
        TaskKind::GatherResource,
        TaskKind::Guard,
        TaskKind::Excavate,
        TaskKind::Construct,
        TaskKind::Tend(FarmJob::Dimple),
        TaskKind::Tend(FarmJob::Harvest),
    ];

    /// How a memory or a conversation names this activity as a topic.
    pub fn phrase(self) -> &'static str {
        match self {
            TaskKind::GatherResource => "working a machine",
            TaskKind::Guard => "standing guard",
            TaskKind::Excavate => "cutting rock",
            TaskKind::Construct => "raising a structure",
            TaskKind::Tend(FarmJob::Dimple) => "dimpling phase space",
            TaskKind::Tend(FarmJob::Harvest) => "bringing in a harvest",
        }
    }
}

/// A generic ongoing job: `worker` progresses `target` over multiple ticks.
/// This is deliberately generic so base-building work and any future
/// colonist-style job assignment share one mechanism.
#[derive(Component)]
pub struct Task {
    pub kind: TaskKind,
    pub target: Entity,
    pub progress: u32,
    pub required: u32,
}

/// A program that is a Repair Bay's business rather than the base's: off the
/// line until it is whole again.
///
/// A marker and nothing else, `OffShift`'s shape and lifetime: what a
/// downed program *does* is derived from it every beat rather than stored
/// beside it. It is staff by derivation (`party::role_of` — it is neither in
/// the party nor wielded), the scheduler's posting half skips it, and
/// `drift_idle_staff` walks it to a Repair Bay.
///
/// **Two things insert it, and they mean the same thing by it.**
/// `Game::bench_or_dissolve` on the Forgiving arm of a death, and
/// `Game::admit_the_badly_hurt` for a staff program that has fallen below
/// `tuning::BAY_ADMISSION_HP_FRACTION`. The second is why this is no longer
/// described as a benched *corpse*: a Bay that served only the killed served
/// nobody at all under Permadeath, where the bench does not exist, and left
/// a raid's surviving defender with no route back to full.
///
/// **`Game::run_repair_bays` is the only thing that clears it**, at full
/// HP. Nothing else may: a program that cannot reach a Bay stays downed, and
/// dropping the marker on an unreachable Bay would silently heal it. That
/// one-way door is also why admission is refused outright while no Bay
/// stands — the right price for a program that died, and quite the wrong one
/// for one that is merely hurt.
#[derive(Component, Clone, Debug)]
pub struct Downed;

/// A tamed program pinned in a Research Station's pen — `Game::pin_subject`'s
/// one door, and the only fact this feature stores.
///
/// **Arrival is derived, never stored.** The subject is standing in its pen
/// exactly when its own `Position` equals `Game::study_pen(station)`; a
/// second `arrived: bool` here would be a field disagreeing with the
/// coordinates that already answer the question.
///
/// `station` is the structure `Entity`, not a tile: `Game::study_pen` reads
/// the structure's own `Position` and def fresh on every call, so the pen
/// moves with the building rather than needing to be re-derived here if a
/// Station is ever picked up and placed again in a mod.
#[derive(Component, Clone, Copy, Debug)]
pub struct UnderStudy {
    pub station: Entity,
}

/// A tamed program revived as a prisoner in a Holding Cell's pen —
/// `Game::jail_program`'s one door.
///
/// **Behaves exactly like `UnderStudy` at every site unless a comment says
/// otherwise**: no `Task`, no sortie, no post, never moved, and it occupies
/// its pen as ground (`party::walks_the_base`). The body is spawned on the
/// pen cell, so unlike a subject nothing ever has to walk it there and
/// `drift_idle_staff` never reads it.
///
/// `cell` is the structure `Entity`, `UnderStudy::station`'s shape; a cell's
/// occupant is derived with a query (`Game::cell_prisoner`), never stored on
/// the cell. `attempts` is decompile rolls already failed and `progress`
/// the warden beats since the last roll.
///
/// `record` is the `DownedProgram` the body was booted from, carried whole
/// so a released prisoner hands back the kill it came from (level and
/// condition included) and a breakdown is priced from that level rather than
/// the level-1 body. `None` only for a prisoner loaded from a save that
/// predates the field; the readers then rebuild one from the body.
#[derive(Component, Clone, Debug)]
pub struct Jailed {
    pub cell: Entity,
    pub attempts: u32,
    pub progress: u32,
    pub record: Option<crate::items::DownedProgram>,
}

/// A tamed program held in a Power Siphon — the grid's supply from that
/// siphon is counted off this marker, so a siphon with no holder supplies
/// nothing.
///
/// `siphon` is the structure `Entity`, the same shape as
/// `UnderStudy::station`: the program points at the machine holding it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Siphoned {
    pub siphon: Entity,
}

/// A tamed program posted at an outpost — `Game::post_to_outpost`'s one
/// door, and the only fact this feature stores about who is standing there.
///
/// The tile, not an `Entity`: `outposts::Outpost` is a record in
/// `resources::Outposts` keyed by tile rather than a spawned structure, so
/// there is no station entity to name the way `UnderStudy::station` names
/// one. `Game::outpost_crew(tile)` is the one door back from a tile to the
/// programs posted there.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostedAt(pub (i32, i32));

/// A base program walking clear of a tantrum, and where to.
///
/// **Purely visual, and it must never touch anything but the body's
/// position**: a posted worker keeps its `Task` and its progress, and
/// every per-tick walker (`haul_step_system`, the dig, build and farm
/// crews, `drift_idle_staff`) skips a body carrying this, otherwise it
/// would be pulled straight back to its post the next tick.
///
/// Not saved, `Brawls`' reason: the fight it answers is not saved either,
/// and a reload costs a bystander nothing but the walk.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fleeing {
    pub goal: (i32, i32),
    /// The tick the marker clears on. `None` while a brawl is still open
    /// beside the body; set once it closes.
    pub until: Option<u64>,
}
