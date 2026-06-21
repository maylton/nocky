#!/usr/bin/env bash
set -euo pipefail

if ! command -v playerctl >/dev/null 2>&1; then
  echo "playerctl is not installed. On Arch/CachyOS: sudo pacman -S playerctl" >&2
  exit 1
fi

PLAYER="$(playerctl --list-all 2>/dev/null | grep -i 'nocky' | head -n1 || true)"
if [[ -z "$PLAYER" ]]; then
  echo "Nocky was not found on the MPRIS session bus." >&2
  echo "Open the application and run this script again." >&2
  exit 1
fi

echo "Player: $PLAYER"
echo
playerctl --player="$PLAYER" metadata --format $'Status: {{status}}\nTitle: {{title}}\nArtist: {{artist}}\nAlbum: {{album}}\nArt: {{mpris:artUrl}}\nLength: {{mpris:length}}'
