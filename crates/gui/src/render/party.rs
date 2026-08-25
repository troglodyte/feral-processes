//! The companion roster and the three-page fusion flow.

use super::inventory::equipped_row;
use super::popup::*;
use super::*;

/// The help lines above the roster.
///
/// **They deliberately say nothing about wielding a program as a weapon.**
/// That command is an easter egg: it is reachable, it changes the run, and
/// nothing in the game's text points at it. Extracted into a const so
/// `the_companion_screen_never_advertises_the_hidden_key` can read them —
/// a later helpful edit then fails a test rather than quietly spoiling it.
/// The gear line follows the two above it — bare `E ...`, not the `[E]quip`
/// label style the item-action page uses. It also cannot name the slot a
/// wielded program fills, because that word starts with the capital letter
/// the census below forbids anywhere in these lines. That is the constraint
/// working, not a phrasing accident.
///
/// `U` sits directly under `E` because it is that same errand run
/// backwards, and it has to be named: the picker's own `(Unequip)` row is a
/// keypress and a slot choice away and only ever empties the one slot, so a
/// player who never finds this line never learns the screen can undress a
/// program in one press at all.
fn companion_help() -> [String; 7] {
    [
        format!(
            "P adds the highlighted program to your party (max {MAX_PARTY_SIZE}), or stands a member back down."
        ),
        "< and > move the highlighted member along the battle line; the front slot draws the most fire."
            .to_string(),
        "N renames the highlighted program; clear the name to go back to its species."
            .to_string(),
        "E fits gear to the highlighted program, out of your own cargo."
            .to_string(),
        "U takes every piece of gear back off the highlighted program, into your cargo."
            .to_string(),
        "M reads the highlighted program's manifest — its full stat sheet."
            .to_string(),
        "R reads what the highlighted program remembers, and how it feels about it."
            .to_string(),
    ]
}

/// One program's three equipment slots — the same three rows the inventory
/// leads with, through the same formatter, for a program rather than for the
/// player.
///
/// The decompiler line is a standing note rather than a per-item warning:
/// ten shipped items carry the stat, `components::Decompiler` is player-only,
/// and a program simply never attempts a capture. Saying so once here is
/// what keeps that from being something a player discovers by wasting a
/// module on it.
pub(super) fn draw_companion_equip(
    game: &mut Game,
    program: Option<Entity>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let name = program.and_then(|p| {
        game.owned_pets()
            .into_iter()
            .find(|row| row.entity == p)
            .map(|row| row.name)
    });
    let (Some(program), Some(name)) = (program, name) else {
        draw_popup(
            "Program Gear",
            PopupSize::Small,
            &[text_row("That program is gone.")],
            refusal,
            painter,
            m,
        );
        return;
    };
    let mut rows = vec![
        Row::TextColored(format!("{name}'s gear"), CYAN),
        text_row("Number to swap or unequip. Gear comes out of your cargo and goes back to it."),
        text_row("A Decompiler bonus does nothing on a program - only you attempt a capture."),
        text_row(""),
    ];
    for (i, slot) in EquipmentSlot::ALL.into_iter().enumerate() {
        rows.push(equipped_row(
            i,
            slot,
            game.worn(program, slot),
            i == selected,
            game,
        ));
    }
    rows.push(text_row(""));
    rows.push(text_row(
        "[I] inspect — full stats, and what a granted routine actually does",
    ));
    rows.push(text_row("Esc to go back; Up/Down + Enter also work"));
    draw_popup("Program Gear", PopupSize::Large, &rows, refusal, painter, m);
}

/// What one program remembers — `R` from the roster.
///
/// **Every figure comes out of `Game::memory_report` and `Game::morale`**,
/// `draw_gear_inspect`'s rule: a renderer that weighed a memory itself would
/// be the fourth screen in this repo to keep a private copy of a formula the
/// engine already owns, and the subject of a row cannot be named here at all
/// — a species needs `SpeciesDb` and a destroyed program needs the name the
/// record captured when it was written.
///
/// The page does not scroll — `draw_popup` pages a `Row::Item` span and
/// there are none here — so its height is held by
/// `the_tallest_memory_page_fits_its_popup` rather than by a scrollbar.
pub(super) fn draw_companion_memories(
    game: &mut Game,
    program: Option<Entity>,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let name = program.and_then(|p| {
        game.owned_pets()
            .into_iter()
            .find(|row| row.entity == p)
            .map(|row| row.name)
    });
    let (Some(program), Some(name)) = (program, name) else {
        draw_popup(
            "Memories",
            PopupSize::Small,
            &[text_row("That program is gone.")],
            refusal,
            painter,
            m,
        );
        return;
    };
    let rows = memory_page_rows(&name, game.morale(program), &game.memory_report(program));
    draw_popup("Memories", PopupSize::Large, &rows, refusal, painter, m);
}

