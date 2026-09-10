//! Getting out of a battle — by jacking out, or because it is over.
//!
//! Both routes funnel through `end_battle`, which is the only place
//! `BattleState` is dropped. The deferred reap it performs is the reason it
//! exists at all: `BattleState::planned` indexes `Party` positionally, so a
//! member killed mid-fight cannot leave the roster until the fight does.

use crate::resources::LairFight;
use crate::tactical::TacticalBattle;
use crate::tuning::{FLEE_COUNTERATTACK_CHANCE, JACK_OUT_LUCK_MAX, JACK_OUT_LUCK_MIN, MAX_NEMESES};
use crate::*;

/// How a fight ended, in the terms `Game::finish_fight` needs and no others.
///
/// **What each combat model has to answer, rather than what either one
/// stores.** The teardown used to read `BattleState` in six places — four of
/// them asking the same question in slightly different words — which is the
/// abstract model's vocabulary and could not be handed a fight fought on a
/// battle map. A model states its ending once, here.
///
/// There is no `rewards` field: `Game::settle_rewards` drains the tally
/// through `Game::fight_rewards_mut`, which already answers for either
/// model, and it runs while the resource is still standing.
pub(crate) struct FightVerdict {
    /// Whether the party emptied the hostile roster. Not "is anything alive"
    /// — see `Game::all_living_enemies`.
    pub(crate) won: bool,
    /// How many rounds it took, for the results header and the telemetry.
    pub(crate) rounds: u32,
    /// Whether the hostiles outweighed the party at the bell, which
    /// `Game::form_victory_memories` reads for the `hard_won` tag.
    pub(crate) outmatched: bool,
    /// The Stack lair this fight was roused from, if it was one.
    pub(crate) lair: Option<LairFight>,
}

