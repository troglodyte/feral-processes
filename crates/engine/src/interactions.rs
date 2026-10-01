//! What two idle programs can say to each other, loaded from
//! `assets/interactions/`.
//!
//! One def per kind of exchange: which memory it leaves on the listener and
//! the speaker, and how likely it is given the speaker's disposition and the
//! band of the speaker's opinion of the listener. The catalogue is **data**;
//! the pass that rolls and writes is `Game::note_interactions`.
//!
//! **An empty database is valid and inert**, `MemoryDb`'s rule: an absent
//! `assets/interactions/` is silent and means no program ever talks.

use crate::bonds::Bond;
use crate::components::{Memory, MemorySubject, Position, ProgramId};
use crate::derive::{fold, index, unit};
use crate::disposition::Disposition;
use crate::memories::{MemoryDb, MemoryId, MemorySubjectKind};
use crate::tuning::{BOND_WITNESS_REACH, CONVERSATION_MAX_LINES, INTERACTION_SALT};
use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Which of the pair a line belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Speaker,
    Listener,
}

/// One line of an exchange.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub by: Role,
    pub text: String,
}

/// The slot names a line may use, each written `{name}`.
const KNOWN_SLOTS: [&str; 3] = ["speaker", "listener", "topic"];

/// The `{name}` tokens of `text`, without the braces. The one reader of the
/// slot syntax: the loader validates with it and the renderer fills with it,
/// so the two cannot disagree about what a slot is.
pub fn slots(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        let open = rest.find('{')?;
        let close = rest[open..].find('}')? + open;
        let name = &rest[open + 1..close];
        rest = &rest[close + 1..];
        Some(name)
    })
}

/// Whether an exchange can be loaded: a playable length and only known slots.
fn exchange_problem(lines: &[Line]) -> Option<String> {
    if !(2..=CONVERSATION_MAX_LINES).contains(&lines.len()) {
        return Some(format!(
            "{} lines (need 2 to {CONVERSATION_MAX_LINES})",
            lines.len()
        ));
    }
    lines
        .iter()
        .flat_map(|l| slots(&l.text))
        .find(|s| !KNOWN_SLOTS.contains(s))
        .map(|s| format!("unknown slot {{{s}}}"))
}

/// One kind of exchange. `assets/interactions/README.md` is the schema.
#[derive(Clone, Debug, Deserialize)]
pub struct InteractionDef {
    pub id: String,
    /// For the README table and a later screen; not shown yet.
    pub name: String,
    /// Written on the listener, about the speaker. Ignored when `gossip`:
    /// the hearsay def comes from the speaker's own memory's `spreads_as`.
    pub listener_memory: String,
    /// Written on the speaker, about the listener.
    #[serde(default)]
    pub speaker_memory: Option<String>,
    /// Base pick weight; finite and `>= 0`.
    pub weight: f32,
    /// Multiplier on the **speaker's** disposition; absent is `1.0`.
    #[serde(default)]
    pub by_disposition: BTreeMap<Disposition, f32>,
    /// Multiplier on the band of the speaker's opinion of the listener;
    /// absent is `1.0`.
    #[serde(default)]
    pub by_band: BTreeMap<Bond, f32>,
    /// Tells the speaker's strongest tellable memory instead of writing a
    /// fixed one.
    #[serde(default)]
    pub gossip: bool,
    /// What the pair can say, each inner list one back-and-forth. Empty is
    /// valid: the record then renders as the def's `name`.
    #[serde(default)]
    pub exchanges: Vec<Vec<Line>>,
}

impl InteractionDef {
    fn non_finite_field(&self) -> Option<&'static str> {
        let ok = |v: &f32| v.is_finite() && *v >= 0.0;
        if !ok(&self.weight) {
            Some("weight")
        } else if !self.by_disposition.values().all(ok) {
            Some("by_disposition")
        } else if !self.by_band.values().all(ok) {
            Some("by_band")
        } else {
            None
        }
    }

    /// The pick weight for this speaker and band. Gossip availability is the
    /// caller's to apply, because it needs the speaker's memories.
    pub fn weight_for(&self, speaker: Disposition, band: Bond) -> f32 {
        self.weight
            * self.by_disposition.get(&speaker).copied().unwrap_or(1.0)
            * self.by_band.get(&band).copied().unwrap_or(1.0)
    }
}

