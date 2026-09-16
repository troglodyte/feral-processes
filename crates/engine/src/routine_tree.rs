//! Deriving the routine research tree from `AbilityDb`.
//!
//! Nothing here is authored. A routine's family, its rung on the scope
//! ladder, and the prerequisite that gates it are all read off the ability's
//! own `name` and `target` — see the design in
//! `docs/superpowers/specs/2026-09-16-routine-research-tree-design.md` §2.
//! `every_battle_ability_family_is_contiguous_from_single_upward` in
//! `tests/assets.rs` used to keep its own copy of `family`/`scope_rank`; it
//! now calls these instead, so the census and the schema cannot drift apart.

use crate::abilities::{AbilityDb, AbilityDef, AbilityEffect, AbilityId, AbilityTarget};
use crate::research::{ResearchDef, ResearchTree};

/// The scope word an ability's `name` must end in, given what it targets.
/// `OneAlly` and `OneEnemyGroupFront` share "Single" — one recipient either
/// way, and which side it lands on is never in doubt from the picker.
fn scope_word(target: AbilityTarget) -> &'static str {
    use AbilityTarget::*;
    match target {
        OneAlly | OneEnemyGroupFront => "Single",
        WholeParty => "Party",
        WholeEnemyGroup => "Group",
        AllEnemies => "Everyone",
    }
}

/// Strips a trailing ` vN.N` version tag, which is how two abilities in the
/// same family at the same scope are told apart by magnitude.
fn without_version_tag(name: &str) -> &str {
    let Some((base, tag)) = name.rsplit_once(' ') else {
        return name;
    };
    let is_tag = tag.strip_prefix('v').is_some_and(|v| {
        v.split_once('.')
            .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
            && v.chars().all(|c| c.is_ascii_digit() || c == '.')
    });
    if is_tag { base } else { name }
}

/// The family an ability's display name declares — everything before the
/// scope word, with any version tag already gone. `"Fork Bomb Group"` is
/// `"Fork Bomb"`, and so is `"Fork Bomb Everyone"`.
pub fn family(def: &AbilityDef) -> String {
    let base = without_version_tag(&def.name);
    base.trim_end_matches(scope_word(def.target)).trim().into()
}

/// How far up the scope ladder a target reaches. The two sides share the
/// ladder rather than having one each: one recipient, one group, the field
/// — an ally-facing family simply has nowhere to go above rung 1, since
/// `WholeParty` already *is* everyone on your side.
pub fn scope_rank(target: AbilityTarget) -> u8 {
    use AbilityTarget::*;
    match target {
        OneAlly | OneEnemyGroupFront => 0,
        WholeParty | WholeEnemyGroup => 1,
        AllEnemies => 2,
    }
}

/// The `(major, minor)` pair a name's trailing `vN.N` tag declares, or
/// `(1, 0)` for a name with no tag — the untagged rung of a family is
/// always its first version.
pub fn version(name: &str) -> (u32, u32) {
    let Some((_, tag)) = name.rsplit_once(' ') else {
        return (1, 0);
    };
    let Some(v) = tag.strip_prefix('v') else {
        return (1, 0);
    };
    let Some((major, minor)) = v.split_once('.') else {
        return (1, 0);
    };
    if major.is_empty() || minor.is_empty() || !v.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return (1, 0);
    }
    (major.parse().unwrap_or(1), minor.parse().unwrap_or(0))
}

/// Whether `id` is welded permanently into its slot — `decompile` alone.
/// Keyed to `abilities::DECOMPILE_ABILITY_ID` rather than an
/// asset-authored flag, for `Game::routine_is_permanent`'s own reason:
/// permanence is not a property a file may claim.
fn is_permanent(id: &str) -> bool {
    id == crate::abilities::DECOMPILE_ABILITY_ID
}

/// Whether `def` gets a synthesised research node at all. False for an
/// **exclusive** routine (stays boss-drop and trader-only), the one
/// **permanent** routine every player already has, a **passive**
/// (`triggers.is_some()` — a node would make a gear grant researchable),
/// and a **`Summon`** (no source exists for one today).
pub fn gets_node(_abilities: &AbilityDb, def: &AbilityDef) -> bool {
    !def.exclusive
        && !is_permanent(&def.id)
        && def.triggers.is_none()
        && !matches!(def.effect, AbilityEffect::Summon { .. })
}

/// The one rung within `def`'s family that must be researched (or known)
/// before `def` can be, or `None` at the family's root.
///
/// Within a family: version *n* at a scope requires the next lower version
/// at the same scope; the lowest version at a scope requires the lowest
/// version at the nearest lower scope that exists in the family; the lowest
/// version at the lowest existing scope requires nothing. Only peers that
/// themselves get a node are considered — an excluded ability (passive,
/// exclusive, permanent, summon) can never be a prerequisite because it can
/// never be researched.
pub fn routine_prereq(abilities: &AbilityDb, def: &AbilityDef) -> Option<AbilityId> {
    let fam = family(def);
    let scope = scope_rank(def.target);
    let ver = version(&def.name);
    let peers: Vec<&AbilityDef> = abilities
        .all()
        .filter(|d| gets_node(abilities, d) && family(d) == fam)
        .collect();

    if let Some(prior) = peers
        .iter()
        .filter(|d| scope_rank(d.target) == scope && version(&d.name) < ver)
        .max_by_key(|d| version(&d.name))
    {
        return Some(prior.id.clone());
    }

    let lower_scope = peers
        .iter()
        .map(|d| scope_rank(d.target))
        .filter(|&s| s < scope)
        .max()?;
    peers
        .iter()
        .filter(|d| scope_rank(d.target) == lower_scope)
        .min_by_key(|d| version(&d.name))
        .map(|d| d.id.clone())
}

/// The synthesised id for `ability`'s research node — `"routine/<id>"`,
/// stable across a rename of the ability's display name.
pub fn node_id(ability: &str) -> String {
    format!("routine/{ability}")
}

/// One synthesised `ResearchDef` per `gets_node`-eligible ability —
/// `ResearchDb::load_dir`'s counterpart to `ItemDb::synthesise_etched_disks`.
/// A synthesised node carries no material bill (spec §2 "Cost"), and its
/// `requires` names only the one prerequisite rung `routine_prereq` derives.
pub fn synthesise_nodes(abilities: &AbilityDb) -> Vec<ResearchDef> {
    abilities
        .all()
        .filter(|def| gets_node(abilities, def))
        .map(|def| {
            let requires = routine_prereq(abilities, def)
                .map(|id| vec![node_id(&id)])
                .unwrap_or_default();
            let zone = if def.research_zone == 0 {
                1
            } else {
                def.research_zone
            };
            ResearchDef {
                id: node_id(&def.id),
                name: def.name.clone(),
                description: def.description.clone(),
                cost: crate::tuning::routine_research_cost(
                    scope_rank(def.target),
                    version(&def.name),
                ),
                materials: Vec::new(),
                min_zone: zone,
                requires,
                recommended: false,
                unlocks_structures: Vec::new(),
                unlocks_recipes: Vec::new(),
                unlocks_abilities: Vec::new(),
                unlocks_tools: Vec::new(),
                tree: ResearchTree::Routines,
                teaches: Some(def.id.clone()),
                opens_routine_tree: false,
            }
        })
        .collect()
}
