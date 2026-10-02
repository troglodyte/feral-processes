//! Production lines: a connected run of machines that one worker runs.
//!
//! A line is **derived from the grid on every call and never stored**, so
//! building, demolishing, upgrading or refitting needs no hook to keep it
//! right. An edge is a call into the pull's own reach
//! (`collect::feeders_by_tile`, `collect::ORTHOGONAL`, the `With<Stock>`
//! filter) and the recipe functions in `systems`, so a link can never be
//! drawn where nothing would move.

use std::collections::HashMap;

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;

use crate::game::base::collect::{ORTHOGONAL, feeders_by_tile};
use crate::*;

/// A line's identity: its lowest `(x, y)` member's tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LineKey(pub (i32, i32));

/// One weakly connected component of the feed graph. A machine with no edges
/// is a line of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub key: LineKey,
    /// Downstream rank ascending, `(x, y)` breaking ties.
    pub members: Vec<Entity>,
    /// Longest edge path from the member to a sink, parallel to `members`.
    pub rank: Vec<u32>,
}

/// Groups `nodes` into lines. `edge(a, b)` says node `a` feeds node `b`, by
/// index into `nodes`.
///
/// Rank is the longest path to a sink with back edges dropped in `(x, y)`
/// DFS order, so a malformed cycle (a mod could write one) degrades to a
/// deterministic order rather than a panic or a loop.
pub(crate) fn group(
    nodes: &[(Entity, (i32, i32))],
    edge: impl Fn(usize, usize) -> bool,
) -> Vec<Line> {
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&i| nodes[i].1);

    let mut component = vec![usize::MAX; nodes.len()];
    let mut next_component = 0;
    for &start in &order {
        if component[start] != usize::MAX {
            continue;
        }
        let mut stack = vec![start];
        component[start] = next_component;
        while let Some(at) = stack.pop() {
            for &other in &order {
                if component[other] == usize::MAX && (edge(at, other) || edge(other, at)) {
                    component[other] = next_component;
                    stack.push(other);
                }
            }
        }
        next_component += 1;
    }

    let mut rank = vec![0u32; nodes.len()];
    let mut state = vec![Visit::Unseen; nodes.len()];
    for &root in &order {
        if state[root] == Visit::Unseen {
            rank_from(root, &order, &edge, &mut state, &mut rank);
        }
    }

    let mut lines: Vec<Line> = (0..next_component)
        .map(|c| {
            let mut members: Vec<usize> = order
                .iter()
                .copied()
                .filter(|&i| component[i] == c)
                .collect();
            members.sort_by_key(|&i| (rank[i], nodes[i].1));
            Line {
                key: LineKey(
                    members
                        .iter()
                        .map(|&i| nodes[i].1)
                        .min()
                        .expect("a component has a member"),
                ),
                rank: members.iter().map(|&i| rank[i]).collect(),
                members: members.iter().map(|&i| nodes[i].0).collect(),
            }
        })
        .collect();
    lines.sort_by_key(|l| l.key);
    lines
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Visit {
    Unseen,
    Open,
    Done,
}

/// Post-order DFS: a node's rank is one more than its deepest kept
/// successor. An edge into an `Open` node closes a cycle and is skipped.
fn rank_from(
    at: usize,
    order: &[usize],
    edge: &impl Fn(usize, usize) -> bool,
    state: &mut [Visit],
    rank: &mut [u32],
) {
    state[at] = Visit::Open;
    let mut deepest = None;
    for &to in order {
        if to == at || !edge(at, to) {
            continue;
        }
        match state[to] {
            Visit::Open => continue,
            Visit::Unseen => rank_from(to, order, edge, state, rank),
            Visit::Done => {}
        }
        deepest = deepest.max(Some(rank[to] + 1));
    }
    rank[at] = deepest.unwrap_or(0);
    state[at] = Visit::Done;
}

/// Every line in the base, lines of one included, sorted by key.
///
/// Built over the same `(Entity, &Structure, &Position), With<Stock>`
/// population `assembler_system` pulls through, with neighbours found by
/// `ORTHOGONAL` in `feeders_by_tile`'s map. A free function over the world
/// because the labour pass has no `Game` to call a method on.
pub(crate) fn lines_in(world: &mut World) -> Vec<Line> {
    let mut query = world.query_filtered::<(Entity, &Structure, &Position), With<Stock>>();
    let mut placed: Vec<(Entity, (i32, i32), StructureId)> = query
        .iter(world)
        .map(|(e, s, p)| (e, (p.x, p.y), s.kind.clone()))
        .collect();
    placed.sort_by_key(|(_, tile, _)| *tile);
    let by_tile = feeders_by_tile(query.iter(world));
    let index: HashMap<Entity, usize> = placed
        .iter()
        .enumerate()
        .map(|(i, (e, _, _))| (*e, i))
        .collect();

    let db = world.resource::<StructureDb>();
    let items = world.resource::<ItemDb>();
    let defs: Vec<_> = placed.iter().map(|(_, _, k)| db.get(k)).collect();
    let nodes: Vec<(Entity, (i32, i32))> = placed.iter().map(|(e, tile, _)| (*e, *tile)).collect();

    group(&nodes, |a, b| {
        let (ax, ay) = nodes[a].1;
        let adjacent = ORTHOGONAL.iter().any(|(dx, dy)| {
            by_tile
                .get(&(ax + dx, ay + dy))
                .is_some_and(|e| index[e] == b)
        });
        adjacent
            && matches!((defs[a], defs[b]), (Some(da), Some(db)) if crate::systems::feeds(da, db, items))
    })
}

