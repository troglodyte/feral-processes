//! A candidate roster, and how it becomes `.ron` files a human can read.
//!
//! The tuner **patches the original file text** rather than deserialising a
//! `SpeciesDef` and writing it back. A round-trip through `ron` would
//! reformat every line and reorder every field, so the proposal's diff
//! against `assets/species/` would show the whole file changed and tell a
//! reviewer nothing. Patching means `diff` shows exactly the numbers that
//! moved — and a human reading that diff is the only thing standing between
//! this tool and the shipped game.

use std::fmt::Write as _;

/// Decimal places a float field is written to. A search step produces
/// values like `0.6877884316068232`; a person reviewing a diff needs a
/// number, not a residue.
const FLOAT_DECIMALS: i32 = 3;

/// The numeric fields a candidate may move.
///
/// Deliberately a closed set rather than "any number in the file". A
/// species' `glyph`, `habitats`, `moves` and `work_resource` are what it
/// *is*; these six are what it is *worth*, and only the second kind is a
/// tuning question. Widening this is a design decision, not a config change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    BaseHp,
    BaseAtk,
    BaseDef,
    BaseSpeed,
    TamingDifficulty,
    GrowthMultiplier,
}

impl Field {
    pub const ALL: [Field; 6] = [
        Field::BaseHp,
        Field::BaseAtk,
        Field::BaseDef,
        Field::BaseSpeed,
        Field::TamingDifficulty,
        Field::GrowthMultiplier,
    ];

    /// The key exactly as it appears in a species `.ron` file.
    pub fn key(self) -> &'static str {
        match self {
            Field::BaseHp => "base_hp",
            Field::BaseAtk => "base_atk",
            Field::BaseDef => "base_mitigation",
            Field::BaseSpeed => "base_speed",
            Field::TamingDifficulty => "taming_difficulty",
            Field::GrowthMultiplier => "growth_multiplier",
        }
    }

    pub fn from_key(key: &str) -> Option<Field> {
        Field::ALL.into_iter().find(|f| f.key() == key)
    }

    /// Whether this field deserialises into an integer.
    ///
    /// Load-bearing for rendering: writing `base_hp: 42.0` produces a file
    /// the engine refuses to parse, and a species silently dropped from a
    /// candidate roster would be measured as an easier game rather than as
    /// a broken one.
    pub fn is_integer(self) -> bool {
        match self {
            Field::BaseHp | Field::BaseAtk | Field::BaseDef | Field::BaseSpeed => true,
            Field::TamingDifficulty | Field::GrowthMultiplier => false,
        }
    }

    /// This field's value rendered as RON of the right type.
    ///
    /// Floats are rounded to `FLOAT_DECIMALS`, which is a readability rule
    /// with teeth: a proposal is a file a person reviews and commits, and
    /// `taming_difficulty: 0.6877884316068232` is float noise from a search
    /// step rather than a decision anyone made. Rounding happens here, in
    /// the one place a value becomes text, so the number that gets measured
    /// is the same number that gets written — rounding only at proposal
    /// time would report a roster that was never actually fought.
    pub fn render(self, value: f64) -> String {
        if self.is_integer() {
            format!("{}", value.round() as i64)
        } else {
            let factor = 10f64.powi(FLOAT_DECIMALS);
            // `{}` on 1.0f64 prints "1", which RON reads as an integer and
            // serde then refuses for an f32 field.
            let mut s = format!("{}", (value * factor).round() / factor);
            if !s.contains('.') {
                s.push_str(".0");
            }
            s
        }
    }
}

/// Rewrites the named fields in one species file, leaving every other byte
/// alone.
///
/// A field the file does not carry is *inserted* rather than skipped —
/// `growth_multiplier` and `base_speed` are `#[serde(default)]`, so most
/// shipped species omit them, and a candidate that could not move a field
/// simply because its default was implicit would have a different search
/// space per species for no stated reason.
pub fn patch_species(text: &str, changes: &[(Field, f64)]) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();

    for &(field, value) in changes {
        let rendered = field.render(value);
        let existing = lines.iter().position(|line| {
            line.trim_start()
                .strip_prefix(field.key())
                .is_some_and(|rest| rest.starts_with(':'))
        });
        match existing {
            Some(i) => {
                let indent: String = lines[i].chars().take_while(|c| c.is_whitespace()).collect();
                lines[i] = format!("{indent}{}: {rendered},", field.key());
            }
            None => {
                // Before the closing paren, which is the last non-empty
                // line of a species file.
                let close = lines
                    .iter()
                    .rposition(|line| line.trim() == ")")
                    .unwrap_or(lines.len().saturating_sub(1));
                lines.insert(close, format!("    {}: {rendered},", field.key()));
            }
        }
    }

    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Every field's current value for one species, read off the file text.
