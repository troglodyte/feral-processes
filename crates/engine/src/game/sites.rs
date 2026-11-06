//! The shell around the Sites stores (`sites/`): every writer goes through
//! `note_site`, so a log line is one call at the door that did the event.

use crate::RouteDestinationId;
use crate::sites::{SiteLogEntry, SiteLogKind};
use crate::tuning::SITE_LOG_CAP;
use crate::*;

impl Game {
    /// Logs `text` on `site`'s ring at the current tick. Draws no RNG.
    pub(crate) fn note_site(&mut self, site: RouteDestinationId, kind: SiteLogKind, text: String) {
        let tick = self.current_tick();
        self.sites
            .log
            .push(site, SiteLogEntry { kind, tick, text }, SITE_LOG_CAP);
    }
}
