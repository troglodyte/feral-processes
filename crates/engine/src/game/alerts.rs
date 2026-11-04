//! The four doors gui reaches the alert board through, plus the one door
//! every in-crate source posts through — see `alerts` for the board itself.

use crate::Game;
use crate::alerts::{self, AlertBoard, AlertKind};
use crate::views::AlertView;

impl Game {
    /// The board, newest first, shaped for gui — see `views::AlertView`.
    pub fn alerts(&self) -> Vec<AlertView> {
        self.world
            .resource::<AlertBoard>()
            .alerts
            .iter()
            .map(|alert| AlertView {
                kind: alert.kind.clone(),
                text: alert.text.clone(),
                count: alert.count,
                unread: alert.unread,
            })
            .collect()
    }

    /// How many rows are unread — what the status bar's badge counts.
    pub fn unread_alerts(&self) -> usize {
        self.world
            .resource::<AlertBoard>()
            .alerts
            .iter()
            .filter(|alert| alert.unread)
            .count()
    }

    /// Marks every alert read. The one thing opening the board does to it.
    pub fn mark_alerts_read(&mut self) {
        for alert in self.world.resource_mut::<AlertBoard>().alerts.iter_mut() {
            alert.unread = false;
        }
    }

    /// Dismisses the alert at `index`. Out of range is a no-op — see
    /// `alerts::dismiss`.
    pub fn dismiss_alert(&mut self, index: usize) {
        alerts::dismiss(&mut self.world.resource_mut::<AlertBoard>(), index);
    }

    /// A one-line wrapper over `alerts::post`, for the `Game`-method sources
    /// (bevy systems with no `Game` call `alerts::post` directly against a
    /// `ResMut<AlertBoard>` instead).
    pub(crate) fn post_alert(
        &mut self,
        kind: AlertKind,
        subject: impl Into<String>,
        text: impl Into<String>,
    ) {
        alerts::post(
            &mut self.world.resource_mut::<AlertBoard>(),
            kind,
            subject,
            text,
        );
    }

    /// The one door for an amenity visit: the ledger tally and the alert are
    /// written together so they cannot drift.
    ///
    /// Posts on a blocked visit when the need is not latched; the latch clears
    /// when a visit ends and the access window holds nothing blocked for the
    /// need, so a fix is announced as over and a later block posts again.
    /// `site` is where it waited or what it was routed to.
    pub(crate) fn note_amenity_visit(
        &mut self,
        need: &crate::needs::NeedId,
        outcome: crate::base_ledger::VisitOutcome,
        site: Option<crate::components::Position>,
    ) {
        use crate::base_ledger::VisitOutcome;
        self.note_visit(need, outcome);
        let now = self.current_tick();
        let blocked_since = self
            .world
            .resource::<crate::base_ledger::BaseLedger>()
            .recent_visits(need, now)
            .blocked();
        if blocked_since == 0 {
            self.world
                .resource_mut::<AlertBoard>()
                .amenity_blocked
                .remove(need);
            return;
        }
        if outcome == VisitOutcome::Served
            || !self
                .world
                .resource_mut::<AlertBoard>()
                .amenity_blocked
                .insert(need.clone())
        {
            return;
        }
        let name = site
            .and_then(|at| self.structure_name_at(at))
            .unwrap_or_else(|| "amenity".to_string());
        let text = if outcome == VisitOutcome::Waited {
            format!("Staff waited for a place at the {name}")
        } else {
            format!("Staff can't reach the {name}")
        };
        self.post_alert(AlertKind::AmenityBlocked, need.as_str(), text);
    }

    fn structure_name_at(&self, at: crate::components::Position) -> Option<String> {
        let db = self.world.resource::<crate::structures::StructureDb>();
        self.world
            .iter_entities()
            .find_map(|e| {
                let kind = &e.get::<crate::components::Structure>()?.kind;
                (*e.get::<crate::components::Position>()? == at).then_some(kind)
            })
            .and_then(|kind| db.get(kind))
            .map(|def| def.name.clone())
    }
}
