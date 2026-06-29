"""Validation and sanitization for adding one item to a YouTube Music playlist.

This module is intentionally network-free. Native controls remain out of scope;
the first checkpoint only defines the smallest request and response boundary
needed for a future authenticated mutation.
"""

from __future__ import annotations

import re
from typing import Any

_PLAYLIST_ID_RE = re.compile(r"^[A-Za-z0-9_-]{2,200}$")
_VIDEO_ID_RE = re.compile(r"^[A-Za-z0-9_-]{11}$")
_SUCCESS_STATUS = "STATUS_SUCCEEDED"


def _playlist_id(value: Any) -> str:
    playlist_id = str(value or "").strip()
    if playlist_id.startswith("VL"):
        playlist_id = playlist_id[2:]
    if not _PLAYLIST_ID_RE.fullmatch(playlist_id):
        raise RuntimeError("Invalid YouTube Music playlist ID")
    return playlist_id


def _video_id(value: Any) -> str:
    video_id = str(value or "").strip()
    if not _VIDEO_ID_RE.fullmatch(video_id):
        raise RuntimeError("Invalid YouTube video ID")
    return video_id


def normalize_add_request(payload: Any) -> dict[str, Any]:
    """Return a single-item, duplicate-safe request.

    Ownership and effective editability are required inputs from the read-only
    metadata contract. They are checked before session or client access.
    """

    if not isinstance(payload, dict):
        raise RuntimeError("Expected a playlist item addition object")
    if payload.get("owned") is not True or payload.get("editable") is not True:
        raise RuntimeError("Playlist ownership and editability must be confirmed")

    playlist_id = _playlist_id(payload.get("playlist_id") or payload.get("playlistId"))

    source_playlist = str(
        payload.get("source_playlist") or payload.get("sourcePlaylist") or ""
    ).strip()
    if source_playlist:
        raise RuntimeError("Source-playlist additions are not supported in this checkpoint")

    if payload.get("duplicates") is True:
        raise RuntimeError("Duplicate playlist items are not allowed")

    raw_video_ids = payload.get("video_ids") or payload.get("videoIds")
    if raw_video_ids is None:
        raw_video_ids = [payload.get("video_id") or payload.get("videoId")]
    if not isinstance(raw_video_ids, list) or len(raw_video_ids) != 1:
        raise RuntimeError("This checkpoint adds exactly one playlist item")

    video_id = _video_id(raw_video_ids[0])
    return {
        "playlist_id": playlist_id,
        "video_ids": [video_id],
        "duplicates": False,
    }


def sanitize_add_result(
    raw_result: Any,
    *,
    playlist_id: str,
    video_id: str,
) -> dict[str, Any]:
    """Return only confirmation fields required before server reconciliation."""

    status = ""
    if isinstance(raw_result, str):
        status = raw_result.strip()
    elif isinstance(raw_result, dict):
        status = str(raw_result.get("status") or "").strip()

    if status != _SUCCESS_STATUS:
        raise RuntimeError("YouTube Music did not confirm the playlist item addition")

    return {
        "playlist_id": _playlist_id(playlist_id),
        "video_id": _video_id(video_id),
        "added_count": 1,
        "reconciliation_required": True,
    }
