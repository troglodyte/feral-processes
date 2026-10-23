use super::*;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    /// Damage only. The to-hit roll comes from speed on both sides — see
    /// `battle::accuracy_of`. Feeding this to-hit as well would compound
    /// quadratically and move every `balance_sim` curve.
    pub atk: i32,
    /// **Percentage points**, not subtractive absorption. Innate plus
    /// whatever gear `Game::apply_equipment_delta` has baked in; buffs and
    /// the cap are applied on top by `Game::effective_mitigation`.
    ///
    /// **Never scaled by level or zone.** A percentage that grows per level
    /// approaches immunity, so `progression::stats_after_levels` and
    /// `ZoneLevel::stat_multiplier` both leave it alone. Levelling buys HP,
    /// attack, accuracy and evasion; mitigation comes from gear and from
    /// what a species innately is. This is the rule that keeps the
    /// percentage form safe, and the one most likely to be "corrected" by
    /// someone restoring symmetry with the other stats.
    pub mitigation: i32,
}

impl Stats {
    pub fn hp_fraction(&self) -> f32 {
        if self.max_hp <= 0 {
            0.0
        } else {
            (self.hp as f32 / self.max_hp as f32).clamp(0.0, 1.0)
        }
    }

    /// A rough "how strong is this" scalar — effective HP plus attack.
    ///
    /// Used by gear ratings, trade valuation, the roster's figures and the
    /// jack-out odds. **Not** how outmatched the player is by one program:
    /// at HP ten times the size of attack this sum measures HP alone, which
    /// is why the con colours, kill XP and capture odds read
    /// `battle::threat_ratio` instead. Mitigation is priced as the soak it
    /// buys, `battle::effective_hp`.
    pub fn power(&self) -> i32 {
        crate::battle::effective_hp(self.max_hp, self.mitigation).round() as i32 + self.atk
    }
}

/// The full end of a reserve's range. Lives here beside `PowerReserve`
/// rather than in `tuning/`, because it is the type's own documented
/// invariant rather than a difficulty knob.
pub const POWER_MAX: f32 = 100.0;

/// The empty end. Below it Power is meaningless, and its readers (the status
/// bars, `battle::power_attack_multiplier`) assume it holds.
pub const POWER_MIN: f32 = 0.0;

/// What a combatant has left to spend on routine calls, `POWER_MIN..=
/// POWER_MAX`. One meter, since the Fatigue half was deleted — that one
/// refilled on its own and was spent by two routines, while Power is the only
/// thing that kills by attrition and the budget every routine call draws on.
///
/// **The float is private, and that is the point of the type.** The range was
/// an invariant held by convention across a dozen sites, each hand-rolling
/// its own `.max(POWER_MIN)` or `.min(POWER_MAX)` — one forgotten clamp and a
/// reserve reads negative or overfull to `power_attack_multiplier` and every
/// status bar. A private field makes the compiler hold it instead, the same
/// move as `Game`'s private `world`.
///
/// The API is exactly the operations the call sites perform and nothing
/// speculative. A caller wanting an eighth is a signal to re-read the call
/// site, not to widen the type.
#[derive(Component, Clone, Copy, Debug)]
pub struct PowerReserve(f32);

impl Default for PowerReserve {
    fn default() -> Self {
        Self(POWER_MAX)
    }
}

impl PowerReserve {
    /// Clamps, because both callers are load paths: a save file and a mod's
    /// numbers are equally outside this crate's control.
    pub fn new(value: f32, max: f32) -> Self {
        Self(value.clamp(POWER_MIN, max))
    }

    pub fn get(&self) -> f32 {
        self.0
    }

    /// Whether `cost` is affordable. `>=`, so a reserve holding exactly the
    /// cost may spend it — the refusal is for a reserve that would go
    /// negative, not one that lands on empty.
    pub fn holds(&self, cost: f32) -> bool {
        self.0 >= cost
    }

    pub fn spend(&mut self, cost: f32) {
        self.0 = (self.0 - cost).max(POWER_MIN);
    }

    /// Adds, capped at `max`. `restore(0.0, max)` is also how a lowered
    /// maximum trims the reserve (`Game::recompute_derived`): a raised one
    /// leaves it where it was, and neither ever refills.
    pub fn restore(&mut self, amount: f32, max: f32) {
        self.0 = (self.0 + amount).min(max);
    }

