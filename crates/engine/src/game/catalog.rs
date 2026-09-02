//! Read-only lookups against the loaded asset databases — item, structure,
//! and species metadata, plus the capacity checks that gate them.

use crate::*;

impl Game {
    pub fn structure_defs(&self) -> Vec<StructureDef> {
        self.world
            .resource::<StructureDb>()
            .all()
            .cloned()
            .collect()
    }

    /// Every loaded item definition, id-sorted (see `ItemDb::all`).
    ///
    /// It was `cfg(test)` on the stated grounds that nothing outside the
    /// crate needed it. Something does now — see `affix_defs` below, which
    /// names the one consumer both exist for.
    pub fn item_defs(&self) -> Vec<ItemDef> {
        self.world.resource::<ItemDb>().all().cloned().collect()
    }

    /// Every affix the game knows about — the mirror of `item_defs`, and
    /// the one door to `AffixDb` from outside that module.
    ///
    /// Public alongside `item_defs` for one consumer:
    /// `no_shipped_copy_name_outgrows_the_swap_name_column`, which walks
    /// every item × tier × affix through `copy_name` to check the widest
    /// name the shipped assets can build still fits the column app-core
    /// pads it to. That census cannot live in this crate — the column is
    /// app-core's — and it is the only thing standing between a long affix
    /// landing and every swap row below it silently misaligning.
    pub fn affix_defs(&self) -> Vec<crate::affixes::AffixDef> {
        self.world
            .resource::<crate::affixes::AffixDb>()
            .all()
            .cloned()
            .collect()
    }

    /// One item definition by id, or `None` if nothing declares it.
    ///
    /// Returns a clone rather than a borrow because its callers go on to
    /// mutate the world with what they read — `refactor_companion` decides
    /// its refusals off the `upgrade` field and then writes `Stats`, which a
    /// live borrow of the `ItemDb` resource would not allow.
    pub(crate) fn item_def(&self, item: &ItemId) -> Option<ItemDef> {
        self.world.resource::<ItemDb>().get(item.as_str()).cloned()
    }

    /// The display name for `id`, falling back to the raw id if the item set
    /// doesn't define it (a save referencing a since-removed mod item). The
    /// fallback borrows `id`, so the returned reference is bound to the
    /// shorter of `self` and `id`.
    pub fn item_name<'a>(&'a self, id: &'a ItemId) -> &'a str {
        self.world
            .resource::<ItemDb>()
            .get(id.as_str())
            .map(|d| d.name.as_str())
            .unwrap_or_else(|| id.as_str())
    }

    /// The display name for `id` with its `ItemCategory::short_label` in
    /// brackets after it — what a log line uses, where there is no column to
    /// put the tag in front of the way the inventory and trade screens do.
    ///
    /// A drop line is the one place an item is named to a player who has not
    /// opened a screen listing it, so the tag is the whole of what tells them
    /// whether the thing that just landed is gear or stock.
    pub fn item_name_tagged(&self, id: &ItemId) -> String {
        format!(
            "{} [{}]",
            self.item_name(id),
            self.item_category(id).short_label()
        )
    }

    /// Every conversion a structure runs, each expanded back to the raw
    /// inputs it bottoms out in.
    ///
    /// The roots are `systems::produced_item`'s answer, which is already the
    /// whole of "could this structure put something in an output buffer" —
    /// so this cannot list a machine the base does not actually run, or miss
    /// one it does. Each root's own step comes from `systems::assembly_recipe`
    /// for the same reason: a machine's recipe *is* the assembled item's
    /// recipe, so the chain on screen and the batch the machine stages are
    /// one lookup rather than two that could drift.
    ///
    /// An item can legitimately appear twice with different makers — Power
    /// Cell is a Power Conduit's whole output and is also bench-craftable
    /// from fragments, and ICE Breaker the same with the Compiler. A root
    /// step reports the structure; an expanded dependency reports the item's
    /// own `requires_structure`. Both are true, and the screen shows the one
    /// that matters where it stands.
    ///
    /// An *ingredient* names its tap only when no recipe makes it (see
    /// `RecipeInput::source`), which is the same rule seen from the other
    /// end: Core Fragment names the Mining Node because nothing on screen
    /// makes one, while Power Cell names no Power Conduit because the bench
    /// step that makes these ones is already a line of this chain. Without
    /// that clause a chain would claim two sources for a single item.
    ///
    /// Shallowest chain first, then by name, so the screen opens on the taps
    /// and reads down into what needs a base.
    pub fn recipe_chains(&self) -> Vec<RecipeChain> {
        let structures = self.world.resource::<StructureDb>();
        let items = self.world.resource::<ItemDb>();
        let mut chains: Vec<RecipeChain> = structures
            .all()
            .filter_map(|def| {
                let output = crate::systems::produced_item(def)?;
                let inputs = crate::systems::assembly_recipe(def, items).unwrap_or(&[]);
                let mut steps = Vec::new();
                // Seeded with the root, whose own step is appended below: a
                // recipe that reaches back to what it makes would otherwise
                // emit that step twice.
                let mut seen = vec![output.clone()];
                for (id, _) in inputs {
                    self.expand_recipe(id, &mut seen, &mut steps);
                }
                steps.push(RecipeStep {
                    inputs: self.recipe_inputs(inputs),
                    maker: Some(def.name.clone()),
                    output: self.item_name(output).to_string(),
                    // A tap and only a tap declines to quote a yield; every
                    // other root is an assembler running a one-unit batch.
                    output_qty: def.work.is_none().then_some(1),
                });
                Some(RecipeChain {
                    product: self.item_name(output).to_string(),
                    description: self.item_description(output).map(str::to_string),
                    steps,
                })
            })
            .collect();
        chains.sort_by(|a, b| (a.steps.len(), &a.product).cmp(&(b.steps.len(), &b.product)));
        chains
    }

