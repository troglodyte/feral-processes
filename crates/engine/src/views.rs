//! Read-only snapshots of engine state, shaped for the renderer.
//!
//! Every one of these is produced by a `Game` method and consumed by
//! app-core/gui. They are plain data with no back-reference into the ECS —
//! that is what keeps the renderer from reaching into the `World`.

use crate::abilities::AffinityKind;
use crate::battle::ActionOption;
use crate::classes::PlayerClass;
use crate::components::{EquippedItem, GlyphColor, MachineStatus, Rarity, TaskKind};
use crate::game::party::ProgramRole;
use crate::icon::PlayerIcon;
use crate::items::{GearCopy, ItemId};
use crate::perks::Perk;
use crate::research::ResearchId;
use crate::resources::DifficultyMode;
use crate::species::{AffinityClass, MoveDef};
use crate::structures::StructureId;
use crate::tools::{ToolCategory, ToolId};
use crate::world::Biome;
use bevy_ecs::prelude::Entity;

/// One node of the research tree as the menus see it — see
/// `Game::research_nodes`.
pub struct ResearchStatus {
    pub id: ResearchId,
    pub name: String,
    pub description: String,
    pub cost: u32,
    pub state: ResearchState,
    /// Whether the player can pay `cost` right now. Independent of `state`:
    /// a node can be `Available` but unaffordable, or affordable but
    /// `Locked`.
    pub affordable: bool,
    /// Whether this node sits on a path the tree recommends — see
    /// `ResearchDb::recommended_ids`. Independent of `state` for the same
    /// reason `affordable` is: a recommended node is still locked until its
    /// prerequisites are paid for, and the menu says both things at once.
    pub recommended: bool,
    /// Abilities this node hands over as routine items when researched.
    /// `cfg(test)`: read only by engine tests today, neither the renderer
    /// nor app-core touches it, and a `pub(crate)` field that's merely
    /// unread (rather than absent from non-test builds) still warns.
    #[cfg(test)]
    pub(crate) unlocks_abilities: Vec<crate::abilities::AbilityId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResearchState {
    Unlocked,
    Available,
    /// Why a node can't be taken, rather than just greying it out. The two
    /// reasons are independent and both may hold at once — a deep node in a
    /// fresh game owes a prerequisite *and* a breach — so they are two
    /// fields rather than two variants: a variant would force an arbitrary
    /// precedence between them and lose whichever it did not pick.
    Locked {
        /// Display names of the prerequisites still missing.
        missing: Vec<String>,
        /// `Some(n)` when the player's zone is below the node's `min_zone`;
        /// `None` when the zone is satisfied and only prereqs are in the way.
        min_zone: Option<u32>,
    },
}

/// One stack of cargo the player is carrying: `qty` of exactly this copy.
/// A plain copy lives in `components::Inventory`, anything fused or rare in
/// `components::GearCopies` — `GearCopy::is_plain` is which.
///
/// A struct rather than the `(ItemId, u32)` pair this replaced, because
/// every consumer now has to say *which copy* it means. Changing the type
/// instead of adding a parallel list is the point: a screen or handler that
/// summed a fused copy together with its plain spares — or an Overclocked
/// weapon together with an ordinary one — would be silently wrong, and the
/// compiler is what stops that.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryRow {
    pub copy: GearCopy,
    pub qty: u32,
}

pub struct PlayerStatus {
    pub position: (i32, i32),
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    ///
    /// Named `strength` rather than `power` because this is the one struct
    /// carrying both it and the reserve below, and the status screen prints
    /// them two lines apart. Two different numbers labelled "Power" is what
    /// the vocabulary pass exists to remove.
    pub strength: i32,
    pub decompiler: i32,
    /// What the player has left to spend on routine calls — see
    /// `components::PowerReserve`.
    pub power: f32,
    /// The player's cargo, and the one list every "what does the player
    /// have" screen reads. Banked items (`ItemDef::banked`) are **not** in
    /// it: a bank is not something carried and not something a trader
    /// deals in, so it is neither an inventory row nor a sell row. Ask
    /// `Game::banked` for one by name — the research screen does.
    ///
    /// One row per `GearCopy`: a fused copy, a rare one and their ordinary
    /// spares are separate rows, because they are separate physical things
    /// — see `components::GearCopies`.
    pub inventory: Vec<InventoryRow>,
    /// Units of ordinary cargo currently carried. The Buffer is unbounded, so
    /// this is just how much is stored. It matches the sum of `inventory`,
    /// since both now exclude banked items.
    pub inventory_used: u32,
    /// How many tamed programs the player owns in total right now, and the
    /// cap on that total (see `Game::pet_count`/`Game::pet_capacity`) — party
    /// members, cronjob workers, and idle pets all count. Distinct from
    /// `companions`, which is only the active battle party.
    pub pet_count: usize,
    pub pet_capacity: usize,
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
    pub weapon: Option<EquippedItem>,
    /// The tamed program equipped as the player's weapon, if any (see
    /// `resources::WieldedProgram`). Always `None` when `weapon` is `Some`
    /// and vice versa — the two are mutually exclusive, so the weapon line
    /// renders one or the other and never both.
    pub wielded: Option<WieldedView>,
    pub armor: Option<EquippedItem>,
    pub module: Option<EquippedItem>,
    /// The player's active battle party (see `resources::Party`), in
    /// party-slot order.
    pub companions: Vec<CompanionInfo>,
    /// Which zone sector the player is currently breached into. See
    /// `ZoneLevel`.
    pub zone: u32,
    /// Unspent Perk Points (see `perks::Perk`), earned 1 per level gained.
    pub perk_points: u32,
    /// Which perks have been unlocked so far.
    pub unlocked_perks: Vec<Perk>,
}

/// Full stats for one tamed program the player owns, wherever it is on the
/// map — shown by the pets/roster screen so you can check on (or manage) a
/// cronjob worker without walking over to it. See `Game::owned_pets`.
/// One row on a trading post's buyback shelf: something the player sold
/// here, offered back at a markup — see `Game::buyback_options`. Renderers
/// draw these verbatim and never compute a price of their own.
#[derive(Clone)]
pub struct BuybackOption {
    /// Exactly which copy is on this row. A shelf keeps what was sold to
    /// it, so buying a T2 back returns a T2 and buying an Overclocked one
    /// back returns an Overclocked one — see `resources::BuybackLedger`.
    pub copy: GearCopy,
    pub name: String,
    /// How many are on the shelf — the shelf is a record of the player's own
    /// sales, so this is a hard cap on what `Game::buy_back` will hand over.
    pub qty: u32,
    pub unit_cost: u32,
}

/// Everything the gear inspect page draws about one carried copy, in one
/// call — see `Game::gear_detail`.
///
/// **One derivation, for `Game::copy_bonus`'s reason.** Four screens once
/// rebuilt the scaling chain by hand and all four dropped the affix at
/// once; this page adds a second axis of the same hazard, since a granted
/// routine's magnitudes are scaled for their invoker and a renderer reading
/// `AbilityEffect::Damage`'s authored `power` would quote the level-1
/// figure forever.
pub struct GearDetailView {
    /// Through `Game::copy_name`, so the page and the row that opened it
    /// cannot disagree about what this is.
    pub name: String,
    /// The item's own authored prose, `None` for one whose file leaves it
    /// blank — the same answer `Game::item_description` gives.
    pub description: Option<String>,
    /// The block that only means something for a wearable copy, `None` for
    /// a consumable or a currency.
    pub worn: Option<WornDetailView>,
    /// What the item does *besides* granting a routine — consuming it,
    /// refactoring a program with it, what it adds to a decompile. The
    /// grant is deliberately absent: it has its own block below, and one
    /// line saying `Grants: X` above the prose describing X is the same
    /// fact twice. See `Game::item_effects_besides_grant`.
    pub effects: Vec<String>,
    /// What this copy's affixes add, one line per **distinct** one with
    /// duplicates folded as `of Static ×3` and trade-offs first — see
    /// `Game::affix_lines`. Formatted in the engine for `copy_name`'s
    /// reason, and because the stats go through the one `stat_summary`.
    pub affixes: Vec<String>,
    pub grant: Option<RoutineDetailView>,
}

/// One gear copy's combat rating, and the four axes it came off.
///
/// **Absolute** — every copy is priced against the same reference wearer
/// (`tuning`'s power reference block), which is what lets one figure mean
/// the same thing on the inventory list, a trader's shelf and the swap
/// picker. `Game::copy_power` is the one derivation.
///
/// `total` is the sum of the other four. It is carried rather than left for
/// a caller to add up, because the two proportional axes are rounded once
/// here and a caller summing rounded parts would print a total that does not
/// match the column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemPower {
    pub total: i32,
    /// Attack, plus what the copy's damage band is worth **against** the
    /// band it replaces — negative for a weapon worse than bare fists.
    pub offense: i32,
    /// What the copy's mitigation buys, in effective HP.
    pub survivability: i32,
    /// What the copy's Accuracy is worth as a fraction of the throughput it
    /// multiplies. Never the raw stat: a probability is not a quantity.
    pub accuracy: i32,
    /// ...and the same for Evasion, against the soak it protects.
    pub evasion: i32,
}

