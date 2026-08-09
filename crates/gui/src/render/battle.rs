//! The intrusion screen: the aligned roster table, the party bars, and
//! the pickers layered over them.

use super::bars::*;
use super::field::draw_battle_buffs;
use super::popup::*;
use super::*;
use feral_processes_engine::battle::SpecialOption;
use feral_processes_engine::components::NEED_MAX;

/// Offset that keeps party-slot bar keys clear of the enemy-group keys they
/// share `Fx::bar_ghost`'s map with. Far above `MAX_ENEMY_GROUPS`, so the
/// two ranges can never meet.
const PARTY_BAR_KEY_BASE: u64 = 1000;

fn status_tag(status: &Option<String>) -> String {
    status
        .as_ref()
        .map(|s| format!(" [{s}]"))
        .unwrap_or_default()
}

/// Pads `s` to exactly `width` monospace cells, truncating with `…` when it
/// overruns. Exactness is the contract, not a suggestion: the header row and
/// every roster row are assembled from these, so a cell that comes out the
/// wrong width shifts every column after it and the ledger stops lining up.
///
/// Chars, not bytes — a species name carries a zone tag and a companion name
/// is player-chosen, so either can hold multi-byte glyphs that byte slicing
/// would panic on. The UI font advances `…` exactly like every other glyph;
/// `tests/font_rasterization.rs` checks that.
///
/// `pub(super)`, like `right` below it: the buff panel (`render/field.rs`)
/// clips a routine name to a fixed column the same way a roster clips a
/// species name, and it's the same algorithm either way.
pub(super) fn cell(s: &str, width: usize) -> String {
    if s.chars().count() > width {
        s.chars()
            .take(width.saturating_sub(1))
            .chain(std::iter::once('…'))
            .collect()
    } else {
        format!("{s:<width$}")
    }
}

/// `cell`, but flushed right. Numeric columns align on their last digit so a
/// column of them can be compared by scanning down it.
pub(super) fn right(s: &str, width: usize) -> String {
    if s.chars().count() > width {
        cell(s, width)
    } else {
        format!("{s:>width$}")
    }
}

/// Column widths for both battle rosters, in monospace cells. Constants here
/// rather than in `app-core`: there is one renderer to serve, so there is
/// nothing to share them with and nothing to drift against.
const MARK_W: usize = 3;
const NAME_W: usize = 18;
/// `hp/max` is one cell, so a single `HP` header can sit over the pair.
/// Nine fits `9999/9999`; deep-zone stat scaling doubles per zone, so four
/// digits a side is already well past anything reachable.
const HP_W: usize = 9;
const STAT_W: usize = 3;
/// Widest value is `ENGAGED`.
const REACH_W: usize = 7;
/// Widest condition the engine words is `BLEEDING (12)` — see
/// `Game::status_label`. Anything longer clips rather than shifting DECOMP.
const STATUS_W: usize = 13;
/// `DECOMP` itself is the widest thing in the column; `100%` fits under it.
const DECOMP_W: usize = 6;
/// `FATIGUE` itself is the widest thing in the column — `100/100` fits under
/// it exactly, which `the_fatigue_column_holds_its_place` pins.
const FATIGUE_W: usize = 7;
/// The one line shape both rosters and both headers are built from, so a
/// column cannot move in a row without moving in its header too.
fn roster_line(
    mark: &str,
    name: &str,
    hp: &str,
    atk: &str,
    def: &str,
    reach: &str,
    tail: &str,
) -> String {
    format!(
        "{}{} {} {} {} {} {}",
        cell(mark, MARK_W),
        cell(name, NAME_W),
        cell(hp, HP_W),
        right(atk, STAT_W),
        right(def, STAT_W),
        cell(reach, REACH_W),
        tail,
    )
}

/// The hostile roster's ragged last column is really two more fixed cells —
/// STATUS then DECOMP — so it is composed here rather than at each call site,
/// for the same reason `roster_line` exists: the header and every row are
/// built from one set of widths or they drift.
fn hostile_tail(status: &str, decomp: &str) -> String {
    format!("{} {}", cell(status, STATUS_W), right(decomp, DECOMP_W))
}

