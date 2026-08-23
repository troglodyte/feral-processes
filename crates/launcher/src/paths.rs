//! Every runtime path the game reads or writes, decided in one place.
//!
//! The trap this module exists to close is a second site resolving a path
//! against `CARGO_MANIFEST_DIR` because that is convenient in a dev build.
//! Such a site works on the machine that compiled the binary and nowhere
//! else, and nothing about it fails to compile — so a shipped build looks
//! fine right up until a stranger unzips it.
//!
//! The four dev bins (`savetool`, `arena`, `train`, `tuner`) keep resolving
//! out of the repo on purpose: a tool that is only ever run out of a
//! checkout should find its material there. "One place" is about the
//! *game's* paths.

use std::path::{Path, PathBuf};

/// The directory segment the game's player data lives under, in the OS data
/// directory. Named once rather than repeated at each site that joins it.
const GAME_DIR: &str = "feral-processes";

/// Resolved from this crate's location rather than the current directory, so
/// the tool and the game find the same `dev-saves/` no matter where they are
/// invoked from. Both are only ever run out of the repo.
pub fn repo_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or(crate_dir)
}

/// Where saves, `profile.ron` and `run_history.log` go — in *every* layout,
/// a repo build included.
///
/// Writing beside the executable was rejected: a build unzipped under
/// `Program Files` cannot write there, and the failure mode is a game that
/// appears to save and silently doesn't. Splitting the rule (OS directory
/// when installed, repo when developing) was rejected too — it is two code
/// paths where one will do, and it would mean a dev build cannot reproduce
/// a player's report about where their saves went.
pub fn data_dir() -> PathBuf {
    data_dir_from(dirs::data_dir())
}

/// Split out so the decision is testable without reading the real
/// environment. `dirs` is what asks the OS — reading `%APPDATA%` by hand
/// misses folder redirection, and `SHGetKnownFolderPath` does not.
fn data_dir_from(os: Option<PathBuf>) -> PathBuf {
    match os {
        Some(dir) => dir.join(GAME_DIR),
        // No `HOME`, so there is nowhere else to put it. Infallible rather
        // than a `Result` because the one thing that must not happen is the
        // game refusing to start over a path lookup.
        None => repo_root(),
    }
}

pub fn saves_dir() -> PathBuf {
    data_dir().join("saves")
}

/// Every runtime path the game reads or writes. Built once, in `main`.
pub struct Paths {
    /// The loose asset tree: species, items, structures, abilities, and the
    /// rest. Loose rather than embedded because the moddability rule says a
    /// new species is a file you drop in, never a rebuild.
    pub assets: PathBuf,
    /// Player data: `saves/`, `profile.ron`, `run_history.log`.
    pub data: PathBuf,
    /// Repo-only material. `None` in an installed build, where there is no
    /// checkout to find it in.
    pub dev: Option<DevPaths>,
}

pub struct DevPaths {
    pub arenas: PathBuf,
    pub battle_log: PathBuf,
}

/// Where this build finds everything.
///
/// Installed-ness is *sniffed* rather than flagged: a shipped build cannot
/// run without its `assets/`, so the probe tests something required to be
/// true. A `--features bundled` flag was rejected because forgetting it
/// produces a zip that works only on the build machine, and that failure is
/// invisible until a stranger unzips it.
///
/// Infallible by design. Whether the resolved `assets` directory actually
/// exists is `main`'s check, which is where that error belongs; a path
/// lookup must never be the reason the game refuses to start.
pub fn resolve() -> Paths {
    let exe = std::env::current_exe().ok();
    let exe_dir = exe.as_deref().and_then(Path::parent);
    layout(
        exe_dir,
        std::env::var_os(ASSETS_OVERRIDE).map(PathBuf::from),
    )
}

/// The one environment override in the whole module. A modder can point a
/// build at an alternative asset tree without disturbing the install, and it
/// is the natural switch for testing a shipped build against repo assets.
/// There is deliberately no second per-path override — a matrix of them is
/// how "one module decides every path" stops being true.
const ASSETS_OVERRIDE: &str = "FERAL_ASSETS_DIR";

