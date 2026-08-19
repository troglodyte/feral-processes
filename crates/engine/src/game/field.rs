//! Casting a field routine: the abilities that run outside battle rather
//! than being spent as a Special.
//!
//! Three effects reach this path — `AbilityEffect::FieldBuff`, which arms a
//! running buff (see `components::FieldBuff` and `arm_field_buff`), and the
//! two Stack movement routines `Phase` and `Jump`, which move the party
//! through a frame (see `game/stack_movement.rs`). All three are priced in
//! Power through `routine_power_cost` and charged to **the holder**, not to
//! the player — the same rule the battle Special path holds to, so a
//! companion's routine draws on the companion's own reserve at both ends.
//! `AbilityEffect::field_only` is the predicate that says which reach here.

use crate::abilities::routine_power_cost;
use crate::components::{FieldScope, POWER_MIN};
use crate::game::stack::StackPos;
use crate::tuning::{TRACE_PER_JUMP, TRACE_PER_PHASE};
use crate::*;

impl Game {
    /// Every **field-only** ability installed on you or a program you own,
    /// flat across holders. `Game::routine_holders` is the walk over "you,
    /// then every program you own"; this narrows each holder's installed
    /// slots down to the field-castable ones and reshapes them for the
    /// picker.
    ///
    /// `index` into the returned `Vec` is exactly what `cast_field_routine`
    /// takes, and this is the *only* place that list is built — a filtered
    /// view and a cast-time index can never disagree about what a position
    /// means, which is the trap `battle_special_options` fell into before it
    /// started resolving by stable index instead of filtered position. It is
    /// also the only place a row's cost is decided, which is what stops the
    /// list quoting one price and the cast charging another — and, since
    /// each row is gated against **its own holder's** reserve, the only
    /// place that can stop it quoting one bar and charging another.
    pub fn field_routines(&mut self) -> Vec<FieldRoutineView> {
        let holders = self.routine_holders();
        let underground = self.is_underground();
        let db = self.world.resource::<AbilityDb>();
        let mut rows = Vec::new();
        for holder in &holders {
            let Some(installed) = self.world.get::<Routines>(holder.entity) else {
                continue;
            };
            // The holder's own reserve, because the holder is who pays —
            // the mirror of `ability_unavailable`, and of `spend_power`'s
            // entity below. A missing reserve refuses, so a row can never
            // be offered against a bar that cannot be charged.
            let reserve = self
                .world
                .get::<PowerReserve>(holder.entity)
                .copied()
                .unwrap_or(PowerReserve::new(POWER_MIN));
            for id in &installed.0 {
                let Some(def) = db.get(id) else { continue };
                if !def.effect.field_only() || def.is_passive() {
                    continue;
                }
                let cost = routine_power_cost(def);
                let stack_only = matches!(def.effect, AbilityEffect::Phase | AbilityEffect::Jump);
                rows.push(FieldRoutineView {
                    ability: def.id.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    holder: holder.entity,
                    holder_label: holder.name.clone(),
                    cost: format!("{cost:.0} PWR"),
                    // Ordered so the permanent objection is stated ahead of
                    // the temporary one: telling a player on open grid that
                    // they are short of Power would send them to rest for a
                    // routine that was never going to run up here.
                    unavailable: if stack_only && !underground {
                        Some("only in the Stack".into())
                    } else if !reserve.holds(cost) {
                        Some("not enough PWR".to_string())
                    } else {
                        None
                    },
                    second_pick: match def.effect {
                        // `AbilityDb::load_dir`'s field_buff_target_mismatch
                        // check already refuses a Run-scoped FieldBuff any
                        // target but WholeParty, so OneAlly can only appear
                        // here on a Creature-scoped one.
                        AbilityEffect::FieldBuff { .. } if def.target == AbilityTarget::OneAlly => {
                            FieldCastPick::Ally
                        }
                        AbilityEffect::Jump => FieldCastPick::Cell,
                        _ => FieldCastPick::None,
                    },
                });
            }
        }
        rows
    }

