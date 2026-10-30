use crate::abilities::AffinityKind;
use crate::tests::support::*;
use crate::tuning::{AFFINITY_MAX, AFFINITY_MIN, AFFINITY_NEUTRAL};
use crate::*;

/// The thirty-six hunt-only routines are reachable exactly one way: off a
/// wild carrier. A species file naming one would quietly restore the "just
/// target the species" loop this set exists to break.
///
/// The count is a tripwire, not a target: the pool's size is a design
/// decision and widening it dilutes every routine already in it, since
/// `WILD_ROUTINE_CHANCE` decides whether there is a carrier at all and
/// `wild_weight` only decides what it holds. The six added on 2026-08-26
/// (the Row Hammer ladder, Skim and Segfault's new upper rungs) took the
/// total authored weight from 185 to 212, so each existing entry now turns
/// up about an eighth less often.
///
/// **The routine-tree half of this check is gone, not weakened.** Every
/// ability — hunt-only included — gets a synthesised research node now
/// (`routine_tree::synthesise_nodes`), and that is by design: researching a
/// hunt-only routine's node still requires *discovering* it first, off a
/// carrier, exactly as this test polices for a species kit. A node existing
/// is not a second door in.
#[test]
fn no_species_file_grants_a_wild_only_ability() {
    let game = Game::new(3301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild_only: Vec<String> = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .wild_pool()
        .into_iter()
        .map(|(d, _)| d.id.clone())
        .collect();
    assert_eq!(wild_only.len(), 36, "thirty-six routines are hunt-only");

    for species in game.species_defs() {
        for ability in &species.abilities {
            assert!(
                !wild_only.contains(&ability.id),
                "species {:?} grants {:?}, which is meant to be findable only in the field",
                species.id,
                ability.id
            );
        }
    }
}

/// A cooldown is measured in battle rounds (`Game::wild_retaliate` ticks it
/// down once per round a hostile carrier could have fired again), so it only
/// means something for an ability that can enter a battle — but there it is
/// now the *whole* price of a Special, on both sides. A battle ability with a
/// cooldown of 0 is completely unthrottled: a hostile carrier fires it every
/// single round, and the party side pays nothing else for it either.
///
/// Two kinds of ability never enter a battle at all, so a cooldown on either
/// would just be inert: `decompile`, the player's capture mechanism, which
/// hostiles never use and which must stay spammable so a failed capture roll
/// doesn't change the core loop; and any `FieldBuff` ability, which is run
/// outside battle and limited by `power_cost` instead — the doc on
/// `AbilityEffect::FieldBuff` explains why `cooldown` is dead weight on that
/// variant.
#[test]
fn every_shipped_ability_but_decompile_and_field_routines_has_a_cooldown() {
    let game = Game::new(3302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        if def.id == crate::abilities::DECOMPILE_ABILITY_ID {
            assert_eq!(def.cooldown, 0, "decompile stays spammable, deliberately");
            continue;
        }
        if def.effect.field_only() {
            assert_eq!(
                def.cooldown, 0,
                "ability {:?} is field-only, so cooldown should be left at its default",
                def.id
            );
            continue;
        }
        assert!(
            def.cooldown >= 1,
            "ability {:?} has no cooldown, so nothing throttles it at all — a wild \
             carrier would fire it every round and the party could spam it",
            def.id
        );
    }
}

/// Every routine that moves Integrity rolls a band rather than a fixed
/// figure. The mechanism (`AbilityEffect::spread`, `battle::DamageRange`)
/// defaults to a degenerate band so a mod's file keeps parsing untouched —
/// which is exactly why the *shipped* roster needs a census: all 34 of these
/// authored no spread for as long as the field existed, and a new one would
/// ship deterministic without anything failing.
///
/// The three variants are named rather than matched with a `_ =>` arm, on
/// `render/stack.rs::cell_mark`'s rule: an eleventh Integrity-moving effect
/// should fail to compile here rather than skip the check.
#[test]
fn every_shipped_integrity_routine_rolls_a_band() {
    use crate::abilities::AbilityEffect as E;
    let game = Game::new(3303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        let spread = match &def.effect {
            E::Damage { spread, .. } | E::Heal { spread, .. } | E::Drain { spread, .. } => *spread,
            E::Buff { .. }
            | E::Debuff { .. }
            | E::Cleanse
            | E::Decompile
            | E::FieldBuff { .. }
            | E::Phase
            | E::Jump
            | E::Symlink
            | E::Summon { .. }
            | E::Cloak { .. }
            | E::Emulate { .. }
            | E::Tamper { .. }
            // A relocation moves no Integrity at all — it moves a body.
            | E::Reinforce
            | E::Teleport
            // Built by `routine_craft::compile` alone; its band is
            // `Heal`'s, and no shipped file names it.
            | E::HealOverTime { .. }
            // `Reinforce`'s reason: what rises is scaled by a fixed
            // multiplier off a snapshot, not by a rolled band.
            | E::Reanimate { .. } => continue,
        };
        checked += 1;
        assert!(
            spread > 0,
            "ability {:?} deals a fixed figure every time — author a `spread` so it \
             rolls a band, the way every species move already does",
            def.id
        );
    }
    assert!(
        checked > 0,
        "the census read no Integrity-moving abilities at all, so it proves nothing"
    );
}

/// Every routine that rolls to hit is aimed, and no routine that never rolls
/// pretends to be.
///
/// Two halves, because `AbilityDef::accuracy` fails in both directions.
/// `#[serde(default)]` is 0, so a mod's file keeps parsing untouched — and a
/// shipped routine that forgets the key is silently as blind as a basic
/// attack, which is the bug this whole change exists to close. In the other
/// direction the field is read *only* by `Damage` and `Drain`; authored on a
/// `Buff` or a `Cleanse` it is a number that does nothing, and a modder
/// reading the shipped roster would copy it.
///
/// The variants are named rather than swept with a `_ =>` arm, on
/// `render/stack.rs::cell_mark`'s rule: a new effect that rolls to hit
/// should fail to compile here rather than skip the check.
///
/// Deliberately not a census of the *values* — how aimed a routine is, is a
/// content decision, and `tuning/` is where difficulty lives.
#[test]
fn every_shipped_routine_that_rolls_to_hit_is_aimed_and_no_other_is() {
    use crate::abilities::AbilityEffect as E;
    let game = Game::new(3304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mut aimed, mut blind) = (0, 0);
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        let rolls_to_hit = match &def.effect {
            E::Damage { .. } | E::Drain { .. } => true,
            E::Heal { .. }
            | E::HealOverTime { .. }
            | E::Buff { .. }
            | E::Debuff { .. }
            | E::Cleanse
            | E::Decompile
            | E::FieldBuff { .. }
            | E::Phase
            | E::Jump
            | E::Symlink
            | E::Summon { .. }
            | E::Cloak { .. }
            | E::Emulate { .. }
            // A tamper always lands — the forecast is the feature, and a
            // tamper that could fumble would turn a guaranteed read into a
            // gamble.
            | E::Tamper { .. }
            // A relocation always lands, a tamper's reason: the refusals
            // are all in `Game::tactical_teleport`, above the charge, so
            // there is nothing left for a roll to decide.
            | E::Reinforce
            | E::Teleport
            // `Reinforce`'s reason: a raise always lands, the refusals are
            // all in `Game::tactical_use_routine`, above the charge.
            | E::Reanimate { .. } => false,
        };
        if rolls_to_hit {
            aimed += 1;
            assert!(
                def.accuracy > 0,
                "routine {:?} rolls to hit but authors no `accuracy`, so it is exactly as \
                 blind as a basic attack while costing Power and a cooldown",
                def.id
            );
        } else {
            blind += 1;
            assert_eq!(
                def.accuracy, 0,
                "routine {:?} never rolls to hit, so its `accuracy` is read by nothing — \
                 authoring one here teaches a modder that the key works everywhere",
                def.id
            );
        }
    }
    assert!(
        aimed > 0 && blind > 0,
        "the census read {aimed} rolling and {blind} non-rolling routines, so at least \
         one half of it proves nothing"
    );
}

