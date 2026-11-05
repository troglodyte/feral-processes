//! The one door a notification is queued through.

use crate::Game;
use crate::notifications::{Notification, NotificationKind, NotificationSubject};
use crate::resources::{Notifications, PendingProfileWrites};

/// What a notice carries beyond its kind. `Default` is "nothing to add".
#[derive(Clone, Copy, Default)]
pub struct NotifyArgs<'a> {
    /// `{hole}` placeholders in the title and body. A hole no caller names is
    /// left standing rather than blanked, so a census can see it.
    pub fills: &'a [(&'a str, &'a str)],
    /// A figure the authored copy could not have carried.
    pub detail: Option<&'a str>,
    /// What on the map this is about.
    pub subject: Option<NotificationSubject>,
}

impl Game {
    /// Queues `kind`, to take the screen the next time the player is
    /// standing on the map. `true` if it queued.
    ///
    /// A thin call onto the door with no extras — most
    /// firing sites have nothing dynamic to say.
    pub fn notify(&mut self, kind: NotificationKind) -> bool {
        self.queue_notification(kind, NotifyArgs::default())
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
    /// For a site that needs a subject as well, `notify_with`.
    ///
    pub fn notify_filled(
        &mut self,
        kind: NotificationKind,
        fills: &[(&str, &str)],
        detail: Option<String>,
    ) -> bool {
        self.queue_notification(
            kind,
            NotifyArgs {
                fills,
                detail: detail.as_deref(),
                subject: None,
            },
        )
    }

    /// `notify_filled`, plus the map subject the notice points at.
    ///
    /// **One door carrying an args struct, not a wrapper per combination.**
    /// Fills, a detail and a subject are orthogonal, so the combinations
    /// multiply; `NotifyArgs` is the one place a new extra is added.
    pub fn notify_with(&mut self, kind: NotificationKind, args: NotifyArgs<'_>) -> bool {
        self.queue_notification(kind, args)
    }

    /// The **one door** for milestones. It reads the def and pushes a
    /// *resolved* value; it draws no RNG and writes no log line. Once-per-run
    /// guidance goes through `Game::ftue`, whose run set decides, so a second
    /// `if first_time` check at a site would put the policy in two places.
    ///
    /// `false`, queueing nothing, when the player chose "don't show again" for
    /// this kind (`Profile::notifications_never`). Only the screen is
    /// skipped: a log line or alert-board entry the site makes itself still
    /// happens.
    fn queue_notification(&mut self, kind: NotificationKind, args: NotifyArgs<'_>) -> bool {
        let silence_key = kind.repeats().then(|| kind.slug());
        if silence_key.is_some_and(|slug| self.notification_silenced(slug)) {
            return false;
        }
        let mut notification =
            Notification::from(kind.def()).filled(args.fills, args.detail.map(str::to_string));
        notification.subject = args.subject;
        notification.silence_key = silence_key;
        self.world
            .resource_mut::<Notifications>()
            .push(notification);
        true
    }

    fn notification_silenced(&self, slug: &str) -> bool {
        self.world
            .resource::<crate::achievements::Profile>()
            .notifications_never
            .iter()
            .any(|never| never == slug)
    }

    /// "Don't show again" for a repeating kind, any run. The `ftue_never`
    /// precedent: through `Game` and then `flush_profile_writes`, since the
    /// engine's copy of the profile is authoritative during a run.
    pub fn silence_notification(&mut self, slug: &str) {
        if self.notification_silenced(slug) {
            return;
        }
        self.world
            .resource_mut::<crate::achievements::Profile>()
            .notifications_never
            .push(slug.to_string());
        self.world.resource_mut::<PendingProfileWrites>().ftue = true;
    }

    /// Where a notice's subject is on the map the player is viewing, or
    /// `None` — the notice then draws centred. An entity goes through
    /// `watch_position`, so the marker and Watch share one source. A tile is
    /// on the displayed map whenever the party is on the surface: outposts and
    /// the base anchor share its one coordinate space (ADR 0032), and the
    /// only other place the party can be, the Stack, draws no field.
    pub fn subject_position(&self, subject: &NotificationSubject) -> Option<(i32, i32)> {
        match *subject {
            NotificationSubject::Entity(entity) => self.watch_position(entity),
            NotificationSubject::Tile(tile) => self.field_pos().map(|_| tile),
        }
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
