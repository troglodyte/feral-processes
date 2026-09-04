use bevy_ecs::prelude::{Component, Entity};
use serde::{Deserialize, Serialize};

use crate::MAX_CUSTOM_NAME_LEN;
use crate::abilities::AbilityId;
use crate::classes::PlayerClass;
use crate::icon::PlayerIcon;
use crate::items::{EquipmentSlot, GearCopy, ItemId};
use crate::items_db::ItemDb;
use crate::needs::{NEED_MAX, NEED_MIN, NeedId};
use crate::perks::Perk;
use crate::species::SpeciesId;
use crate::structures::StructureId;
use crate::tuning::{
    GOLD_STAT_MULT, MAX_INDIVIDUAL_ROLL, MIN_INDIVIDUAL_ROLL, PLATINUM_STAT_MULT,
    PRISMATIC_STAT_MULT, SILVER_STAT_MULT,
};

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlyphColor {
    White,
    Gray,
    Green,
    DarkGreen,
    Red,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Brown,
    Orange,
}

impl GlyphColor {
    /// Every authored hue, for a census that must cover all of them —
    /// `Rarity::ALL`'s reason. A renderer's colour table is an exhaustive
    /// match and so cannot miss a new hue; a test naming the hues by hand
    /// can, and silently stops covering the one just added.
    pub const ALL: [GlyphColor; 11] = [
        GlyphColor::White,
        GlyphColor::Gray,
        GlyphColor::Green,
        GlyphColor::DarkGreen,
        GlyphColor::Red,
        GlyphColor::Yellow,
        GlyphColor::Blue,
        GlyphColor::Magenta,
        GlyphColor::Cyan,
        GlyphColor::Brown,
        GlyphColor::Orange,
    ];
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Glyph {
    pub ch: char,
    pub color: GlyphColor,
}

/// Marks the single player-controlled entity.
#[derive(Component)]
pub struct Player;

#[derive(Component, Clone)]
pub struct Creature {
    pub species: SpeciesId,
}

/// A display name that overrides a creature's species name wherever it's
/// shown. Two authors, not one: the player, through `Game::fuse_companions`
/// and `Game::rename_companion`, and — since the nemesis feature —
/// `Game::mark_nemeses`, which writes a bank-derived name (`nemesis::
/// NemesisDb`) to a hostile on its first grudge only, never again
/// (`components::Nemesis`). A nemesis you later decompile joins the roster
/// still wearing the name it earned, and `rename_companion` can overwrite
/// it afterward like any other name, because at that point it is yours.
/// The constructor enforces the length; the tuple field does not, so build
/// one through `sanitize` rather than wrapping a raw string — every writer,
/// bank-derived names included, goes through it.
#[derive(Component, Clone, Debug)]
pub struct CustomName(pub String);

impl CustomName {
    /// What the player typed, reduced to what may actually be stored:
    /// trimmed, truncated to `MAX_CUSTOM_NAME_LEN`, and `None` if nothing
    /// is left.
    ///
    /// **`None` means "no override", and both callers want that** — it is
    /// why this returns an `Option` rather than refusing empty input.
    /// Fusion reads it as "insert no `CustomName`" and a rename reads it as
    /// "remove the one that's there", and those land on the same place: the
    /// species name. So blank is the way back to the default, in both.
    pub fn sanitize(input: Option<String>) -> Option<String> {
        let trimmed = input?
            .trim()
            .chars()
            .take(MAX_CUSTOM_NAME_LEN)
            .collect::<String>();
        (!trimmed.is_empty()).then_some(trimmed)
    }
}

/// The player's chosen class and look, from character-creation's
/// `game::creation::CharacterChoice` — the player-only counterpart to
/// `Creature::species`. `class` feeds the player arm of
/// `Game::ability_affinity`; `sprite` and `colour` do not live on `Glyph`
/// because `GlyphColor` is the eleven-hue *content* palette and the
/// player's own choices are deliberately outside it. They instead ride out
/// to the renderer on `views::PlayerLook`, carried on `EntityView::look`.
///
/// Spawned at its `Default` (no class, no sprite, no colour, no icon) by
/// every constructor and immediately overwritten by
/// `Game::apply_character_choice` — the same "neutral bundle, layered on
/// top" shape `Game::new_with`'s player spawn uses throughout.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct PlayerIdentity {
    pub class: Option<PlayerClass>,
    pub sprite: String,
    /// Which of the renderer's player swatches the glyph wears, **0-based**
    /// — `None` is the `PLAYER` role colour, which is what a player who
    /// never opened the wizard and every save from before it carries.
    /// `Option` rather than a reserved zero for `PowerCell::Unrated`'s
    /// reason: *no answer* is not a bad answer, and a sentinel index shares
    /// one value between "the default" and "the first swatch".
    pub colour: Option<u8>,
    /// The player's hand-drawn 16x16 map avatar — `None` for a glyph, the
    /// same "no answer" reading `colour` already gives, and what every save
    /// and every character before this feature carries.
    pub icon: Option<PlayerIcon>,
}

/// Which zone portal's sector a creature was spawned in — set once at
/// spawn time and never changed afterward, even if the creature is later
/// tamed and carried through a portal into a deeper zone. Drives its stat
/// scale (see `ZoneLevel::stat_multiplier`) and is appended to its display
/// label (e.g. "Scrapper 2") so a deeper-zone catch reads differently from
/// a shallow one.
#[derive(Component, Clone, Copy, Debug)]
pub struct ZonePortal(pub u32);

#[derive(Component, Clone, Copy, Debug)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    /// Damage only. The to-hit roll comes from speed on both sides — see
    /// `battle::accuracy_of`. Feeding this to-hit as well would compound
    /// quadratically and move every `balance_sim` curve.
    pub atk: i32,
    /// **Percentage points**, not subtractive absorption. Innate plus
    /// whatever gear `Game::apply_equipment_delta` has baked in; buffs and
    /// the cap are applied on top by `Game::effective_mitigation`.
    ///
    /// **Never scaled by level or zone.** A percentage that grows per level
    /// approaches immunity, so `progression::stats_after_levels` and
    /// `ZoneLevel::stat_multiplier` both leave it alone. Levelling buys HP,
    /// attack, accuracy and evasion; mitigation comes from gear and from
    /// what a species innately is. This is the rule that keeps the
    /// percentage form safe, and the one most likely to be "corrected" by
    /// someone restoring symmetry with the other stats.
    pub mitigation: i32,
}

impl Stats {
    pub fn hp_fraction(&self) -> f32 {
        if self.max_hp <= 0 {
            0.0
        } else {
            (self.hp as f32 / self.max_hp as f32).clamp(0.0, 1.0)
        }
    }

    /// A rough "how strong is this" scalar — effective HP plus attack.
    ///
    /// Used to gauge relative difficulty (`Game::difficulty_color`), to
    /// price a kill's XP (`progression::kill_xp`'s denominator), and by
    /// trade valuation and the unlock ratios. Summing a *percentage* into a
    /// total the way the old `max_hp + atk + def` did is meaningless, so
    /// mitigation is priced as the soak it actually buys:
    /// `max_hp / (1 - mitigation/100)`.
    ///
    /// The clamp to `MAX_MITIGATION_PERCENT` is load-bearing — it is what
    /// keeps the denominator away from zero on a value that a save, a mod
    /// affix or a stacked buff could hand in past the cap.
    pub fn power(&self) -> i32 {
        let mitigation = self
            .mitigation
            .clamp(0, crate::tuning::MAX_MITIGATION_PERCENT);
        let soak = self.max_hp as f64 / (1.0 - mitigation as f64 / 100.0);
        soak.round() as i32 + self.atk
    }
}

/// The full end of a reserve's range. Lives here beside `PowerReserve`
/// rather than in `tuning.rs`, because it is the type's own documented
/// invariant rather than a difficulty knob.
pub const POWER_MAX: f32 = 100.0;
/// The empty end. Below it Power is meaningless, and its readers (the status
/// bars, `battle::power_attack_multiplier`) assume it holds.
pub const POWER_MIN: f32 = 0.0;

/// What a combatant has left to spend on routine calls, `POWER_MIN..=
/// POWER_MAX`. One meter, since the Fatigue half was deleted — that one
/// refilled on its own and was spent by two routines, while Power is the only
/// thing that kills by attrition and the budget every routine call draws on.
///
/// **The float is private, and that is the point of the type.** The range was
/// an invariant held by convention across a dozen sites, each hand-rolling
/// its own `.max(POWER_MIN)` or `.min(POWER_MAX)` — one forgotten clamp and a
/// reserve reads negative or overfull to `power_attack_multiplier` and every
/// status bar. A private field makes the compiler hold it instead, the same
/// move as `Game`'s private `world`.
///
/// The API is exactly the operations the call sites perform and nothing
/// speculative. A caller wanting an eighth is a signal to re-read the call
/// site, not to widen the type.
#[derive(Component, Clone, Copy, Debug)]
pub struct PowerReserve(f32);

impl Default for PowerReserve {
    fn default() -> Self {
        Self(POWER_MAX)
    }
}

impl PowerReserve {
    /// Clamps, because both callers are load paths: a save file and a mod's
    /// numbers are equally outside this crate's control.
    pub fn new(value: f32) -> Self {
        Self(value.clamp(POWER_MIN, POWER_MAX))
    }

    pub fn get(&self) -> f32 {
        self.0
    }

    /// Whether `cost` is affordable. `>=`, so a reserve holding exactly the
    /// cost may spend it — the refusal is for a reserve that would go
    /// negative, not one that lands on empty.
    pub fn holds(&self, cost: f32) -> bool {
        self.0 >= cost
    }

    pub fn spend(&mut self, cost: f32) {
        self.0 = (self.0 - cost).max(POWER_MIN);
    }

    pub fn restore(&mut self, amount: f32) {
        self.0 = (self.0 + amount).min(POWER_MAX);
    }

    /// `Game::rest`, which sets outright rather than adding.
    pub fn fill(&mut self) {
        self.0 = POWER_MAX;
    }

