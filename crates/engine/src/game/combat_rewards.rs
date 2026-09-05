//! What a won fight pays out: equipment drops, loot, experience, and
//! decompiling a defeated program into a companion.

use crate::items::DownedProgram;
use crate::progression::StatRow;
use crate::tuning::{DECOMPILE_ATTEMPT_BONUS_CAP, GEAR_AFFIX_CHANCE};
use crate::tuning::{
    DECOMPILER_SKILL_PER_LEVEL, NEST_RESPAWN_TICKS, PARTY_XP_DIVISOR, PERK_POINTS_PER_LEVEL,
    STACK_BOSS_PORTAL_FRAGMENT_DROP, SURFACE_BOSS_LOOT_BAND_FLOOR_PERCENT, SURFACE_BOSS_LOOT_DROPS,
    SURFACE_BOSS_LOOT_RARITY_FLOOR, SURFACE_BOSS_LOOT_VALUE_PER_ZONE,
};
use crate::*;

/// Which affix a weighted `pool` rolls, drawn off `rng` — or `None`, either
/// because `GEAR_AFFIX_CHANCE` missed or because the pool is empty.
///
/// **Two rolls, deliberately separate**: the chance decides whether there is
/// an affix at all and the per-affix `weight` decides which, so *adding* an
/// affix to the game cannot change how often affixes appear.
///
/// A free function taking its own `rng` because a copy is rolled from two
/// streams: `Game::roll_affix` draws from `GameRng`, and the caravan shelf
/// draws from a local `StdRng` because a derived shelf may not move the
/// shared stream. One expression rather than two, per `CLAUDE.md` — a doc
/// comment cannot hold two copies of a weighted pick in step.
///
/// Spends no draw at all on an empty pool, for `grant_gear_drop`'s reason: an
/// empty `assets/affixes/` must leave every seeded run exactly where it was.
pub(crate) fn pick_affix(pool: &[(AffixId, u32)], rng: &mut impl rand::RngExt) -> Option<AffixId> {
    // The emptiness check sits above the chance roll so an item whose slot
    // has no affixes spends no draw at all — moving it below would shift
    // every later roll in a run merely because a pool was empty.
    if pool.iter().map(|(_, w)| w).sum::<u32>() == 0 {
        return None;
    }
    if !rng.random_bool(GEAR_AFFIX_CHANCE) {
        return None;
    }
    weighted_affix(pool, rng)
}

/// The weighted walk alone, with no chance gate in front of it — what a
/// caller that has *already decided* this copy gets an affix wants.
///
/// Split out rather than copied because a caravan's standout row and an
/// ordinary drop must agree about which affix a given weight table yields;
/// a second walk is the copy that drifts when `weight` grows a meaning.
pub(crate) fn weighted_affix(
    pool: &[(AffixId, u32)],
    rng: &mut impl rand::RngExt,
) -> Option<AffixId> {
    let total: u32 = pool.iter().map(|(_, w)| w).sum();
    if total == 0 {
        return None;
    }
    let mut roll = rng.random_range(0..total);
    for (id, weight) in pool {
        match roll.checked_sub(*weight) {
            Some(rest) => roll = rest,
            None => return Some(id.clone()),
        }
    }
    None
}

impl Game {
    /// Every gear drop `species` can roll, from both directions the schema
    /// allows it to be declared: the species' own `equipment_drop`, plus
    /// every item whose `droppable` names this species. An item declared on
    /// both sides is rolled once at the better chance rather than twice.
    /// Sorted by item id so a seeded run always consumes its rolls in the
    /// same order.
    ///
    /// A running `DropBoost` field buff scales every chance here by its
    /// power, last — so it applies uniformly regardless of which side of
    /// the schema a drop came from. The result can run past 1.0; the one
    /// caller, `award_loot`, already clamps before rolling, so this leaves
    /// it unclamped rather than duplicating that.
    pub(crate) fn equipment_drops_for(&self, species: &SpeciesDef) -> Vec<(ItemId, f32)> {
        let mut drops: Vec<(ItemId, f32)> = species.equipment_drop.iter().cloned().collect();
        for def in self.world.resource::<ItemDb>().all() {
            let Some(sources) = &def.droppable else {
                continue;
            };
            for chance in sources
                .iter()
                .filter(|(id, _)| *id == species.id)
                .map(|&(_, chance)| chance)
            {
                match drops.iter_mut().find(|(id, _)| *id == def.id) {
                    Some(existing) => existing.1 = existing.1.max(chance),
                    None => drops.push((def.id.clone(), chance)),
                }
            }
        }
        drops.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        let boost_pct = self.field_buff_power(self.player_entity(), FieldBuffKind::DropBoost);
        if boost_pct != 0 {
            let multiplier = 1.0 + boost_pct as f32 / 100.0;
            for (_, chance) in &mut drops {
                *chance *= multiplier;
            }
        }
        drops
    }

