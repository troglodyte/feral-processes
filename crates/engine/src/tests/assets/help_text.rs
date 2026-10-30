use crate::tests::support::*;
use crate::*;

/// The shipped manual, `assets/help/`. Every page parses and every link
/// resolves — `HelpDb::load_dir` skips a malformed page with a warning
/// rather than refusing to start, which is right for a modder's directory
/// and would otherwise let a shipped page go missing in silence.
#[test]
fn every_shipped_help_page_parses_and_every_link_resolves() {
    let (db, warnings) = help::HelpDb::load_dir(&help_assets_dir()).unwrap();
    assert!(warnings.is_empty(), "{warnings:#?}");
    assert!(
        db.pages().len() >= 4,
        "the census must actually walk assets/help, or it passes vacuously"
    );
    assert!(
        db.pages().iter().any(|p| !p.links.is_empty()),
        "no shipped page cross-links another, so the further-reading list is \
         exercised by nothing but fixtures"
    );
}

/// The game's word for using a routine is **run** (or *invoke*); the thing
/// you did is an **invocation**. "Cast" and "spell" are the fantasy words
/// this setting does not use, and player-facing text may never carry them.
///
/// Asserted on whitespace-and-punctuation-delimited **tokens**, never on
/// substrings — `broadcast_storm` is a shipped ability id and "spelled out"
/// is ordinary prose, and a substring match would fail both while proving
/// nothing. Over parsed help pages rather than raw files, on
/// `no_shipped_help_page_names_a_hidden_key`'s reasoning: `assets/help/
/// README.md` is schema documentation, not a page.
///
/// This gate covers **authored content**, which is where the vocabulary
/// actually lives. Player-facing strings built in Rust are held by review;
/// there is no way to enumerate them from a test.
#[test]
fn no_player_facing_text_says_cast_or_spell() {
    const FORBIDDEN: &[&str] = &[
        "cast",
        "casts",
        "casting",
        "caster",
        "casters",
        "recast",
        "recasting",
        "spell",
        "spells",
    ];
    let (checked, complaints) = player_facing_census(FORBIDDEN);
    assert!(
        checked > 200,
        "the census read only {checked} strings, so it is not walking the shipped content"
    );
    assert!(
        complaints.is_empty(),
        "player-facing text uses the fantasy vocabulary — say \"run\"/\"invoke\" a \
         routine, and \"invocation\" for the noun:\n{}",
        messages(&complaints)
    );
}

/// Every authored player-facing string in the shipped content, walked once so
/// that a second vocabulary gate is a *call* rather than a second copy of the
/// walk. Returns how many strings were read — a census that reads nothing
/// passes vacuously — and one complaint per string carrying a forbidden word.
///
/// Matched on whitespace-and-punctuation-delimited **tokens**, never on
/// substrings: `broadcast_storm` is a shipped ability id and "spelled out" is
/// ordinary prose, and a substring match would fail both while proving
/// nothing.
///
/// Covers **authored content**, which is where the vocabulary lives.
/// Player-facing strings built in Rust are held by review; there is no way to
/// enumerate them from a test.
fn player_facing_census(forbidden: &[&str]) -> (i32, Vec<(&'static str, String)>) {
    let offends = |text: &str| -> Option<String> {
        text.split(|c: char| !c.is_ascii_alphabetic())
            .find(|w| forbidden.contains(&w.to_ascii_lowercase().as_str()))
            .map(|w| w.to_string())
    };
    let game = Game::new(3304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut checked = 0;
    let mut complaints = Vec::new();
    let mut check = |what: &'static str, id: &str, text: &str, checked: &mut i32| {
        *checked += 1;
        if let Some(word) = offends(text) {
            complaints.push((what, format!("{what} {id:?} says {word:?} in: {text:?}")));
        }
    };
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        check("ability", &def.id, &def.name, &mut checked);
        check("ability", &def.id, &def.description, &mut checked);
    }
    for def in game.world.resource::<crate::items_db::ItemDb>().all() {
        check("item", def.id.as_str(), &def.name, &mut checked);
        check("item", def.id.as_str(), &def.description, &mut checked);
    }
    for def in game
        .world
        .resource::<crate::structures::StructureDb>()
        .all()
    {
        check("structure", &def.id, &def.name, &mut checked);
        check("structure", &def.id, &def.description, &mut checked);
    }
    for def in game.world.resource::<crate::species::SpeciesDb>().all() {
        check("species", &def.id, &def.name, &mut checked);
    }
    let (help, _) = help::HelpDb::load_dir(&help_assets_dir()).unwrap();
    for page in help.pages() {
        check("help page", &page.id, &page.title, &mut checked);
        for block in page.blocks.iter() {
            match block {
                help::HelpBlock::Paragraph(text) | help::HelpBlock::Bullet(text) => {
                    check("help page", &page.id, text, &mut checked)
                }
                help::HelpBlock::Blank => {}
            }
        }
    }
    (checked, complaints)
}

