use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::abilities::AffinityKind;
use crate::components::{GlyphColor, StatusKind};
use crate::items::ItemId;
use crate::world::Biome;

pub type SpeciesId = String;

/// One ability a species grants a tamed member, and the level it unlocks
/// at. The ability itself is data — see `assets/abilities/` and
/// `crate::abilities::AbilityDef`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeciesAbility {
    pub id: crate::abilities::AbilityId,
    /// Companion level at which this becomes usable. `#[serde(default)]` to
    /// 1 — available as soon as the program is tamed.
    #[serde(default = "default_learn_level")]
    pub level: u32,
}

fn default_learn_level() -> u32 {
    1
}

/// A status condition a move has a chance to inflict on top of its direct
/// damage — see `components::StatusEffects`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveEffect {
    pub kind: StatusKind,
    /// Chance (0.0-1.0) this effect actually applies when the move lands.
    pub chance: f32,
    /// How many battle rounds the effect lasts.
    pub duration: u32,
    /// Bleed damage dealt per round; unused (but still required in the
    /// `.ron` file — use 0) for `Stun`.
    #[serde(default)]
    pub power: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoveDef {
    pub name: String,
    pub power: i32,
    /// Optional status effect this move has a chance to inflict on the
    /// target when it lands, independent of its direct damage.
    /// `#[serde(default)]` so existing species files (including mods)
    /// without this field keep parsing as plain damage-only moves.
    #[serde(default)]
    pub effect: Option<MoveEffect>,
    /// Whether this move reaches past the front line. A group standing
    /// behind `crate::tuning::ENGAGED_GROUPS` can only use its ranged moves, and
    /// idles if it has none. `#[serde(default)]` so existing species files
    /// (including mods) without this field keep parsing as melee, exactly
    /// as they behaved before it existed.
    #[serde(default)]
    pub ranged: bool,
}

fn default_affinity() -> f32 {
    crate::tuning::AFFINITY_NEUTRAL
}

/// A species' multiplier per ability category (see
/// `abilities::AffinityKind`). Every field defaults individually to
/// `AFFINITY_NEUTRAL`, so a file may name only the categories it cares
/// about; the struct as a whole is `#[serde(default)]` on `SpeciesDef`, so
/// a file may omit it entirely. Both defaults are needed — one covers the
/// partial form, the other the absent form.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Affinities {
    #[serde(default = "default_affinity")]
    pub damage: f32,
    #[serde(default = "default_affinity")]
    pub heal: f32,
    #[serde(default = "default_affinity")]
    pub buff: f32,
    #[serde(default = "default_affinity")]
    pub debuff: f32,
    #[serde(default = "default_affinity")]
    pub drain: f32,
}

impl Default for Affinities {
    fn default() -> Self {
        Affinities::NEUTRAL
    }
}

impl Affinities {
    /// No bonus and no penalty in any category — what the player resolves
    /// to before perks, and what a species that declares nothing gets.
    pub const NEUTRAL: Affinities = Affinities {
        damage: crate::tuning::AFFINITY_NEUTRAL,
        heal: crate::tuning::AFFINITY_NEUTRAL,
        buff: crate::tuning::AFFINITY_NEUTRAL,
        debuff: crate::tuning::AFFINITY_NEUTRAL,
        drain: crate::tuning::AFFINITY_NEUTRAL,
    };

    pub fn get(&self, kind: AffinityKind) -> f32 {
        match kind {
            AffinityKind::Damage => self.damage,
            AffinityKind::Heal => self.heal,
            AffinityKind::Buff => self.buff,
            AffinityKind::Debuff => self.debuff,
            AffinityKind::Drain => self.drain,
        }
    }

    /// Every category this species is not neutral in, in a fixed order —
    /// what the manifest screen lists. A species with nothing to say
    /// returns empty and the screen shows no section at all, rather than
    /// five rows of 1.00.
    pub fn non_neutral(&self) -> Vec<(AffinityKind, f32)> {
        [
            AffinityKind::Damage,
            AffinityKind::Heal,
            AffinityKind::Buff,
            AffinityKind::Debuff,
            AffinityKind::Drain,
        ]
        .into_iter()
        .map(|k| (k, self.get(k)))
        .filter(|&(_, v)| v != crate::tuning::AFFINITY_NEUTRAL)
        .collect()
    }

