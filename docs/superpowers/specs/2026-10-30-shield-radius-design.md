# Shields cover a radius

ADR: [0035](../../adr/0035-shields-cover-a-radius.md) (Proposed).

## Intent

A Shield protects the ground around it rather than the whole base, so where you build one matters, and you can see what it covers.
In a siege you fight at the base it now does something: hits on what stands under it are softened, and you see it working.

## Goals

1. Each Shield covers a circle. The radius is `4 + 2 × (tier − 1)` tiles: 4, 6, 8, 10 and 12 at Mk1 to Mk5. *(user)*
2. Shields can be upgraded to Mk5, capped by zone like every other upgrade. *(user)*
3. Every tile the circle touches, even partly, is covered. *(user)*
4. **The circle shows** as a solid blue edge with a light see-through blue fill *(user)*:
   - while the pointer hovers a Shield, on the field map and on the siege board *(user: hover; Claude: both maps)*;
   - in the placement preview when deploying a Shield *(user)*;
   - when choosing an upgrade for a Shield, at the next tier's radius *(user)*.
5. **Sweeps:** a sweep's target gets 2 off for each armed Shield covering it. A target outside every circle gets nothing from Shields. *(user)*
6. **Fought siege:** an enemy hit on a body (staff, the player, a companion) or on a structure under a circle loses 2 for each covering armed Shield. *(user: flat −2, stacking)*
7. **Flash:** each softened hit flashes that Shield's circle at 25% fill, fading in and out quickly and softly. *(user)*
8. **Outline:** the blue pulse on structure outlines shows only on structures covered by an armed Shield. *(user)*
9. **Coils:** in a fought siege, only an armed Shield protects. A Shield that softened at least one hit spends one Charge Coil when the siege ends. *(user)*

## Non-goals

- The off-screen siege: unchanged, base-wide. *(user)*
- Town raids and the friendly-town garrison: unchanged, base-wide. *(Claude)*
- Turrets: their `raid_defense: 1` stays base-wide; they get no radius. *(Claude)*
- Staff tantrums and brawls: not enemy hits, not softened. *(Claude)*
- Ordinary battles away from the base. Wild programs never enter floor, so the only enemy fights at the base are sieges.

## Anti-goals

- Coverage worked out in more than one place. The sweep, the board, the outline and the drawn circle must agree tile for tile, so they all call one function.
- A circle on the siege board that is offset from the field. The board's coordinates are field minus `TacticalBattle::siege_origin`.

## Decided mechanics

- **Coverage** (*Claude*):
  - A tile `(x, y)` is covered by a Shield anchored at `(sx, sy)` with radius `r` when the distance from the anchor tile's centre to the nearest point of the tile's unit square is less than `r`. The anchor's own tile is always covered.
  - One pure function, for example `shield::covers(anchor, radius, tile) -> bool`, plus `covered_tiles(anchor, radius)` for drawing. It lives in a new engine module and its constants go in `tuning/`.
- **Data** (*Claude*, per the ADR):
  - `StructureDef::shield_radius: Option<ShieldRadiusDef { base, per_tier }>`, `#[serde(default)]`.
  - `shield.ron` gets `shield_radius: Some((base: 4, per_tier: 2))` and `upgrade: Some((max_tier: 5, cost: [("core_fragment", 12), ("cache_grain", 1)]))`. That follows the house cost pattern, and the cost is multiplied by the tier reached. An upgrade needs no tamed program, because the Shield runs no job.
- **Armed** means the existing `Game::defence_is_armed`.
- **Sweep** (`Game::run_raid`, *Claude*):
  - Defence = the base-wide defences (`raid_defense` with no `shield_radius`) + the covering armed Shields + the garrison.
  - Coils are spent by the covering Shields and the base-wide defences only. A Shield whose circle missed the target spends nothing.
  - Sweep damage can still reach 0, which keeps "without a scratch" and `EffectKind::Deflected`.
