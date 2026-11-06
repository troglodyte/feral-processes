//! Views of the Sites hub. Every sentence and figure the Sites pages show is
//! built here or in `Game`'s Sites methods; gui only lays them out.

use super::SeenOffer;

/// What a town's Goods tab shows.
#[derive(Clone, Debug, PartialEq)]
pub enum TownGoods {
    /// A known town the party has never stood at, with no live feed.
    NotSeen,
    /// The shelf as it stood on the last visit, prices as shown then.
    Seen {
        offers: Vec<SeenOffer>,
        /// `time_band(..).ago()` of the visit.
        age: &'static str,
    },
    /// The shelf now, read-only: the Price Feeds research is known.
    Live { offers: Vec<SeenOffer> },
}
