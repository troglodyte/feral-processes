use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::affixes::AffixId;
use crate::classes::PlayerClass;
use crate::components::{ActiveFieldBuff, Rarity};
use crate::items::{DownedProgram, EquipmentSlot, ItemId};
use crate::perks::Perk;
use crate::resources::DifficultyMode;
use crate::species::SpeciesId;
use crate::world::Tile;

#[derive(Serialize, Deserialize)]
pub struct PlayerSave {
    pub position: (i32, i32),
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    /// Percentage points — see `components::Stats::mitigation`. **Not**
    /// `#[serde(default)]`: this was `def`, a subtractive absorption number,
    /// and a v30 file's value would load into a percentage slot and mean
    /// something else entirely. A changed meaning under a name it keeps is
    /// exactly the case field-named RON does not cover, so the file is
    /// refused by version instead.
    pub mitigation: i32,
    pub power: f32,
    pub inventory: Vec<(ItemId, u32)>,
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
    pub decompiler: i32,
    pub weapon: Option<ItemId>,
    /// Gear level `weapon` was equipped at — see `components::EquippedItem`.
    pub weapon_level: u32,
    /// Fusion tier `weapon` was equipped at — see `items::GearCopy`.
    pub weapon_fusion_tier: u32,
    /// Rare tier of the worn `weapon` — see `items::GearCopy`. Additive
    /// behind `#[serde(default)]`, like its two siblings below: a save
    /// written before gear had rare tiers loads with ordinary gear, which is
    /// what it had.
    #[serde(default)]
    pub weapon_rarity: Rarity,
    /// **Legacy, load-only.** The one affix a worn copy could carry before
    /// affixes stacked; lifted into `weapon_affixes` by
    /// `affixes_from_save`. Written by nothing — `PlayerSave::fused_gear`
    /// is the precedent, and `Experience::xp_to_next` the shape.
    #[serde(default)]
    pub weapon_affix: Option<AffixId>,
    /// Every affix on the worn weapon — see `items::GearCopy::affixes`.
    #[serde(default)]
    pub weapon_affixes: Vec<AffixId>,
    /// How well the worn copy was compiled — see `items::GearCopy::quality`.
    /// Additive behind a default of `QUALITY_DEFAULT` rather than `u8`'s own
    /// `Default` of 0, which would silently strip a worn item of its whole
    /// bonus on the first reload.
    #[serde(default = "default_worn_quality")]
    pub weapon_quality: u8,
    pub armor: Option<ItemId>,
    pub armor_level: u32,
    pub armor_fusion_tier: u32,
    #[serde(default)]
    pub armor_rarity: Rarity,
    /// **Legacy, load-only** — see `weapon_affix`.
    #[serde(default)]
    pub armor_affix: Option<AffixId>,
    #[serde(default)]
    pub armor_affixes: Vec<AffixId>,
    #[serde(default = "default_worn_quality")]
    pub armor_quality: u8,
    pub module: Option<ItemId>,
    pub module_level: u32,
    pub module_fusion_tier: u32,
    #[serde(default)]
    pub module_rarity: Rarity,
    /// **Legacy, load-only** — see `weapon_affix`.
    #[serde(default)]
    pub module_affix: Option<AffixId>,
    #[serde(default)]
    pub module_affixes: Vec<AffixId>,
    #[serde(default = "default_worn_quality")]
    pub module_quality: u8,
    /// Unspent Perk Points — see `perks::Perk`.
    pub perk_points: u32,
    /// Which perks have been bought, and at what level (see
    /// `components::Perks::level`) — one entry per level bought.
    pub unlocked_perks: Vec<Perk>,
    /// What those perks granted, so `Game::respec_perks` can hand it back —
    /// see `components::BoughtStats`.
    ///
    /// Additive behind a default, so **no `SAVE_FORMAT_VERSION` bump**. A save
    /// written before respec shipped loads with an empty receipt, which means
    /// a respec on it refunds the points and leaves the stats — see
    /// `Game::load` for the `ever_bought` seed that keeps the overflow-XP
    /// price honest across that boundary.
    #[serde(default)]
    pub bought_stats: crate::components::BoughtStats,
    /// Whether this run was started under the onboarding chain.
    ///
    /// `#[serde(default)]` to false, which is what a save written before the
    /// chain existed reads as — and `Game::load` files the whole chain as
    /// finished for those, so a run forty hours old is never told to build a
    /// Home it built long ago. Additive behind a default, so **no
    /// `SAVE_FORMAT_VERSION` bump**.
    #[serde(default)]
    pub tutorial_seeded: bool,
    /// **Legacy, read-only, and on its way out.** Fused copies as
    /// `(item, tier, qty)`, which is how they were stored up to v29.
    ///
    /// A copy now carries a rare tier as well as a fusion tier, and this is
    /// a *positional* tuple — RON matches those by exact arity and refuses a
    /// widened one, so the property could not be added here. (It also cannot
    /// be turned into a named struct with defaulted trailing fields: RON
    /// parses `(` in a struct position as the start of named fields and
    /// raises `ExpectedIdentifier` at the first element rather than falling
    /// through to serde's `visit_seq`. Measured — see
    /// `a_v29_save_still_loads_its_gear_after_the_new_field_lands`.)
    ///
    /// So `gear_copies` below supersedes it and `Game::load` drains this
    /// into that. `skip_serializing_if` means a save written from here on
    /// does not contain the key at all, and since nothing sets
    /// `deny_unknown_fields`, **this field can simply be deleted a release
    /// later** — no migration, no version bump.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fused_gear: Vec<(ItemId, u32, u32)>,
    /// Every carried copy of gear that is not interchangeable with a plain
    /// one, as `(copy, qty)` — see `components::GearCopies`. Plain copies
    /// are in `inventory`, which is the plain-copy store.
    #[serde(default)]
    pub gear_copies: Vec<(GearCopySave, u32)>,
    /// Every wild program taken apart at the kill rather than paid out
    /// directly, still carried — see `components::DownedPrograms`.
    /// `DownedProgram` has no legacy shape to reconcile (unlike
    /// `GearCopySave`'s affix migration), so the save stores it directly
    /// rather than through a parallel `*Save` type. Additive behind a
    /// default, so **no `SAVE_FORMAT_VERSION` bump**: a save written before
    /// this field existed loads with an empty store, which is what it had.
    #[serde(default)]
    pub downed_programs: Vec<DownedProgram>,
    /// The abilities installed in the player's routine slots, in menu order
    /// — see `components::Routines`.
    pub routines: Vec<crate::abilities::AbilityId>,
    /// Every field buff currently running on the player — see
    /// `components::FieldBuff`. Player state, not zone-local, so
    /// `Game::enter_next_zone` must never clear it.
    pub field_buffs: Vec<ActiveFieldBuff>,
    /// Every squad currently away from the base — see `resources::Sorties`.
    ///
    /// Additive behind `#[serde(default)]`, so a save written before sorties
    /// existed loads with none and costs no `SAVE_FORMAT_VERSION` bump. Run
    /// state and not zone-local: the base travels through a breach and so
    /// does anything it has sent out, so `Game::enter_next_zone` must never
    /// clear it.
    #[serde(default)]
    pub sorties: Vec<SortieSave>,
    /// The player's own chosen name — see `components::CustomName`. Empty
    /// for a save written before character creation existed, or for a
    /// choice that named nothing: `CustomName::sanitize` treats the two the
    /// same, so `Game::load` inserts no override for either and the run
    /// reads exactly as it always did.
    #[serde(default)]
    pub name: String,
    /// The player's chosen class — see `components::PlayerIdentity`.
    /// `Game::load` writes it straight back onto `PlayerIdentity` rather
    /// than replaying `Game::apply_character_choice`: the kit and the stat
    /// spend a class implies are already receipts, folded into `inventory`
    /// and the stat fields above, and replaying the choice would double
    /// them.
    #[serde(default)]
    pub class: Option<PlayerClass>,
    /// The player's chosen glyph — `components::Glyph::ch`. Defaulted
    /// through a named function rather than `char`'s own default (`'\0'`),
    /// so a save written before character creation existed loads the `@`
    /// every player before it had.
    #[serde(default = "default_player_glyph")]
    pub glyph: char,
    /// The player's chosen sprite name — see `components::PlayerIdentity`.
    /// Defaulted through a named function for `glyph`'s reason: the map
    /// drew `assets/sprites/player.png` for every player before the wizard
    /// existed, and `String`'s own default is the empty name the renderer
    /// reads as *no sprite*.
    #[serde(default = "default_player_sprite")]
    pub sprite: String,
    /// The player's chosen colour index, 0-based — see
    /// `components::PlayerIdentity`. `#[serde(default)]` on an `Option`
    /// yields `None`, which is exactly what a save written before character
    /// creation existed means: no choice was made, so the glyph wears the
    /// `PLAYER` role colour.
    #[serde(default)]
    pub colour: Option<u8>,
    /// The player's hand-drawn map avatar — see
    /// `components::PlayerIdentity::icon`. Carried as the **encoded
    /// string**, not the struct: `icon::PlayerIcon::decode` yields `None`
    /// on anything it cannot read, so a form this build cannot parse is
    /// inert rather than a version refusal. `#[serde(default)]` on the
    /// `Option` yields `None` for a save written before this feature
    /// existed, which is exactly the glyph-only player it had.
    #[serde(default)]
    pub icon: Option<String>,
}

/// `serde`'s default for `PlayerSave::glyph` — a save written before
/// character creation existed carries no glyph at all, and every player
/// before it was `@`.
fn default_player_glyph() -> char {
    '@'
}

/// `serde`'s default for `PlayerSave::sprite` — the name the map painted
/// for every player before the wizard could choose one, and the same name
/// `CharacterChoice::default()` carries.
fn default_player_sprite() -> String {
    crate::DEFAULT_PLAYER_SPRITE.to_string()
}

