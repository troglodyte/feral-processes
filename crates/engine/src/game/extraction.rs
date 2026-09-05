//! The extraction door: `Game::extraction_yield` (the one derivation of
//! what a tool draws out of a downed program) and `Game::extract_program`
//! (the one act that spends a program on it) — see
//! `docs/superpowers/specs/2026-09-04-program-extraction-design.md`,
//! sections 3 and 4.

use crate::items::DownedProgram;
use crate::species::SpeciesId;
use crate::tools::{ToolDef, ToolId};
use crate::*;

/// How much each tier past 1 scales `extraction_yield`'s unit count — see
/// `tuning::TOOL_TIER_SCALE_STEP`. `1.0` at tier 1, which is
/// `salvage_clamp`'s own tier, so Task 6's drop-neutrality test (fitted
/// against the starter tool) cannot see this curve move.
///
/// Its argument is the tool's own tier plus the bench's term
/// (`Game::extraction_bench_tier` minus one) — the one shared curve
/// `tuning::TOOL_TIER_SCALE_STEP` says both axes take, rather than a
/// second constant that could drift away from it.
fn tier_scale(tier: u32) -> f32 {
    1.0 + tier.saturating_sub(1) as f32 * tuning::TOOL_TIER_SCALE_STEP
}

/// Splits `units` whole items across `pool` by weight, deterministically —
/// largest-remainder apportionment (Hamilton's method) rather than a draw
/// per unit. `extraction_yield` is `&self` because the screen's preview (a
/// later task) calls it once per installed tool with nothing spent, so it
/// cannot touch the shared `GameRng`; apportioning rather than sampling is
/// also what makes `extract_program` calling this once and granting its
/// `Vec` verbatim *sufficient* to prove the previewed figure and the
/// granted one agree — calling this again on the same inputs always
/// returns the same rows, with no coincidence required.
///
/// Every weight is finite and positive by construction —
/// `ToolDef::invalid_yield_weight` refuses a tool file that isn't, at load
/// — so this doesn't re-guard against a bad pool, only an empty or
/// zero-`units` one.
///
/// Hamilton's method carries the Alabama paradox: with a pool of three or
/// more items, one extra unit (`Perk::Teardown`'s bonus, say) can shrink
/// another item's share rather than only ever adding to the total. Both
/// shipped pools have two items, where the paradox cannot occur — a
/// three-item modded pool exhibiting it is this, not a bug.
fn apportion(pool: &[(ItemId, f32)], units: u32) -> Vec<(ItemId, u32)> {
    if units == 0 || pool.is_empty() {
        return Vec::new();
    }
    let total: f32 = pool.iter().map(|(_, weight)| weight).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut allocated: Vec<(ItemId, u32)> = Vec::with_capacity(pool.len());
    let mut remainders: Vec<(usize, f32)> = Vec::with_capacity(pool.len());
    let mut spent = 0u32;
    for (index, (item, weight)) in pool.iter().enumerate() {
        let exact = units as f32 * weight / total;
        let floor = exact.floor();
        allocated.push((item.clone(), floor as u32));
        remainders.push((index, exact - floor));
        spent += floor as u32;
    }
    // The largest fractional remainder claims a leftover unit first —
    // Hamilton's own tie-break rule — with the pool's own order as the
    // second key so two equal remainders resolve identically on every call
    // rather than by float-comparison happenstance.
    remainders.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut left = units - spent;
    for (index, _) in remainders {
        if left == 0 {
            break;
        }
        allocated[index].1 += 1;
        left -= 1;
    }
    allocated.retain(|(_, qty)| *qty > 0);
    allocated
}

impl Game {
    /// `SpeciesDef::rich_in`, or `work_resource` when the species doesn't
    /// author one — spec decision 5, and the whole reason `rich_in` needed
    /// no authoring pass on the 17 shipped species files: every one keeps
    /// paying exactly what it already dropped. `None` both for a species
    /// that names neither and for a species id `SpeciesDb` cannot resolve
    /// at all (a save naming a mod species since removed).
    pub fn rich_in(&self, species: &SpeciesId) -> Option<ItemId> {
        let def = self.world.resource::<SpeciesDb>().get(species)?;
        def.rich_in.clone().or_else(|| def.work_resource.clone())
    }

