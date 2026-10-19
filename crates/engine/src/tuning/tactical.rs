// ─────────────────────────────────────────────────────────────────────────
// Tactical battle grid
// ─────────────────────────────────────────────────────────────────────────

/// What crossing a `BattleCell::Rough` cell costs, against `Open`'s one.
///
/// Two rather than three because a body's whole allowance is small: at
/// three, one rough cell eats most of a turn and the terrain stops being a
/// choice and becomes a wall with extra steps.
pub const TACTICAL_ROUGH_COST: u32 = 2;

/// How many authored prefabs a board of a given side is stamped with, as
/// `(side, min, max)`. A board of a side not listed takes the last row.
///
/// Scaled with the board's area, so a small board gets one or two set pieces
/// and a large one reads as a place rather than a plain with a prop on it.
pub const TACTICAL_PREFABS_BY_SIDE: [(i32, u32, u32); 3] = [
    (TACTICAL_BOARD_SMALL, 1, 2),
    (TACTICAL_BOARD_MEDIUM, 2, 4),
    (TACTICAL_BOARD_LARGE, 4, 6),
];

/// Chance in a thousand that an open cell not under a prefab carries a
/// decoration. Low: decoration is texture, and a floor that is mostly
/// debris stops reading as a floor.
pub const TACTICAL_DECOR_PER_MILLE: u32 = 25;

/// Placements tried per prefab before it is dropped from the board. A prefab
/// that will not fit after this many is not worth a long search; the board is
/// pure in its spec, so dropping it is as deterministic as placing it.
pub const TACTICAL_PREFAB_ATTEMPTS: u32 = 12;

/// How deep a chain of volatile props may detonate one another.
///
/// A line of explosive cells is a fuse; unbounded, one swing would resolve
/// the whole board in a single call. Three keeps a cluster dramatic and a
/// long row finite.
pub const TACTICAL_PROP_CHAIN_MAX: u32 = 3;

/// The three board extents, in cells on a side.
///
/// Three fixed tiers rather than a per-body formula so the player learns
/// their shapes: a board is a place you fight in repeatedly, and one that
/// is a slightly different size every time is one you can never read at a
/// glance. Sized for manoeuvre — deployment is what puts the two sides in
/// contact, so a board large enough to flank on does not open every fight
/// with a walk.
pub const TACTICAL_BOARD_SMALL: i32 = 14;
pub const TACTICAL_BOARD_MEDIUM: i32 = 20;
pub const TACTICAL_BOARD_LARGE: i32 = 28;

/// Total bodies — party plus wild — at which the board steps up a tier.
///
/// Both are reachable and so is the tier between them: the smallest fight
/// the game fields is the player and one wild body, and the largest is
/// `MAX_PARTY_SIZE` against `MAX_PACK_BODIES`.
pub const TACTICAL_MEDIUM_BODIES: u32 = 4;
pub const TACTICAL_LARGE_BODIES: u32 = 7;

/// Cells between the two sides' anchors at deployment.
///
/// The board is sized for manoeuvre and the deployment is sized for
/// contact: six cells is a step or two of closing rather than a march, so
/// no fight opens with both sides walking toward each other for a turn.
pub const TACTICAL_DEPLOY_GAP: i32 = 6;

/// A body of average speed's movement allowance, in cost.
///
/// Read against `TACTICAL_DEPLOY_GAP`: four points closes six cells in two
/// turns, which is the "step or two of closing" the gap is sized for, and
/// it leaves room either side for speed to be worth something — over the
/// shipped roster the band runs two to six, so the slowest body takes three
/// turns to cross what the fastest crosses in one.
pub const TACTICAL_MOVE_BASE: u32 = 4;

/// Points of `Game::combat_speed` that buy one more cell of movement.
///
/// Two, against the shipped roster's 6..14 spread and
/// `DEFAULT_BASE_SPEED`'s 10. One point per cell would triple that spread
/// and let a single stat decide a tactical fight on its own — speed already
/// buys initiative, accuracy and evasion — while three would flatten the
/// roster onto two values.
pub const TACTICAL_MOVE_SPEED_STEP: i32 = 2;