/// The party roster's counterpart: a fixed FATIGUE cell, then the ragged
/// ACTION column. Composed here for the reason `hostile_tail` is — the
/// header and every row are built from one set of widths or they drift.
fn party_tail(fatigue: &str, action: &str) -> String {
    format!("{} {}", right(fatigue, FATIGUE_W), action)
}

/// The FATIGUE cell: what this member has left to spend on routines, or a
/// dash for one that has none to spend. Every companion is a dash — Fatigue
/// is the player's alone, whoever a routine is ordered for — and a dash
/// rather than a copy of the player's number, which would read as five
/// separate pools.
fn fatigue_cell(fatigue: Option<f32>) -> String {
    match fatigue {
        Some(f) => format!("{f:.0}/{NEED_MAX:.0}"),
        None => "—".to_string(),
    }
}

/// The DECOMP cell: the engine's odds against *this* group, or a dash when no
/// taming catalyst is held and there is nothing to quote. Not `0%`, which
/// would read as an allowed-but-hopeless attempt when the action isn't
/// available at all. Why it's blank belongs to the action bar, which already
/// prints the engine's own `no taming catalyst` reason — a second copy of
/// that wording here is exactly how the two would drift.
fn odds_cell(chance: Option<f32>) -> String {
    match chance {
        Some(c) => format!("{:.0}%", c * 100.0),
        None => "—".to_string(),
    }
}

/// `RANGE` because a hostile group's third column is its reach, not a
/// front/back rank the player chose.
fn hostile_header() -> String {
    roster_line(
        "   ",
        "GROUP",
        "HP",
        "ATK",
        "DEF",
        "RANGE",
        &hostile_tail("STATUS", "DECOMP"),
    )
}

fn party_header() -> String {
    roster_line(
        "   ",
        "NAME",
        "HP",
        "ATK",
        "DEF",
        "POS",
        &party_tail("FATIGUE", "ACTION"),
    )
}

/// `roster_line` with the two stat columns taken as the numbers they are.
fn roster_row(
    mark: &str,
    name: &str,
    hp: &str,
    atk: i32,
    def: i32,
    reach: &str,
    tail: &str,
) -> String {
    roster_line(
        mark,
        name,
        hp,
        &atk.to_string(),
        &def.to_string(),
        reach,
        tail,
    )
}

