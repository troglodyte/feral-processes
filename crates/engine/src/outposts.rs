//! A one-tile fixture on the zone surface where posted programs extract
//! materials — see `docs/superpowers/specs/2026-09-23-outposts-design.md`.
//!
//! `OutpostDef` and `OutpostDb::load_dir` follow `NeedDb`'s absent-is-silent
//! pattern: no `assets/outposts/` directory means no outposts exist and the
//! kit refuses with a message, never a panic. `Outpost` is the per-tile
//! record a founded outpost carries; `resources::Outposts` is the keyed
//! collection a `Game` holds of them. Crew is deliberately not part of
//! either — see `components::PostedAt` and `save::CreatureSave::outpost`.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::Deserialize;

use crate::items::ItemId;
use crate::tuning::{
    OUTPOST_DAMAGED_FRACTION, OUTPOST_MAX_INTEGRITY, OUTPOST_STALE_GRACE_TICKS, OUTPOST_TIER_CREW,
    OUTPOST_TIER_GROWTH,
};
use crate::world::Biome;

/// One growth tier's yield table, keyed by the biome under the outpost.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct OutpostTierDef {
    /// The items this tier can produce in a given biome. A biome missing a
    /// row here falls back to this tier's first listed biome (`BTreeMap`
    /// order) — see `yields`.
    #[serde(default)]
    pub yields: BTreeMap<Biome, Vec<ItemId>>,
}

/// The one outpost def a v1 install ships, loaded by `OutpostDb`.
///
/// Every field but `tiers` is `#[serde(default)]` — a future second file
/// (a mod's own outpost variant, once `features:` grows past reserved) should
/// not need every field re-authored just to parse. `tiers` stays required:
/// a def with none would yield nothing at any tier, which is a content
/// mistake worth a load warning rather than a silently empty outpost.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct OutpostDef {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub glyph: char,
    /// The item that founds an outpost when used on the zone surface — see
    /// `Game::found_outpost` and `Game::use_item`.
    #[serde(default)]
    pub kit: ItemId,
    /// Raw, then processed, then complex — index 0 is tier 1 (the tier a
    /// freshly founded outpost starts at). See `yields`.
    pub tiers: Vec<OutpostTierDef>,
    /// Reserved for the forward-camp/frontier-post extension points (design
    /// spec §10): `Rest`, `Stash`, `Reach(radius)`. Always empty in v1 —
    /// `()` is a placeholder element type, swapped out the day a feature
    /// actually lands here rather than designed for now.
    #[serde(default)]
    pub features: Vec<()>,
}

/// Every outpost def the game knows about, loaded from `assets/outposts/`.
///
/// An empty database is a supported state, `NeedDb`'s rule: deleting the
/// directory restores the pre-outpost game rather than breaking an install.
#[derive(Resource, Default)]
pub struct OutpostDb {
    def: Option<OutpostDef>,
}

impl OutpostDb {
    /// The v1 def, if this install ships one. `None` for a deleted
    /// `assets/outposts/` directory or one with no valid file in it.
    pub fn def(&self) -> Option<&OutpostDef> {
        self.def.as_ref()
    }

    /// Loads every `*.ron` file in `dir`, `NeedDb::load_dir`'s pattern: an
    /// absent directory is silent and a malformed file is skipped with a
    /// warning rather than aborting the whole load.
    ///
    /// **The first by filename wins.** v1 ships exactly one file; a second
    /// is a mod's problem to resolve, not a merge this loader attempts.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Self::default(), warnings));
            }
            Err(e) => return Err(e),
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        // Sorted, `NeedDb::load_dir`'s reason: which file wins must not
        // depend on the OS's own directory-listing order.
        paths.sort();
        let mut def = None;
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<OutpostDef>(&text) {
                Ok(parsed) => {
                    if def.is_none() {
                        def = Some(parsed);
                    }
                }
                Err(e) => warnings.push(format!("skipped invalid outpost file {path:?}: {e}")),
            }
        }
        Ok((Self { def }, warnings))
    }
}