/// How far a routine that reaches one recipient may be aimed, in cells.
///
/// One, so a `Single` routine is arm's length exactly as a swing is: the
/// group model draws no distinction between a routine and a basic attack's
/// reach either, and one that could be run two cells away would make the
/// positioning this model is built on optional.
pub const TACTICAL_MELEE_RANGE: u32 = 1;

/// Levels bought per cell of a `Teleport`'s reach — see
/// `abilities::teleport_reach`, which is the formula and the floor.
///
/// Two, so the reach is half the invoker's level. Against `zone_level_cap`
/// that is three cells on arrival in sector 2 and grows to span a board by
/// the deep sectors, which is deliberate: the routine is bought once and the
/// levels are what make it worth carrying. It is **uncapped on purpose** —
/// a ceiling here would make every level past it buy nothing, which reads
/// at the keyboard exactly like the routine being broken.
///
/// Zero would divide by zero, and one makes the reach the level itself,
/// which outruns `TACTICAL_BOARD_LARGE` before a player leaves sector 3.
pub const TELEPORT_LEVELS_PER_CELL: u32 = 2;

/// The furthest a weapon may author a swing, in cells.
///
/// Three, which is half `TACTICAL_DEPLOY_GAP`: a weapon that reached the far
/// side's deployment from the near side's would make the approach this model
/// is built on optional, and a ceiling is what keeps a mod from authoring
/// one. Refused at load past this rather than clamped — a weapon quietly
/// swinging shorter than its file says reads as a nerf rather than a bad
/// value.
pub const TACTICAL_WEAPON_RANGE_MAX: u32 = 3;

/// Accuracy a weapon swung at arm's length adds to its authored `accuracy`.
///
/// The price of reach: a weapon's range replaces its wielder's own
/// (`Game::swing_range`), so a program that shoots two cells gives up the
/// standoff when it takes a blade, and this is what it gets back. Added to
/// the *base* rather than after scaling, so it grows with level, quality and
/// fusion like the rest of the weapon — flat, it would fade to noise by the
/// deep sectors. Rides the wielder's accuracy, so routines get it too.
///
/// Melee is `range` absent or `1`, read by `ItemDef::worn_stats` alone.
pub const MELEE_WEAPON_ACCURACY: i32 = 2;

/// See `MELEE_WEAPON_ACCURACY`. Zero: range is its own reward.
pub const RANGED_WEAPON_ACCURACY: i32 = 0;

/// How far a species' `ranged` basic attack reaches on a battle map.
///
/// Two, and the number is load-bearing. Every shipped species carries
/// exactly two moves and fourteen of seventeen carry exactly one
/// `ranged: true` — only Construct, Scrapper and Sentinel are pure melee.
/// At three, nearly every wild body on the board becomes a shooter and the
/// closing this whole model is built on stops mattering. Two is one cell of
/// standoff: real, and closed by a single step.
///
/// Not to be confused with `MoveDef::ranged`'s *group*-model meaning, which
/// is a yes-or-no about the front line and is untouched by this.
pub const TACTICAL_RANGED_MOVE_RANGE: u32 = 2;

/// How far a routine that reaches a whole side may be aimed, in cells.
///
/// Six, which is `TACTICAL_DEPLOY_GAP`: an area routine reaches the far
/// side's deployment from the near side's, so it is worth running on the
/// first turn of a fight and does not need a march to bring it into play.
pub const TACTICAL_THROWN_RANGE: u32 = 6;

/// What a routine derives for a radius when its file authored no shape —
/// one per side-facing `AbilityTarget`, in cells.
///
/// **The three are not one constant.** What the group model means by "one
/// group" is a handful of bodies standing together and what it means by
/// "everything" is the field, so a single figure would either make a group
/// routine hit the board or a field routine hit two cells. The party's own
/// is the widest of the three because a party is spread by the player's own
/// movement rather than by deployment, and a rally that reached only the
/// bodies pressed against the invoker would never land on the companion
/// that needed it.
pub const TACTICAL_GROUP_RADIUS: u32 = 1;
pub const TACTICAL_FIELD_RADIUS: u32 = 2;
pub const TACTICAL_PARTY_RADIUS: u32 = 3;

