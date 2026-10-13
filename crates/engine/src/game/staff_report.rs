//! The Staff page of the base report: how the staff are, whether the base
//! has the amenities to keep them so, and who has stopped working.
//!
//! Each report is a `&self` derivation and the gap flag is built from them,
//! so the page and the rail's `!` cannot disagree.

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::components::{
    Disgruntled, Downed, Grievance, Needs, OffShift, Position, Structure, Task,
};
use crate::game::base::offshift::{in_reach, open_tiles, places_of};
use crate::needs::{NeedDb, off_shift_share};
use crate::structures::StructureDb;
use crate::views::{
    BoxedInAmenity, CoverageRow, DiscontentRow, GrudgeRow, StaffActivity, WellbeingNeed,
    WellbeingRow, morale_band,
};
use bevy_ecs::prelude::Entity;

/// The measured working share never reads lower than this, so a base that
/// happens to be idle this tick does not under-report the amenities it needs.
const WORKING_SHARE_FLOOR: f32 = 0.5;

/// How many grudges a Discontent row lists.
const GRUDGES_SHOWN: usize = 3;

impl Game {
    /// Staff-role programs, worst first: lowest morale, ties by the lowest
    /// need as a fraction of its `critical`, then entity.
    pub fn staff_wellbeing(&self) -> Vec<WellbeingRow> {
        let amenities = self.amenities_here();
        let db = self.world.resource::<NeedDb>();
        let mut rows: Vec<(f32, WellbeingRow)> = self
            .base_staff()
            .into_iter()
            .map(|who| {
                let needs: Vec<WellbeingNeed> = self
                    .need_levels(who)
                    .into_iter()
                    .map(|n| WellbeingNeed {
                        name: db
                            .get(&n.id.as_str().into())
                            .map_or_else(|| n.id.clone(), |d| d.name.clone()),
                        level: n.level,
                        critical: n.critical,
                        content: n.content,
                    })
                    .collect();
                let lowest = needs
                    .iter()
                    .map(|n| n.level / n.critical.max(f32::EPSILON))
                    .fold(f32::INFINITY, f32::min);
                let at = self.world.get::<Position>(who).copied();
                let activity = if self.world.get::<Downed>(who).is_some() {
                    StaffActivity::Downed
                } else if let Some(off) = self.world.get::<OffShift>(who) {
                    let verb = db
                        .get(&off.need)
                        .map_or_else(String::new, |d| d.servicing.clone());
                    let in_a_bay = at.is_some_and(|here| {
                        amenities
                            .servicing(&off.need)
                            .iter()
                            .any(|(site, _, radius)| in_reach(here, *site, *radius))
                    });
                    let held = self
                        .world
                        .get::<Needs>(who)
                        .and_then(|n| n.serviced_at(&off.need))
                        .is_some();
                    if in_a_bay && !held {
                        StaffActivity::Waiting(verb)
                    } else {
                        StaffActivity::Servicing(verb)
                    }
                } else if self.on_shift(who) {
                    StaffActivity::OnShift
                } else {
                    StaffActivity::Break
                };
                let stats = self.world.get::<crate::components::Stats>(who);
                let morale = self.morale(who);
                (
                    lowest,
                    WellbeingRow {
                        entity: who,
                        name: self.creature_label(who),
                        morale,
                        mood: morale_band(morale),
                        hp: stats.map_or(0, |s| s.hp),
                        max_hp: stats.map_or(0, |s| s.max_hp),
                        needs,
                        activity,
                        grievance: self.grievance(who),
                    },
                )
            })
            .collect();
        rows.sort_by(|(la, a), (lb, b)| {
            a.morale
                .total_cmp(&b.morale)
                .then(la.total_cmp(lb))
                .then(a.entity.cmp(&b.entity))
        });
        rows.into_iter().map(|(_, row)| row).collect()
    }