    /// Hands the player one dropped copy of `item`, rolling its rare tier
    /// on the way in — **the one way a copy above `Ordinary` enters the
    /// game.** Returns the copy so the caller can name it in its own log
    /// line, which is the only thing the four callers do differently.
    ///
    /// `floor` is the worst tier this source may pay, and exists for one
    /// caller: `SURFACE_BOSS_LOOT_RARITY_FLOOR`. Everything else passes
    /// `Ordinary` and takes the bare ladder.
    ///
    /// **Crafting, buying and buying back are deliberately not callers.**
    /// A made or purchased copy is always plain, so found gear is
    /// categorically better than made gear — which is the whole of why a
    /// player would go looking rather than shopping, and is asserted as an
    /// absence by `crafted_gear_is_never_rare`, since an omission is
    /// invisible otherwise. It also keeps `ItemDef::value`'s two meanings
    /// intact: no recipe becomes underpriced for its output, so
    /// `no_craftable_item_is_worth_more_than_its_ingredients` still binds.
    ///
    /// A non-equippable item takes the early return and **spends no RNG
    /// draw**. A rare tier is defined by `EquipmentStats::for_rarity`, so a
    /// Core Fragment has nothing for one to scale; rolling anyway would also
    /// shift the shared `GameRng` stream on every material drop in the game,
    /// which is the kind of change that silently rewrites a seeded combat
    /// test three files away.
    ///
    /// Three rolls in a fixed order — rarity, affix, quality — and the last
    /// of them is last on purpose: for a given seed a dropped copy's tier
    /// and affix are exactly what they were before quality existed, so only
    /// what follows the drop in the stream moves. The quality floor is
    /// `QUALITY_DROP_BASE`, deliberately below what a bench pays: the world
    /// does not make good gear, your base does.
    pub(crate) fn grant_gear_drop(&mut self, item: ItemId, floor: Rarity) -> GearCopy {
        if self.equipment_of(&item).is_none() {
            self.grant_loot(item.clone(), 1, LootSource::Kill);
            return GearCopy::plain(item);
        }
        let rarity = self.roll_gear_rarity().max(floor);
        let affix = self.roll_affix(&item);
        let quality = self.roll_quality(crate::tuning::QUALITY_DROP_BASE);
        let copy = GearCopy::with_affixes(item, rarity, 0, affix.into_iter().collect(), quality);
        self.add_copies(&copy, 1);
        copy
    }

    /// The affix a dropping copy of `item` rolls, or `None` — see
    /// `affixes::AffixDef`.
    ///
    /// **Two rolls, deliberately separate**, the shape `roll_wild_routine`
    /// already uses: `GEAR_AFFIX_CHANCE` decides whether there is an affix
    /// at all, and the per-affix `weight` decides which. Folding them into
    /// one would mean *adding* an affix to the game changed how often
    /// affixes appear, so a mod could not add content without retuning.
    ///
    /// Independent of the rare tier, rather than gated behind it. Rarity is
    /// the axis you read from the row colour and affixes are the axis you
    /// read from the name, and coupling them would mean the overwhelming
    /// majority of drops — the ordinary ones — stayed exactly as
    /// featureless as they were, which is the complaint the whole feature
    /// exists to answer.
    ///
    /// Spends no RNG draw when the item is not equippable or the eligible
    /// pool is empty, for the reason `grant_gear_drop` spends none on a
    /// material: an empty `assets/affixes/` must leave every seeded run
    /// exactly where it was, which is what makes deleting the directory a
    /// supported way to play.
    fn roll_affix(&mut self, item: &ItemId) -> Option<AffixId> {
        let slot = self.equipment_of(item)?.0;
        let pool: Vec<(AffixId, u32)> = self
            .world
            .resource::<AffixDb>()
            .pool_for(slot)
            .into_iter()
            .map(|def| (def.id.clone(), def.weight))
            .collect();
        let mut rng = self.world.resource_mut::<GameRng>();
        pick_affix(&pool, &mut rng.0)
    }

    /// What this copy's affixes are, in its own (sorted) order, skipping
    /// any the build no longer knows.
    ///
    /// The skip is the whole compatibility story for a removed affix: a
    /// save naming one the build no longer has reads as a copy with one
    /// fewer effect rather than failing to load, the same shape
    /// `recognized_routines` gives a removed ability. Every reader goes
    /// through here.
    pub(crate) fn affixes_of(&self, copy: &GearCopy) -> Vec<&crate::affixes::AffixDef> {
        let db = self.world.resource::<AffixDb>();
        copy.affixes.iter().filter_map(|id| db.get(id)).collect()
    }

    /// What this copy is called: its affix's decoration of the item name,
    /// with the rare tier in front.
    ///
    /// **The one place a copy's name is built.** `Rarity::label` makes the
    /// same argument for the tier word alone; this is that plus the affix,
    /// and it is the engine's job rather than a renderer's so the inventory,
    /// the swap picker, the trade screen and a drop line cannot come to
    /// disagree about what a copy is called.
    ///
    /// A copy carries a **list** of affixes, and a name has room for two
    /// words: the first with a `prefix` and the first with a `suffix`, in
    /// the copy's own sorted order. Whatever is left over is counted as
    /// `+N` rather than named, since fusion can reach eight affixes and a
    /// name spelling all of them would not fit any screen that draws one.
    ///
    /// Both words go on through `AffixDef::decorate` rather than being
    /// composed here — the suffix appends and the prefix prepends, so
    /// applying the two in that order *is* the general case, and the affix
    /// word stays joined to a name in exactly one place.
    ///
    /// `+N` is **omitted at zero**, the call `Rarity::label` makes for
    /// `Ordinary` and this function already makes for a copy at spec — so a
    /// copy with one prefix and one suffix names both and gains nothing,
    /// and no name in any existing save moves.
    ///
    /// The quality figure goes **last**, after the tier word, the affix
    /// decoration and the count, and is omitted at `QUALITY_DEFAULT` — the
    /// reason nothing already on screen gets wider when the axis ships,
    /// since every copy in every existing save sits there. It costs seven
    /// cells on the worst case, which is what moved `SWAP_NAME_COLUMN` and
    /// pushed the swap screen's stat column out of the row's un-wrappable
    /// head.
    pub fn copy_name(&self, copy: &GearCopy) -> String {
        let base = self.item_name(&copy.item);
        let affixes = self.affixes_of(copy);
        // An affix carrying both a prefix and a suffix is refused at load
        // (`AffixDef::fault`), so these two are always different affixes and
        // the count below cannot double-subtract one.
        let prefixed = affixes.iter().find(|a| a.prefix.is_some());
        let suffixed = affixes.iter().find(|a| a.suffix.is_some());
        let mut named = base.to_string();
        if let Some(affix) = suffixed {
            named = affix.decorate(&named);
        }
        if let Some(affix) = prefixed {
            named = affix.decorate(&named);
        }
        let unnamed =
            affixes.len() - usize::from(prefixed.is_some()) - usize::from(suffixed.is_some());
        if unnamed > 0 {
            named = format!("{named} +{unnamed}");
        }
        let tiered = match copy.rarity.label() {
            Some(tier) => format!("{tier} {named}"),
            None => named,
        };
        match copy.quality {
            crate::tuning::QUALITY_DEFAULT => tiered,
            q => format!("{tiered} ({q}%)"),
        }
    }

