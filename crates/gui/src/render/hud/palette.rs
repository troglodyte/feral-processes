//! The HUD's colours, addressed by **role** and never by index.
//!
//! Taken from the "2a Paned Command" handoff's two colour tables — the 16
//! ANSI entries for content and the chrome greys for fills, borders and bar
//! troughs. The values are written as hex literals here because that is the
//! form the handoff states them in and the form anyone checking this file
//! against it will be reading; `rgb` does the one conversion.
//!
//! **Two reservations are the design, not a convention.**
//!
//! - [`ATTENTION`] means *the player must act*. An idle structure, an
//!   unspent perk point, a full roster. It is never decorative.
//! - [`THREAT`] means *hostility or inbound harm*. Never an ordinary error.
//!
//! A role addressed by name is what keeps those two true. Reaching past
//! these constants for a raw colour is how a reservation lapses, and a
//! lapsed reservation is invisible until the screen stops meaning anything.
//!
//! Scope is the HUD and the map's entity glyphs. The popup screens keep
//! `render/mod.rs`'s colours; they draw *over* the HUD, so the seam between
//! the two palettes is not visible in play.

// The table is transcribed whole rather than grown a phase at a time, and
// that is a deliberate exception to this repo's delete-what-is-unused rule.
// The reservations below are the load-bearing part of the design, and the
// two tests that hold them — `the_reserved_roles_are_separable` and
// `every_content_role_is_opaque` — can only assert them over the complete
// set. Trimming to what phase 1 paints would delete THREAT and ATTENTION,
// and with them the only thing checking that "act" and "harm" stay
// distinguishable.
//
// This allow is self-liquidating: phases 2, 3 and 4 have consumed
// PANE_TITLE, the CH_* channels and KEYBAR_DIVIDER, and ATTENTION, THREAT,
// HEALTHY and DIVIDER; phase 5 takes ALERT_ROW_BG and KEYCAP_BG, phase 6
// PLAYER and MAP_FLOOR. **Delete this attribute when phase 6 lands** — if it still suppresses anything then,
// that entry is genuinely unused and should go instead.
#![allow(dead_code)]

use crate::paint::Color;

/// One opaque colour from the hex form the handoff writes.
const fn rgb(hex: u32) -> Color {
    Color::new(
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
        1.0,
    )
}

// ---------------------------------------------------------------------------
// Content roles
// ---------------------------------------------------------------------------

/// br yellow — **the player must act**. Reserved; never decorative.
pub(in crate::render) const ATTENTION: Color = rgb(0xe3b341);
/// br red — **hostility and inbound harm**. Reserved; never an ordinary error.
pub(in crate::render) const THREAT: Color = rgb(0xf26d6d);
/// green — a healthy bar fill, and the calm `ALL NOMINAL` state.
pub(in crate::render) const HEALTHY: Color = rgb(0x4fa65b);
/// br cyan — pane titles on their borders.
pub(in crate::render) const PANE_TITLE: Color = rgb(0x56d4dd);
/// br cyan — the player's `@`, and an upgradeable item.
pub(in crate::render) const PLAYER: Color = rgb(0x56d4dd);
/// br white — a keycap letter, or a value being emphasised. Only those.
pub(in crate::render) const EMPHASIS: Color = rgb(0xe8eef4);
/// white — body text and table rows.
pub(in crate::render) const BODY: Color = rgb(0xa8b3bf);
/// br black — dim labels, sub-heads, inert hints.
pub(in crate::render) const LABEL: Color = rgb(0x3a4550);

// ---------------------------------------------------------------------------
// Log channels
// ---------------------------------------------------------------------------

pub(in crate::render) const CH_FIELD: Color = rgb(0x3fa9b5);
pub(in crate::render) const CH_GAIN: Color = rgb(0x4a7fd0);
pub(in crate::render) const CH_BASE: Color = rgb(0x7ee787);
pub(in crate::render) const CH_DEFEND: Color = rgb(0xf26d6d);
pub(in crate::render) const CH_IDLE: Color = rgb(0xe3b341);