    /// `Game::rest`, which sets outright rather than adding.
    pub fn fill(&mut self, max: f32) {
        self.0 = max;
    }

    /// `difficulty.rs`'s Forgiving reboot — the one site that raises *to* a
    /// floor rather than adding. Never lowers: a reboot is meant to leave you
    /// with enough to keep going, and a player who died holding more than the
    /// floor does not get docked for it. Delete this if that call ever
    /// becomes an additive top-up.
    pub fn raise_to_at_least(&mut self, floor: f32, max: f32) {
        self.0 = self.0.max(floor).min(max);
    }
}

/// An entity's maximum Power from its (optional) `Derived`: the one rule
/// `Game::max_power` and every system without a `Game` share. No `Derived`
/// means not derived, so the fallback is `POWER_MAX`.
pub fn max_power_of(derived: Option<&Derived>) -> f32 {
    derived.map_or(POWER_MAX, |d| d.max_power)
}

#[cfg(test)]
mod power_reserve_tests {
    use super::*;

    #[test]
    fn spend_floors_at_empty_rather_than_going_negative() {
        let mut r = PowerReserve::new(5.0, POWER_MAX);
        r.spend(50.0);
        assert_eq!(r.get(), POWER_MIN);
    }

    #[test]
    fn restore_caps_at_the_given_maximum() {
        let mut r = PowerReserve::new(POWER_MAX - 1.0, POWER_MAX);
        r.restore(50.0, POWER_MAX);
        assert_eq!(r.get(), POWER_MAX);

        let mut wide = PowerReserve::new(POWER_MAX, 140.0);
        wide.restore(50.0, 140.0);
        assert_eq!(wide.get(), 140.0);
    }

    #[test]
    fn restore_of_nothing_trims_a_reserve_above_a_lowered_maximum() {
        let mut r = PowerReserve::new(120.0, 140.0);
        r.restore(0.0, 80.0);
        assert_eq!(r.get(), 80.0);
        r.restore(0.0, 200.0);
        assert_eq!(r.get(), 80.0, "a raised maximum never refills");
    }

    #[test]
    fn new_clamps_a_wild_input_at_both_ends() {
        assert_eq!(PowerReserve::new(-40.0, POWER_MAX).get(), POWER_MIN);
        assert_eq!(PowerReserve::new(4000.0, POWER_MAX).get(), POWER_MAX);
        assert_eq!(PowerReserve::new(4000.0, 130.0).get(), 130.0);
    }

    /// The boundary is where a refusal and a charge would disagree if they
    /// were written twice, so it is pinned on both sides of exact.
    #[test]
    fn holds_is_true_at_exactly_the_cost_and_false_one_short() {
        let r = PowerReserve::new(10.0, POWER_MAX);
        assert!(r.holds(10.0), "a reserve holding exactly the cost may pay");
        assert!(!r.holds(10.1));
        assert!(r.holds(9.9));
    }

    /// The bug the Forgiving reboot would otherwise ship: a player who died
    /// with a full reserve being *dropped* to the floor by the thing meant to
    /// help them.
    #[test]
    fn raise_to_at_least_never_lowers_a_reserve_already_above_the_floor() {
        let mut r = PowerReserve::new(90.0, POWER_MAX);
        r.raise_to_at_least(40.0, POWER_MAX);
        assert_eq!(r.get(), 90.0);

        let mut drained = PowerReserve::new(5.0, POWER_MAX);
        drained.raise_to_at_least(40.0, POWER_MAX);
        assert_eq!(drained.get(), 40.0);
    }

    #[test]
    fn fill_sets_outright_to_the_given_maximum() {
        let mut r = PowerReserve::new(1.0, POWER_MAX);
        r.fill(POWER_MAX);
        assert_eq!(r.get(), POWER_MAX);
        r.fill(130.0);
        assert_eq!(r.get(), 130.0);
    }

    #[test]
    fn max_power_of_falls_back_to_the_constant() {
        assert_eq!(max_power_of(None), POWER_MAX);
        let d = Derived {
            max_power: 120.0,
            ..Derived::default()
        };
        assert_eq!(max_power_of(Some(&d)), 120.0);
    }
}

/// Tracks level/XP for the player and any tamed creature. Wild (untamed)
/// creatures don't carry this — they don't level until compiled.
#[derive(Component, Clone, Copy, Debug)]
pub struct Experience {
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            level: 1,
            xp: 0,
            // Called, not copied. This was a literal 20 that silently agreed
            // with `XP_PER_LEVEL_STEP` until the step moved, at which point
            // every entity in the game — the player included — would have
            // bought its first level at a quarter price.
            xp_to_next: crate::progression::xp_for_level(1),
        }
    }
}

