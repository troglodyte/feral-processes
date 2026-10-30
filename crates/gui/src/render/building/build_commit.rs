use super::*;

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
pub(crate) const DEPLOY_NEEDS_A_PROGRAM: &str =
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
pub(crate) const LAST_PROGRAM_CLAUSE: &str =
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
pub(crate) const DEPLOY_NEEDS_A_SECOND_PROGRAM: &str =
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
pub(crate) fn floored_by_the_last_program(game: &mut Game) -> bool {
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
pub(crate) fn no_candidates_refusal(tier: u32, last_program: bool) -> String {
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
pub(crate) struct BuildCommit {
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
    /// Whether this commits a program to a wrecked structure's rebuild site
    /// rather than filing a new order, which changes what the picker's
    /// title, warning and footer call it.
    pub rebuild: bool,
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
    pub(crate) fn goal(&self) -> BuildGoal {
        match self.to_tier {
            Some(to_tier) => BuildGoal::Upgrade { to_tier },
            None => BuildGoal::New,
        }
    }

    pub(crate) fn tier(&self) -> u32 {
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
    pub(super) fn prompt(&self) -> String {
        let what = match (self.to_tier, self.rebuild) {
            (Some(to_tier), _) => format!("Upgrading the {} to Mk{to_tier}", self.label),
            (None, true) => format!("Rebuilding the {}", self.label),
            (None, false) => format!("Deploying the {}", self.label),
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
pub(crate) fn build_commit(app: &mut App) -> Option<BuildCommit> {
    let pending = app.pending_build.clone()?;
    let (label, to_tier, cost) = match pending.clone() {
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
        PendingBuild::Rebuild { site } => (
            app.game.as_ref().and_then(|game| {
                let kind = game
                    .awaiting_program_sites()
                    .into_iter()
                    .find(|s| s.site == site)?
                    .structure;
                game.buildable_structure_defs()
                    .into_iter()
                    .find(|def| def.id == kind)
                    .map(|def| def.name)
            }),
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
        rebuild: matches!(pending, PendingBuild::Rebuild { .. }),
    })
}