    /// Appends the steps that produce `item`, dependencies first, or nothing
    /// at all if it is a drop rather than something made.
    ///
    /// `seen` is marked *before* recursing, which is what makes a mod recipe
    /// naming itself terminate instead of taking the process down — the same
    /// contract as a malformed `.ron` being skipped rather than panicking.
    /// It doubles as the de-duplicator: two branches needing the same
    /// intermediate list it once, under the first branch that reaches it.
    fn expand_recipe(&self, item: &ItemId, seen: &mut Vec<ItemId>, out: &mut Vec<RecipeStep>) {
        if seen.contains(item) {
            return;
        }
        seen.push(item.clone());
        let items = self.world.resource::<ItemDb>();
        let Some(recipe) = items.get(item.as_str()).and_then(|d| d.craftable.as_ref()) else {
            return;
        };
        let cost = recipe.cost.clone();
        let maker = recipe.requires_structure.clone();
        for (id, _) in &cost {
            self.expand_recipe(id, seen, out);
        }
        out.push(RecipeStep {
            inputs: self.recipe_inputs(&cost),
            maker: maker.and_then(|s| {
                self.world
                    .resource::<StructureDb>()
                    .get(s.as_str())
                    .map(|d| d.name.clone())
            }),
            output: self.item_name(item).to_string(),
            output_qty: Some(1),
        });
    }

    fn recipe_inputs(&self, cost: &[(ItemId, u32)]) -> Vec<RecipeInput> {
        cost.iter()
            .map(|(id, q)| RecipeInput {
                item: self.item_name(id).to_string(),
                qty: *q,
                source: self.tap_for(id),
            })
            .collect()
    }

    /// The extractor to build for `item`, or `None` if a recipe makes it (so
    /// a step of the chain already answers the question) or nothing does.
    ///
    /// Only `work` structures count. An assembler's product is craftable by
    /// definition — that is what `systems::assembly_recipe` runs — so it is
    /// excluded by the recipe check above rather than needing its own clause.
    /// Ties break on `StructureDb::all`'s id order, which is what keeps two
    /// modded taps on one item from naming a different one per run.
    fn tap_for(&self, item: &ItemId) -> Option<String> {
        let items = self.world.resource::<ItemDb>();
        if items.get(item.as_str())?.craftable.is_some() {
            return None;
        }
        self.world
            .resource::<StructureDb>()
            .all()
            .find(|def| def.work.as_ref().is_some_and(|w| &w.produces == item))
            .map(|def| def.name.clone())
    }

    /// Which group this item lists under. An id with no definition behind it
    /// sorts as salvage rather than panicking, matching `item_name`'s habit
    /// of falling back to the raw id: a list is not the place to discover a
    /// broken mod.
    pub fn item_category(&self, id: &ItemId) -> ItemCategory {
        self.world
            .resource::<ItemDb>()
            .get(id.as_str())
            .map(|d| d.category())
            .unwrap_or(ItemCategory::Material)
    }

    /// Sort key putting a list in category order, then alphabetical inside a
    /// category. The one place that ordering is decided, so the inventory
    /// screen and a trader's shelf cannot disagree about it.
    pub(crate) fn category_sort_key(&self, id: &ItemId) -> (ItemCategory, String) {
        (self.item_category(id), self.item_name(id).to_string())
    }