/// A fight is an **intrusion**, and the vocabulary around it is security
/// rather than swordplay: an attempt *lands* or is *refused*, Integrity is
/// what refuses it, Bleed reads as a **leak**, Stun as a **stall**, and a
/// defended body **hardens**.
///
/// `player_facing_census`' second caller — the walk is shared, the word lists
/// are not. Two lists because the scope differs:
///
/// - The melee words are wrong on **every** surface.
/// - `bleed` and `stun` are wrong only where a *status* is being specified,
///   which is an ability. "Bleeds off the charge that makes a capture fail"
///   (`static_mesh`) and "Bleeds a hostile Overseer's own watchdog line back"
///   (`watchdog_tap`) are bleeding a line in the engineering sense, which is
///   the register this game wants, not the status the chip calls Leaking.
///
/// Deliberately absent from both lists:
///
/// - **damage** — `Damage 19-31 to every hostile program` is the terse spec
///   form an ability description is written in, and it matches the manifest's
///   own `Damage` row. What the reframe dropped is "damage" as the *unit* on a
///   narration line, which is Rust-side text this gate cannot see anyway.
/// - **swing** — `assets/help/60-your-base.md` swings at rock, which is
///   mining's established word (`swing_damage`, `durability / min_swings`).
/// - **strike** — `Barrier Slam`, `Burrow Strike` and `Ram` are species move
///   names that predate this and are content decisions, not vocabulary slips.
/// - **wound** — a homograph. `charge_coil` has "Power Cells wound into a
///   store", which is the past tense of *wind*, so only `wounded` is bannable
///   and nothing ships it.
#[test]
fn no_player_facing_text_uses_melee_vocabulary() {
    const MELEE: &[&str] = &[
        "wounded", "glance", "glances", "glancing", "brace", "braces", "bracing", "parry",
        "parries", "slash", "slashes", "stab", "stabs",
    ];
    const STATUS: &[&str] = &["bleed", "bleeds", "bleeding", "stun", "stuns", "stunned"];

    let (checked, melee) = player_facing_census(MELEE);
    assert!(
        checked > 200,
        "the census read only {checked} strings, so it is not walking the shipped content"
    );
    assert!(
        melee.is_empty(),
        "player-facing text uses melee vocabulary — a fight is an intrusion, and an \
         attempt lands or is refused:\n{}",
        messages(&melee)
    );

    let (_, status) = player_facing_census(STATUS);
    let in_abilities: Vec<(&str, String)> = status
        .into_iter()
        .filter(|(what, _)| *what == "ability")
        .collect();
    assert!(
        in_abilities.is_empty(),
        "an ability names a status the player sees under another word — Bleed is a \
         leak, Stun is a stall:\n{}",
        messages(&in_abilities)
    );
}

