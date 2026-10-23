use super::*;

/// What a shelf holds against what it can hold — a Depot's own
/// `Stock::output` total against `Stock::capacity`, or a Quarantine Rack's
/// `Racked` programs against `Game::rack_slots` — carried as counts rather
/// than a fraction so "full" is an integer comparison and never a float
/// landing at 0.9999.
///
/// Depots and Racks only: a machine's output buffer filling is already
/// `MachineStatus::Clogged` on the outline, and a bar beside it would say
/// the same thing twice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShelfFill {
    pub held: u32,
    pub capacity: u32,
}

/// One structure on the roster screen — see `Game::structure_report`.
#[derive(Clone)]
pub struct StructureReport {
    /// What a configurable machine is set to make; `None` when unset and for
    /// every structure that is not a configurable machine.
    pub recipe: Option<ItemId>,
    pub entity: Entity,
    /// The def id, so a frontend can group identical structures without
    /// comparing display names.
    pub kind: StructureId,
    pub label: String,
    pub pos: (i32, i32),
    /// Tiles from wherever the player is standing, as a Chebyshev distance —
    /// the metric the map's own movement uses. While the party is
    /// underground this is measured from the entrance tile, because that is
    /// where `Position` stays; `pos` is absolute either way.
    pub distance: i32,
    /// Upgrade tier, or `None` if the def declares no upgrade path — same
    /// meaning as `EntityView::tier`.
    pub tier: Option<u32>,
    /// Current/max raid `Durability`, or `None` for a structure raids can't
    /// target (see `StructureDef::raidable`).
    pub durability: Option<(u32, u32)>,
    pub is_home: bool,
    /// Whether a program can be posted here — an extractor or an assembler.
    /// A workable structure with no assignees is idle and producing nothing,
    /// which is the one thing on this screen the player can act on.
    pub workable: bool,
    /// Whether the player is standing on one of the four tiles this structure
    /// can be worked from — `hauling::at_station`, the same question
    /// `Game::work_structure` refuses on.
    ///
    /// Here rather than derived by a frontend from `distance`, because the
    /// two disagree: `distance` is Chebyshev, so a diagonal neighbour is one
    /// tile away and still not a tile you can work from. A screen offering
    /// "work it yourself" filters on this, and per `CLAUDE.md` the rule is a
    /// call rather than a second copy of the adjacency list.
    pub player_adjacent: bool,
    /// The room this structure stands in, as "Fine dormitory"; `None` in the
    /// commons and in space with no role.
    pub room: Option<String>,
    /// What is staged in this structure's input buffer and waiting in its
    /// output buffer, as display-ready `(item name, count)` pairs.
    ///
    /// Names, not `ItemId`s, and folded here rather than in the renderer:
    /// per `CLAUDE.md` a read-only screen's rows are shaped in the engine,
    /// and resolving an id against the `ItemDb` is exactly the kind of lookup
    /// the renderer has no business doing.
    pub input: Vec<(String, u32)>,
    pub output: Vec<(String, u32)>,
    /// How much `output` holds in total before the machine clogs.
    pub output_capacity: u32,
    /// Why this machine is or isn't producing, or `None` for a structure
    /// that runs no job at all and so has no state to be in.
    pub status: Option<MachineStatus>,
    /// Every program assigned to this structure. A cronjob worker and a
    /// guard can both be on one structure at once, which is why this is a
    /// list and why `EntityView::structure_worker` could not answer it.
    pub assignees: Vec<Assignee>,
    /// A Teardown Rig's standing tool by display **name**, `None` on a rig
    /// nobody has hand-loaded yet and on every structure that is not one.
    ///
    /// A name and not a `ToolId`, `input`/`output`'s reason: resolving an id
    /// against a catalogue is the kind of lookup a renderer has no business
    /// doing. Drawn because a rig with no standing tool is the one that
    /// silently will not fetch from a rack.
    pub standing_tool: Option<String>,
    /// What this structure is fed with to keep working — a defence's or
    /// Repair Bay's `StructureDef::supply`, a burner's `power_upkeep` fuel
    /// — as `(item name, how many its buffer holds at most)`. `None` on a
    /// structure that spends nothing.
    ///
    /// Named even while the buffer holds some, because `input` goes quiet
    /// the moment it empties and `MachineStatus::Uncharged` only says that
    /// it has: a Shield reading "uncharged" told the player nothing about
    /// what the haulers were waiting on. A name, `input`'s reason; the
    /// ceiling is `hauling::input_capacity`, the figure the crew fills to.
    pub supply: Option<(String, u32)>,
    /// A drop pod terminal's charge, `None` on every structure that is not
    /// one (`StructureDef::drop_pod`).
    pub pod: Option<PodState>,
    /// A Holding Cell's occupant and clock, `None` on every structure that is
    /// not one (`StructureDef::holds_prisoner`).
    pub prison: Option<PrisonState>,
    /// The production line this structure belongs to, `Some` only for a line
    /// of two or more.
    pub line: Option<crate::game::base::lines::LineKey>,
}