/// An individual creature's innate quality roll, set once when it's
/// created (see `Game::spawn_wild_creature` / `Game::fuse_companions`)
/// and carried for its lifetime. `hp_roll`/`atk_roll`/`def_roll` are baked
/// into its starting `Stats` at creation time — this component doesn't
/// reapply them later, it just remembers what was rolled so
/// `quality_percent`/`quality_label` can describe it. `growth_roll`
/// actively scales `progression::add_xp`'s growth on every level-up, on
/// top of `SpeciesDef::growth_multiplier`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Potential {
    pub hp_roll: f32,
    pub atk_roll: f32,
    pub def_roll: f32,
    pub growth_roll: f32,
    /// How well this individual assembles a machine that compiles things —
    /// read at exactly one moment, the tick a build it was spent on
    /// finishes, and never applied to the program itself. Deliberately
    /// independent of the four combat rolls above and excluded from
    /// `quality_percent`: an Excellent fighter that builds badly is the
    /// tension the feature exists for.
    pub assembly_roll: f32,
    /// The same for a machine that extracts from the ground. Which of the
    /// two a build reads is decided once, in
    /// `game::base::building::build_quality`.
    pub extraction_roll: f32,
}

impl Potential {
    /// Fallback for an entity with no roll of its own (e.g. a legacy save,
    /// or a test helper that spawns a creature directly) — every roll at
    /// its neutral 1.0, contributing neither a bonus nor a penalty.
    pub const NEUTRAL: Potential = Potential {
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
        assembly_roll: 1.0,
        extraction_roll: 1.0,
    };

    /// A single 0-100 "how good is this individual" percentile: averages
    /// the four *combat* rolls and maps
    /// `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL` onto 0-100. Purely a
    /// display aggregate — each roll still applies independently to its own
    /// stat/growth. A bred roll past `MAX_INDIVIDUAL_ROLL` reads above 100,
    /// which is the point: it is the gain breeding made. The two build rolls
    /// are deliberately not folded in: they say nothing about how this
    /// individual fights.
    pub fn quality_percent(&self) -> u32 {
        let avg = (self.hp_roll + self.atk_roll + self.def_roll + self.growth_roll) / 4.0;
        let pct = (avg - MIN_INDIVIDUAL_ROLL) / (MAX_INDIVIDUAL_ROLL - MIN_INDIVIDUAL_ROLL) * 100.0;
        pct.round().max(0.0) as u32
    }

    /// A coarse, human-readable tier for `quality_percent` — shown next to
    /// a creature in the pets and inspect screens.
    pub fn quality_label(&self) -> &'static str {
        Self::QUALITY_LABELS[self.quality_rung()]
    }

    /// The rung one individual roll sits on, over the same five-name ladder
    /// `quality_label` speaks and the same
    /// `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL` → 0-100 mapping
    /// `quality_percent` uses. One ladder, so a build roll and an overall
    /// tier can never disagree about what "Excellent" means.
    pub fn roll_label(roll: f32) -> &'static str {
        Self::QUALITY_LABELS[Self::roll_rung(roll)]
    }

    /// The five names `roll_label` speaks, worst first.
    const QUALITY_LABELS: [&'static str; 5] = [
        "Poor",
        "Below Average",
        "Average",
        "Above Average",
        "Excellent",
    ];

    /// Which of the five `QUALITY_LABELS` rungs `roll` sits on, Poor at 0.
    fn roll_rung(roll: f32) -> usize {
        let pct =
            (roll - MIN_INDIVIDUAL_ROLL) / (MAX_INDIVIDUAL_ROLL - MIN_INDIVIDUAL_ROLL) * 100.0;
        (pct.round().max(0.0) as usize / 20).min(Self::QUALITY_LABELS.len() - 1)
    }

    /// The rung `quality_label` names, as an index — what
    /// `abilities::companion_routine_slot_bonus` reads, so a slot bonus and
    /// the label on the pets screen cannot disagree about "Excellent".
    pub fn quality_rung(&self) -> usize {
        Self::roll_rung((self.hp_roll + self.atk_roll + self.def_roll + self.growth_roll) / 4.0)
    }

    /// Averages two parents' rolls into one — used when fusing two
    /// companions into one (`Game::fuse_companions`), so the result's
    /// quality reflects both parents rather than an independent fresh
    /// roll.
    pub fn averaged(a: Potential, b: Potential) -> Potential {
        Potential {
            hp_roll: (a.hp_roll + b.hp_roll) / 2.0,
            atk_roll: (a.atk_roll + b.atk_roll) / 2.0,
            def_roll: (a.def_roll + b.def_roll) / 2.0,
            growth_roll: (a.growth_roll + b.growth_roll) / 2.0,
            assembly_roll: (a.assembly_roll + b.assembly_roll) / 2.0,
            extraction_roll: (a.extraction_roll + b.extraction_roll) / 2.0,
        }
    }
}

