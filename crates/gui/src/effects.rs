//! The battle-effects library: named looks that weapons and routines select
//! with their `fx` field.
//!
//! Loaded here, never in the engine, for the same reason sprites are: it is
//! purely presentational. The engine only carries an id on its cues. A
//! missing directory leaves the built-in `streak`; a malformed file is
//! skipped with a warning, the `*Db::load_dir` contract.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

use bevy::prelude::*;
use feral_processes_engine::components::GlyphColor;
use serde::Deserialize;

/// The id every unnamed or unknown `fx` resolves to.
pub const DEFAULT_EFFECT: &str = "streak";

/// How an effect crosses the board from attacker to target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub enum Travel {
    #[default]
    Streak,
    Pulses {
        count: u8,
    },
    Beam {
        hold: f32,
    },
    Zap,
    None,
}

/// What lands on the target (or on each covered cell of an area).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub enum Impact {
    Sparks,
    Explosion { radius: f32 },
    Zap,
    Slash,
    Smoke,
}

fn default_impact() -> Vec<Impact> {
    vec![Impact::Sparks]
}

/// One effect file. Every field but `id` defaults to today's plain streak.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct EffectDef {
    pub id: String,
    /// `None` = the attacker's glyph colour.
    #[serde(default)]
    pub color: Option<GlyphColor>,
    #[serde(default)]
    pub muzzle: bool,
    #[serde(default)]
    pub travel: Travel,
    #[serde(default = "default_impact")]
    pub impact: Vec<Impact>,
    #[serde(default)]
    pub shake: f32,
}

impl EffectDef {
    fn streak() -> Self {
        Self {
            id: DEFAULT_EFFECT.to_string(),
            color: None,
            muzzle: false,
            travel: Travel::Streak,
            impact: default_impact(),
            shake: 0.0,
        }
    }
}

/// Every loaded effect by id; always holds `streak`.
#[derive(Resource)]
pub struct EffectLibrary {
    defs: HashMap<String, EffectDef>,
    warned: Mutex<HashSet<String>>,
}

impl EffectLibrary {
    /// Reads every `*.ron` in `dir`, in sorted order so a duplicate id is
    /// resolved the same way on every machine (the later file wins). A
    /// missing directory yields only the built-in `streak`.
    pub fn load_dir(dir: &Path) -> Self {
        let mut defs = HashMap::new();
        defs.insert(DEFAULT_EFFECT.to_string(), EffectDef::streak());
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut paths: Vec<_> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "ron"))
                .collect();
            paths.sort();
            for path in paths {
                let parsed = std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|text| ron::from_str::<EffectDef>(&text).map_err(|e| e.to_string()));
                match parsed {
                    Ok(def) => {
                        defs.insert(def.id.clone(), def);
                    }
                    Err(e) => warn!("skipping effect {}: {e}", path.display()),
                }
            }
        }
        Self {
            defs,
            warned: Mutex::new(HashSet::new()),
        }
    }

    /// The effect named `id`; `None` and unknown ids give `streak`. An
    /// unknown id warns once per id.
    pub fn get(&self, id: Option<&str>) -> &EffectDef {
        let streak = &self.defs[DEFAULT_EFFECT];
        let Some(id) = id else {
            return streak;
        };
        match self.defs.get(id) {
            Some(def) => def,
            None => {
                let first = self
                    .warned
                    .lock()
                    .map(|mut seen| seen.insert(id.to_string()))
                    .unwrap_or(false);
                if first {
                    warn!("unknown effect `{id}`, drawing `{DEFAULT_EFFECT}`");
                }
                streak
            }
        }
    }
}

/// Startup: loads `assets/effects/` beside the sprites.
pub fn load(mut commands: Commands, frontend: Res<crate::Frontend>) {
    let dir = frontend.app.assets_dir().join("effects");
    commands.insert_resource(EffectLibrary::load_dir(&dir));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fp-effects-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_bare_id_is_todays_streak() {
        let def: EffectDef = ron::from_str("(id: \"x\")").unwrap();
        assert_eq!(
            def,
            EffectDef {
                id: "x".into(),
                ..EffectDef::streak()
            }
        );
        assert_eq!(def.travel, Travel::Streak);
        assert_eq!(def.impact, vec![Impact::Sparks]);
    }

    #[test]
    fn a_malformed_file_is_skipped_and_the_rest_load() {
        let dir = scratch("bad");
        std::fs::write(dir.join("bad.ron"), "(id: ").unwrap();
        std::fs::write(
            dir.join("ok.ron"),
            "(id: \"ok\", travel: Pulses(count: 2), shake: 0.5)",
        )
        .unwrap();
        let lib = EffectLibrary::load_dir(&dir);
        assert_eq!(lib.get(Some("ok")).travel, Travel::Pulses { count: 2 });
        assert_eq!(lib.defs.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_directory_leaves_only_streak() {
        let lib = EffectLibrary::load_dir(Path::new("/nonexistent/effects"));
        assert_eq!(lib.defs.len(), 1);
        assert_eq!(lib.get(None).id, "streak");
    }

    #[test]
    fn unknown_ids_fall_back_to_streak_and_warn_once() {
        let lib = EffectLibrary::load_dir(Path::new("/nonexistent/effects"));
        assert_eq!(lib.get(Some("nope")).id, "streak");
        assert_eq!(lib.get(Some("nope")).id, "streak");
        assert_eq!(lib.warned.lock().unwrap().len(), 1);
    }

    #[test]
    fn the_shipped_library_loads_all_six() {
        let lib = EffectLibrary::load_dir(&assets().join("effects"));
        for id in ["streak", "laser_pulse", "beam", "zap", "slash", "explosion"] {
            assert_eq!(lib.get(Some(id)).id, id);
        }
        assert_eq!(lib.defs.len(), 6);
        assert_eq!(
            lib.get(Some("laser_pulse")).travel,
            Travel::Pulses { count: 3 }
        );
        assert_eq!(lib.get(Some("explosion")).shake, 1.0);
    }

    /// Every `fx: Some("id")` in shipped items and abilities names a real
    /// effect, so the draw-time fallback is unreachable for shipped content.
    #[test]
    fn every_assigned_fx_resolves() {
        let lib = EffectLibrary::load_dir(&assets().join("effects"));
        for kind in ["items", "abilities"] {
            for entry in std::fs::read_dir(assets().join(kind)).unwrap().flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "ron") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).unwrap();
                for line in text.lines() {
                    let line = line.trim();
                    let Some(rest) = line.strip_prefix("fx:") else {
                        continue;
                    };
                    let Some(id) = rest.split('"').nth(1) else {
                        continue;
                    };
                    assert!(
                        lib.defs.contains_key(id),
                        "{} names unknown effect `{id}`",
                        path.display()
                    );
                }
            }
        }
    }
}
