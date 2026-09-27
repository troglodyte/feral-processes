//! `Game`'s API onto the Base staff work table — phase 3 of work
//! assignments (`docs/superpowers/specs/2026-09-27-work-assignments-design.md`
//! §5). `work_table` is the one derivation; `set_duty`, `set_duty_column`
//! and `move_staff_row` are its only writers.

use crate::components::{Duties, StaffRank};
use crate::duties::Duty;
use crate::*;

impl Game {
    /// The one ownership check every writer below shares — refuses
    /// anything that is not a tamed program the player owns, so a stray
    /// entity id from a stale save or a modded screen cannot toggle or
    /// reorder a program that isn't the player's.
    fn require_owned_program(&self, entity: Entity) -> Result<(), String> {
        let player = self.player_entity();
        if self
            .world
            .get::<Tamed>(entity)
            .is_some_and(|t| t.owner == player)
        {
            Ok(())
        } else {
            Err("That is not one of your programs.".to_string())
        }
    }

    /// Every owned program, in raw rank order — `Game::base_staff`'s sort,
    /// over the wider set: that one only ever wants the Staff half. **Not**
    /// the screen's row order: an away program's rank can fall between two
    /// staff ranks, so this mixes the two sections. `display_order` is the
    /// one that doesn't.
    fn roster_order(&mut self) -> Vec<Entity> {
        let mut entities: Vec<Entity> = self
            .owned_program_views()
            .into_iter()
            .map(|v| v.entity)
            .collect();
        entities.sort_by_key(|&e| {
            let rank = self.world.get::<StaffRank>(e);
            (rank.is_none(), rank.map(|r| r.0), e)
        });
        entities
    }

    /// The Base staff screen's own row order: `roster_order` with the Staff
    /// half pulled in front of Away, each half keeping its internal rank
    /// order — `work_table`'s rows are built in exactly this order, and
    /// `move_staff_row` moves within it rather than within `roster_order`,
    /// or `<`/`>` on a staff row would sometimes swap it with an away row
    /// sitting between it and its visible neighbour, with nothing moving
    /// on screen.
    fn display_order(&mut self) -> Vec<Entity> {
        let order = self.roster_order();
        let (staff, away): (Vec<Entity>, Vec<Entity>) = order
            .into_iter()
            .partition(|&e| self.program_role(e) == Some(ProgramRole::Staff));
        let mut result = staff;
        result.extend(away);
        result
    }

    /// One past the highest `StaffRank` any live entity holds —
    /// `roster_parts`'s mint, shared here so a program whose snapshot
    /// predates ranks (`staff_rank: None`) takes exactly the place at the
    /// back of the table a brand new program would, rather than a copy of
    /// this formula drifting between the two doors.
    pub(crate) fn next_staff_rank(&mut self) -> u32 {
        self.world
            .query::<&StaffRank>()
            .iter(&self.world)
            .map(|r| r.0)
            .max()
            .map_or(0, |max| max + 1)
    }

    /// The Base staff screen's one derivation: every column's unworked
    /// count, every row in table order (Staff first, then Away, rank order
    /// held within each), and the header's three figures. Nothing here
    /// hands the renderer the `World` — an `EntityView`, a `String` and a
    /// `Vec<bool>` per row is the whole of it.
    pub fn work_table(&mut self) -> views::WorkTable {
        let demand = self.labour_demand();
        let columns: Vec<views::WorkColumn> = Duty::ALL
            .iter()
            .map(|&duty| views::WorkColumn {
                duty,
                label: duty.label(),
                unworked: demand.unworked.get(&duty).copied().unwrap_or(0),
            })
            .collect();

        let order = self.display_order();
        let mut views_by_entity: std::collections::HashMap<Entity, views::EntityView> = self
            .owned_program_views()
            .into_iter()
            .map(|v| (v.entity, v))
            .collect();

        let mut rows: Vec<views::WorkRow> = Vec::with_capacity(order.len());
        for entity in order {
            let Some(program) = views_by_entity.remove(&entity) else {
                continue;
            };
            let role = self.program_role(entity);
            let section = if role == Some(ProgramRole::Staff) {
                views::WorkSection::Staff
            } else {
                views::WorkSection::Away
            };
            let doing = if section == views::WorkSection::Staff {
                self.staff_activity(entity)
            } else {
                self.program_activity(entity)
            };
            let cells: Vec<bool> = Duty::ALL
                .iter()
                .map(|d| {
                    !self
                        .world
                        .get::<Duties>(entity)
                        .is_some_and(|o| o.off.contains(d))
                })
                .collect();
            let rank = self
                .world
                .get::<StaffRank>(entity)
                .map(|r| r.0)
                .unwrap_or(u32::MAX);
            rows.push(views::WorkRow {
                program,
                section,
                role,
                doing,
                cells,
                rank,
            });
        }
        // `order` is already `display_order` — Staff first, then Away, each
        // still by rank — so the rows built from it need no re-partition.
        views::WorkTable {
            columns,
            rows,
            on_shift: demand.staff,
            jobs: demand.wanted,
            unworked_total: demand.unworked.values().sum(),
        }
    }

