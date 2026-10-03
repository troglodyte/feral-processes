//! The Power Siphon: holding a program in a machine for its power, and the
//! price of letting it go.
//!
//! A held program carries `components::Siphoned` and reads as
//! `ProgramRole::Siphoned`, which is what keeps it out of the labour pool,
//! off the map and out of every door that compares `== Staff`. The ledger
//! counts the siphon's supply only while some program holds a marker
//! pointing at it (`power::ledger`).

use crate::tuning::SIPHON_RELEASE_INTEGRITY_LOSS;
use crate::*;

/// Integrity a program has left after a siphon lets go of it: three
/// quarters of `max_hp` comes off, floored at 1 so a release never kills.
///
/// Pure so the API and its tests ask one rule.
pub(crate) fn siphon_release_hp(hp: i32, max_hp: i32) -> i32 {
    let loss = (max_hp as f32 * SIPHON_RELEASE_INTEGRITY_LOSS) as i32;
    (hp - loss).max(1)
}

impl Game {
    /// Holds `program` in `siphon`.
    ///
    /// **Every refusal lands before anything is written**, `pin_subject`'s
    /// rule, so a refused hold leaves the program's `Task` and the log
    /// alone.
    pub fn siphon_program(&mut self, program: Entity, siphon: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let owner = self
            .world
            .get::<Tamed>(program)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != self.player_entity() {
            return Err("You don't control that program.".into());
        }
        // A partied, wielded, away, pinned or already-held program comes
        // home first; a posted one is `Staff` and is freed below.
        if self.program_role(program) != Some(ProgramRole::Staff) {
            return Err(
                "Only a program on the base staff can be held in a Power Siphon — bring it \
                 home first."
                    .into(),
            );
        }
        if !self.is_siphon(siphon) {
            return Err("That structure can't hold a program.".into());
        }
        if self.siphon_holder(siphon).is_some() {
            return Err("That Power Siphon is already holding a program.".into());
        }
        self.require_base()?;
        // A posted program is `Staff`, so the role check never saw its
        // `Task`: freeing it here is what stops the scheduler's stale post
        // (`pin_subject`'s reason). A carried kill is put back first, as a
        // demolition does, rather than destroyed with the component.
        self.return_carried_program(program);
        self.world
            .entity_mut(program)
            .insert(components::Siphoned { siphon })
            .remove::<Task>()
            .remove::<Carrying>();
        let name = self.creature_label(program);
        self.log(format!("{name} is locked into the Power Siphon."));
        Ok(())
    }

    /// Lets a held program go, at the price of
    /// `SIPHON_RELEASE_INTEGRITY_LOSS` of its `max_hp`.
    pub fn release_siphoned(&mut self, program: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if self.world.get::<components::Siphoned>(program).is_none() {
            return Err("That program isn't held in a Power Siphon.".into());
        }
        self.let_go(program);
        let name = self.creature_label(program);
        self.log(format!("{name} comes out of the Power Siphon hurt."));
        Ok(())
    }

    /// Whoever `siphon` is holding, if anyone.
    pub fn siphon_holder(&self, siphon: Entity) -> Option<Entity> {
        self.world
            .iter_entities()
            .find(|e| {
                e.get::<components::Siphoned>()
                    .is_some_and(|s| s.siphon == siphon)
            })
            .map(|e| e.id())
    }

    /// Every siphon orthogonally beside the party in base space, in `(x, y)`
    /// order — `adjacent_teardown_rigs`' reach.
    pub fn adjacent_siphons(&self) -> Vec<Entity> {
        let Some((px, py)) = self.base_pos() else {
            return Vec::new();
        };
        let mut found: Vec<(i32, i32, Entity)> = self
            .world
            .iter_entities()
            .filter(|e| self.is_siphon(e.id()))
            .filter_map(|e| {
                let p = e.get::<Position>()?;
                Some((p.x, p.y, e.id()))
            })
            .filter(|(x, y, _)| {
                crate::game::base::collect::ORTHOGONAL
                    .iter()
                    .any(|(dx, dy)| (*x, *y) == (px + dx, py + dy))
            })
            .collect();
        found.sort();
        found.into_iter().map(|(_, _, e)| e).collect()
    }

    /// The one place a release writes: the marker comes off and the price
    /// comes out of the program, whichever door asked.
    fn let_go(&mut self, program: Entity) {
        self.world
            .entity_mut(program)
            .remove::<components::Siphoned>();
        if let Some(mut stats) = self.world.get_mut::<Stats>(program) {
            stats.hp = siphon_release_hp(stats.hp, stats.max_hp);
        }
    }

    pub(crate) fn is_siphon(&self, structure: Entity) -> bool {
        self.world
            .get::<Structure>(structure)
            .and_then(|s| self.world.resource::<StructureDb>().get(&s.kind))
            .is_some_and(|def| def.siphons)
    }

    /// A destroyed siphon lets its program go — hurt, as any release is.
    /// Called on both destruction paths (`remove_structure`, which the Home
    /// cascade shares, and `damage_structure`), since a program left holding
    /// a despawned `Entity` would be out of play forever with nothing to
    /// release it. A no-op for every structure that holds nothing.
    pub(crate) fn release_siphon_at(&mut self, structure: Entity) {
        let Some(holder) = self.siphon_holder(structure) else {
            return;
        };
        self.let_go(holder);
        let name = self.creature_label(holder);
        self.log(format!("{name} comes out hurt — the Power Siphon is gone."));
    }
}
