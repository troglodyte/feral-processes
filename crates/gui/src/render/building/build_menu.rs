use super::*;

/// One buildable structure as the build menu needs it: everything that
/// required a `Game` to work out, already worked out.
pub(crate) struct BuildEntry {
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
pub(super) fn category_name(category: StructureCategory) -> &'static str {
    match category {
        StructureCategory::Home => "Home",
        StructureCategory::Extractor => "Extractors",
        StructureCategory::Assembler => "Assemblers",
        StructureCategory::Research => "Research",
        StructureCategory::Utility => "Utility",
        StructureCategory::EntityHealth => "Entity Health",
        StructureCategory::Trade => "Trade",
        StructureCategory::Defence => "Defence",
    }
}

pub(super) fn category_heading(category: StructureCategory) -> Option<&'static str> {
    match category {
        StructureCategory::Home => None,
        StructureCategory::Extractor => Some("── Extractors — produce on their own ──"),
        StructureCategory::Assembler => Some("── Assemblers — fed by what they touch ──"),
        StructureCategory::Research => Some("── Research — study a pinned program ──"),
        StructureCategory::Utility => Some("── Utility ──"),
        StructureCategory::EntityHealth => Some("── Entity Health ──"),
        StructureCategory::Trade => Some("── Trade ──"),
        StructureCategory::Defence => Some("── Defence ──"),
    }
}

/// The build menu's rows, pure so the layout invariant below can be tested
/// without a `Game` or a `Painter`.
///
/// **Every row between the shortfall line and the key footer is a
/// `Row::Item`, including the headings and the descriptions.** `popup_layout` ends the scrollable body at the *last*
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
pub(crate) fn build_menu_rows(
    entries: &[BuildEntry],
    selected: usize,
    shortfall: Option<&str>,
) -> Vec<Row> {
    let mut rows = build_menu_preamble(shortfall);
    let mut current: Option<StructureCategory> = None;
    for (i, entry) in entries.iter().enumerate() {
        let heading = (current != Some(entry.category))
            .then(|| category_heading(entry.category))
            .flatten();
        current = Some(entry.category);
        // A blank line above every name, so one structure's description
        // does not run into the next one's name; a heading brings its own.
        rows.push(blank_row());
        if let Some(heading) = heading {
            rows.push(colored_item_row(heading, false, TEXT_DIM));
        }
        push_entry_rows(&mut rows, entry, i == selected, shortfall);
    }
    push_build_menu_footer(&mut rows);
    rows
}

/// A `Row::Item` rather than `Row::Text`, for the reason `build_menu_rows`
/// gives.
pub(super) fn blank_row() -> Row {
    colored_item_row("", false, TEXT_DIM)
}

/// What both levels of the deploy menu open with.
pub(super) fn build_menu_preamble(shortfall: Option<&str>) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(line) = shortfall {
        // ORANGE and a `Row::Text`, so it is pinned above the scrolling list
        // it is about rather than paging away from it — and so it never
        // joins the `Row::Item` span a keypress is resolved against.
        rows.push(Row::TextColored(line.to_string(), ORANGE));
    }
    rows
}

pub(super) fn push_entry_rows(
    rows: &mut Vec<Row>,
    entry: &BuildEntry,
    selected: bool,
    shortfall: Option<&str>,
) {
    let label = match entry.deployed {
        0 => format!("{} - {}", entry.name, entry.cost),
        n => format!("{} ({n}) - {}", entry.name, entry.cost),
    };
    let affordable = shortfall.is_none() || !entry.needs_program;
    rows.push(match affordable {
        true => item_row(label, selected),
        // `spent_item_row` rather than a hidden row: still selectable,
        // still navigated past, and reading as something there is no
        // point picking right now.
        false => spent_item_row(label, selected),
    });
    // Through `description_rows` rather than one indented `format!`: the
    // shipped descriptions run to 300 characters against a body of about
    // 114, and the wrapped lines have to stay `Row::Item` for the reason
    // `build_menu_rows` gives.
    rows.extend(description_rows(&entry.description));
}

/// Esc goes back on either level — out of a category to the list of them, or
/// from there to the Build page.
pub(super) fn push_build_menu_footer(rows: &mut Vec<Row>) {
    push_key_footer(rows, &["Enter choose", "Esc back"]);
}

/// One row of the deploy menu's top level, everything that needed a `Game`
/// already worked out.
pub(crate) enum BuildLine {
    Structure(BuildEntry),
    Category {
        category: StructureCategory,
        count: usize,
    },
}

/// The deploy menu's top level: the Home with its cost and description, as
/// in a category, and one row per category naming how many it holds. A
/// category row carries no description — the heading that opens the
/// category says what its machines have in common.
pub(crate) fn build_top_rows(
    lines: &[BuildLine],
    selected: usize,
    shortfall: Option<&str>,
) -> Vec<Row> {
    let mut rows = build_menu_preamble(shortfall);
    for (i, line) in lines.iter().enumerate() {
        match line {
            BuildLine::Structure(entry) => {
                rows.push(blank_row());
                push_entry_rows(&mut rows, entry, i == selected, shortfall);
                rows.push(blank_row());
            }
            BuildLine::Category { category, count } => rows.push(item_row(
                format!("{} ({count})", category_name(*category)),
                i == selected,
            )),
        }
    }
    push_build_menu_footer(&mut rows);
    rows
}

pub(crate) fn draw_build_menu(
    game: &mut Game,
    open: Option<StructureCategory>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let status = game.player_status();
    let stock = game.base_stock();
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
    let menu = BuildMenu::of(game, open);
    let entry = |def: &StructureDef| {
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
    };
    let rows = match open {
        Some(_) => {
            let entries: Vec<BuildEntry> = menu
                .rows
                .iter()
                .filter_map(|r| match r {
                    BuildRow::Structure(i) => Some(entry(&menu.defs[*i])),
                    BuildRow::Category(_) => None,
                })
                .collect();
            build_menu_rows(&entries, selected, shortfall)
        }
        None => {
            let lines: Vec<BuildLine> = menu
                .rows
                .iter()
                .map(|r| match r {
                    BuildRow::Structure(i) => BuildLine::Structure(entry(&menu.defs[*i])),
                    BuildRow::Category(c) => BuildLine::Category {
                        category: *c,
                        count: menu.defs.iter().filter(|d| d.category() == *c).count(),
                    },
                })
                .collect();
            build_top_rows(&lines, selected, shortfall)
        }
    };
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
pub(crate) fn build_direction_rows(name: &str, description: &str, cost: &[String]) -> Vec<Row> {
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
        text_row(""),
        text_row(key_hints(DIRECTION_KEYS)),
    ]);
    rows
}

pub(super) const DIRECTION_PROMPT: &str = "Choose a direction to deploy";

/// What the build menu writes after a structure's name. A waived bill (see
/// `StructureDef::first_free`) has no rows at all, and a row that trailed
/// off after the dash read as a structure whose cost had failed to load.
pub(super) fn build_cost_label(cost: &[String]) -> String {
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
pub(crate) fn draw_build_direction(
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
        None => vec![
            text_row(DIRECTION_PROMPT),
            text_row(""),
            text_row(key_hints(DIRECTION_KEYS)),
        ],
    };
    draw_popup(
        "Deploy direction",
        PopupSize::Large,
        &rows,
        refusal,
        painter,
        m,
    );
}
