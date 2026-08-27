//! Spending the levels a Kernel Ring bought.
//!
//! One point per level above `TALENT_START_LEVEL`, spent on one of two choices
//! in the next untaken tier of this companion's class tree
//! (`assets/talents/`). **Points are derived, never stored**: `earned` is the
//! level minus the base cap and `spent` is the length of `components::Talents`,
//! so nothing here can desync from the level or from the list the way a stored
//! count would.

use crate::talents::{TalentDb, TalentId, TalentNode, TalentStat, TalentTree};
use crate::views::{TalentOption, TalentPoints};
use crate::*;

impl Game {
    /// The tree `entity` spends its points in — its class's, or the generic
    /// tree when its species raises no axis or more than one. `None` only if
    /// the install ships no trees at all.
    pub fn talent_tree(&self, entity: Entity) -> Option<&TalentTree> {
        let class = self.creature_class(entity);
        self.world.resource::<TalentDb>().get(class)
    }

    /// What `entity` has earned and spent. Derived on both halves — see this
    /// module's docs.
    pub fn talent_points(&self, entity: Entity) -> TalentPoints {
        let level = self
            .world
            .get::<Experience>(entity)
            .map(|e| e.level)
            .unwrap_or(1);
        TalentPoints {
            earned: level.saturating_sub(crate::tuning::TALENT_START_LEVEL),
            spent: self
                .world
                .get::<Talents>(entity)
                .map_or(0, |t| t.0.len() as u32),
        }
    }

    /// Which tier `entity` is next spending in — the count of what it has
    /// already taken, since a tier costs exactly one point and they are taken
    /// in order.
    fn next_talent_tier(&self, entity: Entity) -> usize {
        self.world.get::<Talents>(entity).map_or(0, |t| t.0.len())
    }

    /// Every node in `entity`'s tree, tier by tier, with what the menu needs
    /// to draw the ladder: what is bought, what one point could buy now, and
    /// what is still out of reach.
    ///
    /// Assembled here rather than in a renderer for the reason
    /// `Game::copy_name` is one function — two screens building the same row
    /// are two screens that can disagree about it.
    pub fn talent_options(&mut self, entity: Entity) -> Vec<TalentOption> {
        let unspent = self.talent_points(entity).unspent();
        let next = self.next_talent_tier(entity);
        let taken = self
            .world
            .get::<Talents>(entity)
            .map(|t| t.0.clone())
            .unwrap_or_default();
        let Some(tree) = self.talent_tree(entity) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for (i, tier) in tree.tiers.iter().enumerate() {
            for choice in &tier.0 {
                rows.push(TalentOption {
                    tier: i as u32 + 1,
                    id: choice.id.clone(),
                    name: choice.name.clone(),
                    description: choice.description.clone(),
                    tag: choice.node.tag(),
                    taken: taken.contains(&choice.id),
                    takeable: i == next && unspent > 0,
                });
            }
        }
        rows
    }

