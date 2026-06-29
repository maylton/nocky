"""Privacy-safe YouTube Music account profile normalization.

This module deliberately handles display metadata only. Authentication headers,
cookies and authorization values never enter the returned profile contract.
"""

from __future__ import annotations

from typing import Any

PROFILE_KEYS = ("name", "channel_handle", "photo_url")


def _text(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, str):
        return value.strip()
    if isinstance(value, (int, float)):
        return str(value).strip()
    return ""


def empty_profile() -> dict[str, str]:
    return {key: "" for key in PROFILE_KEYS}


def normalize_account_profile(
    account_info: Any,
    *,
    fallback_name: str = "",
) -> dict[str, str]:
    """Return the stable, display-only profile contract.

    Current ytmusicapi responses expose ``accountName``, ``channelHandle`` and
    ``accountPhotoUrl`` directly. Older fixtures and compatibility responses may
    nest an active account inside ``accounts``; both shapes are accepted.
    """

    source = account_info if isinstance(account_info, dict) else {}
    nested_accounts = source.get("accounts")
    nested = (
        nested_accounts[0]
        if isinstance(nested_accounts, list)
        and nested_accounts
        and isinstance(nested_accounts[0], dict)
        else {}
    )

    name = _text(
        source.get("accountName")
        or source.get("name")
        or nested.get("accountName")
        or nested.get("name")
        or fallback_name
    )
    channel_handle = _text(
        source.get("channelHandle")
        or source.get("channel_handle")
        or nested.get("channelHandle")
        or nested.get("channel_handle")
    )
    photo_url = _text(
        source.get("accountPhotoUrl")
        or source.get("photo_url")
        or nested.get("accountPhotoUrl")
        or nested.get("photo_url")
    )

    return {
        "name": name,
        "channel_handle": channel_handle,
        "photo_url": photo_url,
    }


def profile_from_session(session: Any) -> dict[str, str]:
    """Read profile metadata from current or legacy saved sessions.

    Existing sessions only stored ``account``. They remain valid and are mapped
    to the new profile name while handle and photo metadata stay empty.
    """

    payload = session if isinstance(session, dict) else {}
    profile = payload.get("profile")
    return normalize_account_profile(
        profile,
        fallback_name=_text(payload.get("account")),
    )


def profile_storage_payload(profile: Any) -> dict[str, str]:
    """Return only the allowlisted display fields suitable for persistence."""

    normalized = normalize_account_profile(profile)
    return {key: normalized[key] for key in PROFILE_KEYS}
