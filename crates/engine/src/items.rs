use crate::affixes::AffixId;
use crate::components::Rarity;
use crate::species::SpeciesId;
use crate::tuning::{
    CONDITION_BASE, CONDITION_BOSS_BONUS, CONDITION_PER_RARITY_STEP, FIGHT_CONDITION_WEIGHT,
    GEAR_LEVEL_STEP, GEAR_RARITY_MIN_BONUS_PER_RUNG, GRADE_PER_LEVEL, GRADE_PER_RARITY_RUNG,
    ITEM_FUSION_BONUS_PER_TIER, ITEM_FUSION_MIN_BONUS_PER_TIER, QUALITY_ABOVE_MAX, QUALITY_DEFAULT,
    QUALITY_SPEC_MAX, QUALITY_UNDER_MAX,
};
use serde::{Deserialize, Serialize};

/// `#[serde(transparent)]` so an `ItemId` serializes as its bare inner string
/// rather than as a `ItemId("...")` tuple-struct — the RON asset files spell
/// item references as plain quoted strings (e.g. `work_resource: Some("power_cell")`),
/// and bincode saves encode it identically to a `String`.
/// `Ord` so `components::Stock` can key its buffers by item in a
/// `BTreeMap`: iteration order there feeds the production-chain pull phase
/// and the save encoding, both of which have to be identical run to run.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(pub String);

impl ItemId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The id of the etched Routine Disk holding `ability` — the item
    /// `ItemDb::synthesise_etched_disks` derives for it.
    ///
    /// One function rather than a `format!` at each site because this id is
    /// the seam between two databases: the ability half spells it when a
    /// boss drop or a market row is built, and the item half spells it when
    /// the disk is derived. A prefix typed twice is a disk nothing can ever
    /// install.
    ///
    /// The `etched_` prefix could in principle collide with a modder's own
    /// item — `ItemDb::synthesise_etched_disks` refuses to overwrite one and
    /// warns instead, which is the same call `load_dir` makes about a
    /// duplicated economy role.
    pub fn etched(ability: &str) -> Self {
        ItemId(format!("{ETCHED_DISK_PREFIX}{ability}"))
    }

    /// The ability burnt onto this disk, or `None` if this is not an etched
    /// disk at all. The inverse of `etched`, and the reason both live here:
    /// a round trip that does not close is a disk that installs the wrong
    /// routine.
    pub fn etched_ability(&self) -> Option<&str> {
        self.0.strip_prefix(ETCHED_DISK_PREFIX)
    }
}

/// What `ItemId::etched` puts in front of an ability id. Not in `ids`
/// because it is not an id — it is the scheme every etched disk's id is
/// built from, and `ids` is a list of specific shipped items.
pub const ETCHED_DISK_PREFIX: &str = "etched_";

impl From<&str> for ItemId {
    fn from(s: &str) -> Self {
        ItemId(s.to_string())
    }
}

impl From<String> for ItemId {
    fn from(s: String) -> Self {
        ItemId(s)
    }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Canonical ids of the shipped items. Used by test setup and data-defined
/// recipes for readability — never by engine *logic*, which goes through
/// economy roles and `ItemDef` fields.
pub mod ids {
    pub const CORE_FRAGMENT: &str = "core_fragment";
    pub const CREDITS: &str = "credits";
    pub const POWER_CELL: &str = "power_cell";
    pub const ICE_BREAKER: &str = "ice_breaker";
    pub const OVERCLOCK_CORE: &str = "overclock_core";
    pub const FIREWALL_PLATING: &str = "firewall_plating";
    pub const NEURAL_AMPLIFIER: &str = "neural_amplifier";
    pub const PORTAL_FRAGMENT: &str = "portal_fragment";

    /// Dropped by a lair guardian and by nothing else — see
    /// `Game::pay_stack_boss_privilege_ring`. Spent at the Develop screen to
    /// open a companion's next Kernel Ring (`Game::open_kernel_ring`).
    /// Deliberately not craftable: a bench recipe would make it renewable on
    /// demand, which is the opposite of what it is for.
    pub const PRIVILEGE_RING: &str = "privilege_ring";
    pub const RESEARCH_DATA: &str = "research_data";
    pub const MONOFILAMENT_WHIP: &str = "monofilament_whip";
    pub const ABLATIVE_PLATING: &str = "ablative_plating";
    pub const CORTEX_HACK: &str = "cortex_hack";
    /// Dormant. It was what a sealed Stack door cost until the seal became
    /// something the party simply shoulders open (`Game::force_seal`), and
    /// nothing spends one now — the id is kept, and the item still ships,
    /// against deciding what it is for instead.
    pub const ACCESS_SHARD: &str = "access_shard";
    /// Burnt to install a routine the player knows — see
    /// `Game::install_routine`. Named from Rust for the same reason
    /// `ACCESS_SHARD` is: what installing costs is engine content, not a
    /// data-driven requirement. The item, and the whole chain that makes it,
    /// are still ordinary `.ron` files.
    pub const ROUTINE_DISK: &str = "routine_disk";
    /// Named from Rust because the starting inventory is engine content
    /// (see `game/lifecycle.rs`) — same reason `CORE_FRAGMENT`, `POWER_CELL`
    /// and `ICE_BREAKER` are, on the same lines; the id itself lives in
    /// `assets/items/outlet.ron`.
    pub const OUTLET: &str = "outlet";
    /// The shipped production chain, named here only so the tests that walk
    /// it can spell it. Nothing in the engine references these: what each
    /// machine builds is authored in `assets/structures/*.ron`, and each
    /// recipe in the item's own file.
    pub const BYTECODE_BLOCK: &str = "bytecode_block";
    pub const BLANK_SUBSTRATE: &str = "blank_substrate";
    pub const CHARGE_COIL: &str = "charge_coil";
    pub const PATCH_ROUTINE: &str = "patch_routine";
}

/// What kind of thing an item is, for grouping the inventory and a trader's
/// list. Derived from the fields an `ItemDef` already declares — see
/// `ItemDef::category` — rather than authored, so a modded item is grouped
/// without its author adding a field, and the grouping cannot drift out of
/// step with the behaviour it describes.
///
/// Declaration order is display order: what you spend, then what you wear,
/// then what you hoard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemCategory {
    Consumable,
    Weapon,
    Armor,
    Module,
    Material,
    Currency,
}

