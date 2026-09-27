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

    /// Every owned program, in table order — the scheduler's pick order,
    /// and the Base staff screen's row order across *both* sections.
    /// `Game::base_staff`'s sort, over the wider set: that one only ever
    /// wants the Staff half.
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

        let order = self.roster_order();
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
        // `order` is already rank order, so partitioning keeps each half in
        // the order it arrived in — Staff first, then Away, each still by
        // rank.
        let (staff, away): (Vec<_>, Vec<_>) = rows
            .into_iter()
            .partition(|r| r.section == views::WorkSection::Staff);
        let mut rows = staff;
        rows.extend(away);

        views::WorkTable {
            columns,
            rows,
            on_shift: demand.staff,
            jobs: demand.wanted,
            unworked_total: demand.unworked.values().sum(),
        }
    }

    /// Checks or unchecks one program's one column. Removes `Duties`
    /// entirely once every column is back on — `DepotFilter`'s
    /// absent-means-unrestricted rule: a save or a mod reading no
    /// component must see every job admitted, not a component that happens
    /// to be empty.
    pub fn set_duty(&mut self, entity: Entity, duty: Duty, on: bool) -> Result<(), String> {
        self.require_owned_program(entity)?;
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
        Ok(())
    }

    /// Checks or unchecks one column for every program the player owns —
    /// the `[A]` key's whole-column toggle. Never refuses: there is no
    /// single entity to be the wrong one, since it reaches every owned
    /// program through `set_duty` in one pass.
    pub fn set_duty_column(&mut self, duty: Duty, on: bool) -> Result<(), String> {
        let entities: Vec<Entity> = self
            .owned_program_views()
            .into_iter()
            .map(|v| v.entity)
            .collect();
        for entity in entities {
            self.set_duty(entity, duty, on)?;
        }
        Ok(())
    }

    /// Moves a program's row by `delta` places in the *whole* table order
    /// (both sections), clamped at either end — a move that would run off
    /// the end is a no-op rather than a refusal, since the `<`/`>` key has
    /// no reason to know where the table ends.
    pub fn move_staff_row(&mut self, entity: Entity, delta: i32) -> Result<(), String> {
        self.require_owned_program(entity)?;
        let order = self.roster_order();
        let Some(index) = order.iter().position(|&e| e == entity) else {
            // Unreachable in practice: `require_owned_program` already
            // established this is a Tamed program the player owns, and
            // every such program carries the `Position` `roster_order`
            // (via `owned_program_views`) requires. Kept as a no-op rather
            // than a panic, since this is reached from a key press.
            return Ok(());
        };
        let last = order.len().saturating_sub(1) as i32;
        let target = (index as i32 + delta).clamp(0, last) as usize;
        if target == index {
            return Ok(());
        }
        let other = order[target];
        let rank_a = self.world.get::<StaffRank>(entity).map(|r| r.0);
        let rank_b = self.world.get::<StaffRank>(other).map(|r| r.0);
        if let (Some(a), Some(b)) = (rank_a, rank_b) {
            self.world.get_mut::<StaffRank>(entity).unwrap().0 = b;
            self.world.get_mut::<StaffRank>(other).unwrap().0 = a;
        }
        Ok(())
    }
}
