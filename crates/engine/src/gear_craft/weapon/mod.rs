//! Weapon parts: the catalogue, what fits what, and the pure functions
//! between a design and the `ItemDef` it runs as.

mod catalogue;
mod compile;
mod decompose;
mod fit;

pub use catalogue::{Category, Frame, Proc, Tilt, WeaponPart};
#[cfg(test)]
pub(crate) use compile::exact_band_of;
pub use compile::{compile, item_id};
pub use decompose::decompose;
pub use fit::{FitError, all_designs_up_to_cap, check, frame_of};