impl ItemCategory {
    /// Compact form for a list row, matching `EquipmentSlot::short_label`'s
    /// case — the two sit in the same column on the inventory screen.
    pub fn short_label(self) -> &'static str {
        match self {
            ItemCategory::Consumable => "USE",
            ItemCategory::Weapon => "WEP",
            ItemCategory::Armor => "ARM",
            ItemCategory::Module => "MOD",
            ItemCategory::Material => "MAT",
            ItemCategory::Currency => "CUR",
        }
    }

    /// The heading a run of rows in this category is drawn under — the long
    /// form of `short_label`, for a list that groups rather than tags.
    ///
    /// One definition rather than one per list, `short_label`'s reason: the
    /// wagon heads its offers and the goods it will take with the same
    /// words, and two tables would eventually disagree about one of them.
    /// The enum's own declaration order is the run order, so the heading and
    /// the sort cannot drift apart either.
    pub fn heading(self) -> &'static str {
        match self {
            ItemCategory::Consumable => "Consumables",
            ItemCategory::Weapon => "Weapons",
            ItemCategory::Armor => "Armor",
            ItemCategory::Module => "Modules",
            ItemCategory::Material => "Materials",
            ItemCategory::Currency => "Currency",
        }
    }
}

/// Which *copy* of an item this is — everything that makes two copies of the
/// same id non-interchangeable, and nothing else.
///
/// Two copies with equal `GearCopy`s are genuinely the same thing and stack;
/// two that differ are separate rows on every screen, separate rows on a
/// trader's shelf, and separate stores in cargo. That is the whole reason
/// this is one value rather than three parameters: **`is_plain` decides
/// which store a copy lives in**, and `count_copies`/`take_copies`/
/// `add_copies` all have to answer that question identically or a copy is
/// written to one store and looked up in the other, which reads to a player
/// as gear vanishing out of cargo.
///
/// It is also what makes the equip/unequip symmetry structural.
/// `Game::apply_equipment_delta` writes a bonus straight into `Stats`, so the
/// unequip must subtract *exactly* what the equip added; with the properties
/// loose, a path that forgot one would subtract less and weld the difference
/// permanently into base stats with no record of where it came from. Because
/// `EquippedItem` stores this whole value and `Game::gear_bonus` computes
/// from it, forgetting one is not expressible.
///
/// `level` is deliberately **not** here. It is a property of the moment an
/// item was put on rather than of the copy — a copy in cargo has no level,
/// and two copies that differ only in the level someone once wore them at
/// are the same copy. It lives on `EquippedItem` beside this.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GearCopy {
    pub item: ItemId,
    /// The rare tier this copy rolled when it dropped, `Ordinary` for
    /// anything crafted, bought, or found before 0.8.9 — see
    /// `Game::grant_gear_drop`.
    #[serde(default)]
    pub rarity: Rarity,
    /// How many times this copy has been fused — see `Game::fuse_item`.
    #[serde(default)]
    pub tier: u32,
    /// The affixes this copy carries — one from the drop that rolled it,
    /// and one more for every copy fused into it. See `affixes::AffixDef`,
    /// `Game::grant_gear_drop` and `Game::fuse_item`. They decide both the
    /// generated name and an extra flat stat bonus, and duplicates count
    /// twice.
    ///
    /// **Always sorted.** This struct is the key of the
    /// `components::GearCopies` ledger, of `EquippedItem` and of a trader's
    /// buyback shelf, and all three find rows by `==`. `[A, B]` and
    /// `[B, A]` are the same copy to a player and must be the same copy to
    /// `Eq`, or one is written to a row and looked up at another — which
    /// reads as gear vanishing out of cargo, the failure this type's own
    /// doc is written to prevent. `GearCopy::with_affixes` is the one
    /// canonicalising constructor; nothing else builds a non-empty list.
    ///
    /// `#[serde(default)]` on a field of a *named* struct, so it is purely
    /// additive: a save written before affixes existed loads with every copy
    /// unaffixed, which is what it had. An id naming an affix the build no
    /// longer has is not an error either — `Game::affixes_of` simply finds
    /// nothing for it and skips it, the same shape `recognized_routines`
    /// gives a removed ability.
    #[serde(default)]
    pub affixes: Vec<AffixId>,
    /// How well this particular copy was compiled, as a percentage of the
    /// item's authored bonus — see `EquipmentStats::for_quality`.
    /// `QUALITY_DEFAULT` is "exactly as designed".
    ///
    /// **An integer on purpose.** This struct is the key of the
    /// `components::GearCopies` ledger and of `EquippedItem`; both find
    /// rows by `==`, so a float would take `Eq` and the keyed-by-value seam
    /// with it.
    ///
    /// `default = "default_quality"` rather than a bare `#[serde(default)]`:
    /// `u8`'s `Default` is 0, which would load every piece of gear in every
    /// existing save at 0% of its authored bonus — a total loss of stats
    /// presenting as a balance bug rather than as a failed load.
    #[serde(default = "default_quality")]
    pub quality: u8,
}