/// A founded outpost's stored state — design spec §2.
///
/// Crew is deliberately absent: membership rides `components::PostedAt` and
/// `save::CreatureSave::outpost`, `resources::Sorties`' `sortie_index`
/// precedent one level over. Stock capacity, crew cap and max integrity come
/// from `tuning.rs`, so none of them is stored either — everything past
/// these six fields is derived on every read (tier, trend, current yields).
#[derive(Clone, Debug, PartialEq)]
pub struct Outpost {
    /// Read once at `Game::found_outpost` and never re-derived: the yield
    /// table is keyed by the ground the outpost was built on, not by
    /// whatever the tile classifies as on a later read.
    pub biome: Biome,
    /// The one stored progress number growth tracks.
    pub growth: u32,
    pub integrity: u32,
    pub stock: BTreeMap<ItemId, u32>,
    /// Ticks spent with `stock` at its cap — `trend`'s Stale/Declining
    /// split (Phase 2).
    pub stale_ticks: u32,
    /// Progress toward the next production cycle (Phase 2).
    pub cycle_progress: u32,
    /// Which `Trend` last posted an alert, so a reload does not repeat one
    /// — design correction 10 (Phase 5). Not part of `save::OutpostSave`:
    /// it is inert until Phase 5 re-seeds it from the freshly-derived trend
    /// right after load, which is what keeps a reload from posting again.
    pub announced: Option<Trend>,
}

impl Outpost {
    /// A freshly founded outpost: no growth, no stock, full integrity.
    pub fn new(biome: Biome, integrity: u32) -> Self {
        Self {
            biome,
            growth: 0,
            integrity,
            stock: BTreeMap::new(),
            stale_ticks: 0,
            cycle_progress: 0,
            announced: None,
        }
    }
}

/// How an outpost is doing right now — derived on every read and never
/// stored, design spec §4. `trend()` (Phase 2) is the one derivation; one
/// function answers the growth bar's colour, the screen's status line and
/// the alert board alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trend {
    Growing,
    Stable,
    Stale,
    Declining,
}

impl Trend {
    /// The screen's one-line status and the alert board's own message,
    /// design spec §4: one function so the bar's colour, the status line
    /// and the alert cannot disagree about why. The sub-causes under
    /// `Declining` are checked in the same order `trend` checks them, so a
    /// call site that reorders one without the other silently starts
    /// mis-explaining an outpost.
    pub fn reason(self, outpost: &Outpost, crew: usize, tier: usize) -> String {
        match self {
            Trend::Declining => {
                if crew < OUTPOST_TIER_CREW[0] {
                    format!("Declining: needs {} crew", OUTPOST_TIER_CREW[0])
                } else if (outpost.integrity as f32)
                    < OUTPOST_MAX_INTEGRITY as f32 * OUTPOST_DAMAGED_FRACTION
                {
                    "Declining: damaged".to_string()
                } else {
                    "Declining: stock sat full too long".to_string()
                }
            }
            Trend::Stale => "Stale: stock full".to_string(),
            Trend::Stable => {
                if tier + 1 < OUTPOST_TIER_CREW.len() {
                    let need = OUTPOST_TIER_CREW[tier + 1].saturating_sub(crew);
                    format!("Stable: post {need} more for tier {}", tier + 2)
                } else {
                    "Stable: max tier reached".to_string()
                }
            }
            Trend::Growing => format!("Growing (+ with {crew} crew)"),
        }
    }
}

/// The highest tier this outpost's stored `growth` **and** its current
/// `crew` both support — design spec §4's ladder, first match from the top
/// down.
///
/// **`growth` is never reset by a crew shortfall.** This can report a lower
/// tier than the record has actually grown to without touching the stored
/// number, so an outpost that loses crew and gets it back recovers its tier
/// instantly rather than re-growing from zero — `Outpost::growth`'s own doc.
pub fn tier(outpost: &Outpost, crew: usize) -> usize {
    (0..OUTPOST_TIER_GROWTH.len())
        .rev()
        .find(|&t| outpost.growth >= OUTPOST_TIER_GROWTH[t] && crew >= OUTPOST_TIER_CREW[t])
        .unwrap_or(0)
}

