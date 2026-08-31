//! What takes the whole screen for a moment.
//!
//! One variant per notification, and the prose beside it. A notification is
//! **not content** — it is a hook into a particular moment in a particular
//! function, and there is no shape to express in data the way `SpeciesDef`
//! or `ItemDef` have one. So this module is the whole feature: the enum is
//! the census, `NotificationKind::def` is the copy, and adding one is a
//! variant plus an arm plus a call site.
//!
//! **`def` is an exhaustive match and must stay so** — `cell_mark`'s rule.
//! Written as a table lookup with a fallback, a new variant would ship
//! blank; written as a match, it fails to compile until somebody writes the
//! words. That is the whole reason the prose lives in a match arm rather
//! than in a `&[(NotificationKind, NotificationDef)]` beside it.
//!
//! This used to be `assets/notifications/*.ron` behind a `NotificationDb`,
//! on the half-data seam `needs::NeedDef` and `memories::MemoryDef` still
//! sit on. It came back because the data half bought nothing: a `.ron` file
//! cannot say *when* it fires, so every notification already needed Rust,
//! and the loader's whole cost — an id newtype, an absent-directory rule, a
//! malformed-file rule, a pairing census in the test suite to catch a def
//! nothing fires — was paid to make seven strings editable.

use crate::components::GlyphColor;

/// Whether a notification may fire more than once.
///
/// Two policies rather than three. The obvious third — once per *run* —
/// was considered and rejected: its latch would have to live on the
/// session-only queue resource, so "once per run" would quietly mean "once
/// per session" and fire again on every reload. A name that lies is worse
/// than a missing policy, and this is an additive variant the day something
/// actually wants it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Repeat {
    /// Fires every time its site is reached. A breach, a contract landing —
    /// news about *this* moment, which has happened again.
    #[default]
    Always,
    /// Fires once and never again, on this machine, across every run. The
    /// latch is `achievements::Profile::seen_notifications`, which is not the
    /// save file — so a tutorial seen in one run stays seen in the next.
    OnceEver,
}

/// Every notification the game can raise.
///
/// **The order of these variants is not save format** — unlike `Perk`, which
/// is bincoded positionally into `PlayerSave`. What reaches disk is
/// `latch_key`'s string, in `profile.ron`, so these may be reordered and
/// renamed freely and only that method's arms are load-bearing.
///
/// Grouped by what the player is being told: a *tutorial* is the first time
/// a mechanic happens to them and latches forever, a *milestone* is news
/// about this moment and repeats. `tutorials_latch_and_milestones_do_not`
/// holds the two groups to their policies, because getting it backwards is
/// silent — a tutorial that re-fires reads as a bug in the screen, and a
/// milestone that fires once reads as one in the trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationKind {
    // --- Tutorials: `OnceEver`, the first time a mechanic happens to you ---
    /// The Home is raised and base space exists — `Game::place_structure`.
    BaseFounding,
    /// The party drops below the on-ramp for the first time —
    /// `Game::descend_to`.
    FirstDescent,
    /// A GC Entropy Sweep opens on the base — `Game::run_raid`.
    FirstRaid,
    /// A work order is filed and accepted — `Game::queue_work_order`.
    FirstWorkOrder,
    /// Static reaches the player for the first time — the movement hook in
    /// `game/turn.rs`, on the step where `Terrain::event` is live on the
    /// destination tile. Fired at the same site the effect is applied, not
    /// on the epoch boundary `Game::note_static_turnover` announces from —
    /// that fires for every biome turnover regardless of where the player
    /// is standing, and a player three biomes away must not be told
    /// something happened to them that didn't.
    FirstStatic,

    // --- Milestones: `Always`, news about a moment that has happened again ---
    /// A portal holds and a new sector resolves — `Game::enter_next_zone`.
    Breach,
    /// A contract is settled and paid out — `Game::complete_contract`. The
    /// one firing site that attaches a `detail`.
    ContractClosed,
    /// The onboarding chain has handed out a mission —
    /// `Game::ensure_tutorial_held`.
    ///
    /// **Deliberately `Always`, and deliberately not grouped with the
    /// tutorials above.** The chain runs on every new game, so a briefing
    /// latched across runs would leave a second playthrough's eleven
    /// missions unexplained. It is also the one templated def: its `{holes}`
    /// are filled by the mission's own `ContractDef` through
    /// `Game::notify_filled`, which is what lets one arm be read for eleven
    /// subjects rather than eleven arms each repeating a mission's own name.
    OnboardingMission,
}