/// `serde`'s default for `GearCopy::quality` — see that field.
fn default_quality() -> u8 {
    QUALITY_DEFAULT
}

impl GearCopy {
    /// An ordinary, unfused copy: what crafting, buying, and every drop
    /// before rare tiers existed produce.
    pub fn plain(item: ItemId) -> Self {
        Self {
            item,
            rarity: Rarity::Ordinary,
            tier: 0,
            affixes: Vec::new(),
            quality: QUALITY_DEFAULT,
        }
    }

    /// A copy carrying `affixes`, canonicalised. **The only way a non-empty
    /// affix list is built** — see that field for why the sort is the
    /// invariant rather than a tidiness.
    pub fn with_affixes(
        item: ItemId,
        rarity: Rarity,
        tier: u32,
        mut affixes: Vec<AffixId>,
        quality: u8,
    ) -> Self {
        // Sorted, not deduped: `[A, B]` and `[B, A]` are the same copy to a
        // player and must be the same copy to `Eq`. Duplicates are the
        // feature — a copy fused from two carrying the same affix is worth
        // it twice.
        affixes.sort();
        Self {
            item,
            rarity,
            tier,
            affixes,
            quality,
        }
    }

    /// Whether this copy is indistinguishable from any other copy of the
    /// same id — which is exactly the question "does it live in
    /// `Inventory`". **The single definition**, so the three functions that
    /// pick a store cannot drift apart; see this type's doc for what happens
    /// if they do.
    ///
    /// A fourth property added to a copy joins the `&&` here and nowhere
    /// else.
    pub fn is_plain(&self) -> bool {
        self.rarity == Rarity::Ordinary
            && self.tier == 0
            && self.affixes.is_empty()
            && self.quality == QUALITY_DEFAULT
    }

    /// Whether two copies may be fused into one — **the single definition**
    /// of what fusion requires to match, read by `Game::fuse_item` and by
    /// the partner search it runs.
    ///
    /// Deliberately narrower than `==`. Quality and affixes go free, which
    /// is the whole feature: quality is thirteen buckets and affixes a
    /// fifth axis, so two field-found copies of one item almost never match
    /// as whole values and fusion stopped firing for anything not crafted
    /// or bought. Rarity stays matched because there is no midpoint rare
    /// tier for a Gold-plus-Ordinary fuse to land on, so either parent's
    /// tier would be laundered into or out of the result depending on which
    /// one won.
    pub fn fusable_with(&self, other: &Self) -> bool {
        self.item == other.item && self.rarity == other.rarity && self.tier == other.tier
    }
}

/// A wild program taken apart at the kill rather than paid out directly —
/// see `docs/superpowers/specs/2026-09-04-program-extraction-design.md`.
/// Carried until a tool consumes it (`Game::extract_program`, a later
/// phase), in `components::DownedPrograms`.
///
/// **Instanced, not a stackable `ItemId`** (decision 1 of the spec). A
/// level-30 Prismatic kill and a level-2 Ordinary one are not
/// interchangeable, so the species, level, rarity, boss flag and condition
/// all travel with the one row rather than collapsing into a count against
/// a per-species id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DownedProgram {
    pub species: SpeciesId,
    pub level: u32,
    pub rarity: Rarity,
    pub boss: bool,
    /// 0..=100, rolled once at the kill and never touched again — see
    /// `tuning::CONDITION_BASE` and its neighbours for the roll, which is a
    /// later phase's to call; this field only carries the result.
    pub condition: u8,
}

impl DownedProgram {
    /// "How good is this program", the one fold of its three graded axes —
    /// every extraction yield formula calls this rather than re-reading
    /// `condition`/`rarity`/`level` and re-deriving its own opinion of them,
    /// the way `Game::copy_bonus` calls `scaled_for_level`/`for_rarity`
    /// rather than each caller repeating the scaling.
    ///
    /// Multiplicative, not additive: an additive fold would let a rolled-to-
    /// zero `condition` be masked by a high rarity or level instead of
    /// gating the whole result, and a destroyed program should not yield as
    /// if it were merely ordinary. `1.0` is the fold's identity — full
    /// condition, `Ordinary` rarity, level 0 — which is what
    /// `GRADE_PER_RARITY_RUNG` and `GRADE_PER_LEVEL` are scaled against.
    ///
    /// Rarity contributes by **rank**, `Rarity::ALL`'s own ladder
    /// (`Rarity::rank`) rather than a second one authored here — the same
    /// rule the condition roll's `CONDITION_PER_RARITY_STEP` follows.
    pub fn grade(&self) -> f32 {
        let condition = self.condition as f32 / 100.0;
        let rarity = 1.0 + self.rarity.rank() as f32 * GRADE_PER_RARITY_RUNG;
        let level = 1.0 + self.level as f32 * GRADE_PER_LEVEL;
        condition * rarity * level
    }

