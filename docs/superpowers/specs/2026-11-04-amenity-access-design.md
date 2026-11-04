# Amenity access, not amenity count

**Status:** design agreed 2026-11-04. Plan: `../plans/2026-11-04-amenity-access.md`.

## Problem

Staff → Coverage flags a need short (`!`) whenever `built < needed`, where
`needed = ceil(staff × off-shift share / AMENITY_CAPACITY)`
(`game/staff_report.rs:175`). The player finds the count cumbersome: it asks for
Sandboxes and Log Analyzer Bays the base may not actually be short of. What the
player should act on is whether staff who go to use one **get in**.

## Decisions

- **Scope:** the two `services:` amenities, the Sandbox (Slack) and the Log Analyzer
  Bay (`defrag_bay`, Coherence). The Data Cache and the Repair Bay are unchanged.
- **`needed` is deleted** from `CoverageRow` and the screen. It is not kept as a hint.
- **A visit** is one `OffShift(need)` episode, ending either when the need reaches
  `content` (the `(Some(_), None)` arm in `offshift.rs` ~540) or when it is
  stranded (`strand_off_shift`). Each visit is tagged with one outcome:
  - `Served`: got a place without waiting.
  - `Waited`: was in reach of an amenity for its need, not granted a place, for
    at least one tick (the allocation in `systems.rs` ~239).
  - `Stranded`: no route (`strand_off_shift`).
  A siphon's removal of `OffShift` (`siphon.rs:72`) is not a visit.
- **Boxed in** stays derived live (`open_tiles < AMENITY_CAPACITY`), as it is today.
- **Short (`!`)** = `built == 0 || waited > 0 || stranded > 0 || !boxed_in.is_empty()`
  over the window.
- **Window:** the last `AMENITY_ACCESS_WINDOW_BUCKETS = 3` `base_ledger` buckets
  (`BUCKET_TICKS = 1000`), so about 2–3k ticks. A fix clears itself as visits age out.
- **Storage:** visits go through `base_ledger::emit` as a new
  `Event::AmenityVisit { need, outcome }`, folded into a new
  `Bucket::visits: BTreeMap<NeedId, VisitTally>` behind `#[serde(default)]`.
  That's additive field-named RON, so `SAVE_FORMAT_VERSION` doesn't change, and
  there's no new `Resource` (no shift in query iteration order). This reuses the
  instrumentation seam's one-door rule instead of adding a second history store.
- **The waited flag** lives on `components::OffShift` as `waited: bool`, and is
  **not saved**. A reload mid-visit can record a waited visit as served, which is
  acceptable. Re-inserting `OffShift` for the *same* need must keep the flag.
- **Alert:** a new `AlertKind::AmenityBlocked`, appended (variant names are save
  format). The subject is the need id. It posts when a `Waited`/`Stranded` visit
  is emitted and the need is not latched. The latch is
  `AlertBoard::amenity_blocked: BTreeSet<NeedId>`, not saved (the `depots_full`
  precedent). It clears when a visit is emitted while the window holds no
  waited/stranded visits for that need. Alerts only, never a log line. Text names
  the amenity, either the one it waited at or the one it was routed to:
  "Staff waited for a place at the Sandbox" / "Staff can't reach the Log Analyzer Bay".
- **Coverage row:**
  `{name:<9}  built {b} places {p}/{c} waited {w}/{v} stranded {s}{ !}  crit {n} low {n}  lost ~{x}%`
  (need names elided to 9 characters, no separators).
  The indented cause line under a short row says what to do:
  nothing built → the existing "build a {built_by}" line; waited → "full: build another {built_by}";
  stranded → "unreachable: lay floor to it"; boxed in → the existing boxed-in line.
  The text must fit (`the_widest_coverage_row_fits_the_content_rect`).

## Non-goals

No change to place allocation, routing, `lost_share`, the Repair Bay or the Data Cache.
No balance effect; `balance_sim` should not move.
