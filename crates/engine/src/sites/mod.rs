//! Sites: the towns, cities and outposts the party knows about, and what
//! the Sites hub shows of each.
//!
//! The two stores here, a per-site event log and the last-seen shelf of each
//! town, are plain Rust owned by `Game`, never ECS resources: a new
//! `Resource` shifts bevy's query order and with it every seeded stream
//! (ADR 0042's precedent). They are saved additively behind
//! `#[serde(default)]`. A site is identified by `RouteDestinationId`, the key
//! the route code already uses for both settlements and outposts.
//!
//! Design: `docs/superpowers/specs/2026-11-06-sites-hub-design.md`.

mod log;
mod seen;
mod time;
mod view;

pub use log::*;
pub use seen::*;
pub use time::*;
pub use view::*;

/// Everything the Sites hub remembers that the world does not.
#[derive(Clone, Debug, Default)]
pub struct SiteRecords {
    pub log: SiteLog,
    pub seen: SeenShelves,
}