/// The allowance's floor and ceiling.
///
/// Not taste: `reach::movement_field` passes the allowance to `walk_field`
/// as its search radius, so an unbounded allowance is an unbounded search,
/// and a body that cannot move at all can neither close nor walk off the
/// board — a fight that cannot finish. Both ends are therefore correctness
/// bounds, and both are reachable only by a mod: the shipped roster derives
/// two through six without either clamp biting.
pub const TACTICAL_MOVE_MIN: u32 = 2;
pub const TACTICAL_MOVE_MAX: u32 = 8;

/// What a hostile is looking for when it picks the cell it will fight from.
///
/// Three terms, and they are read together rather than tuned apart. The
/// reach bonus is worth more than closing the whole width of the largest
/// board, so a cell it can actually hit from always beats a cell that is
/// merely nearer — without that ordering a body walks past the swing it
/// came for. Crowding is the smallest of the three because it is a
/// tie-breaker between cells that are otherwise as good: it should spread a
/// pack that has a choice, never talk a body out of the fight.
pub const TACTICAL_AI_REACH_SCORE: f32 = 40.0;
pub const TACTICAL_AI_CLOSING_WEIGHT: f32 = 1.0;
pub const TACTICAL_AI_CROWDING_WEIGHT: f32 = 0.5;

/// What standing behind a boulder is worth to a hostile, as a fraction of
/// the targets it would have cover against.
///
/// **Bracketed rather than tuned alone**, against the three above. Strictly
/// below `TACTICAL_AI_REACH_SCORE`, so full cover is never worth walking out
/// of reach for; well above `TACTICAL_AI_CLOSING_WEIGHT`, or nothing would
/// ever take a cell for it. The melee tie-break is smaller than crowding for
/// crowding's own reason: a body already in reach may prefer cover between
/// two cells that both reach, and may never be talked out of the fight by
/// it. Changing `COVER_EVASION_PERCENT` is not a reason to move either of
/// these — they are read against the terms beside them, not against what
/// cover is worth in the roll.
pub const TACTICAL_AI_COVER_WEIGHT: f32 = 4.0;
pub const TACTICAL_AI_COVER_TIEBREAK: f32 = 0.25;

/// What a point of expected reaction damage is worth against the three
/// terms above, when a hostile weighs a cell it would have to walk out of
/// somebody's reach to take.
///
/// One, so a step that costs an Integrity is worth exactly one step of
/// closing. Deliberately small against `TACTICAL_AI_REACH_SCORE`: a body
/// should not be talked out of the approach it came for by the swing it
/// will take on the way, only out of repositioning that buys it little.
/// Walking *toward* a body provokes nothing, so this term never slows a
/// charge — it prices disengaging and circling.
pub const TACTICAL_AI_REACTION_WEIGHT: f32 = 1.0;

/// How much a hostile's choice of cell is allowed to wander off the best
/// one, as `policy::sample_scored`'s softmax temperature.
///
/// Not `ENEMY_POLICY_TEMPERATURE`: that one divides a trained policy's
/// learned scores, and these are hand-authored on a scale of their own, so
/// sharing the constant would couple two dials that mean different things.
/// Small against `TACTICAL_AI_CLOSING_WEIGHT`, so a hostile picks among
/// cells within about a step of the best and not among all of them.
///
/// Zero is a supported setting and means argmax with no draw at all —
/// `sample_scored` short-circuits before it touches the RNG, which is what
/// lets a test pin the choice without moving the seeded stream.
pub const TACTICAL_AI_TEMPERATURE: f32 = 0.5;

// ─────────────────────────────────────────────────────────────────────────
// Tactical pack tactics
// ─────────────────────────────────────────────────────────────────────────

/// The odds of each pack tactic, keyed by the behaviour id of the pack's
/// toughest body (an id with no row rolls on `default`).
///
/// Columns are `PackTactic::ALL`'s order: `[Rush, Together, Hold]`. Integer
/// weights, so a roll is `derive::index` over their sum and nothing in it is
/// a float. `Rush` stays above zero in every row, which is what lets a roll
/// whose other tactics are all ineligible still answer.
pub const PACK_TACTIC_ODDS: [(&str, [u32; 3]); 5] = [
    ("default", [2, 2, 2]),
    ("brute", [5, 4, 1]),
    ("hunter", [5, 2, 1]),
    ("skirmisher", [2, 1, 6]),
    ("cautious", [2, 1, 6]),
];