    /// The condition roll a kill takes once, at the moment it leaves a
    /// program behind — spec section 1's formula, verbatim.
    /// `Game::leave_downed_program` is the one caller, so a boss's own
    /// floors (`tuning::BOSS_CONDITION_FLOOR`) apply there rather than here:
    /// this function knows only what a kill's own terms are worth, not that
    /// a boss's result gets raised afterward.
    ///
    /// `overkill_term` is a plain `f32`, not read off a live entity here, so
    /// this stays pure and directly testable against
    /// `FIGHT_CONDITION_WEIGHT = 0.0`'s independence claim without a `Game`
    /// in the room. Multiplying by `0.0` rather than an `if` on the weight:
    /// the axis is meant to fall out of the formula for free the day a
    /// non-zero value earns a play session, not be wired in as a second
    /// branch then.
    pub(crate) fn roll_condition(rarity: Rarity, boss: bool, overkill_term: f32) -> u8 {
        let boss_bonus = if boss { CONDITION_BOSS_BONUS as i32 } else { 0 };
        let fight_term = (FIGHT_CONDITION_WEIGHT * overkill_term).round() as i32;
        let raw = CONDITION_BASE as i32
            + CONDITION_PER_RARITY_STEP as i32 * rarity.rank() as i32
            + boss_bonus
            + fight_term;
        raw.clamp(0, 100) as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EquipmentSlot {
    Weapon,
    Armor,
    Module,
}

impl EquipmentSlot {
    /// Every slot, in the order the equipment panel and the companion slot
    /// page list them. One definition, so a fourth slot cannot reach some
    /// screens and miss others — or, worse, be missed by `Game::gear_bonus`
    /// and `Game::strip_gear`, where a skipped slot is a bonus welded into
    /// the wearer's base stats.
    pub const ALL: [EquipmentSlot; 3] = [
        EquipmentSlot::Weapon,
        EquipmentSlot::Armor,
        EquipmentSlot::Module,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EquipmentSlot::Weapon => "Weapon",
            EquipmentSlot::Armor => "Armor",
            EquipmentSlot::Module => "Module",
        }
    }

    /// Compact form for space-constrained rows — see the inventory list's
    /// equip tag, where it sits beside `ATK`/`DEF`/`DECOMP` and so matches
    /// their case. `label` stays the name for headers and prose.
    pub fn short_label(self) -> &'static str {
        match self {
            EquipmentSlot::Weapon => "WEP",
            EquipmentSlot::Armor => "ARM",
            EquipmentSlot::Module => "MOD",
        }
    }

    /// One character for a roster row, where three slots have to fit beside a
    /// name, a level and two stat pairs — see `Game::gear_tag`.
    ///
    /// Derived from `short_label` rather than matched again: the letters are
    /// the same three words abbreviated further, and a fourth slot added to
    /// `ALL` gets its mark for free instead of silently drawing as whatever a
    /// second match's fallback arm said.
    pub fn initial(self) -> char {
        self.short_label()
            .chars()
            .next()
            .expect("every slot label is non-empty")
            .to_ascii_lowercase()
    }
}

/// Flat stat bonuses an equipped item grants while worn, at gear level 1
/// (base). See `GEAR_LEVEL_STEP`/`EquipmentStats::scaled_for_level` for
/// how a higher gear level scales these up.
///
/// The type is `pub` — a `.ron` file authors one — but the **three scaling
/// axes below are `pub(crate)` on purpose**: outside the engine the only way
/// to price a piece of gear is `Game::copy_bonus`. They were public, and four
/// screens each built the chain themselves out of an item's catalogue entry;
/// every one of them then priced gear as though the affix property did not
/// exist, because none of them could see a `GearCopy`'s affix from an
/// `EquipmentStats`. Keeping them crate-private is what makes that fifth copy
/// fail to compile rather than merely be discouraged.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct EquipmentStats {
    #[serde(default)]
    pub atk: i32,
    /// Percentage points, summed into `Stats::mitigation` by
    /// `Game::apply_equipment_delta` and capped by
    /// `Game::effective_mitigation`.
    #[serde(default)]
    pub mitigation: i32,
    #[serde(default)]
    pub decompiler: i32,
    /// A weapon's damage band, which **overrides** the wielder's natural
    /// attack rather than adding to it — see `Game::attack_range`. Zero on
    /// everything that is not a weapon, and it stays zero through all three
    /// scaling axes: a tier sharpens what an item does and never hands it a
    /// stat it never had.
    #[serde(default)]
    pub damage: crate::battle::DamageRange,
    /// Read live off `Game::gear_bonus` by `battle::accuracy_of`. Unlike
    /// `atk` and `mitigation` this is **not** baked into `Stats` — there is
    /// no field for it there and `apply_equipment_delta` must not invent one.
    #[serde(default)]
    pub accuracy: i32,
    /// See `accuracy`. Light armour buys this where heavy armour buys
    /// `mitigation`, which is what makes the two defensive axes a real
    /// choice.
    #[serde(default)]
    pub evasion: i32,
}

