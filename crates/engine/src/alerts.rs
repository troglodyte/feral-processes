//! The alert board — a capped, saved, player-dismissed list of production
//! blockers and program events. See `docs/superpowers/specs/2026-09-23-alert-board-design.md`.
//!
//! `post` is the **one door** onto the board. It is a free function rather
//! than a `Game` method because several sources (`systems::set_machine_status`
//! and friends) are reached from bevy systems that have no `Game` to call —
//! the same reason `needs::strain` and `memories::sum_intensity` are free
//! functions. `Game::post_alert` is a one-line wrapper for the sources that
//! do have one.

use std::collections::VecDeque;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::MachineStatus;
use crate::tuning::ALERT_BOARD_CAP;

/// What happened. **Variant names are save format** (field-named RON, like
/// `notifications::NotificationKind`): append a new one, never rename or
/// reorder-and-rename an existing one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertKind {
    /// One of the six stall states `components::MachineStatus` can carry.
    /// The status rides inside the kind, so a Lathe that goes Clogged then
    /// Unpowered is two alerts — they need different fixes — while the same
    /// Lathe clogging twice collapses into one.
    MachineStalled(MachineStatus),
    ProgramDowned,
    SweepIncoming,
    SweepHit,
    SiegeIncoming,
    SiegeBegun,
    /// A dig or build site with no route to work it.
    SiteCutOff,
    DepotsFull,
}

/// One row on the board. `subject` is the collapse key the poster mints —
/// engine bookkeeping the renderer never sees; see `views::AlertView` for
/// what gui reads instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alert {
    pub kind: AlertKind,
    pub subject: String,
    pub text: String,
    pub count: u32,
    pub unread: bool,
}

/// Newest first. `depots_full` is a latch, not a second resource — a new
/// `Resource` shifts bevy's query iteration order, and this repo has
/// recorded that as a cause of seeded tests moving (see the memory
/// `rng-stream-shift-exposes-seed-luck-tests`). Only `alerts` is saved: the
/// latch resets on load, which is what the spec asks for anyway.
#[derive(Resource, Default)]
pub struct AlertBoard {
    pub(crate) alerts: VecDeque<Alert>,
    /// Set on the false→true edge of every depot being full
    /// (`game::base::hauling::haul_step_system`) and cleared by the next
    /// successful deposit into any depot.
    ///
    /// `#[allow(dead_code)]` until Task 6 wires up its first reader and
    /// writer.
    #[allow(dead_code)]
    pub(crate) depots_full: bool,
}

/// The one door onto the board. Three steps: collapse (same `kind` and
/// `subject`) moves the entry to the front, bumps `count` and replaces
/// `text`; otherwise a fresh entry is pushed to the front; then truncate
/// from the back. Draws no `GameRng`, so an alert cannot shift the seeded
/// stream.
pub fn post(
    board: &mut AlertBoard,
    kind: AlertKind,
    subject: impl Into<String>,
    text: impl Into<String>,
) {
    let subject = subject.into();
    let text = text.into();
    if let Some(existing_pos) = board
        .alerts
        .iter()
        .position(|alert| alert.kind == kind && alert.subject == subject)
    {
        let mut alert = board
            .alerts
            .remove(existing_pos)
            .expect("position just found");
        alert.count += 1;
        alert.text = text;
        alert.unread = true;
        board.alerts.push_front(alert);
    } else {
        board.alerts.push_front(Alert {
            kind,
            subject,
            text,
            count: 1,
            unread: true,
        });
    }
    cap(board);
}

/// Removes the entry at `index`. Out of range is a no-op.
pub fn dismiss(board: &mut AlertBoard, index: usize) {
    if index < board.alerts.len() {
        board.alerts.remove(index);
    }
}

