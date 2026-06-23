#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_ID="io.github.maylton.Nocky"
EXPECTED_VERSION="0.2.5"
expected_sizes=(32 48 64 128 256 512)

fail() { echo "ERROR: $*" >&2; exit 1; }
pass() { echo "OK: $*"; }

bash -n install.sh || fail "install.sh syntax"
bash -n uninstall.sh || fail "uninstall.sh syntax"
bash -n scripts/check-mpris.sh || fail "check-mpris.sh syntax"
bash -n scripts/check-playback.sh || fail "check-playback.sh syntax"
bash -n scripts/check-youtube.sh || fail "check-youtube.sh syntax"
bash -n scripts/setup-youtube-runtime.sh || fail "setup-youtube-runtime.sh syntax"
python3 -m py_compile helpers/nocky_youtube.py || fail "YouTube helper syntax"
response="$(printf '{}\n' | python3 helpers/nocky_youtube.py status)"
grep -q '"ok": true' <<<"$response" || fail "YouTube helper status"
rm -rf helpers/__pycache__
pass "shell and Python helpers"

grep -q "version = \"${EXPECTED_VERSION}\"" Cargo.toml || fail "Cargo version is not ${EXPECTED_VERSION}"
grep -q "<release version=\"${EXPECTED_VERSION}\"" "data/${APP_ID}.metainfo.xml" || fail "AppStream version is not ${EXPECTED_VERSION}"
pass "release version"

grep -q '^Icon=io.github.maylton.Nocky$' "data/${APP_ID}.desktop" || fail "desktop icon ID"
grep -q '^Exec=nocky$' "data/${APP_ID}.desktop" || fail "desktop executable"
grep -q 'application_id(APP_ID)' src/main.rs || fail "GTK application ID"
grep -q 'const APP_ID: &str = "io.github.maylton.Nocky"' src/main.rs || fail "application ID constant"
grep -q 'mod youtube;' src/main.rs || fail "YouTube Rust module"
grep -q 'startup_source' src/config.rs || fail "startup source configuration"
grep -q 'show_startup_source_dialog' src/main.rs || fail "startup source dialog"
grep -q 'load_with_headers' src/playback.rs || fail "YouTube playback headers"
grep -q 'preload_streams' src/youtube.rs || fail "YouTube queue prefetch"
grep -q 'library-cache.json' src/youtube.rs || fail "YouTube library cache"
grep -q 'STREAM_CACHE_LIMIT = 80' helpers/nocky_youtube.py || fail "bounded YouTube stream cache"
grep -q 'PLAYER_COVER_SIZE: u32 = 1200' src/youtube.rs || fail "HD YouTube artwork"
[[ -f requirements-youtube.txt ]] || fail "YouTube requirements file"
! find . -type f \( -name '.env' -o -name 'youtube-session.json' -o -name 'stream-cache.json' -o -name '*.pyc' \) | grep -q . || fail "sensitive/generated files found"
pass "desktop/application identity and YouTube integration"

for size in "${expected_sizes[@]}"; do
  icon="data/icons/hicolor/${size}x${size}/apps/${APP_ID}.png"
  [[ -f "$icon" ]] || fail "missing ${size}x${size} icon"
  if command -v identify >/dev/null 2>&1; then
    dimensions="$(identify -format '%wx%h' "$icon")"
    [[ "$dimensions" == "${size}x${size}" ]] || fail "$icon has dimensions $dimensions"
  fi
done
pass "hicolor icon set"

python3 - <<'PY'
from pathlib import Path
import configparser
import xml.etree.ElementTree as ET

app_id = 'io.github.maylton.Nocky'
ET.parse(Path('data') / f'{app_id}.metainfo.xml')
parser = configparser.ConfigParser(interpolation=None, strict=True)
parser.optionxform = str
parser.read(Path('data') / f'{app_id}.desktop')
assert 'Desktop Entry' in parser
assert parser['Desktop Entry']['Type'] == 'Application'
assert parser['Desktop Entry']['Icon'] == app_id
PY
pass "desktop and XML parsing"

if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "data/${APP_ID}.desktop"
  pass "desktop-file-validate"
fi

if command -v appstreamcli >/dev/null 2>&1; then
  appstreamcli validate --no-net "data/${APP_ID}.metainfo.xml"
  pass "AppStream validation"
fi

echo "Release metadata verification completed successfully."
