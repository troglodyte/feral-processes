//! `i`'s BESIDE YOU section: one entry per structure beside the party
//! (`Game::adjacent_structures`), its tabs that structure's pages. Each tab
//! is one existing screen, opened for that structure by its `_for` opener
//! (`App::enter_structure_page`). Opening and turning spend no turn.

use crate::*;

/// The BESIDE YOU page table: a rail title and the one screen each
/// `StructurePage` is. Which pages a structure shows, and in what order, is
/// the engine's answer (`AdjacentStructure::pages`).
pub fn structure_page(page: StructurePage) -> &'static FixedPage {
    match page {
        StructurePage::Inventory => &FixedPage {
            title: "Inventory",
            tabs: &[HubTab {
                name: "Inventory",
                mode: Mode::Transfer,
            }],
        },
        StructurePage::Filter => &FixedPage {
            title: "Filter",
            tabs: &[HubTab {
                name: "Filter",
                mode: Mode::DepotFilter,
            }],
        },
        StructurePage::Recipe => &FixedPage {
            title: "Recipe",
            tabs: &[HubTab {
                name: "Recipe",
                mode: Mode::MachineRecipe,
            }],
        },
        StructurePage::Tool => &FixedPage {
            title: "Tool",
            tabs: &[HubTab {
                name: "Tool",
                mode: Mode::RigTool,
            }],
        },
        StructurePage::Siphon => &FixedPage {
            title: "Siphon",
            tabs: &[HubTab {
                name: "Siphon",
                mode: Mode::Siphon,
            }],
        },
        StructurePage::Splice => &FixedPage {
            title: "Splice",
            tabs: &[HubTab {
                name: "Splice",
                mode: Mode::SpliceRig,
            }],
        },
        StructurePage::Grow => &FixedPage {
            title: "Grow",
            tabs: &[HubTab {
                name: "Grow",
                mode: Mode::FarmRecipe,
            }],
        },
        StructurePage::Rebuild => &FixedPage {
            title: "Rebuild",
            tabs: &[HubTab {
                name: "Rebuild",
                mode: Mode::BuildProgram,
            }],
        },
        StructurePage::Build => &FixedPage {
            title: "Build routine",
            tabs: &[HubTab {
                name: "Build routine",
                mode: Mode::RoutineBuild,
            }],
        },
        StructurePage::Weapon => &FixedPage {
            title: "Build weapon",
            tabs: &[HubTab {
                name: "Build weapon",
                mode: Mode::WeaponBuild,
            }],
        },
        StructurePage::Salvage => &FixedPage {
            title: "Salvage",
            tabs: &[HubTab {
                name: "Salvage",
                mode: Mode::WeaponSalvage,
            }],
        },
        StructurePage::Compile => &FixedPage {
            title: "Compile",
            tabs: &[HubTab {
                name: "Compile",
                mode: Mode::Craft,
            }],
        },
        StructurePage::Built => &FixedPage {
            title: "Built routines",
            tabs: &[HubTab {
                name: "Built routines",
                mode: Mode::BuiltRoutines,
            }],
        },
        StructurePage::Breed => &FixedPage {
            title: "Breed",
            tabs: &[HubTab {
                name: "Breed",
                mode: Mode::Breed,
            }],
        },
        StructurePage::Hold => &FixedPage {
            title: "Hold",
            tabs: &[HubTab {
                name: "Hold",
                mode: Mode::PinPrisoner,
            }],
        },
        StructurePage::Info => &FixedPage {
            title: "Info",
            tabs: &[HubTab {
                name: "Info",
                mode: Mode::StructureManifest,
            }],
        },
    }
}

