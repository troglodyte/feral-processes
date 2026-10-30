//! Creating, saving, and restoring a `Game`.
//!
//! `new_with` (which `new` is a one-line delegation to) and `load` are the
//! only two doors into a playable world, and both go through
//! `load_asset_dbs` so neither can produce a `Game` whose item set fails the
//! economy-role check.

mod assets;
mod convert;
mod load;
mod new;
mod profile;
mod restore;
mod save;
mod spawn_saved;

pub(crate) use assets::{AssetDbs, load_asset_dbs};
pub(crate) use spawn_saved::CreatureRestore;
