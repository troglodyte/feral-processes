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
        let (cells, caught) = self.blast_reach(cell, blast);
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

    /// The cells a blast at `cell` covers and the bodies standing in them —
    /// one derivation for the blast and for the AI that weighs setting it
    /// off, so what it scores is what lands.
    fn blast_reach(&self, cell: (i32, i32), blast: Blast) -> (Vec<(i32, i32)>, Vec<Entity>) {
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
        let caught = battle
            .bodies()
            .map(|(body, _)| body)
            .filter(|&body| reach::footprint_hit(&battle.cells_of(body), &covered))
            .filter(|&body| self.world.get::<Stats>(body).is_some())
            .collect();
        (cells, caught)
    }

    /// What `actor` stands to gain from the blast of the prop on `cell`, in
    /// Integrity: the damage it would do to the other side, less what it
    /// would do to its own — the actor included. Every body is hit for the
    /// blast's full figure (it rolls nothing), so this is exact for the
    /// bodies and silent on the chain it might set off.
    pub(crate) fn blast_net_value(&self, actor: Entity, cell: (i32, i32), blast: Blast) -> f32 {
        let own_side = self.acts_for_hostiles(actor);
        self.blast_reach(cell, blast)
            .1
            .into_iter()
            .map(|body| {
                let dealt = self.mitigate_incoming_damage(body, blast.damage as i32) as f32;
                if (self.world.get::<crate::components::Hostile>(body).is_some()) == own_side {
                    -dealt
                } else {
                    dealt
                }
            })
            .sum()
    }

    /// A prop swing the AI would rather take than `body`, or than nothing
    /// when `body` is `None`: the cell to strike.
    ///
    /// Two candidates, both swings (`tactical_attack_prop`) from `cells`
    /// at `range`, and both a plain argmax with no draw:
    ///
    /// - **Detonate** — a volatile prop one blow destroys, scored by
    ///   `blast_net_value` against what the swing at `body` is expected to
    ///   take off it after mitigation. Candidates are sorted by `(y, x)`
    ///   before they are scored and the first of equals wins.
    /// - **Break cover** — only with no `body` to hit, and only when no
    ///   cell the actor can reach sees any of `targets`, and the one
    ///   thing between the actor's cell and the nearest target is a single
    ///   destructible prop it can reach. One blow is not required: it
    ///   keeps swinging on later turns.
    pub(crate) fn best_prop_swing(
        &self,
        actor: Entity,
        from: (i32, i32),
        range: u32,
        body: Option<Entity>,
        targets: &[(i32, i32)],
    ) -> Option<(i32, i32)> {
        let battle = self.world.resource::<TacticalBattle>();
        battle.board.props().next()?;
        let cells = &crate::tactical::footprint_cells_at(from, battle.footprint_of(actor));
        let reaches = |cell: (i32, i32)| reach::swing_reaches(&battle.board, cells, &[cell], range);
        let blow = self.swing_damage(actor);

        let mut volatile: Vec<((i32, i32), &PropCell)> = battle
            .board
            .props()
            .filter(|(_, p)| p.destructible() && p.volatile.is_some())
            .map(|(&cell, p)| (cell, p))
            .collect();
        volatile.sort_by_key(|&((x, y), _)| (y, x));
        let mut best = body.map_or(0.0, |body| self.expected_swing_value(actor, body));
        let mut pick = None;
        for (cell, prop) in volatile {
            let (Some(hp), Some(blast)) = (prop.hp, prop.volatile) else {
                continue;
            };
            if blow.saturating_sub(prop.armour).max(1) < hp || !reaches(cell) {
                continue;
            }
            let value = self.blast_net_value(actor, cell, blast);
            if value > best {
                best = value;
                pick = Some(cell);
            }
        }
        if pick.is_some() || body.is_some() {
            return pick;
        }
        self.cover_to_break(actor, from, cells, targets)
            .filter(|&cell| reaches(cell))
    }

    /// What a swing at `body` is expected to take off it, after mitigation —
    /// `battle::expected_damage` over the real profiles, the scale
    /// `walk_risk` already prices on.
    fn expected_swing_value(&self, actor: Entity, body: Entity) -> f32 {
        let swing = crate::battle::Swing::plain(self.natural_range_of(actor));
        let expected = crate::battle::expected_damage(
            self.combatant_profile(actor, swing),
            self.defender_profile_against(actor, body, swing),
        );
        self.mitigate_incoming_damage(body, expected.round() as i32) as f32
    }

    /// The destructible prop that is the whole of what stands between the
    /// actor and its nearest target, when no cell it can reach would show it
    /// any target at all.
    fn cover_to_break(
        &self,
        actor: Entity,
        from: (i32, i32),
        cells: &[(i32, i32)],
        targets: &[(i32, i32)],
    ) -> Option<(i32, i32)> {
        let battle = self.world.resource::<TacticalBattle>();
        let nearest = targets
            .iter()
            .copied()
            .min_by_key(|&(x, y)| (reach::gap(cells, &[(x, y)]), y, x))?;
        let field = reach::movement_field(battle, actor, self.movement_allowance(actor));
        let sees_a_target = field.keys().chain(cells).any(|&at| {
            targets
                .iter()
                .any(|&t| reach::line_of_sight(&battle.board, at, t))
        });
        if sees_a_target {
            return None;
        }
        let [blocker] = reach::sight_blockers(&battle.board, from, nearest)[..] else {
            return None;
        };
        battle
            .board
            .prop_at(blocker.0, blocker.1)
            .is_some_and(PropCell::destructible)
            .then_some(blocker)
    }
}