/// One Holding Cell as the roster draws it — `Game::prison_state`.
#[derive(Clone, Debug, PartialEq)]
pub struct PrisonState {
    /// Whether a worker holds the warden's post. Without one nothing runs.
    pub warded: bool,
    /// Who is inside, or `None` for an empty cell.
    pub prisoner: Option<PrisonerState>,
}

/// The prisoner in a cell and everything its clock says. Every figure is a
/// call into the engine's own rule: `odds_percent` is `Game::jail_odds`, the
/// figure the next roll is made against.
#[derive(Clone, Debug, PartialEq)]
pub struct PrisonerState {
    pub name: String,
    pub attempts: u32,
    pub max_attempts: u32,
    pub progress: u32,
    pub attempt_ticks: u32,
    pub odds_percent: u32,
}

/// Whether a drop pod terminal can fire. There is no "spent with nothing
/// filed" state: `Game::spend_pod` files the recharge as it spends, and
/// `cancel_build_request` refuses to call one off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PodState {
    Charged,
    /// `percent` is the recharge request's `BuildOrderRow::percent`.
    Recharging {
        percent: u32,
    },
}

impl StructureReport {
    /// A workable structure with nobody posted to it — the one thing on
    /// either structure screen the player can immediately act on, which is
    /// why both colour it yellow and why `Game::attention` counts it.
    ///
    /// Here rather than spelled out at each reader: the renderer's copy of
    /// this lived in `render/building.rs` and the attention model is in the
    /// engine, so the two are in different crates and nothing would fail to
    /// compile when one drifted.
    pub fn is_idle(&self) -> bool {
        self.workable && self.assignees.is_empty()
    }

    /// Whether a production line has nobody on it: one worker runs the whole
    /// line, so a staffed three-machine line has two machines with no
    /// assignee that are not idle, and only a line with no body on *any*
    /// member is.
    pub fn line_is_idle<'a>(members: impl IntoIterator<Item = &'a StructureReport>) -> bool {
        let mut any_workable = false;
        for member in members {
            if !member.assignees.is_empty() {
                return false;
            }
            any_workable |= member.workable;
        }
        any_workable
    }

    /// How many things on the base want a program: every idle structure of
    /// its own, and every idle *line* once however many machines it has.
    pub fn idle_count(structures: &[StructureReport]) -> usize {
        let alone = structures
            .iter()
            .filter(|s| s.line.is_none() && s.is_idle())
            .count();
        let keys: std::collections::BTreeSet<_> =
            structures.iter().filter_map(|s| s.line).collect();
        let lines = keys
            .into_iter()
            .filter(|&key| Self::line_is_idle(structures.iter().filter(|s| s.line == Some(key))))
            .count();
        alone + lines
    }
}

