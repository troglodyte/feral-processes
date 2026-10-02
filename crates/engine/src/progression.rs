use crate::attributes::{AttributeDb, AttributeId, DerivedStat};
use crate::components::{Attributes, Experience, POWER_MAX, Stats};
use crate::species::SpeciesDef;
use crate::tuning::{
    ATK_PER_LEVEL, CANONICAL_ANALYSIS_PER_LEVEL, CANONICAL_PARITY_PER_LEVEL, CRIT_CHANCE,
    CRIT_CHANCE_MAX, DIFFICULTY_EASY_MAX, EMULATION_EDGE, EMULATION_EDGE_PER_PERK_LEVEL,
    FUMBLE_CHANCE, FUMBLE_CHANCE_MAX, HP_PER_LEVEL, MIN_MAX_POWER, MINING_EXTRACTION_CAP,
    PLAYER_BASE_STATS, SETBACK_XP_PENALTY_FRACTION, STAT_POINTS_PER_LEVEL, STATUS_RESIST_MAX,
    STATUS_RESIST_MIN, XP_CHALLENGE_CEIL, XP_CHALLENGE_FLOOR, XP_PER_LEVEL_STEP,
};
use bevy_ecs::entity::Entity;
use std::collections::BTreeMap;

/// One stat's flat per-level growth, scaled by `growth_multiplier` and
/// rounded to the nearest whole point. With `ATK_PER_LEVEL`
/// both at 2, a multiplier has to cross a rounding boundary (roughly
/// +0.25) to actually change those two — `HP_PER_LEVEL` (24) has much finer
/// effective granularity.
fn scaled_growth(per_level: i32, growth_multiplier: f32) -> i32 {
    (per_level as f32 * growth_multiplier).round() as i32
}

/// What a call to `add_xp` granted: how many levels, and the stat growth
/// those levels came with, summed across all of them.
///
/// Returned rather than left for callers to work out by diffing `Stats`
/// themselves, because three separate log sites report it — the player, a
/// party member and a posted worker — and a fourth added later would
/// otherwise silently report nothing. `hp` is deliberately absent: a
/// level-up full-heals, so a delta on it measures the heal, not the growth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelGain {
    pub levels: u32,
    pub max_hp: i32,
    pub atk: i32,
    /// XP that arrived while already at the cap and bought no level.
    ///
    /// Reported rather than acted on, because `add_xp` is a pure function
    /// and has no world to spend it in — `Game::convert_overflow_xp` is
    /// what turns it into Perk Points, and only for the player. It is
    /// *also* still sitting in `Experience::xp`: this is a report of what
    /// went in, not a second store, which is what lets a breach spend the
    /// same pile on real levels.
    pub overflow: u32,
    /// Attribute points banked by `Growth::Points`, to spend on the Points
    /// screen. Zero for `Growth::Auto`.
    pub stat_points: u32,
    /// Parity and Analysis points a `Growth::ProgramPoints` level-up earned
    /// and `Game::apply_program_levels` has yet to place. Zero otherwise.
    pub parity: u32,
    pub analysis: u32,
}

/// How a level-up changes the levelled body's stats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Growth {
    /// Flat per-level growth scaled by `multiplier` (a species' rate) -
    /// companions, workers, sorties and the arena's opponents.
    Auto { multiplier: f32 },
    /// No stat changes: the player banks `STAT_POINTS_PER_LEVEL` a level and
    /// spends them, and `Game::recompute_derived` does the rest.
    Points,
    /// A seated program's level-up: stats do not move and nothing heals here.
    /// The points `program_level_points(multiplier, roll)` yields accumulate
    /// on `LevelGain` for `Game::apply_program_levels`, which places them and
    /// recomputes - so a caller without a `Game` (a bevy system) can still
    /// level a seated program by handing it the gain.
    ProgramPoints { multiplier: f32, roll: f32 },
}

/// The Parity and Analysis one level earns a seated program: the canonical
/// split scaled by the species' growth multiplier `g` and the individual's
/// growth `roll`. At roll 1 and every shipped `g` this is exactly
/// `scaled_growth` of the old per-level HP and attack, since `Parity` buys 6
/// HP and `Analysis` 1 attack.
pub fn program_level_points(g: f32, roll: f32) -> (u32, u32) {
    let scaled = |canonical: u32| (canonical as f32 * g * roll).round() as u32;
    (
        scaled(CANONICAL_PARITY_PER_LEVEL),
        scaled(CANONICAL_ANALYSIS_PER_LEVEL),
    )
}

impl LevelGain {
    /// Folds another `add_xp`'s gain into this one, so a tally can cover a
    /// whole fight rather than a single kill (see `resources::XpTally`).
    ///
    /// Plain summation is exactly right here because `stat_rows` recovers
    /// each row's "before" by subtracting the delta from the stats as they
    /// stand now: three kills' deltas summed, read against the stats after
    /// the third, give the range across all three.
    pub fn absorb(&mut self, other: LevelGain) {
        self.levels += other.levels;
        self.max_hp += other.max_hp;
        self.atk += other.atk;
        self.overflow += other.overflow;
        self.stat_points += other.stat_points;
        self.parity += other.parity;
        self.analysis += other.analysis;
    }

    /// The two rows a level-up's stat block always has, measured against
    /// `stats` as they stand *after* the `add_xp` call that produced this —
    /// so each row's "before" is recovered by subtracting the delta rather
    /// than by the caller having snapshotted anything.
    ///
    /// Two rather than three: levelling never raises mitigation — a
    /// percentage that grows per level approaches immunity — so there is no
    /// third row to draw. See `components::Stats::mitigation`.
    pub fn stat_rows(&self, stats: &Stats) -> [StatRow; 2] {
        [
            StatRow::grown("Max HP", stats.max_hp, self.max_hp),
            StatRow::grown("ATK", stats.atk, self.atk),
        ]
    }
}

/// One line of a level-up's stat block: what grew, and from what to what.
///
/// Deliberately not tied to `LevelGain`'s three fields. The player's block
/// also reports the Perk Point and Decompiler skill a level grants, and
/// neither is anything `add_xp` computes — so the block is built from a list
/// of rows that any caller can extend, rather than from a struct that would
/// carry two fields only one of the three callers ever fills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatRow {
    pub label: &'static str,
    pub before: i32,
    pub after: i32,
}

impl StatRow {
    pub fn new(label: &'static str, before: i32, after: i32) -> Self {
        Self {
            label,
            before,
            after,
        }
    }