    /// What one unit of `id` is worth in trade currency, before a trader's
    /// own `sell_rate` markup. Falls back to `tuning::DEFAULT_ITEM_VALUE`
    /// for an item priced by no file — a mod written before the field
    /// existed, or an id the current item set doesn't define at all.
    ///
    /// The one place a price is decided, for the reason `category_sort_key`
    /// is: `sell_item`, `buyback_unit_cost` and the trade screen all read
    /// it, and a screen that quoted a price the sale then didn't honour is
    /// worse than either number being wrong on its own.
    pub fn item_value(&self, id: &ItemId) -> u32 {
        self.world
            .resource::<ItemDb>()
            .get(id.as_str())
            .map(ItemDb::value_of)
            .unwrap_or(tuning::DEFAULT_ITEM_VALUE)
    }

    /// The character-creation kit shelf — `ItemDb::creation_shelf` called,
    /// never a second derivation. `CreationCatalogue::shelf_rows` is the
    /// other caller; see that function's doc comment for why the two must
    /// be one.
    pub fn creation_shelf_rows(&self) -> Vec<crate::views::StartingItemRow> {
        self.world.resource::<ItemDb>().creation_shelf()
    }

    /// The item's authored description, straight out of its `.ron` file.
    ///
    /// Deliberately *not* derived the way `item_blurb` is: this is prose a
    /// modder writes and can edit without touching Rust, which is the whole
    /// point of it living in the asset. `None` for an item the current item
    /// set doesn't define, or one whose file leaves the field blank.
    pub fn item_description(&self, id: &ItemId) -> Option<&str> {
        let def = self.world.resource::<ItemDb>().get(id.as_str())?;
        (!def.description.is_empty()).then_some(def.description.as_str())
    }

    /// The name and description of the passive routine this item grants
    /// while worn, if it grants one.
    ///
    /// Derived from the ability rather than left to the item's own
    /// `description`, which is mod-controlled free text and cannot be
    /// trusted to stay in step with `grants` — an item whose prose still
    /// names the routine it used to carry is exactly the drift the field
    /// exists to remove. Derived in the engine rather than in the renderer
    /// for the standing reason a read-only screen's rows are: app-core owns
    /// the row count and gui draws it, so a per-row transform folded into
    /// the renderer opens a screen on a row nobody drew.
    pub fn item_grant(&self, id: &ItemId) -> Option<(&str, &str)> {
        let granted = self
            .world
            .resource::<ItemDb>()
            .get(id.as_str())?
            .grants
            .as_ref()?;
        let def = self.world.resource::<AbilityDb>().get(granted)?;
        Some((def.name.as_str(), def.description.as_str()))
    }

