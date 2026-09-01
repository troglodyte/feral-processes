//! Who the player is before the run starts.
//!
//! `CharacterChoice` is the whole of it, `cost()` is the pricing that turns
//! a spend into a valid one, and `Game::apply_character_choice` is what
//! layers a validated choice onto the just-spawned player — stats, identity,
//! kit, routine, in that order.
//!
//! Kit and routine are one-line delegations to `classes::apply_kit` and
//! `abilities::install_starter`. Those two own their own bodies (Phase 2A
//! and 2B of the character-creation feature); what makes them load-bearing
//! *here* is that `CharacterChoice::default()` — no class, no routine — has
//! to keep producing today's player, which is what roughly 1,600 existing
//! `Game::new` call sites construct.

use crate::abilities::AbilityId;
use crate::achievements::MainStat;
use crate::species::AffinityClass;
use crate::*;

/// Everything a run starts as, decided once at creation and never
/// rerolled.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterChoice {
    pub name: String,
    pub class: Option<AffinityClass>,
    pub glyph: char,
    pub sprite: String,
    /// Which player swatch the glyph wears, **0-based**; `None` is the
    /// renderer's `PLAYER` role colour. See
    /// `components::PlayerIdentity::colour` for why this is an `Option`
    /// rather than a reserved zero.
    pub colour: Option<u8>,
    /// Units *bought* per axis, indexed as `MainStat::all()` — not points
    /// spent. `cost()` is what prices a unit, at that axis's own
    /// `tuning::CREATION_COST_*` rate; pricing at conversion time instead
    /// (storing the spend and dividing it back out per axis on read) would
    /// let 4 points on Def buy the same +1 mitigation as 3, silently
    /// eating a point the player chose to spend.
    pub stats: [u32; 4],
    pub routine: Option<AbilityId>,
}

/// Today's player exactly — no class, the `@` glyph, no name, no starter
/// routine, every point unspent. This is what every existing test and
/// every one of the ~1,600 `Game::new` call sites gets, via `Game::new`'s
/// delegation to `Game::new_with(.., &CharacterChoice::default())`.
impl Default for CharacterChoice {
    fn default() -> Self {
        Self {
            name: String::new(),
            class: None,
            glyph: '@',
            sprite: String::new(),
            colour: None,
            stats: [0; 4],
            routine: None,
        }
    }
}

impl CharacterChoice {
    /// Pool points this spend costs, priced per axis through
    /// `crate::tuning::CREATION_COST_*` — `stats[i]` is how many points of axis
    /// `MainStat::all()[i]` are bought, each at that axis's own rate. `None`
    /// above `crate::tuning::CREATION_STAT_POINTS`; `Game::apply_character_choice`
    /// fails closed on that, applying no spend at all rather than a
    /// clamped one.
    pub fn cost(&self) -> Option<u32> {
        // Order matches `MainStat::all()`: Atk, Def, Integrity, Decompiler.
        let costs = [
            crate::tuning::CREATION_COST_ATK,
            crate::tuning::CREATION_COST_DEF,
            crate::tuning::CREATION_COST_INTEGRITY,
            crate::tuning::CREATION_COST_DECOMPILER,
        ];
        let total = self
            .stats
            .iter()
            .zip(costs)
            .try_fold(0u32, |sum, (&points, cost)| {
                sum.checked_add(points.checked_mul(cost)?)
            })?;
        (total <= crate::tuning::CREATION_STAT_POINTS).then_some(total)
    }
}

impl Game {
    /// Layers `choice` onto the just-spawned player, in this order: stats,
    /// identity, kit, routine. Stats and identity are this module's own
    /// logic; kit and routine are one-line delegations — see the module doc
    /// comment.
    pub(crate) fn apply_character_choice(&mut self, choice: &CharacterChoice) {
        self.apply_creation_stats(choice);
        self.apply_creation_identity(choice);
        crate::classes::apply_kit(self, choice.class);
        crate::abilities::install_starter(self, choice.routine.as_ref());
    }

    /// Adds `choice`'s spend on top of `PLAYER_BASE_STATS`, never
    /// redistributing it — every build is therefore at or above the floor
    /// `balance_sim` models. Fails closed: `cost()` is the one gate, checked
    /// once here, and an overspent choice gets no spend at all rather than
    /// a clamped or partial one.
    fn apply_creation_stats(&mut self, choice: &CharacterChoice) {
        if choice.cost().is_none() {
            return;
        }
        let player = self.player_entity();
        for (axis, &points) in MainStat::all().iter().zip(choice.stats.iter()) {
            let points = points as i32;
            match axis {
                MainStat::Atk => self.world.get_mut::<Stats>(player).unwrap().atk += points,
                MainStat::Def => self.world.get_mut::<Stats>(player).unwrap().mitigation += points,
                MainStat::Integrity => {
                    let gain = points * crate::tuning::CREATION_GAIN_INTEGRITY as i32;
                    let mut stats = self.world.get_mut::<Stats>(player).unwrap();
                    stats.max_hp += gain;
                    // Both halves, or the run starts damaged — see
                    // `MainStat::Integrity`'s own doc comment.
                    stats.hp += gain;
                }
                MainStat::Decompiler => {
                    self.world.get_mut::<Decompiler>(player).unwrap().skill += points
                }
            }
        }
    }

    /// The player's chosen glyph, class, sprite, colour and name.
    /// `choice.glyph` writes the existing `Glyph.ch`; `class`/`sprite`/
    /// `colour` overwrite the `PlayerIdentity` `spawn_player` seeded at its
    /// neutral `Default` — `GlyphColor` is the eleven-hue *content* palette
    /// and the player's own choices are deliberately outside it, so the
    /// colour rides `PlayerIdentity` instead of `Glyph.color`. The name
    /// goes through `CustomName::sanitize` like every other writer, so a
    /// blank `choice.name` — `CharacterChoice::default()`'s own value —
    /// inserts no override, exactly as today's nameless player has none.
    fn apply_creation_identity(&mut self, choice: &CharacterChoice) {
        let player = self.player_entity();
        self.world.get_mut::<Glyph>(player).unwrap().ch = choice.glyph;
        *self.world.get_mut::<PlayerIdentity>(player).unwrap() = PlayerIdentity {
            class: choice.class,
            sprite: choice.sprite.clone(),
            colour: choice.colour,
        };
        if let Some(name) = CustomName::sanitize(Some(choice.name.clone())) {
            self.world.entity_mut(player).insert(CustomName(name));
        }
    }
}