/// The page's rows, out of the two engine calls above rather than out of a
/// `Game` — the split `fuse_candidate_rows` makes, and for its reason: the
/// height and width censuses have to measure the page at its **worst** case,
/// and a store holding `MEMORY_CAP_PER_PROGRAM` of the widest shipped def is
/// a state a fixture can state and a `Game` would have to be played into.
pub(super) fn memory_page_rows(name: &str, morale: f32, entries: &[MemoryRow]) -> Vec<Row> {
    let mut rows = vec![
        Row::TextColored(format!("{name}'s memories"), CYAN),
        // The one derived figure the whole store adds up to, and the reason
        // the page has a header at all. Coloured by sign rather than by
        // band: what a player wants off a glance is whether this program is
        // carrying more scar than bond, and a five-step scale would be a
        // claim about magnitude the sum cannot support.
        Row::TextColored(format!("Morale {morale:+.0}"), morale_color(morale)),
        text_row(""),
    ];

    if entries.is_empty() {
        // Also what an install with `assets/memories/` deleted draws, which
        // is the supported way to play without this feature at all.
        rows.push(text_row("Nothing has happened to this one yet."));
    }
    // The blurb is a property of the *kind*, so it is said once and the
    // repeats are left bare: a store holds several entries of one def — three
    // corners of the base that strand a worker, four species that have nearly
    // ended it — and printing one sentence of flavour verbatim four times down
    // a page is worse than not printing it at all. Rows arrive strongest
    // first, so the copy that keeps it is the one that characterises the
    // program.
    let mut said: Vec<&str> = Vec::new();
    for entry in entries {
        let subject = match &entry.subject {
            Some(subject) => format!("{} — {subject}", entry.name),
            None => entry.name.clone(),
        };
        let head = format!("{subject}  ({}, {})", strength(entry.intensity), entry.age);
        let line = if said.contains(&entry.name.as_str()) {
            head
        } else {
            said.push(&entry.name);
            format!("{head}  {}", entry.blurb)
        };
        rows.push(Row::TextColored(line, morale_color(entry.intensity)));
    }

    rows.push(text_row(""));
    rows.push(text_row("Esc to go back"));
    rows
}

/// Grudge, bond, or neither. `TEXT` at zero rather than a third hue: a
/// program with nothing to say about anything is not a state worth a colour.
fn morale_color(value: f32) -> Color {
    if value > 0.0 {
        GREEN
    } else if value < 0.0 {
        RED
    } else {
        TEXT
    }
}

/// A memory's weight, as the row prints it: signed, so a bond and a grudge
/// of the same size are visibly opposite, and rounded to whole points —
/// the fractional part is decay, which the age beside it already says.
fn strength(intensity: f32) -> String {
    format!("{intensity:+.0}")
}

/// One program's lines on the roster: the identity and stats, then whichever
/// of its six optional tags still fit, the rest shed onto indented
/// continuations by `wrapped_row_lines`.
///
/// The `w|a|m` loadout cell sits directly after the stats and ahead of every
/// optional tag, so it holds one column down the list: quality, fusion depth,
/// the wield mark and the activity all come and go per row, and a cell placed
/// after any of them would only line up with rows carrying the same ones.
/// That is also why the head ends there and the tags are handed over as
/// separate segments: the shed has to fall on a boundary between two tags,
/// and the head is exactly the part every row carries.
///
/// Its shortcut is passed in rather than read off the row, since the number
/// keys are the list's position and `PetInfo` has no idea where it landed.
///
/// Six tags at their widest run a roster row 382px past a `PopupSize::Large`
/// body, and nothing clamps a row horizontally — so before this the tail ran
/// off the right edge, taking the activity and CRITICAL with it. Those are
/// the two tags the list is most often being read for, which is why the fix
/// wraps rather than chopping: a chop deletes exactly them.
fn companion_row_lines(shortcut: char, p: &PetInfo) -> Vec<String> {
    let slot = p
        .party_slot
        .map(|s| format!("#{} ", s + 1))
        .unwrap_or_default();
    let head = format!(
        "[{shortcut}] {slot}{} Lv{} - HP {}/{}  ATK {}  MIT {}%  PWR {}  {}",
        p.name, p.level, p.hp, p.max_hp, p.atk, p.mitigation, p.power, p.gear,
    );
    let tags = [
        p.quality
            .as_ref()
            .map(|q| format!(" [{q}]"))
            .unwrap_or_default(),
        fusion_tag(p.fusions),
        if p.wielded { " (WEP)" } else { "" }.to_string(),
        activity_tag(&p.activity),
        if hp_critical(p.hp, p.max_hp) {
            " - CRITICAL"
        } else {
            ""
        }
        .to_string(),
    ];
    wrapped_row_lines(head, &tags)
}

pub(super) fn draw_companion_menu(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let pets = game.owned_pets();
    let mut rows: Vec<_> = companion_help().into_iter().map(text_row).collect();
    if pets.is_empty() {
        rows.push(text_row("(you don't have any compiled programs yet)"));
    }
    for (i, p) in pets.iter().enumerate() {
        // No row colour of its own: `fusion_row` already loses to CRITICAL
        // below, and a third meaning on that axis makes all three unreadable.
        let critical = hp_critical(p.hp, p.max_hp);
        // CRITICAL outranks both the fusion colour and the rare tier: one is
        // a state to act on this turn, the others are permanent properties
        // to read at leisure. `tier_color` settles those two against each
        // other, so this only has to know about the loud one.
        let colored = |text: String, selected: bool| {
            if critical {
                critical_item_row(text, selected)
            } else {
                tier_row(text, selected, p.fusions, p.rarity)
            }
        };
        let mut lines = companion_row_lines(menu_shortcut(i), p).into_iter();
        let head = lines
            .next()
            .expect("companion_row_lines always emits the identity row");
        rows.push(with_icon(
            colored(head, i == selected),
            p.glyph,
            glyph_color(p.color),
        ));
        // A continuation carries this row's own tail rather than a second
        // kind of information, so it keeps the row's colour instead of the
        // dim the fuse picker gives a candidate's routines. Only the head is
        // ever `selected`: the highlight belongs on the line carrying the
        // shortcut, and the popup's scroll anchor is the first selected
        // `Item`, so these cannot disturb it.
        for line in lines {
            rows.push(colored(line, false));
        }
    }
    draw_popup("Party", PopupSize::Large, &rows, refusal, painter, m);
}

