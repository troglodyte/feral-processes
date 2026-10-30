//! The Shield circles the map draws: under the pointer, and as a preview
//! while the player picks where to deploy or which Shield to upgrade.

use crate::*;

/// How a circle is drawn: the solid hover look, or a faint preview edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShieldCircleStyle {
    Hover,
    Preview,
}

/// One circle for the pane being drawn, in that pane's own coordinates:
/// board cells in `Mode::TacticalBattle`, field tiles otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShieldCircle {
    pub anchor: (i32, i32),
    pub radius: u32,
    pub style: ShieldCircleStyle,
}

const NEIGHBOURS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

impl App {
    /// The circles to draw this frame.
    ///
    /// A Shield is one tile, so "the pointer is on its footprint" is the
    /// pointer on its anchor. An unarmed Shield still shows its circle:
    /// the player should see what it would cover.
    pub fn shield_circles(&mut self) -> Vec<ShieldCircle> {
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

    /// The hover circle for the Shield standing on `hover`, shifted by
    /// `origin` (the siege board's offset onto the field) in and back out.
    fn hover_circle(&mut self, hover: Option<(i32, i32)>, origin: (i32, i32)) -> Vec<ShieldCircle> {
        let (Some(cell), Some(game)) = (hover, self.game.as_mut()) else {
            return Vec::new();
        };
        if game.is_underground() {
            return Vec::new();
        }
        let field = (cell.0 + origin.0, cell.1 + origin.1);
        game.view_shields()
            .into_iter()
            .filter(|s| s.anchor == field)
            .map(|s| ShieldCircle {
                anchor: cell,
                radius: s.radius,
                style: ShieldCircleStyle::Hover,
            })
            .collect()
    }

    fn deploy_previews(&mut self) -> Vec<ShieldCircle> {
        let (Some(kind), Some(game)) = (self.pending_structure.as_deref(), self.game.as_ref())
        else {
            return Vec::new();
        };
        let (Some(radius), Some((px, py))) = (game.shield_radius_for(kind, 1), game.field_pos())
        else {
            return Vec::new();
        };
        NEIGHBOURS
            .iter()
            .map(|&(dx, dy)| ShieldCircle {
                anchor: (px + dx, py + dy),
                radius,
                style: ShieldCircleStyle::Preview,
            })
            .collect()
    }

    fn upgrade_previews(&mut self) -> Vec<ShieldCircle> {
        let Some(game) = self.game.as_mut() else {
            return Vec::new();
        };
        let adjacent: Vec<_> = NEIGHBOURS
            .iter()
            .filter_map(|&(dx, dy)| game.adjacent_structure(dx, dy))
            .map(|e| e.entity)
            .collect();
        game.view_shields()
            .into_iter()
            .filter(|s| adjacent.contains(&s.entity))
            .filter_map(|s| {
                Some(ShieldCircle {
                    anchor: s.anchor,
                    radius: s.next_radius?,
                    style: ShieldCircleStyle::Preview,
                })
            })
            .collect()
    }
}
