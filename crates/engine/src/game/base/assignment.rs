//! Who works which post: the priority-ordered bipartite matching behind
//! `Game::schedule_base_labour`.
//!
//! Pure — indices in, indices out — so the rule that decides a base's whole
//! assignment can be tested without standing a base up. The caller owns
//! what an index means (a want in priority order, a body in table order)
//! and what an edge means (`can_take`).
//!
//! **Why a matching and not a cut.** The scheduler used to cut its want
//! list to the number of bodies and hand the survivors out greedily, which
//! assumes any body can take any want. Once the Base staff table can say
//! "this one only builds", that assumption cuts the wrong wants and a
//! greedy hand-out strands work: body A (all on, first in the table) and
//! body B (Build only) against wants `[Build, Dig]` — greedy gives A the
//! build and leaves the dig unworked, when B→Build, A→Dig works both.
//!
//! **Priority is the position in the want list, and the matching never
//! un-seats an earlier want.** Kuhn's augmenting paths run want by want in
//! priority order: a later want may re-seat a body an earlier want claimed
//! onto another body the earlier want can take, but it can never leave that
//! earlier want empty. So the set of wants worked is the lexicographically
//! first one the edges allow, whatever order candidates are tried in.

/// For each want, the body that works it, or `None` if it goes unworked.
///
/// `holder[want]` is the body already posted there, if any. `can_take(want,
/// body)` is the edge; it is asked lazily and memoised, since an edge can
/// cost a walk field.
///
/// Two passes, and the split is the anti-thrash rule:
///
/// 1. **Which wants are worked.** Kuhn in priority order, each want trying
///    its holder first and then the table. This fixes the set and nothing
///    else — which body ends up where along the way is incidental.
/// 2. **Who works them.** Every worked want's holder is seated first, then
///    the rest are filled in priority order by the same augmenting search.
///    Without this a holder can be claimed by an earlier want on the way
///    through pass 1 and end up somewhere new for no gain, restarting its
///    cronjob from zero; with it a body moves only when the matching needs
///    it to.
///
/// Within one search an **unclaimed** candidate is always preferred to
/// re-seating a claimed one, in table order, so on a base where every body
/// can take every want this is exactly "holders stay, idle bodies fill the
/// wants in priority order from the top of the table".
pub(crate) fn assign_by_priority(
    wants: usize,
    bodies: usize,
    holder: &[Option<usize>],
    can_take: impl FnMut(usize, usize) -> bool,
) -> Vec<Option<usize>> {
    debug_assert_eq!(holder.len(), wants);
    let mut edges = Edges {
        bodies,
        memo: vec![None; wants * bodies],
        can_take,
    };

    // Pass 1. A search that fails leaves every body it visited claimed and
    // every edge of theirs explored, so no later search can free one of them
    // either — they are marked dead, and the failures behind a scarce base
    // (a hundred-cell dig plan against ten bodies) cost one sweep rather than
    // one per want.
    let mut claimed: Vec<Option<usize>> = vec![None; bodies];
    let mut dead = vec![false; bodies];
    for want in 0..wants {
        let mut visited = dead.clone();
        if !augment(want, holder, &mut edges, &mut claimed, &mut visited) {
            dead = visited;
        }
    }
    let mut worked = vec![false; wants];
    for want in claimed.iter().flatten() {
        worked[*want] = true;
    }

    // Pass 2.
    let mut claimed: Vec<Option<usize>> = vec![None; bodies];
    for want in 0..wants {
        if let Some(body) = holder[want]
            && worked[want]
            && claimed[body].is_none()
            && edges.get(want, body)
        {
            claimed[body] = Some(want);
        }
    }
    let seated: Vec<bool> = {
        let mut seated = vec![false; wants];
        for want in claimed.iter().flatten() {
            seated[*want] = true;
        }
        seated
    };
    for want in (0..wants).filter(|&w| worked[w] && !seated[w]) {
        let mut visited = vec![false; bodies];
        let found = augment(want, holder, &mut edges, &mut claimed, &mut visited);
        debug_assert!(found, "pass 1 proved want {want} can be worked");
    }

    let mut by_want = vec![None; wants];
    for (body, want) in claimed.iter().enumerate() {
        if let Some(want) = want {
            by_want[*want] = Some(body);
        }
    }
    by_want
}

