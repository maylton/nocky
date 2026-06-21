#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HELPER="${NOCKY_YOUTUBE_HELPER:-${ROOT_DIR}/helpers/nocky_youtube.py}"

find_runtime_python() {
  if [[ -n "${NOCKY_PYTHON:-}" && -x "${NOCKY_PYTHON}" ]]; then
    printf '%s\n' "$NOCKY_PYTHON"
    return
  fi
  for candidate in \
    "${ROOT_DIR}/.nocky-runtime/bin/python3" \
    "${HOME}/.local/share/nocky/runtime/bin/python3" \
    "/usr/local/share/nocky/runtime/bin/python3" \
    "/usr/share/nocky/runtime/bin/python3"; do
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return
    fi
  done
  command -v python3 || true
}

PYTHON="$(find_runtime_python)"
[[ -n "$PYTHON" ]] || { echo "Python 3 was not found." >&2; exit 1; }
[[ -f "$HELPER" ]] || { echo "YouTube helper not found: $HELPER" >&2; exit 1; }

"$PYTHON" -m py_compile "$HELPER"
"$PYTHON" - <<'PY'
import importlib.util
for name in ('requests', 'ytmusicapi'):
    if importlib.util.find_spec(name) is None:
        raise SystemExit(f'Missing Python module: {name}')
print('Python modules: requests and ytmusicapi available')
PY

YTDLP="${NOCKY_YTDLP:-}"
if [[ -z "$YTDLP" ]]; then
  YTDLP="$(command -v yt-dlp || true)"
fi
if [[ -z "$YTDLP" ]]; then
  sibling="$(dirname "$PYTHON")/yt-dlp"
  [[ -x "$sibling" ]] && YTDLP="$sibling"
fi
if [[ -n "$YTDLP" && -x "$YTDLP" ]]; then
  echo "yt-dlp: $($YTDLP --version | head -n 1)"
else
  "$PYTHON" - <<'PY'
import importlib.util
if importlib.util.find_spec('yt_dlp') is None:
    raise SystemExit('yt-dlp executable or Python module was not found')
print('yt-dlp Python module: available')
PY
fi

DENO="${NOCKY_DENO:-}"
if [[ -z "$DENO" ]]; then
  DENO="$(command -v deno || true)"
fi
if [[ -z "$DENO" ]]; then
  sibling="$(dirname "$PYTHON")/deno"
  [[ -x "$sibling" ]] && DENO="$sibling"
fi
[[ -n "$DENO" && -x "$DENO" ]] || { echo "Deno was not found." >&2; exit 1; }
echo "$($DENO --version | head -n 1)"

response="$(printf '{}\n' | "$PYTHON" "$HELPER" status)"
python3 - "$response" <<'PY'
import json, sys
payload = json.loads(sys.argv[1])
assert payload.get('ok') is True, payload
status = payload.get('result') or {}
print('YouTube helper: available')
print('Account session:', 'connected' if status.get('connected') else 'not connected')
print('Storage backend:', status.get('storage') or 'none')
PY

CACHE_ROOT="${XDG_CACHE_HOME:-${HOME}/.cache}/nocky/youtube"
python3 - "$CACHE_ROOT" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
stream_path = root / "stream-cache.json"
library_path = root / "library-cache.json"
cover_dir = root / "covers"


def count_entries(path, key):
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
        value = payload.get(key, {})
        return len(value) if isinstance(value, (dict, list)) else 0
    except Exception:
        return 0


print("Cached stream URLs:", count_entries(stream_path, "streams"))
if library_path.is_file():
    try:
        payload = json.loads(library_path.read_text(encoding="utf-8"))
        print("Cached library tracks:", len(payload.get("library") or []))
        print("Cached liked tracks:", len(payload.get("liked") or []))
        print("Cached playlists:", len(payload.get("playlists") or []))
    except Exception:
        print("Cached library snapshot: unreadable")
else:
    print("Cached library snapshot: none")
print(
    "Cached cover files:",
    sum(1 for path in cover_dir.glob("*") if path.is_file()) if cover_dir.is_dir() else 0,
)
PY
