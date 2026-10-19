use super::*;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlyphColor {
    White,
    Gray,
    Green,
    DarkGreen,
    Red,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Brown,
    Orange,
}

impl GlyphColor {
    /// Every authored hue, for a census that must cover all of them —
    /// `Rarity::ALL`'s reason. A renderer's colour table is an exhaustive
    /// match and so cannot miss a new hue; a test naming the hues by hand
    /// can, and silently stops covering the one just added.
    pub const ALL: [GlyphColor; 11] = [
        GlyphColor::White,
        GlyphColor::Gray,
        GlyphColor::Green,
        GlyphColor::DarkGreen,
        GlyphColor::Red,
        GlyphColor::Yellow,
        GlyphColor::Blue,
        GlyphColor::Magenta,
        GlyphColor::Cyan,
        GlyphColor::Brown,
        GlyphColor::Orange,
    ];
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Glyph {
    pub ch: char,
    pub color: GlyphColor,
}

/// Marks the single player-controlled entity.
#[derive(Component)]
pub struct Player;

#[derive(Component, Clone)]
pub struct Creature {
    pub species: SpeciesId,
}

/// A display name that overrides a creature's species name wherever it's
/// shown. Two authors, not one: the player, through `Game::fuse_companions`
/// and `Game::rename_companion`, and — since the nemesis feature —
/// `Game::mark_nemeses`, which writes a bank-derived name (`nemesis::
/// NemesisDb`) to a hostile on its first grudge only, never again
/// (`components::Nemesis`). A nemesis you later decompile joins the roster
/// still wearing the name it earned, and `rename_companion` can overwrite
/// it afterward like any other name, because at that point it is yours.
/// The constructor enforces the length; the tuple field does not, so build
/// one through `sanitize` rather than wrapping a raw string — every writer,
/// bank-derived names included, goes through it.
#[derive(Component, Clone, Debug)]
pub struct CustomName(pub String);

impl CustomName {
    /// What the player typed, reduced to what may actually be stored:
    /// trimmed, truncated to `MAX_CUSTOM_NAME_LEN`, and `None` if nothing
    /// is left.
    ///
    /// **`None` means "no override", and both callers want that** — it is
    /// why this returns an `Option` rather than refusing empty input.
    /// Fusion reads it as "insert no `CustomName`" and a rename reads it as
    /// "remove the one that's there", and those land on the same place: the
    /// species name. So blank is the way back to the default, in both.
    pub fn sanitize(input: Option<String>) -> Option<String> {
        let trimmed = input?
            .trim()
            .chars()
            .take(MAX_CUSTOM_NAME_LEN)
            .collect::<String>();
        (!trimmed.is_empty()).then_some(trimmed)
    }
}

/// The player's chosen class and look, from character-creation's
/// `game::creation::CharacterChoice` — the player-only counterpart to
/// `Creature::species`. `class` feeds the player arm of
/// `Game::ability_affinity`; `sprite` and `colour` do not live on `Glyph`
/// because `GlyphColor` is the eleven-hue *content* palette and the
/// player's own choices are deliberately outside it. They instead ride out
/// to the renderer on `views::PlayerLook`, carried on `EntityView::look`.
///
/// Spawned at its `Default` (no class, no sprite, no colour, no icon) by
/// every constructor and immediately overwritten by
/// `Game::apply_character_choice` — the same "neutral bundle, layered on
/// top" shape `Game::new_with`'s player spawn uses throughout.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct PlayerIdentity {
    pub class: Option<PlayerClass>,
    pub sprite: String,
    /// Which of the renderer's player swatches the glyph wears, **0-based**
    /// — `None` is the `PLAYER` role colour, which is what a player who
    /// never opened the wizard and every save from before it carries.
    /// `Option` rather than a reserved zero for `PowerCell::Unrated`'s
    /// reason: *no answer* is not a bad answer, and a sentinel index shares
    /// one value between "the default" and "the first swatch".
    pub colour: Option<u8>,
    /// The player's hand-drawn 16x16 map avatar — `None` for a glyph, the
    /// same "no answer" reading `colour` already gives, and what every save
    /// and every character before this feature carries.
    pub icon: Option<PlayerIcon>,
}

/// Which zone portal's sector a creature was spawned in — set once at
/// spawn time and never changed afterward, even if the creature is later
/// tamed and carried through a portal into a deeper zone. Drives its stat
/// scale (see `ZoneLevel::stat_multiplier`) and is appended to its display
/// label (e.g. "Scrapper 2") so a deeper-zone catch reads differently from
/// a shallow one.
#[derive(Component, Clone, Copy, Debug)]
pub struct ZonePortal(pub u32);

/// The stable identity of one program the player owns, minted at
/// `Game::roster_parts` — the single barrier all four doors into the roster
/// pass through.
///
/// `Entity` cannot do this job: `save.rs` resolves everything by position or
/// by index precisely because entity ids are not stable across a save round
/// trip. A memory about one specific program needs a name that is, and this
/// is it.
///
/// Only an owned program carries one. Nothing wild or hostile reaches
/// `roster_parts`, so the absence of this component is what "not on the
/// roster" looks like, the same way an absent `Rarity` reads as `Ordinary`.
/// `0` is the unassigned sentinel a legacy save loads with; real ids start
/// at 1 and `Game::load` mints one for every program carrying the sentinel.
///
/// `Ord` so `Game::owned_pets` can break a tie on it: id order is
/// assignment order, which means "the one you caught first, sorts first."
/// The derived handle (`handles::of`) counts up in that same order, but a
/// custom name replaces it, so the id is the key that always holds.
#[derive(
    Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct ProgramId(pub u32);