    /// Names the first non-finite field, if any. RON accepts bare
    /// `NaN`/`inf` literals, and `f32::clamp` returns NaN for a NaN input
    /// — so the clamp below cannot contain one and the file has to be
    /// refused instead. Same rationale as `AbilityDef::non_finite_field`.
    fn non_finite_field(&self) -> Option<&'static str> {
        for (name, v) in [
            ("damage", self.damage),
            ("heal", self.heal),
            ("buff", self.buff),
            ("debuff", self.debuff),
            ("drain", self.drain),
        ] {
            if !v.is_finite() {
                return Some(name);
            }
        }
        None
    }

    fn clamp_all(&mut self) {
        for v in [
            &mut self.damage,
            &mut self.heal,
            &mut self.buff,
            &mut self.debuff,
            &mut self.drain,
        ] {
            *v = v.clamp(crate::tuning::AFFINITY_MIN, crate::tuning::AFFINITY_MAX);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeciesDef {
    pub id: SpeciesId,
    pub name: String,
    pub glyph: char,
    pub color: GlyphColor,
    pub base_hp: i32,
    pub base_atk: i32,
    pub base_def: i32,
    /// 0.0 (trivial) .. 1.0 (very hard) to tame.
    pub taming_difficulty: f32,
    pub habitats: Vec<Biome>,
    /// This species' initiative baseline — each round every combatant rolls
    /// `base_speed + d10` and acts in descending order (see
    /// `Game::roll_initiative`). `#[serde(default)]` so existing species
    /// files (including mods) without this field keep parsing at the roster
    /// average.
    #[serde(default = "default_base_speed")]
    pub base_speed: i32,
    /// How good a member of this species is at *extracting* — the fourth
    /// term in `systems::mining_success_chance`, so it moves how often a
    /// worked node fizzles rather than what a successful cycle pays.
    ///
    /// Read as a **deviation from `tuning::DEFAULT_BASE_INT`**, which is why
    /// `#[serde(default)]` here is stronger than the usual modding promise:
    /// an existing species file (including a mod's) doesn't merely keep
    /// parsing, it keeps extracting at precisely its old rate, because the
    /// term it contributes is zero.
    ///
    /// Not on `Stats`, and that is a decision rather than a shortcut: a stat
    /// grows on level-up, so a level-20 bruiser would out-think a level-1
    /// specialist and role would collapse back into tier — the confound this
    /// field exists to remove. A species' aptitude is fixed to the species.
    #[serde(default = "default_base_int")]
    pub base_int: i32,
    pub moves: Vec<MoveDef>,
    /// If set, the item a defeated or nest-destroyed member of this species
    /// drops as salvage (`Game::award_loot`, `Game::grant_nest_cache`), and
    /// what the inspection view names as its yield. Despite the name, this
    /// does not gate which structures a tamed member can work — any program
    /// can be posted to any producing structure, and what a cronjob pays out
    /// comes from the structure's own `produces`, not from the worker's
    /// species.
    pub work_resource: Option<ItemId>,
    /// If set, defeating/decompiling this species has a chance (0.0-1.0) to
    /// additionally drop one piece of equipment, independent of
    /// `work_resource`. `#[serde(default)]` so existing species files
    /// (including mods) without this field keep parsing.
    #[serde(default)]
    pub equipment_drop: Option<(ItemId, f32)>,
    /// Marks this species as a boss: excluded from the normal per-tile
    /// habitat spawn roll and instead spawned rarely in its place (see
    /// `Game::try_spawn_habitat_creature`), and guaranteed a cache of
    /// Portal Fragments on defeat instead of the flat drop chance every
    /// other species rolls (see `Game::award_loot`). A boss's stats are
    /// still whatever `base_hp`/`base_atk`/`base_def` are authored as —
    /// there's no separate engine-side multiplier, so make them tough in
    /// the `.ron` file itself. `#[serde(default)]` so existing species
    /// files (including mods) without this field keep parsing as ordinary,
    /// non-boss species.
    #[serde(default)]
    pub is_boss: bool,
    /// The abilities a tamed member of this species is created holding, in
    /// menu order, each gated on the companion's level. Left empty, the
    /// companion is created holding `abilities::FALLBACK_ABILITY_ID`
    /// instead — see `Game::install_innate_routines`, which is the only
    /// place that fallback is resolved. `#[serde(default)]` so existing
    /// species files (including mods) without this field keep parsing.
    #[serde(default)]
    pub abilities: Vec<SpeciesAbility>,
    /// Multiplies this species' per-level stat growth (see
    /// `progression::add_xp`) for a tamed member of it — 1.0 (the default)
    /// grows at the same flat rate as before this field existed; a
    /// higher-tier species can set e.g. `1.5` to out-grow an easy one
    /// level for level. `#[serde(default)]` (via
    /// `crate::tuning::BASELINE_GROWTH_MULTIPLIER`) so existing species
    /// files (including mods) without this field keep growing exactly as
    /// they did before.
    #[serde(default = "default_growth_multiplier")]
    pub growth_multiplier: f32,
    /// This species' per-category ability multipliers — what a member of it
    /// is *good at*, as opposed to `growth_multiplier`, which scales all
    /// three stats uniformly. Applies to whatever is installed in its
    /// routine slots, not only to the abilities this file grants, so a
    /// strong heal affinity with no innate heal is a reason to install one
    /// here rather than a contradiction. `#[serde(default)]` so existing
    /// species files (including mods) without this field keep parsing at
    /// neutral.
    #[serde(default)]
    pub affinities: Affinities,
    /// Lines a tamed member of this species says when goaded into speech.
    /// Purely cosmetic — nothing reads them but the log. A species with
    /// none falls back to a generic engine-side line, so authoring them is
    /// optional in a way `moves` is not. `#[serde(default)]` so existing
    /// species files (including mods) without this field keep parsing.
    #[serde(default)]
    pub taunts: Vec<String>,
    /// Whether this species can spawn as a Nest — a stationary,
    /// destructible object that keeps 2-5 guardians of this species
    /// tethered around it and respawns any that are killed/tamed, until
    /// the nest itself is destroyed (see `components::Nest`,
    /// `Game::try_spawn_habitat_creature`). `#[serde(default)]` so
    /// existing species files (including mods) without this field keep
    /// parsing as non-nesting, same as before this field existed. Never
    /// applies to a boss species regardless of this flag — the habitat
    /// spawn roll only ever considers it for the ordinary (non-boss)
    /// pick.
    #[serde(default)]
    pub can_nest: bool,
}

/// The role a species is read as, independent of its tier — the axis the
/// class system is built on. See `assets/species/README.md`'s "The five
/// classes" for the authored reference.
///
/// It is **derived, never declared**: a class is the one affinity axis a
/// species raises, cross-checked in the censuses against the stat shape and
/// the kit that axis implies. That is why there is no `role` field in the
/// `.ron` schema — three things have to agree, and a field would let a
/// species claim a role its numbers contradict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AffinityClass {
    Striker,
    Bastion,
    Medic,
    Saboteur,
    Leech,
}

impl SpeciesDef {
    /// The class this species reads as, or `None` for one that raises no
    /// axis or more than one.
    ///
    /// `None` is what a boss is — they carry no affinities at all and sit
    /// outside the class system — and it is also the honest answer for a
    /// mod that raises two axes. Both cases must resolve to *no base job*
    /// rather than to some default class, which is what the `Option` buys
    /// over a fallback: a species with no readable role should not silently
    /// acquire a Medic's repair rate.
    pub fn affinity_class(&self) -> Option<AffinityClass> {
        let mut raised = self
            .affinities
            .non_neutral()
            .into_iter()
            .filter(|&(_, v)| v > crate::tuning::AFFINITY_NEUTRAL);
        let (axis, _) = raised.next()?;
        if raised.next().is_some() {
            return None;
        }
        Some(AffinityClass::of_axis(axis))
    }
}

impl AffinityClass {
    /// The class an affinity axis names. Public because the kit census asks
    /// it of an *ability's* category rather than of a species' affinities —
    /// "is this routine the kind of thing this class does" is the same
    /// mapping asked from the other end, and two copies of it could drift
    /// into checking a kit against one class and the stats against another.
    pub fn of_axis(kind: AffinityKind) -> AffinityClass {
        match kind {
            AffinityKind::Damage => AffinityClass::Striker,
            AffinityKind::Buff => AffinityClass::Bastion,
            AffinityKind::Heal => AffinityClass::Medic,
            AffinityKind::Debuff => AffinityClass::Saboteur,
            AffinityKind::Drain => AffinityClass::Leech,
        }
    }
}

fn default_growth_multiplier() -> f32 {
    crate::tuning::BASELINE_GROWTH_MULTIPLIER
}

fn default_base_speed() -> i32 {
    crate::tuning::DEFAULT_BASE_SPEED
}

fn default_base_int() -> i32 {
    crate::tuning::DEFAULT_BASE_INT
}

#[derive(Resource, Default)]
pub struct SpeciesDb {
    species: HashMap<SpeciesId, SpeciesDef>,
}

impl SpeciesDb {
    /// Loads every `*.ron` species definition in `dir`. Malformed files are
    /// skipped (with a returned warning) rather than aborting the whole
    /// load — a single bad custom/mod file shouldn't be able to crash
    /// startup for everything else.
    ///
    /// An entry in `abilities` naming an ability `db` doesn't know is
    /// dropped with a warning rather than taking the species down with it —
    /// a program missing one of its abilities is still perfectly playable,
    /// unlike a research node whose prerequisite doesn't exist.
    pub fn load_dir(
        dir: &Path,
        abilities: &crate::abilities::AbilityDb,
    ) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = SpeciesDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<SpeciesDef>(&text) {
                Ok(mut def) => {
                    let id = def.id.clone();
                    if let Some(field) = def.affinities.non_finite_field() {
                        warnings.push(format!(
                            "skipped invalid species file {path:?}: \
                             affinities.{field} is not a finite number"
                        ));
                        continue;
                    }
                    def.affinities.clamp_all();
                    def.abilities.retain(|a| {
                        let known = abilities.get(&a.id).is_some();
                        if !known {
                            warnings.push(format!(
                                "species {id:?}: unknown ability {:?} — dropped",
                                a.id
                            ));
                        }
                        known
                    });
                    db.species.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid species file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&SpeciesDef> {
        self.species.get(id)
    }

    /// Adds `def`, replacing any species already under its id.
    ///
    /// Test-only, and deliberately so: the shipped roster is a directory of
    /// `.ron` files and a mod's is too, so nothing in the game builds a
    /// species in Rust. It exists for `tests::support::generic_species`, the
    /// blank fixture companion, which has to be a species the roster does
    /// not contain.
    #[cfg(test)]
    pub(crate) fn insert(&mut self, def: SpeciesDef) {
        self.species.insert(def.id.clone(), def);
    }

    /// Ordinary (non-boss) species that can inhabit `biome` — the pool the
    /// normal per-tile spawn roll draws from. Sorted by `id` for the same
    /// reason `all` is, and more urgently: the spawn roll picks out of this
    /// pool *by index*, so an unsorted `HashMap` order makes two
    /// `Game::new(seed)` runs spawn different species and diverge.
    pub fn habitat_matches(&self, biome: Biome) -> Vec<&SpeciesDef> {
        self.sorted_matches(|s| !s.is_boss && s.habitats.contains(&biome))
    }

    /// Boss species that can inhabit `biome` — a separate, much rarer pool
    /// than `habitat_matches`, sorted for the same reason.
    pub fn boss_habitat_matches(&self, biome: Biome) -> Vec<&SpeciesDef> {
        self.sorted_matches(|s| s.is_boss && s.habitats.contains(&biome))
    }

    fn sorted_matches(&self, keep: impl Fn(&SpeciesDef) -> bool) -> Vec<&SpeciesDef> {
        let mut matches: Vec<&SpeciesDef> = self.species.values().filter(|s| keep(s)).collect();
        matches.sort_by(|a, b| a.id.cmp(&b.id));
        matches
    }

    /// Every loaded species, sorted by `id` for a stable, reproducible
    /// order — see `structures::StructureDb::all` for why: `HashMap`
    /// iteration order is randomized per-instance, so without this sort
    /// this list's order would shuffle unpredictably between sessions.
    pub fn all(&self) -> impl Iterator<Item = &SpeciesDef> {
        let mut defs: Vec<&SpeciesDef> = self.species.values().collect();
        defs.sort_by(|a, b| a.id.cmp(&b.id));
        defs.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::AFFINITY_NEUTRAL;
    use std::path::Path;

    fn species_assets_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/species")
    }

    /// The shipped ability set, which `load_dir` validates species kits
    /// against.
    fn shipped_abilities() -> crate::abilities::AbilityDb {
        crate::abilities::AbilityDb::load_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/abilities"),
        )
        .unwrap()
        .0
    }

    /// A class is readable at runtime, not only from a census. Phase 5's
    /// base jobs ask what a posted program *is* while the game is running,
    /// which nothing outside `#[cfg(test)]` could answer before.
    ///
    /// The five arms are spot-checked against real files rather than
    /// hand-built defs, because the mapping is only worth anything if it
    /// agrees with the roster the player actually meets.
    #[test]
    fn a_class_is_the_one_affinity_axis_a_species_raises() {
        let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        for (id, expected) in [
            ("glitch", AffinityClass::Striker),
            ("sentinel", AffinityClass::Bastion),
            ("virus", AffinityClass::Medic),
            ("trojan", AffinityClass::Saboteur),
            ("drone", AffinityClass::Leech),
        ] {
            assert_eq!(
                db.get(id).unwrap().affinity_class(),
                Some(expected),
                "{id} raises the axis that names {expected:?}"
            );
        }
    }

    /// A boss raises nothing, so it is outside the class system rather than
    /// being some default class — which is what keeps a base job from
    /// silently attaching to one, and what `derive`'s `Option` is for.
    #[test]
    fn a_boss_has_no_class() {
        let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        assert_eq!(db.get("overseer").unwrap().affinity_class(), None);
    }

    /// The habitat pools feed the per-tile spawn roll by index
    /// (`Game::try_spawn_habitat_creature`), and the species it lands on
    /// decides whether a further `can_nest` draw happens. Backed by a
    /// `HashMap`, whose iteration order is randomized per instance, two
    /// `Game::new(seed)` runs therefore pick different species, take a
    /// different number of RNG draws, and diverge — the same hazard
    /// `SpeciesDb::all` is sorted to avoid.
    #[test]
    fn habitat_pools_are_ordered_stably_across_instances() {
        let ids =
            |defs: Vec<&SpeciesDef>| -> Vec<String> { defs.iter().map(|s| s.id.clone()).collect() };
        let (a, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        let (b, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();

        for biome in [
            Biome::OpenGrid,
            Biome::Mainframe,
            Biome::NullSector,
            Biome::StaticField,
        ] {
            let pool = ids(a.habitat_matches(biome));
            let mut sorted = pool.clone();
            sorted.sort();
            assert_eq!(pool, sorted, "{biome:?} habitat pool must be sorted by id");
            assert_eq!(
                pool,
                ids(b.habitat_matches(biome)),
                "{biome:?} habitat pool must not vary between db instances"
            );

            let bosses = ids(a.boss_habitat_matches(biome));
            let mut sorted_bosses = bosses.clone();
            sorted_bosses.sort();
            assert_eq!(bosses, sorted_bosses, "{biome:?} boss pool must be sorted");
            assert_eq!(bosses, ids(b.boss_habitat_matches(biome)));
        }
    }

    /// `base_speed` must be `#[serde(default)]` — a mod author's existing
    /// species file predating the field has to keep loading untouched. That
    /// is the modding contract, not a nicety.
    #[test]
    fn base_speed_defaults_when_a_species_file_omits_it() {
        let def: SpeciesDef = ron::from_str(
            r#"(
                id: "testmon",
                name: "Testmon",
                glyph: 't',
                color: Green,
                base_hp: 10,
                base_atk: 1,
                base_def: 1,
                taming_difficulty: 0.5,
                habitats: [OpenGrid],
                moves: [(name: "Poke", power: 1)],
                work_resource: None,
            )"#,
        )
        .expect("a species file with no base_speed must still parse");
        assert_eq!(def.base_speed, crate::tuning::DEFAULT_BASE_SPEED);
    }

    /// Same modding contract as `base_speed` above, and one step stronger:
    /// the baseline is not merely *a* value a mod lands on, it is the value
    /// at which `mining_success_chance`'s deviation term is zero. A species
    /// file predating this field therefore extracts at exactly the rate it
    /// did before the field existed.
    #[test]
    fn base_int_defaults_when_a_species_file_omits_it() {
        let omitted: SpeciesDef = ron::from_str(
            r#"(
                id: "testmon",
                name: "Testmon",
                glyph: 't',
                color: Green,
                base_hp: 10,
                base_atk: 1,
                base_def: 1,
                taming_difficulty: 0.5,
                habitats: [OpenGrid],
                moves: [(name: "Poke", power: 1)],
                work_resource: None,
            )"#,
        )
        .expect("a species file with no base_int must still parse");
        assert_eq!(omitted.base_int, crate::tuning::DEFAULT_BASE_INT);

        let declared: SpeciesDef = ron::from_str(
            r#"(
                id: "testmon",
                name: "Testmon",
                glyph: 't',
                color: Green,
                base_hp: 10,
                base_atk: 1,
                base_def: 1,
                taming_difficulty: 0.5,
                habitats: [OpenGrid],
                base_int: 14,
                moves: [(name: "Poke", power: 1)],
                work_resource: None,
            )"#,
        )
        .expect("a species file declaring base_int must parse");
        assert_eq!(declared.base_int, 14);
    }

    #[test]
    fn ranged_defaults_to_melee_when_a_move_omits_it() {
        let def: MoveDef = ron::from_str(r#"(name: "Poke", power: 3)"#)
            .expect("a move with no `ranged` field must still parse");
        assert!(!def.ranged, "moves default to melee");
    }

    /// The three melee-only bruisers are load-bearing: the reach tests in
    /// `lib.rs` use them as back groups that genuinely cannot connect. If
    /// someone gives one of them a ranged move, those tests would start
    /// passing for the wrong reason.
    #[test]
    fn the_melee_only_species_stay_melee_only_and_everything_else_has_reach() {
        let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        for id in ["scrapper", "sentinel", "construct"] {
            let species = db.get(id).unwrap();
            assert!(
                species.moves.iter().all(|m| !m.ranged),
                "{id} is a melee-only bruiser by design"
            );
        }
        for species in db.all() {
            assert!(
                species.moves.iter().any(|m| !m.ranged),
                "{} must keep at least one melee move, or its front-rank \
                 behaviour changes too",
                species.id
            );
        }
    }

    #[test]
    fn shipped_species_speeds_span_a_meaningful_range() {
        let (db, warnings) =
            SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should load cleanly: {warnings:?}"
        );
        // A Construct is a wall and a Sprite is a spark; if those two ever
        // collapse to the same number, initiative has stopped meaning anything.
        assert!(db.get("sprite").unwrap().base_speed > db.get("construct").unwrap().base_speed);
    }

    #[test]
    fn no_shipped_species_lives_on_the_platform_biome() {
        let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        assert!(
            db.habitat_matches(Biome::Platform).is_empty(),
            "a base platform must have no ordinary habitat species — an empty candidate \
             pool is the entire mechanism behind a base being free of wild spawns"
        );
        assert!(
            db.boss_habitat_matches(Biome::Platform).is_empty(),
            "a base platform must have no boss species either"
        );
    }

    #[test]
    fn habitat_matches_excludes_bosses_and_boss_habitat_matches_includes_only_them() {
        let (db, warnings) =
            SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should all load cleanly: {warnings:?}"
        );

        let normal = db.habitat_matches(Biome::OpenGrid);
        assert!(
            !normal.is_empty(),
            "OpenGrid should have ordinary habitat species"
        );
        assert!(
            normal.iter().all(|s| !s.is_boss),
            "habitat_matches should never include a boss species"
        );

        let bosses = db.boss_habitat_matches(Biome::OpenGrid);
        assert!(
            !bosses.is_empty(),
            "at least one boss species should inhabit OpenGrid"
        );
        assert!(
            bosses.iter().all(|s| s.is_boss),
            "boss_habitat_matches should only ever include boss species"
        );
    }

    /// A Stack lair boss is the only thing in the game that pays the
    /// breaching currency (`STACK_BOSS_PORTAL_FRAGMENT_DROP`), and
    /// `Game::pick_lair_species` draws it from the biome of the **surface
    /// tile the link opens in**. A walkable biome with no boss defined for
    /// it therefore falls back to the toughest ordinary program in the
    /// biome — which is a real fight, but `is_boss` is false, so it pays
    /// nothing and every stack reached through that terrain is a dead end
    /// for progression.
    ///
    /// Only walkable biomes are checked because `Game::spawn_surface_links`
    /// refuses an unwalkable tile, so a link can never open in Data Void or
    /// Black Ice. `Biome::Platform` is excluded for the opposite reason: it
    /// is base floor with no species at all, which is the whole mechanism
    /// behind a base being a safe haven.
    ///
    /// A census, not a rule — this breaks by *removing* a habitat from the
    /// last boss that covers some terrain, which reads as a small tuning
    /// edit right up until a run cannot leave zone 1.
    #[test]
    fn every_biome_a_stack_link_can_open_in_fields_a_boss() {
        let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();

        for biome in [
            Biome::DataVoid,
            Biome::StaticField,
            Biome::NullSector,
            Biome::Mainframe,
            Biome::OpenGrid,
            Biome::BlackIce,
        ] {
            if !biome.walkable() {
                continue;
            }
            assert!(
                !db.boss_habitat_matches(biome).is_empty(),
                "{biome:?} is walkable, so a Stack link can open in it — but no shipped \
                 boss inhabits it, which would make every stack under that terrain pay \
                 no portal fragments at all"
            );
        }
    }

    /// The five classes, as the tuple every non-boss species is checked
    /// against: `(up axis, down axis, budget weight, HP share, ATK share,
    /// DEF share, slowest speed, fastest speed)`. Every share is a
    /// **percent**, and the arithmetic below is integer, because
    /// `50.0f32 * 1.05` is 52.4999… — a census that a species passes or
    /// fails on the binary representation of a tuning fraction is a census
    /// nobody can reason about.
    ///
    /// The **up axis alone** names the class. The down axis is a
    /// consistency check and cannot name one on its own — Bastion and
    /// Medic both damp `damage`.
    /// (class, damped axis, stat weight %, HP/ATK/DEF shares %, speed band)
    type ClassRow = (AffinityClass, AffinityKind, i32, i32, i32, i32, i32, i32);

    #[rustfmt::skip]
    const CLASSES: [ClassRow; 5] = [
        (AffinityClass::Striker,  AffinityKind::Heal,    90, 84, 13, 3, 10, 11),
        (AffinityClass::Saboteur, AffinityKind::Heal,    95, 85, 11, 4, 13, 14),
        (AffinityClass::Medic,    AffinityKind::Damage, 100, 87,  7, 6, 12, 12),
        (AffinityClass::Leech,    AffinityKind::Buff,   105, 90,  8, 2,  8,  9),
        (AffinityClass::Bastion,  AffinityKind::Damage, 110, 88,  4, 8,  6,  7),
    ];

    /// `percent` of `value`, rounded half up.
    fn pct(value: i32, percent: i32) -> i32 {
        (value * percent + 50) / 100
    }

    /// The `CLASSES` row a non-boss species is read as, looked up from
    /// `SpeciesDef::affinity_class` and cross-checked against the axis it
    /// damps.
    ///
    /// Both censuses go through here rather than deriving the class each for
    /// itself: the class is what they disagree about most cheaply, and a
    /// second copy of "raises exactly one axis" is a copy that can drift
    /// into checking the stats against one class and the kit against
    /// another. The derivation itself is no longer one of those copies — it
    /// is the shipping function the base jobs read, so a census passing is
    /// evidence about the game rather than about the test.
    fn class_of(species: &SpeciesDef) -> ClassRow {
        let id = &species.id;
        let class = species.affinity_class().unwrap_or_else(|| {
            panic!(
                "{id} raises no single affinity axis — exactly one is what names its class, \
                 and a species with none or two has no readable role"
            )
        });
        let damped: Vec<AffinityKind> = [
            AffinityKind::Damage,
            AffinityKind::Heal,
            AffinityKind::Buff,
            AffinityKind::Debuff,
            AffinityKind::Drain,
        ]
        .into_iter()
        .filter(|k| species.affinities.get(*k) < AFFINITY_NEUTRAL)
        .collect();
        assert_eq!(
            damped.len(),
            1,
            "{id} damps {} affinity axes; a class is one thing it is good at and \
             one it is bad at",
            damped.len()
        );

        let row = CLASSES.into_iter().find(|c| c.0 == class).unwrap();
        assert_eq!(
            damped[0], row.1,
            "{id} is a {:?} class, which damps {:?}, but it damps {:?}",
            row.0, row.1, damped[0]
        );
        row
    }

    /// Total HP+ATK+DEF a species of each growth band is built from, before
    /// its class weight. Taken from the bands' own means at the time the
    /// classes landed, so the ladder itself did not move.
    fn tier_budget(growth: f32) -> i32 {
        match growth {
            g if g < 1.125 => 50,
            g if g < 1.375 => 105,
            _ => 140,
        }
    }

    /// A class is three things that must agree — the affinity axis, the
    /// stat shape and (from phase 4b) the kit. This is the agreement
    /// between the first two, and it is a cross-check rather than a
    /// restatement of the files: the class is **derived from the
    /// affinities** and the stats are checked against what that class
    /// implies, so editing a stat block without meaning to change the
    /// species' role fails here.
    ///
    /// The mechanism that makes role independent of tier is the split
    /// between the two numbers. The *tier* sets the budget; the *class*
    /// sets both how much of that budget the species gets (a wall carries
    /// more stuff than a glass cannon, by the same factor at every tier)
    /// and how it is spent. So "low DEF for its size" is readable at tier 1
    /// and tier 3 alike, which raw totals never were — a tier-3 striker
    /// out-tanks a tier-1 tank on absolute HP.
    ///
    /// Bosses are excluded: they are outside the class system entirely and
    /// have no affinities at all, which is what `derive` refusing a neutral
    /// species would otherwise make an error.
    #[test]
    fn every_ordinary_species_stat_shape_agrees_with_its_affinity_class() {
        let (db, warnings) =
            SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should all load cleanly: {warnings:?}"
        );

        for species in db.all().filter(|s| !s.is_boss) {
            let id = &species.id;
            let (up, _down, weight, hp_share, atk_share, def_share, slowest, fastest) =
                class_of(species);

            let total = species.base_hp + species.base_atk + species.base_def;
            let budget = tier_budget(species.growth_multiplier);
            let expected = pct(budget, weight);
            assert_eq!(
                total, expected,
                "{id} spends {total} stat points; its growth band's budget is {budget} \
                 and a {up:?} class carries {weight}% of that, so it should spend {expected}"
            );

            for (name, actual, share) in [
                ("HP", species.base_hp, hp_share),
                ("ATK", species.base_atk, atk_share),
                ("DEF", species.base_def, def_share),
            ] {
                let target = pct(total, share);
                assert!(
                    (actual - target).abs() <= 1,
                    "{id} puts {actual} of its {total} points into {name}; a {up:?} class \
                     spends {share}% there, which is {target}. One point of slack is allowed \
                     for the residue that makes the three sum to the budget, no more"
                );
            }

            assert!(
                (slowest..=fastest).contains(&species.base_speed),
                "{id} is a {up:?} class, which is a {slowest}-{fastest} speed, but it is \
                 {}. Speed is the fourth shape axis and it is not only initiative — it \
                 paces the machine a program is posted to (see Game::work_ticks_for)",
                species.base_speed
            );
        }
    }

    /// The authored magnitude of a kit entry, which is what the ladder is
    /// ordered by. Every category a kit may hold carries one; the panic arm
    /// is the reachable-by-editing case of a `Cleanse` or a field buff
    /// reaching the ladder slot, which the checks below refuse first.
    fn kit_power(effect: &crate::abilities::AbilityEffect) -> i32 {
        use crate::abilities::AbilityEffect as E;
        match effect {
            E::Damage { power, .. }
            | E::Heal { power }
            | E::Buff { power, .. }
            | E::Debuff { power, .. }
            | E::Drain { power, .. } => *power,
            other => panic!("{other:?} carries no magnitude to rank a tier ladder by"),
        }
    }

    /// The third leg of a class, and the one a player actually spends a
    /// round on. The stat shape says what a species *is*; the kit says what
    /// it *does*, and the two agreeing is the whole of the design — a
    /// Bastion whose kit heals is a Medic that has been mis-shaped.
    ///
    /// Every species carries two entries: a **class utility** shared
    /// verbatim by all three members of its class, and a **tier rung** it
    /// holds alone. So the class is legible from the first unlock and the
    /// tier from the second, which mirrors the stat budget being set by the
    /// tier and spent by the class.
    ///
    /// Three of the checks are about a trap rather than about taste:
    ///
    /// - **Nothing unlocks at level 1.** `abilities::FALLBACK_ABILITY_ID`
    ///   fills an *empty* kit (`Game::install_innate_routines`), and every
    ///   species granting something at level 1 would make it unreachable —
    ///   it is obtainable no other way than by extraction from a companion
    ///   holding it. Holding the first entry back to level 2 is also what
    ///   makes a fresh capture read as generic before it reads as a class.
    /// - **No field-only effect.** `AffinityKind` is blind to the
    ///   distinction — a `FieldBuff(kind: Def)` reports `Buff` and would
    ///   pass the category check while never appearing in the battle
    ///   Special picker at all, which is the one place a kit is spent.
    /// - **The rungs rank by their authored power**, so a rung assigned to
    ///   the wrong tier fails here rather than shipping. Checked against
    ///   the growth band rather than against a table of ids, which would
    ///   just be the species files copied into a test.
    #[test]
    fn every_ordinary_species_kit_agrees_with_its_affinity_class() {
        use crate::abilities::AbilityEffect;

        let abilities = shipped_abilities();
        let (db, warnings) = SpeciesDb::load_dir(&species_assets_dir(), &abilities).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should all load cleanly: {warnings:?}"
        );

        // (class, growth band, species id, utility id, rung id, rung power)
        let mut kits: Vec<(AffinityClass, f32, String, String, String, i32)> = Vec::new();

        for species in db.all().filter(|s| !s.is_boss) {
            let id = &species.id;
            let (class, ..) = class_of(species);

            assert_eq!(
                species.abilities.len(),
                2,
                "{id} grants {} abilities; every ordinary species carries a class \
                 utility and a tier rung, and nothing else",
                species.abilities.len()
            );

            for entry in &species.abilities {
                let def = abilities.get(&entry.id).unwrap_or_else(|| {
                    panic!("{id} grants {:?}, which no ability file defines", entry.id)
                });
                assert!(
                    entry.level >= 2,
                    "{id} unlocks {:?} at level {}; the first kit entry sits at level 2 \
                     so a level-1 capture still installs {}, which nothing else grants",
                    entry.id,
                    entry.level,
                    crate::abilities::FALLBACK_ABILITY_ID
                );
                assert!(
                    !def.effect.field_only(),
                    "{id} grants {:?}, which is field-only — a kit is spent through the \
                     battle Special picker, where a field routine never appears",
                    entry.id
                );
                let categorised = match (&def.effect, class) {
                    // The one effect with no affinity category that still
                    // belongs to a class: undoing a status is what a Medic
                    // is for, and there is nothing there for an affinity to
                    // scale.
                    (AbilityEffect::Cleanse, AffinityClass::Medic) => true,
                    _ => def.effect.affinity_kind().map(AffinityClass::of_axis) == Some(class),
                };
                assert!(
                    categorised,
                    "{id} is a {class:?} class but grants {:?}, which is {:?}. The kit is \
                     the leg of a class a player actually spends a round on",
                    entry.id,
                    def.effect.affinity_kind()
                );
            }

            let mut entries = species.abilities.clone();
            entries.sort_by_key(|a| a.level);
            assert!(
                entries[0].level < entries[1].level,
                "{id} unlocks both kit entries at level {}; the utility comes first and \
                 the tier rung second",
                entries[0].level
            );
            let rung = abilities.get(&entries[1].id).unwrap();
            kits.push((
                class,
                species.growth_multiplier,
                id.clone(),
                entries[0].id.clone(),
                entries[1].id.clone(),
                kit_power(&rung.effect),
            ));
        }

        for (class, ..) in CLASSES {
            let mut members: Vec<_> = kits.iter().filter(|k| k.0 == class).collect();
            assert_eq!(
                members.len(),
                3,
                "{class:?} has {} species; one per growth band is what makes a class \
                 readable independently of the ladder",
                members.len()
            );

            let utility = &members[0].3;
            for m in &members {
                assert_eq!(
                    &m.3, utility,
                    "{} opens with {:?} but its {class:?} classmates open with {utility:?}; \
                     the first unlock is the shared tell that names the class",
                    m.2, m.3
                );
            }

            members.sort_by(|a, b| a.1.total_cmp(&b.1));
            for pair in members.windows(2) {
                let (lo, hi) = (pair[0], pair[1]);
                assert_ne!(
                    lo.4, hi.4,
                    "{} and {} share the tier rung {:?}; the second unlock is what \
                     separates the members of a class",
                    lo.2, hi.2, lo.4
                );
                assert!(
                    lo.5 < hi.5,
                    "{} ({:.2} growth) holds {:?} at power {}, and {} ({:.2}) holds {:?} \
                     at power {} — a {class:?}'s rung climbs with its growth band",
                    lo.2,
                    lo.1,
                    lo.4,
                    lo.5,
                    hi.2,
                    hi.1,
                    hi.4,
                    hi.5
                );
            }
        }
    }

    #[test]
    fn base_roster_growth_multiplier_rises_with_difficulty_tier() {
        let (db, warnings) =
            SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should all load cleanly: {warnings:?}"
        );

        let get = |id: &str| db.get(id).unwrap().growth_multiplier;
        // Easy-tier species (and SubProcess, Easy/Medium) omit the field
        // entirely in their .ron file and should fall back to the default.
        assert_eq!(get("sprite"), default_growth_multiplier());
        assert_eq!(get("glitch"), default_growth_multiplier());
        assert_eq!(get("sub_process"), default_growth_multiplier());
        // Medium tier
        assert_eq!(get("scrapper"), 1.25);
        // Hard tier
        assert_eq!(get("virus"), 1.5);
        // Boss tier — the fastest-growing companions in the base roster.
        assert_eq!(get("overseer"), 2.0);
        assert_eq!(get("wintermute"), 2.0);
    }

    /// The point of `base_int` is a species axis that is **not** the ladder
    /// wearing another name. Growth multiplier is the ladder, so the property
    /// asserted here is that aptitude cuts across it.
    ///
    /// Deliberately not "INT is uncorrelated with tier": a correlation
    /// threshold is fragile to a one-point retune and would fail for reasons
    /// nobody could read. These two are the things a *player* could notice —
    /// that every rung has both a sharp and a dull program on it, and that
    /// climbing the ladder is not how you get the best extractor.
    ///
    /// Bosses are excluded because they can never be posted to a node, so
    /// their values are flavour and must not be able to carry this either
    /// way.
    #[test]
    fn extraction_aptitude_cuts_across_the_difficulty_ladder() {
        let (db, warnings) =
            SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should all load cleanly: {warnings:?}"
        );

        let ordinary: Vec<&SpeciesDef> = db.all().filter(|s| !s.is_boss).collect();
        let mean = ordinary.iter().map(|s| s.base_int).sum::<i32>() as f64 / ordinary.len() as f64;

        let mut bands: std::collections::BTreeMap<String, Vec<&SpeciesDef>> =
            std::collections::BTreeMap::new();
        for s in &ordinary {
            bands
                .entry(format!("{:.2}", s.growth_multiplier))
                .or_default()
                .push(s);
        }
        assert!(
            bands.len() >= 3,
            "the ladder should have at least three rungs to cut across, found {}",
            bands.len()
        );

        for (band, members) in &bands {
            assert!(
                members.iter().any(|s| (s.base_int as f64) > mean),
                "growth band {band} has no species above the roster's mean aptitude \
                 ({mean:.1}), so on that rung aptitude is just the ladder again"
            );
            assert!(
                members.iter().any(|s| (s.base_int as f64) < mean),
                "growth band {band} has no species below the roster's mean aptitude \
                 ({mean:.1}), so on that rung aptitude is just the ladder again"
            );
        }

        // Phrased as two maxima rather than "the argmax isn't on the top
        // rung", which would be decided by how `max_by_key` happens to break
        // a tie — a property of the iterator, not of the roster.
        let steepest = ordinary
            .iter()
            .map(|s| s.growth_multiplier)
            .fold(f32::MIN, f32::max);
        let sharpest_overall = ordinary.iter().map(|s| s.base_int).max().unwrap();
        let sharpest_on_top_rung = ordinary
            .iter()
            .filter(|s| s.growth_multiplier == steepest)
            .map(|s| s.base_int)
            .max()
            .expect("the steepest band has members");
        assert!(
            sharpest_on_top_rung < sharpest_overall,
            "the best extractor in the game ({sharpest_overall}) is available on the \
             steepest growth band, which is how 'best extractor' collapses back into \
             'furthest up the ladder'"
        );
    }

    #[test]
    fn can_nest_is_set_only_for_the_intended_swarm_flavored_species() {
        let (db, warnings) =
            SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
        assert!(
            warnings.is_empty(),
            "species assets should all load cleanly: {warnings:?}"
        );

        let nesting: Vec<&str> = db
            .all()
            .filter(|s| s.can_nest)
            .map(|s| s.id.as_str())
            .collect();
        let mut nesting = nesting;
        nesting.sort();
        assert_eq!(nesting, vec!["crawler", "scrapper", "trojan", "worm"]);

        // No boss should ever be nest-eligible, regardless of this flag —
        // try_spawn_habitat_creature only rolls a nest for the non-boss
        // pick, but the data itself shouldn't set can_nest on a boss
        // either, to avoid a misleading .ron file.
        assert!(
            db.all().all(|s| !(s.is_boss && s.can_nest)),
            "no boss species should have can_nest set"
        );
    }
}
