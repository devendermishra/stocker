#!/usr/bin/env bash
# Build the native desktop app in direct mode (in-process stocker-core).
# Usage:
#   ./build-standalone.sh           # cargo release binary -> target/release/stocker-web
#   ./build-standalone.sh --bundle  # dx bundle (installer-style; requires dioxus-cli)

set -euo pipefail

repo_root="$(cd "$(dirname "$0")" && pwd)"
cd "$repo_root"

if [[ "${1:-}" == "--bundle" ]]; then
  if ! command -v dx >/dev/null 2>&1; then
    echo "Missing 'dx'. Install: cargo install dioxus-cli --locked" >&2
    exit 1
  fi
  echo "Bundling standalone desktop app (direct mode, release)..."
  (cd frontend && dx bundle --platform desktop --release --no-default-features)
  echo ""
  echo "Done. See dx output above for bundle output directory (or use --out-dir)."
else
  echo "Building standalone desktop binary (direct mode, release)..."
  cargo build -p stocker-web --release --no-default-features --features desktop
  echo ""
  echo "Executable: ${repo_root}/target/release/stocker-web"
fi
