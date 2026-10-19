use super::*;

/// What spending one program on one build does to the machine it raises —
/// the engine's answer, drawn by gui.
///
/// **Never a percentage.** `systems::work_ticks_at_speed` rounds to whole
/// ticks and floors at one, so on a short cycle a real percentage quotes a
/// change that does not happen; the two figures below are what the machine
/// actually runs at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildEffect {
    /// The machine's shipped rate, and its rate as this program would build
    /// it. Both out of `systems::work_ticks_at_speed`.
    Cycle { shipped: u32, built: u32 },
    /// This structure runs no cycle, so build quality cannot reach it.
    NoCycle,
    /// A configurable machine: it has no recipe yet, so there is no pace to
    /// quote, but build quality scales whichever recipe it is set to.
    ScalesRecipe,
}

/// One row of the build picker: a program, the aptitude this particular
/// build reads, and what spending it would do.
pub struct BuildCandidate {
    pub pet: PetInfo,
    /// "Assembly" or "Extraction" — which roll this build reads.
    pub aptitude: &'static str,
    /// `components::Potential::roll_label` of that roll.
    pub label: &'static str,
    pub effect: BuildEffect,
}

/// One work order on the status screen — see `Game::work_order_report`.
///
/// Every field is derived from live world state at the moment it is asked
/// for, and none of it is stored: `machines` is literally the list
/// `game::base::work_orders::wants` hands the scheduler, so what the player
/// reads is what the scheduler believes *by construction* rather than by a
/// comment claiming the two agree.
#[derive(Clone, Debug)]
pub struct WorkOrderReport {
    pub item: ItemId,
    pub label: String,
    /// How many the **base** holds, across every Depot and machine output
    /// buffer. Not the player's inventory — an order says what the base
    /// should hold, and what you are carrying is yours.
    pub have: u32,
    pub target: u32,
    /// What the base is doing about it — see `OrderState`.
    pub state: OrderState,
    /// The sentence naming why, when `Stalled` — the same one
    /// `queue_work_order` would have refused the order with, so the screen
    /// and the refusal cannot word the same break differently.
    pub blocked_by: Option<String>,
    /// The chain, deepest first, and empty when the order is asking for
    /// nobody at all. **Empty is not a state**: a base with nobody in it
    /// reports a full chain and `Queued`, a dormant order reports an empty
    /// one, and a stalled order reports an empty one too — which is exactly
    /// why the state is a field rather than something the screen infers
    /// from this list.
    pub machines: Vec<WorkOrderMachine>,
}

/// What the base is doing about one work order.
///
/// **Four disjoint states derived from `settle_orders`' own three
/// questions**, in its order: has the base got what was asked for, does the
/// walk find anything to do, and — the one thing `settle_orders` cannot
/// answer because it is the outcome rather than the rule — did a body
/// actually end up on the chain.
///
/// That last question is read off the postings rather than re-derived from
/// the scheduler's cut, and the difference is not cosmetic. Two machines in
/// the accumulated want list never get a body: one the base has been built
/// around, which `can_walk_to_post` skips in silence, and one already held
/// by a program the scheduler may not move. A re-derived answer calls both
/// of those `Working` and sends the player off to watch a machine that
/// nobody is ever going to stand at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderState {
    /// Somebody is standing on this order's chain right now.
    Working,
    /// It wants machines and has nobody on any of them — the queue ran out
    /// of bodies before it got here, or it was filed since the last tick.
    /// Not a fault: the line is whole and the base is simply busy.
    Queued,
    /// The base already holds what was asked for. A standing order lives
    /// here — it is the level being held, which is the feature working
    /// rather than a fault — and a one-shot passes through it for the
    /// moment between being satisfied and being popped.
    Dormant,
    /// The walk found nothing to do at all: a machine demolished or swept
    /// to destruction since the order was placed. Skipped rather than
    /// blocking the queue, and stays listed so `blocked_by` can name what
    /// went missing.
    Stalled,
}

/// One machine in a work order's chain — see `WorkOrderReport::machines`.
#[derive(Clone, Debug)]
pub struct WorkOrderMachine {
    pub entity: Entity,
    pub label: String,
    /// Who is posted here, or `None` for a machine waiting on a body.
    pub worker: Option<String>,
    /// What this machine has not got enough of to run a batch, if anything.
    pub short_of: Option<String>,
    /// How far up the recipe tree from the ordered item this sits — 0 is
    /// the machine that makes the ordered thing itself.
    pub depth: u32,
}

