//! Weapon parts: the catalogue, what fits what, and the pure functions
//! between a design and the `ItemDef` it runs as.

mod bill;
mod catalogue;
mod compile;
mod decompose;
mod fit;
mod names;
mod strike;

pub use bill::{frame_bill, part_bill};
pub use catalogue::{Category, Frame, Proc, Tilt, WeaponPart};
#[cfg(test)]
pub(crate) use compile::exact_band_of;
pub use compile::{compile, item_id};
pub use decompose::decompose;
pub use fit::{FitError, all_designs_up_to_cap, check, frame_of};
pub use strike::exploited;