/// Added to the fold the pack roll is drawn from, so it is independent of
/// the board drawn from the same `BattleSpec` (`BattleSpec::base_seed` ends
/// on no such word).
pub const PACK_ROLL_SALT: u64 = 0x9ac7_7ac7_1c5e_ed01;

/// How hard `Together` pulls a body back toward its packmates' centroid, per
/// step beyond `TACTICAL_COHESION_SLACK`, as a multiple of the body's *own*
/// profile closing weight.
///
/// Above one, or a step of closing would always pay for the step of straying
/// it costs and the term would never hold anyone. Relative rather than
/// absolute because profiles scale closing (a brute's is 1.5): an absolute
/// figure held a body back only by an exact tie. Far below
/// `TACTICAL_AI_REACH_SCORE`, so a body that can swing still swings.
pub const TACTICAL_COHESION_FACTOR: f32 = 1.5;

/// How far from its packmates' centroid a body may stand before `Together`
/// starts to charge it for the distance, in steps.
pub const TACTICAL_COHESION_SLACK: u32 = 4;

/// What `Hold` pays a cell for the fraction of the pack's targets it has cover
/// against.
///
/// Well above the melee cover tiebreak and crowding, so a holding body walks
/// to cover rather than shuffling among cells that tie; far below
/// `TACTICAL_AI_REACH_SCORE`, so a target that wanders into reach is still
/// swung at.
pub const TACTICAL_HOLD_COVER_BONUS: f32 = 6.0;

/// What `Hold` charges a cell from which a target is in view (within
/// `TACTICAL_HOLD_ALERT_RANGE` with line of sight): the very sighting that
/// would end the hold. Far past `TACTICAL_HOLD_COVER_BONUS`, so no cover is
/// worth walking into the party's view for.
pub const TACTICAL_HOLD_SIGHT_CHARGE: f32 = 100.0;

/// How near a non-pack body must come to a pack member, in steps, for a hold
/// to count it as sighted.
///
/// **Must stay clearly below `TACTICAL_DEPLOY_GAP`.** The two sides open that
/// far apart, so a range at the gap (it was six) saw the party on the first
/// hostile turn in about five fights in six and a hold played as `Rush`. At
/// four a real fight holds past its first turn in every seed probed (40 of
/// 40, against 6 of 40 at six).
pub const TACTICAL_HOLD_ALERT_RANGE: u32 = 4;

/// How many rounds a hold waits before giving up. Without it a player who
/// also waits stalls the fight for good.
pub const TACTICAL_HOLD_ROUNDS: u32 = 6;

// ─────────────────────────────────────────────────────────────────────────
// Tactical squads
// ─────────────────────────────────────────────────────────────────────────

/// One row of the formation table: an exact set size that folds into a
/// single enhanced body on a battle map, and what that body is worth.
///
/// A table and not a special case, so a larger formation later is another
/// row rather than a second code path — `tactical::squads::plan` walks
/// `FORMATIONS` largest-first, and every reader of a body's shape
/// (`TacticalBattle::footprint_of`, `Game::actions_per_turn`) treats the
/// *absence* of `components::Squad` as the one-cell, one-action row this
/// table does not need to spell out.
///
/// Difficulty tuning, so it lives here and not in `assets/`, per
/// CLAUDE.md's moddability rule: content is moddable, how hard the game is,
/// is not.
pub struct Formation {
    /// The exact number of same-species bodies a set of this size cuts.
    pub members: usize,
    /// The side, in cells, of the square footprint the folded body
    /// occupies.
    pub footprint: u8,
    /// Actions per turn the folded body gets — a single body always gets
    /// one, so this table is where "more than one" starts existing at all.
    pub actions: u8,
    /// The fraction of the members' summed `atk` the folded body swings
    /// with. `Game::spawn_squad` bakes this into `Stats::atk` once, at
    /// formation; `Game::effective_atk`'s `Squad` arm then scales *that*
    /// figure by the squad's own remaining Integrity, so the two rules
    /// compose rather than duplicate.
    pub swing_share: f32,
    /// The noun the engine builds a squad's name from — `"<species>
    /// squad (5)"`.
    pub noun: &'static str,
    /// The mark drawn in the footprint's corner nothing else claims.
    pub mark: char,
}

