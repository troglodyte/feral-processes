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
    ///
    /// A successful cast ticks the clock, the same as `use_item` spending a
    /// turn on a consumable that arms the identical `ActiveFieldBuff`
    /// through the same `arm_field_buff`. Power is renewable and an item is
    /// one-shot, so the item path is already the costlier of the two; not
    /// ticking here would leave a strictly better option on the table for
    /// no reason anyone chose. A refused cast spends nothing and so costs
    /// no time — the tick sits only on the success path, after the buff is
    /// armed.
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
        self.tick();
        Ok(())
    }

    /// Every buff currently running on the player or a party member, for the
    /// map's buff list and the battle roster alike. Reads `FieldBuff` (which
    /// outlives a battle) and `CombatBuff` (armed only during one, e.g. a
    /// brace) off the same holders with no branching on whether a battle is
    /// active — outside one, `CombatBuff` is simply empty, so the list is
    /// field buffs only, exactly the shape `Game::message_history` and
    /// `Game::structure_report` use to keep a screen's row-shaping in the
    /// engine rather than split across it and the renderer.
    ///
    /// Ordered player first, then party members in `Party`'s order — the
    /// same convention `PartySlotView` uses for the roster, so this list and
    /// that one agree on "whose row is whose" without either side having to
    /// think about it. Within one holder, `FieldBuff::active`'s own order
    /// comes first, then any `CombatBuff`; both are small enough that this
    /// is just "stable and explainable" rather than a meaningful priority.
    pub fn active_buffs(&mut self) -> Vec<ActiveBuffView> {
        let player = self.player_entity();
        let mut holders = vec![player];
        holders.extend(self.world.resource::<Party>().0.clone());

        let mut views = Vec::new();
        for holder in holders {
            let holder_label = (holder != player).then(|| self.creature_label(holder));

            if let Some(field) = self.world.get::<FieldBuff>(holder) {
                for buff in field.active.clone() {
                    views.push(ActiveBuffView {
                        name: buff.name,
                        magnitude: buff.kind.magnitude_label(buff.power),
                        remaining: buff.remaining,
                        holder_label: holder_label.clone(),
                    });
                }
            }

            if let Some(active) = self.world.get::<CombatBuff>(holder).and_then(|b| b.active) {
                // `CombatBuff` carries no cast-time name like `FieldBuff`
                // does, only which stat it moves — `Def`/`Atk` share the
                // same two `FieldBuffKind` variants, so the tag is built by
                // the one function that already owns that format rather
                // than a second copy of it here.
                let (name, kind) = match active.kind {
                    BuffKind::Atk => ("Attack", FieldBuffKind::Atk),
                    BuffKind::Def => ("Defense", FieldBuffKind::Def),
                };
                views.push(ActiveBuffView {
                    name: name.to_string(),
                    magnitude: kind.magnitude_label(active.power),
                    remaining: active.remaining,
                    holder_label,
                });
            }
        }
        views
    }
}
