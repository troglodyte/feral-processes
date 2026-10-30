use super::*;

/// One deployed Patch Node and its circle — see `Game::view_patch_nodes`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchNodeView {
    pub entity: Entity,
    pub anchor: (i32, i32),
    pub radius: u32,
    /// The radius after the next upgrade; `None` when it cannot be upgraded
    /// further right now.
    pub next_radius: Option<u32>,
}
