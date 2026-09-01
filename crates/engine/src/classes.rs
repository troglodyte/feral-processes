//! The player's chosen class: an affinity spread and a starting kit,
//! loaded from `assets/classes/*.ron` by `ClassDb`. `assets/classes/README.md`
//! is the schema reference.
//!
//! **Grants affinities, and nothing else.** No stat shape (`ClassShape` in
//! `species.rs` builds *species* blocks, not the player's) and no talent
//! tree (`assets/talents/` is the companion's axis — see
//! `Game::ability_affinity`'s own doc comment on why the two never stack).
//! Keeping the class choice orthogonal to the points spend is what makes
//! the two steps worth having separately: class answers "what am I good
//! at", points answer "how tough am I".
//!
//! **`PerkDb::load_dir`'s exact contract**: a malformed file is skipped
//! with a returned warning rather than aborting the load, and — matching
//! every other absent-is-silent db in this crate (`NeedDb`, `MemoryDb`,
//! `AffixDb`...) — a missing directory loads empty rather than erroring.
//! **An empty `assets/classes/` is a supported install**, the same
//! property those databases hold: with nothing loaded, every axis resolves
//! neutral (`player_class_affinity`) and the hardcoded kit applies
//! (`apply_kit`) — exactly today's game. Both ends have to hold it
//! independently, since a resolver that falls back but a kit that doesn't
//! (or vice versa) is a half-supported install that looks fine until a
//! player actually deletes the directory.
//!
//! **The player stores the class, not the spread**, and `ClassDb` is
//! re-resolved on every read (`Game::player_class_affinity`,
//! `classes::apply_kit`) rather than cached anywhere. A retuned class file
//! therefore reaches a run already in progress — deliberately the opposite
//! of `ActiveContract`, which stores its whole resolved def because a
//! contract is a signed agreement that must not be rewritten under the
//! player.
//!
//! **What a class *does* is code, not data — `perks.rs`'s seam, one
//! directory over.** A class's catalogue entry (name, description, affinity
//! spread, kit) is authored `.ron`; a class's *effect* is a hook into one
//! particular formula with no shared shape to express as data, so it is a
//! named query here: `capture_boost_pct`, `routine_slot_bonus`,
//! `work_tick_scale`. Each is an **exhaustive match** on `PlayerClass` —
//! `cell_mark`'s rule, and
//! `every_perk_has_a_query_that_answers_what_it_is_worth`'s — so a ninth
//! variant fails to compile rather than shipping with an effect silently
//! missing. A class with nothing to say about an axis returns that axis's
//! neutral value, so every call site adds its term unconditionally and no
//! caller branches on the class itself.
//!
//! **The queries do not go through `ClassDb`**, so an effect survives a
//! deleted `assets/classes/`. Nobody can pick a class in that state — the
//! wizard has no rows to offer — but a save already carrying one keeps what
//! it grants, which is the perk seam's behaviour and the honest one: an
//! effect that vanished when a *display* catalogue went missing would be
//! the surprising half.

use crate::Game;
use crate::abilities::AffinityKind;
use crate::components::{Inventory, PlayerIdentity};
use crate::items::ItemId;
use crate::items::ids;
use crate::species::Affinities;
use serde::Serialize;
use crate::views;
use bevy_ecs::prelude::Resource;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// The class the **player** picked at creation. Eight variants: the five
/// `species::AffinityClass` names in `AffinityClass::ALL`'s order, then the
/// three no species can ever be.
///
/// **Deliberately not `AffinityClass`.** That enum is a *species'* derived
/// role and is load-bearing for things the player has nothing to do with —
/// `ClassShape`'s stat blocks, `talents::TalentDb`'s tree keys,
/// `render/manifest.rs::base_job_label`'s base job, `AffinityClass::of_axis`.
/// Adding `Fabricator` there would force every one of those exhaustive
/// matches to answer for a class no species can hold. The two enums sharing
/// five names is what makes the split cheap; collapsing them back into one
/// is what this doc comment exists to refuse.
///
/// **The variant order is save format.** `PlayerSave::class` rides the
/// positional bincode encoding (`save.rs`'s module docs), which stores an
/// enum by variant *index* — so the first five hold their positions and
/// every existing save reads back unchanged. Append, never reorder, exactly
/// as `perks::Perk` requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlayerClass {
    Striker,
    Bastion,
    Medic,
    Saboteur,
    Leech,
    /// Takes programs apart rather than beating them down — see
    /// [`capture_boost_pct`].
    Decompiler,
    /// Runs a deeper kit than anyone — see [`routine_slot_bonus`].
    Invoker,
    /// Keeps the base's machines turning — see [`work_tick_scale`].
    Fabricator,
}

