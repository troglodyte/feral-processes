//! Named save templates, for putting the game into a known state on demand.
//!
//! A template is a `SaveData` rendered as RON — byte-for-byte what `savetool
//! dump` emits — checked into `dev-saves/`. `savetool capture` makes one out
//! of a save you already have; `savetool template` and the game's
//! `--template` flag turn one back into a playable `.bin`. That pair is the
//! whole feature: a world you had to play up to becomes a world you can
//! regenerate in a second, so a test of, say, routine extraction starts from
//! nine programs and a standing Compiler every time instead of from whatever
//! last session left behind.
//!
//! **A template is regenerated, never played in place.** `generate` writes a
//! *copy* under `saves/`, and the source `.ron` is never opened for writing
//! by the game. That indirection is not tidiness: the game autosaves, so a
//! session started directly on the fixture would quietly rewrite it, and the
//! fixture would drift into being a record of the last thing anyone did to
//! it. The copy is expendable and the source is not.
//!
//! Being RON rather than bincode is also what lets a template outlive a
//! `SAVE_FORMAT_VERSION` bump. Bincode has no field names on disk, so a
//! `SaveData` change invalidates every `.bin`; RON is field-named, so a new
//! `#[serde(default)]` field still parses and `generate` stamps the current
//! version on the way out. `every_checked_in_template_still_loads` is what
//! says whether that is still true — when it fails after a format change,
//! the fix is to hand-edit the `.ron`, which is the reason templates are
//! stored in the editable format in the first place.

use std::path::{Path, PathBuf};

use feral_processes_engine::{Game, save};

/// Working copies are prefixed so they are obvious in the load menu and
/// cannot collide with the `save_<timestamp>.bin` a real run creates.
const WORKING_COPY_PREFIX: &str = "dev_";

/// Resolved from this crate's location rather than the current directory, so
/// the tool and the game find the same `dev-saves/` no matter where they are
/// invoked from. Both are only ever run out of the repo.
///
/// The body lives in `paths`, which is the one module that decides where
/// anything is. The dependency runs `dev_template -> paths` and never the
/// other way, or `paths::data_dir`'s fallback would be a cycle.
pub fn repo_root() -> PathBuf {
    crate::paths::repo_root()
}

pub fn assets_dir() -> PathBuf {
    repo_root().join("assets")
}

/// Where the checked-in templates live. Deliberately *not* `saves/`, which
/// is gitignored runtime data — a template is source, and is meant to be
/// committed and reviewed like any other fixture.
pub fn dir() -> PathBuf {
    repo_root().join("dev-saves")
}

/// The `.bin` a template is generated into. Shared by `savetool template`
/// and the game's `--template` flag so that generating with one and picking
/// it out of the load menu after the other lands on the same file. That
/// reason is unchanged; the address now comes from `paths`, which is where
/// the game's saves live rather than in the checkout.
pub fn working_copy(name: &str) -> PathBuf {
    crate::paths::saves_dir().join(format!("{WORKING_COPY_PREFIX}{name}.bin"))
}

/// Every template name available, alphabetically. A missing directory reads
/// as none rather than an error — a checkout without one is just a checkout
/// with nothing to offer.
pub fn list() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir()) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "ron"))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}

/// A template name is a filename, never a path. Rejecting separators and
/// `..` up front is what keeps `--template ../../etc/passwd` from resolving
/// to somewhere outside `dev-saves/` — cheap here, and there is no legitimate
/// template whose name needs either.
fn source(name: &str) -> Result<PathBuf, String> {
    let bad =
        name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\');
    if bad {
        return Err(format!("`{name}` is not a template name"));
    }
    let path = dir().join(format!("{name}.ron"));
    if !path.is_file() {
        return Err(format!("no template `{name}`\n{}", known()));
    }
    Ok(path)
}

/// Rendered for error messages, so a typo answers itself instead of sending
/// you off to `ls`.
pub fn known() -> String {
    let names = list();
    if names.is_empty() {
        format!("no templates in {}", dir().display())
    } else {
        format!("available: {}", names.join(", "))
    }
}