    /// `difficulty.rs`'s Forgiving reboot — the one site that raises *to* a
    /// floor rather than adding. Never lowers: a reboot is meant to leave you
    /// with enough to keep going, and a player who died holding more than the
    /// floor does not get docked for it. Delete this if that call ever
    /// becomes an additive top-up.
    pub fn raise_to_at_least(&mut self, floor: f32) {
        self.0 = self.0.max(floor).min(POWER_MAX);
    }
}

#[cfg(test)]
mod power_reserve_tests {
    use super::*;

    #[test]
    fn spend_floors_at_empty_rather_than_going_negative() {
        let mut r = PowerReserve::new(5.0);
        r.spend(50.0);
        assert_eq!(r.get(), POWER_MIN);
    }

    #[test]
    fn restore_caps_at_full() {
        let mut r = PowerReserve::new(POWER_MAX - 1.0);
        r.restore(50.0);
        assert_eq!(r.get(), POWER_MAX);
    }

    #[test]
    fn new_clamps_a_wild_input_at_both_ends() {
        assert_eq!(PowerReserve::new(-40.0).get(), POWER_MIN);
        assert_eq!(PowerReserve::new(4000.0).get(), POWER_MAX);
    }

    /// The boundary is where a refusal and a charge would disagree if they
    /// were written twice, so it is pinned on both sides of exact.
    #[test]
    fn holds_is_true_at_exactly_the_cost_and_false_one_short() {
        let r = PowerReserve::new(10.0);
        assert!(r.holds(10.0), "a reserve holding exactly the cost may pay");
        assert!(!r.holds(10.1));
        assert!(r.holds(9.9));
    }

    /// The bug the Forgiving reboot would otherwise ship: a player who died
    /// with a full reserve being *dropped* to the floor by the thing meant to
    /// help them.
    #[test]
    fn raise_to_at_least_never_lowers_a_reserve_already_above_the_floor() {
        let mut r = PowerReserve::new(90.0);
        r.raise_to_at_least(40.0);
        assert_eq!(r.get(), 90.0);

        let mut drained = PowerReserve::new(5.0);
        drained.raise_to_at_least(40.0);
        assert_eq!(drained.get(), 40.0);
    }

    #[test]
    fn fill_sets_outright() {
        let mut r = PowerReserve::new(1.0);
        r.fill();
        assert_eq!(r.get(), POWER_MAX);
    }
}

/// A wild creature that will fight rather than flee when engaged.
#[derive(Component)]
pub struct Hostile;

/// Tracks level/XP for the player and any tamed creature. Wild (untamed)
/// creatures don't carry this — they don't level until compiled.
#[derive(Component, Clone, Copy, Debug)]
pub struct Experience {
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            level: 1,
            xp: 0,
            // Called, not copied. This was a literal 20 that silently agreed
            // with `XP_PER_LEVEL_STEP` until the step moved, at which point
            // every entity in the game — the player included — would have
            // bought its first level at a quarter price.
            xp_to_next: crate::progression::xp_for_level(1),
        }
    }
}

#[derive(Component, Default)]
pub struct WanderAi {
    pub cooldown: u32,
}

/// Player-only skill at cracking a program's ICE — raises decompile odds
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

/// An item sitting in an `Equipment` slot: *which copy* went on, and the
/// gear level its stat bonus was scaled for when it did.
///
/// The level is captured at equip time (see
/// `items::EquipmentStats::scaled_for_level`) — like a wild program's
/// zone-scaled stats, it doesn't retroactively change if the player breaches
/// deeper afterward; re-equip to pick up a newly unlocked level. That is
/// exactly why it sits *beside* `GearCopy` rather than inside it: the level
/// is a property of the moment, not of the copy, and a copy back in cargo
/// has no level at all.
///
/// **Storing the copy whole is what keeps the bonus symmetric.**
/// `Game::apply_equipment_delta` writes the bonus into `Stats`, so the
/// unequip has to subtract precisely what the equip added; every property
/// that scales it lives in this one value, so a path cannot subtract a
/// bonus computed from fewer properties than it added. See `GearCopy`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquippedItem {
    pub copy: GearCopy,
    pub level: u32,
}

/// What's currently equipped in each slot, on the player or on any program
/// they own — `Game::check_wearer` is the rule. Each slot's level-scaled
/// stat bonus (see `EquippedItem`, `Game::equipment_of`) is added directly
/// onto `Stats`/`Decompiler` when equipped and subtracted back on unequip —
/// mirroring how leveling directly mutates `Stats` elsewhere, rather than
/// maintaining a separate "base stats" layer.
///
/// Inserted on demand by `Game::equip` rather than at every spawn site: its
/// absence already reads as an empty loadout everywhere it is consulted, so
/// a program grows one only when it first wears something. Because the bonus
/// lives in `Stats`, **no stats operation may run while it is there** — see
/// `Game::gear_bonus` and `Game::strip_gear`.
#[derive(Component, Default, Clone)]
pub struct Equipment {
    pub weapon: Option<EquippedItem>,
    pub armor: Option<EquippedItem>,
    pub module: Option<EquippedItem>,
}

impl Equipment {
    pub fn slot_mut(&mut self, slot: EquipmentSlot) -> &mut Option<EquippedItem> {
        match slot {
            EquipmentSlot::Weapon => &mut self.weapon,
            EquipmentSlot::Armor => &mut self.armor,
            EquipmentSlot::Module => &mut self.module,
        }
    }

    pub fn get(&self, slot: EquipmentSlot) -> Option<EquippedItem> {
        match slot {
            EquipmentSlot::Weapon => self.weapon.clone(),
            EquipmentSlot::Armor => self.armor.clone(),
            EquipmentSlot::Module => self.module.clone(),
        }
    }
}

#[derive(Component, Default, Clone)]
pub struct Inventory {
    pub items: Vec<(ItemId, u32)>,
}

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

/// Player-only: every carried copy of gear that is *not* interchangeable
/// with a plain one, a row per `(GearCopy, qty)`. A copy earns a place here
/// by having been fused (`Game::fuse_item`) or by having dropped at a rare
/// tier (`Game::grant_gear_drop`) — `GearCopy::is_plain` is the predicate,
/// and it is the only thing that decides.
///
/// **`Inventory` is by definition the plain-copy store**, and that is the
/// seam that keeps this out of the production chain entirely: recipes,
/// `Stock`, `assembler_system`, hauling and banking read `Inventory` and
/// therefore cannot encounter a special copy, so none of them needs a tier
/// or rarity rule. That was true of fusion alone and stays true now that a
/// copy has two ways to be special — which is the whole reason rarity was
/// added as a property of the *copy* rather than of the item.
///
/// Keyed by value rather than by position. Two copies with equal
/// `GearCopy`s are genuinely interchangeable, so an index would identify
/// nothing the key does not — and it would be the positional-index trap
/// `BattleState::planned` documents, for no gain.
///
/// `EquippedItem` holds the same key for a copy that is worn rather than
/// carried; this ledger agrees with it rather than shadowing it, which is
/// what the per-`ItemId` predecessor got wrong.
#[derive(Component, Default, Clone)]
pub struct GearCopies {
    pub copies: Vec<(GearCopy, u32)>,
}

impl GearCopies {
    pub fn add(&mut self, copy: GearCopy, qty: u32) {
        // Saturating for the reason `Inventory::add` is.
        if let Some(row) = self.copies.iter_mut().find(|(c, _)| *c == copy) {
            row.1 = row.1.saturating_add(qty);
        } else {
            self.copies.push((copy, qty));
        }
    }

    pub fn count(&self, copy: &GearCopy) -> u32 {
        self.copies
            .iter()
            .find(|(c, _)| c == copy)
            .map(|(_, q)| *q)
            .unwrap_or(0)
    }

    /// Removes up to `qty` of `copy`, returning how many were actually
    /// removed. Drops the row at zero for the reason `Inventory::take`
    /// does — every screen lists these rows.
    pub fn take(&mut self, copy: &GearCopy, qty: u32) -> u32 {
        let Some(pos) = self.copies.iter().position(|(c, _)| c == copy) else {
            return 0;
        };
        let taken = self.copies[pos].1.min(qty);
        self.copies[pos].1 -= taken;
        if self.copies[pos].1 == 0 {
            self.copies.remove(pos);
        }
        taken
    }

    /// Every special copy held, for the cargo total. These are carried like
    /// anything else, so they count against the Buffer figure the player
    /// reads.
    pub fn total(&self) -> u32 {
        self.copies.iter().map(|(_, qty)| *qty).sum()
    }
}

impl Inventory {
    pub fn add(&mut self, item: ItemId, qty: u32) {
        // Saturating so an unbounded Buffer can never wrap a stack's count.
        if let Some(slot) = self.items.iter_mut().find(|(i, _)| *i == item) {
            slot.1 = slot.1.saturating_add(qty);
        } else {
            self.items.push((item, qty));
        }
    }

    pub fn count(&self, item: &ItemId) -> u32 {
        self.items
            .iter()
            .find(|(i, _)| i == item)
            .map(|(_, q)| *q)
            .unwrap_or(0)
    }

    /// Total units of ordinary cargo held. Banked currencies (an item with
    /// `ItemDef::banked` set) are excluded — this is just how full the
    /// (unbounded) Buffer is, shown to the player.
    pub fn cargo_used(&self, db: &ItemDb) -> u32 {
        self.items
            .iter()
            .filter(|(item, _)| !db.get(item.as_str()).is_some_and(|d| d.banked))
            .map(|(_, qty)| *qty)
            .sum()
    }

    /// Removes up to `qty` of `item`, returning how many were actually
    /// removed. Drops the slot entirely once it hits zero, rather than
    /// leaving a `(item, 0)` behind — callers that list `items` (the status
    /// panel, the inventory screen) shouldn't have to filter zero-quantity
    /// stacks themselves.
    pub fn take(&mut self, item: ItemId, qty: u32) -> u32 {
        let Some(pos) = self.items.iter().position(|(i, _)| *i == item) else {
            return 0;
        };
        let taken = self.items[pos].1.min(qty);
        self.items[pos].1 -= taken;
        if self.items[pos].1 == 0 {
            self.items.remove(pos);
        }
        taken
    }
}

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
    /// Mirrors `WorkDef::level`. `None` means a completed gather cycle
    /// always yields, same as before this field existed. `Some(level)` gates
    /// each completion behind a level-based percentage chance instead (see
    /// `systems::task_progress_system`) — a harder, chancier variant that a
    /// structure opts into via its `.ron` file rather than something every
    /// worked node does by default.
    pub level: Option<u32>,
}

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
}