/// Which condition an [`AttentionRow`] reports.
///
/// Carried so a frontend can sort a row into a pane without matching on its
/// prose or its keycap. The match on this is exhaustive, `cell_mark`'s rule:
/// a `_ =>` arm is how a new condition ships with no marker anywhere.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AttentionKind {
    StructureDamaged,
    IdleStructures,
    /// A Research Node is standing and no project is selected, so every cycle
    /// it runs lands nowhere — see `resources::ActiveResearch`.
    NoResearchProject,
    /// A project has all the progress it needs and is waiting on materials the
    /// base is not holding. Its own row rather than folding into
    /// `NoResearchProject`, because the two ask for opposite things: one wants a
    /// decision, this one wants production — and a bill's work order is
    /// *removed* when it completes, so a line consumed before the project
    /// settled leaves nothing outstanding for the player to find.
    ResearchStalled,
    PerkPoints,
    /// Attribute points banked by levelling and not yet spent.
    StatPoints,
    /// A held program has levelled and banked points for the player to spend
    /// from the Manifest.
    ProgramPoints,
    /// The roster has outgrown `Game::pet_capacity`, and the programs past
    /// it are earning `unslotted`. Past, not at: a roster filling its slots
    /// exactly has nothing wrong with it, since slots are not a door.
    Unslotted,
    /// A held `Objective::Deliver` whose counter the player is standing at,
    /// carrying some of what it asked for.
    ///
    /// The hint on the contracts screen only reaches a player who already
    /// opened it; this is the half that reaches one who has not. It is the
    /// only attention row that depends on **where the player is standing**,
    /// which is also why it is not a threat: nothing is going wrong, there is
    /// simply a keypress available that is not available anywhere else.
    ContractDeliverable,
    /// The raid clock has passed `tuning::RAID_PRESSURE_WARN_PERCENT` of its
    /// drawn interval and a GC Entropy Sweep is coming.
    ///
    /// The one attention row that is about something that has **not**
    /// happened yet, which is the whole of what the raid clock bought: every
    /// other row here reports a state the base is already in. It is a threat
    /// for the same reason — nothing on this row can be fixed by opening the
    /// screen it points at, only prepared for.
    RaidIncoming,
    /// `RaidIncoming`'s twin on `resources::SiegePressure`'s clock — see
    /// `Game::siege_warned`.
    SiegeIncoming,
    /// An outpost is `Trend::Stale` or `Trend::Declining` — design
    /// correction 10. Not a threat: unlike a raid warning, this is a state
    /// the player can walk out and fix.
    OutpostTrend,
    /// An outpost's integrity has reached zero and it needs `[R]` repair.
    OutpostDark,
}

/// One thing that needs the player right now — see `Game::attention`.
#[derive(Clone, Debug, PartialEq)]
pub struct AttentionRow {
    pub kind: AttentionKind,
    /// Player-facing, built in the engine and never in a renderer.
    pub text: String,
    /// The map key that opens the screen this is acted on from.
    pub key: char,
    /// Drawn in br red rather than br yellow: hostility or inbound harm,
    /// never an ordinary error.
    pub threat: bool,
}

/// What a manifest subject is asking of the player, derived by
/// `Game::call_to_act` -- the same derivation `Game::attention` counts its
/// point rows from, so the rail's `!` and the HUD badge cannot disagree.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CallToAct {
    /// The player has unspent perk points.
    PerkPoints(u32),
    /// The player has banked stat points, spent with `S` on the perks screen.
    StatPoints(u32),
    /// A program is holding banked attribute points for the player to spend.
    DevPoints(u32),
    /// A staff program with no post and no errand.
    Idle,
}

/// One pile the base is holding, as the stock block lists it — see
/// `Game::base_stock`.
///
/// Carries the tag *and* the name: the stock block over the map draws the
/// name, and the base pane's PRODUCTION rows draw both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StockRow {
    pub item: ItemId,
    pub tag: String,
    pub name: String,
    pub qty: u32,
}

