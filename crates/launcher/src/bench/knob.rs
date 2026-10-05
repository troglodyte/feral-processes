//! A knob is one number in one asset file that a search may move.
//!
//! Patching is textual, against the pristine file text, so a comment or a
//! field order in a shipped `.ron` survives and a patch is never applied on
//! top of a previous patch.

use feral_processes_engine::items_db::ItemDef;
use feral_processes_engine::structures::StructureDef;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Knob {
    /// Path under the assets directory, e.g. `structures/assembly_bay.ron`.
    pub file: String,
    /// Dotted field path: `capacity`, `work.ticks_per_unit`,
    /// `craftable.cost.<item id>`.
    pub field: String,
    pub min: f64,
    pub max: f64,
}

/// Returns `pristine` with the number at `field` replaced by `value`
/// (rounded when `integer`). Errs unless the path names exactly one number.
pub fn patch(pristine: &str, field: &str, value: f64, integer: bool) -> Result<String, String> {
    let (start, end) = locate(pristine, field)?;
    let number = if integer {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:?}")
    };
    Ok(format!(
        "{}{number}{}",
        &pristine[..start],
        &pristine[end..]
    ))
}

/// Byte range of the one number `field` names.
fn locate(text: &str, field: &str) -> Result<(usize, usize), String> {
    let no_match = |why: &str| format!("knob path `{field}`: {why}");
    let segments: Vec<&str> = field.split('.').collect();
    let (scopes, tail) = match segments.as_slice() {
        ["craftable", "cost", id] => (&segments[..1], Tail::CostAmount(id)),
        [scopes @ .., leaf] => (scopes, Tail::Field(leaf)),
        [] => return Err(no_match("empty")),
    };

    let mut open = first_open(text).ok_or_else(|| no_match("no top-level scope"))?;
    for name in scopes {
        let value =
            one(text, open, name).map_err(|n| no_match(&format!("`{name}` matched {n} times")))?;
        open = scope_open(text, value)
            .ok_or_else(|| no_match(&format!("`{name}` is not a struct")))?;
    }
    let close = matching(text, open).ok_or_else(|| no_match("unbalanced brackets"))?;
    match tail {
        Tail::Field(leaf) => {
            let value = one(text, open, leaf)
                .map_err(|n| no_match(&format!("`{leaf}` matched {n} times")))?;
            number_at(text, value).ok_or_else(|| no_match(&format!("`{leaf}` is not a number")))
        }
        Tail::CostAmount(id) => {
            let value = one(text, open, "cost")
                .map_err(|n| no_match(&format!("`cost` matched {n} times")))?;
            let list = skip_ws(text, value);
            if text.as_bytes().get(list) != Some(&b'[') {
                return Err(no_match("`cost` is not a list"));
            }
            let list_close = matching(text, list).ok_or_else(|| no_match("unbalanced brackets"))?;
            debug_assert!(list_close < close);
            let needle = format!("\"{id}\"");
            let hits: Vec<usize> = text[list..list_close]
                .match_indices(&needle)
                .map(|(i, _)| list + i + needle.len())
                .collect();
            let [hit] = hits.as_slice() else {
                return Err(no_match(&format!("`{id}` matched {} times", hits.len())));
            };
            let after_comma = skip_ws(text, *hit);
            if text.as_bytes().get(after_comma) != Some(&b',') {
                return Err(no_match("cost entry is not an (item, amount) tuple"));
            }
            number_at(text, after_comma + 1).ok_or_else(|| no_match("amount is not a number"))
        }
    }
}

