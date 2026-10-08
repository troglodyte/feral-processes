//! The world map's stored state: the fog the party's own walking writes.
//! `resources::ExploredChunks` holds only where the party has been; the base,
//! outposts and route corridors are derived by the view and never stored.

use crate::components::{Position, SurfaceLink};
use crate::game::inspection::link_compass_label;
use crate::resources::{
    ExploredChunks, Locale, Outposts, Routes, Settlements, StackMemory, Standings,
};
use crate::routes::corridor_chunks;
use crate::settlements::growth::{self, outlook, trend};
use crate::settlements::{CompassTarget, SettlementKind};
use crate::tuning::WORLD_MAP_REVEAL_RADIUS_CHUNKS;
use crate::views::{WorldMapCell, WorldMapMark, WorldMapMarkKind, WorldMapRoute, WorldMapView};
use crate::world::{CHUNK_SIZE, WorldMap};
use crate::*;
use std::collections::BTreeSet;

fn chunk_of((x, y): (i32, i32)) -> (i32, i32) {
    (x.div_euclid(CHUNK_SIZE), y.div_euclid(CHUNK_SIZE))
}

fn insert_around(set: &mut BTreeSet<(i32, i32)>, (cx, cy): (i32, i32)) {
    let r = WORLD_MAP_REVEAL_RADIUS_CHUNKS;
    for y in (cy - r)..=(cy + r) {
        for x in (cx - r)..=(cx + r) {
            set.insert((x, y));
        }
    }
}

impl Game {
    /// Reveals the chunks around the party. Called once from `tick_inner`,
    /// the one place every writer of `Position` (walking, recall, the
    /// footprint eject) passes through.
    ///
    /// **Guarded on `Locale::Surface`**, unlike the neighbouring stocking
    /// calls: a Stack frame or base space has its own coordinates, and
    /// marking them would reveal surface chunks the party never stood near.
    pub(crate) fn mark_explored_chunks(&mut self) {
        if !matches!(*self.world.resource::<Locale>(), Locale::Surface) {
            return;
        }
        let pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        let (px, py) = (pos.x.div_euclid(CHUNK_SIZE), pos.y.div_euclid(CHUNK_SIZE));
        let r = WORLD_MAP_REVEAL_RADIUS_CHUNKS;
        let mut explored = self.world.resource_mut::<ExploredChunks>();
        for cy in (py - r)..=(py + r) {
            for cx in (px - r)..=(px + r) {
                explored.0.insert((cx, cy));
            }
        }
    }

