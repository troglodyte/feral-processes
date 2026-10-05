//! A private copy of an asset tree that cleans itself up.
//!
//! Cleanup is `Drop`, not a line at the end of a success path. Each tree is
//! a full copy of `assets/` — around two hundred files — and a run that ends
//! on an error or a panic would otherwise leave every one of them behind.
//! A Ctrl-C does not unwind, so it still leaks the trees alive at that moment.
//! The engine's test fixtures learned this the expensive way: 5,437 stale
//! installs exhausted the filesystem's *inode* table on a tmpfs that was 15%
//! full by bytes, which fails builds machine-wide with an error naming none
//! of it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Distinguishes concurrent trees in one process without a clock, which a
/// reproducible tool has no business reading.
static COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct ScratchAssets {
    dir: PathBuf,
}

impl ScratchAssets {
    /// Copies `from` into a fresh directory under the system temp dir,
    /// named for `tag`, this process and a per-process counter.
    pub fn new(from: &Path, tag: &str) -> Result<Self, String> {
        let dir = std::env::temp_dir().join(format!(
            "feral_{tag}_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        // Built before the copy so a half-finished copy is removed on `?`.
        let scratch = ScratchAssets { dir };
        copy_tree(from, &scratch.dir)?;
        Ok(scratch)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for ScratchAssets {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    for entry in
        std::fs::read_dir(from).map_err(|e| format!("cannot read {}: {e}", from.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| format!("cannot copy {}: {e}", src.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    #[test]
    fn copies_the_tree_and_removes_it_on_drop() {
        let a = ScratchAssets::new(&assets(), "scratch_test").unwrap();
        let b = ScratchAssets::new(&assets(), "scratch_test").unwrap();
        assert_ne!(a.dir(), b.dir());
        assert!(a.dir().join("structures").is_dir());
        let path = a.dir().to_path_buf();
        drop(a);
        assert!(!path.exists());
        assert!(b.dir().exists());
    }
}
