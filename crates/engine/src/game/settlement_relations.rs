//! Standing with a town: the one writer and its movers.
//!
//! `settlements::relations` holds the shape — the band ladder, the clamp,
//! the consequence queries — and this holds the verbs. Every mover is a
//! call to `Game::adjust_standing`, never a write beside it, which is what
//! makes the clamp, the log and any future consequence of *changing* band a
//! thing written once. `set_machine_status`' rule is the one being copied:
//! the door speaks only on a transition, so entering a band is news and
//! staying in it is not.

use crate::resources;
use crate::settlements::SettlementKey;
use crate::settlements::relations::{self, Standing};
use crate::tuning::SETTLEMENT_NOTICE_RADIUS;

use crate::Game;
use crate::resources::MessageKind;

impl Game {
    /// What the town `key` names thinks of the party, as a number.
    pub fn standing(&self, key: SettlementKey) -> i32 {
        self.world
            .resource::<resources::Standings>()
            .0
            .get(&key)
            .map(|relation| relation.standing)
            .unwrap_or(0)
    }

    /// The same answer, banded — the form every reader outside this module
    /// actually wants, since the raw number is a budget and the band is the
    /// meaning.
    pub fn standing_band(&self, key: SettlementKey) -> Standing {
        relations::band(self.standing(key))
    }

    /// **The one door a standing is written through.** Clamps to the
    /// feature's bounds and announces a band crossing, in either direction.
    pub(crate) fn adjust_standing(&mut self, key: SettlementKey, delta: i32) {
        if delta == 0 {
            return;
        }
        let before = self.standing(key);
        let after = relations::clamp(before + delta);
        if after == before {
            return;
        }
        self.world
            .resource_mut::<resources::Standings>()
            .0
            .entry(key)
            .or_default()
            .standing = after;
        let (was, now) = (relations::band(before), relations::band(after));
        if was != now {
            let name = self.settlement_name(key);
            self.log_kind(
                MessageKind::Outcome,
                format!("{name} now regards you as {}.", now.label()),
            );
        }
    }

    /// Folds a basket's whole turnover into the town's trade record and
    /// pays out whatever standing it bought — see `Relation::trade_credits`
    /// for why the remainder is kept rather than rounded away.
    pub(crate) fn credit_trade_volume(&mut self, key: SettlementKey, credits: u32) {
        if credits == 0 {
            return;
        }
        let points = self
            .world
            .resource_mut::<resources::Standings>()
            .0
            .entry(key)
            .or_default()
            .credit_trade(credits);
        self.adjust_standing(key, points);
    }

    /// Every town near enough to `tile` to have heard about it, moved by
    /// `delta`.
    ///
    /// Known towns only. A settlement the party has never walked to has no
    /// record of them either, so crediting one would bank goodwill with a
    /// place that has never met them — and the resolved tile a distance is
    /// measured from only exists once the town has been materialized.
    pub(crate) fn credit_nearby_settlements(&mut self, tile: (i32, i32), delta: i32) {
        let nearby: Vec<SettlementKey> = self
            .world
            .resource::<resources::Settlements>()
            .0
            .iter()
            .filter(|(_, known)| {
                (known.tile.0 - tile.0).abs() <= SETTLEMENT_NOTICE_RADIUS
                    && (known.tile.1 - tile.1).abs() <= SETTLEMENT_NOTICE_RADIUS
            })
            .map(|(key, _)| *key)
            .collect();
        for key in nearby {
            self.adjust_standing(key, delta);
        }
    }

    /// The town's own name, for a line about it — falling back to a generic
    /// rather than refusing to speak, since `adjust_standing` is reachable
    /// with a key no town has been materialized for.
    pub(crate) fn settlement_name(&self, key: SettlementKey) -> String {
        self.world
            .resource::<resources::Settlements>()
            .0
            .get(&key)
            .map(|known| known.def.name.clone())
            .unwrap_or_else(|| "The settlement".to_string())
    }

    // ── Aid ────────────────────────────────────────────────────────────
    //
    // What a town that likes the party is worth. The garrison is passive
    // and lives in `Game::total_raid_defense`; these two are the verbs.

    /// When this town will next hand over a program: `None` if it never
    /// will, `Some(0)` if it will now, `Some(n)` for the ticks left.
    ///
    /// The preview the town screen draws, and the door's own gate is a call
    /// to it — `BuildOrderRow`'s rule, so a screen offering a gift and a
    /// door refusing one cannot disagree.
    pub fn gift_available_in(&self, key: SettlementKey) -> Option<u64> {
        if !self.standing_band(key).gifts_programs() {
            return None;
        }
        let last = self
            .world
            .resource::<resources::Standings>()
            .0
            .get(&key)
            .and_then(|relation| relation.last_gift_tick);
        let Some(last) = last else {
            return Some(0);
        };
        let elapsed = self.current_tick().saturating_sub(last);
        Some(crate::tuning::SETTLEMENT_GIFT_COOLDOWN_TICKS.saturating_sub(elapsed))
    }