    /// How a dropped copy reads in its loot line: the tier's word, the item
    /// name, and the category tag `item_name_tagged` adds.
    ///
    /// A drop line is the one place an item is named to a player who has not
    /// opened a screen, so it is also the only place a rare tier can be
    /// noticed at the moment it is earned — the row colour is a screen away.
    pub(crate) fn drop_label(&self, copy: &GearCopy) -> String {
        format!(
            "{} [{}]",
            self.copy_name(copy),
            self.item_category(&copy.item).short_label()
        )
    }

    /// Adds `qty` copies of `copy` to this fight's salvage tally — or, with
    /// no fight to hold one, announces it where it happened.
    ///
    /// The fallback is not dead weight even though nothing in the game
    /// reaches `award_loot` outside a battle today. It routes through
    /// `announce_drops`, the same formatter the tally flushes through, so the
    /// two can never come to word a drop differently; and it is what stops
    /// the next caller that *is* outside a fight from paying the player
    /// silently. `a_drop_outside_a_battle_is_announced_at_once` holds it.
    pub(crate) fn record_drop(&mut self, copy: GearCopy, qty: u32) {
        if qty == 0 {
            return;
        }
        let Some(mut battle) = self.world.get_resource_mut::<BattleState>() else {
            self.announce_drops(&[(copy, qty)]);
            return;
        };
        let drops = &mut battle.rewards.drops;
        match drops.iter_mut().find(|(held, _)| *held == copy) {
            Some(row) => row.1 += qty,
            None => drops.push((copy, qty)),
        }
    }

    /// Folds `tally` into `companion`'s row of this fight's rewards,
    /// reporting whether there was a fight to fold it into.
    pub(crate) fn record_companion_xp(&mut self, companion: Entity, tally: &XpTally) -> bool {
        let Some(mut battle) = self.world.get_resource_mut::<BattleState>() else {
            return false;
        };
        let rows = &mut battle.rewards.companions;
        match rows.iter_mut().find(|(entity, _)| *entity == companion) {
            Some((_, held)) => held.absorb(tally),
            None => rows.push((companion, tally.clone())),
        }
        true
    }

    /// The salvage tally: a header, then one row per distinct copy.
    ///
    /// A row apiece rather than one comma-joined line because app-core's
    /// `pane_rows` draws a `LogLine` as exactly one row and never wraps it,
    /// so a joined line's width would grow with the number of things that
    /// dropped and run off the right edge — which is `TODO.md`'s bug 3
    /// reached from a new direction. It is also the shape a level-up's stat
    /// block already uses, so the indent already reads as "belongs to the
    /// line above".
    ///
    /// `MessageKind::Loot` on every row is load-bearing at both ends: it is
    /// one of the four kinds `retain_outcomes_since_battle` keeps, so the
    /// tally survives onto the map, and it is what the log filter and the
    /// colour table read.
    fn announce_drops(&mut self, drops: &[(GearCopy, u32)]) {
        if drops.is_empty() {
            return;
        }
        self.log_kind(MessageKind::Loot, "Salvage:");
        for (copy, qty) in drops {
            let row = format!("  {qty} {}", self.drop_label(copy));
            self.log_kind(MessageKind::Loot, row);
        }
    }

    /// The player's line for the whole fight, and the stat block under it.
    ///
    /// Every "before" is recovered by subtracting the tally's delta from the
    /// value as it stands now, so nothing had to be snapshotted when the
    /// fight opened. That is safe for the three stats because a battle
    /// cannot change `max_hp`/`atk`/`def` by any other route — `apply_damage`
    /// moves `hp` alone, and `unequip` refuses while a battle is live.
    /// What the player earned, as lines rather than log calls — see
    /// `announce_xp`.
    fn player_xp_lines(&self, tally: &XpTally) -> Vec<(MessageKind, String)> {
        if tally.is_empty() {
            return Vec::new();
        }
        let player = self.player_entity();
        let Some(stats) = self.world.get::<Stats>(player).copied() else {
            return Vec::new();
        };
        if tally.gain.levels == 0 {
            return vec![(MessageKind::Outcome, format!("You gain {} XP.", tally.xp))];
        }
        let mut rows = tally.gain.stat_rows(&stats).to_vec();
        if tally.perk_points > 0 {
            let now = self
                .world
                .get::<Perks>(player)
                .map(|p| p.points)
                .unwrap_or(0) as i32;
            rows.push(StatRow::new(
                "Perk Points",
                now - tally.perk_points as i32,
                now,
            ));
        }
        if tally.decompiler != 0 {
            let now = self
                .world
                .get::<Decompiler>(player)
                .map(|d| d.skill)
                .unwrap_or(0);
            rows.push(StatRow::new("Decompiler", now - tally.decompiler, now));
        }
        let level = self
            .world
            .get::<Experience>(player)
            .map(|e| e.level)
            .unwrap_or(1);
        let mut lines = vec![(
            MessageKind::LevelUp,
            format!("You gain {} XP, reaching level {level}.", tally.xp),
        )];
        lines.extend(
            progression::stat_block(&rows)
                .into_iter()
                .map(|line| (MessageKind::LevelUp, line)),
        );
        lines
    }

