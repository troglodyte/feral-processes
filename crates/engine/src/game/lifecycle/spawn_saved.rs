//! Spawning the player and each creature from their save rows.

use super::convert::{copy_from_save, worn_from_save};
use super::load::recognized_routines;
use crate::abilities::AbilityId;
use crate::components::{Needs, Tools};
use crate::tuning::CRAFT_ID_PREFIX;
use crate::*;

/// The state a run of `Game::spawn_creature_from_save` carries **across**
/// creatures, rather than within one.
///
/// `Game::load` used to hold all of this as locals threaded through one long
/// loop body, which is why extracting that body needed somewhere to put it.
/// It is one struct rather than six parameters because the list is not
/// finished: every tether resolved *after* the creature array — a settlement,
/// a structure, a sortie — lands here as another deferred vec, and a
/// six-argument function would have become a nine-argument one.
///
/// Everything in it is one of two things:
///
/// - **Deferred work.** `party_slots`, `sortie_members`, `pending_cronjobs`
///   and `pending_patrols` all name an entity a creature is tethered to that
///   does not exist yet — the party has to be sorted by slot once every
///   member is spawned, and a cronjob target, a town and a sortie record are
///   all rebuilt further down `Game::load`. A creature cannot resolve any of
///   them on its own, so it writes down what it wants and the caller does it.
/// - **Shared reference data.** `player` (every restored program is tamed to
///   the same owner), `nest_positions` (built before the creature array so a
///   wild guardian has a live nest to point at) and `next_program_id`.
///
/// `next_program_id` is the one that must not become a local. Minting is a
/// property of a pass over the *whole* array: two creatures carrying the `0`
/// sentinel have to come back as two different programs, and a counter reset
/// per call would give them the same name. The caller writes the final value
/// back to `resources::NextProgramId`.
///
/// **Not here: the wield.** `Game::load` resolves `WieldedProgram` itself
/// from the returned entity, because "at most one, first wins" is a decision
/// about the file as a whole and the restore has no business making it.
pub(crate) struct CreatureRestore {
    /// Who every restored program is tamed to.
    pub(crate) player: Entity,
    /// The next unused program id — bumped only by the `0` sentinel, never
    /// by a snapshot that already carries a name.
    pub(crate) next_program_id: u32,
    /// Nest tile -> nest entity, for a wild guardian's `nest_position`.
    /// Empty is a legitimate value: a caller restoring a tamed program alone
    /// never reaches the branch that reads it.
    pub(crate) nest_positions: HashMap<(i32, i32), Entity>,
    /// `(slot, member)`, collected with the slot index and sorted by the
    /// caller: creatures come back in whatever order they were written,
    /// which is no longer the roster order, and roster order is
    /// mechanically meaningful.
    pub(crate) party_slots: Vec<(u32, Entity)>,
    /// `(sortie index, member)`. Membership comes back from the creature
    /// side, `party_slots`' reason: entity ids are not stable across a save,
    /// so `SortieSave` carries no member list to read.
    pub(crate) sortie_members: Vec<(u32, Entity)>,
    /// `(worker, cronjob)` — a cronjob names its target by tile, and the
    /// structures a tile has to name are rebuilt after the creature array.
    pub(crate) pending_cronjobs: Vec<(Entity, save::CronjobSave)>,
    /// `(member, town tile, was pursuing)` — resolved after
    /// `restore_settlements`, which is what builds the entities a tile has
    /// to name.
    pub(crate) pending_patrols: Vec<(Entity, (i32, i32), bool)>,
    /// `(program, station tile)` — `pending_cronjobs`' shape and its own
    /// resolution point: a Research Station is one of the structures that
    /// array rebuilds, so a study tether can be no sooner than a cronjob's
    /// target is.
    pub(crate) pending_study: Vec<(Entity, (i32, i32))>,
    /// `(program, outpost tile)` — resolved right after `restore_outposts`,
    /// which needs no structure array rebuilt first: an outpost is a record
    /// in `resources::Outposts` keyed by tile, not an entity, so unlike
    /// `pending_study` there is nothing else to wait on.
    pub(crate) pending_outpost_crew: Vec<(Entity, (i32, i32))>,
    /// Programs held in a Power Siphon, with the tile it stands on —
    /// `pending_study`'s deferral, since a siphon is a structure.
    pub(crate) pending_siphon: Vec<(Entity, (i32, i32))>,
    /// Prisoners with the tile of the Holding Cell they fill —
    /// `pending_siphon`'s deferral, since a cell is a structure.
    pub(crate) pending_jailed: Vec<(Entity, save::JailedSave)>,
    /// `(siege_order, member, siege_cell)` — `sortie_members`' shape,
    /// applied to `game::siege::persist::restore` rather than
    /// `restore_sorties`: entity ids aren't stable across a save/load round
    /// trip, so a siege's own membership rides the creature side too.
    pub(crate) pending_siege_members: Vec<(u32, Entity, (i32, i32))>,
    /// `(nemesis_band, body, is_leader)` — linked by
    /// `Game::link_nemesis_bands` once every creature is spawned, since a
    /// follower's leader may sit later in the file than it does.
    pub(crate) pending_nemesis_band: Vec<(u32, Entity, bool)>,
    /// `(besieger, source structure's tile)` — `pending_cronjobs`'
    /// deferral: `components::StolenFrom` names a structure entity, and the
    /// structures a tile has to name are rebuilt after the creature array.
    pub(crate) pending_stolen_from: Vec<(Entity, (i32, i32))>,
    /// Owned programs whose file carried no `staff_rank`, in the order they
    /// were spawned — a save written before ranks existed, or a hand-edited
    /// one. Not deferred for anything to exist first, unlike every other
    /// field here; it is deferred because the rank it gets depends on the
    /// highest rank *any* creature in the file did carry, which isn't known
    /// until the whole array has been read.
    pub(crate) pending_ranks: Vec<Entity>,
    /// The highest `staff_rank` any creature in the file named. A body in
    /// `pending_ranks` is assigned one past this, in file order — `0` if
    /// nothing in the file named a rank at all, so the first such body
    /// becomes rank 1.
    pub(crate) max_rank_seen: u32,
}

