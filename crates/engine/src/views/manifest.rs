use super::*;

/// The dossier page's whole derivation — `Mode::Dossier`'s only input, and
/// a second sheet about whatever body `Mode::Manifest` is showing.
///
/// **Nothing in here is read for a mechanic.** See `attributes.rs`.
pub struct DossierReport {
    /// What the page's title row leads with — `Game::creature_label`'s
    /// answer, so the dossier and the manifest name the same body the same
    /// way.
    pub name: String,
    pub revision: String,
    pub checksum: String,
    /// One sentence of where this program came from, or `None` when
    /// `assets/descriptions/` has nothing for the subject — a mod's
    /// prerogative, and the page simply draws no row.
    pub provenance: Option<String>,
    pub rows: Vec<AttributeRow>,
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
    /// `Game::effective_atk`, for the player and a program alike, so the
    /// page agrees with the roster row.
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    /// The damage band this combatant swings for and its type, already
    /// formatted by `damage::band_label` — a worn weapon's if it has one, else its
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
    /// the manifest's frame census tests), not a fact the engine
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
    /// Active battle statuses — see `Game::status_tags`. Always empty
    /// outside an intrusion.
    pub statuses: Vec<StatusTagView>,
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
    /// An outpost standing on the zone surface. Tile-keyed rather than
    /// `Entity`-keyed — `crate::outposts::Outpost`'s own record lives in
    /// `resources::Outposts`, addressed by tile, and has no entity to name.
    /// `Game::outpost_report(tile)` is the whole of what the player gets,
    /// the same door a bump onto the tile already opens.
    Outpost((i32, i32)),
}

pub enum ManifestSubject {
    Player(PlayerManifest),
    /// Boxed because a program's sheet is three times the player's and the
    /// enum would otherwise be sized by the larger arm everywhere it is
    /// returned — the shape `clippy::large_enum_variant` names.
    Program(Box<ProgramManifest>),
}

impl ManifestView {
    /// Whether this sheet's subject has a memories page to open — true
    /// exactly when it carries a mood, which is every program on the roster
    /// and nothing else. The footer's `[R]` and app-core's refusal both ask
    /// this, so the key cannot be offered where it would be refused.
    pub fn remembers(&self) -> bool {
        matches!(&self.subject, ManifestSubject::Program(p) if p.mood.is_some())
    }
}

/// The player-only half of a manifest.
pub struct PlayerManifest {
    /// See `components::PowerReserve`.
    pub power: f32,
    /// See `PlayerStatus::max_power`.
    pub max_power: f32,
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
    /// The damage-type resistances borrowed from the species being emulated
    /// (`SpeciesDef::resist`), positive only, in `DamageType::ALL` order.
    /// Empty when not emulating: the player has no species of their own.
    pub emulated_resistances: Vec<(crate::damage::DamageType, i32)>,
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
    /// Flat damage removed per hit — see `items::EquipmentStats::deflection`.
    pub deflection: i32,
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
    /// Breeding generation, 0 for anything not bred.
    pub generation: u32,
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
    /// The program's own Analysis attribute (its species' catalogue figure
    /// while unseated). Shown beside `base_speed` in the manifest's WORK box
    /// rather than in SPECIES, because both are about what this program is
    /// like to *post* somewhere.
    pub analysis: i32,
    /// Categories this species is not neutral in, in `AffinityKind` order.
    /// Empty for a species that declares nothing, so the screen omits the
    /// section entirely rather than drawing five rows of 1.00.
    pub affinities: Vec<(AffinityKind, f32)>,
    /// The species' natural damage-type resistances (`SpeciesDef::resist`),
    /// percent, positive only, in `DamageType::ALL` order. Armour is not in
    /// it: this is what the program *is*, and gear shows on its own rows.
    pub resistances: Vec<(crate::damage::DamageType, i32)>,
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
    /// What this program is carrying, and what it adds up to — see
    /// `ManifestMood`.
    ///
    /// **`None` is "has no `Memories` store", not "remembers nothing".** The
    /// store is minted at `roster_parts` and nowhere else, so its absence
    /// *is* "not on the roster" — the same asymmetry `Game::remember` relies
    /// on to leave hostiles, structures and the player alone without a branch
    /// at the call site. A wild program therefore drops the box structurally
    /// rather than by an ownership check here, and an owned one that nothing
    /// has happened to yet is `Some` with an empty `memories`.
    pub mood: Option<ManifestMood>,
    /// The points a seated program levels through. `None` for a program no
    /// door has seated (a wild one), which has none to spend or hold.
    pub points: Option<ProgramPoints>,
}

/// What the manifest shows of a seated program's points: the bank, and
/// whether level-ups are being banked rather than spent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramPoints {
    pub banked: u32,
    pub holding: bool,
}