/// The complaint text out of a [`player_facing_census`] result, tags dropped.
fn messages(complaints: &[(&str, String)]) -> String {
    complaints
        .iter()
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn help_assets_dir() -> std::path::PathBuf {
    test_assets_dir().join("help")
}

/// The easter-egg census over the manual. It used to read `HELP_ROWS`, a
/// const in `crates/gui/src/render/meta.rs`; the manual is authored content
/// now, so it reads `assets/help/` and protects against the **user** editing
/// a page as well as against a developer editing a const.
///
/// Asserted on whitespace-delimited **tokens**, never on substrings.
/// `key name` is the binding idiom these pages use, so a token match catches
/// a real documentation of a key while staying satisfiable: the prose is
/// full of these letters inside ordinary words, and of the lowercase `t`
/// that legitimately binds trade.
///
/// Over parsed pages rather than raw files, so the directory's own
/// `README.md` — which names all three keys in the rule that forbids them —
/// is not itself a violation.
#[test]
fn no_shipped_help_page_names_a_hidden_key() {
    let (db, _) = help::HelpDb::load_dir(&help_assets_dir()).unwrap();
    let mut rows = 0;
    for page in db.pages() {
        for row in help::page_rows(page, help::WRAP_COLUMNS) {
            rows += 1;
            for token in row.split_whitespace() {
                assert!(
                    !matches!(token, "W" | "T" | "Z"),
                    "help page {} names a hidden key: {row:?} — see \
                     crates/engine/EASTER_EGGS.md",
                    page.id
                );
            }
        }
    }
    assert!(rows > 0, "the census must actually walk the manual");
}

/// The Excavation plan is the one verb in the game with no engine-side
/// refusal to teach it: `m` opens a mode, and a mode nobody can find is a
/// feature nobody has. So the manual is where it is learned.
#[test]
fn the_manual_binds_the_excavation_plan_key() {
    let (db, _) = help::HelpDb::load_dir(&help_assets_dir()).unwrap();
    // The key and the verb in one substring, deliberately: `"m "` alone is
    // already inside "collect from adjacent structures", so asserting on it
    // separately passes with every Excavation row deleted.
    let says = db.pages().iter().any(|page| {
        help::page_rows(page, help::WRAP_COLUMNS)
            .iter()
            .any(|row| row.contains("m — Excavation plan"))
    });
    assert!(says, "no help page binds the m key to the Excavation plan");
}

/// The Splice page of a structure's `c` screen has no other way to be found,
/// and the manual is also where removal's fragment price is told, so both
/// are held.
#[test]
fn the_manual_teaches_the_splice_page_and_what_removal_costs() {
    let (db, _) = help::HelpDb::load_dir(&help_assets_dir()).unwrap();
    let text = |id: &str| {
        let page = db
            .pages()
            .iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("no help page {id}"));
        help::page_rows(page, help::WRAP_COLUMNS).join(" ")
    };
    assert!(text("controls").contains("Splice,"));
    let implants = text("implants");
    assert!(implants.contains("core fragments"));
}

/// Two piles cannot share a tag in the base pane's PRODUCTION rows, where a
/// column of tags is read down for the pile that is filling, and a duplicate
/// fails silently: both rows draw, both look right.
///
/// `ItemDef::tag` derives from the name, so a collision is a content
/// accident rather than a code fault, which is why this is a census over the
/// shipped assets rather than an assertion inside `load_dir`. A mod's own
/// collision is its author's to settle with `abbrev`; nothing here refuses
/// their file.
///
/// Restricted to what the strip actually lists — `Material` and `Currency`,
/// the two categories `Game::base_stock` keeps. Etched disks are excluded
/// through `ItemId::etched_ability`, the existing derivation: every disk
/// derives the same family tag, "ED", by construction, and no `abbrev` could
/// be authored on a synthesised def to settle it. That exclusion is a known
/// gap and not a proof — a disk put into a Depot *does* reach the strip — but
/// it is one the family shares with itself rather than with anything a
/// content author wrote.
///
/// Tool carriers are **not** excluded, and the difference is that their tags
/// are already distinct: `ItemDef::tag` reads the first two words' initials,
/// so "Gear Puller Carrier" derives its tool's "GP" and not a shared
/// "Carrier" tag. A carrier colliding with an authored item is a real
/// collision on the strip, settled on the authored item's `abbrev` —
/// `guard_page.ron` conceded "GP" for exactly this reason.
#[test]
fn no_two_shipped_stock_items_share_a_tag() {
    let game = Game::new(921, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<crate::items_db::ItemDb>();

    let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut checked = 0;
    for def in db.all() {
        if def.id.part_of().is_some() || def.id.weapon_part_of().is_some() {
            continue;
        }
        if !matches!(
            def.category(),
            ItemCategory::Material | ItemCategory::Currency
        ) {
            continue;
        }
        checked += 1;
        let tag = def.tag();
        assert!(
            !tag.is_empty(),
            "{} derives an empty stock tag from its name",
            def.id.as_str()
        );
        if let Some(other) = seen.insert(tag.clone(), def.name.clone()) {
            panic!(
                "\"{other}\" and \"{}\" both tag as [{tag}] in the PRODUCTION rows — \
                 settle it with an `abbrev` on one of them",
                def.name
            );
        }
    }
    assert!(
        checked > 10,
        "the census walked {checked} stock items, which is too few to be reading \
         the shipped set at all"
    );
}