/// Every page a structure can show, in rail order: the set `refresh_beside`
/// tests a stranded screen against.
const STRUCTURE_PAGES: [StructurePage; 16] = [
    StructurePage::Inventory,
    StructurePage::Filter,
    StructurePage::Recipe,
    StructurePage::Tool,
    StructurePage::Siphon,
    StructurePage::Splice,
    StructurePage::Grow,
    StructurePage::Rebuild,
    StructurePage::Build,
    StructurePage::Built,
    StructurePage::Weapon,
    StructurePage::Salvage,
    StructurePage::Compile,
    StructurePage::Breed,
    StructurePage::Hold,
    StructurePage::Info,
];

/// The compass word for the side `structure` is first beside.
fn side(structure: &AdjacentStructure) -> &'static str {
    match structure.dir {
        (0, -1) => "north",
        (1, 0) => "east",
        (0, 1) => "south",
        _ => "west",
    }
}

/// `i`'s BESIDE YOU entries: one per structure beside the party, in the
/// engine's clockwise order, its tabs the structure's pages. Two of a name
/// say which side they are on.
pub(crate) fn beside_pages(app: &App, section: &'static str) -> Vec<HubPage> {
    let Some(game) = &app.game else {
        return Vec::new();
    };
    let beside = game.adjacent_structures();
    let named = |name: &str| beside.iter().filter(|s| s.name == name).count();
    beside
        .iter()
        .enumerate()
        .map(|(i, s)| HubPage {
            key: HubPageKey::Structure(s.entity),
            title: if named(&s.name) > 1 {
                format!("{} ({})", s.name, side(s))
            } else {
                s.name.clone()
            },
            glyph: None,
            section: (i == 0).then_some(section),
            tabs: s.pages.iter().map(|&p| structure_page(p).tabs[0]).collect(),
            marked: false,
            name_cut_at_end: true,
        })
        .collect()
}

impl App {
    /// `[i]`: opened on the first structure beside the party (clockwise from
    /// north) if there is one, else where the player left it.
    pub(crate) fn open_inventory(&mut self) {
        if !self.open_on_first_beside() {
            self.open_hub(Hub::Inventory);
        }
    }

    /// `[c]`: `i`, opened on the first structure beside. With no structure
    /// beside, the pack's downed programs alone open the transfer screen;
    /// the caller has already refused when there is nothing at all.
    pub(crate) fn open_beside(&mut self) {
        if !self.open_on_first_beside() {
            self.open_transfer_for(TransferAt::PackOnly);
        }
    }

    /// Opens `i` on the first BESIDE YOU entry, cursor on the rail. Answers
    /// whether there was one.
    fn open_on_first_beside(&mut self) -> bool {
        let Some(game) = &self.game else {
            return false;
        };
        let Some(first) = game.adjacent_structures().first().map(|s| s.entity) else {
            return false;
        };
        self.turn_hub_to(Hub::Inventory, &HubPageKey::Structure(first));
        self.hub_on_rail = true;
        true
    }

    /// The structure the held page names, `None` on any other page.
    pub(crate) fn held_structure(&self) -> Option<Entity> {
        match self.hub_place.as_ref()?.key {
            HubPageKey::Structure(entity) => Some(entity),
            _ => None,
        }
    }

