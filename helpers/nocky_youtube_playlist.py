#!/usr/bin/env python3
"""Read-only YouTube Music playlist metadata helper.

The helper accepts one playlist ID, fetches the authenticated playlist detail,
and emits only the allowlisted contract from ``nocky_playlist_metadata``. It
never sends an edit request and never writes playlist metadata to disk.
"""

from __future__ import annotations

import json
import re
import sys
from typing import Any

import nocky_youtube
from nocky_playlist_metadata import normalize_playlist_detail

_PLAYLIST_ID_RE = re.compile(r"^[A-Za-z0-9_-]{2,200}$")


def _emit(payload: Any) -> None:
    json.dump(payload, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")


def normalize_playlist_id(value: str) -> str:
    playlist_id = str(value or "").strip()
    if playlist_id.startswith("VL"):
        playlist_id = playlist_id[2:]
    if not _PLAYLIST_ID_RE.fullmatch(playlist_id):
        raise RuntimeError("Invalid YouTube Music playlist ID")
    return playlist_id


def fetch_playlist_metadata(playlist_id: str, limit: int = 500) -> dict[str, Any]:
    session = nocky_youtube._load_session()
    headers = session.get("headers")
    if not isinstance(headers, dict) or not headers:
        raise RuntimeError("Connect a YouTube Music browser session first")

    normalized_id = normalize_playlist_id(playlist_id)
    safe_limit = max(1, min(500, int(limit)))
    client = nocky_youtube._create_client(authenticated=True)
    response = client.get_playlist(normalized_id, limit=safe_limit)
    if not isinstance(response, dict):
        raise RuntimeError("YouTube Music returned an invalid playlist response")
    return normalize_playlist_detail(response)


def _arguments(argv: list[str]) -> tuple[str, int]:
    if not argv:
        raise RuntimeError("A YouTube Music playlist ID is required")
    if len(argv) == 1:
        return argv[0], 500
    if len(argv) == 3 and argv[1] == "--limit":
        try:
            return argv[0], int(argv[2])
        except ValueError as error:
            raise RuntimeError("Playlist detail limit must be an integer") from error
    raise RuntimeError("Usage: nocky_youtube_playlist.py PLAYLIST_ID [--limit NUMBER]")


def main(argv: list[str] | None = None) -> int:
    try:
        playlist_id, limit = _arguments(list(sys.argv[1:] if argv is None else argv))
        result = fetch_playlist_metadata(playlist_id, limit)
        _emit({"ok": True, "result": result})
        return 0
    except Exception as error:
        _emit({"ok": False, "error": str(error) or error.__class__.__name__})
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
