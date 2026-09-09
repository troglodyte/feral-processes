//! Turning world state into a stable choice.
//!
//! Several things in the game are a property of *the place* rather than of
//! the moment you looked at it: what a Stack cell says for itself
//! (`descriptions.rs`), what a base-space coordinate is made of
//! (`rock.rs`). None of them
//! may draw from `resources::GameRng` — a draw does not survive a save/load
//! and shifts every later roll in the run — and none may lean on an `StdRng`
//! sequence, whose output is not guaranteed stable across a `rand` upgrade.
//! So each folds the values it is derived from and reduces the result to an
//! index, and this module owns **both** steps: `fold` mixes, `index`
//! reduces, and no caller may hand-write either.

/// FNV-1a's 64-bit offset basis — where a fold with nothing behind it
/// starts.
pub(crate) const FNV_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Continues an FNV-1a fold with further words, **one byte at a time**.
///
/// The counterpart to `index`: that function reduces, this one mixes, and
/// the two are always used together. Shared rather than copied because a
/// fold is precisely the thing a comment cannot keep in sync — three call
/// sites held a character-for-character copy of this loop before it was
/// extracted (`stack::FrameSpec::salted`, `rock::block_seed`,
/// `disposition::seed`), and `tactical::BattleSpec` would have been the
/// fourth.
///
/// Byte-at-a-time rather than XOR-ing each word in whole and multiplying
/// through the prime once, because `index` reads the **high** bits. A
/// whole-word XOR gets exactly one multiply round to spread it, and one
/// round cannot carry a low-bit difference much past the prime's own width
/// (`FNV_PRIME` is ~41 bits) before the fold ends — measured against
/// `FrameSpec::salted`'s history, a whole-word fold leaves many of the 64
/// output bits, including several of the highest, identical across most
/// adjacent input pairs, with the bottom bit alternating in lockstep with
/// one input's parity instead. This is a property of the fold, not a fixed
/// count: three separate measurements came back 21/64 with the top 6 fixed,
/// 22/64 with the top 8, and 23/64 with the top 7, depending on which pairs
/// were sampled, so no single number is asserted by a test. `[a, b]`
/// diverging from `[b, a]` is not the same claim as "adjacent inputs cannot
/// rhyme" — both can be true at once, because `assert_ne!` needs only one
/// bit of difference. Giving every byte its own XOR-then-multiply pass
/// leaves no output bit a fixed function of the input.
pub(crate) fn fold(seed: u64, words: &[u64]) -> u64 {
    let mut h = seed;
    for &word in words {
        for byte in word.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
    }
    h
}

/// Reduces `seed` to an index in `0..len`, for `len > 0`.
///
/// Lemire's `(seed as u128 * len) >> 64` rather than `seed % len`: `%` for a
/// two-entry pool reads nothing but `seed`'s lowest bit, and the
/// multiply-by-odd-prime step every caller's fold ends on provably never
/// disturbs that bit once it is set. Reading the *high* bits instead uses
/// the ones the repeated multiplication actually mixes, so a two-entry pool
/// decorrelates from its neighbours the same as a larger one instead of
/// standing out as the one size that silently breaks the independence a fold
/// promises.
///
/// **Reaching those high bits is not automatic, and is the caller's
/// problem.** One XOR-then-multiply round carries a difference only about
/// the prime's own width (~41 bits) upward, so a value folded in as the
/// *last* word, differing only in its low bits, never reaches bit 63 — which
/// is the bit this function actually reads. `descriptions::Slot::tags`
/// carries the measurement, and `rock::block_seed` is why a block
/// coordinate is folded a byte at a time rather than as one word.
///
/// Shared rather than copied because the `%` version passes every casual
/// test: it anti-correlates two small pools perfectly while looking
/// perfectly reasonable, which is a bug found once and worth never finding
/// again.
pub(crate) fn index(seed: u64, len: usize) -> usize {
    ((seed as u128 * len as u128) >> 64) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The specification of the fold, written out longhand rather than
    /// called: this test is what pins the three call sites that used to
    /// hold their own copy of this loop, so an "optimisation" that folds
    /// whole words again fails here rather than silently re-correlating
    /// every derived value in the game.
    #[test]
    fn fold_is_fnv_1a_one_byte_at_a_time() {
        let mut expected = FNV_BASIS;
        for word in [7_u64, 0xdead_beef_u64] {
            for byte in word.to_le_bytes() {
                expected ^= byte as u64;
                expected = expected.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        assert_eq!(fold(FNV_BASIS, &[7, 0xdead_beef]), expected);
    }

    /// Salting is what `FrameSpec::salted` needs of it: continuing an
    /// existing fold has to equal folding the whole list at once, or a
    /// cell's seed would depend on how many calls it took to build.
    #[test]
    fn fold_continues_from_a_seed_rather_than_restarting() {
        assert_eq!(fold(FNV_BASIS, &[1, 2]), fold(fold(FNV_BASIS, &[1]), &[2]),);
    }

    #[test]
    fn fold_distinguishes_the_order_of_its_words() {
        assert_ne!(fold(FNV_BASIS, &[1, 2]), fold(FNV_BASIS, &[2, 1]));
    }

    /// The property `index` depends on and the reason the fold is
    /// byte-wise: a difference in the low bits of the *last* word has to
    /// reach the bit `index` actually reads.
    ///
    /// Swept rather than asserted on one pair, because a single pair is a
    /// coin flip — bit 63 differing is what a *mixed* bit does half the
    /// time, so one pair agreeing proves nothing and one pair differing
    /// proves nothing either. What separates the two folds is the rate:
    /// measured over these 64 adjacent pairs, byte-at-a-time flips bit 63
    /// in 33 of them and leaves none of the top 8 bits constant, while
    /// XOR-ing each word in whole flips it in **0** and leaves all 8
    /// constant. The floor is well under the measured 33 because the exact
    /// count is a property of the fold and not a number worth pinning; it
    /// is well over the whole-word fold's 0 because that is the failure
    /// this test exists to catch.
    #[test]
    fn a_low_bit_difference_in_the_last_word_reaches_the_top_bit() {
        let flips = (0..64_u64)
            .filter(|n| fold(FNV_BASIS, &[99, *n]) >> 63 != fold(FNV_BASIS, &[99, n + 1]) >> 63)
            .count();
        assert!(
            flips >= 8,
            "bit 63 moved in only {flips} of 64 adjacent pairs; the fold is not reaching \
             the bit `index` reads"
        );
    }
}
