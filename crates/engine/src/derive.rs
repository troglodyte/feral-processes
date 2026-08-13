//! Turning world state into a stable choice.
//!
//! Several things in the game are a property of *the place* rather than of
//! the moment you looked at it: what a Stack cell says for itself
//! (`descriptions.rs`), which sector a zone is (`sectors.rs`). None of them
//! may draw from `resources::GameRng` — a draw does not survive a save/load
//! and shifts every later roll in the run — and none may lean on an `StdRng`
//! sequence, whose output is not guaranteed stable across a `rand` upgrade.
//! So each folds the values it is derived from and reduces the result to an
//! index, and this module owns the reduction step both of them share.

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
/// carries the measurement, and `sectors::sector_seed` is why the zone
/// number is folded a byte at a time rather than as one word.
///
/// Shared rather than copied because the `%` version passes every casual
/// test: it anti-correlates two small pools perfectly while looking
/// perfectly reasonable, which is a bug found once and worth never finding
/// again.
pub(crate) fn index(seed: u64, len: usize) -> usize {
    ((seed as u128 * len as u128) >> 64) as usize
}