/// Every interaction the game knows about, in id order.
#[derive(Resource, Default)]
pub struct InteractionDb {
    defs: BTreeMap<String, InteractionDef>,
}

impl InteractionDb {
    /// Loads every `*.ron` def in `dir`, `ThoughtDb::load_dir`'s shape. A def
    /// whose memory ids do not resolve to Program-subject memories in
    /// `memories` is skipped with a
    /// warning, so a mod deleting a memory costs the interactions that wrote
    /// it and nothing else.
    pub fn load_dir(dir: &Path, memories: &MemoryDb) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = InteractionDb::default();
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        // Sorted, so "the first wins" is the same every run.
        paths.sort();
        let resolves = |id: &str| {
            memories
                .get(&MemoryId::from(id))
                .is_some_and(|m| m.subject == MemorySubjectKind::Program)
        };
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            let mut def = match ron::from_str::<InteractionDef>(&text) {
                Ok(def) => def,
                Err(e) => {
                    warnings.push(format!("skipped invalid interaction file {path:?}: {e}"));
                    continue;
                }
            };
            if let Some(field) = def.non_finite_field() {
                warnings.push(format!(
                    "skipped invalid interaction file {path:?}: {field} is negative or not finite"
                ));
                continue;
            }
            if !def.gossip && !resolves(&def.listener_memory) {
                warnings.push(format!(
                    "skipped interaction file {path:?}: listener_memory {:?} is not a Program-subject memory",
                    def.listener_memory
                ));
                continue;
            }
            if let Some(m) = def.speaker_memory.as_deref().filter(|m| !resolves(m)) {
                warnings.push(format!(
                    "skipped interaction file {path:?}: speaker_memory {m:?} is not a Program-subject memory"
                ));
                continue;
            }
            // Reported by its position in the file, which is what the modder
            // can find; the survivors keep their order.
            let mut index = 0;
            def.exchanges.retain(|lines| {
                let problem = exchange_problem(lines);
                if let Some(problem) = &problem {
                    warnings.push(format!(
                        "dropped exchange {index} of interaction file {path:?}: {problem}"
                    ));
                }
                index += 1;
                problem.is_none()
            });
            if db.defs.contains_key(&def.id) {
                warnings.push(format!(
                    "skipped interaction file {path:?}: {:?} is already defined",
                    def.id
                ));
                continue;
            }
            db.defs.insert(def.id.clone(), def);
        }
        Ok((db, warnings))
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// Every def in id order, which the weighted pick depends on being
    /// stable.
    pub fn iter(&self) -> impl Iterator<Item = &InteractionDef> {
        self.defs.values()
    }
}

/// Pairs idle programs for one pass: each unpaired program, in id order,
/// takes the nearest unpaired one within Chebyshev `BOND_WITNESS_REACH`,
/// ties to the lower id. A program is in at most one pair, and the first of
/// a pair is the candidate speaker.
///
/// Only later programs are candidates: an earlier one is already paired, or
/// found nothing in reach, and reach is symmetric.
pub fn pair_idle(idle: &[(ProgramId, Position)]) -> Vec<(ProgramId, ProgramId)> {
    let mut sorted = idle.to_vec();
    sorted.sort_by_key(|(id, _)| *id);
    let mut paired = vec![false; sorted.len()];
    let mut pairs = Vec::new();
    for i in 0..sorted.len() {
        if paired[i] {
            continue;
        }
        let (id, at) = sorted[i];
        let nearest = (i + 1..sorted.len())
            .filter(|&j| !paired[j])
            .map(|j| {
                (
                    j,
                    (at.x - sorted[j].1.x)
                        .abs()
                        .max((at.y - sorted[j].1.y).abs()),
                )
            })
            .filter(|(_, d)| *d <= BOND_WITNESS_REACH)
            // `min_by_key` keeps the first of equals, and the walk is in id
            // order, so a tie resolves to the lower id.
            .min_by_key(|(_, d)| *d);
        if let Some((j, _)) = nearest {
            paired[i] = true;
            paired[j] = true;
            pairs.push((id, sorted[j].0));
        }
    }
    pairs
}

