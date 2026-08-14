//! Contracts: named, finite objectives with a payout, issued by a Contract
//! Broker.
//!
//! Contracts are data, like achievements and species — `assets/contracts/*.ron`,
//! one file per contract, so adding one is a file drop. What is *not* data is
//! how a board is derived and how many may be held at once, which live in
//! `tuning.rs` beside every other knob.

use std::collections::BTreeMap;
use std::path::Path;

use rand::prelude::*;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::items::ItemId;
use crate::structures::StructureId;

/// A contract's id, e.g. `clear_the_nursery`. A string newtype rather than an
/// enum for the same reason `achievements::AchievementId` is one: a contract
/// is a file drop, and an enum would make it a code change.
///
/// `#[serde(transparent)]` so it spells as a bare quoted string in both the
/// asset files and the save. `Ord` so `ContractDb` can key a `BTreeMap` by it
/// — the contracts screen lists that iteration order, and a `HashMap` would
/// reshuffle the screen between runs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContractId(pub String);

impl ContractId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ContractId {
    fn from(s: &str) -> Self {
        ContractId(s.to_string())
    }
}

impl From<String> for ContractId {
    fn from(s: String) -> Self {
        ContractId(s)
    }
}

impl std::fmt::Display for ContractId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a contract asks for.
///
/// Four of the five are state-shaped and are evaluated by polling, which is
/// why this whole feature costs exactly one new call site: only `Terminate` is
/// event-shaped, and it is recorded into `resources::RunFeats` by `award_loot`
/// the way `Trigger::BossDefeated` already is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Objective {
    /// `None` means any kill; `Some(species_id)` names one, as
    /// `Trigger::BossDefeated` does.
    Terminate { species: Option<String>, count: u32 },
    /// Items handed over at the Broker. The one objective whose progress does
    /// not come from `contract_system` — see `Game::deliver_to_contract`.
    Deliver { item: ItemId, count: u32 },
    /// The party has stood in a Stack frame at this depth or deeper. Read off
    /// `resources::Locale`, never `Position`, which is pinned to the surface
    /// entrance tile while the party is underground.
    Descend { depth: u32 },
    /// The run has breached to this zone or deeper.
    Breach { zone: u32 },
    /// One of these is deployed.
    Build { structure: StructureId },
}

impl Objective {
    /// Units of progress that complete this objective: `count` for the two
    /// counting variants, 1 for the three state-shaped ones — so every
    /// contract displays and completes through one `progress >= target()`
    /// rule and no caller branches on the variant to ask "am I done".
    pub fn target(&self) -> u32 {
        match self {
            Objective::Terminate { count, .. } | Objective::Deliver { count, .. } => *count,
            Objective::Descend { .. } | Objective::Breach { .. } | Objective::Build { .. } => 1,
        }
    }
}

/// What a contract pays. A `Vec<Reward>` rather than one, because a contract
/// paying both Credits and an item is the common case and a single-reward
/// field would force two contracts to express it.
///
/// There is deliberately **no `PortalFragments` variant** — absent rather than
/// unused, so a mod file cannot reach it either. Breaching stays earned by
/// fighting and descending, which is the narrow rule that survives contracts
/// amending "progression is earned by fighting".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reward {
    /// `n` added to the player's `ids::CREDITS`.
    Credits(u32),
    /// `n` plain `Ordinary` copies of that item — never through
    /// `Game::grant_gear_drop`, which is the one door a copy above `Ordinary`
    /// enters the game by.
    Item(ItemId, u32),
    /// `n` XP to the player, through `Game::award_player_xp` so a level-up
    /// full-heals exactly as it does from a kill.
    Xp(u32),
}

/// One authored contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractDef {
    pub id: ContractId,
    pub name: String,
    /// Player-facing, and the only place a player is told what to do.
    /// Authored rather than derived from the objective, so a modder controls
    /// how their contract reads — `AchievementDef::description`'s argument.
    pub description: String,
    pub objective: Objective,
    pub reward: Vec<Reward>,
    /// Gates whether a board may *offer* this contract, never whether an
    /// accepted one may be finished — breaching mid-contract must not strand
    /// it. Spelled as `ResearchDef::min_zone` is: 0 and absent mean the same
    /// thing, and a second spelling for it is a second thing to get wrong.
    #[serde(default)]
    pub min_zone: u32,
    /// Whether finishing it puts it back on the board.
    #[serde(default)]
    pub repeatable: bool,
}

/// Separates a template's id from the parameters a roll filled in —
/// `hunt#drone-6`. Refused in an authored id at load, so the two id spaces
/// cannot collide and `ContractDb::repeatable` can read a rolled contract's
/// template straight back off its id.
pub const ROLLED_ID_SEPARATOR: char = '#';

