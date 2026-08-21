//! Read-only snapshots of engine state, shaped for the renderer.
//!
//! Every one of these is produced by a `Game` method and consumed by
//! app-core/gui. They are plain data with no back-reference into the ECS —
//! that is what keeps the renderer from reaching into the `World`.

use crate::abilities::AffinityKind;
use crate::battle::ActionOption;
use crate::components::{EquippedItem, GlyphColor, MachineStatus, Rarity, TaskKind};
use crate::items::{GearCopy, ItemId};
use crate::perks::Perk;
use crate::research::ResearchId;
use crate::resources::DifficultyMode;
use crate::species::{AffinityClass, MoveDef};
use crate::structures::StructureId;
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
/// routine's magnitudes are scaled for their caster and a renderer reading
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
    pub grant: Option<RoutineDetailView>,
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
    /// What it does, with every magnitude scaled for the caster through the
    /// same `abilities::scaled_range`/`scaled_hp_power`/`scaled_stat_power`
    /// the cast itself uses.
    pub effect: String,
    /// Battle rounds before it can run again. 0 means unthrottled.
    pub cooldown: u32,
    /// What running it costs its caster, through
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

#[derive(Clone)]
pub struct EntityView {
    pub entity: Entity,
    pub pos: (i32, i32),
    pub glyph: char,
    pub color: GlyphColor,
    pub label: String,
    pub is_player: bool,
    pub is_tamed: bool,
    pub is_companion: bool,
    pub is_hostile: bool,
    pub is_structure: bool,
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
    /// Whether this (tamed) entity holds a `TaskKind::GatherResource` post
    /// and is not currently standing at it — walking in to take the job,
    /// carrying a load to a depot, or on its way back.
    ///
    /// The one tamed program a frontend may draw, and only while this is
    /// true. Two reasons it is this narrow. A worker is the only tamed
    /// program whose `Position` the sim keeps honest at all —
    /// `haul_step_system` walks it, while a guard, an idle program and a
    /// party member each keep whatever tile they were on when they took the
    /// job and are never moved again. And at its post it belongs *under* its
    /// machine's glyph: a base at rest should read as buildings, with motion
    /// the only thing that draws the eye.
    pub worker_away_from_post: bool,
    /// Whether this entity's `Position` is a tile the sim actually keeps up
    /// to date — the input `views::drawn_on_surface_map` takes, and the
    /// wider question `worker_away_from_post` above is one answer to.
    ///
    /// Three tamed programs have an honest tile and each for its own
    /// reason: a worker walking to or from its post (`haul_step_system`
    /// moves it), an idle base staff member (`schedule_base_labour` parks it
    /// on a ring around the Home every tick), and nothing else. A guard
    /// keeps whatever tile it was on when it took the job, and a party
    /// companion keeps the tile it was beaten on — neither is ever written
    /// again, so drawing either would claim it is somewhere it isn't.
    ///
    /// Distinct from `worker_away_from_post`, which stayed narrow because a
    /// frontend marks "someone is on this job" with it: an idle program is
    /// on no job, so widening that field would have put the mark on it.
    pub position_is_honest: bool,
    /// If this is a structure, whether a posted program is standing at it
    /// right now — a guard (which never moves, so always) or a worker that
    /// has not stepped off on an errand.
    ///
    /// The other half of `worker_away_from_post`, and the two are exclusive
    /// per posted program: a frontend that marks "someone is on this job"
    /// draws the mark on the program when the program is drawn, and on the
    /// structure when it isn't. Distinct from `structure_worker`, which
    /// counts any `Task` wherever its holder happens to be.
    pub structure_attended: bool,
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
/// 2026-08-14, when base staff arrived. Two kinds of tamed program now have
/// an honest tile: one walking to or from a post, and one **idle in the
/// base**, which `schedule_base_labour` parks on a ring around the Home
/// every tick. A party companion still has neither — its `Position` is the
/// tile it was beaten on, written at capture and never again — so it is
/// still not drawn. `EntityView::position_is_honest` is the value.
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
/// The field-cast twin of `battle::SpecialTargeting`, and named the same way
/// for the same reason: this says which *picker* opens, while
/// `FieldCastTarget` carries what came back out of it.
///
/// Replaced a `needs_ally_target: bool`, which could express two answers and
/// there are now three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldCastPick {
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
/// `FieldCastPick`, exactly as `battle::SpecialTarget` is to
/// `battle::SpecialTargeting` — `Game::cast_field_routine` takes this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldCastTarget {
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
    pub second_pick: FieldCastPick,
}

/// One row of the ally picker a `OneAlly` field routine opens — you, then
/// each active `Party` member, priced by the stats the buff is about to
/// land on. See `Game::field_cast_targets`.
///
/// Its own type rather than more fields on `RoutineHolderView`, which is the
/// *install* picker's row: that screen decides between free slots and this
/// one decides between bodies, and `running` below is a fact about a
/// (routine, target) pair that the shared `Game::routine_holder_view` has no
/// routine to answer for. The glyph, name and level still come from that one
/// builder, so the two pickers cannot describe the same holder two ways.
pub struct FieldCastTargetView {
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
    /// The buff this cast would *overwrite*, if any — see
    /// `RunningBuffView`.
    pub running: Option<RunningBuffView>,
}

/// The routine-armed field buff a pending cast would replace on one target.
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
    /// stat name for a `CombatBuff` — that component carries no cast-time
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
    /// "You" for the player; a program's `CustomName` if it has one, else its
    /// zone-tagged species name (see `Game::zone_tagged_name`).
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
}

/// One worn item and the bonus it is *currently* granting.
///
/// `gear_level`/`fusion_tier` are the values captured on the `EquippedItem`
/// at equip time, and the stat fields are `EquipmentStats::scaled_for_level`
/// then `fused_for_tier` applied with exactly those — not a fresh preview at
/// today's zone level, which is what the inventory screen shows instead.
pub struct ManifestEquipSlot {
    /// `EquipmentSlot::label()` — "Weapon", "Armor", "Module".
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
}

/// One pile the base is holding, as the stock strip lists it — see
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
