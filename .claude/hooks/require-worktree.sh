#!/bin/bash
# Blocks file edits in the primary checkout; agents work only in their own worktree (AGENTS.md, Working agreement).
set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
  echo 'Blocked: the worktree hook needs jq; run bash app/scripts/validate.sh doctor' >&2
  exit 2
fi
path="$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty')"
[[ -n "$path" ]] || exit 0

dir="$(dirname "$path")"
while [[ ! -d "$dir" ]]; do
  dir="$(dirname "$dir")"
done

if ! git_dirs="$(git -C "$dir" rev-parse --path-format=absolute --git-dir --git-common-dir 2>/dev/null)"; then
  exit 0
fi
repo_common_dir="$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)"
if [[ "$(sed -n 1p <<<"$git_dirs")" == "$repo_common_dir" && "$(sed -n 2p <<<"$git_dirs")" == "$repo_common_dir" ]]; then
  echo "Blocked: $path is in the primary checkout. Work in your own worktree as docs/quality/version-control.md describes." >&2
  exit 2
fi
