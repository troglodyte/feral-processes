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

#[cfg(test)]
mod tests {
    use super::*;

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
}
