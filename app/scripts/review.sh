#!/bin/bash
# Runs the fresh Codex and Claude reviews of docs/quality/agent-workflow.md on a frozen snapshot.
# Usage: bash app/scripts/review.sh <effort> <base> <revision> <brief-file>
set -euo pipefail

if (($# != 4)); then
  echo 'Usage: bash app/scripts/review.sh <effort> <base> <revision> <brief-file>' >&2
  exit 2
fi
effort="$1"
case "$effort" in
  ultra | persistent) claude_effort=max ;;
  *) claude_effort="$effort" ;;
esac
base="$(git rev-parse --verify "$2^{commit}")"
revision="$(git rev-parse --verify "$3^{commit}")"
brief="$(realpath "$4")"

primary="$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")"
frozen="$primary/.worktrees/review-${revision:0:12}"
out="$(mktemp -d)"
prompt="$out/prompt.txt"

git worktree prune
if [[ -e "$frozen" ]]; then
  git worktree remove --force "$frozen"
fi
git worktree add --quiet --detach "$frozen" "$revision"
trap 'cd "$primary" && git worktree remove --force "$frozen"' EXIT

{
  cat "$brief"
  printf '\nBase revision: %s. Reviewed tree: %s.\nFrozen diff (git diff %s %s):\n\n' \
    "$base" "$(git rev-parse "$revision^{tree}")" "${base:0:12}" "${revision:0:12}"
  git diff "$base" "$revision"
} >"$prompt"

cd "$frozen"
codex exec --model gpt-5.5 --config model_reasoning_effort="$effort" \
  --config mcp_servers.todoist.enabled=false --sandbox read-only --ephemeral - \
  <"$prompt" >"$out/codex.txt" 2>&1 &
codex_pid=$!
claude --model claude-opus-5-5 --effort "$claude_effort" --print --permission-mode dontAsk \
  --tools Read,Grep,Glob --strict-mcp-config --no-session-persistence \
  --add-dir "$HOME/.cargo/registry/src" "$(rustc --print sysroot)/lib/rustlib/src/rust/library" \
  <"$prompt" >"$out/claude.txt" 2>&1 &
claude_pid=$!
status=0
wait "$codex_pid" || status=1
wait "$claude_pid" || status=1

for reviewer in codex claude; do
  verdict="$(grep -E '^`?VERDICT: (HOLDS|FAILS)`?$' "$out/$reviewer.txt" | tail -n 1 || true)"
  echo "$reviewer: ${verdict:-no verdict} ($out/$reviewer.txt)"
  [[ -n "$verdict" ]] || status=1
done
echo "reviewed tree: $(git rev-parse "$revision^{tree}")"
grep -m 1 '^session id:' "$out/codex.txt" || true
exit "$status"