/// One notification's authored copy.
///
/// `'static` throughout: this is a compile-time table now, so nothing here
/// is allocated and `def` costs nothing to call twice.
#[derive(Clone, Copy, Debug)]
pub struct NotificationDef {
    /// The heading. Short — it is drawn large and does not wrap.
    pub title: &'static str,
    /// The paragraph under it, wrapped at draw time through `text::wrap`.
    /// `\n\n` starts a new paragraph.
    pub body: &'static str,
    /// A name in `assets/sprites/`. **Optional by construction**: the sprite
    /// *substitutes* for `glyph` and never draws beside it, and a name the
    /// texture table has nothing under falls back to the glyph — the
    /// `Painter::sprite` seam's own rule, inherited unchanged. No shipped
    /// notification names one yet.
    pub sprite: Option<&'static str>,
    /// What is drawn when there is no sprite, which today is always.
    pub glyph: char,
    /// Resolved through `hud::palette::glyph`, the one table a content hue is
    /// drawn from. The renderer does not invent a colour for this screen.
    pub color: GlyphColor,
    pub repeat: Repeat,
}

impl NotificationKind {
    /// Every kind, for the censuses and the height check the screen has no
    /// scroll to forgive. `Perk::all`'s shape and its reason: a walk over
    /// the whole enum is what makes a census non-vacuous, and the array
    /// length fails to compile when a variant is added without being listed.
    pub fn all() -> [NotificationKind; 8] {
        [
            NotificationKind::BaseFounding,
            NotificationKind::FirstDescent,
            NotificationKind::FirstRaid,
            NotificationKind::FirstWorkOrder,
            NotificationKind::FirstStatic,
            NotificationKind::Breach,
            NotificationKind::ContractClosed,
            NotificationKind::OnboardingMission,
        ]
    }

    /// What this notification says. **Exhaustive by construction** — see the
    /// module doc.
    pub fn def(self) -> NotificationDef {
        match self {
            NotificationKind::BaseFounding => NotificationDef {
                title: "Base Space",
                body: "Your Home stands, the base runs out of phase with the zone map. Climb up \
                       (# icon) to visit.\n\n [b] for the base menu: build, craft, and assign \
                       entities to structures.\n\n Programs you own and are not in the party will \
                       live and work in the base.",
                sprite: None,
                glyph: '#',
                color: GlyphColor::Cyan,
                repeat: Repeat::OnceEver,
            },
            NotificationKind::FirstDescent => NotificationDef {
                title: "The Stack",
                body: "Below the zone field, the ground stops becomes a stack (a dungeon). Each \
                       frame (level) down is more dangerous than the last.\n\n Something is \
                       scanning your progress, the 'Trace' climbs the deeper and longer you stay, \
                       and what it draws is not friendly, encounters will become more frequent. \
                       You sense something big, humming with energy below.\n\nPress [g] for the \
                       map of the frame you are standing in.",
                sprite: None,
                glyph: '>',
                color: GlyphColor::Cyan,
                repeat: Repeat::OnceEver,
            },
            NotificationKind::FirstRaid => NotificationDef {
                title: "GC Entropy Sweep",
                body: "The garbage collector has found your base. A sweep comes to clean up stray \
                       data, ...your structures, and it will keep coming.\n\nStaff posted at a \
                       machine defend it, and take damage doing so. There must be a way to defend \
                       against it.",
                sprite: None,
                glyph: '!',
                color: GlyphColor::Red,
                repeat: Repeat::OnceEver,
            },
            NotificationKind::FirstWorkOrder => NotificationDef {
                title: "The Order Is Filed",
                body: "You have declared to your semi sentient programs what to craft, and what to \
                       keep in stock. Every tick, the base recomputes which machines that order \
                       needs, who is free to stand at them, and how far along each one is. \n\nA \
                       standing order is a level the base keeps topped up rather than a batch it \
                       fills once. \n\nIf the line stalls, the order screen names the machine it \
                       is waiting on.",
                sprite: None,
                glyph: '&',
                color: GlyphColor::Yellow,
                repeat: Repeat::OnceEver,
            },
            NotificationKind::FirstStatic => NotificationDef {
                title: "Static",
                body: "That is Static, not the ground itself — interference riding on \
                       top of the terrain, tied to the whole biome you are standing in rather \
                       than to this one tile.\n\nIt runs on its own clock. Given time it clears, \
                       and something else may settle over the same ground later. There is \
                       nothing here to switch off.",
                sprite: None,
                glyph: '~',
                color: GlyphColor::Orange,
                repeat: Repeat::OnceEver,
            },
            NotificationKind::Breach => NotificationDef {
                title: "Breach",
                body: "The portal holds long enough. A new sector resolves around you — new \
                       ground, new species, a harder floor and a higher ceiling.\n\nYour base \
                       travels with you.",
                sprite: None,
                glyph: '*',
                color: GlyphColor::Magenta,
                repeat: Repeat::Always,
            },
            NotificationKind::ContractClosed => NotificationDef {
                title: "Contract Closed",
                body: "The Broker marks the contract settled and pays out, more contracts are \
                       available.",
                sprite: None,
                glyph: '$',
                color: GlyphColor::Green,
                repeat: Repeat::Always,
            },
            NotificationKind::OnboardingMission => NotificationDef {
                title: "{name}",
                body: "{description}\n\nASKED OF YOU: {objective}\n\nPress [4] at any time to view \
                       a summary of your contracts.",
                sprite: None,
                glyph: '!',
                color: GlyphColor::Green,
                repeat: Repeat::Always,
            },
        }
    }

