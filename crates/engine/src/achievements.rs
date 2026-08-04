//! The cross-run achievement profile: what can be earned, and the roll that
//! decides what a stat reward turns into.
//!
//! Achievements are data, like species and items — `assets/achievements/*.ron`,
//! one file per rung, so adding one is a file drop. What is *not* data is the
//! ceiling on how much the whole ladder may pay out, which lives in
//! `tuning::MAX_PROFILE_*` beside every other difficulty knob.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// An achievement's id, e.g. `breach_zone_2`. A string newtype rather than
/// an enum for the same reason `items::ItemId` is one: a rung is a file
/// drop, and an enum would make it a code change.
///
/// `#[serde(transparent)]` so it spells as a bare quoted string in both the
/// asset files and `profile.ron`. `Ord` so `AchievementDb` can key a
/// `BTreeMap` by it — the achievements screen lists that iteration order,
/// and a `HashMap` would reshuffle the screen between runs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AchievementId(pub String);

impl AchievementId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for AchievementId {
    fn from(s: &str) -> Self {
        AchievementId(s.to_string())
    }
}

impl From<String> for AchievementId {
    fn from(s: String) -> Self {
        AchievementId(s)
    }
}

impl std::fmt::Display for AchievementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What has to happen for a rung to be earned.
///
/// Three of the four are high-water marks on a monotone counter, which is why
/// `game::achievements::achievement_system` can evaluate them by polling with
/// no call sites at all. `BossDefeated` is the odd one: it is event-shaped, so
/// `award_loot` records the kill into `resources::RunFeats` and the system
/// still decides what that earned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trigger {
    /// The run has breached to this zone or deeper.
    ZoneReached(u32),
    /// The party has stood in a Stack frame at this depth or deeper. Read off
    /// `resources::Locale`, never `Position` — `Position` is pinned to the
    /// surface entrance tile while the party is underground.
    StackDepthReached(u32),
    /// `GameClock.tick` has reached this many cycles.
    CyclesSurvived(u64),
    /// A boss died this run. `None` means any boss; `Some(species_id)` names
    /// one. Fleeing earns nothing — the record is written at the one point
    /// that knows the boss actually died.
    BossDefeated(Option<String>),
}

/// What a rung pays, once, at the start of the next run. Exactly one per
/// achievement, which is what makes the total power bounded by a finite
/// authored list and therefore assertable — see `tuning::MAX_PROFILE_*`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reward {
    /// `n` points into one main stat, chosen by `roll_main_stat` at earn time
    /// and then stored in the profile entry so it never rerolls.
    RandomMainStat(u32),
    /// `n` added to the player's Perk Point pool, spent through the ordinary
    /// `perks.rs` machinery.
    PerkPoints(u32),
    /// The next run begins with this species tamed and owned. Whether the id
    /// names a real species is checked where `SpeciesDb` is in hand, not here.
    StartingProgram(String),
}

/// The four axes `Reward::RandomMainStat` can land on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MainStat {
    Atk,
    Def,
    /// `Stats::max_hp`, and `hp` with it — a run must not start damaged.
    Integrity,
    Decompiler,
}

impl MainStat {
    /// In the order `roll_main_stat` indexes. Appending here changes what
    /// every existing id rolls, which would contradict an already-written
    /// `profile.ron`; the four axes are the player's four axes, so this is
    /// not expected to grow.
    pub fn all() -> [MainStat; 4] {
        [
            MainStat::Atk,
            MainStat::Def,
            MainStat::Integrity,
            MainStat::Decompiler,
        ]
    }

    /// How the stat reads on the achievements screen.
    pub fn label(self) -> &'static str {
        match self {
            MainStat::Atk => "Attack",
            MainStat::Def => "Defense",
            MainStat::Integrity => "Integrity",
            MainStat::Decompiler => "Decompiler",
        }
    }
}

/// One authored rung.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AchievementDef {
    pub id: AchievementId,
    pub name: String,
    /// Player-facing, shown on the achievements screen. Authored rather than
    /// derived from the trigger, so a modder controls how their rung reads.
    pub description: String,
    pub trigger: Trigger,
    pub reward: Reward,
}