/// The shipped formation table: one row, five of a kind folding into a 2x2
/// squad with two actions a turn.
///
/// `swing_share` is expected to move once squads have been played —
/// `docs/measurements/` is the instrument, since `balance_sim` models no
/// battle maps and so gates none of this. At `0.5` and two actions, a fresh
/// squad's turn deals what its five members would deal fighting alone; the
/// remaining differences (two large hits losing less to mitigation than
/// five small ones, the damage landing on one target, no overkill wasted
/// between members) are the formation's actual edge over five loose bodies.
pub const FORMATIONS: &[Formation] = &[Formation {
    members: 5,
    footprint: 2,
    actions: 2,
    swing_share: 0.5,
    noun: "squad",
    mark: '^',
}];

// ─────────────────────────────────────────────────────────────────────────
// Battle summons
// ─────────────────────────────────────────────────────────────────────────

/// Where a forked program is spawned before either combat model seats it.
///
/// `SORTIE_SENTINEL`'s idea, not a copy of it: the two are siblings, each
/// an off-map coordinate for bodies that exist only inside a call that will
/// account for them. A fork's `Position` is read nowhere once it is in a
/// fight — the group model has no board, and the tactical model keeps its
/// own coordinates in `TacticalBattle` — so this is where one stands for
/// exactly as long as it takes to seat it, and where a body that somehow
/// outlived `finish_fight`'s sweep would be found instead of in the zone.
pub const SUMMON_SENTINEL: (i32, i32) = (1 << 21, 1 << 21);

/// How much of `spawning::rarity_mass()` each rank of `Perk::Scheduler`
/// opens as a forked program's chance of coming out above `Ordinary`.
///
/// A multiple of the wild rare rate rather than a probability of its own,
/// so a retune of the rarity ladder carries this with it. At this figure
/// the four ranks the ceiling allows land at roughly 29%, 59%, 88% and
/// saturated — which is what makes rank 4 the end of the ladder on both
/// axes at once rather than a window still opening under a fixed ceiling.
pub const SUMMON_RARITY_WINDOW_PER_RANK: f64 = 8.0;

/// The flat handicap every forked program carries, whatever it rolled.
///
/// Borrowed from `SETTLEMENT_GIFT_STAT_MULT`, the existing precedent for
/// handicapping a program the player did not earn in a fight, and **a guess
/// rather than a measurement**: `balance_sim` models no abilities and no
/// perks, so it gates none of this. The instruments are `dev-arenas/` and a
/// played session.
///
/// It is what makes an unscheduled fork worth less than a companion: at
/// `Ordinary` the whole ladder runs 0.60 through to 1.29 at `Prismatic`, so
/// even a fully invested fork is bounded — and temporary.
pub const SUMMON_STAT_MULT: f32 = 0.6;

/// How many zone steps of the player's own progress through this zone's
/// level band a forked program is worth, via `Game::zone_curve_ratio`.
///
/// One step: a fork at the top of the band is scaled as a spawn one zone
/// deeper would be, and at the bottom of it as one of this zone. Expressed
/// in zone steps rather than as a bespoke curve because that is the ladder
/// `balance_sim` already sweeps.
pub const SUMMON_LEVEL_STAT_STEPS: f32 = 1.0;

/// The multiplier a Respawn cast applies to the fallen body's own snapshot
/// `Stats` (`Game::run_tactical_routine`'s `Reanimate` branch) — a raised
/// body's `SUMMON_STAT_MULT`.
///
/// Higher than a fork's, deliberately: a raised body already earned the
/// numbers it is being handed back at a discount, where a fork's numbers
/// were never earned at all.
pub const REANIMATE_STAT_MULT: f32 = 0.75;

/// What each level of `Perk::SpawnPriority` adds to both a Respawn cast's
/// multiplier and a fork's `SUMMON_STAT_MULT`, via
/// `perks::spawn_priority_stat_bonus`.
///
/// **Uncapped, deliberately** — a raised body or a fork clearing 1.0x its
/// own fallen/live numbers is the point of buying the perk, not a bug to
/// clamp away.
pub const SPAWN_PRIORITY_STAT_PER_LEVEL: f32 = 0.10;