impl CreatureRestore {
    /// A restore with no deferred work recorded yet.
    ///
    /// `nest_positions` is taken up front rather than left to be filled in
    /// afterwards so that the whole of the *input* side is settled at
    /// construction and only the deferred vecs are read back out. A caller
    /// with no nests to offer — anything restoring a single tamed program,
    /// which is every path but `Game::load` — passes an empty map and gets
    /// the same behaviour a save with no nests in it gets.
    pub(crate) fn new(
        player: Entity,
        next_program_id: u32,
        nest_positions: HashMap<(i32, i32), Entity>,
    ) -> Self {
        Self {
            player,
            next_program_id,
            nest_positions,
            party_slots: Vec::new(),
            sortie_members: Vec::new(),
            pending_cronjobs: Vec::new(),
            pending_patrols: Vec::new(),
            pending_study: Vec::new(),
            pending_outpost_crew: Vec::new(),
            pending_siphon: Vec::new(),
            pending_jailed: Vec::new(),
            pending_siege_members: Vec::new(),
            pending_nemesis_band: Vec::new(),
            pending_stolen_from: Vec::new(),
            pending_ranks: Vec::new(),
            max_rank_seen: 0,
        }
    }
}

/// The player as a save wrote them, spawned before `Game` exists — `spawn_player`'s
/// counterpart on the load path, and the reason that one takes a bare start tile.
pub(super) fn spawn_player_from_save(
    world: &mut World,
    player_save: save::PlayerSave,
    player_routines: Vec<AbilityId>,
    player_perk_levels: u32,
) -> Entity {
    world
        .spawn((
            Player,
            Position {
                x: player_save.position.0,
                y: player_save.position.1,
            },
            Glyph {
                ch: player_save.glyph,
                color: GlyphColor::Cyan,
            },
            Stats {
                // Placeholders but for `hp`: `recompute_derived` in
                // `Game::load` writes the rest and clamps `hp` to the new
                // maximum, so the file carries none of them.
                hp: player_save.hp,
                max_hp: player_save.hp,
                atk: 0,
                mitigation: 0,
            },
            // Unbounded until `recompute_derived` below trims it to the
            // derived maximum: the file's Bandwidth is not known yet.
            PowerReserve::new(player_save.power, f32::MAX),
            Experience {
                level: player_save.level,
                xp: player_save.xp,
                // Derived, not read back. `PlayerSave::xp_to_next` is a
                // second copy of `xp_for_level(level)`, which agrees
                // until `XP_PER_LEVEL_STEP` moves — and then hands a
                // pre-retune save a level at the old price. The field
                // stays written (removing one is what earns a
                // `SAVE_FORMAT_VERSION` bump) and is simply not trusted.
                xp_to_next: crate::progression::xp_for_level(player_save.level),
            },
            Decompiler { skill: 0 },
            Equipment {
                weapon: worn_from_save(
                    player_save.weapon,
                    player_save.weapon_level,
                    player_save.weapon_fusion_tier,
                    player_save.weapon_rarity,
                    player_save.weapon_affix.clone(),
                    player_save.weapon_affixes.clone(),
                    player_save.weapon_quality,
                    player_save.weapon_wear,
                ),
                armor: worn_from_save(
                    player_save.armor,
                    player_save.armor_level,
                    player_save.armor_fusion_tier,
                    player_save.armor_rarity,
                    player_save.armor_affix.clone(),
                    player_save.armor_affixes.clone(),
                    player_save.armor_quality,
                    player_save.armor_wear,
                ),
                module: worn_from_save(
                    player_save.module,
                    player_save.module_level,
                    player_save.module_fusion_tier,
                    player_save.module_rarity,
                    player_save.module_affix.clone(),
                    player_save.module_affixes.clone(),
                    player_save.module_quality,
                    player_save.module_wear,
                ),
            },
            Inventory {
                items: player_save.inventory,
            },
            // Gear fusion was uncapped before it shared `MAX_FUSIONS`,
            // so an older save can carry a copy above the ceiling.
            // Only the carried copies are clamped — the worn copies
            // above keep the tier their bonus was applied at, because
            // `Stats` is restored with that bonus already in it and
            // unequipping must subtract exactly what was added.
            //
            // Clamping can collapse two rows onto one key, so this goes
            // through `add` rather than building the `Vec` directly:
            // `GearCopies` holds one row per `GearCopy`, and a duplicate
            // row would make `count` under-report and strand the copies
            // in the row it didn't find.
            //
            // `fused_gear` is the pre-0.8.9 store and is drained here
            // rather than read anywhere else — see its doc in `save/mod.rs`
            // for why the two coexist and when the legacy one goes.
            {
                let mut carried = GearCopies::default();
                let legacy = player_save.fused_gear.into_iter().map(|(item, tier, qty)| {
                    (
                        // These copies predate both the affix and the
                        // quality fields by two years of releases.
                        GearCopy {
                            item,
                            rarity: Rarity::Ordinary,
                            tier,
                            affixes: Vec::new(),
                            quality: crate::tuning::QUALITY_DEFAULT,
                            wear: 0,
                        },
                        qty,
                    )
                });
                let saved = player_save
                    .gear_copies
                    .into_iter()
                    .map(|(copy, qty)| (copy_from_save(copy), qty));
                for (copy, qty) in legacy.chain(saved) {
                    carried.add(
                        GearCopy {
                            tier: copy.tier.min(crate::tuning::MAX_FUSIONS),
                            ..copy
                        },
                        qty,
                    );
                }
                carried
            },
            StatusEffects::default(),
            CombatBuff::default(),
            FieldBuff {
                active: player_save.field_buffs,
            },
            Perks {
                points: player_save.perk_points,
                unlocked: player_save.unlocked_perks,
            },
            // Nested because the bundle tuple above is already at
            // bevy's 15-element ceiling — the query-tuple limit CLAUDE.md
            // already names, reached here from the bundle side instead.
            (
                // `max`, never a plain assignment from
                // `unlocked_perks.len()`. That length is exact for a save
                // written before respec shipped and *wrong* for every save
                // written after one: a respec empties the list and leaves
                // the count standing, so assigning the length would throw
                // the count away and re-open the overflow-XP exploit
                // across a save/load.
                BoughtStats {
                    ever_bought: player_save.bought_stats.ever_bought.max(player_perk_levels),
                    ..player_save.bought_stats
                },
                Routines(player_routines),
                PlayerIdentity {
                    class: player_save.class,
                    sprite: player_save.sprite.clone(),
                    colour: player_save.colour,
                    icon: player_save.icon.as_deref().and_then(PlayerIcon::decode),
                },
                DownedPrograms(player_save.downed_programs),
                // Never `STARTER_TOOL_ID` here — the profile rule.
                // `spawn_player` is `new_with`'s only caller and is
                // where that grant lives; a load restores exactly what
                // the save carried. A save predating tools carries no
                // `tools` key at all, so `PlayerSave::tools`'s own
                // `starter_tools()` default lands the starter tool
                // there instead of an empty loadout — see that field's
                // doc comment.
                Tools(player_save.tools),
                // Kept verbatim, including ids with no def: they contribute
                // nothing but stay removable — see `components::Implants`.
                crate::components::Implants {
                    installed: player_save.implants,
                },
                player_save.phase_keys,
            ),
        ))
        .id()
}