impl PlayerClass {
    /// Every variant, in declaration order. `ClassDb::iter` walks it, so
    /// every screen sees classes in the same order whatever order their
    /// files loaded in. Append here when a variant is appended above.
    pub const ALL: [PlayerClass; 8] = [
        PlayerClass::Striker,
        PlayerClass::Bastion,
        PlayerClass::Medic,
        PlayerClass::Saboteur,
        PlayerClass::Leech,
        PlayerClass::Decompiler,
        PlayerClass::Invoker,
        PlayerClass::Fabricator,
    ];
}

/// One class's authored identity. `affinities` and `kit` are both
/// `#[serde(default)]` so a file may lean on `Affinities::NEUTRAL` (no
/// spread at all, unusual but not an error) or ship no kit — see
/// `assets/classes/README.md`.
#[derive(Clone, Debug, Deserialize)]
pub struct ClassDef {
    pub class: PlayerClass,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub affinities: Affinities,
    #[serde(default)]
    pub kit: Vec<(ItemId, u32)>,
}

/// Every class the game knows about, loaded from `assets/classes/`. See the
/// module doc comment for the absent/malformed-file contract.
#[derive(Resource, Default)]
pub struct ClassDb {
    defs: HashMap<PlayerClass, ClassDef>,
}

impl ClassDb {
    /// Loads every `*.ron` class in `dir`. A malformed file is skipped with
    /// a returned warning — `PerkDb::load_dir`'s rule — and a missing
    /// directory loads empty rather than erroring — `NeedDb::load_dir`'s
    /// rule, and the module doc comment's "supported install" property.
    ///
    /// Two files naming the same `PlayerClass` is not an error; sorted
    /// order (below) makes the alphabetically-last one win, same as
    /// `NeedDb`.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = ClassDb::default();
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<ClassDef>(&text) {
                Ok(def) => {
                    db.defs.insert(def.class, def);
                }
                Err(e) => warnings.push(format!("skipped invalid class file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, class: PlayerClass) -> Option<&ClassDef> {
        self.defs.get(&class)
    }

    /// Sorted by `PlayerClass::ALL`'s declaration order, because every
    /// caller walks it — a `HashMap`'s own order is not deterministic
    /// across runs, and `PlayerClass` carries no `Ord` of its own to sort
    /// by (see its doc comment: its order is save format, not something to
    /// add a derive to lightly).
    pub fn iter(&self) -> impl Iterator<Item = &ClassDef> {
        PlayerClass::ALL.iter().filter_map(|c| self.defs.get(c))
    }
}

/// Stocks the player's starting `Inventory` from `class`'s `ClassDef::kit`,
/// resolved live through `ClassDb`. Falls back to the four hardcoded items
/// `Game::new`'s kit used to hand out (`ICE_BREAKER` 3, `POWER_CELL` 3,
/// `CORE_FRAGMENT` 5, `OUTLET` 2) for `None` *and* for a class the current
/// `ClassDb` cannot resolve — the `apply_kit` half of the empty-directory
/// property the module doc comment describes.
pub fn apply_kit(game: &mut Game, class: Option<PlayerClass>) {
    let kit = class.and_then(|c| {
        game.world
            .resource::<ClassDb>()
            .get(c)
            .map(|d| d.kit.clone())
    });
    let player = game.player_entity();
    let mut inventory = game.world.get_mut::<Inventory>(player).unwrap();
    match kit {
        Some(kit) => {
            for (item, qty) in kit {
                inventory.add(item, qty);
            }
        }
        None => {
            inventory.add(ids::ICE_BREAKER.into(), 3);
            inventory.add(ids::POWER_CELL.into(), 3);
            inventory.add(ids::CORE_FRAGMENT.into(), 5);
            inventory.add(ids::OUTLET.into(), 2);
        }
    }
}

/// Percentage points this class adds to **every** decompile attempt, summed
/// into `taming::DecompilerBonuses::capture_boost_pct` by
/// `Game::player_decompiler_bonuses`.
///
/// That field rather than `skill`: `skill` enters `capture_chance` as
/// `1.0 + skill * DECOMPILER_SKILL_BONUS` and already grows on level-up and
/// off equipment, so a flat class addend there is worth steadily less all
/// run. `capture_boost_pct` multiplies the whole attempt and is worth the
/// same at level 30 as at level 1.
pub fn capture_boost_pct(class: Option<PlayerClass>) -> i32 {
    match class {
        Some(PlayerClass::Decompiler) => crate::tuning::CLASS_DECOMPILE_BOOST_PCT,
        Some(
            PlayerClass::Striker
            | PlayerClass::Bastion
            | PlayerClass::Medic
            | PlayerClass::Saboteur
            | PlayerClass::Leech
            | PlayerClass::Invoker
            | PlayerClass::Fabricator,
        )
        | None => 0,
    }
}

/// Routine slots this class adds on top of the level curve, added by
/// `Game::routine_slots` at its player arm.
///
/// **Added after `player_routine_slots`' own clamp and not re-clamped**,
/// which is exactly what the companion arm beside it already does with
/// `talent_routine_slots`: `PLAYER_ROUTINE_SLOT_CAP` bounds the *level
/// curve*, not the total. Threading this into `player_routine_slots` before
/// that clamp instead would converge to nothing at the cap — the one thing
/// an Invoker is named for, gone in the late run — and would cost that
/// function the purity `balance_sim` and several tests read it as.
pub fn routine_slot_bonus(class: Option<PlayerClass>) -> usize {
    match class {
        Some(PlayerClass::Invoker) => crate::tuning::CLASS_ROUTINE_SLOT_BONUS,
        Some(
            PlayerClass::Striker
            | PlayerClass::Bastion
            | PlayerClass::Medic
            | PlayerClass::Saboteur
            | PlayerClass::Leech
            | PlayerClass::Decompiler
            | PlayerClass::Fabricator,
        )
        | None => 0,
    }
}

/// What this class scales a work cycle's length by, applied in
/// `systems::work_ticks_at_speed` and supplied by `Game::work_ticks_for`.
///
/// Below 1.0 is faster. **The player's, not the worker's** — the asymmetry
/// `systems::CycleModifiers` already documents — so it applies at every
/// machine in this player's base whoever is posted there, and to the player
/// working a node by hand through the same one door.
pub fn work_tick_scale(class: Option<PlayerClass>) -> f64 {
    match class {
        Some(PlayerClass::Fabricator) => crate::tuning::CLASS_WORK_TICK_SCALE,
        Some(
            PlayerClass::Striker
            | PlayerClass::Bastion
            | PlayerClass::Medic
            | PlayerClass::Saboteur
            | PlayerClass::Leech
            | PlayerClass::Decompiler
            | PlayerClass::Invoker,
        )
        | None => 1.0,
    }
}

/// How a class's non-affinity spike reads in `format_trade`, or `None` for
/// a class whose whole trade is its affinity spread.
///
/// Lower case and phrased like an axis name, because `format_trade` folds it
/// in beside the raised axes: `"Bonus to decompiling at the expense of
/// damage"`. Without it the three classes above advertise themselves by their
/// damped axis alone — `"Weaker damage"`, a class with a downside and no
/// upside — because `format_trade` can only see `affinities`. Built here
/// beside `format_trade` so two renderers cannot word one class's trade
/// differently.
fn spike_label(class: PlayerClass) -> Option<&'static str> {
    match class {
        PlayerClass::Decompiler => Some("decompiling"),
        PlayerClass::Invoker => Some("routine slots"),
        PlayerClass::Fabricator => Some("work cycle speed"),
        PlayerClass::Striker
        | PlayerClass::Bastion
        | PlayerClass::Medic
        | PlayerClass::Saboteur
        | PlayerClass::Leech => None,
    }
}

/// What a class's spread is *worth*, one term per non-neutral axis:
/// `"Damage x1.25  Heal x0.75"`.
///
/// The stat-sheet length. `format_trade` below is the picking length — the
/// same axes as prose and with no magnitudes, which is what a catalogue row
/// wants and what a stat sheet does not. Both read `Affinities::non_neutral`,
/// which is the one definition of "an axis this class has an opinion about",
/// so the two lengths cannot disagree about *which* axes they name.
pub fn format_affinity_bonuses(affinities: &Affinities) -> String {
    affinities
        .non_neutral()
        .into_iter()
        .map(|(kind, value)| format!("{} x{value:.2}", kind.label()))
        .collect::<Vec<_>>()
        .join("  ")
}

/// What a class *trades*, as a sentence: `"Bonus to damage at the expense
/// of healing"`.
///
/// **Prose, not a sigil row.** This read `"+Damage  -Healing"` — compact,
/// and meaningless to anyone who had not already worked out that the game
/// has five affinity axes and that a class raises one by trading another
/// away. It is the only line a player has to pick a class from, and the
/// class step is the second screen of a new game.
///
/// Magnitudes are deliberately absent: the picker is choosing a *shape*,
/// and `format_affinity_bonuses` is where the numbers are read back once
/// the run exists.
fn format_trade(class: PlayerClass, affinities: &Affinities) -> String {
    let (up, down): (Vec<_>, Vec<_>) = affinities
        .non_neutral()
        .into_iter()
        .partition(|&(_, value)| value > crate::tuning::AFFINITY_NEUTRAL);
    let names = |axes: Vec<(AffinityKind, f32)>| {
        axes.into_iter()
            .map(|(kind, _)| kind.label().to_lowercase())
            .collect::<Vec<_>>()
    };
    // The spike leads, because a class whose whole upside is a code hook has
    // no raised axis to name and would otherwise open on its damped one.
    let mut up = names(up);
    if let Some(spike) = spike_label(class) {
        up.insert(0, spike.to_string());
    }
    match (join_words(up), join_words(names(down))) {
        (up, down) if !up.is_empty() && !down.is_empty() => {
            format!("Bonus to {up} at the expense of {down}")
        }
        (up, _) if !up.is_empty() => format!("Bonus to {up}"),
        (_, down) if !down.is_empty() => format!("Weaker {down}"),
        // An all-neutral class is a supported mod: no trade to describe.
        _ => String::new(),
    }
}

/// `["damage"]` -> `"damage"`, `["damage", "drain"]` -> `"damage and
/// drain"`, and three or more with commas before the "and". Every shipped
/// class trades exactly one axis for one, so the longer forms exist for
/// mods rather than for anything in `assets/classes/`.
fn join_words(words: Vec<String>) -> String {
    match words.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    }
}

