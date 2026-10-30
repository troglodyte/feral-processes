//! What stops play for a moment, in a panel over the map.
//!
//! One variant per milestone notification, and the prose beside it. A notification is
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

/// Every milestone notification the game can raise: news about a moment,
/// fired every time its site is reached.
///
/// First-time guidance (a tutorial, lore) is not here — it lives in
/// `ftue::ENTRIES`, once per run and subject to the player's guidance
/// settings. Nothing in this enum is latched or reaches disk; `slug`
/// is only the stable name `Display` and the censuses use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationKind {
    /// A portal holds and a new sector resolves — `Game::enter_next_zone`.
    Breach,
    /// The breach that lands on `tuning::RAID_MIN_ZONE` —
    /// `Game::enter_next_zone`, queued behind the `Breach` screen that
    /// function opens with, so the two are read back to back.
    ///
    /// **`Always`, and firing once a run is the trigger's shape rather than
    /// a latch** — `LevelCapReached`'s reason. A run breaches into a given
    /// sector exactly once, and latching on `Profile` instead would leave a
    /// second playthrough's first sweep unexplained, which is
    /// `OnboardingMission`'s argument.
    ///
    /// Hung on the breach rather than on the tick sweeps actually go live:
    /// `Game::raid_check` wants `RAID_MIN_BASE_STAFF` undowned bodies as
    /// well, so this is a warning with a Shield's sixteen Core Fragments of
    /// lead time rather than a report of something already true.
    SweepsBegin,
    /// A contract is settled and paid out — `Game::complete_contract`.
    ///
    /// **Templated, and its holes are the contract's own words.** A screen
    /// reading the same sentence for every contract in the game is what it
    /// replaced: what was asked is `Game::objective_line`, the same
    /// derivation `views::ContractRow` and the completion log line use, and
    /// the payout rides `detail`.
    ContractClosed,
    /// An onboarding mission is settled — `Game::complete_contract`, the
    /// **other** half of the same branch `ContractClosed` is one arm of.
    ///
    /// A kind of its own rather than a fill on `ContractClosed`, because that
    /// copy is false twice over during onboarding: there is no Broker
    /// standing for the chain's first several missions, and the board is
    /// deliberately empty until the chain ends. One def carries one body, so
    /// two bodies is two kinds.
    OnboardingComplete,
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
    /// A town the party knew as a Server has become a Mainframe —
    /// `Game::announce_growth`.
    ///
    /// A milestone rather than a tutorial: it teaches nothing, and it can
    /// happen more than once in a run because a world holds more than one
    /// town.
    SettlementGrown,
    /// A level lands the player exactly on `Game::level_cap` —
    /// `Game::award_player_xp`, the `gain.levels > 0` branch.
    ///
    /// **`Always`, and it needs no guard against repeating inside one
    /// sector.** `progression::add_xp` returns before levelling once
    /// `level >= cap`, so the branch this fires from is unreachable until a
    /// breach raises the ceiling — the once-per-sector behaviour is the
    /// trigger's shape, not a check. Fired on *reaching* the cap rather
    /// than on the first XP banked at it, so the advice arrives before a
    /// kill has been spent learning it the hard way.
    ///
    /// Templated: the ceiling is a different number in every sector, so the
    /// copy cannot name one. What it must not name either is what the next
    /// sector opens — research gates at `min_zone` 2 and 3 and nowhere
    /// above, so a body promising new research would be false at seven of
    /// the ten ceilings. Hence "may".
    LevelCapReached,
    /// A research project is finished and paid for — `Game::settle_research`.
    ///
    /// **`Always`**: every project is news, and the tree has thirty-odd.
    /// Templated off the project's own def, and what it opened rides
    /// `detail` rather than a hole: it is `Game::research_unlocks`' sentence,
    /// derived from live defs, and `None` for a node that opens nothing —
    /// which a hole in the body could only fill with a blank paragraph.
    ResearchComplete,
    /// A study attempt has uncovered a research node — `Game::settle_study`,
    /// through `Game::discover_research`.
    ///
    /// **`Always`**: a run has up to twenty discoveries and each is news.
    /// Templated off the node's own `name` and `description`, so there is no
    /// second copy of that prose to drift — the achievement seam's argument.
    /// No `detail`: what the node *unlocks* is what finishing it buys, and
    /// quoting it here would read as the node already being researched.
    ResearchDiscovered,
    /// A lair guardian drops the current zone's Phase Key —
    /// `Game::roll_phase_key`. `Always`: one per zone per run, so a latch
    /// would silence every run after the first.
    PhaseKeyFound,
    /// A Holding Cell's warden lands a decompile and the prisoner joins the
    /// roster — `Game::settle_jail_attempt`. `Always`: every prisoner is
    /// news, and a run holds many. Templated off the program's label.
    ProgramDecompiled,
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
}

