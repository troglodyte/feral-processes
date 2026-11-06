//! Where a bounty's anchor may be: a fixed lattice of sample points, so the
//! candidate list is a pure function of the home tile.

use crate::game::stack::ring_offset;
use crate::tuning::*;

/// Sample points on concentric rings around `home`, across
/// `BOUNTY_ANCHOR_BAND`. Sorted and deduplicated, so a roll over them never
/// depends on generation order.
pub fn anchor_samples(home: (i32, i32)) -> Vec<(i32, i32)> {
    let (near, far) = BOUNTY_ANCHOR_BAND;
    let mut points = Vec::new();
    let mut ring = near;
    while ring <= far {
        let perimeter = 8 * ring;
        for i in 0..BOUNTY_ANCHOR_RING_SAMPLES {
            let (dx, dy) = ring_offset(ring, i * perimeter / BOUNTY_ANCHOR_RING_SAMPLES);
            points.push((home.0 + dx, home.1 + dy));
        }
        ring += BOUNTY_ANCHOR_RING_STEP;
    }
    points.sort();
    points.dedup();
    points
}