pub(super) fn draw_battle(app: &mut App, fx: &mut Fx, painter: &Painter, m: &Metrics) {
    // Read before the `game` borrow below, which is held for the rest of
    // the function.
    let revealed = app.revealed_battle_log();
    let revealing = app.is_revealing();
    // A finished fight keeps this screen rather than handing off to a
    // summary page — same roster, same log pane, results scrolling into it.
    // All that changes is the bar at the bottom.
    let finished = app.mode == Mode::BattleResult;
    // `App::battle_view`, not `Game::battle_view`: the roster steps with the
    // narration, so a bar drops as the line describing the hit lands rather
    // than a second before the player can read any of it. Once `finished`
    // it answers from the closing roster, `BattleState` being gone.
    let Some(view) = app.battle_view() else {
        return;
    };
    let Some(game) = &mut app.game else { return };
    // A field buff cast before the fight keeps ticking through it (see
    // `Game::active_buffs`), so the panel has to survive the transition
    // from the map screen rather than being a `Mode::Playing`-only readout.
    let buffs = game.active_buffs();

    let w = painter.screen_w();
    // The battle screen sits straight on the window instead of inside a
    // panel, so it holds off the edges by more than panel content does.
    let margin = m.inset * 2.0;
    let mut y = margin;
    let dt = painter.delta();

    painter.ui(
        format!("Hostile programs — round {}", view.round),
        margin,
        y,
        m.font_size,
        TEXT,
    );
    y += m.line_height;
    painter.ui(hostile_header(), margin, y, m.label(), TEXT_DIM);
    y += m.line_height;

    for (idx, g) in view.groups.iter().enumerate() {
        let bar = BarGeometry {
            x: margin,
            y,
            w: w - margin * 2.0,
        };
        let ghost = fx.bar_ghost(idx as u64, g.front_hp, dt);
        let name = if g.count > 1 {
            format!("{} {}s", g.count, g.species_name)
        } else {
            g.species_name.clone()
        };
        // Back groups are desaturated so the reach rule is legible at a
        // glance, rather than something to infer from the log.
        let color = if !g.engaged {
            desaturate(RED)
        } else if g.is_boss {
            MAGENTA
        } else {
            RED
        };
        y = draw_bar(
            bar,
            &roster_row(
                &format!("{}  ", g.letter),
                &format!("{name}{}", if g.is_boss { " [BOSS]" } else { "" }),
                &format!("{}/{}", g.front_hp, g.front_max_hp),
                g.atk,
                g.def,
                if g.engaged { "ENGAGED" } else { "BACK" },
                &hostile_tail(
                    // The engine owns the wording of a condition
                    // ("Bleeding (2)"); upper-casing is presentation, but
                    // abbreviating a vocabulary this renderer does not define
                    // would not be. `OK` rather than blank, because an empty
                    // cell in a ledger reads as missing data.
                    &g.status_effect
                        .as_deref()
                        .map(str::to_uppercase)
                        .unwrap_or_else(|| "OK".to_string()),
                    &odds_cell(g.decompile_chance),
                ),
            ),
            g.front_hp as f32,
            g.front_max_hp.max(1) as f32,
            BarStyle::plain(color),
            painter,
            m,
        );
        draw_ghost_band(
            bar,
            g.front_hp as f32,
            ghost.ghost,
            g.front_max_hp.max(1) as f32,
            color,
            painter,
            m,
        );
        // Damage is inferred from the HP the view reports rather than from
        // a dedicated engine event — a round resolves entirely between two
        // frames, so the drop is unambiguous. Floats spawn at their own
        // row now that there is more than one bar to attribute them to.
        if ghost.damage > 0 {
            fx.spawn_float(format!("-{}", ghost.damage), w / 2.0, bar.y, RED);
        }
        y += m.inset;
    }

    y += m.inset;

    // Hostiles on top, your party on the bottom, with the log between them —
    // the two rosters read as opposing sides with the narration of what
    // passed between them in the middle. The party block is bottom-anchored
    // above the action bar so the log can take exactly the slack left over,
    // which is why its height has to be computed rather than accumulated.
    let log_bottom = painter.screen_h() - m.line_height * 2.0;
    // Two line heights, not one: the block's title *and* its column header
    // sit above the first bar. Getting this wrong shifts the whole party
    // block, since it is bottom-anchored off this figure.
    let party_height = m.line_height * 2.0 + view.party.len() as f32 * bar_row_height(m) + m.inset;
    let party_top = (log_bottom - party_height).max(y);

    let log_height = party_top - y;
    painter.rect(margin, y, w - margin * 2.0, log_height, PANEL_BG);
    painter.rect_lines(margin, y, w - margin * 2.0, log_height, 2.0, BORDER);
    // Floors at 0, not 1. On a window too short to seat both rosters this
    // pane collapses to nothing, and forcing a line into it drew narration
    // at the party block's first row — which the party header then painted
    // over.
    let capacity = ((log_height - margin) / m.line_height).max(0.0) as usize;
    let mut ly = y + margin;
    // The tail, not the head: once a battle's narration outgrows the pane,
    // new lines have to push old ones up and out.
    for e in revealed
        .iter()
        .skip(revealed.len().saturating_sub(capacity))
    {
        if ly + m.line_height > party_top {
            break;
        }
        draw_message_line(e.kind, &e.text, margin + m.inset, ly, painter, m);
        ly += m.line_height;
    }
    // Anchored to the narration pane's own top-right corner and drawn over
    // it last: that pane is flavor text, not tactical data, so occasionally
    // covering a line of it costs far less than putting this over either
    // roster's HP/DECOMP columns, which is everywhere else on this screen.
    draw_battle_buffs(&buffs, w - margin, y, painter, m);

    y = party_top;
    painter.ui(
        format!("Your party — DECOMP {}", view.player_decompiler),
        margin,
        y,
        m.font_size,
        TEXT,
    );
    y += m.line_height;
    painter.ui(party_header(), margin, y, m.label(), TEXT_DIM);
    y += m.line_height;

    for p in &view.party {
        let bar = BarGeometry {
            x: margin,
            y,
            w: w - margin * 2.0,
        };
        let ghost = fx.bar_ghost(PARTY_BAR_KEY_BASE + p.slot as u64, p.hp, dt);
        let active = view.active_slot == Some(p.slot);
        // Danger outranks the active-slot cue: the `>` prefix and the bold
        // face already mark who is acting, and nothing else on this screen
        // says "one more hit and this is gone".
        let color = if hp_critical(p.hp, p.max_hp) {
            RED
        } else if active {
            CYAN
        } else {
            GREEN
        };
        y = draw_bar(
            bar,
            &roster_row(
                &format!("{}{} ", if active { ">" } else { " " }, p.slot + 1),
                &p.name,
                &format!("{}/{}", p.hp, p.max_hp),
                p.atk,
                p.def,
                if p.front { "FRONT" } else { "BACK" },
                // A member's own condition rides in the ACTION column
                // rather than getting a fixed cell of its own that would be
                // empty on almost every row.
                &party_tail(
                    &fatigue_cell(p.fatigue),
                    &format!(
                        "{}{}",
                        p.planned.as_deref().unwrap_or("—"),
                        status_tag(&p.status_effect),
                    ),
                ),
            ),
            p.hp as f32,
            p.max_hp.max(1) as f32,
            BarStyle {
                color,
                bold: active,
            },
            painter,
            m,
        );
        draw_ghost_band(
            bar,
            p.hp as f32,
            ghost.ghost,
            p.max_hp.max(1) as f32,
            color,
            painter,
            m,
        );
        if ghost.damage > 0 {
            fx.spawn_float(format!("-{}", ghost.damage), w / 2.0, bar.y, TEXT);
        }
        y += m.inset;
    }

    // Hidden while narration is still scrolling in: a key pressed then
    // skips the reveal rather than acting, so offering either of these
    // would be advertising a key that doesn't do what it says.
    if !revealing {
        // The fight is over and the screen is being held so its results can
        // be read (see `Mode::BattleResult`), so the bar says how to leave
        // rather than offering actions there is no battle left to spend.
        let bar = if finished {
            "[any key] continue".to_string()
        } else {
            // The action bar is drawn from whatever the engine offers, never
            // from strings authored here — so a new action reaches both
            // renderers without either being touched.
            let mut actions: Vec<String> = view
                .options
                .iter()
                .map(|o| match &o.unavailable {
                    None => o.label.clone(),
                    Some(reason) => format!("{} ({reason})", o.label),
                })
                .collect();
            // Party-level commands come from the engine too, so the two
            // renderers cannot drift on them either.
            actions.extend(game.battle_party_commands().into_iter().map(|c| c.label));
            actions.join("   ")
        };
        painter.ui(
            bar,
            margin,
            painter.screen_h() - m.font_size as f32,
            m.font_size,
            TEXT,
        );
    }

    fx.draw_floats(painter, m);
}

