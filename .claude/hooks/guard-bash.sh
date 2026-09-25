#!/usr/bin/env bash
# PreToolUse guard on Bash. Each rule is a trap that fired in this repo more
# than once while it was only a memory entry; the memory file named in each
# reason has the history. Hook output: deny / ask / allow-with-context.
set -euo pipefail

input=$(cat)
cmd=$(jq -r '.tool_input.command // ""' <<<"$input")
cwd=$(jq -r '.cwd // ""' <<<"$input")
# Match against the command with quoted strings and heredoc bodies removed, so
# a commit message or script that mentions a trapped command is not read as
# running it.
cmd=$(perl -0pe 's/<<-?\s*[\x27"]?(\w+)[\x27"]?.*?\n\1(\n|$)//gs; s/"(?:[^"\\]|\\.)*"//gs; s/\x27[^\x27]*\x27//gs' <<<"$cmd")

decide() { # $1 = deny|ask, $2 = reason
  jq -n --arg d "$1" --arg r "$2" \
    '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: $d, permissionDecisionReason: $r}}'
  exit 0
}

note() { # context for the model, command still runs
  jq -n --arg c "$1" \
    '{hookSpecificOutput: {hookEventName: "PreToolUse", additionalContext: $c}}'
  exit 0
}

dirty() { [ -n "$cwd" ] && [ -n "$(git -C "$cwd" status --porcelain 2>/dev/null)" ]; }

# `git add -A` / `--all` / `.` sweeps another agent's worktree gitlink under .claude/worktrees/.
if grep -qE '(^|[;&|[:space:]])git( -C [^ ]+)? add( [^;&|]*)? (-A|--all|\.)([[:space:];&|]|$)' <<<"$cmd"; then
  decide deny "Stage explicit paths: 'git add -A/--all/.' sweeps up other agents' worktree gitlinks (memory: never-git-add-all-with-another-agent-working)."
fi

# `git stash` (push/pop/apply/bare) on a clean tree pops someone else's stash.
if grep -qE '(^|[;&|[:space:]])git( -C [^ ]+)? stash([[:space:]]*($|[;&|])| (push|pop|apply|save|drop|clear|-))' <<<"$cmd"; then
  decide ask "git stash has popped another session's stash twice here (memory: git-stash-on-a-clean-tree-pops-someone-elses). Commit to a WIP branch instead?"
fi

# Discarding working-tree changes that may be a subagent's uncommitted work.
if grep -qE '(^|[;&|[:space:]])git( -C [^ ]+)? (checkout( [^;&|]*)? (--|\.)([[:space:]]|$)|restore( |$)|reset( [^;&|]*)? --hard|clean -[a-z]*f)' <<<"$cmd" \
  && ! grep -qE 'restore --staged' <<<"$cmd"; then
  if dirty; then
    decide ask "This discards uncommitted changes in a dirty tree; that has destroyed a subagent's work 3x (memory: never-git-checkout-a-subagents-uncommitted-work)."
  fi
fi

# Pushing needs an explicit ask from the user; a subagent once released to main.
if grep -qE '(^|[;&|[:space:]])git( -C [^ ]+)? push([[:space:]]|$)' <<<"$cmd"; then
  decide ask "git push needs the user's explicit go-ahead (memory: subagent-dispatches-must-forbid-push)."
fi

# A full cargo clean costs the ~3.5 minute cold Bevy build.
if grep -qE '(^|[;&|[:space:]])cargo clean([[:space:];&|]|$)' <<<"$cmd" && ! grep -qE 'cargo clean( [^;&|]*)? (-p|--package|--doc|--release)' <<<"$cmd"; then
  decide ask "Full 'cargo clean' forces a cold Bevy rebuild. Stale-path failures need only 'cargo clean -p feral-processes-engine -p feral-processes-app-core'; disk sweeps need only 'rm -rf target/debug/incremental'."
fi

# cargo piped into tail/grep reports the pipe's status, not cargo's.
if grep -qE 'cargo (test|build|clippy|check|run)[^;&]*\|' <<<"$cmd" && ! grep -q 'pipefail' <<<"$cmd"; then
  note "cargo is piped without 'set -o pipefail': the exit status is the last pipe stage's, not cargo's. Read the output for 'test result:' / 'error' rather than trusting the status (memory: cargo-exit-code-is-lost-through-a-pipe)."
fi

exit 0
