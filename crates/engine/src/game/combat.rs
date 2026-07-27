//! Starting a battle and planning a round: pack gathering, initiative, and
//! the action menus the renderer draws from.

use crate::abilities::AbilityId;
use crate::tuning::{DEFAULT_BASE_SPEED, DEFEND_DEF_BONUS, INITIATIVE_DIE, PLAYER_BASE_SPEED};
use crate::*;

impl Game {
    /// The widest `max_group_size` among `members`' own tiles — the ceiling
    /// a gathered cluster fights under, measured the same way in both
    /// `gather_pack` and `group_pack`. Measured the same way, not guaranteed
    /// equal: `gather_pack` truncates *after* measuring, so a widest member
    /// sitting past the cut leaves `group_pack` measuring a narrower ceiling
    /// from the survivors and holding fewer than the pack carries. Benign —
    /// the surplus stays standing, like every other overflow here.
    ///
    /// Read from every member rather than from the cluster's anchor because
    /// group size *doubles* every `GROUP_SIZE_STEP_TILES`: the spawn roll
    /// sized the cluster from its own tile and scattered members up to
    /// `swarm_radius` around it, so an anchor that drifted a few tiles
    /// inward of a step boundary would otherwise halve the fight — a cluster
    /// spawned at distance 90 whose anchor sits at 87 would field 32 of its
    /// 64. A max over a set is order-independent, so this stays
    /// deterministic under a seed without sorting anything.
    fn widest_group_size(&self, members: &[Entity]) -> usize {
        members
            .iter()
            .filter_map(|&e| self.world.get::<Position>(e))
            .map(|p| self.max_group_size(p.x, p.y) as usize)
            .max()
            .unwrap_or(1)
    }

    /// The widest `max_enemy_groups` among `members`' own tiles, measured
    /// from every member for the same reason `widest_group_size` is: the
    /// count rides the same distance curve, so a cluster straddling a step
    /// boundary would otherwise fight under whichever tile its anchor
    /// happened to land on.
    fn widest_enemy_groups(&self, members: &[Entity]) -> usize {
        members
            .iter()
            .filter_map(|&e| self.world.get::<Position>(e))
            .map(|p| self.max_enemy_groups(p.x, p.y))
            .max()
            .unwrap_or(1)
    }

    /// Every alive `Hostile` creature within `swarm_radius` tiles of
    /// `anchor` (Chebyshev distance) — the whole cluster a group spawn
    /// roll placed together (see `try_spawn_habitat_creature`) joins the
    /// fight at once when the player bumps into any one of them. `anchor`
    /// is always first, becoming the initial front target. Truncated to
    /// `widest_enemy_groups` groups' worth of `widest_group_size`, so how deep
    /// a fight this ground can produce is bounded by the same danger curve
    /// that decided how many spawned here, and never exceeds what
    /// `group_pack` can then hold. The remainder stays standing on the map
    /// and is met on the next bump, which is what surplus groups already do.
    pub(crate) fn gather_pack(&mut self, anchor: Entity) -> Vec<Entity> {
        let Some(anchor_pos) = self.world.get::<Position>(anchor).copied() else {
            return vec![anchor];
        };
        // The *radius* is the one thing that can only come from the anchor:
        // there are no members to read until the search has already run.
        // Deliberately asymmetric with the ceiling below — a radius that
        // errs small leaves a member at the fringe, where a ceiling that
        // errs small leaves half the cluster.
        let radius =
            crate::game::spawning::swarm_radius(self.max_group_size(anchor_pos.x, anchor_pos.y));
        let mut pack = vec![anchor];
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Hostile>>();
        for (e, pos) in query.iter(&self.world) {
            if e == anchor {
                continue;
            }
            let dist = (pos.x - anchor_pos.x)
                .abs()
                .max((pos.y - anchor_pos.y).abs());
            if dist <= radius {
                pack.push(e);
            }
        }
        pack.truncate(self.widest_group_size(&pack) * self.widest_enemy_groups(&pack));
        pack
    }

