//! Where a manifest subject stands among everyone on the rail.
//!
//! `rank` is pure over a snapshot of values so the place and pip rules are
//! unit-tested without a `World`; `Game::subject_ranks` only gathers the
//! snapshot.

use crate::components::Potential;
use crate::views::{RankRow, RankedStat, SubjectRanks};
use crate::*;

/// The pips on a compare cell's ruler.
const MAX_PIPS: u8 = 5;

/// `subject`'s place among `values`, or `None` when it is not one of them.
///
/// Ties share a place (1st, 1st, 3rd): a place is one more than the count of
/// values strictly above. Pips read the spread, not the place, so a runaway
/// leader and a photo finish look different: `1 + floor(4 * (v - min) /
/// (max - min))`, and a full ruler when nothing differs, since "no weaker
/// than anyone" is not a reason to draw a low bar.
pub(crate) fn rank(stat: RankedStat, values: &[(Entity, f32)], subject: Entity) -> Option<RankRow> {
    let (_, mine) = values.iter().find(|(e, _)| *e == subject)?;
    let place = 1 + values.iter().filter(|(_, v)| v > mine).count() as u32;
    let min = values.iter().map(|(_, v)| *v).fold(f32::INFINITY, f32::min);
    let max = values
        .iter()
        .map(|(_, v)| *v)
        .fold(f32::NEG_INFINITY, f32::max);
    let pips = if max > min {
        1 + (f32::from(MAX_PIPS - 1) * (mine - min) / (max - min)).floor() as u8
    } else {
        MAX_PIPS
    };
    Some(RankRow {
        stat,
        place,
        of: values.len() as u32,
        pips,
    })
}

impl Game {
    /// Every stat `entity` is ranked on, against everyone `manifest_subjects`
    /// returns; `None` for a body that is not on the rail. Potential and
    /// work aptitude exist only on programs, so they rank among programs and
    /// the player gets no row for them.
    pub fn subject_ranks(&mut self, entity: Entity) -> Option<SubjectRanks> {
        let subjects = self.manifest_subjects();
        if !subjects.contains(&entity) {
            return None;
        }
        let mut damage = Vec::new();
        let mut attack = Vec::new();
        let mut mitigation = Vec::new();
        let mut power = Vec::new();
        let mut potential = Vec::new();
        let mut aptitude = Vec::new();
        for &subject in &subjects {
            // The sheet's own figures, so a rank cannot disagree with the
            // number printed beside it (the player's are gear-adjusted).
            let Some(view) = self.manifest(subject) else {
                continue;
            };
            damage.push((subject, self.natural_range_of(subject).mean() as f32));
            attack.push((subject, view.atk as f32));
            mitigation.push((subject, view.mitigation as f32));
            power.push((subject, view.power as f32));
            if let Some(p) = self.world.get::<Potential>(subject) {
                potential.push((subject, p.quality_percent() as f32));
                aptitude.push((subject, (p.assembly_roll + p.extraction_roll) / 2.0));
            }
        }
        let rows = [
            (RankedStat::Damage, damage),
            (RankedStat::Attack, attack),
            (RankedStat::Mitigation, mitigation),
            (RankedStat::Power, power),
            (RankedStat::Potential, potential),
            (RankedStat::WorkAptitude, aptitude),
        ]
        .into_iter()
        .filter_map(|(stat, values)| rank(stat, &values, entity))
        .collect();
        Some(SubjectRanks { rows })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: u32) -> Vec<Entity> {
        (0..n)
            .map(Entity::from_raw_u32)
            .map(Option::unwrap)
            .collect()
    }

    fn snapshot(values: &[f32]) -> Vec<(Entity, f32)> {
        ids(values.len() as u32)
            .into_iter()
            .zip(values.iter().copied())
            .collect()
    }

    fn rank_of(values: &[f32], index: usize) -> RankRow {
        let snap = snapshot(values);
        rank(RankedStat::Power, &snap, snap[index].0).unwrap()
    }

    #[test]
    fn ties_share_a_place_and_the_next_skips() {
        let values = [9.0, 9.0, 4.0];
        assert_eq!(rank_of(&values, 0).place, 1);
        assert_eq!(rank_of(&values, 1).place, 1);
        assert_eq!(rank_of(&values, 2).place, 3);
        assert_eq!(rank_of(&values, 2).of, 3);
    }

    #[test]
    fn all_equal_gives_a_full_ruler_and_first_place() {
        for i in 0..3 {
            let row = rank_of(&[5.0, 5.0, 5.0], i);
            assert_eq!((row.place, row.pips), (1, 5));
        }
    }

    #[test]
    fn a_single_member_is_first_of_one_with_a_full_ruler() {
        let row = rank_of(&[3.0], 0);
        assert_eq!((row.place, row.of, row.pips), (1, 1, 5));
    }

    #[test]
    fn pips_run_from_one_at_the_minimum_to_five_at_the_maximum() {
        let values = [0.0, 5.0, 10.0, 2.4, 2.6];
        assert_eq!(rank_of(&values, 0).pips, 1);
        assert_eq!(rank_of(&values, 2).pips, 5);
        assert_eq!(rank_of(&values, 1).pips, 3, "the midpoint");
        assert_eq!(rank_of(&values, 3).pips, 1, "floors, so 0.96 is still 1");
        assert_eq!(rank_of(&values, 4).pips, 2);
    }

    #[test]
    fn a_subject_outside_the_snapshot_has_no_rank() {
        let snap = snapshot(&[1.0, 2.0]);
        let stranger = ids(3)[2];
        assert!(rank(RankedStat::Damage, &snap, stranger).is_none());
    }
}
