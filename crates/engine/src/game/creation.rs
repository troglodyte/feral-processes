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
use crate::items::ItemId;
use crate::items_db::ItemDb;
use crate::species::AffinityClass;
use crate::*;

/// The sprite name every player carried before the wizard could choose
/// one, and the name `assets/sprites/player.png` is loaded under. Named
/// here rather than spelled out at each of its three sites — the default
/// choice, `PlayerSave::sprite`'s serde default, and the wizard's first
/// icon — because the three disagreeing is exactly how the player's
/// shipped art went missing once already.
pub const DEFAULT_PLAYER_SPRITE: &str = "player";

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
    /// The starting kit picked off `items_db::creation_shelf`, priced
    /// against `tuning::CREATION_CREDITS`.
    ///
    /// **Non-empty replaces the class kit; empty falls back to it.** That
    /// is the whole of the rule, and it is what keeps two properties
    /// alive at once: `CharacterChoice::default()` still produces today's
    /// player across the ~1,600 `Game::new` call sites, and an empty
    /// `assets/classes/` is still the pre-class game. A sentinel
    /// (`Option<Vec<_>>`) was the alternative and is worse — walking the
    /// step without spending would then be a deliberate `Some(vec![])`
    /// and start the run naked.
    pub items: Vec<(ItemId, u32)>,
}

/// Today's player exactly — no class, the `@` glyph wearing its
/// `DEFAULT_PLAYER_SPRITE`, no name, no starter routine, every point
/// unspent. This is what every existing test and every one of the ~1,600
/// `Game::new` call sites gets, via `Game::new`'s delegation to
/// `Game::new_with(.., &CharacterChoice::default())`.
impl Default for CharacterChoice {
    fn default() -> Self {
        Self {
            name: String::new(),
            class: None,
            glyph: '@',
            sprite: DEFAULT_PLAYER_SPRITE.to_string(),
            colour: None,
            stats: [0; 4],
            routine: None,
            items: Vec::new(),
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
        self.apply_creation_kit(choice);
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

    /// The kit slot: `choice.items` if the player picked one, the class kit
    /// otherwise. See `CharacterChoice::items` for why an empty basket is
    /// the fallback rather than a naked run.
    ///
    /// Fails closed, `apply_creation_stats`' rule: a basket that overspends
    /// the allowance, or names anything `items_db::creation_shelf` does not
    /// offer, applies **no** items at all and takes the class kit instead —
    /// never a clamped or partial basket, and never a way for a
    /// hand-built `CharacterChoice` around what the shelf is allowed to
    /// hold.
    ///
    /// What the basket leaves unspent arrives as Credits, and only on this
    /// branch — crediting the fallback would hand today's kitted player an
    /// allowance they never chose.
    fn apply_creation_kit(&mut self, choice: &CharacterChoice) {
        let Some(spent) = self.creation_basket_cost(&choice.items) else {
            crate::classes::apply_kit(self, choice.class);
            return;
        };
        let credits = self.world.resource::<ItemDb>().trade_currency().cloned();
        let player = self.player_entity();
        let mut inventory = self.world.get_mut::<Inventory>(player).unwrap();
        for (item, qty) in &choice.items {
            inventory.add(item.clone(), *qty);
        }
        if let Some(credits) = credits {
            inventory.add(credits, crate::tuning::CREATION_CREDITS - spent);
        }
    }

    /// What `items` costs out of `tuning::CREATION_CREDITS`, or `None` if
    /// it is not a basket the kit step could have produced — an item the
    /// shelf does not offer, or a total over the allowance. `None` for an
    /// empty basket too, which is what routes it to the class kit.
    fn creation_basket_cost(&self, items: &[(ItemId, u32)]) -> Option<u32> {
        if items.is_empty() {
            return None;
        }
        let shelf = self.creation_shelf_rows();
        let total = items.iter().try_fold(0u32, |sum, (item, qty)| {
            let row = shelf.iter().find(|r| &r.id == item)?;
            sum.checked_add(row.price.checked_mul(*qty)?)
        })?;
        (total <= crate::tuning::CREATION_CREDITS).then_some(total)
    }
}

/// The three databases the creation wizard reads, loaded on their own.
///
/// The wizard runs **before any `Game` exists** — the difficulty it picks is
/// a `Game::new_with` argument — so it cannot ask a `World` for its rows.
/// This is `App`'s `help_db`/`achievement_db` precedent one directory
/// wider: a screen reachable with no run in progress owns the catalogue it
/// draws from.
///
/// Nothing here derives anything itself. `class_rows` and `starter_rows`
/// are calls to `classes::class_rows` and `abilities::starter_rows`, the
/// same two functions `Game::class_rows` and `Game::starter_routine_rows`
/// call — a preview that disagreed with what the run actually granted would
/// be worse than no preview.
///
/// `Default` is the empty catalogue — no classes, no items, no abilities —
/// which is what a frontend gets if the asset tree will not load at all.
/// The wizard then offers no class and no starter routine, which is the
/// pre-creation game, rather than refusing to open.
#[derive(Default)]
pub struct CreationCatalogue {
    classes: crate::classes::ClassDb,
    items: crate::items_db::ItemDb,
    abilities: crate::abilities::AbilityDb,
}

impl CreationCatalogue {
    /// Loads the three directories the wizard needs from `assets_dir`.
    /// Warnings are dropped rather than surfaced: `Game::new_with` loads the
    /// same files a moment later and replays every one of them into the log,
    /// so reporting here would double each line.
    ///
    /// Etched disks are deliberately not synthesised — `ItemDb` is read here
    /// for kit *names* only, and every kit names an authored item.
    pub fn load(assets_dir: &std::path::Path) -> std::io::Result<Self> {
        let (abilities, _) = crate::abilities::AbilityDb::load_dir(&assets_dir.join("abilities"))?;
        let (items, _) = crate::items_db::ItemDb::load_dir(&assets_dir.join("items"), &abilities)?;
        // Absent-is-silent, `ClassDb`'s own contract: an empty catalogue
        // leaves the class step with no rows, which is the pre-class game.
        let (classes, _) = crate::classes::ClassDb::load_dir(&assets_dir.join("classes"))?;
        Ok(Self {
            classes,
            items,
            abilities,
        })
    }

    /// One row per loaded class — `Game::class_rows`' own derivation.
    pub fn class_rows(&self) -> Vec<views::ClassRow> {
        crate::classes::class_rows(&self.classes, &self.items)
    }

    /// The kit shelf — `ItemDb::creation_shelf` called, the same
    /// derivation `Game::creation_shelf_rows` calls, so the wizard cannot
    /// offer a row the run would then refuse.
    pub fn shelf_rows(&self) -> Vec<crate::views::StartingItemRow> {
        self.items.creation_shelf()
    }

    /// The starter pool, priced through `class`'s spread —
    /// `Game::starter_routine_rows`' own derivation, with no perk term
    /// because a player being created has no unlocked perks yet.
    pub fn starter_rows(
        &self,
        class: Option<AffinityClass>,
    ) -> Vec<crate::views::StarterRoutineRow> {
        crate::abilities::starter_rows(&self.abilities, |kind| {
            crate::classes::affinity_with_perk(
                crate::classes::class_affinity(&self.classes, class, kind),
                None,
                kind,
            )
        })
    }
}
