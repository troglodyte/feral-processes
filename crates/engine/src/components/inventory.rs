use super::*;

#[derive(Component, Default, Clone)]
pub struct Inventory {
    pub items: Vec<(ItemId, u32)>,
}

/// Player-only: every wild program taken apart at the kill rather than paid
/// out directly, one row per `DownedProgram` — see that type's doc and
/// `docs/superpowers/specs/2026-09-04-program-extraction-design.md`.
///
/// **The third store**, after `Inventory` and `GearCopies`, for the same
/// reason `GearCopies` exists at all: `Inventory` is by definition the
/// plain-copy store, and a downed program is never plain — a level-30
/// Prismatic kill and a level-2 Ordinary one are not interchangeable, so
/// folding this in would give every reader of `Inventory` (recipes,
/// `Stock`, `assembler_system`, hauling, banking) an instance rule none of
/// them needs. Unlike `GearCopies`, this is **not** keyed by value: two
/// `DownedProgram`s that happen to compare equal are still two separate
/// kills, and a screen listing them by index (`Mode::DownedPrograms`, a
/// later phase) needs a position to select — `Vec<DownedProgram>` rather
/// than `Vec<(DownedProgram, u32)>`.
///
/// Capped at `tuning::MAX_DOWNED_PROGRAMS` — decision 9 of the spec is that
/// a full store refuses the drop with a log line rather than discarding the
/// worst held program, so nothing here truncates on its own.
#[derive(Component, Default, Clone)]
pub struct DownedPrograms(pub Vec<DownedProgram>);

/// One program waiting in a rig, and the tool the player chose for it.
///
/// A **named struct, never a tuple.** RON parses a `(` in a struct position
/// as the start of named fields, so a `Vec<(DownedProgram, ToolId)>` could
/// never be widened and could not be converted to a named struct with
/// defaulted trailing fields either — `WorkOrder`'s own doc records the two
/// shipped fields that had to be drained into named successors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HopperEntry {
    pub program: DownedProgram,
    pub tool: crate::tools::ToolId,
}

/// A Teardown Rig's private store of downed programs — see
/// `StructureDef::strips`, `Game::load_teardown_rig` and
/// `Game::run_teardown_rigs`.
///
/// **The instance boundary.** A `DownedProgram` is instanced, so it lives in
/// its own places rather than in `Inventory`: `DownedPrograms` on the player,
/// `Racked` on a Quarantine Rack, a rig's `Hopper` (this), and
/// `StudyYield` on a Research Station, where a finished study's program
/// waits for a body to rack it. `CarryingProgram` is a body walking one
/// between them: from a rack to a rig, or from a station to a rack. What
/// still holds is the half that matters: what leaves a **rig** is plain
/// items in `Stock::output`, which is what
/// lets hauling, depots, `collect::plan_adjacent_take` and work orders carry
/// the yield with no instance rule at all — decision 2's seam, which
/// `GearCopies`' doc above states and which spec §10.1 declines to spend.
///
/// The rack is the one of those that is not a queue: nothing works through
/// it, and it is filled and emptied by hand from the `c` picker.
///
/// A `Vec` rather than a keyed store, `DownedPrograms`' own reason: two
/// equal-comparing programs are still two separate kills, and the queue is
/// ordered work rather than a bag of interchangeable rows.
///
/// `progress` is ticks spent on the head entry alone. It resets when an
/// entry completes, and it is saved: a rig eight ticks into a twenty-tick
/// program must not restart that program because the player quit.
#[derive(Component, Default, Clone, Debug)]
pub struct Hopper {
    pub queue: Vec<HopperEntry>,
    pub progress: u64,
    /// The tool this rig is set up with — the last one the player handed it
    /// by hand, and the one a carrier arriving by any other route is
    /// stripped with.
    ///
    /// It lives on the **rig** rather than beside the carrier because
    /// `Game::extraction_yield` reads a tool for both halves of its answer:
    /// `yields` is the pool and `tier` is the scale. A rack stores bare
    /// programs and knows nothing about tools, so a crew fetch would have
    /// nowhere to get one from.
    ///
    /// `None` on a rig nobody has fitted, which runs on the starter tool
    /// (`Game::rig_tool`) — derived rather than stored, so pulling a tool
    /// can never hand back a starter carrier nobody paid for. Never a
    /// `Routines` or `Gear` tool — `load_teardown_rig` refuses both before
    /// it writes this.
    pub standing_tool: Option<crate::tools::ToolId>,
}