/// What a copy is worth in its slot, and what it does for the wearer's
/// chance of landing a swing.
pub struct WornDetailView {
    pub slot: crate::items::EquipmentSlot,
    /// The level this copy is priced at — the current zone for one in
    /// cargo, since `Game::equip` locks gear in at the zone it goes on at.
    pub level: u32,
    /// How well this copy compiled, as a percentage of the item's authored
    /// bonus — `items::GearCopy::quality`. Carried on the view rather than
    /// left to the renderer to read off the copy, because
    /// `GearDetailView`'s promise is that the page is one call: a renderer
    /// reaching past it for one figure is how each of the four screens that
    /// rebuilt `copy_bonus` by hand started.
    ///
    /// On the *worn* half deliberately: only equipment rolls quality, so a
    /// consumable's page has nothing to state rather than a defaulted 100.
    pub quality: u8,
    pub stats: crate::items::EquipmentStats,
    /// What this copy is worth in combat, absolutely — `Game::copy_power`.
    ///
    /// Carried on the view rather than left to the renderer to call for
    /// itself, for `quality`'s reason: `GearDetailView`'s promise is that
    /// the page is one call, and a renderer reaching past it for one figure
    /// is how each of the four hand-rolled `copy_bonus` chains started.
    ///
    /// `None` where the copy has no combat axis at all — a Decompiler
    /// module. Priced against the reference wearer and **not** at
    /// `level` above, so the figure on this page is the figure on the row
    /// that opened it.
    pub power: Option<ItemPower>,
    /// The wearer's Accuracy **with this copy in its slot** — what the slot
    /// already holds is taken back off first, so inspecting the piece you
    /// are wearing reports the accuracy you actually have.
    pub accuracy: f64,
    /// ...and their chance to land a swing on `nominal`, through
    /// `battle::hit_chance`. A call and never a copy: a displayed figure
    /// that disagrees with what a swing rolls is the hand-rolled-chain bug
    /// in a new place.
    pub hit_chance: f64,
    pub nominal: NominalHostile,
}

/// The opponent `WornDetailView::hit_chance` is measured against.
///
/// **A display heuristic, not a claim** — the honest answer to "what is my
/// chance to hit" needs an opponent, and there isn't one until a fight
/// starts. This is the game's own definition of a middling program
/// (`balance_sim::median_ordinary_species`, the baseline its survivability
/// sweeps already assume) at the current zone level with no gear, which is
/// exactly what an ambient wild spawn fields: `Game::ability_user_level`
/// falls back to `ZoneLevel` for a creature with no `Experience`, and
/// `battle::evasion_of` reads a species' `base_speed`, which no zone
/// multiplier touches.
///
/// Deliberately **not** filtered to the danger band that can actually spawn
/// in this zone. That needs a biome and would fork `Game::habitat_pools`
/// into a second copy of the band rules — the page carries the zone in
/// `zone` and labels itself a projection instead.
pub struct NominalHostile {
    pub zone: u32,
    pub evasion: f64,
}

/// One routine's mechanics, as the inspect page draws them — see
/// `Game::routine_detail`.
///
/// Every field bar `name`/`description` is derived rather than authored: an
/// ability's `.ron` prose is mod-controlled free text and says whatever its
/// author wanted, which is the same argument `Game::item_grant` makes about
/// reading the ability rather than the item.
pub struct RoutineDetailView {
    pub name: String,
    pub description: String,
    /// When it runs — a passive's `PassiveTrigger::phrase`, or the line for
    /// a routine the player picks.
    pub when: String,
    /// What it lands on, through `AbilityTarget::phrase`.
    pub target: String,
    /// What it does, with every magnitude scaled for the invoker through the
    /// same `abilities::scaled_range`/`scaled_hp_power`/`scaled_stat_power`
    /// the invocation itself uses.
    pub effect: String,
    /// Battle rounds before it can run again. 0 means unthrottled.
    pub cooldown: u32,
    /// What running it costs its invoker, through
    /// `abilities::routine_power_cost` — so the page, the refusal and the
    /// charge cannot quote three different numbers.
    pub power_cost: f32,
    /// Whether it resolves through `battle::resolve_attack` and so can
    /// miss. The two damage effects do; nothing else in the vocabulary
    /// rolls to hit, which is what makes `hit_chance` worth printing beside
    /// a weapon at all.
    pub rolls_to_hit: bool,
}

/// How many of the player's routine holders one purchase at a Stack market
/// writes to — see `Game::market_offers`.
///
/// The ladder is the product: a market sells the *writing* of a routine,
/// never the knowledge of it, so what varies between the three rungs is
/// breadth and nothing else. Prices are `tuning::STACK_MARKET_ROUTINE_PRICE_*`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoutineScope {
    /// One holder the player picks — themself, or any program they own.
    One,
    /// The player and everyone currently fielded in `Party`.
    Party,
    /// The player and every program on the roster.
    Everyone,
}

impl RoutineScope {
    /// Listed cheapest first, which is also narrowest first — the order the
    /// shelf is built in and the order a screen reads down.
    pub const ALL: [RoutineScope; 3] = [
        RoutineScope::One,
        RoutineScope::Party,
        RoutineScope::Everyone,
    ];

    pub fn price(self) -> u32 {
        match self {
            RoutineScope::One => crate::tuning::STACK_MARKET_ROUTINE_PRICE_ONE,
            RoutineScope::Party => crate::tuning::STACK_MARKET_ROUTINE_PRICE_PARTY,
            RoutineScope::Everyone => crate::tuning::STACK_MARKET_ROUTINE_PRICE_EVERYONE,
        }
    }

    /// How many etched disks this rung hands over.
    ///
    /// Fixed constants, **not** the live party and roster sizes. A quantity
    /// read off `Party` would change between the player reading the shelf
    /// and paying for it — a companion left behind, a program dismissed —
    /// which is the same objection `Game::market_program_price` already
    /// makes about folding Trace into a quote. What is on the shelf has to
    /// be what is bought.
    pub fn disks(self) -> u32 {
        match self {
            RoutineScope::One => crate::tuning::STACK_MARKET_ROUTINE_DISKS_ONE,
            RoutineScope::Party => crate::tuning::STACK_MARKET_ROUTINE_DISKS_PARTY,
            RoutineScope::Everyone => crate::tuning::STACK_MARKET_ROUTINE_DISKS_EVERYONE,
        }
    }

    /// How the rung reads on the shelf. Worded as what it would *outfit*
    /// rather than as a tier name or a bare count, because the price and
    /// the quantity are both already on the row and "T2" would say nothing
    /// about why anyone wants three.
    pub fn label(self) -> &'static str {
        match self {
            RoutineScope::One => "one program",
            RoutineScope::Party => "your party",
            RoutineScope::Everyone => "everything you own",
        }
    }
}

/// What one row of a Stack market's shelf is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MarketOfferKind {
    /// A bundle of etched disks — `scope.disks()` of them, ready to install
    /// into whichever holders the player picks. Nothing is added to
    /// `KnownRoutines`: a trader sells the *writing*, never the knowledge.
    /// See `Game::buy_market_offer`.
    Routine {
        ability: String,
        scope: RoutineScope,
    },
    /// A single exclusive routine's etched disk — the rare row. Deliberately
    /// a kind of its own rather than a `Routine` at a fourth scope: it is
    /// sold one at a time and priced off `STACK_MARKET_EXCLUSIVE_PRICE`
    /// rather than off any rung, and rolling it into `RoutineScope` would
    /// mean a rung whose `disks()` and `price()` both lie.
    ExclusiveDisk { ability: String },
    /// A program, adopted through the same `Game::adopt_program` an orphan
    /// goes through.
    Program { species: String },
}

/// One row of a Stack market's shelf, already priced — see
/// `Game::stack_market`. Renderers draw these verbatim.
///
/// `index` is the row's position in the *derived* shelf and is what
/// `Game::buy_market_offer` takes, not its position in this list: a bought
/// row is dropped from the view (that is what "whatever's sold is gone"
/// means on screen) while `resources::FrameMemory::bought` goes on
/// recording it by the index it always had.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MarketOffer {
    pub index: usize,
    pub kind: MarketOfferKind,
    /// The row's headline — what is being sold, and to whom.
    pub name: String,
    /// The line under it: an ability's own description, or what a program
    /// would join at.
    pub detail: String,
    pub price: u32,
    /// Whether the player has the Credits for it. The row is drawn either
    /// way — a shelf you cannot afford yet is a reason to go deeper.
    pub affordable: bool,
}

/// What one row of a caravan's shelf is.
///
/// Four kinds rather than the Stack market's three, and the extra one is
/// `Material`: a Stack trader is a stall in a corridor and deals in things
/// worth carrying out, where a caravan pulls up beside a base and the base
/// wants feedstock. Each variant carries what `Game::buy_caravan_offer`
/// needs to hand the goods over and nothing else.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CaravanOfferKind {
    /// A rolled copy, with its own rarity, affix and quality — the shelf is
    /// derived, so the copy is the same one every time it is looked at.
    Gear(GearCopy),
    /// An etched Routine Disk, by ability id. A disk and never a slot, for
    /// `Game::buy_market_offer`'s reason: who it is for is a question the
    /// player answers later, through `Game::install_disk`.
    Routine(String),
    /// A program, adopted through the same `Game::adopt_program` an orphan
    /// goes through.
    Program(String),
    /// A plain stack of cargo.
    Material(ItemId),
}