    /// Who a `Creature`-scoped field routine can actually land on: you, then
    /// each current `Party` member — exactly the walk `tick_field_buffs` and
    /// `active_buffs` make. `field_routines` above deliberately widens to
    /// "every program you own" because installing/uninstalling a routine on
    /// a benched program is legitimate; casting one is not, since nothing
    /// ticks a buff on an entity that isn't in this narrower set. The ally
    /// picker (`App::field_ally_options`) and its gui row (`draw_field_cast_ally`)
    /// both call this rather than `routine_holders`, and `cast_field_routine`
    /// checks the same set again below so a picker bug can't hand the
    /// engine a target nothing will ever tick.
    ///
    /// `routine_index` is the same index space `cast_field_routine` takes —
    /// the routine about to be cast, which is what decides each row's
    /// `running` tag. An index naming no `FieldBuff` (either Stack movement
    /// routine, or nothing at all) still lists the full roster, untagged:
    /// the roster is a fact about the party and only the tag is a fact about
    /// the pair.
    pub fn field_cast_targets(&mut self, routine_index: usize) -> Vec<FieldCastTargetView> {
        let kind = self.field_routine_buff_kind(routine_index);
        let player = self.player_entity();
        let mut targets = vec![self.field_cast_target_view(player, "You".to_string(), kind)];
        for member in self.world.resource::<Party>().0.clone() {
            let name = self.creature_label(member);
            targets.push(self.field_cast_target_view(member, name, kind));
        }
        targets
    }

    /// What kind of field buff `field_routines()[routine_index]` arms, or
    /// `None` for an index naming a movement routine or nothing.
    fn field_routine_buff_kind(&mut self, routine_index: usize) -> Option<FieldBuffKind> {
        let ability = self.field_routines().get(routine_index)?.ability.clone();
        match self.world.resource::<AbilityDb>().get(&ability)?.effect {
            AbilityEffect::FieldBuff { kind, .. } => Some(kind),
            _ => None,
        }
    }

    /// One ally-picker row: the shared holder derivation, plus the stats the
    /// buff is about to land on and the buff it would displace.
    fn field_cast_target_view(
        &mut self,
        entity: Entity,
        name: String,
        kind: Option<FieldBuffKind>,
    ) -> FieldCastTargetView {
        let holder = self.routine_holder_view(entity, name);
        let (hp, max_hp, atk, def, power) = self
            .world
            .get::<Stats>(entity)
            .map(|s| (s.hp, s.max_hp, s.atk, s.def, s.power()))
            .unwrap_or_default();
        FieldCastTargetView {
            entity: holder.entity,
            glyph: holder.glyph,
            color: holder.color,
            name: holder.name,
            level: holder.level,
            hp,
            max_hp,
            atk,
            def,
            power,
            running: kind.and_then(|kind| self.displaced_field_buff(entity, kind)),
        }
    }

    /// The `Routine`-armed buff of `kind` on `entity` — what
    /// `arm_field_buff` would displace. A `Consumable`-armed one of the same
    /// kind is deliberately not it: that call leaves it running.
    fn displaced_field_buff(&self, entity: Entity, kind: FieldBuffKind) -> Option<RunningBuffView> {
        self.world
            .get::<FieldBuff>(entity)?
            .active
            .iter()
            .find(|b| b.source == BuffSource::Routine && b.kind == kind)
            .map(|b| RunningBuffView {
                name: b.name.clone(),
                magnitude: b.kind.magnitude_label(b.power, b.interval),
                remaining: b.duration_label(),
            })
    }

    /// Whether `entity` is someone a `Creature`-scoped field buff can land
    /// on: the player, or a current `Party` member. The same set
    /// `field_cast_targets` offers and `tick_field_buffs`/`active_buffs`
    /// walk — checked again here because the picker being correct is not a
    /// substitute for the engine enforcing it.
    fn is_field_cast_target(&self, entity: Entity) -> bool {
        entity == self.player_entity() || self.world.resource::<Party>().0.contains(&entity)
    }

