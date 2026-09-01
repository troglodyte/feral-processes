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

use crate::Game;
use crate::abilities::AffinityKind;
use crate::components::{Inventory, PlayerIdentity};
use crate::items::ItemId;
use crate::items::ids;
use crate::species::{Affinities, AffinityClass};
use crate::views;
use bevy_ecs::prelude::Resource;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// One class's authored identity. `affinities` and `kit` are both
/// `#[serde(default)]` so a file may lean on `Affinities::NEUTRAL` (no
/// spread at all, unusual but not an error) or ship no kit — see
/// `assets/classes/README.md`.
#[derive(Clone, Debug, Deserialize)]
pub struct ClassDef {
    pub class: AffinityClass,
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
    defs: HashMap<AffinityClass, ClassDef>,
}

impl ClassDb {
    /// Loads every `*.ron` class in `dir`. A malformed file is skipped with
    /// a returned warning — `PerkDb::load_dir`'s rule — and a missing
    /// directory loads empty rather than erroring — `NeedDb::load_dir`'s
    /// rule, and the module doc comment's "supported install" property.
    ///
    /// Two files naming the same `AffinityClass` is not an error; sorted
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

    pub fn get(&self, class: AffinityClass) -> Option<&ClassDef> {
        self.defs.get(&class)
    }

    /// Sorted by `AffinityClass::ALL`'s declaration order, because every
    /// caller walks it — a `HashMap`'s own order is not deterministic
    /// across runs, and `AffinityClass` carries no `Ord` of its own to sort
    /// by (see its doc comment: it is load-bearing save-format-adjacent
    /// data, not something to add a derive to lightly).
    pub fn iter(&self) -> impl Iterator<Item = &ClassDef> {
        AffinityClass::ALL.iter().filter_map(|c| self.defs.get(c))
    }
}

/// Stocks the player's starting `Inventory` from `class`'s `ClassDef::kit`,
/// resolved live through `ClassDb`. Falls back to the four hardcoded items
/// `Game::new`'s kit used to hand out (`ICE_BREAKER` 3, `POWER_CELL` 3,
/// `CORE_FRAGMENT` 5, `OUTLET` 2) for `None` *and* for a class the current
/// `ClassDb` cannot resolve — the `apply_kit` half of the empty-directory
/// property the module doc comment describes.
pub fn apply_kit(game: &mut Game, class: Option<AffinityClass>) {
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

/// `"+Healing  -Damage"`-style summary of `affinities`, built once here so
/// the creation screen and any other reader of `views::ClassRow` cannot
/// word one class's trade differently.
fn format_axes(affinities: &Affinities) -> String {
    affinities
        .non_neutral()
        .into_iter()
        .map(|(kind, value)| {
            let sign = if value > crate::tuning::AFFINITY_NEUTRAL {
                '+'
            } else {
                '-'
            };
            format!("{sign}{}", kind.label())
        })
        .collect::<Vec<_>>()
        .join("  ")
}

/// `"3x Core Fragment, 4x Power Cell"`-style summary of a kit, for the same
/// reason `format_axes` exists.
fn format_kit(items: &crate::items_db::ItemDb, kit: &[(ItemId, u32)]) -> String {
    kit.iter()
        .map(|(item, qty)| {
            let name = items
                .get(item.as_str())
                .map(|d| d.name.as_str())
                .unwrap_or_else(|| item.as_str());
            format!("{qty}x {name}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `class`'s spread for `kind`, straight off a `ClassDb`. `AFFINITY_NEUTRAL`
/// for no class and for a class the db cannot resolve — the resolver half
/// of the empty-directory property.
///
/// A free function rather than a `Game` method because the creation wizard
/// prices a class before any `Game` exists: `CreationCatalogue` and
/// `Game::class_affinity` are its two callers, so neither can drift from
/// the other by re-deriving the lookup.
pub fn class_affinity(db: &ClassDb, class: Option<AffinityClass>, kind: AffinityKind) -> f32 {
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
pub fn class_rows(classes: &ClassDb, items: &crate::items_db::ItemDb) -> Vec<views::ClassRow> {
    classes
        .iter()
        .map(|def| views::ClassRow {
            class: def.class,
            name: def.name.clone(),
            description: def.description.clone(),
            axes: format_axes(&def.affinities),
            kit: format_kit(items, &def.kit),
        })
        .collect()
}

impl Game {
    /// The player's spread for `kind`, resolved live through `ClassDb` —
    /// see the module doc comment on why this is never cached.
    /// `AFFINITY_NEUTRAL` for no class or for a class the current
    /// `ClassDb` cannot resolve, the resolver half of the empty-directory
    /// property.
    pub(crate) fn player_class_affinity(&self, kind: AffinityKind) -> f32 {
        let class = self
            .world
            .get::<PlayerIdentity>(self.player_entity())
            .and_then(|identity| identity.class);
        self.class_affinity(class, kind)
    }

    /// `class`'s spread for `kind` alone, no perk term — `player_class_affinity`'s
    /// resolve, but for an explicit `class` argument instead of the one on
    /// `PlayerIdentity`. Exists so `player_affinity_for` below can price a
    /// class the player is only considering, not yet committed to the
    /// entity.
    fn class_affinity(&self, class: Option<AffinityClass>, kind: AffinityKind) -> f32 {
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
        class: Option<AffinityClass>,
        kind: AffinityKind,
    ) -> f32 {
        self.affinity_with_perk(self.class_affinity(class, kind), kind)
    }

    /// One row per loaded class, in `ClassDb::iter`'s order, for the
    /// creation screen.
    pub fn class_rows(&self) -> Vec<views::ClassRow> {
        class_rows(
            self.world.resource::<ClassDb>(),
            self.world.resource::<crate::items_db::ItemDb>(),
        )
    }
}