/// A downed program a posted body is physically carrying to a rig — the
/// record of one crew trip, and the third place a `DownedProgram` can be
/// standing while it is between the other three.
///
/// **`Carrying` could not be widened.** That is one `(item, qty)` pair
/// *because* `HAUL_CARRY_CAPACITY` bounds a trip, and a carrier has no
/// `ItemId` at all.
///
/// Two rules written for `Carrying` have this as a second subject, and
/// neither fails to compile if a site is missed — the symptom is a lost kill
/// with no error: `Game::is_on_shift` must never free a body holding one,
/// and both structure-destruction paths must put it back rather than drop
/// it with the `Task`.
#[derive(Component, Clone, Debug)]
pub struct CarryingProgram(pub DownedProgram);

/// The downed program a finished subject-gated project left standing at its
/// Research Station, waiting for an idle body to walk it to a Quarantine Rack
/// (`game::base::study_yield`).
///
/// **Not `Racked`, and not the player's pack.** The station must not read as
/// a rack anywhere — every `Racked` reader (the `c` picker, rack room, the
/// crew's rig fetch) would otherwise treat it as shelf — and research no
/// longer touches the pack, so a full pack cannot stall it. One slot: a
/// station holding a program gates the next completion until the crew has
/// moved it.
///
/// Written lazily by `Game::hold_at_station` rather than at spawn, so a
/// hand-spawned station in a fixture needs nothing; `None` and an absent
/// component read the same.
#[derive(Component, Default, Clone, Debug)]
pub struct StudyYield(pub Option<DownedProgram>);

/// How long a rack carrier has stood on one tile: the tile it was last seen
/// on and the consecutive labour passes it has not left it.
///
/// Runtime only, by omission from the save — a reload restarts the count,
/// which only delays the give-up. Cleared by `Game::rack_carrier_patience`
/// the moment the body moves or stops being a carrier.
#[derive(Component, Clone, Copy, Debug)]
pub struct RackWait {
    pub at: Position,
    pub beats: u32,
}

/// A loose floor item an idle program is carrying to a stockpile cell.
///
/// The cleanup carry state, kept separate from `Carrying`, which stays one
/// `(item, qty)`: a tidied thing may be a gear copy or a downed program, and
/// a hauler's trip has a capacity this errand does not. `Game::drop_load`
/// ends both holds, so every path that stops a carrier holding sets this
/// down too.
#[derive(Component, Clone, Debug)]
pub struct Tidying(pub crate::items::FloorThing);

/// The hold on this body's `Carrying` belongs to the idle clearing errand
/// (`game::base::clearing`): finished output walked out of a machine to a
/// Depot or the stockpile with no post behind it.
///
/// A marker and not a second load state. `Carrying` stays the one load, so
/// every path that sets a carrier's load down covers a clearer unchanged;
/// this says only *whose* trip it is, so `may_tidy`'s "a body holding a
/// hauler's load is mid-trip for somebody else" keeps meaning that, and the
/// clearing errand knows the load is its own to finish. Where the load is
/// going is derived each beat from `Position`, as a hauler's is.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Clearing;

/// Player-only: tool ids installed in the player's tool slots, in slot
/// order — position is what the extraction screen selects by, `Routines`'
/// own reason for keeping its list ordered rather than keyed.
///
/// `Game::install_tool` keeps it to one tool per `ToolCategory` and at most
/// `tuning::MAX_INSTALLED_TOOLS` — for its own writes only: a list loaded
/// from a save is taken as written, and an install replaces just the first
/// same-category tool it finds.
#[derive(Component, Default, Clone)]
pub struct Tools(pub Vec<ToolId>);

