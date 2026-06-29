#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESULT_FILE="$(mktemp)"
trap 'rm -f "$RESULT_FILE"' EXIT
chmod 600 "$RESULT_FILE"

python_candidates=()
[[ -n "${NOCKY_YOUTUBE_PYTHON:-}" ]] && python_candidates+=("$NOCKY_YOUTUBE_PYTHON")
[[ -n "${NOCKY_RUNTIME_DIR:-}" ]] && python_candidates+=("$NOCKY_RUNTIME_DIR/bin/python3")
python_candidates+=(
    "$ROOT_DIR/.nocky-runtime/bin/python3"
    "$HOME/.local/share/nocky/runtime/bin/python3"
    "/usr/local/share/nocky/runtime/bin/python3"
    "/usr/share/nocky/runtime/bin/python3"
)
if command -v python3 >/dev/null 2>&1; then
    python_candidates+=("$(command -v python3)")
fi

YOUTUBE_PYTHON=""
for candidate in "${python_candidates[@]}"; do
    [[ -x "$candidate" ]] || continue
    if "$candidate" -c 'import requests, ytmusicapi' >/dev/null 2>&1; then
        YOUTUBE_PYTHON="$candidate"
        break
    fi
done

if [[ -z "$YOUTUBE_PYTHON" ]]; then
    cat >&2 <<EOF
No Nocky YouTube runtime with ytmusicapi was found.
Create the project-local runtime with:
  ./scripts/setup-youtube-runtime.sh
Then run this smoke test again.
EOF
    exit 2
fi

cd "$ROOT_DIR"
"$YOUTUBE_PYTHON" helpers/nocky_youtube_profiles.py >"$RESULT_FILE" || true

"$YOUTUBE_PYTHON" - "$RESULT_FILE" <<'PY'
from __future__ import annotations

import json
import sys
from pathlib import Path

try:
    payload = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
except Exception as error:
    raise SystemExit(f"Profile discovery returned invalid diagnostic output: {error}")

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
