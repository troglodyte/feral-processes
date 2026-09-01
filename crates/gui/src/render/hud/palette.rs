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
//! **This is the handoff's table minus what the game does not draw.** It was
//! transcribed whole and carried a `dead_code` allow while the phases
//! consumed it; with the last of them landed, the four entries still
//! suppressed — an idle log channel, an alert row's background tint, a
//! keycap chip's, and a second body grey — were dropped rather than left
//! standing as colour nothing paints. The handoff still documents them if
//! anything ever wants them.
//!
//! Scope is the HUD and the map's entity glyphs. The popup screens keep
//! `render/mod.rs`'s colours; they draw *over* the HUD, so the seam between
//! the two palettes is not visible in play.

use crate::paint::Color;
use feral_processes_engine::components::GlyphColor;

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
pub(crate) const ATTENTION: Color = rgb(0xe3b341);
/// br red — **hostility and inbound harm**. Reserved; never an ordinary error.
pub(crate) const THREAT: Color = rgb(0xf26d6d);
/// green — a healthy bar fill, and the calm `ALL NOMINAL` state.
pub(crate) const HEALTHY: Color = rgb(0x4fa65b);
/// yellow — a caution that resolves itself. A machine short of input, a
/// worker walking to its post: the dimmer of the two yellows, because
/// waiting fixes it and [`ATTENTION`] is what the player must get up for.
pub(crate) const WARN: Color = rgb(0xb8943f);
/// br blue — **the player's own plan drawn on the ground**: a marked dig box,
/// the box being previewed under the cursor, the ring the party's tile wears
/// while cutting tools are armed.
///
/// Deliberately not a yellow, which is what these were. A plan is the player
/// having *acted*; [`ATTENTION`] is the base asking them to, and the plan's
/// washes sat 0.11 from it — close enough that a cell the player had marked
/// and a machine that had stalled read as the same news.
pub(crate) const PLAN: Color = rgb(0x4a7fd0);
/// br cyan — pane titles on their borders.
pub(crate) const PANE_TITLE: Color = rgb(0x56d4dd);
/// br cyan — the player's `@`, and an upgradeable item.
pub(crate) const PLAYER: Color = rgb(0x56d4dd);
/// The six colours the character-creation wizard offers for the player's
/// glyph. Kept apart from [`PLAYER`] rather than folded into it:
/// `PlayerIdentity::colour` is an `Option<u8>` indexing this array
/// **0-based**, and its `None` — what `CharacterChoice::default()` and
/// every save from before this feature carries — falls back to [`PLAYER`].
/// An out-of-range index falls back the same way. The `Option` is what
/// keeps "no choice was made" from sharing a value with this array's own
/// first entry, which a reserved zero would have done.
///
/// Warm hues are all spoken for by [`glyph`]'s table or by a reserved role
/// (red, orange, yellow, brown), so these sit in the green-through-magenta
/// range that leaves open. `every_content_hue_is_separable_from_the_others`
/// is what holds each 0.25 from every content hue, from `PLAYER`, and from
/// each other — authored by running that test, not by guessing.
pub(crate) const PLAYER_CHOICES: [Color; 6] = [
    rgb(0xbdcc70), // moss
    rgb(0x6cd936), // lime
    rgb(0x39e69e), // teal
    rgb(0x2d38b2), // indigo
    rgb(0x8a39e6), // violet
    rgb(0xa62972), // rose
];
/// br white — a keycap letter, or a value being emphasised. Only those.
pub(crate) const EMPHASIS: Color = rgb(0xe8eef4);
/// white — body text and table rows.
pub(crate) const BODY: Color = rgb(0xa8b3bf);
/// br black — dim labels, sub-heads, inert hints.
pub(crate) const LABEL: Color = rgb(0x3a4550);

// ---------------------------------------------------------------------------
// Log channels
// ---------------------------------------------------------------------------

pub(crate) const CH_FIELD: Color = rgb(0x3fa9b5);
pub(crate) const CH_GAIN: Color = rgb(0x4a7fd0);
pub(crate) const CH_BASE: Color = rgb(0x7ee787);
pub(crate) const CH_DEFEND: Color = rgb(0xf26d6d);

