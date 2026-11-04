//! The research-pace measurement behind the base AI's balance table
//! (`docs/measurements/2026-11-04-base-ai-research-pace.md`). Both tests are
//! `#[ignore]`d and print; they assert nothing. Run with
//! `cargo test -p feral-processes-engine storyline::measure -- --ignored --nocapture`.
//!
//! The ordinary pace is read off real `dev-saves/` through
//! `Game::host_research_pace`, the call the fusion itself freezes. The tree
//! walk calls `pace` and, for the live figures, `practise`, `analysis_cap`
//! and `training_cost`. `Proposed` is the one stand-in: the same shapes over
//! the proposed figures, so the table can be measured before it is applied.
//! Once applied, `Live` must print what `Proposed` printed and `Proposed` is
//! deleted.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::*;
use crate::research::{ResearchDb, ResearchTree};
use crate::tuning::{BASE_AI_FUGUE_TICKS, BASE_AI_LEVEL_OFFSETS};
use crate::*;

const TEMPLATES: [&str; 4] = ["fred", "study", "extraction", "chains"];

/// The world's own rate (`app_core::WORLD_SPEED_MULTIPLIER`).
const TICKS_PER_SECOND: f64 = 2.0;

fn real_assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn load_template(name: &str) -> Game {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../dev-saves/{name}.ron"));
    let text = std::fs::read_to_string(&src).unwrap();
    let data = crate::save::from_ron(&text).unwrap();
    let path = std::env::temp_dir().join(format!(
        "feral_processes_ai_pace_measure_{name}_{}.bin",
        std::process::id()
    ));
    crate::save::save_to_file(&path, &data).unwrap();
    let game = Game::load(&path, &real_assets()).unwrap();
    let _ = std::fs::remove_file(&path);
    game
}

struct StaffPace {
    id: ProgramId,
    species: String,
    analysis: i32,
    pace: f64,
}

/// Every base staff program's ordinary pace at the first Research Station,
/// and the host the claim would pick.
fn staff_paces(game: &mut Game) -> Option<(Vec<StaffPace>, ProgramId, usize, usize)> {
    let stations = game.research_stations();
    let anchor = stations
        .iter()
        .filter_map(|&e| game.world.get::<Position>(e).map(|p| (p.x, p.y)))
        .min()?;
    let posted = game
        .world
        .query::<&Task>()
        .iter(&game.world)
        .filter(|t| stations.contains(&t.target))
        .count();
    let mut rows = Vec::new();
    for e in game.base_staff() {
        let Some(&id) = game.world.get::<ProgramId>(e) else {
            continue;
        };
        let species = game
            .world
            .get::<Creature>(e)
            .map_or_else(String::new, |c| c.species.to_string());
        let analysis = game.analysis_of_program(e);
        let pace = game.host_research_pace(e, anchor);
        rows.push(StaffPace {
            id,
            species,
            analysis,
            pace,
        });
    }
    rows.sort_by_key(|r| (std::cmp::Reverse(r.analysis), r.id));
    let candidates: Vec<HostCandidate> = rows
        .iter()
        .map(|r| HostCandidate {
            id: r.id,
            analysis: r.analysis,
            free: true,
        })
        .collect();
    let host = pick_host(&candidates)?;
    Some((rows, host, stations.len(), posted))
}

#[test]
#[ignore = "measurement; prints"]
fn measure_ordinary_research_pace() {
    for name in TEMPLATES {
        let mut game = load_template(name);
        let zone = game.world.resource::<ZoneLevel>().0;
        let done = game.world.resource::<Research>().0.len();
        let Some((rows, host, stations, posted)) = staff_paces(&mut game) else {
            println!("== {name}: no Research Station or no staff");
            continue;
        };
        println!(
            "== {name}: zone {zone}, {done} researched, {stations} station(s), {posted} posted, {} staff",
            rows.len()
        );
        for r in &rows {
            let mark = if r.id == host { " <- host" } else { "" };
            println!(
                "  {:>4} {:<22} analysis {:>3}  pace {:.5}/tick{mark}",
                r.id.0, r.species, r.analysis, r.pace
            );
        }
        let mut paces: Vec<f64> = rows.iter().map(|r| r.pace).collect();
        paces.sort_by(f64::total_cmp);
        let mut analyses: Vec<i32> = rows.iter().map(|r| r.analysis).collect();
        analyses.sort();
        println!(
            "  median analysis {}, max {}; pace min {:.5} median {:.5} max {:.5}",
            analyses[analyses.len() / 2],
            analyses[analyses.len() - 1],
            paces[0],
            paces[paces.len() / 2],
            paces[paces.len() - 1],
        );
    }
}

