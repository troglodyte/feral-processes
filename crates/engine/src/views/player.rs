use super::*;

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
    /// The ceiling `power` is read against: derived from Bandwidth, so the
    /// bar must never assume `POWER_MAX`. See `Game::max_power`.
    pub max_power: f32,
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
    /// Unspent attribute points, earned 6 per level gained and spent on the
    /// Points screen.
    pub stat_points: u32,
    /// Which perks have been unlocked so far.
    pub unlocked_perks: Vec<Perk>,
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
    /// What one wide swing of this weapon lands on, and how often — `None`
    /// for a copy declaring no reach, which is everything but a handful of
    /// weapons.
    ///
    /// **On the worn half and formatted in the engine.** Worn, because that
    /// block is by definition what only means something for a wearable copy,
    /// which is exactly what a reach is. Formatted here for `copy_name`'s
    /// reason and because a read-only screen's row count is owned by
    /// app-core: a per-row transform in the renderer opens the page on a row
    /// that is not drawn.
    ///
    /// A row rather than a term in `Game::copy_power`: the rating is
    /// absolute and prices the swing, so a reach weapon paid for with a weak
    /// band rates below its single-target peer and this is what the player
    /// reads instead.
    pub reach: Option<String>,
    /// How far this weapon may be swung from, as the inspect page says it,
    /// or `None` for one that swings at arm's length.
    ///
    /// Absent rather than "Range: 1 cell": every blade in the game would
    /// carry that row, and a row on everything is a row nobody reads.
    ///
    /// Formatted in the engine for `reach`'s reason — a read-only screen's
    /// row count is app-core's, and a per-row transform in the renderer
    /// opens the page on a row that is not drawn.
    pub range: Option<String>,
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

/// The program on the player's weapon line, for the equipped panel. The
/// bonus is `Game::wielded_stat_bonus`'s live figure rather than anything
/// stored, so the panel moves as the program levels or is fused.
pub struct WieldedView {
    pub name: String,
    pub level: u32,
    pub bonus: (i32, i32),
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

/// One status a body carries, for the battle roster and manifest: the
/// def's short tag plus what the renderer needs to say how deep it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusTagView {
    pub tag: String,
    pub stacks: u32,
    /// Battle rounds left.
    pub remaining: u32,
}

/// The player's chosen icon, off `components::PlayerIdentity` — carried on
/// `EntityView::look`, `Some` for the player alone. Kept off `EntityView`
/// itself, rather than two bare fields, so a frontend can match on
/// `Some`/`None` for "is this the player" the same way it already does for
/// `is_anchor`'s structure-only fields, instead of reading `sprite`/`colour`
/// on every other entity and getting an empty string and a 0 back.
#[derive(Clone, Debug)]
pub struct PlayerLook {
    pub sprite: String,
    /// 0-based index into the renderer's player swatches; `None` is the
    /// `PLAYER` role colour. See `components::PlayerIdentity::colour`.
    pub colour: Option<u8>,
    /// The player's drawn 16x16 avatar; `None` for a player who never
    /// opened the editor. See `components::PlayerIdentity::icon`.
    pub icon: Option<PlayerIcon>,
}

/// What an emulating player draws instead of their own `@` — `game::kit::
/// Game::form_look`'s one answer, carried on `EntityView::form` and
/// `tactical::TacticalBody::form` alike so the surface map and the battle
/// board cannot disagree about what an image looks like. `None` for every
/// body that is not emulating; only the player ever carries
/// `components::Emulation`, so this is `Some` for the player alone and
/// never for a hostile or a companion.
///
/// **Not `is_boss: true`** — that flag also feeds XP and con logic, so
/// reusing it would price an emulation as an apex spawn. The boss *styling*
/// (magenta ink, con read in the earmark) is drawn by reading `form.is_
/// some()` at the same two places `is_boss` already is, not by setting
/// `is_boss` itself.
#[derive(Clone, Debug, PartialEq)]
pub struct FormLook {
    pub glyph: char,
    /// `SpeciesDef::sprite_name`'s resolved name — always `Some`, the same
    /// "try it, fall back to the glyph" contract every other sprite name on
    /// `EntityView` carries, so `assets/sprites/` stays optional.
    pub sprite: Option<String>,
}

