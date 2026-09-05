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
    pub fn settlement_name(&self, key: SettlementKey) -> String {
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

    /// What a relay trip to `key` would cost in ticks, or `None` if there is
    /// no trip to quote — an unknown town, or one with nowhere to stand
    /// beside it.
    ///
    /// The screen's figure and the door's charge come from here, so the two
    /// cannot differ. Measured anchor-to-landing, which is the walk the trip
    /// replaces whichever end the party sets off from.
    pub fn travel_cost_ticks(&mut self, key: SettlementKey) -> Option<u64> {
        let landing = self.relay_landing(key)?;
        let (ax, ay) = self.anchor_position()?;
        let steps = (landing.0 - ax).abs().max((landing.1 - ay).abs()) as u64;
        Some(steps * crate::tuning::SETTLEMENT_TRAVEL_TICKS_PER_TILE)
    }

    /// **The relay carries the party out to an Allied town.** Every refusal
    /// lands before anything is spent — no move, no ticks.
    ///
    /// The gate is `Game::dispatch_reach`, the door a squad and a route
    /// already leave through, so the party has to be standing in the base
    /// they are departing from. That is also why this does *not* call
    /// `require_surface`: the Relay is in base space, so a surface check
    /// would refuse the only place the trip can start.
    ///
    /// The trip spends the ticks the walk would have — `move_player` costs
    /// one tick a step, so `SETTLEMENT_TRAVEL_TICKS_PER_TILE` at 1 removes
    /// the encounters and the tedium and removes nothing else. Upkeep,
    /// decay, needs and production all advance exactly as far.
    pub fn travel_to_settlement(&mut self, key: SettlementKey) -> Result<(), String> {
        if self.is_game_over().is_some() {
            return Err("This run is over.".into());
        }
        if self.has_active_battle() {
            return Err("Not in the middle of a fight.".into());
        }
        match self.dispatch_reach() {
            crate::game::sortie::DispatchReach::NoRelay => {
                return Err("You have no Relay to travel from.".into());
            }
            crate::game::sortie::DispatchReach::OffBase => {
                return Err("The Relay only reaches you inside the base.".into());
            }
            crate::game::sortie::DispatchReach::AtRelay => {}
        }
        if !self
            .world
            .resource::<resources::Settlements>()
            .0
            .contains_key(&key)
        {
            return Err("There's no such settlement.".into());
        }
        if !self.standing_band(key).hosts_a_relay() {
            let name = self.settlement_name(key);
            return Err(format!("{name} won't hold a link open for you."));
        }
        let Some(landing) = self.relay_landing(key) else {
            let name = self.settlement_name(key);
            return Err(format!("There's nowhere to set down near {name}."));
        };
        let Some(ticks) = self.travel_cost_ticks(key) else {
            return Err("There's nowhere to set down out there.".into());
        };

        // Past every refusal.
        self.world
            .insert_resource(crate::resources::Locale::Surface);
        self.place_player_at(landing);
        // Arriving must open the town exactly as walking into it does, so
        // there is one arrival behaviour and not two.
        self.world.resource_mut::<resources::PendingVisit>().0 = Some(key);
        let name = self.settlement_name(key);
        self.log(format!("The relay sets you down outside {name}."));
        self.spend_travel_ticks(ticks);
        Ok(())
    }

    /// **And carries the party back from one.** `key` is the town being
    /// stood at, because the gate is that town's willingness to hold the
    /// link — the same `hosts_a_relay` the outbound trip asks.
    ///
    /// The outbound gate cannot be reused: `dispatch_reach` answers
    /// `OffBase` from anywhere but the base, and standing at a town is by
    /// definition not standing at the Relay. So the rule here is the same
    /// two facts minus the departure point — a Relay is standing, and this
    /// town will hold the link.
    pub fn travel_to_anchor(&mut self, key: SettlementKey) -> Result<(), String> {
        if self.is_game_over().is_some() {
            return Err("This run is over.".into());
        }
        if self.has_active_battle() {
            return Err("Not in the middle of a fight.".into());
        }
        if !self.has_relay() {
            return Err("You have no Relay to be pulled back to.".into());
        }
        if !self.settlement_reach(key) {
            return Err("You'd have to be at the town to use its link.".into());
        }
        if !self.standing_band(key).hosts_a_relay() {
            let name = self.settlement_name(key);
            return Err(format!("{name} won't hold a link open for you."));
        }
        let Some(anchor) = self.anchor_position() else {
            return Err("You have nowhere to be pulled back to.".into());
        };

        // Past every refusal.
        let from = self
            .world
            .get::<crate::components::Position>(self.player_entity())
            .map(|p| (p.x, p.y))
            .unwrap_or(anchor);
        let steps = (from.0 - anchor.0).abs().max((from.1 - anchor.1).abs()) as u64;
        self.place_player_at(anchor);
        self.log("The relay pulls you back, standing on the anchor.");
        self.spend_travel_ticks(steps * crate::tuning::SETTLEMENT_TRAVEL_TICKS_PER_TILE);
        Ok(())
    }

    /// The tile a relay trip to `key` sets down on — the nearest walkable
    /// cell **beside** the town, never the town's own.
    ///
    /// Band 1 and not band 0, and that is the whole of it: a settlement tile
    /// admits nobody (`move_player`'s fourth arm queues the visit and leaves
    /// `Position` untouched), so landing on one puts the party somewhere
    /// walking could never have taken them. The ring order is
    /// `spawning::ring_tiles`, shared with `standable_near` rather than
    /// copied.
    ///
    /// A tile holding a wild program, a nest, a Stack entrance or another
    /// town is skipped too — `walkable` alone is not the same question as
    /// "could the party have stepped here", which is the trap
    /// `standable_near`'s own callers have hit before.
    fn relay_landing(&mut self, key: SettlementKey) -> Option<(i32, i32)> {
        let tile = self
            .world
            .resource::<resources::Settlements>()
            .0
            .get(&key)?
            .tile;
        let candidates =
            crate::game::spawning::ring_tiles(tile, 1, crate::tuning::SETTLEMENT_SITE_SEARCH_TILES);
        candidates.into_iter().find(|&(x, y)| {
            self.world
                .resource_mut::<crate::world::WorldMap>()
                .tile(x, y)
                .walkable
                && self.find_wild_creature_at(x, y).is_none()
                && self.find_nest_at(x, y).is_none()
                && self.find_surface_link_at(x, y).is_none()
                && self.find_settlement_at(x, y).is_none()
        })
    }

    fn place_player_at(&mut self, (x, y): (i32, i32)) {
        let player = self.player_entity();
        if let Some(mut pos) = self.world.get_mut::<crate::components::Position>(player) {
            pos.x = x;
            pos.y = y;
        }
    }

    /// The trip's time, spent through the world's own tick so that
    /// everything a walk would have advanced still advances.
    fn spend_travel_ticks(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.tick();
        }
    }

    /// What this town is currently worth to the party, one sentence per aid
    /// it actually offers — the town screen's aid rows.
    ///
    /// **Finished sentences rather than figures**, for two reasons that both
    /// bind. A read-only screen's rows are owned by the engine and drawn by
    /// gui, so a phrase built in the renderer is a transform in the wrong
    /// crate. And this game has never shown the player a tick: a cooldown
    /// quoted as a number is a number nobody can read, so the wait is banded
    /// the way `game::memories::age_phrase` bands a memory's age.
    ///
    /// Every sentence is a **call** to the door that will honour it, so the
    /// page cannot offer something the door then refuses — including the
    /// travel line, which asks for the party's own Relay and not merely the
    /// town's willingness.
    pub(crate) fn settlement_aid_lines(&mut self, key: SettlementKey) -> Vec<String> {
        let mut lines = Vec::new();
        let band = self.standing_band(key);

        let garrisons = band.garrison_defense() > 0
            && match (self.anchor_position(), self.settlement_tile(key)) {
                (Some((ax, ay)), Some((tx, ty))) => {
                    (tx - ax).abs().max((ty - ay).abs())
                        <= crate::tuning::SETTLEMENT_GARRISON_RADIUS
                }
                _ => false,
            };
        if garrisons {
            lines.push(AID_GARRISON.to_string());
        }

        match self.gift_available_in(key) {
            None => {}
            Some(0) => lines.push(AID_GIFT_READY.to_string()),
            Some(remaining) => lines.push(
                wait_line(remaining, crate::tuning::SETTLEMENT_GIFT_COOLDOWN_TICKS).to_string(),
            ),
        }

        if band.hosts_a_relay() && self.has_relay() {
            lines.push(AID_RELAY.to_string());
        }
        lines
    }

    fn settlement_tile(&self, key: SettlementKey) -> Option<(i32, i32)> {
        self.world
            .resource::<resources::Settlements>()
            .0
            .get(&key)
            .map(|known| known.tile)
    }
}