/// One trip in flight.
///
/// **Carries no member list.** Entity ids are not stable across a save, so
/// membership rides `CreatureSave::sortie_index` from the creature side and
/// is reassembled on load — `party_slot`'s precedent, and it exists for
/// exactly the same reason.
///
/// A **named struct, never a positional tuple**: RON matches a tuple by
/// exact arity and refuses a widened one, which is the one shape field-named
/// RON does not save you from. The next field added here is free; on a tuple
/// it would cost a legacy field and a version bump.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SortieSave {
    /// The whole resolved site, not its id. A board that rotates while the
    /// squad is out, or an `assets/sorties/` file edited between sessions,
    /// must not be able to rewrite or strand a trip already in flight —
    /// `ActiveContract` stores a whole `ContractDef` for that reason, and
    /// this is the same rule reaching the save format.
    pub site: crate::sorties::SortieDef,
    pub risk: u32,
    pub ticks_total: u64,
    pub ticks_elapsed: u64,
    pub battles_total: u32,
    pub battles_done: u32,
    pub aborted: bool,
    pub loot: Vec<(ItemId, u32)>,
    pub xp: u32,
    pub kills: u32,
    pub casualties: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct CreatureSave {
    pub species: SpeciesId,
    pub position: (i32, i32),
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    /// Percentage points — see `PlayerSave::mitigation` for why this earned
    /// a version bump rather than a `#[serde(default)]`.
    pub mitigation: i32,
    pub tamed: bool,
    /// What this program has left to spend on routine calls — see
    /// `components::PowerReserve`. Only a companion holds one; a wild
    /// creature's is written and ignored.
    ///
    /// Defaults to full rather than to zero, so companions in a save written
    /// before reserves existed load able to run rather than mysteriously
    /// unable to. Additive behind `#[serde(default)]`, so it earns no version
    /// bump of its own — it rides the one `PlayerSave::fatigue`'s removal
    /// already spent.
    #[serde(default = "full_reserve")]
    pub power: f32,
    /// Only meaningful when `tamed` is true; wild creatures don't level.
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
    /// Only meaningful when `tamed` is true. The target structure is
    /// identified by position rather than entity id, since entity ids
    /// aren't stable across a save/load round trip.
    pub cronjob: Option<CronjobSave>,
    /// This program's index in the player's active party, or `None` if it
    /// isn't a party member. Party order is mechanically meaningful under
    /// soft ranks — front slots draw more enemy fire — so it can't be
    /// rebuilt from creature-iteration order the way it was before.
    /// Supersedes the old `is_companion` flag, which `party_slot.is_some()`
    /// now says.
    pub party_slot: Option<u32>,
    /// Which in-flight sortie this program is away on — an index into
    /// `PlayerSave::sorties`, or `None` for a program that is at the base.
    ///
    /// Written per creature rather than as a member list on the sortie side,
    /// `party_slot`'s reason: entity ids are not stable across a save/load
    /// round trip. Additive behind `#[serde(default)]`, so a save written
    /// before sorties existed loads with every program at home.
    #[serde(default)]
    pub sortie_index: Option<u32>,
    /// Whether this program is the one equipped as the player's weapon (see
    /// `resources::WieldedProgram`). Written per creature rather than as a
    /// player-side entity id for the same reason `party_slot` is: entity ids
    /// are not stable across a save/load round trip. At most one creature in
    /// a file may set it, and `Game::load` takes the first and ignores any
    /// others rather than trusting the file.
    pub wielded: bool,
    /// Which zone sector this creature was originally spawned in (see
    /// `components::ZonePortal`).
    pub zone: u32,
    /// The player's custom display name for this creature, if they set one
    /// (see `components::CustomName`) — via `Game::fuse_companions` or
    /// `Game::rename_companion`. This is a shape change to `CreatureSave`,
    /// so it required bumping `SAVE_FORMAT_VERSION` (bincode has no
    /// granular field-level compatibility here — see that constant's docs).
    pub custom_name: Option<String>,
    /// This creature's individual quality roll — see
    /// `components::Potential`. Persisted so `growth_roll` keeps applying
    /// consistently across save/load rather than resetting; `hp_roll`/
    /// `atk_roll`/`def_roll` are along for the ride purely so
    /// `Potential::quality_percent`/`quality_label` stay accurate too.
    pub hp_roll: f32,
    pub atk_roll: f32,
    pub def_roll: f32,
    pub growth_roll: f32,
    /// How many fusions deep this creature's lineage is — see
    /// `components::FusionCount`. Persisted so the `MAX_FUSIONS` ceiling
    /// survives a save/load instead of resetting to 0 and handing the
    /// player unlimited fusions for free.
    pub fusions: u32,
    /// How many percentage upgrades have been spent on this program — see
    /// `components::Refactors`. Persisted for exactly the reason `fusions`
    /// above is: `MAX_COMPANION_REFACTORS` is the only bound on a buff
    /// chain that runs off a Mining Node forever, and a count that reset to
    /// 0 on load would refill the budget on every reload.
    ///
    /// `#[serde(default)]` does nothing for the bincode save — that
    /// encoding is positional, which is why this field's arrival bumped
    /// `SAVE_FORMAT_VERSION` at all. It is here for the RON round trip that
    /// `savetool dump`/`pack` performs, where fields are named and an older
    /// dump simply won't carry the key.
    #[serde(default)]
    pub refactors: u32,
    /// How many of this program's zone tiers were bought with Recompile
    /// Kernels — see `components::PurchasedTiers`. Persisted because
    /// `Game::program_payout` divides these back out: a count that reset to 0
    /// on load would make saving and reloading launder a bought-up program
    /// into one that reads as having earned every tier, which is the Credit
    /// press this field exists to close.
    ///
    /// Landed in the same unreleased `SAVE_FORMAT_VERSION` 27 as `refactors`
    /// above, so it cost no further bump.
    #[serde(default)]
    pub purchased_tiers: u32,
    /// How many Kernel Rings are open on this program — see
    /// `components::KernelRing`. Persisted because it is the whole of what a
    /// Privilege Ring bought: a count that reset to 0 on load would drop a
    /// developed companion's ceiling back to `TALENT_START_LEVEL` while
    /// leaving the levels it already earned in place, which reads as the
    /// feature not working rather than as a lost field.
    ///
    /// Additive on a field-named RON struct, so it costs no
    /// `SAVE_FORMAT_VERSION` bump: an older file simply carries no key and
    /// loads as an undeveloped program, which it was.
    #[serde(default)]
    pub ring: u32,
    /// Which talent nodes this program has bought, in purchase order — see
    /// `components::Talents`. Ids rather than an index, so a tree edited
    /// between sessions cannot silently hand a program a different node.
    ///
    /// A **receipt**. `hp`/`max_hp`/`atk`/`def` above already carry every
    /// `Stat` node's effect, so nothing on the load path may re-apply this
    /// list — that would compound the bonus on every reload, which is the same
    /// rule `refactors` follows and the reason `Rarity`'s tag is written
    /// without its multiplier.
    ///
    /// Additive on a field-named RON struct, so no version bump: an older file
    /// carries no key and loads as a program that has bought nothing.
    #[serde(default)]
    pub talents: Vec<String>,
    /// What those talents baked into `Stats`, so `Game::respec_talents` can
    /// hand it back — see `components::BoughtStats`. Additive behind a
    /// default, like `talents` above, so no version bump.
    #[serde(default)]
    pub bought_stats: crate::components::BoughtStats,
    /// The abilities installed in this program's routine slots, in menu
    /// order — see `components::Routines`. Persisted rather than re-derived
    /// from its species, because an innate routine can be popped out and a
    /// foreign one plugged in.
    pub routines: Vec<crate::abilities::AbilityId>,
    /// Every field buff currently running on this creature — see
    /// `components::FieldBuff`. A companion sold, extracted, fused away or
    /// killed takes this with it: the entity simply despawns, and neither
    /// `Game::dissolve_tamed_program` nor `Game::fuse_companions` needs to
    /// know this field exists.
    pub field_buffs: Vec<ActiveFieldBuff>,
    /// The nest this creature is tethered to, if it's a `NestGuardian` —
    /// identified by the nest's position rather than its entity id, since
    /// entity ids aren't stable across a save/load round trip (same reason
    /// `cronjob` above resolves by position, not by `Entity`). One nest per
    /// tile, so the key is unambiguous. `None` for an ordinary wild program
    /// or a tamed one.
    ///
    /// This is a shape change to `CreatureSave`, so it required bumping
    /// `SAVE_FORMAT_VERSION` — see that constant's docs.
    pub nest_position: Option<(i32, i32)>,
    /// Whether this creature is currently `Pursuing` the player — see that
    /// component's docs. Meaningless unless `nest_position` is also `Some`.
    ///
    /// This is a shape change to `CreatureSave`, so it required bumping
    /// `SAVE_FORMAT_VERSION` — see that constant's docs.
    pub pursuing: bool,
    /// A load this program is carrying to a depot (`components::Carrying`).
    /// Only meaningful when `tamed` is true, and only a working program can
    /// hold one.
    ///
    /// The *destination* is deliberately absent: which depot is the nearest
    /// one with room is re-derived from position on the tick after the load,
    /// which is the whole reason the live feature stores no state but this.
    ///
    /// This is a shape change to `CreatureSave`, so it required bumping
    /// `SAVE_FORMAT_VERSION` — see that constant's docs.
    pub carrying: Option<(ItemId, u32)>,
    /// The rare-spawn tier this creature rolled — see `components::Rarity`.
    ///
    /// Persisted as the *tag* only. The multiplier it names was already
    /// spent into `Stats` at spawn and those numbers are saved verbatim
    /// above, so `Game::load` must restore this field without re-applying
    /// `stat_mult` — see `Rarity`'s doc for why a second application is
    /// invisible and compounds on every reload.
    ///
    /// `#[serde(default)]` does nothing for the bincode save (see
    /// `SAVE_FORMAT_VERSION`) and is here only so the field-named RON
    /// templates under `dev-saves/` keep parsing without being re-captured,
    /// matching `stock_input`/`stock_output`/`known_routines`/`trace`.
    ///
    /// This is a shape change to `CreatureSave`, so it required bumping
    /// `SAVE_FORMAT_VERSION` — see that constant's docs.
    #[serde(default)]
    pub rarity: Rarity,
    /// Whether this creature spawned as a boss — see `components::Boss`.
    ///
    /// Written for an apex species too, redundantly with its own `is_boss`
    /// flag, so the field means one thing rather than "the rolled half".
    ///
    /// Additive, named and defaulted, so this needs **no**
    /// `SAVE_FORMAT_VERSION` bump — the save has been field-named RON since
    /// v29, which is what retired migrations for exactly this shape of
    /// change. A save written before rolled bosses existed loads with every
    /// creature un-bossed, which is what it was.
    #[serde(default)]
    pub boss: bool,
    /// What this program is wearing — see `components::Equipment`. Only
    /// meaningful when `tamed` is true; nothing else may be geared.
    ///
    /// A `Vec` rather than `PlayerSave`'s nine flat fields, for one reason
    /// that decides it: a single defaulted field means an older RON dump
    /// packs with no hand-editing at all. `PlayerSave`'s existing shape is
    /// what that migration matches, so it stays as it is.
    ///
    /// This bumped `SAVE_FORMAT_VERSION` when it landed, against the
    /// positional bincode format of the time. The save is field-named RON
    /// now (`save_to_file` writes text and `load_from_file` reads it back
    /// as a string), so `#[serde(default)]` is what carries an additive
    /// field today and the same change would cost no bump.
    #[serde(default)]
    pub equipment: Vec<(EquipmentSlot, EquippedItemSave)>,
    /// How many times this program has driven the party out of a fight —
    /// see `components::Nemesis`. Zero (and no component on load) for an
    /// ordinary program.
    ///
    /// Additive and defaulted, so this needed no `SAVE_FORMAT_VERSION`
    /// bump — the save has been field-named RON since v29, which is what
    /// retired migrations for exactly this shape of change; see that
    /// constant's docs. The multiplier this count's promotions spent is
    /// already sitting in `Stats` above and the promoted `rarity` receipt
    /// above that, so `Game::load` must restore the count without
    /// re-applying `promote_rarity` — the same trap `rarity`'s own comment
    /// documents just above.
    #[serde(default)]
    pub nemesis_grudges: u32,
    /// This program's hidden temperament — see
    /// `crate::disposition::Disposition`. Only meaningful for an owned
    /// program; a wild creature's is written `None` and read back nowhere,
    /// exactly as its `memories` is.
    ///
    /// **`Option`, and the `None` is load-bearing.** It is not "no
    /// personality" — it is *this file predates dispositions*, and the load
    /// path answers it by deriving one from `program_id` through
    /// `Disposition::seed`, the same formula `roster_parts` uses for a fresh
    /// program. So an existing save gains exactly the roster it would have
    /// had, rather than a base full of neutral programs. Stored rather than
    /// derived on every read so that editing `Disposition::ALL` later cannot
    /// silently reshuffle a personality the player has already learned to
    /// work around.
    ///
    /// Additive on a field-named RON struct, so it costs no
    /// `SAVE_FORMAT_VERSION` bump.
    #[serde(default)]
    pub disposition: Option<crate::disposition::Disposition>,
    /// This program's stable identity — see `components::ProgramId`. Only
    /// meaningful when `tamed` is true; a wild creature's is written as the
    /// sentinel and ignored.
    ///
    /// `0` is that sentinel, which is exactly what a file written before
    /// this field existed defaults to: `Game::load` mints a fresh id for
    /// every owned program carrying it, and sets the counter past the
    /// highest id it saw. Additive behind `#[serde(default)]`, so it earns
    /// no `SAVE_FORMAT_VERSION` bump — nothing is removed and no field
    /// changes meaning under a name it keeps.
    #[serde(default)]
    pub program_id: u32,
    /// What this program remembers — see `components::Memories`. Only
    /// meaningful when `tamed` is true; a wild creature's is written empty
    /// and ignored, exactly as its `power` is.
    ///
    /// Additive behind `#[serde(default)]`, so it earns no
    /// `SAVE_FORMAT_VERSION` bump — nothing is removed and no field changes
    /// meaning under a name it keeps. The default is load-bearing rather than
    /// decorative here: the on-disk form is field-named RON, so a file
    /// written before memories existed carries no `memories` key at all and
    /// every program in it loads with an empty store.
    #[serde(default)]
    pub memories: Vec<MemorySave>,
    /// Where this program's need reserves stand — see `components::Needs`.
    /// Only meaningful when `tamed` is true, exactly as `memories` is.
    ///
    /// Additive behind `#[serde(default)]`, so no `SAVE_FORMAT_VERSION` bump.
    /// A file written before needs existed carries no key, loads empty, and
    /// `needs_drain_system` seeds it full on the first tick — a program is not
    /// punished for a reload. `Needs::stalled_announced` is deliberately
    /// **not** here: a reload should say the complaint again.
    #[serde(default)]
    pub needs: std::collections::BTreeMap<crate::needs::NeedId, f32>,
    /// Which need has this program off its post, if any — see
    /// `components::OffShift`. The one piece of this feature's state that is
    /// not derived, because it is hysteresis: reloaded without it, a program
    /// mid-errand at `critical + 1` would be judged content and sent straight
    /// back to work, having fixed nothing.
    ///
    /// Additive behind `#[serde(default)]`, so no `SAVE_FORMAT_VERSION` bump.
    #[serde(default)]
    pub off_shift: Option<crate::needs::NeedId>,
    /// Whether this program has downed tools — see
    /// `components::Disgruntled`.
    ///
    /// Saved for `off_shift`'s reason: the marker *is* the hysteresis, so a
    /// reload that dropped it would put a program back to work the moment
    /// the player looked away, at a morale that had not moved. Additive
    /// behind `#[serde(default)]`, so no `SAVE_FORMAT_VERSION` bump.
    #[serde(default)]
    pub disgruntled: Option<crate::components::Grievance>,
    /// Whether this program was on the base staff — see `ProgramRole`. Only
    /// meaningful when `tamed` is true.
    ///
    /// **Written and never read back.** The role is derived from the party
    /// and the wield, both of which `Game::load` already rebuilds, so there
    /// is nothing here to restore and a file claiming otherwise cannot make
    /// the two disagree. `Experience::xp_to_next` is the same shape and for
    /// the same reason: the field stays written because *removing* one is
    /// what earns a `SAVE_FORMAT_VERSION` bump, and this change earns none.
    #[serde(default)]
    pub staff: bool,
    /// Whether this program was benched by a Forgiving death — see
    /// `components::Downed`. Only meaningful when `tamed` is true; nothing
    /// else can be downed.
    ///
    /// Additive behind `#[serde(default)]`, so it earns no
    /// `SAVE_FORMAT_VERSION` bump — the save has been field-named RON since
    /// v29, and a file written before this feature carries no key and loads
    /// with every program upright, which is what it was. Unlike `staff`
    /// above this **is** read back: the state is stored precisely because it
    /// cannot be derived, and a reload that quietly healed a wipe would be
    /// the feature not working.
    #[serde(default)]
    pub downed: bool,
}

