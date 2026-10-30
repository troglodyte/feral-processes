//! The one door a notification is queued through.

use crate::Game;
use crate::notifications::{Notification, NotificationKind};
use crate::resources::Notifications;

impl Game {
    /// Queues `kind`, to take the screen the next time the player is
    /// standing on the map. `true` if it queued.
    ///
    /// A thin call onto `queue_notification` with neither extra — most
    /// firing sites have nothing dynamic to say.
    pub fn notify(&mut self, kind: NotificationKind) -> bool {
        self.queue_notification(kind, &[], None)
    }

    /// `notify`, with `{hole}` placeholders in the title and body replaced and
    /// an optional figure the authored copy could not have carried.
    ///
    /// The holes exist so one arm can be written once and read for many
    /// subjects: the onboarding chain's briefing is one def filled from
    /// whichever mission was just handed out, rather than eleven arms each
    /// repeating a mission's own name and description. A hole no caller names
    /// is left standing rather than blanked, so it is visible to a census
    /// instead of reading as a missing word.
    ///
    /// **Both extras on one wrapper, not one each.** They were two wrappers
    /// while nothing wanted both; a completion screen that words the contract
    /// *and* quotes its payout wants both, and the choice there is a fourth
    /// wrapper for the fourth combination or one that carries what the door
    /// carries. This is the second.
    pub fn notify_filled(
        &mut self,
        kind: NotificationKind,
        fills: &[(&str, &str)],
        detail: Option<String>,
    ) -> bool {
        self.queue_notification(kind, fills, detail)
    }

    /// The **one door** for milestones. It reads the def and pushes a
    /// *resolved* value; it draws no RNG and writes no log line. Every call
    /// queues: once-per-run guidance goes through `Game::ftue`, whose run set
    /// decides, so a second `if first_time` check at a site would put the
    /// policy in two places.
    ///
    /// The two extras are **parameters on that one door**, not doors of
    /// their own — `pursuit_field`'s shape over `walk_field`. They are
    /// orthogonal: a figure the copy could not know (`detail`), and holes in
    /// the copy the caller fills (`fills`). A third wrapper is a signal to
    /// re-read the call site, not to add one — `notify` for a site with
    /// nothing to add, `notify_filled` for one that has.
    ///
    /// Always `true`: every `NotificationKind` has copy by construction.
    fn queue_notification(
        &mut self,
        kind: NotificationKind,
        fills: &[(&str, &str)],
        detail: Option<String>,
    ) -> bool {
        let notification = Notification::from(kind.def()).filled(fills, detail);
        self.world
            .resource_mut::<Notifications>()
            .push(notification);
        true
    }

    /// Pops the oldest queued notification, if any. `take_effects`'
    /// counterpart: a frontend that draws none must still call it, or the
    /// queue grows for the life of the run.
    pub fn take_notification(&mut self) -> Option<Notification> {
        self.world.resource_mut::<Notifications>().pop()
    }

    /// How many are waiting. For a frontend deciding whether the screen it
    /// just dismissed has a successor.
    pub fn notifications_pending(&self) -> usize {
        self.world.resource::<Notifications>().len()
    }
}
