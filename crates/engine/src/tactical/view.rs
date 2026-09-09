//! What a tactical fight looks like from outside the engine.
//!
//! `battle_view`'s counterpart for the second combat model, and derived on
//! every read for the same reason: a fight's state lives in
//! `TacticalBattle` and the world, and a snapshot kept beside them is a
//! second truth to hold in step.
//!
//! **Not `EntityView`.** The map's row carries thirty-odd fields about
//! structures, build sites, postings and haul marks, none of which a battle
//! map draws; the shared drawing rules it feeds — `ConRead::of`,
//! `hud::palette::glyph`, `Painter::sprite` — all take discrete values
//! rather than the row itself, so a body's view can be exactly what a
//! battle map draws and every rule stays a call.

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::components::{
    Creature, Experience, Glyph, GlyphColor, Hostile, Player, PlayerIdentity, Rarity, Stats,
};
use crate::game::inspection::difficulty_color;
use crate::species::SpeciesDb;
use crate::tactical::TacticalBattle;
use crate::tactical::map::Board;
use crate::tactical::reach;
use crate::views::PlayerLook;

/// One body standing on the battle map.
///
/// Every field is something the grid draws. `sprite`, `color`,
/// `difficulty`, `is_boss` and `look` are exactly the arguments the surface
/// map's own tile draw hands its shared helpers, so the two grids cannot
/// disagree about what a program looks like.
#[derive(Clone, Debug)]
pub struct TacticalBody {
    pub entity: Entity,
    /// Where it stands on the board — a battle coordinate, never a world
    /// `Position`.
    pub cell: (i32, i32),
    pub glyph: char,
    /// The resolved sprite name, `EntityView::sprite`'s rule: the override
    /// when authored, the species id otherwise, `None` for a body with no
    /// species at all.
    pub sprite: Option<String>,
    /// The authored hue — what this program *is*.
    pub color: GlyphColor,
    /// How badly it would beat the player, `None` for anything not hostile
    /// so the con read cannot draw under a companion.
    pub difficulty: Option<GlyphColor>,
    pub label: String,
    pub is_player: bool,
    /// `Some` exactly when `is_player`.
    pub look: Option<PlayerLook>,
    pub is_hostile: bool,
    pub is_boss: bool,
    pub rarity: Rarity,
    pub hp_fraction: Option<f32>,
    pub level: Option<u32>,
}

/// One rung of the turn-order strip.
#[derive(Clone, Debug)]
pub struct TurnRow {
    pub entity: Entity,
    pub glyph: char,
    pub color: GlyphColor,
    pub label: String,
    pub is_hostile: bool,
    pub hp_fraction: Option<f32>,
}

/// A tactical fight, as a screen needs it.
#[derive(Clone, Debug)]
pub struct TacticalView {
    /// The board itself rather than a copied grid — `Board` is the one
    /// definition of what a cell is, and `cell()` already answers off the
    /// board for a cursor that has wandered.
    pub board: Board,
    pub bodies: Vec<TacticalBody>,
    /// Fastest first, and in the order the fight settled at the bell.
    pub order: Vec<TurnRow>,
    /// Which rung of `order` is acting, `None` once the board is empty.
    pub active: Option<usize>,
    /// Whether the acting body is the player's to move — **every party body
    /// is**, companions included, so this is `Game::tactical_awaits_input`
    /// and not "is the player acting".
    pub player_turn: bool,
    /// Steps the acting body has left, already net of what it has spent.
    pub allowance: u32,
    /// Whether the acting body has spent its action.
    pub acted: bool,
    pub round: u32,
    /// Every cell the acting body could still reach, its own included.
    pub reachable: Vec<(i32, i32)>,
}

impl Game {
    /// Whether a tactical fight is open.
    ///
    /// The router's counterpart to `Game::in_battle`, and what app-core
    /// reads to know which of the two screens a fight just opened.
    pub fn in_tactical_battle(&self) -> bool {
        self.world.get_resource::<TacticalBattle>().is_some()
    }