/// The implants built into the player, in install order. Player only. An id
/// whose def has gone missing stays here, contributing nothing, so it can
/// still be removed — see `implants::ImplantDb`.
#[derive(Component, Default, Clone, Debug, PartialEq)]
pub struct Implants {
    pub installed: Vec<crate::implants::ImplantId>,
}

/// The Phase Keys the player holds — see `phase_keys`. Player only. Keys are
/// state, not items: no sell, trade or consume path ever sees one, and their
/// effects are derived from `held` on every read, never baked into stats.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseKeys {
    /// Bit `zone - 1` is set when that zone's key is held.
    pub held: u16,
    /// Eligible guardian kills in the current zone that did not drop the key.
    pub misses: u32,
    /// Set once the player has escaped through the Basin Exit.
    pub story_complete: bool,
}

impl PhaseKeys {
    pub fn holds(&self, zone: u32) -> bool {
        (1..=crate::tuning::PHASE_KEY_COUNT).contains(&zone) && self.held & (1 << (zone - 1)) != 0
    }

    pub fn count(&self) -> u32 {
        self.zones().count() as u32
    }

    /// Zones whose key is held, ascending.
    pub fn zones(&self) -> impl Iterator<Item = u32> + '_ {
        (1..=crate::tuning::PHASE_KEY_COUNT).filter(|zone| self.holds(*zone))
    }
}

impl Inventory {
    pub fn add(&mut self, item: ItemId, qty: u32) {
        // Saturating so an unbounded Buffer can never wrap a stack's count.
        if let Some(slot) = self.items.iter_mut().find(|(i, _)| *i == item) {
            slot.1 = slot.1.saturating_add(qty);
        } else {
            self.items.push((item, qty));
        }
    }

    pub fn count(&self, item: &ItemId) -> u32 {
        self.items
            .iter()
            .find(|(i, _)| i == item)
            .map(|(_, q)| *q)
            .unwrap_or(0)
    }

    /// Total units of ordinary cargo held. Banked currencies (an item with
    /// `ItemDef::banked` set) are excluded — this is just how full the
    /// (unbounded) Buffer is, shown to the player.
    pub fn cargo_used(&self, db: &ItemDb) -> u32 {
        self.items
            .iter()
            .filter(|(item, _)| !db.get(item.as_str()).is_some_and(|d| d.banked))
            .map(|(_, qty)| *qty)
            .sum()
    }

    /// Removes up to `qty` of `item`, returning how many were actually
    /// removed. Drops the slot entirely once it hits zero, rather than
    /// leaving a `(item, 0)` behind — callers that list `items` (the status
    /// panel, the inventory screen) shouldn't have to filter zero-quantity
    /// stacks themselves.
    pub fn take(&mut self, item: ItemId, qty: u32) -> u32 {
        let Some(pos) = self.items.iter().position(|(i, _)| *i == item) else {
            return 0;
        };
        let taken = self.items[pos].1.min(qty);
        self.items[pos].1 -= taken;
        if self.items[pos].1 == 0 {
            self.items.remove(pos);
        }
        taken
    }
}

/// A load a posted program is physically carrying to a depot.
///
/// The *only* stored hauling state, and deliberately so. Where the worker is
/// headed (the nearest depot while carrying, its own machine otherwise) and
/// whether it has arrived (`game::base::hauling::at_station`) are both derived
/// from `Position`, so there is one source of truth and no state field that
/// can desync into a worker standing at its machine insisting it is still
/// walking.
///
/// One `(item, qty)` pair rather than a map because
/// `tuning::HAUL_CARRY_CAPACITY` bounds a trip: `Stock::output` is a
/// `BTreeMap` and may hold several item ids, so an uncapped drain would have
/// had to carry all of them, and the save with it.
#[derive(Component, Clone, Debug)]
pub struct Carrying {
    pub item: ItemId,
    pub qty: u32,
}

