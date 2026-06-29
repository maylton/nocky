#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESULT_FILE="$(mktemp)"
trap 'rm -f "$RESULT_FILE"' EXIT
chmod 600 "$RESULT_FILE"

cd "$ROOT_DIR"
python3 helpers/nocky_youtube_profiles.py >"$RESULT_FILE"

python3 - "$RESULT_FILE" <<'PY'
from __future__ import annotations

import json
import sys
from pathlib import Path

payload = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
if not payload.get("ok"):
    raise SystemExit(f"Profile discovery failed: {payload.get('error') or 'unknown error'}")

result = payload.get("result")
if not isinstance(result, dict):
    raise SystemExit("Profile discovery returned an invalid result")

allowed_result = {"state", "deterministic", "profiles"}
if set(result) != allowed_result:
    raise SystemExit("Profile discovery returned unexpected top-level fields")

profiles = result.get("profiles")
if not isinstance(profiles, list):
    raise SystemExit("Profile discovery returned an invalid profile list")

allowed_profile = {
    "profile_id",
    "name",
    "channel_handle",
    "photo_url",
    "kind",
    "is_selected",
    "switchable",
}
for profile in profiles:
    if not isinstance(profile, dict) or set(profile) != allowed_profile:
        raise SystemExit("Profile discovery returned unexpected profile fields")

print("YouTube Music profile discovery")
print(f"state: {result.get('state')}")
print(f"deterministic: {str(bool(result.get('deterministic'))).lower()}")
print(f"profiles: {len(profiles)}")

for index, profile in enumerate(profiles, start=1):
    name = str(profile.get("name") or "Unnamed profile").strip()
    handle = str(profile.get("channel_handle") or "").strip()
    label = f"{name} · {handle}" if handle else name
    kind = str(profile.get("kind") or "unknown")
    selected = " · active" if profile.get("is_selected") else ""
    switchable = "yes" if profile.get("switchable") else "no"
    print(f"{index}. {label} · {kind}{selected} · stable selector: {switchable}")
PY
