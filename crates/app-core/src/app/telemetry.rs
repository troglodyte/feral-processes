//! Turning battle records into a file. The only place in the workspace
//! that names `serde_json`.
//!
//! The engine derives `Serialize` on `telemetry::Record` and hands over
//! values; the dependency stops here, which is what keeps
//! `cargo check --workspace` at ~1s and the engine's dependency list at the
//! seven crates it has always had.

use std::io::Write;

use feral_processes_engine::telemetry::Record;

use crate::*;

/// Appends `records` to `path` as JSON Lines, creating the directory on
/// first write.
///
/// A record that fails to serialize is skipped rather than aborting the
/// batch — unreachable with the shipped types, but a later field could make
/// it so, and losing one line beats losing the rest of the fight.
/// `pub` for `train`, which logs its two evaluation passes and would
/// otherwise need its own `serde_json` and its own idea of the format. The
/// module doc's claim above survives that: the launcher calls this rather
/// than repeating it.
pub fn append_records(path: &Path, records: &[Record]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let mut body = String::new();
    for record in records {
        match serde_json::to_string(record) {
            // `to_string` never emits a newline, so one object really is one
            // line — the property `each_record_is_one_json_line` asserts
            // against the written file.
            Ok(line) => {
                body.push_str(&line);
                body.push('\n');
            }
            Err(e) => eprintln!("skipping an unserializable telemetry record: {e}"),
        }
    }
    file.write_all(body.as_bytes())
}
