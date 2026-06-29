#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESULT_FILE="$(mktemp)"
PAYLOAD_FILE="$(mktemp)"
trap 'rm -f "$RESULT_FILE" "$PAYLOAD_FILE"' EXIT
chmod 600 "$RESULT_FILE" "$PAYLOAD_FILE"

if [[ "${NOCKY_CONFIRM_PLAYLIST_CREATE:-}" != "YES" ]]; then
    cat >&2 <<'EOF'
This smoke test creates one real empty YouTube Music playlist.
Set NOCKY_CONFIRM_PLAYLIST_CREATE=YES to confirm the remote change.
EOF
    exit 2
fi

TITLE="${NOCKY_PLAYLIST_TITLE:-}"
DESCRIPTION="${NOCKY_PLAYLIST_DESCRIPTION:-Created by the Nocky playlist creation smoke test}"
PRIVACY="${NOCKY_PLAYLIST_PRIVACY:-PRIVATE}"

if [[ -z "${TITLE//[[:space:]]/}" ]]; then
    echo "Set NOCKY_PLAYLIST_TITLE to a non-empty test playlist title." >&2
    exit 2
fi

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

"$YOUTUBE_PYTHON" - "$PAYLOAD_FILE" "$TITLE" "$DESCRIPTION" "$PRIVACY" <<'PY'
from __future__ import annotations

import json
import sys
from pathlib import Path

Path(sys.argv[1]).write_text(
    json.dumps(
        {
            "title": sys.argv[2],
            "description": sys.argv[3],
            "privacy": sys.argv[4],
        },
        ensure_ascii=False,
    ),
    encoding="utf-8",
)
PY

cd "$ROOT_DIR"
"$YOUTUBE_PYTHON" helpers/nocky_youtube_playlist_create.py \
    <"$PAYLOAD_FILE" >"$RESULT_FILE" || true

"$YOUTUBE_PYTHON" - "$RESULT_FILE" <<'PY'
from __future__ import annotations

import json
import sys
from pathlib import Path

try:
    payload = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
except Exception as error:
    raise SystemExit(f"Playlist creation returned invalid diagnostic output: {error}")

if not payload.get("ok"):
    raise SystemExit(f"Playlist creation failed: {payload.get('error') or 'unknown error'}")

result = payload.get("result")
if not isinstance(result, dict):
    raise SystemExit("Playlist creation returned an invalid result")

allowed = {"playlist_id", "title", "privacy"}
if set(result) != allowed:
    raise SystemExit("Playlist creation returned unexpected fields")

print("YouTube Music playlist creation")
print(f"title: {result.get('title')}")
print(f"privacy: {result.get('privacy')}")
print(f"playlist id: {result.get('playlist_id')}")
PY