/// The tier `crew` alone would support, ignoring `growth` entirely —
/// `trend`'s reading of "the ceiling this crew can reach." `tier` above can
/// never exceed this for the same `crew`, since it additionally requires
/// `growth` to clear the same thresholds.
fn crew_ceiling(crew: usize) -> usize {
    (0..OUTPOST_TIER_CREW.len())
        .rev()
        .find(|&t| crew >= OUTPOST_TIER_CREW[t])
        .unwrap_or(0)
}

/// How an outpost is doing right now — derived on every read and never
/// stored, design spec §4. **The first match wins**: a call site that
/// checks these in a different order would judge an outpost meeting more
/// than one condition (damaged *and* overstocked, say) by the wrong one.
///
/// `tier` is the caller's own `outposts::tier(outpost, crew)` — not
/// recomputed here, so a caller cannot pass a stale one without every
/// figure on the screen already having disagreed with it first.
pub fn trend(outpost: &Outpost, crew: usize, tier: usize, stock_cap: u32) -> Trend {
    if crew < OUTPOST_TIER_CREW[0]
        || (outpost.integrity as f32) < OUTPOST_MAX_INTEGRITY as f32 * OUTPOST_DAMAGED_FRACTION
        || outpost.stale_ticks > OUTPOST_STALE_GRACE_TICKS
    {
        return Trend::Declining;
    }
    let stock_total: u32 = outpost.stock.values().sum();
    if stock_total >= stock_cap {
        return Trend::Stale;
    }
    if tier >= crew_ceiling(crew) {
        return Trend::Stable;
    }
    Trend::Growing
}

/// The growth bar's fill, 0.0..=1.0 within `tier`'s own band — the fraction
/// of the way from `OUTPOST_TIER_GROWTH[tier]` to the next tier's
/// threshold, and a full bar at the top tier (there is no next band to fill
/// toward).
///
/// **`tier` is the caller's own `outposts::tier(outpost, crew)`**, not
/// recomputed here — `trend`'s own convention, since growth can sit above
/// the band a crew-capped tier reports (growth is never reset by a crew
/// shortfall) and clamping absorbs exactly that without the caller having
/// to know why.
pub fn growth_fill(outpost: &Outpost, tier: usize) -> f32 {
    if tier + 1 >= OUTPOST_TIER_GROWTH.len() {
        return 1.0;
    }
    let lo = OUTPOST_TIER_GROWTH[tier];
    let hi = OUTPOST_TIER_GROWTH[tier + 1];
    ((outpost.growth.saturating_sub(lo)) as f32 / (hi - lo) as f32).clamp(0.0, 1.0)
}

/// The items a `tier`-th outpost (0-indexed: tier 0 is raw) can produce in
/// `biome`, each tagged with the **lowest** tier (0-indexed) it is first
/// offered at — the union of every tier up to and including `tier`, design
/// spec §4. A tier missing a row for `biome` falls back to that tier's first
/// listed biome (`BTreeMap` order), so a new `Biome` variant with no
/// authored row still yields something rather than nothing.
///
/// A `BTreeMap` accumulator rather than a sort-and-dedup pass: iterating
/// tiers low to high and taking `or_insert` is what makes "first tier seen"
/// exactly the tier an item was introduced at, for the outpost screen's own
/// per-yield tier label (`views::OutpostYieldRow`) — `yields` below reads
/// off the same map rather than restating the walk.
pub fn yields_with_tier(def: &OutpostDef, tier: usize, biome: Biome) -> Vec<(ItemId, usize)> {
    let mut first_seen: BTreeMap<ItemId, usize> = BTreeMap::new();
    for (t, tier_def) in def.tiers.iter().enumerate().take(tier + 1) {
        let row = tier_def
            .yields
            .get(&biome)
            .or_else(|| tier_def.yields.values().next());
        if let Some(row) = row {
            for item in row {
                first_seen.entry(item.clone()).or_insert(t);
            }
        }
    }
    first_seen.into_iter().collect()
}