enum Tail<'a> {
    Field(&'a str),
    CostAmount(&'a str),
}

/// The index just after the colon of the single `key:` at depth 0 of the
/// scope opened at `open`; `Err(count)` when there are not exactly one.
fn one(text: &str, open: usize, key: &str) -> Result<usize, usize> {
    let bytes = text.as_bytes();
    let Some(close) = matching(text, open) else {
        return Err(0);
    };
    let mut hits = Vec::new();
    let mut depth = 0usize;
    let mut i = open + 1;
    while i < close {
        if let Some(next) = skip_literal(bytes, i) {
            i = next;
            continue;
        }
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            c if depth == 0 && is_ident(c) && (i == 0 || !is_ident(bytes[i - 1])) => {
                let end = i + bytes[i..].iter().take_while(|b| is_ident(**b)).count();
                if &text[i..end] == key {
                    let colon = skip_ws(text, end);
                    if bytes.get(colon) == Some(&b':') {
                        hits.push(colon + 1);
                    }
                }
                i = end;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    match hits.as_slice() {
        [one] => Ok(*one),
        _ => Err(hits.len()),
    }
}

/// The `(` opening a struct value at `value`, looking through `Some(`.
fn scope_open(text: &str, value: usize) -> Option<usize> {
    let mut i = skip_ws(text, value);
    if text[i..].starts_with("Some(") {
        i = skip_ws(text, i + "Some(".len());
    }
    (text.as_bytes().get(i) == Some(&b'(')).then_some(i)
}

/// The outermost `(` of the file, past any leading comments.
fn first_open(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(next) = skip_literal(bytes, i) {
            i = next;
        } else if bytes[i] == b'(' {
            return Some(i);
        } else {
            i += 1;
        }
    }
    None
}

/// Index of the bracket closing the one at `open`.
fn matching(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        if let Some(next) = skip_literal(bytes, i) {
            i = next;
            continue;
        }
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// If a comment, string or char literal starts at `i`, the index after it.
fn skip_literal(bytes: &[u8], i: usize) -> Option<usize> {
    match bytes[i] {
        b'/' if bytes.get(i + 1) == Some(&b'/') => Some(
            bytes[i..]
                .iter()
                .position(|b| *b == b'\n')
                .map_or(bytes.len(), |n| i + n),
        ),
        b'/' if bytes.get(i + 1) == Some(&b'*') => {
            // RON block comments nest.
            let (mut depth, mut j) = (1usize, i + 2);
            while j < bytes.len() && depth > 0 {
                match (bytes[j], bytes.get(j + 1)) {
                    (b'/', Some(b'*')) => {
                        depth += 1;
                        j += 2;
                    }
                    (b'*', Some(b'/')) => {
                        depth -= 1;
                        j += 2;
                    }
                    _ => j += 1,
                }
            }
            Some(j)
        }
        b'r' if i == 0 || !is_ident(bytes[i - 1]) => {
            let hashes = bytes[i + 1..].iter().take_while(|b| **b == b'#').count();
            if bytes.get(i + 1 + hashes) != Some(&b'"') {
                return None;
            }
            let close: Vec<u8> = std::iter::once(b'"')
                .chain(std::iter::repeat_n(b'#', hashes))
                .collect();
            let body = i + hashes + 2;
            Some(
                bytes[body..]
                    .windows(close.len())
                    .position(|w| w == close.as_slice())
                    .map_or(bytes.len(), |n| body + n + close.len()),
            )
        }
        b'"' => {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'"' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            Some(j + 1)
        }
        b'\'' if bytes.get(i + 2) == Some(&b'\'') => Some(i + 3),
        _ => None,
    }
}

fn skip_ws(text: &str, mut i: usize) -> usize {
    let bytes = text.as_bytes();
    while bytes.get(i).is_some_and(|b| b.is_ascii_whitespace()) {
        i += 1;
    }
    i
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Range of the numeric token starting at or after `from` (past whitespace).
fn number_at(text: &str, from: usize) -> Option<(usize, usize)> {
    let start = skip_ws(text, from);
    let len = text.as_bytes()[start..]
        .iter()
        .take_while(|b| {
            b.is_ascii_digit() || matches!(**b, b'.' | b'+' | b'-' | b'e' | b'E' | b'_')
        })
        .count();
    (len > 0).then_some((start, start + len))
}

/// The value currently at `knob` in the scratch tree `assets`, read through
/// the engine's own types so a patch that landed on the wrong number shows.
/// Phase-1 fields only. A single file is parsed rather than a whole db:
/// `ItemDb::load_dir` needs an `AbilityDb` this has no use for.
pub fn read_back(assets: &Path, knob: &Knob) -> Result<f64, String> {
    let path = assets.join(&knob.file);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let not_phase_1 = || format!("`{}` is not a phase-1 knob", knob.field);
    let parts: Vec<&str> = knob.field.split('.').collect();
    let structure =
        || ron::from_str::<StructureDef>(&text).map_err(|e| format!("{}: {e}", path.display()));
    let missing = || format!("{}: no `{}`", knob.file, knob.field);
    match parts.as_slice() {
        ["capacity"] => Ok(f64::from(structure()?.capacity)),
        ["power_draw"] => Ok(f64::from(structure()?.power_draw)),
        ["work", "ticks_per_unit"] => Ok(f64::from(
            structure()?.work.ok_or_else(missing)?.ticks_per_unit,
        )),
        ["assembles", "ticks_per_unit"] => Ok(f64::from(
            structure()?.assembles.ok_or_else(missing)?.ticks_per_unit,
        )),
        ["craftable", "cost", id] => {
            let item =
                ron::from_str::<ItemDef>(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            let cost = item.craftable.ok_or_else(missing)?.cost;
            cost.iter()
                .find(|(item, _)| item.as_str() == *id)
                .map(|(_, n)| f64::from(*n))
                .ok_or_else(missing)
        }
        _ => Err(not_phase_1()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch_assets::ScratchAssets;

    fn assets() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    const BAY: &str = r#"(
    id: "bay",
    // a comment
    work: None,
    capacity: 10,
    assembles: Some((item: "patch_routine", ticks_per_unit: 20)),
    power_draw: 3,
)"#;

    fn knob(file: &str, field: &str) -> Knob {
        Knob {
            file: file.into(),
            field: field.into(),
            min: 1.0,
            max: 100.0,
        }
    }

    #[test]
    fn round_trip_on_a_real_structure_file() {
        let scratch = ScratchAssets::new(&assets(), "knob_test").unwrap();
        for (field, v, expect) in [
            ("assembles.ticks_per_unit", 33.0, 33.0),
            ("capacity", 41.0, 41.0),
            ("power_draw", 7.0, 7.0),
        ] {
            let k = knob("structures/assembly_bay.ron", field);
            let path = scratch.dir().join(&k.file);
            let text = std::fs::read_to_string(&path).unwrap();
            std::fs::write(&path, patch(&text, field, v, true).unwrap()).unwrap();
            assert_eq!(read_back(scratch.dir(), &k).unwrap(), expect, "{field}");
        }
        let k = knob("structures/power_conduit.ron", "work.ticks_per_unit");
        let path = scratch.dir().join(&k.file);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, patch(&text, &k.field, 9.0, true).unwrap()).unwrap();
        assert_eq!(read_back(scratch.dir(), &k).unwrap(), 9.0);
    }

    #[test]
    fn craftable_cost_amount_round_trips() {
        let scratch = ScratchAssets::new(&assets(), "knob_test").unwrap();
        let k = knob("items/charge_coil.ron", "craftable.cost.power_cell");
        let path = scratch.dir().join(&k.file);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, patch(&text, &k.field, 5.0, true).unwrap()).unwrap();
        assert_eq!(read_back(scratch.dir(), &k).unwrap(), 5.0);
    }

    #[test]
    fn only_the_named_scope_is_touched() {
        let out = patch(BAY, "assembles.ticks_per_unit", 99.0, true).unwrap();
        assert!(out.contains("ticks_per_unit: 99"));
        assert!(out.contains("capacity: 10") && out.contains("// a comment"));
    }

    #[test]
    fn no_match_is_an_error_naming_the_path() {
        let e = patch(BAY, "work.ticks_per_unit", 5.0, true).unwrap_err();
        assert!(e.contains("work.ticks_per_unit"), "{e}");
        let e = patch(BAY, "nonsense", 5.0, true).unwrap_err();
        assert!(e.contains("nonsense"), "{e}");
    }

    #[test]
    fn double_match_is_an_error() {
        let twice = "(\n    capacity: 1,\n    capacity: 2,\n)";
        let e = patch(twice, "capacity", 5.0, true).unwrap_err();
        assert!(e.contains("capacity"), "{e}");
    }

    #[test]
    fn a_nested_block_comment_is_skipped() {
        let text = "(\n    /* capacity: 1, /* capacity: 2, */ ) */\n    capacity: 3,\n)";
        let out = patch(text, "capacity", 9.0, true).unwrap();
        assert!(out.contains("capacity: 9,") && out.contains("capacity: 2, */ )"), "{out}");
    }

    #[test]
    fn a_raw_string_is_skipped() {
        let text = "(\n    note: r#\"capacity: 1, \" ) \"#,\n    capacity: 3,\n)";
        let out = patch(text, "capacity", 9.0, true).unwrap();
        assert!(out.contains("capacity: 9,") && out.contains("capacity: 1, \" )"), "{out}");
    }

    #[test]
    fn integer_knobs_round() {
        let out = patch(BAY, "capacity", 12.6, true).unwrap();
        assert!(out.contains("capacity: 13,"), "{out}");
        let out = patch(BAY, "capacity", 12.6, false).unwrap();
        assert!(out.contains("capacity: 12.6,"), "{out}");
    }

    #[test]
    fn pristine_is_reused() {
        let first = patch(BAY, "capacity", 50.0, true).unwrap();
        let second = patch(BAY, "capacity", 60.0, true).unwrap();
        assert!(first.contains("capacity: 50,"));
        assert!(second.contains("capacity: 60,") && !second.contains("50"));
    }

    #[test]
    fn read_back_refuses_other_fields() {
        let scratch = ScratchAssets::new(&assets(), "knob_test").unwrap();
        let e =
            read_back(scratch.dir(), &knob("structures/assembly_bay.ron", "glyph")).unwrap_err();
        assert!(e.contains("not a phase-1 knob"), "{e}");
    }
}
