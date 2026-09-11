//! The build, staffing, demolition, upgrade and symlink pickers.

use super::manifest::base_job_label;
use super::party::companion_rows;
use super::popup::*;
use super::*;
use feral_processes_app_core::{BaseStaffRow, PendingBuild, ProgramRole, WorkOrderRow};
use feral_processes_engine::components::BuildGoal;
use feral_processes_engine::structures::StructureId;
use feral_processes_engine::{
    BaseOutputReport, BaseOutputRow, BuildCandidate, BuildEffect, LabourDemand, OrderPriority,
    OrderState, WorkProfile, program_tier_required,
};

/// One buildable structure as the build menu needs it: everything that
/// required a `Game` to work out, already worked out.
pub(super) struct BuildEntry {
    /// The structure's own name. Carried apart from `cost` — rather than
    /// arriving as one joined label — because the deployed tag goes
    /// *between* them: a cost line already reads `Bytecode Block (12/20)`,
    /// so a count parenthesised after it is another cost fragment.
    pub name: String,
    /// What it costs, as `build_cost_label` writes it.
    pub cost: String,
    /// How many of this structure the base already counts — standing plus on
    /// order, `Game::deployed_count`. Zero is drawn as nothing at all: a
    /// menu of `(0)`s is noise on every row of a fresh base.
    pub deployed: u32,
    pub description: String,
    pub category: StructureCategory,
    /// Whether this row is one the roster's shortfall is even about —
    /// `StructureDef::needs_program`, carried rather than re-derived here,
    /// so the menu and the engine cannot disagree about which structures a
    /// program is owed for.
    pub needs_program: bool,
}

/// The heading a group opens with, or `None` for Home — a single structure
/// under a "Shelter" banner is a heading longer than the thing it labels.
fn category_heading(category: StructureCategory) -> Option<&'static str> {
    match category {
        StructureCategory::Home => None,
        StructureCategory::Extractor => Some("── Extractors — produce on their own ──"),
        StructureCategory::Assembler => Some("── Assemblers — fed by what they touch ──"),
        StructureCategory::Utility => Some("── Utility ──"),
        StructureCategory::Trade => Some("── Trade ──"),
        StructureCategory::Defence => Some("── Defence ──"),
    }
}

/// The build menu's rows, pure so the layout invariant below can be tested
/// without a `Game` or a `Painter`.
///
/// **Every row after the first is a `Row::Item`, including the headings and
/// the descriptions.** `popup_layout` ends the scrollable body at the *last*
/// `Row::Item` and pins whatever follows as a footer — so a description
/// emitted as `Row::Text` would slice the final structure's description off
/// the bottom of the list and strand it under the scroll indicator, detached
/// from the row it describes. `draw_structures` carries the same fix for the
/// same reason.
///
/// `entries` is assumed to arrive grouped by category, which
/// `StructureDb::all` guarantees; a heading is emitted whenever the category
/// changes, so an ungrouped list would simply repeat headings rather than
/// mislabel anything.
///
/// **`shortfall` greys rather than hides.** Every structure the cost applies
/// to costs the same thing — one program of zone 1 or deeper, which is every
/// program — so a roster that cannot pay stops all of them at once. That
/// makes it one line at the top of the screen and not a tag repeated down
/// every row, and the rows stay listed and stay pickable: a structure that
/// vanished from the catalogue would read as a bug, and the picker the pick
/// lands on says the same thing at more length.
///
/// **The sentence arrives rather than being chosen here**, because there
/// are two of them and only a `Game` can tell which — `DEPLOY_NEEDS_A_PROGRAM`
/// when nothing on the roster is free, `DEPLOY_NEEDS_A_SECOND_PROGRAM` when
/// the roster floor is what fired. `Some`/`None` and not a `bool` beside a
/// string, so a caller cannot pass a sentence and leave the rows lit, or
/// grey the rows and print nothing.
///
/// **An exempt structure never greys**, and `BuildEntry::needs_program`
/// carries which those are rather than this deciding. The Home is one: a
/// fresh run owns zero programs by definition, and told otherwise the very
/// first screen of a new game would be a menu of dim rows saying the player
/// cannot afford the thing they are about to do. A Depot is the other, and
/// it is the row a roster with nothing free most wants lit.
///
/// The flag arrives from `StructureDef::needs_program` — the engine's own
/// rule, the same one `App::handle_build_direction_key` routes the picker
/// off — so the menu cannot come to disagree with what the deploy will
/// actually charge. A `category == Home` test here was the version that
/// could, and it went dim on every Depot the moment the exemption stopped
/// being one category.
pub(super) fn build_menu_rows(
    entries: &[BuildEntry],
    selected: usize,
    shortfall: Option<&str>,
) -> Vec<Row> {
    let mut rows = vec![text_row("Esc to cancel; Up/Down + Enter also work")];
    if let Some(line) = shortfall {
        // ORANGE and a `Row::Text`, so it is pinned above the scrolling list
        // it is about rather than paging away from it — and so it never
        // joins the `Row::Item` span a keypress is resolved against.
        rows.push(Row::TextColored(line.to_string(), ORANGE));
    }
    let mut current: Option<StructureCategory> = None;
    for (i, entry) in entries.iter().enumerate() {
        if current != Some(entry.category) {
            current = Some(entry.category);
            if let Some(heading) = category_heading(entry.category) {
                rows.push(colored_item_row("", false, TEXT_DIM));
                rows.push(colored_item_row(heading, false, TEXT_DIM));
            }
        }
        let label = match entry.deployed {
            0 => format!("[{}] {} - {}", menu_shortcut(i), entry.name, entry.cost),
            n => format!(
                "[{}] {} ({n}) - {}",
                menu_shortcut(i),
                entry.name,
                entry.cost
            ),
        };
        let affordable = shortfall.is_none() || !entry.needs_program;
        rows.push(match affordable {
            true => item_row(label, i == selected),
            // `spent_item_row` rather than a hidden row: still selectable,
            // still navigated past, and reading as something there is no
            // point picking right now.
            false => spent_item_row(label, i == selected),
        });
        // Through `description_rows` rather than one indented `format!`: the
        // shipped descriptions run to 300 characters against a body of about
        // 114, and the wrapped lines have to stay `Row::Item` for the reason
        // above.
        rows.extend(description_rows(&entry.description));
    }
    rows
}

pub(super) fn draw_build_menu(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let status = game.player_status();
    let stock = game.base_stock();
    let defs = game.buildable_structure_defs();
    let entries: Vec<BuildEntry> = defs
        .iter()
        .map(|def| {
            let raw_cost = game.structure_build_cost(def);
            let cost = build_cost_display(game, &raw_cost, &status.inventory, &stock);
            BuildEntry {
                name: def.name.clone(),
                cost: build_cost_label(&cost),
                deployed: game.deployed_count(&def.id),
                description: def.description.clone(),
                category: def.category(),
                needs_program: def.needs_program(),
            }
        })
        .collect();
    // `Game::programs_for_build` and not `owned_pets`, because it is the one
    // derivation of what qualifies — the same call the picker draws and
    // `App::handle_build_program_key` indexes. A menu greyed off a different
    // count would promise a deploy the picker then has nothing to pay for.
    //
    // **Which sentence is a second question with a second answer.** The list
    // being empty is what greys the menu; `floored_by_the_last_program` is
    // what says whether the roster floor is why, and the two cases have to
    // read differently — a base holding one free program told "none of
    // yours is free to spend" is being told something it can see is false.
    let shortfall = game
        .programs_for_build(program_tier_required(BuildGoal::New))
        .is_empty()
        .then(|| match floored_by_the_last_program(game) {
            true => DEPLOY_NEEDS_A_SECOND_PROGRAM,
            false => DEPLOY_NEEDS_A_PROGRAM,
        });
    let rows = build_menu_rows(&entries, selected, shortfall);
    draw_popup("Deploy", PopupSize::Large, &rows, refusal, painter, m);
}

/// The deploy prompt's rows: what is about to be placed, before the compass
/// that places it.
///
/// All `Row::Text` — nothing here is pickable, the direction keys are — so
/// `popup_layout` pins the lot and none of it scrolls, which is what a prompt
/// this short wants. That is also why the description is wrapped here rather
/// than through `description_rows`: unindented, and a plain `Row::Text`,
/// because there is no menu row for it to sit under and no body for it to
/// fall out of. The width is the same `DESCRIBE_WRAP_COLUMNS` so the two
/// deploy screens quote the same prose at the same measure.
pub(super) fn build_direction_rows(name: &str, description: &str, cost: &[String]) -> Vec<Row> {
    let mut rows = vec![Row::TextColored(name.to_string(), YELLOW)];
    rows.extend(
        wrap_text(description, DESCRIBE_WRAP_COLUMNS)
            .into_iter()
            .map(|line| Row::TextColored(line, TEXT_DIM)),
    );
    rows.extend([
        text_row(""),
        // A waived bill is empty rather than zeroed (see
        // `StructureDef::first_free`), so the sentence is phrased here and
        // not filled in from nothing: `Costs ` with the figures missing
        // reads as the screen having failed to load them.
        text_row(if cost.is_empty() {
            "Free to deploy".to_string()
        } else {
            format!("Costs {}", cost.join(", "))
        }),
        text_row(""),
        text_row(DIRECTION_PROMPT),
    ]);
    rows
}

const DIRECTION_PROMPT: &str = "Choose a direction to deploy (arrows/hjkl), Esc to cancel";

/// What the build menu writes after a structure's name. A waived bill (see
/// `StructureDef::first_free`) has no rows at all, and a row that trailed
/// off after the dash read as a structure whose cost had failed to load.
fn build_cost_label(cost: &[String]) -> String {
    if cost.is_empty() {
        "free".to_string()
    } else {
        cost.join(", ")
    }
}