/// Writes `name` out as a playable save at `out`, **overwriting whatever is
/// there**. Overwriting is the point rather than a hazard: a template exists
/// so the same starting state comes back every run, and a generate that
/// preserved the last session's progress would defeat that.
///
/// The written save is loaded back through the real `Game::load` before this
/// returns, and removed again if that fails or arrives short-handed. Loading
/// is not enough on its own: a creature whose species id is no longer in
/// `assets/species/` is *skipped* rather than rejected (`Game::load`, the
/// `continue` at the top of the creature loop), which is right for a player
/// who removed a mod and wrong for a fixture — the save would still open,
/// just without the programs it exists to provide. So the tamed count is
/// compared across the load, and a template that comes back gutted is
/// reported as the breakage it is.
pub fn generate(name: &str, out: &Path) -> Result<(), String> {
    let src = source(name)?;
    let text = std::fs::read_to_string(&src).map_err(|e| format!("{}: {e}", src.display()))?;
    let data = save::from_ron(&text).map_err(|e| format!("{}: {e}", src.display()))?;
    let expected_pets = data.creatures.iter().filter(|c| c.tamed).count();
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    save::save_to_file(out, &data).map_err(|e| format!("{}: {e}", out.display()))?;
    let failure = match Game::load(out, &assets_dir()) {
        Err(e) => Some(format!("does not load: {e}")),
        Ok(mut game) => {
            let got = game.owned_pets().len();
            (got != expected_pets).then(|| {
                format!("loads with {got} of its {expected_pets} programs — a species id it names is no longer in assets/species/")
            })
        }
    };
    if let Some(failure) = failure {
        let _ = std::fs::remove_file(out);
        return Err(format!(
            "template `{name}` {failure}\n\
             (edit {} — it is RON precisely so this is fixable by hand)",
            src.display()
        ));
    }
    Ok(())
}

/// The template `name` as a save on disk, generated into its working copy.
///
/// The one thing a frontend needs from this module that `generate` does not
/// already give it: where the copy goes. Both consumers — the `arena` bin
/// and the game's arena screen — need exactly this and nothing more, so
/// they share it rather than each pairing `working_copy` with `generate`
/// and each deciding whether to append `known()` to the failure.
pub fn resolve(name: &str) -> Result<PathBuf, String> {
    let path = working_copy(name);
    generate(name, &path).map_err(|e| format!("{e}\n{}", known()))?;
    Ok(path)
}

/// Rewrites a scenario's `Template(name)` player into the `Save(path)` the
/// engine can load. The one thing `arena::run` cannot do for itself, since
/// `dev_template` lives in the launcher and the engine cannot see it.
///
/// This lived in the `arena` bin, on the stated reasoning that mutating a
/// scenario was that bin's business. `tuner` is a second bin that must do
/// exactly this before it can measure a scenario, so the reasoning no
/// longer holds and the alternative is the copy this repo forbids.
///
/// Still deliberately not done by the game's arena screen: a scenario saved
/// from there has to keep saying `Template(name)` rather than a path into
/// `saves/`, which is a regenerated working copy and expendable.
pub fn resolve_scenario(
    scenario: &mut feral_processes_engine::arena::Scenario,
) -> Result<(), String> {
    use feral_processes_engine::arena::PlayerSource;
    let PlayerSource::Template(name) = &scenario.player else {
        return Ok(());
    };
    scenario.player = PlayerSource::Save(resolve(name)?);
    Ok(())
}

