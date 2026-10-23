//! The map's bottom key bar: what it says, decided here so the renderer only
//! draws it. The keys' meanings live in app-core, and so does which of them
//! work where the party stands.

use crate::{App, Mode, Walk};

/// One run of `(key, label)` segments; the renderer draws a divider between
/// runs.
pub type KeyGroup = Vec<(String, String)>;

/// Where the party stands, as far as the bar cares: which verbs on it can
/// do anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapLocale {
    Base,
    OpenGrid,
    Stack { market: bool },
}

fn row(key: &str, label: &str) -> (String, String) {
    (key.to_string(), label.to_string())
}

/// The map's own bar for `locale`, in priority order — the renderer takes
/// the longest prefix that fits, so what is last here is what goes first.
///
/// What is not here is in the manual (`?`), the discoverable home for it,
/// and `f` and `L` sit on the filter strip on the opposite border.
///
/// **The order is priority, not reading order, and the census is why.**
/// `the_keybar_fits_the_log_pane` (gui) measures what survives at the
/// smallest supported window. So the crossing pair and the three screens that
/// hold the rest come first, and every locale's bar shares those two groups.
///
/// **`D downed` sits with the screens rather than with the verbs**: it is a
/// store screen and not a field action — `D` opens the same page from the map
/// and from the pack (`App::handle_downed_programs_key`).
///
/// **`hjkl move` and `. wait` are deliberately not here.** Arrow keys walk as
/// well as `hjkl` does, so movement is the one verb a player finds without
/// being told, and `.` does nothing in base space at all. The row they cost
/// was spent on `<>` instead: that pair is the only crossing in the game with
/// no glyph on the map leading to it.
///
/// **One crossing label for all three locales**, `ascend/descend` rather than
/// naming the base: `<` is up and `>` is down whether the party is stepping
/// onto their base or into a stack.
///
/// `TAB` (expand/collapse the log pane) is not here either: the bar has no
/// room to spare for it, and `?` is where it is discovered.
///
/// **The verbs are filtered by locale, because a key the bar names and the
/// place refuses reads as a bug.** `n` cuts only base-space rock; `t` finds a
/// trader only in base space or on a live Stack stall; `c` is not bound
/// underground at all. The Stack's own `g` and `o` take the room that frees,
/// though only about three verbs survive the cut at any supported size —
/// `o` only at 1920x1080 — so the order of the verbs matters more than
/// their number.
pub fn locale_keybar(locale: MapLocale, mining: bool) -> Vec<KeyGroup> {
    // A stall is the reason to stop on this cell, and the bar has room for
    // about three verbs, so its key leads rather than queueing past the cut.
    let mut verbs = match locale {
        MapLocale::Stack { market: true } => vec![row("t", "trade")],
        _ => Vec::new(),
    };
    verbs.push(row("a", "routine"));
    match locale {
        MapLocale::Base => {
            verbs.push(row("c", "collect"));
            verbs.push(row("n", if mining { "mine (on)" } else { "mine" }));
        }
        MapLocale::OpenGrid => verbs.push(row("c", "collect")),
        MapLocale::Stack { .. } => {}
    }
    verbs.push(row("x", "examine"));
    if let MapLocale::Stack { .. } = locale {
        verbs.push(row("g", "map"));
        verbs.push(row("o", "adopt"));
    }
    verbs.push(row("e", "drain"));
    verbs.push(row("r", "recharge"));
    if locale == MapLocale::Base {
        verbs.push(row("t", "trade"));
    }
    verbs.push(row("s", "save"));
    vec![
        vec![row("< >", "ascend/descend")],
        vec![
            row("b", "base"),
            row("i", "pack"),
            row("D", "downed"),
            row("?", "help"),
            row("Esc", "menu"),
        ],
        verbs,
    ]
}

impl App {
    /// The map's bottom bar, one call for every state the map can be in:
    /// the plan's box cursor, a followed camera, a walk under way, then the
    /// map's own bar for where the party stands. A fight's bar is the
    /// renderer's to put over this one.
    ///
    /// Reads `watching` as it stands, so the caller asks
    /// `App::watch_center` first — that is what releases a camera whose
    /// program stopped answering.
    pub fn map_keybar(&self) -> Vec<KeyGroup> {
        if let Some(plan) = self.plan_keybar() {
            return vec![plan];
        }
        let clock = |rows: &mut KeyGroup| {
            rows.push(row("Space", if self.paused { "resume" } else { "pause" }));
            rows.push(row(",", "speed"));
        };
        // Only while the map is the screen: a popup over it takes `Esc` for
        // itself, and the camera stays held for when the popup closes.
        let on_map = self.mode == Mode::Playing;
        if on_map && self.watching.is_some() {
            let mut rows = vec![row("↑↓←→", "step"), row("Esc", "stop watching")];
            clock(&mut rows);
            return vec![rows];
        }
        if on_map && let Some(Walk::Travel { .. }) = self.walk {
            let mut rows = vec![row("Esc", "stop walking")];
            clock(&mut rows);
            return vec![rows];
        }
        let Some(game) = &self.game else {
            return Vec::new();
        };
        let locale = if game.is_underground() {
            MapLocale::Stack {
                market: game.market_underfoot(),
            }
        } else if game.in_base() {
            MapLocale::Base
        } else {
            MapLocale::OpenGrid
        };
        locale_keybar(locale, game.mining())
    }
}
