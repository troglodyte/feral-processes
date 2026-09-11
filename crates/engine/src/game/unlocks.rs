//! Perk and research progression — what the player has unlocked and what
//! unlocking costs.

use crate::game::inspection::power_ratio;
use crate::resources::ActiveResearch;
use crate::taming::{DecompilerBonuses, TargetResistance};
use crate::tuning::DEFAULT_TAMING_DIFFICULTY;
use crate::*;

impl Game {
    /// How many levels of `perk` the player has bought — 0 if none.
    pub fn player_perk_level(&self, perk: Perk) -> u32 {
        self.player_perks().map(|p| p.level(perk)).unwrap_or(0)
    }

    /// The player's bought perks, for the `perks::` queries to read.
    ///
    /// `None` rather than a default is what those queries expect: every one
    /// of them answers 0 for an entity that has never bought anything, so
    /// no caller needs a branch of its own.
    pub(crate) fn player_perks(&self) -> Option<&Perks> {
        self.world.get::<Perks>(self.player_entity())
    }

    /// Everything the player brings to a decompile attempt: their real
    /// `Decompiler` stat (levels plus equipment), whatever
    /// `Perk::ExploitFocus` has bought off the target's HP penalty, and any
    /// running `CaptureBoost` field buff. The one place all three are
    /// assembled — all three `taming::capture_chance` call sites go through
    /// here, so the odds a player is shown and the odds they are rolled
    /// against cannot drift apart.
    pub(crate) fn player_decompiler_bonuses(&self) -> DecompilerBonuses {
        let player = self.player_entity();
        DecompilerBonuses {
            skill: self
                .world
                .get::<Decompiler>(player)
                .map(|d| d.skill)
                .unwrap_or(0),
            hp_penalty_reduction: crate::perks::decompile_hp_penalty_reduction(self.player_perks()),
            // The one place the two whole-attempt boosts are summed: a
            // running `CaptureBoost` field buff, and what the player's class
            // is worth. Both scale the entire attempt, so they belong in the
            // same field rather than as a fourth multiplier of the same shape.
            capture_boost_pct: self.field_buff_power(player, FieldBuffKind::CaptureBoost)
                + crate::classes::capture_boost_pct(self.player_class()),
        }
    }

    /// Everything the target brings to a decompile attempt: its remaining
    /// Integrity, its species' resistance, how many attempts this fight has
    /// already spent on it, and how its `Stats::power` compares to the
    /// player's. The mirror of `player_decompiler_bonuses` above and the one
    /// place these four are assembled, for the same reason — the two call
    /// sites that *show* odds and the one that *rolls* them must not drift
    /// apart, and `prior_attempts` is exactly the sort of term a display
    /// would forget.
    ///
    /// A species the `SpeciesDb` doesn't know falls back to
    /// `DEFAULT_TAMING_DIFFICULTY` rather than refusing, which is what the
    /// battle view already did.
    pub(crate) fn target_resistance(&self, entity: Entity) -> Option<TargetResistance> {
        let stats = self.world.get::<Stats>(entity)?;
        Some(TargetResistance {
            hp_fraction: stats.hp_fraction(),
            taming_difficulty: self
                .world
                .get::<Creature>(entity)
                .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
                .map(|s| s.taming_difficulty)
                .unwrap_or(DEFAULT_TAMING_DIFFICULTY),
            prior_attempts: self.decompile_attempts(entity),
            power_ratio: power_ratio(stats.power(), self.player_power()) as f32,
        })
    }

