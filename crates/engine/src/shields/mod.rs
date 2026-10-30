//! Which tiles a Shield covers: a circle around its anchor that grows with
//! its tier. Every consumer (the sweep, the siege board, the outline, the
//! views) asks here, so the rule lives in one place.

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// A Shield's reach: `base` tiles at tier 1, `per_tier` more per tier after.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShieldRadiusDef {
    pub base: u32,
    pub per_tier: u32,
}

/// The radius, in tiles, of a Shield at `tier` (tiers start at 1).
pub fn radius_at(def: &ShieldRadiusDef, tier: u32) -> u32 {
    def.base + def.per_tier * tier.saturating_sub(1)
}

/// Whether `tile` is covered: the anchor tile's centre is closer than
/// `radius` to the nearest point of the tile's unit square. Works in doubled
/// integers so no float comparison decides an edge tile.
pub fn covers(anchor: (i32, i32), radius: u32, tile: (i32, i32)) -> bool {
    let reach = |a: i32, t: i32| i64::from(((a - t).abs() * 2 - 1).max(0));
    let (dx, dy) = (reach(anchor.0, tile.0), reach(anchor.1, tile.1));
    let r2 = i64::from(radius) * 2;
    dx * dx + dy * dy < r2 * r2
}

/// Whether any of `cells` (a structure's footprint) is covered.
pub fn covers_any(anchor: (i32, i32), radius: u32, cells: &[(i32, i32)]) -> bool {
    cells.iter().any(|&c| covers(anchor, radius, c))
}

/// Every covered tile, sorted by `(x, y)`.
pub fn covered_tiles(anchor: (i32, i32), radius: u32) -> Vec<(i32, i32)> {
    let r = radius as i32;
    let mut tiles: Vec<(i32, i32)> = (anchor.0 - r..=anchor.0 + r)
        .flat_map(|x| (anchor.1 - r..=anchor.1 + r).map(move |y| (x, y)))
        .filter(|&t| covers(anchor, radius, t))
        .collect();
    tiles.sort_unstable();
    tiles
}