// ---------------------------------------------------------------------------
// Chrome — fills, rules and troughs. Outside the 16 content entries.
// ---------------------------------------------------------------------------

pub(in crate::render) const STATUS_BG: Color = rgb(0x0b1117);
pub(in crate::render) const PANE_BORDER: Color = rgb(0x1d2a36);
pub(in crate::render) const BAR_TROUGH: Color = rgb(0x1b2733);
pub(in crate::render) const DIVIDER: Color = rgb(0x141e26);
pub(in crate::render) const KEYBAR_DIVIDER: Color = rgb(0x243040);
pub(in crate::render) const ALERT_ROW_BG: Color = rgb(0x141410);
pub(in crate::render) const KEYCAP_BG: Color = rgb(0x20241a);
pub(in crate::render) const MAP_FLOOR: Color = rgb(0x1c2c3a);
/// A field's label, as against [`SECONDARY`] for its value.
pub(in crate::render) const FIELD_LABEL: Color = rgb(0x5c6773);
pub(in crate::render) const SECONDARY: Color = rgb(0x8b97a5);
pub(in crate::render) const FAINT: Color = rgb(0x4a5563);

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(c: Color) -> (u8, u8, u8) {
        (
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8,
        )
    }

    /// The one thing `rgb` can get wrong: a shifted or masked channel. Every
    /// other value in this file is a hex literal copied from the handoff, so
    /// a census asserting `rgb(0x3fa9b5)` equals `#3fa9b5` would be testing
    /// the same digits twice and proving nothing.
    #[test]
    fn rgb_round_trips_every_channel() {
        for (hex, want) in [
            (0x000000, (0, 0, 0)),
            (0xffffff, (255, 255, 255)),
            (0xff0000, (255, 0, 0)),
            (0x00ff00, (0, 255, 0)),
            (0x0000ff, (0, 0, 255)),
            (0x123456, (0x12, 0x34, 0x56)),
        ] {
            assert_eq!(bytes(rgb(hex)), want, "rgb({hex:#08x})");
        }
    }

    /// The reservations only mean something if the eye can tell them apart.
    /// Guards against a later tidy-up collapsing two roles onto one value,
    /// which would read as the HUD having stopped distinguishing "act" from
    /// "harm" rather than as a colour bug.
    #[test]
    fn the_reserved_roles_are_separable() {
        let roles = [
            ("ATTENTION", ATTENTION),
            ("THREAT", THREAT),
            ("HEALTHY", HEALTHY),
            ("PANE_TITLE", PANE_TITLE),
            ("BODY", BODY),
            ("LABEL", LABEL),
        ];
        for (i, (an, a)) in roles.iter().enumerate() {
            for (bn, b) in &roles[i + 1..] {
                let d = (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
                assert!(d > 0.25, "{an} and {bn} are too close to separate ({d:.3})");
            }
        }
    }

    /// A role at anything but full alpha draws washed into whatever is
    /// behind it, which on the map is the floor and on a pane is its fill —
    /// so the same role would read as two different colours depending on
    /// where it landed.
    #[test]
    fn every_content_role_is_opaque() {
        for (name, c) in [
            ("ATTENTION", ATTENTION),
            ("THREAT", THREAT),
            ("HEALTHY", HEALTHY),
            ("PANE_TITLE", PANE_TITLE),
            ("PLAYER", PLAYER),
            ("EMPHASIS", EMPHASIS),
            ("BODY", BODY),
            ("LABEL", LABEL),
            ("CH_FIELD", CH_FIELD),
            ("CH_GAIN", CH_GAIN),
            ("CH_BASE", CH_BASE),
            ("CH_DEFEND", CH_DEFEND),
            ("CH_IDLE", CH_IDLE),
        ] {
            assert_eq!(c.a, 1.0, "{name} is not opaque");
        }
    }
}
