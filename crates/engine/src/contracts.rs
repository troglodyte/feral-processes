//! Contracts: named, finite objectives with a payout, issued by a Contract
//! Broker.
//!
//! Contracts are data, like achievements and species — `assets/contracts/*.ron`,
//! one file per contract, so adding one is a file drop. What is *not* data is
//! how a board is derived and how many may be held at once, which live in
//! `tuning.rs` beside every other knob.

use std::collections::BTreeMap;
use std::path::Path;

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

#[derive(Resource, Default)]
pub struct ContractDb {
    defs: BTreeMap<ContractId, ContractDef>,
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
        Ok((db, warnings))
    }

    pub fn get(&self, id: &ContractId) -> Option<&ContractDef> {
        self.defs.get(id)
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