/// Which of the acting member's abilities does the Special spend? Rows come
/// from the engine, same contract as the action bar.
pub(super) fn draw_battle_special_menu(app: &mut App, painter: &Painter, m: &Metrics) {
    let selected = app.menu_selected;
    let Some(game) = &mut app.game else { return };
    let Some(slot) = game.battle_active_slot() else {
        return;
    };
    let mut rows = vec![text_row("Which ability?")];
    for (i, o) in game.battle_special_options(slot).into_iter().enumerate() {
        let label = special_row(i, &o);
        // Greyed rather than hidden, and still selectable: the engine refuses
        // it again on commit, and a row that vanished would leave the player
        // wondering where a routine they installed went.
        rows.push(match o.unavailable {
            Some(_) => spent_item_row(label, i == selected),
            None => creature_row(label, i == selected),
        });
    }
    draw_popup("Pick a special", PopupSize::Large, &rows, painter, m);
}

/// One row of that picker: the routine, how many rounds spending it locks it
/// away for, what it does, and — greyed — why it can't be run right now.
///
/// The cooldown is shown because it is the entire price of a Special — no
/// need is charged for one — so without it a player choosing between two
/// ready routines has nothing to weigh them by. It reads as `2 rd` rather
/// than repeating the refusal's "2 more rounds": one is what the routine
/// will cost, the other what it is still costing.
fn special_row(index: usize, option: &SpecialOption) -> String {
    let reason = option
        .unavailable
        .as_deref()
        .map(|r| format!(" ({r})"))
        .unwrap_or_default();
    let price = match option.cooldown {
        0 => String::new(),
        rounds => format!("{rounds} rd — "),
    };
    format!(
        "[{}] {} — {price}{}{reason}",
        index + 1,
        option.name,
        option.detail,
    )
}