#[derive(Resource, Default)]
pub struct AchievementDb {
    defs: BTreeMap<AchievementId, AchievementDef>,
}

impl AchievementDb {
    /// Loads every `*.ron` achievement in `dir`. A malformed file is skipped
    /// with a returned warning rather than aborting the load, same as
    /// `PerkDb::load_dir`.
    ///
    /// Rejected with a warning, as well as anything `ron` refuses: an empty
    /// id, a duplicate id, a reward paying zero, and a `StartingProgram`
    /// naming nothing. Whether that species *exists* is not checked here —
    /// `SpeciesDb` is not in hand, and `Game::grant_profile_rewards` is where
    /// the cross-db check belongs.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = AchievementDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<AchievementDef>(&text) {
                Ok(def) => match reward_complaint(&def) {
                    Some(why) => {
                        warnings.push(format!("skipped invalid achievement file {path:?}: {why}"))
                    }
                    None if db.defs.contains_key(&def.id) => warnings.push(format!(
                        "skipped invalid achievement file {path:?}: id {} is already taken",
                        def.id
                    )),
                    None => {
                        db.defs.insert(def.id.clone(), def);
                    }
                },
                Err(e) => warnings.push(format!("skipped invalid achievement file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &AchievementId) -> Option<&AchievementDef> {
        self.defs.get(id)
    }

    /// Every authored rung, in id order. The achievements screen lists this
    /// directly, so the order has to be stable between runs.
    pub fn iter(&self) -> impl Iterator<Item = &AchievementDef> {
        self.defs.values()
    }
}

/// Why `def` cannot be loaded, or `None` if it is fine. A rung that pays
/// nothing is refused for the same reason `PerkDb` refuses a `cost: 0` perk:
/// it is a mistake that reads as a working file.
fn reward_complaint(def: &AchievementDef) -> Option<String> {
    if def.id.as_str().is_empty() {
        return Some("an achievement needs an id".to_string());
    }
    match &def.reward {
        Reward::RandomMainStat(0) | Reward::PerkPoints(0) => {
            Some("a reward of 0 pays nothing; give it at least 1 or delete the file".to_string())
        }
        Reward::StartingProgram(s) if s.is_empty() => {
            Some("StartingProgram needs a species id".to_string())
        }
        _ => None,
    }
}

/// One rung, earned. What the profile remembers about it beyond the fact
/// itself: when it first happened, whether it has ever been done on
/// permadeath, and — for a `Reward::RandomMainStat` — which stat the roll
/// landed on, so it is decided once and never rerolls.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Earned {
    pub id: AchievementId,
    /// `GameClock.tick` at the moment of the first earn. Not updated by a
    /// re-earn — the record is of the first time.
    pub first_tick: u64,
    /// Whether this has ever been earned on permadeath. Upgrades to `true`
    /// on a later permadeath re-earn and never downgrades; `Profile::record`
    /// is the only thing that may write it.
    #[serde(default)]
    pub permadeath: bool,
    #[serde(default)]
    pub rolled_stat: Option<MainStat>,
}

/// Everything earned across every run, and the one thing in this game that
/// outlives a run.
///
/// Deliberately **not** part of `SaveData`: a save is one run, this spans
/// them, and keeping it out means no `SAVE_FORMAT_VERSION` bump — here or for
/// any field added later, since `#[serde(default)]` keeps an old
/// `profile.ron` parsing the way the `.ron` assets do and bincode cannot.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub earned: Vec<Earned>,
}

impl Profile {
    pub fn contains(&self, id: &AchievementId) -> bool {
        self.earned.iter().any(|e| &e.id == id)
    }

    /// Records a first earn, returning `true`. On a re-earn it returns
    /// `false` and changes nothing except to upgrade `permadeath` to true if
    /// this earn was on permadeath — the difficulty rule lives here and
    /// nowhere else.
    pub fn record(&mut self, entry: Earned) -> bool {
        if let Some(existing) = self.earned.iter_mut().find(|e| e.id == entry.id) {
            existing.permadeath |= entry.permadeath;
            return false;
        }
        self.earned.push(entry);
        true
    }

