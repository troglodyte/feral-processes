use super::*;

/// Every `Mode`, as the status-line census below drives them.
const ALL_MODES: [Mode; 171] = [
    Mode::Dossier,
    Mode::TacticalBattle,
    Mode::TacticalRoutine,
    Mode::TacticalItem,
    Mode::TacticalEmulate,
    Mode::TacticalAim,
    Mode::TacticalResult,
    Mode::MainMenu,
    Mode::CreateCharacter,
    Mode::LoadGame,
    Mode::SaveAction,
    Mode::Playing,
    Mode::Transfer,
    Mode::DepotFilter,
    Mode::RigTool,
    Mode::MachineRecipe,
    Mode::MachineRecipeConfirm,
    Mode::SpliceRig,
    Mode::Battle,
    Mode::BattleTarget,
    Mode::BattleItem,
    Mode::BattleSpecial,
    Mode::BattleAlly,
    Mode::BattleEmulate,
    Mode::BattleResult,
    Mode::Build,
    Mode::BuildDirection,
    Mode::BuildProgram,
    Mode::PinSubject,
    Mode::PinPrisoner,
    Mode::Siphon,
    Mode::RoutineBuild,
    Mode::RoutineBuildPick,
    Mode::RoutineBuildName,
    Mode::BuiltRoutines,
    Mode::WeaponBuild,
    Mode::WeaponBuildPick,
    Mode::WeaponBuildName,
    Mode::WeaponSalvage,
    Mode::DevConsole,
    Mode::DevItemPicker,
    Mode::DevZonePicker,
    Mode::Craft,
    Mode::CraftQuantity,
    Mode::Compiling,
    Mode::WorkOrders,
    Mode::WorkOrderPick,
    Mode::WorkOrderQuantity,
    Mode::BaseStaff,
    Mode::WorkStructure,
    Mode::Remove,
    Mode::RemoveConfirm,
    Mode::RemoveDirection,
    Mode::TrapDirection,
    Mode::UpgradeDirection,
    Mode::InspectDirection,
    Mode::FrameMap,
    Mode::WorldMap,
    Mode::Manifest,
    Mode::StructureManifest,
    Mode::CellDescribe,
    Mode::Settlement,
    Mode::SettlementMarket,
    Mode::SettlementBoard,
    Mode::OutpostVisit,
    Mode::OutpostPost,
    Mode::Inventory,
    Mode::PhaseKeys,
    Mode::EquipSwap,
    Mode::InventoryItemAction,
    Mode::ItemDescribe,
    Mode::EraseQuantity,
    Mode::DownedPrograms,
    Mode::Tools,
    Mode::Companion,
    Mode::CompanionEquip,
    Mode::CompanionMemories,
    Mode::Fuse,
    Mode::FuseSecond,
    Mode::FuseName,
    Mode::Breed,
    Mode::BreedSecond,
    Mode::BreedConfirm,
    Mode::RenamePet,
    Mode::RoutineTarget,
    Mode::Routines,
    Mode::RoutineInstall,
    Mode::FieldRoutine,
    Mode::FieldRoutineAlly,
    Mode::FieldRoutineCell,
    Mode::Excavate,
    Mode::ExcavateBrush,
    Mode::FarmRecipe,
    Mode::Stockpile,
    Mode::WallBox,
    Mode::PutDown,
    Mode::Refactor,
    Mode::RefactorItem,
    Mode::Develop,
    Mode::DevelopProgram,
    Mode::Extract,
    Mode::ExtractPick,
    Mode::ExtractConfirm,
    Mode::Trade,
    Mode::TradeAction,
    Mode::TradeQuantity,
    Mode::StackMarket,
    Mode::Caravan,
    Mode::Dispatch,
    Mode::SortieSquad,
    Mode::RouteCargo,
    Mode::TradeProgramConfirm,
    Mode::RespecPerksConfirm,
    Mode::RespecTalentsConfirm,
    Mode::Perks,
    Mode::AllocateStats,
    Mode::Research,
    Mode::RoutineResearch,
    Mode::AffixResearch,
    Mode::WeaponResearch,
    Mode::ModCopy,
    Mode::ModPickAffix,
    Mode::RefitCopy,
    Mode::Contracts,
    Mode::Alerts,
    Mode::History,
    Mode::Compass,
    Mode::Structures,
    Mode::StructureAssign,
    Mode::Recipes,
    Mode::BaseOutput,
    Mode::BaseOverview,
    Mode::BaseBuild,
    Mode::BaseResearch,
    Mode::BaseTrade,
    Mode::BaseCompile,
    Mode::PartyPrograms,
    Mode::PartyRoutines,
    Mode::StaffWellbeing,
    Mode::StaffCoverage,
    Mode::StaffDiscontent,
    Mode::BaseMachines,
    Mode::BasePower,
    Mode::BaseStock,
    Mode::BuildOrders,
    Mode::DigOrders,
    Mode::FloorOrders,
    Mode::Achievements,
    Mode::GameplayOptions,
    Mode::AudioOptions,
    Mode::GraphicsOptions,
    Mode::SpriteOptions,
    Mode::ManualOptions,
    Mode::Help,
    Mode::HelpPage,
    Mode::Notification,
    Mode::LevelUp,
    Mode::PerkBought,
    Mode::Ending,
    Mode::Escaped,
    Mode::BasinExitConfirm,
    Mode::GameOver,
    Mode::QuitRunConfirm,
    Mode::QuitAppConfirm,
    Mode::ArenaBuilder,
    Mode::ArenaLoad,
    Mode::ArenaSave,
    Mode::ArenaPick,
    Mode::ArenaResult,
    // Both dev-only, both full-pane draws with their refusal on
    // `needs_status_banner`'s strip rather than in a popup — see
    // `sprite_forge.rs`.
    Mode::SpritePicker,
    Mode::SpriteEditor,
];

