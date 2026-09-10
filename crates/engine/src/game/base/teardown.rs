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

use crate::base_grid::BaseGrid;
use crate::components::{
    CarryingProgram, Hopper, MachineStatus, Position, Stock, Structure, Task, TaskKind,
};
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
            let mut posted = self.world.query::<(Entity, &Task)>();
            posted
                .iter(&self.world)
                .find(|(_, t)| t.target == rig && matches!(t.kind, TaskKind::GatherResource))
                .map(|(worker, _)| worker)
        };
        let Some(worker) = staffed else {
            return;
        };

        // The crew fetch, above the hopper read: a delivery this beat is a
        // program the rig starts on the next one, which is the same beat
        // cadence a hand-load lands on.
        self.step_carrier_fetch(rig, worker);

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

    /// One beat of a posted body's trip to a Quarantine Rack: the delivery
    /// if it is already holding a carrier, the pick-up if the rig has run
    /// dry and there is one within reach.
    ///
    /// **One trip, one carrier**, which is what the carry shape holds — the
    /// same rule `Carrying` keeps for a stack of items. The trip takes two
    /// beats and `CarryingProgram` is the record of it: a body holding one
    /// is a body the scheduler must not free and a destruction path must not
    /// drop.
    ///
    /// Here rather than in a bevy system for the reason this whole pass
    /// exists — and specifically so the fetch and the strip cannot disagree
    /// about the hopper's room, which they would the moment they were two
    /// passes with a schedule between them. Both routes into a hopper go
    /// through `Game::hopper_room`.
    ///
    /// **A rig with no standing tool does not fetch**, and falls through to
    /// the `Starved` its empty hopper already writes: nothing is feeding it,
    /// and what the player has to do about that is hand it a tool once.
    /// `Hopper::standing_tool` is the only thing that can name one, since
    /// nobody is standing at the rig when a carrier arrives this way.
    fn step_carrier_fetch(&mut self, rig: Entity, worker: Entity) {
        if let Some(held) = self
            .world
            .get::<CarryingProgram>(worker)
            .map(|c| c.0.clone())
        {
            if self.hopper_room(rig) == 0 {
                // The trip outlives a full hopper rather than destroying the
                // carrier: the body stands holding it until the rig has
                // chewed through what it has.
                return;
            }
            let Some(tool) = self
                .world
                .get::<Hopper>(rig)
                .and_then(|h| h.standing_tool.clone())
            else {
                return;
            };
            let label = self.downed_program_label(&held);
            if let Some(mut hopper) = self.world.get_mut::<Hopper>(rig) {
                hopper.queue.push(crate::components::HopperEntry {
                    program: held,
                    tool,
                });
            }
            self.world.entity_mut(worker).remove::<CarryingProgram>();
            self.log_base(format!("A program loads the rig with {label}."));
            return;
        }

        // Only when the rig has run dry: a fetch is what an idle rig does,
        // not a second stocking rule running beside the player's own.
        let empty = self
            .world
            .get::<Hopper>(rig)
            .is_some_and(|h| h.queue.is_empty());
        if !empty || self.hopper_room(rig) == 0 {
            return;
        }
        if self
            .world
            .get::<Hopper>(rig)
            .and_then(|h| h.standing_tool.clone())
            .is_none()
        {
            return;
        }
        let Some(rack) = self.stocked_rack_in_reach(worker) else {
            return;
        };
        let Some(taken) = self
            .world
            .get_mut::<crate::components::Racked>(rack)
            .filter(|shelf| !shelf.0.is_empty())
            .map(|mut shelf| shelf.0.remove(0))
        else {
            return;
        };
        self.world.entity_mut(worker).insert(CarryingProgram(taken));
    }

    /// The first Quarantine Rack holding a carrier that `worker` could walk
    /// to, in `(x, y)` order.
    ///
    /// `hauling::crew_reach` built from the **body** and asked of each rack,
    /// `schedule_base_labour`'s own discipline: one walk answers every rack
    /// rather than one walk per rack. **An unreachable rack is not a want** —
    /// the body stays on its post and nothing is announced, `build_wants`'
    /// deadlock rule.
    fn stocked_rack_in_reach(&mut self, worker: Entity) -> Option<Entity> {
        let stocked: Vec<(i32, i32, Entity)> = {
            let db = self.world.resource::<StructureDb>();
            let mut found: Vec<(i32, i32, Entity)> = self
                .world
                .iter_entities()
                .filter_map(|e| {
                    let kind = &e.get::<Structure>()?.kind;
                    let p = e.get::<Position>()?;
                    db.get(kind)?.racks.as_ref()?;
                    let shelf = e.get::<crate::components::Racked>()?;
                    (!shelf.0.is_empty()).then_some((p.x, p.y, e.id()))
                })
                .collect();
            found.sort();
            found
        };
        if stocked.is_empty() {
            return None;
        }
        let from = self.world.get::<Position>(worker).copied()?;
        let blocked = self.structure_tiles();
        let pocket_radius = self.world.resource::<BaseGrid>().radius();
        let field = {
            let grid = self.world.resource::<BaseGrid>();
            crate::game::base::hauling::crew_reach(grid, from, &blocked, pocket_radius)
        };
        let grid = self.world.resource::<BaseGrid>();
        stocked
            .into_iter()
            .find(|(x, y, _)| {
                crate::game::base::hauling::reaches(
                    grid,
                    &field,
                    from,
                    Position { x: *x, y: *y },
                    &blocked,
                )
            })
            .map(|(_, _, e)| e)
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