/// Every sentence the aid rows can write, and the whole of the vocabulary.
///
/// **Constants rather than inline literals**, because the town page has no
/// scroll: its width and height censuses live in `crates/gui` and have to
/// measure the *worst case*, and a gui test cannot build a `Game` to ask —
/// `Game::world` is private, which is the architectural rule. Two parallel
/// lists would drift the moment a sentence was reworded, and the drift would
/// show up as a line silently drawn past the right edge, since `draw_row`
/// clips vertically and never horizontally. So the strings are exported and
/// `AID_LINES` is the census both sides read.
pub const AID_GARRISON: &str = "They keep a detachment near your base.";
/// See `AID_GARRISON`.
pub const AID_GIFT_READY: &str = "They will spare you a program for the asking.";
/// See `AID_GARRISON`.
pub const AID_GIFT_SOON: &str = "They have nobody spare for you just yet.";
/// See `AID_GARRISON`.
pub const AID_GIFT_LATER: &str = "They have nobody spare for you for a good while yet.";
/// See `AID_GARRISON`.
pub const AID_RELAY: &str = "Their relay will carry you home.";

/// The census: every sentence above, for the layout gates to measure.
/// A line written by `Game::settlement_aid_lines` and missing here is a line
/// nothing measures, which is the failure this array exists to make
/// impossible.
pub const AID_LINES: [&str; 5] = [
    AID_GARRISON,
    AID_GIFT_READY,
    AID_GIFT_SOON,
    AID_GIFT_LATER,
    AID_RELAY,
];

/// How long until a town will spare a program again, in the player's words.
///
/// `game::memories::age_phrase` turned around: banded against the wait's own
/// span rather than quoted as a tick count, because this game has never
/// shown the player a tick. Two bands rather than four — a cooldown has no
/// tail to describe, it either has most of its span left or it is nearly up.
fn wait_line(remaining: u64, span: u64) -> &'static str {
    if remaining * 2 > span.max(1) {
        AID_GIFT_LATER
    } else {
        AID_GIFT_SOON
    }
}