/// The AI's growth knobs: the live tuning, or the proposed table.
trait Growth {
    fn levels(&self) -> u32;
    fn practise(&self, f: &FusedAi, analysis: i32) -> PracticeOutcome;
    fn cap(&self, f: &FusedAi) -> i32;
    fn training_cost(&self, level: u32) -> u32;
}

struct Live;

impl Growth for Live {
    fn levels(&self) -> u32 {
        BASE_AI_LEVEL_OFFSETS.len() as u32
    }
    fn practise(&self, f: &FusedAi, analysis: i32) -> PracticeOutcome {
        practise(f, analysis, 1)
    }
    fn cap(&self, f: &FusedAi) -> i32 {
        analysis_cap(f)
    }
    fn training_cost(&self, level: u32) -> u32 {
        training_cost(level)
    }
}

/// `practice_cost`'s and `analysis_cap`'s shapes over the proposed figures.
struct Proposed {
    offsets: &'static [i32],
    practice_per_tick: u32,
    practice_base: u32,
    practice_quad: u32,
    training: &'static [u32],
}

impl Growth for Proposed {
    fn levels(&self) -> u32 {
        self.offsets.len() as u32
    }
    fn practise(&self, f: &FusedAi, analysis: i32) -> PracticeOutcome {
        let cap = self.cap(f);
        let (mut analysis, mut practice) = (analysis, f.practice);
        if analysis < cap {
            practice += self.practice_per_tick;
            let g = (analysis - f.analysis_at_fusion).max(0) as u32;
            let cost = self.practice_base + self.practice_quad * g * g;
            if practice >= cost {
                practice -= cost;
                analysis += 1;
            }
        }
        let stalled = analysis >= cap;
        PracticeOutcome {
            analysis,
            practice: if stalled { 0 } else { practice },
            stalled,
        }
    }
    fn cap(&self, f: &FusedAi) -> i32 {
        self.offsets
            .get(f.level as usize)
            .map_or(i32::MAX, |o| f.analysis_at_fusion + o)
    }
    fn training_cost(&self, level: u32) -> u32 {
        self.training.get(level as usize).copied().unwrap_or(0)
    }
}

const PROPOSED: Proposed = Proposed {
    offsets: &[20, 45, 70, 100],
    practice_per_tick: 8,
    practice_base: 1600,
    practice_quad: 1,
    training: &[20, 40, 60, 80],
};

