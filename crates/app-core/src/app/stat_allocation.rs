//! The Points screen, shared: creation spends its pool here and
//! `Mode::AllocateStats` spends the banked level-up points here.
//!
//! **One screen, not two.** `StatAllocation` owns what does not change
//! while the player spends - the pool, which attributes are offered, the
//! opening values and the catalogue - and its methods take the `spent` map,
//! which lives where the caller keeps it: `CharacterChoice::stats` for
//! creation, `App::allocation_spent` for a level-up. Neither caller
//! re-implements the ceiling rule or the preview.
//!
//! **The preview is `progression::derive` called**, once on the opening
//! attributes and once on those plus the spend, from the owner's own
//! `DerivedBase`, each passed through the owner's `StatBonus`
//! (`Game::stat_bonus`) - the call `recompute_derived` itself makes, so held
//! Phase Keys scale the after figure too and a before->after figure is that
//! owner's base stat now and after. It is not the HUD's effective figure: low-Power, program, buff
//! and emulation adjustments and the mitigation cap apply on top, outside a
//! spend.

use std::collections::BTreeMap;

use feral_processes_engine::StatOwner;
use feral_processes_engine::attributes::{AttributeDb, AttributeDef, AttributeId, DerivedStat};
use feral_processes_engine::components::Attributes;
use feral_processes_engine::progression::{DerivedBase, DerivedStats, StatBonus, derive};
use feral_processes_engine::tuning::CREATION_COST_PER_ATTRIBUTE_POINT;

use crate::{App, CreationRow, GameKey, Mode};

/// What the points are for, which decides their price and whether the
/// screen may be left with some unspent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllocationFor {
    /// Character creation: a pool that is lost if not spent, so the wizard
    /// will not let go of the screen while a point is affordable.
    Creation,
    /// Points banked on `StatOwner`, which stay banked if not spent.
    Owned(StatOwner),
}

impl AllocationFor {
    /// Pool points one attribute point costs.
    fn cost(self) -> u32 {
        match self {
            AllocationFor::Creation => CREATION_COST_PER_ATTRIBUTE_POINT,
            AllocationFor::Owned(_) => 1,
        }
    }
}

/// The Points screen's fixed half. See the module doc.
#[derive(Clone, Debug)]
pub struct StatAllocation {
    purpose: AllocationFor,
    pool: u32,
    /// What the derivation starts from: the player's, or the program's own.
    base: DerivedBase,
    rows: Vec<AttributeDef>,
    db: AttributeDb,
    start: Attributes,
    /// What perks, gear, implants and keys hold on top of the derivation.
    /// `None` at creation, where none exists yet.
    bonus: Option<StatBonus>,
}

/// What a Points-screen key does to the highlighted row's count, as
/// `f(current, max)`: Left/Right step by one, Shift goes to an end and Ctrl
/// halves the gap to that end. `None` for any other key. One table for
/// creation and `Mode::AllocateStats`, so the two screens cannot bind the
/// same key differently.
pub(crate) fn spend_adjustment(key: GameKey) -> Option<fn(u32, u32) -> u32> {
    Some(match key {
        GameKey::Left => |units, _| units.saturating_sub(1),
        GameKey::Right => |units, max| (units + 1).min(max),
        GameKey::ShiftLeft => |_, _| 0,
        GameKey::ShiftRight => |_, max| max,
        GameKey::CtrlLeft => |units, _| super::basket::halve(units, 0),
        GameKey::CtrlRight => super::basket::halve,
        _ => return None,
    })
}

/// The total pool cost of `spent`, unclamped by the pool.
pub(crate) fn spent_cost(purpose: AllocationFor, spent: &BTreeMap<AttributeId, u32>) -> u32 {
    spent
        .values()
        .map(|units| units.saturating_mul(purpose.cost()))
        .fold(0, u32::saturating_add)
}

impl StatAllocation {
    /// `start` is the attributes the spend lands on. Only attributes with
    /// effects are offered - a point on the others would buy nothing.
    pub fn new(
        purpose: AllocationFor,
        pool: u32,
        base: DerivedBase,
        db: AttributeDb,
        start: Attributes,
    ) -> Self {
        let rows = db.buyable().cloned().collect();
        Self {
            purpose,
            pool,
            base,
            rows,
            db,
            start,
            bonus: None,
        }
    }

    /// Previews through `bonus`, `Game::stat_bonus` for an owner who
    /// already holds perks, gear, implants or keys.
    pub fn with_bonus(mut self, bonus: StatBonus) -> Self {
        self.bonus = Some(bonus);
        self
    }

    pub fn purpose(&self) -> AllocationFor {
        self.purpose
    }

    pub fn pool(&self) -> u32 {
        self.pool
    }

