//! Starting a battle and planning a round: pack gathering, initiative, and
//! the action menus the renderer draws from.

use crate::abilities::{AbilityId, AffinityKind};
use crate::tuning::{
    AFFINITY_MAX, AFFINITY_NEUTRAL, DEFAULT_BASE_SPEED, DEFEND_MITIGATION_BONUS, INITIATIVE_DIE,
    PLAYER_BASE_SPEED,
};
use crate::*;

impl Game {
    /// The damage band `entity` actually swings for, given the `natural`
    /// range its move or ability authored.
    ///
    /// **A weapon overrides a natural attack, it does not add to it.** A
    /// companion still rolls a species move each turn for its *name* and its
    /// status rider; an equipped weapon supplies the number. Unarmed, the
    /// move's own range applies. The player has no species moves at all —
    /// their `natural` is `tuning::PLAYER_UNARMED_DAMAGE`.
    ///
    /// The override is keyed on the weapon carrying a range, not on the slot
    /// being occupied: a modded weapon authoring none leaves its wielder
    /// swinging naturally rather than disarmed.
    pub(crate) fn attack_range(
        &self,
        entity: Entity,
        natural: battle::DamageRange,
    ) -> battle::DamageRange {
        let worn = self.gear_bonus(entity).damage;
        if worn == battle::DamageRange::default() {
            natural
        } else {
            worn
        }
    }

    /// What one deterministic swing at a thing that cannot dodge lands:
    /// the band's mean plus `effective_atk`, floored at 1.
    ///
    /// The one definition, shared by `Game::attack_nest` and
    /// `Game::strike_rock` — a nest and a wall of base-space rock are the
    /// two things in the game worn down by bumping into them, and a copy of
    /// this formula in either would let them drift. Neither goes through
    /// `battle::resolve_attack`: a structure has no speed and cannot dodge,
    /// and identical swings have to stay identical or wearing something down
    /// becomes a slot machine.
    ///
    /// The band comes from `natural_range_of`, which is the one conversion
    /// from an entity to what it swings for: a `Creature`'s own species
    /// move, a weapon's band where one is worn, and `PLAYER_UNARMED_DAMAGE`
    /// only as the fallback the player actually falls through to. Naming
    /// the player's fists here instead handed every unarmed crew program the
    /// player's band, so a Scrapper and a Medic dug at the same rate.
    pub(crate) fn swing_damage(&self, attacker: Entity) -> u32 {
        let range = self.natural_range_of(attacker);
        (range.mean().round() as i32 + self.effective_atk(attacker)).max(1) as u32
    }

    /// How many members of one species group may fight — the ceiling a
    /// gathered cluster fights under, and the same value in `gather_pack`
    /// and `group_pack` because it depends only on the zone and the depth.
    ///
    /// It used to take the widest `max_group_size` across every member's
    /// own tile, and had to: group size doubled every fifteen tiles, so a
    /// cluster whose anchor had drifted inward of a step boundary would
    /// field half its members. With zone and depth deciding it there is no
    /// per-tile variation left to take a maximum over, which is why the
    /// members are no longer read at all.
    ///
    /// Trace is folded in here as well as in `spawn_pack`, and it has to be
    /// both: the spawn decides how many bodies exist, this decides how many
    /// of them fight. Scaling only the spawn made `TRACE_GROUP_MULT` a
    /// no-op — the surplus was capped back out here and then swept by
    /// `end_battle`'s `StackSpawn` cleanup, so a Hunted ambush fielded
    /// exactly as many programs as a Quiet one.
    ///
    /// Reading the band off the resource is safe *here*, unlike inside
    /// `spawn_pack`: this is only ever reached from `start_battle`, which is
    /// the player's own fight, and Trace is zero unless they are underground.
    /// `fight_depth` rides in on that same safety — see its doc.
    ///
    /// `pub(crate)` only so `arena` can *warn* that a scenario exceeds it —
    /// the arena builds its groups at the size asked for and this decides
    /// nothing there. Widening it further would make a zone's fight ceiling
    /// look like a public question; it is not.
    pub(crate) fn group_size_ceiling(&self) -> usize {
        let base = self.max_group_size(self.fight_depth());
        let cap = crate::game::spawning::zone_group_cap(self.world.resource::<ZoneLevel>().0);
        crate::game::spawning::trace_group_ceiling(base, self.trace_group_mult(), cap) as usize
    }