    /// Reads `path`, with whatever went wrong alongside it. Never `Err` and
    /// never a panic: an unreadable profile must not stop the game starting,
    /// and the worst it can cost is the profile.
    ///
    /// The warning comes back rather than being swallowed because the caller
    /// owns the log — but it is not optional to *ask* for it, which is why
    /// this returns a pair instead of offering a quieter overload. An absent
    /// file is not a warning: a first run has no profile.
    pub fn load(path: &Path) -> (Self, Option<String>) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Self::default(), None),
            Err(e) => {
                return (
                    Self::default(),
                    Some(format!(
                        "could not read {path:?}: {e}; starting from an empty profile"
                    )),
                );
            }
        };
        match ron::from_str::<Profile>(&text) {
            Ok(profile) => (profile, None),
            Err(e) => (
                Self::default(),
                Some(format!(
                    "could not parse {path:?}: {e}; starting from an empty profile. The file has \
                     not been overwritten — fix it or delete it."
                )),
            ),
        }
    }

    /// Writes the profile out as readable, hand-editable RON, struct names
    /// and all. Callers write on every earn rather than at run end: a
    /// permadeath run that ends badly must not lose what it proved.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let config = ron::ser::PrettyConfig::default().struct_names(true);
        let text = ron::ser::to_string_pretty(self, config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, text)
    }
}

/// Which main stat a `Reward::RandomMainStat` rung pays into.
///
/// A pure function of the id, seeded from the id's own bytes and never from
/// `resources::GameRng`, for the reason `Game::orphan_species` gives: the
/// answer is written into `profile.ron` and must be identical after a reload
/// and on every machine, and a draw off the shared stream would also shift
/// every later roll in the run.
///
/// The fold is spelled out rather than handed to `DefaultHasher`, whose
/// output is explicitly not stable across Rust releases — a toolchain upgrade
/// would otherwise silently disagree with every profile already on disk.
/// FNV-1a, 64-bit.
pub fn roll_main_stat(id: &AchievementId) -> MainStat {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in id.as_str().as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    MainStat::all()[(hash % MainStat::all().len() as u64) as usize]
}

/// Every authored rung with what it pays and, where earned, what earning it
/// looked like — in `AchievementDb::iter` order, which is stable between
/// runs so the screen does not reshuffle.
///
/// A free function rather than a `Game` method because the achievements
/// screen opens from the main menu, where there is no run: app-core holds
/// both a db and the profile and calls this either way. It is still engine
/// code, which is what keeps the per-row wording out of the renderer.
pub fn report(db: &AchievementDb, profile: &Profile) -> Vec<crate::views::AchievementRow> {
    db.iter()
        .map(|def| crate::views::AchievementRow {
            name: def.name.clone(),
            description: def.description.clone(),
            reward: reward_label(&def.reward),
            earned: profile.earned.iter().find(|e| e.id == def.id).map(|e| {
                crate::views::EarnedSummary {
                    tick: e.first_tick,
                    permadeath: e.permadeath,
                    rolled_stat: e.rolled_stat.map(|s| s.label().to_string()),
                }
            }),
        })
        .collect()
}