/// `can_take`, asked at most once per pair.
struct Edges<F> {
    bodies: usize,
    memo: Vec<Option<bool>>,
    can_take: F,
}

impl<F: FnMut(usize, usize) -> bool> Edges<F> {
    fn get(&mut self, want: usize, body: usize) -> bool {
        let slot = want * self.bodies + body;
        if let Some(answer) = self.memo[slot] {
            return answer;
        }
        let answer = (self.can_take)(want, body);
        self.memo[slot] = Some(answer);
        answer
    }
}

/// The body order `want` tries: its holder, then the table.
fn candidates(want: usize, bodies: usize, holder: &[Option<usize>]) -> impl Iterator<Item = usize> {
    let first = holder[want];
    first
        .into_iter()
        .chain((0..bodies).filter(move |&b| Some(b) != first))
}

/// One augmenting search from `want`: claim an unclaimed body if one will
/// take it, otherwise re-seat a claimed one whose want can move.
fn augment<F: FnMut(usize, usize) -> bool>(
    want: usize,
    holder: &[Option<usize>],
    edges: &mut Edges<F>,
    claimed: &mut [Option<usize>],
    visited: &mut [bool],
) -> bool {
    let bodies = edges.bodies;
    for body in candidates(want, bodies, holder) {
        if !visited[body] && claimed[body].is_none() && edges.get(want, body) {
            visited[body] = true;
            claimed[body] = Some(want);
            return true;
        }
    }
    for body in candidates(want, bodies, holder) {
        let Some(other) = claimed[body] else {
            continue;
        };
        if visited[body] || !edges.get(want, body) {
            continue;
        }
        visited[body] = true;
        if augment(other, holder, edges, claimed, visited) {
            claimed[body] = Some(want);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::assign_by_priority;

    /// `edges[want]` lists the bodies that may take it.
    fn run(edges: &[&[usize]], bodies: usize, holder: &[Option<usize>]) -> Vec<Option<usize>> {
        assign_by_priority(edges.len(), bodies, holder, |w, b| edges[w].contains(&b))
    }

    /// The spec's stranding case: A (0) takes anything, B (1) builds only,
    /// wants `[Build, Dig]`.
    #[test]
    fn a_restricted_body_is_not_stranded_by_an_earlier_greedy_pick() {
        let got = run(&[&[0, 1], &[0]], 2, &[None, None]);
        assert_eq!(got, vec![Some(1), Some(0)]);
    }

    #[test]
    fn a_later_want_never_empties_an_earlier_one() {
        // Both wants need body 0; the earlier keeps it.
        let got = run(&[&[0], &[0]], 2, &[None, Some(0)]);
        assert_eq!(got, vec![Some(0), None]);
    }

    #[test]
    fn holders_keep_their_posts_and_idle_bodies_fill_from_the_top() {
        // Body 2 holds want 1; bodies 0 and 1 are idle. Everyone can take
        // everything.
        let all: &[usize] = &[0, 1, 2];
        let got = run(&[all, all, all], 3, &[None, Some(2), None]);
        assert_eq!(got, vec![Some(0), Some(2), Some(1)]);
    }

    #[test]
    fn a_holder_moves_only_when_moving_frees_work() {
        // Body 0 holds want 1 but is the only one who can take want 0;
        // body 1 can take want 1.
        let got = run(&[&[0], &[0, 1]], 2, &[None, Some(0)]);
        assert_eq!(got, vec![Some(0), Some(1)]);
    }

    #[test]
    fn a_want_nobody_can_take_goes_unworked_without_costing_the_rest() {
        let got = run(&[&[], &[0], &[1]], 2, &[None, None, None]);
        assert_eq!(got, vec![None, Some(0), Some(1)]);
    }

    #[test]
    fn each_edge_is_asked_once() {
        let mut asked = std::collections::HashSet::new();
        let mut repeats = 0;
        assign_by_priority(4, 3, &[None, None, None, Some(1)], |w, b| {
            if !asked.insert((w, b)) {
                repeats += 1;
            }
            (w + b) % 2 == 0
        });
        assert_eq!(repeats, 0);
    }
}
