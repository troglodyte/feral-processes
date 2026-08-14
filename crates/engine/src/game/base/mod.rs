//! The base: what the player builds, who works it, and what it costs to
//! keep standing.
//!
//! `building` places, upgrades, demolishes and posts; `hauling` walks a
//! posted program to its station and its surplus to a depot; `collect`
//! defines the orthogonal reach a machine and the player both use; and
//! `upkeep` is durability, repair and the GC Entropy Sweep.
//!
//! This is the crate's only nested subsystem directory, and that is
//! deliberate rather than a convention anyone should extend. Combat is
//! seven flat `combat_*.rs` files and the Stack five `stack_*.rs`; the
//! base got a directory because it is the subsystem expected to keep
//! growing. **Do not read it as licence to nest the others** — a later
//! `game/combat/` is its own decision, not a precedent set here.

pub(crate) mod building;
pub(crate) mod collect;
pub(crate) mod hauling;
pub(crate) mod upkeep;
pub(crate) mod work_orders;
