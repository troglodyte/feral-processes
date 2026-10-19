use super::*;

/// One node of the research tree as the menus see it — see
/// `Game::research_nodes`.
pub struct ResearchStatus {
    pub id: ResearchId,
    pub name: String,
    pub description: String,
    pub cost: u32,
    pub state: ResearchState,
    /// The goods this node consumes beside its Research Data, each with
    /// what the party can reach of it right now. Empty for a node that
    /// authored none.
    pub materials: Vec<ResearchMaterial>,
    /// What this node lets the base turn into what, one sentence per
    /// conversion, in the shape "Bytecode Block x3 into Hardened Shell."
    /// Empty for a node that unlocks neither a recipe nor an assembling
    /// structure.
    ///
    /// Derived in the engine rather than authored beside the description —
    /// `ResearchMaterial`'s reason one rung up. A recipe's cost lives in
    /// `assets/items/` (or the node's own `unlocks_recipes`) and a machine's
    /// product in `assets/structures/`, so an authored copy here is the copy
    /// that goes stale the first time either is retuned. See
    /// `Game::research_conversions`.
    pub conversions: Vec<String>,
    /// "Unlocks: Fabricator" — everything this node hands over, by display
    /// name, or `None` for a node that hands over nothing.
    ///
    /// Derived from the four `unlocks_*` lists rather than read off the
    /// description, `conversions`' reason: the prose is authored, and what a
    /// mod's prose says about its own unlocks is whatever the modder wrote.
    /// One line rather than a row per unlock, `ResearchMaterial`'s reason —
    /// `popup_layout` pages this screen by `Row::Item` span. See
    /// `Game::research_unlocks`.
    pub unlocks: Option<String>,
    /// How much of `cost` the base has already fed into this node, in units
    /// of the research currency. Non-zero for a node the player worked and
    /// then abandoned, which is why it is on every row rather than only on
    /// the active one — see `resources::ActiveResearch`.
    pub progress: u32,
    /// The one sentence saying why this base cannot work this node yet, or
    /// `None` if it can — see `Game::research_block`. Not a refusal:
    /// `Game::select_research` takes such a node on anyway, and this is what
    /// the screen flags it with (`!`), on the running project's row too. The
    /// corner readout's `ResearchReadout::Blocked` is the same call, so the
    /// two surfaces cannot name different things.
    pub blocked_by: Option<String>,
    /// Whether this node sits on a path the tree recommends — see
    /// `ResearchDb::recommended_ids`. Independent of `state` for the same
    /// reason `blocked_by` is: a recommended node is still locked until its
    /// prerequisites are researched, and the menu says both things at once.
    pub recommended: bool,
    /// The one routine this node grants, for a synthesised routine node.
    /// `cfg(test)`: read only by engine tests today, neither the renderer
    /// nor app-core touches it, and a `pub(crate)` field that's merely
    /// unread (rather than absent from non-test builds) still warns.
    #[cfg(test)]
    pub(crate) teaches: Option<crate::abilities::AbilityId>,
}

/// What the corner over the map says about research — see
/// `Game::research_readout`.
///
/// Four states because the corner draws four different things, and a
/// progress figure on a waiting project is a number that has stopped moving
/// with nothing saying why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResearchReadout {
    /// A Research Node stands and no project is selected.
    Idle,
    /// The project was taken on with something the base cannot do yet —
    /// `reason` is `Game::research_block`'s sentence, the row's own
    /// `ResearchStatus::blocked_by`, so the `!` here and the `!` on the
    /// research screen name the same thing. Checked ahead of `Stalled` and
    /// whatever the progress, since the wait starts at selection.
    Blocked { name: String, reason: String },
    /// The project is still earning its Research Data.
    Earning {
        name: String,
        earned: u32,
        cost: u32,
    },
    /// The project has all its data and is waiting on `short_of`, the first
    /// material line the base cannot pay — `research_material_shortfall`'s
    /// answer, named.
    Stalled { name: String, short_of: String },
}

/// What the pin mark on one base-space cell says — `Game::view_pinned_at`'s
/// per-cell answer, and the renderer's only way to ask.
///
/// **Two states and not two doors.** `Strained` is `Settled` plus the
/// knowledge that the body is being spent right now, so it is a widening of
/// the one question rather than a second one asked beside it. A separate
/// "which body is straining" door would be a fourth answer to "which body is
/// under study", which is the shape that already shipped wrong once: read off
/// the `components::UnderStudy` marker alone, the mark latched on at
/// selection and rode along for the whole walk to the pen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinMark {
    /// Nothing under study stands here.
    Unpinned,
    /// A subject has settled in its station's pen. The pen's marks, and no
    /// motion: the base is holding it, not working it.
    Settled,
    /// The subject the active project is actually spending, while that
    /// project is earning — `Game::view_pinned_at` has the three terms.
    Strained,
}