    /// Partitions `pack` into one group per species, in first-appearance
    /// order. A cluster spanning more than `widest_enemy_groups` species keeps
    /// only its largest groups, and each group is itself capped at
    /// `widest_group_size` — a spawn roll places one species, so without
    /// that a single deep roll would fight as one column rather than as the
    /// groups the danger curve allows. Neither surplus is returned: both
    /// stay on the map as ordinary hostiles, met on the next bump.
    ///
    /// Members are expected to carry a `Position`, since that ceiling is
    /// read from the ground they stand on; a pack with none falls back to
    /// one member per group. Worth knowing when assembling a pack by hand
    /// rather than from `gather_pack` (see the tests' `insert_battle`) —
    /// place the entities somewhere, or the fight comes out a single file.
    pub(crate) fn group_pack(&self, pack: Vec<Entity>) -> Vec<EnemyGroup> {
        let cap = self.widest_group_size(&pack);
        let max_groups = self.widest_enemy_groups(&pack);
        let mut groups: Vec<EnemyGroup> = Vec::new();
        for entity in pack {
            let Some(species) = self
                .world
                .get::<Creature>(entity)
                .map(|c| c.species.clone())
            else {
                continue;
            };
            match groups.iter_mut().find(|g| g.species == species) {
                Some(group) => {
                    // Over the ceiling: left standing on the map, met on the
                    // next bump — what surplus groups already do.
                    if group.members.len() < cap {
                        group.members.push(entity);
                    }
                }
                None => groups.push(EnemyGroup {
                    species,
                    members: vec![entity],
                }),
            }
        }
        // `sort_by_key` is stable, so equal-sized groups keep
        // first-appearance order and the truncation stays deterministic for
        // seeded tests.
        if groups.len() > max_groups {
            groups.sort_by_key(|g| std::cmp::Reverse(g.members.len()));
            groups.truncate(max_groups);
        }
        groups
    }