    /// Opens `entity`'s `page` through the screen's own opener, leaving the
    /// cursor on whichever half it was. Whatever the previous page held is
    /// dropped first, so a basket left by a page turn is discarded. An
    /// opener that finds its structure gone leaves for the first page of
    /// `i` rather than leaving the previous page up.
    pub(crate) fn enter_structure_page(&mut self, entity: Entity, page: StructurePage) {
        self.drop_page_state();
        match page {
            StructurePage::Inventory => self.open_transfer_for(TransferAt::Structure(entity)),
            StructurePage::Filter => {
                if !self.open_depot_filter_for(entity) {
                    return self.leave_beside();
                }
            }
            StructurePage::Recipe => {
                if !self.open_machine_recipe_for(entity) {
                    return self.leave_beside();
                }
            }
            StructurePage::Tool => {
                if !self.open_rig_tool_for(entity) {
                    return self.leave_beside();
                }
            }
            StructurePage::Siphon => self.open_siphon_for(entity),
            StructurePage::Splice => self.open_splice_rig(),
            StructurePage::Grow => {
                self.pending_plot = Some(entity);
                self.mode = Mode::FarmRecipe;
            }
            StructurePage::Rebuild => {
                self.pending_build = Some(PendingBuild::Rebuild { site: entity });
                self.mode = Mode::BuildProgram;
            }
            StructurePage::Build => self.mode = Mode::RoutineBuild,
            StructurePage::Weapon => self.mode = Mode::WeaponBuild,
            StructurePage::Salvage => self.mode = Mode::WeaponSalvage,
            StructurePage::Compile => self.mode = Mode::Craft,
            StructurePage::Built => self.mode = Mode::BuiltRoutines,
            StructurePage::Breed => {
                self.pending_breed_bay = Some(entity);
                self.mode = Mode::Breed;
            }
            StructurePage::Hold => self.mode = Mode::PinPrisoner,
            StructurePage::Info => {
                self.pending_structure_manifest = Some(entity);
                self.mode = Mode::StructureManifest;
            }
        }
        self.menu_selected = if self.mode == Mode::FarmRecipe {
            self.farm_recipe_row()
        } else {
            0
        };
    }

    /// The `tab`th page of the structure `entity`, if it is still beside
    /// the party and still offers that many.
    pub(crate) fn beside_page(&self, entity: Entity, tab: usize) -> Option<StructurePage> {
        self.game
            .as_ref()?
            .adjacent_structures()
            .into_iter()
            .find(|s| s.entity == entity)?
            .pages
            .get(tab)
            .copied()
    }

    /// Leaves a structure that is gone or no longer offers its page: drops
    /// what the page held and shows `i`'s first entry.
    pub(crate) fn leave_beside(&mut self) {
        self.drop_page_state();
        self.turn_hub_to(Hub::Inventory, &HubPageKey::Named("You"));
    }

    /// Run after every tick. A structure that goes or that the party walks
    /// away from takes its entry with it and `settle_hub` moves the hold;
    /// what its page held is dropped here, and a batch armed on it is
    /// called off, since the hold moving would strand the `HandCraft` and
    /// lose the unit in flight. A structure that stays but no longer offers
    /// the tab on show falls back to its first.
    pub(crate) fn refresh_beside(&mut self) {
        let Some(held) = self.held_structure() else {
            return;
        };
        let shown = base_popup_parent(self.mode).unwrap_or(self.mode);
        let Some(game) = &self.game else { return };
        let here = game
            .adjacent_structures()
            .into_iter()
            .find(|s| s.entity == held);
        let Some(here) = here else {
            if self.mode == Mode::Compiling {
                self.abort_compile();
            }
            self.drop_page_state();
            // Now, not at the next key: a tick can take the structure while
            // no key is pressed, and the screen would draw its stale page.
            self.settle_hub();
            return;
        };
        let stranded = STRUCTURE_PAGES
            .iter()
            .any(|&p| structure_page(p).tabs[0].mode == shown)
            && !here
                .pages
                .iter()
                .any(|&p| structure_page(p).tabs[0].mode == shown);
        if stranded {
            let first = here.pages[0];
            self.enter_structure_page(held, first);
        }
    }

    /// Clears what the open page keeps on `App`, so a reopened page never
    /// shows the last structure's rows.
    pub(crate) fn drop_page_state(&mut self) {
        self.clear_basket();
        self.depot_filter = None;
        self.machine_recipe = None;
        self.rig_tool = None;
        self.siphon = None;
        self.splice_rig = None;
        self.pending_plot = None;
        self.pending_breed_bay = None;
        self.pending_breed_first = None;
        self.pending_breed_second = None;
        self.pending_structure_manifest = None;
        self.drop_routine_craft_state();
        self.drop_gear_craft_state();
        if matches!(self.pending_build, Some(PendingBuild::Rebuild { .. })) {
            self.pending_build = None;
        }
    }
}
