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

case "${1:-all}" in
  format) format ;;
  static) static_checks ;;
  test) unit ;;
  coverage) coverage ;;
  mutants) mutants ;;
  all) static_checks; unit; coverage ;;
  *) echo 'Usage: bash app/scripts/validate.sh [all|static|test|coverage|mutants|format]' >&2; exit 2 ;;
esac
