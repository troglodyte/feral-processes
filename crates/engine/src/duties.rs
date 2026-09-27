//! Which jobs a base-staff program may be handed — the data model behind
//! the Base staff work table (`Mode::BaseStaff`).
//!
//! A [`Duty`] is a checkbox column; v1 ships the scheduler's four
//! `TaskKind`s and nothing narrower. `components::Duties` stores the
//! **unchecked** set, `DepotFilter`'s denied-set precedent — an absent or
//! empty component reads as everything checked, so a save written before
//! this shipped and a newly tamed program both come back able to take any
//! job. [`PostDesc`] carries a post's structure def id even though v1
//! ignores it: the scheduler builds one of these per want, so a future
//! `Duty::Structure(id)` column that overlaps `Operate` costs no new plumbing
//! there — only a new arm here and a generated column.

use crate::components::{Duties, TaskKind};
use crate::structures::StructureId;

/// A restriction column on the Base staff table. `Ord` is the column order
/// the screen draws them in; the save's string form is [`Duty::name`], never
/// this enum's discriminant, so reordering these variants cannot silently
/// rewrite a save.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Duty {
    Operate,
    Guard,
    Dig,
    Build,
}

impl Duty {
    pub const ALL: [Duty; 4] = [Duty::Operate, Duty::Guard, Duty::Dig, Duty::Build];

    /// The save's string form. `latch_key`'s precedent: a name a future
    /// build no longer defines must load inert, never fail the parse — see
    /// [`Duty::from_name`].
    pub fn name(&self) -> &'static str {
        match self {
            Duty::Operate => "operate",
            Duty::Guard => "guard",
            Duty::Dig => "dig",
            Duty::Build => "build",
        }
    }

    /// The inverse of [`Duty::name`]. `None` for a name this build doesn't
    /// know — a retired duty, or a hand-edited file — which is what lets the
    /// load path drop it silently rather than refuse the save.
    pub fn from_name(name: &str) -> Option<Duty> {
        Duty::ALL.into_iter().find(|d| d.name() == name)
    }

    /// Whether this duty covers `post`'s job kind. An exhaustive match on
    /// both enums, `cell_mark`'s rule — a fifth `Duty` variant or a fifth
    /// `TaskKind` fails to compile here rather than silently answering
    /// `false` through a wildcard arm.
    pub fn admits(&self, post: &PostDesc) -> bool {
        match (self, post.kind) {
            (Duty::Operate, TaskKind::GatherResource) => true,
            (Duty::Operate, TaskKind::Guard) => false,
            (Duty::Operate, TaskKind::Excavate) => false,
            (Duty::Operate, TaskKind::Construct) => false,
            (Duty::Guard, TaskKind::GatherResource) => false,
            (Duty::Guard, TaskKind::Guard) => true,
            (Duty::Guard, TaskKind::Excavate) => false,
            (Duty::Guard, TaskKind::Construct) => false,
            (Duty::Dig, TaskKind::GatherResource) => false,
            (Duty::Dig, TaskKind::Guard) => false,
            (Duty::Dig, TaskKind::Excavate) => true,
            (Duty::Dig, TaskKind::Construct) => false,
            (Duty::Build, TaskKind::GatherResource) => false,
            (Duty::Build, TaskKind::Guard) => false,
            (Duty::Build, TaskKind::Excavate) => false,
            (Duty::Build, TaskKind::Construct) => true,
        }
    }
}

/// A post's job kind and, when it has one, the structure def id it names —
/// a build site's goal, or `None` for a dig site (which is not a
/// `Structure` at all — see `TaskKind::Excavate`'s doc). v1's four `Duty`
/// variants partition `TaskKind` alone and never read this field; it exists
/// so the scheduler already has it in hand the day a structure-kind column
/// ships.
pub struct PostDesc<'a> {
    pub kind: TaskKind,
    pub structure: Option<&'a StructureId>,
}

/// Whether a body carrying `off` (its `Duties`, if it has any) may be
/// handed `post`. `None` — no `Duties` component — reads as every column
/// checked, `DepotFilter`'s absent-means-everything rule; otherwise a post
/// is admitted when *some* checked duty admits it, not "the one
/// partitioning duty that does" — the "some" rule is what lets a later
/// `Duty::Structure(id)` column overlap `Operate` with no special case here.
pub fn duty_admits(off: Option<&Duties>, post: &PostDesc) -> bool {
    match off {
        None => true,
        Some(off) => Duty::ALL
            .iter()
            .any(|d| !off.off.contains(d) && d.admits(post)),
    }
}
