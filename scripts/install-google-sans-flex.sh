#!/usr/bin/env bash
set -Eeuo pipefail

FONT_FAMILY="Google Sans Flex"
FONT_DOWNLOAD_URL="https://fonts.google.com/download?family=Google%20Sans%20Flex"
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

The script downloads the official Google Fonts family archive, installs its TTF
files and refreshes the fontconfig cache when fc-cache is available.
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

for command_name in curl unzip find install; do
  command -v "$command_name" >/dev/null 2>&1 || {
    echo "Missing required command: $command_name" >&2
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

FONT_DIR="${PREFIX}/share/fonts/truetype/nocky"
LICENSE_DIR="${PREFIX}/share/doc/nocky/licenses"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

ARCHIVE="${TEMP_DIR}/google-sans-flex.zip"
EXTRACT_DIR="${TEMP_DIR}/google-sans-flex"
mkdir -p "$EXTRACT_DIR"

echo "Downloading ${FONT_FAMILY} from Google Fonts..."
curl --fail --location --retry 3 --retry-delay 1 \
  "$FONT_DOWNLOAD_URL" \
  --output "$ARCHIVE"

unzip -q "$ARCHIVE" -d "$EXTRACT_DIR"

mapfile -d '' FONT_FILES < <(
  find "$EXTRACT_DIR" -type f \( -iname '*.ttf' -o -iname '*.otf' \) -print0 | sort -z
)

if ((${#FONT_FILES[@]} == 0)); then
  echo "The downloaded archive did not contain installable font files." >&2
  exit 1
fi

for font_file in "${FONT_FILES[@]}"; do
  run_install install -D -m 0644 "$font_file" "$FONT_DIR/$(basename "$font_file")"
done

license_file="$(find "$EXTRACT_DIR" -type f -iname 'OFL.txt' -print -quit)"
if [[ -n "$license_file" ]]; then
  run_install install -D -m 0644 "$license_file" \
    "$LICENSE_DIR/google-sans-flex-OFL.txt"
fi

if command -v fc-cache >/dev/null 2>&1; then
  fc-cache -f "$FONT_DIR" >/dev/null 2>&1 || true
fi

printf 'Installed %d %s font file(s) under %s\n' \
  "${#FONT_FILES[@]}" "$FONT_FAMILY" "$FONT_DIR"
