// ---------------------------------------------------------------------------
// Bounty contracts
// ---------------------------------------------------------------------------

/// The lowest sector whose Broker rolls a bounty. Zone 1 pins groups to size 1
/// and the opening ring blocks bosses, so a boss-and-escort set piece is out
/// of place there.
pub const BOUNTY_MIN_ZONE: u32 = 2;

/// How far from the party, in tiles, the quarry's anchor may be placed
/// (inclusive both ends).
pub const BOUNTY_ANCHOR_BAND: (i32, i32) = (25, 60);

/// How far, in tiles, the quarry wanders from its anchor.
pub const BOUNTY_TETHER: i32 = 6;

/// Stat multiplier on the boss path at the lowest bounty sector, as a percent.
pub const BOUNTY_GRADE_BASE_PCT: u32 = 150;
/// Percent added to the grade for every sector above `BOUNTY_MIN_ZONE`.
pub const BOUNTY_GRADE_PER_ZONE_PCT: u32 = 25;

/// Escort members at the lowest bounty sector.
pub const BOUNTY_ESCORT_BASE: u32 = 3;
/// Escort members added for every sector above `BOUNTY_MIN_ZONE`.
pub const BOUNTY_ESCORT_PER_ZONE: u32 = 1;
/// The most distinct species an escort is drawn from.
pub const BOUNTY_ESCORT_SPECIES: usize = 2;

/// Credits at the lowest bounty sector. A Hunt pays at most 84 at any sector,
/// so this is the "clearly beats a Hunt" figure.
pub const BOUNTY_CREDITS_BASE: u32 = 250;
/// Credits added for every sector above `BOUNTY_MIN_ZONE`.
pub const BOUNTY_CREDITS_PER_ZONE: u32 = 75;
/// XP at the lowest bounty sector. A Hunt pays at most 240.
pub const BOUNTY_XP_BASE: u32 = 600;
/// XP added for every sector above `BOUNTY_MIN_ZONE`.
pub const BOUNTY_XP_PER_ZONE: u32 = 150;

/// The first sector whose quarry is `Gold` rather than `Silver`.
pub const BOUNTY_GOLD_ZONE: u32 = 4;
/// The first sector whose quarry is `Platinum`.
pub const BOUNTY_PLATINUM_ZONE: u32 = 6;

/// Chance, per board refresh, that the Broker adds a bounty once the sector
/// allows one.
pub const BOUNTY_BOARD_ODDS: f64 = 0.35;

/// Distance, in tiles, between the concentric rings a bounty's anchor is
/// sampled from across `BOUNTY_ANCHOR_BAND`.
pub const BOUNTY_ANCHOR_RING_STEP: i32 = 5;
/// Points sampled around each ring. A fixed lattice rather than a scan of the
/// whole band: the board is read every frame and has to be stable, so the
/// candidates cannot depend on where the party has walked.
pub const BOUNTY_ANCHOR_RING_SAMPLES: i32 = 16;
/// Tiles of clearance kept from a derived settlement site, on top of the
/// largest footprint, to cover the walk-out `standable_near` does.
pub const BOUNTY_SETTLEMENT_MARGIN: i32 = 12;