/// `class`'s spread for `kind`, straight off a `ClassDb`. `AFFINITY_NEUTRAL`
/// for no class and for a class the db cannot resolve — the resolver half
/// of the empty-directory property.
///
/// A free function rather than a `Game` method because the creation wizard
/// prices a class before any `Game` exists: `CreationCatalogue` and
/// `Game::class_affinity` are its two callers, so neither can drift from
/// the other by re-deriving the lookup.
pub fn class_affinity(db: &ClassDb, class: Option<PlayerClass>, kind: AffinityKind) -> f32 {
    let Some(class) = class else {
        return crate::tuning::AFFINITY_NEUTRAL;
    };
    db.get(class)
        .map(|def| def.affinities.get(kind))
        .unwrap_or(crate::tuning::AFFINITY_NEUTRAL)
}

/// `Game::ability_affinity`'s player-arm sum, taken apart from the entity:
/// a resolved class spread plus `perks::affinity_bonus`, clamped at
/// `AFFINITY_MAX`. **The one place that combination is written.**
/// `Game::affinity_with_perk` passes the player's own perks; the creation
/// wizard passes `None`, because a player being created has none.
pub fn affinity_with_perk(
    class_affinity: f32,
    perks: Option<&crate::components::Perks>,
    kind: AffinityKind,
) -> f32 {
    (class_affinity + crate::perks::affinity_bonus(perks, kind)).min(crate::tuning::AFFINITY_MAX)
}