    /// Spends one talent point on `id`.
    ///
    /// Every refusal lands before anything is spent or recorded, the ordering
    /// `Game::refactor_companion` and `Game::open_kernel_ring` both state. The
    /// id is resolved against the **next tier of this companion's own tree**,
    /// so a node from another class's tree and a node two tiers deep are two
    /// different refusals rather than one accidental permission.
    pub fn take_talent(&mut self, entity: Entity, id: &TalentId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if self
            .world
            .get::<Tamed>(entity)
            .is_none_or(|t| t.owner != self.player_entity())
        {
            return Err("You don't control that program.".into());
        }
        let points = self.talent_points(entity);
        if points.unspent() == 0 {
            return Err(format!(
                "{} has no talent points to spend — they come from levels earned past {}.",
                self.entity_label(entity),
                crate::tuning::TALENT_START_LEVEL
            ));
        }
        let next = self.next_talent_tier(entity);
        let Some(tree) = self.talent_tree(entity) else {
            return Err("This install has no talent trees.".into());
        };
        let Some(tier) = tree.tiers.get(next) else {
            return Err(format!(
                "{} has taken every talent its tree offers.",
                self.entity_label(entity)
            ));
        };
        let Some(choice) = tier.0.iter().find(|c| &c.id == id) else {
            // Split deliberately: a node deeper in the same tree leaves the
            // player a different errand from one that was never on offer.
            let deeper = tree.tiers.iter().any(|t| t.0.iter().any(|c| &c.id == id));
            return Err(if deeper {
                format!(
                    "That talent is deeper in the tree; tier {} comes first.",
                    next + 1
                )
            } else {
                "That talent is not in this program's tree.".to_string()
            });
        };
        let node = choice.node.clone();
        let name = choice.name.clone();

        // Recorded *before* the effect is applied, because two of the four
        // node kinds are read back off this very list: `install_unlocked_
        // routines` asks `talent_abilities` what this program's talents grant,
        // and that has to include the one just bought. Every refusal is already
        // behind us, so nothing here can leave a receipt for something that
        // did not happen.
        let mut talents = self
            .world
            .get::<Talents>(entity)
            .cloned()
            .unwrap_or_default();
        talents.0.push(id.clone());
        self.world.entity_mut(entity).insert(talents);
        self.apply_talent_node(entity, &node);

        let label = self.entity_label(entity);
        self.log(format!("{label} takes {name}."));
        Ok(())
    }

    /// What a node does at purchase. Three of the four kinds do nothing here
    /// and are read on demand instead — an affinity off `Game::ability_affinity`,
    /// a slot off `Game::routine_slots`, and a granted routine through the same
    /// install path a species-kit unlock uses.
    fn apply_talent_node(&mut self, entity: Entity, node: &TalentNode) {
        match node {
            TalentNode::Stat { stat, percent } => self.bake_talent_stat(entity, *stat, *percent),
            TalentNode::Affinity { .. } | TalentNode::RoutineSlot | TalentNode::Accuracy { .. } => {
            }
            // Through `install_unlocked_routines` rather than
            // `install_innate_routines`, with an empty level range so the
            // species half offers nothing: a granted routine then competes for
            // slots exactly as an innate *unlock* does, which includes
            // displacing `FALLBACK_ABILITY_ID`. That is the placeholder doing
            // its job — a companion whose species grants nothing holds it in
            // its one slot, and without the eviction the first talent routine
            // would be refused for want of room and lost.
            TalentNode::Ability { .. } => {
                let level = self
                    .world
                    .get::<Experience>(entity)
                    .map(|e| e.level)
                    .unwrap_or(1);
                self.install_unlocked_routines(entity, level, level);
            }
        }
    }

    /// Raises one stat by `percent`, through the same `refactor::raised` a
    /// percentage buff uses — including its never-less-than-a-whole-point
    /// floor, which is the whole reason this calls it rather than restating
    /// the arithmetic.
    ///
    /// Gear is lifted and put back around the write, exactly as
    /// `refactor_companion` does it and for the same reason: a bonus sitting
    /// in `Stats` during a multiplication is scaled, and the later unequip
    /// subtracts only the unscaled amount, welding the difference into the
    /// program's base stats forever.
    ///
    /// Current HP rises by the delta the maximum rose by rather than
    /// refilling. A level-up full-heals; a talent must not, or the tree would
    /// be carried into fights as the strongest heal in the game.
    fn bake_talent_stat(&mut self, entity: Entity, stat: TalentStat, percent: f32) {
        let gear = self.gear_bonus(entity);
        self.apply_equipment_delta(entity, gear, -1);
        {
            let mut stats = self.world.get_mut::<Stats>(entity).unwrap();
            match stat {
                TalentStat::Hp => {
                    let raised = crate::game::refactor::raised(stats.max_hp, percent);
                    stats.hp += raised - stats.max_hp;
                    stats.max_hp = raised;
                }
                TalentStat::Atk => stats.atk = crate::game::refactor::raised(stats.atk, percent),
                TalentStat::Def => {
                    stats.mitigation = crate::game::refactor::raised(stats.mitigation, percent)
                }
            }
        }
        self.apply_equipment_delta(entity, gear, 1);
    }
}