    /// The taming catalyst a decompile attempt would spend, paired with its
    /// `ItemDef::taming_potency`: whichever item in the player's inventory
    /// declares the highest potency. Which item that is comes purely from
    /// the item data, so a mod's stronger catalyst wins over a shipped one
    /// without any code knowing its id. Ties break on item id, so a stocked
    /// pair of equal catalysts always spends the same stack first. `None`
    /// when the player carries no catalyst at all — the single source of
    /// truth for "decompiling isn't available right now".
    pub(crate) fn taming_catalyst(&self) -> Option<(ItemId, f32)> {
        let db = self.world.resource::<ItemDb>();
        let inv = self.world.get::<Inventory>(self.player_entity())?;
        inv.items
            .iter()
            .filter(|(_, qty)| *qty > 0)
            .filter_map(|(id, _)| {
                db.get(id.as_str())
                    .and_then(|d| d.taming_potency)
                    .map(|potency| (id.clone(), potency))
            })
            .max_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| b.0.as_str().cmp(a.0.as_str()))
            })
    }

    /// Drains banked overflow XP into Perk Points, returning how many were
    /// minted.
    ///
    /// XP earned at the level cap accumulates in `Experience::xp` (see
    /// `progression::add_xp`) instead of being discarded. This is the only
    /// thing that spends it, and only for the player — a companion has no
    /// `Perks` at all, so its overflow just sits there, which is the
    /// behaviour every creature had before the cap existed.
    ///
    /// The price rises with perks ever bought, `OVERFLOW_XP_STEP`'s reason:
    /// flat, this is an unbounded linear power source. It re-reads the price
    /// after every point, so one call cannot mint a run's worth at the
    /// opening rate.
    pub(crate) fn convert_overflow_xp(&mut self) -> u32 {
        let player = self.player_entity();
        let mut minted = 0;
        loop {
            // Off `BoughtStats::ever_bought` rather than `Perks::unlocked`,
            // because a respec empties that list and would reset this
            // escalator to the opening rate — see the field's own doc. The
            // `Perks` lookup stays as the "is this the player" gate it
            // always was.
            if self.world.get::<Perks>(player).is_none() {
                return minted;
            }
            let held = self
                .world
                .get::<BoughtStats>(player)
                .map_or(0, |b| b.ever_bought);
            let price =
                crate::tuning::OVERFLOW_XP_BASE + crate::tuning::OVERFLOW_XP_STEP * (held + minted);
            let Some(mut exp) = self.world.get_mut::<Experience>(player) else {
                return minted;
            };
            if exp.xp < price {
                return minted;
            }
            exp.xp -= price;
            minted += 1;
            if let Some(mut perks) = self.world.get_mut::<Perks>(player) {
                perks.points += 1;
            }
        }
    }

    /// Spends Perk Points to buy another level of `perk` (see
    /// `perks::Perk`). Perks are repeatable — there's no cap on levels,
    /// only on how many Perk Points you've earned.
    ///
    /// Name and price come from `PerkDb`, so a perk whose `.ron` file is
    /// missing or malformed can't be bought at all — the same state the
    /// picker shows by leaving it out of the list.
    pub fn unlock_perk(&mut self, perk: Perk) -> Result<(), String> {
        if self.is_game_over().is_some() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let (name, cost) = {
            let def = self
                .world
                .resource::<PerkDb>()
                .get(perk)
                .ok_or_else(|| "That perk isn't available.".to_string())?;
            (def.name.clone(), def.cost)
        };
        let level = {
            let mut perks = self
                .world
                .get_mut::<Perks>(player)
                .ok_or_else(|| "No perks available.".to_string())?;
            if perks.points < cost {
                return Err(format!(
                    "Not enough Perk Points (need {cost}, have {}).",
                    perks.points
                ));
            }
            perks.points -= cost;
            perks.unlocked.push(perk);
            perks.level(perk)
        };
        // What the gain *is* belongs to the perk; writing it belongs here,
        // so `Stats` keeps one writer. `Buffer` reads the current maximum,
        // which is why the gain is asked for after the purchase.
        let max_hp = self
            .world
            .get::<Stats>(player)
            .map(|s| s.max_hp)
            .unwrap_or(0);
        let mut receipt = self
            .world
            .get::<BoughtStats>(player)
            .copied()
            .unwrap_or_default();
        // Counted for every perk, not just the three that move a stat: this
        // is what `convert_overflow_xp` prices against, and a respec must not
        // be able to lower it.
        receipt.ever_bought += 1;
        if let Some(gain) = crate::perks::purchase_stat_gain(perk, max_hp)
            && let Some(mut stats) = self.world.get_mut::<Stats>(player)
        {
            // The receipt is written from the same value that reaches
            // `Stats`, in the same branch, so the two cannot disagree about
            // what this purchase was worth — the drift a second computation
            // of `purchase_stat_gain` would invite.
            match gain {
                crate::perks::StatGain::Atk(n) => {
                    stats.atk += n;
                    receipt.atk += n;
                }
                crate::perks::StatGain::Mitigation(n) => {
                    stats.mitigation += n;
                    receipt.mitigation += n;
                }
                crate::perks::StatGain::MaxHp(n) => {
                    stats.max_hp += n;
                    stats.hp = stats.max_hp;
                    receipt.max_hp += n;
                }
            }
        }
        self.world.entity_mut(player).insert(receipt);
        self.log(format!("You buy the {name} perk (level {level})."));
        self.note_deed(crate::contracts::Deed::UnlockedPerk);
        Ok(())
    }

    pub fn is_researched(&self, id: &str) -> bool {
        self.world.resource::<Research>().0.contains(id)
    }

    /// Display names of `def`'s prerequisites that aren't unlocked yet, in
    /// the order the file lists them.
    pub(crate) fn missing_prereqs(&self, def: &ResearchDef) -> Vec<String> {
        let db = self.world.resource::<ResearchDb>();
        def.requires
            .iter()
            .filter(|id| !self.is_researched(id))
            .map(|id| {
                db.get(id)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| id.clone())
            })
            .collect()
    }

    /// The zone `def` is still waiting on, or `None` if the party has already
    /// reached it. Read the same way `Game::upgrade_ceiling` reads it, and
    /// the one definition of the gate — `research_nodes` explains it and
    /// `select_research` refuses on it, so a menu cannot promise a node the
    /// purchase would turn down.
    fn research_zone_gate(&self, def: &ResearchDef) -> Option<u32> {
        (def.min_zone > self.world.resource::<ZoneLevel>().0).then_some(def.min_zone)
    }

    /// Every research node, ordered the way the menu shows them: the active
    /// project first, then available, then locked, then already-unlocked, each
    /// group cheapest-first
    /// (see `ResearchDb::all`). Ordering lives here rather than in each
    /// renderer so both peers agree on what `[3]` means.
    ///
    /// A zone-gated node is `Locked` rather than filtered out, for the reason
    /// `Game::upgrade_ceiling` records about a structure stalled at its zone
    /// ceiling: hiding the stalled rows would mean a player who never
    /// breached never learns the tier is there, and the visible band is
    /// exactly what makes breaching worth doing.
    /// One sentence per conversion this node makes possible, recipes first
    /// and then the machines it makes buildable — see
    /// `ResearchStatus::conversions`.
    ///
    /// The two sources reduce to the same pair, `(cost, result)`, which is
    /// why this is one walk and not two shapes: a recipe carries its cost on
    /// the node, and a machine's is that item's own `craftable.cost` through
    /// `systems::assembly_recipe` — the shared answer to "what does this
    /// bench build", so a research line and the bench's own screens cannot
    /// quote different ingredients.
    ///
    /// A structure that assembles nothing contributes nothing, silently: a
    /// Depot and a Log Scraper are perfectly good things for a node to
    /// unlock and simply have no conversion to state.
    fn research_conversions(&self, def: &crate::research::ResearchDef) -> Vec<String> {
        let structures = self.world.resource::<StructureDb>();
        let items = self.world.resource::<ItemDb>();
        def.unlocks_recipes
            .iter()
            .map(|r| (r.cost.as_slice(), &r.result))
            .chain(def.unlocks_structures.iter().filter_map(|id| {
                let structure = structures.get(id)?;
                let cost = crate::systems::assembly_recipe(structure, items)?;
                Some((cost, &structure.assembles.as_ref()?.item))
            }))
            .map(|(cost, result)| self.conversion_line(cost, result))
            .collect()
    }

    /// "Bytecode Block x3 into Hardened Shell." — the one place a conversion
    /// is worded, so the recipe half and the machine half of the list above
    /// read alike.
    ///
    /// Names are resolved here through `Game::item_name` for
    /// `Game::copy_name`'s reason: a renderer spelling an item itself is how
    /// two screens come to call the same thing different things.
    fn conversion_line(&self, cost: &[(ItemId, u32)], result: &ItemId) -> String {
        let inputs = cost
            .iter()
            .map(|(item, qty)| format!("{} x{qty}", self.item_name(item)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{inputs} into {}.", self.item_name(result))
    }

    pub fn research_nodes(&self) -> Vec<ResearchStatus> {
        let recommended = self.world.resource::<ResearchDb>().recommended_ids();
        let active = self.world.resource::<ActiveResearch>().id.clone();
        // One answer per material across the whole pass — see
        // `research_block_memo`. Built out here rather than inside the closure
        // so all thirty-odd nodes share it.
        let mut blocks: HashMap<ItemId, Option<String>> = HashMap::new();
        // And the same for the `have` column: `base_holding` walks every output
        // buffer in the base, the nodes share materials heavily, and this is a
        // per-frame derivation.
        let mut holdings: HashMap<ItemId, u32> = HashMap::new();
        let mut nodes: Vec<ResearchStatus> = self
            .world
            .resource::<ResearchDb>()
            .all()
            .map(|def| {
                let state = if self.is_researched(&def.id) {
                    ResearchState::Unlocked
                } else if active.as_ref() == Some(&def.id) {
                    ResearchState::Active
                } else {
                    let missing = self.missing_prereqs(def);
                    let min_zone = self.research_zone_gate(def);
                    if missing.is_empty() && min_zone.is_none() {
                        ResearchState::Available
                    } else {
                        ResearchState::Locked { missing, min_zone }
                    }
                };
                let materials: Vec<ResearchMaterial> = def
                    .materials
                    .iter()
                    .map(|(item, need)| ResearchMaterial {
                        name: self.item_name(item).to_string(),
                        need: *need,
                        have: *holdings.entry(item.clone()).or_insert_with(|| {
                            crate::game::base::work_orders::base_holding(self, item)
                        }),
                    })
                    .collect();
                // **Asked only of a node the selection could take.** A
                // researched node and the running project are both refused
                // before `research_block` is ever reached, so a sentence on
                // either says why something that is not on offer is not on
                // offer — and a fresh run drew the same amber line under all
                // thirty-four rows, which is the screen telling the player
                // nothing thirty-four times. A `Locked` node keeps its
                // sentence: its wall is real and it is the one the player
                // clears next.
                let blocked_by = match state {
                    ResearchState::Unlocked | ResearchState::Active => None,
                    _ => self.research_block_memo(def, &mut blocks),
                };
                ResearchStatus {
                    id: def.id.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    cost: def.cost,
                    state,
                    progress: self
                        .world
                        .resource::<ActiveResearch>()
                        .progress
                        .get(&def.id)
                        .copied()
                        .unwrap_or(0),
                    // The same call the selection refuses on, not a
                    // re-derivation of it — that is how the screen comes to
                    // offer a row the selection turns down.
                    blocked_by,
                    materials,
                    conversions: self.research_conversions(def),
                    recommended: recommended.contains(&def.id),
                    #[cfg(test)]
                    unlocks_abilities: def.unlocks_abilities.clone(),
                }
            })
            .collect();
        // `sort_by_key` is stable, so cheapest-first survives inside each group.
        nodes.sort_by_key(|n| match n.state {
            ResearchState::Active => 0,
            ResearchState::Available => 1,
            ResearchState::Locked { .. } => 2,
            ResearchState::Unlocked => 3,
        });
        nodes
    }

    /// The research tree laid out as a flow chart — see `ResearchGraph`.
    ///
    /// Derived here rather than in the two things that read it, because
    /// app-core's cursor and gui's boxes asking two different questions
    /// about what is next to a node is how the cursor leaves the boxes.
    pub fn research_graph(&self) -> ResearchGraph {
        let db = self.world.resource::<ResearchDb>();
        // `all()` is cost-then-id, so every pass below is already
        // deterministic where a `HashMap` walk would not be.
        let defs: Vec<&ResearchDef> = db.all().collect();

        // Tier: the longest path from a root, by Kahn. `load_dir` drops a
        // cycle, so this terminates.
        let mut tier: HashMap<&str, usize> = HashMap::new();
        while tier.len() < defs.len() {
            let mut settled = false;
            for def in &defs {
                if tier.contains_key(def.id.as_str()) {
                    continue;
                }
                if def.requires.iter().all(|r| tier.contains_key(r.as_str())) {
                    let depth = def
                        .requires
                        .iter()
                        .map(|r| tier[r.as_str()] + 1)
                        .max()
                        .unwrap_or(0);
                    tier.insert(def.id.as_str(), depth);
                    settled = true;
                }
            }
            if !settled {
                break;
            }
        }

        // Slot: within a tier, ordered by the first-listed parent's slot then
        // by id. Tiers ascend, so a parent's slot is always already known —
        // the rule reads the parent's *slot* and never its tier, which is
        // what lets the one tier-skipping edge fall out of it.
        let tiers = tier.values().map(|t| t + 1).max().unwrap_or(0);
        let mut slot: HashMap<&str, usize> = HashMap::new();
        let mut cells: Vec<ResearchCell> = Vec::with_capacity(defs.len());
        for t in 0..tiers {
            let mut column: Vec<&&ResearchDef> = defs
                .iter()
                .filter(|def| tier.get(def.id.as_str()) == Some(&t))
                .collect();
            column.sort_by_key(|def| {
                // A parent named in `requires` but not loaded cannot happen:
                // `load_dir` drops the node.
                let parent = def
                    .requires
                    .first()
                    .and_then(|r| slot.get(r.as_str()).copied())
                    .unwrap_or(0);
                (parent, def.id.clone())
            });
            for (index, def) in column.into_iter().enumerate() {
                slot.insert(def.id.as_str(), index);
                cells.push(ResearchCell {
                    id: def.id.clone(),
                    tier: t,
                    slot: index,
                });
            }
        }

        let edges: Vec<(ResearchId, ResearchId)> = defs
            .iter()
            .filter(|def| tier.contains_key(def.id.as_str()))
            .flat_map(|def| {
                def.requires
                    .iter()
                    .map(|r| (r.clone(), def.id.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        let widest = cells.iter().map(|c| c.slot + 1).max().unwrap_or(0);
        ResearchGraph {
            cells,
            edges,
            tiers,
            widest,
        }
    }

    /// What completing a node hands over: the routines it teaches and the
    /// tools it teaches you to forge.
    ///
    /// Extracted rather than copied into the project path, because a doc
    /// comment cannot hold two copies of a formula in step and the copy that
    /// drifts is the one nobody runs — `CLAUDE.md` records this biting the
    /// repo four times.
    fn grant_research_knowledge(&mut self, def: &ResearchDef) {
        // Knowledge, not items: what a node hands over is the ability to
        // write this routine onto a blank disk the base has to manufacture.
        for ability in &def.unlocks_abilities {
            let name = self.ability_display_name(ability);
            let fresh = self
                .world
                .resource_mut::<KnownRoutines>()
                .0
                .insert(ability.clone());
            if fresh {
                self.log(format!("You learn the {name} routine."));
            }
        }
        // `unlocks_tools`' own version of the loop above — a tool mirrors a
        // routine rung for rung (spec decision 6), including the
        // fresh-insert check: a second node naming an already-known tool
        // must not repeat the log line.
        for tool in &def.unlocks_tools {
            let name = self
                .world
                .resource::<ToolDb>()
                .get(tool.as_str())
                .map(|t| t.name.clone())
                .unwrap_or_else(|| tool.as_str().to_string());
            let fresh = self
                .world
                .resource_mut::<KnownTools>()
                .0
                .insert(tool.clone());
            if fresh {
                self.log(format!("You learn to forge the {name}."));
            }
        }
    }

    /// Whether this base could ever work `def` at all, or the one sentence
    /// saying why not.
    ///
    /// The shared "can this base work this node" question, so the row the
    /// screen marks blocked and the refusal `select_research` answers with
    /// cannot disagree — `Game::orderable_items`' rule one rung up.
    ///
    /// Two things, in this order. A Research Node has to be standing, and
    /// `work_orders::chain_break` cannot answer that: it refuses every
    /// banked item by construction and names the research currency in its own
    /// doc as the example. Then every material line through `chain_break`
    /// itself, reported with **that function's own sentence verbatim** — the
    /// same sentence the work-order screen shows, and two spellings of one
    /// refusal is the drift this repo keeps recording.
    pub(crate) fn research_block(&self, def: &ResearchDef) -> Option<String> {
        self.research_block_memo(def, &mut HashMap::new())
    }

    /// `research_block`, reusing an answer per item across a whole screen pass.
    ///
    /// **The memo is the difference between a derivation and a per-frame cost.**
    /// `chain_break` walks every entity in the world and rebuilds
    /// `structures_by_tile` on each call, and the shipped tree asks it once per
    /// material line of every node — measured at 3.7 ms against 0.6 ms without
    /// it on a 515-entity base, on a path `group_menu`'s availability closure
    /// runs every frame. The nodes share materials heavily, so one answer per
    /// *item* is the whole saving; `Game::contract_board` carries the same note.
    ///
    /// The memo lives no longer than one call into `research_nodes`. Cached on
    /// the `Game` it would be a new `Resource` — another bevy iteration-order
    /// shift — and would need invalidating on every deploy and demolish.
    fn research_block_memo(
        &self,
        def: &ResearchDef,
        seen: &mut HashMap<ItemId, Option<String>>,
    ) -> Option<String> {
        // Answered once per pass and cached under the currency's own id: it is
        // the same question for every node, and `producers_of` walks every
        // entity in the world.
        let currency = self.research_currency();
        if let Some(no_node) = seen
            .entry(currency.clone())
            .or_insert_with(|| self.no_research_node_block(&currency))
            .clone()
        {
            return Some(no_node);
        }
        def.materials.iter().find_map(|(item, _)| {
            seen.entry(item.clone())
                .or_insert_with(|| crate::game::base::work_orders::chain_break(self, item))
                .clone()
        })
    }

    /// "No Research Node deployed", or `None` if one is.
    ///
    /// Split out so `research_block_memo` can cache it under the currency's own
    /// id — it is the same answer for every node in the tree, and
    /// `producers_of` walks every entity in the world.
    fn no_research_node_block(&self, currency: &ItemId) -> Option<String> {
        if !crate::game::base::work_orders::producers_of(self, currency).is_empty() {
            return None;
        }
        let name = self.item_name(currency);
        Some(
            match crate::game::base::work_orders::makeable_by(self, currency) {
                Some(def) => format!("No {} deployed — that is what makes {name}.", def.name),
                None => format!("Nothing the base can build makes {name}."),
            },
        )
    }

    /// Whether there is a research tree to open at all.
    ///
    /// The base menu's availability closure asks this **every frame**, and it
    /// used to ask it by building the whole of `research_nodes` — every node's
    /// bill counted against every shelf, every conversion line worded, every
    /// chain walked. `Game::contract_board` carries the same note. What the row
    /// actually needs is whether the catalogue has anything in it.
    pub fn has_research_tree(&self) -> bool {
        self.world.resource::<ResearchDb>().all().next().is_some()
    }

    /// Makes `id` the one project the base is working: Research Nodes start
    /// feeding it and its material bill is filed as ordinary High-band work
    /// orders.
    ///
    /// Every refusal lands **before anything is written** —
    /// `commit_caravan_basket`'s rule. `require_base` is in the ladder for
    /// `queue_work_order`'s reason: this reads which machines are standing,
    /// and they stand in base space.
    ///
    /// The materials are filed **once, at selection, and never topped up.** A
    /// per-tick refile would make `cancel_work_order` a no-op on exactly the
    /// orders a player most wants to intervene in. They go **through**
    /// `queue_work_order` rather than around it, so every log line and every
    /// refusal that door owns still applies.
    pub fn select_research(&mut self, id: &str) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_base()?;
        let def = self
            .world
            .resource::<ResearchDb>()
            .get(id)
            .cloned()
            .ok_or_else(|| "Unknown research.".to_string())?;
        if self.is_researched(id) {
            return Err(format!("{} is already researched.", def.name));
        }
        let missing = self.missing_prereqs(&def);
        if !missing.is_empty() {
            return Err(format!("Requires {} first.", missing.join(", ")));
        }
        if let Some(zone) = self.research_zone_gate(&def) {
            return Err(format!("Requires Zone {zone} first."));
        }
        if let Some(active) = self.world.resource::<ActiveResearch>().id.clone() {
            let name = self
                .world
                .resource::<ResearchDb>()
                .get(&active)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| active.clone());
            return Err(format!(
                "The base is already working on {name} — abandon it first."
            ));
        }
        if let Some(reason) = self.research_block(&def) {
            return Err(reason);
        }
        // Only now, with every refusal past, does anything move.
        self.world.resource_mut::<ActiveResearch>().id = Some(def.id.clone());
        for (item, need) in &def.materials {
            // Through the one door, so a line the queue itself would refuse
            // is still refused here — `research_block` has already asked
            // `chain_break` the same question, so this cannot fail, and a
            // future divergence surfaces as a missing order rather than a
            // silent one.
            let _ = self.queue_work_order(
                WorkOrder::batch(item.clone(), *need)
                    .with_priority(crate::game::base::work_orders::OrderPriority::High)
                    .with_research(),
            );
        }
        self.log_base(format!("Research project started: {}.", def.name));
        Ok(())
    }

    /// Stops working the active project, taking its work orders back out with
    /// it and **keeping** its progress.
    ///
    /// The progress entry stays because `ActiveResearch::progress` is per
    /// node: abandoning a long project for a cheap one and coming back is not
    /// destructive. Only completion removes an entry.
    pub fn abandon_research(&mut self) -> Result<(), String> {
        // `select_research`'s first rung, and abandoning needs it for the same
        // reason: it logs and rewrites the queue. `require_base` is
        // deliberately **not** here — a project is picked at the base because
        // that is where the machines are, but giving one up is a decision the
        // player may reach four frames down the Stack, and the screen is
        // `Locality::Anywhere`.
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let Some(active) = self.world.resource::<ActiveResearch>().id.clone() else {
            return Err("The base is not working on any research.".into());
        };
        let name = self
            .world
            .resource::<ResearchDb>()
            .get(&active)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| active.clone());
        self.world.resource_mut::<ActiveResearch>().id = None;
        self.withdraw_research_orders();
        self.log_base(format!("Research project abandoned: {name}."));
        Ok(())
    }

    /// The first material line the active project is short of, once it has all
    /// the progress it needs — or `None` while it is still earning, or while the
    /// base can pay.
    ///
    /// **This is the only surface a stalled project has.** A bill's work order
    /// is *removed* the moment `base_holding` reaches its quantity ("Work order
    /// complete"), and anything that then consumes those units before
    /// `settle_research` next runs — an assembler pulling a feeder, another
    /// order, the player's own transfer picker — leaves the project waiting with
    /// nothing in the queue. Re-filing per tick is ruled out: it would make
    /// `cancel_work_order` a no-op on exactly the orders a player most wants to
    /// intervene in. So the player is told instead.
    ///
    /// Named by the item, because "research is stalled" without saying on what
    /// is a row that cannot be acted on.
    pub(crate) fn research_material_shortfall(&self) -> Option<String> {
        let research = self.world.resource::<ActiveResearch>();
        let id = research.id.as_ref()?;
        let earned = research.progress.get(id).copied().unwrap_or(0);
        let def = self.world.resource::<ResearchDb>().get(id)?;
        if earned < def.cost {
            return None;
        }
        def.materials
            .iter()
            .find(|(item, need)| crate::game::base::work_orders::base_holding(self, item) < *need)
            .map(|(item, _)| self.item_name(item).to_string())
    }

    /// One tick of the completion check: a project finishes when it has the
    /// progress **and** the base can pay the whole bill off its shelves.
    ///
    /// `&&` short-circuits, so the bill is only spent once the progress gate
    /// has passed — the other order spends a project's materials while it is
    /// still half-researched.
    pub(crate) fn settle_research(&mut self) {
        let Some(active) = self.world.resource::<ActiveResearch>().id.clone() else {
            return;
        };
        let Some(def) = self.world.resource::<ResearchDb>().get(&active).cloned() else {
            return;
        };
        let progress = self
            .world
            .resource::<ActiveResearch>()
            .progress
            .get(&active)
            .copied()
            .unwrap_or(0);
        if progress < def.cost
            || !crate::game::base::stock::spend_bill_from_base(
                self,
                &def.materials,
                crate::base_ledger::ConsumeSource::Base,
            )
        {
            return;
        }
        self.world
            .resource_mut::<Research>()
            .0
            .insert(def.id.clone());
        // `log_base`, matching selection and abandonment: completion is a base
        // event now and can fire while the party is four frames down the
        // Stack. A plain `log()` is `MessageKind::Info`, which
        // `retain_outcomes_since_battle` prunes.
        self.log_base(format!("Research complete: {}.", def.name));
        self.grant_research_knowledge(&def);
        {
            let mut research = self.world.resource_mut::<ActiveResearch>();
            research.id = None;
            research.progress.remove(&def.id);
        }
        self.withdraw_research_orders();
    }
}
