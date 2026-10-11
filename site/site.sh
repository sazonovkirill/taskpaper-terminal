#!/usr/bin/env bash
# Builds, previews, checks and screenshots the tpt product website (docs/website.md).
#
# Usage: bash site/site.sh build | serve | check | shoot [scene,...]
#
#   build  render the site into site/public/
#   serve  preview it at http://127.0.0.1:1111 with live reload
#   check  build, then fail on a broken internal link, missing file or missing anchor
#   shoot  build the release tpt and regenerate site/static/screenshots/ (needs tmux, Node
#          Playwright and Chromium); name scenes to regenerate only those
#
# Zola is downloaded once, at a pinned version and checksum, into site/.cache/.
set -euo pipefail

site="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cache="$site/.cache"
zola_version=0.22.1

zola() {
  local target sum
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) target=x86_64-unknown-linux-gnu
      sum=0ca09aa40376aaa9ddfb512ff9ad963262ef95edb0d0f2d5ec6961b6f5cf22ef ;;
    Linux-aarch64) target=aarch64-unknown-linux-gnu
      sum=8af437ec6352f33ccd24d7a1cfcb54a3db95d3ce376dc69525b4ef3fb6b8c1d1 ;;
    Darwin-x86_64) target=x86_64-apple-darwin
      sum=3898709e154ae0593933264a540c869348bdb10d7f1b03a42dfb78d63703b3b5 ;;
    Darwin-arm64) target=aarch64-apple-darwin
      sum=46ac45a9e7628dba8593b124ee8794f4f9aa1c6b569918ecd4bbc5d0be190515 ;;
    *) echo "site.sh: no Zola build for $(uname -s) $(uname -m)" >&2; exit 1 ;;
  esac
  local bin="$cache/zola-$zola_version/zola"
  if [[ ! -x "$bin" ]]; then
    local archive="$cache/zola-$zola_version.tar.gz"
    mkdir -p "$(dirname "$bin")"
    curl -fsSL -o "$archive" \
      "https://github.com/getzola/zola/releases/download/v$zola_version/zola-v$zola_version-$target.tar.gz"
    echo "$sum  $archive" | shasum -a 256 -c - >/dev/null
    tar -xzf "$archive" -C "$(dirname "$bin")"
    rm "$archive"
  fi
  "$bin" --root "$site" "$@"
}

case "${1:-}" in
  build) zola build ;;
  serve) zola serve ;;
  check)
    zola check --skip-external-links
    zola build
    python3 "$site/scripts/check_links.py" "$site/public"
    ;;
  shoot)
    cargo build --release --manifest-path "$site/../app/Cargo.toml"
    python3 "$site/screenshots/shoot.py" "$site/../app/target/release/tpt" \
      "$site/static/screenshots" ${2:+--only "$2"}
    ;;
  *) sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2; exit 2 ;;
esac