    /// A row phrased as "this ended at `after`, having gained `delta`".
    fn grown(label: &'static str, after: i32, delta: i32) -> Self {
        Self::new(label, after - delta, after)
    }
}

/// The indented lines under a level-up announcement, one per stat that
/// actually moved. A stat that didn't is dropped: `ATK_PER_LEVEL` is 2, so a
/// low `growth_multiplier` rounds it away on a given level (see
/// `scaled_growth`), and "ATK 14 → 14" is noise.
///
/// Lives here rather than in a frontend because all three sites that
/// announce a level-up push these straight into `MessageLog` — the engine
/// owns the wording of a log line, and a renderer-side version would leave
/// the base log's copy unformatted.
pub fn stat_block(rows: &[StatRow]) -> Vec<String> {
    rows.iter()
        .filter(|r| r.before != r.after)
        .map(|r| format!("  {} {} → {}", r.label, r.before, r.after))
        .collect()
}

/// XP required to advance from `level` to `level + 1`.
pub fn xp_for_level(level: u32) -> u32 {
    level * XP_PER_LEVEL_STEP
}

/// What a defeated program is worth: its whole HP bar, scaled by how hard it
/// was to put down.
///
/// `threat_ratio` is `battle::threat_ratio` of the victim against the
/// player alone, taken by `Game::kill_xp` — the formula `difficulty_color`
/// buckets into the con-colours drawn on the map, measured against a
/// narrower side. That sharing is the point rather than a convenience: the
/// glyph's colour is the only advance notice a fight's XP value gets, so the
/// two must be one formula. The con reads the whole party and this reads the
/// player alone, so a companion never costs XP. Full XP lands at
/// `DIFFICULTY_EASY_MAX`, the green/yellow boundary, which makes the whole
/// rule "green pays less, yellow and up pays full or more".
///
/// Takes the ratio rather than the two powers so it stays a pure function of
/// numbers, testable without a `Game` — the same reason `add_xp` takes
/// `xp_boost_pct` instead of reading a buff off the world. `Game::kill_xp`
/// is the one caller that knows where the powers come from.
///
/// See `XP_CHALLENGE_FLOOR`/`XP_CHALLENGE_CEIL` for why both clamps are
/// load-bearing, in opposite directions.
///
/// A defeated program always pays *something*, which `XP_CHALLENGE_FLOOR`
/// alone does not deliver: a quarter of a small enough bar rounds to zero,
/// and a kill that silently pays nothing reads as a bug rather than as
/// contempt. Unreachable on the shipped roster — the smallest `base_hp` is
/// 38 — so this is a floor for mods and fixtures.
pub fn kill_xp(victim_max_hp: i32, threat_ratio: f64) -> u32 {
    if victim_max_hp <= 0 {
        return 0;
    }
    let factor = (threat_ratio / DIFFICULTY_EASY_MAX).clamp(XP_CHALLENGE_FLOOR, XP_CHALLENGE_CEIL);
    ((victim_max_hp as f64 * factor).round() as u32).max(1)
}

/// What an emulated body fights with, in place of a `Kit::Innate`'s own
/// `Stats` — see `progression::emulated_stats`.
///
/// Deliberately not `Stats`: an emulation never touches HP (`Stats` is
/// never written by this feature — see spec §1 "Strength"), so a struct
/// with the two fields it actually replaces cannot be mistaken for a
/// third store of health.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmulatedStats {
    pub atk: i32,
    pub mitigation: i32,
}

/// What emulating `def` as a `player_level`-level player, with
/// `fidelity_level` levels of `Perk::EmulationFidelity` bought, fights
/// with.
///
/// A species' base stats are level 1 (see `SpeciesDef::base_atk` and
/// friends), so growing "to the player's level" is `levels_gained =
/// player_level - 1` through `stats_after_levels` — the same growth a
/// tamed member of `def` gets on level-up, at the species' own
/// `growth_multiplier`. Both `atk` and `mitigation` then take
/// `EMULATION_EDGE + EMULATION_EDGE_PER_PERK_LEVEL * fidelity_level`,
/// rounded — `EMULATION_EDGE` sits above 1.0 so an emulation always beats
/// a wild program of the same species at the same level.
///
/// Mitigation is not scaled by level even before the multiplier:
/// `stats_after_levels` already carries it through untouched (percentage
/// points approaching immunity — see `components::Stats::mitigation`), so
/// only the multiplier moves it here. The `MAX_MITIGATION_PERCENT` cap is
/// not applied in this function — that stays `Game::effective_mitigation`'s
/// job, the same as for every other body.
///
/// `Game::kit_of` calls this for an emulating body, and the picker's
/// preview calls it too, so the preview and the fight cannot disagree.
pub fn emulated_stats(def: &SpeciesDef, player_level: u32, fidelity_level: u32) -> EmulatedStats {
    let base = Stats {
        hp: def.base_hp,
        max_hp: def.base_hp,
        atk: def.base_atk,
        mitigation: def.base_mitigation,
    };
    let levels_gained = player_level.saturating_sub(1);
    let grown = stats_after_levels(base, levels_gained, def.growth_multiplier);
    let multiplier = EMULATION_EDGE + EMULATION_EDGE_PER_PERK_LEVEL * fidelity_level as f32;
    EmulatedStats {
        atk: (grown.atk as f32 * multiplier).round() as i32,
        mitigation: (grown.mitigation as f32 * multiplier).round() as i32,
    }
}

/// `base` after `levels_gained` level-ups at `growth_multiplier`, fully
/// healed — the same growth `add_xp` applies per level-up, computed
/// directly rather than by spending XP one level at a time. Lets balance
/// projections (see `crate::balance_sim`) reuse the real growth constants
/// instead of re-deriving them.
pub fn stats_after_levels(base: Stats, levels_gained: u32, growth_multiplier: f32) -> Stats {
    let levels_gained = levels_gained as i32;
    let max_hp = base.max_hp + scaled_growth(HP_PER_LEVEL, growth_multiplier) * levels_gained;
    Stats {
        hp: max_hp,
        max_hp,
        atk: base.atk + scaled_growth(ATK_PER_LEVEL, growth_multiplier) * levels_gained,
        // Carried through untouched. Mitigation is percentage points, and a
        // percentage that grows per level approaches immunity — see
        // `components::Stats::mitigation`.
        mitigation: base.mitigation,
    }
}