    /// A companion's line, and only if it levelled — the same restraint
    /// `award_party_xp` has always shown, for the same reason: a busy fight
    /// with a full roster would otherwise close on a line per member saying
    /// nothing happened.
    fn companion_xp_lines(&self, companion: Entity, tally: &XpTally) -> Vec<(MessageKind, String)> {
        if tally.gain.levels == 0 {
            return Vec::new();
        }
        let Some(stats) = self.world.get::<Stats>(companion).copied() else {
            return Vec::new();
        };
        let level = self
            .world
            .get::<Experience>(companion)
            .map(|e| e.level)
            .unwrap_or(1);
        let name = self.creature_label(companion);
        let mut lines = vec![(
            MessageKind::LevelUp,
            format!("{name} gains {} XP, reaching level {level}.", tally.xp),
        )];
        lines.extend(
            progression::stat_block(&tally.gain.stat_rows(&stats))
                .into_iter()
                .map(|line| (MessageKind::LevelUp, line)),
        );
        lines
    }

    /// Announces how the fight ended and what it paid: the outcome headline,
    /// the last decompile verdict if one is outstanding, one salvage tally,
    /// then one XP line per fighter that earned anything.
    ///
    /// Called at the top of `end_battle`, which puts it ahead of two things
    /// deliberately. Ahead of `dissolve_tamed_program`, because a companion
    /// that died winning the fight is dropped from `Party` and despawned
    /// there, and this is the last moment its levels can be named at all.
    /// And ahead of `retain_outcomes_since_battle`, whose four surviving
    /// kinds are exactly the ones these lines carry — a tally written after
    /// the prune would be in the right place and the wrong order.
    ///
    /// One flush point covers a win and a jack-out alike, because
    /// `end_battle` is the only place `BattleState` is dropped: you keep what
    /// you killed before you ran.
    pub(crate) fn settle_rewards(&mut self) {
        let mut rewards = {
            let Some(mut battle) = self.world.get_resource_mut::<BattleState>() else {
                return;
            };
            std::mem::take(&mut battle.rewards)
        };
        // First of the results, so it reads directly under the blow that
        // ended the fight.
        //
        // **Won is read off the enemies, never off the player** — the same
        // one definition `end_battle`'s telemetry takes a few lines later,
        // and for its reason: a defeat is absorbed inside the round that
        // lands it by `difficulty::death_handling_system`, so the player's
        // HP afterwards says nothing about the outcome. `finish_member` only
        // reaches `end_battle` once `remove_member` has emptied the last
        // group, which is what makes an empty roster the win.
        //
        // Only a win gets a headline, and that is not an omission. The other
        // two ways out already declare themselves in this exact slot, one
        // line higher: `battle_flee` logs "You jack out safely." (or the
        // counter-strike wording) immediately before calling `end_battle`,
        // and a flatline is announced by `death_handling_system` inside the
        // round that lands it. A win was the only silent ending.
        if self.world.resource::<BattleState>().groups.is_empty() {
            self.log_kind(MessageKind::Outcome, "You won!");
        }
        // Above the payout, deliberately: it is the answer to what the
        // catalysts were being spent on, and the narration it stands in for
        // ran before any of the kills did.
        if let Some(verdict) = rewards.decompile_verdict.take() {
            self.log_kind(MessageKind::Outcome, verdict);
        }
        // Sorted rather than left in the order things fell, so the same haul
        // reads the same way however the kills happened to order it.
        rewards.drops.sort_by(|a, b| a.0.cmp(&b.0));
        self.announce_drops(&rewards.drops);
        self.announce_xp(&rewards);
    }

    /// The experience block: a header and one indented line per fighter that
    /// earned something, matching the shape `announce_drops` gives salvage.
    ///
    /// The lines are **built before the header is written**, so a header can
    /// never stand over an empty block. Asking two predicates whether
    /// anything is about to print would be a second copy of the guards inside
    /// `player_xp_lines` and `companion_xp_lines`, and the copy that drifts
    /// is the one nobody runs.
    fn announce_xp(&mut self, rewards: &BattleRewards) {
        let mut lines = self.player_xp_lines(&rewards.player);
        for (companion, tally) in &rewards.companions {
            lines.extend(self.companion_xp_lines(*companion, tally));
        }
        if lines.is_empty() {
            return;
        }
        self.log_kind(MessageKind::Outcome, "Experience:");
        for (kind, line) in lines {
            self.log_kind(kind, format!("  {line}"));
        }
    }

    /// Defeated (not tamed) rogue programs no longer pay their species'
    /// `work_resource` directly — see
    /// `docs/superpowers/specs/2026-09-04-program-extraction-design.md`
    /// section 5. This is what replaces the old `roll_work_resource_drop`
    /// for a kill in front of the player: the roll goes straight into the
    /// store, because the player is standing there to pick it up.
    ///
    /// `false` when the store is full (`tuning::MAX_DOWNED_PROGRAMS`):
    /// spec decision 9 is that the drop is refused and nothing already held
    /// is destroyed, never that the worst program on hand is dropped to
    /// make room.
    ///
    /// The roll lives in `downed_program_for` so a sortie can bank the
    /// *identical* one onto `Sortie::programs` (`game/sortie.rs`) rather
    /// than growing a second copy of it — `Perk::Teardown`'s old trap, and
    /// the whole reason this is one call and not two.
    pub(crate) fn leave_downed_program(&mut self, wild: Entity) -> bool {
        self.downed_program_for(wild)
            .is_some_and(|program| self.push_downed_program(program))
    }

