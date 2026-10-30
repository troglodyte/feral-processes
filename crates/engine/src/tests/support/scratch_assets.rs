use super::*;
use crate::*;

/// A scratch asset install that deletes itself when it goes out of scope.
///
/// Cleanup **must** be a guard rather than a `remove_dir_all` at the end of
/// each test. Every one of these holds a full copy of the shipped assets —
/// eight directories, ~190 files — so a test that panics on a failed assert
/// leaks the lot, and a helper that returns only its `Game` had no way to
/// clean up at all. Between them those two shapes put 5,437 stale installs
/// in `/tmp` and exhausted the filesystem's *inode* table (the tmpfs was
/// only 15% full by bytes), which fails builds and tests across the whole
/// machine with an error naming none of this.
///
/// Dropping it is safe as early as the `Game` is built: `Game` holds a
/// `World` and a `Schedule` and does not retain the assets path, so a
/// helper returning a bare `Game` can let its guard fall at the end of the
/// function. Keep the binding alive only where the *path* is used again,
/// which is what a second `Game::new` or a `Game::load` against the same
/// install needs.
pub(crate) struct ScratchAssets(std::path::PathBuf);

impl std::ops::Deref for ScratchAssets {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}

impl AsRef<std::path::Path> for ScratchAssets {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for ScratchAssets {
    fn drop(&mut self) {
        // Best-effort: a test asserting on a failed install may have left
        // the directory in a state `remove_dir_all` dislikes, and turning
        // that into a second panic during unwinding would abort the process
        // and bury the assertion that actually failed.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh, uniquely-named scratch directory under the OS temp dir, wiped
/// if a stale one from a crashed prior run is somehow still there. Shared
/// by `modded_assets_dir`, `assets_dir_with_extra_structure` and the policy
/// tests (which want a scratch *path* and no assets at all) so they all
/// draw from one counter — collisions were never really possible (`tag`
/// already disambiguates by caller) but there's no reason to run two.
///
/// The directory is deliberately not created: a caller that wants one calls
/// `copy_shipped_assets` or `create_dir_all` itself, and a caller testing an
/// absent file wants the path to stay absent.
pub(crate) fn scratch_assets_dir(tag: &str) -> ScratchAssets {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "feral_processes_{tag}_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    ScratchAssets(dir)
}

/// Copies every shipped asset file into `dir` (which must already exist),
/// skipping the item files named in `omit_items`. The shared body of
/// `modded_assets_dir` and `assets_dir_with_extra_structure` — both need
/// the whole shipped set present so a scratch install still passes
/// `Game::new`'s missing-role startup check.
pub(crate) fn copy_shipped_assets(dir: &std::path::Path, omit_items: &[&str]) {
    let shipped = test_assets_dir();
    for sub in [
        "species",
        "structures",
        "research",
        "items",
        "abilities",
        "perks",
        // Or a scratch install would have no talent trees at all, and
        // `TalentDb::load_dir`'s `?` on the directory would fail the load.
        "talents",
        "achievements",
        "descriptions",
        // The trained enemy policy comes along too, or a modded install
        // would quietly fight under the uniform baseline while the shipped
        // one fought under the weights — a difference no test would name.
        "policies",
        // `default.ron` must exist or `Game::new` refuses to start.
        "behaviours",
        // Same argument again: an install with no affix pool rolls every
        // drop plain, so anything about what gear
        // is worth would be measured against gear that cannot be affixed —
        // silently, since an empty pool is a supported state and not a
        // warning.
        "affixes",
        // The player's stats derive from their attributes, and a level-up
        // banks points that only an attribute with effects can absorb. An
        // install without them levels a player who never grows.
        "attributes",
        // An absent directory loads an empty `StatusDb` silently, so a
        // scratch install without it arms nothing and every status test on
        // it passes vacuously.
        "statuses",
        // Absent loads an empty `CropDb` silently, so a farm test on a
        // scratch install would find no crop to sow.
        "crops",
    ] {
        let dst = dir.join(sub);
        std::fs::create_dir_all(&dst).unwrap();
        for entry in std::fs::read_dir(shipped.join(sub)).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if sub == "items" && omit_items.contains(&name.to_str().unwrap_or_default()) {
                continue;
            }
            std::fs::copy(entry.path(), dst.join(name)).unwrap();
        }
    }
}

/// Copies the shipped `species`/`structures`/`research`/`items`/`abilities`
/// asset dirs into a fresh scratch dir, skipping the item files named in
/// `omit_items` and writing the `extra_*` (filename, RON body) pairs on
/// top — a stand-in for a modded install. The caller removes the
/// directory once its `Game` is done with it.
pub(crate) fn modded_assets_dir(
    tag: &str,
    omit_items: &[&str],
    extra_items: &[(&str, &str)],
    extra_species: &[(&str, &str)],
    extra_research: &[(&str, &str)],
    extra_abilities: &[(&str, &str)],
) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, omit_items);
    for (name, body) in extra_items {
        std::fs::write(dir.join("items").join(name), body).unwrap();
    }
    for (name, body) in extra_species {
        std::fs::write(dir.join("species").join(name), body).unwrap();
    }
    for (name, body) in extra_research {
        std::fs::write(dir.join("research").join(name), body).unwrap();
    }
    for (name, body) in extra_abilities {
        std::fs::write(dir.join("abilities").join(name), body).unwrap();
    }
    dir
}