const CENSUS_REFUSAL: &str = "Requires Zone 3 first.";

/// The modes whose screen needs state a fresh run has not got — a
/// pending trade, a program picked to fuse, a routine chosen to extract.
/// Each draws nothing at all here, so the census can say where a refusal
/// must *not* appear on them but not where it must. Their `draw_popup`
/// calls are threaded the same way every other one is.
const NEEDS_PENDING_STATE: [Mode; 34] = [
    // The Workbench pages are drawn only inside the structure hub frame,
    // which the census app never opens.
    Mode::RoutineBuild,
    Mode::BuiltRoutines,
    Mode::WeaponBuild,
    Mode::WeaponSalvage,
    // Drawn off `App::mod_copy`, which the census app never sets.
    Mode::ModCopy,
    Mode::ModPickAffix,
    // Drawn off `App::refit_copy`, which the census app never sets.
    Mode::RefitCopy,
    // Drawn off `pending_develop_target`, which the census app never
    // sets — the same omission `Mode::FuseSecond` below is here for.
    Mode::RespecTalentsConfirm,
    Mode::BattleTarget,
    Mode::BattleSpecial,
    Mode::CraftQuantity,
    // Nothing is armed in the census fixture — `Mode::Compiling` is
    // only ever entered right after `begin_hand_craft` succeeds, and
    // this app never commits a compile — so `draw_compiling` draws
    // nothing, same as every other pending-state screen here.
    Mode::Compiling,
    Mode::EraseQuantity,
    Mode::FuseSecond,
    Mode::FuseName,
    Mode::BreedSecond,
    Mode::BreedConfirm,
    Mode::RenamePet,
    Mode::Routines,
    Mode::FieldRoutineAlly,
    Mode::Excavate,
    Mode::Stockpile,
    Mode::WallBox,
    Mode::RefactorItem,
    Mode::DevelopProgram,
    Mode::ExtractPick,
    Mode::ExtractConfirm,
    Mode::TradeAction,
    Mode::TradeQuantity,
    Mode::StackMarket,
    // The caravan screen needs a trader actually standing at the
    // counter, which a fresh run has not got — `StackMarket`'s case.
    // `a_caravan_page_says_a_refusal_exactly_once` below is what says
    // its `draw_popup` call is threaded, since this census can only say
    // where a refusal must *not* appear on it.
    Mode::Caravan,
    Mode::TradeProgramConfirm,
    Mode::StructureAssign,
    // The market needs a settlement actually adjacent, which the
    // census app never places — `Mode::Caravan`'s case just above,
    // one vendor over.
    Mode::SettlementMarket,
];