    /// What a defeat is worth, with no opinion about where it lands — the
    /// one roll, shared by the kill in front of the player and the kill six
    /// screens away.
    ///
    /// Boss and rarity are read off `wild` itself
    /// (`is_boss_creature`/`rarity_of`), so this must run before `wild`
    /// despawns — `award_loot`'s call site does, and so does the sortie's.
    /// Level has no source on a wild `Creature` (unlike a companion, it
    /// never carries `Experience`), so it comes from `ability_user_level`
    /// (`game/combat.rs`) — the same "no `Experience`, read `ZoneLevel`
    /// instead" answer `manifest_accuracy`/`manifest_evasion` already give a
    /// wild program, rather than a second one invented here. Known gap: a
    /// Stack kill still reads the *surface* `ZoneLevel`, since depth carries
    /// no level of its own — `ability_user_level`'s existing limitation, not
    /// a new one.
    ///
    /// **Rarity is floored before the condition roll, not after.** A boss's
    /// rarity is raised to `BOSS_RARITY_FLOOR` first, so `roll_condition`
    /// prices condition against the rarity the program actually ships with.
    /// Rolling condition off the pre-floor rarity and raising rarity only
    /// afterward would leave `grade()` — which folds both — understated for
    /// exactly the bosses this floor exists to protect.
    pub(crate) fn downed_program_for(&mut self, wild: Entity) -> Option<DownedProgram> {
        let species = self
            .world
            .get::<Creature>(wild)
            .map(|c| c.species.clone())?;
        let boss = self.is_boss_creature(wild);
        let mut rarity = self.rarity_of(wild);
        if boss {
            rarity = rarity.max(crate::tuning::BOSS_RARITY_FLOOR);
        }
        let level = self.ability_user_level(wild);
        let overkill_term = self.overkill_term(wild);
        let mut condition = DownedProgram::roll_condition(rarity, boss, overkill_term);
        if boss {
            condition = condition.max(crate::tuning::BOSS_CONDITION_FLOOR);
        }
        Some(DownedProgram {
            species,
            level,
            rarity,
            boss,
            condition,
        })
    }

    /// The one writer of `DownedPrograms`'s `Vec` itself — `leave_downed_program`
    /// for an ordinary kill, `grant_nest_cache` for a nest's own bonus
    /// programs, `return_sortie` for what a squad carried home. A full store
    /// logs the refusal (spec decision 9) rather than silently dropping the
    /// program on the floor, the same courtesy every other refusal in the
    /// game gets.
    pub(crate) fn push_downed_program(&mut self, program: DownedProgram) -> bool {
        let player = self.player_entity();
        let full = self
            .world
            .get::<DownedPrograms>(player)
            .is_some_and(|held| held.0.len() >= crate::tuning::MAX_DOWNED_PROGRAMS);
        if full {
            self.log_kind(
                MessageKind::Outcome,
                "No room to carry another downed program — the store is full.",
            );
            return false;
        }
        self.world
            .get_mut::<DownedPrograms>(player)
            .unwrap()
            .0
            .push(program);
        true
    }

    /// The spec's `overkill_term`: how far the killing blow went past zero,
    /// as a negative fraction of `max_hp`. Read directly off `wild`'s
    /// current `Stats` rather than threaded through from the swing that
    /// killed it — `lower_hp` (`combat_damage.rs`) clamps `hp` to zero
    /// before `award_loot` ever runs, so on the ordinary kill path this is
    /// always `0.0`, the formula's identity value. `FIGHT_CONDITION_WEIGHT`
    /// being `0.0` too is what makes that harmless rather than a bug: the
    /// term genuinely varies for a caller that sets `Stats::hp` negative
    /// directly (a test, not a live kill), which is what lets
    /// `DownedProgram::roll_condition`'s independence from it be asserted
    /// against real variation instead of a term that can never move.
    fn overkill_term(&self, wild: Entity) -> f32 {
        let Some(stats) = self.world.get::<Stats>(wild) else {
            return 0.0;
        };
        if stats.max_hp <= 0 {
            return 0.0;
        }
        let past_zero = (-stats.hp).max(0) as f32;
        -(past_zero / stats.max_hp as f32)
    }

    pub(crate) fn award_loot(&mut self, wild: Entity) {
        let Some(species_id) = self.world.get::<Creature>(wild).map(|c| c.species.clone()) else {
            return;
        };
        let Some(species) = self.world.resource::<SpeciesDb>().get(&species_id).cloned() else {
            return;
        };

        self.leave_downed_program(wild);

        for (item, chance) in self.equipment_drops_for(&species) {
            let roll = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(chance.clamp(0.0, 1.0) as f64)
            };
            if roll {
                let copy = self.grant_gear_drop(item, Rarity::Ordinary);
                self.record_drop(copy, 1);
            }
        }

        // Underground this may be the stack's guardian going down, and this
        // is the one point that knows it actually died rather than being
        // fled from. `wild` is passed on because most of what dies in a lair
        // is the escort standing beside it.
        self.mark_lair_cleared(wild);

        // Same "it actually died" guarantee, spent on the other thing that
        // needs it. `raise_trace` no-ops on the surface, which is where the
        // overwhelming majority of these calls come from.
        self.raise_trace(crate::tuning::TRACE_PER_KILL);

        // Fourth consumer of the same guarantee, and the whole instrumentation
        // cost of contracts: a `Terminate` objective is the one thing about a
        // contract that is event-shaped rather than polled. It sits beside the
        // boss record below rather than anywhere else so the two cannot drift
        // about what counts as a kill, and it is a separate field because each
        // is drained by exactly one system.
        self.world
            .resource_mut::<crate::resources::RunFeats>()
            .kills
            .push(species_id.clone());