/// One row of a caravan's shelf, already priced — see `Game::caravan_view`.
///
/// `index` is the row's position in the *derived* shelf and is what
/// `Game::buy_caravan_offer` takes, not its position in this list: a bought
/// row is dropped from the view while `resources::CaravanMemory` goes on
/// recording it by the index it always had. Same split `MarketOffer` makes,
/// and for the same reason.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CaravanOffer {
    pub index: usize,
    pub kind: CaravanOfferKind,
    /// The row's headline — what is being sold.
    pub name: String,
    /// The line under it.
    pub detail: String,
    /// What one of it costs.
    pub unit_cost: u32,
    /// How many the trader has of it. One for a copy of gear and one for a
    /// program; a stack for cargo.
    pub qty: u32,
}

/// The visiting caravan's counter — see `Game::caravan_view`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CaravanView {
    pub trader: String,
    pub description: String,
    pub offers: Vec<CaravanOffer>,
    pub sells: Vec<CaravanSellRow>,
    pub credits: u32,
    pub currency: String,
    /// Ticks until it packs up. The one figure on the screen that is a claim
    /// about *time*, and it is here because the player's whole decision is
    /// whether to go and fetch something.
    pub ticks_left: u32,
}

/// One stack of the player's cargo a caravan will take, priced.
///
/// There is no buyback row to match it: a caravan keeps no shelf, so what is
/// sold here is gone. See `Game::sell_to_caravan`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CaravanSellRow {
    pub copy: GearCopy,
    pub name: String,
    pub held: u32,
    pub unit_price: u32,
}

/// A settlement's own counter — see `Game::settlement_view`.
///
/// Mirrors `CaravanView` and deliberately reuses its two row types:
/// `CaravanOffer`/`CaravanOfferKind` for what is on the shelf,
/// `CaravanSellRow` for what the player may sell into it. The two vendors
/// price the same four kinds of row differently — a caravan reads
/// `Game::caravan_unit_cost`, a settlement reads `Game::settlement_unit_cost`
/// at its own `Temperament` — but a row's *shape* is identical, and a second
/// pair of types here would be the copy that drifts the moment one grows a
/// field the other needs too.
///
/// No `trader`/`description`/`ticks_left` fields: those name a caravan's own
/// identity and its visit's clock, neither of which a settlement has — its
/// name and blurb are already `SettlementView`'s, read from the hub screen
/// this market opens from, and a settlement never leaves.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SettlementMarketView {
    pub offers: Vec<CaravanOffer>,
    pub sells: Vec<CaravanSellRow>,
    pub credits: u32,
    pub currency: String,
    /// The town refuses to trade — `settlements::relations::Standing::
    /// refuses_service`. A *closed* counter rather than an absent one: the
    /// screen still opens, with no rows and a line saying why, because
    /// `None` from `Game::settlement_view` is already spoken for as "the
    /// party has walked away" and closes the screen under them.
    pub closed: bool,
}

/// One stack of the player's cargo a Stack market will take, priced.
///
/// There is no buyback row to match it: a Stack trader keeps no shelf, so
/// what is sold here is gone. See `Game::sell_to_market`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MarketSellRow {
    pub copy: GearCopy,
    pub name: String,
    pub qty: u32,
    pub unit_price: u32,
}

/// Everything a Stack market screen draws — see `Game::stack_market`.
/// `None` from that call is the answer to "is there a live stall here",
/// so no screen has to ask separately.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StackMarketView {
    pub offers: Vec<MarketOffer>,
    pub sells: Vec<MarketSellRow>,
    /// What the player has to spend, and what it is called — so the screen
    /// never names the trade currency itself.
    pub credits: u32,
    pub currency: String,
}

/// One sellable program at a trading post, already priced — see
/// `Game::program_sale_options`. Renderers draw these verbatim and never
/// compute a payout of their own.
///
/// `Clone` because the confirmation screen holds the row the player picked
/// rather than re-querying: the price and the detach list it is asking them
/// to agree to must be the ones they were shown.
#[derive(Clone)]
pub struct ProgramSaleOption {
    pub entity: Entity,
    pub name: String,
    pub level: u32,
    /// `components::Stats::power()` — what the payout is derived from, shown
    /// so the price is explicable rather than arbitrary.
    pub power: i32,
    pub payout: u32,
    /// How many fusions deep this program's lineage is — see
    /// `components::FusionCount`. Carried for the same reason `activity`
    /// is: the screen that permanently erases a program should say what it
    /// is giving up, and a maxed fusion is the least replaceable thing on
    /// the list.
    pub fusions: u32,
    /// This program's rare-spawn tier — carried for exactly the reason
    /// `fusions` above is. An Overclocked program is the other least
    /// replaceable thing that can be on this list, and the tier is not
    /// something a payout figure reflects.
    pub rarity: Rarity,
    /// What the program is doing right now — see `Game::program_activity`.
    /// Shown on the row, so the screen that permanently erases a program
    /// says what it was in the middle of.
    pub activity: String,
    /// What this sale would also cancel, worded for display: e.g. "leaves
    /// your battle party", "stops working the Mining Node". Empty when the
    /// program is idle. The same facts as `activity`, worded as consequences
    /// — the confirmation screen warns, the row informs.
    pub detaches: Vec<String>,
}

/// The program on the player's weapon line, for the equipped panel. The
/// bonus is `Game::wielded_stat_bonus`'s live figure rather than anything
/// stored, so the panel moves as the program levels or is fused.
pub struct WieldedView {
    pub name: String,
    pub level: u32,
    pub bonus: (i32, i32),
}

pub struct PetInfo {
    pub entity: Entity,
    /// The same glyph and colour this program is drawn with on the map, so a
    /// menu row can carry its icon rather than making the player match a name
    /// to a letter. `EntityView` already carries the pair for the same reason;
    /// the two lists overlap (see `render/building.rs`'s `draw_base_staff`,
    /// which reads a row's identity from an `EntityView` and its tier from
    /// here), and a program that looked like one thing on one screen and
    /// another elsewhere would be worse than no icon at all.
    pub glyph: char,
    pub color: GlyphColor,
    pub name: String,
    pub level: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    /// This program's slot in the active party, or `None` if it isn't a
    /// member. Slot order is mechanically meaningful — front slots draw more
    /// fire (see `battle::slot_aggro_weight`) — so a frontend showing the
    /// roster shows the number, not just membership.
    pub party_slot: Option<u32>,
    /// Which of the four roles this program is filling — see
    /// `Game::program_role`. `owned_pets` is sorted into a run per role, so a
    /// screen listing it can head each run by comparing this against the row
    /// above rather than walking the list twice.
    pub role: ProgramRole,
    /// What this pet is doing right now — see `Game::program_activity`.
    pub activity: String,
    /// This individual's rolled quality tier (see `components::Potential`),
    /// e.g. "Excellent (94%)" — `None` for a creature with no `Potential`
    /// (shouldn't happen for anything spawned going forward, but possible
    /// for an old save predating this component).
    pub quality: Option<String>,
    /// How many fusions deep this program's lineage is, 0 to `MAX_FUSIONS`
    /// — see `components::FusionCount`. At `MAX_FUSIONS` it can no longer
    /// be fused.
    pub fusions: u32,
    /// How many of this program's upgrade slots are spent, 0 to
    /// `MAX_COMPANION_REFACTORS` — see `components::Refactors`. Counts the
    /// percentage buffs only; a zone bump spends none and shows up as the
    /// zone tag on `name` instead.
    pub refactors: u32,
    /// How many Kernel Rings this program has open, 0 to
    /// `tuning::KERNEL_RING_MAX` — see `components::KernelRing`. Absent means
    /// 0, like `refactors` above. Each one raises this program's level
    /// ceiling and pays a talent point per level earned above the base cap;
    /// `Game::companion_level_cap` is what the ceiling itself reads off.
    pub ring: u32,
    /// How many talent nodes this program has bought — see
    /// `components::Talents`. The count rather than the list, because every
    /// screen that lists a row goes through `Game::talent_options`.
    pub talents: u32,
    /// This program's rare-spawn tier — see `components::Rarity`. Already
    /// spelled into `name` as a prefix by `Game::creature_label`; carried
    /// separately so a menu can also colour the row without parsing it back
    /// out of the string.
    pub rarity: Rarity,
    /// Whether this is the program equipped as the player's weapon (see
    /// `resources::WieldedProgram`). At most one row in a list carries it.
    pub wielded: bool,
    /// Which of this program's three equipment slots are filled, as the
    /// fixed-width `w|a|m` cell `Game::gear_tag` builds. Pre-formatted rather
    /// than three booleans because `CompanionInfo` shows the same cell on the
    /// status panel, and one loadout must not read two ways.
    pub gear: String,
}

/// What a companion has earned and spent on its talent tree.
///
/// Both halves are **derived** — `earned` from the level, `spent` from the
/// length of `components::Talents` — so neither is a save field and neither
/// can desync from the other. See `game/talents.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TalentPoints {
    pub earned: u32,
    pub spent: u32,
}

impl TalentPoints {
    pub fn unspent(self) -> u32 {
        self.earned.saturating_sub(self.spent)
    }
}

