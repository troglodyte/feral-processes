use super::*;

/// One addressable enemy group on the battle roster — "3 Glitches" as a
/// single row. Only the front member's HP is shown, because it is the only
/// one that can be hit (see `battle::EnemyGroup`).
#[derive(Clone)]
pub struct EnemyGroupView {
    /// Stable for the fight, unlike `letter`: see `battle::GroupId`.
    pub id: crate::battle::GroupId,
    /// Display letter, 'A'.. — how the player addresses this group.
    pub letter: char,
    pub species_name: String,
    pub count: usize,
    pub front_hp: i32,
    pub front_max_hp: i32,
    /// The rare-spawn tier of the *front* member — the same one whose HP the
    /// two fields above report, since a group's members can differ and the
    /// row has one of each to show. Deliberately absent from `species_name`,
    /// which the roster draws into a fixed-width cell an "Overclocked
    /// Scrapper 2" overflows; the renderer tags it outside that column.
    pub front_rarity: Rarity,
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    pub is_boss: bool,
    /// Whether this group is in melee range. A group that isn't can only
    /// act through a ranged move, and renderers dim it to make that legible
    /// rather than leaving the player to infer it from the log.
    pub engaged: bool,
    pub statuses: Vec<StatusTagView>,
    /// Estimated chance (0.0-1.0) a decompile attempt against *this* group's
    /// front member would succeed, given its current HP fraction, its
    /// species' difficulty, and the potency of the catalyst the attempt
    /// would spend (see `Game::taming_catalyst`). `None` when the player
    /// holds no catalyst: there's no potency to quote odds for, and the
    /// action isn't available at all.
    pub decompile_chance: Option<f32>,
    /// Set when some member of the group is mid-charge: the furthest along.
    pub charge: Option<ChargeTag>,
}

/// A charge in progress as a roster row shows it: `charging k/N -> target`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChargeTag {
    /// Turns spent charging so far.
    pub k: u32,
    /// Turns that reach full power.
    pub n: u32,
    /// Where it is aimed: a group letter, a body's label, or "the party".
    pub target: String,
}

/// One row of the player's side of the roster.
#[derive(Clone)]
pub struct PartySlotView {
    /// Index into `BattleState::planned` — 0 is the player.
    pub slot: usize,
    pub entity: Entity,
    /// `Game::creature_short_label` — no tier, no species. The battle
    /// roster's `NAME_W` cell (`gui/src/render/battle.rs`) has no room for
    /// either; `rarity` below is what lets the renderer say the tier some
    /// other way, a trailing tag, following `rarity_tag`'s pattern for the
    /// hostile roster.
    pub name: String,
    /// This slot's rare-spawn tier — see `components::Rarity`. Carried
    /// separately from `name` for the reason `views::PetInfo::rarity` is:
    /// the tier reads back without parsing it out of a string.
    pub rarity: Rarity,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    pub statuses: Vec<StatusTagView>,
    /// What this member has left to spend on routines, or `None` for one
    /// holding no reserve at all. Every roster member carries one
    /// (`Game::roster_parts`), and it is what that member's own Specials and
    /// field routines are charged against — so this is `Some` for the whole
    /// party, and `None` only for a body that was never taken onto it.
    pub power: Option<f32>,
    /// The ceiling `power` is read against - see `Game::max_power`.
    pub max_power: f32,
    /// This round's chosen action rendered for the roster, or `None` if the
    /// slot is still awaiting one.
    pub planned: Option<String>,
    /// Whether this slot is in the front line, which draws more enemy fire
    /// — see `FRONT_SLOTS`. Soft ranks: a back slot is still targetable.
    pub front: bool,
    /// Which of this member's three equipment slots are filled, as the same
    /// fixed-width `w|a|m` cell `Game::gear_tag` builds for the roster and
    /// the status panel. Pre-formatted for the reason `EntityView::gear` is
    /// — a loadout must read one way wherever it is shown, and a fight is
    /// exactly where a member found to be wearing nothing matters most.
    pub gear: String,
    /// Set while this member is mid-charge.
    pub charge: Option<ChargeTag>,
}

pub struct BattleView {
    pub groups: Vec<EnemyGroupView>,
    pub party: Vec<PartySlotView>,
    /// The slot currently choosing an action, or `None` once the round is
    /// fully planned and only needs resolving.
    pub active_slot: Option<usize>,
    /// The action menu for `active_slot`. Renderers draw these verbatim and
    /// never author an action string of their own — see
    /// `Game::battle_action_options`.
    pub options: Vec<ActionOption>,
    pub round: u32,
    pub player_decompiler: i32,
    /// The picture window: whoever the narrated line belongs to, else the
    /// first in line. See `Game::battle_portrait`.
    pub portrait: Option<PortraitView>,
}
