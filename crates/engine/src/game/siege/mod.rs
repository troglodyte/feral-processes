//! A siege: the sector's wild programs walk in through the base's one
//! door, steal what they can carry, wreck what they cannot, and withdraw
//! on a morale break.
//!
//! Separate from the GC Entropy Sweep (`game/base/upkeep.rs`). The two
//! events share nothing but the shape of their clock — `clock.rs` is
//! `resources::RaidPressure`'s pattern applied to its own
//! `resources::SiegePressure` rather than that resource — and the
//! `StructureDef::raid_defense` a turret keeps paying into.
//!
//! `docs/superpowers/archive/specs/2026-09-22-siege-design.md` is the design
//! record; `docs/superpowers/plans/2026-09-22-siege.md` is the task
//! breakdown this module is built against.

pub(crate) mod board;
pub(crate) mod clock;
pub(crate) mod offscreen;
pub(crate) mod persist;
pub(crate) mod raiders;
pub(crate) mod turrets;

use std::collections::{HashSet, VecDeque};

use bevy_ecs::prelude::Entity;
use rand::RngExt;

use crate::Game;
use crate::game::spawning::SpawnEscalation;
use crate::resources::{GameClock, Party, ZoneLevel};
use crate::tactical::TacticalBattle;
use crate::tactical::map::{BattleSpec, NEIGHBOURS};
use crate::world::WorldMap;

use self::offscreen::pack_size;

impl Game {
    /// Opens a siege on the board built from the base itself, rather than
    /// resolving it off-screen — `Game::siege_check`'s home branch, taken
    /// whenever `Game::base_pos().is_some()`.
    ///
    /// **The board is built, not `generate`d** (`board::build`), and
    /// `BattleSpec::side()` is never read on the spec minted here — the
    /// one place a siege departs from every other tactical fight.
    /// `fights_tactically` is not consulted and `start_battle` is not
    /// called: a siege is always tactical, whatever the profile toggle
    /// says.
    ///
    /// `false` on the same reasons `board::build` and an absent `base_pos`
    /// already refuse for — `Game::siege_check` then resolves the siege
    /// off-screen instead of staging it.
    pub(crate) fn open_siege(&mut self) -> bool {
        let Some(siege_board) = board::build(self) else {
            return false;
        };
        let Some(player_cell) = self.base_pos() else {
            return false;
        };

        let zone = self.world.resource::<ZoneLevel>().0;
        let site = self.anchor_position().unwrap_or(player_cell);
        let biome = self
            .world
            .resource_mut::<WorldMap>()
            .tile(site.0, site.1)
            .biome;

        // Read before anything is spawned or seated: every entity that
        // exists in base space *before* this call is what the fight opens
        // with, raiders excepted.
        let structures = self.structure_footprints();
        let staff = self.base_bodies();
        let raiders = self.spawn_siege_pack(site, zone);

        let mut battle = TacticalBattle::open(
            BattleSpec {
                world_seed: self.world.resource::<WorldMap>().seed(),
                site,
                tick: self.world.resource::<GameClock>().tick,
                zone,
                biome,
                bodies: (structures.len() + staff.len() + raiders.len() + 1) as u32,
            },
            siege_board.board.clone(),
        );

        battle.siege_origin = siege_board.origin;
        battle.siege_door = siege_board.door;

        // **Structures are seated as bodies, and left out of initiative.**
        // They are obstacles a swing can be aimed at (Task 10), not
        // combatants — shaped before they are seated, `set_shape`'s own
        // ordering rule. `board::seat_structures` is this loop, shared with
        // `siege::persist::restore`, which needs it again once a save/load
        // round trip has rebuilt the structures fresh.
        let structure_entities =
            board::seat_structures(&mut battle, &siege_board, structures, |e| {
                self.is_barrier(e)
            });

        // **Every body in base space is seated where it already stands.**
        // Nobody is deployed; a body whose own cell fell outside the fill
        // (a program adopted on the surface that has not drifted in) is
        // simply not seated.
        for (entity, pos) in staff {
            if let Some(cell) = siege_board.to_board((pos.x, pos.y)) {
                battle.place(entity, cell);
            }
        }

        // **The player and their party**, at `base_pos` — the door-adjacent
        // free cells taking the overflow once that cell (and its immediate
        // neighbours) are spoken for.
        let party: Vec<Entity> = std::iter::once(self.player_entity())
            .chain(self.world.resource::<Party>().0.iter().copied())
            .filter(|&e| self.creature_alive(e))
            .collect();
        let player_seat = siege_board
            .to_board(player_cell)
            .unwrap_or(siege_board.door);
        for entity in party {
            place_nearby(&mut battle, player_seat, entity);
        }

        // **Raiders enter at the door.** One body to a cell means they file
        // in at and behind it. Counted by what actually seated, not by
        // `raiders.len()` — `TacticalBattle::siege_pack` is
        // `siege::raiders::siege_morale_broken`'s reading of "the pack this
        // fight opened with," and a raider turned away for want of a free
        // cell near the door never joined it.
        let seated: HashSet<Entity> = raiders
            .iter()
            .copied()
            .filter(|&entity| place_nearby(&mut battle, siege_board.door, entity))
            .collect();
        // **Nothing seated is not a siege.** An empty habitat pool
        // (`spawn_siege_pack`'s own doc) or a door with no free cell near it
        // both leave `raiders` with nobody actually on the board — opening
        // one anyway would seat the player and every staff body in a fight
        // with no hostiles in it, `TacticalBattle::siege_pack == 0` reading
        // as "not a siege" to `siege::raiders::siege_morale_broken` and
        // `siege::persist::assemble` alike, so it would neither ever end on
        // a morale break nor ever save. Reported exactly as `board::build`
        // and an absent `base_pos` already are: `false`, on which
        // `Game::siege_check` resolves the siege off-screen instead.
        if seated.is_empty() {
            for &raider in &raiders {
                self.world.despawn(raider);
            }
            return false;
        }
        // A raider spawned but turned away for want of a free cell has
        // nowhere to stand and no fight driving it — left alive it wanders
        // as an ordinary `Hostile` carrying a `Besieger` marker nothing
        // reads, uncounted by every wild-population census and invisible on
        // the surface map (`Game::stands_in_base_space`'s "every other
        // glyph is a zone-map fixture" not being true of a body inside the
        // base). Despawned here rather than left for a sweep that only runs
        // when a fight ends, since this one never opened for it to be in.
        for &raider in &raiders {
            if !seated.contains(&raider) {
                self.world.despawn(raider);
            }
        }
        battle.siege_pack = seated.len() as u32;

        let standing: Vec<Entity> = battle
            .bodies()
            .map(|(entity, _)| entity)
            .filter(|entity| !structure_entities.contains(entity))
            .collect();
        battle.set_initiative(self.roll_turn_order(&standing));

        self.world
            .resource_mut::<crate::resources::MessageLog>()
            .open_battle();
        self.world.insert_resource(battle);
        // **The very first actor needs the same check every later one gets
        // from inside `hand_on_turn`.** Nothing else ever calls
        // `skip_disengaged_turns` before a turn has actually been handed on,
        // so a disengaged staff body that won initiative outright would
        // otherwise be driven through a full AI turn on round one — walking
        // toward the fight, the "accepted consequence" §5 rules out — before
        // anything here ever ends a turn for the wrap to catch.
        self.skip_disengaged_turns();
        self.log_base_kind(
            crate::resources::MessageKind::Raid,
            "Besiegers pour in through the door!".to_string(),
        );
        true
    }