/// One row of the talent ladder — see `Game::talent_options`.
///
/// It carries what a menu row needs and nothing more. `tag` is the node's
/// shape rather than its magnitude, because a screen that formats the
/// magnitude itself is a screen that can disagree with the one next to it.
pub struct TalentOption {
    /// 1-based, for the ladder's headings.
    pub tier: u32,
    pub id: crate::talents::TalentId,
    pub name: String,
    pub description: String,
    /// One word for what kind of node this is: "stat", "affinity", "routine"
    /// or "slot".
    pub tag: &'static str,
    /// Already bought.
    pub taken: bool,
    /// Buyable *right now* — in the next untaken tier, with a point unspent.
    pub takeable: bool,
}

/// Snapshot of the player's active companion, shown in the status panel
/// and during an intrusion.
pub struct CompanionInfo {
    pub entity: Entity,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    /// The companion's current battle status condition, if any (see
    /// `status_label`) — e.g. "Bleeding (2)". Always `None` outside a
    /// battle, since status effects are scoped to a single intrusion.
    pub status: Option<String>,
    /// Terse name of what commanding this companion in battle would do
    /// right now (e.g. "Rally") — see `Game::companion_ability_label`.
    /// Shown wherever a companion is listed outside battle, so the player
    /// can see what its Special would do. In battle the action menu carries
    /// the full label instead (see `Game::battle_action_options`).
    pub ability: String,
    /// The same `w|a|m` loadout cell the roster carries — see
    /// `PetInfo::gear`.
    pub gear: String,
}

/// The player's chosen icon, off `components::PlayerIdentity` — carried on
/// `EntityView::look`, `Some` for the player alone. Kept off `EntityView`
/// itself, rather than two bare fields, so a frontend can match on
/// `Some`/`None` for "is this the player" the same way it already does for
/// `is_anchor`'s structure-only fields, instead of reading `sprite`/`colour`
/// on every other entity and getting an empty string and a 0 back.
#[derive(Clone)]
pub struct PlayerLook {
    pub sprite: String,
    /// 0-based index into the renderer's player swatches; `None` is the
    /// `PLAYER` role colour. See `components::PlayerIdentity::colour`.
    pub colour: Option<u8>,
    /// The player's drawn 16x16 avatar; `None` for a player who never
    /// opened the editor. See `components::PlayerIdentity::icon`.
    pub icon: Option<PlayerIcon>,
}

#[derive(Clone)]
pub struct EntityView {
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
    /// **Its own channel, not the glyph's.** This used to *replace* `color`
    /// for a hostile, so a tile could say either what a program is or how
    /// dangerous it is, never both — and a boss or a nemesis gave up the
    /// danger read entirely to a reserved hue. The map draws it as a bar
    /// along the bottom edge, the mirror of the `rarity` bar along the top,
    /// and identity rides corner marks. Three readings, three channels.
    pub difficulty: Option<GlyphColor>,
    pub label: String,
    pub is_player: bool,
    /// The player's chosen sprite and colour — `Some` exactly when
    /// `is_player` is true, `None` for every other entity. See
    /// `PlayerLook`.
    pub look: Option<PlayerLook>,
    pub is_tamed: bool,
    pub is_companion: bool,
    pub is_hostile: bool,
    pub is_structure: bool,
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
    pub is_boss: bool,
    /// Whether this (creature) entity has beaten the party or driven them
    /// off — see `components::Nemesis`. Wins the glyph colour in
    /// `difficulty_color` over both the power-ratio con read and the boss
    /// override; see `rarity`'s doc for what that costs.
    pub nemesis: bool,
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

/// One structure on the roster screen — see `Game::structure_report`.
#[derive(Clone)]
pub struct StructureReport {
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
    PerkPoints,
    RosterFull,
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

/// One program assigned to a structure — see `StructureReport::assignees`.
#[derive(Clone)]
pub struct Assignee {
    pub entity: Entity,
    pub label: String,
    pub kind: TaskKind,
    /// Ticks done and ticks needed for one cycle. Both 0 for a `Guard`,
    /// which never progresses — `systems::task_progress_system` ignores it.
    pub progress: u32,
    pub required: u32,
    /// The posted program's own level and health.
    ///
    /// Here rather than only on its manifest because a posted program is
    /// often not reachable from the map at all: at its post it is not drawn
    /// (`drawn_on_surface_map`), and `Game::find_target_in_direction` skips
    /// what is not drawn, so the structure's sheet is the one screen that can
    /// tell you how the program working it is doing. `None` for anything
    /// without the component, which in practice is nothing that can hold a
    /// `Task`.
    pub level: Option<u32>,
    pub hp: Option<(i32, i32)>,
}

/// One addressable enemy group on the battle roster — "3 Glitches" as a
/// single row. Only the front member's HP is shown, because it is the only
/// one that can be hit (see `battle::EnemyGroup`).
#[derive(Clone)]
pub struct EnemyGroupView {
    /// Display letter, 'A'.. — how the player addresses this group.
    pub letter: char,
    pub species_name: String,
    pub count: usize,
    pub front_hp: i32,
    pub front_max_hp: i32,
    /// The rare-spawn tier of the *front* member — the same one whose HP the
    /// two fields above report, since a group's members can differ and the
    /// row has one of each to show. Deliberately absent from `species_name`,
    /// which the roster draws into a fixed-width cell an "Overclocked
    /// Scrapper 2" overflows; the renderer tags it outside that column.
    pub front_rarity: Rarity,
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    pub is_boss: bool,
    /// Whether this group is in melee range. A group that isn't can only
    /// act through a ranged move, and renderers dim it to make that legible
    /// rather than leaving the player to infer it from the log.
    pub engaged: bool,
    pub status_effect: Option<String>,
    /// Estimated chance (0.0-1.0) a decompile attempt against *this* group's
    /// front member would succeed, given its current HP fraction, its
    /// species' difficulty, and the potency of the catalyst the attempt
    /// would spend (see `Game::taming_catalyst`). `None` when the player
    /// holds no catalyst: there's no potency to quote odds for, and the
    /// action isn't available at all.
    pub decompile_chance: Option<f32>,
}

/// One row of the player's side of the roster.
#[derive(Clone)]
pub struct PartySlotView {
    /// Index into `BattleState::planned` — 0 is the player.
    pub slot: usize,
    pub entity: Entity,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    pub status_effect: Option<String>,
    /// What this member has left to spend on routines, or `None` for one
    /// holding no reserve at all. Every roster member carries one
    /// (`Game::roster_parts`), and it is what that member's own Specials and
    /// field routines are charged against — so this is `Some` for the whole
    /// party, and `None` only for a body that was never taken onto it.
    pub power: Option<f32>,
    /// This round's chosen action rendered for the roster, or `None` if the
    /// slot is still awaiting one.
    pub planned: Option<String>,
    /// Whether this slot is in the front line, which draws more enemy fire
    /// — see `FRONT_SLOTS`. Soft ranks: a back slot is still targetable.
    pub front: bool,
    /// Which of this member's three equipment slots are filled, as the same
    /// fixed-width `w|a|m` cell `Game::gear_tag` builds for the roster and
    /// the status panel. Pre-formatted for the reason `EntityView::gear` is
    /// — a loadout must read one way wherever it is shown, and a fight is
    /// exactly where a member found to be wearing nothing matters most.
    pub gear: String,
}

pub struct BattleView {
    pub groups: Vec<EnemyGroupView>,
    pub party: Vec<PartySlotView>,
    /// The slot currently choosing an action, or `None` once the round is
    /// fully planned and only needs resolving.
    pub active_slot: Option<usize>,
    /// The action menu for `active_slot`. Renderers draw these verbatim and
    /// never author an action string of their own — see
    /// `Game::battle_action_options`.
    pub options: Vec<ActionOption>,
    pub round: u32,
    pub player_decompiler: i32,
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
    /// `Dir::label` — "N", "E", "S", "W". A compass reading for the player,
    /// not something the renderer projects with.
    pub facing: &'static str,
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
    /// Core Fragment naming the Mining Node. `None` when an earlier step of
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

/// One row of an entity's routine panel — a slot, filled or not.
pub struct RoutineSlotView {
    pub index: usize,
    /// `None` for a free slot.
    pub ability: Option<crate::abilities::AbilityId>,
    /// The ability's name, or "(empty)" for a free slot.
    pub name: String,
    /// The ability's own authored description; empty for a free slot.
    pub description: String,
}

/// One row of the "whose routines?" picker — you and every program you own.
pub struct RoutineHolderView {
    pub entity: Entity,
    /// The map glyph and colour, for the same reason `PetInfo` carries them:
    /// these lists put the player and their programs side by side, and a name
    /// is the slower half of telling them apart.
    pub glyph: char,
    pub color: GlyphColor,
    /// "You" for the player, the program's display name otherwise.
    pub name: String,
    pub level: u32,
    pub filled: usize,
    pub slots: usize,
}

/// One row of the refactor picker — an upgrade item the player is carrying.
/// Held items only, so the screen never offers something that would be
/// refused for want of the item; every other refusal
/// `Game::refactor_companion` makes is about the program, not the shelf.
pub struct UpgradeOption {
    pub item: ItemId,
    pub name: String,
    /// The item's own authored `.ron` description, which is where an upgrade
    /// says what it does — the magnitudes are data, so there is nothing else
    /// the screen could derive the text from.
    pub description: String,
    pub qty: u32,
    /// Whether this one raises the zone tier rather than spending an upgrade
    /// slot. The two tracks read very differently to a player deciding, and
    /// this is the flag rather than the renderer re-deriving it.
    pub zone_bump: bool,
}

/// One row of the etch picker — a routine the player knows. Knowing it is
/// not enough to put it in a slot: it first has to be burnt onto a blank
/// Routine Disk (`Game::etch_disk`), which the screen reports separately
/// through `Game::blank_disks_held`, and only then installed.
///
/// An exclusive routine never appears here. Nothing writes one into
/// `KnownRoutines`, so the list is empty of them by construction rather than
/// by a filter — see `AbilityDef::exclusive`.
pub struct KnownRoutineView {
    pub ability: crate::abilities::AbilityId,
    pub name: String,
    pub description: String,
    /// Etched disks of this routine already in cargo — what the player would
    /// be adding to rather than starting. Read off `Game::etched_disks_of`,
    /// which is also what `Game::install_disk` refuses on, so the screen and
    /// the refusal cannot report different numbers.
    pub held: u32,
}

/// One row of the character-creation kit shelf — an item a new run may
/// spend its `tuning::CREATION_CREDITS` allowance on.
///
/// `price` is `ItemDb::value_of`, the same figure a trader prices the item
/// at, carried on the row rather than re-derived by the wizard: app-core
/// has no `ItemDb`, and a screen that priced a row itself could quote a
/// number the commit does not charge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartingItemRow {
    pub id: crate::items::ItemId,
    pub name: String,
    pub description: String,
    pub price: u32,
}

/// One row of the creation wizard's Perks step — a `PerkDef` as the
/// screen offers it, priced in Perk Points against
/// `tuning::CREATION_PERK_POINTS`. See `CreationCatalogue::perk_rows`.
#[derive(Clone, Debug, PartialEq)]
pub struct StartingPerkRow {
    pub id: crate::perks::Perk,
    pub name: String,
    pub description: String,
    pub cost: u32,
}

/// One row of the creation wizard's Routine step — an `AbilityDef::starter`
/// candidate, priced for the class the player has picked. See
/// `Game::starter_routine_rows`.
#[derive(Clone, Debug, PartialEq)]
pub struct StarterRoutineRow {
    pub id: crate::abilities::AbilityId,
    pub name: String,
    pub description: String,
    /// What it does *for this class* — the magnitude with the class
    /// affinity already applied. This is the field the step exists for.
    pub effect: String,
    pub power_cost: f32,
}

/// One row of the install picker — an etched Routine Disk in cargo, ready to
/// be spent on a slot.
///
/// `qty` is how many of that disk are carried, which is what makes a bought
/// bundle from a Stack market read as one row rather than six. `exclusive`
/// is the flag the screen marks a prize with; it is the ability's, not the
/// disk's, and it is here so no renderer has to reach back into `AbilityDb`
/// to ask.
pub struct EtchedDiskView {
    pub ability: crate::abilities::AbilityId,
    pub name: String,
    pub description: String,
    pub exclusive: bool,
    pub qty: u32,
}

/// One row of the extraction picker — a routine installed on the program
/// about to be broken down. `known` marks one the player has already
/// learned, which `Game::extract_routine` refuses: extraction teaches a
/// routine, and there is nothing to teach twice.
pub struct ExtractableRoutineView {
    pub ability: crate::abilities::AbilityId,
    pub name: String,
    pub description: String,
    pub known: bool,
}

/// Which second pick a field routine needs after it has been chosen, or
/// `None` if there is nothing left to choose.
///
/// The field-routine twin of `battle::SpecialTargeting`, and named the same way
/// for the same reason: this says which *picker* opens, while
/// `FieldRoutineTarget` carries what came back out of it.
///
/// Replaced a `needs_ally_target: bool`, which could express two answers and
/// there are now three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRoutinePick {
    /// Commits from the routine list — a `WholeParty` field buff, or either
    /// Stack movement routine, which acts on the party where it stands.
    None,
    /// One party member (`App::field_ally_options`) — a `Creature`-scoped
    /// `FieldBuff` authoring `AbilityTarget::OneAlly`.
    Ally,
    /// A cell of the frame the party is standing in — `AbilityEffect::Jump`.
    Cell,
}

/// What the player picked, once they have. The payload-carrying twin of
/// `FieldRoutinePick`, exactly as `battle::SpecialTarget` is to
/// `battle::SpecialTargeting` — `Game::run_field_routine` takes this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRoutineTarget {
    None,
    Ally(Entity),
    Cell(i32, i32),
}