/// A scratch install whose `talents/` directory holds the shipped trees plus
/// `files` written on top — the way a mod overriding one class's ladder looks.
///
/// A tree with a magnitude no shipped one carries is the only way to test
/// `Game::ability_affinity`'s clamp from the talent side: the shipped nodes are
/// deliberately small enough that no combination of them reaches
/// `AFFINITY_MAX`, which is the content rule working rather than an omission.
/// A scratch install whose `contracts/` directory is a three-step fixture
/// chain and nothing else.
///
/// Built rather than leaning on the shipped missions, so these tests keep
/// testing the *mechanism* when the shipped wording changes — and so they
/// were already passing before the shipped chain existed.
pub(crate) fn assets_with_fixture_chain(tag: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    let contracts = dir.join("contracts");
    std::fs::create_dir_all(&contracts).unwrap();
    // One ordinary contract beside the chain, so a test about the board
    // being *suppressed* can tell that from a board with nothing to offer.
    std::fs::write(
        contracts.join("fixture_plain.ron"),
        r#"(id: "fixture_plain", name: "Fixture Plain", description: "d",
            objective: Terminate(species: None, count: 1), reward: [Xp(1)])"#,
    )
    .unwrap();
    // Steps of its own, on numbers no shipped mission uses.
    for (n, step) in [(1u32, 9001u32), (2, 9002), (3, 9003)] {
        std::fs::write(
            contracts.join(format!("fixture_step_{n}.ron")),
            format!(
                r#"(id: "fixture_step_{n}", name: "Fixture {n}", description: "d",
                    objective: Perform(deed: Examined), reward: [Xp(1)], tutorial: Some({step}))"#
            ),
        )
        .unwrap();
    }
    dir
}

/// A scratch install whose whole chain is one mission asking for a
/// decompile — what the forced first roll keys off.
///
/// Keyed on the *objective* rather than on a shipped id, so this fixture
/// exercises exactly the rule the engine applies and does not depend on
/// what the shipped chain calls its decompile step.
pub(crate) fn assets_with_decompile_mission(tag: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    let contracts = dir.join("contracts");
    std::fs::create_dir_all(&contracts).unwrap();
    std::fs::write(
        contracts.join("fixture_decompile.ron"),
        r#"(id: "fixture_decompile", name: "Fixture Decompile", description: "d",
            objective: Perform(deed: Tamed), reward: [Xp(1)], tutorial: Some(9001))"#,
    )
    .unwrap();
    dir
}