/// How many times `mode` paints `CENSUS_REFUSAL` with it set.
fn refusals_drawn(app: &mut feral_processes_app_core::App, fx: &mut Fx, mode: Mode) -> usize {
    app.mode = mode;
    app.status_line = Some(CENSUS_REFUSAL.to_string());
    let (_, shapes) = crate::paint::with_painter(|p| draw(app, fx, p, false, false));
    crate::paint::painted_text(&shapes)
        .iter()
        .filter(|t| t.contains(CENSUS_REFUSAL))
        .count()
}

/// **The census this whole change rests on.** `draw_popup` takes its
/// refusal as an argument, so a *new* call site cannot forget it — it
/// will not compile without one — but nothing stops a caller passing
/// `None`, and nothing stops two stacked popups both passing `Some`.
/// This drives every `Mode` through `draw` and counts what was actually
/// painted, which is the only way to tell either apart from a screen
/// that simply looks right.
///
/// Both halves matter and both have caught a real defect: `Playing`
/// fell into the arm that hands the map `None` and showed the message
/// nowhere, and `ArenaSave` drew it on the prompt *and* on the builder
/// underneath it.
#[test]
fn every_screen_draws_a_refusal_exactly_once() {
    let mut app = census_app();
    let mut fx = Fx::new();
    for mode in ALL_MODES {
        let drawn = refusals_drawn(&mut app, &mut fx, mode);
        if NEEDS_PENDING_STATE.contains(&mode) {
            assert_eq!(drawn, 0, "{mode:?} draws nothing here, so it cannot say it");
        } else {
            assert_eq!(
                drawn, 1,
                "{mode:?} painted the refusal {drawn} times, not once"
            );
        }
    }
}

/// **Where** it lands, which is the whole point: under the title and
/// above the first row, not on a strip along an edge the player is not
/// looking at. `painted_text` comes back in paint order, so the row
/// order on screen is the order in that list.
#[test]
fn a_refusal_sits_between_the_title_and_the_first_row() {
    let mut app = census_app();
    let mut fx = Fx::new();
    app.mode = Mode::Research;
    // The list's popup: the graph draws no highlighted rows.
    app.research_graph_view = false;
    app.status_line = Some(CENSUS_REFUSAL.to_string());
    let (_, shapes) = crate::paint::with_painter(|p| draw(&mut app, &mut fx, p, false, false));
    let drawn = crate::paint::painted_text(&shapes);

    let at = |want: &str| drawn.iter().position(|t| t.contains(want));
    let title = at("Research").expect("the popup drew its title");
    let refusal = at(CENSUS_REFUSAL).expect("the popup drew the refusal");
    let first_row = drawn
        .iter()
        .position(|t| t.starts_with("> "))
        .expect("the popup drew a highlighted option");

    assert!(title < refusal, "the refusal was drawn above the title");
    assert!(
        refusal < first_row,
        "the refusal was drawn below the first option instead of over it"
    );
}

