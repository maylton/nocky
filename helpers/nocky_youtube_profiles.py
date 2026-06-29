#!/usr/bin/env python3
"""Read-only YouTube Music account profile discovery diagnostic.

The raw ``account/accounts_list`` response never leaves this process. Only the
allowlisted contract produced by ``nocky_account_discovery`` is written to
stdout. This helper does not switch profiles or persist discovered identifiers.
"""

from __future__ import annotations

import json
import sys
from typing import Any

import nocky_youtube
from nocky_account_discovery import discover_account_profiles


def _emit(payload: Any) -> None:
    json.dump(payload, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")


def _raw_accounts_list(client: Any) -> dict[str, Any]:
    sender = getattr(client, "_send_request", None)
    if not callable(sender):
        raise RuntimeError(
            "The installed ytmusicapi version does not expose account discovery"
        )

    response = sender("account/accounts_list", {})
    if not isinstance(response, dict):
        raise RuntimeError("YouTube Music returned an invalid account-list response")
    return response


def discover_profiles() -> dict[str, Any]:
    session = nocky_youtube._load_session()
    headers = session.get("headers")
    if not isinstance(headers, dict) or not headers:
        raise RuntimeError("Connect a YouTube Music browser session first")

    client = nocky_youtube._create_client(authenticated=True)
    raw_response = _raw_accounts_list(client)
    return discover_account_profiles(raw_response)


def main() -> int:
    try:
        _emit({"ok": True, "result": discover_profiles()})
        return 0
    except Exception as error:
        _emit({"ok": False, "error": str(error) or error.__class__.__name__})
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