/// A worn item on disk. Deliberately **not** `components::EquippedItem`,
/// which it used to be.
///
/// That component now nests its `items::GearCopy` (so a bonus cannot be
/// applied and un-applied from different property sets), and nesting is a
/// shape change RON cannot absorb: a v29 row reads
/// `(item: "arc_lance", level: 2, fusion_tier: 1)`, and against the nested
/// form the required `copy` field is simply absent, which is a load failure
/// rather than a defaulted field. Keeping the *save's* shape flat makes
/// rarity an ordinary additive field here, and costs one conversion each
/// way in `Game::save`/`Game::load`.
///
/// This is also what `PlayerSave` has always done — its equipment is nine
/// flat fields rather than the component — so the two halves of the save
/// now agree rather than one of them shadowing a live type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquippedItemSave {
    pub item: ItemId,
    pub level: u32,
    pub fusion_tier: u32,
    /// The rare tier of the copy worn — see `items::GearCopy`. Additive
    /// behind `#[serde(default)]`, so a save written before gear had rare
    /// tiers loads with every worn copy ordinary, which is what it was.
    #[serde(default)]
    pub rarity: Rarity,
    /// **Legacy, load-only.** The one affix a worn copy could carry before
    /// affixes stacked, lifted into `affixes` by `affixes_from_save`.
    #[serde(default)]
    pub affix: Option<AffixId>,
    /// Every affix on the worn copy — see `items::GearCopy::affixes`.
    /// Additive for the same reason `rarity` is.
    #[serde(default)]
    pub affixes: Vec<AffixId>,
    /// How well the worn copy was compiled — see `items::GearCopy::quality`.
    /// Additive behind a default of `QUALITY_DEFAULT` rather than `u8`'s own
    /// `Default` of 0, which would silently strip a worn item of its whole
    /// bonus on the first reload.
    #[serde(default = "default_worn_quality")]
    pub quality: u8,
}

/// `serde`'s default for every worn copy's quality — the four flat save
/// fields that stand in for a nested `GearCopy`.
fn default_worn_quality() -> u8 {
    crate::tuning::QUALITY_DEFAULT
}

/// A carried copy of gear on disk — `items::GearCopy`'s save shape, flat and
/// field-named with the same field names plus the pre-stacking `affix`.
///
/// It exists so the compatibility shim lives entirely on the save side.
/// A legacy field on `GearCopy` itself would join its `Eq` and split the
/// three `==`-keyed stores that type exists to hold together, which is the
/// failure its own doc comment is written to prevent.
///
/// RON absorbs the widening because the tuple's first element is still a
/// field-named struct with the same field names and one new defaulted field
/// — `EquippedItemSave`'s own trick, applied for its own stated reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GearCopySave {
    pub item: ItemId,
    #[serde(default)]
    pub rarity: Rarity,
    #[serde(default)]
    pub tier: u32,
    /// **Legacy, load-only** — see `EquippedItemSave::affix`.
    #[serde(default)]
    pub affix: Option<AffixId>,
    #[serde(default)]
    pub affixes: Vec<AffixId>,
    #[serde(default = "default_worn_quality")]
    pub quality: u8,
}

/// A save's affixes, taking the list when it has one and lifting the
/// pre-stacking singular field when it does not.
///
/// Load-only: the write side fills `affixes` and leaves `affix` empty. One
/// shared helper because four save sites do this lift — the three flat
/// `PlayerSave` slots, `EquippedItemSave` and `GearCopySave` — and four
/// hand-written copies is four chances for one to drop the legacy field and
/// strip an affix off a reloaded save in silence.
pub fn affixes_from_save(affix: Option<AffixId>, affixes: Vec<AffixId>) -> Vec<AffixId> {
    if affixes.is_empty() {
        affix.into_iter().collect()
    } else {
        affixes
    }
}

/// A nest's state on disk: its species, position, remaining `Durability`,
/// and any guardians still queued to respawn. Guardians tethered to it are
/// *not* stored here — each is its own `CreatureSave` carrying
/// `nest_position`, reconnected to the restored nest by tile on load.
#[derive(Serialize, Deserialize)]
pub struct NestSave {
    pub species: SpeciesId,
    pub position: (i32, i32),
    pub durability: u32,
    /// See `components::Nest::pending_respawns`.
    pub pending_respawns: Vec<u32>,
}

/// A cell of base-space rock the player has started on — see
/// `components::DigSite`, whose `Durability` and mark this carries.
///
/// `announced_stuck` is deliberately absent: it is a crew's "I already said
/// so", true of a conversation rather than of the world, and a reload is
/// exactly when the player should be told again.
#[derive(Serialize, Deserialize, Clone)]
pub struct DigSiteSave {
    /// **Base-space** coordinates, not a tile on the zone surface.
    pub position: (i32, i32),
    pub durability: u32,
    pub marked: bool,
}

