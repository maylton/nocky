#!/usr/bin/env bash
set -Eeuo pipefail

APP_ID="io.github.maylton.Nocky"
BIN_NAME="nocky"
MODE="user"
PREFIX=""

usage() {
  echo "Usage: ./uninstall.sh [--user|--system|--prefix PATH]"
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

run_root() {
  if [[ ${EUID} -eq 0 ]]; then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    echo "This operation requires root privileges, but sudo is unavailable." >&2
    exit 1
  fi
}

remove_file() {
  local path="$1"
  if [[ "$MODE" == "system" ]]; then
    run_root rm -f "$path"
  else
    rm -f "$path"
  fi
}

remove_file "${PREFIX}/bin/${BIN_NAME}"
remove_file "${PREFIX}/share/applications/${APP_ID}.desktop"
remove_file "${PREFIX}/share/metainfo/${APP_ID}.metainfo.xml"

for size in 32x32 48x48 64x64 128x128 256x256 512x512; do
  remove_file "${PREFIX}/share/icons/hicolor/${size}/apps/${APP_ID}.png"
done

if [[ "$MODE" == "system" ]]; then
  run_root rm -rf "${PREFIX}/share/nocky"
else
  rm -rf "${PREFIX}/share/nocky"
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${PREFIX}/share/applications" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "${PREFIX}/share/icons/hicolor" 2>/dev/null || true
fi

echo "Nocky was removed from ${PREFIX}."
echo "User settings, YouTube session data and cached artwork were preserved."
