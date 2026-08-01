//! The things in a Stack frame that can be used up, and the record of
//! their having been.
//!
//! A cache, a seal and a lair each need both halves: a `CellKind` in the
//! generated frame, and an entry in `FrameMemory` saying it has been spent.
//! `stack::generate` is a pure function of `FrameSpec`, so the frame
//! itself comes back identical every time the party steps off and on — the
//! record here is the only thing that stops an emptied cache refilling.
//! Both views consult it through `cache_unopened`, `seal_open` and
//! `lair_cleared`.
//!
//! That is also why the record is what gets saved and the maze is not: it is
//! the run's history, not the world's shape, and no seed can hand it back.

use super::stack::StackPos;
use crate::resources::{FrameMemory, StackMemory};
use crate::stack::CellKind;
use crate::tuning::{
    STACK_CACHE_CREDITS, STACK_CACHE_DEPTH_GROWTH, STACK_CACHE_FRAGMENT_CHANCE, TRACE_PER_CACHE,
    TRACE_PER_SEAL,
};
use crate::*;

impl Game {
    /// The memory of the frame the party is standing in, created empty on
    /// first sight of it.
    pub(crate) fn frame_memory_mut(&mut self, pos: StackPos) -> &mut FrameMemory {
        self.world
            .resource_mut::<StackMemory>()
            .into_inner()
            .0
            .entry((pos.entrance, pos.depth))
            .or_default()
    }

    /// Empties the cache the party is standing on, if there is one and it
    /// has not already been emptied.
    ///
    /// Payout is a depth-scaled pile of Credits, a chance at a portal
    /// fragment, and whatever the item set declares — see
    /// `ItemDef::cache_drop`, which is how a mod adds to what caches hold
    /// without touching this function.
    ///
    /// Credits rather than Core Fragments deliberately: they are the one
    /// currency that survives a breach, so a Stack run banks something the
    /// next sector can still spend.
    pub(crate) fn open_cache(&mut self) {
        let Some(pos) = self.stack_pos() else {
            return;
        };
        if self.cell_underfoot() != Some(CellKind::Cache) {
            return;
        }
        if self
            .world
            .resource::<StackMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.looted.contains(&(pos.x, pos.y)))
        {
            return;
        }
        self.frame_memory_mut(pos).looted.insert((pos.x, pos.y));
        self.raise_trace(TRACE_PER_CACHE);