/// The items a `tier`-th outpost can produce in `biome` — `yields_with_tier`
/// with the per-item tier dropped, `BTreeMap` order (sorted by `ItemId`,
/// deduped by construction).
pub fn yields(def: &OutpostDef, tier: usize, biome: Biome) -> Vec<ItemId> {
    yields_with_tier(def, tier, biome)
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

/// `"Raw"` / `"Processed"` / `"Complex"` — the screen's own name for a tier,
/// design spec §9's header. Not moddable: the three-tier ladder is the
/// tuning ladder's own shape (`OUTPOST_TIER_GROWTH`'s length), so the
/// labels are fixed prose rather than a fourth thing content would have to
/// author in step with it.
pub fn tier_label(tier: usize) -> &'static str {
    match tier {
        0 => "Raw",
        1 => "Processed",
        _ => "Complex",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::OUTPOST_STOCK_CAP;

    fn tier(rows: &[(Biome, &[&str])]) -> OutpostTierDef {
        OutpostTierDef {
            yields: rows
                .iter()
                .map(|(b, items)| (*b, items.iter().map(|s| ItemId(s.to_string())).collect()))
                .collect(),
        }
    }

    fn ids(items: &[&str]) -> Vec<ItemId> {
        items.iter().map(|s| ItemId(s.to_string())).collect()
    }

    #[test]
    fn missing_directory_is_an_empty_db() {
        let dir =
            std::env::temp_dir().join(format!("feral_outposts_missing_{}", std::process::id()));
        let (db, warnings) = OutpostDb::load_dir(&dir).unwrap();
        assert!(db.def().is_none());
        assert!(warnings.is_empty());
    }

    #[test]
    fn a_malformed_file_is_skipped_with_a_warning_and_leaves_the_db_empty() {
        let dir =
            std::env::temp_dir().join(format!("feral_outposts_malformed_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bad.ron"), "not valid ron at all }{").unwrap();
        let (db, warnings) = OutpostDb::load_dir(&dir).unwrap();
        assert!(db.def().is_none());
        assert_eq!(warnings.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_empty_directory_is_an_empty_db_with_no_warnings() {
        let dir = std::env::temp_dir().join(format!("feral_outposts_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (db, warnings) = OutpostDb::load_dir(&dir).unwrap();
        assert!(db.def().is_none());
        assert!(warnings.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn yields_unions_every_tier_up_to_and_including_the_one_asked_for() {
        let def = OutpostDef {
            tiers: vec![
                tier(&[(Biome::Deadlock, &["raw_trace"])]),
                tier(&[(Biome::Deadlock, &["static_mesh"])]),
                tier(&[(Biome::Deadlock, &["core_fragment"])]),
            ],
            ..Default::default()
        };
        assert_eq!(yields(&def, 0, Biome::Deadlock), ids(&["raw_trace"]));
        let mut two = ids(&["raw_trace", "static_mesh"]);
        two.sort();
        assert_eq!(yields(&def, 1, Biome::Deadlock), two);
        let mut three = ids(&["raw_trace", "static_mesh", "core_fragment"]);
        three.sort();
        assert_eq!(yields(&def, 2, Biome::Deadlock), three);
    }

    #[test]
    fn an_unlisted_biome_falls_back_to_the_tiers_first_listed_biome() {
        // `Biome` derives `Ord` off declaration order, so `DataVoid` sorts
        // before `Deadlock` and is the "first listed" row here.
        let def = OutpostDef {
            tiers: vec![tier(&[
                (Biome::DataVoid, &["raw_trace"]),
                (Biome::Deadlock, &["static_mesh"]),
            ])],
            ..Default::default()
        };
        // `NullSector` has no row of its own, so it falls back to
        // `DataVoid`'s — the lowest-sorted key present.
        assert_eq!(yields(&def, 0, Biome::NullSector), ids(&["raw_trace"]));
    }

    #[test]
    fn yields_with_tier_tags_each_item_with_the_lowest_tier_it_first_appears_at() {
        let def = OutpostDef {
            tiers: vec![
                tier(&[(Biome::Deadlock, &["raw_trace"])]),
                tier(&[(Biome::Deadlock, &["raw_trace", "static_mesh"])]),
            ],
            ..Default::default()
        };
        // `raw_trace` reappears in tier 1's row, but it was first offered at
        // tier 0 — reappearing must not bump its tag forward.
        assert_eq!(
            yields_with_tier(&def, 1, Biome::Deadlock),
            vec![
                (ItemId("raw_trace".to_string()), 0),
                (ItemId("static_mesh".to_string()), 1),
            ]
        );
    }

    #[test]
    fn tier_label_names_the_three_shipped_tiers() {
        assert_eq!(tier_label(0), "Raw");
        assert_eq!(tier_label(1), "Processed");
        assert_eq!(tier_label(2), "Complex");
    }

    #[test]
    fn yields_is_sorted_and_deduped() {
        let def = OutpostDef {
            tiers: vec![
                tier(&[(Biome::Deadlock, &["raw_trace"])]),
                tier(&[(Biome::Deadlock, &["raw_trace"])]),
            ],
            ..Default::default()
        };
        assert_eq!(yields(&def, 1, Biome::Deadlock), ids(&["raw_trace"]));
    }

    #[test]
    fn an_empty_db_means_def_is_none() {
        let db = OutpostDb::default();
        assert!(db.def().is_none());
    }

    fn record(growth: u32, integrity: u32, stale_ticks: u32) -> Outpost {
        Outpost {
            growth,
            stale_ticks,
            ..Outpost::new(Biome::Deadlock, integrity)
        }
    }

    fn full_stock() -> BTreeMap<ItemId, u32> {
        let mut stock = BTreeMap::new();
        stock.insert(ItemId("raw_trace".to_string()), OUTPOST_STOCK_CAP);
        stock
    }

    #[test]
    fn a_freshly_founded_outpost_is_tier_zero() {
        let o = record(0, OUTPOST_MAX_INTEGRITY, 0);
        assert_eq!(super::tier(&o, OUTPOST_TIER_CREW[0]), 0);
    }

    #[test]
    fn tier_is_capped_by_crew_even_when_growth_is_high() {
        let o = record(OUTPOST_TIER_GROWTH[2], OUTPOST_MAX_INTEGRITY, 0);
        // Growth alone would qualify for tier 2 (0-indexed), but only one
        // program is posted — the crew ladder's floor.
        assert_eq!(super::tier(&o, OUTPOST_TIER_CREW[0]), 0);
        // Deleted-fix check: without the crew half of the `&&` this reports
        // 2 here, which is exactly the bug — a solo crew running a
        // fully-grown outpost at its top tier.
        //
        // Posting up to the top tier's crew requirement recovers the tier
        // *without* touching `growth` — the number banked by the first call
        // is exactly what makes this immediate rather than a re-grow.
        assert_eq!(super::tier(&o, OUTPOST_TIER_CREW[2]), 2);
    }

    #[test]
    fn tier_is_capped_by_growth_even_when_crew_is_plentiful() {
        let o = record(0, OUTPOST_MAX_INTEGRITY, 0);
        assert_eq!(super::tier(&o, OUTPOST_TIER_CREW[2]), 0);
    }

    #[test]
    fn trend_declining_beats_stale_when_both_apply() {
        // Crew and integrity are both fine, but `stale_ticks` has run past
        // its grace period *and* the stock is sitting full — two conditions
        // that would each pick a different arm on their own. Declining is
        // checked first, so it must win.
        let mut o = record(0, OUTPOST_MAX_INTEGRITY, OUTPOST_STALE_GRACE_TICKS + 1);
        o.stock = full_stock();
        let crew = OUTPOST_TIER_CREW[0];
        let tier = super::tier(&o, crew);
        assert_eq!(trend(&o, crew, tier, OUTPOST_STOCK_CAP), Trend::Declining);
        // Deleted-fix check: swap the order of the two checks in `trend`
        // and this reports `Stale` instead.
    }

    #[test]
    fn trend_declines_below_the_crew_floor() {
        let o = record(0, OUTPOST_MAX_INTEGRITY, 0);
        let crew = OUTPOST_TIER_CREW[0] - 1;
        let tier = super::tier(&o, crew);
        assert_eq!(trend(&o, crew, tier, OUTPOST_STOCK_CAP), Trend::Declining);
    }

    #[test]
    fn trend_declines_while_damaged() {
        let damaged = (OUTPOST_MAX_INTEGRITY as f32 * OUTPOST_DAMAGED_FRACTION) as u32 - 1;
        let o = record(0, damaged, 0);
        let crew = OUTPOST_TIER_CREW[0];
        let tier = super::tier(&o, crew);
        assert_eq!(trend(&o, crew, tier, OUTPOST_STOCK_CAP), Trend::Declining);
    }

    #[test]
    fn trend_is_stale_when_stock_is_full_inside_the_grace_period() {
        let mut o = record(0, OUTPOST_MAX_INTEGRITY, OUTPOST_STALE_GRACE_TICKS);
        o.stock = full_stock();
        let crew = OUTPOST_TIER_CREW[0];
        let tier = super::tier(&o, crew);
        assert_eq!(trend(&o, crew, tier, OUTPOST_STOCK_CAP), Trend::Stale);
    }

    #[test]
    fn trend_is_stable_once_growth_catches_the_crew_ceiling() {
        let crew = OUTPOST_TIER_CREW[1];
        let o = record(OUTPOST_TIER_GROWTH[1], OUTPOST_MAX_INTEGRITY, 0);
        let tier = super::tier(&o, crew);
        assert_eq!(trend(&o, crew, tier, OUTPOST_STOCK_CAP), Trend::Stable);
    }

    #[test]
    fn trend_is_growing_when_crew_outpaces_growth() {
        let crew = OUTPOST_TIER_CREW[2];
        let o = record(0, OUTPOST_MAX_INTEGRITY, 0);
        let tier = super::tier(&o, crew);
        assert_eq!(trend(&o, crew, tier, OUTPOST_STOCK_CAP), Trend::Growing);
    }

    #[test]
    fn growth_fill_is_zero_at_the_bottom_of_a_tiers_band() {
        let o = record(OUTPOST_TIER_GROWTH[1], OUTPOST_MAX_INTEGRITY, 0);
        assert_eq!(growth_fill(&o, 1), 0.0);
    }

    #[test]
    fn growth_fill_is_full_partway_through_the_band() {
        let mid = (OUTPOST_TIER_GROWTH[1] + OUTPOST_TIER_GROWTH[2]) / 2;
        let o = record(mid, OUTPOST_MAX_INTEGRITY, 0);
        let fill = growth_fill(&o, 1);
        assert!(fill > 0.4 && fill < 0.6, "fill was {fill}");
    }

    #[test]
    fn growth_fill_is_always_full_at_the_top_tier() {
        let o = record(OUTPOST_TIER_GROWTH[2], OUTPOST_MAX_INTEGRITY, 0);
        assert_eq!(growth_fill(&o, 2), 1.0);
    }

    #[test]
    fn growth_fill_clamps_above_one_when_growth_outruns_a_crew_capped_tier() {
        // Growth banked all the way to tier 3 while the caller reports tier
        // 1 (a crew shortfall) — the fill must still read as "full", not
        // overflow past 1.0.
        let o = record(OUTPOST_TIER_GROWTH[2], OUTPOST_MAX_INTEGRITY, 0);
        assert_eq!(growth_fill(&o, 0), 1.0);
    }

    #[test]
    fn reason_names_the_crew_floor_when_declining_for_lack_of_crew() {
        let o = record(0, OUTPOST_MAX_INTEGRITY, 0);
        let msg = Trend::Declining.reason(&o, 0, 0);
        assert!(msg.contains(&OUTPOST_TIER_CREW[0].to_string()));
    }

    #[test]
    fn reason_names_the_next_tiers_shortfall_when_stable() {
        let crew = OUTPOST_TIER_CREW[1];
        let o = record(OUTPOST_TIER_GROWTH[1], OUTPOST_MAX_INTEGRITY, 0);
        let msg = Trend::Stable.reason(&o, crew, 1);
        let need = OUTPOST_TIER_CREW[2] - crew;
        assert!(msg.contains(&need.to_string()));
        assert!(msg.contains('3'));
    }
}
