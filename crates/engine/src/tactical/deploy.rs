//! Putting the two sides on the board.
//!
//! The board is sized for manoeuvre; this is sized for contact. The two
//! sides go down `TACTICAL_DEPLOY_GAP` cells apart on the bearing the pack
//! was actually found at, so walking into a pack from the side starts you
//! flanked and no fight opens with a march.

use std::collections::{BTreeSet, VecDeque};

use crate::tactical::footprint_cells_at;
use crate::tactical::map::Board;
use crate::tuning::TACTICAL_DEPLOY_GAP;

/// The eight-way step from one tile toward another.
///
/// A signum per axis rather than an angle: at these board sizes eight
/// directions are all the resolution a deployment can express, and it keeps
/// the whole thing integer. Two bodies on one tile answer north — a fixed
/// answer rather than a panic, because a wild body standing exactly where
/// the player stands is a bump, not a bug.
pub fn bearing(from: (i32, i32), to: (i32, i32)) -> (i32, i32) {
    match ((to.0 - from.0).signum(), (to.1 - from.1).signum()) {
        (0, 0) => (0, -1),
        step => step,
    }
}

/// Where each side's bodies start.
///
/// Two lists rather than a map of entity to cell: this module never sees an
/// `Entity`, which is what keeps every test over it pure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deployment {
    pub party: Vec<(i32, i32)>,
    pub wild: Vec<(i32, i32)>,
}

/// Seats both sides.
///
/// The two anchors sit `TACTICAL_DEPLOY_GAP` apart either side of the
/// board's centre along `bearing`, and each side's rank fans out
/// perpendicular to it, nearest the anchor first. A body whose place is
/// blocked or already taken takes the nearest free block of its own
/// footprint instead.
///
/// `wild` is one footprint per body (`1` for an ordinary body, a
/// formation's `footprint` for a squad) rather than a count — the party
/// side never fields anything wider than one cell, so it stays a plain
/// count.
///
/// The board always has room: the smallest tier is 14x14 with at least 86%
/// of it walkable and the largest fight the game fields is
/// `MAX_PARTY_SIZE + MAX_PACK_BODIES` bodies, which
/// `the_smallest_board_seats_the_largest_fight` pins — at footprint 1 for
/// every body, the shape squads narrow toward but never past.
pub fn plan(board: &Board, bearing: (i32, i32), party: u32, wild: &[u8]) -> Deployment {
    let centre = board.side / 2;
    let half = TACTICAL_DEPLOY_GAP / 2;
    let party_anchor = (centre - bearing.0 * half, centre - bearing.1 * half);
    let wild_anchor = (centre + bearing.0 * half, centre + bearing.1 * half);
    let across = (-bearing.1, bearing.0);

    let party_footprints = vec![1u8; party as usize];
    let mut taken = BTreeSet::new();
    Deployment {
        party: rank(board, party_anchor, across, &party_footprints, &mut taken),
        wild: rank(board, wild_anchor, across, wild, &mut taken),
    }
}

/// One side's line, fanning out from its anchor: the anchor itself, then a
/// cell to either side of it, then two, and so on. `footprints[i]` is the
/// side of the clear NxN block the `i`-th body needs — `1` for an ordinary
/// body.
fn rank(
    board: &Board,
    anchor: (i32, i32),
    across: (i32, i32),
    footprints: &[u8],
    taken: &mut BTreeSet<(i32, i32)>,
) -> Vec<(i32, i32)> {
    footprints
        .iter()
        .enumerate()
        .map(|(i, &footprint)| {
            let step = fan(i as u32);
            let want = (anchor.0 + across.0 * step, anchor.1 + across.1 * step);
            let cell = nearest_free(board, taken, want, footprint)
                .expect("the board has room for every body; see `plan`'s doc");
            taken.extend(footprint_cells_at(cell, footprint));
            cell
        })
        .collect()
}

/// 0, +1, -1, +2, -2, … — the anchor first, then out alternately, so a
/// short rank is centred on its anchor rather than trailing off one side.
fn fan(i: u32) -> i32 {
    let step = (i as i32 + 1) / 2;
    if i % 2 == 1 { step } else { -step }
}

