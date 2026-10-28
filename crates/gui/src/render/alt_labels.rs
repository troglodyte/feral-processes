//! Where the Alt-held labels sit: room names and machine product names laid
//! out greedily in tile units. Pure — no `Painter`, no `Game` — so overlap
//! and leader rules are testable without a frame.

use std::collections::HashSet;

/// Farthest ring (in tiles from the machine) a machine label may be pushed to
/// before it is dropped. The gui has no `tuning/`, so the constant lives here.
const LEADER_RADIUS: i32 = 6;

/// An axis-aligned box in tile units, `x`/`y` its top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TileRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl TileRect {
    fn overlaps(&self, o: &TileRect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }

    fn contains(&self, o: &TileRect) -> bool {
        o.x >= self.x
            && o.y >= self.y
            && o.x + o.w <= self.x + self.w
            && o.y + o.h <= self.y + self.h
    }

    /// The whole tiles this box touches.
    fn tiles(&self) -> impl Iterator<Item = (i32, i32)> {
        let (x0, x1) = (self.x.floor() as i32, (self.x + self.w).ceil() as i32);
        let (y0, y1) = (self.y.floor() as i32, (self.y + self.h).ceil() as i32);
        (y0..y1).flat_map(move |y| (x0..x1).map(move |x| (x, y)))
    }

    fn clamp(&self, p: (f32, f32)) -> (f32, f32) {
        (
            p.0.clamp(self.x, self.x + self.w),
            p.1.clamp(self.y, self.y + self.h),
        )
    }
}

type Segment = ((f32, f32), (f32, f32));

pub(crate) enum LabelKind<'a> {
    /// A room name over the room's cells, preferably on `centre`'s row.
    Room {
        cells: &'a [(i32, i32)],
        centre: (i32, i32),
    },
    /// A machine's product name beside the tile `at`.
    Machine { at: (i32, i32) },
}

/// A label to place; `w`/`h` are tiles and include the backing-box padding.
pub(crate) struct LabelRequest<'a> {
    pub kind: LabelKind<'a>,
    pub w: f32,
    pub h: f32,
}

pub(crate) struct LayoutInput<'a> {
    /// Rooms first, then machines by (y, x); earlier labels win contested spots.
    pub labels: &'a [LabelRequest<'a>],
    /// Tiles a label should avoid covering: glyphs and structure footprints.
    pub soft: &'a HashSet<(i32, i32)>,
    pub bounds: TileRect,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Placement {
    Dropped,
    At {
        rect: TileRect,
        /// Box edge to the machine's tile centre, set only for a ringed spot.
        leader: Option<Segment>,
    },
}

struct Candidate {
    rect: TileRect,
    leader: Option<Segment>,
}

/// How many room cells a label centred on `at` can sit over along its row,
/// counting `at` itself. An L-shaped room's widest row is not the one its
/// label lands on.
fn room_row_cols(cells: &[(i32, i32)], at: (i32, i32)) -> i32 {
    let has = |x: i32| cells.contains(&(x, at.1));
    let run = |step: i32| {
        (1..)
            .map(|d| at.0 + step * d)
            .take_while(|&x| has(x))
            .count() as i32
    };
    // The label is centred on `at`, so only the shorter side bounds it.
    1 + 2 * run(-1).min(run(1))
}

/// Whether a room label `text_w` tiles wide sits inside `cols` cells of room.
/// Text never clips or wraps, so a label that does not fit is dropped.
fn room_label_fits(text_w: f32, cols: i32) -> bool {
    text_w <= cols as f32
}