/// What a body is like apart from what it can do in a fight — see
/// `attributes.rs`.
///
/// **A component of its own and pointedly not a field on `Stats`.** `Stats`
/// is level-scaled by `progression::stats_after_levels`, zone-multiplied by
/// `ZoneLevel::stat_multiplier`, folded into by
/// `Game::apply_equipment_delta` and summed by `Stats::power()`, which
/// `Game::difficulty_color`, `progression::kill_xp` and trade valuation all
/// read. A sixth field there would enter every one of those whether it was
/// wanted or not, and `balance_sim`'s empirical curves would move. Being
/// separate is also what keeps "nothing reads an attribute" true by
/// construction: nothing that reads `Stats` can start reading one by
/// accident.
///
/// **Minted once and stored**, never re-derived on read: the value is a
/// property of the body and the place it came from, and `Disposition`'s own
/// argument applies — a fold that ran on every read would be free to
/// disagree with the one the save recorded.
///
/// `BTreeMap` for `Stock`'s reason: iteration order feeds the save
/// encoding, and a `HashMap` would make the file differ run to run. An
/// entry whose def no file defines is kept and skipped by every reader,
/// exactly as a `Memories` entry is — which is what makes a catalogue
/// edited between sessions a supported thing to do.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Attributes(std::collections::BTreeMap<crate::attributes::AttributeId, i32>);

impl Attributes {
    pub fn get(&self, id: &crate::attributes::AttributeId) -> Option<i32> {
        self.0.get(id).copied()
    }