impl EquipmentStats {
    /// Whether this bonus is nothing at all — an affix carrying it would
    /// rename an item and change nothing.
    ///
    /// **Destructured rather than field-accessed**, on `cell_mark`'s rule: a
    /// seventh stat is a compile error here rather than a field silently
    /// uncounted. `accuracy` was exactly that — an affix authoring only
    /// accuracy was refused at load as empty for as long as the field
    /// existed, so the accuracy axis could only ever be a rider on an ATK
    /// affix, which is part of why it stayed on three weapons.
    pub(crate) fn is_empty(self) -> bool {
        let EquipmentStats {
            atk,
            mitigation,
            decompiler,
            damage,
            accuracy,
            evasion,
        } = self;
        atk == 0
            && mitigation == 0
            && decompiler == 0
            && accuracy == 0
            && evasion == 0
            && damage == crate::battle::DamageRange::default()
    }

    /// Whether anything here is worth having. A penalty is legal *beside* a
    /// bonus — that trade is a shipped affix shape — but a penalty on its own
    /// is a roll no player has a reason to equip.
    ///
    /// Destructured for `is_empty`'s reason, and it carried the same gap:
    /// accuracy and evasion were both invisible to it.
    pub(crate) fn has_upside(self) -> bool {
        let EquipmentStats {
            atk,
            mitigation,
            decompiler,
            damage,
            accuracy,
            evasion,
        } = self;
        atk > 0 || mitigation > 0 || decompiler > 0 || accuracy > 0 || evasion > 0 || damage.max > 0
    }

    /// This item's bonus scaled up for `level` (1 = base, no scaling).
    /// Each component is rounded independently to the nearest whole point.
    pub(crate) fn scaled_for_level(self, level: u32) -> EquipmentStats {
        let factor = 1.0 + GEAR_LEVEL_STEP * (level.max(1) as f64 - 1.0);
        let scale = |v: i32| (v as f64 * factor).round() as i32;
        EquipmentStats {
            atk: scale(self.atk),
            mitigation: scale(self.mitigation),
            decompiler: scale(self.decompiler),
            damage: scale_range(self.damage, scale),
            accuracy: scale(self.accuracy),
            evasion: scale(self.evasion),
        }
    }

    /// This item's bonus scaled up for `tier` fusions (0 = base, no
    /// scaling) — see `ITEM_FUSION_BONUS_PER_TIER`. Applied on top of
    /// `scaled_for_level`, not in place of it.
    ///
    /// A stat the item already has gains at least
    /// `ITEM_FUSION_MIN_BONUS_PER_TIER` per tier, whatever the percentage
    /// works out to. The percentage alone is worthless at the magnitudes
    /// equipment actually ships at — 4 × 1.1 rounds straight back to 4 —
    /// so the floor is what makes a fusion observable rather than a
    /// silent loss of two items. A stat sitting at zero stays at zero: the
    /// floor sharpens what an item does and does not hand it a new stat.
    ///
    /// A stat sitting *below* zero stays where it is too, which is the same
    /// rule `for_rarity` states and reachable by the same route: a drawback
    /// affix (`assets/affixes/README.md`) is folded into the base, so a copy
    /// can carry a negative on an axis its item never had. Scaling that
    /// would make improving a copy deepen its penalty — you would spend
    /// `ITEM_FUSION_COST` copies to make the thing you own worse on one
    /// axis, which is not a trade the affix was authored to offer.
    pub(crate) fn fused_for_tier(self, tier: u32) -> EquipmentStats {
        let factor = 1.0 + ITEM_FUSION_BONUS_PER_TIER * tier as f64;
        let floor = ITEM_FUSION_MIN_BONUS_PER_TIER * tier as i32;
        let scale = |v: i32| {
            if v <= 0 {
                return v;
            }
            ((v as f64 * factor).round() as i32).max(v + floor)
        };
        EquipmentStats {
            atk: scale(self.atk),
            mitigation: scale(self.mitigation),
            decompiler: scale(self.decompiler),
            damage: scale_range(self.damage, scale),
            accuracy: scale(self.accuracy),
            evasion: scale(self.evasion),
        }
    }