/// Who does this buff or heal land on? Lists you and every standing
/// companion — the whole point of aiming one.
pub(super) fn draw_battle_ally_menu(app: &mut App, painter: &Painter, m: &Metrics) {
    let selected = app.menu_selected;
    let title = app.battle_target_title();
    let Some(game) = &mut app.game else { return };
    let mut rows = vec![text_row("Apply to whom?")];
    for (i, a) in game.battle_ally_options().into_iter().enumerate() {
        rows.push(creature_row(
            format!("[{}] {} — {}", i + 1, a.name, a.detail),
            i == selected,
        ));
    }
    draw_popup(&title, PopupSize::Large, &rows, painter, m);
}

/// Which group does the pending action hit? Shows per-group decompile odds,
/// since that's the one action where the choice of target is a real gamble
/// rather than a preference.
pub(super) fn draw_battle_target_menu(app: &mut App, painter: &Painter, m: &Metrics) {
    let selected = app.menu_selected;
    let title = app.battle_target_title();
    let Some(game) = &mut app.game else { return };
    // Deliberately the live view, unlike `draw_battle` above: these letters
    // are what `battle_target_key` resolves against `BattleState::groups`,
    // so a rewound row here would offer a target that no longer exists.
    let Some(view) = game.battle_view() else {
        return;
    };
    let mut rows = vec![text_row("Target which group?")];
    for (i, g) in view.groups.iter().enumerate() {
        let odds = match g.decompile_chance {
            Some(c) => format!(" — decompile {:.0}%", c * 100.0),
            None => String::new(),
        };
        rows.push(creature_row(
            format!(
                "[{}] {} x{} — {}/{} HP {}{}",
                g.letter,
                g.species_name,
                g.count,
                g.front_hp,
                g.front_max_hp,
                if g.engaged { "<engaged>" } else { "<back>" },
                odds,
            ),
            i == selected,
        ));
    }
    draw_popup(&title, PopupSize::Large, &rows, painter, m);
}

