use super::*;

/// One outpost as the surface map marks it — see `Game::outpost_marks`.
///
/// `DigMark`'s shape one subsystem over: a record with no entity still needs
/// a map pass of its own, since it cannot ride `Game::view_entities_at`.
/// **Zone-surface coordinates**, `DigMark::pos`'s own base-space carrying
/// its opposite: an outpost cannot stand anywhere else (design spec §9).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutpostMark {
    pub tile: (i32, i32),
    pub glyph: char,
    /// 0-indexed — `outposts::tier_label`'s own indexing.
    pub tier: usize,
    /// The growth bar's fill, 0.0..=1.0 within the current tier —
    /// `OutpostReport::growth_fill`'s own derivation, read once here rather
    /// than restated.
    pub fill: f32,
    pub trend: crate::outposts::Trend,
    /// Integrity at zero (Phase 5's raid outcome). The map dims the glyph
    /// and draws no bar rather than reading `trend` for it, since a dark
    /// outpost runs no growth and no production for `trend` to describe.
    pub dark: bool,
}

/// One program posted at an outpost, as `Mode::OutpostVisit`'s CREW section
/// lists it — see `OutpostReport::crew`.
///
/// Three plain fields rather than a call back into the roster: this is a
/// view, and `Game::species_name`/`components::Experience::level` are
/// resolved once when the report is built rather than re-asked per row draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutpostCrewRow {
    pub name: String,
    pub species: String,
    pub level: u32,
}

/// One item the outpost's current tier can produce, as the screen's YIELDS
/// section lists it — see `OutpostReport::yields`.
///
/// `tier` is the item's own **1-indexed** tier for display — the lowest
/// tier it is offered at (`outposts::yields_with_tier`), not the outpost's
/// own current tier: a tier-2 outpost still yields its tier-1 items, and
/// the row says which rung each one actually belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutpostYieldRow {
    pub item: ItemId,
    pub name: String,
    pub tier: usize,
}

/// `Mode::OutpostVisit`'s one derivation — see `Game::outpost_report`.
///
/// **Every figure here is resolved once**, `BuildOrderRow`'s own reason: the
/// map's mark, the examine line and this screen must not be free to
/// disagree about the same outpost's growth, trend or stock.
#[derive(Clone, Debug, PartialEq)]
pub struct OutpostReport {
    pub tile: (i32, i32),
    pub name: String,
    pub biome: Biome,
    /// 0-indexed — `outposts::tier_label`'s own indexing.
    pub tier: usize,
    /// `"Raw"` / `"Processed"` / `"Complex"` — `outposts::tier_label(tier)`.
    pub tier_label: &'static str,
    /// 0.0..=1.0 — the growth bar's fill within the current tier band, and
    /// 1.0 at the top tier (there is no next band to fill toward).
    pub growth_fill: f32,
    pub trend: Trend,
    /// `Trend::reason`, so the screen's status line and the alert board
    /// (Phase 5) cannot word the same trend differently.
    pub reason: String,
    pub integrity: u32,
    pub max_integrity: u32,
    pub stock: u32,
    pub stock_cap: u32,
    /// `Game::outpost_route_line` — `None` when no caravan route runs to
    /// this tile. A pre-resolved sentence rather than a route handle,
    /// `Trend::reason`'s own reason: the screen builds no prose of its own.
    pub route: Option<String>,
    /// Sorted by `ProgramId` — `Game::outpost_crew`'s own order, so the
    /// screen's row letters land on the same program every read.
    pub crew: Vec<OutpostCrewRow>,
    pub yields: Vec<OutpostYieldRow>,
    /// What the next tier asks for, or `None` at the top tier. An absolute
    /// crew figure (`tuning::OUTPOST_TIER_CREW`), not a shortfall: it names
    /// the requirement whether or not the current crew already clears it,
    /// since growth alone can still be the thing standing in the way.
    pub next_tier_requirement: Option<String>,
    pub dark: bool,
}