/// A load a posted program is physically carrying to a depot.
///
/// The *only* stored hauling state, and deliberately so. Where the worker is
/// headed (the nearest depot while carrying, its own machine otherwise) and
/// whether it has arrived (`game::base::hauling::at_station`) are both derived
/// from `Position`, so there is one source of truth and no state field that
/// can desync into a worker standing at its machine insisting it is still
/// walking.
///
/// One `(item, qty)` pair rather than a map because
/// `tuning::HAUL_CARRY_CAPACITY` bounds a trip: `Stock::output` is a
/// `BTreeMap` and may hold several item ids, so an uncapped drain would have
/// had to carry all of them, and the save with it.
#[derive(Component, Clone, Debug)]
pub struct Carrying {
    pub item: ItemId,
    pub qty: u32,
}

/// A posted program that found no route to where it is trying to get, and the
/// tick it stopped being able to. Written and cleared by
/// `game::base::hauling::haul_step_system`, the one system that walks a field
/// and so the only one that can know.
///
/// **Written on entry only, so `since` is the start of the episode rather
/// than the last tick of it.** That is what makes a stranding an *event* a
/// `&mut Game` pass can find — `Game::note_strandings` reads `since == now`
/// to remember the ones that began this tick — and it is
/// `systems::set_machine_status`'s rule in another shape: entering a state is
/// news, staying in it is not. Re-inserting the marker every tick would leave
/// nothing to distinguish a route that has just broken from one that has been
/// broken for an hour.
///
/// A cache of that tick's answer rather than stored state, and not saved for
/// the same reason `MachineStatus` isn't: the walk that produced it runs
/// again on the next tick. It exists at all because the status it drives is
/// written by `systems::task_progress_system` — giving `MachineStatus` two
/// writers would have them ping-pong `Unstaffed`↔`Stranded` every tick, and
/// `set_machine_status` logs on every transition.
///
/// `task_progress_system` runs *first* in the chain, so it reads a marker
/// written on the previous tick. That one-tick lag on a status label is the
/// price of leaving the chain order alone, which is load-bearing for the
/// clog/pickup handoff (see `Game::build_schedule`).
#[derive(Component, Clone, Copy, Debug)]
pub struct Stranded {
    /// The `GameClock` tick the worker entered the state on.
    pub since: u64,
}

/// Why a machine is or isn't producing. Present on a structure that runs a
/// job (`StructureDef::work` or `::assembles`) and on one that burns to keep
/// supplying (`StructureDef::power_upkeep`) — absence means "nothing here
/// can stall", which is why a Home never reports a status it could not
/// possibly leave.
///
/// Deliberately not saved. It initialises to `Running` and is corrected on
/// the first tick, so a base that loads starved announces it once — which is
/// information the player wants, and costs no save field.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    /// A sixth variant is allowed here where `views.rs::output_stranded`
    /// refused one, and the difference is worth keeping straight: a base-wide
    /// shortfall is *not* what this says. Under the `(x, y)` cut order one
    /// machine runs while its neighbour two tiles over is dark, so which
    /// machine lost the cut really is that machine's own state — which is the
    /// test that enum is held to.
    ///
    /// Written by `systems::idle_machine_system` and by nothing else;
    /// `task_progress_system`, `assembler_system` and `player_gather_system`
    /// only guard on the same fact via `resources::PowerGrid`, and write no
    /// status of their own.
    Unpowered,
}

impl MachineStatus {
    /// Every status, for `GlyphColor::ALL`'s reason: the renderer's colour
    /// table is exhaustive, but the census holding the palette's reserved
    /// colours off machine states is not, unless it walks this.
    pub const ALL: [MachineStatus; 7] = [
        MachineStatus::Running,
        MachineStatus::Starved,
        MachineStatus::Clogged,
        MachineStatus::Unstaffed,
        MachineStatus::Stranded,
        MachineStatus::Idle,
        MachineStatus::Unpowered,
    ];

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
        }
    }
}

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
    /// never-free-a-`Carrying`-holder rule already covers that, and it has
    /// to: freeing the body drops the `Carrying` with the `Task`, and those
    /// units have already left the shelf they came off.
    Construct,
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

/// A status condition a battle `MoveDef::effect` can inflict on a combatant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusKind {
    /// Deals `ActiveStatus::power` damage at the end of every round it's
    /// active.
    Bleed,
    /// Causes the afflicted side to lose their next action in battle.
    Stun,
    /// Cuts the afflicted side's Evasion by `EXPOSED_EVASION_PERCENT` — see
    /// `Game::combatant_profile`. Armed by the first rung of the fumble
    /// ladder, and **free for content**: `MoveEffect` already lets any
    /// species move inflict a status from `.ron`, so a debuffer species
    /// costs no Rust the day this exists.
    ///
    /// It belongs in `StatusEffects` — conditions inflicted on you, always
    /// unwanted — rather than in `CombatBuff`, which holds one *wanted* buff
    /// at a time.
    Exposed,
}

impl StatusKind {
    /// How the condition reads on a screen. A taxonomy label rather than
    /// authored content — the same call `Rarity::label` makes — so a
    /// routine's inspect line and the log that announces the condition
    /// cannot come to call it two different things.
    pub fn label(self) -> &'static str {
        match self {
            StatusKind::Bleed => "Bleed",
            StatusKind::Stun => "Stun",
            StatusKind::Exposed => "Exposed",
        }
    }
}

/// One combatant's currently active status condition, and how long it has
/// left.
#[derive(Clone, Copy, Debug)]
pub struct ActiveStatus {
    pub kind: StatusKind,
    /// Battle rounds remaining, ticked down at the end of every round bar
    /// the one it landed in — see `landed_this_round`.
    pub remaining: u32,
    /// Bleed damage dealt per round; unused for `Stun`.
    pub power: i32,
    /// True from being armed until the first `Game::tick_status_effects`
    /// after it, which spends itself clearing this flag and does nothing
    /// else — the round a condition lands in is not one of the rounds it
    /// lasts.
    ///
    /// Without it, end-of-round upkeep charged a round to a condition
    /// applied moments earlier in that same round: a `duration: 1` stun
    /// (every stun the shipped roster carries) expired before its victim's
    /// next turn, costing them nothing unless the attacker also happened to
    /// out-roll them on initiative, and `memory_leak`'s advertised "3
    /// rounds" of bleed dealt its first tick instantly and showed two.
    ///
    /// `Game::arm_status` is the only thing that sets it, which is why that
    /// is the only way to write `StatusEffects::active`.
    pub landed_this_round: bool,
}

/// A creature or the player can carry at most one status condition at a
/// time — a fresh application overwrites whatever was active, mirroring a
/// classic single-status-condition model rather than a stacking one.
/// Scoped to a single intrusion: cleared whenever a battle ends, however it
/// ends (kill, tame, flee, or the player going down).
#[derive(Component, Default, Clone, Copy)]
pub struct StatusEffects {
    pub active: Option<ActiveStatus>,
}

/// Which stat a companion's rally/shield temporarily boosts — see
/// `CombatBuff`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuffKind {
    Atk,
    /// Percentage points, the unit `Stats::mitigation` carries — not the
    /// subtractive absorption `Def` named before the combat model changed.
    Mitigation,
}

impl BuffKind {
    /// The stat this raises, as a screen names it. `StatusKind::label`'s
    /// twin, and the word the combat log already uses.
    pub fn label(self) -> &'static str {
        match self {
            BuffKind::Atk => "Attack",
            BuffKind::Mitigation => "Mitigation",
        }
    }
}

/// One active combat buff, and how long it has left.
#[derive(Clone, Copy, Debug)]
pub struct ActiveBuff {
    pub kind: BuffKind,
    /// Battle rounds remaining, ticked down at the end of every round.
    pub remaining: u32,
    pub power: i32,
}

/// A temporary combat buff on any one combatant — the player from a
/// companion's Special or a pre-battle consumable, a companion from bracing
/// (see `Game::begin_defend`). Kept separate from `StatusEffects` because
/// that component is reserved for conditions a hostile move can inflict
/// (always unwanted), which shouldn't be clobbered by — or clobber — a buff.
///
/// Holds at most one buff at a time, so a fresh one overwrites whatever was
/// still active: a companion that braces gives up a Rally it was carrying,
/// which is a real cost of the choice.
///
/// Only the player is spawned holding this component; `begin_defend`
/// inserts it on demand for anyone else. Scoped to a single intrusion,
/// cleared with everything else when a battle ends.
#[derive(Component, Default, Clone, Copy)]
pub struct CombatBuff {
    pub active: Option<ActiveBuff>,
}

/// Which routine or item armed a `FieldBuff` entry. Drives the two
/// collision rules `Game::arm_field_buff` enforces — it is not shown to the
/// player, `ActiveFieldBuff::name` is.
///
/// **The order of these variants is part of the save format**, the same
/// constraint `Perk` documents (`perks.rs`): saves are bincode, which
/// serializes an enum by *name*, so a variant may be reordered freely and
/// renaming one is what breaks a save. See `FieldBuffKind` below.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuffSource {
    Consumable,
    Routine,
}

/// Where a field buff lands. See `FieldBuffKind::scope`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldScope {
    /// Lands on whatever `AbilityTarget` names, and travels with that
    /// creature into whatever battle it's in.
    Creature,
    /// Always lands on the player, regardless of who ran it.
    Run,
}

/// A buff a field routine or consumable can arm outside combat that keeps
/// running after the map turn it was run on — through any battle that
/// follows, unlike `CombatBuff` — and through a save.
///
/// A variant *name* is the save format; the order is not. That is the RON
/// encoding `SAVE_FORMAT_VERSION` 29 moved to — the claim these two enums
/// carried about positional bincode had been stale since. Renaming a variant
/// still breaks a save; reordering no longer does. `Def` was removed under
/// that rule at version 31, which is why it earned a bump.
///
/// There is deliberately no `Def` beside `Mitigation`. Once `Stats::def`
/// became percentage points there was no flat-defence axis left for a second
/// name to describe, and two names on one axis is what makes both unreadable
/// wherever they are summed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldBuffKind {
    Regen,
    Trickle,
    Atk,
    Mitigation,
    CaptureBoost,
    XpBoost,
    EncounterDamp,
    DropBoost,
}