/// The flat floor of a synthesised routine node's cost — the Single, v1.0
/// rung at the bottom of any family, ungated (zone 1).
///
/// Fitted, not measured: `balance_sim` models no abilities, so there is no
/// simulator to check a routine research cost against, only the costs the
/// eleven routine-granting base nodes this feature moves away from used to
/// charge (todo #101), each read off the pre-feature `git show d0891e23`
/// tree. Those bundled several abilities and a zone gate into one number,
/// which this schema deliberately does not — `research_zone` carries the
/// gate now — but the first version review (todo #101) found the schema
/// still needed a **zone term of its own**: without one, `hardened_shell`
/// (zone 2) and `null_route` (zone 3) priced the same as a zone-1 rung of
/// the same scope, which reads as free difficulty. So cost is linear in
/// all three axes, and the fit below covers the gated rungs too rather
/// than only the cheapest ungated ones:
///
/// | old node | ability | scope | version | zone | old cost | new cost |
/// |---|---|---|---|---|---|---|
/// | `self_exec` | `priority_boost` | Single | v1.0 | 1 | 14 | 15 |
/// | `symbolic_links` | `symlink` | Party | v1.0 | 1 | 22 | 25 |
/// | `process_detachment` | `detach` | Single | v1.0 | 1 | 46 | 15 |
/// | `runtime_patching` | `hot_patch` | Single | v1.0 | 2 | 60 | 65 |
/// | `mesh_plating` | `hardened_shell_party` | Party | v1.0 | 3 | 120 | 125 |
/// | `kernel_privileges` | `null_route` | Everyone | v1.0 | 3 | 135 | 135 |
/// | `deep_analysis` | `deep_scan`/`trace_analysis`/`stealth_protocol`/`salvage_routine` | Party | v1.0 | 3 | 130 | 125 |
///
/// `self_exec` and `symbolic_links` are the only two zone-1 nodes granting
/// exactly one ability, so they anchor `ROUTINE_RESEARCH_COST_BASE` and
/// `ROUTINE_RESEARCH_SCOPE_STEP`. `null_route` anchors
/// `ROUTINE_RESEARCH_ZONE_STEP` exactly, and `mesh_plating`/`deep_analysis`
/// confirm it within five either way. `process_detachment` is the one
/// outlier the fit does not chase — `detach` cost as much as a zone-2
/// rung while gated to zone 1, which no axis here reproduces without
/// pulling every other row off by more than it is now.
/// `ROUTINE_RESEARCH_VERSION_STEP` has no old-node precedent at all — no
/// shipped node ever gated two versions of one scope separately — and is a
/// guess of the same shape as the scope step.
pub const ROUTINE_RESEARCH_COST_BASE: u32 = 15;

/// Added once per rung of `routine_tree::scope_rank` above Single.
pub const ROUTINE_RESEARCH_SCOPE_STEP: u32 = 10;

/// Added once per zone above the first (`AbilityDef::research_zone`, where
/// 0 reads as 1) — see `ROUTINE_RESEARCH_COST_BASE`'s doc for why this axis
/// exists at all.
pub const ROUTINE_RESEARCH_ZONE_STEP: u32 = 50;

/// Added once per major version above the family's first.
pub const ROUTINE_RESEARCH_VERSION_STEP: u32 = 15;

/// A synthesised routine node's Research Data cost — linear in scope,
/// zone and major version, per CLAUDE.md's "every difficulty curve is
/// linear." `zone` is the node's own `min_zone` (`AbilityDef::research_zone`
/// with 0 read as 1), so a gated rung costs more than the same scope and
/// version ungated. No material bill: see
/// `docs/superpowers/specs/2026-09-16-routine-research-tree-design.md` §2
/// "Cost" for why a derived node cannot carry one.
pub fn routine_research_cost(scope_rank: u8, version: (u32, u32), zone: u32) -> u32 {
    let zone = zone.max(1);
    ROUTINE_RESEARCH_COST_BASE
        + ROUTINE_RESEARCH_SCOPE_STEP * scope_rank as u32
        + ROUTINE_RESEARCH_ZONE_STEP * (zone - 1)
        + ROUTINE_RESEARCH_VERSION_STEP * version.0.saturating_sub(1)
}
