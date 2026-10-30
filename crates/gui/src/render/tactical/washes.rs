use super::*;

/// How much ink a cloaked body or a Hallucination decoy draws with — see
/// `components::Cloaked` and `tactical::Decoy`. One constant for both: they
/// mean the same thing, *not really there*.
///
/// One multiply at the draw site fades **both** halves of a cloaked body:
/// `Painter::sprite`'s tint multiplies, and `ConRead::glyph_ink` carries the
/// authored alpha through, so a body's art and its glyph dim together with
/// no sixteenth `Painter` operation and no change to `paint.rs`. A decoy
/// draws through `painter.map` alone — see `Painter::sprite`'s own doc for
/// why nothing masquerading as a body may take that door.
///
/// Faded rather than hidden: a cloaked body is still a wall in
/// `reach::movement_field`, so the cell it stands on is already a tell, and
/// a tell is the right amount of information. A decoy is faded for the same
/// reason a cloaked body is — the player is meant to notice it is not real.
pub(super) const FADED_ALPHA: f32 = 0.35;

/// The arrow that hangs over whoever is acting: its width and its height as
/// fractions of a tile, and how far its point is held off the tile's top
/// edge at rest.
///
/// **It hangs *above* the tile rather than sitting in it**, which is what
/// keeps it out of every channel a tile already spends: the rarity bar owns
/// the top edge, the con earmark the top-left corner below it, the HP bar
/// the bottom edge, and the middle is the glyph or the sprite this arrow
/// exists to point at. Drawn inside the cell it would have to be small
/// enough to dodge all four, and an arrow that small is not the thing a
/// player finds by glancing.
///
/// The gap is what the bob swings out of: the arrow's rest position is its
/// *lowest*, so a lift can never carry it down onto the body.
pub(super) const TURN_ARROW_WIDTH: f32 = 0.44;
pub(super) const TURN_ARROW_HEIGHT: f32 = 0.30;
pub(super) const TURN_ARROW_GAP: f32 = 2.0;

/// How heavily a cell the acting body can still step to is washed — and, in
/// `AIM` rather than `PLAN`, a cell a `Radius` routine's centre could legally
/// land on. One constant for both, and that is safe **only** because the two
/// fields never draw on the same cell at once: `draw_tactical_map` suppresses
/// the reach field entirely while `placeable` is non-empty, so the yellow
/// outline *replaces* the blue one for as long as a splash is being aimed
/// rather than sitting over it. The two drawing at once was the first cut of
/// this feature's own bug — two washes stacked read as a colour nobody
/// authored, not as "you may both step here and throw there".
///
/// Under `PREVIEW_ALPHA` so a shaped routine reads over it, and
/// faint enough that the terrain under it stays legible: the wash says a
/// cell is *available*, and a cell whose kind it hid would make it say
/// something it does not know.
pub(super) const REACH_WASH_ALPHA: f32 = 0.13;

/// How thickly the reach field's outer edge is drawn — the placeable
/// field's too, `REACH_WASH_ALPHA`'s reason.
///
/// **The boundary is what makes the field findable, and the wash above is
/// what makes it readable.** At 0.13 of one hue over a near-black tile the
/// wash is legible only to somebody already looking at the right part of the
/// board; raising it far enough to catch the eye is the thing its own comment
/// refuses, since the terrain kind underneath is the other half of what a
/// cell is worth stepping to. A line at full strength spends no ink inside
/// the field at all.
///
/// Drawn along the field's **boundary** and not around each of its cells:
/// every cell outlined whole is a grid of boxes, which reads as sixty marks
/// rather than as one area, and the eye has to count them to find the edge.
pub(super) const REACH_EDGE_PX: f32 = 1.5;

/// How strongly the area preview washes the cells a routine would hit —
/// over `REACH_WASH_ALPHA`, so a shaped routine reads over the fields under
/// it.
pub(super) const PREVIEW_ALPHA: f32 = 0.22;

/// How wide the in-cover sunrise's horizon is, as a fraction of the body's
/// tile — wider than the sun on it, so the line reads as ground.
pub(super) const COVER_MARK_WIDTH: f32 = 0.34;

/// The radius of the in-cover sunrise's sun, as a fraction of the tile.
pub(super) const COVER_SUN_RADIUS: f32 = 0.11;