- **Body hit** (*Claude*):
  - Inside `Game::apply_damage_typed`'s mitigation, and only during a siege battle when the attacker is hostile.
  - The Shield cut is added to deflection. It shares `DEFLECTION_MAX` and the existing floor of 1.
  - `ignores_deflection` damage ignores it too.
- **Structure hit on the board** (*Claude*): both enemy paths, the structure branch of `tactical_attack` and `besieger_wreck`, cut the damage before `damage_structure`, never below 1.
- **Coverage on the board** is read from field positions. A seated body's or structure's board cell plus `siege_origin` gives its field tile.
- **Footprint** (*Claude*): a structure is covered when any cell of its footprint is covered. A sweep target with no `Position` counts as uncovered.
- **Mid-upgrade** (*Claude*): a Shield keeps its current tier's radius until the upgrade site finishes, as every upgrade does.
- **Turrets in a siege** (*Claude*): unchanged, they spend exactly as they do today. Only the Shield gets the "softened at least one hit" rule.
- **Armed** is checked per hit. Because coils are only spent at the end, a Shield armed at the start stays armed for the whole siege.
- **A fought siege that is lost or ends any other way** still spends one coil from each Shield that softened a hit (*Claude*).
- **Existing saves** (*Claude*): no migration. Old Shields come back at Mk1 radius 4, and structures outside it lose sweep cover. That is the intended balance change, and `CHANGELOG.md` says so. `dev-saves/` stays as it is. Tests that relied on base-wide Shields place their Shields in range.
- **Coil at siege end** (*Claude*): record which Shields softened a hit on the battle (unsaved is acceptable only if a reload mid-siege cannot double-spend or skip; otherwise persist it in `SiegeSave`). Spend one coil each when the siege closes.

## Visuals

- **Painter**: add a filled circle and a circle stroke to `crates/gui/src/paint.rs`. It is the only file that names egui (see `.claude/rules/drawing-seam.md`).
- **Circle** (*Claude*):
  - Centre on the anchor tile's centre and use radius `r` tiles in pixels, so the drawn edge is exactly the coverage boundary.
  - Edge: a solid blue stroke about 2 px wide.
  - Fill: the same blue at about 12% alpha.
  - Clip to the map pane.
- **Flash** (*Claude*):
  - The fill rises to 25% alpha over about 0.12 s and fades back over about 0.35 s, eased so it isn't harsh.
  - Each softened hit restarts that Shield's flash instead of stacking.
- **Engine to gui cue** (*Claude*):
  - A new `EffectKind` on the field queue carrying the Shield's anchor and radius, and a new `TacticalFxKind` on the board queue for the same.
  - Both are drained the way every cue is.
  - The gui draws the circle from the cue, not from a lookup.
- **Hover**: app-core exposes the hovered Shield's circle (anchor, radius) the way `tactical_hover_path` exposes a hover-derived overlay. The gui draws it.
- **Placement and upgrade previews** read the radius through the same engine function, at tier 1 and at the next tier.
- **The views carry everything the gui needs**: each Shield's anchor, radius, armed state and the covered-structure flag for the outline. On the board, `TacticalView` exposes `siege_origin` or board-space circles. The gui never computes coverage itself.

## Constraints

- RNG draw order in `run_raid` stays unchanged. Check it with `balance_sim`, and expect the sweep tests in `tests/raids.rs` and `tests/defences.rs` that rely on base-wide Shields to need their Shields placed in range.
- The `seams-sieges` and `seams-base` rules, the `seams` skill and the `seam:` graph entries that describe base-wide `raid_defense` get updated where they move.
- `assets/structures/README.md` and `CHANGELOG.md` get updated. The manual and README don't.
- The save format is unchanged unless the coil-at-siege-end record has to be persisted (see above).

## Left to the builder

- Module and function names, the exact alpha and timing values within the ranges above, and test structure.