/// Formats one fuse-candidate row with the full stat line a fusion
/// decision depends on.
fn fuse_candidate_label(num: char, p: &PetInfo) -> String {
    let fused = fusion_tag(p.fusions);
    let activity = activity_tag(&p.activity);
    format!(
        "[{num}] {} Lv{} - HP {}/{}  ATK {}  MIT {}%  PWR {}{fused}{activity}",
        p.name, p.level, p.hp, p.max_hp, p.atk, p.mitigation, p.power
    )
}

/// One fuse candidate's lines: the stat row its shortcut selects, then the
/// routines it is carrying underneath.
///
/// Fusion derives the result's kit fresh from its species, so a candidate's
/// installed routines are exactly what picking it puts at risk — the naming
/// page says so at the end of the flow, and this is that answer while both
/// picks are still free.
///
/// They shed onto their own lines through `continuation_lines` rather than
/// joining the stat row, for the reason `craft_rows` states: `draw_row`
/// clamps a row vertically and nothing clamps it horizontally, and six slots
/// of shipped routine names run well past the popup's edge. A program
/// carrying nothing gets no line at all, which is what wrapping an empty
/// list already returns.
///
/// Returns the lines rather than drawing them so their width is measurable
/// without a window — see `the_widest_shipped_routine_kit_fits_the_fuse_picker`.
/// The stat row is always present, so a caller may take it unconditionally.
fn fuse_candidate_rows(num: char, p: &PetInfo, routines: &[String]) -> Vec<String> {
    std::iter::once(fuse_candidate_label(num, p))
        .chain(continuation_lines(&routines.join(", ")))
        .collect()
}

/// The names filling `program`'s routine slots, empty ones dropped. One
/// definition for both fuse pages, which have no reason to describe the same
/// program's kit two ways.
fn installed_routines(game: &Game, program: Entity) -> Vec<String> {
    game.routine_view(program)
        .into_iter()
        .filter(|s| s.ability.is_some())
        .map(|s| s.name)
        .collect()
}

/// Pushes one candidate's rows: the selectable stat row, then its routines as
/// dim unselected continuations — the same shape `draw_craft_menu` gives a
/// recipe and its cost, and for the same reason. The highlight belongs on the
/// line carrying the shortcut, and the popup's scroll anchor is the *first*
/// selected `Item`, so these cannot disturb it.
fn push_fuse_candidate(rows: &mut Vec<Row>, game: &Game, i: usize, p: &PetInfo, selected: bool) {
    let mut lines =
        fuse_candidate_rows(menu_shortcut(i), p, &installed_routines(game, p.entity)).into_iter();
    let head = lines
        .next()
        .expect("fuse_candidate_rows always emits the stat row");
    rows.push(with_icon(
        tier_row(head, selected, p.fusions, p.rarity),
        p.glyph,
        glyph_color(p.color),
    ));
    for line in lines {
        rows.push(colored_item_row(line, false, TEXT_DIM));
    }
}

pub(super) fn draw_fuse_menu(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let candidates = game.owned_pets();
    let mut rows = vec![text_row("Fuse which program? Pick the first of two.")];
    if candidates.is_empty() {
        rows.push(text_row("(you have no compiled programs)"));
    }
    for (i, p) in candidates.iter().enumerate() {
        push_fuse_candidate(&mut rows, game, i, p, i == selected);
    }
    draw_popup("Fuse", PopupSize::Large, &rows, refusal, painter, m);
}

pub(super) fn draw_fuse_second_menu(
    game: &mut Game,
    first: Option<Entity>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(first) = first else { return };
    let pets = game.owned_pets();
    let first_label = pets
        .iter()
        .find(|p| p.entity == first)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "it".to_string());
    let candidates: Vec<_> = pets.into_iter().filter(|p| p.entity != first).collect();
    let mut rows = vec![text_row(format!(
        "Fuse {first_label} with which program? Both are consumed."
    ))];
    if candidates.is_empty() {
        rows.push(text_row("(you have no other compiled programs)"));
    }
    for (i, p) in candidates.iter().enumerate() {
        push_fuse_candidate(&mut rows, game, i, p, i == selected);
    }
    draw_popup("Fuse", PopupSize::Large, &rows, refusal, painter, m);
}