/// The compass the build menu hands off to. Drawn `Large` rather than `Small`
/// like the other direction prompts because it carries a structure's
/// description, which is the same text the build menu needed the wider box
/// for.
pub(super) fn draw_build_direction(
    game: &mut Game,
    pending: Option<&str>,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    // Picked out of the same list `handle_build_key` indexed, so the screen
    // cannot describe a structure the handler wouldn't deploy.
    let def = pending.and_then(|id| {
        game.buildable_structure_defs()
            .into_iter()
            .find(|d| d.id == id)
    });
    let rows = match def {
        Some(def) => {
            let status = game.player_status();
            let stock = game.base_stock();
            let cost = build_cost_display(
                game,
                &game.structure_build_cost(&def),
                &status.inventory,
                &stock,
            );
            build_direction_rows(&def.name, &def.description, &cost)
        }
        None => vec![text_row(DIRECTION_PROMPT)],
    };
    draw_popup(
        "Deploy Direction",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// The deploy menu's one line when the roster cannot pay for anything on
/// it: what deploying costs, why none of it is available, and the one
/// structure the rule does not apply to.
///
/// **"None of yours is free to spend", not "you have none".** A deploy asks
/// for `program_tier_required(BuildGoal::New)`, which is 1, and every tamed
/// program is from zone 1 or deeper — so depth can never be the reason a
/// deploy has nothing to offer. What separates an owned program from a
/// spendable one at tier 1 is whether it is *free*:
/// `Game::programs_for_build` drops the one you are wielding as a weapon,
/// anything downed, anything away on a sortie and anything already carrying
/// goods. A player holding their only program as a weapon does have one, and
/// a screen telling them they have none is simply wrong.
///
/// **It names the Home exemption because the greying does.** This line is
/// most often read on the very first screen of a new run, where the player
/// owns nothing and the row they are about to pick is the one row still lit.
///
/// **It is not the only reason the menu greys.** A base down to its last
/// program is stopped by the roster floor rather than by anything about
/// that program — see `LAST_PROGRAM_CLAUSE` and the three lines built on
/// it, one of which replaces this one when the floor is what fired.
pub(super) const DEPLOY_NEEDS_A_PROGRAM: &str =
    "Deploying costs a tamed program. None of yours is free to spend — only Home is exempt.";

/// The frontend's wording for the roster floor: a base may not spend its
/// way to zero programs, because the build crew *is* the roster.
///
/// **The engine's own sentence, not a fourth phrasing of the idea.**
/// `Game::commit_for_build` refuses with "Committing your last program
/// would leave nobody to build the {structure}." — it can name the
/// structure because it was handed the order. A menu line stands above a
/// whole list of structures and has none to name, so it stops at the clause
/// they share. A player who meets the floor at the menu and again at the
/// confirm reads the same words twice, which is what makes the second
/// screen read as the same rule rather than as a new one.
///
/// The three lines below are this clause plus whatever their own screen
/// has to add; `the_three_last_program_lines_share_the_engines_clause`
/// holds them to it. Kept as a `&str` and not a `format!` so that test can
/// compare them without a `Game`.
pub(super) const LAST_PROGRAM_CLAUSE: &str =
    "Committing your last program would leave nobody to build";

/// What the deploy menu says when the roster floor — and not depth, and not
/// availability — is what stops every row on it.
///
/// It keeps `DEPLOY_NEEDS_A_PROGRAM`'s Home tail for that line's reason: the
/// greying spares Home, so the sentence explaining the greying has to say
/// so. It drops that line's "None of yours is free to spend", which would be
/// read by a player looking at a roster with exactly one perfectly free
/// program on it — the state every run sits in from its first tamed program
/// until its second, and so the likeliest first reading of this screen.
pub(super) const DEPLOY_NEEDS_A_SECOND_PROGRAM: &str =
    "Committing your last program would leave nobody to build — only Home is exempt.";

/// Whether a screen with nothing to offer is short because of the **roster
/// floor** — the one reason that is not about any program on the roster —
/// and so which of the two sentences it prints. The single place that
/// decision is made; all three build screens ask it.
///
/// **Two questions and not one.** `Game::build_would_empty_the_roster` is
/// the engine's own rule and is `true` for a roster of none as much as for a
/// roster of one, because the engine has no reason to tell those apart: it
/// refuses either way, and a caller naming a program it does not own is
/// refused for that instead. A *sentence* has every reason to tell them
/// apart — "committing your last program" is a plain falsehood to a fresh
/// run that owns nothing, and that run is reading this line on the very
/// first screen of the game. So the second question is asked here.
///
/// It is a plain roster fact and not a second copy of the floor: this
/// function cannot say a build is allowed, only which true sentence to
/// print about one the engine has already stopped.
pub(super) fn floored_by_the_last_program(game: &mut Game) -> bool {
    game.build_would_empty_the_roster() && !game.owned_pets().is_empty()
}

/// What the picker says when nothing on the roster qualifies, which is a
/// first-class state of this screen and not an edge of it.
///
/// `App::selected_index` returns `None` for a zero-length list, so a player
/// who reaches the picker with no candidate lands on a box with no rows,
/// Esc as the only exit, and — before this — nothing at all explaining why.
/// The engine used to answer that case in a sentence, but app-core reaches
/// `place_structure` with `None` only for Home, and Home is exempt, so that
/// refusal is unreachable from this frontend. This is what fills the gap.
///
/// Three sentences because there are three reasons, and `last_program`
/// answers first because it is the only one of them that is true *whatever*
/// the tier. A base holding one zone 5 program and asked for a zone 4
/// upgrade is stopped by the floor, and "nothing on your roster is from
/// zone 4 or deeper" would be a plainly false thing to tell a player
/// looking at that roster.
///
/// Of the remaining two only one is depth. Tier 1 is a deploy, and every
/// tamed program is from zone 1 or deeper — so blaming depth there would be
/// plainly false too, and what is actually missing is a program that is
/// *free* (not wielded, downed, away on a sortie or carrying). Tier 2 and up
/// is an upgrade, where the roster really can be full of programs that are
/// simply too shallow.
///
/// It says nothing about Home, unlike `DEPLOY_NEEDS_A_PROGRAM`: this screen
/// is never reached for a Home at all, because
/// `App::handle_build_direction_key` places one straight without a picker.
/// That is also why the floor line here is the bare `LAST_PROGRAM_CLAUSE`
/// and not the deploy menu's Home-tailed version.
pub(super) fn no_candidates_refusal(tier: u32, last_program: bool) -> String {
    if last_program {
        format!("{LAST_PROGRAM_CLAUSE}.")
    } else if tier <= 1 {
        "No program on your roster is free to spend on this.".to_string()
    } else {
        format!("Nothing on your roster is from zone {tier} or deeper.")
    }
}

/// What the program picker is about to spend a program on: the name the
/// prompt puts in front of the player, and which kind of order it is.
///
/// Assembled in `build_commit` rather than read off `App::pending_build`
/// here, because the two orders name themselves from different places — a
/// deploy's name is a `StructureDef`, an upgrade's is the roster row it was
/// picked off — and neither is on `PendingBuild`. Carrying the finished name
/// keeps `build_program_rows` pure, so what the prompt says is measurable
/// without a `Game` or a window.
pub(super) struct BuildCommit {
    /// The structure's name, as the menu that chose it wrote it.
    pub label: String,
    /// The tier the order would produce: `None` for a deploy, `Some(n)` for
    /// the tier an upgrade would raise the structure to.
    pub to_tier: Option<u32>,
    /// What `Game::build_candidates` is asked about, from
    /// `App::pending_build_kind` — the def id for a deploy, the standing
    /// structure's own kind for an upgrade.
    ///
    /// Carried here rather than fetched by the renderer because
    /// `draw_build_program` takes `&mut Game` after `build_commit(app)` has
    /// been hoisted, so there is no `App` left to ask.
    pub structure: StructureId,
    /// The bill this order will put the crew on, `build_cost_display`'s
    /// form — empty for a deploy.
    ///
    /// **Only an upgrade carries it, and that is not an oversight.** A
    /// deploy knows its structure a screen earlier, so `draw_build_direction`
    /// quotes the bill there while the player is still choosing where to put
    /// it. An upgrade's target is not known until the direction key lands —
    /// `Mode::UpgradeDirection` is aimed at a tile — so this is the first
    /// screen that *can* price it, and the list it replaced was the only
    /// place the figure used to appear.
    pub cost: Vec<String>,
}

impl BuildCommit {
    /// How deep a program this order demands — **the engine's own rule,
    /// called rather than restated.** `App::handle_build_program_key` hands
    /// `Game::programs_for_build` the number this same function returns, so
    /// the list on screen and the list a keypress indexes ask for the same
    /// depth and cannot drift apart.
    ///
    /// **That is a claim about depth, and only about depth.** It says
    /// nothing about the other rules a commit can fail, and for a while the
    /// picker did offer a program `commit_for_build` then refused: the
    /// roster floor lived in that function alone, so a base holding exactly
    /// one program — the state every run sits in from its first tamed
    /// program until its second — lit the menu, listed that program
    /// undimmed, and refused only after the confirm.
    /// `Game::programs_for_build` now folds the floor in, so the picker
    /// offers nothing there; `commit_for_build` keeps its own check as
    /// defence in depth for callers that never drew a list, and its more
    /// specific sentence can still be reached by a program too shallow for
    /// the tier.
    /// The goal this order carries, derived from `to_tier` rather than
    /// stored beside it — `None` is a deploy, `Some(t)` an upgrade to `t`.
    pub(super) fn goal(&self) -> BuildGoal {
        match self.to_tier {
            Some(to_tier) => BuildGoal::Upgrade { to_tier },
            None => BuildGoal::New,
        }
    }

    pub(super) fn tier(&self) -> u32 {
        program_tier_required(match self.to_tier {
            Some(to_tier) => BuildGoal::Upgrade { to_tier },
            None => BuildGoal::New,
        })
    }

    /// The sentence above the list: what this costs, and how long you can
    /// still take it back.
    ///
    /// **Both halves are load-bearing.** "Permanently" is the warning — this
    /// is the one keypress in the base loop that deletes a program from the
    /// roster, and a picker that read like an equipment menu would be
    /// inviting it. The second sentence is the honesty: while the order is
    /// still a build request the crew has not finished,
    /// `return_build_holdings` gives the program back through either
    /// destruction door, and only `consume_site` — the structure actually
    /// standing — makes the spend final. A prompt saying "never comes back"
    /// would be a plain lie about a refund the game does pay.
    fn prompt(&self) -> String {
        let what = match self.to_tier {
            Some(to_tier) => format!("Upgrading the {} to Mk{to_tier}", self.label),
            None => format!("Deploying the {}", self.label),
        };
        format!(
            "{what} permanently spends one of your programs. Call the order off \
             before the crew finishes and it comes back; once the structure \
             stands, it is gone."
        )
    }
}

/// The name and kind of order `Mode::BuildProgram` is confirming, or `None`
/// when there is no order at all.
///
/// Reads `App` rather than `Game` because that is where both names live —
/// `App::upgradeable_structures` is the list an upgrade's row was picked
/// off, so naming the structure from it means the screen cannot describe a
/// different structure from the one `handle_upgrade_key` recorded. It has to
/// run before `render::draw` takes `&mut app.game`, which is why the caller
/// hoists it up beside `scanned`.
///
/// A name that will not resolve falls back to a plain noun instead of
/// dropping the whole prompt: the *tier* is the part the player cannot
/// afford to lose, and it is carried on `PendingBuild` itself.
pub(super) fn build_commit(app: &mut App) -> Option<BuildCommit> {
    let (label, to_tier, cost) = match app.pending_build.clone()? {
        PendingBuild::Deploy { structure, .. } => (
            app.game
                .as_ref()
                .and_then(|game| {
                    game.buildable_structure_defs()
                        .into_iter()
                        .find(|def| def.id == structure)
                })
                .map(|def| def.name),
            None,
            Vec::new(),
        ),
        PendingBuild::Upgrade { structure, to_tier } => (
            app.upgradeable_structures()
                .into_iter()
                .find(|s| s.entity == structure)
                .map(|s| s.label),
            Some(to_tier),
            upgrade_cost_display(app, structure),
        ),
    };
    Some(BuildCommit {
        label: label.unwrap_or_else(|| UNNAMED_BUILD_TARGET.to_string()),
        to_tier,
        structure: app.pending_build_kind()?,
        cost,
    })
}

/// What the crew will have to fetch to raise `structure` a tier, priced
/// against the pack **and** the base's shelves.
///
/// `build_cost_display` and not `cost_display`, the rule the upgrade list
/// held before it: the crew fetches, so quoting the pack alone would price
/// the order against a store the verb does not read. `Game::upgrade_cost` is
/// the same door that list used, so the figure did not move when the screen
/// under it did.
fn upgrade_cost_display(app: &mut App, structure: Entity) -> Vec<String> {
    let Some(game) = app.game.as_mut() else {
        return Vec::new();
    };
    let Some(cost) = game.upgrade_cost(structure) else {
        return Vec::new();
    };
    let status = game.player_status();
    let stock = game.base_stock();
    build_cost_display(game, &cost, &status.inventory, &stock)
}

/// The noun the prompt falls back to when the order's structure cannot be
/// named — a def id no longer in the catalogue, or an upgrade target that
/// left the scan radius between the pick and the frame. Neither is reachable
/// while the picker is up, and a prompt reading "the structure" is still a
/// warning where a missing prompt would be a blank confirmation.
const UNNAMED_BUILD_TARGET: &str = "structure";

const PROGRAM_PICKER_PROMPT: &str =
    "Which program pays for it? (Esc to cancel; Up/Down + Enter also work)";

/// What the picker says, once and above the list, about a build that runs no
/// work cycle at all.
///
/// A fact about the whole screen belongs above the list rather than repeated
/// on every row, and the rows deliberately do **not** grey: every program is
/// equally valid here, which is exactly the message.
const NO_CYCLE_NOTE: &str =
    "This build does not run a work cycle, so it does not care which program you spend.";

/// The program picker's rows: the warning, then one entry per program the
/// order could be paid with.
///
/// **`candidates` is drawn exactly as it arrives.** It is
/// `Game::build_candidates`'s list, which is the one derivation of what
/// qualifies *and of the order it is in*, and `App::handle_build_program_key`
/// indexes that same call — so a row filtered out or reordered here would
/// make `[c]` spend the program the player read on row `d`.
///
/// The rows themselves are the party screen's, through `companion_rows`:
/// this keypress deletes a program from the roster, so the decision wants
/// the same stat line, tier colour and CRITICAL mark the roster shows, not a
/// bare list of names. The roster's **role headings do not come along**: this
/// list is sorted by the roll the build reads, so it interleaves roles and a
/// heading fired on the change would repeat and mean nothing. The warning
/// they carried — spending a program also empties its job — stays on the row
/// as `PetInfo::activity`.
pub(super) fn build_program_rows(
    commit: Option<&BuildCommit>,
    candidates: &[BuildCandidate],
    selected: usize,
) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    if let Some(commit) = commit {
        // ORANGE, the colour `draw_remove_confirm` warns in, and wrapped at
        // the prose measure every other screen quotes prose at. All
        // `Row::Text`, so `popup_layout` pins them above the scrolling body
        // — the warning must not scroll off the list it is about.
        rows.extend(
            wrap_text(&commit.prompt(), DESCRIBE_WRAP_COLUMNS)
                .into_iter()
                .map(|line| Row::TextColored(line, ORANGE)),
        );
    }
    // Wrapped, and at `DESCRIBE_WRAP_COLUMNS` like the warning above it:
    // `draw_row` clips a row vertically and never horizontally, and a bill
    // naming three materials with their two figures each is already past the
    // body at one line.
    if let Some(commit) = commit.filter(|c| !c.cost.is_empty()) {
        rows.extend(
            wrap_text(
                &format!("Your crew fetches: {}", commit.cost.join(", ")),
                DESCRIBE_WRAP_COLUMNS,
            )
            .into_iter()
            .map(text_row),
        );
    }
    rows.push(text_row(PROGRAM_PICKER_PROMPT));
    if !candidates.is_empty() && candidates.iter().all(|c| c.effect == BuildEffect::NoCycle) {
        rows.extend(
            wrap_text(NO_CYCLE_NOTE, DESCRIBE_WRAP_COLUMNS)
                .into_iter()
                .map(text_row),
        );
    }
    for (i, c) in candidates.iter().enumerate() {
        let mut extra = vec![format!(" [{}: {}]", c.aptitude, c.label)];
        // Two whole tick figures rather than a percentage — see
        // `views::BuildEffect`. A `NoCycle` build carries no tag at all; the
        // sentence above the list has already said why.
        if let BuildEffect::Cycle { shipped, built } = c.effect {
            extra.push(format!(
                " [{} cycle {shipped} -> {built} ticks]",
                commit.map_or("machine", |c| c.label.as_str())
            ));
        }
        rows.extend(companion_rows(
            &c.pet,
            menu_shortcut(i),
            i == selected,
            &extra,
        ));
    }
    rows
}

/// The picker `Mode::BuildProgram` draws: which program pays for the order
/// `App::pending_build` is holding.
///
/// The empty list goes out through `draw_popup`'s refusal argument rather
/// than as a `Row`, which is what keeps it off the `Row::Item` span
/// `App::selected_index` resolves against — the panel grows a line instead
/// of covering one. A refusal the player's last keypress actually raised
/// wins over it: that one is news, and the empty-list sentence is a standing
/// fact the player can already see (there are no rows).
///
/// With no pending order there is nothing to name and nothing to spend, and
/// any key drops `App::handle_build_program_key` straight back to the map.
/// The box is still drawn — a blank window is a screen the player cannot
/// read their way out of — with the deploy tier as the stand-in, which is
/// the shallower of the two and so cannot promise a program the engine would
/// refuse.
pub(super) fn draw_build_program(
    game: &mut Game,
    commit: Option<BuildCommit>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let tier = commit.as_ref().map_or(1, BuildCommit::tier);
    let candidates = match &commit {
        Some(commit) => game.build_candidates(&commit.structure, commit.goal()),
        // With no pending order there is nothing to be for, so there is
        // nothing to quote — the box is still drawn, empty, rather than
        // leaving the player on a blank screen.
        None => Vec::new(),
    };
    // Asked before the list is handed away: the empty list is the same
    // empty list whichever rule emptied it, and only
    // `floored_by_the_last_program` tells a roster that has nothing
    // suitable from one that has exactly one program and may spend none of
    // it. See `no_candidates_refusal`.
    let last_program = floored_by_the_last_program(game);
    let rows = build_program_rows(commit.as_ref(), &candidates, selected);
    let nothing_qualifies = candidates
        .is_empty()
        .then(|| no_candidates_refusal(tier, last_program));
    draw_popup(
        "Commit a program",
        PopupSize::Large,
        &rows,
        refusal.or(nothing_qualifies.as_deref()),
        painter,
        m,
    );
}

/// The roster's standing-instruction toggles for the structure highlighted
/// there: keep it running, keep it guarded, or work it yourself right now.
///
/// Which rows exist is `App::staffing`'s decision — it asks the same two
/// questions `Game::set_standing_job` and `Game::work_structure` refuse on —
/// and this draws the list it is handed rather than filtering again, so the
/// row the handler acts on is the row under the highlight.
pub(super) fn draw_staffing_menu(
    staffing: &Staffing,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(format!(
        "Standing orders for the {} (Esc to close; Up/Down + Enter also work)",
        staffing.target
    ))];
    if staffing.rows.is_empty() {
        rows.push(text_row("(nothing to say about this one)"));
    }
    for (i, row) in staffing.rows.iter().enumerate() {
        let mark = match row.on {
            Some(true) => "[x] ",
            Some(false) => "[ ] ",
            None => "",
        };
        rows.push(item_row(
            format!("[{}] {mark}{}", menu_shortcut(i), row.label),
            i == selected,
        ));
    }
    rows.push(text_row(
        "A standing job is filled only by a program no work order needs.",
    ));
    draw_popup(
        "Standing Orders",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// The work order queue and its status: what the base has been told to
/// hold, how close it is, and which machine each order is waiting on.
///
/// The machine lines under an order are `Game::work_order_report`'s, which
/// is the scheduler's own `wants` walk — so what is on screen is what the
/// base believes by construction rather than by a comment claiming the two
/// agree.
pub(super) fn draw_work_orders(
    rows_in: &[WorkOrderRow],
    demand: LabourDemand,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = Vec::new();
    if let Some(header) = labour_header(demand) {
        rows.push(text_row(header));
    }
    rows.push(text_row(
        "Enter to queue an order, Backspace to drop one, Esc to close",
    ));
    if rows_in.is_empty() {
        rows.push(text_row("(nothing the base can make yet)"));
    }
    for (i, row) in rows_in.iter().enumerate() {
        let mut lines = work_order_lines(row, i, i == selected).into_iter();
        if let Some(head) = lines.next() {
            rows.push(item_row(head, i == selected));
        }
        rows.extend(lines.map(text_row));
    }
    draw_popup("Work Orders", PopupSize::Large, &rows, refusal, painter, m);
}

/// How many bodies short the base is, or `None` when it has enough.
///
/// **Silent when there is no shortfall.** The figure answers "why is
/// nothing happening" from the other direction to `state_tag`: the tag says
/// which order has the base's attention, this says whether the base has
/// anyone to give it. A line that shows on every visit is a line nobody
/// reads by the third one, and a base with bodies to spare has nothing to
/// explain.
///
/// A pure function of the demand for `work_order_lines`' reason — it is a
/// head line, which is to say an unwrapped one, so it is the row on this
/// screen that can actually run off the body.
fn labour_header(demand: LabourDemand) -> Option<String> {
    let short = demand.shortfall();
    if short == 0 {
        return None;
    }
    // **"post", not "machine".** The want list carries standing guard jobs
    // and dig sites as well as machines, so a shortfall is not always a
    // machine standing idle and the header must not claim it is.
    Some(format!(
        "{} post{} wanted, {} program{} on the base — {short} unfilled",
        demand.wanted,
        if demand.wanted == 1 { "" } else { "s" },
        demand.staff,
        if demand.staff == 1 { "" } else { "s" },
    ))
}

/// One work order's lines: the order itself, then a line per machine in the
/// chain — or the sentence naming why it is stalled.
///
/// A pure function of the row rather than pushed straight into the popup,
/// so a headless test can measure how wide the widest of them runs.
/// `draw_row` clamps a row vertically and never horizontally, so nothing
/// else would catch a line that runs off the body.
fn work_order_lines(row: &WorkOrderRow, index: usize, _selected: bool) -> Vec<String> {
    let Some(order) = &row.order else {
        return vec![format!("[{}] New work order...", menu_shortcut(index))];
    };
    let state = state_tag(order.state);
    let mut lines = vec![format!(
        "[{}] {}  {}/{}{state}",
        menu_shortcut(index),
        order.label,
        order.have,
        order.target
    )];
    // Through `continuation_lines` rather than an inline `format!`: a
    // stalled order's reason is a whole sentence and a machine line carries
    // three names, and either can outrun the popup body.
    if let Some(why) = &order.blocked_by {
        lines.extend(continuation_lines(why));
        return lines;
    }
    if order.machines.is_empty() {
        // Three ways to want nobody, and the tag alone does not say which:
        // the base is holding the level, the line broke (handled above), or
        // every machine in it is momentarily busy or clogged.
        lines.extend(continuation_lines(match order.state {
            OrderState::Dormant => "holding — the base has this, so nothing is being made",
            _ => "waiting — nothing to do here yet",
        }));
    }
    for machine in &order.machines {
        let who = match &machine.worker {
            Some(name) => name.clone(),
            None => "no one".to_string(),
        };
        let short = machine
            .short_of
            .as_ref()
            .map(|s| format!(", short of {s}"))
            .unwrap_or_default();
        lines.extend(continuation_lines(&format!(
            "{} — {who}{short}",
            machine.label
        )));
    }
    lines
}

/// The token beside an order's row. Every state carries one, including the
/// healthy ones: the screen's job in a queue several orders deep is to say
/// which order has the base's attention, and a tag that appears only when
/// something is wrong cannot answer that.
/// `HOLDING` rather than the enum's own word, because the sentence that
/// filed the order already said "hold 5 x Core Fragment" — the player's
/// word for the state is the one they typed it in with.
fn state_tag(state: OrderState) -> &'static str {
    match state {
        OrderState::Working => "  WORKING",
        OrderState::Queued => "  QUEUED",
        OrderState::Dormant => "  HOLDING",
        OrderState::Stalled => "  STALLED",
    }
}

/// What a band means, spelled out rather than named: "Normal" alone says
/// nothing about where the order lands, and where it lands is the whole of
/// what the band does.
fn priority_line(priority: OrderPriority) -> &'static str {
    match priority {
        OrderPriority::High => "high — files above the ordinary orders",
        OrderPriority::Normal => "normal — files behind the orders already queued",
        OrderPriority::Low => "low — files below everything, worked with what is left",
    }
}

