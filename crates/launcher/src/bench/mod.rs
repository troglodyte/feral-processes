//! The base bench's launcher half: knobs a search turns, the objective it
//! aims at, and the search.

pub mod knob;
pub mod objective;
pub mod search;

/// A `chains` save in a directory of its own, generated once per test
/// process. `dev_template::resolve` writes one shared working copy, which
/// parallel tests would race on.
#[cfg(test)]
pub(crate) fn chains_save() -> std::path::PathBuf {
    use std::sync::OnceLock;
    static SAVE: OnceLock<std::path::PathBuf> = OnceLock::new();
    SAVE.get_or_init(|| {
        let path = std::env::temp_dir().join(format!("feral_bench_chains_{}.bin", std::process::id()));
        crate::dev_template::generate("chains", &path).expect("chains template generates");
        path
    })
    .clone()
}