    pub fn set(&mut self, id: &crate::attributes::AttributeId, value: i32) {
        self.0.insert(id.clone(), value);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&crate::attributes::AttributeId, i32)> {
        self.0.iter().map(|(id, v)| (id, *v))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The player's derived figures that have no `Stats` field: what
/// `progression::derive` answers beyond hp, attack, mitigation and
/// Decompiler. Written only by `Game::recompute_derived`, and its absence
/// means "not derived" - a companion reads the defaults (`POWER_MAX`, no
/// resist, no extraction, the flat `CRIT_CHANCE` and `FUMBLE_CHANCE`).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Derived {
    pub max_power: f32,
    /// Percent shortening of an armed status; negative lengthens.
    pub status_resist: i32,
    /// The player's capped extra mining chance.
    pub extraction: f32,
    /// The crit band `Game::combatant_profile` hands to the roll.
    pub crit: f64,
    /// The fumble band `Game::combatant_profile` hands to the roll.
    pub fumble: f64,
    /// Tiles of surface sight; only the player's is read.
    pub perception: f32,
}

impl Default for Derived {
    fn default() -> Self {
        Derived {
            max_power: POWER_MAX,
            status_resist: 0,
            extraction: 0.0,
            crit: crate::tuning::CRIT_CHANCE,
            fumble: crate::tuning::FUMBLE_CHANCE,
            perception: crate::tuning::PERCEPTION_BASE_RADIUS,
        }
    }
}

/// Attribute points earned by levelling and not yet spent. Banked by
/// `award_player_xp`, spent by `Game::spend_stat_points`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatPoints(pub u32);

/// The figures a seated program's attributes derive from: what `Stats` would
/// be with every attribute at its catalogue base. Set once by
/// `Game::seat_derived`, read through `Game::derived_base`. Its absence means
/// the program is not derived (a wild one, or an old save before it is
/// seated).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ProgramBase(pub crate::progression::DerivedBase);

/// Whether a derived program's unspent attribute points are held for the
/// player to spend rather than spent for it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HoldPoints(pub bool);

/// How many fusions deep a creature's lineage is (see
/// `Game::fuse_companions`). A creature that was caught or spawned
/// normally has no such component at all, which reads as 0; a fusion's
/// result carries `max(parent_a, parent_b) + 1`, so the number is the
/// *depth* of the deepest chain behind it, not a count of ancestors.
///
/// Once it hits `MAX_FUSIONS` that creature is a finished product: it
/// can't be used as an input to another fusion, which stops a player from
/// laundering an endless supply of duplicates into one runaway program.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FusionCount(pub u32);

/// How many percentage upgrades have been spent on this program — see
/// `Game::refactor_companion`. Absent means zero, the same way `FusionCount`
/// is absent on a program that has never been fused.
///
/// It counts **only** the percentage buffs. A Recompile Kernel raises
/// `ZonePortal` instead and spends nothing here, because that track bounds
/// itself against the player's own zone and a player forced to burn permanent
/// slots just staying current has had the feature taken away at exactly the
/// point it was meant to help.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refactors(pub u32);

/// How many of this program's zone tiers were **bought** with Recompile
/// Kernels rather than earned by being tamed that deep. Absent means none,
/// like `Refactors` above.
///
/// It exists for one reason: `Game::program_payout` pays a fraction of
/// `Stats::power()`, and a kernel multiplies every one of those stats for
/// twelve printable Core Fragments. Left unrecorded, buying tiers and selling
/// the program prints Credits — measured at zone 7, a zone-1 program bought
/// up six tiers sold for 716 against 72 fragments' worth of kernels, and
/// Credits are the one currency that survives a breach. So the sale divides
/// the bought tiers back out, and a trader pays for what a program *is*.
///
/// `ZonePortal` cannot answer this on its own: a tier-4 program is worth
/// tier-4 money when it was tamed in zone 4, and this is what tells those two
/// apart. The percentage buffs deliberately are *not* divided out — five
/// slots is at most a 1.28x on power, which never repays the annealed cores
/// it costs, so there is nothing to close.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PurchasedTiers(pub u32);

/// How many Kernel Rings have been opened on this program — see
/// `Game::open_kernel_ring`. Absent means zero, like `Refactors` and
/// `PurchasedTiers` above.
///
/// Each ring raises this companion's level ceiling by
/// `tuning::LEVELS_PER_RING` and nothing else: it buys *room* to grow, never
/// growth itself. `Game::companion_level_cap` is the one expression of what
/// it is worth, so no call site adds the multiplication itself.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KernelRing(pub u32);

/// Which talent nodes this program has bought, in the order it bought them —
/// see `Game::take_talent`. Absent means none, like `KernelRing` above.
///
/// It is a **receipt**, not a source of truth for anything numeric. A `Stat`
/// node is baked into `Stats` at purchase exactly as a refactor is, so nothing
/// may re-apply this list on load; the other three kinds are read on demand
/// off it. Its length is also the whole of "how many points are spent" — there
/// is no stored count, because a count can desync from the list and from the
/// level while a derivation cannot.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct Talents(pub Vec<crate::talents::TalentId>);

/// Player-only: accumulated Perk Points (earned 1 per level-up) and which
/// perks have been bought with them. See `perks::Perk` — a perk can be
/// bought more than once, so `unlocked` holds one entry per level bought
/// (duplicates allowed) rather than a unique set.
#[derive(Component, Default, Clone, Debug)]
pub struct Perks {
    pub points: u32,
    pub unlocked: Vec<Perk>,
}

impl Perks {
    /// How many levels of `perk` have been bought — 0 if none.
    pub fn level(&self, perk: Perk) -> u32 {
        self.unlocked.iter().filter(|&&p| p == perk).count() as u32
    }
}

/// Every stat point on this creature that came from a spendable choice — a
/// perk level or a `TalentNode::Stat` — and can therefore be handed back by
/// `Game::respec_perks` / `Game::respec_talents`.
///
/// The receipt exists because neither grant is invertible from `Stats`.
/// `Perk::Buffer` and `TalentNode::Stat` are percentages of a maximum that
/// has since moved, and both floor at a whole point, so reversing the
/// arithmetic is off by a point per level in the common case — and a retune
/// of either constant would silently change what an old save's respec hands
/// back. Recording what was granted is the only thing that stays exact.
///
/// One component serves both ledgers: the player has no `Talents` and a
/// companion has no `Perks`, so the two can never write it at once.
///
/// Absent means nothing bought, like `KernelRing` and `Refactors`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoughtStats {
    pub atk: i32,
    pub mitigation: i32,
    pub max_hp: i32,
    /// How many perk levels the player has **ever** bought, which a respec
    /// deliberately does not reset.
    ///
    /// `Game::convert_overflow_xp` prices each minted Perk Point off this
    /// rather than off `Perks::unlocked`, because that list empties on a
    /// respec and the escalator it feeds is the only thing keeping banked
    /// cap XP from being an unbounded power source. Read off the list, a
    /// respec resets the price to the opening rate and the loop is: buy,
    /// wipe, mint the rest cheap.
    ///
    /// Perk purchases only. A companion's talents have no bearing on the
    /// player's XP drain, and counting them here would be exactly the drift
    /// a shared field invites.
    pub ever_bought: u32,
}