///
/// Reads the text rather than a parsed `SpeciesDef` so that "what the file
/// says" and "what the patch will replace" are the same question — a field
/// serde defaulted is absent here, which is exactly what `patch_species`
/// needs to know to decide replace-or-insert.
pub fn read_fields(text: &str) -> Vec<(Field, f64)> {
    let mut found = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let Some((key, rest)) = trimmed.split_once(':') else {
            continue;
        };
        let Some(field) = Field::from_key(key.trim()) else {
            continue;
        };
        if let Ok(value) = rest.trim().trim_end_matches(',').parse::<f64>() {
            found.push((field, value));
        }
    }
    found
}

/// A candidate roster: per species id, the fields that differ from shipped.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Candidate {
    pub species: std::collections::BTreeMap<String, Vec<(Field, f64)>>,
}

impl Candidate {
    /// What this candidate changes relative to `baseline`, as
    /// `(species id, field, was, now)`.
    ///
    /// A candidate carries every movable field of every species, most of
    /// them untouched, so listing the candidate is not a report of what the
    /// search decided — it is a hundred lines in which the four that matter
    /// are invisible. Compares rendered text rather than the `f64`s so that
    /// a change too small to survive rounding is not reported as a change:
    /// the file is what a reviewer applies, and a row saying `0.75 -> 0.75`
    /// is noise that costs trust in the whole report.
    pub fn changes_from(&self, baseline: &Candidate) -> Vec<(String, Field, String, String)> {
        let mut out = Vec::new();
        for (id, fields) in &self.species {
            for (field, value) in fields {
                let now = field.render(*value);
                let was = baseline
                    .species
                    .get(id)
                    .and_then(|b| b.iter().find(|(f, _)| f == field))
                    .map(|(_, v)| field.render(*v))
                    .unwrap_or_else(|| "(default)".into());
                if was != now {
                    out.push((id.clone(), *field, was, now));
                }
            }
        }
        out
    }

