//! A program's display identity, derived from `components::ProgramId` and
//! never stored.
//!
//! There is no `Handle` component and no mint at load: `of` is a pure
//! function of the id, so a save written before this feature gets handles
//! the moment it loads (its programs' `ProgramId`s already exist).
//!
//! Handles count up in assignment order — the first program ever minted is
//! `0x000000`, the next `0x000001` — so a handle doubles as "which one you
//! got first". **The offset is save format:** changing it renames every
//! program in every save that exists.

use crate::components::ProgramId;
use crate::resources::NextProgramId;

/// `"0x"` plus six uppercase hex digits, counting from `0x000000` at
/// `NextProgramId::START`. Ids are minted from `START`, never below it, so
/// the offset keeps `ProgramId(0)` as the unassigned sentinel it already is
/// rather than spending the zero handle on it. An id at or past 2^24 + 1
/// prints more digits — that is 16.7M programs in a single run.
pub fn of(id: ProgramId) -> String {
    format!("0x{:06X}", id.0.wrapping_sub(NextProgramId::START.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first program ever minted (`NextProgramId::START`) reads
    /// `0x000000` and each after it counts up by one — the whole feature.
    #[test]
    fn handles_count_up_from_zero() {
        assert_eq!(of(ProgramId(1)), "0x000000");
        assert_eq!(of(ProgramId(2)), "0x000001");
        assert_eq!(of(ProgramId(3)), "0x000002");
        assert_eq!(of(ProgramId(11)), "0x00000A");
        assert_eq!(of(ProgramId(4096)), "0x000FFF");
        assert_eq!(of(ProgramId(16_777_216)), "0xFFFFFF");
    }

    #[test]
    fn the_first_minted_id_is_the_zero_handle() {
        assert_eq!(
            of(ProgramId(crate::resources::NextProgramId::START.0)),
            "0x000000"
        );
    }
}
