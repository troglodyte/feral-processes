//! The level-up summary page's whole engine half: what the player looked
//! like the instant a level landed (`Game::snapshot_player`), what a
//! typical program of the current sector looks like
//! (`Game::typical_foe`), and the drain that turns the two into a report
//! (`Game::take_level_up_report`).
//!
//! Every duel figure on the report is a call into `battle::hit_chance`,
//! `battle::expected_damage` or `battle::effective_hp` — nothing here rolls
//! its own formula, `CLAUDE.md`'s rule against a doc comment that claims to
//! mirror another module's arithmetic while quietly keeping a second copy.

use crate::progression::StatRow;
use crate::*;

/// Swings no shipped fight can actually reach — see `swings_to`.
pub(crate) const LEVEL_UP_SWINGS_UNREACHABLE: u32 = 999;

/// `ceil(ehp / per_swing)`, floored at the one edge `per_swing` can hit:
/// zero. `battle::hit_chance` never returns 0 (`HIT_CHANCE_MIN` keeps it
/// off the floor), so the only way `per_swing` lands at exactly 0 is a
/// combatant with both a zero-width, zero-power damage band and zero ATK —
/// no shipped species, but reachable from a mod's all-status "move". A
/// literal `(ehp / 0.0).ceil() as u32` would saturate to `u32::MAX` (Rust's
/// float-to-int cast saturates rather than panicking or wrapping) and the
/// page would read "4294967295 swings to win", which is a worse failure
/// than a made-up round number — so this clamps to a constant chosen to
/// read as "not happening" instead.
pub(crate) fn swings_to(ehp: f64, per_swing: f64) -> u32 {
    if per_swing <= 0.0 {
        return LEVEL_UP_SWINGS_UNREACHABLE;
    }
    (ehp / per_swing).ceil() as u32
}

/// The stat rows that moved between two snapshots: Max HP, ATK and
/// Mitigation. The level-up page and the perk preview both draw these, so
/// they cannot disagree about which stats a comparison covers.
pub(crate) fn stat_rows(before: &LevelSnapshot, after: &LevelSnapshot) -> Vec<StatRow> {
    [
        StatRow::new("Max HP", before.max_hp, after.max_hp),
        StatRow::new("ATK", before.combatant.atk, after.combatant.atk),
        StatRow::new("Mitigation", before.mitigation, after.mitigation),
    ]
    .into_iter()
    .filter(|row| row.before != row.after)
    .collect()
}

/// The duel figures of `before` against `after`, both facing the same
/// `foe` (only the player moves). Every figure is a call into
/// `battle::hit_chance` / `expected_damage` / `effective_hp` and
/// `swings_to`.
pub(crate) fn duel_comparison(
    before: &LevelSnapshot,
    after: &LevelSnapshot,
    foe: battle::Combatant,
    foe_ehp: f64,
    zone: u32,
) -> DuelComparison {
    let per_swing_before = battle::expected_damage(before.combatant, foe);
    let per_swing_after = battle::expected_damage(after.combatant, foe);
    let foe_per_swing_before = battle::expected_damage(foe, before.combatant);
    let foe_per_swing_after = battle::expected_damage(foe, after.combatant);
    let player_ehp_before = battle::effective_hp(before.max_hp, before.mitigation);
    let player_ehp_after = battle::effective_hp(after.max_hp, after.mitigation);
    DuelComparison {
        zone,
        hit_chance: (
            battle::hit_chance(before.combatant.accuracy, foe.evasion),
            battle::hit_chance(after.combatant.accuracy, foe.evasion),
        ),
        per_swing: (per_swing_before, per_swing_after),
        swings_to_win: (
            swings_to(foe_ehp, per_swing_before),
            swings_to(foe_ehp, per_swing_after),
        ),
        swings_to_down_you: (
            swings_to(player_ehp_before, foe_per_swing_before),
            swings_to(player_ehp_after, foe_per_swing_after),
        ),
    }
}