/// One row of the field-routine picker — a field-only ability installed on
/// you or a program you own, run outside battle rather than spent as a
/// battle Special. See `Game::field_routines` and
/// `abilities::AbilityEffect::field_only`.
pub struct FieldRoutineView {
    pub ability: crate::abilities::AbilityId,
    pub name: String,
    pub description: String,
    pub holder: Entity,
    /// "You" for the player, the program's display name otherwise — same
    /// convention as `RoutineHolderView::name`.
    pub holder_label: String,
    /// What running this costs, already formatted with its unit — e.g.
    /// `"18 PWR"` or `"12 FTG"`. A string rather than a number because the
    /// two field-only families spend different needs, and a renderer that
    /// picked the noun itself would be a second place for it to be wrong.
    /// Same call `ActiveBuffView::magnitude` already makes.
    pub cost: String,
    /// `Some(reason)` means render it greyed with the reason shown — same
    /// contract as `battle::SpecialOption::unavailable`, and the same reason:
    /// the engine that knows *why* is the one that writes the sentence.
    pub unavailable: Option<String>,
    /// Which picker follows this row, if any.
    pub second_pick: FieldRoutinePick,
}

/// One row of the ally picker a `OneAlly` field routine opens — you, then
/// each active `Party` member, priced by the stats the buff is about to
/// land on. See `Game::field_routine_targets`.
///
/// Its own type rather than more fields on `RoutineHolderView`, which is the
/// *install* picker's row: that screen decides between free slots and this
/// one decides between bodies, and `running` below is a fact about a
/// (routine, target) pair that the shared `Game::routine_holder_view` has no
/// routine to answer for. The glyph, name and level still come from that one
/// builder, so the two pickers cannot describe the same holder two ways.
pub struct FieldRoutineTargetView {
    pub entity: Entity,
    /// The map glyph and colour, for the reason `RoutineHolderView` carries
    /// them: this list puts the player and their programs side by side.
    pub glyph: char,
    pub color: GlyphColor,
    /// "You" for the player, the program's display name otherwise.
    pub name: String,
    pub level: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    /// The buff this invocation would *overwrite*, if any — see
    /// `RunningBuffView`.
    pub running: Option<RunningBuffView>,
}

/// The routine-armed field buff a pending run would replace on one target.
///
/// `Game::arm_field_buff` displaces a running `Routine` buff of the same
/// kind and leaves a `Consumable` one of that kind alone, so this is
/// deliberately narrower than "what is running": it names what is about to
/// go. `ActiveBuffView` is the buff *list's* row and carries a holder label
/// this has no use for — here the row already names the holder.
pub struct RunningBuffView {
    /// `ActiveFieldBuff::name` — the routine that armed it. Two different
    /// routines can arm one kind (Ablative Layer and Long Winter both arm
    /// Mitigation), so "already running" alone would not say which is going.
    pub name: String,
    /// `FieldBuffKind::magnitude_label` of the power actually stored, the
    /// same call `ActiveBuffView::magnitude` makes and for the same reason.
    pub magnitude: String,
    /// `ActiveFieldBuff::duration_label` — `"90t"`, or `"until rest"` for a
    /// routine buff that has no turn count. Rendered here rather than in the
    /// renderer for `magnitude`'s reason: two screens describing one buff two
    /// ways is what building a tag twice buys.
    pub remaining: String,
}

/// One row of the buff list — the map screen's field buffs plus, during a
/// battle, any running `CombatBuff`. See `Game::active_buffs`.
pub struct ActiveBuffView {
    /// `ActiveFieldBuff::name` (the ability or item that armed it), or the
    /// stat name for a `CombatBuff` — that component carries no invocation-time
    /// name of its own, only which stat it moves.
    pub name: String,
    /// `FieldBuffKind::magnitude_label` of the power actually stored, which
    /// is already scaled — see that method's doc for why the tag is built
    /// here rather than in the renderer.
    pub magnitude: String,
    /// `ActiveFieldBuff::duration_label` for a field buff — `"90t"`, or
    /// `"until rest"` for a routine buff with no turn count. A `CombatBuff`
    /// has no such split and is always a round count.
    pub remaining: String,
    /// `Some(program name)` when the buff sits on a companion, `None` for
    /// the player.
    pub holder_label: Option<String>,
}

