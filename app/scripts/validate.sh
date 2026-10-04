#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."

format() {
  cargo fmt --all
}

static_checks() {
  cargo fmt --all --check
  cargo clippy --locked --all-targets --all-features -- -D warnings
  bash -n scripts/validate.sh
}

unit() {
  RUSTFLAGS="${RUSTFLAGS:-} -D warnings" cargo test --locked --all-targets --all-features
}

# Every line outside terminal setup in main.rs must be covered (docs/quality/testing-strategy.md).
coverage() {
  if [[ ! -f src/lib.rs ]]; then
    echo 'coverage: skipped, no src/lib.rs yet'
    return
  fi
  cargo llvm-cov --locked --all-targets --all-features \
    --ignore-filename-regex 'src/main\.rs$' --fail-under-lines 100
}

# Mutation-tests the code changed since this branch left origin/main, including uncommitted changes.
mutants() {
  local diff
  diff="$(mktemp)"
  trap 'rm -f "$diff"' RETURN
  git diff "$(git merge-base origin/main HEAD)" -- . >"$diff"
  if [[ ! -s "$diff" ]]; then
    echo 'mutants: skipped, no changes since origin/main'
    return
  fi
  cargo mutants --in-diff "$diff"
}

# Checks the prerequisites in docs/quality/agent-workflow.md and prints how to fix each missing one.
doctor() {
  local missing=0 rustup_bin toolchain installed component tool
  need() {
    if ! command -v "$1" >/dev/null 2>&1; then
      echo "doctor: missing $1; $2" >&2
      missing=1
    fi
  }
  need git 'install Xcode Command Line Tools: xcode-select --install'
  need openssl 'brew install openssl'
  need claude 'install Claude Code: https://code.claude.com/docs/en/setup'
  need codex 'install Codex CLI: https://developers.openai.com/codex/cli'
  need rustup 'install rustup: https://rustup.rs'
  if command -v claude >/dev/null 2>&1 && ! claude mcp get todoist >/dev/null 2>&1; then
    echo 'doctor: Claude Code has no todoist MCP server; claude mcp add --transport http --scope user todoist https://ai.todoist.net/mcp' >&2
    missing=1
  fi
  if command -v codex >/dev/null 2>&1 && ! codex mcp get todoist >/dev/null 2>&1; then
    echo 'doctor: Codex has no todoist MCP server; codex mcp add todoist --url https://ai.todoist.net/mcp' >&2
    missing=1
  fi
  if rustup_bin="$(command -v rustup)"; then
    if [[ "$(command -v cargo)" != "$(dirname "$rustup_bin")/cargo" ]]; then
      echo "doctor: cargo resolves to $(command -v cargo), not the rustup proxy; put $(dirname "$rustup_bin") first on PATH" >&2
      missing=1
    fi
    if ! toolchain="$(rustup show active-toolchain 2>/dev/null)"; then
      echo 'doctor: no Rust toolchain installed; rustup default stable' >&2
      missing=1
    elif [[ "$toolchain" =~ ^(nightly|beta) ]]; then
      echo "doctor: active toolchain is $toolchain, not a stable release; rustup default stable" >&2
      missing=1
    else
      installed="$(rustup component list --installed)"
      for component in rustfmt clippy llvm-tools; do
        if ! grep -q "^$component" <<<"$installed"; then
          echo "doctor: missing Rust component $component; rustup component add $component" >&2
          missing=1
        fi
      done
    fi
  fi
  for tool in llvm-cov mutants; do
    if ! cargo "$tool" --version >/dev/null 2>&1; then
      echo "doctor: missing cargo-$tool; cargo install --locked cargo-$tool" >&2
      missing=1
    fi
  done
  if ((missing)); then
    return 1
  fi
  echo 'doctor: all prerequisites found'
}

case "${1:-all}" in
  format) format ;;
  static) static_checks ;;
  test) unit ;;
  coverage) coverage ;;
  mutants) mutants ;;
  doctor) doctor ;;
  all) static_checks; unit; coverage ;;
  *) echo 'Usage: bash app/scripts/validate.sh [all|static|test|coverage|mutants|format|doctor]' >&2; exit 2 ;;
esac