/// The weighted pick of one interaction. `seed` is folded once more, so a
/// caller's roll and its pick are different draws from one seed. A gossip def
/// weighs 0 unless `gossip_ok`; every weight 0, or an empty db, is `None`.
pub fn pick(
    db: &InteractionDb,
    speaker: Disposition,
    band: Bond,
    gossip_ok: bool,
    seed: u64,
) -> Option<&InteractionDef> {
    let weight = |d: &InteractionDef| {
        if d.gossip && !gossip_ok {
            0.0
        } else {
            d.weight_for(speaker, band) as f64
        }
    };
    let total: f64 = db.iter().map(weight).sum();
    if total <= 0.0 {
        return None;
    }
    let mut roll = unit(fold(seed, &[1, INTERACTION_SALT])) * total;
    let mut last = None;
    for def in db.iter().filter(|d| weight(d) > 0.0) {
        roll -= weight(def);
        last = Some(def);
        if roll < 0.0 {
            break;
        }
    }
    last
}

/// One conversation as one program remembers it. Holds no generated text:
/// the page renders it from the current templates, so a retuned or removed
/// template reads through. `tick` orders records and is never shown.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConversationRecord {
    pub tick: u64,
    /// The `InteractionDef` id.
    pub interaction: String,
    /// Index into the def's `exchanges`; `None` renders the fallback line.
    pub exchange: Option<u8>,
    /// This program's side; the other program's record has the opposite.
    pub role: Role,
    pub other: ProgramId,
    /// Stamped at the write; a live program's current name wins on read.
    pub other_name: String,
    pub topic: Option<(MemorySubject, String)>,
}

/// What `speaker` is on about: one of its own memories' subjects, never
/// itself, the listener or no subject at all. Subjects are taken in the
/// store's order, de-duplicated, and one is picked by `seed`, so the same
/// state names the same topic.
pub fn topic(
    memories: &[Memory],
    speaker: ProgramId,
    listener: ProgramId,
    seed: u64,
) -> Option<MemorySubject> {
    let mut subjects: Vec<&MemorySubject> = Vec::new();
    for m in memories {
        let excluded = match &m.subject {
            MemorySubject::Nothing => true,
            MemorySubject::Program(p) => *p == speaker || *p == listener,
            _ => false,
        };
        if !excluded && !subjects.contains(&&m.subject) {
            subjects.push(&m.subject);
        }
    }
    if subjects.is_empty() {
        return None;
    }
    Some(subjects[index(fold(seed, &[2, INTERACTION_SALT]), subjects.len())].clone())
}