/// How a reward reads on the screen. A `RandomMainStat` is deliberately
/// unspecific here — the roll *is* predictable from the id, but which stat it
/// landed on is the small reveal of earning it, and the earned row shows the
/// answer through `EarnedSummary::rolled_stat`.
fn reward_label(reward: &Reward) -> String {
    match reward {
        Reward::RandomMainStat(n) => format!("+{n} to one main stat"),
        Reward::PerkPoints(n) => format!("+{n} Perk Point"),
        Reward::StartingProgram(species) => format!("start with a {species}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::{
        MAX_PROFILE_PERK_POINTS, MAX_PROFILE_STARTING_PROGRAMS, MAX_PROFILE_STAT_POINTS,
    };
    use std::collections::HashSet;

    fn temp_profile(tag: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("fp_profile_{tag}_{}.ron", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    fn earned(id: &str, permadeath: bool) -> Earned {
        Earned {
            id: AchievementId::from(id),
            first_tick: 7,
            permadeath,
            rolled_stat: Some(MainStat::Def),
        }
    }

    #[test]
    fn a_profile_survives_a_round_trip_through_ron() {
        let path = temp_profile("roundtrip");
        let mut profile = Profile::default();
        profile.record(Earned {
            id: AchievementId::from("breach_zone_4"),
            first_tick: 812,
            permadeath: true,
            rolled_stat: None,
        });
        profile.record(Earned {
            id: AchievementId::from("breach_zone_2"),
            first_tick: 190,
            permadeath: false,
            rolled_stat: Some(MainStat::Atk),
        });
        profile.save(&path).unwrap();

        let (reloaded, warning) = Profile::load(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(warning, None);
        assert_eq!(reloaded.earned, profile.earned);
    }

    #[test]
    fn an_absent_profile_is_empty_not_an_error() {
        let path = temp_profile("absent");
        let (profile, warning) = Profile::load(&path);
        assert!(profile.earned.is_empty());
        assert_eq!(
            warning, None,
            "a first run has no profile; that is not worth telling the player about"
        );
    }

    #[test]
    fn a_corrupt_profile_is_empty_not_a_panic() {
        let path = temp_profile("corrupt");
        std::fs::write(&path, "this is not RON at all {{{").unwrap();
        let (profile, warning) = Profile::load(&path);
        let _ = std::fs::remove_file(&path);
        assert!(profile.earned.is_empty());
        assert!(warning.is_some(), "a corrupt profile should say so");
    }

    #[test]
    fn recording_the_same_achievement_twice_earns_it_once() {
        let mut profile = Profile::default();
        assert!(profile.record(earned("breach_zone_2", false)));
        assert!(!profile.record(earned("breach_zone_2", false)));
        assert_eq!(profile.earned.len(), 1);
        assert!(profile.contains(&AchievementId::from("breach_zone_2")));
    }

    #[test]
    fn a_permadeath_re_earn_upgrades_the_flag_and_never_downgrades() {
        let mut profile = Profile::default();
        profile.record(earned("breach_zone_2", false));
        profile.record(earned("breach_zone_2", true));
        assert!(
            profile.earned[0].permadeath,
            "a permadeath re-earn should upgrade the flag"
        );
        assert_eq!(
            profile.earned[0].first_tick, 7,
            "the first earn's cycle stands"
        );

        let mut profile = Profile::default();
        profile.record(earned("breach_zone_2", true));
        profile.record(earned("breach_zone_2", false));
        assert!(
            profile.earned[0].permadeath,
            "a forgiving re-earn must never downgrade the flag"
        );
    }

    fn achievement_assets_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/achievements")
    }

    #[test]
    fn the_report_lists_every_authored_achievement() {
        let (db, _) = AchievementDb::load_dir(&achievement_assets_dir()).unwrap();
        let rows = report(&db, &Profile::default());
        assert_eq!(
            rows.len(),
            13,
            "the screen lists every rung, earned or not — the point is showing what is left"
        );
        assert!(rows.iter().all(|r| !r.reward.is_empty()));
    }

    #[test]
    fn an_unearned_achievement_reports_no_summary() {
        let (db, _) = AchievementDb::load_dir(&achievement_assets_dir()).unwrap();
        let rows = report(&db, &Profile::default());
        assert!(rows.iter().all(|r| r.earned.is_none()));
    }

    #[test]
    fn an_earned_achievement_reports_its_cycle_mode_and_rolled_stat() {
        let (db, _) = AchievementDb::load_dir(&achievement_assets_dir()).unwrap();
        let mut profile = Profile::default();
        profile.record(Earned {
            id: AchievementId::from("breach_zone_2"),
            first_tick: 812,
            permadeath: true,
            rolled_stat: Some(MainStat::Integrity),
        });

        let rows = report(&db, &profile);
        let row = rows
            .iter()
            .find(|r| r.name == "First Breach")
            .expect("the shipped ladder has this rung");
        let summary = row.earned.as_ref().expect("it was earned");
        assert_eq!(summary.tick, 812);
        assert!(summary.permadeath);
        assert_eq!(summary.rolled_stat.as_deref(), Some("Integrity"));

        assert!(
            rows.iter().filter(|r| r.earned.is_some()).count() == 1,
            "only the one recorded rung is earned"
        );
    }

    #[test]
    fn the_shipped_ladder_loads() {
        let (db, warnings) = AchievementDb::load_dir(&achievement_assets_dir()).unwrap();
        assert!(
            warnings.is_empty(),
            "shipped achievements should all parse: {warnings:?}"
        );
        assert_eq!(db.iter().count(), 13);
        for def in db.iter() {
            assert!(!def.name.is_empty(), "{} needs a name", def.id);
            assert!(
                !def.description.is_empty(),
                "{} needs a description",
                def.id
            );
        }
    }

    #[test]
    fn a_malformed_achievement_file_is_skipped_not_fatal() {
        let dir = std::env::temp_dir().join(format!("fp_ach_malformed_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.ron"), "(id: \"x\", trigger: NotATrigger)").unwrap();
        std::fs::write(
            dir.join("ok.ron"),
            "(id: \"good\", name: \"Good\", description: \"d\", trigger: ZoneReached(2), \
             reward: PerkPoints(1))",
        )
        .unwrap();

        let (db, warnings) = AchievementDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(warnings.len(), 1, "the junk file should warn: {warnings:?}");
        assert!(db.get(&AchievementId::from("good")).is_some());
        assert_eq!(db.iter().count(), 1);
    }

    #[test]
    fn a_reward_that_pays_nothing_is_refused() {
        let dir = std::env::temp_dir().join(format!("fp_ach_free_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("nothing.ron"),
            "(id: \"nothing\", name: \"N\", description: \"d\", trigger: ZoneReached(2), \
             reward: RandomMainStat(0))",
        )
        .unwrap();
        std::fs::write(
            dir.join("nobody.ron"),
            "(id: \"nobody\", name: \"N\", description: \"d\", trigger: ZoneReached(2), \
             reward: StartingProgram(\"\"))",
        )
        .unwrap();

        let (db, warnings) = AchievementDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(warnings.len(), 2, "both should warn: {warnings:?}");
        assert_eq!(db.iter().count(), 0);
    }

    #[test]
    fn the_full_ladder_stays_under_its_ceiling() {
        let (db, _) = AchievementDb::load_dir(&achievement_assets_dir()).unwrap();
        let mut stat_points = 0;
        let mut perk_points = 0;
        let mut programs = 0;
        for def in db.iter() {
            match &def.reward {
                Reward::RandomMainStat(n) => stat_points += n,
                Reward::PerkPoints(n) => perk_points += n,
                Reward::StartingProgram(_) => programs += 1,
            }
        }
        assert!(
            stat_points <= MAX_PROFILE_STAT_POINTS,
            "a fully-cleared profile hands out {stat_points} stat points, over the \
             {MAX_PROFILE_STAT_POINTS} ceiling. balance_sim does not model the profile — this \
             assertion is the only gate on how much a cross-run profile is worth."
        );
        assert!(
            perk_points <= MAX_PROFILE_PERK_POINTS,
            "a fully-cleared profile hands out {perk_points} Perk Points, over the \
             {MAX_PROFILE_PERK_POINTS} ceiling. balance_sim does not model the profile — this \
             assertion is the only gate on how much a cross-run profile is worth."
        );
        assert!(
            programs <= MAX_PROFILE_STARTING_PROGRAMS,
            "a fully-cleared profile hands out {programs} starting programs, over the \
             {MAX_PROFILE_STARTING_PROGRAMS} ceiling. balance_sim does not model the profile — \
             this assertion is the only gate on how much a cross-run profile is worth."
        );
    }

    #[test]
    fn a_rolled_stat_is_a_pure_function_of_the_id() {
        let (db, _) = AchievementDb::load_dir(&achievement_assets_dir()).unwrap();
        let rolling: Vec<&AchievementDef> = db
            .iter()
            .filter(|d| matches!(d.reward, Reward::RandomMainStat(_)))
            .collect();
        assert!(rolling.len() >= 4, "the ladder should roll more than once");

        for def in &rolling {
            assert_eq!(
                roll_main_stat(&def.id),
                roll_main_stat(&def.id),
                "{} must roll the same stat every time — the answer is written into \
                 profile.ron and has to survive a reload",
                def.id
            );
        }

        let distinct: HashSet<MainStat> = rolling.iter().map(|d| roll_main_stat(&d.id)).collect();
        assert!(
            distinct.len() > 1,
            "every rolling rung landed on the same stat; the roll is not distributing"
        );
    }
}