impl FieldBuffKind {
    /// Which end of a battle party this kind lands on. `Creature`-scoped
    /// kinds land on whatever `AbilityTarget` names; `Run`-scoped kinds
    /// always land on the player, since they're pressure or economy
    /// knobs the whole run feels rather than a single combatant's stats.
    pub fn scope(self) -> FieldScope {
        use FieldBuffKind::*;
        match self {
            Regen | Atk | Mitigation => FieldScope::Creature,
            Trickle | CaptureBoost | XpBoost | EncounterDamp | DropBoost => FieldScope::Run,
        }
    }

    /// The affinity category `abilities::scaled_stat_power` scales this kind's
    /// authored magnitude against, the same call as
    /// `AbilityEffect::affinity_kind`. `None` for the four percentage-rate
    /// kinds: a rate isn't a magnitude in any of the five affinity
    /// categories, the same reasoning that gives `Cleanse`/`Decompile`
    /// `None` there.
    pub fn affinity_kind(self) -> Option<crate::abilities::AffinityKind> {
        use crate::abilities::AffinityKind;
        use FieldBuffKind::*;
        match self {
            Regen | Trickle => Some(AffinityKind::Heal),
            Atk | Mitigation => Some(AffinityKind::Buff),
            CaptureBoost | XpBoost | EncounterDamp | DropBoost => None,
        }
    }

    /// Whether `Game::run_field_routine` should run this kind's authored
    /// `power` through `abilities::scaled_stat_power` (level and affinity) or
    /// deliver it unchanged. The two point-amount kinds scale; the rest do
    /// not, for the reason `AbilityEffect::Drain`'s `heal_fraction` is
    /// excluded from `scaled_hp_power` too: a value that already carries its
    /// own ceiling doesn't need a second one stacked on top. A percentage is
    /// a property of the routine, not of how strong the invoker is — scaling
    /// one the way a flat point value scales would let an authored 10% cut
    /// land anywhere up to 140% off a high-level, high-affinity invoker, the
    /// exact ceiling-defeating outcome the cap on `Mitigation` exists to
    /// prevent.
    ///
    /// **`Trickle` is excluded by that same rule, and it is the difference
    /// between it and `Regen` that decides it.** Both restore a pool, but
    /// `Regen`'s ceiling is `max_hp`, which grows with level — so a scaled
    /// heal stays the same fraction of the bar. Power's ceiling is
    /// `POWER_MAX`, a fixed 100 forever. Scaled, an authored `power: 1` is 7 a
    /// turn at the level cap, which pins a full reserve for the buff's whole
    /// duration and makes the authored number untunable: the level term
    /// swamps whatever the file says. Unscaled, `power` means what it says
    /// at every level.
    pub fn scales_with_invoker(self) -> bool {
        use FieldBuffKind::*;
        match self {
            Regen | Atk => true,
            Trickle | Mitigation | CaptureBoost | XpBoost | EncounterDamp | DropBoost => false,
        }
    }

    /// Whether a **routine**-armed buff of this kind runs until the party
    /// rests instead of counting turns down. The fourth per-kind rule on
    /// this enum, beside `scope`, `affinity_kind` and `scales_with_invoker`,
    /// and read only through `ActiveFieldBuff::runs_until_rest` — which is
    /// where the `BuffSource::Routine` half of the predicate lives, and why
    /// this one is not called anywhere directly except there and the load
    /// check that refuses a `duration` it would ignore.
    ///
    /// **The two over-time kinds are the exceptions, and both directions of
    /// that are load-bearing.** `Regen` and `Trickle` are the only kinds
    /// with a per-tick effect (see `Game::apply_field_buff_tick`); the rest
    /// are read on demand and do nothing as a turn passes. So an until-rest
    /// `Regen` is unbounded healing and an until-rest `Trickle` is unbounded
    /// Power, which is the whole of the Stack's scarcity. They are also the
    /// only two that use `interval`, whose cadence is phased off
    /// `ActiveFieldBuff::remaining` — a counter an until-rest buff does not
    /// have. Excluding them here is what lets `Game::tick_field_buffs` leave
    /// its cadence filter alone.
    pub fn runs_until_rest(self) -> bool {
        use FieldBuffKind::*;
        match self {
            Regen | Trickle => false,
            Atk | Mitigation | CaptureBoost | XpBoost | EncounterDamp | DropBoost => true,
        }
    }

    /// The short tag a buff list shows next to a running entry, e.g.
    /// `"DEF+2"` or `"XP+15%"`. `power` is points for the four flat kinds
    /// and percentage points for the rest. `HP` rather than `INT` for
    /// `Regen`, matching the abbreviation the battle roster header already
    /// uses for Integrity (`render/battle.rs`) — Power had no established
    /// short form there, so `PWR` is new.
    ///
    /// `interval` only reaches the two over-time tags, and only shows when
    /// it is not 1: `HP+2/4t` reads as "2 every four turns", where the
    /// `/t` the other keeps already reads as "per turn". The flat and rate
    /// kinds have no per-tick effect at all (see `apply_field_buff_tick`), so
    /// a cadence on one of them would be describing something that does not
    /// happen.
    pub fn magnitude_label(self, power: i32, interval: u32) -> String {
        let every = if interval > 1 {
            interval.to_string()
        } else {
            String::new()
        };
        match self {
            FieldBuffKind::Regen => format!("HP+{power}/{every}t"),
            FieldBuffKind::Trickle => format!("PWR+{power}/{every}t"),
            FieldBuffKind::Atk => format!("ATK+{power}"),
            FieldBuffKind::Mitigation => format!("DMG-{power}%"),
            FieldBuffKind::CaptureBoost => format!("TAME+{power}%"),
            FieldBuffKind::XpBoost => format!("XP+{power}%"),
            FieldBuffKind::EncounterDamp => format!("ENC-{power}%"),
            FieldBuffKind::DropBoost => format!("DROP+{power}%"),
        }
    }
}

/// One running field buff and how long it has left.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActiveFieldBuff {
    pub kind: FieldBuffKind,
    /// Display name of the ability or item that armed it, captured at invocation.
    /// Stored rather than derived from `kind`: two different routines can
    /// arm the same kind, and the buff list has to tell them apart.
    pub name: String,
    pub power: i32,
    /// Ticks remaining. Turns (`Game::tick_inner`), not battle rounds like
    /// `ActiveBuff::remaining` — a field buff outlives any one battle.
    ///
    /// **Not a lifetime when `runs_until_rest` below says so.** Nothing
    /// decrements it for those and nothing reads it but the cadence filter,
    /// which no until-rest kind reaches. An old save's in-flight routine
    /// buff of such a kind therefore loads with whatever count it had and
    /// simply stops ageing, which is the new behaviour and needs no
    /// migration.
    pub remaining: u32,
    /// Turns between firings — see `AbilityEffect::FieldBuff::interval`, which
    /// is where the authored value comes from. Carried on the running buff
    /// rather than looked up from the def it came from, because `name` is a
    /// display string and there is nothing here to look a def up *by*.
    ///
    /// The cadence is phased off `remaining`: a buff fires whenever
    /// `remaining % interval == 0`. That costs no second counter and no
    /// second save field, at the price of the phase depending on the
    /// duration — a duration that is a multiple of its interval, as every
    /// shipped one is, fires on the first tick and every interval-th after.
    #[serde(default = "crate::abilities::every_turn")]
    pub interval: u32,
    pub source: BuffSource,
}

impl ActiveFieldBuff {
    /// Whether this buff has no turn count at all and runs until the party
    /// rests. **The one definition**, read by `Game::tick_field_buffs` (which
    /// skips ageing it), `drop_until_rest_buffs` below (which ends it) and
    /// `duration_label` (which says so on the row).
    ///
    /// **It reads `source` as well as `kind`, and that half is the load-bearing
    /// one.** `ItemEffect::prebattle_buff` arms this same struct from a
    /// one-shot consumable with its own authored `ticks` — `patch_routine`
    /// arms Mitigation for 120 of them. Keyed on the kind alone, that item
    /// would have gone permanent too, and since `field_buff_power_of` sums a
    /// `Consumable` and a `Routine` entry of one kind rather than picking
    /// between them, its 10% would have stacked under the routine's for the
    /// rest of the expedition. A routine is repeatable and priced in a
    /// reserve that rest refills; an item is spent. Only the routine's half
    /// of that pair was ever meant to last the trip.
    pub fn runs_until_rest(&self) -> bool {
        self.source == BuffSource::Routine && self.kind.runs_until_rest()
    }

    /// The lifetime tag a buff row shows, e.g. `"90t"` or `"until rest"` —
    /// the sibling of `FieldBuffKind::magnitude_label` and built here for
    /// the same reason: a row transform belongs to the engine, so the map's
    /// buff list and the ally picker's "this is what you'd replace" line
    /// cannot describe one buff two ways.
    pub fn duration_label(&self) -> String {
        if self.runs_until_rest() {
            // One word, not "until rest": the map's status column has room
            // for a seven-character tag beside a full-width name and
            // magnitude, and `draw_status_buffs` measures nothing —
            // `draw_row` clips rows vertically and never horizontally, so a
            // longer tag runs off the panel in silence. Pinned by
            // `render::field::tests::the_widest_until_rest_buff_row_fits_the_status_column`.
            "rest".to_string()
        } else {
            format!("{}t", self.remaining)
        }
    }
}

/// Ends every until-rest buff on one holder, returning the names of what
/// went so the caller can log it.
///
/// A free function taking the component directly, for exactly the reason
/// `field_buff_power_of` below is one: the two callers are `Game::rest`, a
/// method, and `difficulty::death_handling_system`, a plain bevy system with
/// no `Game` to reach through. The same split
/// `CLAUDE.md` records for `game::stack::surfaced`.
pub fn drop_until_rest_buffs(buff: &mut FieldBuff) -> Vec<String> {
    let dropped: Vec<String> = buff
        .active
        .iter()
        .filter(|b| b.runs_until_rest())
        .map(|b| b.name.clone())
        .collect();
    buff.active.retain(|b| !b.runs_until_rest());
    dropped
}