/// What one owned program feels, as the manifest's MEMORIES box draws it: the
/// signed sum, the word that sum reads as, and the strongest few entries
/// behind it.
///
/// **One derivation, `Game::memory_report`'s rule.** Every figure here is a
/// call into the engine rather than a formula the renderer keeps a copy of —
/// `memories` is `memory_report` truncated, not a second sort, and `sum` is
/// `Game::morale` rather than a fold over the rows the renderer can see. The
/// renderer can only see the truncated few, so a sum derived there would
/// quietly stop matching the `R` page's.
///
/// `memories` carries **more rows than the box draws**, capped at
/// `MANIFEST_MOOD_MEMORIES`: the truncation is the renderer's, and a view
/// that pre-cut to exactly two would leave the width census unable to measure
/// anything but the two a fixture happened to build.
#[derive(Debug, Clone)]
pub struct ManifestMood {
    /// `Game::morale` — the signed sum of every memory, decayed to now and
    /// felt through this program's disposition.
    pub sum: f32,
    /// What `sum` reads as in words — see `morale_band`.
    pub band: &'static str,
    /// The strongest entries, magnitude-first, as `Game::memory_report`
    /// orders them. Empty for a program nothing has happened to, and for an
    /// install with `assets/memories/` deleted.
    pub memories: Vec<MemoryRow>,
}

/// How many memory rows `ManifestMood` carries. Two more than the manifest
/// draws, so the box's width census has rows to measure past the ones it
/// shows and a later change to how many are drawn is a renderer change.
pub const MANIFEST_MOOD_MEMORIES: usize = 4;

/// Every word `morale_band` can return, best first.
pub const MORALE_BANDS: [&str; 5] = ["devoted", "content", "even", "uneasy", "bitter"];

/// Which word a morale sum reads as.
///
/// **Banded against where morale stops mattering**, not against an invented
/// range: `systems::morale_shift` prices a point at
/// `MEMORY_MORALE_PER_POINT` and caps the whole term at
/// `MEMORY_MORALE_MAX_SHIFT`, so the sum saturates at their quotient and
/// every point past it changes nothing. The extremes are named for that
/// point, and the quotient is *derived* here rather than written as a
/// literal — a retune of either constant has to move these words with it, or
/// the sheet would keep calling a saturated program by a middling name.
///
/// `need_band`'s rule otherwise: four bands whose boundaries are the
/// player's read of the figure rather than the mechanism's own thresholds,
/// so one word means the same thing wherever it appears.
///
/// Symmetric around zero, which is the state three different things share —
/// a program with no memories, one whose grudges and bonds cancel, and an
/// install with `assets/memories/` deleted. "even" is a real answer for all
/// three and not a missing one.
pub fn morale_band(sum: f32) -> &'static str {
    // Where `morale_shift` saturates: past this the sum buys nothing.
    let full =
        (crate::tuning::MEMORY_MORALE_MAX_SHIFT / crate::tuning::MEMORY_MORALE_PER_POINT) as f32;
    // Half of saturation is the inner boundary, so the two outer words are
    // reserved for a program whose feelings are actually moving its work.
    match sum {
        s if s >= full => MORALE_BANDS[0],
        s if s >= full / 2.0 => MORALE_BANDS[1],
        s if s > -full / 2.0 => MORALE_BANDS[2],
        s if s > -full => MORALE_BANDS[3],
        _ => MORALE_BANDS[4],
    }
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

/// One reserve as the headless bench reads it: the raw level beside the
/// def's own thresholds, so a share-below-critical is computed against the
/// number the sim itself uses.
#[derive(Debug, Clone, PartialEq)]
pub struct NeedLevel {
    pub id: String,
    pub level: f32,
    pub critical: f32,
    pub content: f32,
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
    /// The extraction aptitude — how often a cycle lands rather than
    /// fizzles: the program's own Analysis attribute, or its species'
    /// catalogue figure while unseated.
    pub analysis: i32,
    /// The species' base job, or `None` for a boss and for anything else
    /// outside the class system. Carried as the class rather than a finished
    /// phrase so the renderer keeps choosing the player-facing word.
    pub class: Option<AffinityClass>,
}

/// An individual's four `Potential` rolls, surfaced separately rather than
/// only as the aggregate tier the roster shows.
#[derive(Debug, PartialEq)]
pub struct ManifestPotential {
    pub hp_roll: f32,
    pub atk_roll: f32,
    pub def_roll: f32,
    pub growth_roll: f32,
    /// The two build rolls — see `components::Potential`. Listed after the
    /// four combat rolls and before `percent`, which deliberately folds only
    /// the first four.
    pub assembly_roll: f32,
    pub extraction_roll: f32,
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
}