/// What each targeting mode derives when a routine authors no `shape:` or
/// `range:` — which is every routine the game ships.
///
/// A second copy of `AbilityTarget::derived_shape`/`derived_range` on
/// purpose, and the only copy of a formula this repo keeps: it is a *pin*,
/// so moving the derivation has to be deliberate rather than a one-word edit
/// nothing notices. The `match` is what makes a sixth targeting mode a
/// compile error here as well as there — a `#[serde(default)]` field with no
/// census is the trap this exists to close, and a routine that quietly
/// became single-target on a battle map reads as a nerf rather than a bug.
fn derived_geometry(
    target: crate::abilities::AbilityTarget,
) -> (
    crate::abilities::AbilityShape,
    crate::abilities::AbilityRange,
) {
    use crate::abilities::AbilityRange as R;
    use crate::abilities::AbilityShape as S;
    use crate::abilities::AbilityTarget::*;
    match target {
        OneAlly | OneEnemyGroupFront => (S::Single, R { min: 0, max: 1 }),
        WholeParty => (S::Radius { radius: 3 }, R { min: 0, max: 0 }),
        WholeEnemyGroup => (S::Radius { radius: 1 }, R { min: 0, max: 6 }),
        AllEnemies => (S::Radius { radius: 2 }, R { min: 0, max: 6 }),
    }
}

