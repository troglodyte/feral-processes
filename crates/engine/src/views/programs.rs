use super::*;

pub struct PetInfo {
    pub entity: Entity,
    /// The same glyph and colour this program is drawn with on the map, so a
    /// menu row can carry its icon rather than making the player match a name
    /// to a letter. `EntityView` already carries the pair for the same reason;
    /// the two lists overlap (see `render/building.rs`'s `draw_base_staff`,
    /// which reads a row's identity from an `EntityView` and its tier from
    /// here), and a program that looked like one thing on one screen and
    /// another elsewhere would be worse than no icon at all.
    pub glyph: char,
    pub color: GlyphColor,
    pub name: String,
    /// `Game::creature_short_label` — no tier, no species. For a cell too
    /// narrow for `name`'s full form; carry `rarity` alongside it (below)
    /// to show the tier some other way, e.g. a row colour, rather than
    /// parsing it back out of `name`.
    pub short_name: String,
    pub level: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    /// This program's slot in the active party, or `None` if it isn't a
    /// member. Slot order is mechanically meaningful — front slots draw more
    /// fire (see `battle::slot_aggro_weight`) — so a frontend showing the
    /// roster shows the number, not just membership.
    pub party_slot: Option<u32>,
    /// Which of the four roles this program is filling — see
    /// `Game::program_role`. `owned_pets` is sorted into a run per role, so a
    /// screen listing it can head each run by comparing this against the row
    /// above rather than walking the list twice.
    pub role: ProgramRole,
    /// What this pet is doing right now — see `Game::program_activity`.
    pub activity: String,
    /// This individual's rolled quality tier (see `components::Potential`),
    /// e.g. "Excellent (94%)" — `None` for a creature with no `Potential`
    /// (shouldn't happen for anything spawned going forward, but possible
    /// for an old save predating this component).
    pub quality: Option<String>,
    /// This individual's Assembly rung — see `components::Potential::
    /// roll_label`. `None` for a creature with no `Potential`, `quality`'s
    /// own rule.
    pub assembly: Option<String>,
    /// The same for Extraction.
    pub extraction: Option<String>,
    /// How many fusions deep this program's lineage is, 0 to `MAX_FUSIONS`
    /// — see `components::FusionCount`. At `MAX_FUSIONS` it can no longer
    /// be fused.
    pub fusions: u32,
    /// How many of this program's upgrade slots are spent, 0 to
    /// `MAX_COMPANION_REFACTORS` — see `components::Refactors`. Counts the
    /// percentage buffs only; a zone bump spends none and shows up as the
    /// zone tag on `name` instead.
    pub refactors: u32,
    /// How many Kernel Rings this program has open, 0 to
    /// `tuning::KERNEL_RING_MAX` — see `components::KernelRing`. Absent means
    /// 0, like `refactors` above. Each one raises this program's level
    /// ceiling and pays a talent point per level earned above the base cap;
    /// `Game::companion_level_cap` is what the ceiling itself reads off.
    pub ring: u32,
    /// How many talent nodes this program has bought — see
    /// `components::Talents`. The count rather than the list, because every
    /// screen that lists a row goes through `Game::talent_options`.
    pub talents: u32,
    /// This program's rare-spawn tier — see `components::Rarity`. Already
    /// spelled into `name` as a prefix by `Game::creature_label`; carried
    /// separately so a menu can also colour the row without parsing it back
    /// out of the string.
    pub rarity: Rarity,
    /// Whether this is the program equipped as the player's weapon (see
    /// `resources::WieldedProgram`). At most one row in a list carries it.
    pub wielded: bool,
    /// Which of this program's three equipment slots are filled, as the
    /// fixed-width `w|a|m` cell `Game::gear_tag` builds. Pre-formatted rather
    /// than three booleans because `CompanionInfo` shows the same cell on the
    /// status panel, and one loadout must not read two ways.
    pub gear: String,
}

/// Snapshot of the player's active companion, shown in the status panel
/// and during an intrusion.
pub struct CompanionInfo {
    pub entity: Entity,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    /// **Percentage points** — see `components::Stats::mitigation`.
    pub mitigation: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    /// Terse name of what commanding this companion in battle would do
    /// right now (e.g. "Rally") — see `Game::companion_ability_label`.
    /// Shown wherever a companion is listed outside battle, so the player
    /// can see what its Special would do. In battle the action menu carries
    /// the full label instead (see `Game::battle_action_options`).
    pub ability: String,
    /// The same `w|a|m` loadout cell the roster carries — see
    /// `PetInfo::gear`.
    pub gear: String,
}

