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
    ///
    /// `#[allow(dead_code)]` until Task 4 (`bench_or_dissolve`) becomes its
    /// first production caller.
    #[allow(dead_code)]
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
}