/// Place every label, greedily in input order. The result is index-aligned
/// with `input.labels`.
pub(crate) fn layout(input: &LayoutInput) -> Vec<Placement> {
    let mut boxes: Vec<TileRect> = Vec::new();
    let mut leaders: Vec<Segment> = Vec::new();
    input
        .labels
        .iter()
        .map(|req| {
            let cands = match req.kind {
                LabelKind::Room { cells, centre } => room_candidates(cells, centre, req),
                LabelKind::Machine { at } => machine_candidates(at, req),
            };
            let pick = cands
                .into_iter()
                .filter(|c| input.bounds.contains(&c.rect))
                .filter(|c| !boxes.iter().any(|b| b.overlaps(&c.rect)))
                .filter(|c| !leaders.iter().any(|l| segment_crosses_rect(*l, &c.rect)))
                .filter(|c| {
                    c.leader
                        .is_none_or(|l| !boxes.iter().any(|b| segment_crosses_rect(l, b)))
                })
                // `min_by_key` keeps the first of equals, so candidate order
                // is the tie-break: clean beats soft-covering, then a leader
                // crossing no other leader beats one that does.
                .min_by_key(|c| {
                    let soft = c.rect.tiles().any(|t| input.soft.contains(&t));
                    let crosses = c
                        .leader
                        .is_some_and(|l| leaders.iter().any(|o| segments_cross(l, *o)));
                    (soft, crosses)
                });
            match pick {
                Some(c) => {
                    boxes.push(c.rect);
                    leaders.extend(c.leader);
                    Placement::At {
                        rect: c.rect,
                        leader: c.leader,
                    }
                }
                None => Placement::Dropped,
            }
        })
        .collect()
}

/// One candidate per room row, nearest `centre`'s row first (ties: above).
/// The centre's own row keeps the label centred on `centre` when it fits, so a
/// room with no machine near is labelled where it always was; every row
/// otherwise centres on its widest run nearest `centre`. Never a leader.
fn room_candidates(cells: &[(i32, i32)], centre: (i32, i32), req: &LabelRequest) -> Vec<Candidate> {
    let mut rows: Vec<i32> = cells.iter().map(|c| c.1).collect();
    rows.sort_unstable();
    rows.dedup();
    rows.sort_by_key(|&y| ((y - centre.1).abs(), y));
    let mut out = Vec::new();
    for y in rows {
        if y == centre.1
            && cells.contains(&centre)
            && room_label_fits(req.w, room_row_cols(cells, centre))
        {
            out.push(room_box(centre.0 as f32 + 0.5, y, req));
        }
        if let Some((start, len)) = widest_run(cells, y, centre.0)
            && room_label_fits(req.w, len)
        {
            out.push(room_box(start as f32 + len as f32 / 2.0, y, req));
        }
    }
    out
}

fn room_box(cx: f32, row: i32, req: &LabelRequest) -> Candidate {
    Candidate {
        rect: TileRect {
            x: cx - req.w / 2.0,
            y: row as f32 + 0.5 - req.h / 2.0,
            w: req.w,
            h: req.h,
        },
        leader: None,
    }
}

/// (first x, length) of the longest unbroken run of cells on row `y`; ties go
/// to the run whose middle is nearest `near_x`.
fn widest_run(cells: &[(i32, i32)], y: i32, near_x: i32) -> Option<(i32, i32)> {
    let mut xs: Vec<i32> = cells.iter().filter(|c| c.1 == y).map(|c| c.0).collect();
    xs.sort_unstable();
    let mut runs: Vec<(i32, i32)> = Vec::new();
    for x in xs {
        match runs.last_mut() {
            Some((s, l)) if *s + *l == x => *l += 1,
            _ => runs.push((x, 1)),
        }
    }
    runs.into_iter().min_by_key(|&(s, l)| {
        let mid2 = 2 * s + l; // twice the run's middle, kept integral
        (-l, (mid2 - 2 * near_x - 1).abs())
    })
}

/// Machine spots in preference order: the four adjacent ones with no leader
/// (right, left, above, below), then for each ring 2..=`LEADER_RADIUS` the
/// same four axes followed by the diagonals NE, NW, SE, SW, each with a
/// leader. Ring `r` leaves a gap of `r - 1` tiles between machine and box.
fn machine_candidates(at: (i32, i32), req: &LabelRequest) -> Vec<Candidate> {
    let (x, y) = (at.0 as f32, at.1 as f32);
    let centre = (x + 0.5, y + 0.5);
    let (w, h) = (req.w, req.h);
    let mut out = Vec::new();
    for ring in 1..=LEADER_RADIUS {
        let g = (ring - 1) as f32;
        let (right, left) = (x + 1.0 + g, x - g - w);
        let (above, below) = (y - g - h, y + 1.0 + g);
        let (mid_x, mid_y) = (centre.0 - w / 2.0, centre.1 - h / 2.0);
        let spots = [
            (right, mid_y),
            (left, mid_y),
            (mid_x, above),
            (mid_x, below),
            (right, above),
            (left, above),
            (right, below),
            (left, below),
        ];
        // Ring 1 has no diagonals: those touch the machine only at a corner.
        let n = if ring == 1 { 4 } else { spots.len() };
        for &(bx, by) in &spots[..n] {
            let rect = TileRect { x: bx, y: by, w, h };
            let leader = (ring > 1).then(|| (rect.clamp(centre), centre));
            out.push(Candidate { rect, leader });
        }
    }
    out
}

