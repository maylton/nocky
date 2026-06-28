#!/usr/bin/env python3
"""YouTube stream-client policy and diagnostics for Nocky.

This module contains only policy/command-building logic. It deliberately avoids
network access so behavior can be covered by deterministic unit tests.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass
import os
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
ENV_ORDER = "NOCKY_YOUTUBE_STREAM_CLIENTS"

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


def parse_requested_order(raw: str | None) -> list[str]:
    """Return a validated, de-duplicated client order.

    Unknown values are ignored. Disabled profiles such as ``ios`` are accepted
    only when explicitly named by the user/environment.
    """

    if raw is None:
        raw = os.environ.get(ENV_ORDER, "")
    tokens = [token.strip().lower() for token in (raw or "").split(",")]
    requested: list[str] = []
    for token in tokens:
        if token in PROFILES and token not in requested:
            requested.append(token)
    return requested or list(DEFAULT_ORDER)


def ordered_profiles(
    *,
    has_auth: bool,
    failed_client: str = "",
    requested_order: Iterable[str] | None = None,
) -> list[StreamClientProfile]:
    keys = list(requested_order) if requested_order is not None else parse_requested_order(None)
    explicit = requested_order is not None or bool(os.environ.get(ENV_ORDER, "").strip())

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
