//! Views of the Sites hub. Every sentence and figure the Sites pages show is
//! built here or in `Game`'s Sites methods; gui only lays them out.

use super::SeenOffer;
use crate::RouteDestinationId;

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

/// Which header a rail entry sits under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteSection {
    Settlements,
    Outposts,
}

/// One row of the Sites rail. The Caravans entry is app-core's, not a site.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteRailEntry {
    pub site: RouteDestinationId,
    pub name: String,
    pub glyph: char,
    pub section: SiteSection,
    /// The rail's `!`: needs the player's attention.
    pub attention: bool,
}

/// Where the party stands relative to the sites, which decides whether the
/// hub opens and which at-site doors are live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SiteReach {
    pub on_base: bool,
    /// The site the party is beside, if any.
    pub beside: Option<RouteDestinationId>,
}

impl SiteReach {
    pub fn opens(self) -> bool {
        self.on_base || self.beside.is_some()
    }
}

/// A town's Status tab.
#[derive(Clone, Debug, PartialEq)]
pub struct TownStatus {
    pub name: String,
    pub kind: &'static str,
    pub specialty: &'static str,
    pub temperament: &'static str,
    pub standing: &'static str,
    pub hostile: bool,
    /// What the standing means: service, standing routes, raids, patrols,
    /// predation, garrison.
    pub meaning: Vec<String>,
    /// A city's vitality, or a town's growth outlook.
    pub condition: Option<String>,
    pub gift: String,
    pub travel: String,
}

/// One route in flight, worded with no tick figure.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteLine {
    pub site: RouteDestinationId,
    pub site_name: String,
    /// "Outbound" or "Inbound".
    pub leg: &'static str,
    /// What it carries: the manifest, or the proceeds on an inbound leg.
    pub cargo: String,
    /// `time_band(..).until()`, or why it is held up.
    pub due: &'static str,
    pub standing: bool,
    pub stalled: bool,
}

/// The visiting trader, banded.
#[derive(Clone, Debug, PartialEq)]
pub struct CaravanSchedule {
    /// Docked now: the trader's name and `until()` of its departure.
    pub docked: Option<(String, &'static str)>,
    /// The next one to arrive: the trader's name and `until()` of its arrival.
    pub next: Option<(String, &'static str)>,
}

/// One line of a site's log.
#[derive(Clone, Debug, PartialEq)]
pub struct SiteLogLine {
    pub age: &'static str,
    pub text: String,
}
