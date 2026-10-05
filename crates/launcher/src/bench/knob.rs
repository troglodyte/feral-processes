//! A knob is one number in one asset file that a search may move.
//!
//! Patching is textual, against the pristine file text, so a comment or a
//! field order in a shipped `.ron` survives and a patch is never applied on
//! top of a previous patch.

use feral_processes_engine::items_db::ItemDef;
use feral_processes_engine::needs::NeedDef;
use feral_processes_engine::situations::ThoughtDef;
use feral_processes_engine::structures::StructureDef;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Knob {
    /// Path under the assets directory, e.g. `structures/assembly_bay.ron`.
    pub file: String,
    /// Dotted field path, one of `FIELDS`.
    pub field: String,
    pub min: f64,
    pub max: f64,
}

/// Every field a knob may name, as a path pattern (`<x>` stands for one
/// segment, an id) and whether the game reads it as a whole number. The one
/// place a field is listed: integer-ness and the error text derive from it.
const FIELDS: &[(&str, bool)] = &[
    ("capacity", true),
    ("power_draw", true),
    ("work.ticks_per_unit", true),
    ("assembles.ticks_per_unit", true),
    ("craftable.cost.<item>", true),
    ("services.<need>.per_tick", false),
    ("services.<need>.radius", true),
    ("drain_per_tick", false),
    ("working_multiplier", false),
    ("critical", false),
    ("content", false),
    ("morale_weight", false),
    ("intensity", false),
];

fn matches_pattern(pattern: &str, field: &str) -> bool {
    let (mut p, mut f) = (pattern.split('.'), field.split('.'));
    loop {
        match (p.next(), f.next()) {
            (None, None) => return true,
            (Some(a), Some(b)) if a.starts_with('<') || a == b => {}
            _ => return false,
        }
    }
}

/// Fields the game reads as whole numbers; every other knob is a float.
fn is_integer_field(field: &str) -> bool {
    FIELDS
        .iter()
        .any(|&(pattern, integer)| integer && matches_pattern(pattern, field))
}

impl Knob {
    /// Whether a value for this knob is a whole number.
    pub fn is_integer(&self) -> bool {
        is_integer_field(&self.field)
    }

    /// `value` as this knob stores it: rounded to a whole number, or to six
    /// decimals so a proposal is not a string of float noise.
    pub fn snap(&self, value: f64) -> f64 {
        if self.is_integer() {
            value.round()
        } else {
            (value * 1e6).round() / 1e6
        }
    }
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
        ["services", need, leaf @ ("per_tick" | "radius")] => {
            (&segments[..0], Tail::Service(need, leaf))
        }
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
        Tail::Service(need, leaf) => {
            let value = one(text, open, "services")
                .map_err(|n| no_match(&format!("`services` matched {n} times")))?;
            let list = skip_ws(text, value);
            if text.as_bytes().get(list) != Some(&b'[') {
                return Err(no_match("`services` is not a list"));
            }
            let list_close = matching(text, list).ok_or_else(|| no_match("unbalanced brackets"))?;
            let entries = entries_serving(text, list, list_close, need);
            let [entry] = entries.as_slice() else {
                return Err(no_match(&format!(
                    "`services` has {} entries serving `{need}`",
                    entries.len()
                )));
            };
            let value = one(text, *entry, leaf)
                .map_err(|n| no_match(&format!("`{leaf}` matched {n} times")))?;
            number_at(text, value).ok_or_else(|| no_match(&format!("`{leaf}` is not a number")))
        }
    }
}

