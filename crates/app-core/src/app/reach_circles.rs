//! The Shield and Patch Node circles the map draws: under the pointer, and
//! as a preview while the player picks where to deploy or which node to
//! upgrade.

use crate::*;

/// How a circle is drawn: the solid hover look, or a faint preview edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReachCircleStyle {
    Hover,
    Preview,
}

/// Which structure a circle belongs to; the renderer colours by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReachKind {
    Shield,
    Patch,
}

/// One circle for the pane being drawn, in that pane's own coordinates:
/// board cells in `Mode::TacticalBattle`, field tiles otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReachCircle {
    pub anchor: (i32, i32),
    pub radius: u32,
    pub style: ReachCircleStyle,
    pub kind: ReachKind,
}

const NEIGHBOURS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

impl App {
    /// The circles to draw this frame.
    ///
    /// A Shield or Patch Node is one tile, so "the pointer is on its
    /// footprint" is the pointer on its anchor. An unarmed Shield still shows
    /// its circle: the player should see what it would cover.
    pub fn reach_circles(&mut self) -> Vec<ReachCircle> {
        let hover = self.map_hover;
        match self.mode {
            Mode::Playing => self.hover_circle(hover, (0, 0)),
            Mode::TacticalBattle => {
                let Some(view) = self.game.as_mut().and_then(|g| g.tactical_view()) else {
                    return Vec::new();
                };
                if !view.siege {
                    return Vec::new();
                }
                self.hover_circle(hover, view.siege_origin)
            }
            Mode::BuildDirection => self.deploy_previews(),
            Mode::UpgradeDirection => self.upgrade_previews(),
            _ => Vec::new(),
        }
    }

    /// The hover circle for the Shield or Patch Node standing on `hover`, shifted by
    /// `origin` (the siege board's offset onto the field) in and back out.
    fn hover_circle(&mut self, hover: Option<(i32, i32)>, origin: (i32, i32)) -> Vec<ReachCircle> {
        let (Some(cell), Some(game)) = (hover, self.game.as_mut()) else {
            return Vec::new();
        };
        if game.is_underground() {
            return Vec::new();
        }
        let field = (cell.0 + origin.0, cell.1 + origin.1);
        let shields = game
            .view_shields()
            .into_iter()
            .map(|s| (s.anchor, s.radius, ReachKind::Shield));
        let patches = game
            .view_patch_nodes()
            .into_iter()
            .map(|n| (n.anchor, n.radius, ReachKind::Patch));
        shields
            .chain(patches)
            .filter(|&(anchor, ..)| anchor == field)
            .map(|(_, radius, kind)| ReachCircle {
                anchor: cell,
                radius,
                style: ReachCircleStyle::Hover,
                kind,
            })
            .collect()
    }

    fn deploy_previews(&mut self) -> Vec<ReachCircle> {
        let (Some(kind), Some(game)) = (self.pending_structure.as_deref(), self.game.as_ref())
        else {
            return Vec::new();
        };
        let (Some((radius, kind)), Some((px, py))) = (
            game.shield_radius_for(kind, 1)
                .map(|r| (r, ReachKind::Shield))
                .or_else(|| {
                    game.patch_radius_for(kind, 1)
                        .map(|r| (r, ReachKind::Patch))
                }),
            game.field_pos(),
        ) else {
            return Vec::new();
        };
        NEIGHBOURS
            .iter()
            .map(|&(dx, dy)| ReachCircle {
                anchor: (px + dx, py + dy),
                radius,
                style: ReachCircleStyle::Preview,
                kind,
            })
            .collect()
    }

    fn upgrade_previews(&mut self) -> Vec<ReachCircle> {
        let Some(game) = self.game.as_mut() else {
            return Vec::new();
        };
        let adjacent: Vec<_> = NEIGHBOURS
            .iter()
            .filter_map(|&(dx, dy)| game.adjacent_structure(dx, dy))
            .map(|e| e.entity)
            .collect();
        let shields = game
            .view_shields()
            .into_iter()
            .map(|s| (s.entity, s.anchor, s.next_radius, ReachKind::Shield));
        let patches = game
            .view_patch_nodes()
            .into_iter()
            .map(|n| (n.entity, n.anchor, n.next_radius, ReachKind::Patch));
        shields
            .chain(patches)
            .filter(|(entity, ..)| adjacent.contains(entity))
            .filter_map(|(_, anchor, next, kind)| {
                Some(ReachCircle {
                    anchor,
                    radius: next?,
                    style: ReachCircleStyle::Preview,
                    kind,
                })
            })
            .collect()
    }
}