/// One row of the manifest's roster rail -- see `Game::manifest_roster`.
#[derive(Clone, Debug, PartialEq)]
pub struct RosterRow {
    pub entity: Entity,
    pub name: String,
    pub glyph: char,
    pub color: GlyphColor,
    pub level: u32,
    pub in_party: bool,
    pub needs_you: Option<CallToAct>,
}

/// A figure the manifest ranks a subject on.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RankedStat {
    Damage,
    Attack,
    Mitigation,
    Power,
    /// Programs only: `Potential::quality_percent`.
    Potential,
    /// Programs only: the mean of the two build rolls.
    WorkAptitude,
}

/// Where a subject stands on one stat: `place` of `of`, with a 1-5 `pips`
/// reading of how far it sits between the weakest and the strongest.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RankRow {
    pub stat: RankedStat,
    pub place: u32,
    pub of: u32,
    pub pips: u8,
}

/// Every stat a subject is ranked on -- see `Game::subject_ranks`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectRanks {
    pub rows: Vec<RankRow>,
}

/// One place the compass can point at — see `Game::compass_targets`.
///
/// The tier of knowledge lands **only** in `label`, so "a name is earned by
/// walking there" is stated once in the engine and no surface re-decides
/// it. `distance` was the second half of that rule until it turned out to
/// be the wrong half: how far away a recorded place is, is not something
/// arriving teaches you, and withholding it left the first tier unable to
/// answer the question the compass exists for — *do I have the supplies to
/// get there.* A bearing with no figure is a direction to wander in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompassRow {
    pub target: crate::settlements::CompassTarget,
    /// The place's name once reached, a generic noun before that — the
    /// whole of what reaching somewhere buys.
    pub label: String,
    /// `game::stack::bearing`'s eight-point heading, reused rather than
    /// restated.
    pub bearing: &'static str,
    /// The same answer as an arrow, for the block the map draws. Carried
    /// rather than derived by the renderer from `bearing`: a `match` on
    /// those strings would need a fallback arm, and a fallback is how a
    /// ninth heading ships pointing nowhere.
    pub arrow: char,
    /// Chebyshev tiles, and answered for every row the derivation returns.
    pub distance: i32,
    pub visited: bool,
}

/// One program assigned to a structure — see `StructureReport::assignees`.
#[derive(Clone)]
pub struct Assignee {
    pub entity: Entity,
    pub label: String,
    pub kind: TaskKind,
    /// Ticks done and ticks needed for one cycle. Both 0 for a `Guard`,
    /// which never progresses — `systems::task_progress_system` ignores it.
    pub progress: u32,
    pub required: u32,
    /// The posted program's own level and health.
    ///
    /// Here rather than only on its manifest because a posted program is
    /// often not reachable from the map at all: at its post it is not drawn
    /// (`drawn_on_surface_map`), and `Game::find_target_in_direction` skips
    /// what is not drawn, so the structure's sheet is the one screen that can
    /// tell you how the program working it is doing. `None` for anything
    /// without the component, which in practice is nothing that can hold a
    /// `Task`.
    pub level: Option<u32>,
    pub hp: Option<(i32, i32)>,
}

/// One site standing on the Relay's board — see `Game::sortie_board`.
///
/// Every figure on it is a **call** rather than a copy: `ticks` is
/// `Game::sortie_duration(risk, battles)`, the same computation the
/// countdown runs, `views::BuildOrderRow`'s rule. A screen quoting one
/// number while the trip runs another is the failure that rule exists for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SortieRow {
    pub id: crate::sorties::SortieId,
    pub name: String,
    pub description: String,
    /// Steps above the sector baseline — the site's own offset, which is
    /// also what `ticks` is priced off.
    pub risk: u32,
    /// Fixed the moment the offer appeared, not rolled at dispatch, so this
    /// row can be quoted before the player signs for it.
    pub battles: u32,
    pub ticks: u64,
}

/// One trip currently away, as the Relay screen lists it — see
/// `Game::sortie_reports`.
///
/// Every figure is derived off the stored record at the moment it is read;
/// nothing here is a second tally kept in step by hand. Members and
/// casualties are rendered here and not by the renderer, `MemoryRow`'s
/// reason: a Permadeath casualty's entity is gone by the time this is drawn,
/// so its name has to be the one captured when it fell.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SortieReport {
    pub site: String,
    /// Who is still out there, by display name.
    pub members: Vec<String>,
    /// Who did not come back, named at the moment they fell.
    pub casualties: Vec<String>,
    pub kills: u32,
    pub xp: u32,
    pub battles_done: u32,
    pub battles_total: u32,
    /// Ticks still to run. A trip that has aborted still runs this out —
    /// the countdown was always going to take that long, and there is no
    /// teleport home.
    pub ticks_left: u64,
    /// A member has gone down and the remaining battles are being skipped.
    pub aborted: bool,
}

