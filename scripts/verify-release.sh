#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_ID="io.github.maylton.Nocky"
EXPECTED_VERSION="$(awk -F'"' '/^version = / { print $2; exit }' Cargo.toml)"
expected_sizes=(32 48 64 128 256 512)

fail() { echo "ERROR: $*" >&2; exit 1; }
pass() { echo "OK: $*"; }

[[ "$EXPECTED_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] \
  || fail "invalid Cargo version: ${EXPECTED_VERSION}"

for script in \
  install.sh \
  uninstall.sh \
  scripts/check-mpris.sh \
  scripts/check-playback.sh \
  scripts/check-youtube.sh \
  scripts/setup-youtube-runtime.sh \
  scripts/quality-gate.sh \
  scripts/verify-release.sh; do
  [[ -f "$script" ]] || fail "missing $script"
  bash -n "$script" || fail "$script syntax"
done
pass "shell syntax"

python3 -m py_compile helpers/nocky_youtube.py || fail "YouTube helper syntax"
rm -rf helpers/__pycache__
pass "Python helper syntax"

grep -q "version = \"${EXPECTED_VERSION}\"" Cargo.toml \
  || fail "Cargo version mismatch"
python3 - "$EXPECTED_VERSION" <<'LOCKPY'
from pathlib import Path
import re
import sys

version = sys.argv[1]
lock = Path("Cargo.lock").read_text(encoding="utf-8")
match = re.search(
    r'\[\[package\]\]\nname = "nocky"\nversion = "([^"]+)"',
    lock,
)
assert match, "nocky package missing from Cargo.lock"
assert match.group(1) == version, (match.group(1), version)
LOCKPY
grep -q "<release version=\"${EXPECTED_VERSION}\"" \
  "data/${APP_ID}.metainfo.xml" \
  || fail "AppStream version mismatch"
grep -q "Nocky ${EXPECTED_VERSION} is a beta release" README.md \
  || fail "README status version mismatch"
grep -q 'assets/branding/nocky-icon-1024.png' README.md \
  || fail "README does not use the current branding icon"
pass "release version and GitHub branding"

[[ -f CHANGELOG.md ]] || fail "missing CHANGELOG.md"
grep -q "## \[${EXPECTED_VERSION}\]" CHANGELOG.md \
  || fail "missing changelog entry"
[[ -f docs/FROSTED_GLASS.md ]] || fail "missing Frosted Glass documentation"
grep -q "Nocky ${EXPECTED_VERSION}" docs/FROSTED_GLASS.md \
  || fail "Frosted Glass docs version mismatch"
[[ -f assets/themes/frosted-glass.css ]] || fail "missing Frosted Glass CSS"
grep -q 'FrostedGlass' src/config.rs || fail "missing FrostedGlass config variant"
grep -q 'frosted_glass_css' src/theme_css.rs || fail "missing Frosted Glass CSS provider"
grep -q 'clear_playback_queue' src/app/controller/queue.rs \
  || fail "missing atomic Clear all operation"
grep -q 'controller.clear_playback_queue()' src/app/controller/construction.rs \
  || fail "Clear all button is not wired to the atomic operation"
pass "Frosted Glass and queue contracts"

grep -q -- '--verify' install.sh || fail "installer --verify option"
grep -q 'DOC_DIR="${DATA_DIR}/doc/nocky"' install.sh \
  || fail "installer documentation directory"
version_output="$(./install.sh --version)"
grep -q "${EXPECTED_VERSION}" <<<"$version_output" \
  || fail "installer version output"
pass "installer contract"

grep -q '^Icon=io.github.maylton.Nocky$' "data/${APP_ID}.desktop" \
  || fail "desktop icon ID"
grep -q '^Exec=nocky$' "data/${APP_ID}.desktop" \
  || fail "desktop executable"
[[ -f assets/branding/nocky-icon-1024.png ]] \
  || fail "missing current 1024 px branding icon"

for size in "${expected_sizes[@]}"; do
  icon="data/icons/hicolor/${size}x${size}/apps/${APP_ID}.png"
  [[ -f "$icon" ]] || fail "missing ${size}x${size} icon"
  if command -v identify >/dev/null 2>&1; then
    dimensions="$(identify -format '%wx%h' "$icon")"
    [[ "$dimensions" == "${size}x${size}" ]] \
      || fail "$icon has dimensions $dimensions"
  fi
done
pass "desktop identity and icon set"

python3 - <<'METAPY'
from pathlib import Path
import configparser
import xml.etree.ElementTree as ET

app_id = "io.github.maylton.Nocky"
ET.parse(Path("data") / f"{app_id}.metainfo.xml")
parser = configparser.ConfigParser(interpolation=None, strict=True)
parser.optionxform = str
parser.read(Path("data") / f"{app_id}.desktop")
assert parser["Desktop Entry"]["Type"] == "Application"
assert parser["Desktop Entry"]["Icon"] == app_id
METAPY
pass "desktop and AppStream parsing"

cargo metadata --no-deps --locked --format-version 1 >/dev/null \
  || fail "Cargo metadata / lockfile synchronization"
pass "Cargo metadata"

if git ls-files | grep -Eq \
  '(^|/)(\.env|youtube-session\.json|stream-cache\.json|[^/]+\.pyc)$'; then
  fail "tracked sensitive/generated files found"
fi
pass "repository hygiene"

if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "data/${APP_ID}.desktop"
  pass "desktop-file-validate"
fi

if command -v appstreamcli >/dev/null 2>&1; then
  appstreamcli validate --no-net "data/${APP_ID}.metainfo.xml"
  pass "AppStream validation"
fi

echo "Release ${EXPECTED_VERSION} metadata verification completed successfully."