/// What a template leaves open, mirroring `Objective` variant for variant with
/// the numeric fields widened to inclusive ranges.
///
/// A parallel vocabulary is a copy to keep in step, so it earns its place by
/// being the whole of what varies: which species, which item, which structure,
/// and how many. Everything downstream — accepting, progressing, completing —
/// sees only the `Objective` a roll produced and cannot tell it from an
/// authored one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateObjective {
    /// The species is drawn from the sector; a rolled Terminate always names
    /// one, since "any wild program" needs no template to express.
    Terminate {
        count: (u32, u32),
    },
    Deliver {
        count: (u32, u32),
    },
    Descend {
        depth: (u32, u32),
    },
    Breach {
        zone: (u32, u32),
    },
    /// The structure is drawn from the sector, so there is nothing numeric
    /// left to range over.
    Build,
}

/// What this sector can supply a rolled contract with, gathered by
/// `Game::template_pools` and consumed by `ContractTemplate::roll`.
///
/// Passing the pools in rather than letting `roll` reach for a `Game` is what
/// keeps the roll a pure function of `(rng, pools)` — testable without a world,
/// and unable to spend a `GameRng` draw by accident. Each candidate carries its
/// **display name** beside its id because the roll writes the name into the
/// contract's description, and resolving ids to names anywhere but the engine
/// is what `Game::copy_name` exists to prevent.
pub struct TemplatePools {
    pub species: Vec<(String, String)>,
    pub items: Vec<(ItemId, String)>,
    pub structures: Vec<(StructureId, String)>,
    /// The sector the run is in, which is what a rolled `Breach` has to clear.
    pub zone: u32,
}

/// A contract with free variables. `roll` fills them in and produces **the
/// same `ContractDef`** an authored file parses into — an authored contract is
/// a template with no free variables, so there is one accept path, one
/// progress path and one completion path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractTemplate {
    /// The rolled contract's id is built from this and the roll, so it is a
    /// prefix rather than an id in its own right.
    pub id: ContractId,
    /// `{target}` is replaced by the rolled species, item or structure's
    /// display name, `{count}` by the number rolled. Both are optional; a
    /// template with neither simply reads the same however it rolls.
    pub name: String,
    /// The one field a template cannot derive and must author with a hole in
    /// it — `objective_line` already words the objective itself.
    pub description: String,
    pub objective: TemplateObjective,
    /// Paid **per unit of `Objective::target()`**, so asking for ten pays ten
    /// times and the three state-shaped objectives (which target 1) pay the
    /// authored figure flat. Reusing `target()` rather than inventing a scale
    /// is what keeps the rule to one sentence with no branch on the variant.
    pub reward: Vec<Reward>,
    #[serde(default)]
    pub min_zone: u32,
    #[serde(default)]
    pub repeatable: bool,
}

impl ContractTemplate {
    /// Rolls this template into a finishable contract, or `None` when the
    /// sector can supply nothing valid.
    ///
    /// `None` rather than a clamped or partial contract is the whole point:
    /// an objective naming a species that does not live here, an item nothing
    /// here produces, or a structure already standing is unfinishable or
    /// finishes on acceptance, and either is worse than an empty slot.
    pub fn roll(&self, rng: &mut StdRng, pools: &TemplatePools) -> Option<ContractDef> {
        let (objective, slug, target_name, magnitude) = match &self.objective {
            TemplateObjective::Terminate { count } => {
                let (id, name) = pick(rng, &pools.species)?;
                let count = draw(rng, *count, 1)?;
                (
                    Objective::Terminate {
                        species: Some(id.clone()),
                        count,
                    },
                    format!("{id}-{count}"),
                    name.clone(),
                    count,
                )
            }
            TemplateObjective::Deliver { count } => {
                let (id, name) = pick(rng, &pools.items)?;
                let count = draw(rng, *count, 1)?;
                (
                    Objective::Deliver {
                        item: id.clone(),
                        count,
                    },
                    format!("{id}-{count}"),
                    name.clone(),
                    count,
                )
            }
            // Depth 0 is the surface, and progress is `depth >= want`, so a
            // rolled 0 would finish the moment it was accepted.
            TemplateObjective::Descend { depth } => {
                let depth = draw(rng, *depth, 1)?;
                (
                    Objective::Descend { depth },
                    format!("d{depth}"),
                    String::new(),
                    depth,
                )
            }
            // Same trap one sector over: `zone.0 >= want` is already true for
            // anything at or below where the run has reached.
            TemplateObjective::Breach { zone } => {
                let zone = draw(rng, *zone, pools.zone.saturating_add(1))?;
                (
                    Objective::Breach { zone },
                    format!("z{zone}"),
                    String::new(),
                    zone,
                )
            }
            TemplateObjective::Build => {
                let (id, name) = pick(rng, &pools.structures)?;
                (
                    Objective::Build {
                        structure: id.clone(),
                    },
                    id.to_string(),
                    name.clone(),
                    1,
                )
            }
        };

        let fill = |text: &str| {
            text.replace("{count}", &magnitude.to_string())
                .replace("{target}", &target_name)
        };
        let scale = objective.target();
        Some(ContractDef {
            id: ContractId::from(format!("{}{ROLLED_ID_SEPARATOR}{slug}", self.id)),
            name: fill(&self.name),
            description: fill(&self.description),
            objective,
            reward: self
                .reward
                .iter()
                .map(|r| match r {
                    Reward::Credits(n) => Reward::Credits(n.saturating_mul(scale)),
                    Reward::Item(item, n) => Reward::Item(item.clone(), n.saturating_mul(scale)),
                    Reward::Xp(n) => Reward::Xp(n.saturating_mul(scale)),
                })
                .collect(),
            min_zone: self.min_zone,
            repeatable: self.repeatable,
        })
    }
}

