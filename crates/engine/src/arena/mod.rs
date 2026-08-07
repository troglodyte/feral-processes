//! A scenario-driven harness that runs real battles offline.
//!
//! Pick the opponents and, on a fresh player, the items; run N seeded reps;
//! keep the round-by-round transcript. Difficulty can then be tuned by
//! measurement rather than by playing to the fight.
//!
//! This is inside the engine crate deliberately. `start_battle`,
//! `spawn_wild_creature_scaled` and the `world` field are all reachable from
//! here and from nowhere outside, so the arena adds **no public `Game`
//! method at all** — the compiler barrier keeping the renderer out of the
//! ECS is untouched.
//!
//! Its known blind spot, stated rather than hidden: the party plays the
//! game's own All-Attack, which fires no companion Specials. An arena number
//! is a floor on the party's output, the same gap `balance_sim` has.

mod scenario;

pub use scenario::{CompanionSpec, EquipSpec, InventorySpec, OpponentSpec, PlayerSource, Scenario};