    /// The changes against `baseline`, one line apiece.
    pub fn summary(&self, baseline: &Candidate) -> String {
        let changes = self.changes_from(baseline);
        if changes.is_empty() {
            return "(nothing moved)\n".into();
        }
        let mut out = String::new();
        for (id, field, was, now) in changes {
            let _ = writeln!(out, "{id}.{} {was} -> {now}", field.key());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped `drone.ron`, which carries `base_speed` but not
    /// `growth_multiplier` — so it exercises both patch paths.
    const DRONE: &str = r#"(
    id: "drone",
    name: "Drone",
    glyph: 'o',
    color: Gray,
    base_hp: 42,
    base_atk: 3,
    base_mitigation: 2,
    taming_difficulty: 0.15,
    habitats: [OpenGrid, Backplane],
    base_speed: 13,
    moves: [
        (name: "Buzz", power: 4),
        (name: "Recon Ping", power: 3, ranged: true),
    ],
    work_resource: Some("core_fragment"),
)
"#;

    #[test]
    fn patching_a_field_changes_only_that_line() {
        let patched = patch_species(DRONE, &[(Field::BaseHp, 50.0)]);
        let before: Vec<&str> = DRONE.lines().collect();
        let after: Vec<&str> = patched.lines().collect();
        assert_eq!(before.len(), after.len());
        let differing: Vec<_> = before
            .iter()
            .zip(&after)
            .filter(|(b, a)| b != a)
            .map(|(_, a)| *a)
            .collect();
        assert_eq!(differing, vec!["    base_hp: 50,"]);
    }

    #[test]
    fn an_absent_field_is_inserted_rather_than_skipped() {
        let patched = patch_species(DRONE, &[(Field::GrowthMultiplier, 1.25)]);
        assert!(
            patched.contains("    growth_multiplier: 1.25,"),
            "growth_multiplier missing from:\n{patched}"
        );
        assert!(patched.trim_end().ends_with(')'), "closing paren lost");
    }

    #[test]
    fn an_integer_field_never_renders_as_a_float() {
        // `base_hp: 42.0` is a file the engine refuses to parse, and a
        // species dropped from a candidate roster measures as an easier
        // game rather than as a broken one.
        let patched = patch_species(DRONE, &[(Field::BaseHp, 49.7)]);
        assert!(patched.contains("    base_hp: 50,"), "got:\n{patched}");
    }

    #[test]
    fn a_float_field_always_renders_with_a_decimal_point() {
        let patched = patch_species(DRONE, &[(Field::GrowthMultiplier, 1.0)]);
        assert!(
            patched.contains("growth_multiplier: 1.0,"),
            "bare `1` reads back as an integer:\n{patched}"
        );
    }

    #[test]
    fn a_patched_file_still_parses_as_the_engine_reads_it() {
        let patched = patch_species(
            DRONE,
            &[
                (Field::BaseHp, 60.0),
                (Field::TamingDifficulty, 0.4),
                (Field::GrowthMultiplier, 1.5),
            ],
        );
        let def: feral_processes_engine::species::SpeciesDef =
            ron::from_str(&patched).expect("patched species must still parse");
        assert_eq!(def.base_hp, 60);
        assert!((def.taming_difficulty - 0.4).abs() < 1e-6);
        assert!((def.growth_multiplier - 1.5).abs() < 1e-6);
    }

    #[test]
    fn reading_fields_back_reports_what_the_file_says() {
        let fields = read_fields(DRONE);
        assert!(fields.contains(&(Field::BaseHp, 42.0)));
        assert!(fields.contains(&(Field::BaseSpeed, 13.0)));
        // Absent from the file, so absent here — that is what tells the
        // patcher to insert rather than replace.
        assert!(!fields.iter().any(|(f, _)| *f == Field::GrowthMultiplier));
    }

    fn candidate(pairs: &[(&str, Field, f64)]) -> Candidate {
        let mut species: std::collections::BTreeMap<String, Vec<(Field, f64)>> = Default::default();
        for (id, field, value) in pairs {
            species
                .entry((*id).into())
                .or_default()
                .push((*field, *value));
        }
        Candidate { species }
    }

    #[test]
    fn a_float_field_is_rounded_rather_than_written_as_search_residue() {
        // A proposal is a file a person reviews and commits, and
        // `0.6877884316068232` is the residue of a search step rather than
        // a decision anyone made.
        assert_eq!(Field::TamingDifficulty.render(0.6877884316068232), "0.688");
    }

    #[test]
    fn the_summary_reports_only_what_moved() {
        // The whole point: a candidate carries every movable field of every
        // species, so listing the candidate buries the few that changed.
        let base = candidate(&[
            ("drone", Field::BaseHp, 42.0),
            ("rootkit", Field::BaseHp, 120.0),
        ]);
        let proposed = candidate(&[
            ("drone", Field::BaseHp, 42.0),
            ("rootkit", Field::BaseHp, 139.0),
        ]);
        let changes = proposed.changes_from(&base);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, "rootkit");
        assert_eq!(
            (changes[0].2.as_str(), changes[0].3.as_str()),
            ("120", "139")
        );
        assert!(!proposed.summary(&base).contains("drone"));
    }

    #[test]
    fn a_change_too_small_to_survive_rounding_is_not_reported() {
        // The file is what a reviewer applies. A row reading `0.75 -> 0.75`
        // is noise that costs trust in every other row.
        let base = candidate(&[("drone", Field::TamingDifficulty, 0.75)]);
        let proposed = candidate(&[("drone", Field::TamingDifficulty, 0.7501)]);
        assert!(proposed.changes_from(&base).is_empty());
    }

    #[test]
    fn a_field_the_baseline_never_had_is_reported_as_a_default() {
        let base = candidate(&[("drone", Field::BaseHp, 42.0)]);
        let proposed = candidate(&[
            ("drone", Field::BaseHp, 42.0),
            ("drone", Field::GrowthMultiplier, 1.4),
        ]);
        let changes = proposed.changes_from(&base);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].2, "(default)");
    }

    #[test]
    fn patching_is_idempotent() {
        let once = patch_species(DRONE, &[(Field::BaseHp, 50.0)]);
        let twice = patch_species(&once, &[(Field::BaseHp, 50.0)]);
        assert_eq!(once, twice);
    }
}