/// One thing on a base tile's floor, one per tile
/// (`base::floor::spawn_floor_item` is the only writer and asserts the tile
/// is empty), so a drop spreads over tiles rather than stacking. Deliberately
/// has no `Glyph`, for the reason the many queries over
/// `(&Position, &Glyph)` must not see it: the renderer reads
/// `Game::floor_items` instead.
#[derive(Component, Clone, Debug)]
pub struct FloorItem(pub crate::items::FloorThing);

/// A posted program that found no route to where it is trying to get, and the
/// tick it stopped being able to. Written and cleared by
/// `game::base::hauling::haul_step_system`, the one system that walks a field
/// and so the only one that can know.
///
/// **Written on entry only, so `since` is the start of the episode rather
/// than the last tick of it.** That is what makes a stranding an *event* a
/// `&mut Game` pass can find — `Game::note_strandings` reads `since == now`
/// to remember the ones that began this tick — and it is
/// `systems::set_machine_status`'s rule in another shape: entering a state is
/// news, staying in it is not. Re-inserting the marker every tick would leave
/// nothing to distinguish a route that has just broken from one that has been
/// broken for an hour.
///
/// Not saved, like `MachineStatus`: the walk that produced it runs again on
/// the next tick. **It is more than a cache of that tick's answer, though**:
/// `since` gates `STRANDED_SET_DOWN_TICKS`, so a reload restarts the
/// timer and a carrier stranded across one waits the full timeout again.
/// The marker exists at all because the status it drives is
/// written by `systems::task_progress_system` — giving `MachineStatus` two
/// writers would have them ping-pong `Unstaffed`↔`Stranded` every tick, and
/// `set_machine_status` logs on every transition.
///
/// `task_progress_system` runs *first* in the chain, so it reads a marker
/// written on the previous tick. That one-tick lag on a status label is the
/// price of leaving the chain order alone, which is load-bearing for the
/// clog/pickup handoff (see `Game::build_schedule`).
#[derive(Component, Clone, Copy, Debug)]
pub struct Stranded {
    /// The `GameClock` tick the worker entered the state on.
    pub since: u64,
}

/// Marks a stranded carrier whose load has been held past
/// `tuning::STRANDED_SET_DOWN_TICKS` on a route that is still failing.
/// Written only by `haul_step_system`'s route-failed branch and consumed by
/// `Game::set_down_stranded_loads`, so the set-down keeps that branch's
/// gate: a stale `Stranded` left on a re-posted body cannot trigger it.
#[derive(Component, Clone, Copy, Debug)]
pub struct SetDownLoad;

/// A posted walker paying `tuning::SQUEEZE_EXTRA_TICKS` to step into a cell
/// another body holds — the step it is waiting to take, and when the wait
/// began. `hauling::stride` is the one reader.
///
/// Not saved, `Stranded`'s reason: a reload costs a walker at most one more
/// wait, and the walk that wrote it runs again next tick.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Squeezing {
    pub into: Position,
    pub since: u64,
}

#[cfg(test)]
mod inventory_tests {
    use super::*;
    use crate::items::ids;
    use std::path::Path;

    fn item_db() -> ItemDb {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (abilities, _) =
            crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
        ItemDb::load_dir(&assets.join("items"), &abilities)
            .unwrap()
            .0
    }

    #[test]
    fn cargo_used_ignores_banked_currency() {
        let db = item_db();
        let mut inv = Inventory::default();
        inv.add(ItemId::from(ids::CORE_FRAGMENT), 5);
        inv.add(ItemId::from(ids::POWER_CELL), 3);
        inv.add(ItemId::from(ids::RESEARCH_DATA), 90);
        assert_eq!(
            inv.cargo_used(&db),
            8,
            "Research Data is banked, not carried"
        );
    }
}