    /// Every extra effect this item carries, one short line each, for the
    /// screens that *list* items rather than describe one.
    ///
    /// The four fields an item can carry beyond its stat block: a passive
    /// routine granted while worn, what consuming it does, what refactoring
    /// a companion with it does, and what it adds to a decompile. An item
    /// declaring none returns an empty list, which is what tells a renderer
    /// to draw no continuation at all.
    ///
    /// **A stat bonus is deliberately not an effect here.** It already
    /// rides the equip tag on the row's own line (`equip_preview_tag`,
    /// itself built off `Game::copy_bonus`), and repeating it underneath is
    /// the column twice — the same call `item_blurb` makes about naming the
    /// slot beside an `ItemCategory::short_label` column.
    ///
    /// Distinct from `item_blurb`, which is a two-or-three word gloss for a
    /// crafting menu listing things you do not have, and from `item_grant`,
    /// which hands the describe page a routine's full prose. This one is
    /// the middle length, and it **calls** `item_grant` rather than reading
    /// `grants` a second time: an item whose own description still names
    /// the routine it used to carry is exactly the drift that field exists
    /// to remove.
    ///
    /// A magnitude is never formatted here twice either — a pre-battle buff
    /// is priced through `FieldBuffKind::magnitude_label`, the same call the
    /// running buff list makes, so a bottle and the buff it arms cannot
    /// quote different numbers.
    ///
    /// Answers for an unknown id with an empty list rather than refusing:
    /// every caller is a renderer that would only turn a `None` back into
    /// the same empty draw.
    pub fn item_effects(&self, id: &ItemId) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some((name, _)) = self.item_grant(id) {
            lines.push(format!("Grants: {name}"));
        }
        lines.extend(self.item_effects_besides_grant(id));
        lines
    }

    /// `item_effects` without its `Grants:` line — the list for a screen
    /// that draws the granted routine in full underneath, where the one-line
    /// row above would be the same fact twice.
    ///
    /// A **shorter length of the same derivation**, not a trimmed copy of
    /// its output: `item_effects` is this plus the grant line, exactly as
    /// `item_effects` is itself a shorter length of `item_grant`. Splitting
    /// it here is what stops the inspect page string-matching `"Grants:"`
    /// off a finished list, which a modded item's own effect line could
    /// collide with.
    pub fn item_effects_besides_grant(&self, id: &ItemId) -> Vec<String> {
        let Some(def) = self.world.resource::<ItemDb>().get(id.as_str()) else {
            return Vec::new();
        };
        // Copied out of the borrow: the branches below take `&self` again,
        // and a `&ItemDef` held across one would keep the `ItemDb` read
        // alive.
        let consume = def.consume;
        let upgrade = def.upgrade;
        let potency = def.taming_potency;
        let mut lines = Vec::new();
        if let Some(c) = consume {
            let mut parts = Vec::new();
            if c.heal != 0 {
                parts.push(format!("+{} HP", c.heal));
            }
            if c.power != 0.0 {
                parts.push(format!("+{:.0} Power", c.power));
            }
            if let Some(b) = c.prebattle_buff {
                parts.push(format!(
                    "{} for {}t",
                    b.kind.magnitude_label(b.power, 1),
                    b.ticks
                ));
            }
            if !parts.is_empty() {
                lines.push(format!("Use: {}", parts.join("  ")));
            }
        }
        if let Some(u) = upgrade {
            let mut parts = Vec::new();
            for (pct, label) in [
                (u.hp_percent, "HP"),
                (u.atk_percent, "ATK"),
                (u.def_percent, "DEF"),
            ] {
                // Percentage *points* already — `refactor::raised` divides
                // by 100, so a second conversion here would quote a Buffer
                // Extension's +5% HP as +500%.
                if pct != 0.0 {
                    parts.push(format!("+{pct:.0}% {label}"));
                }
            }
            if u.zone_bump {
                parts.push("zone rebuild".to_string());
            }
            if !parts.is_empty() {
                lines.push(format!("Refactor: {}", parts.join("  ")));
            }
        }
        // The *base* `taming::capture_chance` multiplies, not an addend:
        // resistance, skill and any `CaptureBoost` all scale it afterwards.
        // Saying "base" is what stops the line reading as a flat bonus.
        if let Some(p) = potency {
            lines.push(format!("Decompile: base capture {:.0}%", p * 100.0));
        }
        lines
    }

    /// Everything the inspect page draws about one carried copy.
    ///
    /// **The one derivation behind that page**, for `Game::copy_bonus`'s
    /// reason: four screens once rebuilt the gear scaling chain by hand and
    /// all four dropped the affix at once. Every figure here is a call —
    /// `copy_bonus` for the stats, `damage_range_label` for the band,
    /// `battle::hit_chance` for the odds, `routine_detail` for the grant.
    ///
    /// `wearer` is who the copy is measured *for*: the player, or the
    /// program whose slots the picker was opened from. It decides the
    /// accuracy the page quotes and, through `routine_detail`, the level and
    /// affinity every granted magnitude is scaled at.
    ///
    /// An item the current set doesn't define answers with a bare view
    /// rather than refusing, on `item_effects`' grounds: every caller is a
    /// renderer that would turn a `None` back into the same empty draw.
    pub fn gear_detail(&self, copy: &GearCopy, wearer: Entity) -> GearDetailView {
        GearDetailView {
            name: self.copy_name(copy),
            description: self.item_description(&copy.item).map(str::to_string),
            worn: self.worn_detail(copy, wearer),
            effects: self.item_effects_besides_grant(&copy.item),
            affixes: self.affix_lines(copy),
            grant: self
                .world
                .resource::<ItemDb>()
                .get(copy.item.as_str())
                .and_then(|def| def.grants.clone())
                .and_then(|id| self.routine_detail(&id, wearer)),
        }
    }

    /// The affix half of `gear_detail`: one line per **distinct** affix,
    /// saying what it is and what it adds.
    ///
    /// Folded rather than listed one per copy, because fusion keeps
    /// duplicates and eight drawn from a pool of nine is usually three or
    /// four distinct ones — so the common worst case collapses to a handful
    /// of rows on a page that has no scroll. The count is what the entry
    /// carries instead, and the stats are the affix's own authored delta
    /// **times that count**, since that is what `copy_bonus` folds in.
    ///
    /// **Trade-offs lead** — any affix with a negative component. The
    /// renderer caps this block by what fits, so an affix sorting last can
    /// be the line that falls off, and a drawback the player cannot see is
    /// exactly what this block exists to end. After that the order is the
    /// copy's own sorted one, which makes it total.
    ///
    /// An id the build cannot resolve contributes no entry, `affixes_of`'s
    /// rule: a save naming a deleted affix reads as one fewer effect.
    fn affix_lines(&self, copy: &GearCopy) -> Vec<String> {
        let mut folded: Vec<(&crate::affixes::AffixDef, u32)> = Vec::new();
        for affix in self.affixes_of(copy) {
            match folded.iter_mut().find(|(seen, _)| seen.id == affix.id) {
                Some((_, count)) => *count += 1,
                None => folded.push((affix, 1)),
            }
        }
        folded.sort_by_key(|(affix, _)| (!charges_for_itself(affix), affix.id.clone()));
        folded
            .into_iter()
            .map(|(affix, count)| {
                let word = affix
                    .prefix
                    .clone()
                    .or_else(|| affix.suffix.clone())
                    .unwrap_or_else(|| affix.id.as_str().to_string());
                let stacked = scale_stats(affix.stats, count);
                let head = if count > 1 {
                    format!("{word} ×{count}")
                } else {
                    word
                };
                match self.stat_summary(stacked) {
                    summary if summary.is_empty() => head,
                    summary => format!("{head}: {summary}"),
                }
            })
            .collect()
    }

    /// The slot half of `gear_detail`, `None` for anything not wearable.
    ///
    /// The candidate is priced at the **current zone level**, because that
    /// is what `Game::equip` locks a copy in at — the same asymmetry
    /// `equip_swap_rows` documents, and the reason a spare copy of the
    /// weapon you already wear is a real upgrade after a breach.
    fn worn_detail(&self, copy: &GearCopy, wearer: Entity) -> Option<WornDetailView> {
        let (slot, _) = self.equipment_of(&copy.item)?;
        let level = self.world.resource::<ZoneLevel>().0;
        let stats = self.copy_bonus(copy, level)?;

        // What the slot already holds comes back off before the candidate
        // goes on, or inspecting the piece you are wearing counts it twice.
        let held = self
            .worn(wearer, slot)
            .and_then(|worn| self.worn_bonus(&worn))
            .unwrap_or_default();
        let gear_accuracy = self.gear_bonus(wearer).accuracy - held.accuracy + stats.accuracy;
        let accuracy = crate::battle::accuracy_of(
            self.combat_speed(wearer),
            self.ability_user_level(wearer),
            gear_accuracy,
        );

        let zone = self.world.resource::<ZoneLevel>().0;
        let median =
            crate::balance_sim::median_ordinary_species(self.world.resource::<SpeciesDb>());
        let nominal = NominalHostile {
            zone,
            evasion: crate::battle::evasion_of(median.base_speed, zone, 0),
        };
        Some(WornDetailView {
            slot,
            level,
            quality: copy.quality,
            stats,
            power: self.copy_power(copy),
            accuracy,
            hit_chance: crate::battle::hit_chance(accuracy, nominal.evasion),
            nominal,
        })
    }

    /// A signed one-line summary of a gear bonus — `"14–21 DMG +3 ATK"`,
    /// empty when every stat is zero. Signed throughout, so it reads a
    /// *change* as naturally as a total.
    ///
    /// **One formatter**, and in the engine because the affix block on the
    /// inspect page needs it too — a second copy beside app-core's is the
    /// mistake `copy_bonus` is written to prevent, one level down. The
    /// inventory list's equip tag, the equipped panel, the swap picker's two
    /// stat columns and `gear_detail`'s affix lines all print exactly this,
    /// over figures that all come from `copy_bonus`.
    ///
    /// The damage band **leads**, because on a weapon it is the headline
    /// number — what the thing hits for is what two weapons are compared on,
    /// and ATK is the smaller flat term added on top of it. It goes through
    /// `damage_range_label` rather than being formatted here, so a range
    /// printed on any screen cannot disagree with a range printed on
    /// another.
    pub fn stat_summary(&self, mods: crate::items::EquipmentStats) -> String {
        let mut parts = Vec::new();
        if mods.damage != crate::battle::DamageRange::default() {
            parts.push(format!("{} DMG", self.damage_range_label(mods.damage)));
        }
        for (value, name) in [
            (mods.atk, "ATK"),
            (mods.mitigation, "MIT"),
            (mods.accuracy, "ACC"),
            (mods.evasion, "EVA"),
            (mods.decompiler, "DECOMP"),
        ] {
            if value != 0 {
                parts.push(format!("{value:+} {name}"));
            }
        }
        parts.join(" ")
    }

    /// A two-or-three word gloss of what an item *does*, for menus that list
    /// items by name and cost without saying why you'd want one.
    ///
    /// Derived from the item's own definition rather than authored per item,
    /// so a modded item gets one for free and no blurb can drift out of step
    /// with the mechanics it describes. `None` for an item whose definition
    /// says nothing worth glossing — a plain currency reads fine as itself.
    pub fn item_blurb(&self, id: &ItemId) -> Option<String> {
        let def = self.world.resource::<ItemDb>().get(id.as_str())?;
        if let Some((_, stats)) = &def.equipment {
            let mut parts = Vec::new();
            if stats.atk != 0 {
                parts.push(format!("+{} atk", stats.atk));
            }
            if stats.mitigation != 0 {
                parts.push(format!("+{} def", stats.mitigation));
            }
            if stats.decompiler != 0 {
                parts.push(format!("+{} decomp", stats.decompiler));
            }
            // Deliberately not naming the slot when there is no magnitude to
            // quote. Every screen listing an item prints
            // `ItemCategory::short_label` in a column of its own, so a gloss
            // saying "Weapon" beside a `WEP` column is the column twice. The
            // column carries the kind; this carries the magnitude.
            return (!parts.is_empty()).then(|| parts.join(" "));
        }
        if let Some(c) = &def.consume {
            let mut parts = Vec::new();
            if c.power != 0.0 {
                parts.push(format!("+{:.0} power", c.power));
            }
            if c.heal != 0 {
                parts.push(format!("+{} HP", c.heal));
            }
            if c.prebattle_buff.is_some() {
                parts.push("pre-battle buff".to_string());
            }
            if !parts.is_empty() {
                return Some(parts.join(" "));
            }
        }
        if def.taming_potency.is_some() {
            return Some("taming catalyst".to_string());
        }
        None
    }

    pub fn is_equippable(&self, id: &ItemId) -> bool {
        self.equipment_of(id).is_some()
    }

    pub fn equipment_of(&self, id: &ItemId) -> Option<(EquipmentSlot, EquipmentStats)> {
        self.world.resource::<ItemDb>().get(id.as_str())?.equipment
    }

    pub fn is_consumable(&self, id: &ItemId) -> bool {
        self.world
            .resource::<ItemDb>()
            .get(id.as_str())
            .is_some_and(|d| d.consume.is_some())
    }

    /// Whether `id` is a pool rather than cargo — see `ItemDef::banked` and
    /// `assets/items/README.md` for everything that follows from it.
    pub fn is_banked(&self, id: &ItemId) -> bool {
        self.world
            .resource::<ItemDb>()
            .get(id.as_str())
            .is_some_and(|d| d.banked)
    }

    /// How much of a banked item the player holds. The counterpart to
    /// `PlayerStatus::inventory` deliberately not listing it: hiding the row
    /// everywhere would otherwise hide the number from the one screen that
    /// needs it, so a caller that genuinely wants a bank asks for it by name.
    ///
    /// Answers for any item, banked or not, rather than refusing — the
    /// distinction that matters to a caller is *which* item it is asking
    /// about, and a `None` here would only push an `unwrap` into the
    /// renderer.
    pub fn banked(&self, item: &ItemId) -> u32 {
        self.world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(item))
            .unwrap_or(0)
    }

    pub fn currency(&self) -> ItemId {
        self.world
            .resource::<ItemDb>()
            .currency()
            .expect("validated at startup")
            .clone()
    }

    pub fn research_currency(&self) -> ItemId {
        self.world
            .resource::<ItemDb>()
            .research_currency()
            .expect("validated at startup")
            .clone()
    }

    pub fn craft_currency(&self) -> ItemId {
        self.world
            .resource::<ItemDb>()
            .craft_currency()
            .expect("validated at startup")
            .clone()
    }

    /// What every trader pays and charges — see `EconomyRole::TradeCurrency`.
    /// Distinct from `currency`, which is the salvage the build economy runs
    /// on and which no trader deals in.
    pub fn trade_currency(&self) -> ItemId {
        self.world
            .resource::<ItemDb>()
            .trade_currency()
            .expect("validated at startup")
            .clone()
    }

    /// Whether `structure_id` may be built right now. A structure named by
    /// no research file is unlocked by default — that's what keeps Home, the
    /// Mining Node, the Research Node, the Recharger Node and the Zone
    /// Portal available from turn one without a hardcoded whitelist, and
    /// what keeps a structure mod that ships no research file working
    /// unchanged.
    pub(crate) fn structure_unlocked(&self, structure_id: &str) -> bool {
        let db = self.world.resource::<ResearchDb>();
        let mut gates = db
            .all()
            .filter(|def| def.unlocks_structures.iter().any(|s| s == structure_id))
            .peekable();
        if gates.peek().is_none() {
            return true;
        }
        gates.any(|def| self.is_researched(&def.id))
    }

    /// The best deployed structure of `kind`, as an upgrade tier — `None`
    /// when the player has never built one.
    ///
    /// The bench half of a compiled copy's quality floor (see
    /// `Game::player_craft_order`). It is the *best* rather than the
    /// nearest or the first because a bench is a thing the base owns, not
    /// a thing the player walks to: `craft_recipes` already asks only
    /// whether one is standing anywhere, and a second, stricter rule here
    /// would let a recipe be offered and then compile against a bench the
    /// player was not told about.
    ///
    /// A structure carrying no `StructureTier` reads as tier 1, not as
    /// tier 0. The component is only inserted by `build_structure` for a
    /// def declaring `upgrade`, so a bench with no upgrade path and a
    /// bench that has never been upgraded are the same thing to a player,
    /// and the term above the first tier is what a bench upgrade sells.
    pub(crate) fn best_structure_tier(&self, kind: &str) -> Option<u32> {
        self.world
            .iter_entities()
            .filter(|e| e.get::<Structure>().is_some_and(|s| s.kind == kind))
            .map(|e| e.get::<StructureTier>().map(|t| t.0).unwrap_or(1))
            .max()
    }

    /// The structures the build menu offers: `structure_defs` minus anything
    /// still behind unfinished research. `structure_defs` itself stays
    /// unfiltered — it's the general lookup, not the menu.
    pub fn buildable_structure_defs(&self) -> Vec<StructureDef> {
        self.world
            .resource::<StructureDb>()
            .all()
            .filter(|def| self.structure_unlocked(&def.id))
            .cloned()
            .collect()
    }

    /// How many tamed programs the player may own in total right now:
    /// `BASE_PET_CAPACITY` plus every deployed structure's `pet_slot_bonus`
    /// (a Data Cache adds five), plus `Perk::ProcessPool`. Derived on each
    /// call rather than cached, so a cache lost to a raid shrinks the limit
    /// with no invalidation step and the save format stays unchanged.
    ///
    /// The perk is the one term a raid cannot take back, which is the whole
    /// of what it sells: slots that don't depend on a building standing.
    pub fn pet_capacity(&self) -> usize {
        let kinds: Vec<StructureId> = self
            .world
            .iter_entities()
            .filter_map(|e| e.get::<Structure>().map(|s| s.kind.clone()))
            .collect();
        let db = self.world.resource::<StructureDb>();
        let bonus: u32 = kinds
            .iter()
            .filter_map(|k| db.get(k.as_str()))
            .map(|def| def.pet_slot_bonus)
            .sum();
        BASE_PET_CAPACITY + bonus as usize + crate::perks::roster_slot_bonus(self.player_perks())
    }

    /// How many tamed programs the player currently owns, wherever they are —
    /// active party, cronjob workers, and idle pets all count against
    /// `pet_capacity`.
    pub fn pet_count(&self) -> usize {
        let player = self.player_entity();
        self.world
            .iter_entities()
            .filter(|e| e.get::<Tamed>().is_some_and(|t| t.owner == player))
            .count()
    }

    /// Units of cargo currently carried, excluding banked currency. Fused
    /// copies count: they are carried, and this figure has to keep matching
    /// the sum of `PlayerStatus::inventory`, which lists both stores.
    pub fn inventory_used(&self) -> u32 {
        let player = self.player_entity();
        let db = self.world.resource::<ItemDb>();
        let carried = self
            .world
            .get::<Inventory>(player)
            .map(|inv| inv.cargo_used(db))
            .unwrap_or(0);
        let fused = self
            .world
            .get::<GearCopies>(player)
            .map(|f| f.total())
            .unwrap_or(0);
        carried + fused
    }

    /// The actual item cost to deploy `def` right now: `def.build_cost` plus
    /// every `def.zone_build_cost` line whose `min_zone` the current zone has
    /// reached, each grown by `ZONE_PORTAL_COST_GROWTH_PERCENT` of its base
    /// rate per zone level **past the zone it was introduced in** for a
    /// zone-portal structure (see `StructureDef::zone_portal`) — breaching
    /// deeper costs more raw material each time, and a line authored for a
    /// later sector charges its authored base the first zone it can legally
    /// be demanded rather than arriving pre-inflated. `build_cost` is
    /// implicitly `min_zone: 1`, which is what makes this a no-op for every
    /// structure shipped before `zone_build_cost` existed. A non-`zone_portal`
    /// structure gets its qualifying `zone_build_cost` lines appended
    /// unramped — only the `zone_portal` branch grows anything.
    ///
    /// A `StructureDef::first_free` structure is quoted at nothing until the
    /// run has one, and that waiver is resolved **here** rather than at the
    /// deploy: this is the one door the build menu, the deploy prompt, the
    /// filed request's stored bill and the removal refund all read, so a
    /// second site pricing it would be a second opinion about what the
    /// player owes.
    ///
    /// Free needs *both* halves of the count. `FreeBuilds` says what has
    /// been raised, because that is the moment a freebie is genuinely spent
    /// — a request the player cancels must not burn it. On its own that
    /// prices two requests filed side by side at nothing and raises both, so
    /// a pending request counts against the freebie alongside a standing
    /// structure, exactly as `max_deployed` counts them together and for the
    /// same reason.
    pub fn structure_build_cost(&self, def: &StructureDef) -> Vec<(ItemId, u32)> {
        if def.first_free
            && !self
                .world
                .resource::<crate::resources::FreeBuilds>()
                .0
                .contains(&def.id)
            && self.count_build_requests(&def.id) == 0
        {
            return Vec::new();
        }
        let zone = self.world.resource::<ZoneLevel>().0;
        let lines = def
            .build_cost
            .iter()
            .map(|(item, qty)| (1, item.clone(), *qty))
            .chain(
                def.zone_build_cost
                    .iter()
                    .filter(|(min_zone, ..)| *min_zone <= zone)
                    .cloned(),
            );
        if !def.zone_portal {
            return lines.map(|(_, item, qty)| (item, qty)).collect();
        }
        lines
            .map(|(min_zone, item, qty)| {
                (
                    item,
                    zone_portal_cost(qty, zone.saturating_sub(min_zone) + 1),
                )
            })
            .collect()
    }

    pub fn species_defs(&self) -> Vec<SpeciesDef> {
        self.world.resource::<SpeciesDb>().all().cloned().collect()
    }

    /// The perk picker's sections: a heading and the defs under it, in the
    /// order `assets/perks/groups.ron` lists them. A heading is the empty
    /// string for the trailing bucket of anything no section names — see
    /// `PerkDb::grouped`.
    pub fn perk_groups(&self) -> Vec<(String, Vec<PerkDef>)> {
        self.world
            .resource::<PerkDb>()
            .grouped()
            .into_iter()
            .map(|(name, defs)| (name.to_string(), defs.into_iter().cloned().collect()))
            .collect()
    }

    /// Every perk currently on offer, in picker order. The renderer's only
    /// route to a perk's name, description and price — those are authored in
    /// `assets/perks/*.ron`, not derivable from the `Perk` variant, and the
    /// index into this list is what `unlock_perk` expects back.
    ///
    /// `perk_groups` flattened rather than a second walk of the catalogue, so
    /// the index a player types against is the one the sections drew.
    pub fn perk_defs(&self) -> Vec<PerkDef> {
        self.perk_groups()
            .into_iter()
            .flat_map(|(_, defs)| defs)
            .collect()
    }
}

/// Whether an affix charges for what it pays — any negative component in
/// its `stats`. A shipped affix shape, and the reason `affix_lines` sorts
/// the way it does; see `assets/affixes/README.md`.
fn charges_for_itself(affix: &crate::affixes::AffixDef) -> bool {
    let s = affix.stats;
    [
        s.atk,
        s.mitigation,
        s.decompiler,
        s.accuracy,
        s.evasion,
        s.damage.min,
        s.damage.max,
    ]
    .iter()
    .any(|v| *v < 0)
}

/// An affix's authored delta times how many of it a copy carries — what
/// `Game::copy_bonus` folds in for a stacked affix, said once so the page
/// and the total cannot disagree about a duplicate.
fn scale_stats(stats: crate::items::EquipmentStats, count: u32) -> crate::items::EquipmentStats {
    let n = count as i32;
    crate::items::EquipmentStats {
        atk: stats.atk * n,
        mitigation: stats.mitigation * n,
        decompiler: stats.decompiler * n,
        damage: crate::battle::DamageRange {
            min: stats.damage.min * n,
            max: stats.damage.max * n,
        },
        accuracy: stats.accuracy * n,
        evasion: stats.evasion * n,
    }
}
