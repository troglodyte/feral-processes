//! Fighting the terrain: a swing or a routine at a prop, its destruction,
//! and what a volatile one does on the way out.
//!
//! **A prop is not a body.** It holds no initiative slot, takes no hit roll,
//! pays no XP and touches no morale, which is why it is struck through
//! `Board::damage_prop` here rather than seated beside the creatures
//! (the spec's rejected "inert body" design). Everything a destroyed prop
//! does to a *body* goes through `Game::apply_damage`, the one door damage
//! comes through.

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::abilities::{self, AbilityDef, AbilityEffect, AbilityShape};
use crate::battle::DamageRange;
use crate::components::Stats;
use crate::resources::{TacticalFxKind, TacticalFxQueue};
use crate::tactical::TacticalBattle;
use crate::tactical::map::{PropCell, PropHit};
use crate::tactical::props::{Blast, Leaves, PropDb, RUBBLE_PIECE};
use crate::tactical::reach;
use crate::tactical::view::PropView;
use crate::tuning::TACTICAL_PROP_CHAIN_MAX;

/// What a prop is called in a log line: its piece id, spoken.
pub fn prop_name(prop: &PropCell) -> String {
    prop.piece.replace('_', " ")
}

impl Game {
    /// The acting body swings at the prop on `cell`.
    ///
    /// Refused, with nothing spent, when there is no fight, the body has no
    /// action left or is charging, the cell holds nothing a blow can break
    /// (empty, decoration, indestructible), or the swing does not reach it —
    /// `reach::swing_reaches`, the one definition, with the prop's one cell
    /// as the target footprint. No hit roll: damage is the swinger's plain
    /// figure less the prop's armour, at least 1.
    pub fn tactical_attack_prop(&mut self, cell: (i32, i32)) -> bool {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return false;
        };
        let Some(actor) = battle.actor() else {
            return false;
        };
        if battle.actions_left() == 0 || self.is_charging(actor) {
            return false;
        }
        let Some(prop) = battle
            .board
            .prop_at(cell.0, cell.1)
            .filter(|p| p.destructible())
        else {
            return false;
        };
        let armour = prop.armour;
        let actor_cells = battle.cells_of(actor);
        if !reach::swing_reaches(
            &battle.board,
            &actor_cells,
            &[cell],
            self.swing_range(actor),
        ) {
            return false;
        }
        let round_before = battle.round;
        let dmg = self.swing_damage(actor).saturating_sub(armour).max(1);
        self.strike_prop(cell, dmg);
        self.world.resource_mut::<TacticalBattle>().spend_action();
        // A blast can drop any body on the board, the swinger included.
        self.reap_tactical_dead(None);
        self.hand_on_turn(actor, round_before);
        true
    }

    /// The prop on `cell`, as the frontend draws and reads it.
    pub fn tactical_prop_at(&self, cell: (i32, i32)) -> Option<PropView> {
        let battle = self.world.get_resource::<TacticalBattle>()?;
        battle
            .board
            .prop_at(cell.0, cell.1)
            .map(|p| PropView::of(cell, p))
    }

    /// What a damaging routine does to a prop it covers: the mean of the
    /// scaled band plus the invoker's attack, which is the figure a body
    /// takes before mitigation. A mean and not a roll, so a prop in the
    /// blast never moves the seeded stream. `None` for everything that is
    /// not damage.
    pub(crate) fn routine_prop_damage(&self, actor: Entity, ability: &AbilityDef) -> Option<u32> {
        let (AbilityEffect::Damage { power, spread, .. }
        | AbilityEffect::Drain { power, spread, .. }) = &ability.effect
        else {
            return None;
        };
        let band = abilities::scaled_range(
            DamageRange::centred(*power, *spread),
            self.ability_user_level(actor),
            self.ability_affinity(actor, &ability.effect),
        );
        Some((band.mean().round() as i32 + self.effective_atk(actor)).max(1) as u32)
    }

    /// A damaging routine breaks what it covers: `damage` against every
    /// destructible prop on `cells`, less each one's armour.
    pub(crate) fn routine_hits_props(&mut self, cells: &[(i32, i32)], damage: u32) {
        for &cell in cells {
            self.strike_prop_for(cell, damage);
        }
    }

    /// `strike_prop` at an armoured prop's own figure.
    fn strike_prop_for(&mut self, cell: (i32, i32), raw: u32) {
        let armour = self
            .world
            .get_resource::<TacticalBattle>()
            .and_then(|b| b.board.prop_at(cell.0, cell.1).map(|p| p.armour));
        if let Some(armour) = armour {
            self.strike_prop(cell, raw.saturating_sub(armour).max(1));
        }
    }

    fn strike_prop(&mut self, cell: (i32, i32), dmg: u32) {
        self.strike_prop_at_depth(cell, dmg, 0);
    }

    fn strike_prop_at_depth(&mut self, cell: (i32, i32), dmg: u32, depth: u32) {
        let hit = self
            .world
            .resource_mut::<TacticalBattle>()
            .board
            .damage_prop(cell, dmg);
        match hit {
            PropHit::Destroyed(prop) => self.prop_destroyed(cell, prop, depth),
            PropHit::Damaged => {
                if let Some(prop) = self.tactical_prop_at(cell) {
                    self.log(format!("The {} takes a hit.", prop.name));
                }
            }
            PropHit::Missing | PropHit::Indestructible => {}
        }
    }

    /// The prop is already off the board: leave what it leaves, cue it, say
    /// so, and let a volatile one go off at `depth`.
    fn prop_destroyed(&mut self, cell: (i32, i32), prop: PropCell, depth: u32) {
        if prop.leaves == Leaves::Rubble
            && let Some(rubble) = self
                .world
                .get_resource::<PropDb>()
                .and_then(|db| db.piece(RUBBLE_PIECE))
                .map(PropCell::from_def)
        {
            self.world
                .resource_mut::<TacticalBattle>()
                .board
                .place_prop(cell, rubble);
        }
        let volatile = prop.volatile.is_some();
        self.world
            .resource_mut::<TacticalFxQueue>()
            .push(cell, TacticalFxKind::PropDestroyed { volatile });
        let name = prop_name(&prop);
        self.log(if volatile {
            format!("The {name} detonates.")
        } else {
            format!("The {name} collapses.")
        });
        if let Some(blast) = prop.volatile {
            self.detonate(cell, blast, depth);
        }
    }

    /// A volatile prop's blast: full damage to every body within `radius` of
    /// `cell` and a chain into any volatile prop it breaks, to
    /// `TACTICAL_PROP_CHAIN_MAX` deep. Friendly fire is full — nothing here
    /// reads `Hostile`, `reach::recipients`' rule.
    ///
    /// The radius is the one a radius routine covers, `reach::shape_cells`
    /// sight-clipped from the blast, so a standing wall shields what is
    /// behind it.
    fn detonate(&mut self, cell: (i32, i32), blast: Blast, depth: u32) {
        if depth >= TACTICAL_PROP_CHAIN_MAX {
            return;
        }
        let battle = self.world.resource::<TacticalBattle>();
        let cells = reach::shape_cells(
            &battle.board,
            cell,
            cell,
            AbilityShape::Radius {
                radius: blast.radius,
            },
        );
        let covered = cells.iter().copied().collect();
        let caught: Vec<Entity> = battle
            .bodies()
            .map(|(body, _)| body)
            .filter(|&body| reach::footprint_hit(&battle.cells_of(body), &covered))
            .filter(|&body| self.world.get::<Stats>(body).is_some())
            .collect();
        for body in caught {
            let label = self.entity_label(body);
            let dealt = self.apply_damage(body, blast.damage as i32);
            self.log(format!("The blast catches {label} for {dealt} Integrity."));
        }
        for c in cells {
            let hit = self
                .world
                .resource::<TacticalBattle>()
                .board
                .prop_at(c.0, c.1)
                .filter(|p| p.destructible())
                .map(|p| blast.damage.saturating_sub(p.armour).max(1));
            if let Some(dmg) = hit {
                self.strike_prop_at_depth(c, dmg, depth + 1);
            }
        }
    }
}