/// Every field buff currently running on this entity. A `Vec`, not a
/// single slot like `CombatBuff`: a `Consumable` entry and a `Routine`
/// entry coexist, and distinct `Routine` kinds coexist with each other —
/// only a second buff from the *same* source (and, for `Routine`, the same
/// kind) displaces the one already running. `Game::arm_field_buff` is the
/// only writer and enforces both rules; nothing else may push or remove an
/// entry directly.
///
/// Only the player is spawned holding one; `arm_field_buff` inserts it on
/// demand for a companion a `Creature`-scoped buff lands on, the same
/// pattern `arm_buff` uses for `CombatBuff`.
#[derive(Component, Default, Clone)]
pub struct FieldBuff {
    pub active: Vec<ActiveFieldBuff>,
}

/// `buff`'s running `kind` power, `0` when none is active. Sums every
/// matching entry rather than reading just one: a `Consumable` and a
/// `Routine` of the same kind are required to coexist (`arm_field_buff`'s
/// whole reason for two separate displacement rules), and a reader that
/// only saw one of them would make that coexistence pointless — the buff
/// whichever entry it skipped would silently apply nothing.
///
/// A free function taking the component directly, not a method needing a
/// `Game`/`World`, so a plain bevy system with its own `Query<&FieldBuff>`
/// can call it too — `Game::field_buff_power` is a thin wrapper around this
/// for callers that already have an `Entity` and a `World` to fetch from.
pub fn field_buff_power_of(buff: &FieldBuff, kind: FieldBuffKind) -> i32 {
    buff.active
        .iter()
        .filter(|b| b.kind == kind)
        .map(|b| b.power)
        .sum()
}

/// Rounds remaining before each ability this combatant has spent can be
/// used again. Battle-scoped exactly like `CombatBuff` and `StatusEffects`
/// — armed during a fight, ticked at end of round, cleared when the
/// intrusion ends — so nothing here is ever persisted.
#[derive(Component, Default)]
pub struct AbilityCooldowns(pub std::collections::HashMap<crate::abilities::AbilityId, u32>);

/// An individual creature's innate quality roll, set once when it's
/// created (see `Game::spawn_wild_creature` / `Game::fuse_companions`)
/// and carried for its lifetime. `hp_roll`/`atk_roll`/`def_roll` are baked
/// into its starting `Stats` at creation time — this component doesn't
/// reapply them later, it just remembers what was rolled so
/// `quality_percent`/`quality_label` can describe it. `growth_roll`
/// actively scales `progression::add_xp`'s growth on every level-up, on
/// top of `SpeciesDef::growth_multiplier`.
#[derive(Component, Clone, Copy, Debug)]
pub struct Potential {
    pub hp_roll: f32,
    pub atk_roll: f32,
    pub def_roll: f32,
    pub growth_roll: f32,
}

impl Potential {
    /// Fallback for an entity with no roll of its own (e.g. a legacy save,
    /// or a test helper that spawns a creature directly) — every roll at
    /// its neutral 1.0, contributing neither a bonus nor a penalty.
    pub const NEUTRAL: Potential = Potential {
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
    };

    /// A single 0-100 "how good is this individual" percentile: averages
    /// all four rolls and maps `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`
    /// onto 0-100. Purely a display aggregate — each roll still applies
    /// independently to its own stat/growth.
    pub fn quality_percent(&self) -> u32 {
        let avg = (self.hp_roll + self.atk_roll + self.def_roll + self.growth_roll) / 4.0;
        let pct = (avg - MIN_INDIVIDUAL_ROLL) / (MAX_INDIVIDUAL_ROLL - MIN_INDIVIDUAL_ROLL) * 100.0;
        pct.round().clamp(0.0, 100.0) as u32
    }

    /// A coarse, human-readable tier for `quality_percent` — shown next to
    /// a creature in the pets and inspect screens.
    pub fn quality_label(&self) -> &'static str {
        match self.quality_percent() {
            0..=19 => "Poor",
            20..=39 => "Below Average",
            40..=59 => "Average",
            60..=79 => "Above Average",
            _ => "Excellent",
        }
    }

    /// Averages two parents' rolls into one — used when fusing two
    /// companions into one (`Game::fuse_companions`), so the result's
    /// quality reflects both parents rather than an independent fresh
    /// roll.
    pub fn averaged(a: Potential, b: Potential) -> Potential {
        Potential {
            hp_roll: (a.hp_roll + b.hp_roll) / 2.0,
            atk_roll: (a.atk_roll + b.atk_roll) / 2.0,
            def_roll: (a.def_roll + b.def_roll) / 2.0,
            growth_roll: (a.growth_roll + b.growth_roll) / 2.0,
        }
    }
}

/// The stable identity of one program the player owns, minted at
/// `Game::roster_parts` — the single barrier all four doors into the roster
/// pass through.
///
/// `Entity` cannot do this job: `save.rs` resolves everything by position or
/// by index precisely because entity ids are not stable across a save round
/// trip. A memory about one specific program needs a name that is, and this
/// is it.
///
/// Only an owned program carries one. Nothing wild or hostile reaches
/// `roster_parts`, so the absence of this component is what "not on the
/// roster" looks like, the same way an absent `Rarity` reads as `Ordinary`.
/// `0` is the unassigned sentinel a legacy save loads with; real ids start
/// at 1 and `Game::load` mints one for every program carrying the sentinel.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProgramId(pub u32);

/// What one owned program remembers. Minted empty at `Game::roster_parts`
/// beside `ProgramId`, so the absence of this component means "not on the
/// roster" rather than "remembers nothing".
#[derive(Component, Clone, Debug, Default)]
pub struct Memories(pub Vec<Memory>);

/// What one owned program's reserves stand at. Minted empty at
/// `Game::roster_parts` beside `Memories`, so the absence of this component
/// means "not on the roster" rather than "needs nothing".
///
/// **Empty is not the same as full.** Seeding lives in one place,
/// `seed_missing`, called at the top of the drain — one code path covers a
/// freshly spawned program, a program that predates a new def, and a save
/// written before this feature. Do not seed anywhere else.
///
/// Keyed by `NeedId` in a `BTreeMap` for `Stock`'s reason: iteration order
/// feeds the readouts and a `HashMap` would make the save encoding differ run
/// to run.
#[derive(Component, Clone, Debug, Default)]
pub struct Needs {
    reserves: std::collections::BTreeMap<NeedId, f32>,
    /// Which needs have already had their "nothing services this" complaint
    /// said. **Never saved** — a reload should say it again, `DigSite`'s
    /// `announced_stuck` rule.
    stalled_announced: std::collections::BTreeSet<NeedId>,
}

impl Needs {
    pub fn get(&self, id: &NeedId) -> Option<f32> {
        self.reserves.get(id).copied()
    }

    /// Clamps to `NEED_MIN..=NEED_MAX`. **The clamp is the type's**, exactly
    /// as `PowerReserve`'s is: a mod's `per_tick` and a save file's number are
    /// equally outside this crate's control, and no caller clamps.
    pub fn set(&mut self, id: &NeedId, value: f32) {
        self.reserves
            .insert(id.clone(), value.clamp(NEED_MIN, NEED_MAX));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&NeedId, f32)> {
        self.reserves.iter().map(|(id, v)| (id, *v))
    }

    /// Any def in `db` with no entry here starts full. The one seeding site —
    /// see the type doc.
    pub fn seed_missing(&mut self, db: &crate::needs::NeedDb) {
        for def in db.iter() {
            self.reserves.entry(def.id.clone()).or_insert(NEED_MAX);
        }
    }

    /// Whether this need's stall has already been announced.
    pub(crate) fn is_latched(&self, id: &NeedId) -> bool {
        self.stalled_announced.contains(id)
    }

    /// Latches the stall, and reports whether this was the **edge**. The
    /// announcement and the grudge both hang off a `true`, which is what
    /// makes them once each however many ways the need can fail to be met.
    pub(crate) fn latch(&mut self, id: &NeedId) -> bool {
        self.stalled_announced.insert(id.clone())
    }

    /// Clears the latch, so a need that recovers and runs down again
    /// complains a second time — `set_machine_status`' rule, that entering a
    /// state is news and staying in it is not.
    pub(crate) fn unlatch(&mut self, id: &NeedId) {
        self.stalled_announced.remove(id);
    }
}

/// The one thing this feature stores about a program that is not a reserve:
/// **which need has taken it off its post.**
///
/// Everything else is derived — which amenity is nearest, whether the program
/// is being serviced, what it is doing — for `hauling::Errand`'s reason. This
/// cannot be, because it is **hysteresis**: a rule read off the current value
/// alone flickers every tick at the boundary, pulled off at 20, returned at
/// 20.1, drained to 20 again. Inserted below the need's `critical`, removed at
/// its `content`, and the gap between the two is the whole point.
#[derive(Component, Clone, Debug)]
pub struct OffShift {
    pub need: NeedId,
}

/// A program whose morale has run far enough below zero that it has stopped
/// working — the `OffShift` shape on a different meter.
///
/// **Hysteresis is why this is stored and not derived**, exactly as
/// `OffShift` is: read off the current morale alone a body downs tools and
/// picks them up again every tick at the boundary. It is inserted below
/// `MORALE_DOWNS_TOOLS_AT` and removed at `MORALE_RECOVERED_AT`, and the gap
/// between the two is the feature.
///
/// It carries how far past the line the program went, because the ladder has
/// two rungs and the boundary between them needs the same protection as the
/// boundary at the bottom. Severity **ratchets** while the marker is held: a
/// body that has downed tools does not go back to merely sulking as morale
/// wobbles, it goes back to work or it does not. One hysteresis gap, not two.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Disgruntled {
    pub grievance: Grievance,
}

/// How far a program has gone. Ordered least to worst; see `Disgruntled`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Grievance {
    /// Still works, but not at a machine it holds a grudge against — the
    /// posting-time counterpart of `drift_idle_staff`'s tile avoidance,
    /// against the same `MEMORY_AVOIDANCE_THRESHOLD`.
    Sulking,
    /// Takes no posting at all. Leaves the `on_shift` filter, which covers
    /// the posting, the standdown and the `LabourDemand` shortfall at once.
    DownedTools,
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

