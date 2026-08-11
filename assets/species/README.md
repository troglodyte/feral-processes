# Custom species (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup.

## Schema

```ron
(
    id: "unique_snake_case_id",   // must be unique across all species files
    name: "Display Name",
    glyph: 'x',                   // single character shown on the map
    color: Cyan,                  // one of: White, Gray, Green, DarkGreen, Red,
                                   //         Yellow, Blue, Magenta, Cyan, Brown,
                                   //         Orange
                                   // Only shown as-is for a tamed/companion
                                   // program. A *hostile* one is recolored on
                                   // the map by difficulty relative to the
                                   // player's current power (see
                                   // `difficulty_color` in lib.rs) — Green,
                                   // Yellow, Orange, or Red, or Magenta if
                                   // `is_boss` — so this field only matters
                                   // once it's compiled.
    base_hp: 60,
    base_atk: 6,
    base_def: 3,
    taming_difficulty: 0.4,       // 0.0 (trivial) .. 1.0 (very hard) to compile/tame
    habitats: [OpenGrid, Mainframe],
    // Biome options: DataVoid, StaticField, NullSector, Mainframe, OpenGrid, BlackIce, Platform
    // (DataVoid and BlackIce are unwalkable barrier terrain — don't list them as a habitat)
    // Platform is the floor of a player's base. Nothing the world generates uses it — it
    // only exists where a Home has been deployed, and it travels with the base between
    // zones. No shipped species lists it, which is exactly what makes a base free of wild
    // spawns. Listing it here will make your species spawn inside player bases; do that
    // deliberately, not by accident.

    // Optional; can be left out entirely (defaults to 10). This species'
    // initiative baseline. Each round every combatant in the fight — your
    // whole party and every wild program — rolls `base_speed + d10` and
    // acts in descending order, so a faster species tends to strike first
    // without ever being guaranteed to. The shipped roster spans 6
    // (Construct, a wall) to 14 (Sprite, a spark); the player rolls from
    // 11. Leaving this out puts your species at the roster average, which
    // is why an existing species file predating this field keeps working
    // untouched.
    //
    // The same number also sets this species' pace at a machine: posted to
    // a cronjob, it scales how long a work cycle takes, read as a *distance
    // from 10* exactly the way `base_int`, immediately below, explains for
    // extraction odds — 10 costs nothing either side, above it is faster,
    // below it is slower. The shipped extremes: Construct at 6 takes a
    // fifth longer per cycle (a Mining Node's 10 ticks becomes 12, a
    // Fabricator's 30 becomes 36), Sprite at 14 takes a fifth less (10
    // becomes 8, 30 becomes 24), and it can never scale below one cycle per
    // tick, however extreme a modded value. The player has no species and
    // works at the baseline, so a machine's own `ticks_per_unit` (see
    // `assets/structures/README.md`) is exactly what working it by hand
    // costs — unchanged from before this field applied to work at all. That
    // baseline is 10, one lower than the 11 the player rolls for initiative
    // above: two different constants for two different rolls, not a typo —
    // `PLAYER_BASE_SPEED` gives the player a slight edge in a fight, while
    // the work side deliberately sits at the same zero point (`DEFAULT_
    // BASE_SPEED`) every species does, so posting is judged against the
    // same 10 a modded species with no `base_speed` field extracts at.
    //
    // Initiative and work rate are one field, not two, and that's
    // deliberate: "the sprite is quick" is meant to read as quick in a
    // fight and quick at a machine both. There is no way to tune the two
    // apart — that's the design, not an oversight, if you're modding one
    // half only.
    base_speed: 12,

    // Optional; can be left out entirely (defaults to 10). How good this
    // species is at *extracting* — posted to a Mining Node or any other
    // producing structure, it changes how often a cycle fizzles rather than
    // what a successful one pays out. The shipped roster spans 5 (Construct
    // and Glitch, neither of them thinkers) to 15 (SubProcess); the player
    // works a node at 10.
    //
    // That reliability roll only exists at all on a structure whose `work`
    // def sets `level` (see assets/structures/README.md) — a Power Conduit's
    // `work` has no `level`, so every cycle there is a guaranteed yield and
    // base_int has nothing to act on. It only matters at a producer that
    // opted into the chancier variant.
    //
    // The number is read as a *distance from 10*, not as an absolute, which
    // is worth knowing before you tune it: 10 contributes exactly nothing,
    // above it helps and below it hurts. That is why a species file written
    // before this field existed doesn't merely keep parsing — it keeps
    // extracting at precisely the rate it always did.
    //
    // Deliberately not tied to how tough the species is. A Sprite out-mines
    // a Sentinel, and the roster is authored so every difficulty tier has
    // both a sharp program and a dull one on it. If you are adding species,
    // keep that true: aptitude that climbs with tier is just the difficulty
    // ladder wearing a second name, and the player learns nothing from it.
    base_int: 12,

    moves: [
        (name: "Move Name", power: 8),
        (name: "Other Move", power: 5),

        // Optional per-move (defaults to false). A pack fights as species
        // groups, and only the front two are close enough to swing — a
        // group standing further back can use *only* its moves flagged
        // `ranged: true`, and idles with a flavour line if it has none.
        // Leaving this out makes a move melee, exactly how every move
        // behaved before the field existed. Give a species at least one
        // melee move regardless, or it does nothing in the front rank.
        (name: "Reaching Move", power: 6, ranged: true),

        // Optional per-move; omit `effect` entirely for a plain damage-only
        // move. If set, landing this move has a `chance` (0.0-1.0) to also
        // inflict a status condition on the target for `duration` battle
        // rounds, on top of its direct damage. Those rounds are the ones
        // *after* the round it landed in — a `duration: 1` stun costs its
        // victim the next round's action, not a round it may already have
        // acted in. A combatant can only carry one status condition at a
        // time — a fresh one overwrites whatever was active. `kind: Bleed`
        // deals `power` extra damage at the end of every round it's active;
        // `kind: Stun` causes the afflicted side to lose their next action
        // instead (`power` is required by the schema but unused for Stun —
        // just set it to 0).
        (name: "Corrupted Move", power: 6, effect: Some((
            kind: Bleed,       // or `Stun`
            chance: 0.4,
            duration: 3,
            power: 3,
        ))),
    ],
    work_resource: Some("core_fragment"),  // or `None` for no salvage drop
    // Despite the name, work_resource does not decide what a tamed member of
    // this species gathers, and does not gate whether it can be posted to a
    // cronjob — any program can work any structure, and a cronjob's output
    // comes from the structure's own `produces`. What it actually sets is
    // what *killing* a wild one drops. It has two other readers: destroying
    // a Nest of this species pays out from the same field
    // (`Game::grant_nest_cache`), and the inspection view names it as the
    // species' yield.
    //
    // work_resource (above) and equipment_drop (below) both take any item
    // id from assets/items/*.ron — see assets/items/README.md for the
    // schema, and the top-level README's "Item ids" for the full set.

    // Optional; omit entirely for no chance of a gear drop. If set, defeating
    // or decompiling this species has a chance (0.0-1.0) to additionally
    // drop one piece of equipment, independent of `work_resource`.
    //
    // Prefer the item side for new content: an item's own `droppable` lists
    // every species that drops it, so adding a piece of gear is one new file
    // rather than an edit to each species that should drop it. No shipped
    // species uses `equipment_drop` any more for that reason. It remains
    // fully supported — a species mod written against it keeps working, and
    // the two are merged per kill, an item named on both sides being rolled
    // once at the better chance.
    equipment_drop: Some(("firewall_plating", 0.3)),

    // Optional; can be left out entirely (defaults to false). If true, this
    // species is a boss: it's excluded from the normal per-tile habitat spawn
    // roll and spawns in its place only rarely (see `BOSS_SPAWN_CHANCE` in
    // the engine), rendered bold on the map and tagged "[BOSS]" in the
    // inspect/battle screens. Defeating one guarantees a cache of 3-6 Portal
    // Fragments instead of the flat drop chance every other species rolls.
    // There's no separate engine-side stat multiplier for a boss — make
    // `base_hp`/`base_atk`/`base_def` tough here directly (a boss's stats
    // still double per zone level like any other species, on top of this).
    //
    // A boss can also never be decompiled: it's refused as a target when the
    // action is chosen, so it costs neither the round nor the catalyst, and
    // the inspect and battle screens quote no odds for it. That's what lets
    // you make the stats above genuinely huge — they'd otherwise arrive in
    // the player's roster, where fusion compounds them. Since fusing needs
    // two tamed programs, it follows that a boss can never be fused either.
    // Set this flag and `taming_difficulty` stops mattering for the species.
    //
    // A boss is also the one thing that never rolls a rare tier — see the
    // note on those below.
    is_boss: true,

    // Optional; can be left out entirely (defaults to empty). This is not
    // the menu a tamed program offers — it's what gets *installed* into a
    // bounded number of routine slots, once, at specific moments. What ends
    // up commandable in battle is always whatever currently occupies those
    // slots (see "Routines and slots" below), not this list re-read live.
    //
    // Abilities themselves are data: each entry names an `id` from
    // `assets/abilities/`, whose README documents what an ability can do
    // (single- and multi-target damage, debuffs, heals and buffs, and the
    // cooldown that is the whole price of one). Nothing about an ability is defined
    // here — only which ones this species grants, and when.
    //
    // `level` is optional and defaults to 1, meaning the ability installs
    // as soon as the program is tamed (or as soon as slots exist for it —
    // see below). A higher number gates it until the companion reaches
    // that level; companions cap at level 12, so anything above that is
    // permanently unreachable.
    //
    // An id that doesn't exist is dropped with a logged warning and the
    // rest of the species still loads — a program missing one ability is
    // still perfectly playable.
    abilities: [
        (id: "hot_patch"),
        (id: "redundancy_sync", level: 7),
    ],

    // ## Routines and slots
    //
    // A tamed program's abilities live in a small, level-derived number of
    // routine slots (one more every two companion levels, six at most —
    // see `tuning::COMPANION_ROUTINE_SLOT_*`), and what a slot holds is
    // installed at specific moments, not recomputed on the fly:
    //
    //   - **Tame or fusion time.** Every entry above whose `level` is at
    //     or below the program's current level installs, in the order
    //     written, up to however many slots exist yet. If `abilities` is
    //     empty, or none of it has unlocked yet (this species' *first*
    //     unlock is above level 1), the program starts on the fallback
    //     ability, `priority_boost`, instead — so an ability-less or
    //     not-yet-unlocked program still has *something* commandable, and
    //     that filler is itself obtainable by extraction (see
    //     `assets/structures/README.md`).
    //   - **Every level-up.** Whichever entries this level-up's range
    //     newly qualifies for try to install. A free slot takes it
    //     directly. A full kit still takes it if `priority_boost` occupies
    //     one of the slots — the fallback is explicitly a placeholder for
    //     "nothing real yet," so a genuine unlock arriving displaces it
    //     rather than losing to it. Only when every slot already holds a
    //     *real* routine (installed, researched, or another innate
    //     ability) is the unlock logged as lost, permanently — the window
    //     to install it has passed. No shipped species can reach that
    //     genuine-loss state; the closest any comes is exactly the eviction
    //     case above (e.g. a species whose only ability unlocks above
    //     level 1, like the Scrapper's `cascade_overflow` at level 3).
    //
    // Nothing here is permanently welded in: an innate routine can be
    // popped back out (`m` in the routine panel) same as any installed
    // one, freeing the slot and handing back a loose item that installs on
    // any program — including a different species entirely.

    // Optional; can be left out entirely (defaults to 1.0). Multiplies this
    // species' per-level stat growth (see `progression::add_xp`) for a tamed
    // member of it — 1.0 grows at the standard flat rate; a higher-tier
    // species can set e.g. 1.5 to out-grow an easy one level for level. Only
    // affects a *tamed* member's growth as it levels up — a wild spawn's
    // stats (`base_hp`/`base_atk`/`base_def`, zone-scaled) are unaffected.
    // The base roster uses roughly 1.0 for Easy species, 1.25 for Medium,
    // 1.5 for Hard, and 2.0 for bosses.
    growth_multiplier: 1.25,

    // Optional; can be left out entirely, and so can any individual
    // category. Each is a multiplier on the *magnitude* of abilities in
    // that category when a member of this species casts them: 1.1 is a 10%
    // stronger heal, 0.8 a 20% weaker one. Clamped to 0.5-2.0 at load; a
    // non-finite value (RON accepts bare `NaN`/`inf`) skips the whole file
    // with a warning.
    //
    // The five categories match what an ability's `effect` does — see
    // ../abilities/README.md. `Cleanse` and `Decompile` have no magnitude
    // and so have no affinity.
    //
    // This applies to whatever is *installed* in the program's routine
    // slots, not only to the abilities listed above — and a routine can be
    // popped out and installed on a different species entirely. So a
    // species with a strong `heal` and no innate heal is not a mistake:
    // it's a reason to spend a researched heal routine on that program.
    //
    // The manifest screen shows both categories if you name two or fewer.
    // Name three or more and only the first is shown, with the rest
    // collapsing into a single "+N more" line — the note itself costs a
    // row, so three declared categories show one, not two. Name only one
    // or two if you want every one of them visible; a species naming three
    // or more still works, it just won't all be shown on the same screen.
    affinities: (heal: 1.4, damage: 0.85),

    // Optional; can be left out entirely (defaults to false). If true,
    // this species can spawn as a Nest instead of an ordinary lone
    // creature/pack during habitat spawning: a stationary, destructible
    // object that keeps 2-5 guardians of this species tethered within 5
    // tiles of it, respawning any that are killed or tamed 10 ticks
    // later, until the nest itself is destroyed (walk into it to attack
    // it — it never attacks back). Never applies to a boss species,
    // regardless of this flag.
    can_nest: false,

    // Optional; can be left out entirely (defaults to empty). Cosmetic
    // lines a tamed member of this species says in a fight. Nothing reads
    // them but the message log — they change no stat, cost no turn and
    // resolve no round.
    //
    // Written as a *verb phrase* rather than as speech, because the line is
    // logged with the program's name in front of it: "Drone 2 circles once,
    // unimpressed." Start it lowercase and end it with a full stop.
    //
    // A species with none still speaks — the engine has a generic set it
    // falls back to — so leaving this out costs nothing. Give it two or
    // more and they cycle in order.
    taunts: [
        "circles once, unimpressed, and pings them again.",
    ],
)
```

The filename doesn't matter to the loader (only the `id` field does), but
name it after the species for readability, e.g. `crawler.ron`.

## Rare tiers are engine-rolled, not a species field

There is deliberately no `rarity` field, and adding one to a `.ron` file
does nothing. A wild spawn independently rolls **Optimized** or
**Overclocked** — a multiplier on all four stats, on top of the individual
±20% roll — and that happens per *individual*, not per species: every
ordinary species can produce one.

The chances and the multipliers live in `crates/engine/src/tuning.rs`
(`SILVER_SPAWN_CHANCE`, `GOLD_SPAWN_CHANCE`, `SILVER_STAT_MULT`,
`GOLD_STAT_MULT`) rather than here, for the same reason every other
difficulty knob does: content is moddable, how hard the game is, is not.

Two spawns never roll a tier, and both are relevant when authoring a
species. A `is_boss: true` species is excluded, because its stats are
already hand-authored and a blanket multiplier would discard that tuning —
so make a boss as tough as you want it to be, here, and nothing will scale
it further. And nothing rolls one inside the opening ring around the
player's landing site, which is what keeps a fresh run winnable.
