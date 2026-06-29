"""Validation and sanitization for YouTube Music playlist mutations.

The first delivery supports creating empty playlists only. This module is kept
free of network and persistence code so every request is validated before the
YouTube client is created.
"""

from __future__ import annotations

import re
from typing import Any

ALLOWED_PRIVACY = {"PRIVATE", "UNLISTED", "PUBLIC"}
PLAYLIST_ID_PATTERN = re.compile(r"^[A-Za-z0-9_-]+$")


def normalize_create_request(payload: Any) -> dict[str, str]:
    if not isinstance(payload, dict):
        raise RuntimeError("Expected a playlist creation object")

    title = str(payload.get("title") or "").strip()
    if not title:
        raise RuntimeError("Playlist title is required")
    if "<" in title or ">" in title:
        raise RuntimeError("Playlist title contains unsupported characters")

    description = str(payload.get("description") or "").strip()
    privacy = str(payload.get("privacy") or "PRIVATE").strip().upper()
    if privacy not in ALLOWED_PRIVACY:
        raise RuntimeError("Playlist privacy must be PRIVATE, UNLISTED, or PUBLIC")

    video_ids = payload.get("video_ids") or payload.get("videoIds") or []
    source_playlist = str(payload.get("source_playlist") or "").strip()
    if video_ids or source_playlist:
        raise RuntimeError("This checkpoint creates empty playlists only")

    return {
        "title": title,
        "description": description,
        "privacy": privacy,
    }


def sanitize_create_result(
    raw_result: Any,
    *,
    title: str,
    privacy: str,
) -> dict[str, str]:
    playlist_id = ""
    if isinstance(raw_result, str):
        playlist_id = raw_result.strip()
    elif isinstance(raw_result, dict):
        playlist_id = str(raw_result.get("playlistId") or "").strip()

    if not playlist_id or not PLAYLIST_ID_PATTERN.fullmatch(playlist_id):
        raise RuntimeError("YouTube Music did not confirm playlist creation")

    return {
        "playlist_id": playlist_id,
        "title": title,
        "privacy": privacy,
    }