    /// The key a `Repeat::OnceEver` sighting is latched under in
    /// `profile.ron`.
    ///
    /// **These strings are a file format and the variant names are not.**
    /// `Profile::load` discards the *whole* profile — achievements included
    /// — when it cannot parse, so a player who has already seen a tutorial
    /// must keep matching the string their file holds. They are the ids the
    /// deleted `assets/notifications/*.ron` used, unchanged, which is why
    /// they still carry the `tutorial_`/`milestone_` prefixes the enum
    /// expresses by grouping instead.
    pub fn latch_key(self) -> &'static str {
        match self {
            NotificationKind::BaseFounding => "tutorial_base_founding",
            NotificationKind::FirstDescent => "tutorial_first_descent",
            NotificationKind::FirstRaid => "tutorial_first_raid",
            NotificationKind::FirstWorkOrder => "tutorial_first_work_order",
            NotificationKind::FirstStatic => "tutorial_first_static",
            NotificationKind::Breach => "milestone_breach",
            NotificationKind::ContractClosed => "milestone_contract",
            NotificationKind::OnboardingMission => "onboarding_mission",
        }
    }
}

impl std::fmt::Display for NotificationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.latch_key())
    }
}

/// A notification the queue is holding: **resolved text**, never a kind.
///
/// `ActiveContract`'s rule and `Sortie`'s. What travels is the finished
/// prose, which is what lets `achievement_system` push one built from an
/// achievement's own `name` and `description` — a second *source*, not a
/// second door, and the reason a `.ron` per achievement repeating that text
/// was never written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub sprite: Option<String>,
    pub glyph: char,
    pub color: GlyphColor,
    /// A figure the firing site knows and the authored copy cannot: a
    /// contract's payout, worded through the same `Game::reward_line` the
    /// log line uses. **Not a field on `NotificationDef`** — it is drawn
    /// from live game state at the moment the notification fires, so it is a
    /// parameter to the door (`Game::notify_with_detail`) rather than
    /// something the table could ever hold. `None` for every site that has
    /// nothing to add.
    pub detail: Option<String>,
}

impl From<NotificationDef> for Notification {
    fn from(def: NotificationDef) -> Self {
        Notification {
            title: def.title.to_string(),
            body: def.body.to_string(),
            sprite: def.sprite.map(str::to_string),
            glyph: def.glyph,
            color: def.color,
            detail: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `all` is what every census walks, so a variant missing from it makes
    /// each of them quietly weaker. The array length catches an addition;
    /// this catches a duplicate standing in for the one that was forgotten.
    #[test]
    fn all_lists_every_kind_exactly_once() {
        let mut keys: Vec<&str> = NotificationKind::all()
            .iter()
            .map(|k| k.latch_key())
            .collect();
        keys.sort_unstable();
        let listed = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), listed, "a kind is listed twice in `all`");
    }

    /// The latch key reaches `profile.ron` and a duplicate would make two
    /// notifications share one sighting — the second silently never firing.
    #[test]
    fn every_latch_key_is_unique_and_non_empty() {
        let mut keys: Vec<&str> = NotificationKind::all()
            .iter()
            .map(|k| k.latch_key())
            .collect();
        assert!(keys.iter().all(|k| !k.is_empty()));
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), NotificationKind::all().len());
    }

    /// A def with nothing to say is a screen that opens blank. Free when the
    /// copy is a literal, and the thing a `.ron` loader needed a runtime
    /// check for.
    #[test]
    fn every_kind_has_a_title_and_a_body() {
        for kind in NotificationKind::all() {
            let def = kind.def();
            assert!(!def.title.is_empty(), "{kind}");
            assert!(!def.body.is_empty(), "{kind}");
        }
    }
}