/// Node costs and AI gates; `None` keeps the shipped `.ron` figure.
const PROPOSED_NODES: &[(&str, Option<u32>, Option<u32>)] = &[
    ("cache_coherence", Some(60), None),
    ("charge_density", Some(60), None),
    ("mod_bench", Some(60), None),
    ("dispatch", Some(70), None),
    ("firewall", Some(70), None),
    ("overclock", Some(70), None),
    ("drop_pods", Some(75), None),
    ("paging", Some(75), None),
    ("wetware_splicing", Some(75), None),
    ("long_range_decompile", Some(85), None),
    ("neural_amp", Some(85), None),
    ("memory_mapping", Some(90), Some(1)),
    ("program_refactoring", Some(110), None),
    ("capacitance", Some(135), None),
    ("deep_splicing", Some(135), None),
    ("ablative", Some(165), None),
    ("monofilament", Some(165), None),
    ("broadcast_decompile", Some(180), None),
    ("segmentation", Some(180), None),
    ("cortex", Some(190), Some(2)),
    ("deep_analysis", Some(195), Some(1)),
    ("model_inspection", Some(240), Some(3)),
    ("virtual_memory", Some(330), None),
    ("wide_broadcast_decompile", Some(330), Some(3)),
    ("shard_mapping", Some(540), None),
    ("cold_archive", Some(810), Some(4)),
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Player {
    NoFusion,
    Untrained,
    Trained,
}

struct Node {
    cost: u32,
    gate: Option<u32>,
    requires: Vec<String>,
    subject: bool,
}

fn base_tree(
    db: &ResearchDb,
    overrides: &[(&str, Option<u32>, Option<u32>)],
) -> BTreeMap<String, Node> {
    let mut tree: BTreeMap<String, Node> = db
        .all()
        .filter(|d| d.tree == ResearchTree::Base)
        .map(|d| {
            (
                d.id.to_string(),
                Node {
                    cost: d.cost,
                    gate: d.requires_ai_level,
                    requires: d.requires.iter().map(|r| r.to_string()).collect(),
                    subject: d.requires_subject,
                },
            )
        })
        .collect();
    for &(id, cost, gate) in overrides {
        let node = tree.get_mut(id).unwrap_or_else(|| panic!("no node {id}"));
        if let Some(c) = cost {
            node.cost = c;
        }
        node.gate = gate.or(node.gate);
    }
    tree
}

/// The cheapest node whose prerequisites are done and whose gate is open;
/// a tie goes to the lower id.
fn next_node(tree: &BTreeMap<String, Node>, done: &BTreeSet<String>, level: u32) -> Option<String> {
    tree.iter()
        .filter(|(id, n)| {
            !done.contains(*id)
                && n.requires.iter().all(|r| done.contains(r))
                && n.gate.is_none_or(|g| level >= g)
        })
        .min_by_key(|(id, n)| (n.cost, (*id).clone()))
        .map(|(id, _)| id.clone())
}

struct Run {
    awakening: u64,
    end: u64,
    nodes: usize,
    level: u32,
    analysis: i32,
    pace: f64,
    subjects: u32,
    stall_ticks: u64,
    /// From the awakening, as is `break_even`.
    level_ticks: Vec<u64>,
    break_even: Option<u64>,
}

/// Researches the base tree cheapest-first, one station, a project always
/// selected and every subject on hand.
fn walk(
    tree: &BTreeMap<String, Node>,
    growth: &dyn Growth,
    player: Player,
    trigger: usize,
    a0: i32,
    p0: f64,
) -> Run {
    let mut done = BTreeSet::new();
    let (mut tick, mut carry, mut progress) = (0u64, 0.0f64, 0u32);
    let mut subjects = 0;
    // The no-fusion player is the old game on the same costs, so no gate holds.
    let open = if player == Player::NoFusion {
        u32::MAX
    } else {
        0
    };
    let mut current = next_node(tree, &done, open);
    while done.len() < trigger || (player == Player::NoFusion && current.is_some()) {
        let Some(id) = current.clone() else { break };
        tick += 1;
        carry += p0;
        let units = carry.floor();
        carry -= units;
        progress += units as u32;
        if progress >= tree[&id].cost {
            subjects += u32::from(tree[&id].subject);
            done.insert(id);
            progress = 0;
            current = next_node(tree, &done, open);
        }
    }
    let awakening = tick;
    if player == Player::NoFusion {
        return Run {
            awakening,
            end: tick,
            nodes: done.len(),
            level: 0,
            analysis: a0,
            pace: p0,
            subjects,
            stall_ticks: 0,
            level_ticks: Vec::new(),
            break_even: None,
        };
    }
    tick += u64::from(BASE_AI_FUGUE_TICKS);
    let mut f = FusedAi {
        host: ProgramId(0),
        station: (0, 0),
        level: 0,
        analysis_at_fusion: a0,
        pace_at_fusion: p0,
        practice: 0,
        carry: 0.0,
        training: TrainingTarget::default(),
    };
    let mut analysis = a0;
    let mut stall_ticks = 0;
    let mut level_ticks = Vec::new();
    let mut break_even = None;
    loop {
        if break_even.is_none() && pace(&f, analysis) >= p0 {
            break_even = Some(tick - awakening);
        }
        let stalled = analysis >= growth.cap(&f);
        if player == Player::Trained && stalled && f.level < growth.levels() && !f.training.selected
        {
            f.training.selected = true;
        }
        if !f.training.selected && current.is_none() {
            break;
        }
        tick += 1;
        if stalled {
            stall_ticks += 1;
        }
        f.carry += pace(&f, analysis);
        let units = f.carry.floor();
        f.carry -= units;
        let units = units as u32;
        let room = if f.training.selected {
            let cost = growth.training_cost(f.level);
            let room = f.training.progress < cost;
            f.training.progress = (f.training.progress + units).min(cost);
            room
        } else {
            let id = current.as_ref().unwrap();
            let room = progress < tree[id].cost;
            progress = (progress + units).min(tree[id].cost);
            room
        };
        if f.training.selected && f.training.progress >= growth.training_cost(f.level) {
            subjects += 1;
            f.level += 1;
            f.practice = 0;
            f.training = TrainingTarget::default();
            level_ticks.push(tick - awakening);
            if current.is_none() {
                current = next_node(tree, &done, f.level);
            }
        } else if room {
            let out = growth.practise(&f, analysis);
            f.practice = out.practice;
            analysis = out.analysis;
        }
        if let Some(id) = current.clone()
            && progress >= tree[&id].cost
        {
            subjects += u32::from(tree[&id].subject);
            done.insert(id);
            progress = 0;
            current = next_node(tree, &done, f.level);
        }
    }
    Run {
        awakening,
        end: tick,
        nodes: done.len(),
        level: f.level,
        analysis,
        pace: pace(&f, analysis),
        subjects,
        stall_ticks,
        level_ticks,
        break_even,
    }
}

fn hours(ticks: u64) -> String {
    let h = ticks as f64 / TICKS_PER_SECOND / 3600.0;
    format!("{h:.1} h")
}

fn report(label: &str, tree: &BTreeMap<String, Node>, growth: &dyn Growth, a0: i32, p0: f64) {
    let total: u32 = tree.values().map(|n| n.cost).sum();
    let gated: Vec<(&String, u32, u32)> = tree
        .iter()
        .filter_map(|(id, n)| n.gate.map(|g| (id, g, n.cost)))
        .collect();
    let ungated: u32 = tree
        .values()
        .filter(|n| n.gate.is_none())
        .map(|n| n.cost)
        .sum();
    println!(
        "== {label}: {} nodes, {total} points ({ungated} ungated, {} nodes), gated {gated:?}, A0 {a0}, p0 {p0:.5}",
        tree.len(),
        tree.values().filter(|n| n.gate.is_none()).count(),
    );
    for trigger in [5, 7, 10] {
        for player in [Player::NoFusion, Player::Untrained, Player::Trained] {
            let r = walk(tree, growth, player, trigger, a0, p0);
            let levels: Vec<String> = r.level_ticks.iter().map(|&t| hours(t)).collect();
            println!(
                "  trigger {trigger:>2} {player:<9?}: end {:>7} ticks ({}), after awakening {}, \
                 {} nodes, level {}, analysis {}, pace {:.5} ({:.2}x p0), subjects {}, \
                 stalled {}, levels at {levels:?} after awakening, back to p0 at {}",
                r.end,
                hours(r.end),
                hours(r.end - r.awakening),
                r.nodes,
                r.level,
                r.analysis,
                r.pace,
                r.pace / p0,
                r.subjects,
                hours(r.stall_ticks),
                r.break_even.map_or("never".into(), hours),
            );
        }
    }
}

/// Fusion Analysis and ordinary pace read off `fred`, the one played save.
fn fred_host() -> (i32, f64) {
    let mut game = load_template("fred");
    let (rows, host, _, _) = staff_paces(&mut game).unwrap();
    let h = rows.iter().find(|r| r.id == host).unwrap();
    (h.analysis, h.pace)
}

#[test]
#[ignore = "measurement; prints"]
fn measure_tree_completion() {
    let game = load_template("study");
    let db = game.world.resource::<ResearchDb>();
    let (a0, p0) = fred_host();
    report("live placeholders", &base_tree(db, &[]), &Live, a0, p0);
    report(
        "proposed",
        &base_tree(db, PROPOSED_NODES),
        &PROPOSED,
        a0,
        p0,
    );
    for a in [15, 30] {
        report(
            &format!("proposed, host analysis {a}"),
            &base_tree(db, PROPOSED_NODES),
            &PROPOSED,
            a,
            p0,
        );
    }
}