    /// One row per need in the catalogue: what the base has built to service
    /// it against what its staff would need, and how many are short now.
    ///
    /// **Approximate**: `off_shift_share` ignores walking time.
    pub fn need_coverage(&self) -> Vec<CoverageRow> {
        let staff = self.base_staff();
        let working = self.working_share(&staff);
        let taken = self.structure_tiles();
        let grid = self.world.get_resource::<BaseGrid>();
        let structures = self.world.resource::<StructureDb>();
        let rooms = crate::rooms::of_world(&self.world);
        let levels: Vec<Vec<crate::views::NeedLevel>> =
            staff.iter().map(|&w| self.need_levels(w)).collect();
        let built: Vec<(Position, &crate::structures::StructureDef)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let def = structures.get(&e.get::<Structure>()?.kind)?;
                Some((*e.get::<Position>()?, def))
            })
            .collect();
        self.world
            .resource::<NeedDb>()
            .iter()
            .map(|def| {
                let mut count = 0;
                let mut places = 0;
                let mut served = Vec::new();
                let mut boxed_in = Vec::new();
                for (site, structure) in &built {
                    for service in structure.services.iter().filter(|s| s.need == def.id) {
                        let Some(rate) = service.rate() else {
                            continue;
                        };
                        count += 1;
                        places += places_of(grid, *site, service.radius, &taken);
                        served.push(rate * crate::rooms::service_scale_at(&rooms, site.x, site.y));
                        let open = grid
                            .map_or(usize::MAX, |g| open_tiles(g, *site, service.radius, &taken));
                        if open < crate::tuning::AMENITY_CAPACITY {
                            boxed_in.push(BoxedInAmenity {
                                label: structure.name.clone(),
                                x: site.x,
                                y: site.y,
                                open_tiles: open,
                            });
                        }
                    }
                }
                // With nothing built the best structure the catalogue offers
                // stands in, so "needed" still means something.
                let service = if served.is_empty() {
                    structures
                        .all()
                        .flat_map(|s| s.services.iter())
                        .filter(|s| s.need == def.id)
                        .filter_map(|s| s.rate())
                        .fold(0.0, f32::max)
                } else {
                    served.iter().sum::<f32>() / served.len() as f32
                };
                let share = off_shift_share(def, service, working);
                let needed = ((staff.len() as f32 * share / crate::tuning::AMENITY_CAPACITY as f32)
                    .ceil() as usize)
                    .max(1);
                let level_of = |staffer: &Vec<crate::views::NeedLevel>| {
                    staffer.iter().find(|n| n.id == def.id.as_str()).cloned()
                };
                let below = |pick: fn(&crate::views::NeedLevel) -> f32| {
                    levels
                        .iter()
                        .filter_map(level_of)
                        .filter(|n| n.level < pick(n))
                        .count()
                };
                CoverageRow {
                    name: def.name.clone(),
                    built: count,
                    places,
                    capacity: count * crate::tuning::AMENITY_CAPACITY,
                    needed,
                    below_critical: below(|n| n.critical),
                    below_content: below(|n| n.content),
                    lost_share: share,
                    boxed_in,
                }
            })
            .collect()
    }

    /// Programs with a grievance, the worst rung first, then the lowest
    /// morale, then entity.
    pub fn discontent(&self) -> Vec<DiscontentRow> {
        let mut rows: Vec<(Grievance, f32, DiscontentRow)> = self
            .base_staff()
            .into_iter()
            .filter_map(|who| {
                let rung = self.world.get::<Disgruntled>(who)?.grievance;
                let morale = self.morale(who);
                let mut grudges: Vec<GrudgeRow> = self
                    .memory_report(who)
                    .into_iter()
                    .filter(|m| m.intensity < 0.0)
                    .map(|m| GrudgeRow {
                        name: m.name,
                        subject: m.subject,
                        intensity: m.intensity,
                    })
                    .collect();
                // `memory_report` sorts by magnitude; the page wants the
                // most negative, which is the same order among negatives.
                grudges.truncate(GRUDGES_SHOWN);
                Some((
                    rung,
                    morale,
                    DiscontentRow {
                        entity: who,
                        name: self.creature_label(who),
                        rung: rung.as_str(),
                        stopped_work: rung >= Grievance::DownedTools,
                        mood: morale_band(morale),
                        grudges,
                    },
                ))
            })
            .collect();
        rows.sort_by(|(ra, ma, a), (rb, mb, b)| {
            rb.cmp(ra)
                .then(ma.total_cmp(mb))
                .then(a.entity.cmp(&b.entity))
        });
        rows.into_iter().map(|(_, _, row)| row).collect()
    }

    /// Why the Staff page deserves a look, or `None`. Built from the three
    /// reports the page shows.
    pub(crate) fn staff_flag(&self) -> Option<String> {
        let wellbeing = self.staff_wellbeing();
        if wellbeing.is_empty() {
            return None;
        }
        let hurting = wellbeing
            .iter()
            .filter(|r| r.is_critical() || r.activity == StaffActivity::Downed)
            .count();
        if hurting > 0 {
            return Some(format!("{hurting} in trouble"));
        }
        if let Some(short) = self.need_coverage().into_iter().find(|c| c.is_short()) {
            return Some(format!("short of {}", short.name));
        }
        let stopped = self.discontent().iter().filter(|d| d.stopped_work).count();
        (stopped > 0).then(|| format!("{stopped} stopped work"))
    }

    /// The fraction of `staff` holding a `Task`, floored at
    /// `WORKING_SHARE_FLOOR`.
    fn working_share(&self, staff: &[Entity]) -> f32 {
        if staff.is_empty() {
            return WORKING_SHARE_FLOOR;
        }
        let working = staff
            .iter()
            .filter(|&&w| self.world.get::<Task>(w).is_some())
            .count();
        (working as f32 / staff.len() as f32).max(WORKING_SHARE_FLOOR)
    }
}
