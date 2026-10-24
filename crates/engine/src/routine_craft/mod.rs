//! Routine crafting (ADR 0029): the Rust part catalogue a player's routines
//! are built from, and the pure functions between a build and the
//! `AbilityDef` it runs as. Nothing here touches `Game`.
//!
//! - `catalogue`: the parts, what fits what, the part cap, materials.
//! - `compile`: the one place parts become numbers.
//! - `decompose`: the inverse, for extraction and boss drops.
//! - `describe`: the plain-language line a build previews and a def carries.

mod catalogue;
mod compile;
mod decompose;
mod describe;
mod plan;

pub use catalogue::{
    AreaKind, BuildError, Core, CraftedRoutine, Facing, RangeKind, StatusKind, Subroutine, Version,
    fits, material, validate_build,
};
pub use compile::compile;
pub use decompose::decompose;
pub use describe::describe;
pub use plan::{
    BUILD_SLOTS, PlanError, build_rows, check_name, part_options, parts_missing, parts_used,
    plan_build,
};

#[cfg(test)]
mod tests;