    /// The besieging pack: `siege::pack_size(zone)` bodies of one habitat
    /// species drawn at `site`'s biome — `Game::habitat_pools`, the same
    /// pool an ordinary surface pack spawn draws from, no new species
    /// content.
    ///
    /// `Game::spawn_group` rather than `Game::spawn_pack`, because
    /// `spawn_pack` rolls its own size through `roll_group_size` and has no
    /// way to be pinned at `pack_size(zone)` exactly — and `open_siege`'s
    /// pack and `resolve_siege_offscreen`'s priced one must agree on a
    /// count, or the siege you fight and the siege you miss are different
    /// sizes.
    ///
    /// Empty when `site`'s biome offers nothing to spawn — a base founded
    /// somewhere with no habitat species at all, which no shipped biome is.
    fn spawn_siege_pack(&mut self, site: (i32, i32), zone: u32) -> Vec<Entity> {
        let Some((candidates, _boss_candidates)) = self.habitat_pools(site.0, site.1, None, 0)
        else {
            return Vec::new();
        };
        if candidates.is_empty() {
            return Vec::new();
        }
        let species_id = {
            let mut rng = self.world.resource_mut::<crate::resources::GameRng>();
            let idx = rng.0.random_range(0..candidates.len());
            candidates[idx].clone()
        };
        let pack = self.spawn_group(
            &species_id,
            pack_size(zone),
            site.0,
            site.1,
            SpawnEscalation::surface(),
            false,
        );
        for &raider in &pack {
            self.world
                .entity_mut(raider)
                .insert(crate::components::Besieger);
        }
        pack
    }
}

/// Seats `body` at `target`, or the nearest free board cell to it —
/// `TacticalBattle::place`'s own refusal walked outward ring by ring, eight
/// ways, until a cell takes it. A cell it never reaches (an unseatable
/// board, or a board with no free cell left at all) leaves `body` unseated,
/// the same "simply not seated" answer every other seating refusal here
/// gives.
fn place_nearby(battle: &mut TacticalBattle, target: (i32, i32), body: Entity) -> bool {
    if battle.place(body, target) {
        return true;
    }
    let mut seen: HashSet<(i32, i32)> = HashSet::from([target]);
    let mut queue: VecDeque<(i32, i32)> = VecDeque::from([target]);
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in NEIGHBOURS {
            let next = (x + dx, y + dy);
            if !battle.board.in_bounds(next.0, next.1) || !seen.insert(next) {
                continue;
            }
            if battle.place(body, next) {
                return true;
            }
            queue.push_back(next);
        }
    }
    false
}
