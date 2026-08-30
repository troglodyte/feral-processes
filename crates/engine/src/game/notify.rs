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
    /// A thin call onto `notify_with_detail` with no detail — every firing
    /// site but `Game::complete_contract` has nothing dynamic to say.
    pub fn notify(&mut self, id: &NotificationId) -> Result<(), NoNotify> {
        self.notify_with_detail(id, None)
    }

    /// `notify`, with a figure the `.ron` file could not have authored —
    /// `Game::complete_contract`'s payout is the one caller today.
    ///
    /// The **one door**. It resolves the def, checks the latch and pushes a
    /// *resolved* value; it draws no RNG and writes no log line. A caller
    /// fires it unconditionally at its site — the once-only rule is
    /// `Repeat::OnceEver` and lives here, so a second `if first_time` check
    /// in Rust would put the policy in two places and let the two disagree.
    pub fn notify_with_detail(
        &mut self,
        id: &NotificationId,
        detail: Option<String>,
    ) -> Result<(), NoNotify> {
        let Some(def) = self.world.resource::<NotificationDb>().get(id) else {
            return Err(NoNotify::Unknown);
        };
        let mut notification = Notification::from(def);
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
