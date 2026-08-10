//! What a cell of a Stack frame has to say for itself.
//!
//! Content, like species and items — `assets/descriptions/*.ron`, one file
//! per subject, so adding prose is a file drop rather than a code change.
//! This is `crash_logs.rs` generalised and absorbed: pools keyed by a
//! **subject** instead of one flat unkeyed pool, plus slot composition, so
//! one system covers the rot, the doors and everything else rather than two
//! systems covering one cell kind each.
//!
//! **Which fragment a given cell reads is a property of the place** —
//! derived from the frame spec and the cell coordinates, never from
//! `resources::GameRng`. Two reasons, both already learned in
//! `crash_logs.rs`: a draw from the shared stream does not survive a
//! save/load, so the same door would say something different after a
//! reload; and drawing from it to pick a cosmetic string shifts every later
//! roll in the run. Selection is a modulo of an FNV-1a fold rather than an
//! `StdRng` for a third reason — `StdRng`'s output sequence is not
//! guaranteed stable across a `rand` upgrade, and a dependency bump that
//! silently reshuffled every description in the game is exactly the failure
//! worth designing out.
//!
//! This module knows nothing about `Game`. It takes a subject, a condition
//! and a seed; `game/descriptions.rs` owns translating a `CellKind` into the
//! first two and mixing the third.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::Deserialize;

/// One bank file: the subject it describes, and the variants it contributes.
#[derive(Clone, Debug, Deserialize)]
pub struct DescriptionDef {
    /// A dotted domain key — `"stack.door"`, `"stack.cache"`. A `String`
    /// following the `ItemId` precedent rather than an enum, and that is the
    /// whole expansion seam: `"biome.forest"` later is a file drop with no
    /// code change.
    pub subject: String,
    #[serde(default)]
    pub variants: Vec<DescriptionVariant>,
}

/// One reading of a subject, under one condition.
///
/// Three *lengths* of the same content, because they land in three places
/// with different room. They are not truncations of each other: each is
/// authored for where it goes.
#[derive(Clone, Debug, Deserialize)]
pub struct DescriptionVariant {
    /// `None` is the fallback, used when no other variant on this subject
    /// matches.
    ///
    /// Two variants sharing a `when` — in one file or across two — have
    /// their pools concatenated rather than racing, so a mod adds fragments
    /// to a shipped door instead of replacing it and nothing an author
    /// writes goes silently dead. See `DescriptionVariant::absorb`.
    #[serde(default)]
    pub when: Option<String>,
    /// The descriptive clause of the `standing_on` row. Bounded in length:
    /// that row is centred, pixel-measured and **unwrapped**. No `{bearing}`
    /// — you are standing on it.
    #[serde(default)]
    pub underfoot: Vec<String>,
    /// One log line, for a cell coming into view. The log pane draws exactly
    /// one row per line with no wrapping, so this is one sentence by
    /// construction rather than a truncated paragraph. May use `{bearing}`.
    #[serde(default)]
    pub sighted: Vec<String>,
    /// The examine paragraph's first sentence.
    #[serde(default)]
    pub openers: Vec<String>,
    /// Its second. `""` is legal and lets a draw legitimately come out
    /// shorter.
    #[serde(default)]
    pub details: Vec<String>,
    /// Its third. `""` is legal, as in `details`.
    #[serde(default)]
    pub codas: Vec<String>,
}

impl DescriptionVariant {
    /// Concatenates `other`'s pools onto this one's, slot by slot.
    ///
    /// Two files describing one subject under one condition are **additive**
    /// — a mod adds fragments to the shipped door rather than replacing it,
    /// and neither file's prose goes silently dead. First-match-wins would
    /// make the loser dead content with nothing on screen to say so, which
    /// is the failure `crash_logs`' flat pool never had.
    ///
    /// This is also what makes `merge`'s sort load-bearing rather than
    /// cosmetic: the pool a cell indexes into has to be in the same order
    /// every run, and concatenation order is that order.
    fn absorb(&mut self, other: DescriptionVariant) {
        self.underfoot.extend(other.underfoot);
        self.sighted.extend(other.sighted);
        self.openers.extend(other.openers);
        self.details.extend(other.details);
        self.codas.extend(other.codas);
    }
}

/// Which pool a draw comes from.
///
/// Carries two fold words rather than one — the *length* and the *slot
/// within it* — so the three lengths of one cell diverge, and so do the
/// three sentences of one paragraph. Folding a single tag would work today
/// and would quietly stop working the moment a fourth slot was added to an
/// existing length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Underfoot,
    Sighted,
    Opener,
    Detail,
    Coda,
}

impl Slot {
    fn tags(self) -> [u64; 2] {
        match self {
            Slot::Underfoot => [0, 0],
            Slot::Sighted => [1, 0],
            Slot::Opener => [2, 0],
            Slot::Detail => [2, 1],
            Slot::Coda => [2, 2],
        }
    }
}

/// Every shipped and modded description, keyed by subject.
///
/// A `BTreeMap` rather than a `HashMap` so `subjects()` is ordered, which
/// makes the census test's failure message readable and a warning list
/// reproducible.
#[derive(Resource, Default)]
pub struct DescriptionDb {
    subjects: BTreeMap<String, Vec<DescriptionVariant>>,
}

