//! The one door a notification is queued through.

use crate::Game;
use crate::achievements::Profile;
use crate::notifications::{Notification, NotificationDb, NotificationId, Repeat};
use crate::resources::{Notifications, PendingProfileWrites};

/// Why a notification did not queue. **Returned, never logged**: the engine
/// has no runtime warning channel, and the site that fires a notification
/// already writes its own log line — a second one would double every
/// announcement in the history screen.
#[derive(Debug, PartialEq, Eq)]
pub enum NoNotify {
    /// No file defines this id. The ordinary state of a modded install with
    /// `assets/notifications/` deleted, and not an error anywhere.
    Unknown,
    /// `Repeat::OnceEver`, and this machine has already shown it.
    AlreadySeen,
}

impl Game {
    /// Queues the notification `id` names, to take the screen the next time
    /// the player is standing on the map.
    ///
    /// A thin call onto `queue_notification` with neither extra — most
    /// firing sites have nothing dynamic to say.
    pub fn notify(&mut self, id: &NotificationId) -> Result<(), NoNotify> {
        self.queue_notification(id, &[], None)
    }

    /// `notify`, with a figure the `.ron` file could not have authored —
    /// `Game::complete_contract`'s payout is the one caller today.
    pub fn notify_with_detail(
        &mut self,
        id: &NotificationId,
        detail: Option<String>,
    ) -> Result<(), NoNotify> {
        self.queue_notification(id, &[], detail)
    }

    /// `notify`, with `{hole}` placeholders in the title and body replaced.
    ///
    /// It exists so a def can be written once and read for many subjects:
    /// the onboarding chain's briefing is one file filled from whichever
    /// mission was just handed out, rather than eleven files each repeating
    /// a mission's own name and description. A hole no caller names is left
    /// standing rather than blanked, so it is visible to a census instead of
    /// reading as a missing word.
    pub fn notify_filled(
        &mut self,
        id: &NotificationId,
        fills: &[(&str, &str)],
    ) -> Result<(), NoNotify> {
        self.queue_notification(id, fills, None)
    }

    /// The **one door**. It resolves the def, checks the latch and pushes a
    /// *resolved* value; it draws no RNG and writes no log line. A caller
    /// fires it unconditionally at its site — the once-only rule is
    /// `Repeat::OnceEver` and lives here, so a second `if first_time` check
    /// in Rust would put the policy in two places and let the two disagree.
    ///
    /// The two extras are **parameters on that one door**, not doors of
    /// their own — `pursuit_field`'s shape over `walk_field`. They are
    /// orthogonal and the three public names above are the combinations
    /// anything actually asks for: a figure the file could not know
    /// (`detail`), and holes in the file the caller fills (`fills`). A
    /// fourth wrapper is a signal to re-read the call site, not to add one.
    fn queue_notification(
        &mut self,
        id: &NotificationId,
        fills: &[(&str, &str)],
        detail: Option<String>,
    ) -> Result<(), NoNotify> {
        let Some(def) = self.world.resource::<NotificationDb>().get(id) else {
            return Err(NoNotify::Unknown);
        };
        let fill = |text: &str| {
            fills.iter().fold(text.to_string(), |text, (key, value)| {
                text.replace(&format!("{{{key}}}"), value)
            })
        };
        let mut notification = Notification::from(def);
        notification.title = fill(&notification.title);
        notification.body = fill(&notification.body);
        notification.detail = detail;
        let repeat = def.repeat;
        if repeat == Repeat::OnceEver {
            if !self.world.resource_mut::<Profile>().see(id) {
                return Err(NoNotify::AlreadySeen);
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
                .push(id.clone());
        }
        self.world
            .resource_mut::<Notifications>()
            .push(notification);
        Ok(())
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
