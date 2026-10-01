//! How much a program talks, derived from its id.
//!
//! Not a Component and never saved, `handles::of`'s rule: it is a pure
//! function of the id, so nothing needs migrating when it is retuned. The
//! salt keeps the partition independent of `Disposition::seed`, which folds
//! the same id unsalted; without it every Chatty program would share a
//! disposition and the SOCIAL tab would read as one trait shown twice.

use crate::components::ProgramId;
use crate::derive::{FNV_BASIS, fold, index};
use crate::tuning::{
    SOCIABILITY_CHATTY, SOCIABILITY_RESERVED, SOCIABILITY_SALT, SOCIABILITY_SOCIABLE,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sociability {
    Reserved,
    Sociable,
    Chatty,
}

impl Sociability {
    const ALL: [Sociability; 3] = [
        Sociability::Reserved,
        Sociability::Sociable,
        Sociability::Chatty,
    ];

    pub fn of(id: ProgramId) -> Self {
        // Ends on the salt, which separates this from `Disposition::seed`
        // and mixes the id's otherwise-bare final multiply.
        let h = fold(FNV_BASIS, &[id.0 as u64, SOCIABILITY_SALT]);
        Self::ALL[index(h, Self::ALL.len())]
    }

    pub fn chance_mult(self) -> f64 {
        match self {
            Sociability::Reserved => SOCIABILITY_RESERVED,
            Sociability::Sociable => SOCIABILITY_SOCIABLE,
            Sociability::Chatty => SOCIABILITY_CHATTY,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Sociability::Reserved => "Reserved",
            Sociability::Sociable => "Sociable",
            Sociability::Chatty => "Chatty",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disposition::Disposition;

    #[test]
    fn stable_per_id() {
        for id in [0, 1, 7, 1000, u32::MAX] {
            assert_eq!(
                Sociability::of(ProgramId(id)),
                Sociability::of(ProgramId(id))
            );
        }
    }

    #[test]
    fn every_variant_occurs_at_least_a_fifth_of_the_time() {
        for variant in Sociability::ALL {
            let n = (0..1000)
                .filter(|i| Sociability::of(ProgramId(*i)) == variant)
                .count();
            assert!(n >= 200, "{variant:?} appeared {n} times in 1000");
        }
    }

    #[test]
    fn the_partition_is_not_the_dispositions() {
        let ids: Vec<u32> = (0..200).collect();
        let pairs = || ids.iter().flat_map(|a| ids.iter().map(move |b| (*a, *b)));
        let same_disp_diff_soc = pairs().any(|(a, b)| {
            Disposition::seed(a) == Disposition::seed(b)
                && Sociability::of(ProgramId(a)) != Sociability::of(ProgramId(b))
        });
        let same_soc_diff_disp = pairs().any(|(a, b)| {
            Sociability::of(ProgramId(a)) == Sociability::of(ProgramId(b))
                && Disposition::seed(a) != Disposition::seed(b)
        });
        assert!(same_disp_diff_soc && same_soc_diff_disp);

        for d in Disposition::ALL {
            for soc in Sociability::ALL {
                assert!(
                    (0..1000)
                        .any(|i| Disposition::seed(i) == d && Sociability::of(ProgramId(i)) == soc),
                    "{d:?} never pairs with {soc:?} in 1000 ids"
                );
            }
        }
    }

    #[test]
    fn chatty_talks_more_than_reserved() {
        assert!(Sociability::Chatty.chance_mult() > Sociability::Reserved.chance_mult());
    }
}
