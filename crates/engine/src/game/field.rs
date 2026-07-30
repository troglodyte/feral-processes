//! Casting a field routine: spending the Power an `AbilityEffect::FieldBuff`
//! costs outside battle to arm it on whoever it lands on. See
//! `components::FieldBuff` for the buff itself and `arm_field_buff` for how
//! it gets installed.

use crate::components::{FieldScope, NEED_MIN};
use crate::*;

impl Game {
    /// Every `FieldBuff` ability installed on you or a program you own, flat
    /// across holders. `Game::routine_holders` is the walk over "you, then
    /// every program you own"; this narrows each holder's installed slots
    /// down to the field-only ones and reshapes them for the picker.
    ///
    /// `index` into the returned `Vec` is exactly what `cast_field_routine`
    /// takes, and this is the *only* place that list is built — a filtered
    /// view and a cast-time index can never disagree about what a position
    /// means, which is the trap `battle_special_options` fell into before it
    /// started resolving by stable index instead of filtered position.
    pub fn field_routines(&mut self) -> Vec<FieldRoutineView> {
        let holders = self.routine_holders();
        let player = self.player_entity();
        let hunger = self
            .world
            .get::<Needs>(player)
            .map(|n| n.hunger)
            .unwrap_or(0.0);
        let db = self.world.resource::<AbilityDb>();
        let mut rows = Vec::new();
        for holder in &holders {
            let Some(installed) = self.world.get::<Routines>(holder.entity) else {
                continue;
            };
            for id in &installed.0 {
                let Some(def) = db.get(id) else { continue };
                let AbilityEffect::FieldBuff { power_cost, .. } = &def.effect else {
                    continue;
                };
                rows.push(FieldRoutineView {
                    ability: def.id.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    holder: holder.entity,
                    holder_label: holder.name.clone(),
                    power_cost: *power_cost,
                    affordable: hunger >= *power_cost,
                    // `AbilityDb::load_dir`'s field_buff_target_mismatch check
                    // already refuses a Run-scoped FieldBuff any target but
                    // WholeParty, so OneAlly can only appear here on a
                    // Creature-scoped one.
                    needs_ally_target: def.target == AbilityTarget::OneAlly,
                });
            }
        }
        rows
    }

    /// Spends `field_routines()[index]`'s Power cost and arms its buff.
    ///
    /// Refused during a battle and after game over, like any other map
    /// action — but never by `require_surface`: casting reaches no zone-map
    /// state, so it works underground exactly as it does on the surface.
    ///
    /// Every check runs before the first write (the Power deduction), so a
    /// refused cast leaves both Power and every `FieldBuff` untouched: no
    /// buff armed with the cost unpaid, no cost paid with nothing armed.
    pub fn cast_field_routine(
        &mut self,
        index: usize,
        target: Option<Entity>,
    ) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let routines = self.field_routines();
        let Some(view) = routines.get(index) else {
            return Err("No such routine.".into());
        };
        let player = self.player_entity();
        let hunger = self.world.get::<Needs>(player).unwrap().hunger;
        if hunger < view.power_cost {
            return Err(format!("Not enough Power to run {}.", view.name));
        }
        let ability_id = view.ability.clone();
        let holder = view.holder;
        let holder_label = view.holder_label.clone();
        let power_cost = view.power_cost;

        let def = self
            .world
            .resource::<AbilityDb>()
            .get(&ability_id)
            .cloned()
            .expect("field_routines only lists abilities AbilityDb actually holds");
        let AbilityEffect::FieldBuff {
            kind,
            power,
            duration,
            ..
        } = def.effect
        else {
            unreachable!("field_routines only lists AbilityEffect::FieldBuff abilities");
        };

        let recipients: Vec<Entity> = match kind.scope() {
            // `Run`-scoped kinds are pressure/economy knobs the whole run
            // feels, not a single combatant's stats — see
            // `FieldBuffKind::scope`. They always land on the player, even
            // when a companion is the one holding (and paying to run) the
            // routine.
            FieldScope::Run => vec![player],
            FieldScope::Creature => match def.target {
                AbilityTarget::OneAlly => {
                    let Some(target) = target else {
                        return Err(format!("Choose who to run {} on.", def.name));
                    };
                    if !self.creature_alive(target) {
                        return Err("That program isn't there anymore.".into());
                    }
                    vec![target]
                }
                AbilityTarget::WholeParty => std::iter::once(player)
                    .chain(self.world.resource::<Party>().0.clone())
                    .filter(|&e| self.creature_alive(e))
                    .collect(),
                _ => unreachable!(
                    "AbilityDb::load_dir's field_buff_target_mismatch check refuses a \
                     Creature-scoped FieldBuff targeting anything but OneAlly or WholeParty"
                ),
            },
        };

        // The holder is the caster: level and affinity are read off whoever
        // runs the routine, not the player, so the same routine lands
        // stronger off a levelled companion than fresh off capture. The
        // scaled value is what's stored on the buff (not the authored one),
        // so a later level-up doesn't retroactively change it.
        let level = self.ability_user_level(holder);
        let affinity = self.ability_affinity(holder, &def.effect);
        let magnitude = abilities::scaled_power(power, level, affinity);

        {
            let mut needs = self.world.get_mut::<Needs>(player).unwrap();
            needs.hunger = (needs.hunger - power_cost).max(NEED_MIN);
        }
        for entity in recipients {
            self.arm_field_buff(
                entity,
                ActiveFieldBuff {
                    kind,
                    name: def.name.clone(),
                    power: magnitude,
                    remaining: duration,
                    source: BuffSource::Routine,
                },
            );
        }
        self.log(format!("{holder_label} runs {}.", def.name));
        Ok(())
    }
}
