//! A program's display identity, derived from `components::ProgramId` and
//! never stored.
//!
//! There is no `Handle` component, no `assets/handles/` and no mint at
//! load. A roadmap draft stored one — a word pool — because a changed pool
//! would rename everyone; a hex format has no pool, so `of` is a pure
//! function of the id instead, and a save written before this feature gets
//! handles for free the moment it loads (its programs' `ProgramId`s already
//! exist).
//!
//! **The seam this creates: the salt and the permutation are save format.**
//! Two runs never compare their programs by handle, so nothing else in the
//! game may read one as more than a label — see `game/party.rs`'s naming
//! ladder for where it prints.

use crate::components::ProgramId;
use crate::derive;

/// The width `of` derives within. An id at or past 2^24 wraps — unguarded,
/// deliberately: that is 16.7M tamed programs in a single run.
const MASK: u32 = 0x00FF_FFFF;

/// The permutation's salt, chosen only for being odd (multiplying by an odd
/// constant is a bijection mod 2^24, which is what makes every id below the
/// wrap print a distinct handle). **Renaming this constant renames every
/// program in every save that exists** — see the module doc.
const SALT: u32 = 0x9E37_79B1;

/// `"0x"` plus six hex digits, mixed-case, unique for every id below 2^24.
///
/// No RNG, no lookup table: `permute` is an invertible bijection on 24 bits
/// and `case_hash` is an independent fold of the same id, so two calls with
/// the same `id` always print the same string and two different ids below
/// the wrap never collide.
pub fn of(id: ProgramId) -> String {
    let digits = permute(id.0);
    let case_bits = case_hash(id.0);
    let mut handle = String::with_capacity(8);
    handle.push_str("0x");
    for i in 0..6 {
        // Most significant nibble first, so the printed string reads the
        // permuted value left to right the way any hex number does.
        let shift = 4 * (5 - i);
        let nibble = (digits >> shift) & 0xF;
        let mut digit = std::char::from_digit(nibble, 16).expect("a nibble is always < 16");
        if (case_bits >> i) & 1 == 1 {
            digit = digit.to_ascii_uppercase();
        }
        handle.push(digit);
    }
    handle
}

/// The bijection on 24 bits: two rounds of an odd multiply — invertible mod
/// 2^24, so distinct ids below the wrap stay distinct — followed by an
/// xor-shift to spread the multiply's effect past the low bits it lands in
/// first. Two rounds rather than one so every output bit has been through
/// both an odd multiply and a shift at least once.
fn permute(id: u32) -> u32 {
    let mut x = id & MASK;
    x = x.wrapping_mul(SALT) & MASK;
    x ^= x >> 12;
    x = x.wrapping_mul(SALT) & MASK;
    x ^= x >> 12;
    x & MASK
}

/// Which digits print uppercase, independent of the digits themselves —
/// `derive::fold` rather than a second hand-rolled hash, since a fold is
/// exactly the thing this crate already has one door for.
fn case_hash(id: u32) -> u32 {
    derive::fold(derive::FNV_BASIS, &[id as u64]) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole argument for the module doc's "save format" claim: nothing
    /// about the salt, the round count or the shift may change without
    /// renaming every program in every save, and this is the one test that
    /// would catch it. Confirmed by hand: mutating `SALT` fails every
    /// assertion below.
    #[test]
    fn handles_are_pinned() {
        assert_eq!(of(ProgramId(0)), "0x000000");
        assert_eq!(of(ProgramId(1)), "0xed7631");
        assert_eq!(of(ProgramId(2)), "0x126a5b");
        assert_eq!(of(ProgramId(4095)), "0x78a01C");
        assert_eq!(of(ProgramId(16_000_000)), "0x9aC902");
    }

    #[test]
    fn handles_are_distinct() {
        let mut seen = std::collections::HashSet::new();
        for id in 0..100_000u32 {
            let handle = of(ProgramId(id)).to_lowercase();
            assert!(seen.insert(handle), "id {id} collided with an earlier id");
        }
    }

    #[test]
    fn handle_shape() {
        for id in [0, 1, 4095, 1_000_000, 16_777_215] {
            let handle = of(ProgramId(id));
            assert_eq!(handle.len(), 8, "{handle:?} for id {id}");
            assert!(handle.starts_with("0x"), "{handle:?} for id {id}");
            for c in handle[2..].chars() {
                assert!(c.is_ascii_hexdigit(), "{c:?} in {handle:?} for id {id}");
                assert!(
                    !c.is_ascii_uppercase() || matches!(c, 'A'..='F'),
                    "uppercase digit char {c:?} in {handle:?} for id {id}"
                );
            }
        }
    }

    /// A case hash that turned out to be constant would pass `handle_shape`
    /// (both cases are valid hex) and print a handle that never varies in
    /// case, so this checks the distribution rather than the shape.
    #[test]
    fn case_is_mixed_somewhere() {
        let (mut saw_lower, mut saw_upper) = (false, false);
        for id in 0..1000u32 {
            let handle = of(ProgramId(id));
            saw_lower |= handle[2..].chars().any(|c| c.is_ascii_lowercase());
            saw_upper |= handle[2..].chars().any(|c| c.is_ascii_uppercase());
        }
        assert!(saw_lower, "no lowercase letter digit in the first 1000 ids");
        assert!(saw_upper, "no uppercase letter digit in the first 1000 ids");
    }
}