/// The nearest anchor to `want`, breadth-first, whose `footprint`-cell block
/// is entirely walkable, in bounds and unclaimed — `nearest_free`'s general
/// form, `footprint: 1` being the one-cell case every caller but a squad's
/// own seating wants.
///
/// `want` is clamped onto the board first: a long rank on a diagonal
/// bearing runs its outer bodies off the edge, and a search that starts
/// outside has nowhere to start from.
pub(crate) fn nearest_free(
    board: &Board,
    taken: &BTreeSet<(i32, i32)>,
    want: (i32, i32),
    footprint: u8,
) -> Option<(i32, i32)> {
    let from = (
        want.0.clamp(0, board.side - 1),
        want.1.clamp(0, board.side - 1),
    );
    let mut seen = BTreeSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some(at) = queue.pop_front() {
        let cells = footprint_cells_at(at, footprint);
        if cells.iter().all(|&(x, y)| {
            board.in_bounds(x, y) && board.walkable(x, y) && !taken.contains(&(x, y))
        }) {
            return Some(at);
        }
        for (dx, dy) in crate::tactical::map::NEIGHBOURS {
            let next = (at.0 + dx, at.1 + dy);
            if board.in_bounds(next.0, next.1) && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tactical::map::{BattleSpec, generate};
    use crate::world::Biome;

    fn board(bodies: u32) -> Board {
        generate(BattleSpec {
            world_seed: 31,
            site: (2, 2),
            tick: 44,
            zone: 2,
            biome: Biome::Backplane,
            bodies,
        })
    }

    #[test]
    fn a_bearing_is_the_eight_way_step_from_one_tile_toward_another() {
        assert_eq!(bearing((0, 0), (5, 0)), (1, 0));
        assert_eq!(bearing((0, 0), (0, -3)), (0, -1));
        assert_eq!(bearing((4, 4), (1, 9)), (-1, 1));
    }

    /// Two bodies on one tile is not a direction. A fixed answer rather
    /// than a panic, because a wild body standing exactly where the player
    /// stands is a bump, not a bug.
    #[test]
    fn a_bearing_to_your_own_tile_is_north() {
        assert_eq!(bearing((3, 3), (3, 3)), (0, -1));
    }

    #[test]
    fn everybody_gets_their_own_walkable_cell() {
        let board = board(9);
        let plan = plan(&board, (1, 0), 4, &[1; 5]);
        assert_eq!(plan.party.len(), 4);
        assert_eq!(plan.wild.len(), 5);

        let mut all: Vec<(i32, i32)> = plan.party.iter().chain(plan.wild.iter()).copied().collect();
        let placed = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), placed, "two bodies were deployed onto one cell");
        for (x, y) in all {
            assert!(board.in_bounds(x, y), "({x}, {y}) is off the board");
            assert!(board.walkable(x, y), "({x}, {y}) cannot be stood on");
        }
    }

    /// Sized for contact, not for a walk — but not on top of each other
    /// either. The anchors are `TACTICAL_DEPLOY_GAP` apart along the
    /// bearing and the ranks spread perpendicular to it, so the closest
    /// pair is the gap less whatever drift a taken cell forced.
    #[test]
    fn the_two_sides_start_apart_on_every_bearing() {
        let board = board(9);
        for bearing in [
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (1, -1),
            (-1, -1),
        ] {
            let plan = plan(&board, bearing, 4, &[1; 5]);
            let closest = plan
                .party
                .iter()
                .flat_map(|p| {
                    plan.wild
                        .iter()
                        .map(move |w| (p.0 - w.0).abs().max((p.1 - w.1).abs()))
                })
                .min()
                .expect("an empty deployment");
            assert!(
                closest >= 2,
                "bearing {bearing:?} deployed the sides at {closest}"
            );
        }
    }

    /// The bearing is what makes a flank a flank: come at a pack from a
    /// different quarter and the two sides stand somewhere else.
    #[test]
    fn a_different_bearing_deploys_a_different_fight() {
        let board = board(9);
        assert_ne!(
            plan(&board, (1, 0), 4, &[1; 5]).party,
            plan(&board, (0, 1), 4, &[1; 5]).party
        );
    }

    #[test]
    fn the_same_board_and_bearing_deploy_identically() {
        let board = board(5);
        assert_eq!(
            plan(&board, (1, 1), 2, &[1; 3]).party,
            plan(&board, (1, 1), 2, &[1; 3]).party
        );
        assert_eq!(
            plan(&board, (1, 1), 2, &[1; 3]).wild,
            plan(&board, (1, 1), 2, &[1; 3]).wild
        );
    }

    /// The largest fight the game can field, on the smallest board it will
    /// ever be fought on, still seats everybody — which is the invariant
    /// `place` leans on when it says the board has room.
    #[test]
    fn the_smallest_board_seats_the_largest_fight() {
        let party = crate::tuning::MAX_PARTY_SIZE as u32;
        let wild = crate::tuning::MAX_PACK_BODIES;
        let board = board(2);
        assert_eq!(board.side, crate::tuning::TACTICAL_BOARD_SMALL);
        let footprints = vec![1u8; wild as usize];
        let plan = plan(&board, (1, 0), party, &footprints);
        assert_eq!(plan.party.len() + plan.wild.len(), (party + wild) as usize);
    }

    /// A footprint wider than one cell is seated on a clear block of its
    /// own size, not merely on a clear single cell inside it.
    #[test]
    fn a_wider_body_is_seated_on_a_clear_block_of_its_own_size() {
        let board = board(4);
        let plan = plan(&board, (1, 0), 1, &[2]);
        assert_eq!(plan.wild.len(), 1);
        for (x, y) in footprint_cells_at(plan.wild[0], 2) {
            assert!(board.in_bounds(x, y), "({x}, {y}) is off the board");
            assert!(board.walkable(x, y), "({x}, {y}) cannot be stood on");
        }
    }

    /// Mixed footprints on one side never overlap each other, whatever
    /// order they are handed in.
    #[test]
    fn mixed_footprints_on_one_side_never_overlap() {
        let board = board(9);
        let plan = plan(&board, (1, 0), 1, &[2, 1, 1, 2]);
        let mut all: Vec<(i32, i32)> = Vec::new();
        for (&anchor, &footprint) in plan.wild.iter().zip(&[2u8, 1, 1, 2]) {
            all.extend(footprint_cells_at(anchor, footprint));
        }
        let placed = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), placed, "two footprints overlapped a cell");
    }
}
