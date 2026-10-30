//! The `?` shortcut sheet: every key bound where the player stands, decided
//! here so the renderer only draws it.
//!
//! Plain data and one pure function per area. The lists are read off the
//! handlers (`playing.rs`, `battle.rs`, `tactical.rs`), not off the manual;
//! a key listed here must be bound there, because a popup entry that does
//! nothing reads as a bug. Key strings match the bars' byte for byte
//! (`"Esc"`, `"↑↓←→"`, `"< >"`) so the agreement tests compare plain text.

use crate::{App, Mode};

/// Where the player is, as far as the keys are concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutArea {
    /// The field, on or beside laid floor.
    Base,
    /// The field anywhere else.
    Field,
    /// Underground.
    Stack,
    /// The battle map.
    TacticalMap,
    /// The abstract group fight.
    ClassicBattle,
}

/// One titled run of `(key, label)` rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutGroup {
    pub title: &'static str,
    pub keys: Vec<(&'static str, &'static str)>,
}

/// What the popup draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutSheet {
    pub title: String,
    pub groups: Vec<ShortcutGroup>,
}

fn group(title: &'static str, keys: &[(&'static str, &'static str)]) -> ShortcutGroup {
    ShortcutGroup {
        title,
        keys: keys.to_vec(),
    }
}

/// The keys the map binds on the surface and underground alike: the top
/// match of `handle_playing_key`, which runs before the Stack hand-off.
fn screens() -> ShortcutGroup {
    group(
        "Screens",
        &[
            ("?", "hold: this list"),
            ("b", "base report"),
            ("i", "pack and party"),
            ("D", "downed programs"),
            ("N", "alerts"),
            ("u", "compass"),
            ("L", "history"),
            ("f", "filter the log"),
            ("Tab", "expand the log pane"),
            ("1-4", "side column tabs"),
            ("Esc", "game menu"),
        ],
    )
}

fn clock_and_zoom() -> [(&'static str, &'static str); 3] {
    [
        ("Space", "pause / resume"),
        (",", "world speed"),
        ("+ -", "zoom"),
    ]
}

fn field_groups() -> Vec<ShortcutGroup> {
    let mut moving = vec![
        ("↑↓←→", "move"),
        ("hjkl", "move"),
        ("numpad 1379", "step diagonally"),
        (".", "wait a turn"),
        ("g", "world map"),
    ];
    moving.extend(clock_and_zoom());
    vec![
        ShortcutGroup {
            title: "Moving",
            keys: moving,
        },
        group(
            "Actions",
            &[
                ("a", "call a routine"),
                ("c", "open the structure beside you"),
                ("n", "cutting tools out / away"),
                ("x", "examine a direction"),
                ("e", "drain a Power Cell"),
                ("r", "recharge"),
                ("t", "trade"),
                ("s", "save"),
            ],
        ),
        screens(),
    ]
}

/// Base is the field's list plus the keys that only do anything on the
/// base's floor, appended to Actions rather than written out a second time.
fn base_groups() -> Vec<ShortcutGroup> {
    let mut groups = field_groups();
    let actions = groups
        .iter_mut()
        .find(|g| g.title == "Actions")
        .expect("the field list has an Actions group");
    actions.keys.extend([
        ("v", "lay a floor tile"),
        ("m", "excavation plan"),
        ("z", "stockpile zone"),
        ("d", "demolish a direction"),
        ("o", "pick up"),
        ("y", "put down"),
    ]);
    groups
}

fn stack_groups() -> Vec<ShortcutGroup> {
    let mut moving = vec![
        ("↑↓←→", "forward, about-face, turn left, turn right"),
        ("hjkl", "turn left, about-face, forward, turn right"),
        ("< >", "ascend / descend"),
        (".", "wait a turn"),
        ("g", "frame map"),
    ];
    moving.extend(clock_and_zoom());
    vec![
        ShortcutGroup {
            title: "Moving",
            keys: moving,
        },
        group(
            "Actions",
            &[
                ("a", "call a routine"),
                ("x", "examine a direction"),
                ("o", "adopt an orphaned process"),
                ("t", "trade, where someone sells"),
                ("e", "drain a Power Cell"),
                ("r", "recharge"),
                ("s", "save"),
            ],
        ),
        screens(),
    ]
}