/// One row per loaded class, in `ClassDb::iter`'s order. The creation
/// screen's rows, derived once — `Game::class_rows` and
/// `CreationCatalogue::class_rows` are both calls to this.
pub fn class_rows(classes: &ClassDb) -> Vec<views::ClassRow> {
    classes
        .iter()
        .map(|def| views::ClassRow {
            class: def.class,
            name: def.name.clone(),
            description: def.description.clone(),
            trade: format_trade(def.class, &def.affinities),
        })
        .collect()
}

impl Game {
    /// The player's spread for `kind`, resolved live through `ClassDb` —
    /// see the module doc comment on why this is never cached.
    /// `AFFINITY_NEUTRAL` for no class or for a class the current
    /// `ClassDb` cannot resolve, the resolver half of the empty-directory
    /// property.
    /// The player's class as the manifest reads it back: its display name
    /// and what its spread is worth. `None` for a classless run — every
    /// save from before character creation, and `CharacterChoice::default`
    /// — and for a class the current `ClassDb` cannot resolve, which is
    /// `player_class_affinity`'s empty-directory property one level up.
    pub fn player_class_view(&self) -> Option<views::PlayerClassView> {
        let class = self
            .world
            .get::<PlayerIdentity>(self.player_entity())
            .and_then(|identity| identity.class)?;
        let def = self.world.resource::<ClassDb>().get(class)?;
        Some(views::PlayerClassView {
            name: def.name.clone(),
            bonuses: format_affinity_bonuses(&def.affinities),
        })
    }