/// Free-text naming page shown after both fuse candidates are picked.
/// Blank and Enter keeps the default species name.
pub(super) fn draw_fuse_name_menu(
    game: &mut Game,
    first: Option<Entity>,
    second: Option<Entity>,
    name_input: &str,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let (Some(first), Some(second)) = (first, second) else {
        return;
    };
    let pets = game.owned_pets();
    let label_of = |e: Entity| {
        pets.iter()
            .find(|p| p.entity == e)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "it".to_string())
    };
    let mut rows = vec![
        text_row(format!(
            "Fusing {} and {}.",
            label_of(first),
            label_of(second)
        )),
        text_row(""),
        item_row(
            format!(
                "Name it (optional, {} max): {name_input}",
                feral_processes_engine::MAX_CUSTOM_NAME_LEN
            ),
            true,
        ),
        text_row(""),
    ];
    // The result's kit is derived fresh from its species, not merged from
    // its parents' — anything installed manually on either one (research,
    // extraction, a swap) does not survive, so this is the last screen where
    // backing out with Esc still saves it.
    let losses = game.fusion_routine_losses(first, second);
    if !losses.is_empty() {
        let names: Vec<&str> = losses.iter().map(|a| a.name.as_str()).collect();
        rows.push(text_row(format!(
            "This may lose: {} (anything not innate to the result).",
            names.join(", ")
        )));
        rows.push(text_row(""));
    }
    rows.push(text_row(
        "Type a name, Enter to fuse (blank keeps the default name)",
    ));
    rows.push(text_row("Esc to go back and re-pick the second program"));
    draw_popup("Fuse", PopupSize::Small, &rows, refusal, painter, m);
}

/// The rename page's two footer lines, extracted so
/// `the_rename_footer_fits_its_popup` can measure them — this page is
/// `PopupSize::Small`, which is half the window, and these are the widest
/// thing on it by a wide margin.
fn rename_help() -> [&'static str; 2] {
    [
        "Enter to confirm; clear the field to go back to the species name",
        "Esc to leave the name as it is",
    ]
}