/// A structure the player has asked for that the base has not raised yet —
/// see `components::BuildSite`.
///
/// A **named struct, never a positional tuple**, per the standing rule.
///
/// **`delivered` is the load-bearing field.** Those units left their
/// shelves when a builder picked them up and are physically standing on the
/// cell; dropped from the save they would be destroyed by a reload, and the
/// crew would fetch them a second time out of a base that no longer has
/// them. `progress` is saved beside it for the same reason a `DigSite`'s
/// `Durability` is — a part-raised structure is work already done.
///
/// The two announcement latches are deliberately absent, `DigSiteSave`'s
/// rule: they are a crew's "I already said so", true of a conversation
/// rather than of the world, and a reload is exactly when the player should
/// be told again.
///
/// `required` is absent too, and that is `BuildSite::required_ticks`'
/// derived-never-stored argument reaching the save: the meter is priced off
/// `cost`, which is here, so a saved figure could only ever disagree with
/// the bill of materials beside it after a retune.
#[derive(Serialize, Deserialize, Clone)]
pub struct BuildSiteSave {
    /// **Base-space** coordinates, not a tile on the zone surface.
    pub position: (i32, i32),
    pub structure: crate::structures::StructureId,
    /// The bill of materials as it was priced when the request was filed —
    /// see `components::BuildSite::cost` for why it is carried rather than
    /// re-read from the `StructureDef` on load.
    pub cost: Vec<(crate::items::ItemId, u32)>,
    pub delivered: Vec<(crate::items::ItemId, u32)>,
    pub progress: u32,
    /// Whether raising this stands a new structure up or advances the one
    /// already on the cell a tier — see `components::BuildGoal`.
    ///
    /// Additive behind `#[serde(default)]`, so it costs no
    /// `SAVE_FORMAT_VERSION` bump: a file written before upgrades were
    /// filed loads every site as `New`, which is exactly what that run had.
    #[serde(default)]
    pub goal: crate::components::BuildGoal,
}

/// A caravan mid-journey — see `components::Caravan`.
///
/// A **named struct, never a positional tuple**, per the standing rule: a
/// tuple is the one shape field-named RON does not save you from.
///
/// What is *not* here is the point. The trader's identity, its shelf and the
/// ticks it arrived and departs on are all derived from `visit` and the
/// base's own seed, so writing any of them would be a second source of truth
/// that a retune could put out of step with the first.
///
/// `position` is in **whichever space the stage says**, exactly as the
/// `Position` component it comes from is — see `CaravanStage::in_base_space`.
#[derive(Serialize, Deserialize, Clone)]
pub struct CaravanSave {
    pub position: (i32, i32),
    pub stage: crate::components::CaravanStage,
    pub visit: u64,
    pub arrival_tile: (i32, i32),
    pub stage_ticks: u32,
}

/// Which of the visiting caravan's rows have been bought — see
/// `resources::CaravanMemory`.
///
/// A **named struct, never a positional tuple**, per the standing rule. A
/// pair is exactly the shape that reads fine today and arrives as a legacy
/// positional field the moment a third thing is worth recording about a
/// visit.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CaravanMemorySave {
    pub visit: Option<u64>,
    pub bought: Vec<usize>,
}

/// One remembered thing on disk — see `components::Memory`.
///
/// A **named struct, never a positional tuple**. A tuple is the one shape
/// field-named RON does not save you from: the next property a memory grows
/// would arrive as a legacy positional field rather than as a defaulted named
/// one.
///
/// What the memory is *worth* is deliberately absent. Intensity is derived
/// from the game clock at read time, so there is nothing here that could come
/// back out of step with the tick it is measured against.
///
/// `subject` is `components::MemorySubject` itself rather than a mirror of it,
/// which is the opposite call from `CronjobKind` below — see that enum's
/// doc comment there and `MemorySubject`'s own for why a mirror is right for
/// one and wrong for the other.
#[derive(Serialize, Deserialize)]
pub struct MemorySave {
    pub def: crate::memories::MemoryId,
    pub subject: crate::components::MemorySubject,
    pub subject_name: Option<String>,
    pub reinforced: u64,
    pub strikes: u32,
}

/// Mirrors `components::TaskKind` for persistence — kept separate so the
/// engine-internal enum doesn't need to derive `Serialize`/`Deserialize`.
#[derive(Serialize, Deserialize, Default, Clone, Copy)]
pub enum CronjobKind {
    #[default]
    GatherResource,
    Guard,
}

/// An in-progress work assignment (a "cronjob") a tamed creature is running
/// against a structure, persisted so it survives save/load instead of
/// silently dropping the worker's progress.
#[derive(Serialize, Deserialize)]
pub struct CronjobSave {
    pub target_position: (i32, i32),
    pub progress: u32,
    pub required: u32,
    pub kind: CronjobKind,
}

#[derive(Serialize, Deserialize)]
pub struct StructureSave {
    pub kind: String,
    pub position: (i32, i32),
    /// Current raid durability — see `components::Durability`.
    pub durability: Option<u32>,
    /// Current upgrade tier — see `components::StructureTier`. `None` for a
    /// structure whose def declares no upgrade path.
    pub tier: Option<u32>,
    /// This structure's local buffers — see `components::Stock`. Both are
    /// live player state: `stock_input` is a batch a machine has already
    /// pulled from its neighbours and not yet spent, and `stock_output` is
    /// finished goods nobody has collected. Losing either across a save
    /// would refund or void whatever the base produced while unattended.
    ///
    /// Vec-of-pairs on disk rather than the live `BTreeMap`, matching how
    /// `build_cost` is already encoded, and rebuilt into the map on restore.
    /// `Stock::capacity` is *not* here: it is a property of the def, so a
    /// modder retuning a structure's buffer size should see it apply to the
    /// ones already standing.
    ///
    /// `#[serde(default)]` does nothing for bincode (see
    /// `SAVE_FORMAT_VERSION`) — it is here for the field-named RON that
    /// `dev-saves/` templates are written in, so an existing template keeps
    /// parsing instead of needing re-capture.
    #[serde(default)]
    pub stock_input: Vec<(ItemId, u32)>,
    #[serde(default)]
    pub stock_output: Vec<(ItemId, u32)>,
    /// The two halves of `components::StandingJob` — keep this machine
    /// worked, and keep this structure guarded, whether or not an order
    /// asks for it.
    ///
    /// Two flat bools rather than the component, matching how
    /// `EquippedItemSave` keeps a worn copy flat: a nested struct is a
    /// shape change RON cannot absorb, while a defaulted bool is free.
    /// Additive and defaulted, so no `SAVE_FORMAT_VERSION` bump.
    #[serde(default)]
    pub standing_work: bool,
    #[serde(default)]
    pub standing_guard: bool,
    /// Ticks of charge left on a supplier that burns Power Cells to stay on
    /// the grid — see `components::PowerFuel`. Live state rather than
    /// something the next tick recomputes: losing it would refuel every
    /// supplier in the base on each reload.
    ///
    /// Defaulted to a **full** charge rather than `u32`'s zero, `quality`'s
    /// reason one field family over: a save written before suppliers burned
    /// anything would otherwise load with its whole grid dry and the base
    /// dark on the first tick. Additive behind a default, so no
    /// `SAVE_FORMAT_VERSION` bump.
    #[serde(default = "default_power_fuel")]
    pub power_fuel: u32,
}

/// `serde`'s default for a supplier's remaining charge — a full one, so a
/// save written before `StructureSave::power_fuel` existed loads fuelled.
fn default_power_fuel() -> u32 {
    crate::tuning::POWER_UPKEEP_TICKS
}

/// One trading post's buyback shelf on disk: the trader kind and tile that
/// key it, then what is on it — see `SaveData::buyback_shelves`.
pub type BuybackShelfSave = (
    crate::structures::StructureId,
    (i32, i32),
    Vec<(GearCopySave, u32)>,
);

/// The pre-0.8.9 shelf shape, whose rows are positional `(item, tier, qty)`
/// triples. Read-only, and superseded by `BuybackShelfSave` for exactly the
/// reason `PlayerSave::fused_gear` is — see that field's doc for the
/// measured RON behaviour that forces a new field rather than a widened
/// one. The two go away together.
pub type LegacyBuybackShelfSave = (
    crate::structures::StructureId,
    (i32, i32),
    Vec<(ItemId, u32, u32)>,
);

