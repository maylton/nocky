#!/usr/bin/env python3
"""Return and refresh privacy-safe YouTube Music profile metadata."""

from __future__ import annotations

import json
import sys
from typing import Any

import nocky_youtube
from nocky_account_profile import (
    normalize_account_profile,
    profile_from_session,
    profile_storage_payload,
)


def _emit(payload: Any) -> None:
    json.dump(payload, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")


def _refreshed_profile(session: dict[str, Any]) -> dict[str, str]:
    stored = profile_from_session(session)
    headers = session.get("headers")
    if not isinstance(headers, dict) or not headers:
        return stored

    try:
        client = nocky_youtube._create_client(authenticated=True)
        account_info = client.get_account_info()
        refreshed = normalize_account_profile(
            account_info,
            fallback_name=stored.get("name", ""),
        )
    except Exception as error:
        # Profile metadata is decorative. A transient account-menu failure must
        # never turn a valid authenticated session into a disconnected account.
        print(f"Nocky YouTube account profile refresh skipped: {error}", file=sys.stderr)
        return stored

    session["profile"] = profile_storage_payload(refreshed)
    if refreshed.get("name"):
        session["account"] = refreshed["name"]

    nocky_youtube._store_session(session)
    return refreshed


def main() -> int:
    try:
        session = nocky_youtube._load_session()
        profile = _refreshed_profile(session)
        _emit({"ok": True, "result": profile})
        return 0
    except Exception as error:
        _emit({"ok": False, "error": str(error) or error.__class__.__name__})
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