    /// Runs `field_routines()[index]` — arming a buff, or moving the party
    /// through a Stack frame.
    ///
    /// Refused during a battle and after game over, like any other map
    /// action — but never by `require_surface`. A `FieldBuff` reaches no
    /// zone-map state, so it works underground exactly as it does on the
    /// surface; and the two movement routines have the *opposite* problem
    /// from the one that guard exists for. They read and write
    /// `Locale::Stack`'s own coordinates rather than a `Position` pinned to
    /// the entrance tile, so what they need is the presence of that locale,
    /// which is `Game::stack_pos` returning `None`.
    ///
    /// The holder pays. Every row was gated against that holder's reserve in
    /// `field_routines` above, so the charge and the refusal read the same
    /// bar — a companion running a party-wide buff spends its own Power even
    /// though a `Run`-scoped kind lands on the player regardless of who cast
    /// it. Rest is what refills a companion's reserve, so the party's field
    /// budget is base-bound exactly as its battle Specials are.
    ///
    /// Every check runs before the first write (the need deduction), so a
    /// refused cast spends nothing and arms nothing: no buff with the cost
    /// unpaid, no cost paid with nothing to show for it, no party moved
    /// through a wall for free.
    ///
    /// A successful cast ticks the clock, the same as `use_item` spending a
    /// turn on a consumable that arms the identical `ActiveFieldBuff`
    /// through the same `arm_field_buff`. Power is renewable and an item is
    /// one-shot, so the item path is already the costlier of the two; not
    /// ticking here would leave a strictly better option on the table for
    /// no reason anyone chose. A refused cast spends nothing and so costs
    /// no time — the tick sits only on the success path.
    pub fn cast_field_routine(
        &mut self,
        index: usize,
        pick: FieldCastTarget,
    ) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let routines = self.field_routines();
        let Some(view) = routines.get(index) else {
            return Err("No such routine.".into());
        };
        if let Some(reason) = &view.unavailable {
            return Err(format!("Can't run {} — {reason}.", view.name));
        }
        let ability_id = view.ability.clone();
        let holder = view.holder;
        let holder_label = view.holder_label.clone();

        let def = self
            .world
            .resource::<AbilityDb>()
            .get(&ability_id)
            .cloned()
            .expect("field_routines only lists abilities AbilityDb actually holds");

        match def.effect {
            AbilityEffect::Phase => return self.cast_phase(holder, &def, pick),
            AbilityEffect::Jump => return self.cast_jump(holder, &def, pick),
            _ => {}
        }

        let player = self.player_entity();
        let AbilityEffect::FieldBuff {
            kind,
            power,
            duration,
            interval,
        } = def.effect
        else {
            unreachable!(
                "field_routines lists only field-only effects, and the two above returned"
            );
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
                    let FieldCastTarget::Ally(target) = pick else {
                        return Err(format!("Choose who to run {} on.", def.name));
                    };
                    if !self.is_field_cast_target(target) {
                        return Err(
                            "That program isn't in your active party — bring it along first."
                                .into(),
                        );
                    }
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
        //
        // `kind.scales_with_caster()` gates this: a percentage-point kind
        // (`Mitigation`, `CaptureBoost`, `XpBoost`, `EncounterDamp`,
        // `DropBoost`) is delivered exactly as authored regardless of who
        // casts it — see that method's doc for why a rate doesn't scale the
        // way a point amount does.
        let magnitude = if kind.scales_with_caster() {
            let level = self.ability_user_level(holder);
            let affinity = self.ability_affinity(holder, &def.effect);
            abilities::scaled_stat_power(power, level, affinity)
        } else {
            power
        };

        self.spend_power(holder, routine_power_cost(&def));
        for entity in recipients {
            self.arm_field_buff(
                entity,
                ActiveFieldBuff {
                    kind,
                    name: def.name.clone(),
                    power: magnitude,
                    remaining: duration,
                    interval,
                    source: BuffSource::Routine,
                },
            );
        }
        self.log(format!("{holder_label} runs {}.", def.name));
        self.tick();
        Ok(())
    }

    /// The Stack position both movement routines need, or the refusal for
    /// being on open grid.
    ///
    /// `field_routines` already greys those rows on the surface, so a player
    /// working the menu never reaches this — it is here because the picker
    /// being correct is not a substitute for the engine enforcing it, the
    /// same call `is_field_cast_target` makes about the ally list.
    fn movement_cast_pos(&self, name: &str) -> Result<StackPos, String> {
        self.stack_pos()
            .ok_or_else(|| format!("{name} only runs inside the Stack."))
    }