/// Why each base report page deserves a look, or `None` when it does not.
/// One field per page that can be flagged; the rail's `!` and the Overview
/// read this and nothing else. A page with no gap to report (Output) holds
/// a field anyway so a later phase fills it without reshaping the rail.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BaseReportFlags {
    pub production: Option<String>,
    pub staff: Option<String>,
    pub stock: Option<String>,
    pub work_orders: Option<String>,
    pub staffing: Option<String>,
    pub build_orders: Option<String>,
    pub alerts: Option<String>,
}

/// One structure that is not running, or is hurt, on the Machines screen —
/// see `Game::machine_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineRow {
    pub entity: Entity,
    pub label: String,
    /// **Base-space** coordinates.
    pub pos: (i32, i32),
    /// `Running` for a machine listed only because it is damaged.
    pub status: crate::components::MachineStatus,
    /// Current/max `Durability` when below full, else `None`.
    pub damage: Option<(u32, u32)>,
}

/// One structure's share of the grid - see `PowerReport`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerLine {
    pub label: String,
    /// **Base-space** coordinates.
    pub pos: (i32, i32),
    /// What it puts on the grid now: 0 for a dry or unsiphoned supplier.
    pub supply: u32,
    /// What it takes, whether or not anyone is posted to it.
    pub draw: u32,
    /// A machine that lost the cut and is not running for want of grid.
    pub dark: bool,
}

/// The base grid on the Power screen - see `Game::power_report`. `draw` and
/// `supply` are `Game::base_power`'s figures, and each is the sum of the
/// lines beneath it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerReport {
    pub draw: u32,
    pub supply: u32,
    /// Largest contributor first.
    pub lines: Vec<PowerLine>,
}

/// One item on the Stock screen: what the base holds against the target of
/// the work order that asks for it, if one does - see `Game::stock_report`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StockReportRow {
    pub item: ItemId,
    pub tag: String,
    pub name: String,
    pub qty: u32,
    /// The order's target, and where that order stands.
    pub order: Option<(u32, OrderState)>,
}

impl StockReportRow {
    /// Whether a work order wants more than the base holds. The sort, the
    /// red figure and the Stock flag all ask this.
    pub fn is_short(&self) -> bool {
        self.order.is_some_and(|(target, _)| self.qty < target)
    }

    /// Short, and the order that should be filling the gap is not moving.
    pub fn is_stalled_short(&self) -> bool {
        self.is_short() && matches!(self.order, Some((_, OrderState::Stalled)))
    }
}

/// What a staff program is doing right now, as the Wellbeing page words it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StaffActivity {
    /// May be handed a job: `Game::on_shift`.
    OnShift,
    /// Off shift and on its way to, or being served at, an amenity for the
    /// need whose `servicing` verb this holds.
    Servicing(String),
    /// Off shift and standing in reach of the amenity, which has no place
    /// free: still draining.
    Waiting,
    /// Benched and recovering in a Repair Bay.
    Downed,
    /// Off shift for no need: a respite, or tools downed.
    Break,
}

/// One need on a Wellbeing row, ready to draw: the reserve as a fraction of
/// a full one, and whether it is under the def's `critical`.
#[derive(Clone, Debug, PartialEq)]
pub struct WellbeingNeed {
    pub name: String,
    pub fraction: f32,
    pub below_critical: bool,
}

/// One Staff-role program on the Wellbeing page - see `Game::staff_wellbeing`.
#[derive(Clone, Debug, PartialEq)]
pub struct WellbeingRow {
    pub entity: Entity,
    pub name: String,
    /// The morale sum, `Game::morale`.
    pub morale: f32,
    /// `morale_band` of it.
    pub mood: &'static str,
    pub hp: i32,
    pub max_hp: i32,
    pub needs: Vec<WellbeingNeed>,
    pub activity: StaffActivity,
    /// The grievance rung's wire name, `Game::grievance`.
    pub grievance: Option<&'static str>,
}

