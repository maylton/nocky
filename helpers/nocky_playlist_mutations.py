"""Validation and sanitization for YouTube Music playlist operations.

The installed module contains the non-network contracts shared by playlist
creation, read-only metadata inspection and single-item addition. Every request
is validated before the YouTube client is created, and raw service responses
never cross the helper boundary.
"""

from __future__ import annotations

import re
from typing import Any

ALLOWED_PRIVACY = {"PRIVATE", "UNLISTED", "PUBLIC"}
PLAYLIST_ID_PATTERN = re.compile(r"^[A-Za-z0-9_-]+$")
VIDEO_ID_PATTERN = re.compile(r"^[A-Za-z0-9_-]{11}$")
_SUCCESS_STATUS = "STATUS_SUCCEEDED"


def _text(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, str):
        return value.strip()
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, dict):
        for key in ("name", "title", "text"):
            text = _text(value.get(key))
            if text:
                return text
    return ""


def normalize_playlist_id(value: Any) -> str:
    playlist_id = _text(value)
    if playlist_id.startswith("VL"):
        playlist_id = playlist_id[2:]
    if not playlist_id or not PLAYLIST_ID_PATTERN.fullmatch(playlist_id):
        raise RuntimeError("Invalid YouTube Music playlist ID")
    return playlist_id


def normalize_video_id(value: Any) -> str:
    video_id = _text(value)
    if not VIDEO_ID_PATTERN.fullmatch(video_id):
        raise RuntimeError("Invalid YouTube video ID")
    return video_id


def normalize_playlist_detail(payload: Any) -> dict[str, Any]:
    """Return the privacy-safe read-only playlist metadata contract."""

    source = payload if isinstance(payload, dict) else {}
    tracks: list[dict[str, str]] = []

    raw_tracks = source.get("tracks")
    if isinstance(raw_tracks, list):
        for track in raw_tracks:
            if not isinstance(track, dict):
                continue
            video_id = _text(track.get("videoId") or track.get("video_id"))
            if not video_id:
                continue
            tracks.append(
                {
                    "video_id": video_id,
                    "set_video_id": _text(
                        track.get("setVideoId") or track.get("set_video_id")
                    ),
                    "title": _text(track.get("title") or track.get("name")),
                }
            )

    playlist_id = ""
    for key in ("playlistId", "playlist_id", "id"):
        playlist_id = _text(source.get(key))
        if playlist_id:
            break
    if not playlist_id:
        playlist_id = _text(source.get("browseId") or source.get("browse_id"))
    if playlist_id.startswith("VL"):
        playlist_id = playlist_id[2:]

    privacy = _text(source.get("privacy") or source.get("privacyStatus")).upper()
    if privacy not in ALLOWED_PRIVACY:
        privacy = ""

    owned = source.get("owned") is True
    return {
        "playlist_id": playlist_id,
        "title": _text(source.get("title") or source.get("name")),
        "owned": owned,
        "privacy": privacy,
        "editable": owned and bool(playlist_id),
        "tracks": tracks,
    }


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


def normalize_add_request(payload: Any) -> dict[str, Any]:
    """Return a single-item, duplicate-safe addition request."""

    if not isinstance(payload, dict):
        raise RuntimeError("Expected a playlist item addition object")
    if payload.get("owned") is not True or payload.get("editable") is not True:
        raise RuntimeError("Playlist ownership and editability must be confirmed")

    playlist_id = normalize_playlist_id(
        payload.get("playlist_id") or payload.get("playlistId")
    )
    source_playlist = str(
        payload.get("source_playlist") or payload.get("sourcePlaylist") or ""
    ).strip()
    if source_playlist:
        raise RuntimeError("Source-playlist additions are not supported")
    if payload.get("duplicates") is True:
        raise RuntimeError("Duplicate playlist items are not allowed")

    raw_video_ids = payload.get("video_ids") or payload.get("videoIds")
    if raw_video_ids is None:
        raw_video_ids = [payload.get("video_id") or payload.get("videoId")]
    if not isinstance(raw_video_ids, list) or len(raw_video_ids) != 1:
        raise RuntimeError("Exactly one playlist item is required")

    return {
        "playlist_id": playlist_id,
        "video_ids": [normalize_video_id(raw_video_ids[0])],
        "duplicates": False,
    }


def sanitize_add_result(
    raw_result: Any,
    *,
    playlist_id: str,
    video_id: str,
) -> dict[str, Any]:
    status = ""
    if isinstance(raw_result, str):
        status = raw_result.strip()
    elif isinstance(raw_result, dict):
        status = str(raw_result.get("status") or "").strip()

    if status != _SUCCESS_STATUS:
        raise RuntimeError("YouTube Music did not confirm the playlist item addition")

    return {
        "playlist_id": normalize_playlist_id(playlist_id),
        "video_id": normalize_video_id(video_id),
        "added_count": 1,
        "reconciliation_required": True,
    }