/// Files every onboarding mission as finished, so the ordinary contract
/// board is live.
///
/// Every test that wants a board needs this, because a run with an
/// unfinished chain has an empty one by design. Cheaper and more honest than
/// each test playing the chain: what those tests are about is the board.
pub(crate) fn skip_tutorial(game: &mut Game) {
    let ids: Vec<crate::contracts::ContractId> = game
        .world
        .resource::<crate::contracts::ContractDb>()
        .tutorial_chain()
        .iter()
        .map(|d| d.id.clone())
        .collect();
    let mut held = game
        .world
        .resource_mut::<crate::resources::ActiveContracts>();
    held.active.retain(|c| c.def.tutorial.is_none());
    for id in ids {
        if !held.done.contains(&id) {
            held.done.push(id);
        }
    }
}

pub(crate) fn assets_dir_with_talents(tag: &str, files: &[(&str, &str)]) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    for (name, body) in files {
        std::fs::write(dir.join("talents").join(name), body).unwrap();
    }
    dir
}

/// A scratch install carrying one extra item on top of the shipped
/// catalogue, for a modded item a shipped file can't stand in for.
pub(crate) fn assets_dir_with_extra_item(tag: &str, name: &str, body: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    std::fs::write(dir.join("items").join(name), body).unwrap();
    dir
}

/// A scratch install carrying one extra research node, for a bill no shipped
/// node can stand in for — a test about `ResearchDef::materials` wants to say
/// exactly what the price is without pinning itself to content that is
/// retuned.
pub(crate) fn assets_dir_with_extra_research(tag: &str, name: &str, body: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    std::fs::write(dir.join("research").join(name), body).unwrap();
    dir
}

pub(crate) fn assets_dir_with_extra_structure(tag: &str, name: &str, body: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    std::fs::write(dir.join("structures").join(name), body).unwrap();
    dir
}

/// A scratch install carrying one extra item *and* one extra structure —
/// the pair a modded machine needs, since a machine runs its product's own
/// `craftable.cost` and there is no recipe on the structure to write
/// instead.
pub(crate) fn assets_dir_with_extra_machine(
    tag: &str,
    item: (&str, &str),
    structure: (&str, &str),
) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    std::fs::write(dir.join("items").join(item.0), item.1).unwrap();
    std::fs::write(dir.join("structures").join(structure.0), structure.1).unwrap();
    dir
}

/// A scratch install carrying one extra achievement on top of the shipped
/// ladder, for cases that need a rung the real assets deliberately don't have.
pub(crate) fn scratch_assets_with_achievement(id: &str, body: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(id);
    copy_shipped_assets(&dir, &[]);
    std::fs::write(dir.join("achievements").join(format!("{id}.ron")), body).unwrap();
    dir
}

/// A modded install missing `core_fragment.ron` — the item that holds
/// the Currency economy role — so `Game::new`'s missing-role startup
/// abort (see `ItemDb::missing_roles`) can be exercised against an
/// otherwise-valid item set.
pub(crate) fn assets_dir_missing_currency_item() -> ScratchAssets {
    modded_assets_dir(
        "missing_currency",
        &["core_fragment.ron"],
        &[],
        &[],
        &[],
        &[],
    )
}

/// A scratch install carrying every shipped floor finish except `omit` —
/// the stand-in for a mod that renamed or deleted a `.ron` file between a
/// save and its later load. `copy_shipped_assets` never touches `floors/`
/// (`FloorDb::load_dir`'s absent-is-silent rule means most tests don't need
/// it there at all), so this copies that directory itself.
pub(crate) fn assets_dir_missing_floor(tag: &str, omit: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    let shipped = test_assets_dir();
    let dst = dir.join("floors");
    std::fs::create_dir_all(&dst).unwrap();
    for entry in std::fs::read_dir(shipped.join("floors")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name.to_str() == Some(omit) {
            continue;
        }
        std::fs::copy(entry.path(), dst.join(name)).unwrap();
    }
    dir
}