/// What a Base staff column toggles. A Drop Trooper is not a `Duty` — it is
/// opt-in where a duty is opt-out, and `duty_admits` never reads it — but it
/// is drawn and edited as a column like one, so the table keeps one list of
/// columns and every reader that acts on a column matches on this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkColumnKey {
    Duty(Duty),
    /// `components::DropTrooper`, the column drawn only once a drop pod
    /// terminal stands.
    DropTrooper,
}

/// One checkbox column of the Base staff work table — one of `Duty::ALL`,
/// generated rather than a fixed list so a later structure-kind duty needs
/// no screen change, then the Drop Trooper column once a terminal stands.
/// See `Game::work_table`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkColumn {
    pub key: WorkColumnKey,
    /// The header text — `Duty::label`, decided once there rather than
    /// `Duty::name` (the save's lowercase form) upper-cased at the call
    /// site.
    pub label: &'static str,
    /// The wants this duty admits that the matching found no body for —
    /// `resources::LabourDemand::unworked`'s per-column read, drawn as
    /// `N!` on the header when non-zero and nothing when it is.
    pub unworked: usize,
}

/// Which side of the party/staff split a `WorkRow` is on. Both halves are
/// editable — an away program's restriction takes effect once it comes
/// home, `Duties` narrowing what the scheduler may hand it whether or not
/// it is standing on the base right now — the split only decides where the
/// row is drawn and that the away half is dimmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkSection {
    Staff,
    Away,
}

/// One row of the Base staff work table — a program you own, what it is
/// doing, and which of `WorkTable::columns` it may currently be handed.
#[derive(Clone)]
pub struct WorkRow {
    pub program: EntityView,
    pub section: WorkSection,
    /// `Game::program_role`'s own answer — `Some(ProgramRole::Staff)` is
    /// exactly `section == WorkSection::Staff`; carried as the role rather
    /// than restated as a bool so a reader that wants to know *which* away
    /// role (sortie, wielded, under study...) doesn't need a second call.
    pub role: Option<ProgramRole>,
    /// What it is doing right now — `Game::staff_activity` on the Staff
    /// half, `Game::program_activity` on the Away half, the same split
    /// `BaseStaffRow` used to make.
    pub doing: String,
    /// Aligned with `WorkTable::columns` — for a duty column, whether this
    /// program may currently be handed it (`!Duties::off.contains(duty)`);
    /// for the Drop Trooper column, whether it carries the marker.
    pub cells: Vec<bool>,
    /// The table's own order key — `components::StaffRank`, carried so a
    /// test can assert row order without re-deriving it from `program` or
    /// `cells`.
    pub rank: u32,
}

/// The Base staff screen's one derivation: every column's unworked count,
/// every row in table order (Staff first, then Away, rank order held within
/// each), and the header's three figures. `Game::work_table` is the one
/// door; app-core's key handling and gui's drawing both read this rather
/// than deriving a row list each.
#[derive(Clone)]
pub struct WorkTable {
    pub columns: Vec<WorkColumn>,
    pub rows: Vec<WorkRow>,
    /// `resources::LabourDemand::staff` — on-shift bodies the scheduler had
    /// to work with.
    pub on_shift: usize,
    /// `resources::LabourDemand::wanted` — every want the scheduler
    /// accumulated, worked or not.
    pub jobs: usize,
    /// The sum of `resources::LabourDemand::unworked`'s counts — a want
    /// with no body, whether short of bodies or short of *eligible* ones.
    pub unworked_total: usize,
}

/// One production line of two or more machines — see `Game::line_reports`.
#[derive(Clone, Debug)]
pub struct LineReport {
    pub key: crate::game::base::lines::LineKey,
    /// In feed order: sources first, the end of the line last.
    pub members: Vec<Entity>,
    /// Display names, parallel to `members`.
    pub names: Vec<String>,
    /// The member the line's one worker is posted on, if anyone is.
    pub active: Option<Entity>,
    /// The active machine's status, or with nobody posted the first
    /// non-`Idle` member status from the end of the line.
    pub status: MachineStatus,
    /// An ingredient a `Starved` member lacks that no member makes and no
    /// Depot holds.
    pub missing: Option<ItemId>,
    /// `missing` as the roster says it: "Compiler needs Bytecode Block".
    pub missing_text: Option<String>,
}