/// One candidate from a pool, or `None` when the sector supplies none.
fn pick<'a, T>(rng: &mut StdRng, pool: &'a [(T, String)]) -> Option<&'a (T, String)> {
    (!pool.is_empty()).then(|| &pool[rng.random_range(0..pool.len())])
}

/// A number from an inclusive authored range, raised to `floor` first. `None`
/// when the floor has eaten the range — which is a real answer rather than an
/// error: a `Breach(2, 6)` template simply has nothing to offer in sector 6.
fn draw(rng: &mut StdRng, (lo, hi): (u32, u32), floor: u32) -> Option<u32> {
    let lo = lo.max(floor);
    (lo <= hi).then(|| rng.random_range(lo..=hi))
}

#[derive(Resource, Default)]
pub struct ContractDb {
    defs: BTreeMap<ContractId, ContractDef>,
    templates: BTreeMap<ContractId, ContractTemplate>,
}

impl ContractDb {
    /// Loads every `*.ron` contract in `dir`. A malformed file is skipped with
    /// a returned warning rather than aborting the load, same as
    /// `AchievementDb::load_dir`.
    ///
    /// Rejected with a warning, as well as anything `ron` refuses: an empty
    /// id, a duplicate id, and a reward that pays nothing. Whether the item,
    /// species or structure an objective names *exists* is not checked here —
    /// no other db is in hand, exactly as `AchievementDb::load_dir` defers its
    /// `StartingProgram` check. The shipped set is covered by its census test.
    ///
    /// An absent directory is silent and leaves the db empty, the rule
    /// `AffixDb` and `SectorDb` already follow: an install without contracts
    /// is the pre-contract game, and a board with nothing on it is a
    /// supported way to play rather than a failure to start.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = ContractDb::default();
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        for entry in entries {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<ContractDef>(&text) {
                Ok(def) => match complaint(&def) {
                    Some(why) => {
                        warnings.push(format!("skipped invalid contract file {path:?}: {why}"))
                    }
                    None if db.defs.contains_key(&def.id) => warnings.push(format!(
                        "skipped invalid contract file {path:?}: id {} is already taken",
                        def.id
                    )),
                    None => {
                        db.defs.insert(def.id.clone(), def);
                    }
                },
                Err(e) => warnings.push(format!("skipped invalid contract file {path:?}: {e}")),
            }
        }
        db.load_templates(&dir.join("templates"), &mut warnings)?;
        Ok((db, warnings))
    }

    /// Loads `dir/templates/*.ron`. Absent is silent and leaves none, the same
    /// rule the contract directory itself follows and for the same reason:
    /// roughly sixty test fixtures build a partial assets tree, and the README
    /// already promises that deleting the directory gives the game back as it
    /// was without it.
    fn load_templates(&mut self, dir: &Path, warnings: &mut Vec<String>) -> std::io::Result<()> {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        for entry in entries {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<ContractTemplate>(&text) {
                Ok(template) => match template_complaint(&template) {
                    Some(why) => {
                        warnings.push(format!("skipped invalid contract template {path:?}: {why}"))
                    }
                    None if self.templates.contains_key(&template.id) => warnings.push(format!(
                        "skipped invalid contract template {path:?}: id {} is already taken",
                        template.id
                    )),
                    None => {
                        self.templates.insert(template.id.clone(), template);
                    }
                },
                Err(e) => warnings.push(format!("skipped invalid contract template {path:?}: {e}")),
            }
        }
        Ok(())
    }

    pub fn get(&self, id: &ContractId) -> Option<&ContractDef> {
        self.defs.get(id)
    }

    /// Whether finishing `id` puts it back on the board.
    ///
    /// The one statement of it, because the answer has two sources: an
    /// authored contract carries the flag itself, and a rolled one inherits
    /// its template's. `accept_contract` asked the db directly before
    /// templates existed, so a rolled id resolved to `None` and read as *not*
    /// repeatable by accident — the right default, arrived at the wrong way,
    /// and one that would have silently stopped honouring a repeatable
    /// template.
    ///
    /// An id belonging to neither is not repeatable: that is a contract whose
    /// file or template has been deleted mid-run, and the run's own copy is
    /// what finishes it.
    pub fn repeatable(&self, id: &ContractId) -> bool {
        if let Some(def) = self.defs.get(id) {
            return def.repeatable;
        }
        match id.as_str().split_once(ROLLED_ID_SEPARATOR) {
            Some((template, _)) => self
                .templates
                .get(&ContractId::from(template))
                .is_some_and(|t| t.repeatable),
            None => false,
        }
    }

    pub fn template(&self, id: &ContractId) -> Option<&ContractTemplate> {
        self.templates.get(id)
    }

    /// Every template, in id order — stable between runs for the same reason
    /// `iter` is, since the board salts its roll off each template's id.
    pub fn templates(&self) -> impl Iterator<Item = &ContractTemplate> {
        self.templates.values()
    }

    /// Every authored contract, in id order. The board draws from this, so the
    /// order has to be stable between runs or a seeded board would not be.
    pub fn iter(&self) -> impl Iterator<Item = &ContractDef> {
        self.defs.values()
    }
}