/// The battle screen's picture window — one body, resolved the way the map
/// would draw it, so the window and the map never show different looks.
/// See `Game::portrait_of`.
#[derive(Clone, Debug, PartialEq)]
pub struct PortraitView {
    pub sprite: Option<String>,
    pub glyph: char,
    pub color: GlyphColor,
    pub name: String,
    /// Try the player's drawn icon before `sprite` — set exactly when the
    /// map would draw it: the player, not emulating, with an icon drawn.
    pub drawn_icon: bool,
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

/// One attribute, as the dossier page draws it — the def's prose joined to
/// the body's own number.
///
/// Both names travel: `name` is this setting's word and `legacy` the
/// old-school one the row puts in parentheses after it, which is what lets
/// a player who has never read a line of code know what they are looking
/// at.
#[derive(Clone)]
pub struct AttributeRow {
    pub name: String,
    pub legacy: String,
    pub value: i32,
    pub short: String,
    pub meaning: String,
}

/// The player's class, as their own manifest reads it back — see
/// `Game::player_class_view`. Not a `ClassRow`: that carries the starting
/// kit, which the wizard's Kit step *replaces*, so quoting it on a sheet
/// read mid-run would name items the player never had.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerClassView {
    pub name: String,
    /// One `(axis label, multiplier)` per non-neutral axis,
    /// `classes::affinity_bonuses`.
    pub bonuses: Vec<(String, f32)>,
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

/// What `Mode::RigTool` draws — `DepotFilterView`'s shape, including its
/// self-closing rule: `Game::rig_tool_view` answers `None` once `rig` stops
/// being a standing rig the player is beside, so a machine demolished or
/// walked away from under the screen closes it rather than drawing a stale
/// row.
///
/// `tile` is base space and it is the header, `DepotFilterView`'s own
/// reason: a base may stand two rigs against one cell and the player has to
/// be told which one they are fitting.
///
/// `installed` is the tool the rig strips with, and `built_in` says it is
/// the starter running on a rig nobody has fitted (`Game::rig_tool`) — no
/// carrier stands behind it, so there is nothing to pull.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RigToolView {
    pub tile: (i32, i32),
    pub name: String,
    pub installed: Option<RigToolRow>,
    pub candidates: Vec<RigToolRow>,
    pub built_in: bool,
}

/// One tool as the rig screen names it — `ToolRow` without the player's
/// slot, since a rig holds exactly one and has no ladder to place it on.
///
/// `ticks` is a *call* into `Game::extraction_ticks` rather than the def's
/// raw figure, `BuildOrderRow`'s rule: the screen quotes what a strip will
/// actually cost at this base's bench tier, so a number read here and a
/// number paid at the rig cannot differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RigToolRow {
    pub id: crate::tools::ToolId,
    pub name: String,
    pub category: crate::tools::ToolCategory,
    pub tier: u32,
    pub ticks: u64,
    pub carriers_held: u32,
}

/// The four duel figures against a typical program of `zone`, each a
/// `(before, after)` pair. Built by `game::level_up::duel_comparison`, the
/// one place the arithmetic lives, and shared by the level-up page, the
/// Points screen and the perk preview.
#[derive(Clone, Debug, PartialEq)]
pub struct DuelComparison {
    pub zone: u32,
    pub hit_chance: (f64, f64),
    pub per_swing: (f64, f64),
    pub swings_to_win: (u32, u32),
    pub swings_to_down_you: (u32, u32),
}