/// One remembered thing: which kind it is, what it was about, when it was
/// last reinforced, and how many times.
///
/// **What it is worth is not here.** Intensity is derived from the game clock
/// at read time — see `Memory::intensity` — for the reason `Platform`'s
/// radius, a program's role and a Broker's board are derived: nothing ticks,
/// nothing oscillates, reinforcement is a single field write, and a stored
/// weight cannot drift out of step with the clock the way a per-tick
/// decrement can.
#[derive(Clone, Debug)]
pub struct Memory {
    /// Which `memories::MemoryDef` this is a record of. An id no file
    /// defines is **kept**, not dropped: restoring a removed mod file
    /// restores the memories that named it, and every reader already skips
    /// what it cannot resolve.
    pub def: crate::memories::MemoryId,
    pub subject: MemorySubject,
    /// The subject's display name as of the last reinforcement, captured at
    /// the write rather than resolved at the read — a program can be
    /// destroyed, and the screen still has to say who the memory is about.
    pub subject_name: Option<String>,
    /// The `resources::GameClock` tick this last landed on. The zero point
    /// of the decay, and the only field reinforcement writes besides
    /// `strikes`.
    pub reinforced: u64,
    /// How many times it has landed, clamped at read time by the def's
    /// `strike_cap`.
    pub strikes: u32,
}

/// What a memory is *about*.
///
/// Serde lives on this enum directly rather than on a `save::` mirror of it,
/// which is the opposite call from `save::CronjobKind`. A mirror here would
/// be a second copy of a six-variant enum that a new variant must be added to
/// twice with nothing failing to compile if it isn't — and the whole point of
/// `kind()`'s exhaustive match is that a new variant *must* fail to compile.
/// The on-disk form is field-named RON, so variants encode by name and
/// reordering them is not a save-format change (unlike `perks::Perk`, which
/// bincode encodes positionally).
///
/// **`BaseTile`, not `Place`.** Base space and surface space are the same two
/// integers meaning different things, and reading one as the other is what
/// put the base's roster on the open grid. Naming the space in the type is
/// what stops that recurring. A *surface* variant, when content asks for one,
/// is zone-local and has to be wiped by name in `Game::enter_next_zone`
/// alongside `StackMemory`, `BuybackLedger` and `PopulatedChunks`; a base
/// tile needs no such wipe, because the base travels with the party.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MemorySubject {
    /// About nothing in particular — a memory of an event, not of a thing.
    Nothing,
    Program(ProgramId),
    Species(SpeciesId),
    Structure(StructureId),
    /// **Base-space** coordinates, never a tile on the zone surface.
    BaseTile {
        x: i32,
        y: i32,
    },
    Activity(TaskKind),
}

impl MemorySubject {
    /// Which `MemorySubjectKind` this payload satisfies, so a write can be
    /// checked against the def that declared one.
    ///
    /// Exhaustive, and it stays exhaustive — `render/stack.rs`'s `cell_mark`
    /// rule. Under a `_ =>` arm a seventh variant would ship answering the
    /// wrong kind, and the symptom is a trigger silently refused rather than
    /// a compiler error.
    pub fn kind(&self) -> crate::memories::MemorySubjectKind {
        use crate::memories::MemorySubjectKind as K;
        match self {
            MemorySubject::Nothing => K::Nothing,
            MemorySubject::Program(_) => K::Program,
            MemorySubject::Species(_) => K::Species,
            MemorySubject::Structure(_) => K::Structure,
            MemorySubject::BaseTile { .. } => K::BaseTile,
            MemorySubject::Activity(_) => K::Activity,
        }
    }
}

impl Memory {
    /// What this memory is worth on tick `now`: signed, decayed, and derived
    /// on every read.
    ///
    /// ```text
    /// valence * min(strikes, strike_cap) * 2^-(elapsed / (half_life * MULT))
    /// ```
    ///
    /// The decay is a magnitude scale and never a sign flip, because `morale`
    /// is a signed sum over this figure — a grudge that decayed into a
    /// fondness would read as a program cheering up because it was hurt a
    /// while ago.
    pub fn intensity(&self, def: &crate::memories::MemoryDef, now: u64) -> f32 {
        self.intensity_with(def, now, crate::tuning::MEMORY_HALF_LIFE_MULTIPLIER)
    }

    /// `intensity` with the global stickiness dial supplied rather than read.
    ///
    /// The dial is a parameter here for `walk_field`'s reason: at its shipped
    /// neutral value a test cannot tell a formula that honours it from one
    /// that ignores it, so the only way to *prove* the dial reaches the
    /// denominator is to vary it.
    pub(crate) fn intensity_with(
        &self,
        def: &crate::memories::MemoryDef,
        now: u64,
        stickiness: f32,
    ) -> f32 {
        // A memory reinforced later than `now` is unreachable through
        // `Game::remember`, but a hand-edited save can hold one and an
        // underflow here is a panic in release arithmetic, not a wrong
        // number.
        let elapsed = now.saturating_sub(self.reinforced) as f32;
        let strikes = self.strikes.min(def.strike_cap) as f32;
        def.valence * strikes * 2f32.powf(-elapsed / (def.half_life as f32 * stickiness))
    }
}

/// The rare-spawn tier a creature rolled when it was created, if any — see
/// `Game::roll_rarity`. A creature that rolled ordinary has no such
/// component at all, which reads as `Ordinary`, so no test fixture or
/// hand-built bundle has to know this exists.
///
/// **This is a record of a multiplier already spent, not a live one — at
/// two sites now, not one.** `stat_mult` is applied at spawn, inside
/// `Game::spawn_wild_creature_scaled`, and baked into `Stats` there the same
/// way `Potential`'s three stat rolls are. `Game::promote_rarity` is the
/// second and last: a nemesis mark that ratchets this field up a rung
/// multiplies `Stats` by the *step* between the old and new tier's
/// `stat_mult`, never by the new tier's absolute value — reapplying that
/// would compound the spawn roll a second time. `Game::load` restores the
/// resulting numbers verbatim and `Game::fuse_companions` derives them from
/// its parents, so neither of those may apply this at all — the shape is
/// `EquippedItem::fusion_tier`, whose doc makes the same argument. Applying
/// it outside those two sites compounds the bonus on every reload,
/// invisibly, because a stat carries no record of where it came from. The
/// regression to head off is a later reader finding a multiplier that
/// appears to go unused and "finishing the job"; `a_shiny_survives_a_save_
/// round_trip` asserts the stats come back *unchanged* for exactly that
/// reason.
///
/// Player-facing text says Optimized/Overclocked rather than silver/gold,
/// which is the `MessageKind::Raid` / "GC Entropy Sweep" split: the enum and
/// the save field name the colours, everything a player reads names the
/// thing. `label` is the one place that translation happens.
///
/// **Append new tiers, never reorder.** The derived `Ord` *is* the worth
/// order and `Game::fuse_companions` inherits `max(a, b)` off it, so moving
/// a variant silently changes what a fusion produces. The save no longer
/// cares — since v29 the payload is field-named RON and an enum is written
/// by name — but `the_tiers_order_by_how_good_they_are` pins the half that
/// still bites.
///
/// Since 0.8.9 this axis covers **gear as well as programs**: a dropped
/// weapon rolls a tier the same way a wild program does, and reads in the
/// same words and the same colour. `EquipmentStats::for_rarity` is the gear
/// side of `stat_mult`.
#[derive(
    Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum Rarity {
    #[default]
    Ordinary,
    Silver,
    Gold,
    Platinum,
    Prismatic,
}

impl Rarity {
    /// Every tier, worst first. One definition, for the reason
    /// `EquipmentSlot::ALL` gives: the spawn ladder, the gear roll and the
    /// colour table all walk it, so a sixth rung cannot reach some of them
    /// and miss others — and `rank` is defined as the position in it.
    pub const ALL: [Rarity; 5] = [
        Rarity::Ordinary,
        Rarity::Silver,
        Rarity::Gold,
        Rarity::Platinum,
        Rarity::Prismatic,
    ];

    /// How far up the ladder this tier sits, `Ordinary` being 0.
    ///
    /// Read by `EquipmentStats::for_rarity`'s floor, which needs a *count*
    /// of rungs rather than a multiplier — the same job
    /// `ITEM_FUSION_MIN_BONUS_PER_TIER` does with a fusion tier, which is
    /// already a number. Derived from `ALL` rather than written out again,
    /// so the two cannot disagree.
    pub fn rank(self) -> u32 {
        Rarity::ALL.iter().position(|r| *r == self).unwrap_or(0) as u32
    }

    /// What this tier multiplies stats by — every one of a creature's four,
    /// or an item's three (see `EquipmentStats::for_rarity`).
    ///
    /// Deliberately one ladder for both. An Overclocked program and an
    /// Overclocked weapon are the same promise to the player, and a retune
    /// that moved only one of them would make the shared colour and the
    /// shared word a lie.
    pub fn stat_mult(self) -> f32 {
        match self {
            Rarity::Ordinary => 1.0,
            Rarity::Silver => SILVER_STAT_MULT,
            Rarity::Gold => GOLD_STAT_MULT,
            Rarity::Platinum => PLATINUM_STAT_MULT,
            Rarity::Prismatic => PRISMATIC_STAT_MULT,
        }
    }

    /// How the tier reads to a player, or `None` for an ordinary one —
    /// `Option` rather than an empty string so a caller has to decide what
    /// "no tier" looks like in its own context, the way `fusion_color`
    /// returns `Option` so a louder rule can compose with it.
    ///
    /// The words continue the compiler vocabulary the first two set, which
    /// is what lets one ladder cover programs and gear without either
    /// reading oddly: a program and a weapon are both things that were
    /// compiled better than usual.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Rarity::Ordinary => None,
            Rarity::Silver => Some("Optimized"),
            Rarity::Gold => Some("Overclocked"),
            Rarity::Platinum => Some("Unrolled"),
            Rarity::Prismatic => Some("Bare-Metal"),
        }
    }
}