/// Every targeting mode answers with a shape and a range, and with the ones
/// documented in the design.
#[test]
fn every_targeting_mode_derives_the_geometry_it_is_documented_to() {
    use crate::abilities::AbilityTarget::*;
    for target in [
        OneAlly,
        WholeParty,
        OneEnemyGroupFront,
        WholeEnemyGroup,
        AllEnemies,
    ] {
        let (shape, range) = derived_geometry(target);
        assert_eq!(
            target.derived_shape(),
            shape,
            "{target:?} derives a shape the census does not expect"
        );
        assert_eq!(
            target.derived_range(),
            range,
            "{target:?} derives a range the census does not expect"
        );
    }
}

/// The five tamper routines are the first to author a `shape:`/`range:` of
/// their own; every other shipped routine still resolves through the
/// derivation above. Either way, what a routine resolves to has to be
/// something a battle map can draw and aim.
#[test]
fn every_shipped_routine_resolves_to_a_shape_a_battle_map_can_use() {
    use crate::abilities::AbilityShape;
    let game = Game::new(3307, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut single = 0;
    let mut area = 0;
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        let range = def.tactical_range();
        assert!(
            range.min <= range.max,
            "{:?} may be aimed between {} and {} cells away, which is nowhere",
            def.id,
            range.min,
            range.max
        );
        match def.tactical_shape() {
            AbilityShape::Single => single += 1,
            AbilityShape::Radius { radius } => {
                area += 1;
                assert!(radius > 0, "{:?} blasts a radius of nothing", def.id);
            }
            AbilityShape::Line { length } => {
                assert!(length > 0, "{:?} draws a line of nothing", def.id);
            }
            AbilityShape::Cone { length, degrees } => {
                assert!(length > 0, "{:?} opens a cone of nothing", def.id);
                assert!(
                    (1..=360).contains(&degrees),
                    "{:?} opens a cone {degrees} degrees wide",
                    def.id
                );
            }
        }
    }
    assert!(
        single > 0 && area > 0,
        "the census read {single} single-target and {area} area routines, so at least \
         one half of it proves nothing"
    );
}

/// The scope word an ability's `name` must end in, given what it targets.
/// `OneAlly` and `OneEnemyGroupFront` share "Single" — one recipient either
/// way, and which side it lands on is never in doubt from the picker.
fn scope_word(target: crate::abilities::AbilityTarget) -> &'static str {
    use crate::abilities::AbilityTarget::*;
    match target {
        OneAlly | OneEnemyGroupFront => "Single",
        WholeParty => "Party",
        WholeEnemyGroup => "Group",
        AllEnemies => "Everyone",
    }
}

/// Strips a trailing ` vN.N` version tag, which is how two abilities in the
/// same family at the same scope are told apart by magnitude. A thin
/// wrapper kept local to this test: `routine_tree::version` returns the
/// parsed tag rather than the stripped name, which is what this file's own
/// callers want.
fn without_version_tag(name: &str) -> &str {
    let Some((base, tag)) = name.rsplit_once(' ') else {
        return name;
    };
    let is_tag = tag.strip_prefix('v').is_some_and(|v| {
        v.split_once('.')
            .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
            && v.chars().all(|c| c.is_ascii_digit() || c == '.')
    });
    if is_tag { base } else { name }
}