/// Drops the oldest entries until the board is at or under
/// `ALERT_BOARD_CAP`. `post` and `Game::load` both call this — load, so a
/// hand-edited save cannot exceed the cap.
pub fn cap(board: &mut AlertBoard) {
    while board.alerts.len() > ALERT_BOARD_CAP {
        board.alerts.pop_back();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stalled(status: MachineStatus) -> AlertKind {
        AlertKind::MachineStalled(status)
    }

    #[test]
    fn a_fresh_post_is_unread_with_count_one() {
        let mut board = AlertBoard::default();
        post(
            &mut board,
            AlertKind::ProgramDowned,
            "prog-1",
            "Widget is down.",
        );
        assert_eq!(board.alerts.len(), 1);
        let alert = &board.alerts[0];
        assert_eq!(alert.count, 1);
        assert!(alert.unread);
        assert_eq!(alert.text, "Widget is down.");
    }

    #[test]
    fn a_collapse_bumps_count_moves_to_front_and_replaces_text() {
        let mut board = AlertBoard::default();
        post(
            &mut board,
            stalled(MachineStatus::Clogged),
            "lathe@1,1",
            "The Lathe is clogged.",
        );
        post(
            &mut board,
            AlertKind::ProgramDowned,
            "prog-2",
            "Scout is down.",
        );
        // Re-posting the first alert should move it back to the front.
        post(
            &mut board,
            stalled(MachineStatus::Clogged),
            "lathe@1,1",
            "The Lathe is clogged again.",
        );

        assert_eq!(
            board.alerts.len(),
            2,
            "same identity collapses, not a third row"
        );
        let front = &board.alerts[0];
        assert_eq!(front.kind, stalled(MachineStatus::Clogged));
        assert_eq!(front.subject, "lathe@1,1");
        assert_eq!(front.count, 2);
        assert!(front.unread);
        assert_eq!(front.text, "The Lathe is clogged again.");
    }

    #[test]
    fn the_same_subject_with_a_different_status_is_a_separate_entry() {
        let mut board = AlertBoard::default();
        post(
            &mut board,
            stalled(MachineStatus::Clogged),
            "lathe@1,1",
            "The Lathe is clogged.",
        );
        post(
            &mut board,
            stalled(MachineStatus::Unpowered),
            "lathe@1,1",
            "The Lathe is dark.",
        );

        assert_eq!(board.alerts.len(), 2, "different status, different alert");
        assert!(board.alerts.iter().all(|a| a.count == 1));
    }

    #[test]
    fn the_cap_drops_the_oldest() {
        let mut board = AlertBoard::default();
        for i in 0..(ALERT_BOARD_CAP + 1) {
            post(
                &mut board,
                AlertKind::ProgramDowned,
                format!("prog-{i}"),
                format!("Program {i} is down."),
            );
        }

        assert_eq!(board.alerts.len(), ALERT_BOARD_CAP);
        // Newest first: prog-0 was pushed first and should have fallen off
        // the back once the cap was exceeded.
        assert!(board.alerts.iter().all(|a| a.subject != "prog-0"));
        assert_eq!(
            board.alerts.front().unwrap().subject,
            format!("prog-{ALERT_BOARD_CAP}")
        );
    }

    #[test]
    fn dismiss_removes_exactly_one_entry() {
        let mut board = AlertBoard::default();
        post(&mut board, AlertKind::ProgramDowned, "prog-1", "one");
        post(&mut board, AlertKind::ProgramDowned, "prog-2", "two");
        post(&mut board, AlertKind::ProgramDowned, "prog-3", "three");

        dismiss(&mut board, 1);

        assert_eq!(board.alerts.len(), 2);
        assert_eq!(board.alerts[0].subject, "prog-3");
        assert_eq!(board.alerts[1].subject, "prog-1");
    }

    #[test]
    fn dismiss_out_of_range_is_a_no_op() {
        let mut board = AlertBoard::default();
        post(&mut board, AlertKind::ProgramDowned, "prog-1", "one");

        dismiss(&mut board, 5);

        assert_eq!(board.alerts.len(), 1);
    }
}