/// Everything the engine knows about one subject, for the manifest screen —
/// the player, a program you own, or a wild one. Shared header fields plus a
/// `subject` carrying the half that differs, so "the player has no Potential
/// roll" is a type-level fact rather than an `Option` a renderer can forget
/// to check.
///
/// `equipment` used to sit in `PlayerManifest` on the same argument — "a
/// program has no equipment" — and that stopped being true in 0.8.0, when
/// any program the player owns became able to wear gear. It is a shared
/// field for the same reason `routines` is: both are things a *wearer* has,
/// and neither cares which kind of subject is carrying them.
pub struct ManifestView {
    pub entity: Entity,
    /// The player's own name if they gave one at creation, "You" if they
    /// did not; a program's `CustomName` if it has one, else its
    /// zone-tagged species name (see `Game::zone_tagged_name`).
    ///
    /// **"You" is still the log's word for the player** and is deliberately
    /// not touched — a name belongs on the sheet the player opened to read
    /// about themselves, not in a line describing a swing.
    pub name: String,
    pub glyph: char,
    pub color: GlyphColor,
    /// `None` for a wild program, which carries no `Experience` until it is
    /// compiled.
    pub level: Option<u32>,
    /// `(xp, xp_to_next)`, `None` for the same reason `level` is.
    pub xp: Option<(u32, u32)>,
    pub hp: i32,
    pub max_hp: i32,
    /// The player's is `Game::effective_atk` (equipment folded in); a
    /// program's is its raw `Stats`.
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    /// The damage band this combatant swings for, already formatted by
    /// `Game::damage_range_label` — a worn weapon's if it has one, else its
    /// first species move's, else the player's unarmed band.
    ///
    /// Formatted here rather than in a renderer for the reason `copy_name`
    /// is: two screens printing a range two ways is how they come to
    /// disagree with the damage the fight actually deals.
    pub damage: String,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    /// The to-hit half of a fight, through `battle::accuracy_of` /
    /// `evasion_of` — **derived, never stored**, so quoting them here is two
    /// calls and not a cached copy that can drift from what
    /// `battle::resolve_attack` rolls.
    ///
    /// Carried for both subjects even though only the player's are drawn:
    /// the combat block belongs to one struct, and the program page's
    /// omission is a row budget it has run out of (see
    /// `render/manifest_layout`'s clearance sweep), not a fact the engine
    /// declines to compute. That makes buying it room later a layout change
    /// rather than a data change.
    ///
    /// `f32` rather than `i32` because both constants are halves —
    /// `ACCURACY_PER_LEVEL` is 0.5 — so a level-1 player rounds from 11.5,
    /// and a stat sheet that rounds a stat is quoting a number the roll does
    /// not use.
    pub accuracy: f32,
    /// See `accuracy`.
    pub evasion: f32,
    /// Active battle status condition, e.g. "Bleeding (2)" — see
    /// `Game::status_label`. Always `None` outside an intrusion.
    pub status_effect: Option<String>,
    /// Every routine slot, filled or empty. Reuses `RoutineSlotView` rather
    /// than a parallel type, so the manifest and the routines menu cannot
    /// disagree about what is installed.
    pub routines: Vec<RoutineSlotView>,
    /// One entry per *occupied* equipment slot — an empty slot is absent
    /// rather than listed as "(none)", so the section shrinks to what is
    /// actually worn and disappears entirely for a wild program, which has
    /// no `Equipment` component at all.
    pub equipment: Vec<ManifestEquipSlot>,
    pub subject: ManifestSubject,
}

/// What the inspector found in the direction you pointed — see
/// `Game::find_target_in_direction`. The variant is the answer that walk
/// already computed, so a caller routing to a screen never has to ask the
/// world a second time what kind of thing it is holding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectTarget {
    Creature(Entity),
    Structure(Entity),
    /// A trader walking through, or standing beside the counter. Its own
    /// variant rather than `Creature`, because nothing may target one as a
    /// combat participant: it carries no `Creature`, no `Stats` and no
    /// `Hostile`, and a manifest opened on it would be a sheet with nothing
    /// on it. What the player gets instead is the trader's own line, through
    /// `Game::caravan_blurb`.
    Caravan(Entity),
    /// A structure on order that the crew has not raised yet. Its own
    /// variant rather than `Structure`, because there is no structure there
    /// to inspect: no stock, no status, no tier, nothing to upgrade or
    /// demolish. What the player gets instead is the request's own state —
    /// what is still to be fetched and how far along the raising is —
    /// through `Game::build_order_row`.
    BuildSite(Entity),
    /// A settlement standing on the zone surface. Its own variant rather
    /// than `Structure`, for `Caravan` and `BuildSite`'s reason one level
    /// out: a manifest opened on a settlement would be a sheet with nothing
    /// on it — no `Stats`, no stock, no tier. What the player gets instead
    /// is the whole hub, through `Game::settlement_report` — reading a
    /// town's identity from across the map is what examine is *for*, and
    /// it stays correct once a later phase adds a market: reading a
    /// Broker's board and signing it are already two different questions
    /// (`Game::broker_reach`), so a shelf gets its own reach check without
    /// this variant moving.
    Settlement(Entity),
}

/// The whole of a settlement's hub screen — `Game::settlement_report`.
///
/// Identity only, Phase 2's decision: no action rows and no stubs for a
/// market or a job board, both of which land as their own fields later
/// rather than widening this one under a name that stops matching what it
/// holds.
///
/// Every label is a call onto the resolved def's own enum — `kind.label()`,
/// `specialty.label()`, `temperament.label()` — rather than a `match`
/// re-stated here, which is what keeps a new catalogue variant's label
/// living in exactly one place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettlementView {
    pub name: String,
    pub kind: &'static str,
    pub specialty: &'static str,
    pub temperament: &'static str,
    pub blurb: String,
    /// How the town regards the party — `Standing::label()`, a call onto
    /// the band's own enum for `kind`/`specialty`/`temperament`'s reason.
    pub standing: &'static str,
    /// One sentence per aid this town currently offers, already in the
    /// player's words — see `Game::settlement_aid_lines`. Empty when it
    /// offers none, and an empty list draws no rows and no header.
    pub aid: Vec<String>,
}

pub enum ManifestSubject {
    Player(PlayerManifest),
    /// Boxed because a program's sheet is three times the player's and the
    /// enum would otherwise be sized by the larger arm everywhere it is
    /// returned — the shape `clippy::large_enum_variant` names.
    Program(Box<ProgramManifest>),
}

/// The player-only half of a manifest.
pub struct PlayerManifest {
    /// See `components::PowerReserve`.
    pub power: f32,
    pub decompiler: i32,
    pub perk_points: u32,
    /// Every perk bought at least once, as (display name, level).
    pub perks: Vec<(String, u32)>,
    pub position: (i32, i32),
    pub zone: u32,
    pub pet_count: usize,
    pub pet_capacity: usize,
    pub cargo_used: u32,
    pub party: Vec<CompanionInfo>,
    /// Credits and Portal Fragments — `ItemDef::banked` pools, deliberately
    /// outside `PlayerStatus::inventory`, which is why the sheet's cargo
    /// count says nothing about either. Read through `Game::banked` rather
    /// than from an `Inventory`, since a bank is not something carried.
    pub credits: u32,
    pub portal_fragments: u32,
    /// Which mode this run is being played on. Carried as the mode rather
    /// than a finished phrase for the reason `ProgramManifest::base_job` is
    /// — the renderer is where every player-facing word on this screen is
    /// chosen.
    pub difficulty: DifficultyMode,
    /// `Game::current_tick` — how long the run has been going, in the same
    /// unit the log's cycle counter uses.
    pub cycle: u64,
    /// How many contracts the run is currently signed to
    /// (`Game::active_contracts`). A count and not the rows: the board and
    /// the contracts screen are where a player reads what they say, and a
    /// stat sheet quoting objectives would be a third wording of them.
    pub active_contracts: usize,
    /// The class picked at creation, resolved live through `ClassDb` — see
    /// `Game::player_class_view`. `None` for a classless run.
    pub class: Option<PlayerClassView>,
}

/// One worn item and the bonus it is *currently* granting.
///
/// `gear_level`/`fusion_tier` are the values captured on the `EquippedItem`
/// at equip time, and the stat fields are `EquipmentStats::scaled_for_level`
/// then `fused_for_tier` applied with exactly those — not a fresh preview at
/// today's zone level, which is what the inventory screen shows instead.
pub struct ManifestEquipSlot {
    /// `EquipmentSlot::short_label()` — "WEP", "ARM", "MOD", the vocabulary
    /// every other list that names gear already tags a row with.
    ///
    /// The compact form and not `label()`, because this row shares a
    /// half-width box with the bonus column and the name beside it is
    /// `Game::copy_name`'s, which spends its width on a tier word, a prefix
    /// affix, a suffix phrase and a quality figure. Four characters of
    /// "Weapon: " is what the affix at the end of that name is short of at
    /// 1280x720 — see the gui's `a_dropped_equipment_row_keeps_the_affix_in_
    /// its_name`.
    pub slot: String,
    pub item_name: String,
    pub gear_level: u32,
    pub fusion_tier: u32,
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    pub decompiler: i32,
}