/// How many segments the sun's half-disc is drawn with. Enough to read as
/// round at the largest tile, few enough to stay a cheap convex fan.
pub(super) const COVER_SUN_SEGMENTS: u16 = 8;

/// Radians a second of the charge outline's pulse.
pub(super) const CHARGE_PULSE_RATE: f32 = 6.0;

/// The dimmest the charge outline gets, so it never vanishes mid-pulse.
pub(super) const CHARGE_PULSE_MIN_ALPHA: f32 = 0.35;

/// The tag on a charging body: `CHG k/N`.
pub(super) fn charge_tag_text(k: u32, n: u32) -> String {
    format!("CHG {k}/{n}")
}

/// One kind of wash the battle map lays on the ground, and the word the
/// legend gives it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Wash {
    Move,
    Cover,
    Danger,
    Aim,
    Hits,
    /// The walk a click on the hovered cell would commit — move, chosen.
    Path,
    /// Cells a party-side charge has locked.
    ChargeParty,
    /// Cells a hostile charge has locked: where the blow will land unless
    /// the victim steps out or the charger is stunned.
    ChargeHostile,
}

impl Wash {
    /// **`Hits` is the cursor's white and not `THREAT`** (bug 13). The
    /// preview used to be the danger field's red at a stronger alpha, and
    /// where the two met — a blast aimed across cells the body would be
    /// swung at walking to — they read as one wash. White is what the aim
    /// cursor outlines in, so "white is where your action lands" is one
    /// meaning on one channel, and red is left to inbound harm alone.
    pub(super) fn color(self) -> Color {
        match self {
            Wash::Move => palette::PLAN,
            Wash::Cover => palette::HEALTHY,
            Wash::Danger => palette::THREAT,
            Wash::Aim => palette::AIM,
            Wash::Hits => palette::EMPHASIS,
            // The move wash's own blue, laid over it at the preview's
            // stronger alpha and without an edge: the path is a subset of
            // the reach, so "the same hue, deeper" is "move, chosen" on
            // one channel rather than a sixth colour to learn.
            Wash::Path => palette::PLAN,
            Wash::ChargeParty => palette::PLAYER,
            Wash::ChargeHostile => palette::OFFLINE,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Wash::Move => "move",
            Wash::Cover => "cover",
            Wash::Danger => "danger",
            Wash::Aim => "aim",
            Wash::Hits => "hits",
            Wash::Path => "path",
            Wash::ChargeParty => "your charge",
            Wash::ChargeHostile => "their charge",
        }
    }
}

/// Every wash the board lays down this frame, in draw order, each with the
/// cells it covers. An empty field is left out.
///
/// **One movement wash a cell** (bug 13): move, cover and danger partition
/// `reachable`, danger over cover over move. Stacked at a wash's alpha each,
/// the three mixed into a colour nobody authored and the key named none of
/// it. Danger wins because an inbound reaction is louder news than a
/// boulder — a sheltered cell that still costs a swing is a cell that costs
/// a swing.
///
/// **The one list both the draw and the legend read.** Bug 13 was a board
/// of five washes nobody had named; a key derived separately from what was
/// drawn would be a second answer free to disagree with the first.
pub(super) fn board_washes(
    reachable: &[(i32, i32)],
    covered: &[(i32, i32)],
    provoking: &[(i32, i32)],
    overlay: &Overlay,
    party_charge: &[(i32, i32)],
    hostile_charge: &[(i32, i32)],
) -> Vec<(Wash, Vec<(i32, i32)>)> {
    let (placeable, preview, path) = (&overlay.placeable, &overlay.preview, &overlay.path);
    let mut washes = Vec::new();
    // Movement is drawn for **either side**, off `TacticalView::reachable`
    // alone and never off `player_turn`: a wash that appeared only for
    // bodies the player commands hid the half of the board a fight is
    // planned *against*. One colour for both — whose turn it is is the
    // turn arrow's answer, and tinting the wash by side too would put one
    // answer on two channels.
    //
    // **Suppressed while `placeable` is showing.** The aim field shipped as
    // "the movement outline, but yellow" — a replacement, not a second
    // field stacked on the first. Drawing both washed every reachable cell
    // twice and read as a colour nobody authored.
    //
    if placeable.is_empty() {
        let only = |field: &[(i32, i32)], louder: &[&[(i32, i32)]]| -> Vec<(i32, i32)> {
            field
                .iter()
                .filter(|c| louder.iter().all(|l| !l.contains(c)))
                .copied()
                .collect()
        };
        washes.extend([
            (Wash::Move, only(reachable, &[covered, provoking])),
            (Wash::Cover, only(covered, &[provoking])),
            (Wash::Danger, provoking.to_vec()),
            // Over the three movement washes, because it is a route
            // *through* them — a crossing through a danger cell has to show
            // the danger and the route both — and only while they show: a
            // walk is not on offer while the aim field is.
            (Wash::Path, path.to_vec()),
        ]);
    }
    // A locked charge is standing information, not a movement option, so it
    // is drawn whether or not the aim field is showing and under the
    // preview.
    washes.extend([
        (Wash::ChargeParty, party_charge.to_vec()),
        (Wash::ChargeHostile, hostile_charge.to_vec()),
    ]);
    // `placeable` is already empty for every shape but `Radius`
    // (`Game::tactical_placeable_cells`' own gate). The preview goes last,
    // over everything: a routine resolves wherever it is aimed whether or
    // not the body could walk there.
    washes.extend([
        (Wash::Aim, placeable.to_vec()),
        (Wash::Hits, preview.to_vec()),
    ]);
    washes.retain(|(_, field)| !field.is_empty());
    washes
}