impl NotificationKind {
    /// Every kind, for the censuses and the height check the screen has no
    /// scroll to forgive. `Perk::all`'s shape and its reason: a walk over
    /// the whole enum is what makes a census non-vacuous, and the array
    /// length fails to compile when a variant is added without being listed.
    pub fn all() -> [NotificationKind; 11] {
        [
            NotificationKind::Breach,
            NotificationKind::SweepsBegin,
            NotificationKind::ContractClosed,
            NotificationKind::OnboardingComplete,
            NotificationKind::OnboardingMission,
            NotificationKind::SettlementGrown,
            NotificationKind::LevelCapReached,
            NotificationKind::ResearchComplete,
            NotificationKind::ResearchDiscovered,
            NotificationKind::PhaseKeyFound,
            NotificationKind::ProgramDecompiled,
        ]
    }

    /// What this notification says. **Exhaustive by construction** — see the
    /// module doc.
    pub fn def(self) -> NotificationDef {
        match self {
            NotificationKind::Breach => NotificationDef {
                title: "Escalation",
                body: "The portal holds long enough. A new sector resolves around you — new \
                       ground, new species, a harder floor and a higher ceiling.\n\nYour base \
                       travels with you.",
                sprite: None,
                glyph: '*',
                color: GlyphColor::Magenta,
            },
            NotificationKind::SweepsBegin => NotificationDef {
                title: "Sweeps Begin",
                body: "The first sector was beneath the garbage collector's notice. This one is \
                       not. From here a sweep comes for your base as stray data — it damages \
                       whatever it lands on, and staff posted at a machine defend it and are hurt \
                       doing so.\n\nYou will hear one coming. Sweep telemetry thickens around \
                       the anchor well before it lands, and that is the time to post staff and \
                       patch what is damaged.\n\nA Shield soaks damage off every sweep against \
                       every structure \
                       you own, and a second one stacks with the first. A Patch Node writes \
                       damaged structures back up across the whole base on its own, while you are \
                       somewhere else.\n\n[b] for the base report, then Deploy a structure on the Build page.",
                sprite: None,
                // The Shield's own glyph and hue, so the screen and the thing
                // it is telling you to build agree on what to look for. The
                // sweep itself keeps `FirstRaid`'s red `!`.
                glyph: '^',
                color: GlyphColor::Blue,
            },
            NotificationKind::ContractClosed => NotificationDef {
                title: "Contract Closed",
                body: "{name}\n\nCOMPLETED: {objective}\n\nThe Broker marks it settled and pays \
                       out. Fresh work is on the board.",
                sprite: None,
                glyph: '$',
                color: GlyphColor::Green,
            },
            NotificationKind::OnboardingComplete => NotificationDef {
                title: "Mission Complete",
                body: "{name}\n\nCOMPLETED: {objective}\n\n{progress}",
                sprite: None,
                glyph: '!',
                color: GlyphColor::Green,
            },
            NotificationKind::OnboardingMission => NotificationDef {
                title: "{name}",
                body: "{description}\n\nASKED OF YOU: {objective}\n\nPress [4] at any time to view \
                       a summary of your contracts.",
                sprite: None,
                glyph: '!',
                color: GlyphColor::Green,
            },
            NotificationKind::SettlementGrown => NotificationDef {
                title: "It Grew",
                body: "A place you knew as a stop has become a destination. Its shelves run \
                       deeper now, and what it keeps on them is worth the walk in a way it was \
                       not before.\n\nTowns grow on their own, in their own time. They grow \
                       sooner where somebody has been spending.",
                sprite: None,
                // Asked of the kind rather than written as `'M'`, so the
                // screen cannot end up naming a character the map has
                // stopped drawing. `Orange` is the hue `spawn_settlement_at`
                // paints every town in, and its comment there is the reason.
                glyph: crate::settlements::SettlementKind::Mainframe.glyph(),
                color: GlyphColor::Orange,
            },
            NotificationKind::LevelCapReached => NotificationDef {
                title: "Ceiling Reached",
                body: "Level {level} is as far as sector {zone} goes. Kills still pay: the XP \
                       banks and converts straight into Perk Points, so your build keeps \
                       moving.\n\nEscalating is what raises the ceiling. A new sector lifts the \
                       cap, tiers up what your structures can become, and may open research \
                       that is closed to you here.",
                sprite: None,
                glyph: '^',
                color: GlyphColor::Yellow,
            },
            NotificationKind::ResearchComplete => NotificationDef {
                title: "Research Complete",
                body: "{name}\n\n{description}",
                sprite: None,
                // The Research Node's own glyph and hue, `SweepsBegin`'s
                // reason: the screen and the machine that did the work agree
                // on what to look for.
                glyph: 'R',
                color: GlyphColor::Cyan,
            },
            NotificationKind::ResearchDiscovered => NotificationDef {
                title: "Research Discovered",
                body: "{name}\n\n{description}",
                sprite: None,
                // `ResearchComplete`'s glyph and hue: the same machine did
                // the work, and the two halves of one node's story should
                // not look like two different systems.
                glyph: 'R',
                color: GlyphColor::Cyan,
            },
            NotificationKind::PhaseKeyFound => NotificationDef {
                title: "Phase Key Recovered",
                body: "{name}\n\n{flavour}",
                sprite: None,
                glyph: '*',
                color: GlyphColor::Yellow,
            },
            NotificationKind::ProgramDecompiled => NotificationDef {
                title: "Program Decompiled",
                body: "{name} accepts its new parameters and joins your roster.\n\nA warden \
                       kept at the Holding Cell until the decompile took.",
                sprite: None,
                // The Holding Cell's own glyph and hue, `ResearchComplete`'s
                // reason: the screen and the machine that did the work agree.
                glyph: 'J',
                color: GlyphColor::Red,
            },
        }
    }

