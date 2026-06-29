#!/usr/bin/env python3
"""Add one item to one confirmed-owned YouTube Music playlist.

The helper is not wired into GTK. It accepts a single JSON request on stdin and
returns only the sanitized confirmation contract. The caller must reconcile the
playlist through a fresh read before treating native state as final.
"""

from __future__ import annotations

import json
import sys
from typing import Any

import nocky_youtube
from nocky_playlist_add_contract import normalize_add_request, sanitize_add_result


def _emit(payload: Any) -> None:
    json.dump(payload, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")


def _read_input() -> dict[str, Any]:
    raw = sys.stdin.read()
    if not raw.strip():
        return {}
    payload = json.loads(raw)
    if not isinstance(payload, dict):
        raise RuntimeError("Expected a playlist item addition object")
    return payload


def add_playlist_item(payload: Any) -> dict[str, Any]:
    request = normalize_add_request(payload)

    session = nocky_youtube._load_session()
    headers = session.get("headers")
    if not isinstance(headers, dict) or not headers:
        raise RuntimeError("Connect a YouTube Music browser session first")

    client = nocky_youtube._create_client(authenticated=True)
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
        video_id=request["video_ids"][0],
    )


def main() -> int:
    try:
        result = add_playlist_item(_read_input())
        _emit({"ok": True, "result": result})
        return 0
    except Exception as error:
        _emit({"ok": False, "error": str(error) or error.__class__.__name__})
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
