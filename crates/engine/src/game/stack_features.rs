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
use crate::resources::{CurrentStack, FrameMemory, LairFight, StackMemory};
use crate::stack::CellKind;
use crate::tuning::{
    STACK_BREAKPOINT_CHANCE, STACK_BREAKPOINT_PARTIAL_RADIUS, STACK_CACHE_CREDITS,
    STACK_CACHE_DEPTH_GROWTH, STACK_CORRUPTION_HP_PERCENT, STACK_CORRUPTION_MIN_DAMAGE,
    TRACE_PER_BREAKPOINT, TRACE_PER_CACHE, TRACE_PER_SEAL,
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

        let depth_mult = STACK_CACHE_DEPTH_GROWTH.powi(pos.depth as i32 - 1);
        let credits = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(STACK_CACHE_CREDITS)
        };
        let credits = ((credits as f32) * depth_mult).round() as u32;

        self.log_kind(MessageKind::Loot, "A cache, still sealed. You crack it.");
        // After the line that says what was taken, so a band crossing reads
        // as the consequence of cracking the cache rather than as something
        // that happened first. `force_seal` orders these the same way.
        self.raise_trace(TRACE_PER_CACHE);
        let landed = self.grant_loot(self.trade_currency(), credits);
        if landed > 0 {
            self.log_kind(
                MessageKind::Loot,
                format!("{landed} credits, skimmed off some long-dead process."),
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
            if roll {
                let copy = self.grant_gear_drop(item, Rarity::Ordinary);
                self.log_kind(
                    MessageKind::Loot,
                    format!("Also inside: a {}.", self.drop_label(&copy)),
                );
            }
        }
    }

    /// Shoulders a sealed door on `cell` open as the party walks into it.
    ///
    /// Nothing is spent — the seal is a barrier, not a lock, and the cost of
    /// forcing it is the noise (`TRACE_PER_SEAL`) rather than an item. What
    /// it does record is that this door now stands open, so the way back out
    /// is drawn as a way out: a seal that re-shut behind the party would
    /// redraw their own route as a wall.
    pub(crate) fn force_seal(&mut self, pos: StackPos, cell: (i32, i32)) {
        let already_open = self
            .world
            .resource::<StackMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.opened.contains(&cell));
        if already_open {
            return;
        }

        self.frame_memory_mut(pos).opened.insert(cell);
        self.log_kind(
            MessageKind::Outcome,
            "You put your shoulder to the seal. It gives, loudly.",
        );
        self.raise_trace(TRACE_PER_SEAL);
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
        let esc = self.stack_escalation(pos.depth);
        let pack = self.spawn_pack(&species, is_boss, ex, ey, esc);
        if pack.is_empty() {
            return;
        }
        for &member in &pack {
            self.world.entity_mut(member).insert(StackSpawn);
        }
        // `spawn_pack` spawns the guardian first and extends with the escort
        // behind it, so the lair's own program is the head of the pack.
        let guardian = pack[0];
        self.remember_fight();
        self.log_kind(
            MessageKind::Outcome,
            "The stack opens out. Something very large is already awake.",
        );
        self.start_battle(pack);
        // After the fight opens, since `start_battle` is what installs the
        // resource this is written to.
        if let Some(mut battle) = self.world.get_resource_mut::<BattleState>() {
            battle.lair = Some(LairFight { pos, guardian });
        }
    }

    pub(crate) fn pick_lair_species(&mut self, pos: StackPos) -> Option<(String, bool)> {
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

    /// Whether the sealed door on `cell` has already been forced open.
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

    /// The frame whose lair `entity` guards, if the live fight was roused
    /// from one and `entity` is the program it was built around.
    ///
    /// The one statement of "this is the guardian", which two things ask in
    /// opposite directions: `mark_lair_cleared` on the way out of the fight,
    /// and `battle_set_action` at plan time to refuse it as a decompile
    /// target. A copy of the comparison in the refusal is a copy that can
    /// come to disagree about what a guardian is, and the two disagreeing is
    /// precisely a stack the player may empty and never finish.
    fn lair_guarded_by(&self, entity: Entity) -> Option<StackPos> {
        self.world
            .get_resource::<BattleState>()
            .and_then(|battle| battle.lair)
            .filter(|lair| lair.guardian == entity)
            .map(|lair| lair.pos)
    }

    /// Whether the live fight's lair was built around `entity` — see
    /// `lair_guarded_by`.
    pub(crate) fn is_lair_guardian(&self, entity: Entity) -> bool {
        self.lair_guarded_by(entity).is_some()
    }

    /// Records that this stack's guardian has left the fight for good.
    ///
    /// Called from `award_loot`, which is the one place that knows a hostile
    /// actually died rather than merely being fled from, and from
    /// `attempt_decompile`, which is the one other way a program leaves a
    /// fight and does not come back. Both pass the program itself, because
    /// `award_loot` fires for *every* kill in the game and most of a lair's
    /// pack is escort.
    ///
    /// `FrameMemory::cleared` is the single record: it says the lair is
    /// spent, which is what stops it refilling, and `end_battle` reads it
    /// back against `BattleState::lair` to decide whether the stack comes
    /// down with it. Which frame that is comes off the battle rather than
    /// off the party's own `Locale`, for the reason `LairFight` documents.
    pub(crate) fn mark_lair_cleared(&mut self, victim: Entity) {
        let Some(pos) = self.lair_guarded_by(victim) else {
            return;
        };
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

    /// Jacks into the breakpoint the party is standing on, if there is one
    /// and it has not already been used.
    ///
    /// Marks the cells it resolves seen, walls included, so the map draws as
    /// a frame rather than as a floor plan floating in nothing.
    ///
    /// The jack is a roll (`STACK_BREAKPOINT_CHANCE`): it takes the whole
    /// frame, or it half-resolves and hands over
    /// `STACK_BREAKPOINT_PARTIAL_RADIUS` of substrate around the party. The
    /// port is burnt either way — the `jacked` record goes in **before** the
    /// roll, so there is no ordering in which a failed jack leaves something
    /// to try again. `TRACE_PER_BREAKPOINT` is charged either way too, on the
    /// same argument: the loudest thing the party can do is announcing
    /// themselves to the substrate, and that happens when they jack in rather
    /// than when it works.
    ///
    /// Rolled off `GameRng` rather than off `FrameSpec::rng_seed` like the
    /// frame's own shape: what a place *is* has to survive a save/load, but
    /// this is a property of the moment you jacked in, exactly as an orphan's
    /// species and its stats divide.
    ///
    /// Ordered like `open_cache`: the line saying what happened, then the
    /// Trace raise, so a band crossing reads as the consequence rather than
    /// as something that happened first.
    pub(crate) fn trip_breakpoint(&mut self) {
        let Some(pos) = self.stack_pos() else {
            return;
        };
        if self.cell_underfoot() != Some(CellKind::Breakpoint)
            || self.breakpoint_spent(pos, (pos.x, pos.y))
        {
            return;
        }
        self.frame_memory_mut(pos).jacked.insert((pos.x, pos.y));

        let resolved = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(STACK_BREAKPOINT_CHANCE)
        };

        let Some(level) = self.world.resource::<CurrentStack>().0.as_ref() else {
            return;
        };
        let (width, height) = (level.width, level.height);
        let r = STACK_BREAKPOINT_PARTIAL_RADIUS;
        let cells: Vec<(i32, i32)> = if resolved {
            (0..height)
                .flat_map(|y| (0..width).map(move |x| (x, y)))
                .collect()
        } else {
            (pos.y - r..=pos.y + r)
                .flat_map(|y| (pos.x - r..=pos.x + r).map(move |x| (x, y)))
                .filter(|&(x, y)| x >= 0 && y >= 0 && x < width && y < height)
                .collect()
        };
        self.frame_memory_mut(pos).seen.extend(cells);

        self.log_kind(
            MessageKind::Outcome,
            if resolved {
                "You jack into the port. The frame resolves around you, whole."
            } else {
                "You jack into the port. It stutters, spits, and gives up \
                 nothing but the substrate you are standing in."
            },
        );
        self.raise_trace(TRACE_PER_BREAKPOINT);
    }

    /// Whether the breakpoint on `cell` has already been jacked into — what
    /// both views use to stop advertising a spent one, exactly as
    /// `cache_unopened` does for an emptied cache.
    pub(crate) fn breakpoint_spent(&self, pos: StackPos, cell: (i32, i32)) -> bool {
        self.world
            .resource::<StackMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.jacked.contains(&cell))
    }

    /// Adopts the orphaned process the party is standing on, for one taming
    /// catalyst.
    ///
    /// **Every refusal lands before anything is spent and before anything is
    /// spawned**, which is the whole of this function's ordering and the one
    /// thing worth reading it for. `attempt_decompile` is the model for that
    /// ordering.
    ///
    /// It is **not** the model for the refusal strings, and that is a
    /// reversal worth reading before "restoring" one vocabulary. Its
    /// `no taming catalyst` / `roster is full` are `ability_unavailable`
    /// reasons, built to sit *after* a greyed-out battle row
    /// ("Decompile — no taming catalyst"). These land alone on
    /// `App::status_line` for four seconds, where a lowercase fragment reads
    /// as no response at all — which is how a player reported the `o` key as
    /// doing nothing. The concept is shared; the rendering is not, and the
    /// split already existed: `Game::purchase_stack_program` refuses a full
    /// roster with this exact sentence.
    ///
    /// Deliberately **not** done, each for a reason a later reader should
    /// not undo:
    ///
    /// - **No `StackSpawn` tag.** That component marks a creature
    ///   `end_battle` despawns on the way out (`game/combat_teardown.rs`).
    ///   An orphan never fights; tagging one would delete it the next time
    ///   the party won a fight underground.
    /// - **No XP.** `attempt_decompile` awards it for a fight that was won.
    ///   Nothing was fought here.
    /// - **No `Party` push.** The roster is the destination; which programs
    ///   are *fielded* is a separate choice the player makes elsewhere.
    /// - **No Trace.** Trace's three sources are all things the party
    ///   *breaks*, and an orphan is a rescue. That is a design judgement
    ///   rather than a constraint — if the first playtest says the Stack is
    ///   too quiet, `raise_trace` here is the first knob to reach for.
    pub fn adopt_orphan(&mut self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let Some(pos) = self.stack_pos() else {
            return Err("There's nothing like that here.".into());
        };
        let cell = (pos.x, pos.y);
        if self.cell_underfoot() != Some(CellKind::Orphan) || !self.orphan_present(pos, cell) {
            return Err("There's nothing like that here.".into());
        }
        if let Some(block) = self.adopt_block() {
            return Err(block.refusal().into());
        }
        // `adopt_block` just cleared the pack, and nothing between here and
        // there can empty it.
        let (catalyst, _) = self
            .taming_catalyst()
            .expect("adopt_block reported no obstacle");
        let species = self
            .orphan_species(pos)
            .ok_or_else(|| "The process is too far gone to reach.".to_string())?;

        // Past every refusal: from here the catalyst is spent and the
        // program exists.
        let player = self.player_entity();
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(catalyst, 1);
        let depth_mult = self.stack_depth_multiplier();
        let (ex, ey) = pos.entrance;
        let Some(program) = self.adopt_program(&species, ex, ey, depth_mult) else {
            return Err("The process is too far gone to reach.".into());
        };
        self.frame_memory_mut(pos).adopted.insert(cell);

        let name = self.creature_label(program);
        self.log_kind(
            MessageKind::Outcome,
            format!("{name} has been running alone down here. It comes with you."),
        );
        Ok(())
    }

    /// What stands between the party and the orphan underfoot, or `None` if
    /// nothing does.
    ///
    /// **The one ladder for that question.** `adopt_orphan` refuses on it and
    /// `Game::stack_view` warns with it, so the row underfoot and the key can
    /// never disagree about whether an adoption is on. Two copies of these
    /// two checks would drift, and the drift is the invisible kind: the row
    /// goes on offering while the key quietly refuses.
    ///
    /// Only the obstacles the player can *do something about*. `adopt_orphan`
    /// also refuses mid-battle and at a cell with no orphan on it, but the
    /// first cannot be true while this view is drawn and the second is what
    /// the match arm calling this has already decided.
    pub(crate) fn adopt_block(&self) -> Option<AdoptBlock> {
        if self.taming_catalyst().is_none() {
            return Some(AdoptBlock::NoCatalyst);
        }
        if self.pet_count() >= self.pet_capacity() {
            return Some(AdoptBlock::RosterFull);
        }
        None
    }

    /// Which program this frame's orphan is, or `None` if the entrance's
    /// biome fields nothing ordinary at all.
    ///
    /// The same pool `maybe_stack_encounter` fights out of — the biome
    /// above the link, so which link you picked still matters and no new
    /// content is needed — but drawn from an RNG seeded off the frame spec
    /// rather than off `GameRng`. That is forced, not chosen: the party has
    /// to be able to see what a program is before paying an `ice_breaker`
    /// for it, so the answer has to survive a save/load, and `GameRng`'s
    /// stream position is not persisted. See
    /// `the_species_a_frame_offers_survives_a_save_and_load`.
    ///
    /// Never a boss. `maybe_stack_encounter` refuses one for a fight the
    /// party did not see coming, and a free boss companion is a stronger
    /// version of the same objection.
    pub(crate) fn orphan_species(&mut self, pos: StackPos) -> Option<String> {
        let (ex, ey) = pos.entrance;
        let spec = self.frame_spec(pos.depth, pos.frames, pos.entrance);
        let (candidates, _) = self.habitat_pools(ex, ey)?;
        if candidates.is_empty() {
            return None;
        }
        // Salted off the frame's own stream, like `pick_lair_species`, so
        // which program is down here doesn't correlate with the shape of
        // the dead end it is sitting in.
        const ORPHAN_SALT: u64 = 0xDEAD_C0DE;
        let mut rng = StdRng::seed_from_u64(spec.rng_seed() ^ ORPHAN_SALT);
        Some(candidates[rng.random_range(0..candidates.len())].clone())
    }

    /// Whether the orphan on `cell` is still there to be adopted — what
    /// both views use to stop advertising one already taken, and what
    /// `adopt_orphan` refuses a second adoption on.
    pub(crate) fn orphan_present(&self, pos: StackPos, cell: (i32, i32)) -> bool {
        !self
            .world
            .resource::<StackMemory>()
            .0
            .get(&(pos.entrance, pos.depth))
            .is_some_and(|m| m.adopted.contains(&cell))
    }

    /// Drops the party a frame if they have walked onto a fault.
    ///
    /// The depth guard is belt and braces over `stack::generate`, which does
    /// not lay faults on a bottom frame at all — but a fault that fired there
    /// would put the party inside a frame that does not exist, and this is
    /// cheaper than the bug.
    pub(crate) fn take_fault(&mut self) {
        if self.has_active_battle() || self.is_game_over().is_some() {
            return;
        }
        let Some(pos) = self.stack_pos() else {
            return;
        };
        if self.cell_underfoot() != Some(CellKind::Fault) || pos.depth >= pos.frames {
            return;
        }
        self.fall_to(pos.depth + 1, pos.frames, pos.entrance);
    }

    /// Bleeds the player for standing on corrupted substrate.
    ///
    /// A fraction of maximum HP rather than a flat figure: Stack depth is
    /// uncorrelated with player level, so any constant is lethal at level 1
    /// and free by mid-run. See `STACK_CORRUPTION_HP_PERCENT`.
    ///
    /// Goes through `Game::apply_damage`, the one path that lowers a
    /// creature's HP — so anything that must see all damage sees this too,
    /// and a Mitigation field buff blunts it, which is exactly what a
    /// mitigation field ought to do.
    ///
    /// The player alone. Corrupting the party would route program deaths and
    /// the permadeath path through something that is not a fight.
    pub(crate) fn bleed_corruption(&mut self) {
        if self.cell_underfoot() != Some(CellKind::Corruption) {
            return;
        }
        let player = self.player_entity();
        let Some(stats) = self.world.get::<Stats>(player) else {
            return;
        };
        let damage = ((stats.max_hp as f32 * STACK_CORRUPTION_HP_PERCENT).round() as i32)
            .max(STACK_CORRUPTION_MIN_DAMAGE);
        self.apply_damage(player, damage);
        self.log_kind(
            MessageKind::Outcome,
            format!("The substrate here is rotten. It takes {damage} off you."),
        );
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

/// Why an adoption underfoot would be refused — see `Game::adopt_block`.
///
/// Read on two surfaces that want different lengths of the same fact: a
/// sentence on `App::status_line` when the key is pressed, and a few words
/// appended to the row underfoot before it is. Both mappings are exhaustive
/// matches, so a third obstacle fails to compile at both rather than
/// silently going unreported on one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdoptBlock {
    NoCatalyst,
    RosterFull,
}

impl AdoptBlock {
    /// The refusal `adopt_orphan` returns, which lands alone on
    /// `App::status_line` — so a full sentence, not one of
    /// `Game::ability_unavailable`'s battle-row fragments.
    pub(crate) fn refusal(self) -> &'static str {
        match self {
            Self::NoCatalyst => "You need an ICE Breaker to adopt a process.",
            Self::RosterFull => "Your roster is full.",
        }
    }
}
