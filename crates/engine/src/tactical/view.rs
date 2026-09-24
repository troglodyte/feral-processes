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
use crate::abilities::{AbilityShape, TamperKind};
use crate::components::{
    Creature, Experience, Glyph, GlyphColor, Hostile, Player, PlayerIdentity, Rarity, Squad, Stats,
    Tampered,
};
use crate::game::inspection::difficulty_color;
use crate::species::SpeciesDb;
use crate::tactical::TacticalBattle;
use crate::tactical::ai::ForecastAction;
use crate::tactical::map::Board;
use crate::tactical::reach;
use crate::tuning::FORMATIONS;
use crate::views::{FormLook, PlayerLook};

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
    /// What this body draws instead of `look`/`glyph` while emulating —
    /// `EntityView::form`'s twin, `Game::form_look`'s other reader.
    pub form: Option<FormLook>,
    pub is_hostile: bool,
    pub is_boss: bool,
    pub rarity: Rarity,
    pub hp_fraction: Option<f32>,
    pub level: Option<u32>,
    /// Whether no picker may name this body — see `components::Cloaked`.
    ///
    /// **Nothing is hidden from the player's view.** A cloaked body still
    /// appears here and in the turn strip, whichever side it is on; the
    /// renderer fades it. Omitting a hostile from `bodies` so no renderer
    /// can leak it is the presentation half of "a body that approaches
    /// unseen", and is deliberately not built yet.
    pub cloaked: bool,
    /// Whether this body has partial cover against the body whose turn it
    /// is, and then only while that body is hostile.
    ///
    /// **Off on the player's own turn**, where the aim has not been chosen
    /// yet: app-core turns the mark on for the hostile being aimed at
    /// through `Game::body_in_cover` instead. Off on a finished board too —
    /// see `TacticalView::frozen`.
    pub in_cover: bool,
    /// The side, in cells, of the square footprint this body occupies —
    /// `TacticalBattle::footprint_of`'s own answer, `1` for every body
    /// without a `Squad`. What `draw_body` scales its whole draw to, not
    /// just the mark: the glyph or sprite, the rarity bar, the con earmark,
    /// the cover mark and the HP bar all span the footprint rather than one
    /// cell.
    pub footprint: u8,
    /// `Some` exactly when this body is a folded `Squad`.
    ///
    /// **The name is not carried here.** `TacticalBody::label` already has
    /// it — `Game::entity_label`'s own `Squad` arm builds `"<species> squad
    /// (5)"` once, in the engine, so the turn strip and an examine line
    /// reading the same field cannot show two different counts as a
    /// capture thins the squad. This carries only what a draw needs and
    /// the stat block does not already say: the mark drawn in the
    /// footprint's own corner and the member count for anyone drawing the
    /// HP bar's own reading beside it.
    pub squad: Option<SquadView>,
}

/// What a folded squad's body draws that a single one does not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SquadView {
    /// How many bodies are still folded into this one — `Squad::members`'s
    /// own length, read live so a capture's count is never stale.
    pub members: usize,
    /// The mark drawn in the footprint's corner nothing else claims —
    /// `tuning::Formation::mark`.
    pub mark: char,
    /// The noun `Game::entity_label`'s own `Squad` arm built its name
    /// from — `tuning::Formation::noun`.
    pub noun: &'static str,
}

/// A `Tampered` slot's own tag, in the strip's short vocabulary — see the
/// design doc's "What the player sees" (`HOT`, `COLD`, `PROF`, `INJ`,
/// `HALL`).
///
/// **`Temperature` splits in two and the other three don't**, because
/// `Temperature` is the one kind whose value can cross the line
/// `TamperKind::runs_cold` draws — `Profiled`, `Injected` and
/// `Hallucinating` are each a single fixed rule, not a number the strip
/// would otherwise have to read the units of. The threshold itself is that
/// method's, shared with the take-hold log line rather than restated here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TamperTag {
    Hot,
    Cold,
    Profiled,
    Injected,
    Hallucinating,
}

