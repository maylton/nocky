"""Privacy-safe normalization for authenticated YouTube Music playlist details.

The module is intentionally network-free. It accepts the dictionary returned by
``ytmusicapi.get_playlist`` and exposes only fields required to decide whether a
future playlist operation can be offered safely.
"""

from __future__ import annotations

from typing import Any

_ALLOWED_PRIVACY = {"PRIVATE", "UNLISTED", "PUBLIC"}


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


def _playlist_id(payload: dict[str, Any]) -> str:
    for key in ("playlistId", "playlist_id", "id"):
        value = _text(payload.get(key))
        if value:
            return value[2:] if value.startswith("VL") else value

    browse_id = _text(payload.get("browseId") or payload.get("browse_id"))
    return browse_id[2:] if browse_id.startswith("VL") else browse_id


def _privacy(value: Any) -> str:
    privacy = _text(value).upper()
    return privacy if privacy in _ALLOWED_PRIVACY else ""


def normalize_playlist_detail(payload: Any) -> dict[str, Any]:
    """Return the read-only playlist editability contract.

    Duplicate video IDs are preserved because each playlist occurrence can have
    a distinct ``setVideoId`` and therefore a distinct removal identity.
    """

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

    playlist_id = _playlist_id(source)
    owned = source.get("owned") is True
    return {
        "playlist_id": playlist_id,
        "title": _text(source.get("title") or source.get("name")),
        "owned": owned,
        "privacy": _privacy(
            source.get("privacy") or source.get("privacyStatus")
        ),
        "editable": owned and bool(playlist_id),
        "tracks": tracks,
    }
