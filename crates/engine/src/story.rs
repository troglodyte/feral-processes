//! The ending text, loaded from `assets/story/ending.ron`.
//!
//! A list of screens the player pages through after leaving the Basin. The
//! story is data so it can be modded; the rule that the game must stay
//! completable is code, so a missing or malformed file falls back to a short
//! built-in ending rather than an empty one (the `PhaseKeyDb` protection).

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One page of the ending: a title and its paragraphs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndingScreen {
    pub title: String,
    pub body: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct EndingFile {
    screens: Vec<EndingScreen>,
}

/// The screens shown on escape; never empty.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct EndingText {
    screens: Vec<EndingScreen>,
}

impl Default for EndingText {
    fn default() -> Self {
        EndingText {
            screens: vec![EndingScreen {
                title: "Escaped".to_string(),
                body: vec![
                    "You step through the exit and the Phase-Manifold Basin lets go.".to_string(),
                    "The keys stay with you.".to_string(),
                ],
            }],
        }
    }
}

impl EndingText {
    /// Loads `dir/ending.ron`. A missing file is silent and yields the
    /// fallback; a file that does not parse, or lists no screens, warns and
    /// yields the fallback.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let path = dir.join("ending.ron");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok((EndingText::default(), Vec::new()));
            }
            Err(e) => return Err(e),
        };
        let fallback = |why: String| {
            Ok((
                EndingText::default(),
                vec![format!("skipped invalid ending file {path:?}: {why}")],
            ))
        };
        match ron::from_str::<EndingFile>(&text) {
            Ok(file) if file.screens.is_empty() => fallback("it lists no screens".to_string()),
            Ok(file) => Ok((
                EndingText {
                    screens: file.screens,
                },
                Vec::new(),
            )),
            Err(e) => fallback(e.to_string()),
        }
    }

    pub fn screens(&self) -> &[EndingScreen] {
        &self.screens
    }
}