    /// Spends `def`'s Power off `caster` and steps the party through one wall.
    ///
    /// `phase_landing` has already applied every refusal by the time
    /// anything below runs, so the Power is spent on a move that is
    /// certain to happen. Trace is raised for the phase itself *before*
    /// `arrive`, so the crossing reads as the consequence of the routine
    /// rather than of whatever the party landed on top of.
    fn cast_phase(
        &mut self,
        caster: Entity,
        def: &AbilityDef,
        pick: FieldCastTarget,
    ) -> Result<(), String> {
        if pick != FieldCastTarget::None {
            return Err(format!("{} takes no target.", def.name));
        }
        let pos = self.movement_cast_pos(&def.name)?;
        let landing = self.phase_landing(pos)?;

        self.spend_power(caster, routine_power_cost(def));
        self.log_kind(
            MessageKind::Outcome,
            "The wall goes soft for exactly as long as it takes to cross it.",
        );
        self.relocate_within_frame(landing);
        self.raise_trace(TRACE_PER_PHASE);
        self.arrive();
        self.tick();
        Ok(())
    }

    /// Spends `def`'s Power off `caster` and moves the party to the cell they pointed
    /// at — or kills them, if that cell turns out to be solid.
    ///
    /// The Power comes off either way: the routine ran, and what it found
    /// at the address is not something the party gets refunded for. Trace
    /// and `arrive` belong only to the arrival — a party that resolved
    /// inside rock has arrived nowhere, and `die_in_the_rock` deliberately
    /// never writes `Locale`, so there is no cell for the arrival tail to
    /// act on.
    fn cast_jump(
        &mut self,
        caster: Entity,
        def: &AbilityDef,
        pick: FieldCastTarget,
    ) -> Result<(), String> {
        let FieldCastTarget::Cell(x, y) = pick else {
            return Err(format!("Choose where to run {} to.", def.name));
        };
        let pos = self.movement_cast_pos(&def.name)?;
        self.jump_refusal(pos, (x, y))?;

        self.spend_power(caster, routine_power_cost(def));
        if self.jump_is_lethal((x, y)) {
            self.die_in_the_rock();
        } else {
            self.log_kind(
                MessageKind::Outcome,
                "You jump to an address nobody validated. It resolves.",
            );
            self.relocate_within_frame((x, y));
            self.raise_trace(TRACE_PER_JUMP);
            self.arrive();
        }
        self.tick();
        Ok(())
    }

    /// Takes `cost` off `entity`'s reserve — **the one write path**, and the
    /// only place a routine's price is charged.
    ///
    /// `entity` rather than the player, because the caster pays: a
    /// companion's Special draws on the companion's own reserve. That single
    /// parameter is the whole of "every companion tracks their power level"
    /// on the spending side.
    ///
    /// **A missing reserve is a no-op**, which is what makes hostiles safe
    /// here without a branch — they hold none by design and are never gated
    /// on one. It is the mirror of `ability_unavailable`, where a missing
    /// reserve *refuses*: nothing may be charged that was never offered, and
    /// nothing may be offered that cannot be charged.
    ///
    /// Every refusal is already behind it at every call site.
    pub(crate) fn spend_power(&mut self, entity: Entity, cost: f32) {
        if let Some(mut reserve) = self.world.get_mut::<PowerReserve>(entity) {
            reserve.spend(cost);
        }
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
                        name: buff.name.clone(),
                        magnitude: buff.kind.magnitude_label(buff.power, buff.interval),
                        remaining: buff.duration_label(),
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
                    // A combat buff has no cadence — it is a flat stat while it
                    // lasts — so it asks for the every-turn tag.
                    magnitude: kind.magnitude_label(active.power, 1),
                    // Rounds, and always a count: `ActiveFieldBuff`'s
                    // until-rest split has no equivalent here, since a combat
                    // buff cannot outlive the fight it was armed in.
                    remaining: format!("{}t", active.remaining),
                    holder_label,
                });
            }
        }
        views
    }
}