/// Which consumable does this slot spend? Lists only what's actually
/// usable — the action is greyed out with a reason before it gets here.
pub(super) fn draw_battle_item_menu(app: &mut App, painter: &Painter, m: &Metrics) {
    let selected = app.menu_selected;
    let Some(game) = &mut app.game else { return };
    let items = game.battle_usable_items();
    let mut rows = vec![text_row(
        "Use which item? It costs this member their round.",
    )];
    for (i, item) in items.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {}", menu_shortcut(i), game.item_name(item)),
            i == selected,
        ));
    }
    draw_popup("Use an item", PopupSize::Large, &rows, painter, m);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A refused menu pick ("Requires Automation first.") only reaches the
    /// player through `App::status_line`, and every gameplay menu draws a
    /// popup over the log pane that used to be the sole place it appeared —
    /// which made a refusal indistinguishable from a dead keypress.
    /// Where the ragged last column must start: every fixed cell plus the
    /// single space separating each pair. Spelled out here rather than in
    /// the layout itself, so it is an independent expectation the tests hold
    /// `roster_line` to — if the format string gains or loses a separator,
    /// these tests fail instead of quietly agreeing with it.
    const TAIL_COL: usize = MARK_W + NAME_W + 1 + HP_W + 1 + STAT_W + 1 + STAT_W + 1 + REACH_W + 1;

    /// Reads `line` from char offset `col` — the rows are monospace, so a
    /// char offset *is* a screen column.
    fn at(line: &str, col: usize) -> String {
        line.chars().skip(col).collect()
    }

    /// The header and every row are assembled from one set of widths, so the
    /// ragged last column begins at the same offset on every line of a
    /// roster block. That alignment is the whole ledger effect — asserted
    /// rather than left to two format strings staying in step by hand.
    #[test]
    fn every_roster_line_puts_its_tail_at_the_same_column() {
        let lines = [
            hostile_header(),
            roster_row(
                "A  ",
                "4 Null Daemons",
                "18/30",
                9,
                4,
                "ENGAGED",
                &hostile_tail("BLEEDING (2)", &odds_cell(Some(0.62))),
            ),
            roster_row(
                "B  ",
                "Warden Process",
                "44/44",
                14,
                9,
                "BACK",
                &hostile_tail("OK", &odds_cell(Some(0.18))),
            ),
            party_header(),
            roster_row(
                ">1 ",
                "You",
                "21/30",
                11,
                6,
                "FRONT",
                &party_tail(&fatigue_cell(Some(62.0)), "Attack A"),
            ),
            roster_row(
                " 2 ",
                "Sparkgrub",
                "18/18",
                7,
                3,
                "FRONT",
                &party_tail(&fatigue_cell(None), "Defend"),
            ),
        ];
        for line in &lines {
            assert_eq!(
                line.chars().take(TAIL_COL).count(),
                TAIL_COL,
                "{line:?} is shorter than the fixed columns"
            );
        }
        assert!(at(&lines[0], TAIL_COL).starts_with("STATUS"));
        assert!(at(&lines[1], TAIL_COL).starts_with("BLEEDING"));
        assert!(at(&lines[3], TAIL_COL).starts_with("FATIGUE"));
        assert!(at(&lines[4], TAIL_COL).starts_with(" 62/100"));
    }

    /// A whole roster block, character-exact. The other tests assert the
    /// invariants; this one shows what the screen actually reads like, so a
    /// change to any width is reviewable as a diff of the output rather than
    /// of arithmetic.
    #[test]
    fn a_hostile_block_reads_as_an_aligned_table() {
        let block = [
            hostile_header(),
            roster_row(
                "A  ",
                "4 Null Daemons",
                "18/30",
                9,
                4,
                "ENGAGED",
                &hostile_tail("BLEEDING (2)", &odds_cell(Some(0.62))),
            ),
            roster_row(
                "B  ",
                "Warden Process",
                "44/44",
                14,
                9,
                "BACK",
                &hostile_tail("OK", &odds_cell(Some(0.18))),
            ),
            roster_row(
                "C  ",
                "Sentinel [BOSS]",
                "120/120",
                22,
                15,
                "BACK",
                &hostile_tail("OK", &odds_cell(None)),
            ),
        ]
        .join("\n");
        assert_eq!(
            block,
            "   GROUP              HP        ATK DEF RANGE   STATUS        DECOMP\n\
             A  4 Null Daemons     18/30       9   4 ENGAGED BLEEDING (2)     62%\n\
             B  Warden Process     44/44      14   9 BACK    OK               18%\n\
             C  Sentinel [BOSS]    120/120    22  15 BACK    OK                 —"
        );
    }

    /// The party block, character-exact for the same reason — and this one is
    /// the diagram `docs/manual.md` prints under "Both sides are listed as a
    /// stat table", which is copied from here rather than aligned by hand.
    #[test]
    fn a_party_block_reads_as_an_aligned_table() {
        let block = [
            party_header(),
            roster_row(
                ">1 ",
                "You",
                "21/30",
                11,
                6,
                "FRONT",
                &party_tail(&fatigue_cell(Some(62.0)), "Attack A"),
            ),
            roster_row(
                " 2 ",
                "Sparkgrub",
                "18/18",
                7,
                3,
                "FRONT",
                &party_tail(&fatigue_cell(None), "Defend"),
            ),
        ]
        .join("\n");
        assert_eq!(
            block,
            "   NAME               HP        ATK DEF POS     FATIGUE ACTION\n\
             >1 You                21/30      11   6 FRONT    62/100 Attack A\n\
             \u{20}2 Sparkgrub          18/18       7   3 FRONT         — Defend"
        );
    }

    /// DECOMP is a fixed cell inside the hostile tail, not something appended
    /// after a condition — so it sits under its header whatever the engine
    /// words the condition as. `BLEEDING (12)` is the widest one there is.
    #[test]
    fn the_decompile_odds_column_holds_its_place() {
        const DECOMP_COL: usize = TAIL_COL + STATUS_W + 1;
        let bleeding = roster_row(
            "A  ",
            "4 Null Daemons",
            "18/30",
            9,
            4,
            "ENGAGED",
            &hostile_tail("BLEEDING (12)", &odds_cell(Some(0.62))),
        );
        let ok = roster_row(
            "B  ",
            "Glitch",
            "8/8",
            3,
            1,
            "ENGAGED",
            &hostile_tail("OK", &odds_cell(Some(1.0))),
        );
        assert!(at(&hostile_header(), DECOMP_COL).starts_with("DECOMP"));
        assert_eq!(at(&bleeding, DECOMP_COL), "   62%");
        assert_eq!(at(&ok, DECOMP_COL), "  100%");
    }

    /// With no taming catalyst there are no odds to quote — a decompile can't
    /// be attempted at all — so the cell holds a dash rather than a `0%` that
    /// would read as an allowed attempt that never lands.
    #[test]
    fn the_odds_cell_is_a_dash_without_a_catalyst() {
        assert_eq!(odds_cell(None), "—");
        let row = roster_row(
            "A  ",
            "Glitch",
            "8/8",
            3,
            1,
            "ENGAGED",
            &hostile_tail("OK", &odds_cell(None)),
        );
        assert_eq!(at(&row, TAIL_COL + STATUS_W + 1), "     —");
    }

    /// And each header label sits over the column it names.
    #[test]
    fn the_header_labels_sit_over_their_columns() {
        let h = party_header();
        assert!(at(&h, MARK_W).starts_with("NAME"));
        assert!(at(&h, MARK_W + NAME_W + 1).starts_with("HP"));
        assert!(at(&h, TAIL_COL).starts_with("FATIGUE"));
        assert!(at(&h, TAIL_COL + FATIGUE_W + 1).starts_with("ACTION"));
    }

    /// FATIGUE is a fixed cell in the party tail, so ACTION starts at the
    /// same column whether the row is the player's or a companion's — and
    /// `100/100`, the widest reading there is, fits under the header without
    /// widening it.
    #[test]
    fn the_fatigue_column_holds_its_place() {
        const ACTION_COL: usize = TAIL_COL + FATIGUE_W + 1;
        let you = roster_row(
            ">1 ",
            "You",
            "21/30",
            11,
            6,
            "FRONT",
            &party_tail(&fatigue_cell(Some(NEED_MAX)), "Special: Null Route"),
        );
        let pet = roster_row(
            " 2 ",
            "Sparkgrub",
            "18/18",
            7,
            3,
            "FRONT",
            &party_tail(&fatigue_cell(None), "Defend"),
        );
        assert_eq!(fatigue_cell(Some(NEED_MAX)).chars().count(), FATIGUE_W);
        assert_eq!(
            at(&you, TAIL_COL)
                .chars()
                .take(FATIGUE_W)
                .collect::<String>(),
            "100/100"
        );
        assert_eq!(
            at(&pet, TAIL_COL)
                .chars()
                .take(FATIGUE_W)
                .collect::<String>(),
            "      —"
        );
        assert!(at(&you, ACTION_COL).starts_with("Special: Null Route"));
        assert!(at(&pet, ACTION_COL).starts_with("Defend"));
    }

    /// The picker prices each routine against the FATIGUE column behind it,
    /// and an unaffordable one still says what it would have cost — the
    /// number the engine's refusal is talking about.
    #[test]
    fn a_special_row_prices_the_routine_and_carries_its_refusal() {
        fn option(cooldown: u32, unavailable: Option<&str>) -> SpecialOption {
            SpecialOption {
                index: 0,
                name: "Cascade Overflow".to_string(),
                detail: "Damage a whole group".to_string(),
                targeting: feral_processes_engine::battle::SpecialTargeting::Enemy,
                sweeps_party: false,
                unavailable: unavailable.map(str::to_string),
                cooldown,
            }
        }

        assert_eq!(
            special_row(0, &option(2, None)),
            "[1] Cascade Overflow — 2 rd — Damage a whole group"
        );
        assert_eq!(
            special_row(2, &option(2, Some("2 more rounds"))),
            "[3] Cascade Overflow — 2 rd — Damage a whole group (2 more rounds)"
        );
        // Decompile is the one battle routine that arms nothing, and a
        // price of zero is worse than no price at all — it reads as a
        // number the player should be weighing.
        assert_eq!(
            special_row(0, &option(0, None)),
            "[1] Cascade Overflow — Damage a whole group"
        );
    }

    /// Fatigue is one pool and it is the player's — a companion's cell holds
    /// a dash rather than a copy of that number, which would read as a pool
    /// of its own.
    #[test]
    fn a_companions_fatigue_cell_is_a_dash() {
        assert_eq!(fatigue_cell(None), "—");
        assert_eq!(fatigue_cell(Some(0.0)), "0/100");
        // Rounded, not truncated toward a reading the player can't act on:
        // decay leaves fractions, and `4.6` left of a 5.0-cost routine is
        // nearer 5 than 4.
        assert_eq!(fatigue_cell(Some(61.5)), "62/100");
    }

    /// An over-long name is clipped rather than allowed to shove the stats
    /// rightward — the failure this whole design exists to prevent.
    #[test]
    fn a_long_name_does_not_shift_the_columns_after_it() {
        let long = roster_row(
            "A  ",
            "4 Corrupted Null Daemons of Yendor",
            "8/8",
            3,
            1,
            "ENGAGED",
            "OK",
        );
        assert_eq!(long.chars().take(TAIL_COL).count(), TAIL_COL);
        assert!(long.contains('…'), "the clipped name has to show it");
        assert!(at(&long, TAIL_COL).starts_with("OK"));
    }

    /// Numbers right-align so a column of them can be compared by scanning
    /// down it, which is the reason for having columns at all.
    #[test]
    fn stat_columns_are_right_aligned() {
        let row = roster_row("A  ", "Glitch", "8/8", 3, 1, "ENGAGED", "OK");
        let atk = MARK_W + NAME_W + 1 + HP_W + 1;
        assert_eq!(
            row.chars().skip(atk).take(STAT_W).collect::<String>(),
            "  3"
        );
        assert_eq!(
            row.chars()
                .skip(atk + STAT_W + 1)
                .take(STAT_W)
                .collect::<String>(),
            "  1"
        );
    }

    #[test]
    fn cell_pads_short_content_to_exactly_the_column_width() {
        assert_eq!(cell("You", 8), "You     ");
        assert_eq!(cell("", 3), "   ");
        assert_eq!(cell("exact", 5), "exact");
    }

    /// A name longer than its column has to lose its tail. Letting it
    /// through would push every column after it right, which defeats the
    /// entire point of a ledger.
    #[test]
    fn cell_truncates_over_width_content_and_marks_it() {
        let out = cell("4 Corrupted Null Daemons", 12);
        assert_eq!(out.chars().count(), 12);
        assert!(
            out.ends_with('…'),
            "a clipped cell has to show that it was clipped, got {out:?}"
        );
    }

    /// Counted in chars, not bytes: `zone_tagged_name` and a player-chosen
    /// companion name can both hold multi-byte glyphs, and slicing one
    /// mid-glyph would panic.
    #[test]
    fn cell_counts_characters_not_bytes() {
        assert_eq!(cell("Ünïcödé", 7).chars().count(), 7);
        assert_eq!(cell("Ünïcödé", 4).chars().count(), 4);
        assert_eq!(cell("Ünïcödé", 9).chars().count(), 9);
    }
}

#[cfg(test)]
mod reveal_tests {
    /// Both log panes show the *tail* of what they are given, not the head:
    /// once narration outgrows the pane, new lines have to push old ones up
    /// and out. Taking the head instead would freeze the pane on the
    /// opening lines and never show the round the player just fought.
    #[test]
    fn a_pane_shows_the_newest_lines_once_it_overflows() {
        let revealed: Vec<String> = (0..10).map(|i| format!("line {i}")).collect();
        let capacity = 3;

        let shown: Vec<&String> = revealed
            .iter()
            .skip(revealed.len().saturating_sub(capacity))
            .collect();

        assert_eq!(shown, vec!["line 7", "line 8", "line 9"]);
    }

    /// Fewer lines than the pane holds must all be drawn — `saturating_sub`
    /// rather than a subtraction that would underflow to a huge skip and
    /// draw nothing at all.
    #[test]
    fn a_pane_that_is_not_full_shows_everything() {
        let revealed = ["line 0", "line 1"];
        let capacity = 10;

        let shown: Vec<&&str> = revealed
            .iter()
            .skip(revealed.len().saturating_sub(capacity))
            .collect();

        assert_eq!(shown.len(), 2);
    }
}