enum Tail<'a> {
    Field(&'a str),
    CostAmount(&'a str),
    /// `(need id, leaf)` of the one `services` entry serving that need.
    Service(&'a str, &'a str),
}

/// The `(` of every struct directly inside the list `list..close` whose
/// `need:` is the string `need`.
fn entries_serving(text: &str, list: usize, close: usize, need: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let wanted = format!("\"{need}\"");
    let mut out = Vec::new();
    let mut i = list + 1;
    while i < close {
        if let Some(next) = skip_literal(bytes, i) {
            i = next;
        } else if bytes[i] == b'(' {
            let Some(end) = matching(text, i) else { break };
            if let Ok(value) = one(text, i, "need") {
                let at = skip_ws(text, value);
                if text[at..].starts_with(&wanted) {
                    out.push(i);
                }
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    out
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

fn supported() -> String {
    FIELDS
        .iter()
        .map(|&(pattern, _)| pattern)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The value currently at `knob` in the scratch tree `assets`, read through
/// the engine's own types so a patch that landed on the wrong number shows.
/// A single file is parsed rather than a whole db:
/// `ItemDb::load_dir` needs an `AbilityDb` this has no use for.
pub fn read_back(assets: &Path, knob: &Knob) -> Result<f64, String> {
    let path = assets.join(&knob.file);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let unsupported = || {
        format!(
            "`{}` is not a supported knob field; supported: {}",
            knob.field,
            supported()
        )
    };
    let parse_err = |e: ron::error::SpannedError| format!("{}: {e}", path.display());
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
        ["drain_per_tick" | "working_multiplier" | "critical" | "content" | "morale_weight"] => {
            let need = ron::from_str::<NeedDef>(&text).map_err(parse_err)?;
            Ok(f64::from(match knob.field.as_str() {
                "drain_per_tick" => need.drain_per_tick,
                "working_multiplier" => need.working_multiplier,
                "critical" => need.critical,
                "content" => need.content,
                _ => need.morale_weight,
            }))
        }
        ["intensity"] => Ok(f64::from(
            ron::from_str::<ThoughtDef>(&text)
                .map_err(parse_err)?
                .intensity,
        )),
        ["services", id, leaf @ ("per_tick" | "radius")] => {
            let services = structure()?.services;
            let mut serving = services.iter().filter(|s| s.need.as_str() == *id);
            match (serving.next(), serving.next()) {
                (Some(s), None) if *leaf == "radius" => Ok(f64::from(s.radius)),
                (Some(s), None) => Ok(f64::from(s.per_tick)),
                _ => Err(missing()),
            }
        }
        _ => Err(unsupported()),
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

    /// One real file and concrete path per `FIELDS` entry, so a field added
    /// to the table without a `read_back` arm (or the reverse) fails here.
    #[test]
    fn every_listed_field_reads_back_and_matches_its_integer_flag() {
        let examples = [
            ("capacity", "structures/assembly_bay.ron", "capacity"),
            ("power_draw", "structures/assembly_bay.ron", "power_draw"),
            (
                "work.ticks_per_unit",
                "structures/power_conduit.ron",
                "work.ticks_per_unit",
            ),
            (
                "assembles.ticks_per_unit",
                "structures/assembly_bay.ron",
                "assembles.ticks_per_unit",
            ),
            (
                "craftable.cost.<item>",
                "items/charge_coil.ron",
                "craftable.cost.power_cell",
            ),
            (
                "services.<need>.per_tick",
                "structures/defrag_bay.ron",
                "services.coherence.per_tick",
            ),
            (
                "services.<need>.radius",
                "structures/defrag_bay.ron",
                "services.coherence.radius",
            ),
            ("drain_per_tick", "needs/coherence.ron", "drain_per_tick"),
            (
                "working_multiplier",
                "needs/coherence.ron",
                "working_multiplier",
            ),
            ("critical", "needs/coherence.ron", "critical"),
            ("content", "needs/coherence.ron", "content"),
            ("morale_weight", "needs/coherence.ron", "morale_weight"),
            ("intensity", "thoughts/beside_friend.ron", "intensity"),
        ];
        for &(pattern, integer) in FIELDS {
            let &(_, file, field) = examples
                .iter()
                .find(|(p, ..)| *p == pattern)
                .unwrap_or_else(|| panic!("no example for `{pattern}`"));
            assert!(matches_pattern(pattern, field), "{field}");
            let k = knob(file, field);
            assert_eq!(k.is_integer(), integer, "{field}");
            read_back(&assets(), &k).unwrap_or_else(|e| panic!("{pattern}: {e}"));
        }
        assert_eq!(examples.len(), FIELDS.len());
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
        assert!(
            out.contains("capacity: 9,") && out.contains("capacity: 2, */ )"),
            "{out}"
        );
    }

    #[test]
    fn a_raw_string_is_skipped() {
        let text = "(\n    note: r#\"capacity: 1, \" ) \"#,\n    capacity: 3,\n)";
        let out = patch(text, "capacity", 9.0, true).unwrap();
        assert!(
            out.contains("capacity: 9,") && out.contains("capacity: 1, \" )"),
            "{out}"
        );
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

    fn write_and_read(file: &str, field: &str, value: f64) -> f64 {
        let scratch = ScratchAssets::new(&assets(), "knob_test").unwrap();
        let k = knob(file, field);
        let path = scratch.dir().join(&k.file);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, patch(&text, field, value, k.is_integer()).unwrap()).unwrap();
        read_back(scratch.dir(), &k).unwrap()
    }

    #[test]
    fn every_need_field_round_trips() {
        for (field, v) in [
            ("drain_per_tick", 0.0375),
            ("working_multiplier", 2.5),
            ("critical", 31.0),
            ("content", 64.5),
            ("morale_weight", -4.25),
        ] {
            let got = write_and_read("needs/slack.ron", field, v);
            assert!((got - v).abs() < 1e-6, "{field}: {got}");
        }
    }

    #[test]
    fn a_thought_intensity_round_trips_negative_and_positive() {
        // no_amenity ships at -2.0.
        assert_eq!(
            write_and_read("thoughts/no_amenity.ron", "intensity", -0.5),
            -0.5
        );
        assert_eq!(
            write_and_read("thoughts/no_amenity.ron", "intensity", 1.5),
            1.5
        );
        // beside_friend ships positive; the sign flips cleanly.
        assert_eq!(
            write_and_read("thoughts/beside_friend.ron", "intensity", -3.0),
            -3.0
        );
    }

    #[test]
    fn a_service_per_tick_and_integer_radius_round_trip() {
        let f = "structures/defrag_bay.ron";
        let got = write_and_read(f, "services.coherence.per_tick", 0.85);
        assert!((got - 0.85).abs() < 1e-6, "{got}");
        assert_eq!(write_and_read(f, "services.coherence.radius", 3.4), 3.0);
        let out = patch(&shipped_defrag(), "services.coherence.radius", 3.4, true).unwrap();
        assert!(out.contains("radius: 3)"), "{out}");
    }

    fn shipped_defrag() -> String {
        std::fs::read_to_string(assets().join("structures/defrag_bay.ron")).unwrap()
    }

    const TWO_SERVICES: &str = r#"(
    id: "x",
    services: [
        (need: "coherence", per_tick: 0.6, radius: 1),
        (need: "slack", per_tick: 0.2, radius: 2),
    ],
)"#;

    #[test]
    fn a_service_is_found_by_its_need_and_only_that_entry_moves() {
        let out = patch(TWO_SERVICES, "services.slack.per_tick", 0.9, false).unwrap();
        assert!(
            out.contains("(need: \"slack\", per_tick: 0.9, radius: 2)"),
            "{out}"
        );
        assert!(out.contains("per_tick: 0.6, radius: 1"), "{out}");
        let out = patch(TWO_SERVICES, "services.coherence.radius", 4.0, true).unwrap();
        assert!(
            out.contains("radius: 4)") && out.contains("radius: 2)"),
            "{out}"
        );
    }

    #[test]
    fn a_service_with_no_matching_need_is_an_error() {
        let e = patch(TWO_SERVICES, "services.warmth.per_tick", 1.0, false).unwrap_err();
        assert!(
            e.contains("services.warmth.per_tick") && e.contains("0 entries"),
            "{e}"
        );
        let e = patch(BAY, "services.coherence.radius", 1.0, true).unwrap_err();
        assert!(e.contains("services"), "{e}");
    }

    #[test]
    fn two_services_for_one_need_is_ambiguous() {
        let text = TWO_SERVICES.replace("\"slack\"", "\"coherence\"");
        let e = patch(&text, "services.coherence.per_tick", 1.0, false).unwrap_err();
        assert!(e.contains("2 entries"), "{e}");
    }

    #[test]
    fn integer_ness_follows_the_field() {
        assert!(knob("f", "services.coherence.radius").is_integer());
        assert!(!knob("f", "services.coherence.per_tick").is_integer());
        assert!(!knob("f", "intensity").is_integer());
        assert!(knob("f", "capacity").is_integer());
        assert_eq!(knob("f", "drain_per_tick").snap(0.01234567891), 0.012346);
    }

    #[test]
    fn read_back_refuses_other_fields() {
        let scratch = ScratchAssets::new(&assets(), "knob_test").unwrap();
        let e =
            read_back(scratch.dir(), &knob("structures/assembly_bay.ron", "glyph")).unwrap_err();
        assert!(e.contains("not a supported knob field"), "{e}");
        assert!(
            e.contains("intensity") && e.contains("services.<need>.radius"),
            "{e}"
        );
    }
}