/// The key to the board's washes: a swatch and a word for each one drawn
/// this frame, bottom-left inside the map pane.
///
/// **Only what is on the board**, which is `draw_tactical_map`'s return
/// value — two or three entries on an ordinary turn, none when nothing is
/// washed — so the key never teaches a colour the player cannot see.
/// Bottom-left because the turn strip and the tamper block take the
/// top-right, and the map pane's bottom border carries no strip to clear.
/// Rows wrap to the pane's width; only a pane narrower than one entry, or
/// shorter than the rows, draws no key.
pub(crate) fn draw_wash_legend(washes: &[Wash], pane: Rect, painter: &Painter, m: &Metrics) {
    if washes.is_empty() {
        return;
    }
    let pad = m.line_height * 0.35;
    let size = m.small();
    let swatch = f32::from(size) * 0.7;
    let space = painter.measure_ui_advance(" ", size);
    let gap = space * 2.0;
    let entry = |w: Wash| swatch + space + painter.measure_ui_advance(w.label(), size);
    // **Wrapped, never dropped.** A key that silently vanished when one row
    // did not fit left a narrow board with washes and nothing naming them —
    // bug 13 again at a smaller window.
    let room = pane.w - m.inset * 2.0 - pad * 2.0;
    let mut rows: Vec<Vec<Wash>> = vec![Vec::new()];
    let mut used = 0.0;
    for &wash in washes {
        let ew = entry(wash);
        let row = rows.last_mut().expect("rows starts with one");
        if !row.is_empty() && used + gap + ew > room {
            rows.push(vec![wash]);
            used = ew;
        } else {
            used += if row.is_empty() { ew } else { gap + ew };
            row.push(wash);
        }
    }
    let row_width =
        |row: &[Wash]| row.iter().map(|&w| entry(w)).sum::<f32>() + gap * (row.len() - 1) as f32;
    let widest = rows.iter().map(|r| row_width(r)).fold(0.0, f32::max);
    let w = pad * 2.0 + widest;
    let h = pad * 2.0 + m.line_height * rows.len() as f32;
    let x = pane.x + m.inset;
    let y = pane.y + pane.h - m.inset - h;
    if widest > room || y < pane.y {
        return;
    }
    painter.rect(x, y, w, h, Color::new(0.04, 0.06, 0.08, 0.88));
    painter.rect_lines(x, y, w, h, 1.0, palette::PANE_BORDER);
    for (i, row) in rows.iter().enumerate() {
        let baseline = y + pad + m.line_height * i as f32 + f32::from(size) * 0.8;
        let mut at = x + pad;
        for &wash in row {
            painter.rect(at, baseline - swatch, swatch, swatch, wash.color());
            at += swatch + space;
            painter.ui(wash.label(), at, baseline, size, palette::BODY);
            at += painter.measure_ui_advance(wash.label(), size) + gap;
        }
    }
}