/// Which of `def`'s exchanges is said: any when there is a topic, otherwise
/// only those that never name one. `None` when nothing qualifies, which the
/// reader renders as the fallback line.
pub fn pick_exchange(def: &InteractionDef, has_topic: bool, seed: u64) -> Option<u8> {
    let fits: Vec<usize> = def
        .exchanges
        .iter()
        .enumerate()
        .filter(|(_, lines)| {
            has_topic
                || !lines
                    .iter()
                    .flat_map(|l| slots(&l.text))
                    .any(|s| s == "topic")
        })
        .map(|(i, _)| i)
        .collect();
    if fits.is_empty() {
        return None;
    }
    u8::try_from(fits[index(fold(seed, &[3, INTERACTION_SALT]), fits.len())]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_text(id: &str) -> String {
        format!(
            "(id: \"{id}\", name: \"n\", blurb: \"b\", valence: 1.0, half_life: 100, \
             subject: Program, strike_cap: 3)"
        )
    }

    fn memories() -> MemoryDb {
        let dir = crate::tests::support::scratch_assets_dir("interactions_memories");
        std::fs::create_dir_all(&*dir).unwrap();
        for id in ["chatted_with", "insulted_by"] {
            std::fs::write(dir.join(format!("{id}.ron")), memory_text(id)).unwrap();
        }
        let (db, w) = MemoryDb::load_dir(&dir).unwrap();
        assert!(w.is_empty(), "{w:?}");
        db
    }

    fn def_text(id: &str, extra: &str) -> String {
        format!(
            "(id: \"{id}\", name: \"n\", listener_memory: \"chatted_with\", weight: 1.0, {extra})"
        )
    }

    fn load(files: &[(&str, String)]) -> (InteractionDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir("interactions");
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        InteractionDb::load_dir(&dir, &memories()).unwrap()
    }

    #[test]
    fn a_well_formed_def_loads_and_iterates_in_id_order() {
        let (db, w) = load(&[
            ("z.ron", def_text("zeta", "")),
            (
                "a.ron",
                def_text(
                    "alpha",
                    "speaker_memory: Some(\"insulted_by\"), by_band: {Enemy: 4.0}",
                ),
            ),
        ]);
        assert!(w.is_empty(), "{w:?}");
        let ids: Vec<_> = db.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["alpha", "zeta"]);
        let alpha = db.iter().next().unwrap();
        assert_eq!(alpha.weight_for(Disposition::Steady, Bond::Enemy), 4.0);
        assert_eq!(alpha.weight_for(Disposition::Steady, Bond::Close), 1.0);
    }

    #[test]
    fn an_absent_directory_is_an_empty_db_without_warnings() {
        let dir = crate::tests::support::scratch_assets_dir("interactions_absent");
        let (db, w) = InteractionDb::load_dir(&dir.join("nope"), &memories()).unwrap();
        assert!(db.is_empty() && w.is_empty());
    }

    #[test]
    fn a_malformed_file_is_skipped_without_losing_its_neighbours() {
        let (db, w) = load(&[
            ("bad.ron", "(id: \"x\", name:".to_string()),
            ("good.ron", def_text("good", "")),
        ]);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(db.iter().count(), 1);
    }

    #[test]
    fn a_bad_weight_or_multiplier_is_skipped() {
        let (db, w) = load(&[
            (
                "a.ron",
                def_text("a", "").replace("weight: 1.0", "weight: -1.0"),
            ),
            (
                "b.ron",
                def_text("b", "").replace("weight: 1.0", "weight: inf"),
            ),
            ("c.ron", def_text("c", "by_band: {Close: -0.5}")),
            ("d.ron", def_text("d", "by_disposition: {Amiable: NaN}")),
        ]);
        assert_eq!(w.len(), 4, "{w:?}");
        assert!(db.is_empty());
    }

    fn exchange(lines: &[(&str, &str)]) -> String {
        let lines: Vec<String> = lines
            .iter()
            .map(|(by, text)| format!("(by: {by}, text: \"{text}\")"))
            .collect();
        format!("[{}]", lines.join(", "))
    }

    #[test]
    fn slots_yields_each_braced_name() {
        let got: Vec<_> = slots("a {x} b {y}{z} {open").collect();
        assert_eq!(got, ["x", "y", "z"]);
    }

    #[test]
    fn a_bad_exchange_is_dropped_with_a_warning_and_the_def_loads() {
        let ok = exchange(&[("Speaker", "hi {topic}"), ("Listener", "ok {speaker}")]);
        let unknown = exchange(&[("Speaker", "hi {nope}"), ("Listener", "ok")]);
        let one = exchange(&[("Speaker", "hi")]);
        let five = exchange(&[("Speaker", "a"); 5]);
        for (bad, name) in [(unknown, "unknown"), (one, "one"), (five, "five")] {
            let (db, w) = load(&[("a.ron", def_text("a", &format!("exchanges: [{bad}, {ok}]")))]);
            assert_eq!(w.len(), 1, "{name}: {w:?}");
            assert!(w[0].contains("exchange 0"), "{name}: {w:?}");
            let def = db.iter().next().unwrap_or_else(|| panic!("{name}: no def"));
            assert_eq!(def.exchanges.len(), 1, "{name}");
            assert_eq!(def.exchanges[0][0].by, Role::Speaker);
        }
    }

    #[test]
    fn a_def_without_exchanges_loads_clean() {
        let (db, w) = load(&[("a.ron", def_text("a", ""))]);
        assert!(w.is_empty(), "{w:?}");
        assert!(db.iter().next().unwrap().exchanges.is_empty());
    }

    fn memory_about(subject: MemorySubject) -> Memory {
        Memory {
            def: MemoryId::from("chatted_with"),
            subject,
            subject_name: None,
            reinforced: 0,
            strikes: 1,
        }
    }

    #[test]
    fn topic_is_never_the_speaker_the_listener_or_nothing() {
        let me = ProgramId(1);
        let you = ProgramId(2);
        let them = ProgramId(3);
        let held = [
            memory_about(MemorySubject::Nothing),
            memory_about(MemorySubject::Program(me)),
            memory_about(MemorySubject::Program(you)),
            memory_about(MemorySubject::Program(them)),
            memory_about(MemorySubject::Program(them)),
        ];
        for seed in 0..50 {
            assert_eq!(
                topic(&held, me, you, seed),
                Some(MemorySubject::Program(them))
            );
        }
    }

    #[test]
    fn topic_with_no_candidates_is_none() {
        let me = ProgramId(1);
        let you = ProgramId(2);
        assert_eq!(topic(&[], me, you, 0), None);
        let held = [
            memory_about(MemorySubject::Nothing),
            memory_about(MemorySubject::Program(you)),
        ];
        assert_eq!(topic(&held, me, you, 0), None);
    }

    #[test]
    fn topic_reaches_every_candidate_across_seeds() {
        let held = [
            memory_about(MemorySubject::Program(ProgramId(5))),
            memory_about(MemorySubject::Program(ProgramId(6))),
        ];
        let seen: std::collections::BTreeSet<_> = (0..64)
            .filter_map(|s| topic(&held, ProgramId(1), ProgramId(2), s))
            .map(|t| format!("{t:?}"))
            .collect();
        assert_eq!(seen.len(), 2);
    }

    fn def_with(exchanges: &[&str]) -> InteractionDef {
        let lines: Vec<String> = exchanges
            .iter()
            .map(|t| exchange(&[("Speaker", t), ("Listener", "ok")]))
            .collect();
        let (db, w) = load(&[(
            "a.ron",
            def_text("a", &format!("exchanges: [{}]", lines.join(", "))),
        )]);
        assert!(w.is_empty(), "{w:?}");
        db.iter().next().unwrap().clone()
    }

    #[test]
    fn without_a_topic_only_topic_free_exchanges_are_picked() {
        let def = def_with(&["about {topic}", "plain", "also {topic}", "plain too"]);
        for seed in 0..50 {
            let i = pick_exchange(&def, false, seed).unwrap();
            assert!(i == 1 || i == 3, "{i}");
        }
        let seen: std::collections::BTreeSet<_> = (0..64)
            .filter_map(|s| pick_exchange(&def, true, s))
            .collect();
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn no_qualifying_exchange_is_none() {
        assert_eq!(pick_exchange(&def_with(&[]), true, 0), None);
        assert_eq!(pick_exchange(&def_with(&["about {topic}"]), false, 0), None);
    }

    #[test]
    fn the_ring_keeps_the_newest_and_drops_the_oldest() {
        use crate::components::Conversations;
        let mut ring = Conversations::default();
        for tick in 0..=crate::tuning::CONVERSATION_RING as u64 {
            ring.push(ConversationRecord {
                tick,
                interaction: "talk".into(),
                exchange: None,
                role: Role::Speaker,
                other: ProgramId(2),
                other_name: "x".into(),
                topic: None,
            });
        }
        assert_eq!(ring.0.len(), crate::tuning::CONVERSATION_RING);
        assert_eq!(ring.0.front().unwrap().tick, 32);
        assert_eq!(ring.0.back().unwrap().tick, 1);
    }

    #[test]
    fn a_duplicate_id_warns_and_the_first_file_wins() {
        let (db, w) = load(&[
            (
                "a.ron",
                def_text("same", "").replace("weight: 1.0", "weight: 2.0"),
            ),
            ("b.ron", def_text("same", "")),
        ]);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(db.iter().next().unwrap().weight, 2.0);
    }

    #[test]
    fn an_unresolved_memory_id_is_skipped() {
        let (db, w) = load(&[
            ("a.ron", def_text("a", "").replace("chatted_with", "nope")),
            ("b.ron", def_text("b", "speaker_memory: Some(\"nope\")")),
        ]);
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(db.is_empty());
    }

    #[test]
    fn a_non_program_subject_memory_is_skipped() {
        let dir = crate::tests::support::scratch_assets_dir("interactions_nothing_memory");
        std::fs::create_dir_all(&*dir).unwrap();
        std::fs::write(
            dir.join("bare.ron"),
            memory_text("bare").replace("subject: Program", "subject: Nothing"),
        )
        .unwrap();
        std::fs::write(dir.join("chatted_with.ron"), memory_text("chatted_with")).unwrap();
        let (mem, w) = MemoryDb::load_dir(&dir).unwrap();
        assert!(w.is_empty(), "{w:?}");
        let dir = crate::tests::support::scratch_assets_dir("interactions_nothing_defs");
        std::fs::create_dir_all(&*dir).unwrap();
        std::fs::write(
            dir.join("a.ron"),
            def_text("a", "").replace("chatted_with", "bare"),
        )
        .unwrap();
        std::fs::write(
            dir.join("b.ron"),
            def_text("b", "speaker_memory: Some(\"bare\")"),
        )
        .unwrap();
        let (db, w) = InteractionDb::load_dir(&dir, &mem).unwrap();
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(db.is_empty());
    }

    #[test]
    fn a_gossip_defs_listener_memory_is_not_resolved() {
        let (db, w) = load(&[(
            "g.ron",
            def_text("gossip", "gossip: true").replace("chatted_with", "ignored"),
        )]);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(db.iter().count(), 1);
    }
}

#[cfg(test)]
mod pass_tests {
    use super::*;

    fn at(id: u32, x: i32, y: i32) -> (ProgramId, Position) {
        (ProgramId(id), Position { x, y })
    }

    fn ids(pairs: Vec<(ProgramId, ProgramId)>) -> Vec<(u32, u32)> {
        pairs.into_iter().map(|(a, b)| (a.0, b.0)).collect()
    }

    #[test]
    fn a_program_pairs_with_the_nearest_in_reach() {
        let got = pair_idle(&[at(1, 0, 0), at(2, 2, 0), at(3, 1, 0)]);
        assert_eq!(ids(got), [(1, 3)]);
    }

    #[test]
    fn out_of_reach_programs_stay_unpaired() {
        let far = BOND_WITNESS_REACH + 1;
        assert!(pair_idle(&[at(1, 0, 0), at(2, far, 0)]).is_empty());
        assert_eq!(
            ids(pair_idle(&[
                at(1, 0, 0),
                at(2, BOND_WITNESS_REACH, BOND_WITNESS_REACH)
            ])),
            [(1, 2)]
        );
    }

    #[test]
    fn ties_go_to_the_lower_id_whatever_the_input_order() {
        let got = pair_idle(&[at(9, 1, 0), at(1, 0, 0), at(5, -1, 0)]);
        assert_eq!(ids(got), [(1, 5)]);
    }

    #[test]
    fn an_odd_count_leaves_one_out_and_nobody_pairs_twice() {
        let got = pair_idle(&[at(1, 0, 0), at(2, 1, 0), at(3, 2, 0)]);
        assert_eq!(ids(got), [(1, 2)]);
    }

    #[test]
    fn empty_and_single_inputs_pair_nothing() {
        assert!(pair_idle(&[]).is_empty());
        assert!(pair_idle(&[at(1, 0, 0)]).is_empty());
    }

    fn shipped_shape() -> InteractionDb {
        let mdir = crate::tests::support::scratch_assets_dir("pick_memories");
        std::fs::create_dir_all(&*mdir).unwrap();
        for id in [
            "chatted_with",
            "complimented_by",
            "slighted_by",
            "insulted_by",
        ] {
            std::fs::write(
                mdir.join(format!("{id}.ron")),
                format!(
                    "(id: \"{id}\", name: \"n\", blurb: \"b\", valence: 1.0, half_life: 100, \
                     subject: Program, strike_cap: 3)"
                ),
            )
            .unwrap();
        }
        let (mem, w) = MemoryDb::load_dir(&mdir).unwrap();
        assert!(w.is_empty(), "{w:?}");
        let dir = crate::tests::support::scratch_assets_dir("pick_interactions");
        std::fs::create_dir_all(&*dir).unwrap();
        let defs = [
            (
                "small_talk",
                "chatted_with",
                3.0,
                "{Enemy: 0.3, Rival: 0.6, Neutral: 1.0, Friend: 1.3, Close: 1.5}",
                "{Amiable: 1.3}",
                false,
            ),
            (
                "compliment",
                "complimented_by",
                1.0,
                "{Enemy: 0.05, Rival: 0.2, Neutral: 1.0, Friend: 1.5, Close: 2.0}",
                "{Amiable: 1.8, Abrasive: 0.3}",
                false,
            ),
            (
                "slight",
                "slighted_by",
                0.6,
                "{Enemy: 3.0, Rival: 2.5, Neutral: 1.0, Friend: 0.3, Close: 0.1}",
                "{Abrasive: 2.5, Amiable: 0.3}",
                false,
            ),
            (
                "insult",
                "insulted_by",
                0.3,
                "{Enemy: 4.0, Rival: 3.0, Neutral: 0.3, Friend: 0.02, Close: 0.01}",
                "{Abrasive: 3.0, Amiable: 0.1}",
                false,
            ),
            ("gossip", "ignored", 1.0, "{}", "{}", true),
        ];
        for (id, mem_id, weight, band, disp, gossip) in defs {
            std::fs::write(
                dir.join(format!("{id}.ron")),
                format!(
                    "(id: \"{id}\", name: \"n\", listener_memory: \"{mem_id}\", weight: {weight}, \
                     by_band: {band}, by_disposition: {disp}, gossip: {gossip})"
                ),
            )
            .unwrap();
        }
        let (db, w) = InteractionDb::load_dir(&dir, &mem).unwrap();
        assert!(w.is_empty(), "{w:?}");
        db
    }

    fn tally(
        db: &InteractionDb,
        d: Disposition,
        b: Bond,
        gossip_ok: bool,
    ) -> BTreeMap<String, u32> {
        let mut counts = BTreeMap::new();
        for n in 0..200u64 {
            let seed = fold(crate::derive::FNV_BASIS, &[n, INTERACTION_SALT]);
            if let Some(def) = pick(db, d, b, gossip_ok, seed) {
                *counts.entry(def.id.clone()).or_insert(0) += 1;
            }
        }
        counts
    }

    #[test]
    fn rivals_slight_and_insult_more_than_they_compliment() {
        let c = tally(&shipped_shape(), Disposition::Abrasive, Bond::Rival, false);
        let n = |k: &str| c.get(k).copied().unwrap_or(0);
        assert!(n("slight") + n("insult") > n("compliment"), "{c:?}");
    }

    #[test]
    fn close_friends_compliment_more_than_they_slight_and_insult() {
        let c = tally(&shipped_shape(), Disposition::Amiable, Bond::Close, false);
        let n = |k: &str| c.get(k).copied().unwrap_or(0);
        assert!(n("compliment") > n("slight") + n("insult"), "{c:?}");
    }

    #[test]
    fn gossip_is_never_picked_when_not_ok_and_can_be_when_ok() {
        let db = shipped_shape();
        assert!(!tally(&db, Disposition::Steady, Bond::Close, false).contains_key("gossip"));
        assert!(tally(&db, Disposition::Steady, Bond::Close, true).contains_key("gossip"));
    }

    #[test]
    fn all_zero_weights_and_an_empty_db_pick_nothing() {
        let db = shipped_shape();
        // A db holding only gossip is all-zero while `gossip_ok` is false.
        let only_gossip = InteractionDb {
            defs: db
                .defs
                .iter()
                .filter(|(_, d)| d.gossip)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        };
        assert!(pick(&only_gossip, Disposition::Steady, Bond::Neutral, false, 5).is_none());
        assert!(
            pick(
                &InteractionDb::default(),
                Disposition::Steady,
                Bond::Neutral,
                true,
                5
            )
            .is_none()
        );
    }
}