/// Split from `resolve` so the decision is testable without an installed
/// build to run it from.
fn layout(exe_dir: Option<&Path>, assets_override: Option<PathBuf>) -> Paths {
    // Empty reads as unset, matching `dev_console::dev_flag`'s rule for the
    // `FERAL_DEV_*` flags. Its `!= "0"` half does not carry over: "0" is a
    // legitimate directory name, and this is a path rather than a boolean.
    let assets_override = assets_override.filter(|p| !p.as_os_str().is_empty());
    let data = data_dir();

    // Beside the exe first, then a macOS bundle's `Contents/Resources`, then
    // the repo. `dev` is `Some` **iff** the repo layout was chosen: the two
    // are one decision, not two.
    let installed = exe_dir.and_then(|dir| {
        let beside = dir.join("assets");
        if beside.is_dir() {
            return Some(beside);
        }
        // `parent()` rather than `join("..")`: the result is handed to the
        // asset loader and then printed in a startup error, so it wants to
        // be a path a reader recognises.
        let resources = dir.parent()?.join("Resources").join("assets");
        resources.is_dir().then_some(resources)
    });

    match installed {
        Some(assets) => Paths {
            assets: assets_override.unwrap_or(assets),
            data,
            dev: None,
        },
        None => {
            let repo = repo_root();
            Paths {
                assets: assets_override.unwrap_or_else(|| repo.join("assets")),
                data,
                dev: Some(DevPaths {
                    arenas: repo.join("dev-arenas"),
                    // Beside `dev-saves/`, `dev-arenas/` and `dev-training/`,
                    // and gitignored: written only when `FERAL_DEV_LOG` is
                    // set. See `dev-logs/README.md`.
                    battle_log: repo.join("dev-logs").join("battles.jsonl"),
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, uniquely-named scratch directory, wiped on drop. Same shape
    /// as the engine suite's `scratch_assets_dir`, which the launcher cannot
    /// reach into.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "feral_processes_paths_{tag}_{}_{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        /// Creates `sub` under the scratch root and hands back its path.
        fn dir(&self, sub: &str) -> PathBuf {
            let path = self.0.join(sub);
            std::fs::create_dir_all(&path).unwrap();
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_data_dir_is_the_os_dir_under_the_game_name() {
        let os = PathBuf::from("/x/y");
        assert_eq!(
            data_dir_from(Some(os.clone())),
            os.join("feral-processes"),
            "the game's data goes in a directory of its own under the OS one"
        );
    }

    #[test]
    fn no_os_data_dir_falls_back_to_the_repo() {
        assert_eq!(
            data_dir_from(None),
            repo_root(),
            "with no HOME there is nowhere else to put it, and the repo is \
             where every build kept it before this"
        );
    }

    #[test]
    fn saves_sit_under_the_data_dir() {
        assert_eq!(saves_dir(), data_dir().join("saves"));
    }

    #[test]
    fn assets_beside_the_exe_is_an_installed_build() {
        let scratch = Scratch::new("installed");
        let assets = scratch.dir("assets");

        let paths = layout(Some(scratch.path()), None);

        assert_eq!(paths.assets, assets);
        assert!(
            paths.dev.is_none(),
            "an installed build has no repo to find dev material in"
        );
    }

    #[test]
    fn no_assets_beside_the_exe_is_a_repo_build() {
        let scratch = Scratch::new("repo");

        let paths = layout(Some(scratch.path()), None);

        assert_eq!(paths.assets, repo_root().join("assets"));
        let dev = paths.dev.expect("a repo build knows where dev material is");
        assert_eq!(dev.arenas, repo_root().join("dev-arenas"));
        assert_eq!(
            dev.battle_log,
            repo_root().join("dev-logs").join("battles.jsonl")
        );
    }

    /// The entire macOS `.app` provision: inside a bundle the executable
    /// sits at `Contents/MacOS/` and its resources at `Contents/Resources/`.
    /// Checked *after* the beside-the-exe probe.
    #[test]
    fn a_mac_bundle_finds_its_assets_in_resources() {
        let scratch = Scratch::new("bundle");
        let exe_dir = scratch.dir("Contents/MacOS");
        let assets = scratch.dir("Contents/Resources/assets");

        let paths = layout(Some(&exe_dir), None);

        assert_eq!(paths.assets, assets);
        assert!(paths.dev.is_none(), "a bundle is an installed build");
    }

    #[test]
    fn an_unknown_exe_location_falls_back_to_the_repo() {
        let paths = layout(None, None);

        assert_eq!(paths.assets, repo_root().join("assets"));
        assert!(paths.dev.is_some());
    }

    /// Both halves in one test on purpose: the installed half alone passes
    /// against an implementation that ignores the layout entirely.
    #[test]
    fn the_assets_override_wins_in_both_layouts() {
        let scratch = Scratch::new("override");
        scratch.dir("assets");
        let elsewhere = scratch.dir("elsewhere");

        let installed = layout(Some(scratch.path()), Some(elsewhere.clone()));
        assert_eq!(installed.assets, elsewhere, "beside an assets/ directory");

        let bare = Scratch::new("override_repo");
        let repo = layout(Some(bare.path()), Some(elsewhere.clone()));
        assert_eq!(repo.assets, elsewhere, "with no assets/ beside the exe");
    }

    #[test]
    fn an_empty_assets_override_reads_as_unset() {
        let scratch = Scratch::new("empty_override");
        let assets = scratch.dir("assets");

        let paths = layout(Some(scratch.path()), Some(PathBuf::from("")));

        assert_eq!(paths.assets, assets);
    }

    #[test]
    fn data_is_the_os_directory_in_both_layouts() {
        let scratch = Scratch::new("data_installed");
        scratch.dir("assets");
        assert_eq!(layout(Some(scratch.path()), None).data, data_dir());

        let bare = Scratch::new("data_repo");
        assert_eq!(layout(Some(bare.path()), None).data, data_dir());
    }
}
