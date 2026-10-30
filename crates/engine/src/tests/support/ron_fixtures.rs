use super::*;
use crate::*;

/// A `Creature`-scoped `FieldBuff` ability — field-only, so it must never
/// surface as something a battle picker or a wild carrier can spend a round
/// on. Shared between the picker-filtering and wild-retaliation tests
/// rather than defined twice.
pub(crate) const FIELD_ONLY_ABILITY: &str = r#"(
    id: "test_field_regen",
    name: "Test Field Regen",
    description: "d",
    target: OneAlly,
    power_cost: 5.0,
    effect: FieldBuff(kind: Regen, power: 2, duration: 20),
)"#;

/// A `Run`-scoped `FieldBuff` — `field_buff_target_mismatch` requires
/// `target: WholeParty` on this scope even though `Game::run_field_routine`
/// ignores it and always lands the buff on the player alone. For the test
/// asserting that a `Run`-scoped routine held by a companion still lands on
/// the player, not the companion.
pub(crate) const FIELD_ONLY_RUN_ABILITY: &str = r#"(
    id: "test_field_xp_boost",
    name: "Test Field XP Boost",
    description: "d",
    target: WholeParty,
    power_cost: 4.0,
    effect: FieldBuff(kind: XpBoost, power: 15),
)"#;

/// A `Creature`-scoped, `WholeParty`-targeted `FieldBuff` — for the test
/// asserting an invocation arms every living party member (and skips a dead one)
/// rather than just the invoker.
pub(crate) const FIELD_ONLY_PARTY_ABILITY: &str = r#"(
    id: "test_field_atk",
    name: "Test Field Atk",
    description: "d",
    target: WholeParty,
    power_cost: 3.0,
    effect: FieldBuff(kind: Atk, power: 4),
)"#;

/// A `Creature`-scoped, percentage-magnitude `FieldBuff` — `Mitigation` is
/// the one percentage kind with a real `affinity_kind` (`Buff`, the same
/// category `FIELD_ONLY_PARTY_ABILITY`'s `Atk` uses), so running the two off
/// the same high-level, high-affinity holder is what proves
/// `FieldBuffKind::scales_with_invoker` actually splits them: `Atk` scales,
/// `Mitigation` lands at exactly its authored value either way.
pub(crate) const FIELD_ONLY_MITIGATION_ABILITY: &str = r#"(
    id: "test_field_mitigation",
    name: "Test Field Mitigation",
    description: "d",
    target: WholeParty,
    power_cost: 5.0,
    effect: FieldBuff(kind: Mitigation, power: 10),
)"#;

/// A species declaring two abilities, so the multi-ability paths can be
/// exercised without depending on shipped kit assignments. The second is
/// gated above a fresh companion's level 1, which is what pins down
/// `Game::actor_abilities`' level filtering.
pub(crate) const TWO_ABILITY_SPECIES: &str = r#"(
    id: "test_medic",
    name: "Test Medic",
    glyph: 'm',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_mitigation: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    base_speed: 10,
    moves: [(name: "Poke", power: 3)],
    abilities: [
        (id: "hot_patch"),
        (id: "sandbox", level: 5),
    ],
)"#;

/// Spawns a tamed member of `TWO_ABILITY_SPECIES` into the party of a
/// game built on a modded install that ships it.
pub(crate) fn game_with_two_ability_companion() -> (Game, Entity) {
    let dir = modded_assets_dir(
        "two_ability_species",
        &[],
        &[],
        &[("test_medic.ron", TWO_ABILITY_SPECIES)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let medic = game
        .world
        .spawn((
            Creature {
                species: "test_medic".to_string(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 5,
                mitigation: 1,
            },
            Tamed { owner: player },
            Experience::default(),
            PowerReserve::default(),
        ))
        .id();
    game.install_innate_routines(medic);
    enlist(&mut game, medic);
    (game, medic)
}

/// A species granting *two* abilities at one level, so an unlock is certain
/// to run a companion out of routine slots whatever its bonuses — the
/// eviction branch, which has a log line of its own.
pub(crate) const CONTENDING_UNLOCK_SPECIES: &str = r#"(
    id: "test_crowded",
    name: "Test Crowded",
    glyph: 'c',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_mitigation: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    base_speed: 10,
    moves: [(name: "Poke", power: 3)],
    abilities: [
        (id: "hot_patch"),
        (id: "sandbox", level: 3),
        (id: "cascade_overflow", level: 3),
    ],
)"#;

/// Spawns a tamed member of `CONTENDING_UNLOCK_SPECIES` into the party of a
/// game built on a modded install that ships it.
pub(crate) fn game_with_contending_unlocks_companion() -> (Game, Entity) {
    let dir = modded_assets_dir(
        "contending_unlock_species",
        &[],
        &[],
        &[("test_crowded.ron", CONTENDING_UNLOCK_SPECIES)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let crowded = game
        .world
        .spawn((
            Creature {
                species: "test_crowded".to_string(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 5,
                mitigation: 1,
            },
            Tamed { owner: player },
            Experience::default(),
            PowerReserve::default(),
        ))
        .id();
    game.install_innate_routines(crowded);
    enlist(&mut game, crowded);
    (game, crowded)
}

/// A costed `Heal` — the shape that reaches the field routine list once
/// `AbilityDef::field_runnable` accepts a priced heal. A degenerate band
/// (`spread` unauthored, so 0) so a test can assert an exact restore
/// without reading the RNG.
pub(crate) const FIELD_HEAL_ABILITY: &str = r#"(
    id: "test_field_patch",
    name: "Test Field Patch",
    description: "d",
    target: OneAlly,
    power_cost: 6.0,
    effect: Heal(power: 10),
)"#;

/// The same heal aimed at everyone, for the wide arm of the field path.
pub(crate) const FIELD_HEAL_PARTY_ABILITY: &str = r#"(
    id: "test_field_patch_party",
    name: "Test Field Patch Party",
    description: "d",
    target: WholeParty,
    power_cost: 7.0,
    effect: Heal(power: 10),
)"#;

/// A `Heal` costing nothing — what stays a battle Special, because outside
/// a battle there is no round counter to enforce its cooldown and Power is
/// the only throttle left. See `AbilityDef::field_runnable`.
pub(crate) const FREE_HEAL_ABILITY: &str = r#"(
    id: "test_free_patch",
    name: "Test Free Patch",
    description: "d",
    target: OneAlly,
    cooldown: 1,
    effect: Heal(power: 10),
)"#;