        // Through the one door rather than off the species: a rolled boss is
        // an ordinary species carrying `components::Boss`, and would have paid
        // nothing here.
        if self.is_boss_creature(wild) {
            // Third consumer of the same "it actually died" guarantee. The
            // record is all that happens here: what it earned is
            // `achievement_system`'s to decide, in this same tick.
            self.world
                .resource_mut::<crate::resources::RunFeats>()
                .bosses_defeated
                .push(species_id.clone());

            match self.stack_pos() {
                Some(pos) => {
                    self.pay_stack_boss_fragments(pos.depth);
                    self.pay_stack_boss_privilege_ring();
                }
                None => self.pay_surface_boss_gear(),
            }
        }
    }

    /// The breaching currency, and the only place in the game that pays it
    /// (`STACK_BOSS_PORTAL_FRAGMENT_DROP`). Reached only from `award_loot`'s
    /// boss branch while `Locale::Stack` is live, where a boss can only be a
    /// lair guardian — so this is what the party went down for.
    fn pay_stack_boss_fragments(&mut self, depth: u32) {
        let qty = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(STACK_BOSS_PORTAL_FRAGMENT_DROP) * depth
        };
        let landed = self.grant_loot(self.craft_currency(), qty, LootSource::Kill);
        self.record_drop(GearCopy::plain(self.craft_currency()), landed);
    }

    /// The one source of a companion's level ceiling
    /// (`STACK_BOSS_PRIVILEGE_RING_DROP`), and the second thing a lair
    /// guardian pays that a surface boss does not. It rides the same
    /// `is_boss_creature`-and-underground gate as the fragments above rather
    /// than opening a door of its own: what the party went down for is one
    /// question with two answers.
    ///
    /// No `GameRng` draw at all, so adding it moved no seeded roll in the
    /// game — see `STACK_BOSS_PRIVILEGE_RING_DROP` for why the count is flat.
    fn pay_stack_boss_privilege_ring(&mut self) {
        let ring = ItemId::from(crate::items::ids::PRIVILEGE_RING);
        let landed = self.grant_loot(
            ring.clone(),
            crate::tuning::STACK_BOSS_PRIVILEGE_RING_DROP,
            LootSource::Kill,
        );
        self.record_drop(GearCopy::plain(ring), landed);
    }

    /// What a boss killed on the surface pays instead: gear from
    /// `surface_boss_loot`'s zone band, on top of the species' own
    /// `equipment_drops_for` rolls that every kill gets.
    ///
    /// Drawn with replacement, so a zone whose band happens to be thin pays
    /// the same *number* of items as a rich one — a boss is a wall wherever
    /// it is met, and the band already says how good the items are.
    fn pay_surface_boss_gear(&mut self) {
        let pool = self.surface_boss_loot();
        if pool.is_empty() {
            return;
        }
        for _ in 0..SURFACE_BOSS_LOOT_DROPS {
            let item = {
                let mut rng = self.world.resource_mut::<GameRng>();
                pool[rng.0.random_range(0..pool.len())].clone()
            };
            let copy = self.grant_gear_drop(item, SURFACE_BOSS_LOOT_RARITY_FLOOR);
            self.record_drop(copy, 1);
        }
    }

    /// The pool a defeated surface boss draws from: every equippable item
    /// whose `ItemDef::value` sits in this zone's band, which is
    /// `SURFACE_BOSS_LOOT_VALUE_PER_ZONE` per zone wide at the top and
    /// `SURFACE_BOSS_LOOT_BAND_FLOOR_PERCENT` of that at the bottom.
    ///
    /// Derived from `value` rather than a new schema field, so a modded item
    /// joins the pool by existing and the ladder documented in
    /// `assets/items/README.md` is the single place a tier is declared. The
    /// equipment filter is what keeps non-gear that happens to be worth the
    /// same — an Access Shard is worth 12, exactly a Hardened Shell — out of
    /// a payout that is supposed to make the party stronger.
    ///
    /// A band that selects nothing falls back to the best gear there is
    /// rather than paying nothing: the ceiling climbs forever but the ladder
    /// does not, so a deep enough run would otherwise walk off the top of it.
    ///
    /// Sorted by id so a seeded run consumes its draws in the same order
    /// however the item files happen to load — the same guarantee
    /// `equipment_drops_for` and `open_cache` make.
    pub(crate) fn surface_boss_loot(&self) -> Vec<ItemId> {
        let zone = self.world.resource::<ZoneLevel>().0;
        let ceiling = SURFACE_BOSS_LOOT_VALUE_PER_ZONE.saturating_mul(zone);
        let floor = ceiling * SURFACE_BOSS_LOOT_BAND_FLOOR_PERCENT / 100;

        let gear: Vec<(ItemId, u32)> = self
            .world
            .resource::<ItemDb>()
            .all()
            .filter(|def| def.equipment.is_some())
            .map(|def| {
                (
                    def.id.clone(),
                    def.value.unwrap_or(crate::tuning::DEFAULT_ITEM_VALUE),
                )
            })
            .collect();
        let mut pool: Vec<ItemId> = gear
            .iter()
            .filter(|(_, value)| (floor..=ceiling).contains(value))
            .map(|(id, _)| id.clone())
            .collect();
        if pool.is_empty() {
            let best = gear.iter().map(|&(_, value)| value).max().unwrap_or(0);
            pool = gear
                .iter()
                .filter(|&&(_, value)| value == best)
                .map(|(id, _)| id.clone())
                .collect();
        }
        pool.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        pool
    }

    /// What defeating `victim` is worth: its whole HP bar, scaled by
    /// `progression::kill_xp`'s challenge factor.
    ///
    /// The one place the two powers behind that factor are gathered, so a
    /// third award site cannot quietly price a kill off something else — the
    /// two that exist (a kill in `combat_round`, a decompile here) differ
    /// only in how the program stopped fighting.
    ///
    /// The denominator is the player's power **alone**, deliberately not the
    /// party's. A companion makes a fight easier, so counting the roster in
    /// would dock the player XP for recruiting one — turning the party into
    /// a cost, when it is the point.
    ///
    /// Must be read before the victim's `Stats` can change; the decompile
    /// caller takes it while the program is still hostile, for that reason.
    pub(crate) fn kill_xp(&self, victim: Entity) -> u32 {
        let Some(victim_stats) = self.world.get::<Stats>(victim) else {
            return 0;
        };
        let player_power = self
            .world
            .get::<Stats>(self.player_entity())
            .map(|s| s.power())
            .unwrap_or(1);
        progression::kill_xp(
            victim_stats.max_hp,
            crate::game::inspection::power_ratio(victim_stats.power(), player_power),
        )
    }

    /// Awards `amount` XP to the player, growing stats and fully healing on
    /// any level-up gained, then awards every current party member half as
    /// much (see `award_party_xp`) — fighting beside you pays off even on
    /// rounds where only the player's hit actually lands. Silently does
    /// nothing for the player if they're somehow missing an `Experience`
    /// component (shouldn't happen in practice).
    pub(crate) fn award_player_xp(&mut self, player: Entity, amount: u32) {
        // `XpBoost` is `FieldScope::Run`, so it reads off the player
        // regardless of whether `player` here is the player themself (the
        // only caller today, but the parameter doesn't guarantee it).
        let xp_boost_pct = self.field_buff_power(self.player_entity(), FieldBuffKind::XpBoost);
        let level_cap = self.level_cap();
        let (gain, new_level) = {
            let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
            let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, player) else {
                return;
            };
            let gain = progression::add_xp(
                &mut exp,
                &mut stats,
                amount,
                crate::tuning::BASELINE_GROWTH_MULTIPLIER,
                // The player is capped too now, and at the same number as
                // every companion — `Game::level_cap`.
                Some(level_cap),
                xp_boost_pct,
            );
            (gain, exp.level)
        };
        let mut tally = XpTally {
            xp: amount,
            gain,
            ..XpTally::default()
        };
        // At the cap the XP is banked rather than spent, and this is the one
        // place it is drained. Folded into the same tally the level-up path
        // writes, so the Perk Points a player earned show up wherever they
        // already showed up — a point earned by overflow reads no differently
        // from one earned by levelling, because it is not different.
        if gain.overflow > 0 {
            tally.perk_points += self.convert_overflow_xp();
        }
        if gain.levels > 0 {
            // The player's tally runs longer than a companion's: a level also
            // pays a Perk Point and a point of Decompiler skill, and neither
            // was announced anywhere before this, so a player could bank
            // points for a run without learning they had any.
            if let Some(mut perks) = self.world.get_mut::<Perks>(player) {
                tally.perk_points = PERK_POINTS_PER_LEVEL * gain.levels;
                perks.points += tally.perk_points;
            }
            if let Some(mut decompiler) = self.world.get_mut::<Decompiler>(player) {
                tally.decompiler = DECOMPILER_SKILL_PER_LEVEL * gain.levels as i32;
                decompiler.skill += tally.decompiler;
            }
        }
        // The level itself is announced where it happens and the totals wait
        // for `settle_rewards`: `add_xp` full-heals on a level, so a player
        // watching their HP snap back mid-fight needs the cause on screen
        // then. Outside a battle there is nothing to wait for, and the tally
        // is announced on the spot through the same formatter — see
        // `record_drop` for why that fallback is a formatter call rather than
        // a second wording.
        let stored = self
            .world
            .get_resource_mut::<BattleState>()
            .map(|mut b| b.rewards.player.absorb(&tally))
            .is_some();
        if !stored {
            // Unindented and with no `Experience:` header: outside a fight
            // these are standalone news, not rows in a results block.
            for (kind, line) in self.player_xp_lines(&tally) {
                self.log_kind(kind, line);
            }
        } else if gain.levels > 0 {
            self.log_kind(
                MessageKind::LevelUp,
                format!("You reach level {new_level}!"),
            );
        }
        self.award_party_xp(amount / PARTY_XP_DIVISOR);
    }

    /// Awards `amount` XP to every program in the active party (see
    /// `resources::Party`), each independently able to level up from it —
    /// the party-wide, half-rate companion to `award_player_xp`. A no-op
    /// for any party member somehow missing `Experience` (shouldn't happen
    /// in practice) or if the party is empty. Only logs a level-up, not
    /// every ordinary gain, so a busy fight doesn't flood the feed with a
    /// line per party member per kill.
    pub(crate) fn award_party_xp(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        let party = self.world.resource::<Party>().0.clone();
        for companion in party {
            self.award_companion_xp(companion, amount);
        }
    }

    /// One owned program's share of a kill, grown, levelled and announced.
    ///
    /// Split out of `award_party_xp` rather than copied beside it: a sortie
    /// pays its squad the same way a fight beside the player pays the party,
    /// and a second copy of the growth roll, the cap and the tally is
    /// exactly the drifted-formula trap this repo keeps falling into. What
    /// differs between the two callers is *who* is paid and how much, which
    /// is what the parameters are.
    pub(crate) fn award_companion_xp(&mut self, companion: Entity, amount: u32) {
        if amount == 0 {
            return;
        }
        let xp_boost_pct = self.field_buff_power(self.player_entity(), FieldBuffKind::XpBoost);
        let species_growth = self
            .world
            .get::<Creature>(companion)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|s| s.growth_multiplier)
            .unwrap_or(crate::tuning::BASELINE_GROWTH_MULTIPLIER);
        let individual_roll = self
            .world
            .get::<Potential>(companion)
            .map(|p| p.growth_roll)
            .unwrap_or(Potential::NEUTRAL.growth_roll);
        let growth_multiplier = species_growth * individual_roll;
        let before_level = self
            .world
            .get::<Experience>(companion)
            .map(|e| e.level)
            .unwrap_or(1);
        let level_cap = self.level_cap();
        let gain = {
            let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
            let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, companion) else {
                return;
            };
            progression::add_xp(
                &mut exp,
                &mut stats,
                amount,
                growth_multiplier,
                Some(level_cap),
                xp_boost_pct,
            )
        };
        let level = self
            .world
            .get::<Experience>(companion)
            .map(|e| e.level)
            .unwrap_or(before_level);
        let tally = XpTally {
            xp: amount,
            gain,
            ..XpTally::default()
        };
        let stored = self.record_companion_xp(companion, &tally);
        if !stored {
            for (kind, line) in self.companion_xp_lines(companion, &tally) {
                self.log_kind(kind, line);
            }
        } else if gain.levels > 0 {
            let name = self.creature_label(companion);
            self.log_kind(
                MessageKind::LevelUp,
                format!("{name} reaches level {level}!"),
            );
        }
        if gain.levels > 0 {
            self.install_unlocked_routines(companion, before_level, level);
        }
    }

    /// One decompile attempt against `group`'s front program: spends a
    /// catalyst, rolls `taming::capture_chance`, and on success converts the
    /// target into a tamed program and drops it from the group. Returns
    /// whether that ended the battle.
    ///
    /// The roster-full refusal lives in `ability_unavailable` alone now: a
    /// greyed row can't be planned, and `battle_set_action` refuses one that
    /// somehow is, and nothing inside a resolving round grows `pet_count`
    /// except a successful decompile itself, so that state can't reach here.
    ///
    /// The no-catalyst guard below stays, though: `ability_unavailable`
    /// checks it per slot at *plan* time, but the catalyst is a round-wide
    /// pool, not a per-slot one — two party members can each plan Decompile
    /// while only one catalyst is held, both pass the per-slot check, and the
    /// first to resolve spends the only copy. Without this guard the second
    /// would hit an `expect` instead of a refusal.
    pub(crate) fn attempt_decompile(&mut self, group: usize, player: Entity) -> bool {
        let Some((catalyst, potency)) = self.taming_catalyst() else {
            self.log_kind(
                MessageKind::Outcome,
                "No taming catalyst left — the decompile attempt fizzles.",
            );
            return false;
        };
        let Some(front) = self.front_of_group(group) else {
            return false;
        };
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(catalyst, 1);

        let bonuses = self.player_decompiler_bonuses();
        // Read before the increment below, deliberately: the count this
        // rolls against is the count the battle screen has been showing all
        // along, so the odds cell is always exactly what the next attempt
        // gets rather than what the last one got.
        let resistance = self.target_resistance(front).unwrap();
        let chance = taming::capture_chance(potency, resistance, bonuses);
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance as f64)
        };
        // The chain's decompile mission cannot be failed: a run of bad rolls
        // would end onboarding permanently. The catalyst above is already
        // spent, so only the roll is forced — the lesson that decompiling is
        // priced in catalysts is the half that stays.
        //
        // Below the odds read, deliberately, so what the battle screen has
        // been showing stays honest about what the roll would have been.
        let roll = roll || self.tutorial_grants_capture();
        let attempts = {
            let mut battle = self.world.resource_mut::<BattleState>();
            let counter = battle.decompile_attempts.entry(front).or_insert(0);
            *counter += 1;
            *counter
        };

        if !roll {
            // Naming the cap matters: without it a player reads a rising
            // number and keeps feeding catalysts to a wall.
            let fraying = if attempts < DECOMPILE_ATTEMPT_BONUS_CAP {
                " Its defences fray a little."
            } else {
                " Its defences are as frayed as they will get."
            };
            let verdict = format!("The program's ICE holds — decompile failed!{fraying}");
            // `Info`, so the prune drops the run of them; the copy held for
            // `settle_rewards` is what reaches the summary. See
            // `BattleRewards::decompile_verdict`.
            self.log_kind(MessageKind::Info, verdict.clone());
            self.world
                .resource_mut::<BattleState>()
                .rewards
                .decompile_verdict = Some(verdict);
            return false;
        }
        self.world
            .resource_mut::<BattleState>()
            .rewards
            .decompile_verdict = None;
        self.note_deed(crate::contracts::Deed::Tamed);

        // Taken while the program is still hostile: `kill_xp` reads its
        // `Stats`, and everything below this line is the act of turning it
        // into a companion.
        let earned = self.kill_xp(front);
        let nest = self.world.get::<NestGuardian>(front).map(|g| g.nest);
        self.world
            .entity_mut(front)
            .remove::<(Hostile, WanderAi, NestGuardian, Pursuing)>();
        // Battle-scoped state has to be cleared here rather than left to
        // `end_battle`/`clear_battle_status_effects`: `front` is about to
        // leave its group below, so if other groups are still standing the
        // fight goes on without it ever reaching that teardown, and a
        // mirrored buff or a routine's own cooldown would otherwise ride
        // into the roster and never tick again.
        if let Some(mut s) = self.world.get_mut::<StatusEffects>(front) {
            s.active = None;
        }
        if let Some(mut b) = self.world.get_mut::<CombatBuff>(front) {
            b.active = None;
        }
        if let Some(mut c) = self.world.get_mut::<AbilityCooldowns>(front) {
            c.0.clear();
        }
        let parts = self.roster_parts();
        self.world.entity_mut(front).insert(parts);
        self.install_innate_routines(front);
        if let Some(nest) = nest
            && let Some(mut n) = self.world.get_mut::<Nest>(nest)
        {
            n.pending_respawns.push(NEST_RESPAWN_TICKS);
        }
        self.log_kind(
            MessageKind::Outcome,
            "ICE breached! The program now runs under your control.",
        );
        self.award_player_xp(player, earned);
        // The other way a program leaves a fight and does not come back.
        // `award_loot` carries this for a kill and taming spends no loot, so
        // without it a captured guardian left the lair unspent — refilling
        // on the next visit, over a stack that could never be finished.
        // Unreachable while `battle_set_action` refuses a guardian outright,
        // and kept because the record is what the collapse reads: a third
        // way out of a fight should not have to remember to write it.
        self.mark_lair_cleared(front);
        if self.remove_member(group, 0) {
            self.end_battle(player, Some(front));
            return true;
        }
        self.log("Another rogue program from the pack engages!");
        false
    }

    /// Whether the run's live onboarding mission is the one that teaches
    /// decompiling.
    ///
    /// The one place the chain changes a shipped formula's outcome, and it is
    /// bounded to a single mission of a single run: read off the live mission
    /// rather than a flag, so it disarms itself the moment the chain moves
    /// on and there is no state to leave set.
    ///
    /// Keyed on the **objective**, not the id, so it stays content-driven —
    /// a mod authoring its own `Perform(deed: Tamed)` mission gets the same
    /// guarantee, and renaming the shipped file changes nothing.
    fn tutorial_grants_capture(&self) -> bool {
        self.current_tutorial().is_some_and(|def| {
            matches!(
                def.objective,
                crate::contracts::Objective::Perform {
                    deed: crate::contracts::Deed::Tamed
                }
            )
        })
    }
}