/// One thing a program remembers, as `Mode::CompanionMemories` lists it —
/// see `Game::memory_report`.
///
/// **The subject is rendered here and not by the renderer.** A `Species`
/// subject needs `SpeciesDb`, a `Structure` subject `StructureDb`, and a
/// `Program` subject the name captured on the record when it was written;
/// the renderer holds none of the three. It is also `render/stack.rs`'s
/// `cell_mark` rule — the match behind this field is exhaustive, so a
/// seventh `MemorySubject` variant fails to compile rather than shipping a
/// row that names nothing.
///
/// `intensity` and `age_ticks` are numbers rather than pre-rendered text, in
/// the other direction: neither needs the world, and pre-rendering them
/// would invent a second formatting seam for the one screen that reads this.
#[derive(Clone, Debug)]
pub struct MemoryRow {
    /// `memories::MemoryDef::name` — its first reader.
    pub name: String,
    /// `memories::MemoryDef::blurb` — likewise.
    pub blurb: String,
    /// What it is about, already named. `None` for
    /// `components::MemorySubject::Nothing`, which is a memory of an event
    /// rather than of a thing and so has nothing to name — an empty string
    /// would leave the renderer testing for one.
    pub subject: Option<String>,
    /// Signed and decayed to the current tick: `components::Memory::
    /// intensity`, projected rather than recomputed.
    pub intensity: f32,
    /// How long ago it last landed, in words — and **not** a tick count.
    /// The game has no player-facing unit of time: nothing in any screen or
    /// any log line has ever said "tick", so a figure here would be the
    /// first, and a number the player has no scale for is not an answer.
    ///
    /// Banded against the **def's own half-life**, which is the only
    /// yardstick that makes two memories comparable: 6,000 ticks is fresh
    /// for a mauling and ancient for a bad shift, and the same phrase on
    /// both would mean neither. Derived here rather than in the renderer
    /// because the half-life is on the def, which never reaches the page.
    pub age: String,
}

/// One row of `Mode::DownedPrograms`'s list — see `Game::downed_program_rows`.
///
/// `grade` is `items::DownedProgram::grade()`'s own answer, carried rather
/// than left for the renderer to re-fold from `condition`/`rarity`/`level` —
/// `message_history`'s rule, that a per-row transform belongs in the engine.
#[derive(Clone, Debug, PartialEq)]
pub struct DownedProgramRow {
    /// The species' display name, falling back to the raw id for a mod
    /// species since removed — `Game::downed_program_label`'s tolerance.
    pub name: String,
    pub level: u32,
    pub rarity: Rarity,
    /// 0..=100 — `items::DownedProgram::condition`, unrolled.
    pub condition: u8,
    pub boss: bool,
    pub grade: f32,
}

/// The extraction bench a screen names — `Game::extraction_bench`. Absent
/// entirely when none stands, so a renderer never has to read a tier of
/// zero as "none".
#[derive(Clone, Debug)]
pub struct ExtractionBenchView {
    pub name: String,
    pub tier: u32,
}

/// One installed tool and what it would do to the program on the block —
/// `Game::extraction_options`. Carries the tool's display name so the
/// renderer joins nothing: the phase-1 rows zipped this list against
/// `installed_tools()` positionally, which was structural but left the
/// renderer holding two sequences that had to stay in step.
#[derive(Clone, Debug)]
pub struct ExtractionOptionView {
    pub tool: ToolId,
    pub name: String,
    /// What this use costs in time, bench discount already applied.
    pub ticks: u64,
    pub preview: ExtractionPreview,
}