    /// The open tactical fight, or `None`.
    pub fn tactical_view(&mut self) -> Option<TacticalView> {
        self.world.get_resource::<TacticalBattle>()?;
        let player_power = self.player_power();
        // **The door and not a third predicate.** `tactical_ai_actor` gates
        // on `Hostile` because every party body is the player's to command,
        // and `tactical_awaits_input` is its complement — asked here as
        // `actor == player` instead, a companion's turn drew the keybar's
        // "the wild side is moving" and no reach wash while app-core, which
        // reads the real door, sat waiting for a key.
        let awaits_input = self.tactical_awaits_input();
        let battle = self.world.resource::<TacticalBattle>();
        let board = battle.board.clone();
        let round = battle.round;
        let acted = battle.acted();
        let spent = battle.spent();
        let actor = battle.actor();
        let placed: Vec<(Entity, (i32, i32))> = battle.bodies().collect();
        let initiative: Vec<Entity> = battle.initiative().to_vec();

        let bodies: Vec<TacticalBody> = placed
            .iter()
            .map(|&(entity, cell)| self.body_view(entity, cell, player_power))
            .collect();
        let order: Vec<TurnRow> = initiative
            .iter()
            .map(|&entity| self.turn_row(entity))
            .collect();
        let active = actor.and_then(|a| initiative.iter().position(|&e| e == a));

        // Net of what has been spent, so a screen never has to subtract —
        // and saturating, because a `Rough` cell may cost more than the step
        // that entered it had left.
        let allowance = actor
            .map(|a| self.movement_allowance(a).saturating_sub(spent))
            .unwrap_or(0);
        let reachable = actor
            .map(|a| {
                let battle = self.world.resource::<TacticalBattle>();
                reach::movement_field(battle, a, allowance)
                    .into_keys()
                    .collect()
            })
            .unwrap_or_default();

        Some(TacticalView {
            board,
            bodies,
            order,
            active,
            player_turn: awaits_input,
            allowance,
            acted,
            round,
            reachable,
        })
    }

    /// Who is standing on a battle-map cell, if anybody.
    pub fn tactical_occupant(&self, cell: (i32, i32)) -> Option<Entity> {
        self.world.get_resource::<TacticalBattle>()?.occupant(cell)
    }

    /// Which cells a routine would cover, aimed where it is aimed.
    ///
    /// **A call into the geometry that resolves it**, never a second
    /// derivation: `reach::recipients` covers a body when `shape_cells`
    /// covers its cell, so a previewed blast and a delivered one cannot
    /// differ. `index` indexes `actor_abilities`, exactly as
    /// `tactical_use_routine` does.
    pub fn tactical_shape_cells(&mut self, index: usize, aim: (i32, i32)) -> Vec<(i32, i32)> {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return Vec::new();
        };
        let Some(actor) = battle.actor() else {
            return Vec::new();
        };
        let Some(from) = battle.cell_of(actor) else {
            return Vec::new();
        };
        let Some(ability) = self.actor_abilities(actor).into_iter().nth(index) else {
            return Vec::new();
        };
        let battle = self.world.resource::<TacticalBattle>();
        reach::shape_cells(&battle.board, from, aim, ability.tactical_shape())
    }

    fn body_view(&self, entity: Entity, cell: (i32, i32), player_power: i32) -> TacticalBody {
        let glyph = self.world.get::<Glyph>(entity).copied();
        let stats = self.world.get::<Stats>(entity);
        let is_player = self.world.get::<Player>(entity).is_some();
        let is_hostile = self.world.get::<Hostile>(entity).is_some();
        TacticalBody {
            entity,
            cell,
            glyph: glyph.map(|g| g.ch).unwrap_or('?'),
            sprite: self
                .world
                .get::<Creature>(entity)
                .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
                .map(|def| def.sprite_name().to_string()),
            color: glyph.map(|g| g.color).unwrap_or(GlyphColor::White),
            difficulty: is_hostile
                .then(|| stats.map(|s| difficulty_color(s.power(), player_power)))
                .flatten(),
            label: self.entity_label(entity),
            is_player,
            look: is_player.then(|| {
                let identity = self.world.get::<PlayerIdentity>(entity);
                PlayerLook {
                    sprite: identity.map(|i| i.sprite.clone()).unwrap_or_default(),
                    colour: identity.and_then(|i| i.colour),
                    icon: identity.and_then(|i| i.icon.clone()),
                }
            }),
            is_hostile,
            is_boss: self.is_boss_creature(entity),
            rarity: self.rarity_of(entity),
            hp_fraction: stats.map(|s| s.hp_fraction()),
            level: self.world.get::<Experience>(entity).map(|e| e.level),
        }
    }

    fn turn_row(&self, entity: Entity) -> TurnRow {
        let glyph = self.world.get::<Glyph>(entity).copied();
        TurnRow {
            entity,
            glyph: glyph.map(|g| g.ch).unwrap_or('?'),
            color: glyph.map(|g| g.color).unwrap_or(GlyphColor::White),
            label: self.entity_label(entity),
            is_hostile: self.world.get::<Hostile>(entity).is_some(),
            hp_fraction: self.world.get::<Stats>(entity).map(|s| s.hp_fraction()),
        }
    }
}
