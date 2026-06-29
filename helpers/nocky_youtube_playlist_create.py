#!/usr/bin/env python3
"""Create one empty YouTube Music playlist through a sanitized contract."""

from __future__ import annotations

import json
import sys
from typing import Any

import nocky_youtube
from nocky_playlist_mutations import normalize_create_request, sanitize_create_result


def _emit(payload: Any) -> None:
    json.dump(payload, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")


def _read_input() -> dict[str, Any]:
    raw = sys.stdin.read()
    if not raw.strip():
        return {}
    payload = json.loads(raw)
    if not isinstance(payload, dict):
        raise RuntimeError("Expected a playlist creation object")
    return payload


def create_empty_playlist(payload: Any) -> dict[str, str]:
    request = normalize_create_request(payload)

    session = nocky_youtube._load_session()
    headers = session.get("headers")
    if not isinstance(headers, dict) or not headers:
        raise RuntimeError("Connect a YouTube Music browser session first")

    client = nocky_youtube._create_client(authenticated=True)
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


def main() -> int:
    try:
        result = create_empty_playlist(_read_input())
        _emit({"ok": True, "result": result})
        return 0
    except Exception as error:
        _emit({"ok": False, "error": str(error) or error.__class__.__name__})
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
