use super::*;

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

/// What one row of a Stack market's shelf is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MarketOfferKind {
    /// A bundle of one Subroutine part — `scope.parts()` of them, for
    /// building routines at the Workbench. See `Game::buy_market_offer`.
    Part {
        part: crate::routine_craft::Subroutine,
        scope: RoutineScope,
    },
    /// Every part of one exclusive routine — the rare row. Deliberately a
    /// kind of its own rather than a `Part` at a fourth scope: it is sold
    /// whole and priced off `STACK_MARKET_EXCLUSIVE_PRICE` rather than off
    /// any rung, and rolling it into `RoutineScope` would mean a rung whose
    /// `parts()` and `price()` both lie.
    ExclusiveRoutine { ability: String },
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
    /// One Subroutine part, for building routines at the Workbench.
    Part(crate::routine_craft::Subroutine),
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
    /// your battle party", "stops working the Extractor". Empty when the
    /// program is idle. The same facts as `activity`, worded as consequences
    /// — the confirmation screen warns, the row informs.
    pub detaches: Vec<String>,
}

/// The whole of a settlement's hub screen — `Game::settlement_report`.
///
/// Identity only, Phase 2's decision: no action rows and no stubs for a
/// market or a job board, both of which land as their own fields later
/// rather than widening this one under a name that stops matching what it
/// holds.
///
/// Every label is a call onto the resolved def's own enum —
/// `specialty.label()`, `temperament.label()` — rather than a `match`
/// re-stated here, which is what keeps a new catalogue variant's label
/// living in exactly one place. `kind` is the same call one door further
/// out: `Game::settlement_kind().label()`, never `def.kind`, because the
/// effective kind is the authored one folded with the run's growth latch
/// — see `game/settlement_growth.rs`' module doc.
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
    /// How this city is doing — `growth::Vitality::label()`, on
    /// `kind`/`specialty`/`temperament`'s precedent of calling onto the
    /// band's own enum rather than wording it here.
    ///
    /// **`None` for a Server**, which has no band: a word that never
    /// changes is worse than no word, and the row is dropped entirely
    /// rather than drawn saying "Steady" forever. See
    /// `Game::settlement_vitality`.
    pub vitality: Option<&'static str>,
    /// One sentence per aid this town currently offers, already in the
    /// player's words — see `Game::settlement_aid_lines`. Empty when it
    /// offers none, and an empty list draws no rows and no header.
    pub aid: Vec<String>,
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
    /// **How and where it is satisfied**, or `None` where the objective is its
    /// own instruction — see `Game::objective_hint`.
    ///
    /// Its own field rather than more words on `objective_line`, because that
    /// line has fourteen characters of headroom on the widest shipped row
    /// before it runs off `PopupSize::Large` and a town's name does not fit in
    /// them. Wrapped by the renderer the way `description` is.
    ///
    /// Live for a held contract and static for an offer: standing at the
    /// counter is what turns "carry them to your Broker" into "you are
    /// carrying 6, pick this row", and on an offer picking the row *signs*
    /// rather than delivers.
    pub hint: Option<String>,
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