    /// This item's bonus scaled up for the rare tier the *copy* rolled when
    /// it dropped (`Ordinary` = base, no scaling) — see `components::Rarity`
    /// and `Game::grant_gear_drop`. Applied on top of the other two, not in
    /// place of either.
    ///
    /// Deliberately the same shape as `fused_for_tier`, floor and all, and
    /// for the same reason: the percentage alone is worthless at the
    /// magnitudes equipment ships at, so `GEAR_RARITY_MIN_BONUS_PER_RUNG`
    /// is what makes a tier observable rather than a colour that changes no
    /// number. A stat sitting at zero stays at zero.
    ///
    /// The multiplier is shared with programs (`Rarity::stat_mult`) rather
    /// than being a second gear-only ladder, so one retune moves the word,
    /// the colour and both sets of numbers together.
    ///
    /// **Why this walks the rungs instead of applying one multiplier.** A
    /// creature's stats are in the tens or hundreds, where a 1.5x and a 1.8x
    /// are plainly different numbers. Gear ships at 1..=4 points, where they
    /// are frequently the *same* number: `round(3 * 1.5)` and
    /// `round(3 * 1.8)` are both 5, so a single-multiplier form makes an
    /// Overclocked weapon identical to an Optimized one on every base-3 stat
    /// in the game — a different colour, a different word, and no difference
    /// the player can act on. Walking the ladder and taking
    /// `max(multiplier, previous_rung + GEAR_RARITY_MIN_BONUS_PER_RUNG)`
    /// makes every rung strictly better than the one below it *by
    /// construction*, at every base value, which is the property
    /// `every_rare_tier_is_worth_more_than_the_one_below_it` pins.
    ///
    /// A stat sitting at zero stays at zero — a tier sharpens what an item
    /// does and does not hand it a stat it never had.
    pub(crate) fn for_rarity(self, rarity: Rarity) -> EquipmentStats {
        let scale = |base: i32| {
            if base <= 0 {
                return base;
            }
            (1..=rarity.rank()).fold(base, |value, rank| {
                let by_multiplier =
                    (base as f64 * Rarity::ALL[rank as usize].stat_mult() as f64).round() as i32;
                by_multiplier.max(value + GEAR_RARITY_MIN_BONUS_PER_RUNG)
            })
        };
        EquipmentStats {
            atk: scale(self.atk),
            mitigation: scale(self.mitigation),
            decompiler: scale(self.decompiler),
            damage: scale_range(self.damage, scale),
            accuracy: scale(self.accuracy),
            evasion: scale(self.evasion),
        }
    }

    /// This item's bonus scaled for how well *this copy* was compiled
    /// (`QUALITY_DEFAULT` = the authored numbers, no scaling) — see
    /// `items::GearCopy::quality`. Applied on top of `scaled_for_level` and
    /// underneath the two floored axes; `Game::copy_bonus` owns that order
    /// and argues for it.
    ///
    /// **No per-step floor, unlike its two siblings.** Theirs exist to make
    /// a *discrete rung* observable at the magnitudes gear ships at; quality
    /// is continuous and is meant to be a fine gradient, and a floor would
    /// flatten the whole band onto one number on a 4-point stat.
    ///
    /// Being floor-free is also why it cannot go last: a bare percentage on
    /// an unscaled 4-point stat is eaten by rounding, and worse, it can
    /// invert the rare tiers — base atk 4 gives a `Silver` copy at 70% the
    /// same 4 an `Ordinary` copy at 130% rounds up to 5, which makes the row
    /// colour a lie about which copy is better.
    ///
    /// A stat at zero stays at zero and a negative one is left alone, both
    /// for the reasons `for_rarity` gives.
    pub(crate) fn for_quality(self, quality: u8) -> EquipmentStats {
        let factor = quality as f64 / QUALITY_DEFAULT as f64;
        let scale = |v: i32| {
            if v <= 0 {
                return v;
            }
            (v as f64 * factor).round() as i32
        };
        EquipmentStats {
            atk: scale(self.atk),
            mitigation: scale(self.mitigation),
            decompiler: scale(self.decompiler),
            damage: scale_range(self.damage, scale),
            accuracy: scale(self.accuracy),
            evasion: scale(self.evasion),
        }
    }
}

/// Which of four rungs a copy's quality reads as. The renderer maps this to
/// a colour and a weight; the thresholds are the engine's so the five sites
/// that draw a category tag cannot come to disagree about them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityBand {
    Under,
    AsDesigned,
    Above,
    Exceptional,
}

/// Which band `quality` falls in — see `QualityBand` and
/// `tuning::QUALITY_UNDER_MAX` and its two siblings.
pub fn quality_band(quality: u8) -> QualityBand {
    match quality {
        q if q <= QUALITY_UNDER_MAX => QualityBand::Under,
        q if q <= QUALITY_SPEC_MAX => QualityBand::AsDesigned,
        q if q <= QUALITY_ABOVE_MAX => QualityBand::Above,
        _ => QualityBand::Exceptional,
    }
}