fn tactical_groups() -> Vec<ShortcutGroup> {
    vec![
        group(
            "Moving",
            &[
                ("↑↓←→ numpad", "step, or move the aim cursor"),
                ("Enter", "confirm the aim"),
                ("Esc", "cancel the aim / clear the target"),
            ],
        ),
        group(
            "Actions",
            &[
                ("a", "attack"),
                ("d", "defend"),
                ("s", "special"),
                ("U", "use item"),
                ("V", "revert an emulation"),
                ("E", "end turn"),
            ],
        ),
        group("Charging", &[("H", "hold"), ("X", "release")]),
        group(
            "Whole fight",
            &[
                ("R", "resolve"),
                ("A", "auto-attack"),
                ("any key", "stop auto-attack"),
                ("?", "hold: this list"),
            ],
        ),
    ]
}

fn classic_groups() -> Vec<ShortcutGroup> {
    vec![
        group(
            "Actions",
            &[
                ("a", "attack"),
                ("d", "defend"),
                ("s", "special"),
                ("r", "revert an emulation"),
                ("u", "use item"),
                ("Esc", "undo the last pick"),
            ],
        ),
        group("Charging", &[("h", "hold"), ("x", "release")]),
        group(
            "Whole party",
            &[
                ("A", "all attack"),
                ("D", "all defend"),
                ("R", "resolve"),
                ("j", "jack out"),
            ],
        ),
        group(
            "Screen",
            &[("↑↓", "scroll the narration"), ("?", "hold: this list")],
        ),
    ]
}

/// The sheet for `area`. Pure.
pub fn shortcut_sheet_for(area: ShortcutArea) -> ShortcutSheet {
    let (name, groups) = match area {
        ShortcutArea::Base => ("BASE", base_groups()),
        ShortcutArea::Field => ("FIELD", field_groups()),
        ShortcutArea::Stack => ("STACK", stack_groups()),
        ShortcutArea::TacticalMap => ("TACTICAL", tactical_groups()),
        ShortcutArea::ClassicBattle => ("TACTICAL", classic_groups()),
    };
    ShortcutSheet {
        title: format!("SHORTCUTS — {name}"),
        groups,
    }
}

impl ShortcutSheet {
    /// Whether `key` is listed anywhere on the sheet.
    pub fn lists(&self, key: &str) -> bool {
        self.groups
            .iter()
            .any(|g| g.keys.iter().any(|(k, _)| *k == key))
    }
}

impl App {
    /// The area `?` is live in, if it is: the map, the battle map and its
    /// aim cursor, and the group fight. Elsewhere `?` is ordinary typed text
    /// (a name, a filter), so this is `None` there and the key passes
    /// through.
    pub fn shortcut_area(&self) -> Option<ShortcutArea> {
        match self.mode {
            Mode::Playing => {
                let game = self.game.as_ref()?;
                Some(if game.is_underground() {
                    ShortcutArea::Stack
                } else if game.in_base_reach() {
                    ShortcutArea::Base
                } else {
                    ShortcutArea::Field
                })
            }
            Mode::TacticalBattle | Mode::TacticalAim => Some(ShortcutArea::TacticalMap),
            Mode::Battle => Some(ShortcutArea::ClassicBattle),
            _ => None,
        }
    }

    /// What the held `?` shows, or `None` where `?` is not live.
    pub fn shortcut_sheet(&self) -> Option<ShortcutSheet> {
        self.shortcut_area().map(shortcut_sheet_for)
    }
}