/// The stats an entity derives from before any attribute moves them.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DerivedBase {
    pub max_hp: i32,
    pub atk: i32,
    pub mitigation: i32,
    pub decompiler: i32,
    pub max_power: f32,
    pub status_resist: i32,
    pub extraction: f32,
    pub crit: f64,
    pub fumble: f64,
}

impl DerivedBase {
    /// The player's level-1 numbers, from the tuning constants.
    pub fn player() -> Self {
        DerivedBase {
            max_hp: PLAYER_BASE_STATS.max_hp,
            atk: PLAYER_BASE_STATS.atk,
            mitigation: PLAYER_BASE_STATS.mitigation,
            decompiler: 0,
            max_power: POWER_MAX,
            status_resist: 0,
            extraction: 0.0,
            crit: CRIT_CHANCE,
            fumble: FUMBLE_CHANCE,
        }
    }

    /// The base a seated program derives from: the three `Stats` figures it
    /// already holds (gear and `BoughtStats` already taken off) less what its
    /// attributes contribute, so `derive` of it gives the same figures back.
    /// Everything else starts where the player's does. Integer arithmetic on
    /// the same `attribute_contribution` `derive` adds, so the round trip is
    /// exact and no clamp can bite.
    pub fn program(max_hp: i32, atk: i32, mitigation: i32, contribution: &DerivedStats) -> Self {
        DerivedBase {
            max_hp: max_hp - contribution.max_hp,
            atk: atk - contribution.atk,
            mitigation: mitigation - contribution.mitigation,
            ..DerivedBase::player()
        }
    }
}

/// What `derive` answers: every stat an attribute can feed, already clamped
/// to its range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DerivedStats {
    pub max_hp: i32,
    pub atk: i32,
    pub mitigation: i32,
    pub decompiler: i32,
    pub max_power: f32,
    pub status_resist: i32,
    pub extraction: f32,
    /// The crit band `battle::resolve_attack` reads, before its clamp to
    /// the hit chance.
    pub crit: f64,
    /// The fumble band, before its clamp to `1 - hit chance`.
    pub fumble: f64,
}

impl DerivedStats {
    /// The figure for `stat` as a float, so a screen can list any stat an
    /// attribute feeds without naming each field.
    pub fn get(&self, stat: crate::attributes::DerivedStat) -> f32 {
        use crate::attributes::DerivedStat as S;
        match stat {
            S::MaxHp => self.max_hp as f32,
            S::Atk => self.atk as f32,
            S::Mitigation => self.mitigation as f32,
            S::Decompiler => self.decompiler as f32,
            S::MaxPower => self.max_power,
            S::StatusResist => self.status_resist as f32,
            S::Extraction => self.extraction,
            S::Crit => self.crit as f32,
            S::Fumble => self.fumble as f32,
        }
    }
}

/// How many rounds a status armed for `duration` lasts on a body with
/// `status_resist` percent Persistence: shortened by that share, lengthened
/// when it is negative, and never below one round. No RNG - resist changes
/// how long a condition lasts, not whether it lands.
pub fn resisted_duration(duration: u32, status_resist: i32) -> u32 {
    let scaled = duration as f32 * (1.0 - status_resist as f32 / 100.0);
    (scaled.round() as u32).max(1)
}

/// What `attrs` add above their catalogue bases, per stat: the rounded sum of
/// `per_point * (value - base)` with no base added and no range applied, so it
/// can be negative. `derive` adds a base to it and clamps, and `Game::seat_derived`
/// subtracts it from a program's stored figures to find the base they imply -
/// one sum, so the two cannot disagree. Crit, fumble and extraction stay
/// fractional, as they are in `derive`.
pub fn attribute_contribution(attrs: &Attributes, db: &AttributeDb) -> DerivedStats {
    let mut sums = BTreeMap::<DerivedStat, f32>::new();
    for def in db.iter() {
        let delta = attrs.get(&def.id).map_or(0, |v| v - def.base) as f32;
        for effect in &def.effects {
            *sums.entry(effect.stat).or_insert(0.0) += effect.per_point * delta;
        }
    }
    let sum = |stat: DerivedStat| sums.get(&stat).copied().unwrap_or(0.0);
    let rounded = |stat: DerivedStat| sum(stat).round() as i32;
    DerivedStats {
        max_hp: rounded(DerivedStat::MaxHp),
        atk: rounded(DerivedStat::Atk),
        mitigation: rounded(DerivedStat::Mitigation),
        decompiler: rounded(DerivedStat::Decompiler),
        max_power: sum(DerivedStat::MaxPower).round(),
        status_resist: rounded(DerivedStat::StatusResist),
        extraction: sum(DerivedStat::Extraction),
        crit: sum(DerivedStat::Crit) as f64,
        fumble: sum(DerivedStat::Fumble) as f64,
    }
}

/// The one formula. For each stat, `base + attribute_contribution`, then
/// clamped to that stat's range. An attribute the store does not hold counts
/// as its base and contributes nothing. Nothing else computes a derived stat:
/// the game, the Points preview and `balance_sim` all call this.
pub fn derive(base: &DerivedBase, attrs: &Attributes, db: &AttributeDb) -> DerivedStats {
    let c = attribute_contribution(attrs, db);
    DerivedStats {
        max_hp: (base.max_hp + c.max_hp).max(1),
        atk: (base.atk + c.atk).max(1),
        mitigation: (base.mitigation + c.mitigation).max(0),
        decompiler: (base.decompiler + c.decompiler).max(0),
        max_power: (base.max_power + c.max_power).round().max(MIN_MAX_POWER),
        status_resist: (base.status_resist + c.status_resist)
            .clamp(STATUS_RESIST_MIN, STATUS_RESIST_MAX),
        extraction: (base.extraction + c.extraction).clamp(0.0, MINING_EXTRACTION_CAP),
        crit: (base.crit + c.crit).clamp(0.0, CRIT_CHANCE_MAX),
        fumble: (base.fumble + c.fumble).clamp(0.0, FUMBLE_CHANCE_MAX),
    }
}

/// Whose attributes a spend raises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatOwner {
    Player,
    /// A seated program; refused with `SpendError::NoSuchTarget` otherwise.
    Program(Entity),
}

/// Why `Game::spend_stat_points` wrote nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpendError {
    /// The spend costs more than the owner has banked.
    InsufficientPoints,
    /// The owner has no banked points or no such attribute in the catalogue.
    NoSuchTarget,
    /// The attribute exists but has no effects, so a point buys nothing.
    NotBuyable,
}