/// How many fusions deep a creature's lineage is (see
/// `Game::fuse_companions`). A creature that was caught or spawned
/// normally has no such component at all, which reads as 0; a fusion's
/// result carries `max(parent_a, parent_b) + 1`, so the number is the
/// *depth* of the deepest chain behind it, not a count of ancestors.
///
/// Once it hits `MAX_FUSIONS` that creature is a finished product: it
/// can't be used as an input to another fusion, which stops a player from
/// laundering an endless supply of duplicates into one runaway program.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FusionCount(pub u32);

/// How many percentage upgrades have been spent on this program — see
/// `Game::refactor_companion`. Absent means zero, the same way `FusionCount`
/// is absent on a program that has never been fused.
///
/// It counts **only** the percentage buffs. A Recompile Kernel raises
/// `ZonePortal` instead and spends nothing here, because that track bounds
/// itself against the player's own zone and a player forced to burn permanent
/// slots just staying current has had the feature taken away at exactly the
/// point it was meant to help.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refactors(pub u32);

/// How many of this program's zone tiers were **bought** with Recompile
/// Kernels rather than earned by being tamed that deep. Absent means none,
/// like `Refactors` above.
///
/// It exists for one reason: `Game::program_payout` pays a fraction of
/// `Stats::power()`, and a kernel multiplies every one of those stats for
/// twelve printable Core Fragments. Left unrecorded, buying tiers and selling
/// the program prints Credits — measured at zone 7, a zone-1 program bought
/// up six tiers sold for 716 against 72 fragments' worth of kernels, and
/// Credits are the one currency that survives a breach. So the sale divides
/// the bought tiers back out, and a trader pays for what a program *is*.
///
/// `ZonePortal` cannot answer this on its own: a tier-4 program is worth
/// tier-4 money when it was tamed in zone 4, and this is what tells those two
/// apart. The percentage buffs deliberately are *not* divided out — five
/// slots is at most a 1.28x on power, which never repays the annealed cores
/// it costs, so there is nothing to close.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PurchasedTiers(pub u32);

/// How many Kernel Rings have been opened on this program — see
/// `Game::open_kernel_ring`. Absent means zero, like `Refactors` and
/// `PurchasedTiers` above.
///
/// Each ring raises this companion's level ceiling by
/// `tuning::LEVELS_PER_RING` and nothing else: it buys *room* to grow, never
/// growth itself. `Game::companion_level_cap` is the one expression of what
/// it is worth, so no call site adds the multiplication itself.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KernelRing(pub u32);

/// Which talent nodes this program has bought, in the order it bought them —
/// see `Game::take_talent`. Absent means none, like `KernelRing` above.
///
/// It is a **receipt**, not a source of truth for anything numeric. A `Stat`
/// node is baked into `Stats` at purchase exactly as a refactor is, so nothing
/// may re-apply this list on load; the other three kinds are read on demand
/// off it. Its length is also the whole of "how many points are spent" — there
/// is no stored count, because a count can desync from the list and from the
/// level while a derivation cannot.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct Talents(pub Vec<crate::talents::TalentId>);

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
/// `docs/seams.md`; 0.13.0 shipped two fixes for exactly this bug class, and
/// a wrong-space read is silent.
#[derive(Component, Clone, Copy, Debug, Default)]
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
    /// Whether the crew has already said it has nothing to floor this one
    /// with. `announced_stuck`'s rule and `announced_stuck`'s reason —
    /// entering a state is news, staying in it is not — but a second field
    /// rather than a shared one, because the two leave the player different
    /// errands: no route is a wall to cut, no substrate is a shelf to fill.
    ///
    /// Neither is saved: `save::DigSiteSave` writes the mark and the meter,
    /// so a reload says both again, which is right — the run that was told
    /// is over.
    pub announced_dry: bool,
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

    fn filed(structure: StructureId, cost: Vec<(ItemId, u32)>, goal: BuildGoal) -> Self {
        Self {
            structure,
            cost,
            delivered: Vec::new(),
            progress: 0,
            announced_dry: false,
            announced_stuck: false,
            goal,
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

/// Which leg of its journey a caravan is on.
///
/// **The stage decides which space the caravan stands in**, which is why
/// `Game::stands_in_base_space` reads a field rather than testing for the
/// component's presence: this is the first entity besides the player that
/// changes spaces, and asking "is it a caravan" would put it in one space
/// forever. See `in_base_space`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaravanStage {
    /// Walking across the zone surface toward the anchor.
    Approaching,
    /// Standing **on** the anchor tile, about to phase out.
    Docking,
    /// Inside base space, walking to the Market.
    Crossing,
    /// Standing beside the Market, open for trade.
    Docked,
    /// Back on the anchor tile, walking out the way it came.
    Leaving,
}

impl CaravanStage {
    /// Whether a caravan at this stage stands in base space.
    ///
    /// `Docking` and `Leaving` are the two transition ticks and both answer
    /// **false**, decided rather than defaulted: each is spent standing on
    /// the anchor tile *on the zone surface*, which is the tile the caravan
    /// walked to and the tile it walks away from. Answering true for either
    /// would draw it inside the base on a surface coordinate — base space's
    /// origin and the zone spawn point commonly alias, so the glyph would
    /// land somewhere plausible and stay wrong.
    pub fn in_base_space(self) -> bool {
        matches!(self, CaravanStage::Crossing | CaravanStage::Docked)
    }
}

/// A trader walking in, standing beside the Market, and walking out again.
///
/// **The journey is entity state; the schedule and the shelf are not.** When
/// this visit was due, who is walking in and what they carry are all derived
/// in `game/caravan.rs` from the base's own seed. What lives here is what
/// could not be derived: which leg it is on and where it is standing.
///
/// The third entity to carry a base-space `Position`, after a posted program
/// and a `DigSite` — and the first that carries one only *sometimes*. See
/// `CaravanStage::in_base_space`.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Caravan {
    pub stage: CaravanStage,
    /// Which scheduled visit this caravan is, so it can be matched back to
    /// its own shelf after a reload.
    pub visit: u64,
    /// The surface tile it appeared on, and the one it walks back to. Kept
    /// rather than re-derived from the bearing, because a caravan that
    /// arrived under one sector's `ZoneLevel` must leave the way it came
    /// even if something about the derivation moves under it.
    pub arrival_tile: (i32, i32),
    /// How many ticks it has spent on the current stage. Read by the two
    /// stuck cases, which are the only things that care.
    pub stage_ticks: u32,
    /// Whether the base has already been told this trader cannot reach the
    /// counter. `DigSite::announced_stuck`'s field, rule and reason: the
    /// stall lasts the rest of the visit, and entering a state is news while
    /// staying in it is not.
    ///
    /// Not saved, for `DigSite::announced_stuck`'s reason too — it is a
    /// conversation's "I already said so", and a reload is exactly when the
    /// player should be told again.
    pub announced_stuck: bool,
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

/// A way down into the Stack, standing on the zone map. Carries `Position`
/// and `Glyph` alongside this, the way a `Nest` does, but no `Durability`:
/// an entrance is walked into, not attacked.
///
/// Walking onto one is checked in `Game::move_player` before the generic
/// blocking-structure test, the same way a nest and a zone portal are.
#[derive(Component, Clone, Copy, Debug)]
pub struct SurfaceLink;

/// The permanent door into base space — the player's pocket-dimension base,
/// stepped into by walking onto this tile (`Game::enter_base`, wired in a
/// later task). Modeled on `SurfaceLink`: it carries `Position` and `Glyph`
/// alongside this, and stands on the zone map like any other surface
/// entity.
///
/// Deliberately **not** a `Structure`. That is load-bearing rather than
/// cosmetic: "every `Structure` entity is in base space" is how this slice
/// tells a deployed building apart from anything standing on the surface,
/// with no marker component of its own — making the anchor a `Structure`
/// would silently break that rule.
///
/// There is exactly one, spawned once by `Game::new` (and restored once by
/// `Game::load`) and never destroyed: it carries no `Durability`, so
/// `run_raid`'s `(With<Durability>, With<Structure>)` query cannot select
/// it. Unlike a
/// `SurfaceLink`, it survives `Game::enter_next_zone`'s stale-entity sweep
/// — it is not zone-local — and is moved to the new zone's spawn point
/// rather than despawned and respawned, so its identity carries across a
/// breach.
#[derive(Component, Clone, Copy, Debug)]
pub struct BaseAnchor;

/// Tags a wild program that was conjured for a Stack encounter rather
/// than found on the zone map.
///
/// It exists at surface coordinates like any other creature — the player's
/// `Position` is pinned to the link while they're underground, so that is
/// where the pack lands — but it has no business surviving the fight. Left
/// alive after a jack-out it would be standing around the link mouth when
/// the party climbs back out, a pack from a place the player left. So
/// `Game::end_battle` despawns whatever still carries this.
#[derive(Component, Clone, Copy, Debug)]
pub struct StackSpawn;

/// A creature that spawned as a boss.
///
/// Two things are bosses and only one of them is a species. An **apex**
/// species (`SpeciesDef::is_boss`) is always one and is hand-authored tough;
/// any other species can be **rolled** into one at `BOSS_SPAWN_CHANCE` and is
/// scaled by `tuning::BOSS_STAT_MULT` instead. This component is written at
/// both, so a query can ask without reaching for the db.
///
/// `Game::is_boss_creature` is still the one door, and it keeps the species
/// fallback: a fixture that hand-spawns an apex species outside `spawn_pack`
/// never gets a component, and must still be a boss.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boss;

/// How many times this program has sent the party away from a fight —
/// escalated by `Game::mark_nemeses` on a jack-out or a Forgiving defeat,
/// never a win. Absent means it never has, and nothing in this feature ever
/// takes it back to absent: the cap (`tuning::MAX_NEMESES`) refuses a *new*
/// mark once full, but an existing one always still escalates, so demotion
/// is not a case any caller has to handle.
#[derive(Component, Clone, Copy, Debug)]
pub struct Nemesis(pub u32);

/// Tags a wild creature as tethered to a `Nest` — see
/// `systems::wander_ai_system`'s radius check. Removed (not the
/// creature) when its nest is destroyed (`Game::attack_nest`) or when the
/// creature itself is killed/tamed, at which point it either despawns or
/// resumes ordinary untethered behavior.
#[derive(Component, Clone, Copy, Debug)]
pub struct NestGuardian {
    pub nest: Entity,
}

/// Marks a `NestGuardian` roused by an attack on its own nest — driven at
/// the player instead of wandering (`systems::wander_ai_system` excludes
/// it; the drive itself is Task 4's `nest_aggro_tick`). Deliberately no
/// target field: the player is the only thing anything in this game
/// pursues. And deliberately no duration: the chase ends spatially or with
/// the nest, never on a timer — a second pursuable target or a timed chase
/// is the signal to revisit this, not to bolt a field onto it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Pursuing;

/// Present on a structure whose `StructureDef::temporary` is set —
/// counts down by one on every ordinary game tick (see
/// `Game::tick_inner`) until it hits 0, at which point the structure
/// collapses. Ticks spent inside a `Game::rest` cycle deliberately don't
/// decrement this.
#[derive(Component, Clone, Copy, Debug)]
pub struct Temporary {
    pub ticks_remaining: u32,
}

/// Player-only: accumulated Perk Points (earned 1 per level-up) and which
/// perks have been bought with them. See `perks::Perk` — a perk can be
/// bought more than once, so `unlocked` holds one entry per level bought
/// (duplicates allowed) rather than a unique set.
#[derive(Component, Default, Clone)]
pub struct Perks {
    pub points: u32,
    pub unlocked: Vec<Perk>,
}

impl Perks {
    /// How many levels of `perk` have been bought — 0 if none.
    pub fn level(&self, perk: Perk) -> u32 {
        self.unlocked.iter().filter(|&&p| p == perk).count() as u32
    }
}

/// Every stat point on this creature that came from a spendable choice — a
/// perk level or a `TalentNode::Stat` — and can therefore be handed back by
/// `Game::respec_perks` / `Game::respec_talents`.
///
/// The receipt exists because neither grant is invertible from `Stats`.
/// `Perk::Buffer` and `TalentNode::Stat` are percentages of a maximum that
/// has since moved, and both floor at a whole point, so reversing the
/// arithmetic is off by a point per level in the common case — and a retune
/// of either constant would silently change what an old save's respec hands
/// back. Recording what was granted is the only thing that stays exact.
///
/// One component serves both ledgers: the player has no `Talents` and a
/// companion has no `Perks`, so the two can never write it at once.
///
/// Absent means nothing bought, like `KernelRing` and `Refactors`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoughtStats {
    pub atk: i32,
    pub mitigation: i32,
    pub max_hp: i32,
    /// How many perk levels the player has **ever** bought, which a respec
    /// deliberately does not reset.
    ///
    /// `Game::convert_overflow_xp` prices each minted Perk Point off this
    /// rather than off `Perks::unlocked`, because that list empties on a
    /// respec and the escalator it feeds is the only thing keeping banked
    /// cap XP from being an unbounded power source. Read off the list, a
    /// respec resets the price to the opening rate and the loop is: buy,
    /// wipe, mint the rest cheap.
    ///
    /// Perk purchases only. A companion's talents have no bearing on the
    /// player's XP drain, and counting them here would be exactly the drift
    /// a shared field invites.
    pub ever_bought: u32,
}

