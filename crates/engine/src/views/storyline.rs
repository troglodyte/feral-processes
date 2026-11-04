/// The base AI as the research screen reads it: its level, how far its
/// Analysis may grow on its own, and the state of "Train your AI". `Some`
/// from `Game::base_ai_view` once it has fused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseAiView {
    pub level: u32,
    pub analysis: i32,
    /// The Analysis it stalls at, or `None` past the last level, where it
    /// never stalls.
    pub next_threshold: Option<i32>,
    pub stalled: bool,
    /// Whether "Train your AI" may be chosen: it has stalled.
    pub training_available: bool,
    pub training_selected: bool,
    pub training_progress: u32,
    pub training_cost: u32,
}