/// One entry in `Game::craft_recipes` — compiling `result` consumes `cost`.
///
/// `cost` is what the player is actually charged, `Perk::LeanCompiler`
/// already taken off, so a screen may quote it verbatim. The authored `.ron`
/// quantities are not reachable from here on purpose: that is what a machine
/// consumes (`systems::assembly_recipe`) and what the Recipes chains show,
/// and a player's perk has no business in either.
pub struct CraftRecipe {
    pub result: ItemId,
    pub cost: Vec<(ItemId, u32)>,
    /// The bench this recipe is compiled at, if it names one — the id of a
    /// `StructureDef`, already known to be standing or the recipe would not
    /// be in the list.
    ///
    /// Carried rather than looked up again because it decides what a copy
    /// compiles at (`Game::player_craft_order`), and the two halves of
    /// `craft_recipes` read it out of two different databases: an item's own
    /// `craftable` def and a research file's `unlocks_recipes`.
    pub requires_structure: Option<String>,
}

/// One ingredient of a `RecipeStep`, and where the player is meant to get it.
pub struct RecipeInput {
    /// Display name, resolved for the same reason `RecipeStep`'s are.
    pub item: String,
    /// How many one batch consumes.
    pub qty: u32,
    /// The extractor that taps this, for an ingredient no recipe makes —
    /// Core Fragment naming the Extractor. `None` when an earlier step of
    /// the same chain already produces it, and for a drop nothing produces at
    /// all. See `Game::recipe_chains`.
    pub source: Option<String>,
}

/// One conversion in a `RecipeChain`: what goes in, what runs it, what comes
/// out. Names are resolved here rather than left as ids, because a step's
/// maker is a *structure* and a renderer holding only item ids could not
/// name it.
pub struct RecipeStep {
    /// Empty for an extractor, which draws from nothing rather than from a
    /// recipe.
    pub inputs: Vec<RecipeInput>,
    /// The structure that runs this step, or `None` for one the player
    /// compiles by hand at no bench.
    pub maker: Option<String>,
    pub output: String,
    /// Units one batch yields — always `Some(1)`, since a recipe is `cost`
    /// and nothing else, and `None` for an extractor, whose payout
    /// `systems::node_payout` scales by upgrade tier and zone depth. Carried
    /// rather than left for a renderer to assume, so the day a recipe gains a
    /// yield the screens follow it.
    pub output_qty: Option<u32>,
}

/// One entry on the Recipes screen — everything that has to be made, in the
/// order it has to be made, to end up with `product`. See
/// `Game::recipe_chains`.
pub struct RecipeChain {
    pub product: String,
    /// The product's own authored description — the screen's answer to why
    /// you would make one, where the steps only say how. Resolved here for
    /// the reason `RecipeStep`'s names are: a renderer holding item ids has
    /// no `ItemDb` to look prose up in. `None` for a product whose file
    /// leaves the field blank; every shipped item carries one.
    pub description: Option<String>,
    /// Deepest dependency first; `product`'s own step is always last.
    pub steps: Vec<RecipeStep>,
}

