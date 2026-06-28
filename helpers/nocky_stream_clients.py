#!/usr/bin/env python3
"""YouTube stream-client policy and diagnostics for Nocky.

This module contains only policy/command-building logic. It deliberately avoids
network access so behavior can be covered by deterministic unit tests.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass
import json
import os
from pathlib import Path
import re
from typing import Iterable


@dataclass(frozen=True)
class StreamClientProfile:
    key: str
    label: str
    player_client: str
    auth_mode: str
    enabled_by_default: bool
    description: str

    def can_run(self, has_auth: bool) -> bool:
        return self.auth_mode != "required" or has_auth

    def use_auth(self, has_auth: bool) -> bool:
        return has_auth and self.auth_mode in {"prefer", "required"}


PROFILES: dict[str, StreamClientProfile] = {
    "web_music": StreamClientProfile(
        key="web_music",
        label="WEB_REMIX",
        player_client="web_music",
        auth_mode="prefer",
        enabled_by_default=True,
        description="Primary YouTube Music web client; prefers the connected session.",
    ),
    "web_creator": StreamClientProfile(
        key="web_creator",
        label="WEB_CREATOR",
        player_client="web_creator",
        auth_mode="required",
        enabled_by_default=True,
        description="Authenticated creator web client used as a Premium fallback.",
    ),
    "tv": StreamClientProfile(
        key="tv",
        label="TVHTML5",
        player_client="tv",
        auth_mode="prefer",
        enabled_by_default=True,
        description="TV web client; reliable fallback and supports cookies.",
    ),
    "android_vr": StreamClientProfile(
        key="android_vr",
        label="Android VR",
        player_client="android_vr",
        auth_mode="never",
        enabled_by_default=True,
        description="Native client fallback that does not rely on browser cookies.",
    ),
    "web": StreamClientProfile(
        key="web",
        label="WEB",
        player_client="web",
        auth_mode="prefer",
        enabled_by_default=True,
        description="General web client retained as the final compatibility fallback.",
    ),
    "ios": StreamClientProfile(
        key="ios",
        label="iOS / iPadOS",
        player_client="ios",
        auth_mode="never",
        enabled_by_default=False,
        description="Disabled by default because token/throttling behavior is less predictable.",
    ),
}

DEFAULT_ORDER = ("web_music", "web_creator", "tv", "android_vr", "web")
CANONICAL_ORDER = tuple(PROFILES)
ENV_ORDER = "NOCKY_YOUTUBE_STREAM_CLIENTS"
ENV_CONFIG_FILE = "NOCKY_CONFIG_FILE"

_TERMINAL_PATTERNS = (
    "age-restricted",
    "age restricted",
    "confirm your age",
    "private video",
    "video is private",
    "video unavailable",
    "this video has been removed",
    "copyright",
    "not available in your country",
    "not available in your region",
)

_AUTH_PATTERNS = (
    "only available to music premium members",
    "music premium members",
    "premium members",
    "sign in to confirm you're not a bot",
    "sign in to confirm you’re not a bot",
    "login required",
    "authentication required",
    "use --cookies-from-browser or --cookies",
)

_RECOVERABLE_PATTERNS = (
    "http error 403",
    "403 forbidden",
    "http error 429",
    "too many requests",
    "requested format is not available",
    "no video formats found",
    "no playable audio stream",
    "nsig extraction failed",
    "signature extraction failed",
    "po token",
    "timed out",
    "timeout",
    "connection reset",
    "temporary failure",
    "remote end closed connection",
)


def _validated_keys(values: Iterable[object]) -> list[str]:
    requested: list[str] = []
    for value in values:
        key = str(value).strip().lower()
        if key in PROFILES and key not in requested:
            requested.append(key)
    return requested


def _config_path() -> Path:
    override = os.environ.get(ENV_CONFIG_FILE, "").strip()
    if override:
        return Path(override).expanduser()
    root = Path(
        os.environ.get("XDG_CONFIG_HOME", "").strip()
        or (Path.home() / ".config")
    )
    return root / "nocky" / "config.json"


def configured_order() -> list[str] | None:
    """Read the normalized enabled order from Nocky's persisted configuration.

    Invalid or unavailable configuration is ignored so the built-in policy remains
    a reliable fallback. This function never returns sensitive configuration data.
    """

    try:
        payload = json.loads(_config_path().read_text(encoding="utf-8"))
    except (OSError, ValueError, TypeError):
        return None

    sources = payload.get("youtube_stream_sources")
    if not isinstance(sources, dict):
        return None

    raw_order = sources.get("order")
    raw_disabled = sources.get("disabled")
    if not isinstance(raw_order, list) and not isinstance(raw_disabled, list):
        return None

    order = _validated_keys(raw_order if isinstance(raw_order, list) else [])
    for key in CANONICAL_ORDER:
        if key not in order:
            order.append(key)

    disabled = set(
        _validated_keys(raw_disabled if isinstance(raw_disabled, list) else [])
    )
    enabled = [key for key in order if key not in disabled]
    return enabled or None


def _requested_order_state(raw: str | None) -> tuple[list[str], bool]:
    if raw is not None:
        requested = _validated_keys((raw or "").split(","))
        return (requested or list(DEFAULT_ORDER), bool(requested))

    environment = os.environ.get(ENV_ORDER, "").strip()
    if environment:
        requested = _validated_keys(environment.split(","))
        return (requested or list(DEFAULT_ORDER), bool(requested))

    persisted = configured_order()
    if persisted:
        return persisted, True

    return list(DEFAULT_ORDER), False


def parse_requested_order(raw: str | None) -> list[str]:
    """Return a validated, de-duplicated client order.

    Explicit environment values have priority. When no environment override is
    present, the persisted Nocky preference is used. Disabled profiles such as
    ``ios`` are accepted only when explicitly enabled in one of those sources.
    """

    order, _ = _requested_order_state(raw)
    return order


def ordered_profiles(
    *,
    has_auth: bool,
    failed_client: str = "",
    requested_order: Iterable[str] | None = None,
) -> list[StreamClientProfile]:
    if requested_order is not None:
        keys = _validated_keys(requested_order)
        explicit = True
    else:
        keys, explicit = _requested_order_state(None)

    profiles: list[StreamClientProfile] = []
    for key in keys:
        profile = PROFILES.get(str(key).strip().lower())
        if profile is None or not profile.can_run(has_auth):
            continue
        if not profile.enabled_by_default and not explicit:
            continue
        if profile not in profiles:
            profiles.append(profile)

    failed_client = failed_client.strip().lower()
    if failed_client:
        failed = [profile for profile in profiles if profile.key == failed_client]
        profiles = [profile for profile in profiles if profile.key != failed_client] + failed
    return profiles


def build_attempt_command(
    base_command: list[str],
    webpage_url: str,
    profile: StreamClientProfile,
    auth_args: list[str],
) -> list[str]:
    command = list(base_command)
    if command and command[-1] == webpage_url:
        command.pop()
    if profile.use_auth(bool(auth_args)):
        command.extend(auth_args)
    command.extend(
        [
            "--extractor-args",
            f"youtube:player_client={profile.player_client}",
            webpage_url,
        ]
    )
    return command


def error_category(message: str) -> str:
    normalized = (message or "").lower()
    if any(pattern in normalized for pattern in _TERMINAL_PATTERNS):
        return "terminal"
    if any(pattern in normalized for pattern in _AUTH_PATTERNS):
        return "authentication"
    if any(pattern in normalized for pattern in _RECOVERABLE_PATTERNS):
        return "recoverable"
    return "unknown"


def should_try_next_client(message: str) -> bool:
    return error_category(message) != "terminal"


def concise_process_error(stderr: str, stdout: str = "", line_limit: int = 6) -> str:
    lines = [line.strip() for line in (stderr or stdout or "").splitlines() if line.strip()]
    return redact_sensitive_text("\n".join(lines[-line_limit:]) or "yt-dlp could not resolve this track")


def redact_sensitive_text(message: str) -> str:
    text = message or ""
    text = re.sub(r"https?://[^\s'\"]+", "<redacted-url>", text)
    text = re.sub(
        r"(?i)(cookie|authorization|x-goog-authuser)\s*[:=]\s*[^\s,;]+",
        r"\1=<redacted>",
        text,
    )
    return text


def policy_snapshot(*, has_auth: bool, failed_client: str = "") -> dict[str, object]:
    order = ordered_profiles(has_auth=has_auth, failed_client=failed_client)
    return {
        "order": [profile.key for profile in order],
        "clients": [
            {
                **asdict(profile),
                "available": profile.can_run(has_auth),
                "uses_auth": profile.use_auth(has_auth),
            }
            for profile in PROFILES.values()
        ],
    }
