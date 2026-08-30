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

/// Something the player did, recorded for `Objective::Perform`.
///
/// A **closed engine enum, not a string**. A deed is an engine *event*, not
/// content: a mod cannot emit one, so the openness a string would buy is
/// openness onto nothing. What a string would buy instead is a mission
/// naming a deed that does not exist, loading with no warning and never
/// completing — the failure the README already documents for a `Terminate`
/// naming a species that is gone, and one there is no reason to repeat where
/// the vocabulary is closed.
///
/// A deed carries **no parameters**. `QueuedStandingOrder` does not name the
/// item and `PostedStaff` does not name the structure: the mission's
/// description is where the player is told what to order and where to post,
/// and a parameterised deed would be a second place the same instruction is
/// written. A mission that genuinely has to tell two postings apart is a new
/// variant here, not a field on an existing one.
///
/// Every variant must have a caller of `Game::note_deed` — asserted
/// exhaustively by `every_deed_has_an_emit_site`, `cell_mark`'s rule, so a
/// variant with no writer fails the build rather than shipping a mission
/// that can never complete.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Deed {
    /// `x` found something. `Game::find_target_in_direction`.
    Examined,
    /// A decompile succeeded. `Game::attempt_decompile`.
    Tamed,
    /// The transfer screen moved something *out* of a container.
    /// `Game::transfer_items`.
    TookFromContainer,
    /// A work order was queued with `standing` set.
    /// `Game::queue_work_order`.
    QueuedStandingOrder,
    /// A Perk Point was spent. `Game::unlock_perk`.
    UnlockedPerk,
    /// A machine was set to be kept staffed. `Game::set_standing_job` —
    /// the player's own key, not `post_worker`, which is the scheduler's.
    PostedStaff,
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
    /// This many of an item are in the player's pack **at once**.
    ///
    /// Not `Deliver`: nothing is handed over and nothing is spent, so it
    /// needs no Broker and can be met four frames down. That is why it
    /// exists — the onboarding chain has to teach that fighting pays in
    /// stock before a Contract Broker has been built.
    ///
    /// State-shaped and **latched**, like `Build` and `Descend`: once met it
    /// stays met, so spending the stock on the next thing the chain asks for
    /// does not un-finish it.
    Hold { item: ItemId, count: u32 },
    /// The player did a particular thing. The one event-shaped objective
    /// besides `Terminate`, and the whole of the onboarding chain's new
    /// vocabulary — six verbs behind one variant, because a variant each
    /// would grow every match on `Objective` and make the seventh verb a
    /// schema change.
    Perform { deed: Deed },
}

/// Everything about the run a state-shaped objective can be asked against.
///
/// A struct rather than positional arguments because `Objective::already_met`
/// has two readers that must not drift — `contract_system` advances by it and
/// `Game::offerable` refuses a board slot on it — so every objective added
/// widened one signature at two call sites. A field costs neither, and the
/// next objective costs a field.
pub struct ObjectiveState {
    /// Stack depth, read from `resources::Locale` and never from `Position`,
    /// which is pinned to the surface entrance tile while underground.
    pub depth: u32,
    pub zone: u32,
    /// Every deployed structure's kind.
    pub standing: Vec<StructureId>,
    /// What the player is carrying, for `Objective::Hold`.
    pub carried: Vec<(ItemId, u32)>,
}

impl ObjectiveState {
    /// Units of `item` in the pack, 0 if none — carrying nothing of it is the
    /// common case, not an error.
    pub fn count(&self, item: &ItemId) -> u32 {
        self.carried
            .iter()
            .find(|(i, _)| i == item)
            .map(|(_, q)| *q)
            .unwrap_or(0)
    }
}

impl Objective {
    /// Units of progress that complete this objective: `count` for the two
    /// counting variants, 1 for the three state-shaped ones — so every
    /// contract displays and completes through one `progress >= target()`
    /// rule and no caller branches on the variant to ask "am I done".
    pub fn target(&self) -> u32 {
        match self {
            Objective::Terminate { count, .. } | Objective::Deliver { count, .. } => *count,
            Objective::Descend { .. }
            | Objective::Breach { .. }
            | Objective::Build { .. }
            | Objective::Hold { .. }
            | Objective::Perform { .. } => 1,
        }
    }

