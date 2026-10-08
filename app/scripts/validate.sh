#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."

format() {
  cargo fmt --all
}

static_checks() {
  cargo fmt --all --check
  cargo clippy --locked --all-targets --all-features -- -D warnings
  cargo deny --locked check
  bash -n scripts/validate.sh
  bash -n scripts/review.sh
  layers
  pending_snapshots
}

# Document and App are the UI-independent core (docs/quality/code-quality.md, Architecture).
layers() {
  local found=0
  if [[ -d src/document ]] && grep -rnwE 'ratatui|crossterm|app|ui' src/document | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//'; then
    echo 'layers: src/document may depend only on std and small pure crates, never on ratatui, crossterm, app or ui' >&2
    found=1
  fi
  if [[ -d src/app ]] && grep -rnwE 'ratatui|crossterm|ui' src/app | grep -vE '^[^:]+:[0-9]+:[[:space:]]*//'; then
    echo 'layers: src/app may depend only on document; turn key events into app messages and render in src/ui' >&2
    found=1
  fi
  return "$found"
}

# Pending insta snapshots are never committed (docs/quality/testing-strategy.md, Snapshots).
pending_snapshots() {
  local pending
  pending="$(find . -path ./target -prune -o \( -name '*.snap.new' -o -name '*.pending-snap' \) -print)"
  if [[ -n "$pending" ]]; then
    echo "$pending" >&2
    echo 'snapshots: review these with cargo insta review before validating' >&2
    return 1
  fi
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

# Mutation-tests the code changed since this branch left origin/main, including uncommitted and untracked files.
# Paths are made relative to app/, because cargo-mutants matches diff paths against the crate root.
mutants() {
  local diff file
  diff="$(mktemp)"
  trap 'rm -f "$diff"' RETURN
  git diff --relative "$(git merge-base origin/main HEAD)" -- . >"$diff"
  while IFS= read -r file; do
    git diff --no-index -- /dev/null "$file" >>"$diff" || [[ $? -eq 1 ]]
  done < <(git ls-files --others --exclude-standard -- .)
  if [[ ! -s "$diff" ]]; then
    echo 'mutants: skipped, no changes since origin/main'
    return
  fi
  # Builds of parallel jobs share one jobserver capped at the CPU count, so half the CPUs keeps every core busy.
  CARGO_MUTANTS_JOBS="${CARGO_MUTANTS_JOBS:-$(( ($(getconf _NPROCESSORS_ONLN) + 1) / 2 ))}" cargo mutants --in-diff "$diff"
}

# Times opening a 100k-line outline and keystrokes on it against N-PERF-01 and N-PERF-02.
bench() {
  cargo bench --locked --bench perf
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
  # Cloud sessions run without Codex, which reviews milestones on the owner's Mac, and get Todoist as a connector.
  local cloud=0
  [[ "${CLAUDE_CODE_REMOTE:-}" == true ]] && cloud=1
  ((cloud)) || need codex 'install Codex CLI: https://developers.openai.com/codex/cli'
  need rustup 'install rustup: https://rustup.rs'
  need jq 'brew install jq'
  if [[ "$(git config core.hooksPath || true)" != .githooks ]]; then
    echo 'doctor: the pre-push validation hook is not installed; git config core.hooksPath .githooks' >&2
    missing=1
  fi
  if ((!cloud)) && command -v claude >/dev/null 2>&1 && ! claude mcp get todoist >/dev/null 2>&1; then
    echo 'doctor: Claude Code has no todoist MCP server; claude mcp add --transport http --scope user todoist https://ai.todoist.net/mcp' >&2
    missing=1
  fi
  if ((!cloud)) && command -v codex >/dev/null 2>&1 && ! codex mcp get todoist >/dev/null 2>&1; then
    echo 'doctor: Codex has no todoist MCP server; codex mcp add todoist --url https://ai.todoist.net/mcp' >&2
    missing=1
  fi
  if rustup_bin="$(command -v rustup)"; then
    if [[ "$(command -v cargo)" != "$(dirname "$rustup_bin")/cargo" ]]; then
      echo "doctor: cargo resolves to $(command -v cargo), not the rustup proxy; put $(dirname "$rustup_bin") first on PATH" >&2
      missing=1
    fi
    if ! toolchain="$(rustup show active-toolchain 2>/dev/null)"; then
      echo 'doctor: the pinned Rust toolchain is not installed; rustup toolchain install' >&2
      missing=1
    elif [[ "$toolchain" != "$(sed -n 's/^channel = "\(.*\)"$/\1/p' ../rust-toolchain.toml)"-* ]]; then
      echo "doctor: active toolchain is $toolchain, not the release pinned in rust-toolchain.toml; unset RUSTUP_TOOLCHAIN and run rustup override unset" >&2
      missing=1
    elif ! installed="$(rustup component list --installed 2>/dev/null)"; then
      echo 'doctor: the pinned Rust toolchain is not installed; rustup toolchain install' >&2
      missing=1
    else
      for component in rustfmt clippy llvm-tools rust-src; do
        if ! grep -q "^$component" <<<"$installed"; then
          echo "doctor: missing Rust component $component; rustup component add $component" >&2
          missing=1
        fi
      done
    fi
  fi
  for tool in llvm-cov mutants deny; do
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

# Runs the real-user scenarios against the real binary, those whose path contains a filter when any is given.
scenarios() {
  cargo test --locked --test scenarios -- "$@"
}

# Runs the key-sequence fuzzer far longer than `all` does, on a fresh seed unless PROPTEST_RNG_SEED names one.
fuzz() {
  local seed="${PROPTEST_RNG_SEED:-$(od -An -N4 -tu4 /dev/urandom | tr -d ' ')}"
  echo "fuzz: seed $seed; rerun with PROPTEST_RNG_SEED=$seed"
  PROPTEST_RNG_SEED="$seed" PROPTEST_CASES="${PROPTEST_CASES:-1000}" cargo test --locked --release --test ui fuzz::
}

# Lists Must requirements that no test or source file names yet (docs/quality/testing-strategy.md, Traceability).
trace() {
  local id untraced=0 total=0
  while IFS= read -r id; do
    total=$((total + 1))
    if ! grep -rqw -- "$id" src tests 2>/dev/null; then
      echo "$id"
      untraced=$((untraced + 1))
    fi
  done < <(grep -hoE '^\| (FR|F|N)-[A-Z]+-[0-9]+ \|.*\| Must \||^\*\*FR-[A-Z]+-[0-9]+ \(Must\)' ../docs/functional-requirements.md ../docs/requirements.md |
    grep -oE '(FR|F|N)-[A-Z]+-[0-9]+' | awk '!seen[$0]++')
  echo "trace: $untraced of $total Must requirements are not referenced by any test or source file"
}

case "${1:-all}" in
  format) format ;;
  static) static_checks ;;
  test) unit ;;
  coverage) coverage ;;
  mutants) mutants ;;
  doctor) doctor ;;
  trace) trace ;;
  bench) bench ;;
  scenarios) scenarios "${@:2}" ;;
  fuzz) fuzz ;;
  # The coverage run is the test run: llvm-cov runs every test, and clippy above already denies warnings.
  all) static_checks; coverage ;;
  *) echo 'Usage: bash app/scripts/validate.sh [all|static|test|coverage|mutants|format|doctor|trace|bench|scenarios [filter...]|fuzz]' >&2; exit 2 ;;
esac