impl Game {
    /// One row of the save format's `creatures` array back into the world —
    /// and **the single answer to "how a creature comes back"**.
    ///
    /// `Game::load` runs it once per row. A cancelled build order runs it
    /// once, to hand back the tamed program the order was holding. There is
    /// deliberately no second copy: a refund that rebuilt a program from its
    /// own hand-written component list would drift from the loader one field
    /// at a time, and the drift would surface as a program that reloads from
    /// a file correctly and comes back subtly wrong from a cancel — a
    /// difference nothing in the game would ever show the player directly.
    ///
    /// **The mirror of `Game::creature_save_for`**, and the two are meant to
    /// be read together: a field added to `CreatureSave` needs a write there
    /// and a read here, and `tests::save_roundtrip` is the gate on both.
    ///
    /// `None` means the file names a species this install no longer ships —
    /// a `.ron` deleted between sessions. The caller drops that row rather
    /// than failing the whole load, which is what makes deleting a species
    /// file supported instead of save-breaking.
    ///
    /// **This mints the same component set `Game::roster_parts` does** —
    /// `Tamed`, `Experience`, `PowerReserve`, `ProgramId`, `Memories`,
    /// `Needs` and a `Disposition` — and pointedly does not call it. The two
    /// are opposites despite the shared list: `roster_parts` mints a *fresh*
    /// program, drawing the next id from `resources::NextProgramId` and
    /// handing back defaults, whereas everything here comes off the file and
    /// re-minting any of it is the bug this function exists to avoid. So the
    /// two have to be kept in step by hand: a new component that belongs to
    /// every owned program has to be added in both places, or a program will
    /// have it from the moment it is tamed and quietly lose it on the next
    /// reload.
    ///
    /// Two things it must not do. Both compile, and both stay invisible long
    /// past the point where anyone would connect them to a load:
    ///
    /// - **Reissue a `ProgramId`.** An id in the file is that program's
    ///   name. A fresh one orphans the program's own memories *and* every
    ///   other program's memories naming it as their subject. Only the `0`
    ///   sentinel — what a file written before ids existed carries for
    ///   everybody on the roster — draws from `ctx.next_program_id`.
    /// - **Re-stamp a memory.** Intensity is derived from `GameClock` on
    ///   every read and never stored (see `components::Memory`), so a fresh
    ///   `reinforced` tick resets the decay curve and makes a restore
    ///   *deepen* an old grudge rather than preserve it.
    pub(crate) fn spawn_creature_from_save(
        &mut self,
        c: &save::CreatureSave,
        ctx: &mut CreatureRestore,
    ) -> Option<Entity> {
        // The one `None` this returns, and the reason the return is an
        // `Option` at all: a file naming a species whose `.ron` is gone.
        let species = self
            .world
            .resource::<SpeciesDb>()
            .get(&c.species)
            .cloned()?;
        let (routines, mut dropped_routines) =
            recognized_routines(&c.routines, self.world.resource::<AbilityDb>());
        dropped_routines.retain(|id| !id.starts_with(CRAFT_ID_PREFIX));
        if !dropped_routines.is_empty() {
            self.log(format!(
                "{} carried {} — no longer available, and the slot is now empty.",
                species.name,
                dropped_routines.join(", ")
            ));
        }
        let party_slot = c.party_slot;
        let sortie_index = c.sortie_index;
        // Every creature has these, not only owned ones, so this is
        // resolved here rather than inside the `tamed` block below. An
        // empty map is what a save written before the feature carries, and
        // minting here rather than on first read is what keeps
        // `Game::dossier_report` a `&self` derivation. Built before the
        // spawn because the `EntityWorldMut` below holds the world.
        let attributes = if c.attributes.is_empty() {
            crate::attributes::mint(
                self.world.resource::<crate::attributes::AttributeDb>(),
                crate::attributes::body_seed(
                    self.world.resource::<WorldMap>().seed(),
                    c.position.0,
                    c.position.1,
                    species.id.as_str(),
                    c.zone,
                ),
                &species.attributes,
            )
        } else {
            let mut restored = crate::components::Attributes::default();
            for (id, value) in &c.attributes {
                restored.set(id, *value);
            }
            restored
        };
        let mut entity = self.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: c.position.0,
                y: c.position.1,
            },
            Glyph {
                ch: species.glyph,
                color: species.color,
            },
            Stats {
                hp: c.hp,
                max_hp: c.max_hp,
                atk: c.atk,
                mitigation: c.mitigation,
            },
            c.potential(),
            ZonePortal(c.zone),
            StatusEffects::default(),
            FusionCount(c.fusions),
            Refactors(c.refactors),
            PurchasedTiers(c.purchased_tiers),
            Routines(routines),
            // The tag only. `Stats` above are the recorded numbers and
            // already carry this tier's multiplier from the spawn that
            // rolled it — re-applying `stat_mult` here would compound
            // the bonus on every reload. See `Rarity`'s doc.
            c.rarity,
        ));
        // Only when non-zero, the same idiom `Nemesis` and `Equipment`
        // use below: absent must keep meaning "no ring open", since
        // `Game::companion_level_cap` reads it as `map_or(0, ..)`.
        if c.ring > 0 {
            entity.insert(KernelRing(c.ring));
        }
        // The receipt only. `Stats` above are the recorded numbers and
        // already carry every `Stat` node this list names — re-applying
        // here would compound the bonus on every reload, the same trap
        // `c.rarity` above documents.
        if !c.talents.is_empty() {
            entity.insert(Talents(
                c.talents
                    .iter()
                    .map(|id| crate::talents::TalentId::from(id.as_str()))
                    .collect(),
            ));
        }
        // A receipt, like `Talents` above: what those nodes already added
        // to the recorded `Stats`, kept so `Game::respec_talents` can take
        // exactly that much back out.
        if c.bought_stats != crate::components::BoughtStats::default() {
            entity.insert(c.bought_stats);
        }
        if let Some(name) = c.custom_name.clone() {
            entity.insert(CustomName(name));
        }
        // Inserted only when the count is nonzero, the same idiom
        // `Equipment`/`FieldBuff` use below: an absent component must
        // keep meaning "not a nemesis", since `mark_nemeses`' cap counts
        // live holders by querying `With<Nemesis>` rather than tracking
        // them anywhere. `Stats` above already carry every promotion this
        // grudge count earned — nothing here may touch them, or a
        // reload would compound `promote_rarity`'s multiplier on top of
        // itself. See the `c.rarity` comment a few lines above for the
        // same trap on the tag it promoted.
        if c.nemesis_grudges > 0 {
            entity.insert(Nemesis(c.nemesis_grudges));
            // Absent already reads as zero, so only a running muster is
            // written back.
            if c.nemesis_muster_ticks > 0 {
                entity.insert(crate::components::NemesisMuster {
                    ticks: c.nemesis_muster_ticks,
                });
            }
            if let Some((x, y)) = c.nemesis_home {
                entity.insert(crate::components::NemesisHome(Position { x, y }));
            }
        }
        // Inserted only when nonzero: absent already reads as generation 0
        // and as ready to breed.
        if c.generation > 0 {
            entity.insert(crate::components::Generation(c.generation));
        }
        if c.breed_ready_at > 0 {
            entity.insert(crate::components::BreedReadyAt(c.breed_ready_at));
        }
        // Inserted only when set, so an absent component keeps meaning
        // "not a boss" — `is_boss_creature`'s species fallback still
        // answers for an apex species loaded from a file written before
        // this field existed. `Stats` above already carry
        // `BOSS_STAT_MULT` from the spawn that rolled it; nothing here
        // may re-apply it, the same trap `c.rarity` documents.
        if c.boss {
            entity.insert(Boss);
        }
        // Inserted only when something is worn, so an absent component
        // keeps meaning "wears nothing" — the invariant `Game::equip`
        // relies on when it grows one on demand. A v27 dump defaults
        // this to empty and lands here.
        if !c.equipment.is_empty() {
            let mut worn = Equipment::default();
            for (slot, saved) in c.equipment.clone() {
                *worn.slot_mut(slot) = worn_from_save(
                    Some(saved.item),
                    saved.level,
                    saved.fusion_tier,
                    saved.rarity,
                    saved.affix,
                    saved.affixes,
                    saved.quality,
                    saved.wear,
                );
            }
            entity.insert(worn);
        }
        // Only the player is spawned holding a `FieldBuff` — see that
        // component's docs — so a creature with none recorded stays
        // without one, the same as a freshly tamed program.
        if !c.field_buffs.is_empty() {
            entity.insert(FieldBuff {
                active: c.field_buffs.clone(),
            });
        }
        entity.insert(attributes);
        if c.tamed {
            let creature_id = entity.id();
            // Minted here for a file written before ids existed, which
            // carries the sentinel for everyone. An id already in the
            // file is that program's name and is never reissued.
            let program_id = if c.program_id == 0 {
                ctx.next_program_id += 1;
                ctx.next_program_id - 1
            } else {
                c.program_id
            };
            // Owned programs only, beside the id: the store's absence is
            // what "not on the roster" means, so a wild creature's
            // `memories` is written empty and read back nowhere, exactly
            // as its `power` is. An entry naming a def no file defines is
            // restored rather than purged — see `components::Memory`.
            let memories = Memories(
                c.memories
                    .iter()
                    .map(|m| Memory {
                        def: m.def.clone(),
                        subject: m.subject.clone(),
                        subject_name: m.subject_name.clone(),
                        reinforced: m.reinforced,
                        strikes: m.strikes,
                    })
                    .collect(),
            );
            // Absent until a program's first conversation, so an empty list
            // inserts nothing and the two stay the same state.
            if !c.conversations.is_empty() {
                entity.insert(crate::components::Conversations(
                    c.conversations.iter().cloned().collect(),
                ));
            }
            // A file written before needs existed carries no key and
            // loads empty; `needs_drain_system` seeds it full on the first
            // tick, which is the one seeding site.
            let mut needs = Needs::default();
            for (id, value) in &c.needs {
                needs.set(id, *value);
            }
            if let Some(need) = c.off_shift.clone() {
                entity.insert(crate::components::OffShift { need });
            }
            if c.downed {
                entity.insert(crate::components::Downed);
            }
            if let Some(grievance) = c.disgruntled {
                entity.insert(crate::components::Disgruntled {
                    grievance,
                    stranded: c.disgruntled_stranded,
                    // Never saved, `Needs::stalled_announced`'s rule — a
                    // reload says the line again if the body is still
                    // stranded when it loads.
                    told: false,
                });
            }
            // An unknown name (a duty retired since the file was written)
            // is dropped silently, as an unknown FTUE key is — and if every name
            // was unknown, the resulting empty set is not inserted at all,
            // so the program reads as unrestricted rather than as
            // explicitly (and meaninglessly) barred from nothing.
            let off: std::collections::BTreeSet<_> = c
                .off_duties
                .iter()
                .filter_map(|name| crate::duties::Duty::from_name(name))
                .collect();
            if !off.is_empty() {
                entity.insert(crate::components::Duties { off });
            }
            if c.drop_trooper {
                entity.insert(crate::components::DropTrooper);
            }
            match c.staff_rank {
                Some(rank) => {
                    entity.insert(crate::components::StaffRank(rank));
                    ctx.max_rank_seen = ctx.max_rank_seen.max(rank);
                }
                None => ctx.pending_ranks.push(creature_id),
            }
            entity.insert((
                ProgramId(program_id),
                memories,
                needs,
                // The one seeding site for a file written before
                // dispositions existed, `Needs::seed_missing`'s role.
                // Derived from the id rather than defaulted to `Steady`,
                // so an existing roster gains the personalities it would
                // have had instead of a base full of neutral programs.
                c.disposition
                    .unwrap_or_else(|| crate::disposition::Disposition::seed(program_id)),
                Tamed { owner: ctx.player },
                PowerReserve::new(c.power, POWER_MAX),
                Experience {
                    level: c.level,
                    xp: c.xp,
                    // Derived, like the player's in `Game::load` — both
                    // load paths, or the stale value survives on half the
                    // roster.
                    xp_to_next: crate::progression::xp_for_level(c.level),
                },
            ));
            // Unlike a cronjob target, a load names no entity, so it
            // needs none of the deferred `pending_cronjobs` treatment.
            if let Some((item, qty)) = c.carrying.clone() {
                entity.insert(Carrying { item, qty });
                if c.clearing {
                    entity.insert(crate::components::Clearing);
                }
            }
            if let Some(program) = c.carrying_program.clone() {
                entity.insert(crate::components::CarryingProgram(program));
            }
            if let Some(thing) = c.tidying.clone() {
                entity.insert(crate::components::Tidying(thing));
            }
            if let Some(index) = sortie_index {
                ctx.sortie_members.push((index, creature_id));
            }
            if let (Some(order), Some(cell)) = (c.siege_order, c.siege_cell) {
                ctx.pending_siege_members.push((order, creature_id, cell));
            }
            if let Some(slot) = party_slot {
                ctx.party_slots.push((slot, creature_id));
            } else if let Some(tile) = c.study_station {
                // **Checked before `cronjob`.** `Game::pin_subject` frees a
                // program's stale `Task` the moment it is pinned, so the two
                // fields no longer co-occur in a save this build writes —
                // but an older save can carry both for a creature that was
                // posted when the bug that let that happen shipped, and the
                // study tether has to be the one that survives a reload,
                // not the stale job the pin was supposed to have replaced.
                // Deferred for `pending_cronjobs`' own reason: a Research
                // Station is one of the structures rebuilt further down
                // `Game::load`, so there is nothing yet for a tile to name.
                ctx.pending_study.push((creature_id, tile));
            } else if let Some(tile) = c.outpost {
                // Same precedence argument as `study_station` above, one
                // role over: `Game::post_to_outpost` frees a program's stale
                // `Task` the moment it is posted, so this and `cronjob`
                // shouldn't co-occur in a save this build writes, but an
                // older or hand-edited one could carry both.
                ctx.pending_outpost_crew.push((creature_id, tile));
            } else if let Some(tile) = c.siphon {
                // After `study_station` and `outpost`, before `cronjob`:
                // `Game::siphon_program` frees the program's `Task`, so a
                // held program is never also posted, and where a hand-edited
                // save says both, the hold is the tether that survives.
                ctx.pending_siphon.push((creature_id, tile));
            } else if let Some(jailed) = c.jailed.clone() {
                // Beside `siphon`, for its reason: `Game::jail_program` spawns
                // the body with no `Task`, so a prisoner is never also
                // posted, and a hand-edited save that says both keeps the
                // cell.
                ctx.pending_jailed.push((creature_id, jailed));
            } else if let Some(cronjob) = c.cronjob.clone() {
                ctx.pending_cronjobs.push((creature_id, cronjob));
            }
            // Nothing restores a staff marker, and `c.staff` is read
            // nowhere: the role is derived from the party and the wield,
            // both of which this load path already rebuilds. That also
            // retires the absorption rule this branch used to carry —
            // a save written before work orders existed came back with
            // no flag on disk and had to be rescued by its `Task`, and
            // now everything outside the party is staff regardless.
            // `Experience::xp_to_next` is the same shape: still written,
            // never read back, and so **no `SAVE_FORMAT_VERSION` bump**.
        } else {
            entity.insert((Hostile, WanderAi::default()));
            // `carrying`'s own restore, missing here until now — written
            // unconditionally above for any creature holding `Carrying`,
            // tamed or not, but only ever read back on the tamed branch.
            // A besieger killed after a reload has to still drop what it
            // was holding, so a wild body needs this too.
            if let Some((item, qty)) = c.carrying.clone() {
                entity.insert(Carrying { item, qty });
            }
            if c.besieger {
                entity.insert(crate::components::Besieger);
            }
            if let Some(tile) = c.stolen_from {
                ctx.pending_stolen_from.push((entity.id(), tile));
            }
            if let (Some(order), Some(cell)) = (c.siege_order, c.siege_cell) {
                ctx.pending_siege_members.push((order, entity.id(), cell));
            }
            if let Some(band) = c.nemesis_band {
                ctx.pending_nemesis_band
                    .push((band, entity.id(), c.nemesis_grudges > 0));
            }
            // A nest_position resolving to nothing (the nest's species
            // is gone, or the save predates nests) is dropped silently
            // rather than failing the load — the creature just comes
            // back as an ordinary wild program.
            if let Some(nest) = c
                .nest_position
                .and_then(|p| ctx.nest_positions.get(&p).copied())
            {
                entity.insert(NestGuardian { nest });
                if c.pursuing {
                    entity.insert(Pursuing);
                }
            }
            // Deferred rather than resolved here: settlement entities
            // are rebuilt further down `Game::load`, after every creature,
            // so there is nothing yet for a tile to name.
            // `pending_cronjobs`' treatment, for the same reason.
            if let Some(tile) = c.patrol_position {
                ctx.pending_patrols.push((entity.id(), tile, c.pursuing));
            }
        }
        let id = entity.id();
        if let Some(base) = c.base.filter(|_| c.tamed) {
            // Restored, never seated: the record carries no figures to seat
            // from, and `Game::load` refuses a tamed record without a base.
            // Recompute last, once gear, the receipt and attributes are
            // all in place.
            self.world.entity_mut(id).insert((
                crate::components::ProgramBase(base),
                crate::components::Derived::default(),
                crate::components::StatPoints(c.stat_points),
                crate::components::HoldPoints(c.hold_points),
            ));
            self.recompute_derived(id);
        }
        Some(id)
    }
}
