#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

patterns=(
  'Show or hide' 'Search the library' 'Choose Music Folder' 'Rescan Library'
  'Download Lyrics' 'Toggle Automatic' 'No track selected' 'Nothing playing'
  'Previous track' 'Next track' 'Play or pause' 'Repeat current track'
  'No synchronized lyrics' 'Searching synchronized lyrics' 'Select a track first'
  'Connect account' 'Import browser session' 'Checking account'
  'Search songs' 'Library synchronized' 'Your local collection'
)

found=0
for pattern in "${patterns[@]}"; do
  if grep -RFn --include='*.rs' "$pattern" src/; then
    found=1
  fi
done

if [[ $found -eq 0 ]]; then
  echo 'OK: nenhuma das strings inglesas conhecidas foi encontrada na interface.'
else
  echo 'Aviso: ainda existem strings inglesas conhecidas listadas acima.' >&2
  exit 1
fi
