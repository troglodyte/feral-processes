# 0012 Open questions from the review

Status: Open (2026-10-16)

Things the review found that need a decision from the user, not an ADR.
Move each to its own record, or strike it out, once answered.

## Roster views show raw `atk`

`PetInfo` (`crates/engine/src/game/party.rs:770`) and `CompanionInfo` (`party.rs:583`) fill `atk` from `stats.atk`.
`PlayerStatus` (`party.rs:305`) and `PartySlotView` (`game/combat_round.rs:978`) fill it from `effective_atk`, which folds perks and gear.
Either the roster is meant to show base stats and the battle view effective ones, or two of the four views are wrong.
If the former, the field should be named for what it is in each view.

## Does the executor hypothesis explain the suite-only flakes?

ADR 0001 shows the iteration command and the gate run different executors.
Whether that is the cause of `posted-worker level-up`, `research underground`, `creation allowance` and `sharper-program mining` flaking only in the full suite is untested.
The test is cheap: pin `ExecutorKind::SingleThreaded` under `--workspace` and run each thirty times.

## Should `deny_unknown_fields` apply to modded content?

ADR 0010 proposes it for every asset kind.
A stricter parser rejects a modder's typo instead of defaulting it, which is the moddability rule's intent, but it also rejects a file written for a newer schema by an older binary.
The `#[serde(default)]` rule covers the other direction.