/// Only the world seed and the sparse tile overlay are persisted; unmodified
/// terrain regenerates deterministically from the seed on load.
#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub seed: u32,
    pub tick: u64,
    pub difficulty: DifficultyMode,
    /// Why this run ended, if it has — `resources::GameOver::reason`, which
    /// is written in exactly one place, `difficulty::death_handling_system`'s
    /// Permadeath arm. `Game::load` refuses a save carrying one.
    ///
    /// This is the whole of what makes Permadeath permanent. `GameOver` is a
    /// resource and was persisted nowhere, so a flatline left the slot
    /// holding the last autosave — at most `AUTOSAVE_INTERVAL_TICKS` before
    /// the death, and with no record of it. The mode cost a harsher
    /// `end_battle` and bought nothing the player could not undo from the
    /// main menu.
    ///
    /// The reason rather than a bare flag, because the load list and the
    /// refusal both want to say *what* ended, and `history_summary` already
    /// proves that string is the sentence to say it with.
    ///
    /// Additive behind `#[serde(default)]`, so it costs no
    /// `SAVE_FORMAT_VERSION` bump and no `dev-saves/` recapture: a file
    /// written before it existed loads as a run still going, which is
    /// exactly what that run was. `savetool` reads and writes it like any
    /// other field, so a developer can still dump, clear and pack a dead
    /// run back to life — the refusal is the game's, not the format's.
    #[serde(default)]
    pub game_over: Option<String>,
    pub player: PlayerSave,
    pub creatures: Vec<CreatureSave>,
    pub structures: Vec<StructureSave>,
    /// Every nest standing in the zone — see `components::Nest`. Without
    /// this, a save/reload silently deleted every nest: a free way out of
    /// a swarm the player provoked, and a way to launder a nest destroyed
    /// most of the way to its cache.
    pub nests: Vec<NestSave>,
    /// Every wall the player has started cutting or marked — see
    /// `components::DigSite`. Without it a half-cut wall heals on reload and
    /// a drawn plan is lost, both of which a player meets in their first
    /// session with a base.
    ///
    /// Additive behind `#[serde(default)]`, so it costs no
    /// `SAVE_FORMAT_VERSION` bump and no `dev-saves/` recapture: a file
    /// written before it existed loads with no dig sites, which is exactly
    /// what that run had.
    #[serde(default)]
    pub dig_sites: Vec<DigSiteSave>,
    /// Structures on order and not yet raised — see
    /// `components::BuildSite`.
    ///
    /// Additive behind `#[serde(default)]`, so it costs no
    /// `SAVE_FORMAT_VERSION` bump and no `dev-saves/` recapture: a file
    /// written before it existed loads with nothing on order, which is
    /// exactly what that run had.
    #[serde(default)]
    pub build_sites: Vec<BuildSiteSave>,
    /// The caravan standing in the sector or in the base, if one is —
    /// see `components::Caravan`. Additive behind `#[serde(default)]`, so it
    /// costs no `SAVE_FORMAT_VERSION` bump: a file written before it existed
    /// loads with no caravan, which is exactly what that run had.
    #[serde(default)]
    pub caravans: Vec<CaravanSave>,
    /// Which of the visiting caravan's rows have been bought — see
    /// `resources::CaravanMemory`. Additive behind `#[serde(default)]`, so it
    /// costs no `SAVE_FORMAT_VERSION` bump: a file written before it existed
    /// loads with an empty memory under visit 0, which no live visit matches.
    #[serde(default)]
    pub caravan_memory: CaravanMemorySave,
    pub tile_overrides: Vec<((i32, i32), Tile)>,
    /// The base's pocket-dimension coordinate space — see
    /// `base_grid::BaseGrid`. Saved wholesale, the same way `stack_memory`
    /// below is: no seed can reproduce what the player mined and floored, so
    /// unlike zone terrain there is nothing to regenerate it from.
    ///
    /// Supersedes `claimed_tiles`, which named ground the base owned but
    /// carried no state of its own — a claim leaves no entity behind, and
    /// `tile_overrides` only carried the *floor* it stamped, not the fact
    /// that the base owned it. `BaseGrid` needs neither workaround: base
    /// space is its own coordinate system, so a floored cell already *is*
    /// the record of ownership.
    ///
    /// `#[serde(default)]` does nothing for the *real* save file — a v31
    /// file never reaches this deserializer, refused by the version line in
    /// `load_from_file` before RON is even parsed — but it is here for the
    /// field-named RON that `dev-saves/` templates are written in, matching
    /// `known_routines`/`trace`/`rarity`: an existing template loads with an
    /// empty (fully solid) base rather than needing a hand recapture.
    #[serde(default)]
    pub base_grid: crate::base_grid::BaseGrid,
    /// Whether the player's own bump into rock cuts it —
    /// `resources::MiningMode`.
    ///
    /// Additive behind a default, so a save written before the toggle
    /// existed loads with mining **off**. That is the safe reading twice
    /// over: it is the new default, and a save that never expressed a
    /// preference should not arm a tool that destroys terrain.
    #[serde(default)]
    pub mining: bool,
    /// Which `StructureDef::first_free` structures this run has already had
    /// for nothing — see `resources::FreeBuilds`.
    ///
    /// Additive behind a default, so a save written before the waiver
    /// existed loads with its freebie unspent. That is the generous reading
    /// and the only one available: nothing in an older file records whether
    /// a Broker already standing in it was paid for, and a save that cannot
    /// answer should not be charged twice.
    #[serde(default)]
    pub free_builds: crate::resources::FreeBuilds,
    /// The anchor's tile on the zone surface — see `components::BaseAnchor`.
    /// Persisted rather than derived from `spawn_point` on load: the two
    /// agree today (the anchor is auto-placed at each zone's spawn point and
    /// re-placed there on every breach) but nothing enforces that they must,
    /// and deriving one from the other is exactly the kind of shortcut whose
    /// bug would be invisible against a spawn point that is usually
    /// `(0, 0)` in a test fixture.
    ///
    /// `#[serde(default)]` is the whole compatibility story since v29 (see
    /// that constant's docs): a file written before this field existed —
    /// every `dev-saves/` template as of this change — loads it as `None`,
    /// and `Game::load` falls back to `spawn_point` for that one case. It
    /// does not earn `SAVE_FORMAT_VERSION` a second bump in this slice: an
    /// *added* field behind a default is additive, unlike `base_grid`
    /// replacing `claimed_tiles` above, which is a removal and already spent
    /// the only bump this migration needs.
    #[serde(default)]
    pub anchor: Option<(i32, i32)>,
    /// Which zone sector the player had breached into.
    pub zone: u32,
    /// Where the player materialized on breaching into that zone — see
    /// `resources::ZoneSpawnPoint`.
    pub spawn_point: (i32, i32),
    /// Each trading post's buyback shelf — see `resources::BuybackLedger`,
    /// whose `BTreeMap` this is the flattened, key-ordered form of. Not part
    /// of `StructureSave` because a shelf outlives its building and can sit
    /// on a tile holding nothing at all.
    ///
    /// **Legacy, read-only, drained by `Game::load` into `buyback_shelves`.**
    /// See `PlayerSave::fused_gear`, which is the same situation and carries
    /// the reasoning; these two fields are the whole cost of gear rarity
    /// landing without a save-format bump, and both can be deleted in one
    /// later release with no migration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buyback: Vec<LegacyBuybackShelfSave>,
    /// Every trading post's buyback shelf — see `resources::BuybackLedger`.
    #[serde(default)]
    pub buyback_shelves: Vec<BuybackShelfSave>,
    /// Which research nodes have been unlocked — see `research::ResearchDb`.
    /// Sorted on write so the encoded bytes don't depend on `HashSet`
    /// iteration order.
    pub researched: Vec<crate::research::ResearchId>,
    /// Which routines the player has learned — see `resources::KnownRoutines`.
    /// Sorted on write for the reason `researched` is: the encoded bytes must
    /// not depend on set iteration order.
    ///
    /// `#[serde(default)]` does nothing for bincode (see
    /// `SAVE_FORMAT_VERSION`); it is here so an existing `dev-saves/` RON
    /// template keeps parsing without being re-captured, exactly as `trace`
    /// documents below.
    #[serde(default)]
    pub known_routines: Vec<crate::abilities::AbilityId>,
    /// Every Stack entrance standing on the zone map — see
    /// `components::SurfaceLink`. Only the tile: an entrance carries no
    /// state of its own, and which stack it opens onto is a pure function
    /// of the world seed and the depth walked to.
    pub link_sites: Vec<(i32, i32)>,
    /// Whether the player was on the surface or down the Stack, and where —
    /// see `resources::Locale`. The frame itself is *not* here: it
    /// regenerates from `seed` and the saved depth, exactly as terrain
    /// regenerates from `seed` alone.
    pub locale: crate::resources::Locale,
    /// What the party has learned about each Stack frame walked in this
    /// zone — see `resources::StackMemory`. The one piece of Stack state
    /// that is saved rather than regenerated: a frame is a pure function of
    /// its spec, but which parts of it the player has *seen* is history.
    pub stack_memory: crate::resources::StackMemory,
    /// Which world chunks of this zone have had their wild population
    /// placed — see `resources::PopulatedChunks`. Saved rather than derived
    /// because it is history, not geometry: the chunks a run has stocked
    /// depend on where that run walked. Without it a reload would re-stock
    /// every chunk the player had already cleared out.
    ///
    /// `#[serde(default)]` earns its keep here — an older save simply
    /// carries no marks, so the ground around wherever it left the player is
    /// stocked once on load and the rest arrives as they travel.
    #[serde(default)]
    pub populated_chunks: crate::resources::PopulatedChunks,
    /// How loud the party has been in the stack they are currently in — see
    /// `resources::Trace`. Zero whenever `locale` is `Surface`, since
    /// `Game::clear_stack` is the one place it resets.
    ///
    /// Persisted because without it, saving mid-dive would be a free Trace
    /// reset — a worse exploit than any the meter creates.
    ///
    /// `#[serde(default)]` does nothing for bincode, which is positional and
    /// covered by the version bump. It is here for the field-named RON that
    /// `dev-saves/` templates are written in: see
    /// `crates/launcher/src/dev_template.rs`, which documents that a new
    /// defaulted field is exactly what lets an existing template keep
    /// parsing instead of needing re-capture.
    #[serde(default)]
    pub trace: u32,
    /// Contracts the run is holding, with their progress — see
    /// `resources::ActiveContracts`. Each carries the whole resolved
    /// `ContractDef`, so a contract whose asset file has since been edited or
    /// deleted still finishes and still pays exactly as it read when it was
    /// accepted.
    ///
    /// `#[serde(default)]` is the whole compatibility story since v29: the
    /// payload is field-named RON, so a file written before this field
    /// existed loads it as empty and costs no `SAVE_FORMAT_VERSION` bump.
    #[serde(default)]
    pub contracts: Vec<crate::resources::ActiveContract>,
    /// Which contracts this run has finished, so a non-repeatable one is not
    /// offered again. Ids rather than defs: nothing needs to read a finished
    /// contract's terms back.
    #[serde(default)]
    pub contracts_done: Vec<crate::contracts::ContractId>,
    /// What the base has been told to hold — see `resources::WorkOrders`.
    ///
    /// The whole of the feature's state. Which machines a line needs, who
    /// stands where and how far along it is are all derived from live world
    /// state each tick, so none of that is here and none of it can go stale
    /// against a base that has since been rebuilt.
    ///
    /// `#[serde(default)]` is the whole compatibility story since v29: a
    /// file written before this field existed loads it as empty and costs no
    /// `SAVE_FORMAT_VERSION` bump.
    #[serde(default)]
    pub work_orders: Vec<crate::game::base::work_orders::WorkOrder>,
    /// The next `components::ProgramId` to hand out — see
    /// `resources::NextProgramId`. Without it a reload would reissue ids
    /// already spent, and two programs would answer to one name.
    ///
    /// `Game::load` takes the greater of this and one past the highest
    /// `CreatureSave::program_id` in the file, so a hand-edited or
    /// savetool-packed save that carries ids this counter never saw still
    /// loads safely.
    ///
    /// `#[serde(default)]`, so a file written before this field existed
    /// loads it as `0` and costs no `SAVE_FORMAT_VERSION` bump — that
    /// legacy `0` is exactly the case the `.max` above answers.
    #[serde(default)]
    pub next_program_id: u32,
    /// What the base has produced and consumed — see
    /// `base_ledger::BaseLedger`. History, not derivable: nothing else in
    /// the file records a cycle that has already happened, and the whole
    /// point of the counters is that they outlive the machine that filled
    /// them.
    ///
    /// `#[serde(default)]`, so a file written before this field existed
    /// loads with an empty ledger and costs no `SAVE_FORMAT_VERSION` bump.
    /// That is the truthful reading: a run recorded before the ledger
    /// existed genuinely has no history to show.
    ///
    /// **Do not read the "that is why it required bumping
    /// `SAVE_FORMAT_VERSION`" sentences elsewhere in this file as applying
    /// here.** Those are historical and describe changes made against the
    /// positional bincode format that pre-dates 0.8.0; the payload has been
    /// field-named RON since, and an added field behind a default is
    /// additive.
    #[serde(default)]
    pub base_ledger: crate::base_ledger::BaseLedger,
}

