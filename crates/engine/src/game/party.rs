//! The player's own roster — status, the programs in the party and the
//! bank, and moving programs between them.

use crate::tuning::{FUSION_LESSER_STAT_DIVISOR, MAX_FUSIONS};
use crate::*;

impl Game {
    pub fn player_status(&self) -> PlayerStatus {
        let pet_count = self.pet_count();
        let pet_capacity = self.pet_capacity();
        let player = self.player_entity();
        let stats = self.world.get::<Stats>(player).unwrap();
        let needs = self.world.get::<Needs>(player).unwrap();
        let pos = self.world.get::<Position>(player).unwrap();
        let inv = self.world.get::<Inventory>(player).unwrap();
        let exp = self.world.get::<Experience>(player).unwrap();
        let decompiler = self
            .world
            .get::<Decompiler>(player)
            .map(|d| d.skill)
            .unwrap_or(0);
        let equipment = self
            .world
            .get::<Equipment>(player)
            .cloned()
            .unwrap_or_default();
        let perks = self.world.get::<Perks>(player);
        let atk = self.effective_atk(player);
        let def = self.effective_def(player);
        let db = self.world.resource::<ItemDb>();
        PlayerStatus {
            position: (pos.x, pos.y),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk,
            def,
            power: stats.max_hp + atk + def,
            decompiler,
            hunger: needs.hunger,
            fatigue: needs.fatigue,
            inventory: inv.items.clone(),
            inventory_used: inv.cargo_used(db),
            pet_count,
            pet_capacity,
            level: exp.level,
            xp: exp.xp,
            xp_to_next: exp.xp_to_next,
            weapon: equipment.weapon,
            armor: equipment.armor,
            module: equipment.module,
            companions: self.party_info(),
            zone: self.world.resource::<ZoneLevel>().0,
            perk_points: perks.map(|p| p.points).unwrap_or(0),
            unlocked_perks: perks.map(|p| p.unlocked.clone()).unwrap_or_default(),
        }
    }

