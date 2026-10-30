//! `Game`'s side of the FTUE: gather, decide (`ftue::decide`), apply.

use crate::Game;
use crate::achievements::Profile;
use crate::ftue::{
    FtueDecision, FtuePlayed, FtueTrigger, PendingCrafted, crafted_key, decide, entry_for,
};
use crate::notifications::Notification;
use crate::resources::{Notifications, PendingProfileWrites};

impl Game {
    /// Fires `trigger`'s message if the list has one and the rules allow.
    /// `true` if it queued. Call sites fire unconditionally; the run set is
    /// what makes it once.
    pub fn ftue(&mut self, trigger: FtueTrigger) -> bool {
        self.ftue_filled(trigger, &[], None)
    }

    /// `ftue`, with `{hole}` placeholders filled and an optional detail line,
    /// as `notify_filled` does.
    pub fn ftue_filled(
        &mut self,
        trigger: FtueTrigger,
        fills: &[(&str, &str)],
        detail: Option<String>,
    ) -> bool {
        self.ftue_key_fired(&trigger.key(), entry_for(&trigger), fills, detail)
    }

    /// The key-keyed core: triggers whose id is only known at run time
    /// (a research completing, a structure standing) arrive as a key and are
    /// matched to an entry by it. Latches even with no entry, so every hook is
    /// observable through `ftue_shown` without a shipped message.
    pub(crate) fn ftue_key_fired(
        &mut self,
        key: &str,
        entry: Option<&'static crate::ftue::FtueEntry>,
        fills: &[(&str, &str)],
        detail: Option<String>,
    ) -> bool {
        let decision = decide(
            key,
            &self.world.resource::<FtuePlayed>().0,
            &self.world.resource::<Profile>().ftue,
        );
        if decision == FtueDecision::AlreadyShown {
            return false;
        }
        self.world
            .resource_mut::<FtuePlayed>()
            .0
            .insert(key.to_string());
        let (FtueDecision::Show, Some(entry)) = (decision, entry) else {
            return false;
        };
        let fill = |text: &str| {
            fills.iter().fold(text.to_string(), |text, (hole, value)| {
                text.replace(&format!("{{{hole}}}"), value)
            })
        };
        self.world
            .resource_mut::<Notifications>()
            .push(Notification {
                title: fill(entry.title),
                body: fill(entry.body),
                sprite: entry.sprite.map(str::to_string),
                glyph: entry.glyph,
                color: entry.color,
                detail,
                ftue_key: Some(key.to_string()),
            });
        true
    }

    /// Fires the entry whose trigger key is `key`, for the hooks that only
    /// hold a runtime id.
    pub(crate) fn ftue_by_key(&mut self, key: &str) -> bool {
        let entry = crate::ftue::ENTRIES.iter().find(|e| e.on.key() == key);
        self.ftue_key_fired(key, entry, &[], None)
    }

    /// Whether `key` has been handled this run — shown or latched. `&self`,
    /// for hook tests and the frontend.
    pub fn ftue_shown(&self, key: &str) -> bool {
        self.world.resource::<FtuePlayed>().0.contains(key)
    }

    /// Fires `Crafted` for each item the assembler made since the last call.
    /// Sorted, so the order never depends on query iteration.
    pub(crate) fn drain_pending_crafted(&mut self) {
        let mut items = std::mem::take(&mut self.world.resource_mut::<PendingCrafted>().0);
        items.sort();
        items.dedup();
        for item in items {
            self.ftue_by_key(&crafted_key(&item.0));
        }
    }

    /// "Don't show this message again", any run.
    pub fn ftue_never(&mut self, key: &str) {
        let mut profile = self.world.resource_mut::<Profile>();
        if profile.ftue.never.iter().any(|never| never == key) {
            return;
        }
        profile.ftue.never.push(key.to_string());
        self.world.resource_mut::<PendingProfileWrites>().ftue = true;
    }

    /// The guidance switch.
    pub fn set_guidance(&mut self, enabled: bool) {
        let mut profile = self.world.resource_mut::<Profile>();
        if profile.ftue.enabled == enabled {
            return;
        }
        profile.ftue.enabled = enabled;
        self.world.resource_mut::<PendingProfileWrites>().ftue = true;
    }

    /// "Show all again": guidance on and every opt-out forgotten.
    pub fn reset_guidance(&mut self) {
        self.world.resource_mut::<Profile>().ftue = Default::default();
        self.world.resource_mut::<PendingProfileWrites>().ftue = true;
    }

    pub fn guidance_enabled(&self) -> bool {
        self.world.resource::<Profile>().ftue.enabled
    }
}