    /// The best tier of any standing structure whose
    /// `StructureDef::extracts_programs` is set, or `0` when none stands —
    /// never a gate (spec decision 7), only a term. Ownership rather than
    /// proximity, `Game::can_extract_routines`' rule
    /// (`game/routines.rs:456`) rather than a distance check.
    pub fn extraction_bench_tier(&self) -> u32 {
        self.world
            .resource::<StructureDb>()
            .all()
            .filter(|def| def.extracts_programs)
            .filter_map(|def| self.best_structure_tier(&def.id))
            .max()
            .unwrap_or(0)
    }

    /// The bench a screen names, when one stands. `None` and no name when
    /// none does, rather than a "no bench" string built here — what to say
    /// about an absence is the renderer's business.
    pub fn extraction_bench(&self) -> Option<crate::views::ExtractionBenchView> {
        let tier = self.extraction_bench_tier();
        if tier == 0 {
            return None;
        }
        Some(crate::views::ExtractionBenchView {
            name: self.bench_name(|def| def.extracts_programs),
            tier,
        })
    }

    /// What extracting `program` with `tool` grants — the one derivation,
    /// called by `extract_program` (below) and by the screen's preview (a
    /// later task) alike, so a quoted figure and a granted one cannot
    /// differ.
    ///
    /// `units = round(TOOL_BASE_UNITS * tier_scale(tool.tier + bench) *
    /// program.grade())`, split across `tool.yields` by weight
    /// (`apportion`), plus `Perk::Teardown`'s `salvage_bonus` added to the
    /// unit count as a flat addend — never a second `GameRng` draw, the
    /// discipline the retired `roll_work_resource_drop` followed and this
    /// reasserts (`extraction_yield_spends_no_gamerng_draw_even_with_
    /// teardown_bought`). `rich_in`'s bonus part is added on top,
    /// regardless of the tool's own category, from `tuning::
    /// RICH_IN_UNITS` — merged into an existing row rather than a
    /// duplicate one if the tool's own pool already names the same item
    /// (`scrapper` extracted with `salvage_clamp`, both naming
    /// `core_fragment`, is exactly this case).
    ///
    /// Still no `structure_tier` parameter, now that phase 3 has authored
    /// the structure that would supply one: the tier is read inside, from
    /// `Game::extraction_bench_tier`, because a parameter with exactly one
    /// correct value is how the screen ends up quoting a tier-0 figure
    /// while the act grants a tier-3 one — the divergence this function
    /// being the one derivation exists to prevent.
    pub fn extraction_yield(&self, program: &DownedProgram, tool: &ToolDef) -> Vec<(ItemId, u32)> {
        // The bench's term is `tier - 1`, not `tier` — a bench that has
        // never been upgraded pays nothing, and the upgrade is what sells
        // yield. See `tuning::TOOL_TIER_SCALE_STEP`'s neighbouring doc.
        let bench = self.extraction_bench_tier().saturating_sub(1);
        let scale = tier_scale(tool.tier + bench);
        let base_units = (tuning::TOOL_BASE_UNITS * scale * program.grade()).round() as u32;
        let bonus = crate::perks::salvage_bonus(self.player_perks());
        let units = base_units + bonus;

        let mut granted = apportion(&tool.yields, units);

        if let Some(rich) = self.rich_in(&program.species) {
            match granted.iter_mut().find(|(item, _)| *item == rich) {
                Some(entry) => entry.1 += tuning::RICH_IN_UNITS,
                None => granted.push((rich, tuning::RICH_IN_UNITS)),
            }
        }

        granted
    }