impl Game {
    /// Attempts to jack out, returning whether the party actually got clear.
    ///
    /// The escape is a roll, not a given — `battle::jack_out_chance` weighs
    /// your side's summed power against the pack's, times a luck draw. A
    /// failed attempt burns the round: every engaged group swings, the
    /// round counter advances and end-of-round upkeep runs, but no XP is
    /// docked. You pay the setback only for an escape you actually got,
    /// which is what stops repeated attempts from bleeding progression on
    /// top of HP.
    pub fn battle_flee(&mut self) -> bool {
        if self.is_game_over().is_some() {
            return false;
        }
        let Some(player) = self.world.get_resource::<BattleState>().map(|b| b.player) else {
            return false;
        };
        // The sixth emission site, and the only `ActionKind` with no
        // `BattleAction` behind it: jacking out is its own entry point, not
        // a planned action `resolve_one_action` ever sees. Recorded after
        // the two refusals above and before the roll, so what is in the file
        // is an attempt actually made — whether it got clear is the
        // `fight_end` that follows, or its absence.
        let fight = self.fight_id();
        let round = self.telemetry_round();
        self.record(|g| crate::telemetry::Record::PartyAction {
            fight,
            round,
            slot: 0,
            actor: g.telemetry_actor_label(0, player),
            kind: crate::telemetry::ActionKind::Flee,
            name: None,
            target_slot: None,
        });
        let luck = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(JACK_OUT_LUCK_MIN..=JACK_OUT_LUCK_MAX)
        };
        let chance =
            battle::jack_out_chance(self.party_side_power(), self.enemy_side_power(), luck);
        let escaped = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance)
        };
        if !escaped {
            self.log("The exit route collapses — they're still on you!");
            self.all_wild_retaliate(player);
            // The attempt cost the whole party its round, so the fight
            // advances exactly as a resolved round does — same upkeep, same
            // counter. `tick_round_status_effects` is also what ends the
            // battle if that volley flatlined the player.
            if let Some(mut battle) = self.world.get_resource_mut::<BattleState>() {
                battle.round += 1;
                let slots = battle.planned.len();
                battle.planned = vec![None; slots];
            }
            self.tick_round_status_effects(player);
            self.tick();
            return false;
        }
        let got_hit = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(FLEE_COUNTERATTACK_CHANCE)
        };
        if got_hit {
            self.log_kind(
                MessageKind::Outcome,
                "You jack out, but not before taking a parting counter-strike!",
            );
            self.all_wild_retaliate(player);
        } else {
            self.log_kind(MessageKind::Outcome, "You jack out safely.");
        }
        // A forced jack-out costs a little progress too — nothing drastic,
        // same mild setback as a flatline (see `death_handling_system`).
        if let Some(mut exp) = self.world.get_mut::<Experience>(player) {
            let xp_lost = progression::apply_setback_xp_penalty(&mut exp);
            if xp_lost > 0 {
                self.log_kind(
                    MessageKind::Outcome,
                    format!("Bailing out costs you {xp_lost} XP."),
                );
            }
        }
        // Collected before `end_battle` drops `BattleState` — this is the
        // pack that was actually in the fight, not every pursuer in the
        // zone. Only these lose `Pursuing`; a guardian still walking in
        // from elsewhere keeps chasing.
        let battle_members: Vec<Entity> = self
            .world
            .resource::<BattleState>()
            .groups
            .iter()
            .flat_map(|g| g.members.iter().copied())
            .collect();
        let front = self.front_of_group(0);
        self.end_battle(player, front);
        // A successful jack-out shakes the pack that caught you: without
        // this, `pursuit_tick` (inside the `tick` below) would find the
        // same guardians still adjacent and still `Pursuing`, and
        // re-engage before the player's next input ever arrived — under
        // permadeath, every attempt to leave would cost the XP setback
        // above for nothing. `NestGuardian` is untouched, so a cleared
        // guardian stays tethered and resumes ordinary wandering, exactly
        // like a `despawn_nest` survivor; the nest re-provokes it the next
        // time `attack_nest` lands a hit. A failed attempt (the branch
        // above) shakes nobody.
        for member in battle_members {
            if let Ok(mut entity) = self.world.get_entity_mut(member) {
                entity.remove::<Pursuing>();
            }
        }
        self.tick();
        true
    }

    /// Clears any residual status effects, combat buffs, cloaks and ability
    /// cooldowns from the player, every party member, and every hostile
    /// still in the fight. Status conditions are scoped to a single
    /// intrusion, so nothing should carry forward once one ends, however it
    /// ends. `wild` is `None` when the pack is already gone, and may name an
    /// entity that has already left its group (a decompile) or already
    /// despawned (a kill) and so isn't reachable through
    /// `all_living_enemies` — in which case clearing it again is a no-op,
    /// but neither case may skip clearing your own side.
    pub(crate) fn clear_battle_status_effects(&mut self, player: Entity, wild: Option<Entity>) {
        if let Some(mut s) = self.world.get_mut::<StatusEffects>(player) {
            s.active = None;
        }
        if let Some(mut b) = self.world.get_mut::<CombatBuff>(player) {
            b.active = None;
        }
        if let Some(mut c) = self.world.get_mut::<AbilityCooldowns>(player) {
            c.0.clear();
        }
        // `Cloaked` is battle-scoped exactly as the two above are, which is
        // what keeps it out of `save.rs` — left set, it would follow the
        // player out of the fight and hide them from the next one.
        self.uncloak(player);
        // Every hostile still in the fight, not only the one passed in.
        // Survivors of a jack-out stay on the map, and a mirrored buff left
        // armed on one never ticks down — `effective_atk`/`effective_mitigation`
        // read `CombatBuff` unconditionally, so it would be a free stat
        // forever. `wild` is still taken because it may name a program that
        // has already left its group (a successful decompile).
        let mut hostiles: Vec<Entity> = self.all_living_enemies();
        hostiles.extend(wild);
        for hostile in hostiles {
            if let Some(mut s) = self.world.get_mut::<StatusEffects>(hostile) {
                s.active = None;
            }
            if let Some(mut b) = self.world.get_mut::<CombatBuff>(hostile) {
                b.active = None;
            }
            if let Some(mut c) = self.world.get_mut::<AbilityCooldowns>(hostile) {
                c.0.clear();
            }
            self.uncloak(hostile);
        }
        let party = self.world.resource::<Party>().0.clone();
        for companion in party {
            if let Some(mut s) = self.world.get_mut::<StatusEffects>(companion) {
                s.active = None;
            }
            // Companions hold `CombatBuff` too, now that a Rally or Shield
            // can be aimed at one. Left set, it never ticks down outside a
            // battle and `effective_mitigation`/`effective_atk` read it
            // unconditionally, so it would be a permanent free stat.
            if let Some(mut b) = self.world.get_mut::<CombatBuff>(companion) {
                b.active = None;
            }
            if let Some(mut c) = self.world.get_mut::<AbilityCooldowns>(companion) {
                c.0.clear();
            }
            self.uncloak(companion);
        }
    }

    /// Drops `entity`'s cloak with no line and no transition check — the
    /// teardown's half, where `Game::break_cloak` is the *action's* half. A
    /// fight ending is not a reveal, and announcing one per body would put
    /// three lines under every won fight.
    ///
    /// `get_entity_mut` rather than `entity_mut`: teardown is reached with
    /// entities that have already despawned (a kill) or left their group (a
    /// decompile), and clearing one of those again has to stay a no-op.
    fn uncloak(&mut self, entity: Entity) {
        if let Ok(mut body) = self.world.get_entity_mut(entity) {
            body.remove::<Cloaked>();
        }
    }

    /// Deletes the finished fight's blow-by-blow, keeping only its results.
    ///
    /// **Called when the player leaves the results screen, never from
    /// `end_battle`.** The prune used to run inside `end_battle`, which runs
    /// inside `battle_resolve_round` — so the decisive round's narration was
    /// deleted before a frontend had revealed a single line of it, and the
    /// fight read as jumping from the kill straight to the salvage. Deferring
    /// it is what puts the final blows, the outcome, the salvage and the XP
    /// on screen in that order.
    ///
    /// `Mode::BattleResult` has exactly one key handler and nothing ticks
    /// there, so app-core has one exit to call this from. Miss it and the
    /// blow-by-blow follows the player onto the map, which is the whole thing
    /// the prune exists to stop.
    pub fn prune_battle_narration(&mut self) {
        self.world
            .resource_mut::<MessageLog>()
            .retain_outcomes_since_battle();
    }

    /// Tears the current battle down: every combat-only effect cleared from
    /// both sides, companions killed during the fight finally reaped, and
    /// `BattleState` dropped.
    ///
    /// Reaping the dead happens here rather than the moment they fall
    /// because `BattleState::planned` indexes `Party` positionally (see
    /// `actor_entity`) — removing a member mid-battle shifts every member
    /// behind it into the wrong slot. The death itself is announced when it
    /// happens, in `apply_damage`; only the despawn waits.
    ///
    /// `wild` is passed in rather than looked up because two of the callers
    /// have already popped the group: the entity whose status must be
    /// cleared is the one that just died or was decompiled, not whatever
    /// stepped up behind it. A freshly tamed program joining the party still
    /// Bleeding is the bug this guards.
    pub(crate) fn end_battle(&mut self, player: Entity, wild: Option<Entity>) {
        let verdict = {
            let battle = self.world.resource::<BattleState>();
            FightVerdict {
                // **Emptied, not "nothing alive".** A jack-out taken with
                // every hostile at zero HP and not yet reaped has hostiles
                // that are dead and a fight that was not won, and only the
                // roster can tell the two apart.
                won: battle.groups.is_empty(),
                rounds: battle.round,
                outmatched: battle.outmatched,
                lair: battle.lair,
            }
        };
        self.finish_fight(player, wild, verdict);
    }

    /// Tears a finished fight down, whichever model was holding it.
    ///
    /// **The one ending.** Both combat models come through here, and a
    /// `FightVerdict` is the whole of what they answer it with — everything
    /// below this line is the same sequence in the same order, because the
    /// order is the thing that would drift. `end_battle` is the abstract
    /// model's verdict-builder and nothing more.
    ///
    /// The two fight resources are removed together rather than by a branch:
    /// they are never both present, so "drop whichever held this fight" is
    /// one statement per model and no decision at all.
    pub(crate) fn finish_fight(
        &mut self,
        player: Entity,
        wild: Option<Entity>,
        verdict: FightVerdict,
    ) {
        // Before the closing capture and everything after it: the tally has
        // to be written while the dead are still nameable and while the
        // prune at the bottom is still ahead of it. `settle_rewards` carries
        // the full argument.
        self.settle_rewards(verdict.won);
        // First of the teardown proper, deliberately: `dissolve_tamed_program` below drops the dead
        // out of `Party` and despawns them, and a companion that died
        // winning the fight is the one thing the results page most needs to
        // report. A copy, not a live read — the entities are gone by the
        // time anything draws it.
        let closing = self.closing_rows().map(|(groups, party)| ClosingRoster {
            groups,
            party,
            round: verdict.rounds,
            player_decompiler: self.player_decompiler_bonuses().skill,
        });
        self.world.resource_mut::<BattleTimeline>().closing = closing;
        // Beside the `closing` capture and for the same reason: the reap
        // below drops the dead out of `Party` and despawns them, so this is
        // the last moment `companions_downed` can be counted at all.
        //
        // "Won" is read off the enemies, never off the player — a defeat is
        // absorbed inside the round that lands it by
        // `difficulty::death_handling_system`, which in Forgiving reboots the
        // player, so their HP afterwards says nothing about the outcome.
        // `finish_member` only reaches here once `remove_member` has emptied
        // the last group, which is what makes an empty roster the win.
        let fight = self.fight_id();
        self.record(|g| crate::telemetry::Record::FightEnd {
            fight,
            rounds: verdict.rounds,
            won: verdict.won,
            player_hp_frac: g
                .world
                .get::<Stats>(player)
                .map(|s| s.hp_fraction())
                .unwrap_or(0.0),
            companions_downed: g
                .world
                .resource::<Party>()
                .0
                .iter()
                .filter(|&&e| !g.creature_alive(e))
                .count() as u32,
        });
        self.clear_battle_status_effects(player, wild);
        let dead: Vec<Entity> = self
            .world
            .resource::<Party>()
            .0
            .iter()
            .copied()
            .filter(|&e| !self.creature_alive(e))
            .collect();
        // The detachment lines `dissolve_tamed_program` writes are `Info`
        // kind, so `prune_battle_narration` is still what keeps them off the
        // map and leaves the `Outcome` death line to reach it alone. They do
        // now scroll past on the results screen, where they used to be
        // deleted before anything could reveal them — a dead companion
        // reads its death line and then its detachment.
        for program in dead {
            self.bench_or_dissolve(program);
        }
        // A Stack pack that outlived the fight — the party jacked out —
        // has nowhere to go: it stands at surface coordinates around the
        // link mouth, and would be waiting there when they climb back out.
        //
        // `Without<Tamed>` is load-bearing, not defensive: decompiling one of
        // these mid-fight makes it the player's, and sweeping it up with the
        // rest would delete a program they just earned.
        let strays: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, (With<StackSpawn>, Without<Tamed>)>();
            query.iter(&self.world).collect()
        };
        for stray in strays {
            self.world.despawn(stray);
        }
        // Below the stray sweep and above `BattleState`'s removal a few
        // lines down — see `mark_nemeses`'s own doc for why that window is
        // narrow rather than a preference.
        self.mark_nemeses(verdict.won);
        // Beside `mark_nemeses` and inside the same window, because the two
        // are the same event read from opposite ends: what a fight the party
        // lost leaves standing, and what a fight it won leaves in the
        // survivors. Both need `BattleState`, which goes a few lines down.
        self.form_victory_memories(verdict.won, verdict.outmatched);
        // Deliberately *not* pruned here — see `prune_battle_narration`.
        // The decisive round has not been revealed yet at this point, so
        // deleting it now is deleting it before anyone can read it.
        //
        // The frames are still dropped: they index a roster that is about to
        // go with `BattleState`, and `App::battle_view` answers from
        // `BattleTimeline::closing` for the whole of `Mode::BattleResult`.
        self.world.resource_mut::<BattleTimeline>().frames.clear();
        self.world.remove_resource::<BattleState>();
        self.world.remove_resource::<TacticalBattle>();
        // Last of the teardown, and below the prune deliberately: the
        // collapse rewrites the locale and the sector's links, which is the
        // fight's consequence rather than part of it, and its lines are
        // pushed after the prune has run so they reach the map whatever kind
        // they carry.
        //
        // The guardian going down is what this asks about, not the fight
        // being won: a party that put the lair's own program down and then
        // ran from its escort has still finished the stack.
        if let Some(lair) = verdict.lair
            && self.lair_cleared(lair.pos)
        {
            self.collapse_stack(lair.pos.entrance);
        }
    }

    /// Marks every living hostile still in the fight when it tears down —
    /// the trigger a jack-out and a Forgiving defeat share, since both leave
    /// `BattleState::groups` non-empty exactly like the telemetry `won` field
    /// above reads it. A fight that emptied every group marks nobody.
    ///
    /// A Stack fight's survivors are already gone by the time this runs — the
    /// `StackSpawn` stray sweep above despawns them first, so
    /// `all_living_enemies` finds nothing left with live `Stats` to query
    /// against. Underground losses marking nobody is therefore a consequence
    /// of call order, not a check this function makes.
    ///
    /// The cap (`MAX_NEMESES`) is counted by querying live holders rather
    /// than tracked anywhere — the entities are the ledger. An
    /// already-marked hostile always escalates even with the cap full; only
    /// a *fresh* mark is refused, which is what keeps this asymmetric rather
    /// than needing a demotion path.
    pub(crate) fn mark_nemeses(&mut self, won: bool) {
        // Belt and braces with `all_living_enemies()` returning empty on a
        // win: an emptied roster leaves nothing for the loop below to mark
        // as the code stands today, so this guard is provably redundant and
        // no test can tell the two apart — deleting it leaves every test in
        // `tests/nemesis.rs` green. It stays anyway, because it is the
        // spec's rule in its own words ("if hostiles are still standing when
        // the battle tears down, every living one is marked"), stated rather
        // than inferred from another function's incidental behaviour.
        // Without it, a model that left a dead-but-unreaped body in its
        // roster would start marking hostiles on wins with nothing here to
        // catch it. Don't "clean this up" for being uncovered — it is
        // uncovered on purpose.
        if won {
            return;
        }
        let mut holders = self
            .world
            .query_filtered::<Entity, With<Nemesis>>()
            .iter(&self.world)
            .count();
        for hostile in self.all_living_enemies() {
            // `fresh` is tracked separately from `marked`: an escalation
            // (the `nemesis.0 += 1` arm) is a mark too, but the name is
            // written once, on the grudge that actually inserts `Nemesis`
            // — see `name_new_nemesis`.
            let (marked, fresh) = if let Some(mut nemesis) = self.world.get_mut::<Nemesis>(hostile)
            {
                nemesis.0 += 1;
                (true, false)
            } else if holders < MAX_NEMESES {
                self.world.entity_mut(hostile).insert(Nemesis(1));
                holders += 1;
                (true, true)
            } else {
                (false, false)
            };
            // A hostile the cap refused gets no grudge at all, so it must
            // not climb the rarity ladder either — promotion is what a mark
            // does, not something a fight's mere survival earns.
            if marked {
                self.promote_rarity(hostile);
                if fresh {
                    self.name_new_nemesis(hostile);
                }
                // The same shake `battle_flee` performs on a successful
                // jack-out, generalised to the path that function's own
                // comment doesn't cover: a Forgiving defeat with no
                // structure to warp to leaves the player's `Position`
                // exactly where it was, adjacent to whatever just beat
                // them. A `NestGuardian` marked here is promoted and
                // healed to its new max in the same breath — if it were
                // still `Pursuing`, `pursuit_tick` (already the first
                // thing to call `start_battle` from inside `tick_inner`,
                // and running later in the very same tick via
                // `death_handling_system`) would re-engage it before the
                // player's next input ever arrived. `NestGuardian` itself
                // is left alone, so the guardian keeps its tether and just
                // resumes ordinary wandering; `attack_nest` re-provokes it
                // next time. Only a hostile that actually got marked loses
                // its `Pursuing` here — one the cap refused keeps chasing,
                // exactly as it did before this feature existed.
                self.world.entity_mut(hostile).remove::<Pursuing>();
            }
        }
    }

    /// Writes a bank-derived name to a hostile on its **first** grudge only
    /// — `mark_nemeses`' `fresh` flag is what enforces that, not a check in
    /// here, since by the time this runs `Nemesis` is already present on
    /// every caller and can't tell first from second apart on its own.
    ///
    /// The seed is folded from the creature's own identity
    /// (`nemesis::name_seed`) rather than drawn from `resources::GameRng` —
    /// see that module's doc for why. `Potential::NEUTRAL` stands in for a
    /// hand-built test fixture that carries no roll of its own, the same
    /// fallback `quality_percent` uses. An empty name bank is left silently
    /// unnamed: `mark_nemeses` still marks, promotes and recharges without
    /// it — see `NemesisDb`'s own doc for why that is not a fault.
    fn name_new_nemesis(&mut self, hostile: Entity) {
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
        let seed = crate::nemesis::name_seed(&species, &potential);
        let Some(name) = self
            .world
            .resource::<crate::nemesis::NemesisDb>()
            .name(seed)
        else {
            return;
        };
        let Some(sanitized) = CustomName::sanitize(Some(name.to_string())) else {
            return;
        };
        self.world.entity_mut(hostile).insert(CustomName(sanitized));
    }

    /// Promotes `entity` one rung up the rarity ladder and fully recharges
    /// it, returning the tier it lands on. The only caller is
    /// `mark_nemeses`, on a living `Hostile` — see `spawning.rs`'s "Rarity
    /// multiplies here and exactly here" comment for the other site.
    ///
    /// `Rarity` is a *receipt* for a multiplier already baked into `Stats`
    /// at spawn (`spawn_wild_creature_scaled`), not a value anything may
    /// apply on its own. So this does not multiply by `new.stat_mult()` —
    /// that number is measured from `Ordinary` and would compound on top of
    /// whatever the spawn roll already spent. It multiplies by the **step**
    /// between the two tiers, `new.stat_mult() / old.stat_mult()`, and
    /// writes the new tier back as the updated receipt. The same class of
    /// bug already has a guard on the load path (`lifecycle.rs`, loading a
    /// `CreatureSave`) with the same reasoning — this is the second and
    /// last place a rarity multiplier is allowed to touch `Stats`.
    ///
    /// `Rarity::ALL`'s own top is the ceiling: past `Prismatic` the step is
    /// `1.0` and this is a no-op on stats, though the grudge that got the
    /// program here keeps rising regardless — that increment lives in the
    /// loop above, not here.
    ///
    /// The recharge is folded in rather than left to a second call, because
    /// nothing in this feature ever wants a promotion without the heal that
    /// follows it — `hp = max_hp` raises HP, which is why this stays clear
    /// of `apply_damage`'s rule that it is the only path allowed to lower
    /// it.
    pub(crate) fn promote_rarity(&mut self, entity: Entity) -> Rarity {
        let old = self
            .world
            .get::<Rarity>(entity)
            .copied()
            .unwrap_or_default();
        let new = Rarity::ALL
            .get(old.rank() as usize + 1)
            .copied()
            .unwrap_or(old);
        let step = new.stat_mult() / old.stat_mult();
        if let Some(mut stats) = self.world.get_mut::<Stats>(entity) {
            stats.max_hp = (stats.max_hp as f32 * step).round() as i32;
            stats.atk = (stats.atk as f32 * step).round() as i32;
            stats.mitigation = (stats.mitigation as f32 * step).round() as i32;
            stats.hp = stats.max_hp;
        }
        self.world.entity_mut(entity).insert(new);
        new
    }
}