/// The creature-only half of a manifest — an owned program or a wild one.
pub struct ProgramManifest {
    /// The species name, present only when a `CustomName` is overriding it,
    /// so the header can show "Hexed (Scrapper 2)" without repeating itself
    /// for an unrenamed program.
    pub species_name: Option<String>,
    pub is_hostile: bool,
    pub is_tamed: bool,
    pub is_companion: bool,
    pub is_boss: bool,
    /// What this program is doing right now — see `Game::program_activity`.
    /// `None` for a program you don't own, which has no job to report.
    pub activity: Option<String>,
    /// Where this program is posted, as `(what the post is, the structure's
    /// label)` — see `Game::program_post`, which `activity` above is built
    /// on top of, so the two cannot name different structures.
    ///
    /// `None` for a program with no `Task`: idle, in the party, or wild.
    /// Carried as the kind rather than as a finished phrase for the same
    /// reason `base_job` is — the renderer is where every other
    /// player-facing word on this screen is chosen.
    pub post: Option<(TaskKind, String)>,
    /// `None` for a creature with no `Potential` component — an old save
    /// predating it, or a test helper that spawned one directly.
    pub potential: Option<ManifestPotential>,
    pub fusions: u32,
    /// `tuning::MAX_FUSIONS`, carried so the renderer prints "1/3" without
    /// importing a tuning constant of its own.
    pub max_fusions: u32,
    /// The rare-spawn tier (`components::Rarity`). The manifest is the page
    /// a player opens to find out what something *is*, so the tier belongs
    /// here as a fact about the program and not only as the colour of a bar
    /// on the map — which is the only place it used to appear for anything
    /// the player was not already fighting.
    pub rarity: Rarity,
    /// Spent upgrade slots and `tuning::MAX_COMPANION_REFACTORS`, the same
    /// pair as `fusions`/`max_fusions` above and for the same reason.
    pub refactors: u32,
    pub max_refactors: u32,
    /// Kernel rings open on this program and `tuning::KERNEL_RING_MAX`, the
    /// same pair as `fusions`/`max_fusions` above — see
    /// `components::KernelRing`.
    pub ring: u32,
    pub max_ring: u32,
    /// The level this program can reach with the rings it has
    /// (`Game::companion_level_cap`). Carried beside the ring count because
    /// the count alone says nothing about what it bought.
    pub level_cap: u32,
    /// Talent points spent and earned — see `Game::talent_points`. Both
    /// derived; neither is stored anywhere.
    pub talents_spent: u32,
    pub talents_earned: u32,
    /// The zone tier this program is scaled to (`components::ZonePortal`),
    /// beside the zone the player is currently standing in.
    ///
    /// This pair is the whole reason the manifest carries either. Nothing on
    /// screen otherwise tells a player in zone 4 that the Scrapper they have
    /// carried since the opening ring is three doublings behind the ground
    /// under it — the tag on its name says "1" and means nothing without the
    /// number to compare it to.
    pub zone_tier: u32,
    pub player_zone: u32,
    pub habitats: Vec<Biome>,
    pub moves: Vec<MoveDef>,
    pub work_resource: Option<ItemId>,
    pub taming_difficulty: f32,
    /// Estimated decompile chance if an intrusion started right now, using
    /// the creature's current HP fraction. `None` when the player holds no
    /// taming catalyst: there is no potency to quote odds for, and the action
    /// isn't available at all.
    pub decompile_chance: Option<f32>,
    pub growth_multiplier: f32,
    pub base_speed: i32,
    /// Drawn as "Analysis" — `base_int` is the field name, not the word the
    /// player reads. Shown beside `base_speed` in the manifest's WORK box
    /// rather than in SPECIES, because both are about what this program is
    /// like to *post* somewhere.
    pub base_int: i32,
    /// Categories this species is not neutral in, in `AffinityKind` order.
    /// Empty for a species that declares nothing, so the screen omits the
    /// section entirely rather than drawing five rows of 1.00.
    pub affinities: Vec<(AffinityKind, f32)>,
    /// The class this species reads as, which decides what it does when
    /// posted to a structure — extraction, guarding or repair, or nothing
    /// at all for the two classes with no base job.
    ///
    /// Carried as the class rather than as a finished phrase, because the
    /// class is the vocabulary `assets/species/README.md` teaches and the
    /// renderer is where every other player-facing word on this screen is
    /// chosen ("Analysis" above is the same decision). `None` for a boss and
    /// for anything else outside the class system.
    pub base_job: Option<AffinityClass>,
    /// Where this program's reserves stand — see `needs::NeedDef`. **Empty**
    /// for a program that carries none, and for an install with
    /// `assets/needs/` deleted, so the section is absent entirely rather than
    /// present and empty.
    pub needs: Vec<NeedRow>,
}

/// One need on the manifest: what it is called, how it is doing, and what the
/// program is doing about it.
///
/// **Banded in words, never a number.** There is no player-facing float in
/// this game and no player-facing tick count, and a reserve is neither more
/// nor less legible for being shown as `37.4`.
#[derive(Debug, Clone, PartialEq)]
pub struct NeedRow {
    pub name: String,
    pub band: &'static str,
    /// The def's `servicing` verb while the program is off shift for this
    /// need, `None` otherwise.
    pub servicing: Option<String>,
}

/// Which word a reserve reads as, as a fraction of `NEED_MAX`.
///
/// Four bands, and the boundaries are deliberately not the def's own
/// `critical`/`content`: those are the *mechanism* and vary per need, while
/// this is the player's read of a bar and has to mean the same thing on every
/// row of the page.
pub fn need_band(fraction: f32) -> &'static str {
    match fraction {
        f if f >= 0.75 => "steady",
        f if f >= 0.45 => "fraying",
        f if f >= 0.20 => "strained",
        _ => "critical",
    }
}

/// What a program is worth at a post: how fast it cycles, how reliably, and
/// what its class adds on top. The three facts the Base Staff screen ranks
/// programs by, and the same trio the manifest's WORK box draws.
///
/// A grouping and not an abstraction — no trait, no second implementor. It
/// exists so the walk from an entity to its `SpeciesDef` is written once
/// instead of once per fact, since all three answers come off the same def
/// and all three are `None` together when it is missing.
///
/// Deliberately **not** the battle stats. Nothing on this struct is read by
/// combat and nothing combat reads belongs on it: `work_ticks_at_speed`
/// takes `speed`, the gather roll takes `analysis`, and `class` decides only
/// what a landed cycle is worth. A program's Attack has no effect at a
/// machine, so a staffing screen that showed it would be describing a
/// relationship the sim does not have.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkProfile {
    /// `SpeciesDef::base_speed` — the cycle rate, against
    /// `tuning::DEFAULT_BASE_SPEED` as the baseline that leaves a machine's
    /// shipped `ticks_per_unit` unchanged (see `systems::work_ticks_at_speed`).
    pub speed: i32,
    /// `SpeciesDef::base_int`, the extraction aptitude — how often a cycle
    /// lands rather than fizzles. Named for the word the player reads
    /// ("Analysis") rather than the field, the choice `ManifestEntry`
    /// already documents.
    pub analysis: i32,
    /// The species' base job, or `None` for a boss and for anything else
    /// outside the class system. Carried as the class rather than a finished
    /// phrase so the renderer keeps choosing the player-facing word.
    pub class: Option<AffinityClass>,
}

/// An individual's four `Potential` rolls, surfaced separately rather than
/// only as the aggregate tier the party menu shows.
#[derive(Debug, PartialEq)]
pub struct ManifestPotential {
    pub hp_roll: f32,
    pub atk_roll: f32,
    pub def_roll: f32,
    pub growth_roll: f32,
    /// `Potential::quality_percent`.
    pub percent: u32,
    /// `Potential::quality_label`.
    pub label: String,
}

/// One rung on the achievements screen, earned or not — the screen lists
/// every authored achievement, because the point is showing what is left.
///
/// Built by `achievements::report` rather than by a `Game` method, unlike
/// every other view here: the screen is reachable from the main menu, where
/// there is no run and so no `Game` to ask. app-core holds the db and the
/// profile and calls that function; both the row count it scrolls against
/// and the rows gui draws come from the one call, which is the read-only
/// screen rule the history and roster screens follow.
pub struct AchievementRow {
    pub name: String,
    pub description: String,
    /// What it pays, already worded for the screen.
    pub reward: String,
    /// `None` for a rung not yet earned.
    pub earned: Option<EarnedSummary>,
}

/// What is known about an achievement that has been earned.
pub struct EarnedSummary {
    /// The cycle it was first earned on.
    pub tick: u64,
    /// Whether it has ever been earned on permadeath.
    pub permadeath: bool,
    /// Which stat the roll landed on, for a `Reward::RandomMainStat`.
    pub rolled_stat: Option<String>,
}

/// One contract, worded — an offer on a Broker's board or one the run is
/// already holding. `Game::contract_board` and `Game::active_contracts`
/// return these and renderers draw them verbatim.
///
/// The engine composes `objective_line` and `reward_line` rather than handing
/// a renderer the `Objective` and `Reward` to word itself, for the reason
/// `Game::copy_name` exists: two screens must not word one contract
/// differently, and a drop line and the screen you open next disagreeing
/// about what you just took is exactly the failure that costs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ContractRow {
    /// Which town posted it, or `None` for the run's own Broker — see
    /// `resources::ActiveContract::issuer`. Carried on the row so a screen
    /// can tell a held town job from a held Broker job without a second
    /// lookup, and so the delivery key is the row's own.
    pub issuer: Option<crate::settlements::SettlementKey>,
    /// That town's name, resolved once here rather than in a renderer —
    /// `Game::copy_name`'s rule.
    pub issuer_name: Option<String>,
    pub id: crate::contracts::ContractId,
    pub name: String,
    pub description: String,
    /// What the objective asks, already worded.
    pub objective_line: String,
    pub reward_line: String,
    /// 0 on an offer that has not been accepted.
    pub progress: u32,
    /// `Objective::target()` — every contract displays and completes through
    /// one `progress >= target` rule.
    pub target: u32,
    /// Whether this is an onboarding mission — see
    /// `Game::ensure_tutorial_held`. The renderer draws these green and
    /// app-core refuses to give one back.
    pub tutorial: bool,
}

