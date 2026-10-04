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

case "${1:-all}" in
  format) format ;;
  static) static_checks ;;
  test) unit ;;
  all) static_checks; unit ;;
  *) echo 'Usage: bash app/scripts/validate.sh [all|static|test|format]' >&2; exit 2 ;;
esac