#[cfg(test)]
mod potential_tests {
    use super::Potential;
    use crate::tuning::{MAX_INDIVIDUAL_ROLL, MIN_INDIVIDUAL_ROLL};

    /// All six rolls at `r`.
    fn flat(r: f32) -> Potential {
        Potential {
            hp_roll: r,
            atk_roll: r,
            def_roll: r,
            growth_roll: r,
            assembly_roll: r,
            extraction_roll: r,
        }
    }

    #[test]
    fn quality_percent_maps_the_roll_range_onto_0_to_100() {
        assert_eq!(flat(0.8).quality_percent(), 0);
        assert_eq!(Potential::NEUTRAL.quality_percent(), 50);
        assert_eq!(flat(1.2).quality_percent(), 100);
    }

    /// A bred roll can pass `MAX_INDIVIDUAL_ROLL`, and reads above 100%
    /// rather than hiding the gain; every roll a wild spawn can make reads
    /// exactly as the clamped mapping always did.
    #[test]
    fn a_bred_roll_reads_above_100_and_a_wild_roll_never_does() {
        assert_eq!(flat(1.248).quality_percent(), 112);
        assert_eq!(flat(1.28).quality_percent(), 120);
        assert_eq!(flat(1.28).quality_label(), "Excellent");
        for step in 0..=200 {
            let r = MIN_INDIVIDUAL_ROLL
                + (MAX_INDIVIDUAL_ROLL - MIN_INDIVIDUAL_ROLL) * (step as f32 / 200.0);
            let old = ((r - MIN_INDIVIDUAL_ROLL) / (MAX_INDIVIDUAL_ROLL - MIN_INDIVIDUAL_ROLL)
                * 100.0)
                .round()
                .clamp(0.0, 100.0) as u32;
            assert_eq!(flat(r).quality_percent(), old, "wild roll {r} moved");
        }
    }

    #[test]
    fn quality_label_buckets_the_percent_into_a_coarse_tier() {
        assert_eq!(flat(0.8).quality_label(), "Poor");
        assert_eq!(Potential::NEUTRAL.quality_label(), "Average");
        assert_eq!(flat(1.2).quality_label(), "Excellent");
    }

    #[test]
    fn roll_label_walks_the_same_five_rungs_quality_label_does() {
        assert_eq!(Potential::roll_label(MIN_INDIVIDUAL_ROLL), "Poor");
        assert_eq!(Potential::roll_label(MAX_INDIVIDUAL_ROLL), "Excellent");
        // One ladder: a flat `Potential` reports whatever its own roll does.
        for step in 0..=10 {
            let r = MIN_INDIVIDUAL_ROLL
                + (MAX_INDIVIDUAL_ROLL - MIN_INDIVIDUAL_ROLL) * (step as f32 / 10.0);
            assert_eq!(
                flat(r).quality_label(),
                Potential::roll_label(r),
                "the two ladders disagree at {r}"
            );
        }
    }

    #[test]
    fn neutral_is_neutral_on_every_axis() {
        let n = Potential::NEUTRAL;
        assert_eq!(
            (
                n.hp_roll,
                n.atk_roll,
                n.def_roll,
                n.growth_roll,
                n.assembly_roll,
                n.extraction_roll
            ),
            (1.0, 1.0, 1.0, 1.0, 1.0, 1.0)
        );
    }