impl WellbeingRow {
    /// Whether any need is below its own `critical`. The flag and the page
    /// both ask this.
    pub fn is_critical(&self) -> bool {
        self.needs.iter().any(|n| n.below_critical)
    }
}

/// An amenity with fewer open standing tiles than the capacity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoxedInAmenity {
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub open_tiles: usize,
}

/// One need's line on the Coverage page - see `Game::need_coverage`.
/// **Approximate**: the share ignores walking time.
#[derive(Clone, Debug, PartialEq)]
pub struct CoverageRow {
    pub name: String,
    pub built: usize,
    /// Effective places summed over the built amenities.
    pub places: usize,
    /// Places the whole capacity would give: `built` times the capacity.
    pub capacity: usize,
    pub needed: usize,
    pub below_critical: usize,
    pub below_content: usize,
    /// The share of staff time expected to be spent off shift for this
    /// need, 0..=1.
    pub lost_share: f32,
    pub boxed_in: Vec<BoxedInAmenity>,
}

impl CoverageRow {
    /// Short of amenities: none, or fewer than the staff need.
    pub fn is_short(&self) -> bool {
        self.built == 0 || self.built < self.needed
    }
}

/// One memory on the Discontent page.
#[derive(Clone, Debug, PartialEq)]
pub struct GrudgeRow {
    pub name: String,
    pub subject: Option<String>,
    pub intensity: f32,
}

/// One disgruntled program - see `Game::discontent`.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscontentRow {
    pub entity: Entity,
    pub name: String,
    pub rung: &'static str,
    /// At `DownedTools` or worse: the page's gap flag.
    pub stopped_work: bool,
    pub mood: &'static str,
    /// The three most negative memories, worst first.
    pub grudges: Vec<GrudgeRow>,
}

/// One item's line on the base output page — see `Game::base_output_report`.
///
/// Every figure is carried as a number rather than pre-rendered text: none
/// of them needs the world, and formatting them here would invent a second
/// formatting seam for the one screen that reads this. `name` is the
/// exception, because resolving it needs `ItemDb`, which never reaches the
/// page.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseOutputRow {
    pub item: ItemId,
    pub name: String,
    /// Landed in the sector the party is standing in, summed over the
    /// buckets stamped with it.
    pub sector: u32,
    /// Landed over the whole run, from the lifetime totals that never roll
    /// off — which is what lets a fresh save show something.
    pub run: u32,
    /// The machine's share of `run`, and the player's own. Split because a
    /// combined figure hides what hand-compiling is contributing, which is
    /// the question the whole instrument was built to answer.
    pub machine: u32,
    pub hand: u32,
    /// The most recent buckets' production, oldest first, for a sparkline.
    /// Empty for a run with no history yet.
    pub spark: Vec<u32>,
}

/// What the base has made, as `Mode::BaseOutput` draws it.
///
/// **Flows and totals only.** The player has infinite cargo and never
/// interacts with a Depot, so a quantity sitting in a buffer is invisible
/// plumbing to them — no depot fill, no machine buffer, no "units in
/// store".
#[derive(Clone, Debug)]
pub struct BaseOutputReport {
    /// The sector the `sector` columns are counting.
    pub zone: u32,
    /// Items some structure's `work` produces.
    pub mined: Vec<BaseOutputRow>,
    /// Everything else that was made: an assembler's output, and anything
    /// the player compiled by hand.
    pub compiled: Vec<BaseOutputRow>,
    /// **Called, never recomputed.** `Game::attention` is one derivation
    /// with three surfaces already reading it; a fourth that worked out its
    /// own answer is the drift that seam exists to prevent.
    pub attention: Vec<AttentionRow>,
}

/// One row of the alert board as gui draws it — `Game::alerts`.
///
/// No `subject`: the collapse key is engine bookkeeping (`alerts::Alert`'s
/// own field), and gui never reads it — it only draws `text` and, when
/// `count > 1`, a trailing `×N`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertView {
    pub kind: AlertKind,
    pub text: String,
    pub count: u32,
    pub unread: bool,
}