    /// One row per held program, in store order — `Mode::DownedPrograms`'s
    /// whole list. The species' display name falls back to the raw id for a
    /// mod species since removed, `downed_program_label`'s own tolerance,
    /// and `grade` is `DownedProgram::grade()`'s own answer rather than a
    /// second fold of condition/rarity/level built here.
    pub fn downed_program_rows(&self) -> Vec<crate::views::DownedProgramRow> {
        let player = self.player_entity();
        let db = self.world.resource::<SpeciesDb>();
        self.world
            .get::<DownedPrograms>(player)
            .map(|held| {
                held.0
                    .iter()
                    .map(|program| crate::views::DownedProgramRow {
                        name: db
                            .get(&program.species)
                            .map(|def| def.name.clone())
                            .unwrap_or_else(|| program.species.clone()),
                        level: program.level,
                        rarity: program.rarity,
                        condition: program.condition,
                        boss: program.boss,
                        grade: program.grade(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every installed tool and what it would give for the program at
    /// `index` — `extraction_yield` called once per tool, in
    /// `installed_tools`' own slot order, so the screen's preview and
    /// `extract_program`'s grant read off the identical call. Empty for an
    /// index the store doesn't hold, rather than a panic — the same
    /// tolerance `extraction_yield`'s own callers already take on a stale
    /// index.
    pub fn extraction_options(&self, index: usize) -> Vec<(ToolId, Vec<(ItemId, u32)>)> {
        let player = self.player_entity();
        let Some(program) = self
            .world
            .get::<DownedPrograms>(player)
            .and_then(|held| held.0.get(index))
        else {
            return Vec::new();
        };
        self.installed_tools()
            .iter()
            .map(|tool| (tool.id.clone(), self.extraction_yield(program, tool)))
            .collect()
    }

    /// What to call `program` in a log line — its species' display name
    /// (falling back to the raw id for a mod species since removed) and
    /// its level.
    fn downed_program_label(&self, program: &DownedProgram) -> String {
        let name = self
            .world
            .resource::<SpeciesDb>()
            .get(&program.species)
            .map(|def| def.name.as_str())
            .unwrap_or(program.species.as_str());
        format!("the level {} {name}", program.level)
    }

    /// The one door a downed program is consumed through — spec section 4.
    /// Refusals, in order, all before anything is spent: the run is over
    /// or a battle is active, `index` names no held program, `tool` isn't
    /// installed. Then the program is removed, `extraction_yield` is
    /// called exactly once and its `Vec` granted verbatim through
    /// `grant_loot` under `LootSource::Extract`, one log line, and finally
    /// `self.tick()` `tool.ticks` times — `commit_caravan_basket`'s
    /// ordering (refusals, then spend, then the log, then the tick cost).
    ///
    /// The tick loop breaks early on a game over or a battle opening
    /// mid-spend (`nest_aggro_tick` is precedent for either), the same
    /// shape a `Drag` step's own multi-tick loop takes — nothing here
    /// needs to unwind, since everything it could interrupt already
    /// landed before the first tick was spent.
    pub fn extract_program(&mut self, index: usize, tool: &ToolId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".to_string());
        }
        let player = self.player_entity();
        let program = self
            .world
            .get::<DownedPrograms>(player)
            .and_then(|held| held.0.get(index))
            .cloned()
            .ok_or_else(|| "No such downed program.".to_string())?;
        let tool_def = self
            .installed_tools()
            .into_iter()
            .find(|def| &def.id == tool)
            .ok_or_else(|| "That tool isn't installed.".to_string())?;

        let granted = self.extraction_yield(&program, &tool_def);

        self.world
            .get_mut::<DownedPrograms>(player)
            .unwrap()
            .0
            .remove(index);

        for (item, qty) in &granted {
            self.grant_loot(item.clone(), *qty, LootSource::Extract);
        }

        let label = self.downed_program_label(&program);
        if granted.is_empty() {
            self.log_kind(
                MessageKind::Loot,
                format!(
                    "You strip {label} down with the {} and salvage nothing usable.",
                    tool_def.name
                ),
            );
        } else {
            let parts: Vec<String> = granted
                .iter()
                .map(|(item, qty)| format!("{qty} {}", self.item_name(item)))
                .collect();
            self.log_kind(
                MessageKind::Loot,
                format!(
                    "You strip {label} down with the {}: {}.",
                    tool_def.name,
                    parts.join(", ")
                ),
            );
        }

        for _ in 0..tool_def.ticks {
            if self.is_game_over().is_some() || self.has_active_battle() {
                break;
            }
            self.tick();
        }

        Ok(())
    }
}