/// The spend that reproduces the old automatic per-level growth: 4 Parity and
/// 2 Analysis a level, which `canonical_spend_is_the_old_per_level_growth`
/// holds equal to `HP_PER_LEVEL`, `ATK_PER_LEVEL` and the retired Decompiler
/// grant against the real assets.
/// `balance_sim` models the player with this.
pub fn canonical_spend(levels: u32) -> BTreeMap<AttributeId, u32> {
    BTreeMap::from([
        (
            AttributeId::from("parity"),
            CANONICAL_PARITY_PER_LEVEL * levels,
        ),
        (
            AttributeId::from("analysis"),
            CANONICAL_ANALYSIS_PER_LEVEL * levels,
        ),
    ])
}

/// Docks `exp` a mild fraction (`SETBACK_XP_PENALTY_FRACTION`) of its
/// current in-level XP as a death/jack-out penalty, returning how much was
/// lost (0 if there was none to lose). Never drops `xp` below 0 and never
/// touches `level` or `xp_to_next` — nothing drastic, just a setback.
pub fn apply_setback_xp_penalty(exp: &mut Experience) -> u32 {
    let lost = ((exp.xp as f64) * SETBACK_XP_PENALTY_FRACTION).round() as u32;
    exp.xp -= lost;
    lost
}