/// **The empty program picker, painted.** A fresh run owns no programs,
/// so a build order that reached this screen would land on a box with no
/// rows — `App::selected_index` returns `None` for a zero-length list —
/// with Esc as the only exit and, before this, nothing on screen saying
/// why. The engine's own sentence for that case is unreachable from this
/// frontend: app-core passes `None` to `place_structure` only for Home,
/// and Home is exempt.
///
/// Painted rather than asserted on the rows, because that is the half a
/// row test cannot see: the sentence goes out through `draw_popup`'s
/// refusal argument, and this says it arrives on the surface. The title
/// is checked in the same breath — a `Mode` missing from the draw match
/// falls into `_ => {}` and ships a blank screen that compiles clean.
#[test]
fn an_empty_program_picker_says_why_it_is_empty() {
    let mut app = census_app();
    let mut fx = Fx::new();
    app.mode = Mode::BuildProgram;
    app.pending_build = Some(feral_processes_app_core::PendingBuild::Deploy {
        structure: "extractor".to_string(),
        dx: 1,
        dy: 0,
    });
    app.status_line = None;
    let (_, shapes) = crate::paint::with_painter(|p| draw(&mut app, &mut fx, p, false, false));
    let drawn = crate::paint::painted_text(&shapes);

    let says = |want: &str| drawn.iter().any(|t| t.contains(want));
    assert!(
        says("Commit a program"),
        "the picker drew no title: {drawn:?}"
    );
    assert!(
        says("Extractor"),
        "the prompt names the structure the order is for: {drawn:?}"
    );
    assert!(
        says("permanently"),
        "and says what committing a program costs: {drawn:?}"
    );
    assert!(
        says("No program on your roster is free to spend on this."),
        "an empty picker is silent about why it is empty: {drawn:?}"
    );
}

/// **The upgrade arm, painted.** `building::build_commit`'s
/// `PendingBuild::Upgrade` branch resolves its label off
/// `App::upgradeable_structures().find(|s| s.entity == structure)` — a
/// re-scan, not a stored name — and before this test nothing in this
/// crate drove it through the real `draw` path: every pure test in
/// `building.rs` builds a `BuildCommit` straight from a literal label,
/// which cannot catch that `.find` missing and falling back to the
/// generic `"structure"` noun (`building::UNNAMED_BUILD_TARGET`). A
/// prior review round named this gap alongside the empty-picker one
/// above.
///
/// `app_in_base_with_an_extractor` (`render/test_support.rs`) gives this a
/// real, scan-reachable Extractor to upgrade: `App::upgradeable_structures`
/// finds the same `Entity` this test then hands `PendingBuild::Upgrade`,
/// so the prompt painted here is the one a genuine upgrade order would
/// show, not a fabricated stand-in the `.find` could never have matched.
#[test]
fn an_upgrade_prompt_names_the_real_structure_it_would_spend_a_program_on() {
    let mut app = test_support::app_in_base_with_an_extractor(4002);
    let mut fx = Fx::new();
    let structure = app
        .upgradeable_structures()
        .into_iter()
        .find(|s| s.label.starts_with("Extractor"))
        .expect("the fixture placed an Extractor within scan range")
        .entity;
    app.mode = Mode::BuildProgram;
    app.pending_build = Some(feral_processes_app_core::PendingBuild::Upgrade {
        structure,
        to_tier: 2,
    });
    app.status_line = None;
    let (_, shapes) = crate::paint::with_painter(|p| draw(&mut app, &mut fx, p, false, false));
    let drawn = crate::paint::painted_text(&shapes);
    let says = |want: &str| drawn.iter().any(|t| t.contains(want));
    assert!(
        says("Upgrading the Extractor (Core Fragment) to Mk2"),
        "the real structure's own name, not the generic fallback: {drawn:?}"
    );
    assert!(
        !says("Upgrading the structure to Mk2"),
        "a `.find` that missed its target would fall back to the generic noun: {drawn:?}"
    );
}

/// Nothing is drawn where nothing was refused — the popup grows a line
/// only when there is one, so a screen's layout is unchanged the rest of
/// the time.
#[test]
fn a_screen_with_nothing_refused_draws_no_refusal() {
    let mut app = census_app();
    let mut fx = Fx::new();
    for mode in [Mode::Playing, Mode::Research, Mode::Inventory, Mode::Battle] {
        app.mode = mode;
        app.status_line = None;
        let (_, shapes) = crate::paint::with_painter(|p| draw(&mut app, &mut fx, p, false, false));
        assert!(
            !crate::paint::painted_text(&shapes)
                .iter()
                .any(|t| t.contains(CENSUS_REFUSAL)),
            "{mode:?} painted a refusal that was never raised"
        );
    }
}