/// One structure on order and not yet raised — see `components::BuildSite`.
///
/// **The one derivation of what a build request looks like**, and it is
/// deliberately built before there is a screen to draw it. Three readers
/// want the same answer: the examine page, which is how a player checks
/// what a site is still short of; the map, which needs to know a cell is a
/// pending build at all; and the build-order screen this is shaped for.
/// Two of those exist today. Written per-caller, the third would arrive as
/// a fourth opinion about how far along a build is.
///
/// **Every figure here is a call, not a copy** — `BuildSite::outstanding`,
/// `BuildSite::required_ticks`, `Game::structure_name`, `Game::item_name` —
/// so a screen can never report a percentage the crew disagrees with.
///
/// Names rather than ids in `outstanding`: this is a view, and an `ItemId`
/// leaking onto a screen reads as a renderer bug.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildOrderRow {
    pub entity: Entity,
    /// **Base-space** coordinates.
    pub pos: (i32, i32),
    /// Whether this raises a new structure or advances the one already on
    /// the cell a tier — see `components::BuildGoal`. Read through
    /// `BuildOrderRow::label`, which is what a screen prints.
    pub goal: crate::components::BuildGoal,
    /// The display name of the structure the request is about. On an upgrade
    /// that is the machine already standing there.
    pub structure: String,
    /// Units of material standing on the cell, against `materials`, the
    /// total the bill calls for. The delivery half of the job's progress.
    pub delivered: u32,
    pub materials: u32,
    /// What is still to be fetched, as `(item name, quantity)` in the
    /// structure's own recipe order. Empty once the materials are all in.
    pub outstanding: Vec<(String, u32)>,
    /// Ticks of construction done against `required_ticks`. Both zero-based
    /// and both meaningless until `outstanding` is empty — nothing is raised
    /// while anything is still being carried.
    pub ticks: u32,
    pub required_ticks: u32,
    /// The program posted to this site, if one is. `None` is a real state
    /// and not a fault: a base with no spare body leaves requests standing.
    pub builder: Option<String>,
    /// A rebuild site still waiting for the player to commit a program
    /// (`Game::commit_rebuild_program`). Nothing is fetched or raised until
    /// they do, so the crew-facing fields above are not the story.
    pub awaiting_program: bool,
    /// Why the crew has said this request cannot go on, if it has: the
    /// scheduler's `BuildSite` latches, read here and never kept apart.
    pub blocked: Option<BuildBlock>,
}

/// Why a build request is standing still, as the crew announced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildBlock {
    /// Nothing anywhere to fetch the materials from.
    Dry,
    /// No route from the crew to the site.
    Unreachable,
}

impl BuildBlock {
    /// What a screen prints for it.
    pub fn label(self) -> &'static str {
        match self {
            BuildBlock::Dry => "out of materials",
            BuildBlock::Unreachable => "cut off",
        }
    }
}

impl BuildOrderRow {
    /// What a screen calls this request — `Factory`, or `Factory → Mk3`.
    ///
    /// Composed here rather than baked into `structure`, so the name and the
    /// tier cannot be split back apart by a caller that wants only one.
    pub fn label(&self) -> String {
        match self.goal {
            crate::components::BuildGoal::New => self.structure.clone(),
            crate::components::BuildGoal::Upgrade { to_tier } => {
                format!("{} → Mk{to_tier}", self.structure)
            }
            crate::components::BuildGoal::Recharge => {
                format!("{} — recharging", self.structure)
            }
        }
    }

    /// How far along the whole job is, 0..=100 — deliveries and
    /// construction weighted by how much of the work each is.
    ///
    /// **Derived here rather than by each screen**, so the map's frame, an
    /// examine line and a future order list cannot round the same job three
    /// ways. A structure costing nothing at all reports 100 for the delivery
    /// half rather than dividing by zero.
    pub fn percent(&self) -> u32 {
        let fetched = if self.materials == 0 {
            1.0
        } else {
            self.delivered as f32 / self.materials as f32
        };
        let raised = if self.required_ticks == 0 {
            1.0
        } else {
            (self.ticks as f32 / self.required_ticks as f32).min(1.0)
        };
        // Halves, because the two legs are the two halves of the job and
        // nothing measured says otherwise yet. A weighting worth having is
        // one taken off a play session, not invented here.
        (((fetched + raised) / 2.0) * 100.0).round() as u32
    }
}

/// One item the transfer screen offers a row for — see
/// `Game::transfer_offer_at`.
///
/// The union of the two old offers: `on_shelves` is what the adjacent
/// `Stock` buffers are holding of it, `carried` what the pack holds. An item
/// on both sides is **one** row carrying both figures, which is the whole
/// reason the two screens became one.
///
/// **`carried` is a holding and `can_put` is a permission**, and they part
/// company in three cases: no Depot beside the party, a banked item, and a
/// quantity larger than the room left in the Depots whose
/// `components::DepotFilter` will take it — all three of which may still be
/// taken. The screen draws `carried` — a
/// column headed `you` that reads 0 while the pack holds twelve is a lie the
/// player cannot check — and `App::put_available` clamps against `can_put`.
/// `on_shelves` needs no second figure because it is both at once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferRow {
    pub item: ItemId,
    pub on_shelves: u32,
    pub carried: u32,
    pub can_put: u32,
}

