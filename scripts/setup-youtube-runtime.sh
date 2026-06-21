#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNTIME_DIR="${NOCKY_RUNTIME_DIR:-${ROOT_DIR}/.nocky-runtime}"
DENO_VERSION="2.8.3"

usage() {
  cat <<USAGE
Create a project-local YouTube Music runtime for Nocky development.

Usage: ./scripts/setup-youtube-runtime.sh [--runtime PATH]

The default runtime is:
  ${ROOT_DIR}/.nocky-runtime

Nocky automatically discovers this runtime when launched with cargo run.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runtime)
      shift
      [[ $# -gt 0 ]] || { echo "--runtime requires a path" >&2; exit 2; }
      RUNTIME_DIR="$1"
      ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

for command_name in python3 curl unzip; do
  command -v "$command_name" >/dev/null 2>&1 || {
    echo "Missing required command: $command_name" >&2
    exit 1
  }
done

if ! python3 -m venv --help >/dev/null 2>&1; then
  echo "Python venv support is unavailable. Install python3-venv or your distribution's equivalent." >&2
  exit 1
fi

echo "Creating Nocky YouTube runtime at: $RUNTIME_DIR"
rm -rf "$RUNTIME_DIR"
python3 -m venv --system-site-packages "$RUNTIME_DIR"
"$RUNTIME_DIR/bin/python3" -m pip install --upgrade pip
"$RUNTIME_DIR/bin/python3" -m pip install -r "$ROOT_DIR/requirements-youtube.txt"

case "$(uname -m)" in
  x86_64|amd64) deno_arch="x86_64-unknown-linux-gnu" ;;
  aarch64|arm64) deno_arch="aarch64-unknown-linux-gnu" ;;
  *)
    echo "Deno could not be installed automatically for architecture $(uname -m)." >&2
    echo "Install Deno manually and ensure it is available in PATH." >&2
    deno_arch=""
    ;;
esac

if [[ -n "$deno_arch" ]] && ! command -v deno >/dev/null 2>&1; then
  temp_dir="$(mktemp -d)"
  trap 'rm -rf "$temp_dir"' EXIT
  curl -fL \
    "https://github.com/denoland/deno/releases/download/v${DENO_VERSION}/deno-${deno_arch}.zip" \
    -o "$temp_dir/deno.zip"
  unzip -q "$temp_dir/deno.zip" -d "$temp_dir"
  install -m 0755 "$temp_dir/deno" "$RUNTIME_DIR/bin/deno"
fi

"$RUNTIME_DIR/bin/python3" - <<'PY'
import requests
import ytmusicapi
import yt_dlp
print("Python modules: requests, ytmusicapi and yt-dlp available")
PY

echo
printf 'Runtime ready: %s\n' "$RUNTIME_DIR"
printf 'Start Nocky with: cargo run\n'