/// Adds `gained` XP, applying as many level-ups as the total allows (a big
/// enough gain can jump more than one level at once), stopping dead at
/// `level_cap` — an already-capped entity doesn't even accumulate the XP.
/// `None` means no ceiling at all; every live caller passes
/// `Some(Game::level_cap())`, which is one number for the player and every
/// companion. XP arriving at the cap is **banked into `exp.xp` and reported
/// as `LevelGain::overflow`**, not discarded. Each level-up grows max HP/attack/defense
/// (`Growth::Auto`, scaled by a species' `growth_multiplier`) or banks stat
/// points (`Growth::Points`, the player) and fully heals. Returns a `LevelGain` — how many levels, and the growth
/// they came with — so callers can both decide whether to log a "level up"
/// message and say what it gave.
///
/// `xp_boost_pct` scales `gained` itself, before anything else runs, by a
/// running `FieldBuffKind::XpBoost` field buff's power in percentage
/// points — callers with no such buff pass `0`. Kept as a parameter rather
/// than read off the world in here: `add_xp` has to stay a pure function so
/// it's testable without constructing a `Game`.
pub fn add_xp(
    exp: &mut Experience,
    stats: &mut Stats,
    gained: u32,
    growth: Growth,
    level_cap: Option<u32>,
    xp_boost_pct: i32,
) -> LevelGain {
    let cap = level_cap.unwrap_or(u32::MAX);
    let gained = (gained as f32 * (1.0 + xp_boost_pct as f32 / 100.0)).round() as u32;
    // **Banked, not discarded.** This used to return before touching `exp`
    // at all, so XP earned at the cap simply vanished. It accumulates in
    // `Experience::xp` — a field that is already saved and already idle up
    // here — and the caller decides what to do with it: the player converts
    // it to Perk Points, everyone else leaves it sitting. A breach then
    // spends the same pile on real levels, which is why banking and taxing
    // need only the one accumulator between them.
    exp.xp += gained;
    if exp.level >= cap {
        return LevelGain {
            overflow: gained,
            ..LevelGain::default()
        };
    }
    let mut gain = LevelGain::default();
    while exp.level < cap && exp.xp >= exp.xp_to_next {
        exp.xp -= exp.xp_to_next;
        exp.level += 1;
        exp.xp_to_next = xp_for_level(exp.level);
        gain.levels += 1;
        match growth {
            Growth::Auto { multiplier } => {
                let (hp, atk) = (
                    scaled_growth(HP_PER_LEVEL, multiplier),
                    scaled_growth(ATK_PER_LEVEL, multiplier),
                );
                stats.max_hp += hp;
                stats.atk += atk;
                gain.max_hp += hp;
                gain.atk += atk;
            }
            Growth::Points => gain.stat_points += STAT_POINTS_PER_LEVEL,
            Growth::ProgramPoints { multiplier, roll } => {
                let (parity, analysis) = program_level_points(multiplier, roll);
                gain.parity += parity;
                gain.analysis += analysis;
                continue;
            }
        }
        stats.hp = stats.max_hp;
    }
    gain
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attributes::AttributeDb;

    fn shipped_db() -> AttributeDb {
        AttributeDb::load_dir(&crate::tests::support::test_assets_dir().join("attributes"))
            .unwrap()
            .0
    }

    /// An attribute store holding every catalogue base, with `moves` added.
    fn attrs_at_base(db: &AttributeDb, moves: &[(&str, i32)]) -> Attributes {
        let mut attrs = Attributes::default();
        for def in db.iter() {
            attrs.set(&def.id, def.base);
        }
        for (id, delta) in moves {
            let id = AttributeId::from(*id);
            attrs.set(&id, attrs.get(&id).unwrap() + delta);
        }
        attrs
    }

    #[test]
    fn derive_at_catalogue_bases_is_the_players_base_stats() {
        let db = shipped_db();
        let d = derive(&DerivedBase::player(), &attrs_at_base(&db, &[]), &db);
        assert_eq!(d.max_hp, PLAYER_BASE_STATS.max_hp);
        assert_eq!(d.atk, PLAYER_BASE_STATS.atk);
        assert_eq!(d.mitigation, PLAYER_BASE_STATS.mitigation);
        assert_eq!(d.decompiler, 0);
        assert_eq!(d.max_power, POWER_MAX);
        assert_eq!(d.status_resist, 0);
        assert_eq!(d.extraction, 0.0);
        assert_eq!(d.crit, CRIT_CHANCE);
        assert_eq!(d.fumble, FUMBLE_CHANCE);
    }

    #[test]
    fn an_absent_attribute_counts_as_its_base() {
        let db = shipped_db();
        let d = derive(&DerivedBase::player(), &Attributes::default(), &db);
        assert_eq!(d.max_hp, PLAYER_BASE_STATS.max_hp);
        assert_eq!(d.max_power, POWER_MAX);
    }

    #[test]
    fn each_effect_moves_only_its_own_stats() {
        let db = shipped_db();
        let base = DerivedBase::player();
        let at = derive(&base, &attrs_at_base(&db, &[]), &db);
        let parity = derive(&base, &attrs_at_base(&db, &[("parity", 3)]), &db);
        assert_eq!(parity.max_hp, at.max_hp + 18);
        assert_eq!((parity.atk, parity.mitigation), (at.atk, at.mitigation));
        assert_eq!(parity.max_power, at.max_power);
        let footprint = derive(&base, &attrs_at_base(&db, &[("footprint", 4)]), &db);
        assert_eq!(footprint.mitigation, at.mitigation + 4);
        assert_eq!(footprint.max_hp, at.max_hp + 8);
        assert_eq!(footprint.atk, at.atk);
        let analysis = derive(&base, &attrs_at_base(&db, &[("analysis", 2)]), &db);
        assert_eq!(analysis.atk, at.atk + 2);
        assert_eq!(analysis.decompiler, 2);
        assert!((analysis.extraction - 0.01).abs() < 1e-6);
        assert_eq!(analysis.max_hp, at.max_hp);
        let bandwidth = derive(&base, &attrs_at_base(&db, &[("bandwidth", 5)]), &db);
        assert_eq!(bandwidth.max_power, POWER_MAX + 10.0);
        assert_eq!(bandwidth.max_hp, at.max_hp);
        let persistence = derive(&base, &attrs_at_base(&db, &[("persistence", 7)]), &db);
        assert_eq!(persistence.status_resist, 7);
        assert_eq!(persistence.atk, at.atk);
        let entropy = derive(&base, &attrs_at_base(&db, &[("entropy", 10)]), &db);
        // Volatility: both bands widen together.
        assert!((entropy.crit - (CRIT_CHANCE + 0.02)).abs() < 1e-6);
        assert!((entropy.fumble - (FUMBLE_CHANCE + 0.01)).abs() < 1e-6);
        assert_eq!((entropy.max_hp, entropy.atk), (at.max_hp, at.atk));
        assert_eq!(parity.crit, at.crit);
        assert_eq!(parity.fumble, at.fumble);
    }

    /// Two half-point terms sum to a whole point: rounding each term first
    /// would give 1 + 1 = 2 (or 0 + 0), rounding the sum gives 1.
    #[test]
    fn the_sum_is_rounded_once_per_stat() {
        let dir = crate::tests::support::scratch_assets_dir("derive_rounding");
        std::fs::create_dir_all(&*dir).unwrap();
        for (id, per_point) in [("a", 0.5), ("b", 0.5)] {
            std::fs::write(
                dir.join(format!("{id}.ron")),
                format!(
                    "(id: \"{id}\", name: \"{id}\", legacy: \"x\", short: \"s\", \
                     meaning: \"m\", base: 10, spread: 0, \
                     effects: [(stat: Atk, per_point: {per_point})], does: \"d\")"
                ),
            )
            .unwrap();
        }
        let (db, warnings) = AttributeDb::load_dir(&dir).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let d = derive(
            &DerivedBase::player(),
            &attrs_at_base(&db, &[("a", 1), ("b", 1)]),
            &db,
        );
        assert_eq!(d.atk, PLAYER_BASE_STATS.atk + 1);
    }

    #[test]
    fn the_clamps_hold() {
        let db = shipped_db();
        let base = DerivedBase::player();
        let low = derive(
            &base,
            &attrs_at_base(
                &db,
                &[
                    ("parity", -1000),
                    ("analysis", -1000),
                    ("footprint", -1000),
                    ("bandwidth", -1000),
                    ("persistence", -1000),
                    ("entropy", -1000),
                ],
            ),
            &db,
        );
        assert_eq!(low.max_hp, 1);
        assert_eq!(low.atk, 1);
        assert_eq!(low.mitigation, 0);
        assert_eq!(low.decompiler, 0);
        assert_eq!(low.max_power, MIN_MAX_POWER);
        assert_eq!(low.status_resist, STATUS_RESIST_MIN);
        assert_eq!(low.extraction, 0.0);
        assert_eq!((low.crit, low.fumble), (0.0, 0.0));
        let high = derive(
            &base,
            &attrs_at_base(
                &db,
                &[("analysis", 1000), ("persistence", 1000), ("entropy", 1000)],
            ),
            &db,
        );
        assert_eq!(high.crit, CRIT_CHANCE_MAX);
        assert_eq!(high.fumble, FUMBLE_CHANCE_MAX);
        assert_eq!(high.status_resist, STATUS_RESIST_MAX);
        assert_eq!(high.extraction, MINING_EXTRACTION_CAP);
    }

    /// Past the crit cap, a point of Entropy would buy fumble and nothing
    /// else, so the two caps are set to land on the same point. Retuning
    /// either `per_point` without the caps fails here.
    #[test]
    fn entropy_reaches_both_caps_at_the_same_point() {
        let db = shipped_db();
        let base = DerivedBase::player();
        let points_to_cap = (0..1000)
            .find(|n| {
                derive(&base, &attrs_at_base(&db, &[("entropy", *n)]), &db).crit >= CRIT_CHANCE_MAX
            })
            .unwrap();
        let at_cap = derive(
            &base,
            &attrs_at_base(&db, &[("entropy", points_to_cap)]),
            &db,
        );
        assert!(
            (at_cap.fumble - FUMBLE_CHANCE_MAX).abs() < 1e-6,
            "{}",
            at_cap.fumble
        );
        let below = derive(
            &base,
            &attrs_at_base(&db, &[("entropy", points_to_cap - 1)]),
            &db,
        );
        assert!(below.fumble < FUMBLE_CHANCE_MAX);
    }

    #[test]
    fn canonical_spend_is_the_old_per_level_growth() {
        let db = shipped_db();
        for n in 0..=40u32 {
            let mut attrs = attrs_at_base(&db, &[]);
            for (id, points) in canonical_spend(n) {
                attrs.set(&id, attrs.get(&id).unwrap() + points as i32);
            }
            let d = derive(&DerivedBase::player(), &attrs, &db);
            let old = stats_after_levels(
                PLAYER_BASE_STATS,
                n,
                crate::tuning::BASELINE_GROWTH_MULTIPLIER,
            );
            assert_eq!(
                (d.max_hp, d.atk, d.mitigation),
                (old.max_hp, old.atk, old.mitigation),
                "level {n}"
            );
            assert_eq!(d.decompiler, 2 * n as i32, "level {n}");
        }
        assert_eq!(
            canonical_spend(3).values().sum::<u32>(),
            STAT_POINTS_PER_LEVEL * 3
        );
    }
    #[test]
    fn program_points_equal_the_old_scaled_growth_at_roll_one() {
        let db = shipped_db();
        for g in [1.0f32, 1.25, 1.5, 2.0] {
            let (parity, analysis) = program_level_points(g, 1.0);
            let attrs = attrs_at_base(
                &db,
                &[("parity", parity as i32), ("analysis", analysis as i32)],
            );
            let d = derive(&DerivedBase::player(), &attrs, &db);
            assert_eq!(
                d.max_hp - PLAYER_BASE_STATS.max_hp,
                scaled_growth(HP_PER_LEVEL, g),
                "hp at g {g}"
            );
            assert_eq!(
                d.atk - PLAYER_BASE_STATS.atk,
                scaled_growth(ATK_PER_LEVEL, g),
                "atk at g {g}"
            );
        }
    }

    #[test]
    fn program_points_level_up_moves_no_stats_and_does_not_heal() {
        let mut exp = Experience::default();
        let mut stats = Stats {
            hp: 3,
            max_hp: 30,
            atk: 5,
            mitigation: 0,
        };
        let owed = exp.xp_to_next;
        let gain = add_xp(
            &mut exp,
            &mut stats,
            owed,
            Growth::ProgramPoints {
                multiplier: 1.5,
                roll: 1.0,
            },
            None,
            0,
        );
        assert_eq!(gain.levels, 1);
        assert_eq!((gain.parity, gain.analysis), (6, 3));
        assert_eq!((stats.hp, stats.max_hp, stats.atk), (3, 30, 5));
        assert_eq!((gain.max_hp, gain.atk), (0, 0));
    }

    use crate::tuning::{
        BASELINE_GROWTH_MULTIPLIER, DIFFICULTY_EASY_MAX, DIFFICULTY_EVEN_MAX, TALENT_START_LEVEL,
        XP_CHALLENGE_CEIL, XP_CHALLENGE_FLOOR,
    };

    /// The rule a player can state from the map's con-colours alone: full XP
    /// arrives exactly where green becomes yellow, so anything reading yellow
    /// or worse pays its whole HP bar and anything green pays less.
    #[test]
    fn a_kill_pays_its_full_hp_bar_at_the_green_yellow_boundary() {
        assert_eq!(kill_xp(200, DIFFICULTY_EASY_MAX), 200);
        assert!(
            kill_xp(200, DIFFICULTY_EASY_MAX - 0.2) < 200,
            "still green: pays less than its bar"
        );
        assert!(
            kill_xp(200, DIFFICULTY_EVEN_MAX) > 200,
            "an even match is past the boundary and pays more"
        );
    }

    /// The floor is what keeps the opening ring from paying literally
    /// nothing once the party outclasses it — farming should be pointless,
    /// not broken.
    #[test]
    fn an_outclassed_victim_pays_the_floor_rather_than_nothing() {
        assert_eq!(kill_xp(200, 0.001), (200.0 * XP_CHALLENGE_FLOOR) as u32);
        assert_eq!(
            kill_xp(200, 0.0),
            kill_xp(200, 0.001),
            "the floor holds all the way down, so a ratio of zero is not a special case"
        );
    }

    /// Without the ceiling a Stack guardian would earn a multiplier on top
    /// of HP that `STACK_DEPTH_STAT_STEP` has already inflated — the double
    /// count that made a handful of deep fights worth several levels.
    #[test]
    fn an_overwhelming_victim_pays_the_ceiling_rather_than_its_whole_ratio() {
        let capped = (200.0 * XP_CHALLENGE_CEIL) as u32;
        assert_eq!(kill_xp(200, 8.0), capped);
        assert_eq!(
            kill_xp(200, 40.0),
            capped,
            "five times deeper out of its depth is still the same cap"
        );
    }

    /// The whole point of the change: the *same* program is worth less to a
    /// stronger party. Asserted as a fall across a rising player power
    /// rather than against fixed numbers, so it fails if the factor is ever
    /// flattened back to a constant.
    #[test]
    fn the_same_victim_pays_less_as_the_party_outgrows_it() {
        let victim_power = 53.0; // a zone-1 drone, 48 + 4 + 1
        let earned: Vec<u32> = [98.0, 200.0, 400.0]
            .iter()
            .map(|player_power| kill_xp(48, victim_power / player_power))
            .collect();
        assert!(
            earned[0] > earned[1] && earned[1] > earned[2],
            "a drone should pay a level-1 party more than a grown one, got {earned:?}"
        );
    }

    #[test]
    fn a_victim_with_no_hp_bar_pays_nothing_at_any_ratio() {
        assert_eq!(kill_xp(0, 4.0), 0);
        assert_eq!(kill_xp(-5, 4.0), 0, "and a negative bar is not a reward");
    }

    /// A quarter of a small enough bar rounds to zero, and a kill that pays
    /// literally nothing reads as a bug rather than as contempt.
    #[test]
    fn a_victim_too_small_to_round_up_still_pays_one() {
        assert_eq!(kill_xp(1, 0.001), 1);
        assert_eq!(kill_xp(2, 0.001), 1);
    }

    fn base_stats() -> Stats {
        Stats {
            hp: 10,
            max_hp: 10,
            atk: 5,
            mitigation: 5,
        }
    }

    #[test]
    fn a_multi_level_jump_reports_the_summed_stat_growth() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        // Exactly the two levels `large_xp_gain_can_grant_multiple_levels`
        // buys, so every delta reported has to be twice one level's growth
        // rather than the last level's alone.
        let two_levels = xp_for_level(1) + xp_for_level(2);
        let gain = add_xp(
            &mut exp,
            &mut stats,
            two_levels,
            Growth::Auto {
                multiplier: BASELINE_GROWTH_MULTIPLIER,
            },
            None,
            0,
        );
        assert_eq!(gain.levels, 2);
        assert_eq!(gain.max_hp, 2 * HP_PER_LEVEL);
        assert_eq!(gain.atk, 2 * ATK_PER_LEVEL);
    }

    #[test]
    fn a_growth_multiplier_that_rounds_a_stat_away_reports_no_gain_for_it() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        // 1.1x leaves ATK_PER_LEVEL (2) rounding back to 2 — the case
        // `scaled_growth`'s doc warns about — while HP_PER_LEVEL (24) moves.
        // A stat that did move must still be reported, so this is not just
        // "everything is zero". The window is narrower than it was at 1 point
        // a level: 1.25x now genuinely moves ATK, which is the granularity
        // `HP_PER_LEVEL`'s `K = 2` was meant to buy back.
        let gain = add_xp(
            &mut exp,
            &mut stats,
            xp_for_level(1),
            Growth::Auto { multiplier: 1.1 },
            None,
            0,
        );
        assert_eq!(gain.levels, 1);
        assert_eq!(gain.max_hp, 26);
        assert_eq!(gain.atk, ATK_PER_LEVEL, "1.1 * 2 rounds back to 2");

        // 0.2x rounds ATK away entirely: 0.4 rounds to 0.
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let gain = add_xp(
            &mut exp,
            &mut stats,
            xp_for_level(1),
            Growth::Auto { multiplier: 0.2 },
            None,
            0,
        );
        assert_eq!(gain.levels, 1);
        assert_eq!(gain.atk, 0, "0.2 * 2 rounds to no attack gain");
    }

    #[test]
    fn points_growth_banks_six_a_level_and_grows_nothing() {
        let mut exp = Experience::default();
        let mut stats = Stats {
            hp: 3,
            ..base_stats()
        };
        let two_levels = xp_for_level(1) + xp_for_level(2);
        let gain = add_xp(&mut exp, &mut stats, two_levels, Growth::Points, None, 0);
        assert_eq!(gain.levels, 2);
        assert_eq!(gain.stat_points, 2 * STAT_POINTS_PER_LEVEL);
        assert_eq!((gain.max_hp, gain.atk), (0, 0));
        assert_eq!((stats.max_hp, stats.atk, stats.mitigation), (10, 5, 5));
        assert_eq!(stats.hp, stats.max_hp, "a level still full-heals");
    }

    #[test]
    fn auto_growth_banks_no_points() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let gain = add_xp(
            &mut exp,
            &mut stats,
            xp_for_level(1),
            Growth::Auto { multiplier: 1.0 },
            None,
            0,
        );
        assert_eq!(gain.levels, 1);
        assert_eq!(gain.stat_points, 0);
        assert!(stats.max_hp > 10);
    }

    #[test]
    fn a_gain_that_levels_nothing_reports_no_growth() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let gain = add_xp(
            &mut exp,
            &mut stats,
            5,
            Growth::Auto {
                multiplier: BASELINE_GROWTH_MULTIPLIER,
            },
            None,
            0,
        );
        assert_eq!(gain.levels, 0);
        assert_eq!(gain.max_hp, 0);
        assert_eq!(gain.atk, 0);
    }

    #[test]
    fn a_stat_block_skips_the_stats_that_did_not_move() {
        let rows = [
            StatRow::new("Max HP", 108, 120),
            StatRow::new("ATK", 14, 14),
            StatRow::new("DEF", 11, 12),
        ];
        assert_eq!(
            stat_block(&rows),
            vec![
                "  Max HP 108 → 120".to_string(),
                "  DEF 11 → 12".to_string()
            ],
            "a stat that did not change is noise, not news"
        );
    }

    #[test]
    fn a_stat_block_with_nothing_to_say_is_empty() {
        let rows = [StatRow::new("ATK", 14, 14)];
        assert!(stat_block(&rows).is_empty());
    }

    #[test]
    fn a_level_gain_builds_its_rows_from_the_stats_it_produced() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let gain = add_xp(
            &mut exp,
            &mut stats,
            xp_for_level(1),
            Growth::Auto {
                multiplier: BASELINE_GROWTH_MULTIPLIER,
            },
            None,
            0,
        );
        // The rows are derived from the *post-call* stats and the deltas, so
        // they have to name the values the creature actually ended on.
        assert_eq!(
            stat_block(&gain.stat_rows(&stats)),
            vec![
                format!("  Max HP 10 → {}", 10 + HP_PER_LEVEL),
                format!("  ATK 5 → {}", 5 + ATK_PER_LEVEL),
            ]
        );
    }

    #[test]
    fn xp_below_threshold_does_not_level_up() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let levels = add_xp(
            &mut exp,
            &mut stats,
            5,
            Growth::Auto {
                multiplier: BASELINE_GROWTH_MULTIPLIER,
            },
            None,
            0,
        )
        .levels;
        assert_eq!(levels, 0);
        assert_eq!(exp.level, 1);
        assert_eq!(exp.xp, 5);
        assert_eq!(stats.max_hp, 10);
    }

    #[test]
    fn enough_xp_levels_up_and_grows_stats() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        let levels = add_xp(
            &mut exp,
            &mut stats,
            xp_for_level(1),
            Growth::Auto {
                multiplier: BASELINE_GROWTH_MULTIPLIER,
            },
            None,
            0,
        )
        .levels;
        assert_eq!(levels, 1);
        assert_eq!(exp.level, 2);
        assert_eq!(stats.max_hp, 10 + HP_PER_LEVEL);
        assert_eq!(stats.hp, stats.max_hp, "level up should fully heal");
        assert_eq!(stats.atk, 5 + ATK_PER_LEVEL);
        assert_eq!(
            stats.mitigation, 5,
            "levelling never raises mitigation — it is percentage points"
        );
    }

    #[test]
    fn large_xp_gain_can_grant_multiple_levels() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        // Both levels' cost plus 5 to spare should clear both and carry the
        // remainder into the third.
        let levels = add_xp(
            &mut exp,
            &mut stats,
            xp_for_level(1) + xp_for_level(2) + 5,
            Growth::Auto {
                multiplier: BASELINE_GROWTH_MULTIPLIER,
            },
            None,
            0,
        )
        .levels;
        assert_eq!(levels, 2);
        assert_eq!(exp.level, 3);
        assert_eq!(exp.xp, 5);
    }

    #[test]
    fn growth_multiplier_scales_stat_gains_per_level_up() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        // 1.5x rounds HP_PER_LEVEL (24) to 36 and ATK_PER_LEVEL (2) to 3,
        // crossing the rounding boundary scaled_growth's doc comment warns
        // about — a smaller multiplier like 1.1 wouldn't move ATK at all.
        let levels = add_xp(
            &mut exp,
            &mut stats,
            xp_for_level(1),
            Growth::Auto { multiplier: 1.5 },
            None,
            0,
        )
        .levels;
        assert_eq!(levels, 1);
        assert_eq!(
            stats.max_hp,
            10 + 36,
            "1.5x should scale HP growth up from 24 to 36"
        );
        assert_eq!(
            stats.atk,
            5 + 3,
            "1.5x should scale ATK growth up from 2 to 3"
        );
        assert_eq!(
            stats.mitigation, 5,
            "no growth multiplier reaches mitigation — it is percentage points"
        );
    }

    #[test]
    fn stats_after_levels_matches_add_xp_at_the_same_growth_multiplier() {
        let mut exp = Experience::default();
        let mut stats = base_stats();
        for _ in 0..3 {
            let needed = exp.xp_to_next;
            add_xp(
                &mut exp,
                &mut stats,
                needed,
                Growth::Auto { multiplier: 1.5 },
                None,
                0,
            );
        }
        let projected = stats_after_levels(base_stats(), 3, 1.5);
        assert_eq!(stats.max_hp, projected.max_hp);
        assert_eq!(stats.atk, projected.atk);
        assert_eq!(stats.mitigation, projected.mitigation);
    }

    #[test]
    fn add_xp_stops_leveling_at_the_creature_cap() {
        let mut exp = Experience {
            level: TALENT_START_LEVEL,
            xp: 0,
            xp_to_next: xp_for_level(TALENT_START_LEVEL),
        };
        let mut stats = base_stats();

        let gain = add_xp(
            &mut exp,
            &mut stats,
            10_000,
            Growth::Auto {
                multiplier: BASELINE_GROWTH_MULTIPLIER,
            },
            Some(TALENT_START_LEVEL),
            0,
        );
        let levels = gain.levels;
        assert_eq!(
            gain.overflow, 10_000,
            "and it is reported, which is how the caller knows to convert it"
        );

        assert_eq!(
            levels, 0,
            "an already-capped creature shouldn't level up further"
        );
        assert_eq!(exp.level, TALENT_START_LEVEL);
        // It used to assert this was 0 — XP at the cap was discarded here.
        // It is banked instead, so the player's caller can spend it on Perk
        // Points and a breach can spend whatever is left on real levels.
        assert_eq!(
            exp.xp, 10_000,
            "XP awarded at the cap is banked, not thrown away"
        );
        assert_eq!(stats.max_hp, 10, "stats still shouldn't grow past the cap");
    }

    #[test]
    fn add_xp_caps_a_multi_level_jump_at_the_creature_cap() {
        let mut exp = Experience {
            level: TALENT_START_LEVEL - 1,
            xp: 0,
            xp_to_next: xp_for_level(TALENT_START_LEVEL - 1),
        };
        let mut stats = base_stats();

        // Enough XP to clear several levels if uncapped.
        let levels = add_xp(
            &mut exp,
            &mut stats,
            100_000,
            Growth::Auto {
                multiplier: BASELINE_GROWTH_MULTIPLIER,
            },
            Some(TALENT_START_LEVEL),
            0,
        )
        .levels;

        assert_eq!(
            levels, 1,
            "should only be able to gain the one level up to the cap"
        );
        assert_eq!(exp.level, TALENT_START_LEVEL);
    }

    /// The player passes no cap at all, so they keep leveling well past
    /// the ceiling creatures stop at.
    #[test]
    fn add_xp_without_a_cap_levels_past_the_creature_ceiling() {
        let mut exp = Experience {
            level: TALENT_START_LEVEL,
            xp: 0,
            xp_to_next: xp_for_level(TALENT_START_LEVEL),
        };
        let mut stats = base_stats();

        let levels = add_xp(
            &mut exp,
            &mut stats,
            100_000,
            Growth::Auto {
                multiplier: BASELINE_GROWTH_MULTIPLIER,
            },
            None,
            0,
        )
        .levels;

        assert!(levels > 0, "an uncapped entity should keep leveling");
        assert!(
            exp.level > TALENT_START_LEVEL,
            "uncapped leveling should pass the creature ceiling, got {}",
            exp.level
        );
        assert!(
            stats.max_hp > 10,
            "uncapped level-ups should still grow stats"
        );
    }

    /// A 50% `XpBoost` turns three-quarters of a level's XP into more than
    /// the whole of it, clearing the threshold to level 2 — a case a flat
    /// "differs in the right direction" assertion wouldn't catch, since the
    /// unboosted gain alone wouldn't level up at all.
    #[test]
    fn xp_boost_pct_scales_gained_xp_before_leveling() {
        let three_quarters = xp_for_level(1) * 3 / 4;
        let mut unboosted_exp = Experience::default();
        let mut unboosted_stats = base_stats();
        let levels = add_xp(
            &mut unboosted_exp,
            &mut unboosted_stats,
            three_quarters,
            Growth::Auto {
                multiplier: BASELINE_GROWTH_MULTIPLIER,
            },
            None,
            0,
        )
        .levels;
        assert_eq!(
            levels, 0,
            "three-quarters of a level's XP alone doesn't clear the threshold"
        );
        assert_eq!(unboosted_exp.xp, three_quarters);

        let mut boosted_exp = Experience::default();
        let mut boosted_stats = base_stats();
        let levels = add_xp(
            &mut boosted_exp,
            &mut boosted_stats,
            three_quarters,
            Growth::Auto {
                multiplier: BASELINE_GROWTH_MULTIPLIER,
            },
            None,
            50,
        )
        .levels;
        assert_eq!(
            levels, 1,
            "a 50% XpBoost lifts three-quarters of a level past the whole of it"
        );
        assert_eq!(boosted_exp.level, 2);
        assert_eq!(
            boosted_exp.xp,
            three_quarters * 3 / 2 - xp_for_level(1),
            "the excess carries over into the new level"
        );
    }

    #[test]
    fn setback_penalty_docks_a_mild_fraction_of_in_level_xp() {
        let mut exp = Experience {
            level: 3,
            xp: 10,
            xp_to_next: 40,
        };
        let lost = apply_setback_xp_penalty(&mut exp);
        assert_eq!(lost, 2, "20% of 10 xp");
        assert_eq!(exp.xp, 8);
        assert_eq!(exp.level, 3, "a setback should never touch level");
        assert_eq!(
            exp.xp_to_next, 40,
            "a setback should never touch xp_to_next"
        );
    }

    #[test]
    fn setback_penalty_is_a_no_op_with_zero_xp() {
        let mut exp = Experience::default();
        assert_eq!(apply_setback_xp_penalty(&mut exp), 0);
        assert_eq!(exp.xp, 0);
    }
}