    /// The surface at chunk scale, `radius` chunks either side of
    /// `center_chunk`, or `None` unless the party is on the surface.
    ///
    /// `&mut self` only because outpost crew, nests and links are
    /// query-backed; it writes nothing, and settles no commerce (pending
    /// drift is `growth::settle_commerce` read, not applied).
    ///
    /// A chunk is revealed when the party walked near it (`ExploredChunks`)
    /// or when it is derived here and never stored: around the base anchor,
    /// around each outpost, and along each route's corridor. A mark appears
    /// only on a revealed chunk.
    pub fn world_map(&mut self, center_chunk: (i32, i32), radius: i32) -> Option<WorldMapView> {
        self.require_surface().ok()?;
        let party_pos = *self.world.get::<Position>(self.player_entity())?;
        let party_tile = (party_pos.x, party_pos.y);
        let anchor = self.anchor_position();

        let outpost_tiles: Vec<(i32, i32)> = self
            .world
            .resource::<Outposts>()
            .0
            .keys()
            .copied()
            .collect();
        let route_ends: Vec<((i32, i32), (i32, i32))> = match anchor {
            Some(anchor) => self
                .world
                .resource::<Routes>()
                .0
                .iter()
                .map(|r| (anchor, r.destination.tile()))
                .collect(),
            None => Vec::new(),
        };

        let mut revealed = self.world.resource::<ExploredChunks>().0.clone();
        if let Some(anchor) = anchor {
            insert_around(&mut revealed, chunk_of(anchor));
        }
        for &tile in &outpost_tiles {
            insert_around(&mut revealed, chunk_of(tile));
        }
        for &(from, to) in &route_ends {
            revealed.extend(corridor_chunks(chunk_of(from), chunk_of(to)));
        }

        let (left, top) = (center_chunk.0 - radius, center_chunk.1 - radius);
        let side = 2 * radius + 1;
        let cells = {
            let map = self.world.resource::<WorldMap>();
            (0..side)
                .map(|row| {
                    (0..side)
                        .map(|col| {
                            let chunk = (left + col, top + row);
                            if revealed.contains(&chunk) {
                                WorldMapCell::Explored(map.biome_at(
                                    chunk.0 * CHUNK_SIZE + CHUNK_SIZE / 2,
                                    chunk.1 * CHUNK_SIZE + CHUNK_SIZE / 2,
                                ))
                            } else {
                                WorldMapCell::Unknown
                            }
                        })
                        .collect()
                })
                .collect()
        };

        let in_view = |chunk: (i32, i32)| {
            revealed.contains(&chunk)
                && (left..left + side).contains(&chunk.0)
                && (top..top + side).contains(&chunk.1)
        };
        let mut marks: Vec<((i32, i32), WorldMapMark)> = Vec::new();
        let mut push = |tile: (i32, i32), kind, label: String, target| {
            let chunk = chunk_of(tile);
            if in_view(chunk) {
                marks.push((
                    tile,
                    WorldMapMark {
                        chunk,
                        kind,
                        label,
                        target,
                    },
                ));
            }
        };

        if let Some(anchor) = anchor {
            push(
                anchor,
                WorldMapMarkKind::Home,
                "home".to_string(),
                Some(CompassTarget::Home),
            );
        }

        let known: Vec<_> = self
            .world
            .resource::<Settlements>()
            .0
            .iter()
            .map(|(key, town)| (*key, town.tile, town.compass_label()))
            .collect();
        let seed = self.world.resource::<WorldMap>().seed();
        let now = self.current_tick();
        let epoch = now / crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS;
        for (key, tile, label) in known {
            if !in_view(chunk_of(tile)) {
                continue;
            }
            let Some(kind) = self.settlement_kind(key) else {
                continue;
            };
            let standing = self.standing_band(key);
            let relation = self
                .world
                .resource::<Standings>()
                .0
                .get(&key)
                .copied()
                .unwrap_or_default();
            let settled = growth::settle_commerce(
                &relation,
                epoch,
                standing == crate::settlements::Standing::Hostile,
            );
            let mainframe = kind == SettlementKind::Mainframe;
            let vitality = self.settlement_vitality(key).map(|_| {
                growth::vitality(settled.commerce)
                    .max(growth::vitality_floor(relation.traded, standing))
            });
            push(
                tile,
                WorldMapMarkKind::Town {
                    standing,
                    kind,
                    vitality,
                    trend: mainframe.then(|| trend(settled.commerce, settled.commerce_at_epoch)),
                    outlook: (!mainframe)
                        .then(|| outlook(now, growth::due_with_pull(seed, key, settled.commerce))),
                    sends_raiders: standing.sends_raiders(),
                    fields_patrols: standing.fields_patrols(),
                    preys_on_routes: standing.preys_on_routes(),
                    refuses_service: standing.refuses_service(),
                    allows_standing_route: standing.allows_standing_route(),
                },
                label,
                Some(CompassTarget::Town(key)),
            );
        }

        for mark in self.outpost_marks() {
            let label = self.outpost_destination_name(mark.tile);
            push(
                mark.tile,
                WorldMapMarkKind::Outpost {
                    trend: mark.trend,
                    dark: mark.dark,
                },
                label,
                Some(CompassTarget::Outpost(mark.tile)),
            );
        }

        let walked: BTreeSet<(i32, i32)> = self
            .world
            .resource::<StackMemory>()
            .0
            .keys()
            .map(|k| k.0)
            .collect();
        let links: Vec<(i32, i32)> = {
            let mut q = self.world.query_filtered::<&Position, With<SurfaceLink>>();
            q.iter(&self.world).map(|p| (p.x, p.y)).collect()
        };
        for tile in links {
            push(
                tile,
                WorldMapMarkKind::StackLink,
                link_compass_label(walked.contains(&tile)).to_string(),
                Some(CompassTarget::Link(tile)),
            );
        }

        for (_, pos) in self.nest_positions() {
            push(
                (pos.x, pos.y),
                WorldMapMarkKind::Nest,
                "a nest".to_string(),
                None,
            );
        }

        marks.sort_by_key(|(tile, _)| {
            (
                (tile.0 - party_tile.0)
                    .abs()
                    .max((tile.1 - party_tile.1).abs()),
                *tile,
            )
        });

        let routes = route_ends
            .into_iter()
            .map(|(from, to)| WorldMapRoute {
                from_chunk: chunk_of(from),
                to_chunk: chunk_of(to),
                preyed_by: self.route_predators(from, to),
            })
            .collect();

        Some(WorldMapView {
            center: center_chunk,
            radius,
            cells,
            marks: marks.into_iter().map(|(_, m)| m).collect(),
            routes,
            party: chunk_of(party_tile),
        })
    }
}