    /// Whether the run already meets this objective, so accepting it would
    /// pay out on the spot.
    ///
    /// The one statement of it, and it has two readers that must not drift:
    /// `contract_system` advances the state-shaped objectives by exactly
    /// this, and `Game::offerable` refuses to put one on the board while it is
    /// already true. They were one expression in the system alone until a
    /// board was read against the `contracts` template and offered
    /// *Stand Up a Refinery* to a base with a Refinery standing in it — 45
    /// Credits, 5 Power Cells and 140 XP for pressing a key — and offered
    /// *Reach sector 3* to a run already in sector 3.
    ///
    /// The event-shaped objectives are never already met: `Terminate` and
    /// `Deliver` because a contract asking for zero of something is refused
    /// at load, and `Perform` because a deed is a thing that happens rather
    /// than a state the run is in — a board would otherwise refuse to offer
    /// one forever.
    ///
    /// The run's side of the question is one `ObjectiveState` rather than a
    /// widening argument list, because with two readers every objective
    /// added cost a signature change at both.
    pub fn already_met(&self, state: &ObjectiveState) -> bool {
        match self {
            // Event-shaped, so never *already* true: a board would otherwise
            // refuse to offer one forever.
            Objective::Terminate { .. } | Objective::Deliver { .. } | Objective::Perform { .. } => {
                false
            }
            Objective::Descend { depth } => state.depth >= *depth,
            Objective::Breach { zone } => state.zone >= *zone,
            Objective::Build { structure } => state.standing.contains(structure),
            Objective::Hold { item, count } => state.count(item) >= *count,
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
    /// Whether this is one of the jobs a run is onboarded with. An unfinished
    /// starter takes a board slot ahead of everything else — see
    /// `Game::board_defs`. A `min_zone` of 0 is not the same thing and cannot
    /// stand in for it: the board draws its three uniformly, so a starter that
    /// merely *may* be offered is a coin flip rather than the first thing a
    /// player is handed.
    ///
    /// Authored only. A template has no such field, so a rolled contract is
    /// never a starter — the arc is a written sequence, not whatever this
    /// sector happened to supply.
    #[serde(default)]
    pub starter: bool,
    /// Which step of the onboarding chain this mission is, if any. Absent on
    /// an ordinary contract, which is every shipped contract but eleven.
    ///
    /// A **step, not an index**: the shipped missions are spaced 10 apart so
    /// inserting one later never renumbers the others. The chain itself is
    /// `ContractDb::tutorial_chain`, and the run's position in it is derived
    /// from `ActiveContracts::done` rather than stored — see
    /// `Game::ensure_tutorial_held`.
    ///
    /// Refused at load beside `starter` or `repeatable`: a tutorial mission
    /// is never offered, so a board-slot flag on one is a claim about
    /// something that cannot happen, and a repeatable one would leave and
    /// re-enter the chain forever.
    ///
    /// `min_zone` is not refused here but is inert — nothing gates a mission
    /// the player is handed.
    #[serde(default)]
    pub tutorial: Option<u32>,
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
        self.fill(&mut Choose::Random(rng), pools)
    }

    /// The same roll taken at its **widest** — the longest-named candidate and
    /// the top of every range.
    ///
    /// Exists for the renderer's width census, which has to measure the widest
    /// row the shipped assets can ever build rather than whichever one a seed
    /// happened to produce. It shares `fill` with the real roll rather than
    /// repeating its five arms, because a census measuring a second copy of
    /// the construction is measuring the copy rather than the game.
    pub fn widest(&self, pools: &TemplatePools) -> Option<ContractDef> {
        self.fill(&mut Choose::Widest, pools)
    }

    fn fill(&self, choose: &mut Choose, pools: &TemplatePools) -> Option<ContractDef> {
        let (objective, slug, target_name, magnitude) = match &self.objective {
            TemplateObjective::Terminate { count } => {
                let (id, name) = choose.pick(&pools.species)?;
                let count = choose.draw(*count, 1)?;
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
                let (id, name) = choose.pick(&pools.items)?;
                let count = choose.draw(*count, 1)?;
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
                let depth = choose.draw(*depth, 1)?;
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
                let zone = choose.draw(*zone, pools.zone.saturating_add(1))?;
                (
                    Objective::Breach { zone },
                    format!("z{zone}"),
                    String::new(),
                    zone,
                )
            }
            TemplateObjective::Build => {
                let (id, name) = choose.pick(&pools.structures)?;
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
            starter: false,
            tutorial: None,
        })
    }
}

/// How a template's free variables get their values. The two callers differ in
/// exactly this and agree on everything else, so it is a parameter rather than
/// a second copy of `fill`'s five arms.
enum Choose<'a> {
    /// A board's offer.
    Random(&'a mut StdRng),
    /// The renderer's width census.
    Widest,
}

impl Choose<'_> {
    /// One candidate from a pool, or `None` when the sector supplies none.
    fn pick<'p, T>(&mut self, pool: &'p [(T, String)]) -> Option<&'p (T, String)> {
        match self {
            Choose::Random(rng) => {
                (!pool.is_empty()).then(|| &pool[rng.random_range(0..pool.len())])
            }
            Choose::Widest => pool.iter().max_by_key(|(_, name)| name.chars().count()),
        }
    }

    /// A number from an inclusive authored range, raised to `floor` first.
    /// `None` when the floor has eaten the range — a real answer rather than an
    /// error: a `Breach(2, 6)` template simply has nothing to offer in sector 6.
    fn draw(&mut self, (lo, hi): (u32, u32), floor: u32) -> Option<u32> {
        let lo = lo.max(floor);
        if lo > hi {
            return None;
        }
        Some(match self {
            Choose::Random(rng) => rng.random_range(lo..=hi),
            // The top of the range is the most digits it can print.
            Choose::Widest => hi,
        })
    }
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
        // Sorted before parsing, `MemoryDb::load_dir`'s rule: two files
        // claiming one id — or, now, one tutorial step — have to resolve the
        // same way on every machine, and `read_dir` gives no such promise.
        let mut paths: Vec<std::path::PathBuf> = entries
            .map(|e| e.map(|e| e.path()))
            .collect::<std::io::Result<_>>()?;
        paths.sort();
        for path in paths {
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
                    None if def.tutorial.is_some()
                        && db.defs.values().any(|d| d.tutorial == def.tutorial) =>
                    {
                        warnings.push(format!(
                            "skipped invalid contract file {path:?}: tutorial step {} is \
                             already taken",
                            def.tutorial.expect("guarded by is_some above")
                        ))
                    }
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

    /// The onboarding chain: every def carrying a `tutorial` step, in step
    /// order. The one derivation of what the chain is.
    ///
    /// Sorted by step and then by id. The second key is unreachable while
    /// `load_dir` refuses a duplicate step; it is here so the order is total
    /// on its own rather than resting on that refusal.
    pub fn tutorial_chain(&self) -> Vec<&ContractDef> {
        let mut chain: Vec<&ContractDef> = self
            .defs
            .values()
            .filter(|d| d.tutorial.is_some())
            .collect();
        chain.sort_by(|a, b| (a.tutorial, &a.id).cmp(&(b.tutorial, &b.id)));
        chain
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
    if def.tutorial.is_some() && def.starter {
        return Some(
            "a tutorial mission is handed to the player, never offered, so it cannot \
             also be a starter — a starter flag on one claims a board slot it can \
             never occupy"
                .to_string(),
        );
    }
    if def.tutorial.is_some() && def.repeatable {
        return Some(
            "a tutorial mission cannot be repeatable: the chain's position is derived \
             from what has been finished, so a repeatable one would leave and re-enter \
             it forever"
                .to_string(),
        );
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
