//! The one door a notification is queued through.

use crate::Game;
use crate::achievements::Profile;
use crate::notifications::{Notification, NotificationKind, Repeat};
use crate::resources::{Notifications, PendingProfileWrites};

impl Game {
    /// Queues `kind`, to take the screen the next time the player is
    /// standing on the map. `true` if it queued.
    ///
    /// A thin call onto `queue_notification` with neither extra — most
    /// firing sites have nothing dynamic to say.
    pub fn notify(&mut self, kind: NotificationKind) -> bool {
        self.queue_notification(kind, &[], None)
    }

    /// `notify`, with a figure the authored copy could not have carried —
    /// `Game::complete_contract`'s payout is the one caller today.
    pub fn notify_with_detail(&mut self, kind: NotificationKind, detail: Option<String>) -> bool {
        self.queue_notification(kind, &[], detail)
    }

    /// `notify`, with `{hole}` placeholders in the title and body replaced.
    ///
    /// It exists so one arm can be written once and read for many subjects:
    /// the onboarding chain's briefing is one def filled from whichever
    /// mission was just handed out, rather than eleven arms each repeating a
    /// mission's own name and description. A hole no caller names is left
    /// standing rather than blanked, so it is visible to a census instead of
    /// reading as a missing word.
    pub fn notify_filled(&mut self, kind: NotificationKind, fills: &[(&str, &str)]) -> bool {
        self.queue_notification(kind, fills, None)
    }

    /// The **one door**. It reads the def, checks the latch and pushes a
    /// *resolved* value; it draws no RNG and writes no log line. A caller
    /// fires it unconditionally at its site — the once-only rule is
    /// `Repeat::OnceEver` and lives here, so a second `if first_time` check
    /// in Rust would put the policy in two places and let the two disagree.
    ///
    /// The two extras are **parameters on that one door**, not doors of
    /// their own — `pursuit_field`'s shape over `walk_field`. They are
    /// orthogonal and the three public names above are the combinations
    /// anything actually asks for: a figure the copy could not know
    /// (`detail`), and holes in the copy the caller fills (`fills`). A
    /// fourth wrapper is a signal to re-read the call site, not to add one.
    ///
    /// **The only way this returns `false` is a spent `OnceEver` latch.**
    /// It used to have a second refusal — "no file defines this id" — which
    /// went with the `.ron` catalogue: every `NotificationKind` now has copy
    /// by construction, so an unknown notification is not a state that
    /// exists.
    fn queue_notification(
        &mut self,
        kind: NotificationKind,
        fills: &[(&str, &str)],
        detail: Option<String>,
    ) -> bool {
        let def = kind.def();
        let fill = |text: &str| {
            fills.iter().fold(text.to_string(), |text, (key, value)| {
                text.replace(&format!("{{{key}}}"), value)
            })
        };
        let mut notification = Notification::from(def);
        notification.title = fill(&notification.title);
        notification.body = fill(&notification.body);
        notification.detail = detail;
        if def.repeat == Repeat::OnceEver {
            if !self.world.resource_mut::<Profile>().see(kind) {
                return false;
            }
            // The latch is only worth anything once it reaches disk, and
            // app-core owns the path. This is the same channel an earned
            // achievement uses, which is what puts the write *below*
            // `after_tick`'s `in_arena()` early return — so "an arena
            // session touches no disk" holds here by omission rather than
            // by a check. Do not add one.
            self.world
                .resource_mut::<PendingProfileWrites>()
                .seen
                .push(kind);
        }
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