    pub(crate) fn start_battle(&mut self, pack: Vec<Entity>) {
        let player = self.player_entity();
        // A `CombatBuff` armed on the map by a consumable (see `use_item`'s
        // `prebattle_buff`) must carry into the fight it was armed for —
        // intentionally left untouched here, unlike `clear_battle_status_effects`.
        let groups = self.group_pack(pack);
        let name = groups
            .first()
            .and_then(|g| self.world.resource::<SpeciesDb>().get(&g.species))
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "program".to_string());
        let others = groups
            .iter()
            .map(|g| g.members.len())
            .sum::<usize>()
            .saturating_sub(1);
        let slots = self.world.resource::<Party>().0.len() + 1;
        self.world.insert_resource(BattleState {
            player,
            groups,
            round: 1,
            planned: vec![None; slots],
            finished: false,
            player_won: false,
        });
        if others > 0 {
            self.log(format!(
                "A pack of rogue programs intercepts your signal — a {name} takes point, {others} more behind it!"
            ));
        } else {
            self.log(format!("A rogue {name} intercepts your signal!"));
        }
    }

    /// The front member of `group` — the only one that takes hits.
    pub(crate) fn front_of_group(&self, group: usize) -> Option<Entity> {
        self.world
            .get_resource::<BattleState>()?
            .groups
            .get(group)?
            .front()
    }

    /// How many groups are still standing.
    pub(crate) fn living_group_count(&self) -> usize {
        self.world
            .get_resource::<BattleState>()
            .map(|b| b.groups.len())
            .unwrap_or(0)
    }

    /// Every living enemy across every group, in group-then-slot order.
    pub(crate) fn all_living_enemies(&self) -> Vec<Entity> {
        let Some(battle) = self.world.get_resource::<BattleState>() else {
            return Vec::new();
        };
        battle
            .groups
            .iter()
            .flat_map(|g| g.members.iter().copied())
            .filter(|&e| self.creature_alive(e))
            .collect()
    }

    /// The entity an `Actor` currently refers to, or `None` if that slot is
    /// empty (a party member stood down, an enemy already despawned).
    pub(crate) fn actor_entity(&self, actor: battle::Actor) -> Option<Entity> {
        match actor {
            battle::Actor::Party(0) => Some(self.player_entity()),
            battle::Actor::Party(i) => self.world.resource::<Party>().0.get(i - 1).copied(),
            battle::Actor::Enemy { group, slot } => self
                .world
                .get_resource::<BattleState>()?
                .groups
                .get(group)?
                .members
                .get(slot)
                .copied(),
        }
    }

    /// `entity`'s species `base_speed`, or the roster default if it has no
    /// `Creature` component (the player, who rolls from
    /// `PLAYER_BASE_SPEED` instead).
    pub(crate) fn species_base_speed(&self, entity: Entity) -> i32 {
        self.world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|s| s.base_speed)
            .unwrap_or(DEFAULT_BASE_SPEED)
    }

    /// Every living party member, plus each enemy group's front
    /// `battle::attackers_in_group` members rather than its whole roster —
    /// in descending initiative order. Ties break on a stable key — party
    /// before enemies, then slot / group index — so a seeded run always
    /// produces the same order.
    pub(crate) fn roll_initiative(&mut self) -> Vec<battle::Actor> {
        let Some(battle_state) = self.world.get_resource::<BattleState>() else {
            return Vec::new();
        };
        let group_sizes: Vec<usize> = battle_state
            .groups
            .iter()
            .map(|g| g.members.len())
            .collect();
        let party_len = self.world.resource::<Party>().0.len();

        // Built in tie-break order — player, party in slot order, then
        // enemies group-then-slot — so the stable sort below leaves equal
        // initiative rolls in exactly this order.
        let mut actors: Vec<battle::Actor> = (0..=party_len).map(battle::Actor::Party).collect();
        for (group, size) in group_sizes.into_iter().enumerate() {
            actors.extend(
                (0..battle::attackers_in_group(size))
                    .map(|slot| battle::Actor::Enemy { group, slot }),
            );
        }

        let mut rolled: Vec<(i32, battle::Actor)> = Vec::new();
        for actor in actors {
            let Some(entity) = self.actor_entity(actor) else {
                continue;
            };
            if !self.creature_alive(entity) {
                continue;
            }
            let base = match actor {
                battle::Actor::Party(0) => PLAYER_BASE_SPEED,
                _ => self.species_base_speed(entity),
            };
            let roll = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(0..=INITIATIVE_DIE)
            };
            rolled.push((base + roll, actor));
        }
        rolled.sort_by_key(|&(initiative, _)| std::cmp::Reverse(initiative));
        rolled.into_iter().map(|(_, actor)| actor).collect()
    }

    /// Whether `slot` still holds a combatant able to act. A party member
    /// knocked offline keeps its slot for the rest of the battle (it only
    /// leaves `Party` in `end_battle`, because these indices are positional
    /// into it), so the slot is still occupied — just by someone at 0 HP,
    /// who must not hold the round open, since nothing can fill it.
    pub(crate) fn slot_can_act(&self, slot: usize) -> bool {
        self.actor_entity(battle::Actor::Party(slot))
            .is_some_and(|entity| self.creature_alive(entity))
    }

    /// The party slot currently awaiting an action, or `None` when every
    /// slot that can still act has one.
    pub fn battle_active_slot(&self) -> Option<usize> {
        let battle = self.world.get_resource::<BattleState>()?;
        (0..battle.planned.len())
            .find(|&slot| battle.planned[slot].is_none() && self.slot_can_act(slot))
    }

    pub fn battle_round_ready(&self) -> bool {
        self.world.get_resource::<BattleState>().is_some_and(|b| {
            (0..b.planned.len()).all(|slot| b.planned[slot].is_some() || !self.slot_can_act(slot))
        })
    }

    pub fn battle_set_action(&mut self, slot: usize, action: BattleAction) -> Result<(), String> {
        let group_count = self.living_group_count();
        let Some(battle) = self.world.get_resource::<BattleState>() else {
            return Err("No active intrusion.".to_string());
        };
        if slot >= battle.planned.len() {
            return Err(format!("Slot {slot} isn't in your party."));
        }
        let planned_len = battle.planned.len();
        let target_group = match &action {
            BattleAction::Attack { group } => Some(*group),
            // A party-facing Special has no group to validate at all.
            BattleAction::Special {
                target: battle::SpecialTarget::EnemyGroup { group },
                ..
            } => Some(*group),
            _ => None,
        };
        if let Some(group) = target_group
            && group >= group_count
        {
            return Err("That group is already down.".to_string());
        }
        // An ally-targeted Special carries two more indices that reach the
        // world unchecked otherwise: the recipient's slot and which of the
        // acting member's abilities to spend. Both resolve to `None` at
        // resolve time and silently cost the member its round — while still
        // charging the player fatigue — so they are refused here instead.
        if let BattleAction::Special { ability, target } = &action {
            if let battle::SpecialTarget::Ally { slot: ally } = target
                && *ally >= planned_len
            {
                return Err(format!("Slot {ally} isn't in your party."));
            }
            let options = self.battle_special_options(slot);
            if *ability >= options.len() {
                return Err("That party member has no such ability.".to_string());
            }
            if let Some(reason) = &options[*ability].unavailable {
                return Err(format!("That ability isn't ready: {reason}."));
            }
        }
        self.world.resource_mut::<BattleState>().planned[slot] = Some(action);
        Ok(())
    }

    /// Assigns `action` to every slot that is still unplanned and able to act
    /// — the party-wide `[A]`/`[D]` commands. Slots that already hold a choice
    /// keep it, and a slot failing `slot_can_act` (a knocked-out companion) is
    /// left alone, matching what `battle_active_slot` would have skipped.
    pub fn battle_plan_remaining(&mut self, action: BattleAction) -> Result<(), String> {
        let Some(battle) = self.world.get_resource::<BattleState>() else {
            return Err("No active intrusion.".to_string());
        };
        let open: Vec<usize> = (0..battle.planned.len())
            .filter(|&slot| battle.planned[slot].is_none())
            .collect();
        for slot in open {
            if !self.slot_can_act(slot) {
                continue;
            }
            self.battle_set_action(slot, action.clone())?;
        }
        Ok(())
    }

    /// Clears `slot`'s plan and every slot after it, so the cursor lands
    /// back on `slot` — the player is correcting a choice, and everything
    /// they picked *after* it was picked in light of the mistake.
    pub fn battle_clear_action(&mut self, slot: usize) {
        let Some(mut battle) = self.world.get_resource_mut::<BattleState>() else {
            return;
        };
        for entry in battle.planned.iter_mut().skip(slot) {
            *entry = None;
        }
    }

    /// How many routines `entity` can hold right now. The player and a
    /// companion grow slots at different rates on purpose — see
    /// `tuning::PLAYER_ROUTINE_SLOT_PER_LEVEL`.
    pub fn routine_slots(&self, entity: Entity) -> usize {
        let level = self
            .world
            .get::<Experience>(entity)
            .map(|e| e.level)
            .unwrap_or(1);
        if entity == self.player_entity() {
            abilities::player_routine_slots(level)
        } else {
            abilities::companion_routine_slots(level)
        }
    }

    /// Installs the kit `entity`'s species grants at its current level,
    /// replacing whatever it holds. Called once when a program comes into
    /// existence — a decompile or a fusion — never afterwards.
    ///
    /// A species declaring no abilities gets `FALLBACK_ABILITY_ID` instead,
    /// which is what keeps an ability-less species commandable and keeps
    /// that ability obtainable by extraction: nothing else grants it.
    ///
    /// No shipped species declares more level-appropriate abilities than it
    /// has slots for at tame time, so the overflow this logs is mod-safety
    /// only — same rationale as `install_unlocked_routines`.
    pub(crate) fn install_innate_routines(&mut self, entity: Entity) {
        let level = self
            .world
            .get::<Experience>(entity)
            .map(|e| e.level)
            .unwrap_or(1);
        let slots = self.routine_slots(entity);
        let declared: Vec<AbilityId> = self
            .world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|s| s.abilities.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.level <= level)
            .map(|a| a.id)
            .filter(|id| self.world.resource::<AbilityDb>().get(id).is_some())
            .collect();
        let installed = if declared.is_empty() {
            vec![abilities::FALLBACK_ABILITY_ID.to_string()]
        } else {
            if declared.len() > slots {
                let name = self.creature_label(entity);
                for id in &declared[slots..] {
                    self.log(format!(
                        "{name} has no free routine slot for {} — the unlock is lost.",
                        self.ability_display_name(id)
                    ));
                }
            }
            declared.into_iter().take(slots).collect()
        };
        self.world.entity_mut(entity).insert(Routines(installed));
    }

    /// Installs every species ability whose unlock level lands in
    /// `(from_level, to_level]` — the ones this level-up just reached.
    ///
    /// If every slot is full, an unlock evicts `FALLBACK_ABILITY_ID` rather
    /// than being dropped: the fallback is explicitly a placeholder for a
    /// companion whose species grants nothing *yet* (see
    /// `install_innate_routines`), so a real innate unlock displacing it is
    /// the placeholder doing its job. This is exactly the shipped Scrapper's
    /// case — its only ability unlocks at level 3, a level-1 tame installs
    /// the fallback into its one slot, and without eviction the level-3
    /// unlock would find that slot "full" and be lost forever.
    ///
    /// Only when every slot instead holds a *real* routine — installed,
    /// researched, or another innate ability — is the unlock logged and
    /// dropped for good: the window it could have been installed in has
    /// passed. No shipped species can reach that genuine-loss state (the
    /// most any declares is two abilities, the latest at level 8, and four
    /// slots exist by then), so this remains mod-safety only.
    pub(crate) fn install_unlocked_routines(
        &mut self,
        entity: Entity,
        from_level: u32,
        to_level: u32,
    ) {
        let reached: Vec<AbilityId> = self
            .world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|s| s.abilities.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.level > from_level && a.level <= to_level)
            .map(|a| a.id)
            .filter(|id| self.world.resource::<AbilityDb>().get(id).is_some())
            .collect();
        if reached.is_empty() {
            return;
        }
        let slots = self.routine_slots(entity);
        let name = self.creature_label(entity);
        for id in reached {
            let mut installed = self
                .world
                .get::<Routines>(entity)
                .map(|r| r.0.clone())
                .unwrap_or_default();
            if installed.contains(&id) {
                continue;
            }
            if installed.len() >= slots {
                if let Some(pos) = installed
                    .iter()
                    .position(|a| a == abilities::FALLBACK_ABILITY_ID)
                {
                    installed[pos] = id;
                    self.world.entity_mut(entity).insert(Routines(installed));
                    continue;
                }
                self.log(format!(
                    "{name} has no free routine slot for {} — the unlock is lost.",
                    self.ability_display_name(&id)
                ));
                continue;
            }
            installed.push(id);
            self.world.entity_mut(entity).insert(Routines(installed));
        }
    }

    /// Every ability the combatant at `entity` can be commanded to use, in
    /// menu order: whatever is installed in its routine slots. Menu and
    /// resolution both go through this, so the two cannot disagree about
    /// what a slot knows.
    ///
    /// May be empty for anyone — a member with nothing installed is offered
    /// no Special at all (see `battle_action_options`). A companion's kit is
    /// installed at tame/fuse time and topped up on the level-ups that reach
    /// a species unlock (`install_innate_routines`,
    /// `install_unlocked_routines`); nothing is resolved here.
    pub(crate) fn actor_abilities(&self, entity: Entity) -> Vec<AbilityDef> {
        let db = self.world.resource::<AbilityDb>();
        self.world
            .get::<Routines>(entity)
            .map(|r| r.0.as_slice())
            .unwrap_or_default()
            .iter()
            .filter_map(|id| db.get(id).cloned())
            .collect()
    }

    /// Consumable items the player is actually holding — the pool
    /// `BattleAction::UseItem` draws from, and what the in-battle item
    /// picker lists. The map's inventory screen is a different flow: there
    /// an item is spent for free, in battle it costs that slot its round.
    pub fn battle_usable_items(&self) -> Vec<ItemId> {
        let player = self.player_entity();
        let db = self.world.resource::<ItemDb>();
        let Some(inv) = self.world.get::<Inventory>(player) else {
            return Vec::new();
        };
        inv.items
            .iter()
            .filter(|(id, count)| {
                *count > 0 && db.get(id.as_str()).is_some_and(|d| d.consume.is_some())
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Why `entity` can't spend `ability` right now, or `None` if it can.
    /// Both reasons are refused in `battle_set_action` too, so a greyed row
    /// can never be planned and silently waste the member's round.
    pub(crate) fn ability_unavailable(
        &self,
        entity: Entity,
        ability: &AbilityDef,
    ) -> Option<String> {
        let remaining = self
            .world
            .get::<AbilityCooldowns>(entity)
            .and_then(|c| c.0.get(&ability.id).copied())
            .unwrap_or(0);
        if remaining > 0 {
            return Some(format!("{remaining} more rounds"));
        }
        // Decompile is refused for two reasons no other ability has. They
        // used to live in `attempt_decompile`, which refunded the round
        // silently; here the row greys with the reason instead.
        if matches!(ability.effect, AbilityEffect::Decompile) {
            if self.taming_catalyst().is_none() {
                return Some("no taming catalyst".to_string());
            }
            if self.pet_count() >= self.pet_capacity() {
                return Some("roster is full".to_string());
            }
        }
        let fatigue = self
            .world
            .get::<Needs>(self.player_entity())
            .map(|n| n.fatigue)
            .unwrap_or(0.0);
        if fatigue < ability.fatigue_cost {
            return Some("not enough Fatigue".to_string());
        }
        None
    }

    /// The abilities party `slot` can choose between for a Special, as
    /// engine-authored menu rows. Both renderers draw these verbatim, same
    /// contract as `battle_action_options` — a species that gains an ability
    /// reaches both without either being touched.
    ///
    /// Empty exactly when `battle_action_options` hides the Special row for
    /// this slot: see `Game::actor_abilities`.
    pub fn battle_special_options(&self, slot: usize) -> Vec<SpecialOption> {
        let Some(entity) = self.actor_entity(battle::Actor::Party(slot)) else {
            return Vec::new();
        };
        self.actor_abilities(entity)
            .into_iter()
            .enumerate()
            .map(|(index, ability)| SpecialOption {
                index,
                name: ability.name.clone(),
                detail: ability.description.clone(),
                targeting: ability.target.targeting(),
                sweeps_party: ability.target == AbilityTarget::WholeParty,
                unavailable: self.ability_unavailable(entity, &ability),
            })
            .collect()
    }

    /// The party members a buff or heal can be aimed at, as engine-authored
    /// menu rows — you and every companion, including ones already planned
    /// this round. Knocked-out members are left out: a buff on a downed
    /// member would be spent for nothing.
    pub fn battle_ally_options(&self) -> Vec<AllyOption> {
        let Some(battle) = self.world.get_resource::<BattleState>() else {
            return Vec::new();
        };
        (0..battle.planned.len())
            .filter_map(|slot| {
                let entity = self.actor_entity(battle::Actor::Party(slot))?;
                let stats = self.world.get::<Stats>(entity)?;
                if stats.hp <= 0 {
                    return None;
                }
                Some(AllyOption {
                    slot,
                    name: if slot == 0 {
                        "You".to_string()
                    } else {
                        self.creature_label(entity)
                    },
                    detail: format!("{}/{} HP", stats.hp, stats.max_hp),
                })
            })
            .collect()
    }

    /// The action menu for party `slot`. This is the single place the
    /// action set is defined; both renderers draw whatever this returns.
    pub fn battle_action_options(&self, slot: usize) -> Vec<ActionOption> {
        let Some(entity) = self.actor_entity(battle::Actor::Party(slot)) else {
            return Vec::new();
        };
        let is_player = slot == 0;
        let mut options = vec![
            ActionOption {
                kind: ActionKind::Attack,
                key: 'a',
                label: "[a]ttack".to_string(),
                detail: "Strike a hostile group".to_string(),
                target: TargetSpec::EnemyGroup,
                unavailable: None,
            },
            ActionOption {
                kind: ActionKind::Defend,
                key: 'd',
                label: "[d]efend".to_string(),
                detail: format!("+{DEFEND_DEF_BONUS} DEF this round, and draw fire"),
                target: TargetSpec::None,
                unavailable: None,
            },
        ];

        // Hidden, not greyed: with routines installable at will, an empty
        // kit is a state the player chose, and a permanently greyed row
        // teaches nothing they don't already know.
        if !self.actor_abilities(entity).is_empty() {
            options.push(ActionOption {
                kind: ActionKind::Special,
                key: 's',
                label: "[s]pecial".to_string(),
                detail: self.ability_label(entity),
                target: TargetSpec::SpecialAbility,
                unavailable: None,
            });
        }

        if is_player {
            options.push(ActionOption {
                kind: ActionKind::UseItem,
                key: 'u',
                label: "[u]se item".to_string(),
                detail: "Spend a consumable".to_string(),
                target: TargetSpec::InventoryItem,
                unavailable: self
                    .battle_usable_items()
                    .is_empty()
                    .then(|| "no usable items".to_string()),
            });
        }

        options
    }

    /// The party-level commands, which apply to every slot at once instead of
    /// to `battle_active_slot`. Kept here rather than as renderer literals so
    /// the two frontends cannot drift — the same reason
    /// `battle_action_options` exists.
    pub fn battle_party_commands(&self) -> Vec<PartyCommand> {
        vec![
            PartyCommand {
                kind: PartyCommandKind::AllAttack,
                key: 'A',
                label: "[A]ll attack".to_string(),
                needs_target: self.living_group_count() > 1,
            },
            PartyCommand {
                kind: PartyCommandKind::AllDefend,
                key: 'D',
                label: "[D] all defend".to_string(),
                needs_target: false,
            },
            PartyCommand {
                kind: PartyCommandKind::JackOut,
                key: 'j',
                label: "[j]ack out".to_string(),
                needs_target: false,
            },
        ]
    }
}