/// Which line each member of a line of two or more belongs to. A line of one
/// is absent, which is what lets every reader treat it as "today's rule".
pub(crate) fn membership(lines: &[Line]) -> HashMap<Entity, LineKey> {
    lines
        .iter()
        .filter(|l| l.members.len() > 1)
        .flat_map(|l| l.members.iter().map(|&m| (m, l.key)))
        .collect()
}

/// What `collapse` reads of a worker's `Task`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Held {
    pub target: Entity,
    pub progress: u32,
    pub required: u32,
}

/// The `GatherResource` task each line's worker holds, by line. A line has
/// one worker, so a second holder (a save from before lines) is resolved by
/// entity order rather than query order.
pub(crate) fn line_holders(
    world: &mut World,
    member_of: &HashMap<Entity, LineKey>,
) -> HashMap<LineKey, Held> {
    let mut query = world.query::<(Entity, &Task)>();
    let mut held: Vec<(Entity, Held)> = query
        .iter(world)
        .filter(|(_, t)| t.kind == TaskKind::GatherResource && member_of.contains_key(&t.target))
        .map(|(e, t)| {
            (
                e,
                Held {
                    target: t.target,
                    progress: t.progress,
                    required: t.required,
                },
            )
        })
        .collect();
    held.sort_by_key(|(e, _)| *e);
    let mut by_line = HashMap::new();
    for (_, task) in held {
        by_line.entry(member_of[&task.target]).or_insert(task);
    }
    by_line
}

/// Folds every member's `GatherResource` want into one want per line
/// (spec §3 steps 1 and 2).
///
/// The line takes the position of its first member want, so priority is
/// untouched, and names its **active machine**, one of the members that
/// itself wanted a body: an order for a middle product must not run the end
/// machine. Wants for lines of one and every other kind pass through.
///
/// 1. A worker mid-cycle on a wanted member stays there, because `post_worker`
///    resets progress and a switch would throw the cycle away.
/// 2. Otherwise the wanted member furthest downstream.
pub(crate) fn collapse(
    wants: Vec<(Entity, TaskKind)>,
    lines: &[Line],
    holding: impl Fn(LineKey) -> Option<Held>,
) -> Vec<(Entity, TaskKind)> {
    let by_member: HashMap<Entity, usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.members.len() > 1)
        .flat_map(|(i, l)| l.members.iter().map(move |&m| (m, i)))
        .collect();

    let mut out: Vec<(Entity, TaskKind)> = Vec::with_capacity(wants.len());
    let mut slot: HashMap<usize, usize> = HashMap::new();
    let mut wanted: HashMap<usize, Vec<Entity>> = HashMap::new();
    for (machine, kind) in wants {
        let line = (kind == TaskKind::GatherResource)
            .then(|| by_member.get(&machine).copied())
            .flatten();
        let Some(line) = line else {
            out.push((machine, kind));
            continue;
        };
        wanted.entry(line).or_default().push(machine);
        slot.entry(line).or_insert_with(|| {
            out.push((machine, kind));
            out.len() - 1
        });
    }
    for (line, at) in slot {
        let members = &wanted[&line];
        let mid_cycle = holding(lines[line].key)
            .filter(|t| members.contains(&t.target) && 0 < t.progress && t.progress < t.required);
        out[at].0 = match mid_cycle {
            Some(task) => task.target,
            // `members` is in rank order, so the first wanted one is the
            // furthest downstream.
            None => *lines[line]
                .members
                .iter()
                .find(|m| members.contains(m))
                .expect("a line with a want has a wanted member"),
        };
    }
    out
}

impl Game {
    /// Every line in the base, lines of one included, sorted by key.
    pub fn production_lines(&mut self) -> Vec<Line> {
        lines_in(&mut self.world)
    }

    /// The line `machine` belongs to, `Some` only for a line of two or more.
    pub fn line_of(&mut self, machine: Entity) -> Option<LineKey> {
        self.production_lines()
            .into_iter()
            .find(|l| l.members.len() > 1 && l.members.contains(&machine))
            .map(|l| l.key)
    }

    /// `machine`'s whole line, or just `machine` when it is a line of one.
    pub(crate) fn line_members(&mut self, machine: Entity) -> Vec<Entity> {
        self.production_lines()
            .into_iter()
            .find(|l| l.members.contains(&machine))
            .map(|l| l.members)
            .unwrap_or_else(|| vec![machine])
    }
}
