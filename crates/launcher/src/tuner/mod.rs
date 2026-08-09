//! Offline roster tuning: propose species stats that hit authored fight
//! targets, by measuring real arena fights rather than by feel.
//!
//! Nothing here ships in the game. The engine does not know this exists —
//! the entire seam is `arena::run`'s `assets_dir` parameter, so a candidate
//! roster is measured by writing it to a scratch directory and running the
//! real fight against it. No engine change, no runtime cost, no dependency
//! the player pays for.
//!
//! **The output is a proposal, never an edit.** Tuned files land under
//! `dev-tuning/out/` for a human to diff against `assets/species/` and apply
//! by hand. An unattended process rewriting game content is not something
//! this tool does.
//!
//! Two limits are worth knowing before reading a number out of it, and both
//! are properties of the arena rather than of the search. The headless arena
//! plays the game's own All-Attack every round, so **no companion Specials
//! ever fire** and ability magnitudes go unmeasured — every measurement here
//! is a *floor* on what a real party outputs. And the seed set is pinned for
//! a whole run so candidates are compared on identical fights, which invites
//! overfitting to those seeds; that is what the held-out set exists to catch.

pub mod constraints;
pub mod objective;
pub mod roster;
pub mod score;
