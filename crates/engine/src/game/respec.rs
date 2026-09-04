//! Taking a ledger back for a flat Credit price.
//!
//! Two doors — `Game::respec_perks` for the player's perks and
//! `Game::respec_talents` for one companion's talents — because what comes
//! back besides stats differs: perk points are a stored number to credit,
//! talent points are derived from a list and refund themselves when it is
//! cleared. One shared `unbake_bought_stats`, because two writes into `Stats`
//! are two chances to disagree about what a purchase was worth.
//!
//! Every refusal lands before a Credit moves, `commit_caravan_basket`'s rule.

use crate::abilities::AbilityId;
use crate::components::BoughtStats;
use crate::items::ids;
use crate::tuning::RESPEC_CREDIT_COST;
use crate::views::{RespecQuote, RespecSubject};
use crate::*;

impl Game {
    /// What a respec would cost and hand back, for the screens' footer and
    /// the confirm page. Every figure a call, nothing stored.
    pub fn respec_quote(&self, subject: RespecSubject) -> RespecQuote {
        let credits = self.banked(&ids::CREDITS.into());
        let (purchases, points_returned) = match subject {
            RespecSubject::Perks => {
                let perks = self.player_perks();
                (
                    perks.map_or(0, |p| p.unlocked.len() as u32),
                    self.perk_point_refund(),
                )
            }
            RespecSubject::Talents(entity) => {
                let taken = self.world.get::<Talents>(entity).map_or(0, |t| t.0.len()) as u32;
                (taken, taken)
            }
        };
        RespecQuote {
            cost: RESPEC_CREDIT_COST,
            credits,
            purchases,
            points_returned,
            refusal: self.respec_refusal(subject),
        }
    }

    /// What clearing `Perks::unlocked` is worth in Perk Points, priced at the
    /// catalogue's *current* cost.
    ///
    /// A perk whose `.ron` cost was retuned since it was bought refunds what
    /// it would cost today, which is the only price the player can see on the
    /// screen they are standing on.
    fn perk_point_refund(&self) -> u32 {
        let Some(perks) = self.player_perks() else {
            return 0;
        };
        let db = self.world.resource::<crate::perks::PerkDb>();
        perks
            .unlocked
            .iter()
            .filter_map(|p| db.get(*p))
            .map(|d| d.cost)
            .sum()
    }

