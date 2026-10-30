use super::*;

/// One deployed Shield and its circle — see `Game::view_shields`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShieldView {
    pub entity: Entity,
    pub anchor: (i32, i32),
    pub radius: u32,
    /// The radius after the next upgrade; `None` when it cannot be upgraded
    /// further right now.
    pub next_radius: Option<u32>,
    /// Whether it holds the coil that makes it soften a hit. An unarmed
    /// Shield still shows its circle, so the player sees what it would cover.
    pub armed: bool,
    pub raid_defense: u32,
}
