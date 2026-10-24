use super::*;

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

/// How many copies of one part a purchase at a Stack market hands over —
/// see `Game::market_offers`. Prices are `tuning::STACK_MARKET_ROUTINE_PRICE_*`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoutineScope {
    One,
    Party,
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

    /// How many parts this rung hands over.
    pub fn parts(self) -> u32 {
        match self {
            RoutineScope::One => crate::tuning::STACK_MARKET_PARTS_ONE,
            RoutineScope::Party => crate::tuning::STACK_MARKET_PARTS_PARTY,
            RoutineScope::Everyone => crate::tuning::STACK_MARKET_PARTS_EVERYONE,
        }
    }

    /// How the rung reads on the shelf: the price and the quantity are both
    /// already on the row, so this says only what size of lot it is.
    pub fn label(self) -> &'static str {
        match self {
            RoutineScope::One => "a single",
            RoutineScope::Party => "a small lot",
            RoutineScope::Everyone => "a crate",
        }
    }
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
    /// Whether this slot's routine is welded in — `Game::
    /// routine_is_permanent`, resolved here so the screen can say so on the
    /// row rather than let the player find out by pressing it. False for a
    /// free slot.
    pub fixed: bool,
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
/// about to be broken down.
pub struct ExtractableRoutineView {
    pub ability: crate::abilities::AbilityId,
    pub name: String,
    pub description: String,
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