/// One line of a research node's material bill — see
/// `ResearchDef::materials`.
///
/// `name` is resolved in the engine through `Game::item_name`, for
/// `Game::copy_name`'s reason: a renderer building the name is how a bill
/// and the refusal that quotes it come to word the same shortfall
/// differently.
///
/// `have` counts what the **base** is holding — every Depot and every machine
/// output buffer, through `work_orders::base_holding` — because that is exactly
/// what `Game::settle_research` spends from. It is deliberately not the
/// player's pack: a project's bill is paid off the shelves by the base, wherever
/// the player happens to be standing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchMaterial {
    pub name: String,
    pub need: u32,
    pub have: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResearchState {
    Unlocked,
    /// The one project the base is working right now — see
    /// `resources::ActiveResearch`. A variant rather than a flag beside the
    /// enum, read the other way round from `Locked`'s two *fields*: these four
    /// really are disjoint, so a `bool` alongside would be a second thing to
    /// keep in step.
    Active,
    Available,
    /// Why a node can't be taken, rather than just greying it out. The two
    /// reasons are independent and both may hold at once — a deep node in a
    /// fresh game owes a prerequisite *and* a breach — so they are two
    /// fields rather than two variants: a variant would force an arbitrary
    /// precedence between them and lose whichever it did not pick.
    Locked {
        /// Display names of the prerequisites still missing.
        missing: Vec<String>,
        /// `Some(n)` when the player's zone is below the node's `min_zone`;
        /// `None` when the zone is satisfied and only prereqs are in the way.
        min_zone: Option<u32>,
    },
}

/// Which way an arrow key moves on the research graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphDir {
    Up,
    Down,
    Left,
    Right,
}

/// One node's place on the research flow chart: which column it sits in and
/// how far down that column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchCell {
    pub id: ResearchId,
    pub tier: usize,
    pub slot: usize,
}

/// The research tree drawn as a grid — a cell per node, the edge list, and
/// the two figures a renderer needs to size a box.
///
/// Keyed by `ResearchId` throughout and never by index into
/// `Game::research_nodes()`, which re-sorts by `ResearchState` as the player
/// buys things.
///
/// `tier` is the *longest* path from a root, so every edge points strictly
/// rightward: a node with one shallow parent and one deep one takes the deep
/// one's depth, and the short leg stretches across the gap rather than
/// pointing backwards.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResearchGraph {
    /// In `(tier, slot)` order.
    pub cells: Vec<ResearchCell>,
    /// (prerequisite, dependent), both `ResearchId`.
    pub edges: Vec<(ResearchId, ResearchId)>,
    /// One past the deepest tier; 0 for an empty tree.
    pub tiers: usize,
    /// The slot count of the most crowded tier; 0 for an empty tree.
    pub widest: usize,
}

impl ResearchGraph {
    /// A linear scan of the cells. The shipped tree is 34 nodes and this is
    /// called a handful of times a frame; an index map would be the
    /// optimization ahead of evidence this repo's principles name.
    pub fn cell(&self, id: &str) -> Option<&ResearchCell> {
        self.cells.iter().find(|c| c.id == id)
    }

    /// Where an arrow key lands from `from`. Total: an id with no cell, or a
    /// step off the edge of the grid, returns `from` itself.
    ///
    /// Up and down clamp rather than wrapping. A list wraps because its ends
    /// are adjacent on screen; a column's are at opposite edges of the pane,
    /// so a wrap there reads as the cursor teleporting.
    pub fn step(&self, from: &str, dir: GraphDir) -> ResearchId {
        let Some(cell) = self.cell(from) else {
            return from.to_string();
        };
        let landed = match dir {
            GraphDir::Up | GraphDir::Down => {
                let want = match dir {
                    GraphDir::Up => cell.slot.checked_sub(1),
                    _ => Some(cell.slot + 1),
                };
                want.and_then(|slot| {
                    self.cells
                        .iter()
                        .find(|c| c.tier == cell.tier && c.slot == slot)
                })
            }
            GraphDir::Left | GraphDir::Right => {
                let target = match dir {
                    GraphDir::Left => cell.tier.checked_sub(1),
                    _ => Some(cell.tier + 1),
                };
                target.filter(|t| *t < self.tiers).and_then(|target| {
                    self.cells
                        .iter()
                        .filter(|c| c.tier == target)
                        // The nearest slot, ties toward the lower one, so
                        // the walk is reversible in the common case and a
                        // held arrow key does not drift.
                        .min_by_key(|c| (c.slot.abs_diff(cell.slot), c.slot))
                })
            }
        };
        landed
            .map(|c| c.id.clone())
            .unwrap_or_else(|| from.to_string())
    }
}