/// Bumped whenever `SaveData` (or anything it contains, transitively)
/// changes shape in *any* way — a field added/removed/reordered, an enum
/// gaining a variant, all of it.
///
/// bincode encodes everything *positionally*: it has no field names or
/// self-describing structure on disk, so a struct is really just "decode
/// exactly `fields.len()` values in order," where `fields.len()` is
/// whatever the *current* type definition says. serde's `#[serde(default)]`
/// (which genuinely works for the RON-based species/structure asset files,
/// since RON *is* self-describing) does **not** give bincode saves any
/// backward compatibility: an old file missing a newly-added field doesn't
/// decode that field as its default, it desyncs every byte read after that
/// point and produces garbage — which usually doesn't fail until some much
/// later, unrelated field happens to decode into a nonsense enum
/// discriminant. That's a footgun this project hit directly: several
/// fields below used to carry `#[serde(default = ...)]` on the assumption
/// that it made old saves keep loading, and it silently didn't.
///
/// The fix is this version prefix (see `save_to_file`/`load_from_file`): a
/// save written by a different version is rejected up front with a clear
/// error, instead of decoded into corruption. There is no partial/granular
/// compatibility — any shape change at all means bumping this constant,
/// and every save written under the old version stops loading. That's an
/// intentional, simple tradeoff for a single-player game rather than
/// building real schema migration.
/// 19 → 20: `StructureSave` gained `stock_input`/`stock_output` for the
/// production-chain buffers (`components::Stock`).
/// 20 → 21: `known_routines` — a routine is knowledge plus a Routine Disk
/// rather than an item, so what the player can install is now save state
/// instead of whatever `routine_*` items happened to be in cargo.
/// 21 → 22: `ActiveFieldBuff` gained `interval`, so a running buff carries
/// the cadence it fires on rather than firing every turn.
/// 22 → 23: `CreatureSave` gained `wielded`, for the tamed program equipped
/// as the player's weapon (`resources::WieldedProgram`).
/// 23 → 24: `CreatureSave` gained `carrying`, for a program mid-delivery to
/// a depot (`components::Carrying`).
/// 24 → 25: gear fuses per physical copy, so `PlayerSave` carries
/// `GearCopies`'s `(item, tier, qty)` rows and every entry point naming an
/// item names a tier beside it. Backfilled — this bump shipped undocumented.
/// 25 → 26: `CreatureSave` gained `rarity`, the rare-spawn tier
/// (`components::Rarity`).
/// 26 → 27: `CreatureSave` gained `refactors` and `purchased_tiers`, the
/// spent companion upgrade slots and the zone tiers bought with Recompile
/// Kernels (`components::Refactors`, `components::PurchasedTiers`).
/// 27 → 28: `CreatureSave` gained `equipment` — any program the player owns
/// can now wear the three gear slots the player wears, so a loadout is per
/// creature rather than player-only.
/// 28 → 29: the encoding stopped being positional. Every bump listed above
/// is a struct *gaining* a field — nine in a row, no removals, no changed
/// meanings — and bincode is the only reason any of them broke a save. The
/// payload is now the same field-named RON `savetool dump` prints, so a
/// field added behind `#[serde(default)]` loads out of a file written
/// before it existed and needs no bump at all. **This is the last entry
/// this list should need for an additive change.** What still earns one is
/// a field removed, or one whose meaning changes under a name it keeps —
/// and that needs real migration code, which no encoding could have saved
/// you from.
/// 29 → 30: `PlayerSave::fatigue` removed. The Fatigue meter is gone — one
/// need, Power, is now the budget every routine call draws on — and this is
/// the first bump of the kind the entry above describes: a field *removed*,
/// which field-named RON does not save you from. Kept-and-ignored was the
/// alternative and costs the same bump on the next property while leaving a
/// lie in the struct.
/// 30 → 31: `Stats::def` became `Stats::mitigation` and changed *unit* —
/// subtractive absorption became percentage points. This is the second kind
/// the entry above describes: not a field removed but one whose meaning
/// changed, which field-named RON cannot rescue because the name is what it
/// matches on. A v30 file would load `def: 6` into a percentage slot and
/// read as 6% mitigation rather than 6 points of soak, so it is refused by
/// version instead. `FieldBuffKind::Mitigation` folding into `Mitigation` rides the
/// same bump.
/// 31 → 32: `claimed_tiles` removed, `base_grid` added — the base is
/// leaving the zone surface for its own pocket-dimension coordinate space
/// (`base_grid::BaseGrid`; see TODO #36, "the base, out of phase"). A field
/// *removed* is exactly the case field-named RON does not excuse from a
/// bump, the same reasoning `fatigue`'s removal at 30 spent. This is the
/// only bump the whole migration needs: later tasks in the same slice add
/// further save fields (an anchor's position, among them) behind
/// `#[serde(default)]`, which is additive and free.
pub const SAVE_FORMAT_VERSION: u32 = 32;

/// `CreatureSave::power`'s serde default — see that field.
fn full_reserve() -> f32 {
    crate::components::POWER_MAX
}

/// Renders a save as editable RON, for the `savetool` binary.
///
/// The on-disk form is field-named RON too, so this is a pretty-printer
/// rather than a decoder — what it buys is the `savetool` round trip and
/// the guarantee that an edited dump packs back to the same save.
/// `a_save_survives_a_round_trip_through_ron_unchanged` pins that.
pub fn to_ron(data: &SaveData) -> io::Result<String> {
    ron::ser::to_string_pretty(data, ron::ser::PrettyConfig::default())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Parses the RON produced by `to_ron` back into a save.
pub fn from_ron(text: &str) -> io::Result<SaveData> {
    ron::from_str(text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Writes the version as the first line and the save as the same
/// field-named RON `to_ron` produces.
///
/// The version is a line rather than a struct field so it can be read
/// without parsing — a file this build cannot understand is refused *by
/// version*, which is a sentence a player can act on, instead of by a parse
/// error about a byte offset. It is a line rather than the 4-byte binary
/// prefix it used to be so the whole file stays hand-editable in a text
/// editor, which an encoding this legible may as well allow.
pub fn save_to_file(path: &Path, data: &SaveData) -> io::Result<()> {
    let text = to_ron(data)?;
    std::fs::write(path, format!("{SAVE_FORMAT_VERSION}\n{text}"))
}

pub fn load_from_file(path: &Path) -> io::Result<SaveData> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        // A save written by a build older than 0.8.0 is bincode, so it is not
        // valid UTF-8 and never reaches the version line below. It is the same
        // refusal — this build cannot read that file — and saying so is the
        // difference between a player deleting it and a player filing a bug
        // about invalid UTF-8.
        if e.kind() == io::ErrorKind::InvalidData {
            return io::Error::new(io::ErrorKind::InvalidData, OLD_FORMAT_REFUSAL);
        }
        e
    })?;
    let Some((version_line, payload)) = text.split_once('\n') else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "save file is too short to be valid",
        ));
    };
    let Ok(version) = version_line.trim().parse::<u32>() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            OLD_FORMAT_REFUSAL,
        ));
    };
    if version != SAVE_FORMAT_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "incompatible save version (v{version}, this build reads v{SAVE_FORMAT_VERSION}) — \
                 delete it and start a new game"
            ),
        ));
    }
    from_ron(payload)
}

/// Both routes to "this file predates the text format", worded once. A
/// bincode save fails as invalid UTF-8 if its bytes happen to be malformed
/// and as an unparseable version line if they happen not to be, and a player
/// should not get two different sentences for one situation.
const OLD_FORMAT_REFUSAL: &str = "this save was written by an older version of the game and can't \
                                  be read — delete it and start a new game";