/// What one more level of a perk changes: the stat rows that moved and the
/// duel, both from a trial that is rolled back (`Game::preview_perk`).
#[derive(Clone, Debug, PartialEq)]
pub struct PerkPreview {
    pub stats: Vec<StatRow>,
    pub duel: DuelComparison,
}

/// A purchased perk read back: what it is, the level now held, and what it
/// changed (`Game::buy_perk`).
#[derive(Clone, Debug, PartialEq)]
pub struct PerkReport {
    pub name: String,
    pub level: u32,
    pub description: String,
    pub preview: PerkPreview,
}

/// The level-up summary page, read back from `Game::take_level_up_report`.
/// Several levels gained before the page shows are **one page**:
/// `from_level` is the level the player held before the first of them, and
/// `to_level` is where they stand now.
///
/// Every pair is `(before, after)`. `stats` is `StatRow`'s own shape — the
/// log's own labels and arrow format — with a row already dropped when it
/// did not move, so a level that moved no stat ships an empty list rather
/// than a page of unchanged rows.
///
/// The duel figures are measured against `duel.zone`'s typical foe (see
/// `Game::typical_foe`), which does not itself change between the two
/// columns — only the player does. `perk_points_gained` and
/// `stat_points_gained` are `current − snapshot`, not
/// `PERK_POINTS_PER_LEVEL * levels`, so an overflow point earned in the
/// same award counts too. A level-up grows no stats on its own: the
/// banked stat points are what it earned, spent on the Points screen.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelUpReport {
    pub from_level: u32,
    pub to_level: u32,
    pub stats: Vec<StatRow>,
    pub duel: DuelComparison,
    pub perk_points_gained: u32,
    pub perk_points_unspent: u32,
    pub stat_points_gained: u32,
    pub stat_points_unspent: u32,
}

/// One implant the player has built in — see `Game::implant_view`.
#[derive(Clone, PartialEq, Debug)]
pub struct InstalledImplantRow {
    pub id: crate::implants::ImplantId,
    /// The def's name, or the raw id when the def is gone (`known` false).
    pub name: String,
    /// False for an id whose def has gone missing: it contributes nothing,
    /// costs nothing to remove and returns nothing.
    pub known: bool,
    pub description: String,
    pub load: u32,
    /// Extra Power per tick this implant adds, before perks.
    pub upkeep: f32,
    pub downside: Option<String>,
    /// `core_fragment`s taking it out costs.
    pub removal_fragments: u32,
}

/// One implant item in the pack, ready to install.
#[derive(Clone, PartialEq, Debug)]
pub struct InstallableImplantRow {
    pub item: ItemId,
    pub name: String,
    pub description: String,
    pub load: u32,
    pub upkeep: f32,
    pub downside: Option<String>,
    pub count: u32,
}

/// The Splice Rig screen's whole picture of the player's implants.
#[derive(Clone, PartialEq, Debug)]
pub struct ImplantView {
    pub load: u32,
    pub cap: u32,
    /// Load past the cap, or 0.
    pub overload: u32,
    /// `core_fragment`s in the pack, what a removal is paid from.
    pub fragments: u32,
    pub installed: Vec<InstalledImplantRow>,
    pub installable: Vec<InstallableImplantRow>,
}

/// One held Phase Key as the inventory's Phase Keys tab shows it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HeldPhaseKey {
    pub name: String,
    pub flavour: String,
    /// One line, from `KeyEffect::summary`; empty for a key with no effect.
    pub effect: String,
}

/// A Phase Key slot: the zone it belongs to, and the key if recovered.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PhaseKeySlot {
    pub zone: u32,
    pub held: Option<HeldPhaseKey>,
}

/// The Phase Keys tab's whole picture. `slots` always has
/// `tuning::PHASE_KEY_COUNT` rows in zone order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PhaseKeysView {
    pub slots: Vec<PhaseKeySlot>,
    pub held_count: u32,
    pub story_complete: bool,
}