/// Applies an axis's own `scale` to **both ends** of a damage band,
/// independently.
///
/// Two of the three axes carry a per-step floor, and a floor does not
/// commute with a multiplier — scaling the midpoint and re-deriving the
/// width would give a different answer at both ends. Every `scale` already
/// leaves a zero at zero, which is what keeps armour's empty band empty
/// through all three axes.
fn scale_range(
    range: crate::battle::DamageRange,
    scale: impl Fn(i32) -> i32,
) -> crate::battle::DamageRange {
    crate::battle::DamageRange {
        min: scale(range.min),
        max: scale(range.max),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::MAX_FUSIONS;

    #[test]
    fn for_quality_is_a_percentage_of_the_authored_bonus() {
        let base = EquipmentStats {
            atk: 10,
            mitigation: 4,
            damage: crate::battle::DamageRange { min: 5, max: 15 },
            ..EquipmentStats::default()
        };

        let same = base.for_quality(QUALITY_DEFAULT);
        assert_eq!(same.atk, 10, "100% is the identity");
        assert_eq!(same.damage.min, 5);
        assert_eq!(same.damage.max, 15);

        let good = base.for_quality(130);
        assert_eq!(good.atk, 13);
        assert_eq!(good.mitigation, 5);
        assert_eq!(
            (good.damage.min, good.damage.max),
            (7, 20),
            "both ends of a band scale, or a high roll collapses it to a point"
        );

        let poor = base.for_quality(70);
        assert_eq!(poor.atk, 7);
        assert_eq!((poor.damage.min, poor.damage.max), (4, 11));
    }

    /// The two rules `for_rarity` states, reachable here by the same route:
    /// quality sharpens what an item does rather than handing it a stat it
    /// never had, and improving a copy never deepens a drawback affix's
    /// penalty.
    #[test]
    fn for_quality_leaves_a_zero_at_zero_and_a_negative_where_it_is() {
        let base = EquipmentStats {
            atk: 0,
            evasion: -3,
            ..EquipmentStats::default()
        };
        for quality in [70u8, QUALITY_DEFAULT, 130] {
            let scaled = base.for_quality(quality);
            assert_eq!(scaled.atk, 0, "a stat the item does not have stays absent");
            assert_eq!(scaled.evasion, -3, "a drawback is never deepened");
        }
    }

    /// Every boundary in the four-band ladder, and the one that matters
    /// most: `QUALITY_DEFAULT` lands in the band that reads as no change,
    /// so every copy in every existing save is repainted by nothing.
    #[test]
    fn quality_band_buckets_the_whole_range() {
        use crate::tuning::{QUALITY_MAX, QUALITY_MIN};
        assert_eq!(quality_band(QUALITY_MIN), QualityBand::Under);
        assert_eq!(quality_band(90), QualityBand::Under);
        assert_eq!(quality_band(95), QualityBand::AsDesigned);
        assert_eq!(quality_band(QUALITY_DEFAULT), QualityBand::AsDesigned);
        assert_eq!(quality_band(105), QualityBand::AsDesigned);
        assert_eq!(quality_band(110), QualityBand::Above);
        assert_eq!(quality_band(120), QualityBand::Above);
        assert_eq!(quality_band(125), QualityBand::Exceptional);
        assert_eq!(quality_band(QUALITY_MAX), QualityBand::Exceptional);
    }

    #[test]
    fn scaled_for_level_adds_100_percent_of_base_per_level_above_1() {
        let base = EquipmentStats {
            atk: 4,
            mitigation: 0,
            decompiler: 0,
            ..EquipmentStats::default()
        };
        assert_eq!(
            base.scaled_for_level(1).atk,
            4,
            "level 1 should be unscaled base"
        );
        assert_eq!(
            base.scaled_for_level(2).atk,
            8,
            "level 2 should be base + 100% of base (4 * 2 = 8)"
        );
        assert_eq!(
            base.scaled_for_level(3).atk,
            12,
            "level 3 adds another 100% of base, not another 100% of level 2 \
             (4 * 3 = 12, not 16) — gear tracks a linear zone curve"
        );
        assert_eq!(
            base.scaled_for_level(0).atk,
            4,
            "level 0 should clamp to level 1's unscaled base"
        );
    }

    /// Neither investable axis may deepen a drawback. A copy's penalty is
    /// part of its base and so grows with gear level like everything else,
    /// but fusing it or rolling it at a rare tier are things the *player*
    /// spends on — and spending to make a copy worse on one axis is the
    /// trade nobody would take. See `assets/affixes/README.md`.
    #[test]
    fn neither_fusion_nor_a_rare_tier_deepens_a_penalty() {
        let charged = EquipmentStats {
            atk: 4,
            mitigation: 0,
            decompiler: -2,
            ..EquipmentStats::default()
        };
        assert_eq!(
            charged.fused_for_tier(MAX_FUSIONS).decompiler,
            -2,
            "a fusion deepened the penalty it was bought to improve past"
        );
        assert_eq!(charged.for_rarity(Rarity::Prismatic).decompiler, -2);
        assert!(
            charged.fused_for_tier(MAX_FUSIONS).atk > 4,
            "the bonus axis must still be worth fusing for"
        );
    }

    #[test]
    fn an_ordinary_tier_leaves_an_item_exactly_as_it_was() {
        let base = EquipmentStats {
            atk: 3,
            mitigation: 1,
            decompiler: 2,
            ..EquipmentStats::default()
        };
        let same = base.for_rarity(Rarity::Ordinary);
        assert_eq!((same.atk, same.mitigation, same.decompiler), (3, 1, 2));
    }

    /// Every rung has to move every stat the item actually has. The floor
    /// exists precisely because the percentage alone does not guarantee it
    /// at the magnitudes gear ships at (1..=4 a stat).
    #[test]
    fn every_rare_tier_is_worth_more_than_the_one_below_it() {
        // Every magnitude shipped gear actually uses. 3 is the one that
        // caught the original single-multiplier form: round(3 * 1.5) and
        // round(3 * 1.8) are both 5.
        for stat in [1, 2, 3, 4] {
            let base = EquipmentStats {
                atk: stat,
                ..Default::default()
            };
            for pair in Rarity::ALL.windows(2) {
                let lower = base.for_rarity(pair[0]).atk;
                let upper = base.for_rarity(pair[1]).atk;
                assert!(
                    upper > lower,
                    "at base {stat}, {:?} ({upper}) must beat {:?} ({lower})",
                    pair[1],
                    pair[0]
                );
            }
        }
    }

    /// A tier sharpens what an item does; it does not hand it a stat it
    /// never had. Same rule `fused_for_tier` follows, and the reason the
    /// floor is guarded on `v > 0`.
    #[test]
    fn a_rare_tier_never_invents_a_stat_the_item_lacks() {
        let weapon_only = EquipmentStats {
            atk: 3,
            ..Default::default()
        };
        for tier in Rarity::ALL {
            let scaled = weapon_only.for_rarity(tier);
            assert_eq!(scaled.mitigation, 0, "{tier:?} invented a DEF bonus");
            assert_eq!(scaled.decompiler, 0, "{tier:?} invented a DECOMP bonus");
        }
    }

    /// **The three axes do not commute, so the order `Game::gear_bonus`
    /// applies them in is load-bearing rather than stylistic.**
    ///
    /// Two of them carry a per-step floor, and a floor is not commutative
    /// with a multiplier: applying rarity to a base 3 and then levelling
    /// gives a different number from levelling first and then applying
    /// rarity, because the floor is measured against whatever it is handed.
    /// That is fine — one order is canonical — but it means a call site that
    /// reorders the chain silently changes the stat, and an unequip that
    /// reordered it relative to its equip would leave the difference welded
    /// into the wearer's base `Stats` with no record of where it came from.
    ///
    /// This asserts the asymmetry exists so nobody "tidies" the chain on the
    /// assumption that it can't matter.
    #[test]
    fn the_gear_axes_do_not_commute_so_the_order_is_load_bearing() {
        let base = EquipmentStats {
            atk: 3,
            mitigation: 2,
            decompiler: 0,
            ..EquipmentStats::default()
        };
        let canonical = base
            .scaled_for_level(3)
            .fused_for_tier(2)
            .for_rarity(Rarity::Gold);
        let reordered = base
            .for_rarity(Rarity::Gold)
            .scaled_for_level(3)
            .fused_for_tier(2);
        assert_ne!(
            (canonical.atk, canonical.mitigation),
            (reordered.atk, reordered.mitigation),
            "if these ever agree, this test has stopped protecting anything — \
             check whether a floor was removed from one of the three axes"
        );
    }

    #[test]
    fn equipment_stats_round_trip_ron_with_omitted_zero_fields() {
        let full: EquipmentStats = ron::from_str("(atk: 3, def: 0, decompiler: 0)").unwrap();
        assert_eq!((full.atk, full.mitigation, full.decompiler), (3, 0, 0));
        // Zero fields may be omitted thanks to per-field serde defaults.
        let partial: EquipmentStats = ron::from_str("(atk: 4)").unwrap();
        assert_eq!(
            (partial.atk, partial.mitigation, partial.decompiler),
            (4, 0, 0)
        );
    }

    #[test]
    fn both_ends_of_a_damage_range_carry_the_per_step_floor() {
        // A floor does not commute with a multiplier, so the ends cannot be
        // scaled by a shortcut that scales the midpoint and re-derives the
        // width. Fusing a 4-9 weapon must lift both ends by at least
        // ITEM_FUSION_MIN_BONUS_PER_TIER per tier.
        let base = EquipmentStats {
            damage: crate::battle::DamageRange { min: 4, max: 9 },
            ..EquipmentStats::default()
        };
        let fused = base.fused_for_tier(2);
        assert!(fused.damage.min >= base.damage.min + 2 * ITEM_FUSION_MIN_BONUS_PER_TIER);
        assert!(fused.damage.max >= base.damage.max + 2 * ITEM_FUSION_MIN_BONUS_PER_TIER);
        assert!(fused.damage.max >= fused.damage.min);
    }

    #[test]
    fn a_zero_damage_range_stays_zero_through_every_axis() {
        // Armour has no damage range and must never be handed one — the
        // same rule the other axes already state.
        let armour = EquipmentStats {
            mitigation: 4,
            ..EquipmentStats::default()
        };
        let scaled = armour
            .scaled_for_level(6)
            .fused_for_tier(3)
            .for_rarity(Rarity::ALL[Rarity::ALL.len() - 1]);
        assert_eq!(scaled.damage, crate::battle::DamageRange::default());
    }

    #[test]
    fn accuracy_and_evasion_scale_on_the_same_three_axes_as_every_other_stat() {
        let light = EquipmentStats {
            evasion: 3,
            accuracy: 2,
            ..EquipmentStats::default()
        };
        let scaled = light.scaled_for_level(4);
        assert!(scaled.evasion > light.evasion);
        assert!(scaled.accuracy > light.accuracy);
    }

    /// The per-tier floor is what makes a fusion observable at the
    /// magnitudes flat gear actually ships at — 1 to 4 points of `atk` or
    /// `decompiler`, where 20% a tier rounds straight back to where it
    /// started.
    ///
    /// It used to be demonstrable on armour too. It is not any more:
    /// mitigation became percentage points and every armour number tripled,
    /// so the percentage now wins there. This is the case that kept the
    /// floor honest, moved to an axis that still shows it.
    #[test]
    fn the_fusion_floor_beats_the_percentage_at_the_magnitudes_gear_ships_at() {
        let flat = EquipmentStats {
            atk: 2,
            ..EquipmentStats::default()
        };
        // 2 * 1.4 rounds to 3; the floor is 2 + 2 = 4 and has to win.
        assert_eq!(flat.fused_for_tier(2).atk, 4);
        assert_eq!(
            (flat.atk as f64 * (1.0 + ITEM_FUSION_BONUS_PER_TIER * 2.0)).round() as i32,
            3,
            "the percentage alone would have paid less, which is the point"
        );
    }
}