    /// A stable name for this kind, for `Display` and the censuses.
    pub fn slug(self) -> &'static str {
        match self {
            NotificationKind::Breach => "milestone_breach",
            NotificationKind::SweepsBegin => "milestone_sweeps_begin",
            NotificationKind::ContractClosed => "milestone_contract",
            NotificationKind::OnboardingComplete => "onboarding_complete",
            NotificationKind::OnboardingMission => "onboarding_mission",
            NotificationKind::SettlementGrown => "milestone_settlement_grown",
            NotificationKind::LevelCapReached => "milestone_level_cap",
            NotificationKind::ResearchComplete => "milestone_research_complete",
            NotificationKind::ResearchDiscovered => "milestone_research_discovered",
            NotificationKind::PhaseKeyFound => "milestone_phase_key_found",
            NotificationKind::ProgramDecompiled => "milestone_program_decompiled",
        }
    }
}

impl std::fmt::Display for NotificationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
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
    /// log line uses — on both completion screens, since a mission of the
    /// onboarding chain pays exactly as a Broker's contract does.
    /// **Not a field on `NotificationDef`** — it is drawn
    /// from live game state at the moment the notification fires, so it is a
    /// parameter to the door (`Game::notify_with_detail`) rather than
    /// something the table could ever hold. `None` for every site that has
    /// nothing to add.
    pub detail: Option<String>,
    /// The FTUE key this popup was raised for, so app-core knows to offer
    /// "don't show again". `None` for every milestone.
    pub ftue_key: Option<String>,
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
            ftue_key: None,
        }
    }
}

impl Notification {
    /// This notification with `{hole}` placeholders filled in its title and
    /// body and `detail` attached: the one fill both the milestone door and
    /// the FTUE door go through. A hole nobody names is left standing, so a
    /// census can see it.
    pub fn filled(mut self, fills: &[(&str, &str)], detail: Option<String>) -> Self {
        let fill = |text: &str| {
            fills.iter().fold(text.to_string(), |text, (hole, value)| {
                text.replace(&format!("{{{hole}}}"), value)
            })
        };
        self.title = fill(&self.title);
        self.body = fill(&self.body);
        self.detail = detail;
        self
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
        let mut keys: Vec<&str> = NotificationKind::all().iter().map(|k| k.slug()).collect();
        keys.sort_unstable();
        let listed = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), listed, "a kind is listed twice in `all`");
    }

    /// The latch key reaches `profile.ron` and a duplicate would make two
    /// notifications share one sighting — the second silently never firing.
    #[test]
    fn every_slug_is_unique_and_non_empty() {
        let mut keys: Vec<&str> = NotificationKind::all().iter().map(|k| k.slug()).collect();
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