/// Picking what to order — `Game::orderable_items`, which asks the same
/// chain question the queue refuses on, so nothing here can be rejected.
pub(super) fn draw_work_order_pick(
    items: &[(ItemId, String)],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(
        "What should the base hold? (Esc to cancel; Up/Down + Enter also work)",
    )];
    if items.is_empty() {
        rows.push(text_row("(nothing the base can make — deploy a machine)"));
    }
    for (i, (_, name)) in items.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {name}", menu_shortcut(i)),
            i == selected,
        ));
    }
    draw_popup(
        "New Work Order",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

const WORK_ORDER_QUANTITY_KEYS: &str =
    "[S] Standing order   [P] Priority   Digits then Enter   Esc to go back";

/// How many of it. The same two-page shape the compile flow uses.
///
/// `PopupSize::Large`, as `draw_craft_quantity` is: the sentences on it are
/// prose rather than menu rows, and the widest already ran 8px past a small
/// box before this page gained a toggle to explain.
/// Eight for `draw_arena_result`'s reason: the refusal is a parameter,
/// not something a draw function reaches for.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_work_order_quantity(
    game: &Game,
    item: Option<ItemId>,
    typed: &str,
    standing: bool,
    priority: OrderPriority,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let name = item
        .as_ref()
        .map(|i| game.item_name(i).to_string())
        .unwrap_or_default();
    let shown = if typed.is_empty() { "1" } else { typed };
    let rows: Vec<Row> = work_order_quantity_lines(&name, shown, standing, priority)
        .into_iter()
        .map(text_row)
        .collect();
    draw_popup(
        "Order Quantity",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// The quantity page's lines, pure for `work_order_lines`' reason: a page
/// built out of text rows has no scroll and `draw_row` never clips one
/// horizontally, so a sentence that outgrows the popup body is lost in
/// silence and only a headless measurement catches it.
fn work_order_quantity_lines(
    name: &str,
    shown: &str,
    standing: bool,
    priority: OrderPriority,
) -> Vec<String> {
    vec![
        format!("How many {name} should the base hold?"),
        String::new(),
        format!("Quantity: {shown}"),
        String::new(),
        format!(
            "Standing order: {}",
            if standing {
                "on — the base tops this level back up as it drains"
            } else {
                "off — one batch, and the order is done with"
            }
        ),
        String::new(),
        format!("Priority: {}", priority_line(priority)),
        String::new(),
        // `base_holding` sums machine and depot buffers only, so a player
        // carrying forty of the thing still reads 0/20 on the queue screen.
        "The target is what the base holds. What you are carrying is yours.".to_string(),
        String::new(),
        WORK_ORDER_QUANTITY_KEYS.to_string(),
    ]
}

/// The roster as the base sees it: every program the player owns, the role
/// it is in, and what it is doing right now.
///
/// **Read-only.** Roles are derived — a program you own and are not fighting
/// with is base staff — so there is nothing on this screen to toggle. The
/// Companions screen is where a party is picked, and the base takes whatever
/// that leaves.
pub(super) fn draw_base_staff(
    game: &mut Game,
    staff_rows: &[BaseStaffRow],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let pets = game.owned_pets();
    let rows = base_staff_menu_rows(staff_rows, &pets, selected);
    draw_popup("Base Staff", PopupSize::Large, &rows, refusal, painter, m);
}

/// What a program is worth at a post, as the staff row says it.
///
/// The class goes through `manifest::base_job_label` rather than a second
/// mapping of its own: that one is exhaustive on purpose so a sixth class
/// cannot ship without deciding what it does at a post, and a copy here
/// would be a way to dodge that.
fn work_summary(work: Option<WorkProfile>) -> String {
    let Some(work) = work else {
        // The species is not in the db, so there are no numbers to quote —
        // saying so beats printing the roster's defaults as if authored.
        return "species not loaded".to_string();
    };
    let job = work
        .class
        .map(base_job_label)
        .unwrap_or_else(|| "no base job".to_string());
    format!("Spd {} · Ana {} · {job}", work.speed, work.analysis)
}

/// The Base Staff popup's rows: a shortcut line naming the program and what
/// it brings to a post, and an indented line under it for what it is doing
/// now.
///
/// Two lines because one busts the row budget. The widest realistic row — a
/// Gold fused program of the longest species name, with a zone tag, the
/// work summary and the longest activity — is 106 characters against
/// `ROW_WRAP_COLUMNS`' 100. Measured against the real font that row does
/// still *fit* the reference 1440x900 geometry, but by about two characters
/// (1203px of 1243px including the draw prefix), and `draw_row` clamps a row
/// vertically and never horizontally, so nothing catches the row that
/// finally doesn't. The activity is the half that moved, because it is the
/// half you read second.
///
/// Split out of `draw_base_staff` so the layout is reachable without a
/// `Game`, which is what `every_base_staff_activity_stays_inside_the_scrollable_body`
/// and `the_widest_base_staff_row_stays_inside_the_popup` need.
pub(super) fn base_staff_menu_rows(
    staff_rows: &[BaseStaffRow],
    pets: &[PetInfo],
    selected: usize,
) -> Vec<Row> {
    let mut rows = vec![text_row(
        "Every program you own works the base unless it is in your party. Esc to close.",
    )];
    if staff_rows.is_empty() {
        rows.push(text_row("(no compiled programs — beat one first)"));
    }
    for (i, row) in staff_rows.iter().enumerate() {
        let pet = pets.iter().find(|p| p.entity == row.program.entity);
        rows.push(with_icon(
            tier_row(
                format!(
                    "[{}] {} — {}",
                    menu_shortcut(i),
                    row.program.label,
                    work_summary(row.work)
                ),
                i == selected,
                pet.map(|p| p.fusions).unwrap_or(0),
                pet.map(|p| p.rarity).unwrap_or_default(),
            ),
            row.program.glyph,
            glyph_color(row.program.color),
        ));
        let side = if row.role == Some(ProgramRole::Staff) {
            format!("base, {}", row.doing)
        } else {
            row.doing.clone()
        };
        // Dim `Item`s that can never be selected, not `Row::Text`:
        // `popup_layout` ends the scrollable body at the *last* `Row::Item`,
        // so text sub-lines under the last program would be pinned into the
        // footer alongside this screen's legend and drawn detached at the
        // bottom. That is the bug `routines::description_row` exists for, in
        // the same shape. `continuation_lines` rather than one `format!` so
        // the indent is the shared one and a long activity wraps.
        rows.extend(
            continuation_lines(&side)
                .into_iter()
                .map(|line| colored_item_row(line, false, TEXT_DIM)),
        );
    }
    rows.push(text_row(
        "Base staff are posted automatically by your work orders.",
    ));
    rows
}

pub(super) fn draw_structure_menu(
    structures: &[EntityView],
    title: &str,
    prompt: &str,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(format!(
        "{prompt} (Esc to cancel; Up/Down + Enter also work)"
    ))];
    if structures.is_empty() {
        rows.push(text_row("(no structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        let assigned = s
            .structure_worker
            .as_ref()
            .map(|w| format!(" (assigned: {w})"))
            .unwrap_or_default();
        let durability = s
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        rows.push(with_icon(
            item_row(
                format!(
                    "[{}] {} at ({}, {}){}{}",
                    menu_shortcut(i),
                    s.label,
                    s.pos.0,
                    s.pos.1,
                    durability,
                    assigned
                ),
                i == selected,
            ),
            s.glyph,
            glyph_color(s.color),
        ));
    }
    draw_popup(title, PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_remove_menu(
    structures: &[EntityView],
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let mut rows = vec![text_row(
        "Demolish which structure? Removing Home destroys the whole base. (Esc to cancel; Up/Down + Enter also work)",
    )];
    if structures.is_empty() {
        rows.push(text_row("(no structures nearby)"));
    }
    for (i, s) in structures.iter().enumerate() {
        let durability = s
            .durability
            .map(|(hp, max)| format!(" [HP {hp}/{max}]"))
            .unwrap_or_default();
        let home_tag = if s.is_home { " (Home)" } else { "" };
        rows.push(with_icon(
            item_row(
                format!(
                    "[{}] {} at ({}, {}){}{}",
                    menu_shortcut(i),
                    s.label,
                    s.pos.0,
                    s.pos.1,
                    durability,
                    home_tag
                ),
                i == selected,
            ),
            s.glyph,
            glyph_color(s.color),
        ));
    }
    draw_popup(
        "Demolish Structure",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}

pub(super) fn draw_remove_confirm(
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let rows = vec![
        Row::TextColored(
            "Removing Home destroys every other structure in this base and refunds".to_string(),
            ORANGE,
        ),
        Row::TextColored(
            "30% of each one's materials. This can't be undone.".to_string(),
            ORANGE,
        ),
        text_row(""),
        item_row("[y] Yes, demolish everything", selected == 0),
        item_row("[n] No, cancel", selected == 1),
    ];
    draw_popup(
        "Confirm Demolish Home",
        PopupSize::Small,
        &rows,
        refusal,
        painter,
        m,
    );
}

/// The structure roster: everything standing in the zone and every program
/// posted to it.
///
/// Read-only, and the one screen that shows the base as a whole rather than
/// what happens to be within `MENU_SCAN_RADIUS` — see
/// `Game::structure_report`, which is also where the row order is decided so
/// that this draws it rather than inventing one.
///
/// An idle workable structure is drawn in yellow and says so in words: it is
/// the only thing on this screen the player can act on, and the point of
/// looking is usually to find it.
pub(super) fn draw_structures(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let report = game.structure_report();
    let assigned: usize = report.iter().map(|s| s.assignees.len()).sum();
    let idle = report
        .iter()
        .filter(|s| s.workable && s.assignees.is_empty())
        .count();
    let (draw, supply) = game.base_power();
    let mut rows = vec![
        text_row(format!(
            "{} structure{}, {assigned} program{} assigned, {idle} idle",
            report.len(),
            if report.len() == 1 { "" } else { "s" },
            if assigned == 1 { "" } else { "s" },
        )),
        grid_header_row(draw, supply),
        text_row(""),
    ];
    if report.is_empty() {
        rows.push(text_row("You have deployed nothing yet."));
    }
    for (i, s) in report.iter().enumerate() {
        rows.push(colored_item_row(
            structure_headline(s),
            i == selected,
            if structure_is_idle(s) { YELLOW } else { TEXT },
        ));
        // A structure's sub-lines are `Row::Item` (never selected) rather than
        // `Row::Text` so they sit inside the popup's scrollable body:
        // `popup_layout` ends that body at the *last* Item and pins whatever
        // follows it as a footer, which would otherwise leave the final
        // structure's assignees stuck on screen while the list scrolled past
        // them.
        for (line, color) in structure_detail_lines(s) {
            rows.push(colored_item_row(line, false, color));
        }
    }
    rows.push(text_row(""));
    // Enter is surface-only, matching `App::handle_structures_key` — the
    // roster still reads underground, so the hint has to stop advertising a
    // key that would only be refused down there.
    rows.push(text_row(if game.is_underground() {
        "Up/Down to scroll, Esc to close."
    } else {
        "Up/Down to scroll, Enter to staff, Esc to close."
    }));
    draw_popup("Structures", PopupSize::Large, &rows, refusal, painter, m);
}

/// The roster's second header row: the base's grid, red when it is short.
///
/// Reads `draw` and `supply` straight from `Game::base_power` rather than
/// comparing any one machine's numbers — which machine went dark is
/// `MachineStatus::Unpowered`'s job, not this row's. **"Grid", never
/// "Power"**: `Power` already names a creature's `PowerReserve` in the status
/// column, and this pane sits two panels from it.
fn grid_header_row(draw: u32, supply: u32) -> Row {
    let text = format!("Grid  {draw} / {supply}");
    if draw > supply {
        Row::TextColored(text, RED)
    } else {
        text_row(text)
    }
}

/// A workable structure with nobody posted to it — the one thing on either
/// structure screen the player can immediately act on, which is why both
/// colour it yellow.
///
/// A call and not a second copy: `Game::attention` counts the same thing in
/// the engine, so the two readers are in different crates and nothing would
/// fail to compile if one drifted.
pub(super) fn structure_is_idle(s: &StructureReport) -> bool {
    s.is_idle()
}

/// The one-line summary of a structure: what it is, where, how far, and how
/// battered. Shared with the inspector's single-structure sheet.
pub(super) fn structure_headline(s: &StructureReport) -> String {
    let tier = s.tier.map(|t| format!(" T{t}")).unwrap_or_default();
    let durability = s
        .durability
        .map(|(hp, max)| format!("  {hp}/{max} HP"))
        .unwrap_or_default();
    format!(
        "{}{tier}  ({}, {})  {}d{durability}",
        s.label, s.pos.0, s.pos.1, s.distance
    )
}

/// Everything under the headline — idleness, assignees, a stall, the two
/// buffers — as `(text, colour)` pairs.
///
/// Extracted rather than written twice: the `B` roster and the inspector's
/// sheet describe the same machine, and a detail screen that disagreed with
/// the roster about whether something was starved is exactly the drift
/// `CLAUDE.md` means by a mirror having to be a call. The two differ only in
/// which `Row` kind they wrap these in — the roster needs `Row::Item` so its
/// lines scroll, the sheet does not scroll at all.
pub(super) fn structure_detail_lines(s: &StructureReport) -> Vec<(String, Color)> {
    let mut lines = Vec::new();
    if structure_is_idle(s) {
        lines.push(("  idle — nobody assigned".to_string(), YELLOW));
    }
    for a in &s.assignees {
        lines.push((format!("  {}", assignee_line(a)), TEXT_DIM));
    }
    // A stall is drawn in yellow for the same reason an idle structure is:
    // it is a thing the player can walk over and fix.
    if let Some(line) = stall_line(s) {
        lines.push((format!("  {line}"), YELLOW));
    }
    if let Some(line) = buffer_line("in", &s.input, None) {
        lines.push((line, TEXT_DIM));
    }
    if let Some(line) = buffer_line("out", &s.output, Some(s.output_capacity)) {
        lines.push((line, TEXT_DIM));
    }
    // A rig's standing tool. Drawn even when unset, and in yellow when it
    // is: a rig nobody has hand-loaded is the one that will not fetch from a
    // rack, and that is a thing the player can walk over and fix.
    if let Some(tool) = &s.standing_tool {
        lines.push((format!("  set up with the {tool}"), TEXT_DIM));
    }
    lines
}

/// Why a machine is stalled, or `None` when it is running or is not a
/// machine at all. `Idle` says nothing here — the "nobody assigned" line
/// already above it is the same fact in better words.
fn stall_line(s: &StructureReport) -> Option<&'static str> {
    match s.status? {
        MachineStatus::Starved => Some("starved — nothing is feeding it"),
        MachineStatus::Clogged => Some("clogged — collect from it with c"),
        MachineStatus::Unstaffed => Some("no one at it — its program is away"),
        MachineStatus::Stranded => Some("cut off — its program can't reach it"),
        // Says the fact and stops. It used to end "build a Recharger Node",
        // which is the wrong move on the base most likely to be reading this
        // line: one whose Rechargers are all standing there `Dry`. A fifth
        // would go dry beside them. The grid figure on the status bar is
        // where the player reads how short, and a dry supplier says so in its
        // own words.
        MachineStatus::Unpowered => Some("dark — the grid is short"),
        MachineStatus::Dry => Some("out of fuel — no Power Cells beside it"),
        MachineStatus::Running | MachineStatus::Idle => None,
    }
}

/// One buffer as a line, or `None` when it is empty — a base of empty
/// buffers would otherwise double the length of this screen to say nothing.
fn buffer_line(label: &str, stock: &[(String, u32)], capacity: Option<u32>) -> Option<String> {
    if stock.is_empty() {
        return None;
    }
    let contents = stock
        .iter()
        .map(|(name, n)| format!("{n} {name}"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(match capacity {
        Some(cap) => {
            let used: u32 = stock.iter().map(|(_, n)| n).sum();
            format!("  {label}: {contents}  [{used}/{cap}]")
        }
        None => format!("  {label}: {contents}"),
    })
}

/// One assignee row: who it is, how it is holding up, what it is doing, and
/// how far into a cycle it is. A guard has no cycle to be partway through —
/// `systems::task_progress_system` ignores the kind entirely — so it gets no
/// progress figure rather than a permanent `0/0`.
///
/// The vitals are here because this is the only screen that can carry them
/// for a posted program: at its post it is not drawn on the map, and the
/// inspector names only what is drawn, so there is no way to open its own
/// manifest without first calling it off the job.
fn assignee_line(a: &Assignee) -> String {
    let who = format!("{}{}", a.label, assignee_vitals(a));
    match a.kind {
        TaskKind::GatherResource => format!("{who} — cronjob {}/{}", a.progress, a.required),
        TaskKind::Guard => format!("{who} — guarding"),
        // A dig site is not a structure, so nothing on this screen can be
        // holding an `Excavate` post — the row exists because the match is
        // exhaustive, and it reads like the cronjob one for the day
        // something does.
        TaskKind::Excavate => format!("{who} — cutting {}/{}", a.progress, a.required),
        // A build site is not a structure either, so this row is unreachable
        // for the same reason the one above it is. It reads like the cronjob
        // one for the day something puts a builder on this screen.
        TaskKind::Construct => format!("{who} — building {}/{}", a.progress, a.required),
    }
}

/// `" Lv7 HP 18/22"`, or empty for anything missing the components. Matching
/// the party roster's `Lv{n} HP {a}/{b}` so one program reads the same on
/// both screens.
fn assignee_vitals(a: &Assignee) -> String {
    let level = a.level.map(|l| format!(" Lv{l}")).unwrap_or_default();
    let hp =
        a.hp.map(|(hp, max)| format!(" HP {hp}/{max}"))
            .unwrap_or_default();
    format!("{level}{hp}")
}

/// What the base has made — the base menu's "Base output" row.
///
/// **Every figure comes out of `Game::base_output_report`**,
/// `draw_companion_memories`' rule: the sector/run split, which section an
/// item belongs in and which rows survive the cap are all decided in the
/// engine, so this page cannot disagree with the ledger a retune was done
/// from. `Game::attention` is reached the same way — through the report,
/// which *calls* it rather than restating what needs the player.
///
/// The page does not scroll — `draw_popup` pages a `Row::Item` span and
/// there are none here — so its height is held by
/// `the_tallest_base_output_page_fits_its_popup` rather than by a scrollbar.
pub(super) fn draw_base_output(
    game: &mut Game,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let report = game.base_output_report();
    let rows = base_output_rows(&report);
    draw_popup("Base Output", PopupSize::Large, &rows, refusal, painter, m);
}

/// How wide the item column is. A name longer than this is cut rather than
/// allowed to push the four figures out of their columns: the page is read
/// down a column, and one modded name would otherwise skew every row under
/// it.
const OUTPUT_NAME_COLUMN: usize = 22;

/// The page's rows, out of the report rather than out of a `Game` — the
/// split `memory_page_rows` makes, and for its reason: the height and width
/// censuses have to measure the page at its **worst** case, which is a
/// report a fixture can state and a `Game` would have to be played into.
pub(super) fn base_output_rows(report: &BaseOutputReport) -> Vec<Row> {
    let mut rows = vec![Row::TextColored(
        format!("sector {} · this run", report.zone),
        CYAN,
    )];

    if report.mined.is_empty() && report.compiled.is_empty() {
        rows.push(text_row("The base has not made anything yet."));
    } else {
        rows.push(Row::TextColored(
            format!(
                "  {:<width$}{:>8}{:>9}{:>8}{:>8}",
                "",
                "sector",
                "machine",
                "hand",
                "run",
                width = OUTPUT_NAME_COLUMN,
            ),
            TEXT_DIM,
        ));
        // Both sections carry the same four figures rather than the sector/run
        // pair the design sketch gave MINED alone. An item sits in one
        // section on *dominant* provenance, so a Power Cell a machine makes
        // more of than the player does lands under MINED — and dropping the
        // hand column there would hide exactly the units the page exists to
        // count.
        for (heading, section) in [("MINED", &report.mined), ("COMPILED", &report.compiled)] {
            if section.is_empty() {
                continue;
            }
            rows.push(Row::TextColored(heading.to_string(), YELLOW));
            for row in section.iter() {
                rows.push(text_row(base_output_line(row)));
            }
        }
    }

    // Every row `Game::attention` holds, never a slice of them: it caps
    // itself at four by construction, and the page's row budget is set
    // against that — a page that dropped the fourth would be silent about
    // exactly the state the player opened it to act on.
    rows.push(text_row(""));
    for row in &report.attention {
        rows.push(Row::TextColored(
            format!("needs attention: {}", row.text),
            if row.threat { RED } else { YELLOW },
        ));
    }

    // The sentence and the key share a row because rows are what this page
    // is short of. The sentence itself is the one thing this page is better
    // placed to teach than any other screen: without it, a sector total that
    // grew while the party was underground reads as a counting bug.
    rows.push(text_row(
        "The base works while you are away — the sector total counts that time. Esc to go back.",
    ));
    rows
}

/// One item's line: the name, the four figures, and the shape of the last
/// few windows.
fn base_output_line(row: &BaseOutputRow) -> String {
    let name = if row.name.chars().count() > OUTPUT_NAME_COLUMN {
        let mut cut: String = row.name.chars().take(OUTPUT_NAME_COLUMN - 1).collect();
        cut.push('…');
        cut
    } else {
        row.name.clone()
    };
    format!(
        "  {:<width$}{:>8}{:>9}{:>8}{:>8}  {}",
        name,
        row.sector,
        row.machine,
        row.hand,
        row.run,
        spark_line(&row.spark),
        width = OUTPUT_NAME_COLUMN,
    )
}

/// The eight block glyphs a sparkline is drawn from, shortest first.
///
/// U+2581..U+2588, which DejaVu Sans Mono — the UI face, and the one every
/// popup row is measured in — carries; the map's unscii face is never
/// reached from a popup.
const SPARK_BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A row's recent windows as bars, scaled to **that row's own** peak.
///
/// Per row rather than across the page: a sparkline beside a four-figure
/// total is being read for a shape — is this climbing or falling — and
/// scaling every row to the busiest item on the page flattens each of the
/// slower ones into a single flat line that says nothing.
///
/// The windows are the ones the ledger *kept*: a bucket with no events is
/// never stored, so a quiet stretch closes up rather than drawing as a gap.
fn spark_line(values: &[u32]) -> String {
    let max = values.iter().copied().max().unwrap_or(0);
    values
        .iter()
        .map(|v| {
            if max == 0 {
                SPARK_BARS[0]
            } else {
                SPARK_BARS[(*v as usize * (SPARK_BARS.len() - 1)) / max as usize]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::structures::StructureDb;

    pub(super) fn view(tier: u32, ceiling: u32, max_tier: u32) -> EntityView {
        EntityView {
            entity: Entity::PLACEHOLDER,
            pos: (0, 0),
            glyph: 'n',
            sprite: None,
            color: GlyphColor::White,
            label: "Mining Node".into(),
            is_player: false,
            look: None,
            is_tamed: false,
            is_companion: false,
            is_hostile: false,
            is_structure: true,
            is_anchor: false,
            is_home: false,
            tier: Some(tier),
            ceiling: Some(ceiling),
            max_tier: Some(max_tier),
            is_boss: false,
            nemesis: false,
            patrol: None,
            difficulty: None,
            can_work: false,
            can_trade: false,
            issues_contracts: false,
            structure_worker: None,
            wears_job_mark: false,
            position_is_honest: true,
            structure_attended: false,
            recovering: false,
            build: None,
            output_stranded: false,
            hp_fraction: None,
            level: None,
            durability: None,
            fusions: 0,
            rarity: Rarity::Ordinary,
            machine_status: None,
            linked_edges: Vec::new(),
        }
    }

    fn row_text(row: &Row) -> &str {
        match row {
            Row::Text(t) | Row::TextColored(t, _) => t,
            Row::Item { text, .. } => text,
        }
    }

    /// Every structure the game ships, as the deploy menu lists it. Read off
    /// `assets/structures/` rather than hand-written because how long a
    /// description runs is a property of the content: one authored longer
    /// tomorrow has to clear the same width, and a fixture quoting today's
    /// worst case would stop testing the wrap the moment the assets moved.
    fn shipped_entries() -> Vec<BuildEntry> {
        let dir = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/structures"
        ));
        let (db, _) = StructureDb::load_dir(dir).expect("the shipped structures load");
        db.all()
            .map(|def| BuildEntry {
                name: def.name.clone(),
                cost: build_cost_label(&[]),
                deployed: WIDEST_DEPLOYED_TAG,
                description: def.description.clone(),
                category: def.category(),
                needs_program: def.needs_program(),
            })
            .collect()
    }

    /// The shipped description that runs longest, which is the one both
    /// deploy screens have to survive.
    fn widest_shipped_description() -> String {
        shipped_entries()
            .into_iter()
            .map(|e| e.description)
            .max_by_key(|d| d.chars().count())
            .expect("the shipped assets define structures")
    }

    /// The deployed count the width fixtures carry. Nothing caps how many of
    /// an uncapped structure a base may stand, so the tag is measured at the
    /// widest figure a grid could plausibly reach rather than at the shipped
    /// `max_deployed` ceilings — which are 1 and 3, and would leave the row
    /// two characters narrower than the one a player can actually build to.
    const WIDEST_DEPLOYED_TAG: u32 = 99;

    /// A description longer than anything authored, so the wrap is what is
    /// tested rather than the assets happening to be short enough.
    fn synthetic_description() -> String {
        "Recompiles damaged structures across the whole base, itself included, ".repeat(6)
    }

    /// A row says how many of that structure the base already counts, and
    /// says nothing at all when the answer is none.
    ///
    /// The tag sits between the name and the cost rather than after it: a
    /// cost line already reads `Bytecode Block (12/20)`, so `(3)` on the end
    /// of one is a fourth cost fragment rather than a count.
    #[test]
    fn a_deploy_row_is_tagged_with_how_many_already_stand() {
        let entries = vec![
            BuildEntry {
                name: "Mining Node".to_string(),
                cost: "12 Core Fragments".to_string(),
                deployed: 3,
                description: "Cuts ore.".to_string(),
                category: StructureCategory::Extractor,
                needs_program: true,
            },
            BuildEntry {
                name: "Lathe".to_string(),
                cost: "12 Core Fragments".to_string(),
                deployed: 0,
                description: "Turns parts.".to_string(),
                category: StructureCategory::Extractor,
                needs_program: true,
            },
        ];
        let rows = build_menu_rows(&entries, 0, None);
        let texts: Vec<&str> = rows.iter().map(row_text).collect();

        assert!(
            texts
                .iter()
                .any(|t| t.contains("Mining Node (3) - 12 Core Fragments")),
            "a row with three standing says so: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|t| t.contains("Lathe - 12 Core Fragments")),
            "and a row with none is left alone: {texts:?}"
        );
        assert!(
            !texts.iter().any(|t| t.contains("(0)")),
            "a fresh base is not a menu of zeroes: {texts:?}"
        );
    }

    /// `draw_row` clamps a row vertically and **never horizontally**, so a
    /// structure's authored description — up to about 300 characters against
    /// a `PopupSize::Large` body of roughly 114 — was drawn straight off the
    /// right edge of the deploy menu in silence.
    #[test]
    fn no_deploy_menu_row_runs_past_the_popup_body() {
        let mut entries = shipped_entries();
        entries.push(BuildEntry {
            name: "Overlong Node".to_string(),
            cost: build_cost_label(&[]),
            deployed: WIDEST_DEPLOYED_TAG,
            description: synthetic_description(),
            category: StructureCategory::Utility,
            needs_program: true,
        });
        // All three forms: each greyed one carries a line the deployable one
        // does not, and an unwrapped sentence runs off the edge just as
        // silently. Both shortfall sentences, because they are different
        // lengths and only the longer of the two can be the one that
        // overflows.
        for shortfall in [
            None,
            Some(DEPLOY_NEEDS_A_PROGRAM),
            Some(DEPLOY_NEEDS_A_SECOND_PROGRAM),
        ] {
            for row in build_menu_rows(&entries, 0, shortfall) {
                let text = row_text(&row);
                assert!(
                    text.chars().count() <= ROW_WRAP_COLUMNS,
                    "a {} char deploy row runs past the {ROW_WRAP_COLUMNS} column body: {text:?}",
                    text.chars().count()
                );
            }
        }
    }

    /// Wrapping a description costs rows, and `popup_layout` ends the
    /// scrollable body at the *last* `Row::Item` — so a continuation line
    /// emitted as `Row::Text` would strand the final structure's description
    /// under the scroll indicator, detached from the row it describes. The
    /// rule `build_menu_rows` documents, asserted over the real assets.
    #[test]
    fn every_deploy_description_stays_inside_the_scrollable_body() {
        let entries = shipped_entries();
        for (selected, shortfall) in [
            (0, None),
            (entries.len() - 1, None),
            (0, Some(DEPLOY_NEEDS_A_PROGRAM)),
            (0, Some(DEPLOY_NEEDS_A_SECOND_PROGRAM)),
        ] {
            let rows = build_menu_rows(&entries, selected, shortfall);
            let last_item = rows
                .iter()
                .rposition(|r| matches!(r, Row::Item { .. }))
                .expect("a structure is an item row");
            assert_eq!(
                last_item,
                rows.len() - 1,
                "the deploy menu pinned {} rows below its list — a description \
                 is detached from the structure it belongs to",
                rows.len() - last_item - 1
            );
        }
    }

    /// The deploy prompt draws the same description in a box with no scroll
    /// at all — every row is `Row::Text`, so `popup_layout` pins the lot —
    /// which is the other half of the same fault.
    #[test]
    fn no_deploy_prompt_row_runs_past_the_popup_body() {
        for description in [widest_shipped_description(), synthetic_description()] {
            let rows = build_direction_rows(
                "Fabricator",
                &description,
                &["Bytecode Block (12/20)".to_string()],
            );
            for row in &rows {
                let text = row_text(row);
                assert!(
                    text.chars().count() <= ROW_WRAP_COLUMNS,
                    "a {} char deploy prompt row runs past the {ROW_WRAP_COLUMNS} column body: {text:?}",
                    text.chars().count()
                );
            }
            // Wrapped, not truncated: the prompt is the last thing the player
            // reads before placing the structure, so a description that lost
            // its tail would be worse than one that ran off the edge.
            let drawn: Vec<&str> = rows
                .iter()
                .flat_map(|r| row_text(r).split_whitespace())
                .collect();
            for word in description.split_whitespace() {
                assert!(drawn.contains(&word), "the wrap dropped {word:?}");
            }
        }
    }

    /// The prompt has no scroll, so a description tall enough to outgrow the
    /// box loses its tail in silence — `the_tallest_memory_page_fits_its_popup`'s
    /// trap, in the screen wrapping just moved rows into.
    ///
    /// Swept across window heights rather than measured at one: `ui_metrics`
    /// clamps the font at both ends, so below the clamp the box keeps
    /// shrinking while the line height stops.
    #[test]
    fn the_tallest_deploy_prompt_fits_its_popup() {
        let rows = build_direction_rows(
            "Fabricator",
            &widest_shipped_description(),
            &["Bytecode Block (12/20)".to_string()],
        )
        .len();
        for h in (600..=2160).step_by(60) {
            let m = crate::text::ui_metrics(h as f32);
            let cap = popup_max_rows(h as f32, PopupSize::Large, &m);
            assert!(
                rows + REFUSAL_MAX_LINES <= cap,
                "the deploy prompt builds a {rows}-row page into a {cap}-row popup at {h}px"
            );
        }
    }

    /// The other axis, in pixels rather than columns: `ROW_WRAP_COLUMNS` is
    /// a proxy for the real box, and this is the measurement it stands in
    /// for. Both deploy screens are `PopupSize::Large`.
    #[test]
    fn no_deploy_row_overflows_its_popup_in_pixels() {
        let mut entries = shipped_entries();
        entries.push(BuildEntry {
            name: "Overlong Node".to_string(),
            cost: build_cost_label(&[]),
            deployed: WIDEST_DEPLOYED_TAG,
            description: synthetic_description(),
            category: StructureCategory::Utility,
            needs_program: true,
        });
        let menu = build_menu_rows(&entries, 0, None);
        let greyed = build_menu_rows(&entries, 0, Some(DEPLOY_NEEDS_A_PROGRAM));
        let floored = build_menu_rows(&entries, 0, Some(DEPLOY_NEEDS_A_SECOND_PROGRAM));
        let prompt = build_direction_rows(
            "Fabricator",
            &widest_shipped_description(),
            &["Bytecode Block (12/20)".to_string()],
        );
        crate::paint::with_painter(|p| {
            let m = crate::text::ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for row in menu
                .iter()
                .chain(greyed.iter())
                .chain(floored.iter())
                .chain(prompt.iter())
            {
                let text = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t.clone(),
                    // `draw_row`'s own prefix on an item row.
                    Row::Item { text, .. } => format!("     {text}"),
                };
                let drawn = p.measure_ui_advance(&text, m.font_size);
                assert!(
                    drawn <= room,
                    "a deploy row overflows its popup by {:.0}px ({drawn:.0} into {room:.0}):\n{text}",
                    drawn - room
                );
            }
        });
    }

    /// The deploy prompt used to be a bare compass, which meant the one screen
    /// where a structure is actually placed was also the one that never said
    /// what was being placed. Both halves matter: the identity, and the keys
    /// that act on it.
    #[test]
    fn the_deploy_prompt_names_what_is_being_placed_and_still_says_how() {
        let rows = build_direction_rows(
            "Mining Node",
            "Extracts Core Fragments on a timer.",
            &["Core Fragment (5/12)".to_string()],
        );
        let text: Vec<&str> = rows.iter().map(row_text).collect();
        assert!(text.contains(&"Mining Node"), "{text:?}");
        assert!(
            text.contains(&"Extracts Core Fragments on a timer."),
            "{text:?}"
        );
        assert!(
            text.iter().any(|t| t.contains("Core Fragment (5/12)")),
            "the cost carries the have/need figures the refusal would otherwise be the first news of: {text:?}"
        );
        assert!(text.contains(&DIRECTION_PROMPT), "{text:?}");
    }

    /// A waived bill has no rows to join, and both screens that quote one
    /// have to say so in words. Left to `join`, the menu row trailed off
    /// after its dash and the prompt read "Costs " — which is what a screen
    /// that failed to load a price looks like, not a free structure.
    #[test]
    fn a_waived_bill_reads_as_free_on_both_screens_that_quote_it() {
        assert_eq!(build_cost_label(&[]), "free");
        assert_eq!(
            build_cost_label(&["Core Fragment (5/12)".to_string()]),
            "Core Fragment (5/12)",
            "and a bill that exists is still quoted in full"
        );

        let rows = build_direction_rows("Contract Broker", "Posts work.", &[]);
        let text: Vec<&str> = rows.iter().map(row_text).collect();
        assert!(
            text.contains(&"Free to deploy"),
            "the deploy prompt says it in a sentence rather than quoting an empty bill: {text:?}"
        );
        assert!(
            !text.iter().any(|t| t.starts_with("Costs")),
            "and does not also quote a cost: {text:?}"
        );
    }

    /// The colour of each *structure* row, in entry order — the headings and
    /// the wrapped descriptions are `Row::Item` too (they have to be, see
    /// `build_menu_rows`), and only the lines carrying a shortcut stand for
    /// something the player picks.
    fn entry_colors(rows: &[Row]) -> Vec<Color> {
        rows.iter()
            .filter_map(|r| match r {
                Row::Item { text, color, .. } if text.starts_with('[') => Some(*color),
                _ => None,
            })
            .collect()
    }

    /// A roster with nothing free to spend stops **every** deploy but Home,
    /// so the menu says it once and greys the rest — rather than tagging
    /// each row with a requirement that is the same on all of them.
    ///
    /// Grey, never hide: the rows are still there and still pickable. A
    /// structure that vanished from the catalogue reads as a bug, and the
    /// pick lands on the picker, which says the same thing at length.
    #[test]
    fn a_roster_with_no_free_program_greys_the_deploy_menu() {
        let entries = shipped_entries();

        let open = build_menu_rows(&entries, 0, None);
        assert!(
            !open.iter().any(|r| row_text(r) == DEPLOY_NEEDS_A_PROGRAM),
            "a menu that can be used says nothing about a cost it can meet"
        );

        let shut = build_menu_rows(&entries, 0, Some(DEPLOY_NEEDS_A_PROGRAM));
        assert_eq!(
            shut.iter()
                .filter(|r| row_text(r) == DEPLOY_NEEDS_A_PROGRAM)
                .count(),
            1,
            "once for the screen, not once per row"
        );
        assert!(
            matches!(shut.first(), Some(Row::Text(_))),
            "the Esc line still leads, so the warning sits under it and above the list"
        );
        assert_eq!(
            shut.iter()
                .filter(|r| matches!(r, Row::Item { .. }))
                .count(),
            open.iter()
                .filter(|r| matches!(r, Row::Item { .. }))
                .count(),
            "nothing was hidden, so no row a keypress resolves to has moved"
        );

        let colors = entry_colors(&shut);
        assert_eq!(colors.len(), entries.len(), "one row per structure");
        for (entry, color) in entries.iter().zip(&colors) {
            assert_eq!(
                *color,
                if entry.needs_program { TEXT_DIM } else { TEXT },
                "{} reads wrong on a roster that can pay for nothing",
                entry.name
            );
        }
        assert!(
            entry_colors(&open).iter().all(|c| *c == TEXT),
            "and a usable menu is not dim anywhere"
        );
    }

    /// **Home never greys.** It is the one structure the engine waives the
    /// program cost for, and it is the one a fresh run — which owns zero
    /// programs by definition — has to be able to found. Greyed with the
    /// rest, the very first screen of a new game would be a menu of dim rows
    /// telling the player they cannot afford the thing they are about to do.
    #[test]
    fn home_stays_lit_on_a_roster_that_can_pay_for_nothing_else() {
        let entries = shipped_entries();
        let home = entries
            .iter()
            .position(|e| e.category == StructureCategory::Home)
            .expect("the shipped assets define a Home");
        assert_eq!(
            entry_colors(&build_menu_rows(&entries, 0, Some(DEPLOY_NEEDS_A_PROGRAM)))[home],
            TEXT
        );
        // And under the *other* shortfall too: the roster floor greys the
        // same rows for a different reason, and Home is exempt from the
        // program cost outright (`Game::structure_needs_program`), so a run
        // whose one program cannot be spent must still be able to found one.
        assert_eq!(
            entry_colors(&build_menu_rows(
                &entries,
                0,
                Some(DEPLOY_NEEDS_A_SECOND_PROGRAM)
            ))[home],
            TEXT
        );
    }

    /// **Nor does a Depot.** A storing structure costs no program either
    /// (`StructureDef::needs_program`), and a shelf is exactly what a base
    /// whose roster has nothing free still wants to be able to put up — so
    /// greying it would dim the one row on the screen that is still worth
    /// picking.
    ///
    /// Asserted over every shipped structure the engine says is exempt
    /// rather than over the id `"depot"`: the ladder runs to Mk6, the Zone
    /// Portal is exempt too, and a seventh shelf is a file rather than a
    /// code change.
    #[test]
    fn a_storing_structure_stays_lit_on_a_roster_that_can_pay_for_nothing() {
        let entries = shipped_entries();
        let colors = entry_colors(&build_menu_rows(&entries, 0, Some(DEPLOY_NEEDS_A_PROGRAM)));
        let shelves: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.needs_program && e.category != StructureCategory::Home)
            .map(|(i, _)| i)
            .collect();
        assert!(
            !shelves.is_empty(),
            "the shipped assets define at least one storing structure"
        );
        for i in shelves {
            assert_eq!(
                colors[i], TEXT,
                "{} costs no program, so a roster shortfall is not about it",
                entries[i].name
            );
        }
    }

    fn assignee(kind: TaskKind) -> Assignee {
        Assignee {
            entity: Entity::PLACEHOLDER,
            label: "Sub-Process (Z9)".into(),
            kind,
            progress: 999,
            required: 999,
            level: Some(99),
            hp: Some((999, 999)),
        }
    }

    #[test]
    fn an_assignee_row_reads_the_way_the_party_roster_does() {
        assert_eq!(
            assignee_line(&assignee(TaskKind::GatherResource)),
            "Sub-Process (Z9) Lv99 HP 999/999 — cronjob 999/999"
        );
        assert_eq!(
            assignee_line(&assignee(TaskKind::Guard)),
            "Sub-Process (Z9) Lv99 HP 999/999 — guarding"
        );
    }

    /// A program without the components still gets a row rather than a line
    /// full of placeholder figures.
    #[test]
    fn an_assignee_with_no_stats_reads_as_it_did_before_the_vitals() {
        let bare = Assignee {
            level: None,
            hp: None,
            ..assignee(TaskKind::Guard)
        };
        assert_eq!(assignee_line(&bare), "Sub-Process (Z9) — guarding");
    }

    /// The structure sheet is a `PopupSize::Small`, which is half the window
    /// wide — and `draw_row` clamps rows vertically but never horizontally,
    /// so a line too long for the box runs off its edge rather than wrapping
    /// or being cut. Adding the vitals lengthened every assignee row, so the
    /// worst case is measured against the real box rather than eyeballed.
    #[test]
    fn the_longest_assignee_row_fits_the_structure_sheet() {
        let m = crate::text::ui_metrics(900.0);
        let line = format!("  {}", assignee_line(&assignee(TaskKind::GatherResource)));
        crate::paint::with_painter(|p| {
            let box_w = p.screen_w() * 0.5;
            let text_w = p.measure_ui_advance(&line, m.font_size);
            assert!(
                text_w + 2.0 * m.pad < box_w,
                "an assignee row is {text_w}px inside a {box_w}px sheet: {line:?}"
            );
        });
    }

    /// The roster's second header row: what it says, and — the actual point
    /// of the row — that it goes red exactly when the grid can't cover its
    /// machines. A flush grid stays a plain row so red keeps meaning "short".
    #[test]
    fn the_roster_header_reports_the_grid() {
        let short = grid_header_row(15, 12);
        assert_eq!(row_text(&short), "Grid  15 / 12");
        match short {
            Row::TextColored(_, color) => {
                assert_eq!(color, RED, "a grid short of supply should read red")
            }
            Row::Text(_) | Row::Item { .. } => panic!("a grid short of supply should read red"),
        }

        let flush = grid_header_row(12, 15);
        assert_eq!(row_text(&flush), "Grid  12 / 15");
        assert!(
            matches!(flush, Row::Text(_)),
            "a grid with supply to spare should not read red"
        );
    }

    fn structure_report(status: MachineStatus) -> StructureReport {
        StructureReport {
            entity: Entity::PLACEHOLDER,
            kind: "mining_node".to_string(),
            label: "Mining Node".to_string(),
            pos: (0, 0),
            distance: 0,
            tier: None,
            durability: None,
            is_home: false,
            workable: true,
            player_adjacent: false,
            input: Vec::new(),
            output: Vec::new(),
            output_capacity: 0,
            status: Some(status),
            assignees: Vec::new(),
            standing_tool: None,
        }
    }

    /// The two grid stalls, and the difference between them is the whole
    /// point of the pair.
    ///
    /// This line used to end "build a Recharger Node", on the reading that
    /// `Unpowered` is the one status whose fix is a build. That advice is
    /// wrong on the base most likely to be reading it: one whose Rechargers
    /// are all standing there `Dry`, where a fifth would go dry beside them.
    /// The line now states the fact and stops, and the *cause* says what to
    /// do about itself.
    ///
    /// Asserted as an absence as well as an equality, because the equality
    /// alone passes against any rewrite at all — including one that puts the
    /// build advice back in different words.
    #[test]
    fn a_dark_machine_says_the_grid_is_short_without_prescribing_a_build() {
        let dark = stall_line(&structure_report(MachineStatus::Unpowered));
        assert_eq!(dark, Some("dark — the grid is short"));
        assert!(
            !dark.unwrap().contains("build"),
            "a base whose Rechargers are all dry does not need a fifth one"
        );
    }

    /// The cause, and the line the inspector's sheet draws when `i` finds a
    /// dry Recharger Node — `draw_structure_manifest` calls this builder
    /// rather than restating it, so the roster and the sheet cannot drift.
    ///
    /// It names the fix, which is what splitting `Dry` off `Starved` bought:
    /// a supplier has no input buffer and no program, so "nothing is feeding
    /// it" sent the player looking for two things that do not exist on it.
    #[test]
    fn a_dry_supplier_says_it_is_out_of_fuel_and_where_fuel_goes() {
        let dry = stall_line(&structure_report(MachineStatus::Dry));
        assert_eq!(dry, Some("out of fuel — no Power Cells beside it"));
        assert_ne!(
            dry,
            stall_line(&structure_report(MachineStatus::Starved)),
            "sharing the sentence is what made this unreadable"
        );
    }

    /// `draw_structures` is a `PopupSize::Large` box and, per the
    /// `popup row width IS testable headlessly` memory, `draw_row` never
    /// clips a row horizontally — an overlong row just runs off the edge.
    /// The grid header is short and fixed, so this confirms that rather
    /// than assuming it: worst-case four-digit numbers on both sides still
    /// fit comfortably inside the roster.
    #[test]
    fn the_grid_header_row_fits_the_structure_roster() {
        let m = crate::text::ui_metrics(900.0);
        let row = grid_header_row(9999, 9999);
        let line = row_text(&row);
        crate::paint::with_painter(|p| {
            let box_w = p.screen_w() * 0.88;
            let text_w = p.measure_ui_advance(line, m.font_size);
            assert!(
                text_w + 2.0 * m.pad < box_w,
                "the grid header is {text_w}px inside an {box_w}px roster: {line:?}"
            );
        });
    }
}

/// The program picker: the screen that spends a tamed program on a build
/// order, and the one keypress in the base loop that deletes a program from
/// the roster.
#[cfg(test)]
mod build_program_tests {
    use super::*;
    use crate::render::test_pet;

    fn row_text(row: &Row) -> &str {
        match row {
            Row::Text(t) | Row::TextColored(t, _) => t,
            Row::Item { text, .. } => text,
        }
    }

    fn deploy(label: &str) -> BuildCommit {
        BuildCommit {
            label: label.to_string(),
            to_tier: None,
            structure: "fabricator".to_string(),
            cost: Vec::new(),
        }
    }

    fn upgrade(label: &str, to_tier: u32) -> BuildCommit {
        upgrade_costing(label, to_tier, Vec::new())
    }

    /// The same, with the bill the crew would fetch — the figure the list
    /// this screen replaced used to carry per row.
    fn upgrade_costing(label: &str, to_tier: u32, cost: Vec<String>) -> BuildCommit {
        BuildCommit {
            label: label.to_string(),
            to_tier: Some(to_tier),
            structure: "mining_node".to_string(),
            cost,
        }
    }

    /// A candidate whose machine really does have a rate to change.
    fn candidate(name: &str, effect: BuildEffect) -> BuildCandidate {
        BuildCandidate {
            pet: test_pet(name, "w|a|m"),
            aptitude: "Assembly",
            label: "Excellent",
            effect,
        }
    }

    fn cycling(name: &str) -> BuildCandidate {
        candidate(
            name,
            BuildEffect::Cycle {
                shipped: 20,
                built: 18,
            },
        )
    }

    /// The tier is the engine's own answer, not a restated copy: this is the
    /// number `App::handle_build_program_key` hands `programs_for_build`, so
    /// a second rule here would offer a program the engine then refuses.
    #[test]
    fn the_picker_asks_the_engine_how_deep_a_program_it_needs() {
        assert_eq!(deploy("Fabricator").tier(), 1, "every deploy is tier 1");
        assert_eq!(upgrade("Mining Node", 4).tier(), 4);
    }

    /// The prompt names what is being paid for and says what paying costs.
    /// **"Permanently" is the whole point of the sentence** — this keypress
    /// deletes a program, and a picker that read like an equipment menu
    /// would be inviting it.
    #[test]
    fn the_picker_names_the_structure_and_says_the_cost_is_permanent() {
        let rows = build_program_rows(Some(&deploy("Fabricator")), &[cycling("Sparkgrub")], 0);
        let text = rows.iter().map(row_text).collect::<Vec<_>>().join(" ");
        assert!(text.contains("Fabricator"), "{text}");
        assert!(text.contains("permanently"), "{text}");
    }

    /// An upgrade names the tier it is buying, because that is what decides
    /// how deep a program it costs — "Mining Node" alone would leave the
    /// player reading a zone requirement with nothing to attach it to.
    #[test]
    fn an_upgrade_prompt_names_the_tier_the_program_is_buying() {
        let rows = build_program_rows(Some(&upgrade("Mining Node", 3)), &[], 0);
        let text = rows.iter().map(row_text).collect::<Vec<_>>().join(" ");
        assert!(text.contains("Mining Node"), "{text}");
        assert!(text.contains("Mk3"), "{text}");
        assert!(text.contains("permanently"), "{text}");
    }

    /// The refund is real — `return_build_holdings` hands the program back
    /// through either destruction door while the order is still a build
    /// request — so the prompt says how long the take-back lasts rather than
    /// claiming the program never comes back.
    #[test]
    fn the_prompt_says_the_spend_is_final_only_once_the_structure_stands() {
        let text = deploy("Fabricator").prompt();
        assert!(text.contains("comes back"), "{text}");
        assert!(text.contains("stands"), "{text}");
    }

    /// The roster's own row format, not a bare list of names: this decision
    /// is which program to delete, so it wants the stat line the party
    /// screen shows.
    #[test]
    fn the_picker_draws_each_candidate_as_the_roster_draws_it() {
        let pets = [cycling("Sparkgrub"), cycling("Nibbler")];
        let rows = build_program_rows(Some(&deploy("Fabricator")), &pets, 0);
        let text = rows.iter().map(row_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("HP 22/28"), "the roster's stat line: {text}");
        assert!(text.contains("Sparkgrub"), "{text}");
        assert!(text.contains("Nibbler"), "{text}");
    }

    /// **The renderer draws what app-core counts, and never filters it a
    /// second time.** `App::handle_build_program_key` resolves a keypress
    /// against `programs_for_build`'s list by index, so a row dropped or
    /// reordered here would spend the program on the row below the one the
    /// player read.
    #[test]
    fn every_candidate_gets_a_row_in_the_order_it_arrived() {
        let pets: Vec<BuildCandidate> = ["Sparkgrub", "Nibbler", "Grinder"]
            .into_iter()
            .map(cycling)
            .collect();
        let rows = build_program_rows(Some(&deploy("Fabricator")), &pets, 0);
        let heads: Vec<&str> = rows
            .iter()
            .filter(|r| matches!(r, Row::Item { .. }))
            .map(row_text)
            .filter(|t| t.starts_with('['))
            .collect();
        assert_eq!(
            heads.len(),
            pets.len(),
            "one shortcut per candidate: {heads:?}"
        );
        for (i, pet) in pets.iter().enumerate() {
            let want = format!("[{}]", menu_shortcut(i));
            assert!(
                heads[i].starts_with(&want) && heads[i].contains(&pet.pet.name),
                "row {i} is {:?}, not {want} {}",
                heads[i],
                pet.pet.name
            );
        }
    }

    /// The quote is two whole tick figures and no percent sign — see
    /// `views::BuildEffect` for why a percentage would be wrong on a short
    /// cycle.
    #[test]
    fn a_picker_row_quotes_the_cycle_in_ticks() {
        let rows = build_program_rows(Some(&deploy("Fabricator")), &[cycling("Sparkgrub")], 0);
        let text = rows.iter().map(row_text).collect::<Vec<_>>().join("\n");
        assert!(
            text.contains("cycle 20 -> 18 ticks"),
            "both figures are not quoted: {text}"
        );
        assert!(
            text.contains("[Assembly: Excellent]"),
            "the aptitude tag is missing: {text}"
        );
        // The stat line's own `MIT 5%` is not the quote, so the assertion is
        // on the tag: a percentage *of the change* is the reading
        // `BuildEffect` exists to refuse.
        let quote = text
            .split("[Fabricator cycle")
            .nth(1)
            .expect("the cycle tag is drawn");
        assert!(
            !quote.split(']').next().unwrap_or_default().contains('%'),
            "the cycle quote is a percentage: {quote}"
        );
    }

    /// A build with no cycle says so once, above the list, and no row
    /// carries a cycle tag — `Cycle { shipped: n, built: n }` is the right
    /// answer for a cycle too short to move and the wrong one here.
    #[test]
    fn a_no_cycle_build_says_so_once_above_the_list() {
        let candidates: Vec<BuildCandidate> = ["Sparkgrub", "Nibbler", "Grinder"]
            .into_iter()
            .map(|n| candidate(n, BuildEffect::NoCycle))
            .collect();
        let rows = build_program_rows(Some(&deploy("Depot")), &candidates, 0);

        let joined = rows.iter().map(row_text).collect::<Vec<_>>().join(" ");
        assert_eq!(
            joined.matches("does not run a work cycle").count(),
            1,
            "the sentence is repeated or missing: {joined}"
        );
        // Pinned, not scrolled: `popup_layout` puts `Row::Text` in the
        // header, and a fact about the whole screen must not page away from
        // the list it is about.
        let note = rows
            .iter()
            .position(|r| row_text(r).contains("does not run a work cycle"))
            .expect("the sentence is drawn");
        let first_item = rows
            .iter()
            .position(|r| matches!(r, Row::Item { .. }))
            .expect("the rows are drawn");
        assert!(note < first_item, "the sentence is below the list");
        assert!(
            matches!(rows[note], Row::Text(_)),
            "the sentence would scroll with the list"
        );

        for row in rows.iter().filter(|r| matches!(r, Row::Item { .. })) {
            assert!(
                !row_text(row).contains("cycle"),
                "a row quoted a cycle a Depot does not have: {}",
                row_text(row)
            );
        }
    }

    /// The role headings did not survive the sort, so what each program is
    /// doing has to still be on its row — that is what keeps "spending it
    /// also empties a job" on the screen.
    #[test]
    fn the_picker_still_says_what_each_program_is_doing() {
        let mut posted = cycling("Sparkgrub");
        posted.pet.activity = "working Mining Node".to_string();
        let mut idle = cycling("Nibbler");
        idle.pet.activity = "idle".to_string();

        let rows = build_program_rows(Some(&deploy("Fabricator")), &[posted, idle], 0);
        let text = rows.iter().map(row_text).collect::<Vec<_>>().join("\n");
        assert!(
            text.contains("working Mining Node"),
            "the post is not on the row: {text}"
        );
        assert!(text.contains("idle"), "{text}");
        for heading in ["Base staff", "In your party"] {
            assert!(
                !text.contains(heading),
                "a role heading survived the sort: {text}"
            );
        }
    }

    /// The pixel counterpart of `no_program_picker_row_runs_past_the_popup_body`,
    /// in `no_deploy_row_overflows_its_popup_in_pixels`' shape — the worst
    /// case now carries the aptitude tag, the cycle quote, an overlong
    /// structure name and an overlong program name.
    #[test]
    fn no_picker_row_overflows_its_popup_in_pixels() {
        let candidates: Vec<BuildCandidate> = (0..6)
            .map(|i| {
                let mut c = candidate(
                    &format!("Overclocked Overlong Program {i} 10"),
                    BuildEffect::Cycle {
                        shipped: 9999,
                        built: 9999,
                    },
                );
                c.aptitude = "Extraction";
                c.label = "Below Average";
                c.pet.quality = Some("Below Average (100%)".to_string());
                c.pet.assembly = Some("Below Average".to_string());
                c.pet.extraction = Some("Below Average".to_string());
                c.pet.activity = "guarding Contract Broker".to_string();
                c.pet.fusions = MAX_FUSIONS;
                c.pet.hp = 1;
                c.pet.max_hp = 1234;
                c.pet.atk = 1234;
                c.pet.power = 1234;
                c
            })
            .collect();
        let commits = [
            deploy("Recompiled Kernel Substrate Assembly Bay"),
            upgrade("Recompiled Kernel Substrate Assembly Bay", 9),
            // With a bill on it: three materials and their two figures each
            // is the longest line this screen can be handed, and it arrived
            // with `Mode::UpgradeDirection` — the list it replaced wrapped
            // its own rows and this one has to wrap too.
            upgrade_costing(
                "Recompiled Kernel Substrate Assembly Bay",
                9,
                vec![
                    "Recompiled Bytecode Block (120/144)".to_string(),
                    "Cache Grain Lattice (0/96)".to_string(),
                    "Kernel Substrate Ingot (18/72)".to_string(),
                ],
            ),
        ];
        crate::paint::with_painter(|p| {
            let m = crate::text::ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for commit in &commits {
                for row in build_program_rows(Some(commit), &candidates, 0) {
                    let text = match &row {
                        Row::Text(t) | Row::TextColored(t, _) => t.clone(),
                        // `draw_row`'s own prefix on an item row.
                        Row::Item { text, .. } => format!("     {text}"),
                    };
                    let drawn = p.measure_ui_advance(&text, m.font_size);
                    assert!(
                        drawn <= room,
                        "a picker row overflows its popup by {:.0}px \
                         ({drawn:.0} into {room:.0}):\n{text}",
                        drawn - room
                    );
                }
            }
        });
    }

    /// The empty picker is a **first-class state**, not an edge.
    /// `App::selected_index` returns `None` for a zero-length list, so a
    /// player who reaches it with nothing to spend lands on a box with no
    /// rows and Esc as the only exit — and, without this, nothing at all
    /// saying why.
    ///
    /// Three sentences because there are three reasons and only one of them
    /// is depth. A deploy asks for tier 1, and every tamed program is from
    /// zone 1 or deeper, so blaming depth there would be plainly false to a
    /// player looking at a roster full of programs.
    ///
    /// The roster floor answers ahead of both, and this pins that order: it
    /// is the one reason that holds at *every* tier, so a base with one
    /// zone 5 program asked for a zone 4 upgrade must not be told its
    /// roster is too shallow.
    #[test]
    fn an_empty_picker_says_why_and_a_deploy_never_blames_depth() {
        assert_eq!(
            no_candidates_refusal(1, false),
            "No program on your roster is free to spend on this.",
            "at tier 1 the roster is deep enough by construction; being free is the question"
        );
        assert_eq!(
            no_candidates_refusal(4, false),
            "Nothing on your roster is from zone 4 or deeper."
        );
        assert_eq!(
            no_candidates_refusal(4, true),
            "Committing your last program would leave nobody to build.",
            "the floor answers first: a lone zone 5 program is not too shallow for a zone 4 \
             upgrade, and saying so would be plainly false"
        );
        assert_eq!(
            no_candidates_refusal(1, true),
            no_candidates_refusal(4, true),
            "the floor is about the base, so the tier cannot change what it says"
        );
        for last_program in [true, false] {
            assert!(
                !no_candidates_refusal(1, last_program).contains("Home"),
                "the picker is never reached for a Home, so it has no exemption to explain"
            );
        }
    }

    /// Both lines the frontend prints for the roster floor quote
    /// `LAST_PROGRAM_CLAUSE`, which is `Game::commit_for_build`'s own
    /// wording with the structure name — the one thing a menu cannot know —
    /// taken off the end.
    ///
    /// Held here because the alternative is what this branch has already
    /// had to undo twice: screens each inventing their own phrasing of one
    /// rule, so a player who meets the floor at the menu and again at the
    /// confirm reads two different sentences and has to work out whether
    /// they are the same refusal.
    ///
    /// **Two lines where there were three.** The upgrade menu had one of its
    /// own; `Mode::UpgradeDirection` has no list to head, and the floor now
    /// reaches an upgrading player at the picker like every other refusal.
    #[test]
    fn the_last_program_lines_share_the_engines_clause() {
        for line in [
            DEPLOY_NEEDS_A_SECOND_PROGRAM,
            &no_candidates_refusal(1, true),
        ] {
            assert!(
                line.starts_with(LAST_PROGRAM_CLAUSE),
                "{line:?} does not open with the engine's own clause"
            );
        }
        assert!(
            DEPLOY_NEEDS_A_SECOND_PROGRAM.contains("Home"),
            "the deploy menu greys everything but Home, so its line has to say so"
        );
    }

    /// The bill the crew will fetch is on this screen, because
    /// `Mode::UpgradeDirection` is the first screen that knows which machine
    /// the order is for and so cannot carry it.
    ///
    /// Priced against the pack **and** the shelves — `build_cost_display`'s
    /// form, which `build_commit` reaches through `Game::upgrade_cost`, the
    /// same door the list this replaced used.
    #[test]
    fn an_upgrade_commit_quotes_what_the_crew_has_to_fetch() {
        let commit = upgrade_costing("Mining Node", 2, vec!["Bytecode Block (4/12)".to_string()]);
        let rows = build_program_rows(Some(&commit), &[cycling("Sparkgrub")], 0);
        let drawn: Vec<&str> = rows.iter().map(row_text).collect();
        assert!(
            drawn.iter().any(|t| t.contains("Bytecode Block (4/12)")),
            "the order has to say what it will cost the base: {drawn:?}"
        );
    }

    /// A deploy does not repeat it: `draw_build_direction` has already
    /// quoted the bill one screen earlier, where the structure was chosen
    /// before the tile.
    #[test]
    fn a_deploy_commit_does_not_repeat_the_bill() {
        let rows = build_program_rows(Some(&deploy("Fabricator")), &[cycling("Sparkgrub")], 0);
        let drawn: Vec<&str> = rows.iter().map(row_text).collect();
        assert!(
            !drawn.iter().any(|t| t.starts_with("Your crew fetches")),
            "the deploy direction screen already said it: {drawn:?}"
        );
    }

    /// Nothing here runs off the right edge: `draw_row` clamps a row
    /// vertically and never horizontally, so an unwrapped prompt is drawn
    /// straight past the box in silence — the fault the deploy menu shipped
    /// with, in the screen that inherits its shape.
    #[test]
    fn no_program_picker_row_runs_past_the_popup_body() {
        let pets: Vec<BuildCandidate> = (0..12)
            .map(|i| cycling(&format!("Overlong Program Name {i}")))
            .collect();
        let commits = [
            deploy("Recompiled Kernel Substrate Assembly Bay"),
            upgrade("Recompiled Kernel Substrate Assembly Bay", 9),
            // The bill `Mode::UpgradeDirection` pushed onto this screen —
            // unwrapped it is 118 columns, so this case is what holds the
            // wrap in place.
            upgrade_costing(
                "Recompiled Kernel Substrate Assembly Bay",
                9,
                vec![
                    "Recompiled Bytecode Block (120/144)".to_string(),
                    "Cache Grain Lattice (0/96)".to_string(),
                    "Kernel Substrate Ingot (18/72)".to_string(),
                ],
            ),
        ];
        for commit in &commits {
            for row in build_program_rows(Some(commit), &pets, 0) {
                let text = row_text(&row);
                assert!(
                    text.chars().count() <= ROW_WRAP_COLUMNS,
                    "a {} char picker row runs past the {ROW_WRAP_COLUMNS} column body: {text:?}",
                    text.chars().count()
                );
            }
        }
    }

    /// `popup_layout` ends the scrollable body at the *last* `Row::Item` and
    /// pins whatever follows as a footer, so a continuation line under the
    /// final program would be torn off it and drawn at the bottom of the
    /// box. Nothing may follow the last item row.
    #[test]
    fn every_picker_row_stays_inside_the_scrollable_body() {
        let pets: Vec<BuildCandidate> = (0..6).map(|i| cycling(&format!("Program {i}"))).collect();
        for selected in [0, pets.len() - 1] {
            let rows = build_program_rows(Some(&deploy("Fabricator")), &pets, selected);
            let last = rows
                .iter()
                .rposition(|r| matches!(r, Row::Item { .. }))
                .expect("a candidate is an item row");
            assert_eq!(
                last,
                rows.len() - 1,
                "the picker pinned {} rows below its list",
                rows.len() - last - 1
            );
        }
    }

    /// The warning is pinned above the list rather than scrolling with it:
    /// every prompt row is `Row::Text`, so `popup_layout` puts the lot in
    /// the header. A warning that pages off the screen it is about is not a
    /// warning.
    #[test]
    fn the_warning_is_pinned_above_the_candidates() {
        let pets: Vec<BuildCandidate> = (0..40).map(|i| cycling(&format!("Program {i}"))).collect();
        let rows = build_program_rows(Some(&deploy("Fabricator")), &pets, 39);
        let first_item = rows
            .iter()
            .position(|r| matches!(r, Row::Item { .. }))
            .expect("a candidate is an item row");
        let warning = rows
            .iter()
            .position(|r| row_text(r).contains("permanently"))
            .expect("the picker drew its warning");
        assert!(
            warning < first_item,
            "the warning fell into the scrolling body"
        );
    }

    /// **The only test in this file that drives `draw_build_program`
    /// itself, rather than `build_program_rows` in isolation.** Every other
    /// test above pins the row *content* against hand-built `PetInfo`
    /// fixtures; none of them touch the three lines inside
    /// `draw_build_program` that turn a real `Game` into that list —
    /// `game.programs_for_build(tier)`, handed to `build_program_rows` as
    /// `&candidates`. A prior review round found that gap: mutate
    /// `&candidates` to `&[]`, or substitute `game.owned_pets()` for
    /// `game.programs_for_build(tier)`, and every test in this crate still
    /// passed — the one painted test that reaches this screen
    /// (`an_empty_program_picker_says_why_it_is_empty`, `render/mod.rs`)
    /// uses a fresh run with zero programs, so both mutations are
    /// indistinguishable from the correct code on that fixture.
    ///
    /// `game_with_a_free_and_a_wielded_program` (`render/test_support.rs`)
    /// closes it: one program `programs_for_build` keeps, one it drops
    /// because it's wielded. `&candidates -> &[]` paints neither name.
    /// `owned_pets()` in place of `programs_for_build` paints both. Only the
    /// real call, passed through untouched, paints exactly the free one.
    #[test]
    fn draw_build_program_only_lists_what_the_engine_will_accept() {
        let mut game = crate::render::test_support::game_with_a_free_and_a_wielded_program(4001);
        let m = crate::text::ui_metrics(900.0);
        let (_, shapes) = crate::paint::with_painter(|p| {
            draw_build_program(&mut game, Some(deploy("Fabricator")), 0, None, p, &m);
        });
        let drawn = crate::paint::painted_text(&shapes);
        let says = |want: &str| drawn.iter().any(|t| t.contains(want));
        assert!(
            says("Free Sparkgrub"),
            "programs_for_build(1) keeps this one: {drawn:?}"
        );
        assert!(
            !says("Wielded Sparkgrub"),
            "programs_for_build(1) drops a wielded program — owned_pets() would not: {drawn:?}"
        );
    }

    /// **The one-program base, end to end**, on all three screens that ask
    /// the roster whether a build can be paid for.
    ///
    /// This is the state every run sits in from its first tamed program
    /// until its second, so for most players it is the *first* time these
    /// screens are read with anything on the roster at all. Before R27 the
    /// roster floor lived only in `Game::commit_for_build`: both menus lit,
    /// the picker listed that program undimmed, and the refusal arrived
    /// after the player had already confirmed spending it.
    ///
    /// Driven through the real `draw_*` calls rather than the pure row
    /// builders, `draw_build_program_only_lists_what_the_engine_will_accept`'s
    /// reason: the row builders take the shortfall and the candidate list as
    /// arguments, so every one of them is satisfied by a caller that asks
    /// the wrong question. What is on trial here is the question.
    ///
    /// And it asserts the *sentence*, not just the greying. Once
    /// `programs_for_build` returns nothing, the old line —
    /// `DEPLOY_NEEDS_A_PROGRAM`, "none of yours is free to spend" — would be
    /// shown to a player looking at one perfectly free program, which is a
    /// screen contradicting itself.
    #[test]
    fn a_one_program_base_greys_every_build_screen_and_says_which_rule_stopped_it() {
        let mut game = crate::render::test_support::game_with_a_single_program(4002);
        let m = crate::text::ui_metrics(900.0);

        assert!(
            floored_by_the_last_program(&mut game),
            "precondition: one owned program is the roster floor, not an empty roster"
        );

        // The deploy menu: one line for the screen, and every non-Home row
        // dimmed.
        let (_, shapes) = crate::paint::with_painter(|p| {
            draw_build_menu(&mut game, 0, None, p, &m);
        });
        let drawn = crate::paint::painted_text(&shapes);
        let says = |drawn: &[String], want: &str| drawn.iter().any(|t| t.contains(want));
        assert!(
            says(&drawn, DEPLOY_NEEDS_A_SECOND_PROGRAM),
            "the deploy menu has to name the rule that stopped it: {drawn:?}"
        );
        assert!(
            !says(&drawn, "None of yours is free to spend"),
            "and must not tell a player with one free program that none of theirs is free"
        );
        let dimmed = crate::paint::painted_runs_in(&shapes, TEXT_DIM, false);
        assert!(
            dimmed.iter().any(|t| t.contains("Mining Node")),
            "an unaffordable structure row is greyed, not hidden: {dimmed:?}"
        );

        // The picker the greyed rows still lead to: nothing offered, and the
        // same rule named a third time.
        let (_, shapes) = crate::paint::with_painter(|p| {
            draw_build_program(&mut game, Some(deploy("Mining Node")), 0, None, p, &m);
        });
        let drawn = crate::paint::painted_text(&shapes);
        assert!(
            !says(&drawn, "Lone Sparkgrub"),
            "the picker must not offer the one program the engine will refuse: {drawn:?}"
        );
        assert!(
            says(&drawn, LAST_PROGRAM_CLAUSE),
            "and it says why it is empty: {drawn:?}"
        );
    }
}

#[cfg(test)]
mod work_order_tests {
    use super::*;
    use feral_processes_engine::items::ItemId;
    use feral_processes_engine::{WorkOrderMachine, WorkOrderReport};

    /// The widest row this screen can draw is a machine line carrying a
    /// machine name, a worker name and a shortfall — and the runner-up is a
    /// stalled order's whole sentence. `draw_row` clamps a row vertically
    /// and **never horizontally**, so a row past the popup body simply runs
    /// off it; two shipped screens already do that because nobody measured.
    ///
    /// **Every state is measured, not just the one that used to carry a
    /// tag.** Before the four states only a stalled order put anything after
    /// the count, so a head line was the shortest row on the screen and
    /// nothing here had to think about it; now all four do. The head is
    /// also the only row on this screen that is *not* wrapped —
    /// `continuation_lines` bounds every other one — so it is the row a tag
    /// can actually run off, and it is measured here against the longest
    /// shipped item name and four-digit counts rather than a convenient
    /// short one.
    #[test]
    fn no_work_order_row_runs_past_the_popup_body() {
        let report = WorkOrderReport {
            item: ItemId::from("singularity_matrix"),
            label: "Singularity Matrix".to_string(),
            have: 9999,
            target: 9999,
            state: OrderState::Working,
            blocked_by: None,
            machines: vec![WorkOrderMachine {
                entity: Entity::PLACEHOLDER,
                label: "Annealing Node".to_string(),
                worker: Some("Sub-Process Lv12".to_string()),
                short_of: Some("Blank Substrate".to_string()),
                depth: 2,
            }],
        };
        let stalled = WorkOrderReport {
            state: OrderState::Stalled,
            blocked_by: Some(
                "Nothing beside the Annealing Node is making Blank Substrate — a machine can \
                 only take what a neighbour has finished."
                    .to_string(),
            ),
            machines: Vec::new(),
            ..report.clone()
        };
        // The two states that draw a tag over an empty chain, which is the
        // combination the sentence under the head has to fit beside.
        let dormant = WorkOrderReport {
            state: OrderState::Dormant,
            blocked_by: None,
            machines: Vec::new(),
            ..report.clone()
        };
        let queued = WorkOrderReport {
            state: OrderState::Queued,
            ..dormant.clone()
        };
        let rows = [
            WorkOrderRow {
                order: Some(report),
            },
            WorkOrderRow {
                order: Some(stalled),
            },
            WorkOrderRow {
                order: Some(dormant),
            },
            WorkOrderRow {
                order: Some(queued),
            },
            WorkOrderRow { order: None },
        ];

        for row in &rows {
            for line in work_order_lines(row, 0, false) {
                assert!(
                    line.chars().count() <= ROW_WRAP_COLUMNS,
                    "a {} char row runs past the {ROW_WRAP_COLUMNS} column body: {line:?}",
                    line.chars().count()
                );
            }
        }

        // The shortfall header, measured here rather than in its own test
        // because it is drawn into this same popup and is unwrapped for the
        // same reason a head line is. Three digits everywhere is past any
        // base a `max_deployed` roster can field, which is the point: the
        // row has to fit before anyone finds out where the real ceiling is.
        let header = labour_header(LabourDemand {
            wanted: 999,
            staff: 998,
        })
        .expect("a base one body short has something to say");
        assert!(
            header.chars().count() <= ROW_WRAP_COLUMNS,
            "a {} char header runs past the {ROW_WRAP_COLUMNS} column body: {header:?}",
            header.chars().count()
        );
        let wide = labour_header(LabourDemand {
            wanted: 999,
            staff: 0,
        })
        .expect("a base with nobody in it has something to say");
        assert!(
            wide.chars().count() <= ROW_WRAP_COLUMNS,
            "a {} char header runs past the {ROW_WRAP_COLUMNS} column body: {wide:?}",
            wide.chars().count()
        );
    }

    /// **Silence is the whole design of the header**: it says nothing when
    /// the base has a body for every post, so a player who never runs short
    /// never learns to skip a line at the top of this screen. A base with
    /// no orders and no staff at all is the same answer for the same
    /// reason — nothing was asked for, so nothing went unfilled.
    #[test]
    fn a_base_with_bodies_to_spare_draws_no_shortfall_header() {
        assert_eq!(
            labour_header(LabourDemand {
                wanted: 2,
                staff: 3
            }),
            None
        );
        assert_eq!(
            labour_header(LabourDemand {
                wanted: 3,
                staff: 3
            }),
            None
        );
        assert_eq!(labour_header(LabourDemand::default()), None);
    }

    /// The quantity page is prose in a popup with no scroll, and its widest
    /// sentence had already outgrown the small box it used to be drawn in.
    /// Measured in pixels rather than columns because that is the failure —
    /// a column count is a proxy that was never checked here.
    #[test]
    fn no_work_order_quantity_row_runs_past_the_popup_body() {
        let m = crate::text::ui_metrics(900.0);
        // Longer than any name the shipped items carry, so a mod naming
        // something unreasonably has room too.
        let name = "Recompiled Kernel Substrate Blank";
        crate::paint::with_painter(|p| {
            let box_w = p.screen_w() * 0.88;
            let bands = [
                OrderPriority::High,
                OrderPriority::Normal,
                OrderPriority::Low,
            ];
            for (standing, priority) in [true, false]
                .into_iter()
                .flat_map(|s| bands.map(|b| (s, b)))
            {
                for line in work_order_quantity_lines(name, "9999", standing, priority) {
                    let text_w = p.measure_ui_advance(&line, m.font_size);
                    assert!(
                        text_w + 2.0 * m.pad < box_w,
                        "a {text_w}px row runs past the {box_w}px body: {line:?}"
                    );
                }
            }
        });
    }
}

#[cfg(test)]
mod base_staff_tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;
    use feral_processes_engine::species::AffinityClass;

    fn staff_row(
        label: &str,
        work: Option<WorkProfile>,
        doing: &str,
        role: Option<ProgramRole>,
    ) -> BaseStaffRow {
        let mut program = super::tests::view(1, 1, 1);
        program.label = label.to_string();
        program.is_structure = false;
        program.is_tamed = true;
        BaseStaffRow {
            program,
            role,
            doing: doing.to_string(),
            work,
        }
    }

    /// The widest row the shipped content can produce: a Gold, thrice-fused
    /// program of the longest species name carrying a zone tag, the widest
    /// work summary, and the longest activity — "guarding the Contract
    /// Broker" over the longest structure name in `assets/structures/`.
    fn widest_staff_row() -> BaseStaffRow {
        staff_row(
            "Gold Sub-Process Lv18 [z9]",
            Some(WorkProfile {
                speed: 14,
                analysis: 18,
                class: Some(AffinityClass::Leech),
            }),
            "guarding the Contract Broker",
            Some(ProgramRole::Staff),
        )
    }

    /// `popup_layout` ends the scrollable body at the *last* `Row::Item` and
    /// pins everything after it as a footer. This screen has a legend, so an
    /// activity emitted as `Row::Text` would put the last program's activity
    /// below the scroll indicator, detached from the program it describes —
    /// the bug the `every_*_stays_inside_the_scrollable_body` family in
    /// `popup.rs` guards for the routine and build pickers.
    ///
    /// Asserted on the row list rather than through `popup_layout` because
    /// the cut is a property of where the last item sits and nothing else:
    /// the footer is every row after it, at any window size.
    #[test]
    fn every_base_staff_activity_stays_inside_the_scrollable_body() {
        for n in 1..6 {
            for selected in [0, n - 1] {
                let staff: Vec<BaseStaffRow> = (0..n)
                    .map(|i| {
                        staff_row(
                            &format!("Program {i}"),
                            None,
                            "idle",
                            Some(ProgramRole::Staff),
                        )
                    })
                    .collect();
                let rows = base_staff_menu_rows(&staff, &[], selected);
                let last_item = rows
                    .iter()
                    .rposition(|r| matches!(r, Row::Item { .. }))
                    .expect("a program is an item row");
                assert_eq!(
                    rows.len() - last_item - 1,
                    1,
                    "with {n} programs the popup pinned {} rows below the list, \
                     not the single legend it is allowed — an activity is \
                     detached from the program it belongs to",
                    rows.len() - last_item - 1
                );
            }
        }
    }

    /// Nothing clamps a popup row horizontally, so a staff row wider than the
    /// Base Staff popup's body runs off its right edge and takes the work
    /// summary with it — which is the whole reason the row carries one.
    ///
    /// **Both budgets, because only one of them discriminates.** In pixels
    /// the widest row clears the reference geometry either way, so that half
    /// would pass just as happily with the activity folded back onto the
    /// shortcut line — it is here for `no_roster_row_overflows_its_popup`'s
    /// reason, to catch the day `ROW_WRAP_COLUMNS` stops being the right
    /// budget. The column count is the half that fails if the two lines are
    /// rejoined, and it is the budget the rest of the file is written
    /// against (see `no_work_order_row_runs_past_the_popup_body`).
    #[test]
    fn the_widest_base_staff_row_stays_inside_the_popup() {
        for row in base_staff_menu_rows(&[widest_staff_row()], &[], 0) {
            let text = match &row {
                Row::Text(t) | Row::TextColored(t, _) => t.clone(),
                Row::Item { text, .. } => text.clone(),
            };
            assert!(
                text.chars().count() <= ROW_WRAP_COLUMNS,
                "a {} char Base Staff row runs past the {ROW_WRAP_COLUMNS} \
                 column body: {text:?}",
                text.chars().count()
            );
        }
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            let rows = base_staff_menu_rows(&[widest_staff_row()], &[], 0);
            for row in &rows {
                let text = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t.clone(),
                    Row::Item { text, .. } => format!("     {text}"),
                };
                let drawn = p.measure_ui_advance(&text, m.font_size);
                assert!(
                    drawn <= room,
                    "the widest Base Staff row overflows its popup by {:.0}px \
                     ({drawn:.0} into {room:.0}):\n{text}",
                    drawn - room
                );
            }
        });
    }

    /// The three facts reach the row, and a species the db never loaded says
    /// so rather than quoting the roster's defaults as if someone authored
    /// them for it.
    #[test]
    fn a_staff_row_spells_out_the_work_profile() {
        let rows = base_staff_menu_rows(&[widest_staff_row()], &[], 0);
        let head = rows
            .iter()
            .find_map(|r| match r {
                Row::Item { text, .. } => Some(text.clone()),
                _ => None,
            })
            .expect("the program is an item row");
        assert!(head.contains("Spd 14"), "{head}");
        assert!(head.contains("Ana 18"), "{head}");
        assert!(
            head.contains("Leech"),
            "the class is the third fact that decides a posting: {head}"
        );

        let unknown = base_staff_menu_rows(
            &[staff_row("Modded", None, "idle", Some(ProgramRole::Staff))],
            &[],
            0,
        );
        let head = unknown
            .iter()
            .find_map(|r| match r {
                Row::Item { text, .. } => Some(text.clone()),
                _ => None,
            })
            .unwrap();
        assert!(head.contains("not loaded"), "{head}");
    }
}

#[cfg(test)]
mod base_output_tests {
    use super::*;
    use feral_processes_engine::items::ItemId;

    /// The page at its worst: both sections full to the report's own cap,
    /// the widest name a shipped item could carry, and every row
    /// `Game::attention` can produce at once.
    fn tallest_base_output_page() -> Vec<Row> {
        use feral_processes_engine::tuning::{BASE_OUTPUT_MAX_ROWS, BASE_OUTPUT_SPARK_BUCKETS};
        use feral_processes_engine::{AttentionKind, AttentionRow, BaseOutputRow};

        let row = |i: usize| BaseOutputRow {
            item: ItemId(format!("item_{i}")),
            // Longer than the column, so the cut is measured rather than
            // assumed: a modded item set is the case that reaches it.
            name: "Singularity Matrix Mk4 Prototype".to_string(),
            sector: 999_999,
            run: 999_999,
            machine: 999_999,
            hand: 999_999,
            spark: vec![999_999; BASE_OUTPUT_SPARK_BUCKETS],
        };
        let attention = |kind, text: &str, threat| AttentionRow {
            kind,
            text: text.to_string(),
            key: 'b',
            threat,
        };
        base_output_rows(&BaseOutputReport {
            zone: 10,
            mined: (0..BASE_OUTPUT_MAX_ROWS).map(row).collect(),
            compiled: (0..BASE_OUTPUT_MAX_ROWS).map(row).collect(),
            // All four rows `Game::attention` can hold at once, at the
            // widest wording each of them builds.
            attention: vec![
                attention(
                    AttentionKind::StructureDamaged,
                    "Recompiler Bay damaged",
                    true,
                ),
                attention(
                    AttentionKind::IdleStructures,
                    "12 nodes without a program",
                    false,
                ),
                attention(AttentionKind::PerkPoints, "12 perk points unspent", false),
                attention(AttentionKind::RosterFull, "roster full (12/12)", false),
            ],
        })
    }

    /// **The page has no scroll.** `draw_popup` pages a `Row::Item` span and
    /// this page has none, so a row past the bottom is dropped in silence —
    /// `the_tallest_memory_page_fits_its_popup`'s trap. Raising
    /// `BASE_OUTPUT_MAX_ROWS` past what fits means giving the page a scroll
    /// first.
    ///
    /// Swept across window heights rather than measured at one: `ui_metrics`
    /// clamps the font at both ends, so below the clamp the box keeps
    /// shrinking while the line height stops.
    #[test]
    fn the_tallest_base_output_page_fits_its_popup() {
        let rows = tallest_base_output_page().len();
        for h in (600..=2160).step_by(60) {
            let m = crate::text::ui_metrics(h as f32);
            let cap = popup_max_rows(h as f32, PopupSize::Large, &m);
            assert!(
                rows + REFUSAL_MAX_LINES <= cap,
                "a full base builds a {rows}-row page into a {cap}-row popup at {h}px"
            );
        }
    }

    /// The other axis, and the one nothing clamps at all: `draw_row` clips a
    /// row vertically and never horizontally, so a line past the right edge
    /// is simply lost — and on this page the tail of a row is the run total
    /// and the sparkline.
    #[test]
    fn no_base_output_row_overflows_its_popup() {
        let rows = tallest_base_output_page();
        crate::paint::with_painter(|p| {
            let m = crate::text::ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for row in &rows {
                let line = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t,
                    _ => continue,
                };
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "a base output row overflows the page by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// The columns are read down, not across, so a name wider than its
    /// column is cut rather than allowed to shift the four figures beside
    /// it — one modded item would otherwise skew every row under it.
    #[test]
    fn a_long_name_is_cut_rather_than_moving_the_columns() {
        use feral_processes_engine::BaseOutputRow;
        use feral_processes_engine::tuning::BASE_OUTPUT_SPARK_BUCKETS;

        let wide = BaseOutputRow {
            item: ItemId("wide".to_string()),
            name: "M".repeat(OUTPUT_NAME_COLUMN * 2),
            sector: 1,
            run: 2,
            machine: 2,
            hand: 0,
            spark: vec![1; BASE_OUTPUT_SPARK_BUCKETS],
        };
        let narrow = BaseOutputRow {
            name: "Core Fragment".to_string(),
            ..wide.clone()
        };
        let (wide, narrow) = (base_output_line(&wide), base_output_line(&narrow));
        assert!(wide.contains('…'), "the name was not cut: {wide}");
        assert_eq!(
            wide.chars().count(),
            narrow.chars().count(),
            "the cut name moved the columns:\n{wide}\n{narrow}"
        );
    }

    /// **Both sections carry the machine/hand split**, not the sector/run
    /// pair the design sketch gave MINED alone. An item sits in a section on
    /// dominant provenance, so a Power Cell whose machines outproduce the
    /// player lands under MINED — and dropping the hand column there would
    /// hide exactly the units the page was built to count.
    #[test]
    fn a_mined_row_still_says_what_the_player_made_by_hand() {
        use feral_processes_engine::BaseOutputRow;

        let rows = base_output_rows(&BaseOutputReport {
            zone: 2,
            mined: vec![BaseOutputRow {
                item: ItemId("power_cell".to_string()),
                name: "Power Cell".to_string(),
                sector: 40,
                run: 90,
                machine: 60,
                hand: 30,
                spark: Vec::new(),
            }],
            compiled: Vec::new(),
            attention: Vec::new(),
        });
        let line = rows
            .iter()
            .filter_map(|r| match r {
                Row::Text(t) | Row::TextColored(t, _) => Some(t),
                _ => None,
            })
            .find(|t| t.contains("Power Cell"))
            .expect("the mined row is drawn");
        assert!(line.contains("60") && line.contains("30"), "{line}");
    }

    /// A base that has made nothing says so. The row is hidden from the
    /// menu in that state (`Game::has_base_output`), but a run can spend its
    /// last produced unit and open the page from a menu already on screen.
    #[test]
    fn an_empty_ledger_draws_a_sentence_rather_than_a_blank_page() {
        let rows = base_output_rows(&BaseOutputReport {
            zone: 1,
            mined: Vec::new(),
            compiled: Vec::new(),
            attention: Vec::new(),
        });
        assert!(rows.iter().any(|r| match r {
            Row::Text(t) | Row::TextColored(t, _) => t.contains("not made anything"),
            _ => false,
        }));
    }
}