        let depth_mult = STACK_CACHE_DEPTH_GROWTH.powi(pos.depth as i32 - 1);
        let credits = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(STACK_CACHE_CREDITS)
        };
        let credits = ((credits as f32) * depth_mult).round() as u32;

        self.log_kind(MessageKind::Loot, "A cache, still sealed. You crack it.");
        let landed = self.grant_loot(self.trade_currency(), credits);
        if landed > 0 {
            self.log_kind(
                MessageKind::Loot,
                format!("{landed} credits, skimmed off some long-dead process."),
            );
        }

        let fragment_roll = {
            let chance = (STACK_CACHE_FRAGMENT_CHANCE * pos.depth as f64).min(1.0);
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance)
        };
        if fragment_roll && self.grant_loot(self.craft_currency(), 1) > 0 {
            self.log_kind(
                MessageKind::Loot,
                "A portal fragment, wedged in the casing.",
            );
        }

        // Sorted by id so a seeded run consumes its rolls in the same order
        // however the item files happen to load — the same guarantee
        // `equipment_drops_for` makes.
        let mut table: Vec<(ItemId, f32)> = self
            .world
            .resource::<ItemDb>()
            .all()
            .filter_map(|def| def.cache_drop.map(|chance| (def.id.clone(), chance)))
            .collect();
        table.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

        for (item, chance) in table {
            let roll = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(chance.clamp(0.0, 1.0) as f64)
            };
            if roll && self.grant_loot(item.clone(), 1) > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("Also inside: a {}.", self.item_name(&item)),
                );
            }
        }
    }

    /// Whether the party may walk into `cell`, opening a sealed door on the
    /// way if they are carrying something that opens it.
    ///
    /// Spends the shard rather than keeping it, and records the door as open
    /// so the way back out is free — a one-way vault that charged you again
    /// on the return trip would be a tax on having gone in.
    pub(crate) fn pass_seal(&mut self, pos: StackPos, cell: (i32, i32)) -> bool {
        let already_open = self
            .world
            .resource::<StackMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.opened.contains(&cell));
        if already_open {
            return true;
        }

        let shard = ItemId::from(ids::ACCESS_SHARD);
        let player = self.player_entity();
        let spent = self
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(shard, 1);
        if spent == 0 {
            self.log("Sealed. The lock wants authorization you don't have.".to_string());
            return false;
        }
        self.frame_memory_mut(pos).opened.insert(cell);
        self.log_kind(
            MessageKind::Outcome,
            "You burn an access shard. The seal releases.",
        );
        self.raise_trace(TRACE_PER_SEAL);
        true
    }

    /// Starts the boss fight if the party has just walked into an uncleared
    /// lair.
    ///
    /// The species is drawn from the link tile's biome, like every other
    /// Stack encounter, but from the boss pool rather than the ordinary
    /// one, and from an RNG seeded off the frame spec rather than off
    /// `GameRng`: which thing guards a stack is a property of the stack, not
    /// of how many rolls happened first, so leaving and coming back cannot
    /// reroll it into something easier.
    ///
    /// Not every biome fields a boss — no shipped Static Field species does
    /// — and there the lair falls back to the toughest ordinary program the
    /// biome has, which at the bottom of a deep stack is no small thing.
    pub(crate) fn rouse_lair(&mut self) {
        if self.has_active_battle() || self.is_game_over().is_some() {
            return;
        }
        let Some(pos) = self.stack_pos() else {
            return;
        };
        if self.cell_underfoot() != Some(CellKind::Lair) || self.lair_cleared(pos) {
            return;
        }

        let (ex, ey) = pos.entrance;
        let Some((species, is_boss)) = self.pick_lair_species(pos) else {
            return;
        };
        let depth_mult = self.stack_depth_multiplier();
        let pack = self.spawn_pack(&species, is_boss, ex, ey, depth_mult);
        if pack.is_empty() {
            return;
        }
        for &member in &pack {
            self.world.entity_mut(member).insert(StackSpawn);
        }
        self.remember_fight();
        self.log_kind(
            MessageKind::Outcome,
            "The stack opens out. Something very large is already awake.",
        );
        self.start_battle(pack);
    }

    fn pick_lair_species(&mut self, pos: StackPos) -> Option<(String, bool)> {
        let (ex, ey) = pos.entrance;
        let biome = self.world.resource_mut::<WorldMap>().tile(ex, ey).biome;
        let spec = self.frame_spec(pos.depth, pos.frames, pos.entrance);

        let bosses: Vec<String> = self
            .world
            .resource::<SpeciesDb>()
            .boss_habitat_matches(biome)
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
        if !bosses.is_empty() {
            // Salted off the level's own stream so the choice of guardian
            // doesn't correlate with the shape of the room it stands in.
            const LAIR_SALT: u64 = 0x1A19_B055;
            let mut rng = StdRng::seed_from_u64(spec.rng_seed() ^ LAIR_SALT);
            return Some((bosses[rng.random_range(0..bosses.len())].clone(), true));
        }

        let db = self.world.resource::<SpeciesDb>();
        db.habitat_matches(biome)
            .into_iter()
            .max_by_key(|s| s.base_hp + s.base_atk + s.base_def)
            .map(|s| (s.id.clone(), false))
    }

    /// Whether the sealed door on `cell` has already been burned open.
    pub(crate) fn seal_open(&self, pos: StackPos, cell: (i32, i32)) -> bool {
        self.world
            .resource::<StackMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.opened.contains(&cell))
    }

    pub(crate) fn lair_cleared(&self, pos: StackPos) -> bool {
        self.world
            .resource::<StackMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.cleared)
    }

    /// Records that this stack's guardian is down, so its lair does not
    /// refill. Called from `award_loot`, which is the one place that knows a
    /// hostile actually died rather than merely being fled from.
    pub(crate) fn mark_lair_cleared(&mut self) {
        let Some(pos) = self.stack_pos() else {
            return;
        };
        if self.cell_underfoot() != Some(CellKind::Lair) {
            return;
        }
        self.frame_memory_mut(pos).cleared = true;
    }

    /// Whether the cache on `cell` of the frame the party is in is still
    /// unopened — what both views use to stop advertising an empty one.
    pub(crate) fn cache_unopened(&self, pos: StackPos, cell: (i32, i32)) -> bool {
        !self
            .world
            .resource::<StackMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.looted.contains(&cell))
    }

    /// Marks the cell the party is standing on as somewhere a fight started,
    /// for the map to pin.
    pub(crate) fn remember_fight(&mut self) {
        let Some(pos) = self.stack_pos() else {
            return;
        };
        self.frame_memory_mut(pos).fights.insert((pos.x, pos.y));
    }
}