    /// How many attributes are offered.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Pool points still unspent.
    pub fn remaining(&self, spent: &BTreeMap<AttributeId, u32>) -> u32 {
        self.pool.saturating_sub(spent_cost(self.purpose, spent))
    }

    /// Whether another point is still affordable - the creation wizard's
    /// halt condition, `roll_kit_basket`'s, read as a question.
    pub fn can_spend_more(&self, spent: &BTreeMap<AttributeId, u32>) -> bool {
        !self.rows.is_empty() && self.purpose.cost() <= self.remaining(spent)
    }

    /// The value an attribute reads once `spent` is applied.
    fn value(&self, def: &AttributeDef, spent: &BTreeMap<AttributeId, u32>) -> i32 {
        self.start.get(&def.id).unwrap_or(def.base)
            + spent.get(&def.id).copied().unwrap_or(0) as i32
    }

    /// The attributes with `spent` applied.
    fn spent_attributes(&self, spent: &BTreeMap<AttributeId, u32>) -> Attributes {
        let mut attrs = self.start.clone();
        for def in &self.rows {
            attrs.set(&def.id, self.value(def, spent));
        }
        attrs
    }

    /// One row per offered attribute, in id order.
    pub fn rows(&self, spent: &BTreeMap<AttributeId, u32>) -> Vec<CreationRow> {
        let held = |derived: DerivedStats| self.bonus.map_or(derived, |b| b.apply(derived));
        let before = held(derive(&self.base, &self.start, &self.db));
        let after = held(derive(&self.base, &self.spent_attributes(spent), &self.db));
        self.rows
            .iter()
            .map(|def| {
                let mut stats: Vec<DerivedStat> = Vec::new();
                for effect in &def.effects {
                    if !stats.contains(&effect.stat) {
                        stats.push(effect.stat);
                    }
                }
                CreationRow::Attribute {
                    id: def.id.clone(),
                    name: def.name.clone(),
                    legacy: def.legacy.clone(),
                    spent: spent.get(&def.id).copied().unwrap_or(0),
                    value: self.value(def, spent),
                    effects: stats
                        .into_iter()
                        .map(|stat| (stat, before.get(stat), after.get(stat)))
                        .collect(),
                    cost: self.purpose.cost(),
                }
            })
            .collect()
    }

    /// Applies `f(current, max)` to row `row`'s count, where `max` is the
    /// most that row could hold given what the *other* rows have spent -
    /// counting a row's own units against its own ceiling would make it
    /// unlowerable once the pool ran out. `Err` carries the refusal for a
    /// request that lands nowhere because the pool is empty: worth saying,
    /// since the arrow keys clamp by construction.
    pub fn spend_on_row(
        &self,
        spent: &mut BTreeMap<AttributeId, u32>,
        row: usize,
        f: impl FnOnce(u32, u32) -> u32,
    ) -> Result<(), String> {
        let Some(def) = self.rows.get(row) else {
            return Ok(());
        };
        let cost = self.purpose.cost();
        let before = spent.get(&def.id).copied().unwrap_or(0);
        let others = spent_cost(self.purpose, spent).saturating_sub(before.saturating_mul(cost));
        let max = self.pool.saturating_sub(others) / cost;
        let after = f(before, max).min(max);
        if after == before && before == max {
            return Err(format!(
                "No points left - {} costs {cost} a point.",
                def.name
            ));
        }
        match after {
            0 => spent.remove(&def.id),
            n => spent.insert(def.id.clone(), n),
        };
        Ok(())
    }

    /// `(name, value)` for each offered attribute, once `spent` is applied -
    /// the Summary's lines.
    pub fn values(&self, spent: &BTreeMap<AttributeId, u32>) -> Vec<(String, i32)> {
        self.rows
            .iter()
            .map(|def| (def.name.clone(), self.value(def, spent)))
            .collect()
    }
}

/// Which screen opened `Mode::AllocateStats`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AllocationOrigin {
    /// The level-up report: committing goes on to Perks if any Perk Points
    /// are unspent, and back to the map otherwise.
    #[default]
    LevelUp,
    /// The Perks screen (`S`): both keys go back to it.
    Perks,
    /// A program's Manifest (`S` on its Stats tab): both keys go back to the
    /// sheet, whose subject and tab are still `App`'s. `parked` is the
    /// roster row the sheet was holding in `menu_selected`, which the
    /// screen's own cursor borrows meanwhile.
    Manifest { parked: usize },
}

