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
Fontsource's Google Fonts mirror, converts them to TTF for Pango/Fontconfig,
validates the resulting fonts and refreshes the local font cache.
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

for command_name in curl install python3 fc-scan fc-match fc-cache woff2_decompress; do
  command -v "$command_name" >/dev/null 2>&1 || {
    echo "Missing required command: $command_name" >&2
    if [[ "$command_name" == "woff2_decompress" ]]; then
      echo "On Arch Linux, install it with: sudo pacman -S --needed woff2" >&2
    fi
    exit 1
  }
done

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

validate_download() {
  local font_file="$1"

  python3 - "$font_file" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
magic = path.read_bytes()[:4]
if magic != b"wOF2":
    raise SystemExit(
        f"Downloaded file is not a WOFF2 font: {path.name} (magic={magic!r})"
    )
PY
}

validate_system_font() {
  local font_file="$1"

  python3 - "$font_file" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
magic = path.read_bytes()[:4]
valid = {b"OTTO", b"\x00\x01\x00\x00"}
if magic not in valid:
    raise SystemExit(
        f"Converted file is not a TTF/OpenType font: {path.name} (magic={magic!r})"
    )
PY

  local detected_family
  detected_family="$(fc-scan --format='%{family}\n' "$font_file" 2>/dev/null || true)"
  if [[ "$detected_family" != *"Google Sans Flex"* ]]; then
    echo "Fontconfig did not recognize Google Sans Flex in $(basename "$font_file")." >&2
    echo "Detected family: ${detected_family:-<none>}" >&2
    exit 1
  fi

  printf '%s' "$detected_family"
}

FONT_DIR="${PREFIX}/share/fonts/nocky"
LICENSE_DIR="${PREFIX}/share/doc/nocky/licenses"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

printf 'Downloading and converting %s fonts...\n' "$FONT_FAMILY"

installed_count=0
matched_family=""
for remote_name in "${FONT_FILES[@]}"; do
  source_url="${FONT_SOURCE_BASE}/${remote_name}"
  woff2_path="${TEMP_DIR}/google-sans-flex-${remote_name}"
  ttf_path="${woff2_path%.woff2}.ttf"

  curl --fail --location --retry 3 --retry-delay 1 \
    --header 'Accept: font/woff2,application/octet-stream;q=0.9,*/*;q=0.1' \
    "$source_url" \
    --output "$woff2_path"

  validate_download "$woff2_path"
  woff2_decompress "$woff2_path"

  [[ -s "$ttf_path" ]] || {
    echo "WOFF2 conversion did not produce $(basename "$ttf_path")." >&2
    exit 1
  }

  detected_family="$(validate_system_font "$ttf_path")"
  if [[ -z "$matched_family" ]]; then
    matched_family="${detected_family%%,*}"
    matched_family="${matched_family%%$'\n'*}"
  fi

  run_install install -D -m 0644 \
    "$ttf_path" \
    "$FONT_DIR/$(basename "$ttf_path")"
  installed_count=$((installed_count + 1))
done

# Remove files installed by the previous broken WOFF2-only implementation.
run_install rm -f "$FONT_DIR"/google-sans-flex-*.woff2

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

fc-cache -f "$FONT_DIR" >/dev/null

match_output="$(fc-match --format='%{file}\n%{family}\n' "$matched_family" 2>/dev/null || true)"
matched_file="${match_output%%$'\n'*}"
matched_name="${match_output#*$'\n'}"
matched_name="${matched_name%%$'\n'*}"

case "$matched_file" in
  "$FONT_DIR"/*) : ;;
  *)
    echo "Installation completed, but Fontconfig still selects another font." >&2
    echo "Requested family: ${matched_family:-$FONT_FAMILY}" >&2
    echo "fc-match file: ${matched_file:-<none>}" >&2
    echo "fc-match family: ${matched_name:-<none>}" >&2
    exit 1
    ;;
esac

printf 'Installed %d Google Sans Flex TTF file(s) under %s\n' \
  "$installed_count" "$FONT_DIR"
printf 'Fontconfig match: %s (%s)\n' "$matched_name" "$matched_file"