// ---------------------------------------------------------------------------
// Chrome — fills, rules and troughs. Outside the 16 content entries.
// ---------------------------------------------------------------------------

pub(crate) const STATUS_BG: Color = rgb(0x0b1117);
pub(crate) const PANE_BORDER: Color = rgb(0x1d2a36);
pub(crate) const BAR_TROUGH: Color = rgb(0x1b2733);
pub(crate) const DIVIDER: Color = rgb(0x141e26);
pub(crate) const KEYBAR_DIVIDER: Color = rgb(0x243040);
/// A field's label, as against `BODY` for its value.
pub(crate) const FIELD_LABEL: Color = rgb(0x5c6773);
pub(crate) const FAINT: Color = rgb(0x4a5563);

// ---------------------------------------------------------------------------
// The map's content hues
// ---------------------------------------------------------------------------

/// What a `GlyphColor` is drawn in — the map's entity glyphs, and the same
/// program's glyph wherever a screen shows it.
///
/// Addressed by the authored hue rather than by a colour, which is what keeps
/// the two reservations above intact: [`ATTENTION`] appears nowhere in this
/// table, since a hostile that would beat you is not a thing you must get up
/// and do, and [`THREAT`] is reached only by the rung `difficulty_color`
/// paints a creature that would.
///
/// `components::GlyphColor` is content's vocabulary — every species and
/// structure file authors one of these eleven names — so this is a hue table
/// and not a role table, and it is exhaustive for `cell_mark`'s reason: a
/// twelfth hue must not compile until someone has said what it looks like.
///
/// Nine of the eleven are entries in the handoff's sixteen. **Brown and
/// orange are not in it**, and stand outside it the way the chrome greys do:
/// collapsing either onto its nearest entry would make two authored hues
/// indistinguishable, and a Mining Node that reads as a Lathe — or a con
/// ladder whose top two rungs read alike — is worse than a hue outside the
/// table. Both are drawn at the table's saturation, which is what keeps them
/// from reading as louder than the sixteen they sit among.
pub(crate) const fn glyph(c: GlyphColor) -> Color {
    match c {
        // br white and white: the two neutral glyph entries, in the order
        // the handoff assigns them — `H` and `&` bright, `R`/`T`/`o` body.
        GlyphColor::White => EMPHASIS,
        GlyphColor::Gray => BODY,
        // The two greens, brighter first: br green is the handoff's own pet
        // glyph, and green the bar fill a dark-green thing sits nearest.
        GlyphColor::Green => rgb(0x7ee787),
        GlyphColor::DarkGreen => HEALTHY,
        GlyphColor::Red => THREAT,
        GlyphColor::Yellow => WARN,
        // `difficulty_color`'s third rung, between a caution and the thing
        // that will kill you, and it has to stay clear of both. The table's
        // other red — entry 1, its damage-number red — is far enough from br
        // red by channel distance and is still the wrong colour here: the
        // two are the same *hue* and differ mostly in lightness, which the
        // map's vignette is free to eat, so the top two rungs of the ladder
        // would read as one. This one climbs in hue, and
        // `the_con_ladder_gets_hotter_at_every_rung` is what says by how
        // much.
        GlyphColor::Orange => rgb(0xe07a3c),
        // The loud halves of blue and magenta: both are `difficulty_color`
        // spending a creature's whole colour on one fact — a nemesis, a boss
        // — so neither may read as ordinary ground.
        GlyphColor::Blue => rgb(0x4a7fd0),
        GlyphColor::Magenta => rgb(0xcf8ee0),
        GlyphColor::Cyan => rgb(0x3fa9b5),
        GlyphColor::Brown => rgb(0x8a6a3a),
    }
}

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

    fn dist(a: Color, b: Color) -> f32 {
        (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs()
    }

    /// **The reservation, asserted where content could take it.** Every hue
    /// in [`glyph`] is authored in an asset file, by anyone, for anything —
    /// so if br yellow were reachable from that table, a modded structure
    /// could wear the colour the status bar uses for "you must act" and the
    /// badge would stop meaning anything.
    ///
    /// br red is reachable, from exactly one hue: the rung
    /// `difficulty_color` paints a creature that would beat you, which is
    /// hostility and is what the colour is for.
    #[test]
    fn no_content_hue_takes_a_reserved_role_by_accident() {
        for c in GlyphColor::ALL {
            assert_ne!(
                glyph(c),
                ATTENTION,
                "{c:?} wears ATTENTION, which is the HUD's alone"
            );
            if glyph(c) == THREAT {
                assert_eq!(
                    c,
                    GlyphColor::Red,
                    "{c:?} wears THREAT, which only the deadly rung may"
                );
            }
        }
    }

    /// Eleven hues authored by content, drawn one glyph to a tile over a
    /// tinted biome and dimmed by a vignette — two that read alike are two
    /// things the player cannot tell apart on the map.
    ///
    /// The separation and not the literals, `the_tier_colours_are_separable
    /// _from_their_neighbours`' rule, so a retune is free to move any of
    /// them. `PLAYER` is in the walk because the map draws it as a glyph
    /// beside the rest, and `PLAYER_CHOICES` for the same reason — the
    /// wizard's colours are glyphs on the same map and this is the only
    /// place their separability from content, from `PLAYER`, and from each
    /// other is enforced.
    #[test]
    fn every_content_hue_is_separable_from_the_others() {
        let mut hues: Vec<(String, Color)> = GlyphColor::ALL
            .into_iter()
            .map(|c| (format!("{c:?}"), glyph(c)))
            .collect();
        hues.push(("PLAYER".to_string(), PLAYER));
        for (i, colour) in PLAYER_CHOICES.into_iter().enumerate() {
            hues.push((format!("PLAYER_CHOICES[{i}]"), colour));
        }
        for (i, (name, colour)) in hues.iter().enumerate() {
            for (other_name, other) in hues.iter().skip(i + 1) {
                assert!(
                    dist(*colour, *other) > 0.25,
                    "{name} is only {:.2} from {other_name}",
                    dist(*colour, *other)
                );
            }
        }
    }

    /// `difficulty_color`'s four rungs are one read — "can I win this fight"
    /// — and a ladder has to climb. Warmth is the axis it climbs on, so each
    /// rung carries more red over green than the one below it, **by a
    /// margin**: the bare inequality passes against two reds that differ
    /// only in lightness, which is what the map's vignette is free to eat.
    /// That is the assertion `every_content_hue_is_separable_from_the_others`
    /// cannot make — channel distance cannot see that two colours are the
    /// same hue.
    ///
    /// Named rather than walked, because the order *is* the assertion: the
    /// engine's ladder runs easy → even → tough → deadly, and this says the
    /// colours agree with it.
    #[test]
    fn the_con_ladder_gets_hotter_at_every_rung() {
        /// The smallest step the shipped ladder takes is 0.12, and entry 1's
        /// red as the third rung would take one of 0.06.
        const MARGIN: f32 = 0.10;
        let warmth = |c: GlyphColor| {
            let c = glyph(c);
            c.r - c.g
        };
        let rungs = [
            GlyphColor::Green,
            GlyphColor::Yellow,
            GlyphColor::Orange,
            GlyphColor::Red,
        ];
        for pair in rungs.windows(2) {
            assert!(
                warmth(pair[1]) - warmth(pair[0]) > MARGIN,
                "{:?} ({:.2}) is not clearly hotter than {:?} ({:.2})",
                pair[1],
                warmth(pair[1]),
                pair[0],
                warmth(pair[0])
            );
        }
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
            ("WARN", WARN),
        ]
        .into_iter()
        .chain(
            GlyphColor::ALL
                .into_iter()
                .map(|c| ("a content hue", glyph(c))),
        ) {
            assert_eq!(c.a, 1.0, "{name} is not opaque");
        }
    }
}