    /// The depth the fight being assembled is happening at, or `None` on the
    /// surface. Every member of an underground pack stands on the entrance
    /// tile — the party's `Position` is pinned there — so their tiles report
    /// the base's doorstep and depth is the only thing that can say how far
    /// down this is.
    ///
    /// Safe as a resource read for the same reason `trace_group_mult` is
    /// above: both ceiling helpers are reached only from `group_pack`, and
    /// that is only ever the player's own fight. Inside `spawn_pack` the
    /// identical read would be the documented leak.
    fn fight_depth(&self) -> Option<u32> {
        self.stack_pos().map(|pos| pos.depth)
    }

    /// How many distinct species groups this fight may hold.
    ///
    /// Both ceilings used to scan every member's tile and take the widest,
    /// because the curves were distance-driven and a cluster could straddle
    /// a step boundary. Zone and depth are properties of the fight rather
    /// than of any one tile, so there is nothing left to take the maximum
    /// over.
    ///
    /// `pub(crate)` for `arena`'s warning, on the same terms as
    /// `group_size_ceiling` above.
    pub(crate) fn enemy_group_ceiling(&self) -> usize {
        self.max_enemy_groups(self.fight_depth())
    }

    /// Every alive `Hostile` creature within `swarm_radius` tiles of
    /// `anchor` (Chebyshev distance) — the whole cluster a group spawn
    /// roll placed together (see `try_spawn_habitat_creature`) joins the
    /// fight at once when the player bumps into any one of them. `anchor`
    /// is always first, becoming the initial front target. Truncated to
    /// `enemy_group_ceiling` groups' worth of `group_size_ceiling`, so how deep
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
        let radius = crate::game::spawning::swarm_radius(self.max_group_size(None));
        let mut pack = vec![anchor];
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Hostile>>();
        let sweep: Vec<Entity> = query
            .iter(&self.world)
            .filter(|(e, pos)| {
                *e != anchor
                    && (pos.x - anchor_pos.x)
                        .abs()
                        .max((pos.y - anchor_pos.y).abs())
                        <= radius
            })
            .map(|(e, _)| e)
            .collect();
        // A boss is `is_boss` because it *spawns as its own group*, and past
        // zone 1 it arrives with an escort built for it in `spawn_pack`. It
        // roams the open map like anything else (`try_spawn_habitat_creature`
        // passes `allow_boss: true`), so without this it would be swept into
        // whatever ordinary cluster it happened to be standing near. The
        // anchor is exempt by construction: bumping a boss has to fight it.
        pack.extend(sweep.into_iter().filter(|&e| !self.is_boss_creature(e)));
        pack.truncate(self.group_size_ceiling() * self.enemy_group_ceiling());
        pack
    }

    /// Partitions `pack` into one group per species, in first-appearance
    /// order. A cluster spanning more than `enemy_group_ceiling` species keeps
    /// only its largest groups, and each group is itself capped at
    /// `group_size_ceiling` — a spawn roll places one species, so without
    /// that a single deep roll would fight as one column rather than as the
    /// groups the danger curve allows. Neither surplus is returned: both
    /// stay on the map as ordinary hostiles, met on the next bump.
    ///
    pub(crate) fn group_pack(&self, pack: Vec<Entity>) -> Vec<EnemyGroup> {
        let cap = self.group_size_ceiling();
        let max_groups = self.enemy_group_ceiling();
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

    /// The pack the player bumped into, capped and partitioned into groups
    /// by `group_pack` before the fight opens around it. **The only path
    /// that caps a pack** — `begin_battle`'s other caller, `arena`, wants
    /// the composition it authored rather than the one this zone could
    /// roll, and a third caller that does want capping calls `group_pack`
    /// itself.
    pub(crate) fn start_battle(&mut self, pack: Vec<Entity>) {
        let groups = self.group_pack(pack);
        self.begin_battle(groups);
    }

    /// What one side of a fight weighs, by summed `Stats::power()`.
    ///
    /// The player's own arm is not a special case: `Stats::power` is the one
    /// expression of what a body is worth in a fight, and it reads the same
    /// component on either side of the line. Widened to `i64` because it sums
    /// a whole pack — the summands are `i32` and a deep zone's group can carry
    /// several of them near the top of the range.
    ///
    /// A body with no `Stats` weighs nothing rather than being skipped, which
    /// is the same answer and needs no filter.
    fn summed_power(&self, who: impl Iterator<Item = Entity>) -> i64 {
        who.filter_map(|e| self.world.get::<Stats>(e))
            .map(|s| s.power() as i64)
            .sum()
    }

    /// Opens a battle around `groups` verbatim. Called by `start_battle`,
    /// which caps its pack first, and by `arena`, which does not.
    pub(crate) fn begin_battle(&mut self, groups: Vec<EnemyGroup>) {
        let player = self.player_entity();
        // Neither `CombatBuff` nor `FieldBuff` is touched here — a
        // companion's Rally/Shield left active going into a fight lives in
        // the former, a pre-battle consumable's buff in the latter (see
        // `use_item`'s `prebattle_buff`), and both must carry into the
        // fight they were armed for. `clear_battle_status_effects` is what
        // clears `CombatBuff` once the fight ends; `FieldBuff` outlives it.
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
        // Taken before the groups are moved into the resource, and before the
        // first blow — see `BattleState::outmatched` for why a fight has to
        // weigh itself at the bell rather than at teardown.
        let hostile_weight =
            self.summed_power(groups.iter().flat_map(|g| g.members.iter().copied()));
        let party_weight = self.summed_power(
            std::iter::once(player).chain(self.world.resource::<Party>().0.iter().copied()),
        );
        // Opened before the intercept line below, so that line is the first
        // thing the battle pane shows.
        self.world.resource_mut::<MessageLog>().open_battle();
        self.world.insert_resource(BattleState {
            player,
            round_targets: groups.iter().map(|g| g.members.clone()).collect(),
            groups,
            round: 1,
            planned: vec![None; slots],
            finished: false,
            player_won: false,
            decompile_attempts: HashMap::new(),
            rewards: BattleRewards::default(),
            lair: None,
            outmatched: hostile_weight > party_weight,
        });
        // After `BattleState` is in place, deliberately: the party slots and
        // the groups are both read back off it, so a record taken earlier
        // would describe a fight that does not exist yet.
        let fight = self.next_fight_id();
        self.record(|g| crate::telemetry::Record::FightStart {
            fight,
            seed: g.world.resource::<WorldMap>().seed() as u64,
            zone: g.world.resource::<ZoneLevel>().0,
            depth: g.stack_pos().map(|p| p.depth).unwrap_or(0),
            party: g.telemetry_party(),
            enemies: g.telemetry_enemy_groups(),
        });
        if others > 0 {
            self.log(format!(
                "A pack of rogue programs intercepts your signal — a {name} takes point, {others} more behind it!"
            ));
        } else {
            self.log(format!("A rogue {name} intercepts your signal!"));
        }
        // The first nemesis in the opening groups, group-then-slot order —
        // deterministic, and there is no notion of "the" nemesis when a
        // pack holds two, so picking one rather than logging every one of
        // them is the whole rule. `all_living_enemies` reads `BattleState`,
        // so this has to run after it's inserted above.
        if let Some(taunter) = self
            .all_living_enemies()
            .into_iter()
            .find(|&e| self.world.get::<Nemesis>(e).is_some())
        {
            self.log_nemesis_taunt(taunter);
        }
    }

    /// Logs what `hostile` — already known to carry `Nemesis` — has to say
    /// at the top of this fight, or nothing on an empty taunt bank.
    ///
    /// `MessageKind::Info`, matching `game/taunt.rs`'s player-triggered
    /// counterpart exactly and for the same reason:
    /// `MessageLog::retain_outcomes_since_battle` keeps only `Outcome`,
    /// `Loot`, `LevelUp`, `Raid` and `Complete`, so this line is pruned the
    /// moment the fight ends and never follows the player onto the map.
    fn log_nemesis_taunt(&mut self, hostile: Entity) {
        let Some(nemesis) = self.world.get::<Nemesis>(hostile).copied() else {
            return;
        };
        let Some(species) = self
            .world
            .get::<Creature>(hostile)
            .map(|c| c.species.clone())
        else {
            return;
        };
        let potential = self
            .world
            .get::<Potential>(hostile)
            .copied()
            .unwrap_or(Potential::NEUTRAL);
        // The same seed `name_new_nemesis` derived this program's name
        // from — `NemesisDb::taunt` folds the grudge count on top of it,
        // rather than this call site folding its own, so the fold lives in
        // exactly one place.
        let seed = crate::nemesis::name_seed(&species, &potential);
        let Some(line) = self
            .world
            .resource::<crate::nemesis::NemesisDb>()
            .taunt(seed, nemesis.0)
            .map(str::to_string)
        else {
            return;
        };
        let label = self.creature_label(hostile);
        self.log_kind(MessageKind::Info, format!("{label} {line}"));
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

    /// Summed `Stats::power` of everyone still standing on the player's
    /// side, and of every living enemy. The two inputs to
    /// `battle::jack_out_chance` — kept here beside `all_living_enemies`
    /// because both read the same `BattleState` shape, and split from the
    /// formula itself so the formula stays testable without a `World`.
    pub(crate) fn party_side_power(&self) -> i32 {
        let player = self.player_entity();
        std::iter::once(player)
            .chain(self.world.resource::<Party>().0.iter().copied())
            .filter(|&e| self.creature_alive(e))
            .filter_map(|e| self.world.get::<Stats>(e).map(|s| s.power()))
            .sum()
    }

    pub(crate) fn enemy_side_power(&self) -> i32 {
        self.all_living_enemies()
            .into_iter()
            .filter_map(|e| self.world.get::<Stats>(e).map(|s| s.power()))
            .sum()
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
    /// The speed `entity` fights at — initiative, accuracy and evasion alike.
    ///
    /// **One rule, because the three must not disagree.** The player has no
    /// `Creature` and so no species speed, and `PLAYER_BASE_SPEED` is a shade
    /// above `DEFAULT_BASE_SPEED` deliberately. Initiative used to name that
    /// constant at its own call site while `species_base_speed`'s fallback
    /// handed everything else the default — which left the player acting
    /// first against an average opponent and yet hitting and dodging as
    /// though a shade slower than one.
    pub(crate) fn combat_speed(&self, entity: Entity) -> i32 {
        if entity == self.player_entity() {
            PLAYER_BASE_SPEED
        } else {
            self.species_base_speed(entity)
        }
    }

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
            let base = self.combat_speed(entity);
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
        // charging for it — so they are refused here instead.
        if let BattleAction::Special { ability, target } = &action {
            if let battle::SpecialTarget::Ally { slot: ally } = target
                && *ally >= planned_len
            {
                return Err(format!("Slot {ally} isn't in your party."));
            }
            // `SpecialOption::index` is the stable identity — the position in
            // `actor_abilities` that `ability` names — not `options`' own
            // position, which shifts once a field-only entry is filtered out
            // of the menu. Resolving by that field rather than indexing
            // `options` positionally is what app-core's own consumers already
            // do (`battle_target_title`, `handle_battle_special_key`), and
            // matching that idiom here is what keeps this callsite from
            // silently disagreeing with the menu about which row is which.
            let options = self.battle_special_options(slot);
            let Some(option) = options.iter().find(|o| o.index == *ability) else {
                return Err("That party member has no such ability.".to_string());
            };
            if let Some(reason) = &option.unavailable {
                return Err(format!("That ability isn't ready: {reason}."));
            }
            // A boss is an encounter, never a companion. The durable reason
            // is `growth_multiplier` — 2.0 on both bosses against 1.5 on
            // every ordinary species — so a captured one outgrows the roster
            // it joins however modest its `base_hp` looks at capture, and
            // fusion's `max + min/2` then compounds that.
            //
            // A lair's guardian is refused for a second reason that has
            // nothing to do with what it would be worth in the roster: the
            // stack comes down when the guardian does, and a program walked
            // off with leaves a lair with nothing left to beat standing
            // over a stack that can never be finished. Every shipped
            // guardian is already covered by the boss half — the fallback
            // `pick_lair_species` takes where a biome fields no boss is an
            // ordinary program, and this is the clause that catches it.
            //
            // This can't join Decompile's other two refusals in
            // `ability_unavailable` — that takes no target, because the
            // ability is chosen before the group, so the row cannot grey on
            // something only the target knows. Refusing here costs the player
            // neither the round nor the catalyst.
            if let battle::SpecialTarget::EnemyGroup { group } = target
                && let Some(actor) = self.actor_entity(battle::Actor::Party(slot))
                && matches!(
                    self.actor_abilities(actor).get(*ability).map(|a| &a.effect),
                    Some(AbilityEffect::Decompile)
                )
                && self.front_of_group(*group).is_some_and(|front| {
                    self.is_boss_creature(front) || self.is_lair_guardian(front)
                })
            {
                return Err("That program's ICE is beyond decompiling.".to_string());
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
    ///
    /// A companion's `talents::TalentNode::RoutineSlot` nodes are added here,
    /// in the companion arm only: the player is not a companion and must not
    /// read a companion tree. `abilities::companion_routine_slots` stays a
    /// pure function of level, because several tests and `balance_sim` read it
    /// as one.
    pub fn routine_slots(&self, entity: Entity) -> usize {
        let level = self
            .world
            .get::<Experience>(entity)
            .map(|e| e.level)
            .unwrap_or(1);
        if entity == self.player_entity() {
            abilities::player_routine_slots(level)
        } else {
            abilities::companion_routine_slots(level) + self.talent_routine_slots(entity)
        }
    }

    /// Every ability `entity`'s talents grant. Folded into the `declared` list
    /// both install paths already build, rather than installed by a second path
    /// beside them: a granted routine has to behave *exactly* like a
    /// species-kit unlock — same slot competition, same treatment of what the
    /// program was carrying when it was decompiled — and one list is the only
    /// way to guarantee that.
    fn talent_abilities(&self, entity: Entity) -> Vec<AbilityId> {
        let Some(taken) = self.world.get::<Talents>(entity) else {
            return Vec::new();
        };
        let Some(tree) = self.talent_tree(entity) else {
            return Vec::new();
        };
        tree.tiers
            .iter()
            .flat_map(|tier| tier.0.iter())
            .filter(|choice| taken.0.contains(&choice.id))
            .filter_map(|choice| match &choice.node {
                crate::talents::TalentNode::Ability { id } => Some(id.clone()),
                _ => None,
            })
            .filter(|id| self.world.resource::<AbilityDb>().get(id).is_some())
            .collect()
    }

    /// How many extra routine slots `entity`'s talents have bought.
    fn talent_routine_slots(&self, entity: Entity) -> usize {
        let Some(taken) = self.world.get::<Talents>(entity) else {
            return 0;
        };
        let Some(tree) = self.talent_tree(entity) else {
            return 0;
        };
        tree.tiers
            .iter()
            .flat_map(|tier| tier.0.iter())
            .filter(|choice| {
                taken.0.contains(&choice.id)
                    && matches!(choice.node, crate::talents::TalentNode::RoutineSlot)
            })
            .count()
    }

    /// Installs the kit `entity`'s species grants at its current level,
    /// merged with whatever it was already carrying. Called once when a
    /// program comes into existence — a decompile or a fusion — never
    /// afterwards.
    ///
    /// A wild program can spawn carrying a routine its species never grants
    /// (`Game::roll_wild_routine`); that routine is the reason the player
    /// decompiled it, so it keeps its slot and the species kit fills in
    /// around it. Anything that doesn't fit is lost — see
    /// `install_unlocked_routines` for why there is nowhere for it to go.
    ///
    /// A species declaring no abilities gets `FALLBACK_ABILITY_ID` instead,
    /// which is what keeps an ability-less species commandable and keeps
    /// that ability obtainable by extraction: nothing else grants it. A
    /// carrier never gets it — the fallback fills an empty kit, and a
    /// carrier's is not empty.
    pub(crate) fn install_innate_routines(&mut self, entity: Entity) {
        let level = self
            .world
            .get::<Experience>(entity)
            .map(|e| e.level)
            .unwrap_or(1);
        let slots = self.routine_slots(entity);
        let mut declared: Vec<AbilityId> = self
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
        // After the species kit, so a talent takes the slot the kit left over
        // rather than one the kit needed.
        declared.extend(self.talent_abilities(entity));
        // Whatever this program was already holding is what it was found
        // carrying in the field — see `Game::roll_wild_routine`. That is the
        // prize the player decompiled it for, so it keeps its place and the
        // species kit fills in around it.
        let carried: Vec<AbilityId> = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .unwrap_or_default();

        let mut installed = carried.clone();
        for id in declared {
            if installed.contains(&id) {
                continue;
            }
            if installed.len() >= slots {
                let name = self.creature_label(entity);
                let ability_name = self.ability_display_name(&id);
                self.log(format!(
                    "{name} has no free routine slot for {ability_name} — it is lost."
                ));
                continue;
            }
            installed.push(id);
        }
        // The fallback fills an *empty* kit. A carrier already holds
        // something real, so it never gets the placeholder.
        if installed.is_empty() {
            installed.push(abilities::FALLBACK_ABILITY_ID.to_string());
        }
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
    /// unlock would find that slot "full" and be lost forever. The eviction
    /// is logged (naming both routines) rather than silent, because the
    /// matched slot might just as easily hold a Hyperthread Single v1.0 the player
    /// deliberately installed by hand — the id match can't tell the two
    /// apart, so the player at least gets to read what happened.
    ///
    /// Only when every slot instead holds a *real* routine — installed,
    /// researched, another innate ability, or one the program was found
    /// carrying in the field — is the unlock lost outright. There is nowhere
    /// for it to wait: a routine off a slot is knowledge the player either
    /// has or doesn't, and an innate routine was never taught to them. A
    /// carried routine is never the fallback, so it is never the thing
    /// evicted.
    pub(crate) fn install_unlocked_routines(
        &mut self,
        entity: Entity,
        from_level: u32,
        to_level: u32,
    ) {
        let mut reached: Vec<AbilityId> = self
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
        // A talent's routines ride this list too, so a companion that gained a
        // slot with the same level-up has somewhere to put one. Already-held
        // ids are skipped below, so re-offering them costs nothing.
        reached.extend(self.talent_abilities(entity));
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
                    // Matched by id alone, so this fires the same way
                    // whether that slot holds the auto-installed placeholder
                    // or a Hyperthread Single v1.0 the player chose to install
                    // themselves — there is no stored provenance to tell the
                    // two apart, and inventing one is out of scope. Logging
                    // either way is what keeps eviction from reading as data
                    // loss with no explanation: overwritten in place, not
                    // popped back to inventory the way `uninstall_routine`
                    // would, so it really is gone.
                    let evicted_name = self.ability_display_name(abilities::FALLBACK_ABILITY_ID);
                    let unlock_name = self.ability_display_name(&id);
                    self.log(format!(
                        "{name} swaps out {evicted_name} to make room for {unlock_name} — \
                         {evicted_name} is gone for good."
                    ));
                    installed[pos] = id;
                    self.world.entity_mut(entity).insert(Routines(installed));
                    continue;
                }
                self.log(format!(
                    "{name} has no free routine slot for {} — it is lost.",
                    self.ability_display_name(&id)
                ));
                continue;
            }
            installed.push(id);
            self.world.entity_mut(entity).insert(Routines(installed));
        }
    }

    /// The level an ability's magnitude scales from when `entity` uses it —
    /// see `abilities::scaled_stat_power` and `abilities::scaled_hp_power`.
    ///
    /// The player and companions have `Experience`. Wild programs do not:
    /// they scale by zone and distance instead, so a hostile carrier reads
    /// the current `ZoneLevel`, which is the closest analogue it has and
    /// keeps its routine in step with the fight it turns up in.
    ///
    /// One helper for all three cases deliberately — three call sites each
    /// resolving a level would be three formulas to drift.
    pub(crate) fn ability_user_level(&self, entity: Entity) -> u32 {
        self.world
            .get::<Experience>(entity)
            .map(|e| e.level)
            .unwrap_or_else(|| self.world.resource::<ZoneLevel>().0)
    }

    /// The caster's multiplier for `effect`'s category — the affinity half
    /// of an ability's magnitude, alongside `ability_user_level`'s scale.
    /// Resolved from `actor`, never from a recipient: an affinity is a
    /// property of who casts.
    ///
    /// The player's comes from perks and a companion's from its species, and
    /// the two can never stack — checked by identity against
    /// `player_entity()` rather than by which components `actor` happens to
    /// carry, so the no-stacking property holds by construction rather than
    /// by the player having no `Creature` and a companion having no
    /// `Perks` staying true by convention. A wild program has a `Creature`
    /// like any other, which is how a species affinity reaches a hostile
    /// carrier for free.
    ///
    /// The perk arm is clamped at `AFFINITY_MAX`, the same ceiling a species
    /// file is clamped to at load (`Affinities::clamp_all`): perk levels are
    /// uncapped, so without this a long enough game would let the player's
    /// own casts exceed the bound `tuning.rs` reasons about everywhere else.
    /// `.min` rather than `.clamp`, because the perk arithmetic — a `u32`
    /// level times a finite constant — cannot produce NaN or undershoot
    /// `AFFINITY_MIN`, so there is no lower bound to express.
    pub(crate) fn ability_affinity(&self, actor: Entity, effect: &AbilityEffect) -> f32 {
        let Some(kind) = effect.affinity_kind() else {
            return AFFINITY_NEUTRAL;
        };
        if actor == self.player_entity() {
            let affinity = AFFINITY_NEUTRAL
                + kind.perk_bonus_per_level() * self.player_perk_level(kind.perk()) as f32;
            return affinity.min(AFFINITY_MAX);
        }
        let species = self
            .world
            .get::<Creature>(actor)
            .and_then(|c| self.species_affinities(&c.species))
            .map(|a| a.get(kind))
            .unwrap_or(AFFINITY_NEUTRAL);
        // Talents are the *companion's* axis, in the creature arm only: perks
        // are the player's, and the two never stack. Clamped the same way the
        // perk arm above is, and for the same reason — a mod's tree may author
        // any magnitude it likes, and `tuning.rs` reasons about `AFFINITY_MAX`
        // everywhere else.
        (species * self.talent_affinity_mult(actor, kind)).min(AFFINITY_MAX)
    }

    /// The product of every `TalentNode::Affinity` `actor` has taken for
    /// `kind` — `AFFINITY_NEUTRAL` when it has none, so the arm above is one
    /// expression either way.
    fn talent_affinity_mult(&self, actor: Entity, kind: AffinityKind) -> f32 {
        let Some(taken) = self.world.get::<Talents>(actor) else {
            return AFFINITY_NEUTRAL;
        };
        let Some(tree) = self.talent_tree(actor) else {
            return AFFINITY_NEUTRAL;
        };
        tree.tiers
            .iter()
            .flat_map(|tier| tier.0.iter())
            .filter(|choice| taken.0.contains(&choice.id))
            .filter_map(|choice| match &choice.node {
                crate::talents::TalentNode::Affinity { kind: k, mult } if *k == kind => Some(*mult),
                _ => None,
            })
            .product::<f32>()
            * AFFINITY_NEUTRAL
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

    /// The routines a program wielded as a weapon could actually fire —
    /// what `party_member_attacks`'s proc rolls from. One predicate, so the
    /// roll and any screen that wants to preview it cannot disagree.
    ///
    /// `actor_abilities` minus two exclusions. A field-only effect has
    /// nothing to resolve against a battle recipient, and that is the
    /// existing `AbilityEffect::field_only` rather than a fourth spelling of
    /// its three variants. `Decompile` is excluded because a free capture
    /// roll on every attack would spend an ICE Breaker the player never
    /// authorised, undercutting taming as something earned by fighting — and
    /// it is resolved by group index rather than by recipient, so it would
    /// not survive the `use_ability` path anyway.
    ///
    /// May be empty, and that is a legitimate outcome rather than an error:
    /// every tamed program has innate routines, but nothing guarantees any
    /// of them are battle-legal. Such a program simply never procs.
    pub(crate) fn wieldable_routines(&self, entity: Entity) -> Vec<AbilityDef> {
        self.actor_abilities(entity)
            .into_iter()
            .filter(|d| !d.effect.field_only())
            .filter(|d| !matches!(d.effect, AbilityEffect::Decompile))
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
    /// Every reason is refused in `battle_set_action` too, so a greyed row
    /// can never be planned and silently waste the member's round.
    ///
    /// **The reserve is read off `entity`, and that is the whole of what
    /// makes companion reserves work.** The caster pays, so asking the entity
    /// being enquired about — rather than assuming the player — gives a
    /// companion's Special its own budget with no second code path. A routine
    /// is priced in a cooldown *and* a Power cost: the cooldown says "not
    /// again yet", the reserve says "not any more".
    ///
    /// **A missing `PowerReserve` refuses rather than permits.** Hostiles
    /// hold none by design, and they never reach here — `choose_wild_action`
    /// picks their moves. Between a companion that cannot cast because a
    /// roster door skipped `roster_parts`, and one with silently unlimited
    /// Power, the first is the failure that gets reported.
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
        let cost = abilities::routine_power_cost(ability);
        if cost > 0.0
            && !self
                .world
                .get::<PowerReserve>(entity)
                .is_some_and(|r| r.holds(cost))
        {
            return Some(format!("needs {cost:.0} PWR"));
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
        None
    }

    /// The abilities party `slot` can choose between for a Special, as
    /// engine-authored menu rows. Both renderers draw these verbatim, same
    /// contract as `battle_action_options` — a species that gains an ability
    /// reaches both without either being touched.
    ///
    /// Empty exactly when `battle_action_options` hides the Special row for
    /// this slot: see `Game::actor_abilities`.
    ///
    /// A field-only effect is filtered out — see `AbilityEffect::field_only`;
    /// none of them has a battle mechanic to run — but only *after*
    /// `enumerate`, so `index` still names its true position in
    /// `actor_abilities`. Filtering first would renumber every row after a
    /// dropped one, and `battle_set_action` resolves `index` straight back
    /// against the unfiltered list.
    pub fn battle_special_options(&self, slot: usize) -> Vec<SpecialOption> {
        let Some(entity) = self.actor_entity(battle::Actor::Party(slot)) else {
            return Vec::new();
        };
        self.actor_abilities(entity)
            .into_iter()
            .enumerate()
            .filter(|(_, ability)| !ability.effect.field_only() && !ability.is_passive())
            .map(|(index, ability)| SpecialOption {
                index,
                name: ability.name.clone(),
                detail: ability.description.clone(),
                targeting: ability.target.targeting(),
                sweeps_party: ability.target == AbilityTarget::WholeParty,
                unavailable: self.ability_unavailable(entity, &ability),
                cooldown: ability.cooldown,
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
                detail: format!("+{DEFEND_MITIGATION_BONUS}% mitigation this round, and draw fire"),
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