/// Whether the segment passes through the open interior of `r`; touching an
/// edge or corner does not count, so a leader may start on its own box.
fn segment_crosses_rect(s: Segment, r: &TileRect) -> bool {
    let (d, p) = ((s.1.0 - s.0.0, s.1.1 - s.0.1), s.0);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (pk, qk) in [
        (-d.0, p.0 - r.x),
        (d.0, r.x + r.w - p.0),
        (-d.1, p.1 - r.y),
        (d.1, r.y + r.h - p.1),
    ] {
        if pk == 0.0 {
            if qk <= 0.0 {
                return false;
            }
        } else {
            let t = qk / pk;
            if pk < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    t1 - t0 > 1e-4
}

fn segments_cross(a: Segment, b: Segment) -> bool {
    let cross = |o: (f32, f32), p: (f32, f32), q: (f32, f32)| {
        (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0)
    };
    let (d1, d2) = (cross(b.0, b.1, a.0), cross(b.0, b.1, a.1));
    let (d3, d4) = (cross(a.0, a.1, b.0), cross(a.0, a.1, b.1));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIG: TileRect = TileRect {
        x: -50.0,
        y: -50.0,
        w: 100.0,
        h: 100.0,
    };

    fn machine(at: (i32, i32)) -> LabelRequest<'static> {
        LabelRequest {
            kind: LabelKind::Machine { at },
            w: 3.0,
            h: 1.0,
        }
    }

    fn room<'a>(cells: &'a [(i32, i32)], centre: (i32, i32), w: f32) -> LabelRequest<'a> {
        LabelRequest {
            kind: LabelKind::Room { cells, centre },
            w,
            h: 1.0,
        }
    }

    fn run(labels: &[LabelRequest], soft: &[(i32, i32)]) -> Vec<Placement> {
        let soft: HashSet<_> = soft.iter().copied().collect();
        layout(&LayoutInput {
            labels,
            soft: &soft,
            bounds: BIG,
        })
    }

    fn rect(p: &Placement) -> TileRect {
        match p {
            Placement::At { rect, .. } => *rect,
            Placement::Dropped => panic!("dropped"),
        }
    }

    fn leader(p: &Placement) -> Option<Segment> {
        match p {
            Placement::At { leader, .. } => *leader,
            Placement::Dropped => panic!("dropped"),
        }
    }

    fn grid(w: i32, h: i32) -> Vec<(i32, i32)> {
        (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).collect()
    }

    #[test]
    fn a_room_label_wider_than_its_room_is_dropped() {
        assert!(room_label_fits(5.0, 5));
        assert!(!room_label_fits(5.1, 5));
    }

    /// An L: a row of five over a column of one. The label sits on the
    /// column's row, so the room's five-wide top must not make it fit.
    #[test]
    fn a_label_is_measured_against_its_own_row() {
        let l: Vec<(i32, i32)> = (0..5).map(|x| (x, 0)).chain([(0, 1)]).collect();
        assert_eq!(room_row_cols(&l, (2, 0)), 5);
        assert_eq!(room_row_cols(&l, (0, 1)), 1);
        // Centred on the run's end cell, the label has no room past it.
        assert_eq!(room_row_cols(&l, (0, 0)), 1);
        assert_eq!(room_row_cols(&l, (1, 0)), 3);
        assert!(!room_label_fits(5.0, room_row_cols(&l, (0, 1))));
        assert!(room_label_fits(5.0, room_row_cols(&l, (2, 0))));
    }

    #[test]
    fn a_lone_machine_gets_the_right_hand_spot_with_no_line() {
        let out = run(&[machine((10, 10))], &[(10, 10)]);
        let r = rect(&out[0]);
        assert_eq!((r.x, r.y), (11.0, 10.0));
        assert_eq!(leader(&out[0]), None);
    }

    #[test]
    fn with_the_right_blocked_it_gets_the_left() {
        let out = run(&[machine((10, 10))], &[(11, 10)]);
        assert_eq!(rect(&out[0]).x, 7.0);
        assert_eq!(leader(&out[0]), None);
    }

    #[test]
    fn with_all_four_adjacent_spots_blocked_it_is_ringed_with_a_leader() {
        let soft = [(11, 10), (9, 10), (10, 9), (10, 11)];
        let out = run(&[machine((10, 10))], &soft);
        let r = rect(&out[0]);
        let (from, to) = leader(&out[0]).expect("a ringed spot has a leader");
        assert_eq!(to, (10.5, 10.5));
        assert_eq!(from, r.clamp(to));
        assert!(!r.tiles().any(|t| soft.contains(&t)));
    }

    #[test]
    fn two_adjacent_machines_never_overlap() {
        let ats = [(10, 10), (11, 10)];
        let out = run(&[machine(ats[0]), machine(ats[1])], &ats);
        assert!(!rect(&out[0]).overlaps(&rect(&out[1])));
    }

    #[test]
    fn a_room_moves_to_another_row_when_a_machine_claims_its_centre_row() {
        let cells = grid(6, 3);
        // The machine comes first, so its right-hand label takes row 1 of x 6..9
        // and the room's centre-row box (x 1..5) is free; push it onto the room
        // by putting the machine inside the room instead.
        let labels = [machine((0, 1)), room(&cells, (3, 1), 4.0)];
        let out = run(&labels, &[]);
        let r = rect(&out[1]);
        assert_eq!(r.y, 0.0, "above the centre row first");
        assert!(!rect(&out[0]).overlaps(&r));
    }

    #[test]
    fn no_leader_crosses_another_labels_box() {
        let ats: Vec<(i32, i32)> = (0..4)
            .flat_map(|y| (0..4).map(move |x| (x * 2, y * 2)))
            .collect();
        let labels: Vec<_> = ats.iter().map(|&a| machine(a)).collect();
        let out = run(&labels, &ats);
        let mut saw_leader = false;
        for (i, a) in out.iter().enumerate() {
            let Placement::At {
                leader: Some(l), ..
            } = a
            else {
                continue;
            };
            saw_leader = true;
            for (j, b) in out.iter().enumerate() {
                if let Placement::At { rect, .. } = b
                    && i != j
                {
                    assert!(
                        !segment_crosses_rect(*l, rect),
                        "leader {i} crosses box {j}"
                    );
                }
            }
        }
        assert!(saw_leader, "the crowd should force at least one leader");
    }

    #[test]
    fn the_same_input_gives_the_same_output() {
        let ats = [(3, 3), (4, 3), (3, 4), (8, 8)];
        let labels: Vec<_> = ats.iter().map(|&a| machine(a)).collect();
        assert_eq!(run(&labels, &ats), run(&labels, &ats));
    }

    #[test]
    fn a_label_with_no_free_spot_within_the_radius_is_dropped() {
        // A pane exactly the size of the machine's tile leaves nowhere to go.
        let soft = HashSet::new();
        let labels = [machine((0, 0))];
        let out = layout(&LayoutInput {
            labels: &labels,
            soft: &soft,
            bounds: TileRect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            },
        });
        assert_eq!(out[0], Placement::Dropped);
    }

    #[test]
    fn rooms_are_unchanged_when_no_machine_is_near() {
        // Today's placement: centred on the label cell, on that cell's row.
        let cells: Vec<(i32, i32)> = (0..5).map(|x| (x, 0)).collect();
        let r = rect(&run(&[room(&cells, (2, 0), 4.0)], &[])[0]);
        assert_eq!((r.x + r.w / 2.0, r.y + r.h / 2.0), (2.5, 0.5));
    }

    #[test]
    fn a_soft_only_free_neighbourhood_still_places_the_label() {
        let soft: Vec<(i32, i32)> = (-8..=8)
            .flat_map(|y| (-8..=8).map(move |x| (x, y)))
            .collect();
        let out = run(&[machine((0, 0))], &soft);
        assert_eq!(rect(&out[0]).x, 1.0);
    }
}
