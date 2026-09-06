//! The Teardown Rig's own tick: what a loaded hopper turns into, and when
//! — see `docs/superpowers/specs/2026-09-04-program-extraction-design.md`
//! section 10.
//!
//! **A `&mut Game` pass rather than a bevy system, and that is forced.**
//! `Game::extraction_yield` and `Game::extraction_ticks` are `&Game`
//! methods folding perks, `SpeciesDb`, `ItemDb` and the best standing bench
//! tier; a bevy system cannot call them and would have to re-derive the
//! formula, which is exactly the crack section 3's "one derivation" exists
//! to prevent. So this joins `run_dig_crew`, `run_build_crew`,
//! `run_repair_bays`, `run_sorties` and `run_routes` in `game/turn.rs`
//! instead, and the rig calls the same two functions the player's own
//! extraction calls.

use crate::components::{Hopper, MachineStatus, Position, Stock, Structure, Task, TaskKind};
use crate::resources::{BattleTelemetry, GameClock, MessageKind, MessageLog, PowerGrid};
use crate::structures::StructureDb;
use crate::systems::{StallSite, set_machine_status};
use bevy_ecs::prelude::Entity;

use crate::Game;

impl Game {
    /// Every standing rig that carries a hopper, in tile order.
    ///
    /// Sorted for `run_repair_bays`' and `assembler_system`'s shared
    /// reason: bevy's iteration order is not stable, and two rigs finishing
    /// in a different order between runs would reorder their log lines.
    fn teardown_rigs(&mut self) -> Vec<Entity> {
        let db = self.world.resource::<StructureDb>();
        let mut found: Vec<(i32, i32, Entity)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let kind = &e.get::<Structure>()?.kind;
                let p = e.get::<Position>()?;
                db.get(kind)?.strips.as_ref()?;
                e.contains::<Hopper>().then_some((p.x, p.y, e.id()))
            })
            .collect();
        found.sort();
        found.into_iter().map(|(_, _, e)| e).collect()
    }

    /// One tick of every rig — `run_repair_bays`' shape.
    pub(crate) fn run_teardown_rigs(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        for rig in self.teardown_rigs() {
            self.step_teardown_rig(rig);
        }
    }

    /// The gates mirror `assembler_system`'s, each writing `MachineStatus`
    /// on the transition alone — `set_machine_status`' rule that entering a
    /// state is news and staying in it is not.
    ///
    /// **Which gate writes which status is decided by what
    /// `idle_machine_system` will do a moment later**, since this pass runs
    /// before the schedule. A dark rig and an unstaffed one are both left
    /// alone here, exactly as `assembler_system` leaves them, because that
    /// system writes `Unpowered` and `Idle` over anything this wrote. It
    /// *skips* a staffed, lit machine entirely, so the three states a
    /// working rig can be in are this function's alone to say.
    fn step_teardown_rig(&mut self, rig: Entity) {
        if self.world.resource::<PowerGrid>().is_dark(rig) {
            return;
        }
        // The same predicate `assembler_system` uses, read through the world
        // rather than a system's query — the shape `run_repair_bays` takes.
        let staffed = {
            let mut posted = self.world.query::<&Task>();
            posted
                .iter(&self.world)
                .any(|t| t.target == rig && matches!(t.kind, TaskKind::GatherResource))
        };
        if !staffed {
            return;
        }

        // **`Starved` and not `Idle` for an empty hopper**, because
        // `idle_machine_system` does not reach a staffed machine: it
        // `continue`s on `worked` before its `Idle` write. The hopper *is*
        // this machine's input buffer, and "nothing is feeding it" is what
        // an assembler says about an empty one.
        let Some(entry) = self
            .world
            .get::<Hopper>(rig)
            .and_then(|h| h.queue.first().cloned())
        else {
            self.set_rig_status(rig, MachineStatus::Starved);
            return;
        };

        let Some(tool) = self
            .installed_tools()
            .into_iter()
            .find(|def| def.id == entry.tool)
        else {
            // The tool was uninstalled after the load. The queue is the
            // player's instruction and the program is not destroyed for it;
            // the rig simply has nothing to work it with, which is the same
            // shape as an empty hopper rather than a program that vanished.
            self.set_rig_status(rig, MachineStatus::Starved);
            return;
        };

        // **The completion gate is room for the whole payout, not room for
        // one unit.** The assembler can ask for one because it makes one at
        // a time; a program pays several at once, and clamping to the room
        // available would destroy the rest. A rig that cannot hold the
        // payout holds the program (decision 9).
        let granted = self.extraction_yield(&entry.program, &tool);
        let total: u32 = granted.iter().map(|(_, qty)| *qty).sum();
        let room = self
            .world
            .get::<Stock>(rig)
            .map(|s| s.output_room())
            .unwrap_or(0);
        if room < total {
            self.set_rig_status(rig, MachineStatus::Clogged);
            return;
        }
        self.set_rig_status(rig, MachineStatus::Running);

        // Quoted once, before the spend — a bench demolished mid-strip must
        // not change what this program was already priced at, the rule
        // `extract_program` takes on its own tick loop.
        let due = self.extraction_ticks(&tool);
        let progress = {
            let mut hopper = self.world.get_mut::<Hopper>(rig).unwrap();
            hopper.progress += 1;
            hopper.progress
        };
        if progress < due {
            return;
        }

        {
            let mut hopper = self.world.get_mut::<Hopper>(rig).unwrap();
            hopper.queue.remove(0);
            hopper.progress = 0;
        }
        {
            let mut stock = self.world.get_mut::<Stock>(rig).unwrap();
            for (item, qty) in &granted {
                *stock.output.entry(item.clone()).or_default() += qty;
            }
        }

        // One line per completed program, unlike the assembler, which logs
        // no per-unit line. An ICE Breaker is a known constant output; a
        // program's yield is unique, unrepeatable information the player has
        // no other record of.
        let label = self.downed_program_label(&entry.program);
        let parts: Vec<String> = granted
            .iter()
            .map(|(item, qty)| format!("{qty} {}", self.item_name(item)))
            .collect();
        let line = if parts.is_empty() {
            format!("The rig strips {label} and salvages nothing usable.")
        } else {
            format!("The rig strips {label}: {}.", parts.join(", "))
        };
        self.log_base_kind(MessageKind::Loot, line);
    }

    /// `assembler_system`'s `announce` closure in a `&mut Game` pass: the
    /// same `set_machine_status` door, reached through `resource_scope`
    /// because the log and the telemetry buffer are resources this world
    /// also holds the status component in.
    fn set_rig_status(&mut self, rig: Entity, next: MachineStatus) {
        let Some(kind) = self.world.get::<Structure>(rig).map(|s| s.kind.clone()) else {
            return;
        };
        let Some(name) = self
            .world
            .resource::<StructureDb>()
            .get(&kind)
            .map(|d| d.name.clone())
        else {
            return;
        };
        let Some(tile) = self.world.get::<Position>(rig).map(|p| (p.x, p.y)) else {
            return;
        };
        let tick = self.world.resource::<GameClock>().tick;
        self.world
            .resource_scope(|world, mut log: bevy_ecs::prelude::Mut<MessageLog>| {
                world.resource_scope(
                    |world, mut telemetry: bevy_ecs::prelude::Mut<BattleTelemetry>| {
                        let Some(mut status) = world.get_mut::<MachineStatus>(rig) else {
                            return;
                        };
                        set_machine_status(
                            &mut status,
                            next,
                            &name,
                            &mut log,
                            StallSite {
                                telemetry: &mut telemetry,
                                tick,
                                machine: tile,
                                kind: &kind,
                            },
                        );
                    },
                );
            });
    }
}