    #[test]
    fn quality_percent_still_ignores_the_build_rolls() {
        let fighter_who_cannot_build = Potential {
            hp_roll: MAX_INDIVIDUAL_ROLL,
            atk_roll: MAX_INDIVIDUAL_ROLL,
            def_roll: MAX_INDIVIDUAL_ROLL,
            growth_roll: MAX_INDIVIDUAL_ROLL,
            assembly_roll: MIN_INDIVIDUAL_ROLL,
            extraction_roll: MIN_INDIVIDUAL_ROLL,
        };
        assert_eq!(fighter_who_cannot_build.quality_percent(), 100);
        assert_eq!(fighter_who_cannot_build.quality_label(), "Excellent");
    }

    #[test]
    fn averaged_splits_the_difference_between_two_parents() {
        let a = Potential {
            hp_roll: 0.8,
            atk_roll: 1.0,
            def_roll: 1.2,
            growth_roll: 0.9,
            assembly_roll: 1.1,
            extraction_roll: 0.85,
        };
        let b = Potential {
            hp_roll: 1.2,
            atk_roll: 1.0,
            def_roll: 0.8,
            growth_roll: 1.1,
            assembly_roll: 0.9,
            extraction_roll: 1.15,
        };
        let fused = Potential::averaged(a, b);
        assert_eq!(fused.hp_roll, 1.0);
        assert_eq!(fused.atk_roll, 1.0);
        assert_eq!(fused.def_roll, 1.0);
        assert_eq!(fused.growth_roll, 1.0);
        assert_eq!(fused.assembly_roll, 1.0);
        assert_eq!(fused.extraction_roll, 1.0);
    }

    #[test]
    fn averaged_folds_all_six_rolls() {
        // Six distinct values a side, none repeated anywhere: two fields
        // holding the same number cannot catch being crossed.
        let a = Potential {
            hp_roll: 0.81,
            atk_roll: 0.83,
            def_roll: 0.85,
            growth_roll: 0.87,
            assembly_roll: 1.19,
            extraction_roll: 0.89,
        };
        let b = Potential {
            hp_roll: 0.91,
            atk_roll: 0.93,
            def_roll: 0.95,
            growth_roll: 0.97,
            assembly_roll: 1.17,
            extraction_roll: 0.99,
        };
        let f = Potential::averaged(a, b);
        for (got, want) in [
            (f.hp_roll, 0.86),
            (f.atk_roll, 0.88),
            (f.def_roll, 0.90),
            (f.growth_roll, 0.92),
            (f.assembly_roll, 1.18),
            (f.extraction_roll, 0.94),
        ] {
            assert!((got - want).abs() < 1e-5, "{got} != {want}");
        }
        // Two Excellent builders fuse into an Excellent builder.
        assert_eq!(Potential::roll_label(f.assembly_roll), "Excellent");
    }
}

#[cfg(test)]
mod stats_tests {
    use super::*;
    use crate::tuning::MAX_MITIGATION_PERCENT;

    #[test]
    fn power_prices_mitigation_as_effective_hp() {
        // 100 HP behind 50% mitigation is worth 200 HP of soak.
        let soft = Stats {
            hp: 100,
            max_hp: 100,
            atk: 10,
            mitigation: 0,
        };
        let armoured = Stats {
            mitigation: 50,
            ..soft
        };
        assert_eq!(soft.power(), 110);
        assert_eq!(armoured.power(), 210);
    }

    #[test]
    fn power_cannot_divide_by_zero_at_the_mitigation_cap() {
        // MAX_MITIGATION_PERCENT is capped strictly below 100, and that cap
        // is load-bearing here as well as in the damage path.
        let capped = Stats {
            hp: 100,
            max_hp: 100,
            atk: 0,
            mitigation: MAX_MITIGATION_PERCENT,
        };
        assert!(capped.power() > 0 && capped.power() < 100_000);
    }

    #[test]
    fn power_clamps_a_mitigation_beyond_the_cap() {
        // A save, a mod affix or a stacked buff can hand this a raw number
        // past the cap; `power` must not go negative or infinite on one.
        let overcapped = Stats {
            hp: 100,
            max_hp: 100,
            atk: 0,
            mitigation: 400,
        };
        let capped = Stats {
            mitigation: MAX_MITIGATION_PERCENT,
            ..overcapped
        };
        assert_eq!(overcapped.power(), capped.power());
    }

    #[test]
    fn power_ignores_current_hp() {
        let hurt = Stats {
            hp: 1,
            max_hp: 100,
            atk: 10,
            mitigation: 0,
        };
        let whole = Stats { hp: 100, ..hurt };
        assert_eq!(hurt.power(), whole.power());
    }
}