/// Records an existing save as the template `name`, overwriting one of that
/// name if it exists. The save is read through `load_from_file`, so a `.bin`
/// from an older format is refused here rather than being frozen into a
/// fixture that nothing can generate from.
pub fn capture(save_path: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.starts_with('.') {
        return Err(format!("`{name}` is not a template name"));
    }
    let data =
        save::load_from_file(save_path).map_err(|e| format!("{}: {e}", save_path.display()))?;
    let text = save::to_ron(&data).map_err(|e| e.to_string())?;
    let dest = dir().join(format!("{name}.ron"));
    std::fs::create_dir_all(dir()).map_err(|e| format!("{}: {e}", dir().display()))?;
    std::fs::write(&dest, text).map_err(|e| format!("{}: {e}", dest.display()))?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::components::Rarity;

    /// The regression gate for the whole directory: a `SaveData` change or a
    /// `SAVE_FORMAT_VERSION` bump that a template cannot survive fails here,
    /// rather than the next time somebody reaches for one.
    #[test]
    fn every_checked_in_template_still_loads() {
        let names = list();
        assert!(
            !names.is_empty(),
            "no templates in {} — this test would otherwise pass having checked nothing",
            dir().display()
        );
        for name in names {
            let out = std::env::temp_dir().join(format!("feral_processes_template_{name}.bin"));
            let generated = generate(&name, &out);
            let _ = std::fs::remove_file(&out);
            assert!(generated.is_ok(), "{}", generated.unwrap_err());
        }
    }

    /// The gate that stops the next template being captured short:
    /// `every_checked_in_template_still_loads` only proves the RON parses —
    /// this is what proves the base it describes can actually power itself,
    /// by reading `Game::base_power` — the same `game::base::power::ledger`
    /// the engine itself runs — rather than re-summing `power_supply`/
    /// `power_draw` here. A hand-summed copy would stay faithful only until
    /// `ledger` changed what it counts, and would keep green-lighting a
    /// template that had quietly gone dark.
    #[test]
    fn every_checked_in_templates_base_can_power_itself() {
        for name in list() {
            let out =
                std::env::temp_dir().join(format!("feral_processes_template_{name}_power.bin"));
            generate(&name, &out).unwrap();
            let game = Game::load(&out, &assets_dir()).unwrap();
            let _ = std::fs::remove_file(&out);

            let (draw, supply) = game.base_power();

            assert!(
                supply >= draw,
                "template `{name}`'s base draws {draw} against {supply} supply \
                 — it would load with a dark machine"
            );
        }
    }

    /// **The concrete evidence that gear rarity landed without breaking a
    /// save.** `extraction` is a checked-in v29 file whose `fused_gear` is
    /// four positional `(item, tier, qty)` rows — the shape RON cannot widen
    /// in place, and the reason `SaveData` carries a legacy field drained on
    /// load (see `PlayerSave::fused_gear`).
    ///
    /// Asserting on a template rather than a hand-written string is the
    /// point: this is a real file a player could have, written by a build
    /// that predates the feature, and it is the only test here that would
    /// catch the drain being dropped in a later tidy-up.
    #[test]
    fn a_pre_rarity_templates_fused_gear_survives_the_load() {
        let out = std::env::temp_dir().join("feral_processes_template_legacy_gear.bin");
        generate("extraction", &out).unwrap();
        let game = Game::load(&out, &assets_dir()).unwrap();
        let _ = std::fs::remove_file(&out);

        let fused: Vec<_> = game
            .player_status()
            .inventory
            .into_iter()
            .filter(|row| row.copy.tier > 0)
            .collect();
        assert_eq!(
            fused.len(),
            4,
            "extraction.ron carries four fused rows; got {fused:?}"
        );
        assert!(
            fused.iter().all(|row| row.copy.rarity == Rarity::Ordinary),
            "a copy saved before rare tiers existed must load as ordinary: {fused:?}"
        );
        assert!(
            fused
                .iter()
                .any(|row| row.copy.item.as_str() == "scrap_ward" && row.copy.tier == 3),
            "the T3 Scrap Ward must come back at its own tier, not flattened: {fused:?}"
        );
    }

    /// Loading is not the bar for `contracts` either: it exists so a session
    /// testing the contracts loop starts *at a board it can sign*, and a
    /// template that parked the player off their own slab would load
    /// perfectly and refuse every offer. The research buy and the build are
    /// exactly what the template is for skipping.
    #[test]
    fn the_contracts_template_starts_at_a_board_it_can_take_from() {
        let out = std::env::temp_dir().join("feral_processes_template_contracts_board.bin");
        generate("contracts", &out).unwrap();
        let mut game = Game::load(&out, &assets_dir()).unwrap();
        let _ = std::fs::remove_file(&out);

        assert_eq!(
            game.broker_reach(),
            feral_processes_engine::BrokerReach::AtBroker,
            "the template has to park the player on the base, not merely near \
             the Broker — reading the board is not taking from it"
        );
        let board = game
            .contract_board()
            .expect("the template deploys a Broker");
        assert!(
            !board.is_empty(),
            "a Broker in reach with an empty board is a template that tests nothing"
        );
        assert!(
            board.iter().any(|row| row.id.as_str().contains('#')),
            "and the board has to carry a rolled contract, since that is the \
             half a playtest cannot reach any other way: offers={:?}",
            board.iter().map(|r| r.id.as_str()).collect::<Vec<_>>()
        );
    }

    /// Loading is not the bar for `settlements` either: it exists so a
    /// session testing towns opens *standing at one that will deal with
    /// them*, and the nearest town in this world is 128 tiles from the
    /// anchor — a template parked at the base would load perfectly and put
    /// the whole feature an hour away. So this reads the town page the way
    /// the screen does, through `Game::settlement_report`, whose aid lines
    /// are themselves reach-gated: a non-empty list is the assertion that
    /// the party is close enough for an Allied town to do anything for
    /// them.
    #[test]
    fn the_settlements_template_opens_at_an_allied_town() {
        let out = std::env::temp_dir().join("feral_processes_template_settlements_town.bin");
        generate("settlements", &out).unwrap();
        let mut game = Game::load(&out, &assets_dir()).unwrap();
        let _ = std::fs::remove_file(&out);

        let key = game
            .view_entities(2, 2)
            .into_iter()
            .find_map(|view| game.settlement_key(view.entity))
            .expect("the template has to park the party beside a town, not merely near one");

        let report = game.settlement_report(key);
        assert_eq!(
            report.standing, "Allied",
            "`{}` is the town the party stands at and the aid features are \
             gated on its band; got {}",
            report.name, report.standing
        );
        assert!(
            !report.aid.is_empty(),
            "an Allied town offering nothing means the party is out of reach \
             of its garrison, its gift and the trip home — which is the whole \
             of what this template exists to open on"
        );
        assert!(
            game.structure_report()
                .into_iter()
                .any(|s| s.kind == "relay"),
            "the trip home needs somewhere to come back to: dispatch, routes \
             and inbound travel all read `StructureDef::dispatches_sorties`"
        );
    }

    /// Loading is not the bar for `extraction` either. Its whole value is
    /// state that a format migration or a careless hand-edit can strip while
    /// the RON still parses perfectly: the original capture predated the
    /// feature and carried no `tools:` key at all, and a template with an
    /// empty `known_tools` would load, open the Tools screen, and show one
    /// row — which is the state this recapture exists to replace. So this
    /// reads what the two screens read.
    #[test]
    fn the_extraction_template_opens_on_the_whole_kit() {
        let out = std::env::temp_dir().join("feral_processes_template_extraction_kit.bin");
        generate("extraction", &out).unwrap();
        let game = Game::load(&out, &assets_dir()).unwrap();
        let _ = std::fs::remove_file(&out);

        let rows = game.tool_rows();
        assert!(
            rows.len() > 1,
            "one row is the pre-recapture state: the starter alone, nothing              researched — got {rows:?}"
        );
        assert!(
            rows.iter().filter(|r| r.slot.is_some()).count() > 1,
            "a single filled slot shows no loadout, and installing is what              the screen is for"
        );
        assert!(
            rows.iter().any(|r| r.slot.is_none() && r.carriers_held > 0),
            "a forged carrier sitting uninstalled is what gives `[I]` on the              Tools screen something to do"
        );

        let programs = game.downed_program_rows();
        assert!(
            programs.len() > 1,
            "the store is what a tool is pointed at; an empty one puts every              yield preview and the extraction door itself out of reach"
        );
        assert!(
            programs
                .iter()
                .map(|p| p.rarity)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1,
            "one rarity cannot show what grade does to a yield, which is the              axis the preview exists to expose"
        );
        assert!(
            game.can_extract_routines(),
            "a Compiler has to stand: `extraction_bench_tier` is the speed              term, and the routine branch is gated on a bench outright"
        );
    }

    /// Loading is not the bar for `chains`: it exists so that a session
    /// testing production chains starts with one *running*, and a template
    /// whose machines are misaligned by one tile would load perfectly and
    /// sit there starved. So this ticks it and looks for product.
    #[test]
    fn the_chains_template_starts_with_a_chain_that_actually_runs() {
        let out = std::env::temp_dir().join("feral_processes_template_chains_runs.bin");
        generate("chains", &out).unwrap();
        let mut game = Game::load(&out, &assets_dir()).unwrap();
        let _ = std::fs::remove_file(&out);

        let terminal_before = game
            .structure_report()
            .into_iter()
            .find(|s| s.kind == "assembly_bay")
            .map(|s| s.output.iter().map(|(_, n)| n).sum::<u32>())
            .expect("the template stands an Assembly Bay");

        for _ in 0..400 {
            game.wait();
        }

        let bay = game
            .structure_report()
            .into_iter()
            .find(|s| s.kind == "assembly_bay")
            .unwrap();
        let terminal_after: u32 = bay.output.iter().map(|(_, n)| n).sum();
        assert!(
            terminal_after > terminal_before,
            "the template's chain produced nothing in 400 ticks — status {:?}, in {:?}, out {:?}",
            bay.status,
            bay.input,
            bay.output
        );
    }

    #[test]
    fn resolving_an_unknown_template_names_it_and_lists_the_known_ones() {
        let err = resolve("not_a_template").unwrap_err();
        assert!(err.contains("not_a_template"), "{err}");
        assert!(
            err.contains(&list()[0]),
            "the known names are missing: {err}"
        );
    }

    #[test]
    fn resolving_a_template_generates_its_working_copy() {
        let path = resolve("extraction").unwrap();
        assert_eq!(path, working_copy("extraction"));
        assert!(path.is_file(), "{} was not written", path.display());
    }

    #[test]
    fn a_template_name_cannot_reach_outside_the_template_directory() {
        for name in ["", ".", "..", "../secrets", "sub/dir", "a\\b"] {
            assert!(
                source(name).is_err(),
                "`{name}` should not resolve to a template"
            );
        }
    }

    /// `savetool template` and `--template` have to agree on the filename, or
    /// generating with one and loading with the other silently uses two
    /// different saves.
    #[test]
    fn a_working_copy_lands_in_saves_under_the_dev_prefix() {
        let path = working_copy("extraction");
        assert_eq!(path.file_name().unwrap(), "dev_extraction.bin");
        assert_eq!(path.parent().unwrap(), crate::paths::saves_dir());
    }
}
