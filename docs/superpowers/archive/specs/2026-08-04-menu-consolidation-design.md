# Menu consolidation

The map screen binds 27 keys. This groups seventeen of them behind three:
`b` for the base, `p` for the party, `i` for the pack.

## The key map

Thirteen keys are retired from the map screen: `c` `w` `W` `G` `R` `U` `B`
`T` `f` `m` `M` `d` `v`. They keep working nowhere — the group menu is the
only path, so the flat surface actually shrinks rather than growing a second
system the help screen has to document.

Flat, and unchanged unless noted:

| | |
|---|---|
| Move | `hjkl`/arrows, `.` wait |
| World | `e` drain, `r` recharge, `C` collect, `t` trade, `u` symlink, `a` cast routine |
| Examine | `x` — **was perks**, now inspect-a-direction |
| Pack | `i` — **was inspect**, now opens the inventory directly |
| Meta | `L` `F` `s` `q` `?` `+/-` |
| Stack | `>` `<` `o` `g` |

`x` moves to inspect because `i` is being taken by the pack and perks is
moving into the party menu, which frees it. `x` for examine is the roguelike
convention, so the collision resolves in the direction a player would guess.

The pack gets no submenu: it is one screen, so `i` opens it directly.

| `b` — Base | | `p` — Party | |
|---|---|---|---|
| Deploy | surface | Companions | anywhere |
| Compile | anywhere | Manifest | anywhere |
| Cronjob | surface | Fuse | anywhere |
| Work it yourself | surface | Install routine | anywhere |
| Guard | surface | Extract | anywhere |
| Upgrade | surface | Perks | anywhere |
| Demolish | surface | | |
| Structures roster | anywhere | | |
| Research | anywhere | | |

Collect, trade and cast-routine stay flat despite being on-topic: they are
pressed every few turns while walking, and a group menu is a keystroke tax
on anything that frequent. Compile goes under `b` despite also being
frequent, because it belongs to the base in a way the player already thinks
of it.

## What decides a row is shown

A row is shown when

1. it is not surface-only while `Game::is_underground()`, **and**
2. the screen it opens would have at least one row.

Clause 2 is deliberately shallow — it asks only the *first* screen a row
opens. Cronjob can therefore list a worker and then land on an empty
structure picker. That is a cheap mistake rather than a dead end, because
Esc backs out into the base menu; chasing it properly needs a bespoke chain
predicate per row, which is exactly what clause 2 exists to avoid.

Clause 1 is a `surface_only` flag on the row descriptor rather than an
`is_underground()` check inside each predicate. It has to stay in step with
the engine's `require_surface` callers, and a readable table is what makes
that possible. Emptiness alone would not do the job: the six structure rows
read `view_entities` around the player's `Position`, which stays pinned to
the surface entrance tile while the party is underground, so those menus
would cheerfully list a base four frames above.

## One row table, two consumers

```rust
pub struct GroupMenuRow { pub label: &'static str, target: Mode }

impl App {
    pub fn base_menu_rows(&mut self) -> Vec<GroupMenuRow>;
    pub fn party_menu_rows(&mut self) -> Vec<GroupMenuRow>;
}
```

The handler resolves `rows[idx].target`; the renderer draws `rows[i].label`.
Both call the same function, and that is load-bearing rather than tidy: rows
are hidden dynamically, so a renderer building its own copy of the list
would drift out of index with the handler and `selected_index(2)` would open
a different screen from the one under the highlight.

Same seam as `Game::message_history` and `Game::structure_report` — app-core
owns what a screen's rows are, gui draws them.

## The extraction that makes clause 2 answerable

`view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS).filter(…)` is currently
written twice for each of four filters — once in `app/building.rs`, once in
`render/building.rs`. An availability predicate would be a third copy. Each
candidate list becomes one method on `App` instead:

| Method | Replaces copies in | Backs |
|---|---|---|
| `nearby_programs()` | cronjob + guard handlers, `render/building.rs` | Cronjob, Guard |
| `workable_structures()` | cronjob-structure + work-structure handlers, renderer | Cronjob, Work it |
| `nearby_structures()` | guard-structure + remove handlers, renderer | Guard, Demolish |
| `upgradeable_structures()` | upgrade handler, renderer | Upgrade |

This is the largest part of the diff and the reason the change spans two
crates.

## Navigation

`App::menu_origin: Option<Mode>` is set when a group menu dispatches to a
screen. Each screen's `self.mode = Mode::Playing` Esc arm becomes
`self.close_screen()`, which returns to `menu_origin.take()` or
`Mode::Playing`.

Completing an action still lands on the map — only Esc walks back up. So
placing a structure ends on the map, but backing out of the build screen
returns to the base menu.

## Gates

- app-core tests: a row hidden for emptiness and a row hidden underground;
  that a hidden row cannot be reached by index; that Esc returns to the
  group menu while completion does not; that each retired key is inert on
  the map screen.
- `render/meta.rs`'s help screen rewritten — it documents all 27 keys today.
- `README.md` and `CHANGELOG.md` swept for key claims this falsifies.
- `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