/// Why `def` cannot be loaded, or `None` if it is fine. A contract that pays
/// nothing is refused for the same reason `AchievementDb` refuses a zero
/// reward: it is a mistake that reads as a working file.
fn complaint(def: &ContractDef) -> Option<String> {
    if def.id.as_str().is_empty() {
        return Some("a contract needs an id".to_string());
    }
    if def.id.as_str().contains(ROLLED_ID_SEPARATOR) {
        return Some(format!(
            "a contract id may not contain '{ROLLED_ID_SEPARATOR}': that is what \
             separates a template from the parameters a roll filled in, and an \
             authored id carrying one could collide with a rolled contract"
        ));
    }
    if def.reward.is_empty() {
        return Some(
            "a contract with no reward pays nothing; give it one or delete the file".to_string(),
        );
    }
    if def
        .reward
        .iter()
        .any(|r| matches!(r, Reward::Credits(0) | Reward::Item(_, 0) | Reward::Xp(0)))
    {
        return Some("a reward of 0 pays nothing; give it at least 1 or delete it".to_string());
    }
    None
}

/// Why `t` cannot be loaded, or `None` if it is fine.
///
/// The reward checks are the authored ones, applied to the **per-unit**
/// figure: a template paying 0 a unit pays nothing however much it asks for.
/// The two that are only a template's problem are an id that would collide
/// with a rolled one, and a `{target}` hole in an objective that names nothing
/// to fill it with — which would otherwise reach a player as the literal text.
fn template_complaint(t: &ContractTemplate) -> Option<String> {
    if t.id.as_str().is_empty() {
        return Some("a contract template needs an id".to_string());
    }
    if t.id.as_str().contains(ROLLED_ID_SEPARATOR) {
        return Some(format!(
            "a contract template id may not contain '{ROLLED_ID_SEPARATOR}': it is \
             the separator a rolled id is built with"
        ));
    }
    if t.reward.is_empty() {
        return Some(
            "a template with no reward pays nothing; give it one or delete the file".to_string(),
        );
    }
    if t.reward
        .iter()
        .any(|r| matches!(r, Reward::Credits(0) | Reward::Item(_, 0) | Reward::Xp(0)))
    {
        return Some(
            "a template reward of 0 pays nothing at any size; give it at least 1 or delete it"
                .to_string(),
        );
    }
    let names_a_target = match t.objective {
        TemplateObjective::Terminate { .. }
        | TemplateObjective::Deliver { .. }
        | TemplateObjective::Build => true,
        TemplateObjective::Descend { .. } | TemplateObjective::Breach { .. } => false,
    };
    if !names_a_target && (t.name.contains("{target}") || t.description.contains("{target}")) {
        return Some(
            "this objective rolls no species, item or structure, so there is \
             nothing to put in a {target} hole"
                .to_string(),
        );
    }
    let range = match t.objective {
        TemplateObjective::Terminate { count } | TemplateObjective::Deliver { count } => {
            Some(count)
        }
        TemplateObjective::Descend { depth } => Some(depth),
        TemplateObjective::Breach { zone } => Some(zone),
        TemplateObjective::Build => None,
    };
    if let Some((lo, hi)) = range
        && lo > hi
    {
        return Some(format!(
            "the range ({lo}, {hi}) is back to front and can roll nothing"
        ));
    }
    None
}