impl DescriptionDb {
    /// Loads every `*.ron` bank file in `dir`. A malformed file is skipped
    /// with a returned warning rather than aborting the load, same as
    /// `AbilityDb::load_dir` and `CrashLogDb::load_dir`.
    ///
    /// IO and ordering are deliberately separate calls — see `merge` for why
    /// the sort has to be tested against it directly rather than through
    /// this method.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut defs: Vec<(String, DescriptionDef)> = Vec::new();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let file_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<DescriptionDef>(&text) {
                Ok(def) => defs.push((file_id, def)),
                Err(e) => warnings.push(format!("skipped invalid description file {path:?}: {e}")),
            }
        }
        let subjects = merge(defs);
        Ok((DescriptionDb { subjects }, warnings))
    }

    /// Every loaded subject, in key order. For the census test and for
    /// reporting; nothing in play iterates the bank.
    pub fn subjects(&self) -> impl Iterator<Item = &str> {
        self.subjects.keys().map(String::as_str)
    }

    /// How many distinct conditions `subject` carries — one per `when`,
    /// including the fallback, since `load_dir` merges duplicates.
    pub fn variant_count(&self, subject: &str) -> usize {
        self.subjects.get(subject).map_or(0, Vec::len)
    }

    /// The variant of `subject` matching `condition`, falling back to the
    /// `when: None` one.
    ///
    /// A missing condition falls back rather than returning nothing, so
    /// authoring a new spent state is additive: the bank keeps answering
    /// with the general reading until someone writes the specific one.
    fn variant(&self, subject: &str, condition: Option<&str>) -> Option<&DescriptionVariant> {
        let variants = self.subjects.get(subject)?;
        condition
            .and_then(|c| variants.iter().find(|v| v.when.as_deref() == Some(c)))
            .or_else(|| variants.iter().find(|v| v.when.is_none()))
    }

    /// One line for the row under the first-person view, or `None` when the
    /// bank has nothing for this subject — which is a mod's prerogative and
    /// leaves the caller free to fall back to its own literal.
    pub(crate) fn underfoot(
        &self,
        subject: &str,
        condition: Option<&str>,
        seed: u64,
    ) -> Option<&str> {
        let v = self.variant(subject, condition)?;
        pick(&v.underfoot, fold(seed, Slot::Underfoot))
    }

    /// One log line for a cell coming into view. May contain `{bearing}`;
    /// the caller fills it.
    pub(crate) fn sighted(
        &self,
        subject: &str,
        condition: Option<&str>,
        seed: u64,
    ) -> Option<&str> {
        let v = self.variant(subject, condition)?;
        pick(&v.sighted, fold(seed, Slot::Sighted))
    }

    /// The examine paragraph — opener, detail and coda joined with a single
    /// space, empty fragments dropped.
    ///
    /// `None` when there is no opener: a paragraph that is only a detail is
    /// not a reading of anything, and the caller has a corridor fallback for
    /// exactly that case. May contain `{bearing}`.
    ///
    /// An opener that picks as `""` counts as no opener, unlike `details`
    /// and `codas` where `""` is a legal, blessed shorter draw: the opener
    /// is the sentence that makes this a *reading of something*, so an
    /// empty one is authoring error rather than an intentional short form.
    pub(crate) fn paragraph(
        &self,
        subject: &str,
        condition: Option<&str>,
        seed: u64,
    ) -> Option<String> {
        let v = self.variant(subject, condition)?;
        let opener = pick(&v.openers, fold(seed, Slot::Opener))?;
        if opener.is_empty() {
            return None;
        }
        let detail = pick(&v.details, fold(seed, Slot::Detail)).unwrap_or_default();
        let coda = pick(&v.codas, fold(seed, Slot::Coda)).unwrap_or_default();
        Some(
            [opener, detail, coda]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

/// Sorts `defs` by `(subject, file id)` and folds them into one pool per
/// subject, merging variants that share a condition via
/// `DescriptionVariant::absorb`.
///
/// Split out of `load_dir` and kept `pub(crate)` — rather than inlined and
/// private — because the sort is the one load-bearing line in this module
/// and it cannot be pinned by a test that drives `load_dir`'s directory
/// walk. `std::fs::read_dir`'s order is unspecified, but on at least one
/// real filesystem it already comes back alphabetically — the same order
/// the sort produces — so a `load_dir`-level test can pass with the
/// `sort_by` below deleted and prove nothing. Calling `merge` directly with
/// a deliberately mis-ordered `Vec` is the only way to exercise the sort
/// itself; `tests::descriptions` does exactly that.
pub(crate) fn merge(
    mut defs: Vec<(String, DescriptionDef)>,
) -> BTreeMap<String, Vec<DescriptionVariant>> {
    defs.sort_by(|(a_id, a), (b_id, b)| (&a.subject, a_id).cmp(&(&b.subject, b_id)));

    let mut subjects: BTreeMap<String, Vec<DescriptionVariant>> = BTreeMap::new();
    for (_, def) in defs {
        let variants = subjects.entry(def.subject).or_default();
        for incoming in def.variants {
            match variants.iter_mut().find(|v| v.when == incoming.when) {
                Some(existing) => existing.absorb(incoming),
                None => variants.push(incoming),
            }
        }
    }
    subjects
}

/// Continues the caller's fold with the slot's own two words, so the three
/// lengths of one cell — and the three sentences of one paragraph — do not
/// all land on index 0 of their pools together.
///
/// Deliberately the same FNV-1a step `FrameSpec::salted` uses. Two schemes
/// mixing the same seed differently is how a description ends up stable in
/// one length and not in another.
fn fold(seed: u64, slot: Slot) -> u64 {
    let mut h = seed;
    for word in slot.tags() {
        h ^= word;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Indexes `pool` by `seed`, or `None` when the pool is empty — which is
/// legal at every slot and is how a two-sentence paragraph is authored.
fn pick(pool: &[String], seed: u64) -> Option<&str> {
    if pool.is_empty() {
        return None;
    }
    Some(&pool[(seed % pool.len() as u64) as usize])
}
