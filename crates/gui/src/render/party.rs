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
fn companion_help() -> [String; 4] {
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
            i + 1,
            slot.label(),
            game.worn(program, slot),
            i == selected,
            game,
        ));
    }
    rows.push(text_row(""));
    rows.push(text_row("Esc to go back; Up/Down + Enter also work"));
    draw_popup("Program Gear", PopupSize::Large, &rows, painter, m);
}

/// One program's line on the roster.
///
/// The `w|a|m` loadout cell sits directly after the stats and ahead of every
/// optional tag, so it holds one column down the list: quality, fusion depth,
/// the wield mark and the activity all come and go per row, and a cell placed
/// after any of them would only line up with rows carrying the same ones.
/// Its shortcut is passed in rather than read off the row, since the number
/// keys are the list's position and `PetInfo` has no idea where it landed.
fn companion_row(shortcut: char, p: &PetInfo) -> String {
    let slot = p
        .party_slot
        .map(|s| format!("#{} ", s + 1))
        .unwrap_or_default();
    let activity = activity_tag(&p.activity);
    let quality = p
        .quality
        .as_ref()
        .map(|q| format!(" [{q}]"))
        .unwrap_or_default();
    let fused = fusion_tag(p.fusions);
    let wielded = if p.wielded { " (WEP)" } else { "" };
    format!(
        "[{shortcut}] {slot}{} Lv{} - HP {}/{}  PWR {}  {}{}{}{}{}{}",
        p.name,
        p.level,
        p.hp,
        p.max_hp,
        p.power,
        p.gear,
        quality,
        fused,
        wielded,
        activity,
        if hp_critical(p.hp, p.max_hp) {
            " - CRITICAL"
        } else {
            ""
        }
    )
}

pub(super) fn draw_companion_menu(
    game: &mut Game,
    selected: usize,
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
        let text = companion_row(menu_shortcut(i), p);
        // CRITICAL outranks both the fusion colour and the rare tier: one is
        // a state to act on this turn, the others are permanent properties
        // to read at leisure. `tier_color` settles those two against each
        // other, so this only has to know about the loud one.
        rows.push(with_icon(
            if critical {
                critical_item_row(text, i == selected)
            } else {
                tier_row(text, i == selected, p.fusions, p.rarity)
            },
            p.glyph,
            glyph_color(p.color),
        ));
    }
    draw_popup("Party", PopupSize::Large, &rows, painter, m);
}

/// Formats one fuse-candidate row with the full stat line a fusion
/// decision depends on.
fn fuse_candidate_label(num: char, p: &PetInfo) -> String {
    let fused = fusion_tag(p.fusions);
    let activity = activity_tag(&p.activity);
    format!(
        "[{num}] {} Lv{} - HP {}/{}  ATK {}  DEF {}  PWR {}{fused}{activity}",
        p.name, p.level, p.hp, p.max_hp, p.atk, p.def, p.power
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

pub(super) fn draw_fuse_menu(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
    let candidates = game.owned_pets();
    let mut rows = vec![text_row("Fuse which program? Pick the first of two.")];
    if candidates.is_empty() {
        rows.push(text_row("(you have no compiled programs)"));
    }
    for (i, p) in candidates.iter().enumerate() {
        push_fuse_candidate(&mut rows, game, i, p, i == selected);
    }
    draw_popup("Fuse", PopupSize::Large, &rows, painter, m);
}

pub(super) fn draw_fuse_second_menu(
    game: &mut Game,
    first: Option<Entity>,
    selected: usize,
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
    draw_popup("Fuse", PopupSize::Large, &rows, painter, m);
}

/// Free-text naming page shown after both fuse candidates are picked.
/// Blank and Enter keeps the default species name.
pub(super) fn draw_fuse_name_menu(
    game: &mut Game,
    first: Option<Entity>,
    second: Option<Entity>,
    name_input: &str,
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
    draw_popup("Fuse", PopupSize::Small, &rows, painter, m);
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
    draw_popup("Rename", PopupSize::Small, &rows, painter, m);
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
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/items");
        let (items, warnings) =
            feral_processes_engine::items_db::ItemDb::load_dir(&dir).expect("the items load");
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

    /// The roster is where a program's gear is *fitted*, so it is also where
    /// the player is deciding which one to fit next — and the list is the
    /// only screen that can answer "which of these is still bare" without
    /// three keypresses per program.
    #[test]
    fn a_roster_row_carries_the_loadout_cell() {
        assert!(companion_row('a', &pet("Kestrel", "w|a|m")).contains("w|a|m"));
        let bare = companion_row('b', &pet("Nine", ".|.|."));
        assert!(bare.contains(".|.|."), "{bare}");
    }

    /// The cell sits ahead of the tags that come and go — quality, fusion,
    /// the wield mark, the activity — because a column that only lines up on
    /// rows carrying the same optional tags lines up nowhere.
    #[test]
    fn the_loadout_cell_precedes_the_optional_tags() {
        let mut p = pet("Kestrel", "w|.|.");
        p.quality = Some("Excellent (94%)".to_string());
        p.wielded = true;
        let row = companion_row('a', &p);
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
pub(super) fn draw_refactor(game: &mut Game, selected: usize, painter: &Painter, m: &Metrics) {
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
    draw_popup("Refactor", PopupSize::Large, &rows, painter, m);
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
    draw_popup("Refactor", PopupSize::Large, &rows, painter, m);
}
