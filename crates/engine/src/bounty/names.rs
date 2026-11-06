//! The word banks a quarry's name is drawn from: `<Name> the <Epithet>`.
//!
//! Plain, machine-flavoured words only. No occult vocabulary and no slogans
//! (standing content rule).

use rand::prelude::*;

const NAMES: [&str; 24] = [
    "Vesk", "Orrin", "Kade", "Brannoch", "Tolven", "Ruck", "Hesper", "Dovan", "Marrek", "Sable",
    "Quill", "Garrow", "Pell", "Ixen", "Torvald", "Neris", "Calder", "Wick", "Jarrow", "Ostrum",
    "Fenn", "Lodge", "Varga", "Thane",
];

const EPITHETS: [&str; 24] = [
    "Long Wire",
    "Cold Socket",
    "Broken Latch",
    "Rust Bearing",
    "Grey Relay",
    "Loose Bolt",
    "Slow Fuse",
    "Iron Clamp",
    "Burnt Circuit",
    "Heavy Gate",
    "Split Rail",
    "Dead Channel",
    "Hard Drive",
    "Flat Line",
    "Open Port",
    "Stripped Gear",
    "Hollow Pipe",
    "Black Cable",
    "Worn Spindle",
    "Static Hand",
    "Tin Crown",
    "Short Circuit",
    "Rough Cut",
    "Steel Knuckle",
];

/// A quarry's name, e.g. "Vesk the Long Wire".
pub fn quarry_name(rng: &mut impl Rng) -> String {
    let name = NAMES[rng.random_range(0..NAMES.len())];
    let epithet = EPITHETS[rng.random_range(0..EPITHETS.len())];
    format!("{name} the {epithet}")
}