#[cfg(test)]
mod rarity_tests {
    use super::Rarity;

    /// Walks `ALL` rather than naming pairs, so a sixth rung is covered the
    /// moment it is added instead of leaving a test that still passes while
    /// checking five of six.
    #[test]
    fn rarity_multiplies_every_stat() {
        assert_eq!(Rarity::Ordinary.stat_mult(), 1.0, "ordinary must be inert");
        for pair in Rarity::ALL.windows(2) {
            assert!(
                pair[1].stat_mult() > pair[0].stat_mult(),
                "{:?} must be worth more than {:?}",
                pair[1],
                pair[0]
            );
        }
    }

    #[test]
    fn only_a_rare_tier_has_a_label() {
        assert_eq!(Rarity::Ordinary.label(), None);
        for tier in Rarity::ALL.into_iter().skip(1) {
            assert!(
                tier.label().is_some(),
                "{tier:?} needs a player-facing name"
            );
        }
    }

    /// Every rung's label is distinct. Two tiers reading the same way is the
    /// failure that survives every other test here — the stats would differ,
    /// the colour would differ, and the word on screen would not.
    #[test]
    fn no_two_tiers_read_the_same() {
        let mut seen: Vec<&str> = Rarity::ALL.into_iter().filter_map(|r| r.label()).collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(before, seen.len(), "two rare tiers share a label: {seen:?}");
    }

    /// `Game::fuse_companions` inherits `max(parent_a, parent_b)`, so the
    /// derived `Ord` has to agree with the tiers' worth. Reordering the
    /// variants would silently make a fusion of a gold and a silver
    /// produce a silver.
    #[test]
    fn the_tiers_order_by_how_good_they_are() {
        for pair in Rarity::ALL.windows(2) {
            assert!(
                pair[1] > pair[0],
                "{:?} must outrank {:?}",
                pair[1],
                pair[0]
            );
        }
        assert_eq!(Rarity::Silver.max(Rarity::Gold), Rarity::Gold);
        assert_eq!(Rarity::default(), Rarity::Ordinary);
        assert_eq!(
            Rarity::ALL[0],
            Rarity::Ordinary,
            "ALL must start at the inert rung — `rank` and the spawn ladder both index off it"
        );
    }

    /// `rank` is what the gear floor multiplies, so it has to be the
    /// position in `ALL` and not a second hand-written ladder.
    #[test]
    fn rank_is_the_position_in_the_ladder() {
        for (i, tier) in Rarity::ALL.into_iter().enumerate() {
            assert_eq!(tier.rank(), i as u32, "{tier:?} is out of step with ALL");
        }
    }
}

#[cfg(test)]
mod potential_tests {
    use super::Potential;

    #[test]
    fn quality_percent_maps_the_roll_range_onto_0_to_100() {
        let worst = Potential {
            hp_roll: 0.8,
            atk_roll: 0.8,
            def_roll: 0.8,
            growth_roll: 0.8,
        };
        let neutral = Potential::NEUTRAL;
        let best = Potential {
            hp_roll: 1.2,
            atk_roll: 1.2,
            def_roll: 1.2,
            growth_roll: 1.2,
        };
        assert_eq!(worst.quality_percent(), 0);
        assert_eq!(neutral.quality_percent(), 50);
        assert_eq!(best.quality_percent(), 100);
    }

    #[test]
    fn quality_label_buckets_the_percent_into_a_coarse_tier() {
        assert_eq!(
            Potential {
                hp_roll: 0.8,
                atk_roll: 0.8,
                def_roll: 0.8,
                growth_roll: 0.8,
            }
            .quality_label(),
            "Poor"
        );
        assert_eq!(Potential::NEUTRAL.quality_label(), "Average");
        assert_eq!(
            Potential {
                hp_roll: 1.2,
                atk_roll: 1.2,
                def_roll: 1.2,
                growth_roll: 1.2,
            }
            .quality_label(),
            "Excellent"
        );
    }

    #[test]
    fn averaged_splits_the_difference_between_two_parents() {
        let a = Potential {
            hp_roll: 0.8,
            atk_roll: 1.0,
            def_roll: 1.2,
            growth_roll: 0.9,
        };
        let b = Potential {
            hp_roll: 1.2,
            atk_roll: 1.0,
            def_roll: 0.8,
            growth_roll: 1.1,
        };
        let fused = Potential::averaged(a, b);
        assert_eq!(fused.hp_roll, 1.0);
        assert_eq!(fused.atk_roll, 1.0);
        assert_eq!(fused.def_roll, 1.0);
        assert_eq!(fused.growth_roll, 1.0);
    }
}

#[cfg(test)]
mod inventory_tests {
    use super::*;
    use crate::items::ids;
    use std::path::Path;

    fn item_db() -> ItemDb {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (abilities, _) =
            crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
        ItemDb::load_dir(&assets.join("items"), &abilities)
            .unwrap()
            .0
    }

    #[test]
    fn cargo_used_ignores_banked_currency() {
        let db = item_db();
        let mut inv = Inventory::default();
        inv.add(ItemId::from(ids::CORE_FRAGMENT), 5);
        inv.add(ItemId::from(ids::POWER_CELL), 3);
        inv.add(ItemId::from(ids::RESEARCH_DATA), 90);
        assert_eq!(
            inv.cargo_used(&db),
            8,
            "Research Data is banked, not carried"
        );
    }
}

#[cfg(test)]
mod stats_tests {
    use super::*;
    use crate::tuning::MAX_MITIGATION_PERCENT;

    #[test]
    fn power_prices_mitigation_as_effective_hp() {
        // 100 HP behind 50% mitigation is worth 200 HP of soak.
        let soft = Stats {
            hp: 100,
            max_hp: 100,
            atk: 10,
            mitigation: 0,
        };
        let armoured = Stats {
            mitigation: 50,
            ..soft
        };
        assert_eq!(soft.power(), 110);
        assert_eq!(armoured.power(), 210);
    }

    #[test]
    fn power_cannot_divide_by_zero_at_the_mitigation_cap() {
        // MAX_MITIGATION_PERCENT is capped strictly below 100, and that cap
        // is load-bearing here as well as in the damage path.
        let capped = Stats {
            hp: 100,
            max_hp: 100,
            atk: 0,
            mitigation: MAX_MITIGATION_PERCENT,
        };
        assert!(capped.power() > 0 && capped.power() < 100_000);
    }

    #[test]
    fn power_clamps_a_mitigation_beyond_the_cap() {
        // A save, a mod affix or a stacked buff can hand this a raw number
        // past the cap; `power` must not go negative or infinite on one.
        let overcapped = Stats {
            hp: 100,
            max_hp: 100,
            atk: 0,
            mitigation: 400,
        };
        let capped = Stats {
            mitigation: MAX_MITIGATION_PERCENT,
            ..overcapped
        };
        assert_eq!(overcapped.power(), capped.power());
    }

    #[test]
    fn power_ignores_current_hp() {
        let hurt = Stats {
            hp: 1,
            max_hp: 100,
            atk: 10,
            mitigation: 0,
        };
        let whole = Stats { hp: 100, ..hurt };
        assert_eq!(hurt.power(), whole.power());
    }
}