/// Minimal nod to Dwarf Fortress's legends: on a permadeath run ending, a
/// short structured summary is appended to a plain-text history log.
pub fn append_run_history(path: &Path, summary: &str) -> io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{summary}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pre-stacking save wrote one optional `affix`; this build writes a
    /// list. The singular field is kept, load-only, and lifted — so a save
    /// written before affixes stacked keeps the one it had without a
    /// `SAVE_FORMAT_VERSION` bump.
    ///
    /// Parsed as a RON fragment rather than round-tripped through a whole
    /// save for `a_v29_save_still_loads_its_gear_after_the_new_field_lands`'
    /// reason: the fragment is what an old file on disk actually contains,
    /// and building one by hand is the only way to have a file this build
    /// never wrote.
    #[test]
    fn a_pre_stacking_save_lifts_its_singular_affix() {
        let old: GearCopySave = ron::from_str(
            r#"(item: "kinetic_edge", tier: 1, rarity: Silver, affix: Some("of_static"))"#,
        )
        .expect("a pre-stacking row must still load");
        assert_eq!(
            affixes_from_save(old.affix, old.affixes),
            vec![AffixId::from("of_static")],
            "the singular field must be lifted into the list"
        );
    }

    /// The write side fills `affixes` and leaves the legacy field empty, so
    /// this is the shape every save from here on carries.
    #[test]
    fn a_stacked_save_loads_every_affix_it_names() {
        let new: GearCopySave =
            ron::from_str(r#"(item: "kinetic_edge", tier: 1, affixes: ["hardened", "of_static"])"#)
                .expect("the stacked row shape must load");
        assert!(
            new.affix.is_none(),
            "a save written by this build carries no singular field"
        );
        assert_eq!(
            affixes_from_save(new.affix, new.affixes),
            vec![AffixId::from("hardened"), AffixId::from("of_static")],
            "both affixes must survive the load"
        );
    }

    /// Neither key: every copy in every save written before affixes existed
    /// at all. Both fields default, and the copy reads as unaffixed.
    #[test]
    fn a_pre_affix_save_loads_unaffixed() {
        let ancient: GearCopySave =
            ron::from_str(r#"(item: "shim_blade", tier: 0)"#).expect("a pre-affix row must load");
        assert!(
            affixes_from_save(ancient.affix, ancient.affixes).is_empty(),
            "a copy that never had an affix must load with none"
        );
    }

    /// The one save surface gear rarity cannot widen in place.
    /// `PlayerSave::fused_gear` ships as a positional 3-tuple
    /// (`dev-saves/extraction.ron:63` is literally `("scrap_ward", 3, 1)`),
    /// and RON parses a `(` in a struct position as the start of *named*
    /// fields — it raises `ExpectedIdentifier` rather than falling through
    /// to serde's `visit_seq`, so converting the row to a named struct with
    /// defaulted trailing fields does **not** load an old save. Measured,
    /// not assumed.
    ///
    /// So the two shapes coexist for one release instead: `fused_gear` stays
    /// exactly as it was and is read-only, `gear_copies` is the new store,
    /// and `Game::load` drains the first into the second. `fused_gear` is
    /// skipped when empty, so a save written from here on carries only the
    /// new field — and since nothing sets `deny_unknown_fields`, the legacy
    /// field can be deleted outright a release later with no bump.
    #[test]
    fn a_v29_save_still_loads_its_gear_after_the_new_field_lands() {
        #[derive(Debug, Serialize, Deserialize)]
        struct GearCopyProbe {
            item: ItemId,
            tier: u32,
            qty: u32,
            #[serde(default)]
            rarity: Rarity,
            #[serde(default)]
            affix: Option<String>,
        }

        #[derive(Debug, Default, Serialize, Deserialize)]
        struct PlayerProbe {
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            fused_gear: Vec<(ItemId, u32, u32)>,
            #[serde(default)]
            gear_copies: Vec<GearCopyProbe>,
        }

        // A v29 player: legacy rows, and no `gear_copies` field at all.
        let old: PlayerProbe = ron::from_str(r#"(fused_gear: [("scrap_ward", 3, 1)])"#)
            .expect("a v29 save must still load");
        assert_eq!(old.fused_gear, vec![(ItemId::from("scrap_ward"), 3, 1)]);
        assert!(
            old.gear_copies.is_empty(),
            "the new store defaults empty so load can drain the legacy rows into it"
        );

        // A save written after this change: new rows, no legacy field.
        let new: PlayerProbe = ron::from_str(
            r#"(gear_copies: [(item: "kinetic_edge", tier: 1, qty: 2, rarity: Silver)])"#,
        )
        .expect("the new row shape must load");
        assert_eq!(new.gear_copies[0].rarity, Rarity::Silver);
        assert_eq!(new.gear_copies[0].qty, 2);
        assert!(
            new.gear_copies[0].affix.is_none(),
            "affix lands in a later phase and must default until then"
        );

        // Nothing carries `deny_unknown_fields`, so dropping `fused_gear` in
        // a later release is not a format break either.
        let dropped: GearCopyProbe =
            ron::from_str(r#"(item: "shim_blade", tier: 1, qty: 1, retired_field: 7)"#)
                .expect("an unknown field must be ignored, not refused");
        assert_eq!(dropped.item, ItemId::from("shim_blade"));

        // And a fresh save carries no legacy field at all.
        let written = ron::ser::to_string_pretty(
            &PlayerProbe {
                gear_copies: vec![GearCopyProbe {
                    item: ItemId::from("shim_blade"),
                    tier: 0,
                    qty: 1,
                    rarity: Rarity::Gold,
                    affix: None,
                }],
                ..Default::default()
            },
            ron::ser::PrettyConfig::default(),
        )
        .unwrap();
        assert!(
            !written.contains("fused_gear"),
            "an empty legacy field must not be written back out: {written}"
        );
    }

    fn sample_data() -> SaveData {
        SaveData {
            seed: 1,
            base_ledger: Default::default(),
            tick: 0,
            difficulty: DifficultyMode::Forgiving,
            game_over: None,
            player: PlayerSave {
                position: (0, 0),
                hp: 30,
                max_hp: 30,
                atk: 6,
                mitigation: 2,
                power: 100.0,
                inventory: Vec::new(),
                level: 1,
                xp: 0,
                xp_to_next: 20,
                decompiler: 0,
                weapon: None,
                weapon_level: 1,
                weapon_fusion_tier: 0,
                weapon_rarity: Rarity::Ordinary,
                weapon_affix: None,
                weapon_affixes: Vec::new(),
                weapon_quality: crate::tuning::QUALITY_DEFAULT,
                armor: None,
                armor_level: 1,
                armor_fusion_tier: 0,
                armor_rarity: Rarity::Ordinary,
                armor_affix: None,
                armor_affixes: Vec::new(),
                armor_quality: crate::tuning::QUALITY_DEFAULT,
                module: None,
                module_level: 1,
                module_fusion_tier: 0,
                module_rarity: Rarity::Ordinary,
                module_affix: None,
                module_affixes: Vec::new(),
                module_quality: crate::tuning::QUALITY_DEFAULT,
                fused_gear: Vec::new(),
                gear_copies: Vec::new(),
                downed_programs: Vec::new(),
                perk_points: 0,
                unlocked_perks: Vec::new(),
                bought_stats: crate::components::BoughtStats::default(),
                tutorial_seeded: true,
                routines: Vec::new(),
                field_buffs: Vec::new(),
                sorties: Vec::new(),
                name: String::new(),
                class: None,
                glyph: '@',
                sprite: String::new(),
                colour: None,
                icon: None,
            },
            creatures: Vec::new(),
            structures: Vec::new(),
            nests: Vec::new(),
            dig_sites: Vec::new(),
            build_sites: Vec::new(),
            caravans: Vec::new(),
            caravan_memory: CaravanMemorySave::default(),
            tile_overrides: Vec::new(),
            base_grid: crate::base_grid::BaseGrid::default(),
            mining: false,
            free_builds: crate::resources::FreeBuilds::default(),
            anchor: None,
            zone: 1,
            spawn_point: (0, 0),
            buyback: Vec::new(),
            buyback_shelves: Vec::new(),
            researched: Vec::new(),
            known_routines: Vec::new(),
            link_sites: Vec::new(),
            locale: crate::resources::Locale::Surface,
            stack_memory: crate::resources::StackMemory::default(),
            populated_chunks: crate::resources::PopulatedChunks::default(),
            trace: 0,
            contracts: Vec::new(),
            contracts_done: Vec::new(),
            work_orders: Vec::new(),
            next_program_id: crate::resources::NextProgramId::START.0,
        }
    }

    /// A minimal tamed program, for the fixtures that need the save to hold
    /// a creature at all.
    fn sample_creature() -> CreatureSave {
        CreatureSave {
            species: "scrapper".to_string(),
            position: (1, 1),
            hp: 10,
            max_hp: 10,
            atk: 3,
            mitigation: 1,
            tamed: true,
            power: crate::components::POWER_MAX,
            level: 1,
            xp: 0,
            xp_to_next: 20,
            cronjob: None,
            party_slot: None,
            sortie_index: None,
            wielded: false,
            zone: 1,
            custom_name: None,
            hp_roll: 1.0,
            atk_roll: 1.0,
            def_roll: 1.0,
            growth_roll: 1.0,
            fusions: 0,
            refactors: 0,
            purchased_tiers: 0,
            ring: 0,
            talents: Vec::new(),
            bought_stats: crate::components::BoughtStats::default(),
            routines: Vec::new(),
            field_buffs: Vec::new(),
            nest_position: None,
            pursuing: false,
            carrying: None,
            rarity: Rarity::Ordinary,
            boss: false,
            nemesis_grudges: 0,
            equipment: Vec::new(),
            program_id: 1,
            disposition: None,
            disgruntled: None,
            memories: Vec::new(),
            needs: Default::default(),
            off_shift: None,
            staff: false,
            downed: false,
        }
    }

    /// The savetool's whole premise: a save dumped to RON, then packed back,
    /// must be the same save. Byte identity of the bincode encoding is the
    /// strictest form of that and catches a field silently dropped by the
    /// text encoding, which a field-by-field assertion would miss.
    #[test]
    fn a_save_survives_a_round_trip_through_ron_unchanged() {
        let mut data = sample_data();
        data.player.inventory = vec![(ItemId::from("core_fragment"), 3)];
        data.player.power = 62.5;
        data.tile_overrides = vec![(
            (4, -7),
            Tile {
                biome: crate::world::Biome::Platform,
                walkable: true,
                rock_shade: None,
            },
        )];
        data.zone = 3;
        // `StackMemory` is a map keyed by a *tuple* (`FrameKey`), which is
        // exactly where a text encoding tends to give up, and `Locale` is a
        // struct-variant enum. Both are in the round trip deliberately.
        data.locale = crate::resources::Locale::Stack {
            depth: 2,
            frames: 4,
            x: 9,
            y: 11,
            facing: crate::stack::Dir::West,
            entrance: (4, -7),
        };
        // A `BTreeSet` of tuples, which is the same place a text encoding
        // tends to give up.
        data.populated_chunks.0.insert((3, -2));
        data.populated_chunks.0.insert((-14, 9));
        data.stack_memory.0.insert(
            ((4, -7), 2),
            crate::resources::FrameMemory {
                seen: [(1, 1), (1, 2)].into_iter().collect(),
                looted: [(3, 3)].into_iter().collect(),
                opened: Default::default(),
                cleared: true,
                fights: [(5, 5)].into_iter().collect(),
                jacked: [(7, 2)].into_iter().collect(),
                adopted: [(9, 4)].into_iter().collect(),
                bought: [0, 4].into_iter().collect(),
            },
        );

        let before = bincode::serde::encode_to_vec(&data, bincode::config::standard()).unwrap();
        let text = to_ron(&data).unwrap();
        let parsed = from_ron(&text).unwrap();
        let after = bincode::serde::encode_to_vec(&parsed, bincode::config::standard()).unwrap();

        assert_eq!(
            before, after,
            "a RON round trip must not change a single byte of the save"
        );
    }

    /// The save on disk is the same field-named RON `savetool dump` prints,
    /// behind one line naming the version. Positional encodings are why
    /// every bump from v19 to v28 broke saves for a change that only ever
    /// *added* a field — see `SAVE_FORMAT_VERSION`.
    #[test]
    fn a_save_file_is_field_named_text() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_text_{}.bin",
            std::process::id()
        ));
        save_to_file(&path, &sample_data()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let loaded = load_from_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert!(
            text.starts_with(&format!("{SAVE_FORMAT_VERSION}\n")),
            "the version is the first line, so it can be read without parsing the rest"
        );
        assert!(
            text.contains("seed:"),
            "the payload names its fields: {}",
            &text[..text.len().min(120)]
        );
        assert_eq!(loaded.seed, sample_data().seed);
    }

    /// A save written before the mining toggle existed carries no `mining`
    /// field at all, and must load with the player's bump **disarmed** —
    /// both because that is the new default and because a save that never
    /// expressed a preference must not arm a tool that destroys terrain.
    ///
    /// Deleting the line rather than trusting the derive: `#[serde(default)]`
    /// on a `bool` is exactly the shape that reads as obviously right and
    /// silently isn't if the attribute is ever dropped in a refactor.
    #[test]
    fn a_save_without_the_mining_field_loads_disarmed() {
        let mut data = sample_data();
        data.mining = true;
        let text = to_ron(&data).unwrap();
        assert!(
            text.contains("mining: true"),
            "the fixture must actually write the field to be a real test"
        );
        let older = text.replace("mining: true,", "");
        let loaded = from_ron(&older).expect("an absent field must still parse");
        assert!(!loaded.mining);
    }

    /// The whole point of the format. A field added behind
    /// `#[serde(default)]` must load out of a file written before it
    /// existed, with no migration code and no version bump — which is what
    /// every single bump in this file's history would have needed.
    #[test]
    fn a_save_file_written_before_a_defaulted_field_existed_still_loads() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_older_{}.bin",
            std::process::id()
        ));
        let mut data = sample_data();
        data.creatures.push(sample_creature());
        save_to_file(&path, &data).unwrap();

        // Exactly what an older build would have written: the same file
        // with a since-added key absent. Derived from the real one rather
        // than hand-written, so the fixture cannot drift.
        let text = std::fs::read_to_string(&path).unwrap();
        let older: String = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("equipment: ["))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !older.contains("equipment: ["),
            "the key has to actually be gone for this to prove anything"
        );
        std::fs::write(&path, &older).unwrap();

        let loaded = match load_from_file(&path) {
            Ok(loaded) => loaded,
            Err(e) => panic!("an older file must still load: {e}"),
        };
        let _ = std::fs::remove_file(&path);
        assert!(loaded.creatures[0].equipment.is_empty());
    }

    /// The same guarantee for slice 2's own added field, and the reason
    /// `SAVE_FORMAT_VERSION` stayed at 32 for it: every `dev-saves/`
    /// template predates `dig_sites` and must load with no dig sites, which
    /// is exactly the base those runs had.
    #[test]
    fn a_save_written_before_dig_sites_existed_still_loads() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_no_dig_{}.bin",
            std::process::id()
        ));
        save_to_file(&path, &sample_data()).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let older: String = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("dig_sites: ["))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !older.contains("dig_sites:"),
            "the key has to actually be gone for this to prove anything"
        );
        std::fs::write(&path, &older).unwrap();

        let loaded = match load_from_file(&path) {
            Ok(loaded) => loaded,
            Err(e) => panic!("a file written before dig sites must still load: {e}"),
        };
        let _ = std::fs::remove_file(&path);
        assert!(
            loaded.dig_sites.is_empty(),
            "and comes back with no walls half-cut"
        );
    }

    /// The same demand of the caravan field: a run that predates it must
    /// load with no trader standing, which is exactly what that run had.
    #[test]
    fn a_save_written_before_caravans_existed_still_loads() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_no_caravan_{}.bin",
            std::process::id()
        ));
        save_to_file(&path, &sample_data()).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let older: String = text
            .lines()
            .filter(|line| !line.trim_start().starts_with("caravans: ["))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !older.contains("caravans:"),
            "the key has to actually be gone for this to prove anything"
        );
        std::fs::write(&path, &older).unwrap();

        let loaded = match load_from_file(&path) {
            Ok(loaded) => loaded,
            Err(e) => panic!("a file written before caravans must still load: {e}"),
        };
        let _ = std::fs::remove_file(&path);
        assert!(loaded.caravans.is_empty(), "and nobody is standing there");
    }

    /// The biome rename is not a save-format break, and this is what says
    /// so: a save written before it holds `Deadlock` in its tile overlay
    /// and must still load, while a save written now says `Deadlock`. The
    /// `serde(alias)` is the whole mechanism, so removing it fails here
    /// rather than in a player's next session.
    #[test]
    fn a_save_holding_the_old_cold_biome_name_still_loads() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_biome_{}.bin",
            std::process::id()
        ));
        let mut data = sample_data();
        data.tile_overrides = vec![(
            (4, -7),
            Tile {
                biome: crate::world::Biome::Deadlock,
                walkable: true,
                rock_shade: None,
            },
        )];
        save_to_file(&path, &data).unwrap();

        // Derived from the real file rather than hand-written, so the
        // fixture cannot drift from what the writer actually emits.
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("Deadlock"),
            "a save written now must use the new name"
        );
        std::fs::write(&path, text.replace("Deadlock", "StaticField")).unwrap();

        let loaded = match load_from_file(&path) {
            Ok(loaded) => loaded,
            Err(e) => panic!("a save written before the rename must still load: {e}"),
        };
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            loaded.tile_overrides[0].1.biome,
            crate::world::Biome::Deadlock
        );
    }

    /// A save from a build that wrote bincode is not text at all, so the
    /// version line is missing entirely. It has to be refused by version,
    /// the way any other unreadable save is, rather than surfacing a parse
    /// error about a byte offset.
    #[test]
    fn a_save_from_a_binary_format_build_is_refused_by_version() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_binary_{}.bin",
            std::process::id()
        ));
        let mut bytes = 28u32.to_le_bytes().to_vec();
        bytes.extend(
            bincode::serde::encode_to_vec(sample_data(), bincode::config::standard()).unwrap(),
        );
        std::fs::write(&path, bytes).unwrap();

        let err = match load_from_file(&path) {
            Ok(_) => panic!("a binary save must not decode as text"),
            Err(e) => e.to_string(),
        };
        let _ = std::fs::remove_file(&path);
        assert!(
            err.contains("older version of the game"),
            "the refusal should name the cause, not a byte offset: {err}"
        );
    }

    /// The Fatigue meter is gone, so nothing on disk may claim one. Asserted
    /// against the serialized text and against a payload that omits the key,
    /// because a field merely left in the struct and ignored would pass a
    /// struct-level check and still cost the next property a version bump.
    #[test]
    fn a_save_neither_writes_nor_requires_a_fatigue_field() {
        let ron = to_ron(&sample_data()).unwrap();
        assert!(
            !ron.contains("fatigue"),
            "a save must not carry a Fatigue meter:\n{ron}"
        );
        assert!(
            from_ron(&ron).is_ok(),
            "and a payload written without one has to load"
        );
    }

    #[test]
    fn a_save_round_trips_through_the_current_version() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_roundtrip_{}.bin",
            std::process::id()
        ));
        save_to_file(&path, &sample_data()).unwrap();
        let loaded = load_from_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded.seed, 1);
    }

    #[test]
    fn a_save_written_with_a_different_version_is_rejected_cleanly_instead_of_corrupting() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_badversion_{}.bin",
            std::process::id()
        ));
        // A save at a version this build does not read, written the way a
        // build at that version would have: the same text framing, a
        // different number on the first line.
        std::fs::write(&path, format!("999\n{}", to_ron(&sample_data()).unwrap())).unwrap();

        let Err(err) = load_from_file(&path) else {
            panic!("loading a mismatched-version save should fail, not succeed");
        };
        let _ = std::fs::remove_file(&path);
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(
            err.to_string().contains("incompatible save version"),
            "error should clearly say the save is from an incompatible version, got: {err}"
        );
    }

    /// There is no migration path (see `SAVE_FORMAT_VERSION`'s docs), so a
    /// save written under the immediately preceding version must be refused
    /// exactly like any other mismatch rather than silently decoded into
    /// garbage — the adjacent version being the one where a near-miss decode
    /// is most plausible.
    ///
    /// Written relative to the constant rather than against a hardcoded pair
    /// so it keeps testing the adjacent case across every future bump. It
    /// last named 14 -> 15 (adding `field_buffs`) and had gone stale by the
    /// time the constant reached 16.
    #[test]
    fn a_save_written_at_the_previous_version_is_refused() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_prev_version_{}.bin",
            std::process::id()
        ));
        std::fs::write(
            &path,
            format!(
                "{}\n{}",
                SAVE_FORMAT_VERSION - 1,
                to_ron(&sample_data()).unwrap()
            ),
        )
        .unwrap();

        let Err(err) = load_from_file(&path) else {
            panic!("a save one version back should not load");
        };
        let _ = std::fs::remove_file(&path);
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(
            err.to_string().contains("incompatible save version"),
            "error should clearly say the save is from an incompatible version, got: {err}"
        );
    }

    /// `dev-saves/extraction.ron` deserializes by field name — RON is
    /// self-describing, unlike the positional bincode save (see
    /// `SAVE_FORMAT_VERSION`'s docs) — so a `SaveData` field rename that
    /// forgets to update the template's keys breaks `--template extraction`
    /// at load. The launcher's `dev_template` tests do cover this file too
    /// (`every_checked_in_template_still_loads` enumerates every template
    /// and parses it), but this test still earns its place: it lives in the
    /// crate that owns `SaveData`, so `cargo test -p feral-processes-engine`
    /// catches RON-key drift without building the launcher.
    #[test]
    fn the_extraction_template_parses_into_save_data() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dev-saves/extraction.ron");
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        if let Err(e) = from_ron(&text) {
            panic!("dev-saves/extraction.ron should parse into SaveData: {e}");
        }
    }

    /// A save carrying a Stack position and a live Trace, written and read
    /// back through the real file round trip rather than the RON one above.
    ///
    /// This used to pin `SAVE_FORMAT_VERSION` to 15, which was the whole
    /// claim phase 1 of the Stack work rested on: renaming types moves no
    /// encoded byte. Phase 2 adds `trace`, which genuinely does, so the pin
    /// is gone rather than merely retargeted at 16 — a hardcoded version in
    /// a test taxes every future phase that legitimately bumps it, and
    /// phases 3 and 4 are both expected to. What is worth asserting is that
    /// the *payload* survives, which is what the bump exists to protect.
    #[test]
    fn a_stack_position_and_its_trace_survive_a_binary_save_round_trip() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_stack_roundtrip_{}.bin",
            std::process::id()
        ));
        let mut data = sample_data();
        data.locale = crate::resources::Locale::Stack {
            depth: 2,
            frames: 4,
            x: 9,
            y: 11,
            facing: crate::stack::Dir::West,
            entrance: (4, -7),
        };
        data.trace = 123;
        save_to_file(&path, &data).unwrap();
        let loaded = load_from_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            loaded.locale, data.locale,
            "the Stack position did not survive the round trip"
        );
        assert_eq!(
            loaded.trace, 123,
            "Trace did not survive the round trip — saving mid-dive would \
             be a free reset"
        );
    }

    #[test]
    fn a_truncated_file_fails_cleanly_instead_of_panicking() {
        let path = std::env::temp_dir().join(format!(
            "feral_processes_save_truncated_{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, [1, 2]).unwrap();
        let Err(err) = load_from_file(&path) else {
            panic!("loading a truncated save should fail, not succeed");
        };
        let _ = std::fs::remove_file(&path);
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
