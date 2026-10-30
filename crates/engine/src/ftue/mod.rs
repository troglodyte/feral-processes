//! The first-time user experience: every short message the game shows the
//! first time something happens in a run — a research finishing, a structure
//! standing, an item made, a named game event.
//!
//! **`entries::ENTRIES` is the one file to edit to write a message.** A
//! trigger names a research, structure or item by its asset id, so unlike a
//! `NotificationKind` the list is a table checked by census tests rather
//! than an exhaustive match: an id is a string, and the compiler cannot see
//! a typo in one. `tests::every_entry_names_a_real_asset` is what does.
//!
//! A message shows once per *run*. The run set lives in the save
//! (`FtuePlayed`), the player's "don't show again" list and the guidance
//! switch live in `Profile::ftue`, and `decide` is the pure rule joining
//! them. A key is latched even when nothing is shown, so turning guidance
//! back on mid-run does not flood the player with what they skipped.

mod entries;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::GlyphColor;
use crate::items::ItemId;

pub use entries::ENTRIES;

/// A named game event the engine fires from the site the tutorial lived at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FtueEvent {
    /// The Home is raised and the base floor exists — `Game::place_structure`.
    BaseFounding,
    /// The party drops below the on-ramp — `Game::descend_to`.
    FirstDescent,
    /// A GC Entropy Sweep opens on the base — `Game::run_raid`.
    FirstRaid,
    /// Raiders out of a Hostile town reach the base — `Game::run_town_raid`.
    /// Distinct from `FirstRaid`: the sweep is weather and this is a
    /// consequence, and firing the sweep's copy here would teach the wrong
    /// lesson.
    FirstTownRaid,
    /// A work order is filed and accepted — `Game::queue_work_order`.
    FirstWorkOrder,
    /// The reserve first crosses under `tuning::LOW_POWER_ATTACK_THRESHOLD` —
    /// `Game::note_low_power`, read off the reserve once a tick rather than
    /// fired from a spend, because most runs cross by passive drain in a
    /// bevy system with no `Game`. The threshold is the attack penalty's own,
    /// so the screen cannot say attacks weaken on a tick where they do not.
    LowPower,
    /// A program is benched by a Forgiving death with no Repair Bay
    /// standing — `Game::bench_or_dissolve`. Gated on
    /// `StructureDef::recovery` standing, not the Bay's id.
    DownedProgram,
    /// Static reaches the player — the movement hook in `game/turn.rs`, on
    /// the step where `Terrain::event` is live on the destination tile, not
    /// the biome-wide epoch boundary: a player three biomes away must not be
    /// told something happened to them.
    FirstStatic,
    /// The player extracts their first image —
    /// `Game::extract_image_from_program`.
    LearnedImage,
}

impl FtueEvent {
    pub const ALL: [FtueEvent; 9] = [
        FtueEvent::BaseFounding,
        FtueEvent::FirstDescent,
        FtueEvent::FirstRaid,
        FtueEvent::FirstTownRaid,
        FtueEvent::FirstWorkOrder,
        FtueEvent::LowPower,
        FtueEvent::DownedProgram,
        FtueEvent::FirstStatic,
        FtueEvent::LearnedImage,
    ];

    /// Exhaustive on purpose: the string is a file format (`profile.ron`'s
    /// "don't show again" list and the save's run set), so a new event must
    /// choose its own.
    fn slug(self) -> &'static str {
        match self {
            FtueEvent::BaseFounding => "base_founding",
            FtueEvent::FirstDescent => "first_descent",
            FtueEvent::FirstRaid => "first_raid",
            FtueEvent::FirstTownRaid => "first_town_raid",
            FtueEvent::FirstWorkOrder => "first_work_order",
            FtueEvent::LowPower => "low_power",
            FtueEvent::DownedProgram => "downed_program",
            FtueEvent::FirstStatic => "first_static",
            FtueEvent::LearnedImage => "learned_image",
        }
    }
}

/// What makes an entry fire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FtueTrigger {
    /// That research completes (a research id).
    Research(&'static str),
    /// That structure comes to stand in the base (a structure id).
    Built(&'static str),
    /// That item is made, by any path (an item id).
    Crafted(&'static str),
    Event(FtueEvent),
}

impl FtueTrigger {
    /// The stable string that reaches disk, in the save and in the profile.
    pub fn key(&self) -> String {
        match self {
            FtueTrigger::Research(id) => research_key(id),
            FtueTrigger::Built(id) => built_key(id),
            FtueTrigger::Crafted(id) => crafted_key(id),
            FtueTrigger::Event(event) => format!("event:{}", event.slug()),
        }
    }
}

/// The key a research completing latches under. A free function so a call
/// site holding a runtime id and the `&'static str` in an entry build the
/// same string.
pub fn research_key(id: &str) -> String {
    format!("research:{id}")
}

pub fn built_key(id: &str) -> String {
    format!("built:{id}")
}

pub fn crafted_key(id: &str) -> String {
    format!("crafted:{id}")
}

/// One authored message. Fields mirror `notifications::NotificationDef`.
#[derive(Clone, Copy, Debug)]
pub struct FtueEntry {
    pub on: FtueTrigger,
    /// The heading. Short — it is drawn large and does not wrap.
    pub title: &'static str,
    /// Wrapped at draw time; `\n\n` starts a new paragraph. The panel does
    /// not scroll, so a body must fit — the notification screen's height
    /// test walks this list.
    pub body: &'static str,
    /// A name in `assets/sprites/`; substitutes for `glyph`, and a missing
    /// one falls back to it.
    pub sprite: Option<&'static str>,
    pub glyph: char,
    pub color: GlyphColor,
}

/// The entry for a trigger, if the list has one.
pub fn entry_for(trigger: &FtueTrigger) -> Option<&'static FtueEntry> {
    ENTRIES.iter().find(|entry| entry.on == *trigger)
}

/// What `Profile` keeps: the switch and the per-message opt-outs. Strings,
/// not triggers, so a key a later build retires is inert rather than a parse
/// error that costs the player their whole profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FtueGuidance {
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    #[serde(default)]
    pub never: Vec<String>,
}

fn enabled_default() -> bool {
    true
}

impl Default for FtueGuidance {
    fn default() -> Self {
        Self {
            enabled: true,
            never: Vec::new(),
        }
    }
}

/// The keys already handled this run, shown or not. Saved with the run.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct FtuePlayed(pub BTreeSet<String>);

/// Items the assembler made this tick. The assembler is a bevy system with
/// no `Game` to notify from, so it pushes here and `Game` drains after the
/// tick — the low-Power pattern.
#[derive(Resource, Clone, Debug, Default)]
pub struct PendingCrafted(pub Vec<ItemId>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FtueDecision {
    /// Queue the message, and latch the key.
    Show,
    /// Latch the key, queue nothing: guidance is off or the player opted out.
    LatchOnly,
    /// Already handled this run.
    AlreadyShown,
}

/// The whole rule. Pure: no `Game`, no RNG.
pub fn decide(key: &str, run_shown: &BTreeSet<String>, guidance: &FtueGuidance) -> FtueDecision {
    if run_shown.contains(key) {
        FtueDecision::AlreadyShown
    } else if !guidance.enabled || guidance.never.iter().any(|never| never == key) {
        FtueDecision::LatchOnly
    } else {
        FtueDecision::Show
    }
}
