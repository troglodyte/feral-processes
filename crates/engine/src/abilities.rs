use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::{BuffKind, StatusKind};
use crate::species::MoveEffect;

pub type AbilityId = String;

/// The ability every companion falls back to when its species declares
/// none. Validated at startup (see `Game::new`) rather than defended at
/// every call site, the same way a missing economy role aborts the load.
pub const FALLBACK_ABILITY_ID: &str = "priority_boost";

/// Who an ability lands on. Which picker the UI opens for it — if any — is
/// `AbilityTarget::targeting`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityTarget {
    /// One party member the player picks.
    OneAlly,
    /// Every living party member, no picker.
    WholeParty,
    /// The front member of one enemy group the player picks.
    OneEnemyGroupFront,
    /// Every living member of one enemy group the player picks.
    WholeEnemyGroup,
    /// Every living enemy in every group, no picker.
    AllEnemies,
}

/// What an ability does to each of its recipients.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AbilityEffect {
    /// Direct damage through `battle::compute_damage`, so it scales with the
    /// user's ATK exactly as a `MoveDef` does, plus an optional status rider
    /// — the same shape a move already has.
    Damage {
        power: i32,
        #[serde(default)]
        status: Option<MoveEffect>,
    },
    Heal {
        power: i32,
    },
    Buff {
        kind: BuffKind,
        power: i32,
        duration: u32,
    },
    Debuff {
        kind: StatusKind,
        power: i32,
        duration: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AbilityDef {
    pub id: AbilityId,
    pub name: String,
    /// The one-line detail the ability picker shows. Authored rather than
    /// computed from `effect`, so a modder controls exactly how their
    /// ability reads.
    pub description: String,
    pub target: AbilityTarget,
    pub effect: AbilityEffect,
    /// Battle rounds before this ability can be used again by the same
    /// combatant. `#[serde(default)]` — 0 means usable every round.
    #[serde(default)]
    pub cooldown: u32,
    /// Player Fatigue spent commanding this ability. `#[serde(default)]` to
    /// the flat cost commanding a companion has always charged, so an
    /// ability omitting it behaves as before.
    #[serde(default = "default_fatigue_cost")]
    pub fatigue_cost: f32,
}

fn default_fatigue_cost() -> f32 {
    crate::COMPANION_COMMAND_FATIGUE_COST
}

impl AbilityDef {
    /// Names the first field holding a NaN or infinity, if any. RON accepts
    /// bare `NaN`/`inf` literals and they survive every clamp downstream —
    /// cheaper to refuse the file at load than to defend every read. Same
    /// rationale as `ItemDef::non_finite_field`.
    fn non_finite_field(&self) -> Option<&'static str> {
        if !self.fatigue_cost.is_finite() {
            return Some("fatigue_cost");
        }
        if let AbilityEffect::Damage {
            status: Some(status),
            ..
        } = &self.effect
            && !status.chance.is_finite()
        {
            return Some("effect.status.chance");
        }
        None
    }
}

#[derive(Resource, Default)]
pub struct AbilityDb {
    abilities: HashMap<AbilityId, AbilityDef>,
}

impl AbilityDb {
    /// Loads every `*.ron` ability in `dir`. A malformed file is skipped
    /// with a returned warning rather than aborting the load, same as
    /// `ItemDb::load_dir`.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = AbilityDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<AbilityDef>(&text) {
                Ok(def) => {
                    if let Some(field) = def.non_finite_field() {
                        warnings.push(format!(
                            "skipped invalid ability file {path:?}: {field} is not a finite number"
                        ));
                        continue;
                    }
                    db.abilities.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid ability file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&AbilityDef> {
        self.abilities.get(id)
    }

    /// Every loaded ability, by id. `HashMap` iteration order is randomized
    /// per instance, so without this the picker's numbering would shuffle
    /// between sessions even though nothing about the files changed.
    pub fn all(&self) -> impl Iterator<Item = &AbilityDef> {
        let mut defs: Vec<&AbilityDef> = self.abilities.values().collect();
        defs.sort_by(|a, b| a.id.cmp(&b.id));
        defs.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `files` as `.ron` into a fresh temp dir and loads an
    /// `AbilityDb` from it.
    fn load(tag: &str, files: &[(&str, &str)]) -> (AbilityDb, Vec<String>) {
        let dir =
            std::env::temp_dir().join(format!("feral_abilities_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(format!("{name}.ron")), body).unwrap();
        }
        let result = AbilityDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    const VALID: &str = r#"(
        id: "test_sweep",
        name: "Test Sweep",
        description: "Damage 6 to one group.",
        target: WholeEnemyGroup,
        effect: Damage(power: 6),
    )"#;

    #[test]
    fn a_valid_def_loads_with_defaulted_optional_fields() {
        let (db, warnings) = load("valid", &[("test_sweep", VALID)]);
        let def = db.get("test_sweep").expect("valid ability should load");
        assert_eq!(def.name, "Test Sweep");
        assert_eq!(def.target, AbilityTarget::WholeEnemyGroup);
        assert_eq!(def.cooldown, 0, "cooldown defaults to none");
        assert_eq!(
            def.fatigue_cost,
            crate::COMPANION_COMMAND_FATIGUE_COST,
            "an ability declaring no cost charges what commanding always did"
        );
        assert!(warnings.is_empty(), "a valid def warns about nothing");
    }

    #[test]
    fn a_malformed_file_is_skipped_with_a_warning_and_the_rest_still_load() {
        let (db, warnings) = load(
            "malformed",
            &[("test_sweep", VALID), ("broken", "(this is not ron")],
        );
        assert!(
            db.get("test_sweep").is_some(),
            "one bad mod file must not take the others down"
        );
        assert_eq!(warnings.len(), 1, "exactly the bad file should warn");
        assert!(warnings[0].contains("broken"));
    }

    #[test]
    fn all_is_ordered_by_id() {
        let b =
            r#"(id: "b", name: "B", description: "d", target: OneAlly, effect: Heal(power: 1))"#;
        let a =
            r#"(id: "a", name: "A", description: "d", target: OneAlly, effect: Heal(power: 1))"#;
        let (db, _) = load("order", &[("b", b), ("a", a)]);
        let ids: Vec<&str> = db.all().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["a", "b"],
            "HashMap order is randomized per instance; the menu must not be"
        );
    }

    #[test]
    fn the_shipped_set_loads_clean() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join("abilities");
        let (db, warnings) = AbilityDb::load_dir(&dir).unwrap();
        assert!(
            warnings.is_empty(),
            "the shipped set must not warn: {warnings:?}"
        );
        assert_eq!(db.all().count(), 10, "10 abilities ship with the game");
        assert!(
            db.get(FALLBACK_ABILITY_ID).is_some(),
            "the fallback ability must ship, or every companion loses its Special"
        );
    }
}