    /// A creature's own display name: the player's `CustomName` if they set
    /// one (currently only via `Game::fuse_companions`), else its species
    /// name (falling back to the raw species id if the species definition
    /// is somehow missing). `None` if `entity` isn't a `Creature` at all.
    pub(crate) fn creature_name(&self, entity: Entity) -> Option<String> {
        let c = self.world.get::<Creature>(entity)?;
        if let Some(custom) = self.world.get::<CustomName>(entity) {
            return Some(custom.0.clone());
        }
        Some(
            self.world
                .resource::<SpeciesDb>()
                .get(&c.species)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| c.species.clone()),
        )
    }

    /// `creature_name`, zone-tagged, falling back to a generic label if
    /// `entity` isn't a `Creature`.
    pub(crate) fn creature_label(&self, entity: Entity) -> String {
        match self.creature_name(entity) {
            Some(name) => self.zone_tagged_name(entity, name),
            None => "Program".to_string(),
        }
    }

    /// Appends a creature's `ZonePortal` to its species name for display
    /// (e.g. "Scrapper 2"), so a deeper-zone catch reads differently from a
    /// shallow one at a glance. Falls back to the bare name if the entity
    /// has no `ZonePortal` — expected for creatures hand-spawned outside the
    /// normal `spawn_wild_creature` path (e.g. in tests).
    pub(crate) fn zone_tagged_name(&self, entity: Entity, name: String) -> String {
        match self.world.get::<ZonePortal>(entity) {
            Some(zone) => format!("{name} {}", zone.0),
            None => name,
        }
    }

    /// Whether `entity` is a creature of a boss species (`SpeciesDef::is_boss`).
    /// `false` for anything that isn't a creature, or whose species failed
    /// to resolve.
    pub(crate) fn is_boss_creature(&self, entity: Entity) -> bool {
        self.world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .is_some_and(|s| s.is_boss)
    }

    pub(crate) fn companion_info(&self, entity: Entity) -> Option<CompanionInfo> {
        let stats = self.world.get::<Stats>(entity)?;
        Some(CompanionInfo {
            entity,
            name: self.creature_label(entity),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk: stats.atk,
            def: stats.def,
            power: stats.power(),
            status: self.status_label(entity),
            ability: self.ability_label(entity),
        })
    }

    /// Terse label for what commanding `entity` in battle would do right
    /// now. A member with several abilities reads as a count, since no one
    /// of them is *the* answer until the player picks in
    /// `Mode::BattleSpecial`.
    pub(crate) fn ability_label(&self, entity: Entity) -> String {
        match self.actor_abilities(entity).as_slice() {
            // Only the player can be empty: `companion_abilities` resolves
            // the fallback rather than returning nothing.
            [] => "No routines researched".to_string(),
            [only] => only.name.clone(),
            many => format!("{} abilities", many.len()),
        }
    }

    /// How many fusions deep `entity`'s lineage is (see
    /// `components::FusionCount`) — 0 for anything caught or spawned
    /// normally, up to `MAX_FUSIONS`, at which point it can't be fused
    /// again.
    pub fn fusion_count(&self, entity: Entity) -> u32 {
        self.world
            .get::<FusionCount>(entity)
            .map(|f| f.0)
            .unwrap_or(0)
    }

    /// Display string for `entity`'s rolled `Potential`, e.g.
    /// "Excellent (94%)" — `None` if it has no `Potential` component (an
    /// old save predating it, or a non-creature entity).
    pub(crate) fn potential_quality_label(&self, entity: Entity) -> Option<String> {
        let potential = self.world.get::<Potential>(entity)?;
        Some(format!(
            "{} ({}%)",
            potential.quality_label(),
            potential.quality_percent()
        ))
    }

    /// Snapshot of every current party member (see `resources::Party`), in
    /// party-slot order.
    pub(crate) fn party_info(&self) -> Vec<CompanionInfo> {
        self.world
            .resource::<Party>()
            .0
            .iter()
            .filter_map(|&e| self.companion_info(e))
            .collect()
    }

    /// Full stats for every tamed program the player owns, anywhere on the
    /// map — unlike `view_entities`, not limited to what's currently in
    /// view. Lets you check on a cronjob worker's HP/level without walking
    /// over to it.
    pub fn owned_pets(&mut self) -> Vec<PetInfo> {
        let player = self.player_entity();
        let party = self.world.resource::<Party>().0.clone();
        let owned: Vec<Entity> = {
            let mut query = self.world.query::<(Entity, &Tamed)>();
            query
                .iter(&self.world)
                .filter(|(_, t)| t.owner == player)
                .map(|(e, _)| e)
                .collect()
        };
        owned
            .into_iter()
            .filter_map(|entity| {
                let stats = *self.world.get::<Stats>(entity)?;
                let level = self
                    .world
                    .get::<Experience>(entity)
                    .map(|e| e.level)
                    .unwrap_or(1);
                Some(PetInfo {
                    entity,
                    name: self.creature_label(entity),
                    level,
                    hp: stats.hp,
                    max_hp: stats.max_hp,
                    atk: stats.atk,
                    def: stats.def,
                    power: stats.power(),
                    is_companion: party.contains(&entity),
                    activity: self.program_activity(entity),
                    quality: self.potential_quality_label(entity),
                    fusions: self.fusion_count(entity),
                })
            })
            .collect()
    }

    /// Display string for `entity`'s current active status condition, if
    /// any — e.g. "Bleeding (2)" or "Stunned (1)", the number being battle
    /// rounds remaining. `None` if it has no active condition.
    pub(crate) fn status_label(&self, entity: Entity) -> Option<String> {
        let active = self.world.get::<StatusEffects>(entity)?.active?;
        Some(match active.kind {
            StatusKind::Bleed => format!("Bleeding ({})", active.remaining),
            StatusKind::Stun => format!("Stunned ({})", active.remaining),
        })
    }

    /// Adds `creature` (a tamed program you own) to your active battle
    /// party (see `resources::Party`), up to `MAX_PARTY_SIZE` at once.
    /// Clears an in-progress cronjob task on it first — a program can only
    /// be doing one job (working a structure, or fighting beside you) at a
    /// time.
    pub fn add_companion(&mut self, creature: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let owner = self
            .world
            .get::<Tamed>(creature)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != player {
            return Err("You don't control that program.".into());
        }
        if self.world.resource::<Party>().0.contains(&creature) {
            return Err("That program is already in your party.".into());
        }
        if self.world.resource::<Party>().0.len() >= MAX_PARTY_SIZE {
            return Err(format!(
                "Your party is full ({MAX_PARTY_SIZE} max) — stand one down first."
            ));
        }
        self.world.entity_mut(creature).remove::<Task>();
        self.world.resource_mut::<Party>().0.push(creature);
        let name = self.creature_label(creature);
        self.log(format!("{name} falls in alongside you."));
        Ok(())
    }

    /// Stands `creature` down from the active party, if it's a member — it
    /// remains a tamed program, just no longer commandable in battle. A
    /// no-op (no log) if it wasn't in the party to begin with.
    pub fn remove_companion(&mut self, creature: Entity) {
        let was_present = {
            let mut party = self.world.resource_mut::<Party>();
            let before = party.0.len();
            party.0.retain(|&e| e != creature);
            party.0.len() != before
        };
        if was_present {
            let name = self.creature_label(creature);
            self.log(format!("{name} falls back from active duty."));
        }
    }

    /// Fuses two of the player's tamed programs (`a` and `b`, any species,
    /// party members or not) into one new tamed program, consuming both.
    /// The result keeps the species (and so the moves/work aptitude) of
    /// whichever input is the higher level — ties favor `a` — at that same
    /// level, with each stat computed as `higher + lower / 2` so a fusion
    /// is always stronger than either input alone without simply summing
    /// them (which would make repeated fusion runaway). A resource sink for
    /// duplicate catches: there's no separate item cost, since losing two
    /// programs to gain one is the cost.
    ///
    /// Fusion depth is capped: neither input may already be `MAX_FUSIONS`
    /// deep (see `components::FusionCount`), and the result is one deeper
    /// than its deepest input.
    /// `custom_name`, if given, is trimmed and truncated to
    /// `MAX_CUSTOM_NAME_LEN` characters and becomes the fused program's
    /// display name everywhere (see `CustomName`) instead of its species
    /// name. Blank (or all-whitespace) is treated the same as `None`.
    pub fn fuse_companions(
        &mut self,
        a: Entity,
        b: Entity,
        custom_name: Option<String>,
    ) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if a == b {
            return Err("Pick two different programs to fuse.".into());
        }
        let player = self.player_entity();
        for e in [a, b] {
            let owner = self
                .world
                .get::<Tamed>(e)
                .ok_or_else(|| "Both programs must be compiled under your control.".to_string())?
                .owner;
            if owner != player {
                return Err("You don't control both programs.".into());
            }
        }
        for e in [a, b] {
            if self.fusion_count(e) >= MAX_FUSIONS {
                let name = self.creature_label(e);
                return Err(format!(
                    "{name} has already been fused {MAX_FUSIONS} times — it can't be fused again."
                ));
            }
        }
        let fused_depth = self.fusion_count(a).max(self.fusion_count(b)) + 1;
        let (species_a, exp_a, stats_a, potential_a) = (
            self.world.get::<Creature>(a).unwrap().species.clone(),
            *self.world.get::<Experience>(a).unwrap(),
            *self.world.get::<Stats>(a).unwrap(),
            self.world
                .get::<Potential>(a)
                .copied()
                .unwrap_or(Potential::NEUTRAL),
        );
        let (species_b, exp_b, stats_b, potential_b) = (
            self.world.get::<Creature>(b).unwrap().species.clone(),
            *self.world.get::<Experience>(b).unwrap(),
            *self.world.get::<Stats>(b).unwrap(),
            self.world
                .get::<Potential>(b)
                .copied()
                .unwrap_or(Potential::NEUTRAL),
        );
        let (species_id, level) = if exp_a.level >= exp_b.level {
            (species_a, exp_a.level)
        } else {
            (species_b, exp_b.level)
        };
        let species = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .cloned()
            .ok_or_else(|| "That species is no longer available.".to_string())?;

        fn fuse_stat(x: i32, y: i32) -> i32 {
            x.max(y) + x.min(y) / FUSION_LESSER_STAT_DIVISOR
        }
        let fused_hp = fuse_stat(stats_a.max_hp, stats_b.max_hp);
        let fused_atk = fuse_stat(stats_a.atk, stats_b.atk);
        let fused_def = fuse_stat(stats_a.def, stats_b.def);
        let fused_potential = Potential::averaged(potential_a, potential_b);

        let name_a = self.creature_label(a);
        let name_b = self.creature_label(b);
        self.world
            .resource_mut::<Party>()
            .0
            .retain(|&e| e != a && e != b);
        self.world.despawn(a);
        self.world.despawn(b);

        let final_name: Option<String> = custom_name.and_then(|n| {
            let trimmed = n.trim();
            (!trimmed.is_empty()).then(|| {
                trimmed
                    .chars()
                    .take(MAX_CUSTOM_NAME_LEN)
                    .collect::<String>()
            })
        });

        let player_pos = *self.world.get::<Position>(player).unwrap();
        let mut fused = self.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
            Glyph {
                ch: species.glyph,
                color: species.color,
            },
            Stats {
                hp: fused_hp,
                max_hp: fused_hp,
                atk: fused_atk,
                def: fused_def,
            },
            fused_potential,
            Tamed { owner: player },
            Experience {
                level,
                xp: 0,
                xp_to_next: progression::xp_for_level(level),
            },
            ZonePortal(1),
            StatusEffects::default(),
            FusionCount(fused_depth),
        ));
        let fused_entity = fused.id();
        if let Some(name) = &final_name {
            fused.insert(CustomName(name.clone()));
        }
        self.install_innate_routines(fused_entity);
        self.log(match &final_name {
            Some(name) => format!(
                "You fuse {name_a} and {name_b} into {name}, a new {}.",
                species.name
            ),
            None => format!(
                "You fuse {name_a} and {name_b} into a new {}.",
                species.name
            ),
        });
        Ok(())
    }
}
