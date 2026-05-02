#!/usr/bin/env bash
# Build the native desktop app in direct mode (in-process stocker-core).
#
# Usage:
#   ./build-standalone.sh              # cargo release binary
#   ./build-standalone.sh --bundle     # dx bundle (installer-style; needs dioxus-cli)
#   ./build-standalone.sh --dry-run    # print commands only
#   ./build-standalone.sh --help

set -euo pipefail

repo_root="$(cd "$(dirname "$0")" && pwd)"
cd "$repo_root"

usage() {
  sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
}

dry_run=0
bundle=0
for arg in "$@"; do
  case "$arg" in
    --help|-h) usage ;;
    --dry-run) dry_run=1 ;;
    --bundle) bundle=1 ;;
    *)
      echo "Unknown option: $arg" >&2
      echo "Try: $0 --help" >&2
      exit 1
      ;;
  esac
done

release_base="${repo_root}/target/release/stocker-web"

if [[ "$bundle" -eq 1 ]]; then
  if [[ "$dry_run" -eq 1 ]]; then
    echo "[dry-run] (cd frontend && dx bundle --platform desktop --release --no-default-features)"
    exit 0
  fi
  if ! command -v dx >/dev/null 2>&1; then
    echo "Missing 'dx'. Install: cargo install dioxus-cli --locked" >&2
    exit 1
  fi
  echo "Bundling standalone desktop app (direct mode, release)..."
  (cd frontend && dx bundle --platform desktop --release --no-default-features)
  echo ""
  echo "Done. See dx output above for the bundle directory (optional: pass --out-dir to dx)."
else
  if [[ "$dry_run" -eq 1 ]]; then
    echo "[dry-run] cargo build -p stocker-web --release --no-default-features --features desktop"
    exit 0
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Missing 'cargo'. Install Rust: https://rustup.rs/" >&2
    exit 1
  fi
  echo "Building standalone desktop binary (direct mode, release)..."
  cargo build -p stocker-web --release --no-default-features --features desktop
  echo ""
  if [[ -f "${release_base}.exe" ]]; then
    echo "Executable: ${release_base}.exe"
  else
    echo "Executable: ${release_base}"
  fi
fi
