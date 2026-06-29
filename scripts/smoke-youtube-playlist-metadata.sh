#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

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

"$YOUTUBE_PYTHON" - "$ROOT_DIR" <<'PY'
from __future__ import annotations

import sys
from pathlib import Path
from typing import Any

root = Path(sys.argv[1])
sys.path.insert(0, str(root / "helpers"))

import nocky_youtube
from nocky_playlist_metadata import normalize_playlist_detail


def text(value: Any) -> str:
    return str(value or "").strip()


def playlist_id(item: dict[str, Any]) -> str:
    value = text(
        item.get("playlistId")
        or item.get("playlist_id")
        or item.get("id")
        or item.get("browseId")
        or item.get("browse_id")
    )
    return value[2:] if value.startswith("VL") else value


session = nocky_youtube._load_session()
headers = session.get("headers")
if not isinstance(headers, dict) or not headers:
    raise SystemExit("Playlist metadata smoke test failed: connect YouTube Music first")

client = nocky_youtube._create_client(authenticated=True)
playlists = client.get_library_playlists(limit=25) or []
selected: dict[str, Any] | None = None

for item in playlists[:25]:
    if not isinstance(item, dict):
        continue
    candidate_id = playlist_id(item)
    if not candidate_id:
        continue
    try:
        raw = client.get_playlist(candidate_id, limit=500)
    except Exception:
        continue
    normalized = normalize_playlist_detail(raw)
    if normalized.get("owned") is True and normalized.get("editable") is True:
        selected = normalized
        break

if selected is None:
    raise SystemExit(
        "Playlist metadata smoke test could not find an owned editable playlist"
    )

allowed_top = {"playlist_id", "title", "owned", "privacy", "editable", "tracks"}
allowed_track = {"video_id", "set_video_id", "title"}
if set(selected) != allowed_top:
    raise SystemExit("Playlist metadata smoke test found unexpected top-level fields")

tracks = selected.get("tracks")
if not isinstance(tracks, list):
    raise SystemExit("Playlist metadata smoke test returned an invalid track list")
for track in tracks:
    if not isinstance(track, dict) or set(track) != allowed_track:
        raise SystemExit("Playlist metadata smoke test found unexpected track fields")

complete = [
    track
    for track in tracks
    if text(track.get("video_id")) and text(track.get("set_video_id"))
]
video_ids = [text(track.get("video_id")) for track in tracks if text(track.get("video_id"))]
set_ids = [text(track.get("set_video_id")) for track in complete]
duplicate_occurrences = len(video_ids) - len(set(video_ids))
unique_set_identity = len(set_ids) == len(set(set_ids))

print("YouTube Music playlist metadata")
print("owned: true")
print(f"privacy: {selected.get('privacy') or 'unknown'}")
print("editable: true")
print(f"tracks: {len(tracks)}")
print(f"complete occurrence identities: {len(complete)}")
print(f"duplicate video occurrences: {duplicate_occurrences}")
print(f"unique set identities: {str(unique_set_identity).lower()}")
PY
