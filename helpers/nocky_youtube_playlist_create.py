#!/usr/bin/env python3
"""Sanitized YouTube Music playlist helper.

The installed entry point handles empty-playlist creation, read-only metadata
inspection and one duplicate-safe item addition. Every operation emits only its
allowlisted contract, and add requests revalidate remote ownership before the
mutation is attempted.
"""

from __future__ import annotations

import json
import sys
from typing import Any

import nocky_youtube
from nocky_playlist_mutations import (
    normalize_add_request,
    normalize_create_request,
    normalize_playlist_detail,
    normalize_playlist_id,
    sanitize_add_result,
    sanitize_create_result,
)


def _emit(payload: Any) -> None:
    json.dump(payload, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")


def _read_input() -> dict[str, Any]:
    raw = sys.stdin.read()
    if not raw.strip():
        return {}
    payload = json.loads(raw)
    if not isinstance(payload, dict):
        raise RuntimeError("Expected a playlist helper object")
    return payload


def _authenticated_client() -> Any:
    session = nocky_youtube._load_session()
    headers = session.get("headers")
    if not isinstance(headers, dict) or not headers:
        raise RuntimeError("Connect a YouTube Music browser session first")
    return nocky_youtube._create_client(authenticated=True)


def _read_metadata(client: Any, playlist_id: str, limit: int) -> dict[str, Any]:
    reader = getattr(client, "get_playlist", None)
    if not callable(reader):
        raise RuntimeError("The installed YouTube Music runtime cannot inspect playlists")

    raw_result = reader(playlist_id, limit=limit)
    if not isinstance(raw_result, dict):
        raise RuntimeError("YouTube Music returned an invalid playlist response")

    result = normalize_playlist_detail(raw_result)
    if result.get("playlist_id") != playlist_id:
        raise RuntimeError("YouTube Music returned mismatched playlist metadata")
    return result


def create_empty_playlist(payload: Any) -> dict[str, str]:
    request = normalize_create_request(payload)
    client = _authenticated_client()
    creator = getattr(client, "create_playlist", None)
    if not callable(creator):
        raise RuntimeError("The installed YouTube Music runtime cannot create playlists")

    raw_result = creator(
        request["title"],
        request["description"],
        privacy_status=request["privacy"],
    )
    return sanitize_create_result(
        raw_result,
        title=request["title"],
        privacy=request["privacy"],
    )


def fetch_playlist_metadata(payload: Any) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise RuntimeError("Expected a playlist metadata object")

    playlist_id = normalize_playlist_id(
        payload.get("playlist_id") or payload.get("playlistId")
    )
    try:
        limit = int(payload.get("limit") or 500)
    except (TypeError, ValueError) as error:
        raise RuntimeError("Playlist detail limit must be an integer") from error
    safe_limit = max(1, min(500, limit))

    client = _authenticated_client()
    return _read_metadata(client, playlist_id, safe_limit)


def add_playlist_item(payload: Any) -> dict[str, Any]:
    request = normalize_add_request(payload)
    client = _authenticated_client()

    metadata = _read_metadata(client, request["playlist_id"], 500)
    if metadata.get("owned") is not True or metadata.get("editable") is not True:
        raise RuntimeError("YouTube Music did not confirm playlist ownership and editability")
    video_id = request["video_ids"][0]
    if any(track.get("video_id") == video_id for track in metadata.get("tracks", [])):
        raise RuntimeError("The track is already in the playlist")

    adder = getattr(client, "add_playlist_items", None)
    if not callable(adder):
        raise RuntimeError("The installed YouTube Music runtime cannot add playlist items")

    raw_result = adder(
        request["playlist_id"],
        videoIds=request["video_ids"],
        duplicates=False,
    )
    return sanitize_add_result(
        raw_result,
        playlist_id=request["playlist_id"],
        video_id=video_id,
    )


def execute(payload: Any) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise RuntimeError("Expected a playlist helper object")
    operation = str(payload.get("operation") or "create").strip().lower()
    if operation == "create":
        return create_empty_playlist(payload)
    if operation == "metadata":
        return fetch_playlist_metadata(payload)
    if operation == "add":
        return add_playlist_item(payload)
    raise RuntimeError("Unsupported playlist helper operation")


def main() -> int:
    try:
        result = execute(_read_input())
        _emit({"ok": True, "result": result})
        return 0
    except Exception as error:
        _emit({"ok": False, "error": str(error) or error.__class__.__name__})
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