/// An ability's display name is `<Family> <Scope>`, plus an optional
/// `vN.N` when two in one family share a scope and differ only in
/// magnitude. The suffix is the whole point: the picker shows the name
/// before it shows anything else, so "does this hit one thing, a group, or
/// the field" has to be readable without opening the description.
///
/// This is a naming *policy*, not a schema rule — `AbilityDb` loads a
/// badly-named file happily, and a mod is free to ignore it. What the test
/// guards is the shipped set, where a new file that skips the suffix would
/// otherwise read as a different kind of thing from its 40 neighbours.
///
/// **A `Summon` is excluded, because its `target` is not a scope.** The
/// effect is seated by the combat model rather than resolved over
/// recipients, and `WholeParty` is the ally-facing target
/// `summon_target_mismatch` pins it to so that a picker never opens on it —
/// it names nobody. Suffixing one "Party" would be telling the player it
/// lands on their party, which is the one thing it does not do.
///
/// **`Emulate` is excluded for the same reason.** Its `target: WholeParty`
/// is the same no-picker shape, chosen because the effect names its own
/// recipient (the acting body) directly rather than trusting
/// `Game::ability_recipients` — see the variant's own doc — and "Emulate
/// Party" would tell the player it lands on their party too. `Teleport`
/// and `Reinforce` join them on the same no-picker shape. `Reanimate` joins
/// them too: which marks rise is answered off the aim cell and the
/// authored `shape:`/`range:`, never off `target`, so "Respawn Party" would
/// tell the player it lands on their party alone, which it does not.
#[test]
fn every_shipped_ability_name_ends_in_the_scope_it_targets() {
    let game = Game::new(3303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .all()
        .filter(|d| {
            !matches!(
                d.effect,
                crate::abilities::AbilityEffect::Summon { .. }
                    | crate::abilities::AbilityEffect::Emulate { .. }
                    | crate::abilities::AbilityEffect::Teleport
                    | crate::abilities::AbilityEffect::Reinforce
                    | crate::abilities::AbilityEffect::Reanimate { .. }
            )
        })
    {
        let expected = scope_word(def.target);
        let base = without_version_tag(&def.name);
        assert!(
            base.ends_with(expected) && base.len() > expected.len(),
            "ability {:?} is named {:?}; targeting {:?} it should end in {:?}",
            def.id,
            def.name,
            def.target,
            expected
        );
    }
}

/// Every shipped routine states whether running it ends the invoker's own
/// cloak, and the table is what says which answer each kind gives.
///
/// `AbilityEffect::breaks_cloak` is exhaustive, so a new effect kind fails
/// to *compile* rather than shipping as a routine you can run from inside a
/// cloak for free — `render/stack.rs::cell_mark`'s rule. What a compiler
/// cannot hold is that the answers stay the ones the design gives, which is
/// the second `match` here: it is a copy on purpose, in the same spirit as
/// `every_shipped_routine_resolves_to_a_shape_a_battle_map_can_use`, so
/// flipping an arm in `abilities.rs` has to be a decision taken twice.
///
/// Both halves must be non-empty, or a roster with nothing aggressive in it
/// would pass an assertion about aggression.
#[test]
fn every_shipped_routine_states_whether_it_breaks_a_cloak() {
    use crate::abilities::AbilityEffect as E;
    let game = Game::new(3306, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (mut aggressive, mut peaceful) = (0, 0);
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        let expected = match &def.effect {
            E::Damage { .. } | E::Drain { .. } | E::Debuff { .. } | E::Decompile => true,
            // A tamper names the other side, `Debuff`'s reason exactly.
            E::Tamper { .. } => true,
            E::Heal { .. }
            | E::HealOverTime { .. }
            | E::Buff { .. }
            | E::Cleanse
            | E::Cloak { .. }
            | E::FieldBuff { .. }
            | E::Phase
            | E::Jump
            | E::Summon { .. }
            | E::Emulate { .. }
            // A relocation names a cell; `Game::tactical_teleport` breaks
            // the cloak itself when the body it moved was a hostile.
            | E::Reinforce
            | E::Teleport
            | E::Symlink
            // `Reinforce`'s reason: fielding more of your own side names
            // nobody on the other.
            | E::Reanimate { .. } => false,
        };
        assert_eq!(
            def.effect.breaks_cloak(),
            expected,
            "ability {:?} disagrees with the design about whether running it is \
             the aggressive act a cloak is waiting for",
            def.id
        );
        if expected {
            aggressive += 1
        } else {
            peaceful += 1
        }
    }
    assert!(
        aggressive > 0 && peaceful > 0,
        "the census read {aggressive} aggressive and {peaceful} peaceful routines, \
         so one half of it proves nothing"
    );
}

/// Two abilities sharing a display name is invisible in the picker — the
/// player sees two identical rows and picks by luck. The version tag exists
/// so same-family, same-scope siblings stay distinguishable; this is what
/// makes forgetting one a failure rather than a shrug.
#[test]
fn no_two_shipped_abilities_share_a_display_name() {
    let game = Game::new(3304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut seen: std::collections::HashMap<&str, &crate::abilities::AbilityId> =
        std::collections::HashMap::new();
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        if let Some(other) = seen.insert(def.name.as_str(), &def.id) {
            panic!(
                "abilities {:?} and {:?} both display as {:?}",
                other, def.id, def.name
            );
        }
    }
}

/// **Nothing you can choose to run is free.** A routine is priced in two
/// things — a cooldown, which says "not again yet", and a Power cost, which
/// says "not any more" — and a routine that authors no cost has only half of
/// that. On the map it has neither: there are no rounds out there for a
/// cooldown to count, so Power is the *whole* price of a field invocation.
///
/// **No exceptions list, deliberately.** Five files reached this state by
/// authoring no `power_cost` key at all rather than by anyone deciding they
/// should be free — the tail of the 2026-08-17 flip off `fatigue_cost`, which
/// renamed the key everywhere it appeared and could not touch the five
/// ladders where it appeared nowhere. A `0.0` default makes "the author
/// priced this at nothing" and "the author never saw this field" the same
/// state, and the only thing that can tell them apart is a census that
/// refuses both.
///
/// **Passives are excluded because they are not run.** `power_cost` is what
/// invoking costs the invoker, and a passive is never invoked: it fires on
/// its trigger, takes no turn, and is throttled by its cooldown alone. That
/// is the same split `every_everyone_scope_routine_pays_the_everyone_tier_price`
/// makes, for the same reason.
#[test]
fn every_runnable_routine_is_priced_in_power() {
    let game = Game::new(3309, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        if def.is_passive() {
            continue;
        }
        assert!(
            crate::abilities::routine_power_cost(def) > 0.0,
            "{:?} can be run and costs no Power; a routine with no price has \
             only its cooldown throttling it, and on the map it has nothing",
            def.id
        );
    }
}

/// `AbilityDef::validate` refuses a non-finite cost at load, and that check
/// has to survive the rename off `fatigue_cost` rather than be lost with it.
/// Asserted over the real files, since a negative cost would *pay* the invoker
/// for running and nothing downstream defends against one.
#[test]
fn every_shipped_power_cost_is_finite_and_non_negative() {
    let game = Game::new(3307, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        assert!(
            def.power_cost.is_finite() && def.power_cost >= 0.0,
            "{:?} authors power_cost {}",
            def.id,
            def.power_cost
        );
    }
}

/// The 2026-08-17 flip was a key rename, not an authoring pass: the numbers
/// were already in the files, priced back when `fatigue_cost` meant exactly
/// what `power_cost` means now. Three abilities from three different bands
/// pin that nothing moved in translation.
///
/// What the rename could not carry over was a value nobody had written. Five
/// ladders authored no `fatigue_cost` at all, so there was no key to rename
/// and they inherited the new field's `0.0` — free to run, in a game where
/// Power is a routine's whole price outside battle. They were priced on
/// 2026-09-08 and `every_runnable_routine_is_priced_in_power` is what keeps
/// the sixth from appearing; `deadlock` stands for the five here, so a
/// silent revert to the default is caught by a number as well as by a rule.
#[test]
fn the_flip_to_power_cost_carried_the_authored_numbers_over_unchanged() {
    let game = Game::new(3308, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::abilities::AbilityDb>();
    for (id, expected) in [
        ("deadlock", 6.0),
        // Taming is really paid for in the Breach Kit the call spends. The
        // token is here because `decompile` is the one routine with no
        // cooldown, so a reserve is all that ever refuses it.
        ("decompile", 1.0),
    ] {
        let def = db.get(id).unwrap_or_else(|| panic!("{id} ships"));
        assert_eq!(def.power_cost, expected, "{id}");
    }
}

/// Reaching every hostile on the field is the top of the scope ladder, and
/// the shipped set charges for it consistently: no `AllEnemies` routine
/// costs under 15 Power or comes off cooldown in under 4 rounds. That ladder
/// is what every Everyone-tier magnitude was derived against, and nothing
/// else gates it — a new file could otherwise undercut a whole family's
/// Group tier while reaching further than it.
///
/// The cost half is skipped for **passives**, and only for them: `power_cost`
/// is what running a routine costs its invoker, and a passive is never run —
/// it fires on its trigger and takes no turn. The cooldown half still applies
/// to everything, passive or not, because a cooldown is what bounds how often
/// the effect lands, and that question does not care who asked for it.
#[test]
fn every_everyone_scope_routine_pays_the_everyone_tier_price() {
    use crate::abilities::AbilityTarget;
    let game = Game::new(3306, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .all()
        .filter(|d| d.target == AbilityTarget::AllEnemies)
    {
        assert!(
            def.is_passive() || def.power_cost >= 15.0,
            "{:?} reaches the whole field for {} Power; the Everyone tier starts at 15",
            def.id,
            def.power_cost
        );
        assert!(
            def.cooldown >= 4,
            "{:?} reaches the whole field on a {}-round cooldown; the Everyone tier starts at 4",
            def.id,
            def.cooldown
        );
    }
}

pub(super) const AFFINITY_SPECIES: &str = r#"(
    id: "test_healer",
    name: "Test Healer",
    glyph: 'h',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_mitigation: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [(name: "Poke", power: 3)],
    affinities: (heal: 1.5, damage: 0.8),
)"#;

#[test]
fn a_species_declares_affinities_and_omitted_ones_stay_neutral() {
    let dir = crate::tests::support::modded_assets_dir(
        "affinity_species",
        &[],
        &[],
        &[("test_healer.ron", AFFINITY_SPECIES)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_healer").unwrap();
    assert_eq!(aff.get(AffinityKind::Heal), 1.5);
    assert_eq!(aff.get(AffinityKind::Damage), 0.8);
    // The three the file never named must default individually, not leave
    // the whole struct at its all-neutral fallback.
    assert_eq!(aff.get(AffinityKind::Buff), AFFINITY_NEUTRAL);
    assert_eq!(aff.get(AffinityKind::Debuff), AFFINITY_NEUTRAL);
    assert_eq!(aff.get(AffinityKind::Drain), AFFINITY_NEUTRAL);
}

#[test]
fn a_species_file_with_no_affinities_field_still_loads_neutral() {
    // The #[serde(default)] contract that keeps every shipped file and
    // every third-party mod parsing untouched.
    let dir = crate::tests::support::modded_assets_dir(
        "affinity_absent",
        &[],
        &[],
        &[("test_plain.ron", crate::tests::support::TWO_ABILITY_SPECIES)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_medic").unwrap();
    for kind in [
        AffinityKind::Damage,
        AffinityKind::Heal,
        AffinityKind::Buff,
        AffinityKind::Debuff,
        AffinityKind::Drain,
    ] {
        assert_eq!(aff.get(kind), AFFINITY_NEUTRAL);
    }
}

#[test]
fn an_out_of_range_affinity_is_clamped_at_load() {
    let body = AFFINITY_SPECIES.replace("heal: 1.5", "heal: 99.0");
    let dir = crate::tests::support::modded_assets_dir(
        "affinity_clamped",
        &[],
        &[],
        &[("test_healer.ron", &body)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_healer").unwrap();
    assert_eq!(aff.get(AffinityKind::Heal), AFFINITY_MAX);
}

#[test]
fn an_affinity_below_the_floor_is_clamped_at_load() {
    let body = AFFINITY_SPECIES.replace("heal: 1.5", "heal: 0.0");
    let dir = crate::tests::support::modded_assets_dir(
        "affinity_clamped_low",
        &[],
        &[],
        &[("test_healer.ron", &body)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_healer").unwrap();
    assert_eq!(aff.get(AffinityKind::Heal), AFFINITY_MIN);
}