/// Free-text naming page for a program already on the roster, opened with
/// `N`. Seeded with the name it already carries, so the field is empty only
/// when the player has emptied it — which is what clears the name.
pub(super) fn draw_rename_menu(
    game: &mut Game,
    target: Option<Entity>,
    name_input: &str,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(target) = target else {
        return;
    };
    let pets = game.owned_pets();
    let subject = pets
        .iter()
        .find(|p| p.entity == target)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "it".to_string());
    let mut rows = vec![
        text_row(format!("Renaming {subject}.")),
        text_row(""),
        item_row(
            format!(
                "Name ({} max): {name_input}",
                feral_processes_engine::MAX_CUSTOM_NAME_LEN
            ),
            true,
        ),
        text_row(""),
    ];
    rows.extend(rename_help().into_iter().map(text_row));
    draw_popup("Rename", PopupSize::Small, &rows, refusal, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::with_painter;
    use crate::text::ui_metrics;

    /// `draw_row` clamps a row vertically and nothing clamps it
    /// horizontally, so a footer wider than its popup silently runs off the
    /// right edge. The rename page is `PopupSize::Small` — half the window,
    /// where the fusion pages that inspired it are `Large` — so its footers
    /// have meaningfully less room than they look like they do.
    #[test]
    fn the_rename_footer_fits_its_popup() {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.5 is `PopupSize::Small`'s width fraction; 1440x900 is the
            // geometry `ui_metrics` is calibrated against.
            let room = 1440.0 * 0.5 - m.pad * 2.0;
            for line in rename_help() {
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "the rename footer overflows its popup by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// The refactor picker prints an upgrade item's own authored `.ron`
    /// description under each row, and those are the longest descriptions in
    /// the item set — an upgrade has to say what it does, because its
    /// magnitudes are data and there is nothing else on screen to read them
    /// off. Same hazard the rename footer above documents: nothing clamps a
    /// row horizontally, so an over-long line runs off the popup's right
    /// edge rather than wrapping.
    ///
    /// Measured against the shipped assets rather than a literal, so an
    /// author lengthening one of the eight fails this rather than shipping a
    /// line that runs off the box.
    #[test]
    fn every_upgrade_items_description_fits_the_refactor_picker() {
        // Straight off the item files rather than through a `Game`: the
        // picker lists cargo, and a census wants every shipped upgrade
        // whether or not a player is carrying it.
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (abilities, _) =
            feral_processes_engine::abilities::AbilityDb::load_dir(&assets.join("abilities"))
                .expect("the abilities load");
        let (items, warnings) =
            feral_processes_engine::items_db::ItemDb::load_dir(&assets.join("items"), &abilities)
                .expect("the items load");
        assert!(warnings.is_empty(), "{warnings:?}");
        let described: Vec<String> = items
            .all()
            .filter(|d| d.upgrade.is_some())
            .map(|d| format!("    {}", d.description))
            .collect();
        assert!(
            described.len() >= 7,
            "the census found {} upgrade items, so it is measuring nothing",
            described.len()
        );
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for line in &described {
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "an upgrade description overflows the picker by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }

    use super::super::test_pet as pet;

    /// The widest roster row the game can put on screen, as `(lines, why)`.
    ///
    /// Enumerated rather than reasoned about: several of the six optional
    /// tags exclude each other (a party member's activity is "in party", a
    /// wielded program is stood down from the party), and picking the worst
    /// case by argument is how a census ends up measuring a row nobody can
    /// reach while the reachable one overflows.
    ///
    /// The ingredients are the real ceilings — `MAX_CUSTOM_NAME_LEN`,
    /// `MAX_FUSIONS`, the longest shipped structure name behind an activity,
    /// the widest quality label, `Rarity::Gold`'s "Overclocked" — because the
    /// name half is capped in characters and the UI face is monospace, so the
    /// row *is* bounded even though half of it is player-authored.
    fn widest_roster_rows() -> Vec<(Vec<String>, String)> {
        // 12 characters, `MAX_CUSTOM_NAME_LEN`, plus the rare tier's word in
        // front and the zone tag behind, which `creature_label` appends to a
        // custom name exactly as it does to a species name.
        let name = format!(
            "Overclocked {} 10",
            "M".repeat(feral_processes_engine::MAX_CUSTOM_NAME_LEN)
        );
        // "Below Average" and "Above Average" tie for the widest label.
        let quality = "Below Average (100%)".to_string();
        let mut out = Vec::new();
        for (slot, activity, why) in [
            (Some(0), "in party", "a front-slot party member"),
            (None, "guarding Contract Broker", "a posted guard"),
            (None, "equipped as weapon", "the wielded program"),
        ] {
            let mut p = pet(&name, "w|a|m");
            p.party_slot = slot;
            p.activity = activity.to_string();
            p.quality = Some(quality.clone());
            p.fusions = MAX_FUSIONS;
            p.wielded = activity == "equipped as weapon";
            // Four digits apiece: a refactored, fused, geared program's bar
            // and power both reach them, and the column is not padded.
            p.level = 6;
            p.hp = 1;
            p.max_hp = 1234;
            p.power = 1234;
            // Nothing caps attack, so it takes the same four digits the bar
            // and the power scalar do. Mitigation is capped by
            // `Game::effective_mitigation` at `MAX_MITIGATION_PERCENT`, so
            // its widest reachable reading is that constant and not a
            // guess.
            p.atk = 1234;
            p.mitigation = feral_processes_engine::tuning::MAX_MITIGATION_PERCENT;
            out.push((companion_row_lines('a', &p), why.to_string()));
        }
        out
    }

    /// Nothing clamps a popup row horizontally (see `continuation_lines`), so
    /// a roster row wider than the Party popup's body runs off its right edge
    /// and takes its tail with it — the activity and CRITICAL, which are the
    /// two tags a player is most likely to be reading the list for.
    ///
    /// Measured against the real font rather than counted in characters,
    /// because `ROW_WRAP_COLUMNS` is a budget in cells and the question here
    /// is whether that budget is still the right one — a wrap that packs to
    /// 100 cells is no defence if the body only has room for 90.
    #[test]
    fn no_roster_row_overflows_its_popup() {
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for (lines, why) in widest_roster_rows() {
                for (i, line) in lines.iter().enumerate() {
                    // The head draws through `with_icon`, so its label carries
                    // the selection prefix *and* the glyph's reserved slot;
                    // a continuation is a plain row and reserves no slot.
                    let prefix = if i == 0 { "     " } else { "  " };
                    let drawn = p.measure_ui_advance(format!("{prefix}{line}"), m.font_size);
                    assert!(
                        drawn <= room,
                        "line {i} of the widest roster row ({why}) overflows \
                         the Party popup by {:.0}px ({drawn:.0} into {room:.0}):\n{line}",
                        drawn - room
                    );
                }
            }
        });
    }

    /// The wrap may not silently drop a tag: a row that sheds its tail is
    /// only better than one that runs off the edge if the tail is still on
    /// screen. Asserted on the *joined* lines so it holds however the six
    /// tags happen to be distributed.
    #[test]
    fn a_wrapped_roster_row_keeps_every_tag() {
        for (lines, why) in widest_roster_rows() {
            assert!(lines.len() > 1, "{why} is the case that needs wrapping");
            let joined = lines.join(" ");
            for tag in ["Below Average (100%)", "fused 3/3 - maxed", "CRITICAL"] {
                assert!(joined.contains(tag), "{why} lost {tag:?}:\n{lines:#?}");
            }
        }
    }

    /// The roster is where a program's gear is *fitted*, so it is also where
    /// the player is deciding which one to fit next — and the list is the
    /// only screen that can answer "which of these is still bare" without
    /// three keypresses per program.
    /// The roster is the screen a player compares programs on — which one
    /// goes in the party, which one gets the gear — so it carries the two
    /// figures a fight turns on and not only the two meters. `MIT` is
    /// percentage points, the same unit and the same word the fuse picker
    /// and the field-routine picker already use for it.
    #[test]
    fn a_roster_row_carries_the_combat_figures() {
        let mut p = pet("Kestrel", "w|a|m");
        p.atk = 8;
        p.mitigation = 5;
        let head = companion_row_lines('a', &p).remove(0);
        assert!(head.contains("ATK 8"), "{head}");
        assert!(head.contains("MIT 5%"), "{head}");
    }

    #[test]
    fn a_roster_row_carries_the_loadout_cell() {
        let head = |p: &PetInfo| companion_row_lines('a', p).remove(0);
        assert!(head(&pet("Kestrel", "w|a|m")).contains("w|a|m"));
        let bare = head(&pet("Nine", ".|.|."));
        assert!(bare.contains(".|.|."), "{bare}");
    }

    /// An ordinary program spends one line. The wrap is for the extreme row
    /// the census above measures, and a list that put every program on two
    /// lines would halve how many the popup can show to fix a row most bases
    /// never field.
    #[test]
    fn an_ordinary_roster_row_stays_on_one_line() {
        let mut p = pet("Kestrel", "w|a|m");
        p.quality = Some("Average (54%)".to_string());
        assert_eq!(companion_row_lines('a', &p).len(), 1);
    }

    /// The cell sits ahead of the tags that come and go — quality, fusion,
    /// the wield mark, the activity — because a column that only lines up on
    /// rows carrying the same optional tags lines up nowhere.
    #[test]
    fn the_loadout_cell_precedes_the_optional_tags() {
        let mut p = pet("Kestrel", "w|.|.");
        p.quality = Some("Excellent (94%)".to_string());
        p.wielded = true;
        let row = companion_row_lines('a', &p).join(" ");
        let cell = row.find("w|.|.").expect("the cell is drawn");
        assert!(cell < row.find("Excellent").unwrap(), "{row}");
        assert!(cell < row.find("WEP").unwrap(), "{row}");
        assert!(cell < row.find("in party").unwrap(), "{row}");
    }

    /// The gear key is the opposite of the one below: it has a screen, it
    /// costs cargo, and a player who never finds it never equips a program
    /// at all. So the help has to name it.
    #[test]
    fn the_companion_screen_names_the_gear_key() {
        assert!(
            companion_help().iter().any(|line| line.starts_with("E ")),
            "the roster must say which key opens a program's gear: {:?}",
            companion_help()
        );
    }

    /// `U` is the only way to empty a program's three slots without walking
    /// the picker once per slot, and nothing on the roster hints that it
    /// exists — the gear cell reads `w|a|m` whether or not that is
    /// reversible. So the line naming it is the whole affordance.
    #[test]
    fn the_companion_screen_names_the_strip_key() {
        assert!(
            companion_help().iter().any(|line| line.starts_with("U ")),
            "the roster must say which key takes a program's gear back off: {:?}",
            companion_help()
        );
    }

    /// Party membership is the one thing this screen is *for*, and since the
    /// row shortcuts stopped toggling it there is nothing but this line to
    /// point at the key. A player who never finds it never fields a party.
    #[test]
    fn the_companion_screen_names_the_party_key() {
        assert!(
            companion_help().iter().any(|line| line.starts_with("P ")),
            "the roster must say which key stands a program in the party: {:?}",
            companion_help()
        );
    }

    /// The manifest is the one screen that states a program's potential
    /// rolls, affinities, growth and base job — none of which the roster's own
    /// stat row carries. Reached from the party menu's picker as well, so a
    /// player who never finds this key is not locked out of anything; what the
    /// line buys is that comparing two programs' full sheets doesn't mean
    /// backing all the way out to a menu between each one.
    #[test]
    fn the_companion_screen_names_the_manifest_key() {
        assert!(
            companion_help().iter().any(|line| line.starts_with("M ")),
            "the roster must say which key opens a program's manifest: {:?}",
            companion_help()
        );
    }

    /// Fusion derives the result's kit fresh from its species, so what a
    /// candidate is carrying is exactly what picking it puts at risk. The
    /// naming page already says so; this is that answer two keypresses
    /// earlier, while both picks are still free.
    #[test]
    fn a_fuse_candidate_lists_its_routines_under_its_stat_row() {
        let lines = fuse_candidate_rows(
            'a',
            &pet("Kestrel", "w|a|m"),
            &["Sandbox".to_string(), "Hyperthread Single v1.0".to_string()],
        );
        assert!(lines[0].contains("Kestrel"), "{lines:?}");
        assert!(
            !lines[0].contains("Sandbox"),
            "the stat row stays the row the eye scans: {lines:?}"
        );
        let under = lines[1..].join(" ");
        assert!(
            under.contains("Sandbox") && under.contains("Hyperthread Single v1.0"),
            "every installed routine is named underneath: {lines:?}"
        );
    }

    /// Same hazard the two censuses above document: `draw_row` clamps a row
    /// vertically and nothing clamps it horizontally. Six slots
    /// (`COMPANION_ROUTINE_SLOT_CAP`) of the widest shipped routine names run
    /// well past a `PopupSize::Large` body on one line, so they have to shed
    /// onto continuation lines rather than off the right edge.
    ///
    /// Measured against the real ability set at its worst case rather than a
    /// literal, so an author naming a routine longer fails this instead of
    /// shipping a line that runs off the box.
    #[test]
    fn the_widest_shipped_routine_kit_fits_the_fuse_picker() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/abilities");
        let (db, warnings) = feral_processes_engine::abilities::AbilityDb::load_dir(&dir)
            .expect("the abilities load");
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut names: Vec<String> = db.all().map(|d| d.name.clone()).collect();
        names.sort_by_key(|n| std::cmp::Reverse(n.chars().count()));
        names.truncate(feral_processes_engine::tuning::COMPANION_ROUTINE_SLOT_CAP as usize);
        assert!(
            names.len() > 1,
            "the census found {} routines, so it is measuring nothing",
            names.len()
        );
        let lines = fuse_candidate_rows('a', &pet("Kestrel", "w|a|m"), &names);
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for line in &lines {
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "a fuse candidate's line overflows the picker by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// A `MemoryRow` differing only in the fields a test cares about.
    fn memory(name: &str, subject: Option<&str>, intensity: f32) -> MemoryRow {
        MemoryRow {
            name: name.to_string(),
            blurb: "It stayed with me.".to_string(),
            subject: subject.map(str::to_string),
            intensity,
            age: "recently".to_string(),
        }
    }

    /// The one derived figure the page exists to head itself with. Two
    /// programs with opposite stores must not draw the same header — a test
    /// that only checked a number was *present* passes against a hardcoded
    /// zero, which is exactly what a header reading `morale` off nothing
    /// would be.
    #[test]
    fn the_page_heads_itself_with_the_morale_figure() {
        let sour = memory_page_rows(
            "Kestrel",
            -14.0,
            &[memory("Mauled by", Some("Glitch"), -14.0)],
        );
        let sweet = memory_page_rows("Kestrel", 9.0, &[memory("Fought beside", Some("Vex"), 9.0)]);

        let header = |rows: &[Row]| match &rows[1] {
            Row::Text(t) | Row::TextColored(t, _) => t.clone(),
            _ => panic!("the second row is the header"),
        };
        assert!(header(&sour).contains("-14"), "{}", header(&sour));
        assert!(header(&sweet).contains("+9"), "{}", header(&sweet));
    }

    /// A row has to say what the memory is *about*, or two maulings by
    /// different things are one indistinguishable row repeated. The subject
    /// is the half the renderer cannot derive.
    #[test]
    fn a_row_names_its_def_and_its_subject() {
        let rows = memory_page_rows(
            "Kestrel",
            -8.0,
            &[memory("Mauled by", Some("Zero-Day"), -8.0)],
        );
        let text = joined(&rows);

        assert!(text.contains("Mauled by"), "{text}");
        assert!(text.contains("Zero-Day"), "{text}");
    }

    /// A memory about nothing in particular names the def alone. The row
    /// must not print a separator with nothing after it.
    #[test]
    fn a_subjectless_row_names_the_def_alone() {
        let rows = memory_page_rows("Kestrel", 5.0, &[memory("Won against the odds", None, 5.0)]);
        let entry = match &rows[3] {
            Row::Text(t) | Row::TextColored(t, _) => t.clone(),
            _ => panic!("the first entry row is a text row"),
        };

        assert!(entry.contains("Won against the odds"), "{entry}");
        assert!(
            !entry.contains(" — "),
            "an em-dash with nothing after it is a subject the row does not have: {entry}"
        );
    }

    /// `MemoryDef::blurb` is authored, censused for being non-empty, and
    /// until this page had no reader at all. Without this the field is dead
    /// content the shipped catalogue is nonetheless held to.
    #[test]
    fn the_blurb_reaches_the_page() {
        let rows = memory_page_rows("Kestrel", 5.0, &[memory("Won against the odds", None, 5.0)]);

        assert!(
            joined(&rows).contains("It stayed with me."),
            "{:?}",
            joined(&rows)
        );
    }

    /// An empty store says so. It is also what an install with
    /// `assets/memories/` deleted draws — the supported way to play without
    /// this feature — so a blank box would be the whole of what that player
    /// ever sees of this screen.
    #[test]
    fn an_empty_store_says_so_rather_than_drawing_a_blank_box() {
        let rows = memory_page_rows("Kestrel", 0.0, &[]);
        let text = joined(&rows);

        assert!(text.contains("Nothing has happened"), "{text}");
    }

    fn joined(rows: &[Row]) -> String {
        rows.iter()
            .filter_map(|r| match r {
                Row::Text(t) | Row::TextColored(t, _) => Some(t.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The memories page is the one thing on the roster with no other way
    /// in: it is not reachable from the party menu, from the map, or from a
    /// manifest, so a player who never finds this key never sees the
    /// feature at all.
    #[test]
    fn the_companion_screen_names_the_memories_key() {
        assert!(
            companion_help().iter().any(|line| line.starts_with("R ")),
            "the roster must say which key opens a program's memories: {:?}",
            companion_help()
        );
    }

    /// The worst page this screen can ever build, out of the **real**
    /// catalogue: `MEMORY_CAP_PER_PROGRAM` entries, each as wide as the
    /// widest shipped def and each carrying its own blurb.
    ///
    /// A census and not a fixture — the worst case is a property of the
    /// assets *and* of how the page packs them, so a def authored longer, a
    /// subject rendered longer, or a second line added per entry has to fail
    /// here rather than be caught by eye.
    ///
    /// **Every name is distinct** and two characters wider than any real
    /// one, so the blurb is never deduped away and the row measured is a
    /// little wider than one the game can build. Over-measuring is the safe
    /// direction for a fit census; deduped, this would measure eleven rows
    /// that carry no blurb at all and pass against a page that overflows.
    fn tallest_memory_page() -> Vec<Row> {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (db, warnings) =
            feral_processes_engine::memories::MemoryDb::load_dir(&assets.join("memories"))
                .expect("the catalogue loads");
        assert!(warnings.is_empty(), "{warnings:?}");
        let widest = db
            .all()
            .max_by_key(|def| def.name.chars().count() + def.blurb.chars().count())
            .expect("the census must walk a real catalogue");

        let subject = widest_subject(&assets);
        let entries: Vec<MemoryRow> = (0..feral_processes_engine::tuning::MEMORY_CAP_PER_PROGRAM)
            .map(|i| MemoryRow {
                name: format!("{}{i:02}", widest.name),
                blurb: widest.blurb.clone(),
                subject: Some(subject.clone()),
                intensity: -widest.valence.abs() * widest.strike_cap as f32,
                age: "a while ago".to_string(),
            })
            .collect();
        memory_page_rows(&subject, -99.0, &entries)
    }

    /// The widest thing a row's subject can be, off the assets.
    ///
    /// A `Program` subject is `Game::creature_label`'s output — a rarity
    /// tier, then a name, then a zone number — and the name is either a
    /// species' or a custom one at `MAX_CUSTOM_NAME_LEN`. A `Species`
    /// subject is a display name alone, so the program form dominates it and
    /// is what this builds.
    fn widest_subject(assets: &std::path::Path) -> String {
        let (abilities, _) =
            feral_processes_engine::abilities::AbilityDb::load_dir(&assets.join("abilities"))
                .expect("the abilities load");
        let (species, warnings) = feral_processes_engine::species::SpeciesDb::load_dir(
            &assets.join("species"),
            &abilities,
        )
        .expect("the species load");
        assert!(warnings.is_empty(), "{warnings:?}");
        let longest_species = species
            .all()
            .map(|def| def.name.chars().count())
            .max()
            .expect("the census must walk a real roster");
        let name_len = longest_species.max(feral_processes_engine::MAX_CUSTOM_NAME_LEN);
        // The widest rarity label, and the deepest zone `balance_sim` sweeps
        // to — the two things `creature_label` wraps a name in.
        format!("Prismatic {} 10", "M".repeat(name_len))
    }

    /// **The page has no scroll.** `draw_popup` pages a `Row::Item` span and
    /// this page has none, so a row past the bottom is dropped in silence —
    /// the trap `the_tallest_gear_page_fits_its_popup` exists to catch, and
    /// this is its mirror. Raising `MEMORY_CAP_PER_PROGRAM` past what fits
    /// means giving the page a scroll first.
    ///
    /// Swept rather than measured at one window, for the gear page's reason:
    /// `ui_metrics` clamps the font at both ends, so below the clamp the box
    /// keeps shrinking while the line height stops and the tightest window
    /// is the smallest one.
    #[test]
    fn the_tallest_memory_page_fits_its_popup() {
        let rows = tallest_memory_page().len();
        for h in (600..=2160).step_by(60) {
            let m = ui_metrics(h as f32);
            let cap = popup_max_rows(h as f32, PopupSize::Large, &m);
            // Plus a refusal's room, for `the_tallest_gear_page_fits_its_popup`'s
            // reason: this page has no scroll either.
            assert!(
                rows + REFUSAL_MAX_LINES <= cap,
                "a full store builds a {rows}-row page into a {cap}-row popup at {h}px"
            );
        }
    }

    /// The other axis, and the one nothing clamps at all: `draw_row` clips a
    /// row vertically and never horizontally, so a line past the right edge
    /// is simply lost. On this page the tail of a row is the strength and
    /// the age — the two figures the row is read for.
    #[test]
    fn no_memory_row_overflows_its_popup() {
        let rows = tallest_memory_page();
        with_painter(|p| {
            let m = ui_metrics(900.0);
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
                    "a memory row overflows the page by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }

    /// The easter-egg census. Wielding a program as your weapon is
    /// deliberately undocumented in-game, and the help lines above the
    /// roster are the one place a well-meaning edit would give it away.
    #[test]
    fn the_companion_screen_never_advertises_the_hidden_key() {
        for line in companion_help() {
            let lower = line.to_lowercase();
            assert!(
                !lower.contains("weapon"),
                "the help must not name what the key does: {line:?}"
            );
            assert!(!lower.contains("wield"), "nor the verb for it: {line:?}");
            assert!(
                !line.contains('W'),
                "nor press the key in front of the player: {line:?}"
            );
        }
    }
}

/// Page one of the refactor flow: which program to upgrade.
///
/// The zone tag is already spelled into `PetInfo::name` by
/// `Game::creature_label`, which is exactly the number this screen is about —
/// so a player choosing between two programs can see which one is behind
/// without opening a manifest for each.
pub(super) fn draw_refactor(
    game: &mut Game,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let zone = game.player_status().zone;
    let programs = game.owned_pets();
    let mut rows = vec![
        text_row("Refactor which program? An upgrade is permanent and cannot be taken back off."),
        text_row(format!("You are in zone {zone}.")),
    ];
    for (i, p) in programs.iter().enumerate() {
        rows.push(with_icon(
            tier_row(
                format!(
                    "[{}] {} Lv{}{}{}",
                    menu_shortcut(i),
                    p.name,
                    p.level,
                    fusion_tag(p.fusions),
                    refactor_tag(p.refactors)
                ),
                i == selected,
                p.fusions,
                p.rarity,
            ),
            p.glyph,
            glyph_color(p.color),
        ));
    }
    draw_popup("Refactor", PopupSize::Large, &rows, refusal, painter, m);
}

/// Page two: what to spend on it.
///
/// The rows come from `Game::companion_upgrades`, which lists cargo only, so
/// this page cannot offer something the engine would then refuse for want of
/// the item. Every other refusal is about the program and lands in the status
/// line instead.
pub(super) fn draw_refactor_item(
    game: &mut Game,
    target: Option<Entity>,
    selected: usize,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    let Some(target) = target else {
        return;
    };
    let subject = game
        .owned_pets()
        .into_iter()
        .find(|p| p.entity == target)
        .map(|p| (p.name, p.refactors));
    let offered = game.companion_upgrades();

    let mut rows = Vec::new();
    if let Some((name, refactors)) = &subject {
        rows.push(text_row(format!("Refactoring {name}.")));
        rows.push(text_row(format!(
            "Upgrade slots: {refactors}/{MAX_COMPANION_REFACTORS} spent. A zone rebuild costs none."
        )));
    }
    for (i, u) in offered.iter().enumerate() {
        rows.push(item_row(
            format!(
                "[{}] {}{}  x{}",
                menu_shortcut(i),
                u.name,
                if u.zone_bump { " (zone rebuild)" } else { "" },
                u.qty
            ),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", u.description)));
    }
    draw_popup("Refactor", PopupSize::Large, &rows, refusal, painter, m);
}