/// One site standing on the Relay's board — see `Game::sortie_board`.
///
/// Every figure on it is a **call** rather than a copy: `ticks` is
/// `Game::sortie_duration(risk, battles)`, the same computation the
/// countdown runs, `views::BuildOrderRow`'s rule. A screen quoting one
/// number while the trip runs another is the failure that rule exists for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SortieRow {
    pub id: crate::sorties::SortieId,
    pub name: String,
    pub description: String,
    /// Steps above the sector baseline — the site's own offset, which is
    /// also what `ticks` is priced off.
    pub risk: u32,
    /// Fixed the moment the offer appeared, not rolled at dispatch, so this
    /// row can be quoted before the player signs for it.
    pub battles: u32,
    pub ticks: u64,
}

/// One trip currently away, as the Relay screen lists it — see
/// `Game::sortie_reports`.
///
/// Every figure is derived off the stored record at the moment it is read;
/// nothing here is a second tally kept in step by hand. Members and
/// casualties are rendered here and not by the renderer, `MemoryRow`'s
/// reason: a Permadeath casualty's entity is gone by the time this is drawn,
/// so its name has to be the one captured when it fell.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SortieReport {
    pub site: String,
    /// Who is still out there, by display name.
    pub members: Vec<String>,
    /// Who did not come back, named at the moment they fell.
    pub casualties: Vec<String>,
    pub kills: u32,
    pub xp: u32,
    pub battles_done: u32,
    pub battles_total: u32,
    /// Ticks still to run. A trip that has aborted still runs this out —
    /// the countdown was always going to take that long, and there is no
    /// teleport home.
    pub ticks_left: u64,
    /// A member has gone down and the remaining battles are being skipped.
    pub aborted: bool,
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
}

impl BuildOrderRow {
    /// What a screen calls this request — `Lathe`, or `Lathe → Mk3`.
    ///
    /// Composed here rather than baked into `structure`, so the name and the
    /// tier cannot be split back apart by a caller that wants only one.
    pub fn label(&self) -> String {
        match self.goal {
            crate::components::BuildGoal::New => self.structure.clone(),
            crate::components::BuildGoal::Upgrade { to_tier } => {
                format!("{} → Mk{to_tier}", self.structure)
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

/// One pile the base is holding, as the stock strip lists it — see/// One pile the base is holding, as the stock strip lists it — see
/// `Game::base_stock`.
///
/// Carries the tag *and* the name: the strip draws the tag alone, and the
/// name is what a caller with room shows instead of teaching the player a
/// glossary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StockRow {
    pub item: ItemId,
    pub tag: String,
    pub name: String,
    pub qty: u32,
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

/// One thing a program remembers, as `Mode::CompanionMemories` lists it —
/// see `Game::memory_report`.
///
/// **The subject is rendered here and not by the renderer.** A `Species`
/// subject needs `SpeciesDb`, a `Structure` subject `StructureDb`, and a
/// `Program` subject the name captured on the record when it was written;
/// the renderer holds none of the three. It is also `render/stack.rs`'s
/// `cell_mark` rule — the match behind this field is exhaustive, so a
/// seventh `MemorySubject` variant fails to compile rather than shipping a
/// row that names nothing.
///
/// `intensity` and `age_ticks` are numbers rather than pre-rendered text, in
/// the other direction: neither needs the world, and pre-rendering them
/// would invent a second formatting seam for the one screen that reads this.
#[derive(Clone, Debug)]
pub struct MemoryRow {
    /// `memories::MemoryDef::name` — its first reader.
    pub name: String,
    /// `memories::MemoryDef::blurb` — likewise.
    pub blurb: String,
    /// What it is about, already named. `None` for
    /// `components::MemorySubject::Nothing`, which is a memory of an event
    /// rather than of a thing and so has nothing to name — an empty string
    /// would leave the renderer testing for one.
    pub subject: Option<String>,
    /// Signed and decayed to the current tick: `components::Memory::
    /// intensity`, projected rather than recomputed.
    pub intensity: f32,
    /// How long ago it last landed, in words — and **not** a tick count.
    /// The game has no player-facing unit of time: nothing in any screen or
    /// any log line has ever said "tick", so a figure here would be the
    /// first, and a number the player has no scale for is not an answer.
    ///
    /// Banded against the **def's own half-life**, which is the only
    /// yardstick that makes two memories comparable: 6,000 ticks is fresh
    /// for a mauling and ancient for a bad shift, and the same phrase on
    /// both would mean neither. Derived here rather than in the renderer
    /// because the half-life is on the def, which never reaches the page.
    pub age: String,
}

/// One item the transfer screen offers a row for — see
/// `Game::transfer_offer`.
///
/// The union of the two old offers: `on_shelves` is what the adjacent
/// `Stock` buffers are holding of it, `carried` what the pack holds. An item
/// on both sides is **one** row carrying both figures, which is the whole
/// reason the two screens became one.
///
/// **`carried` is a holding and `can_put` is a permission**, and they part
/// company in exactly two cases: no Depot beside the party, and a banked
/// item, both of which may still be taken. The screen draws `carried` — a
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

/// The player's class, as their own manifest reads it back — see
/// `Game::player_class_view`. Not a `ClassRow`: that carries the starting
/// kit, which the wizard's Kit step *replaces*, so quoting it on a sheet
/// read mid-run would name items the player never had.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerClassView {
    pub name: String,
    /// One term per non-neutral axis, `classes::format_affinity_bonuses`.
    pub bonuses: String,
}

/// One class offered on the creation screen — see `Game::class_rows`.
///
/// `trade` is pre-formatted in the engine (`classes::format_trade`) so the
/// two renderers cannot word one class's trade differently —
/// `Game::copy_name`'s reason.
///
/// **There is no kit summary.** The row carried one until the wizard grew
/// a Kit step, which *replaces* what a class authored — so listing the
/// class kit on the picker was advertising equipment the player was about
/// to buy for themselves two screens later. `ClassDef::kit` is untouched
/// and still the fallback for an empty basket.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassRow {
    pub class: PlayerClass,
    pub name: String,
    pub description: String,
    /// `"Bonus to damage at the expense of healing"` — what this class
    /// gives up and what it gets for it, as a sentence.
    pub trade: String,
}

/// Which ledger a respec would clear — the parameter both `Game::respec_quote`
/// and its refusal take, so the screens' footer and the commit ask exactly one
/// question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RespecSubject {
    /// The player's perks.
    Perks,
    /// One companion's talents.
    Talents(bevy_ecs::entity::Entity),
}

/// What a respec would cost and hand back — see `Game::respec_quote`.
///
/// Every figure is a call and none is stored, `views::BuildOrderRow`'s rule:
/// the footer on the picker, the confirm page and the commit all read this
/// one derivation, so they cannot quote different numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct RespecQuote {
    /// `tuning::RESPEC_CREDIT_COST`, carried here so no renderer names it.
    pub cost: u32,
    /// Credits the player is holding.
    pub credits: u32,
    /// How many perk levels or talents would be wiped.
    pub purchases: u32,
    /// Points that come back — Perk Points, or talent points freed by
    /// clearing the list.
    pub points_returned: u32,
    /// Why this is refused, or `None` if it would go through. The screens
    /// grey the row on this and `App::report` speaks the same sentence.
    pub refusal: Option<String>,
}

/// One row of `Mode::DownedPrograms`'s list — see `Game::downed_program_rows`.
///
/// `grade` is `items::DownedProgram::grade()`'s own answer, carried rather
/// than left for the renderer to re-fold from `condition`/`rarity`/`level` —
/// `message_history`'s rule, that a per-row transform belongs in the engine.
#[derive(Clone, Debug, PartialEq)]
pub struct DownedProgramRow {
    /// The species' display name, falling back to the raw id for a mod
    /// species since removed — `Game::downed_program_label`'s tolerance.
    pub name: String,
    pub level: u32,
    pub rarity: Rarity,
    /// 0..=100 — `items::DownedProgram::condition`, unrolled.
    pub condition: u8,
    pub boss: bool,
    pub grade: f32,
}

/// The extraction bench a screen names — `Game::extraction_bench`. Absent
/// entirely when none stands, so a renderer never has to read a tier of
/// zero as "none".
#[derive(Clone, Debug)]
pub struct ExtractionBenchView {
    pub name: String,
    pub tier: u32,
}

/// One row of `Mode::Tools`'s list — see `Game::tool_rows`.
///
/// One row per tool the player *knows* plus any tool actually *installed*
/// (plan decision 3, task 5): a tool researched but never forged still
/// needs a row for the forge verb to act on, and the starter tool is
/// installed without ever entering `KnownTools` (task 1's own ruling), so
/// neither set alone would list it. `slot`/`carriers_held` are the two
/// figures that tell those rows apart — a screen reading this needs no
/// second lookup into either store.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolRow {
    pub id: ToolId,
    pub name: String,
    pub category: ToolCategory,
    pub tier: u32,
    pub ticks: u64,
    /// `Some(slot index)` while installed, `None` for a known-but-not-yet-
    /// forged-or-installed tool.
    pub slot: Option<usize>,
    /// Carriers held in `Inventory` (`ItemId::tool(id)`) — forged but not
    /// yet installed.
    pub carriers_held: u32,
}