/// One downed program the transfer picker offers, on whichever side of the
/// basket it is currently standing — see `Game::rack_offer_at`.
///
/// **Not a widened `TransferRow`.** That row's three figures are quantities
/// of a fungible item, and a carrier has none: its range is `[-1, +1]` and
/// its two columns are 1/0 or 0/1. `racked` is which side it is sitting on,
/// and the screen derives both figures from it.
///
/// No handle on the program itself: the **index** into `rack_offer_at`'s list
/// is what the basket names, `load_teardown_rig`'s own idiom, so an
/// `Entity` never crosses into a view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferCarrier {
    /// `Game::downed_program_label`'s sentence — "the level 9 Scrapper".
    pub label: String,
    /// True when it stands on a rack, so the only move it has is a take.
    pub racked: bool,
}

/// One Depot's filter as its screen draws it — see
/// `Game::depot_filter_view`.
///
/// `tile` is base space, and it is the header: with up to four Depots
/// orthogonally adjacent the player has to be told which one they are
/// editing. `room` is that Depot's own `Stock::output_room`, not the
/// transfer picker's shared budget, for the same reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepotFilterView {
    pub tile: (i32, i32),
    pub room: u32,
    pub rows: Vec<DepotFilterRow>,
}

/// One row's standing at one Depot — a single item, or a whole family
/// (`items` longer than one) that is denied and allowed together.
///
/// `held` is what that Depot is holding of it right now, summed over a
/// family, drawn because a
/// filter says only what may come **in**: denying something already on the
/// shelf leaves it there, and a row reading "denied" beside a count of
/// twelve is the screen telling the truth rather than contradicting
/// itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DepotFilterRow {
    pub items: Vec<ItemId>,
    pub name: String,
    pub held: u32,
    pub allowed: bool,
}

/// What the recipe menu draws for one configurable machine.
///
/// `tile` is base space and the header, `RigToolView`'s reason: a base may
/// stand several machines and the player has to be told which one they are
/// setting. `held` is the units a change would empty (input plus output), so
/// the screen can warn before the player makes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineRecipeView {
    pub tile: (i32, i32),
    pub name: String,
    pub current: Option<ItemId>,
    pub held: u32,
    pub rows: Vec<MachineRecipeRow>,
}

/// One researched recipe on the menu. `inputs` is empty for an extraction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineRecipeRow {
    pub item: ItemId,
    pub name: String,
    pub inputs: Vec<(String, u32)>,
    pub pace: RecipePace,
    pub current: bool,
}

/// How long a recipe takes, as a word. Banded on `ticks_per_unit` by the
/// `tuning::RECIPE_PACE_*` constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipePace {
    Quick,
    Steady,
    Slow,
    VerySlow,
}

impl RecipePace {
    pub fn of(ticks_per_unit: u32) -> Self {
        use crate::tuning::{RECIPE_PACE_QUICK_MAX, RECIPE_PACE_SLOW_MAX, RECIPE_PACE_STEADY_MAX};
        match ticks_per_unit {
            t if t <= RECIPE_PACE_QUICK_MAX => Self::Quick,
            t if t <= RECIPE_PACE_STEADY_MAX => Self::Steady,
            t if t <= RECIPE_PACE_SLOW_MAX => Self::Slow,
            _ => Self::VerySlow,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Quick => "quick",
            Self::Steady => "steady",
            Self::Slow => "slow",
            Self::VerySlow => "very slow",
        }
    }
}

/// The finish painted over one cell of laid floor — `Game::view_finishes_at`,
/// `Tile`'s own shape widened for a layer gui draws over it rather than
/// folded into `Tile` itself, since `Tile` rides the save through
/// `SaveData::tile_overrides` and a finish does not.
///
/// Carries the shade and the sprite key rather than a `FloorId`, `Tile`'s own
/// argument for `rock_shade` over a rock kind: gui never reads `FloorDb`, so
/// what it draws with has to already be the resolved answer.
#[derive(Clone, Debug, PartialEq)]
pub struct FinishView {
    pub shade: crate::floors::FloorShade,
    pub sprite: String,
    pub name: String,
}