    pub(crate) fn player_class_affinity(&self, kind: AffinityKind) -> f32 {
        self.class_affinity(self.player_class(), kind)
    }

    /// The class the player picked, straight off `PlayerIdentity`. `None`
    /// for a run created before the wizard existed and for one that skipped
    /// the step.
    ///
    /// The one read of that field, so the three effect queries above and the
    /// affinity resolve below cannot disagree about whose class is being
    /// asked for — the class is the *player's* wherever a formula reads it,
    /// including at a machine some other body is standing at.
    pub(crate) fn player_class(&self) -> Option<PlayerClass> {
        self.world
            .get::<PlayerIdentity>(self.player_entity())
            .and_then(|identity| identity.class)
    }

    /// `class`'s spread for `kind` alone, no perk term — `player_class_affinity`'s
    /// resolve, but for an explicit `class` argument instead of the one on
    /// `PlayerIdentity`. Exists so `player_affinity_for` below can price a
    /// class the player is only considering, not yet committed to the
    /// entity.
    fn class_affinity(&self, class: Option<PlayerClass>, kind: AffinityKind) -> f32 {
        class_affinity(self.world.resource::<ClassDb>(), class, kind)
    }

    /// The perk half of `ability_affinity`'s player arm — a resolved class
    /// spread plus `perks::affinity_bonus`'s per-kind perk bonus (`Perk::
    /// DamageAffinity`, `HealAffinity`, ...), clamped at `AFFINITY_MAX`. The
    /// one place that combination is computed: `ability_affinity` calls it
    /// with `player_class_affinity`'s resolve (the entity's own class),
    /// `player_affinity_for` below with an explicit class's — so neither
    /// path can drift from the other by re-deriving the sum itself.
    pub(crate) fn affinity_with_perk(&self, class_affinity: f32, kind: AffinityKind) -> f32 {
        affinity_with_perk(class_affinity, self.player_perks(), kind)
    }

    /// `ability_affinity`'s player-arm formula in full, for an explicit
    /// `class` rather than the one on `PlayerIdentity`. This is the door
    /// `starter_routine_rows` prices a row through against a class the
    /// creation wizard's player has only picked, not yet committed to the
    /// entity.
    pub(crate) fn player_affinity_for(
        &self,
        class: Option<PlayerClass>,
        kind: AffinityKind,
    ) -> f32 {
        self.affinity_with_perk(self.class_affinity(class, kind), kind)
    }

    /// One row per loaded class, in `ClassDb::iter`'s order, for the
    /// creation screen.
    pub fn class_rows(&self) -> Vec<views::ClassRow> {
        class_rows(self.world.resource::<ClassDb>())
    }
}
