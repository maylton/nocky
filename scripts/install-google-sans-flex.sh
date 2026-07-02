#!/usr/bin/env bash
set -Eeuo pipefail

FONT_FAMILY="Google Sans Flex Variable"
FONT_SOURCE_BASE="https://cdn.jsdelivr.net/fontsource/fonts/google-sans-flex:vf@latest"
FONT_FILES=(
  "latin-wght-normal.woff2"
  "latin-ext-wght-normal.woff2"
)
LICENSE_URL="https://cdn.jsdelivr.net/npm/@fontsource-variable/google-sans-flex@latest/LICENSE"
MODE="user"
PREFIX=""

usage() {
  cat <<'EOF'
Install Google Sans Flex for Nocky's Material Expressive theme.

Usage: ./scripts/install-google-sans-flex.sh [OPTIONS]

Options:
  --user          Install for the current user under ~/.local (default)
  --system        Install under /usr/local (may require sudo)
  --prefix PATH   Install under a custom prefix
  -h, --help      Show this help

The installer downloads the Latin and Latin Extended variable WOFF2 files from
Fontsource's Google Fonts mirror, validates them with Fontconfig and refreshes
the local font cache.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --user) MODE="user" ;;
    --system) MODE="system" ;;
    --prefix)
      shift
      [[ $# -gt 0 ]] || { echo "--prefix requires a path" >&2; exit 2; }
      MODE="custom"
      PREFIX="$1"
      ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

case "$MODE" in
  user) PREFIX="${HOME}/.local" ;;
  system) PREFIX="/usr/local" ;;
  custom) : ;;
esac

for command_name in curl install python3; do
  command -v "$command_name" >/dev/null 2>&1 || {
    echo "Missing required command: $command_name" >&2
    exit 1
  }
done

command -v fc-scan >/dev/null 2>&1 || {
  echo "Missing required command: fc-scan (provided by fontconfig)" >&2
  exit 1
}

run_install() {
  if [[ "$MODE" == "system" && ${EUID} -ne 0 ]]; then
    command -v sudo >/dev/null 2>&1 || {
      echo "System installation requires root privileges, but sudo is unavailable." >&2
      exit 1
    }
    sudo "$@"
  else
    "$@"
  fi
}

validate_font_file() {
  local font_file="$1"

  python3 - "$font_file" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
magic = path.read_bytes()[:4]
valid = {b"wOF2", b"wOFF", b"OTTO", b"\x00\x01\x00\x00"}
if magic not in valid:
    raise SystemExit(
        f"Downloaded file is not a supported font: {path.name} (magic={magic!r})"
    )
PY

  local detected_family
  detected_family="$(fc-scan --format='%{family}\n' "$font_file" 2>/dev/null || true)"
  if [[ "$detected_family" != *"Google Sans Flex"* ]]; then
    echo "Fontconfig did not recognize Google Sans Flex in $(basename "$font_file")." >&2
    echo "Detected family: ${detected_family:-<none>}" >&2
    exit 1
  fi
}

FONT_DIR="${PREFIX}/share/fonts/nocky"
LICENSE_DIR="${PREFIX}/share/doc/nocky/licenses"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

printf 'Downloading %s variable fonts...\n' "$FONT_FAMILY"

installed_count=0
for remote_name in "${FONT_FILES[@]}"; do
  source_url="${FONT_SOURCE_BASE}/${remote_name}"
  local_path="${TEMP_DIR}/google-sans-flex-${remote_name}"

  curl --fail --location --retry 3 --retry-delay 1 \
    --header 'Accept: font/woff2,application/octet-stream;q=0.9,*/*;q=0.1' \
    "$source_url" \
    --output "$local_path"

  validate_font_file "$local_path"
  run_install install -D -m 0644 \
    "$local_path" \
    "$FONT_DIR/$(basename "$local_path")"
  installed_count=$((installed_count + 1))
done

license_file="${TEMP_DIR}/google-sans-flex-OFL.txt"
if curl --fail --location --retry 2 --retry-delay 1 \
  "$LICENSE_URL" \
  --output "$license_file"; then
  run_install install -D -m 0644 \
    "$license_file" \
    "$LICENSE_DIR/google-sans-flex-OFL.txt"
else
  echo "Warning: the optional OFL license copy could not be downloaded." >&2
fi

if command -v fc-cache >/dev/null 2>&1; then
  fc-cache -f "$FONT_DIR" >/dev/null
fi

matched_family="$(fc-match --format='%{family}\n' "$FONT_FAMILY" 2>/dev/null || true)"
if [[ "$matched_family" != *"Google Sans Flex"* ]]; then
  echo "Installation completed, but Fontconfig still does not select Google Sans Flex." >&2
  echo "fc-match returned: ${matched_family:-<none>}" >&2
  exit 1
fi

printf 'Installed %d %s font file(s) under %s\n' \
  "$installed_count" "$FONT_FAMILY" "$FONT_DIR"
printf 'Fontconfig match: %s\n' "$matched_family"