    /// The one statement of when a respec is refused, so the quote's greyed
    /// footer and the commit cannot disagree about why.
    fn respec_refusal(&self, subject: RespecSubject) -> Option<String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Some("Can't do that right now.".into());
        }
        match subject {
            RespecSubject::Perks => {
                let perks = self.player_perks();
                if perks.is_none_or(|p| p.unlocked.is_empty()) {
                    return Some("You have no perks to refund.".into());
                }
                // `Perk::ProcessPool` is the one term in `pet_capacity` a
                // respec takes away, so a full roster would be over capacity
                // the moment it is refunded. Refused rather than left
                // over-full: there is no rule anywhere else in the game for a
                // roster above its cap, and inventing one here would be a
                // second meaning for `pet_capacity`.
                let without = self
                    .pet_capacity()
                    .saturating_sub(crate::perks::roster_slot_bonus(perks));
                let held = self.pet_count();
                if held > without {
                    return Some(format!(
                        "Refunding Process Pool would leave {held} programs in {without} slots — release {} first.",
                        held - without
                    ));
                }
            }
            RespecSubject::Talents(entity) => {
                if self
                    .world
                    .get::<Tamed>(entity)
                    .is_none_or(|t| t.owner != self.player_entity())
                {
                    return Some("You don't control that program.".into());
                }
                if self
                    .world
                    .get::<Talents>(entity)
                    .is_none_or(|t| t.0.is_empty())
                {
                    return Some(format!(
                        "{} has taken no talents to refund.",
                        self.entity_label(entity)
                    ));
                }
            }
        }
        let credits = self.banked(&ids::CREDITS.into());
        if credits < RESPEC_CREDIT_COST {
            return Some(format!(
                "Not enough Credits (need {RESPEC_CREDIT_COST}, have {credits})."
            ));
        }
        None
    }

    /// Refunds every perk level for `tuning::RESPEC_CREDIT_COST`.
    pub fn respec_perks(&mut self) -> Result<(), String> {
        if let Some(refusal) = self.respec_refusal(RespecSubject::Perks) {
            return Err(refusal);
        }
        let refund = self.perk_point_refund();
        let player = self.player_entity();
        self.charge_respec();
        self.unbake_bought_stats(player);
        if let Some(mut perks) = self.world.get_mut::<Perks>(player) {
            perks.points += refund;
            perks.unlocked.clear();
        }
        // No `Deed::UnlockedPerk`. `unlock_perk` records one for contracts,
        // and a respec is not an unlock — noting one here would let a
        // wipe-and-rebuy loop farm the objective.
        self.log(format!(
            "You unwind your perks. {refund} Perk Points return."
        ));
        Ok(())
    }

    /// Refunds every talent `entity` has taken, same price.
    pub fn respec_talents(&mut self, entity: Entity) -> Result<(), String> {
        if let Some(refusal) = self.respec_refusal(RespecSubject::Talents(entity)) {
            return Err(refusal);
        }
        // Asked before the list is cleared, since it is derived from it.
        let granted = self.talent_abilities(entity);
        self.charge_respec();
        self.unbake_bought_stats(entity);
        // Clearing the list *is* the point refund — `talent_points` derives
        // `spent` from its length, which is the payoff of points being
        // derived and never stored.
        self.world.entity_mut(entity).insert(Talents(Vec::new()));
        self.rebuild_routines_after_talent_loss(entity, &granted);
        let label = self.entity_label(entity);
        self.log(format!("{label} unwinds its talents."));
        Ok(())
    }

    /// Takes the flat price. Called only after `respec_refusal` has cleared
    /// the sale, so the Credits are known to be there.
    fn charge_respec(&mut self) {
        let player = self.player_entity();
        if let Some(mut inv) = self.world.get_mut::<Inventory>(player) {
            inv.take(ids::CREDITS.into(), RESPEC_CREDIT_COST);
        }
    }

    /// Subtracts `entity`'s receipt from its `Stats` and zeroes it — the one
    /// writer, shared by both doors.
    ///
    /// Gear is lifted and put back around the write, exactly as
    /// `bake_talent_stat` and `refactor_companion` do it and for the same
    /// reason: a bonus sitting in `Stats` during the operation is subtracted
    /// at the worn value and the later unequip takes it out again.
    ///
    /// Current HP clamps to the new maximum rather than refilling. A respec
    /// must not be the strongest heal in the game — `bake_talent_stat`'s own
    /// argument, one direction over.
    ///
    /// `ever_bought` is deliberately untouched.
    fn unbake_bought_stats(&mut self, entity: Entity) {
        let Some(receipt) = self.world.get::<BoughtStats>(entity).copied() else {
            return;
        };
        let gear = self.gear_bonus(entity);
        self.apply_equipment_delta(entity, gear, -1);
        if let Some(mut stats) = self.world.get_mut::<Stats>(entity) {
            stats.atk -= receipt.atk;
            stats.mitigation -= receipt.mitigation;
            stats.max_hp -= receipt.max_hp;
            stats.hp = stats.hp.min(stats.max_hp);
        }
        self.apply_equipment_delta(entity, gear, 1);
        self.world.entity_mut(entity).insert(BoughtStats {
            atk: 0,
            mitigation: 0,
            max_hp: 0,
            ever_bought: receipt.ever_bought,
        });
    }

    /// Puts `entity`'s routine kit back to what its species and level alone
    /// would give it, after its talents have been refunded.
    ///
    /// Removes what the talents *granted* rather than clearing `Routines` and
    /// re-installing: a program's slots also hold routines the player
    /// installed by hand from a disk, and those are not the tree's to take
    /// back. `TalentNode::RoutineSlot` is why the truncate is needed at all —
    /// refunding it narrows `routine_slots`, so a kit that exactly filled the
    /// widened row no longer fits.
    fn rebuild_routines_after_talent_loss(&mut self, entity: Entity, granted: &[AbilityId]) {
        let slots = self.routine_slots(entity);
        if let Some(mut routines) = self.world.get_mut::<Routines>(entity) {
            routines.0.retain(|id| !granted.contains(id));
            routines.0.truncate(slots);
        }
        // Refills from the species kit if the truncate left room, and puts the
        // placeholder back if the program is now holding nothing at all.
        self.install_innate_routines(entity);
    }
}