impl TamperTag {
    fn of(kind: TamperKind) -> Self {
        match kind {
            TamperKind::Temperature(_) if kind.runs_cold() => Self::Cold,
            TamperKind::Temperature(_) => Self::Hot,
            TamperKind::Profiled => Self::Profiled,
            TamperKind::Injected => Self::Injected,
            TamperKind::Hallucinating { .. } => Self::Hallucinating,
        }
    }
}

/// A `Profiled` hostile's published intent — `Game::tactical_forecast`'s own
/// doc for why this is never cached: it is built fresh every time the view
/// is, so a cooldown ticked by round upkeep between now and the body's turn
/// cannot leave a stale forecast on the strip.
#[derive(Clone, Debug, PartialEq)]
pub struct ForecastView {
    /// The routine's display name, or `"swing"` — resolved here, so no
    /// renderer holds an `AbilityId` it would have to look up itself.
    pub action: String,
    pub walk: Vec<(i32, i32)>,
    pub target: Option<(i32, i32)>,
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
    /// Every `Tampered` slot this body carries, in `TamperSlot` order.
    pub tags: Vec<TamperTag>,
    /// `Some` only for a `Profiled` hostile that has not started its turn —
    /// see `Game::tactical_forecast`.
    pub forecast: Option<ForecastView>,
    /// Whether `Game::taken_over` reads this body as the AI's rather than
    /// the player's for as long as its entry lasts.
    pub taken_over: bool,
}

/// A Hallucination's fake, as a screen needs it — `tactical::Decoy` less
/// `owner_hostile`, which says which side a body has to be on to see the
/// decoy and so answers nothing a renderer asks. `of_player` is `Decoy`'s
/// own field and carries over unchanged: it is what says the fake wears the
/// `PLAYER` role rather than the hue the player merely spawned with, the
/// same reduction `TacticalBody` already makes for a real body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecoyView {
    pub cell: (i32, i32),
    pub glyph: char,
    pub color: GlyphColor,
    pub of_player: bool,
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
    /// Whether the acting body has no actions left to spend this turn —
    /// `actions_left() == 0`, always true after the one action a body
    /// without a `Squad` gets.
    pub acted: bool,
    pub round: u32,
    /// Every cell the acting body could still reach, its own included.
    pub reachable: Vec<(i32, i32)>,
    /// The subset of `reachable` the acting body cannot walk to without
    /// being swung at on the way — `Game::walk_risk`'s own answer, so the
    /// tint and the AI's scoring cannot come to disagree about which cells
    /// cost something.
    pub provoking: Vec<(i32, i32)>,
    /// Every Hallucination fake on the board, both sides'.
    pub decoys: Vec<DecoyView>,
    /// The subset of `reachable` that has cover against at least one hostile
    /// the acting body can see — built the way `provoking` is, one call per
    /// cell into the rule the roll reads.
    pub covered: Vec<(i32, i32)>,
}

impl TacticalView {
    /// The board with nothing left to act: no actor, no reach and no turn
    /// waiting on a key, so a finished fight's board draws no turn arrow
    /// and no reach wash offering a move there is no fight left to spend.
    ///
    /// **Every AI-only field on a rung goes with it, and so do the
    /// decoys.** A closing roster is a *result* screen, not a resumed
    /// fight: a `Profiled` hostile's forecast, a hijacked companion's mark
    /// and a Hallucination's fakes are all things the AI would still be
    /// acting on, and none of them survive to a board with no actor left.
    pub fn frozen(self) -> Self {
        let bodies = self
            .bodies
            .into_iter()
            .map(|body| TacticalBody {
                in_cover: false,
                ..body
            })
            .collect();
        let order = self
            .order
            .into_iter()
            .map(|row| TurnRow {
                tags: Vec::new(),
                forecast: None,
                taken_over: false,
                ..row
            })
            .collect();
        Self {
            active: None,
            player_turn: false,
            allowance: 0,
            reachable: Vec::new(),
            provoking: Vec::new(),
            covered: Vec::new(),
            bodies,
            order,
            decoys: Vec::new(),
            ..self
        }
    }
}

impl Game {
    /// The board the last fight ended on, when it was a tactical one — what
    /// a finished tactical fight's results are drawn over, `TacticalBattle`
    /// being gone. See `ClosingRoster::board`.
    pub fn tactical_result_view(&self) -> Option<TacticalView> {
        self.world
            .resource::<crate::resources::BattleTimeline>()
            .closing
            .as_ref()?
            .board
            .clone()
    }