impl App {
    /// Opens `Mode::AllocateStats` on `owner`'s banked points, or says why
    /// not. The one writer of `stat_allocation`.
    pub(crate) fn open_stat_allocation(&mut self, owner: StatOwner, origin: AllocationOrigin) {
        let Some(game) = &self.game else { return };
        let banked = game.stat_points_of(owner);
        if banked == 0 {
            self.refuse("No stat points to spend.");
            return;
        }
        let entity = match owner {
            StatOwner::Player => game.player_entity(),
            StatOwner::Program(program) => program,
        };
        self.stat_allocation = Some(
            StatAllocation::new(
                AllocationFor::Owned(owner),
                banked,
                game.derived_base(entity),
                game.attribute_db(),
                game.attributes_of(entity),
            )
            .with_bonus(game.stat_bonus(entity)),
        );
        self.allocation_spent.clear();
        self.allocation_origin = origin;
        self.status_line = None;
        self.menu_selected = 0;
        self.refresh_allocation_duel();
        self.mode = Mode::AllocateStats;
    }

    /// The one writer of `allocation_duel`: the fight the pending spend
    /// would give, read from the engine on every change to the spend.
    fn refresh_allocation_duel(&mut self) {
        // The duel is the player against a typical foe, so only the
        // player's own spend has one to show.
        let for_player = matches!(
            self.stat_allocation.as_ref().map(|a| a.purpose()),
            Some(AllocationFor::Owned(StatOwner::Player))
        );
        if !for_player {
            self.allocation_duel = None;
            return;
        }
        let spend: Vec<_> = self
            .allocation_spent
            .iter()
            .map(|(id, points)| (id.clone(), *points))
            .collect();
        self.allocation_duel = self
            .game
            .as_mut()
            .and_then(|g| g.preview_stat_spend(&spend));
    }

    /// The Points screen's rows, for the renderer.
    pub fn allocation_rows(&self) -> Vec<CreationRow> {
        self.stat_allocation
            .as_ref()
            .map(|a| a.rows(&self.allocation_spent))
            .unwrap_or_default()
    }

    /// Points still unspent on the open Points screen.
    pub fn allocation_points_left(&self) -> u32 {
        self.stat_allocation
            .as_ref()
            .map_or(0, |a| a.remaining(&self.allocation_spent))
    }

    /// The pool on the open Points screen.
    pub fn allocation_pool(&self) -> u32 {
        self.stat_allocation.as_ref().map_or(0, |a| a.pool())
    }

    /// `Up`/`Down` move, `Left`/`Right` (with Shift and Ctrl) spend, `Enter`
    /// commits and `Esc` leaves without spending.
    pub(crate) fn handle_allocate_stats_key(&mut self, key: GameKey) {
        let Some(allocation) = self.stat_allocation.clone() else {
            self.mode = Mode::Playing;
            return;
        };
        match key {
            GameKey::Esc => self.leave_allocation(false),
            GameKey::Enter => self.commit_allocation(),
            GameKey::Up | GameKey::Down => self.scroll(key, allocation.len()),
            _ => {
                if let Some(f) = spend_adjustment(key) {
                    match allocation.spend_on_row(&mut self.allocation_spent, self.menu_selected, f)
                    {
                        Ok(()) => self.status_line = None,
                        Err(why) => self.refuse(why),
                    }
                    self.refresh_allocation_duel();
                }
            }
        }
    }

    fn commit_allocation(&mut self) {
        let spend: Vec<_> = self
            .allocation_spent
            .iter()
            .map(|(id, points)| (id.clone(), *points))
            .collect();
        let Some(AllocationFor::Owned(owner)) = self.stat_allocation.as_ref().map(|a| a.purpose())
        else {
            return;
        };
        let Some(game) = &mut self.game else { return };
        match game.spend_stat_points(owner, &spend) {
            Ok(()) => self.leave_allocation(true),
            Err(why) => self.refuse(format!("Cannot spend those points: {why:?}.")),
        }
    }

    /// Closes the Points screen. `committed` is whether the spend landed:
    /// from the level-up report a commit carries on to Perks while Perk
    /// Points are unspent, and `Esc` closes the whole flow to the map.
    fn leave_allocation(&mut self, committed: bool) {
        self.stat_allocation = None;
        self.allocation_spent.clear();
        self.status_line = None;
        self.menu_selected = 0;
        let perk_points = self
            .game
            .as_ref()
            .map_or(0, |g| g.player_status().perk_points);
        self.refresh_allocation_duel();
        match (self.allocation_origin, committed) {
            (AllocationOrigin::Manifest { parked }, _) => {
                self.menu_selected = parked;
                self.mode = Mode::Manifest;
            }
            (AllocationOrigin::Perks, _) => self.open_perks(),
            (AllocationOrigin::LevelUp, true) if perk_points > 0 => self.open_level_up_perks(),
            (AllocationOrigin::LevelUp, _) => self.leave_level_up_flow(),
        }
    }
}