    /// **The one door a gifted program comes through.** Every refusal lands
    /// before anything is spent, `commit_caravan_basket`'s rule — nothing
    /// below writes the relation or spawns anything until every check has
    /// passed.
    ///
    /// **The species is derived, never drawn.** Folding the world seed, the
    /// town's region and its gift count gives a roll a reload cannot change
    /// and, more importantly, one that spends no `resources::GameRng` — a
    /// gift must not shift the seeded stream every later encounter is drawn
    /// from.
    ///
    /// **What arrives is labour, not power** — `SETTLEMENT_GIFT_STAT_MULT`
    /// sits below the 1.0 an adopted or purchased program gets, because a
    /// free companion scaled to the zone is the shape this game closed off
    /// to keep progression earned by fighting. It lands at the anchor and
    /// becomes base staff by omission: `ProgramRole` is derived, and a
    /// program that is not in the party, not wielded and not away **is**
    /// staff.
    pub fn request_program_gift(&mut self, key: SettlementKey) -> Result<(), String> {
        if self.is_game_over().is_some() {
            return Err("This run is over.".into());
        }
        if self.has_active_battle() {
            return Err("Not in the middle of a fight.".into());
        }
        let Some(known) = self
            .world
            .resource::<resources::Settlements>()
            .0
            .get(&key)
            .cloned()
        else {
            return Err("There's no such settlement.".into());
        };
        if !self.settlement_reach(key) {
            return Err("You'd have to walk over there to ask.".into());
        }
        match self.gift_available_in(key) {
            None => {
                let name = self.settlement_name(key);
                return Err(format!("{name} doesn't owe you that kind of favour."));
            }
            Some(0) => {}
            Some(_) => {
                let name = self.settlement_name(key);
                return Err(format!("{name} has nothing spare for you yet."));
            }
        }
        let Some((ax, ay)) = self.anchor_position() else {
            return Err("You have nowhere to send it.".into());
        };
        let gifts_taken = self
            .world
            .resource::<resources::Standings>()
            .0
            .get(&key)
            .map(|relation| relation.gifts_taken)
            .unwrap_or(0);
        let Some(species) = self.gift_species(key, known.tile, gifts_taken) else {
            let name = self.settlement_name(key);
            return Err(format!("{name} has nobody to spare."));
        };

        // Past every refusal: from here the request is granted.
        let mult = if known.def.specialty == crate::settlements::Specialty::Programs {
            crate::tuning::SETTLEMENT_GIFT_STAT_MULT * crate::tuning::SETTLEMENT_GIFT_SPECIALTY_MULT
        } else {
            crate::tuning::SETTLEMENT_GIFT_STAT_MULT
        };
        let Some(program) = self.adopt_program(&species, ax, ay, mult) else {
            return Err("Nothing came through the link.".into());
        };
        let tick = self.current_tick();
        {
            let mut standings = self.world.resource_mut::<resources::Standings>();
            let relation = standings.0.entry(key).or_default();
            relation.last_gift_tick = Some(tick);
            relation.gifts_taken = relation.gifts_taken.saturating_add(1);
        }
        let town = self.settlement_name(key);
        let name = self.creature_label(program);
        // Base news, not field news: the program arrives at the anchor and
        // the player is standing at the town when they ask.
        self.log_base_kind(
            MessageKind::Loot,
            format!("{town} sends {name} to your base."),
        );
        Ok(())
    }

    /// Which program a town hands over — its own region's ordinary pool,
    /// picked by a fold of `(world seed, region, gifts taken)` rather than
    /// by `resources::GameRng`.
    ///
    /// **The ordinary pool only.** `habitat_pools`' second half is the
    /// biome's apex species, and a gifted boss inverts the whole "labour,
    /// not power" decision this feature rests on.
    ///
    /// The pool is **sorted** before the pick, `pick_lair_species`' reason:
    /// the draw indexes into it, so an unsorted pool would make a town's
    /// gift differ between runs on one seed.
    fn gift_species(
        &mut self,
        key: SettlementKey,
        tile: (i32, i32),
        gifts_taken: u32,
    ) -> Option<String> {
        let (mut pool, _apex) = self.habitat_pools(tile.0, tile.1, None, 0)?;
        if pool.is_empty() {
            return None;
        }
        pool.sort();
        let world_seed = self.world.resource::<crate::world::WorldMap>().seed() as u64;
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for word in [
            world_seed,
            key.rx as i64 as u64,
            key.ry as i64 as u64,
            gifts_taken as u64,
            crate::tuning::SETTLEMENT_GIFT_SALT,
        ] {
            h = crate::game::contracts::fold(h, &word.to_le_bytes());
        }
        Some(pool[crate::derive::index(h, pool.len())].clone())
    }
}