/// What a tool would draw out — the three categories answer in different
/// currencies, and the `Routines` one cannot answer with an outcome at all.
#[derive(Clone, Debug)]
pub enum ExtractionPreview {
    /// `Game::extraction_items`: every item some roll could grant, named
    /// without a count because the count is rolled at the act. Empty when
    /// even the band's top rolls to no units at all.
    Items(Vec<ItemId>),
    /// A `Routines` tool: how many unfamiliar routines the draw would come
    /// from (`routine_candidates`'s pool, counted rather than named — spec
    /// §4 "Concealment": extraction previews a downed *wild* program's kit,
    /// so a name here is exactly the leak the inspect sheet's `unseen`
    /// guards against). An outcome cannot be quoted either way — the draw
    /// has not happened, and making it happen to fill a menu row would let
    /// looking at the menu change what you get.
    Routine(usize),
    /// A `Routines` tool with an empty pool: the refusal `extract_program`
    /// would answer with, shown before it is spent.
    NothingToLearn,
    /// A `Gear` tool: each candidate item and its live chance, from
    /// `Game::gear_chances` — the same call the pull makes, so the quoted
    /// figure and the rolled one are one value rather than two. Tier
    /// scaling and any running `DropBoost` are already in these numbers.
    ///
    /// The first raw percentages on an *extraction* screen (spec §9.6) —
    /// `battle.rs` and `inventory.rs` already print odds elsewhere in the
    /// game — chosen over a name-only list because an upgraded Compiler
    /// must not read identically to a fresh one.
    Chances(Vec<(String, f32)>),
    /// An `Image` tool: the species `Game::image_yield` would teach, the
    /// same call `extract_program`'s `Image` branch grants from — a quoted
    /// species and a learned one cannot differ (todo #100 Task 5). Unlike
    /// `Routine`'s deliberate concealment, naming the species here is the
    /// point: spec §4 "Learning an image" says the row reads "image:
    /// Scrapper", not a count. An image already known reuses `NothingToLearn`
    /// rather than a second empty state — `image_yield` answers `None` for
    /// the same reason `extract_program` refuses before anything is spent.
    Image(SpeciesId),
}

/// One line of the SOCIAL tab: what a program makes of another.
#[derive(Clone, Debug)]
pub struct RelationshipRow {
    /// The live short label, or the name stamped on the memory once the
    /// subject has left.
    pub name: String,
    pub bond: crate::bonds::Bond,
    /// Signed, from `Game::opinion_about` with departure memories left out,
    /// so the row reads the bond as it stood; the band is derived from it.
    pub opinion: f32,
    pub gone: bool,
    /// The live subject's own glyph; `None` once it has left. Carried on the
    /// row because two programs can wear the same label, so no lookup by
    /// name can say which one a row is.
    pub glyph: Option<(char, GlyphColor)>,
}

/// One line of a rendered conversation: who says it, already named.
#[derive(Clone, Debug, PartialEq)]
pub struct SpokenLine {
    pub who: String,
    pub text: String,
    /// The program whose page this is said it. A flag and not a comparison
    /// of `who`, because two programs can wear the same label.
    pub own: bool,
}

/// One conversation as the TALK tab draws it, slots already filled.
#[derive(Clone, Debug, PartialEq)]
pub struct ExchangeView {
    pub lines: Vec<SpokenLine>,
}

/// What the SOCIAL tab draws for one owned program.
#[derive(Clone, Debug)]
pub struct SocialView {
    /// Strongest `|opinion|` first.
    pub relationships: Vec<RelationshipRow>,
    /// At most two phrases, from `Game::known_for`.
    pub known_for: Vec<String>,
    /// How much it talks, `Sociability::name`: the reason one program is in
    /// more conversations than another.
    pub sociability: &'static str,
}

/// Which species a breeding would yield, in display names.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BreedSpeciesPreview {
    /// Same species twice, or an authored hybrid for the pair.
    Certain(String),
    /// An unauthored cross: either parent's species, by chance.
    OneOf(String, String),
}

/// The lowest and highest one of the child's six rolls can land on.
#[derive(Clone, PartialEq, Debug)]
pub struct RollRange {
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
}

/// What `Game::breed` would make of two programs, before anything is spent.
#[derive(Clone, PartialEq, Debug)]
pub struct BreedPreview {
    pub species: BreedSpeciesPreview,
    pub generation: u32,
    pub rolls: Vec<RollRange>,
}

/// A child still in a bay.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IncubatingChild {
    /// Display name.
    pub species: String,
    pub generation: u32,
    /// Zero once it is due. A screen turns this into a vague word.
    pub ticks_left: u64,
    /// Due, but the roster is full, so it waits in the bay.
    pub held: bool,
}

impl IncubatingChild {
    /// How far off the hatch is, in words — the player has no scale for a
    /// tick count. Banded against `INCUBATION_TICKS`, the span the child
    /// started with.
    pub fn when(&self) -> &'static str {
        let span = crate::tuning::INCUBATION_TICKS;
        match self.ticks_left {
            _ if self.held => "roster full, waiting",
            0 => "ready",
            t if t * 3 <= span => "ready soon",
            t if t * 3 <= span * 2 => "ready in a while",
            _ => "ready much later",
        }
    }
}

/// One incubation slot of a bay.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IncubationView {
    pub child: Option<IncubatingChild>,
}