    /// The toggle itself, shared by `set_duty` and `set_duty_column` so
    /// neither restates it — and so `set_duty_column` reassigns once for the
    /// whole column rather than once per row, a matching per row being
    /// wasted work that the last one overwrites.
    fn toggle_duty(&mut self, entity: Entity, duty: Duty, on: bool) {
        if let Some(mut duties) = self.world.get_mut::<Duties>(entity) {
            if on {
                duties.off.remove(&duty);
            } else {
                duties.off.insert(duty);
            }
        } else if !on {
            self.world.entity_mut(entity).insert(Duties {
                off: [duty].into_iter().collect(),
            });
        }
        if self
            .world
            .get::<Duties>(entity)
            .is_some_and(|d| d.off.is_empty())
        {
            self.world.entity_mut(entity).remove::<Duties>();
        }
    }

    /// Checks or unchecks one program's one column. Removes `Duties`
    /// entirely once every column is back on — `DepotFilter`'s
    /// absent-means-unrestricted rule: a save or a mod reading no
    /// component must see every job admitted, not a component that happens
    /// to be empty.
    ///
    /// **Ends with a reassignment**, `cancel_build_request`'s habit after a
    /// change the scheduler cares about: without it the table's `N!` counts
    /// and header are the *previous* pass's figures until the next real
    /// tick, since `Mode::BaseStaff` spends none of its own. The posting half
    /// alone (`reassign_base_labour`), never the whole beat — the beat rolls
    /// tantrums, and a keypress with the clock stopped must not re-roll one.
    pub fn set_duty(&mut self, entity: Entity, duty: Duty, on: bool) -> Result<(), String> {
        self.require_owned_program(entity)?;
        self.toggle_duty(entity, duty, on);
        self.reassign_base_labour();
        Ok(())
    }

    /// Checks or unchecks one column for every program the player owns —
    /// the `[A]` key's whole-column toggle. Never refuses: there is no
    /// single entity to be the wrong one, since it reaches every owned
    /// program through `toggle_duty` in one pass, then reassigns once.
    pub fn set_duty_column(&mut self, duty: Duty, on: bool) -> Result<(), String> {
        let entities: Vec<Entity> = self
            .owned_program_views()
            .into_iter()
            .map(|v| v.entity)
            .collect();
        for entity in entities {
            self.toggle_duty(entity, duty, on);
        }
        self.reassign_base_labour();
        Ok(())
    }

    /// Moves a program's row by `delta` places in `display_order` — the
    /// *screen's* table order, both sections — clamped at either end: a
    /// move that would run off the end is a no-op rather than a refusal,
    /// since the `<`/`>` key has no reason to know where the table ends.
    ///
    /// **Renumbers the whole order densely, 0.. by position, after the
    /// swap** — never by exchanging the two rows' rank *values*. Two
    /// programs can carry the same rank (a build-site refund mints one off
    /// the live count, which no longer includes the program it is
    /// replacing) or a rank at all (a pre-feature save's `staff_rank:
    /// None`, minted by `next_staff_rank` on load or on refund); swapping
    /// values a tie shares leaves both unchanged, and swapping into a `None`
    /// leaves one row with no rank to compare next time. Renumbering by
    /// position instead makes every row's rank distinct and present in one
    /// pass, whatever it inherited.
    pub fn move_staff_row(&mut self, entity: Entity, delta: i32) -> Result<(), String> {
        self.require_owned_program(entity)?;
        let mut order = self.display_order();
        let Some(index) = order.iter().position(|&e| e == entity) else {
            // Unreachable in practice: `require_owned_program` already
            // established this is a Tamed program the player owns, and
            // every such program carries the `Position` `display_order`
            // (via `owned_program_views`) requires. Kept as a no-op rather
            // than a panic, since this is reached from a key press.
            return Ok(());
        };
        let last = order.len().saturating_sub(1) as i32;
        let target = (index as i32 + delta).clamp(0, last) as usize;
        if target == index {
            return Ok(());
        }
        order.swap(index, target);
        for (rank, &e) in order.iter().enumerate() {
            self.world.entity_mut(e).insert(StaffRank(rank as u32));
        }
        Ok(())
    }
}