impl Game {
    /// `player`'s level-up "before"/"now" state: level, the two stats a
    /// level-up can move, Perk Points, stat points, and the swing
    /// profile `Game::duel_damage` itself builds — `combatant_profile`'s
    /// attacker side, whose `evasion` field the report also reads for the
    /// defender side (see `resources::LevelSnapshot`).
    ///
    /// Mirrors `Game::duel_damage`'s attacker profile —
    /// `combatant_profile(player, Swing { range: natural_range_of(player),
    /// ..Default::default() })`; a defender profile built with
    /// `Swing::default()` would differ from it only in `range`, which the
    /// report never reads off the defender side, so one `Combatant` serves
    /// both columns. Unlike `duel_damage`, this deliberately stops at the
    /// swing: the page's "swings to win" counts single swings, not rounds,
    /// so `Game::attacks_for` — a Striker's second swing — is never
    /// applied. Multiplying it in would price a Striker's page in rounds
    /// while every other class's reads in swings.
    pub(crate) fn snapshot_player(&self, player: Entity) -> LevelSnapshot {
        let swing = battle::Swing {
            range: self.natural_range_of(player),
            ..Default::default()
        };
        LevelSnapshot {
            level: self.world.get::<Experience>(player).map_or(1, |e| e.level),
            max_hp: self.world.get::<Stats>(player).map_or(0, |s| s.max_hp),
            mitigation: self.effective_mitigation(player),
            perk_points: self.world.get::<Perks>(player).map_or(0, |p| p.points),
            stat_points: self
                .world
                .get::<crate::components::StatPoints>(player)
                .map_or(0, |p| p.0),
            combatant: self.combatant_profile(player, swing),
        }
    }

    /// The level-up page's typical opponent — `Game::worn_detail`'s
    /// `NominalHostile` median (`balance_sim::median_ordinary_species`) at
    /// the current `ZoneLevel`, given the full stat block
    /// `battle::expected_damage` needs rather than just an evasion figure.
    /// Its `Combatant` and effective HP, since `take_level_up_report` reads
    /// both more than once and the foe is the same in both the "before" and
    /// "after" columns — only the player moves.
    ///
    /// A projection, not a spawn guarantee: it is not filtered to what can
    /// actually spawn in the current biome, `NominalHostile`'s own reason
    /// (that would fork `Game::habitat_pools`).
    pub(crate) fn typical_foe(&self) -> (battle::Combatant, f64) {
        let zone = self.world.resource::<ZoneLevel>().0;
        let median =
            crate::balance_sim::median_ordinary_species(self.world.resource::<SpeciesDb>());
        let wild = crate::balance_sim::wild_stats_at_zone(median, zone);
        let underived = crate::components::Derived::default();
        let combatant = battle::Combatant {
            accuracy: battle::accuracy_of(median.base_speed, zone, 0),
            evasion: battle::evasion_of(median.base_speed, zone, 0),
            atk: wild.atk,
            range: median.natural_range(),
            crit: underived.crit,
            fumble: underived.fumble,
        };
        (
            combatant,
            battle::effective_hp(wild.max_hp, wild.mitigation),
        )
    }

    /// Drains the pending level-up, if any — `Game::take_notification`'s
    /// shape, and the app-core hook's door onto it. Builds the report from
    /// the stored "before" snapshot and the player's state *now*, so an
    /// "after" column read on the frame the page opens reflects whatever
    /// happened between the level and the read (a Perk spent, gear
    /// swapped) rather than a second snapshot taken at the level itself.
    pub fn take_level_up_report(&mut self) -> Option<LevelUpReport> {
        let snapshot = self.world.resource_mut::<PendingLevelUp>().0.take()?;
        let player = self.player_entity();
        let now = self.snapshot_player(player);
        let (foe, foe_ehp) = self.typical_foe();

        let duel = duel_comparison(
            &snapshot,
            &now,
            foe,
            foe_ehp,
            self.world.resource::<ZoneLevel>().0,
        );

        Some(LevelUpReport {
            from_level: snapshot.level,
            to_level: now.level,
            stats: stat_rows(&snapshot, &now),
            duel,
            // `current − snapshot`, not `PERK_POINTS_PER_LEVEL * levels` —
            // correction 6. An
            // overflow point banked by the same award that levelled is then
            // counted too, which is correct: it really was earned between
            // the two snapshots.
            perk_points_gained: now.perk_points.saturating_sub(snapshot.perk_points),
            perk_points_unspent: now.perk_points,
            stat_points_gained: now.stat_points.saturating_sub(snapshot.stat_points),
            stat_points_unspent: now.stat_points,
        })
    }
}
