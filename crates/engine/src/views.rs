//! Read-only snapshots of engine state, shaped for the renderer.
//!
//! Every one of these is produced by a `Game` method and consumed by
//! app-core/gui. They are plain data with no back-reference into the ECS —
//! that is what keeps the renderer from reaching into the `World`.

use crate::abilities::AffinityKind;
use crate::battle::ActionOption;
use crate::components::{EquippedItem, GlyphColor, TaskKind};
use crate::items::ItemId;
use crate::perks::Perk;
use crate::research::ResearchId;
use crate::species::MoveDef;
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
    /// Display names of the prerequisites still missing — the menu shows
    /// *why* a node can't be taken rather than just greying it out.
    Locked {
        missing: Vec<String>,
    },
}

pub struct PlayerStatus {
    pub position: (i32, i32),
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    pub decompiler: i32,
    pub hunger: f32,
    pub fatigue: f32,
    pub inventory: Vec<(ItemId, u32)>,
    /// Units of ordinary cargo currently carried. The Buffer is unbounded, so
    /// this is just how much is stored; it excludes banked currency (see
    /// `ItemId::bank_limit`), so it will not match the sum of `inventory`
    /// when Research Data is held.
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
    pub item: ItemId,
    pub name: String,
    /// How many are on the shelf — the shelf is a record of the player's own
    /// sales, so this is a hard cap on what `Game::buy_back` will hand over.
    pub qty: u32,
    pub unit_cost: u32,
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

pub struct PetInfo {
    pub entity: Entity,
    pub name: String,
    pub level: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
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
}

/// Snapshot of the player's active companion, shown in the status panel
/// and during an intrusion.
pub struct CompanionInfo {
    pub entity: Entity,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
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
    pub is_boss: bool,
    pub can_work: bool,
    /// Whether this (structure) entity is a trading post (see
    /// `StructureDef::trade`).
    pub can_trade: bool,
    /// If this is a structure, the label of the (tamed) entity currently
    /// working it via cronjob, if any.
    pub structure_worker: Option<String>,
    pub hp_fraction: Option<f32>,
    pub level: Option<u32>,
    /// If this is a structure, its current/max raid `Durability`.
    pub durability: Option<(u32, u32)>,
    /// How many fusions deep this (creature) entity's lineage is, 0 to
    /// `MAX_FUSIONS` — see `components::FusionCount`. At `MAX_FUSIONS` it
    /// can no longer be an input to a fusion, which the fuse menus show.
    pub fusions: u32,
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
    /// Whether the def declares a `work` recipe. A workable structure with no
    /// assignees is idle and producing nothing, which is the one thing on
    /// this screen the player can act on.
    pub workable: bool,
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
    pub atk: i32,
    pub def: i32,
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
    pub def: i32,
    pub status_effect: Option<String>,
    /// What this member has left to spend on routines, or `None` for one
    /// that has none to spend — every companion. Fatigue lives on the
    /// player's `Needs` alone, and it is the player who pays for a routine
    /// however it was ordered, so this is `Some` on slot 0 and nowhere else.
    pub fatigue: Option<f32>,
    /// This round's chosen action rendered for the roster, or `None` if the
    /// slot is still awaiting one.
    pub planned: Option<String>,
    /// Whether this slot is in the front line, which draws more enemy fire
    /// — see `FRONT_SLOTS`. Soft ranks: a back slot is still targetable.
    pub front: bool,
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
pub struct CraftRecipe {
    pub result: ItemId,
    pub cost: Vec<(ItemId, u32)>,
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
    /// "You" for the player, the program's display name otherwise.
    pub name: String,
    pub level: u32,
    pub filled: usize,
    pub slots: usize,
}

/// One row of the install picker — a loose routine held in inventory.
pub struct RoutineItemView {
    pub item: ItemId,
    pub name: String,
    pub description: String,
    pub count: u32,
}

/// One row of the field-routine picker — a `FieldBuff` ability installed on
/// you or a program you own, run outside battle rather than spent as a
/// battle Special. See `Game::field_routines`.
pub struct FieldRoutineView {
    pub ability: crate::abilities::AbilityId,
    pub name: String,
    pub description: String,
    pub holder: Entity,
    /// "You" for the player, the program's display name otherwise — same
    /// convention as `RoutineHolderView::name`.
    pub holder_label: String,
    pub power_cost: f32,
    /// Whether the player can pay `power_cost` right now.
    pub affordable: bool,
    /// Whether casting this routine needs a `target` — true only for a
    /// `Creature`-scoped `FieldBuff` authoring `AbilityTarget::OneAlly`; a
    /// `Run`-scoped routine (always `WholeParty`, see
    /// `abilities::AbilityDef::field_buff_target_mismatch`) or a
    /// `WholeParty` one needs no picker.
    pub needs_ally_target: bool,
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
    pub remaining: u32,
    /// `Some(program name)` when the buff sits on a companion, `None` for
    /// the player.
    pub holder_label: Option<String>,
}

/// Everything the engine knows about one subject, for the manifest screen —
/// the player, a program you own, or a wild one. Shared header fields plus a
/// `subject` carrying the half that differs, so "the player has no Potential
/// roll" and "a program has no equipment" are type-level facts rather than
/// `Option`s a renderer can forget to check.
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
    pub def: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    /// Active battle status condition, e.g. "Bleeding (2)" — see
    /// `Game::status_label`. Always `None` outside an intrusion.
    pub status_effect: Option<String>,
    /// Every routine slot, filled or empty. Reuses `RoutineSlotView` rather
    /// than a parallel type, so the manifest and the routines menu cannot
    /// disagree about what is installed.
    pub routines: Vec<RoutineSlotView>,
    pub subject: ManifestSubject,
}

pub enum ManifestSubject {
    Player(PlayerManifest),
    Program(ProgramManifest),
}

/// The player-only half of a manifest.
pub struct PlayerManifest {
    pub hunger: f32,
    pub fatigue: f32,
    pub decompiler: i32,
    /// One entry per *occupied* slot — an empty slot is absent rather than
    /// listed as "(none)", so the section shrinks to what is actually worn.
    pub equipment: Vec<ManifestEquipSlot>,
    pub perk_points: u32,
    /// Every perk bought at least once, as (display name, level).
    pub perks: Vec<(String, u32)>,
    pub position: (i32, i32),
    pub zone: u32,
    pub pet_count: usize,
    pub pet_capacity: usize,
    pub cargo_used: u32,
    pub party: Vec<CompanionInfo>,
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
    pub def: i32,
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
    /// `None` for a creature with no `Potential` component — an old save
    /// predating it, or a test helper that spawned one directly.
    pub potential: Option<ManifestPotential>,
    pub fusions: u32,
    /// `tuning::MAX_FUSIONS`, carried so the renderer prints "1/3" without
    /// importing a tuning constant of its own.
    pub max_fusions: u32,
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
    /// Categories this species is not neutral in, in `AffinityKind` order.
    /// Empty for a species that declares nothing, so the screen omits the
    /// section entirely rather than drawing five rows of 1.00.
    pub affinities: Vec<(AffinityKind, f32)>,
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