    /// Whether `defender` has cover against a shot from `attacker`.
    ///
    /// **The door app-core lights the aim mark through**, while the player
    /// is aiming at a particular hostile and the view's own `in_cover` is
    /// off. It agrees with the roll by construction: this and
    /// `defender_profile_against` both call `reach::cover_between`, and
    /// neither reimplements the arc.
    pub(crate) fn body_in_cover(&self, attacker: Entity, defender: Entity) -> bool {
        self.world
            .get_resource::<TacticalBattle>()
            .and_then(|battle| {
                let from = battle.cell_of(attacker)?;
                // `defender_profile_against`'s own conversion: the nearest of
                // the defender's footprint cells to the attacker, one cell
                // today without a `Squad`.
                let at = reach::nearest_cell(&battle.cells_of(defender), from)?;
                Some(reach::cover_between(&battle.board, from, at))
            })
            .unwrap_or(false)
    }

    /// Whether a tactical fight is open.
    ///
    /// The router's counterpart to `Game::in_battle`, and what app-core
    /// reads to know which of the two screens a fight just opened.
    pub fn in_tactical_battle(&self) -> bool {
        self.world.get_resource::<TacticalBattle>().is_some()
    }

    /// The open tactical fight's round, or `None` — what the renderer
    /// watches once a frame to mark a wrap, without `tactical_view`'s clone
    /// of the whole board.
    pub fn tactical_round(&self) -> Option<u32> {
        self.world.get_resource::<TacticalBattle>().map(|b| b.round)
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
        let acted = battle.actions_left() == 0;
        let spent = battle.spent();
        let actor = battle.actor();
        let placed: Vec<(Entity, (i32, i32))> = battle.bodies().collect();
        let initiative: Vec<Entity> = battle.initiative().to_vec();
        let decoys: Vec<DecoyView> = battle
            .decoys()
            .iter()
            .map(|d| DecoyView {
                cell: d.cell,
                glyph: d.glyph,
                color: d.color,
                of_player: d.of_player,
            })
            .collect();

        // Only a hostile's turn earns the standing mark. On the player's own
        // turn nothing has been aimed yet, and app-core lights the aimed
        // hostile through `body_in_cover` instead.
        let marks_cover_against = actor.filter(|&a| self.is_hostile(a));
        let bodies: Vec<TacticalBody> = placed
            .iter()
            .map(|&(entity, cell)| self.body_view(entity, cell, player_power, marks_cover_against))
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
        let reachable: Vec<(i32, i32)> = actor
            .map(|a| {
                let battle = self.world.resource::<TacticalBattle>();
                reach::movement_field(battle, a, allowance)
                    .into_keys()
                    .collect()
            })
            .unwrap_or_default();
        // A *call* into the term the walk scoring reads, never a second
        // reading of the geometry: what the player is shown and what a
        // hostile prices are the same figure.
        let provoking: Vec<(i32, i32)> = actor
            .map(|a| {
                reachable
                    .iter()
                    .copied()
                    .filter(|&cell| self.walk_risk(a, cell) > 0.0)
                    .collect()
            })
            .unwrap_or_default();

        // `provoking`'s pattern: one call per cell into the one rule, so
        // the wash and the roll cannot disagree about which cells shelter.
        let covered: Vec<(i32, i32)> = actor
            .map(|a| {
                let hostiles: Vec<(i32, i32)> = placed
                    .iter()
                    .filter(|&&(e, _)| e != a && self.is_hostile(e) != self.is_hostile(a))
                    .map(|&(_, cell)| cell)
                    .collect();
                reachable
                    .iter()
                    .copied()
                    .filter(|&cell| {
                        hostiles
                            .iter()
                            .any(|&from| reach::cover_between(&board, from, cell))
                    })
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
            provoking,
            decoys,
            covered,
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

    /// Where a `Radius` routine's centre may legally be placed, aimed from
    /// the acting body — every cell in range that the actor can also see.
    ///
    /// **A call into the same two doors `tactical_use_routine`'s refusal
    /// reads**, `reach::in_range` then `reach::aim_in_sight`, never a second
    /// derivation of what "legal to aim at" means — so the outline this
    /// feeds and the refusal a click past its edge gets cannot disagree.
    /// `index` indexes `actor_abilities`, `tactical_shape_cells`'s rule.
    ///
    /// Empty for every shape but `Radius`. A `Single` is thrown at a cell
    /// too, but it resolves onto whoever is standing there rather than a
    /// centre the player chooses freely, so there is nothing to outline that
    /// a highlighted occupant would not already show; a `Line` or a `Cone`
    /// is aimed as a *direction* and has no "legal centre" at all —
    /// `aim_in_sight`'s own rule, restated by the gate below rather than
    /// left to fall out of a range check that would pass every cell on the
    /// board.
    pub fn tactical_placeable_cells(&mut self, index: usize) -> Vec<(i32, i32)> {
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
        let shape = ability.tactical_shape();
        if !matches!(shape, AbilityShape::Radius { .. }) {
            return Vec::new();
        }
        let range = ability.tactical_range();
        let battle = self.world.resource::<TacticalBattle>();
        let board = &battle.board;
        let actor_cells = battle.cells_of(actor);
        board
            .cells()
            .filter_map(|(cell, _)| {
                (reach::in_range(&actor_cells, cell, range)
                    && reach::aim_in_sight(board, from, cell, shape))
                .then_some(cell)
            })
            .collect()
    }

    fn body_view(
        &self,
        entity: Entity,
        cell: (i32, i32),
        player_power: i32,
        attacker: Option<Entity>,
    ) -> TacticalBody {
        let glyph = self.world.get::<Glyph>(entity).copied();
        let stats = self.world.get::<Stats>(entity);
        let is_player = self.world.get::<Player>(entity).is_some();
        let is_hostile = self.world.get::<Hostile>(entity).is_some();
        let footprint = self
            .world
            .get_resource::<TacticalBattle>()
            .map(|battle| battle.footprint_of(entity))
            .unwrap_or(1);
        let squad = self.world.get::<Squad>(entity).map(|squad| {
            let formation = FORMATIONS.get(squad.formation);
            SquadView {
                members: squad.members.len(),
                mark: formation.map(|f| f.mark).unwrap_or('^'),
                noun: formation.map(|f| f.noun).unwrap_or("squad"),
            }
        });
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
            form: self.form_look(entity),
            is_hostile,
            is_boss: self.is_boss_creature(entity),
            rarity: self.rarity_of(entity),
            hp_fraction: stats.map(|s| s.hp_fraction()),
            level: self.world.get::<Experience>(entity).map(|e| e.level),
            cloaked: self.is_cloaked(entity),
            in_cover: attacker.is_some_and(|a| self.body_in_cover(a, entity)),
            footprint,
            squad,
        }
    }

    fn turn_row(&self, entity: Entity) -> TurnRow {
        let glyph = self.world.get::<Glyph>(entity).copied();
        let tags = self
            .world
            .get::<Tampered>(entity)
            .map(|tampered| {
                tampered
                    .slots()
                    .map(|(_, kind)| TamperTag::of(kind))
                    .collect()
            })
            .unwrap_or_default();
        // Built live, never cached: round upkeep ticks cooldowns between now
        // and this body's turn, and a forecast read once at battle start
        // would go stale the moment anything changed.
        let forecast = self.tactical_forecast(entity).map(|forecast| ForecastView {
            action: match forecast.action {
                ForecastAction::Swing => "swing".to_string(),
                ForecastAction::Routine(id) => self.ability_display_name(&id),
            },
            walk: forecast.walk,
            target: forecast.target,
        });
        TurnRow {
            entity,
            glyph: glyph.map(|g| g.ch).unwrap_or('?'),
            color: glyph.map(|g| g.color).unwrap_or(GlyphColor::White),
            label: self.entity_label(entity),
            is_hostile: self.world.get::<Hostile>(entity).is_some(),
            hp_fraction: self.world.get::<Stats>(entity).map(|s| s.hp_fraction()),
            tags,
            forecast,
            taken_over: self.taken_over(entity),
        }
    }
}
