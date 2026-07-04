#!/usr/bin/env python3
"""Nocky YouTube Music helper.

This sidecar keeps the native GTK application independent from Python APIs while
reusing the same proven integration strategy as the user's Nocturne project:
`ytmusicapi` for catalogue/account data and `yt-dlp` + Deno for temporary audio
stream URLs. Commands read JSON from stdin and write one JSON document to stdout.
"""

from __future__ import annotations

import fcntl
import functools
import hashlib
import json
import locale
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, urlparse, urlsplit, urlunsplit

from nocky_youtube_feed import (
    build_library_overview,
    build_structured_home,
    extract_inner_tube_home_chips,
    find_inner_tube_home_section_list,
    load_cached_page,
    save_cached_page,
)
from nocky_youtube_home_debug import write_home_debug_dump
from nocky_youtube_home_v3 import build as build_home_v3_source
from nocky_youtube_innertube_home import (
    missing_artwork_by_section,
    parse_inner_tube_home_sections,
)

from nocky_stream_clients import (
    build_attempt_command,
    concise_process_error,
    error_category,
    ordered_profiles,
    policy_snapshot,
    should_try_next_client,
)

try:
    import requests
    from ytmusicapi import YTMusic
    from ytmusicapi.exceptions import YTMusicServerError, YTMusicUserError
    from ytmusicapi.parsers import playlists as ytmusic_playlist_parsers
    try:
        from ytmusicapi.continuations import get_continuation_params as ytmusic_get_continuation_params
        from ytmusicapi.parsers.browsing import parse_mixed_content as ytmusic_parse_mixed_content
    except Exception:
        ytmusic_get_continuation_params = None
        ytmusic_parse_mixed_content = None
    try:
        from ytmusicapi.setup import setup as ytmusicapi_setup
    except Exception:
        ytmusicapi_setup = None
except Exception as error:  # pragma: no cover - reported to the native app
    requests = None
    YTMusic = None
    YTMusicServerError = RuntimeError
    YTMusicUserError = RuntimeError
    ytmusic_playlist_parsers = None
    ytmusic_get_continuation_params = None
    ytmusic_parse_mixed_content = None
    ytmusicapi_setup = None
    IMPORT_ERROR = error
else:
    IMPORT_ERROR = None


def _install_ytmusicapi_playlist_count_compat() -> None:
    """Prevent ytmusicapi from converting an empty playlist count with int("").

    ytmusicapi 1.12.1 extracts digits from the playlist subtitle and checks the
    resulting list with ``is not None``. An empty playlist therefore reaches
    ``to_int("")`` and raises ValueError. Patch only the playlists parser's local
    converter so digit-less count labels become zero while valid counts retain
    the upstream implementation.
    """

    parser = ytmusic_playlist_parsers
    if parser is None:
        return
    converter = getattr(parser, "to_int", None)
    if not callable(converter) or getattr(converter, "_nocky_empty_count_safe", False):
        return

    def safe_playlist_count(value: Any) -> int:
        text = str(value or "")
        if not re.search(r"\d", text):
            return 0
        return converter(text)

    safe_playlist_count._nocky_empty_count_safe = True  # type: ignore[attr-defined]
    parser.to_int = safe_playlist_count


_install_ytmusicapi_playlist_count_compat()

try:
    import gi
    gi.require_version("Secret", "1")
    from gi.repository import Secret
except Exception:
    Secret = None

APP_ID = "io.github.maylton.Nocky"
SCHEMA_NAME = APP_ID + ".YouTubeMusic"
AUTH_ATTRIBUTES = {"account": "youtube-music-browser"}
DEFAULT_USER_AGENT = (
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
)
VIDEO_ID_PATTERN = re.compile(r"^[A-Za-z0-9_-]{11}$")
STREAM_CACHE_SAFETY = 45
STREAM_CACHE_TTL = 600
STREAM_CACHE_LIMIT = 80


def _config_dir() -> Path:
    root = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    return root / "nocky"


def _cache_dir() -> Path:
    root = Path(os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache")
    return root / "nocky" / "youtube"


def _session_path() -> Path:
    return _config_dir() / "youtube-session.json"


def _stream_cache_path() -> Path:
    return _cache_dir() / "stream-cache.json"


def _stream_cache_lock_path() -> Path:
    return _cache_dir() / "stream-cache.lock"


def _home_feed_cache_path() -> Path:
    return _cache_dir() / "home-feed-v4.json"


def _emit(payload: Any) -> None:
    json.dump(payload, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")


def _read_input() -> dict[str, Any]:
    raw = sys.stdin.read()
    if not raw.strip():
        return {}
    payload = json.loads(raw)
    if not isinstance(payload, dict):
        raise RuntimeError("Expected a JSON object on stdin")
    return payload


def _secret_schema():
    if Secret is None:
        return None
    return Secret.Schema.new(
        SCHEMA_NAME,
        Secret.SchemaFlags.NONE,
        {"account": Secret.SchemaAttributeType.STRING},
    )


def _store_session(payload: dict[str, Any]) -> str:
    serialized = json.dumps(payload)
    schema = _secret_schema()
    if schema is not None:
        try:
            Secret.password_store_sync(
                schema,
                AUTH_ATTRIBUTES,
                Secret.COLLECTION_DEFAULT,
                "Nocky YouTube Music browser session",
                serialized,
                None,
            )
            fallback = _session_path()
            if fallback.exists():
                fallback.unlink()
            return "secret-service"
        except Exception:
            pass

    path = _session_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(".tmp")
    temporary.write_text(serialized, encoding="utf-8")
    os.chmod(temporary, 0o600)
    temporary.replace(path)
    os.chmod(path, 0o600)
    return "protected-file"


def _load_session() -> dict[str, Any]:
    schema = _secret_schema()
    if schema is not None:
        try:
            value = Secret.password_lookup_sync(schema, AUTH_ATTRIBUTES, None)
            if value:
                payload = json.loads(value)
                if isinstance(payload, dict):
                    return payload
        except Exception:
            pass

    path = _session_path()
    if not path.is_file():
        return {}
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}
    return payload if isinstance(payload, dict) else {}


def _clear_session() -> None:
    schema = _secret_schema()
    if schema is not None:
        try:
            Secret.password_clear_sync(schema, AUTH_ATTRIBUTES, None)
        except Exception:
            pass
    path = _session_path()
    if path.exists():
        path.unlink()


def _headers_from_json(raw_text: str) -> dict[str, str]:
    try:
        payload = json.loads(raw_text)
    except Exception:
        return {}
    if not isinstance(payload, dict):
        return {}
    return {
        str(key): str(value)
        for key, value in payload.items()
        if isinstance(value, str)
    }


def _headers_from_curl(raw_text: str) -> dict[str, str]:
    if "curl " not in raw_text and not raw_text.strip().startswith("curl"):
        return {}
    try:
        args = shlex.split(raw_text.replace("\\\n", " "))
    except ValueError:
        return {}

    headers: dict[str, str] = {}
    index = 0
    while index < len(args):
        arg = args[index]
        value = ""
        if arg in ("-H", "--header") and index + 1 < len(args):
            value = args[index + 1]
            index += 2
        elif arg.startswith("-H") and len(arg) > 2:
            value = arg[2:]
            index += 1
        elif arg.startswith("--header="):
            value = arg.split("=", 1)[1]
            index += 1
        elif arg in ("-b", "--cookie") and index + 1 < len(args):
            headers["cookie"] = args[index + 1]
            index += 2
            continue
        elif arg.startswith("--cookie="):
            headers["cookie"] = arg.split("=", 1)[1]
            index += 1
            continue
        else:
            index += 1
        _add_header(headers, value)
    return headers


def _headers_from_lines(raw_text: str) -> dict[str, str]:
    headers: dict[str, str] = {}
    remembered_key = ""
    for raw_line in raw_text.splitlines():
        line = raw_line.strip()
        if not line or line.startswith(":"):
            continue
        if line.startswith(("fetch(", "headers:", "{", "}", "});", "),")):
            continue
        if ":" in line and not line.startswith(":"):
            _add_header(headers, line)
            remembered_key = ""
            continue
        if line.endswith(":"):
            remembered_key = line[:-1].strip().strip("'\"").lower()
            continue
        if remembered_key:
            headers[remembered_key] = line.rstrip(",").strip("'\"")
            remembered_key = ""
    return headers


def _headers_from_cookie(raw_text: str) -> dict[str, str]:
    text = raw_text.strip()
    if "\n" in text or "=" not in text:
        return {}
    return {"cookie": text.removeprefix("Cookie:").removeprefix("cookie:").strip()}


def _add_header(headers: dict[str, str], raw_header: str) -> None:
    header = (raw_header or "").strip()
    if not header:
        return
    if header.startswith("$"):
        header = header[1:].strip()
    header = header.strip("'\"")
    if header.startswith(":") or ":" not in header:
        return
    key, value = header.split(":", 1)
    key = key.strip().strip("'\"").lower()
    value = value.strip().rstrip(",").strip("'\"")
    if key:
        headers[key] = value


def _cookie_value(cookie: str, names: tuple[str, ...]) -> str:
    wanted = {name.lower() for name in names}
    for part in cookie.split(";"):
        if "=" not in part:
            continue
        key, value = part.split("=", 1)
        if key.strip().lower() in wanted:
            return value.strip()
    return ""


def _ensure_authorization(headers: dict[str, str]) -> None:
    cookie = headers.get("cookie") or headers.get("Cookie") or ""
    if not cookie:
        return
    sapisid = _cookie_value(
        cookie,
        ("__Secure-3PAPISID", "SAPISID", "__Secure-1PAPISID", "APISID"),
    )
    if not sapisid:
        return
    origin = headers.get("origin") or headers.get("x-origin") or "https://music.youtube.com"
    timestamp = str(int(time.time()))
    digest = hashlib.sha1(f"{timestamp} {sapisid} {origin}".encode()).hexdigest()
    headers["authorization"] = f"SAPISIDHASH {timestamp}_{digest}"


def _minimal_auth_headers(headers: dict[str, str]) -> dict[str, str]:
    cookie = headers.get("cookie", "")
    if not _cookie_value(
        cookie,
        ("__Secure-3PAPISID", "SAPISID", "__Secure-1PAPISID", "APISID"),
    ):
        raise RuntimeError(
            "The imported session does not contain a SAPISID-family YouTube cookie."
        )

    allowed = (
        "cookie",
        "x-goog-authuser",
        "user-agent",
        "origin",
        "referer",
        "x-origin",
        "accept",
        "accept-language",
        "content-type",
        "x-goog-visitor-id",
        "x-youtube-client-name",
        "x-youtube-client-version",
    )
    minimal = {key: headers[key] for key in allowed if headers.get(key)}
    _ensure_authorization(minimal)
    return minimal


def _parse_browser_headers(raw_text: str) -> dict[str, str]:
    raw_text = (raw_text or "").strip()
    if not raw_text:
        raise RuntimeError("The clipboard is empty")

    headers = _headers_from_json(raw_text)
    if not headers:
        headers = _headers_from_curl(raw_text)
    if not headers and ytmusicapi_setup is not None:
        try:
            parsed = ytmusicapi_setup(filepath=None, headers_raw=raw_text)
            if isinstance(parsed, dict):
                headers = parsed
            elif isinstance(parsed, str) and parsed.strip():
                headers = json.loads(parsed)
        except Exception:
            headers = {}
    if not headers:
        headers = _headers_from_lines(raw_text)
    if not headers:
        headers = _headers_from_cookie(raw_text)

    normalized = {
        str(key).strip().lower(): str(value).strip()
        for key, value in headers.items()
        if str(key).strip() and str(value).strip()
    }
    if not normalized.get("cookie"):
        raise RuntimeError(
            "Invalid cookies. Copy a Cookie header or a full music.youtube.com request as cURL."
        )

    normalized.setdefault("x-goog-authuser", "0")
    normalized.setdefault("user-agent", DEFAULT_USER_AGENT)
    normalized.setdefault("origin", "https://music.youtube.com")
    normalized.setdefault("referer", "https://music.youtube.com/")
    normalized.setdefault("x-origin", "https://music.youtube.com")
    normalized.setdefault("accept", "*/*")
    normalized.setdefault("accept-language", _accept_language())
    return _minimal_auth_headers(normalized)


def _require_dependencies() -> None:
    if YTMusic is None:
        raise RuntimeError(f"ytmusicapi is not installed: {IMPORT_ERROR}")


def _session(timeout: float = 20.0):
    if requests is None:
        raise RuntimeError(f"requests is not installed: {IMPORT_ERROR}")
    session = requests.Session()
    original = session.request

    def request(method, url, **kwargs):
        kwargs.setdefault("timeout", timeout)
        return original(method, url, **kwargs)

    session.request = request
    return session


def _system_locale() -> str:
    for key in ("LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"):
        value = os.environ.get(key, "").strip()
        if value:
            return value
    try:
        language, _encoding = locale.getlocale()
    except Exception:
        language = None
    return language or ""


def _is_neutral_locale(value: str) -> bool:
    locale_name = re.split(r"[.@]", value.strip(), maxsplit=1)[0].lower()
    return locale_name in {"c", "posix"}


def _locale_candidates() -> list[str]:
    candidates: list[str] = []
    for key in ("LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"):
        for candidate in re.split(r"[:;,]", os.environ.get(key, "")):
            normalized = candidate.strip().replace("-", "_")
            if normalized and not _is_neutral_locale(normalized):
                candidates.append(normalized)

    if candidates:
        return candidates

    detected = _system_locale().strip().replace("-", "_")
    if detected and not _is_neutral_locale(detected):
        return [detected]
    return []


def _language() -> str:
    for candidate in _locale_candidates():
        language = candidate.split("_", 1)[0].split(".", 1)[0].lower()
        if language in {"pt", "es", "en"}:
            return language
    return "en"


def _location() -> str:
    for candidate in _locale_candidates():
        locale_name = re.split(r"[.@]", candidate, maxsplit=1)[0]
        parts = locale_name.split("_")
        if len(parts) >= 2 and len(parts[1]) == 2 and parts[1].isalpha():
            return parts[1].upper()
    return ""


def _accept_language(language: str = "", location: str = "") -> str:
    language = language or _language()
    location = location or _location()
    primary = f"{language}-{location}" if location else language
    if language == "en":
        return f"{primary},en;q=0.9" if primary != "en" else "en-US,en;q=0.9"
    return f"{primary},{language};q=0.9,en;q=0.7"


def _locale_cache_namespace() -> str:
    return f"{_language()}-{_location() or 'global'}"


def _create_client(authenticated: bool = True):
    _require_dependencies()
    session = _session()
    language = _language()
    location = _location()
    if authenticated:
        payload = _load_session()
        headers = payload.get("headers")
        if isinstance(headers, dict) and headers:
            normalized = {
                str(key).strip().lower(): str(value).strip()
                for key, value in headers.items()
                if str(key).strip() and str(value).strip()
            }
            normalized["accept-language"] = _accept_language(language, location)
            _ensure_authorization(normalized)
            return YTMusic(
                normalized,
                requests_session=session,
                language=language,
                location=location,
            )
    return YTMusic(
        requests_session=session,
        language=language,
        location=location,
    )


def _account_name(account_info: Any) -> str:
    if isinstance(account_info, dict):
        for key in ("accountName", "name", "email"):
            value = str(account_info.get(key) or "").strip()
            if value:
                return value
        accounts = account_info.get("accounts") or []
        if accounts and isinstance(accounts[0], dict):
            return str(accounts[0].get("accountName") or accounts[0].get("name") or "").strip()
    return ""


def _upgrade_thumbnail_url(url: str, size: int = 1200) -> str:
    url = (url or "").strip()
    if not url:
        return ""
    parts = urlsplit(url)
    path = parts.path
    upgraded = re.sub(r"=w\d+-h\d+([^/?#]*)$", f"=w{size}-h{size}\\1", path)
    if parts.netloc == "yt3.ggpht.com":
        upgraded = re.sub(r"=s\d+([^/?#]*)$", f"=w{size}-h{size}-p-l90-rj", upgraded)
    else:
        upgraded = re.sub(r"=s\d+([^/?#]*)$", f"=s{size}\\1", upgraded)
    if upgraded == path and "googleusercontent.com" in parts.netloc and "=" not in path.rsplit("/", 1)[-1]:
        upgraded = f"{path}=s{size}"
    return urlunsplit(parts._replace(path=upgraded))


def _thumbnail_candidates(value: Any) -> list[dict[str, Any]]:
    candidates: list[dict[str, Any]] = []
    visited: set[int] = set()

    def walk(node: Any) -> None:
        if isinstance(node, dict):
            identity = id(node)
            if identity in visited:
                return
            visited.add(identity)
            url = str(node.get("url") or "").strip()
            if url:
                candidates.append(node)
            for child in node.values():
                if isinstance(child, (dict, list, tuple)):
                    walk(child)
        elif isinstance(node, (list, tuple)):
            identity = id(node)
            if identity in visited:
                return
            visited.add(identity)
            for child in node:
                walk(child)

    walk(value)
    return candidates


def _thumbnail_area(item: dict[str, Any]) -> int:
    try:
        width = int(item.get("width") or 0)
        height = int(item.get("height") or 0)
    except (TypeError, ValueError):
        return 0
    return max(0, width) * max(0, height)


def _best_thumbnail(thumbnails: Any) -> str:
    candidates = _thumbnail_candidates(thumbnails)
    if not candidates:
        return ""
    candidate = max(candidates, key=_thumbnail_area)
    return _upgrade_thumbnail_url(str(candidate.get("url") or ""))


def _thumbnails(result: dict[str, Any]) -> Any:
    return result


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


def _names(value: Any) -> str:
    if isinstance(value, list):
        names = [_text(item) for item in value]
        return ", ".join(name for name in names if name)
    return _text(value)


def _duration_text_to_seconds(value: Any) -> int:
    text = _text(value)
    if not text:
        return 0
    parts = text.split(":")
    if len(parts) > 1 and all(part.strip().isdigit() for part in parts):
        total = 0
        for part in parts:
            total = total * 60 + int(part)
        return total
    return 0


def _duration(value: Any) -> int:
    try:
        return max(0, int(value or 0))
    except (TypeError, ValueError):
        return 0


def _duration_seconds(result: dict[str, Any]) -> int:
    return (
        _duration(result.get("duration_seconds") or result.get("durationSeconds"))
        or _duration_text_to_seconds(result.get("duration") or result.get("length"))
    )


def _playlist_id(result: dict[str, Any]) -> str:
    for key in ("playlistId", "playlist_id", "audioPlaylistId", "playlist"):
        value = _text(result.get(key))
        if value:
            return value
    result_type = _text(result.get("resultType") or result.get("result_type")).lower()
    if result_type == "playlist":
        value = _text(result.get("id"))
        if value:
            return value
    browse_id = _text(result.get("browseId") or result.get("browse_id"))
    if browse_id.startswith("VL") and len(browse_id) > 2:
        return browse_id[2:]
    if browse_id.startswith(("PL", "RD", "OLAK5uy_")):
        return browse_id
    return ""


def _playlist_kind(title: str, source: str = "") -> str:
    if source == "Library playlist":
        return "library"
    text = f"{title} {source}".lower()
    if any(token in text for token in ("mix", "radio", "supermix")):
        return "mix"
    return "recommended" if source else "library"


def _song_item(result: dict[str, Any], result_type: str = "song") -> dict[str, Any] | None:
    video_id = _text(result.get("videoId") or result.get("video_id"))
    title = _text(result.get("title") or result.get("name"))
    if not video_id or not title:
        return None
    artist = _names(result.get("artists") or result.get("artist"))
    album_data = result.get("album") or {}
    album = _text(album_data) if isinstance(album_data, dict) else _text(album_data)
    duration = _duration_seconds(result)
    thumbnail_url = _best_thumbnail(_thumbnails(result))
    if not thumbnail_url and VIDEO_ID_PATTERN.fullmatch(video_id):
        thumbnail_url = f"https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"
    subtitle = " • ".join(value for value in (artist, album, _format_duration(duration)) if value)
    return {
        "result_type": result_type,
        "title": title,
        "subtitle": subtitle,
        "video_id": video_id,
        "browse_id": "",
        "album": album,
        "artist": artist,
        "playlist_kind": "",
        "params": "",
        "duration_seconds": duration,
        "thumbnail_url": thumbnail_url,
    }


def _playlist_item(result: dict[str, Any], source: str = "") -> dict[str, Any] | None:
    title = _text(result.get("title") or result.get("name"))
    browse_id = _playlist_id(result)
    if not title or not browse_id:
        return None
    count = _text(result.get("count") or result.get("itemCount") or result.get("trackCount"))
    author = _names(result.get("author") or result.get("artists") or result.get("channel"))
    description = _text(result.get("description"))
    kind = source or "Playlist"
    if not count and description:
        match = re.search(r"(\d[\d.,]*)\s+(?:songs|tracks|músicas|faixas|canciones)", description, re.IGNORECASE)
        if match:
            count = match.group(1)
    detail = f"{count} tracks" if count else description or kind
    subtitle = " • ".join(value for value in (author, detail) if value)
    return {
        "result_type": "playlist",
        "title": title,
        "subtitle": subtitle,
        "video_id": _text(result.get("videoId") or result.get("video_id")),
        "browse_id": browse_id,
        "album": "",
        "artist": author,
        "playlist_kind": _playlist_kind(title, source),
        "params": _text(result.get("params")),
        "duration_seconds": 0,
        "thumbnail_url": _best_thumbnail(_thumbnails(result)),
    }


def _album_item(result: dict[str, Any], source: str = "") -> dict[str, Any] | None:
    title = _text(result.get("title") or result.get("name"))
    browse_id = _text(result.get("browseId") or result.get("browse_id"))
    if not title or not browse_id:
        return None
    artist = _names(result.get("artists") or result.get("artist"))
    year = _text(result.get("year"))
    subtitle = " • ".join(value for value in (artist, year, source) if value)
    return {
        "result_type": "album",
        "title": title,
        "subtitle": subtitle,
        "video_id": "",
        "browse_id": browse_id,
        "album": title,
        "artist": artist,
        "playlist_kind": "",
        "params": _text(result.get("params")),
        "duration_seconds": 0,
        "thumbnail_url": _best_thumbnail(_thumbnails(result)),
    }


def _artist_item(result: dict[str, Any], source: str = "") -> dict[str, Any] | None:
    title = _text(result.get("artist") or result.get("title") or result.get("name"))
    browse_id = _text(result.get("browseId") or result.get("browse_id") or result.get("channelId"))
    if not title or not browse_id:
        return None
    subtitle = source or "Artist"
    return {
        "result_type": "artist",
        "title": title,
        "subtitle": subtitle,
        "video_id": "",
        "browse_id": browse_id,
        "album": "",
        "artist": title,
        "playlist_kind": "",
        "params": _text(result.get("params")),
        "duration_seconds": 0,
        "thumbnail_url": _best_thumbnail(_thumbnails(result)),
    }


def _song_collection_items(
    results: Any,
    source: str,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Derive album and artist collection rows from song metadata.

    YouTube Music often returns a populated song library while the dedicated
    library-albums/library-artists endpoints are empty. Title-only rows remain
    useful because the native collection loader can resolve them through search.
    """

    albums: list[dict[str, Any]] = []
    artists: list[dict[str, Any]] = []

    for result in results or []:
        if not isinstance(result, dict):
            continue

        album_data = result.get("album") or {}
        if isinstance(album_data, dict):
            album_title = _text(
                album_data.get("name")
                or album_data.get("title")
                or album_data.get("text")
            )
            album_id = _text(
                album_data.get("id")
                or album_data.get("browseId")
                or album_data.get("browse_id")
            )
        else:
            album_title = _text(album_data)
            album_id = ""

        song_artists = _names(result.get("artists") or result.get("artist"))
        thumbnail_url = _best_thumbnail(_thumbnails(result))
        if album_title:
            albums.append(
                {
                    "result_type": "album",
                    "title": album_title,
                    "subtitle": " • ".join(
                        value for value in (song_artists, source) if value
                    ),
                    "video_id": "",
                    "browse_id": album_id,
                    "album": album_title,
                    "artist": song_artists,
                    "playlist_kind": "",
                    "params": "",
                    "duration_seconds": 0,
                    "thumbnail_url": thumbnail_url,
                }
            )

        artist_values = result.get("artists") or result.get("artist") or []
        if not isinstance(artist_values, list):
            artist_values = [artist_values]
        for artist_data in artist_values:
            if isinstance(artist_data, dict):
                artist_title = _text(
                    artist_data.get("name")
                    or artist_data.get("title")
                    or artist_data.get("artist")
                )
                artist_id = _text(
                    artist_data.get("id")
                    or artist_data.get("browseId")
                    or artist_data.get("browse_id")
                    or artist_data.get("channelId")
                )
            else:
                artist_title = _text(artist_data)
                artist_id = ""
            if not artist_title:
                continue
            artists.append(
                {
                    "result_type": "artist",
                    "title": artist_title,
                    "subtitle": source,
                    "video_id": "",
                    "browse_id": artist_id,
                    "album": "",
                    "artist": artist_title,
                    "playlist_kind": "",
                    "params": "",
                    "duration_seconds": 0,
                    "thumbnail_url": thumbnail_url,
                }
            )

    return _dedupe(albums), _dedupe(artists)


def _search_item(result: dict[str, Any]) -> dict[str, Any] | None:
    result_type = str(result.get("resultType") or "").strip().lower()
    if result_type in {"song", "video"}:
        return _song_item(result, result_type)
    if result_type == "playlist":
        return _playlist_item(result, "Playlist")
    if result_type == "album":
        return _album_item(result)
    if result_type == "artist":
        return _artist_item(result)
    return None


def _dedupe(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    seen: set[tuple[str, str, str]] = set()
    output = []
    for item in items:
        key = (item.get("result_type", ""), item.get("video_id") or item.get("browse_id", ""), item.get("title", ""))
        if key in seen:
            continue
        seen.add(key)
        output.append(item)
    return output


def _home_playlist_items(client, limit: int) -> list[dict[str, Any]]:
    return _home_suggestions(client, limit).get("playlists", [])


def _home_suggestions(client, limit: int) -> dict[str, list[dict[str, Any]]]:
    suggestions: dict[str, list[dict[str, Any]]] = {
        "playlists": [],
        "albums": [],
        "artists": [],
    }
    if limit <= 0 or not hasattr(client, "get_home"):
        return suggestions
    try:
        rows = client.get_home(limit=limit)
    except Exception as error:
        print(f"Nocky YouTube home lookup skipped: {error}", file=sys.stderr)
        return suggestions

    for row in rows or []:
        if not isinstance(row, dict):
            continue
        section = _text(row.get("title")) or "Recommended"
        for result in row.get("contents") or []:
            if not isinstance(result, dict):
                continue
            result_type = _text(result.get("resultType")).lower()
            browse_id = _text(result.get("browseId") or result.get("browse_id"))
            if result_type == "album" or browse_id.startswith("MPRE"):
                if album := _album_item(result, section):
                    suggestions["albums"].append(album)
                continue
            if result_type == "artist" or (
                browse_id.startswith("UC")
                and not _text(result.get("playlistId") or result.get("playlist_id"))
                and not _text(result.get("videoId") or result.get("video_id"))
            ):
                if artist := _artist_item(result, section):
                    suggestions["artists"].append(artist)
                continue
            if playlist := _playlist_item(result, section):
                suggestions["playlists"].append(playlist)
    return {key: _dedupe(items) for key, items in suggestions.items()}


def _playlist_tracks_from_watch(
    client,
    playlist_id: str,
    video_id: str,
    limit: int,
    radio: bool,
) -> list[dict[str, Any]]:
    if not hasattr(client, "get_watch_playlist"):
        return []
    attempts: list[dict[str, Any]] = []
    if video_id and playlist_id:
        attempts.append({"videoId": video_id, "playlistId": playlist_id, "limit": limit, "radio": radio})
    if playlist_id:
        attempts.append({"playlistId": playlist_id, "limit": limit, "radio": radio})
    if video_id:
        attempts.append({"videoId": video_id, "limit": limit, "radio": radio})

    last_error: Exception | None = None
    for kwargs in attempts:
        try:
            data = client.get_watch_playlist(**kwargs)
        except Exception as error:
            last_error = error
            continue
        tracks = data.get("tracks") if isinstance(data, dict) else []
        items = [
            item
            for result in tracks or []
            if isinstance(result, dict)
            if (item := _song_item(result))
        ]
        if items:
            return items

    if last_error is not None:
        print(f"Nocky YouTube watch playlist fallback failed for {playlist_id or video_id}: {last_error}", file=sys.stderr)
    return []


def _format_duration(seconds: int) -> str:
    if seconds <= 0:
        return ""
    minutes, second = divmod(seconds, 60)
    hours, minutes = divmod(minutes, 60)
    return f"{hours:d}:{minutes:02d}:{second:02d}" if hours else f"{minutes:d}:{second:02d}"


def _extract_video_id(value: str) -> str:
    value = value.strip()
    if VIDEO_ID_PATTERN.fullmatch(value):
        return value
    parsed = urlparse(value)
    hostname = (parsed.hostname or "").lower()
    if hostname in {"youtu.be", "www.youtu.be"}:
        candidate = parsed.path.strip("/").split("/")[0]
        if VIDEO_ID_PATTERN.fullmatch(candidate):
            return candidate
    if hostname in {"youtube.com", "www.youtube.com", "music.youtube.com", "m.youtube.com"}:
        if parsed.path == "/watch":
            candidate = parse_qs(parsed.query).get("v", [""])[0]
            if VIDEO_ID_PATTERN.fullmatch(candidate):
                return candidate
        for prefix in ("/shorts/", "/embed/", "/live/"):
            if parsed.path.startswith(prefix):
                candidate = parsed.path[len(prefix):].split("/")[0]
                if VIDEO_ID_PATTERN.fullmatch(candidate):
                    return candidate
    raise RuntimeError("Invalid YouTube video ID or URL")


def _load_stream_cache() -> dict[str, Any]:
    path = _stream_cache_path()
    if not path.is_file():
        return {}
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}
    return payload.get("streams", {}) if isinstance(payload, dict) else {}


def _save_stream_cache(cache: dict[str, Any]) -> None:
    path = _stream_cache_path()
    lock_path = _stream_cache_lock_path()
    path.parent.mkdir(parents=True, exist_ok=True)

    with lock_path.open("a+", encoding="utf-8") as lock_file:
        fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)
        merged = _load_stream_cache()
        merged.update(cache)
        now = time.time()
        valid_items = [
            (key, value)
            for key, value in merged.items()
            if isinstance(value, dict)
            and float(value.get("expires_at") or 0) > now + STREAM_CACHE_SAFETY
            and value.get("stream_url")
        ]
        valid_items.sort(
            key=lambda item: float(item[1].get("expires_at") or 0),
            reverse=True,
        )
        valid = dict(valid_items[:STREAM_CACHE_LIMIT])
        temporary = path.with_name(f"{path.name}.{os.getpid()}.tmp")
        temporary.write_text(
            json.dumps({"streams": valid}, indent=2),
            encoding="utf-8",
        )
        temporary.replace(path)
        fcntl.flock(lock_file.fileno(), fcntl.LOCK_UN)


def _yt_dlp_command() -> list[str]:
    configured = os.environ.get("NOCKY_YTDLP", "").strip()
    if configured and Path(configured).is_file():
        return [configured]
    binary = shutil.which("yt-dlp")
    if binary:
        return [binary]
    sibling = Path(sys.executable).with_name("yt-dlp")
    if sibling.is_file():
        return [str(sibling)]
    try:
        import yt_dlp  # noqa: F401
    except Exception as error:
        raise RuntimeError(f"yt-dlp is not installed: {error}") from error
    return [sys.executable, "-m", "yt_dlp"]


def _deno_path() -> str:
    configured = os.environ.get("NOCKY_DENO", "").strip()
    if configured and Path(configured).is_file():
        return configured
    binary = shutil.which("deno")
    if binary:
        return binary
    sibling = Path(sys.executable).with_name("deno")
    return str(sibling) if sibling.is_file() else ""


def _write_yt_dlp_cookie_file(cookie_header: str) -> Path | None:
    pairs: list[tuple[str, str]] = []
    for raw_part in cookie_header.split(";"):
        part = raw_part.strip()
        if not part or "=" not in part:
            continue
        name, value = part.split("=", 1)
        name = name.strip()
        value = value.strip()
        if not name or not value:
            continue
        pairs.append((name, value))

    if not pairs:
        return None

    descriptor, raw_path = tempfile.mkstemp(
        prefix="nocky-youtube-",
        suffix=".cookies.txt",
    )
    os.close(descriptor)
    path = Path(raw_path)
    os.chmod(path, 0o600)

    lines = ["# Netscape HTTP Cookie File", ""]
    for name, value in pairs:
        lines.append(
            "\t".join(
                (
                    ".youtube.com",
                    "TRUE",
                    "/",
                    "TRUE",
                    "0",
                    name,
                    value,
                )
            )
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


def _yt_dlp_auth_args() -> tuple[list[str], Path | None]:
    payload = _load_session()
    stored_headers = payload.get("headers")
    if not isinstance(stored_headers, dict) or not stored_headers:
        return [], None

    headers = {
        str(key).strip().lower(): str(value).strip()
        for key, value in stored_headers.items()
        if str(key).strip() and str(value).strip()
    }
    headers["accept-language"] = _accept_language()

    args: list[str] = []
    cookie_file = _write_yt_dlp_cookie_file(headers.get("cookie", ""))
    if cookie_file is not None:
        args += ["--cookies", str(cookie_file)]

    forwarded_headers = (
        ("user-agent", "User-Agent"),
        ("referer", "Referer"),
        ("origin", "Origin"),
        ("accept-language", "Accept-Language"),
        ("x-goog-authuser", "X-Goog-AuthUser"),
    )
    for key, display_name in forwarded_headers:
        value = headers.get(key, "")
        if value:
            args += ["--add-header", f"{display_name}: {value}"]

    return args, cookie_file


def _premium_authentication_error(message: str) -> bool:
    normalized = message.lower()
    return (
        "only available to music premium members" in normalized
        or "music premium members" in normalized
        or "premium members" in normalized
    )


def _session_authentication_error(message: str) -> bool:
    normalized = message.lower()
    return (
        "sign in to confirm you're not a bot" in normalized
        or "sign in to confirm you’re not a bot" in normalized
        or "login required" in normalized
        or "authentication required" in normalized
        or "use --cookies-from-browser or --cookies" in normalized
    )


def _select_audio_stream(payload: dict[str, Any]) -> tuple[dict[str, Any], str]:
    selected_format = payload
    stream_url = str(payload.get("url") or "").strip()
    if stream_url:
        return selected_format, stream_url

    formats = [
        item
        for item in (payload.get("formats") or [])
        if isinstance(item, dict)
        and str(item.get("url") or "").strip()
        and str(item.get("acodec") or "none") != "none"
    ]
    audio_only = [
        item for item in formats if str(item.get("vcodec") or "none") == "none"
    ]
    candidates = audio_only or formats
    if not candidates:
        return selected_format, ""

    selected_format = max(
        candidates,
        key=lambda item: (
            float(item.get("abr") or 0),
            float(item.get("tbr") or 0),
            int(item.get("filesize") or item.get("filesize_approx") or 0),
        ),
    )
    return selected_format, str(selected_format.get("url") or "").strip()


def _stream_resolution_error(
    errors: list[tuple[str, str]],
    *,
    has_auth: bool,
) -> RuntimeError:
    details = [detail for _client, detail in errors]
    categories = [error_category(detail) for detail in details]
    terminal = next(
        (detail for detail, category in zip(details, categories) if category == "terminal"),
        "",
    )
    if terminal:
        return RuntimeError(terminal)

    if not has_auth and any(category == "authentication" for category in categories):
        return RuntimeError(
            "This track requires a connected YouTube Music browser session. "
            "Connect the account in Nocky and try again."
        )

    if has_auth and details and all(
        "requested format is not available" in detail.lower()
        or "no playable audio stream" in detail.lower()
        for detail in details
    ):
        return RuntimeError(
            "__NOCKY_PREMIUM_STREAM_UNAVAILABLE__"
            "The configured YouTube clients did not expose a compatible audio stream."
        )

    if has_auth and any(_session_authentication_error(detail) for detail in details):
        return RuntimeError(
            "YouTube rejected the saved browser session during stream extraction. "
            "Reconnect the account in Nocky with a fresh music.youtube.com request."
        )

    summary = " | ".join(f"{client}: {detail}" for client, detail in errors)
    return RuntimeError(f"All configured YouTube stream clients failed: {summary}")


def _resolve_stream(video_or_url: str, force: bool = False) -> dict[str, Any]:
    video_id = _extract_video_id(video_or_url)
    cache = _load_stream_cache()
    cached = cache.get(video_id)
    failed_client = ""
    if force and isinstance(cached, dict):
        failed_client = str(cached.get("stream_client") or "").strip()
        # Expire a URL already rejected by the CDN before trying another client.
        cache[video_id] = {"expires_at": 0}
        _save_stream_cache(cache)
        cache.pop(video_id, None)
        cached = None
    if (
        not force
        and isinstance(cached, dict)
        and cached.get("stream_url")
        and float(cached.get("expires_at") or 0) > time.time() + STREAM_CACHE_SAFETY
    ):
        return cached

    command = _yt_dlp_command()
    deno = _deno_path()
    if not deno:
        raise RuntimeError("Deno is not installed. Run the Nocky installer with --install-youtube.")

    webpage_url = f"https://www.youtube.com/watch?v={video_id}"
    command += [
        "--no-config",
        "--no-playlist",
        "--skip-download",
        "--dump-single-json",
        "--check-formats",
        "--format",
        "bestaudio[protocol^=http]/bestaudio/best",
        "--retries",
        "3",
        "--extractor-retries",
        "3",
        "--fragment-retries",
        "3",
        "--socket-timeout",
        "20",
        "--no-warnings",
        "--no-progress",
        "--no-update",
        "--js-runtimes",
        f"deno:{deno}",
        webpage_url,
    ]

    auth_args, cookie_file = _yt_dlp_auth_args()
    attempts = ordered_profiles(
        has_auth=bool(auth_args),
        failed_client=failed_client,
    )
    if not attempts:
        raise RuntimeError("No compatible YouTube stream client is enabled")

    attempted_clients: list[str] = []
    errors: list[tuple[str, str]] = []
    selected_profile = None
    selected_format: dict[str, Any] = {}
    payload: dict[str, Any] = {}
    stream_url = ""

    try:
        for profile in attempts:
            attempted_clients.append(profile.key)
            attempt_command = build_attempt_command(
                command,
                webpage_url,
                profile,
                auth_args,
            )
            try:
                process = subprocess.run(
                    attempt_command,
                    capture_output=True,
                    text=True,
                    timeout=60,
                    check=False,
                )
            except subprocess.TimeoutExpired:
                detail = "The YouTube stream resolver timed out"
                errors.append((profile.key, detail))
                continue

            if process.returncode != 0:
                detail = concise_process_error(process.stderr, process.stdout)
                errors.append((profile.key, detail))
                if not should_try_next_client(detail):
                    break
                continue

            try:
                payload = json.loads(process.stdout)
            except json.JSONDecodeError as error:
                detail = f"yt-dlp returned invalid JSON: {error}"
                errors.append((profile.key, detail))
                continue

            selected_format, stream_url = _select_audio_stream(payload)
            if not stream_url:
                detail = "yt-dlp returned no playable audio stream URL"
                errors.append((profile.key, detail))
                continue

            selected_profile = profile
            break
    finally:
        if cookie_file is not None:
            try:
                cookie_file.unlink(missing_ok=True)
            except OSError:
                pass

    if selected_profile is None or not stream_url:
        raise _stream_resolution_error(errors, has_auth=bool(auth_args))

    headers = {
        str(key): str(value)
        for key, value in (
            selected_format.get("http_headers")
            or payload.get("http_headers")
            or {}
        ).items()
        if value is not None
    }
    thumbnail = str(payload.get("thumbnail") or "")
    thumbnails = payload.get("thumbnails") or []
    if thumbnails:
        thumbnail = _best_thumbnail(thumbnails) or thumbnail

    result = {
        "video_id": video_id,
        "stream_url": stream_url,
        "webpage_url": str(payload.get("webpage_url") or webpage_url),
        "title": str(payload.get("track") or payload.get("title") or f"YouTube video {video_id}").strip(),
        "artist": str(payload.get("artist") or payload.get("uploader") or payload.get("channel") or "YouTube").strip(),
        "album": str(payload.get("album") or "YouTube Music").strip(),
        "duration_seconds": _duration(payload.get("duration")),
        "thumbnail_url": _upgrade_thumbnail_url(thumbnail),
        "http_headers": headers,
        "format_id": str(selected_format.get("format_id") or ""),
        "protocol": str(selected_format.get("protocol") or ""),
        "container": str(selected_format.get("ext") or ""),
        "audio_codec": str(selected_format.get("acodec") or ""),
        "stream_client": selected_profile.key,
        "stream_client_label": selected_profile.label,
        "attempted_clients": attempted_clients,
        "fallback_used": len(attempted_clients) > 1,
        "expires_at": time.time() + STREAM_CACHE_TTL,
    }
    cache[video_id] = result
    _save_stream_cache(cache)
    return result


def command_stream_clients(_payload: dict[str, Any]) -> dict[str, object]:
    session = _load_session()
    return policy_snapshot(has_auth=bool(session.get("headers")))


def command_status(_payload: dict[str, Any]) -> dict[str, Any]:
    session = _load_session()
    return {
        "connected": bool(session.get("headers")),
        "account": str(session.get("account") or ""),
        "storage": str(session.get("storage") or ""),
    }


def command_connect(payload: dict[str, Any]) -> dict[str, Any]:
    _require_dependencies()
    raw = str(payload.get("raw") or "")
    headers = _parse_browser_headers(raw)
    client = YTMusic(headers, requests_session=_session())
    account_info = client.get_account_info()
    account = _account_name(account_info) or "Connected account"
    session_payload = {"headers": headers, "account": account}
    storage = _store_session(session_payload)
    # Save the backend name for diagnostics without exposing headers.
    session_payload["storage"] = storage
    _store_session(session_payload)
    return {"connected": True, "account": account, "storage": storage}


def command_disconnect(_payload: dict[str, Any]) -> dict[str, Any]:
    _clear_session()
    return {"connected": False, "account": ""}


def command_search(payload: dict[str, Any]) -> list[dict[str, Any]]:
    query = str(payload.get("query") or "").strip()
    if not query:
        return []
    filter_name = str(payload.get("filter") or "songs").strip().lower()
    filters = {"all": None, "songs": "songs", "videos": "videos", "albums": "albums", "artists": "artists", "playlists": "playlists"}
    client = _create_client(authenticated=True)
    results = client.search(query, filter=filters.get(filter_name), limit=max(1, min(50, int(payload.get("limit") or 25))))
    return _dedupe([item for result in results if isinstance(result, dict) if (item := _search_item(result))])


def command_library(payload: dict[str, Any]) -> list[dict[str, Any]]:
    client = _create_client(authenticated=True)
    if not _load_session().get("headers"):
        raise RuntimeError("Connect a YouTube Music browser session first")
    limit = max(1, min(500, int(payload.get("limit") or 100)))
    try:
        results = client.get_library_songs(limit=limit, order="recently_added")
    except TypeError:
        results = client.get_library_songs(limit=limit)
    return _dedupe([item for result in (results or []) if isinstance(result, dict) if (item := _song_item(result))])


def command_liked(payload: dict[str, Any]) -> list[dict[str, Any]]:
    client = _create_client(authenticated=True)
    if not _load_session().get("headers"):
        raise RuntimeError("Connect a YouTube Music browser session first")
    data = client.get_liked_songs(limit=max(1, min(500, int(payload.get("limit") or 100))))
    results = data.get("tracks") or [] if isinstance(data, dict) else data or []
    return _dedupe([item for result in results if isinstance(result, dict) if (item := _song_item(result))])


def command_rate(payload: dict[str, Any]) -> bool:
    client = _create_client(authenticated=True)
    if not _load_session().get("headers"):
        raise RuntimeError("Connect a YouTube Music browser session first")

    video_id = _extract_video_id(str(payload.get("video_id") or ""))
    liked = bool(payload.get("liked"))
    client.rate_song(video_id, "LIKE" if liked else "INDIFFERENT")
    return liked


def command_playlists(payload: dict[str, Any]) -> list[dict[str, Any]]:
    client = _create_client(authenticated=True)
    if not _load_session().get("headers"):
        raise RuntimeError("Connect a YouTube Music browser session first")
    limit = max(1, min(500, int(payload.get("limit") or 100)))
    home_limit = max(0, min(12, int(payload["home_limit"] if "home_limit" in payload else 8)))
    results = client.get_library_playlists(limit=limit)
    library_items = [
        item
        for result in (results or [])
        if isinstance(result, dict)
        if (item := _playlist_item(result, "Library playlist"))
    ]
    return _dedupe(library_items + _home_playlist_items(client, home_limit))


def command_home(payload: dict[str, Any]) -> dict[str, list[dict[str, Any]]]:
    client = _create_client(authenticated=True)
    if not _load_session().get("headers"):
        raise RuntimeError("Connect a YouTube Music browser session first")
    limit = max(0, min(12, int(payload.get("limit") or 8)))
    return _home_suggestions(client, limit)


def _feed_cache_key(
    kind: str,
    continuation: str = "",
    section_limit: int = 0,
    params: str = "",
) -> str:
    params_key = hashlib.sha1(params.encode("utf-8")).hexdigest()[:12] if params else "root"
    return f"{_locale_cache_namespace()}:{kind}:{params_key}:{continuation or '0'}:{section_limit}"


def _library_method(client: Any, name: str, limit: int, **kwargs: Any) -> Any:
    method = getattr(client, name, None)
    if method is None:
        return []
    try:
        return method(limit=limit, **kwargs)
    except TypeError:
        try:
            return method(limit=limit)
        except TypeError:
            return method()
    except Exception as error:
        print(f"Nocky YouTube library section {name} skipped: {error}", file=sys.stderr)
        return []


def _inner_tube_home_response(client: Any, params: str = "") -> tuple[dict[str, Any], dict[str, Any]]:
    sender = getattr(client, "_send_request", None)
    if not callable(sender):
        raise RuntimeError("The installed ytmusicapi version does not expose the Web Home request")
    body: dict[str, Any] = {"browseId": "FEmusic_home"}
    if params:
        body["params"] = params
    response = sender("browse", body)
    if not isinstance(response, dict):
        raise RuntimeError("YouTube Music returned an invalid Home response")
    return body, response


def _inner_tube_home_rows(
    client: Any,
    params: str,
    limit: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    body, response = _inner_tube_home_response(client, params)
    debug_pages: list[dict[str, Any]] = [{"kind": "root", "response": response}]
    section_list = find_inner_tube_home_section_list(response)
    contents = section_list.get("contents") if isinstance(section_list.get("contents"), list) else []
    if not contents:
        raise RuntimeError("YouTube Music did not return Home sections")

    # Raw renderers are the primary source. This preserves artwork, endpoint
    # identity and item ordering before ytmusicapi's mixed-content parser can
    # simplify or discard renderer-specific fields.
    rows = parse_inner_tube_home_sections(contents)
    if not rows and ytmusic_parse_mixed_content is not None:
        print(
            "Nocky YouTube Home direct parser returned no rows; using metadata fallback",
            file=sys.stderr,
        )
        rows = list(ytmusic_parse_mixed_content(contents) or [])

    sender = getattr(client, "_send_request")
    current = section_list
    seen_continuations: set[str] = set()
    while (
        len(rows) < limit
        and current.get("continuations")
        and ytmusic_get_continuation_params is not None
    ):
        additional_params = ytmusic_get_continuation_params(current)
        if not additional_params or additional_params in seen_continuations:
            break
        seen_continuations.add(additional_params)
        continuation_response = sender("browse", body, additional_params)
        if isinstance(continuation_response, dict):
            debug_pages.append({"kind": "continuation", "response": continuation_response})
        continuation_contents = continuation_response.get("continuationContents")
        if not isinstance(continuation_contents, dict):
            break
        current = continuation_contents.get("sectionListContinuation")
        if not isinstance(current, dict):
            break
        raw_contents = current.get("contents") if isinstance(current.get("contents"), list) else []
        if not raw_contents:
            break
        parsed = parse_inner_tube_home_sections(raw_contents)
        if not parsed and ytmusic_parse_mixed_content is not None:
            parsed = list(ytmusic_parse_mixed_content(raw_contents) or [])
        if not parsed:
            break
        rows.extend(parsed)

    missing = missing_artwork_by_section(rows)
    if missing:
        summary = ", ".join(
            f"{title}: {count}/{total}"
            for title, count, total in missing[:12]
        )
        print(
            f"Nocky YouTube raw Home items still missing artwork: {summary}",
            file=sys.stderr,
        )

    debug_destination = str(os.environ.get("NOCKY_HOME_DEBUG_DUMP") or "").strip()
    if debug_destination:
        try:
            debug_path = write_home_debug_dump(
                debug_destination,
                pages=debug_pages,
                rows=rows,
                selected_params=params,
            )
            print(f"Nocky YouTube Home renderer diagnostics: {debug_path}", file=sys.stderr)
        except Exception as debug_error:
            print(
                f"Nocky YouTube Home renderer diagnostics failed: {debug_error}",
                file=sys.stderr,
            )
    return rows, response


def _cached_root_home_chips(section_limit: int) -> list[dict[str, str]]:
    root = load_cached_page(
        _home_feed_cache_path(),
        _feed_cache_key("home", "", section_limit, ""),
        allow_stale=True,
    )
    return list((root or {}).get("chips") or [])


def command_home_v2(payload: dict[str, Any]) -> dict[str, Any]:
    if not _load_session().get("headers"):
        raise RuntimeError("Connect a YouTube Music browser session first")
    continuation = str(payload.get("continuation") or "").strip()
    params = _text(payload.get("params"))
    try:
        offset = max(0, int(continuation or 0))
    except ValueError as error:
        raise RuntimeError("Invalid YouTube Music feed continuation") from error
    section_limit = max(1, min(12, int(payload.get("section_limit") or 6)))
    include_native_v3_source = bool(payload.get("include_native_v3_source"))
    cache_key = _feed_cache_key("home", continuation, section_limit, params)
    client = _create_client(authenticated=True)

    try:
        fetch_limit = max(12, min(36, offset + section_limit + 1))
        chips = _cached_root_home_chips(section_limit)
        rows, raw_response = _inner_tube_home_rows(client, params, fetch_limit)
        if offset == 0 or not chips:
            chips = extract_inner_tube_home_chips(raw_response) or chips
        if params and not chips:
            try:
                _body, root_response = _inner_tube_home_response(client)
                chips = extract_inner_tube_home_chips(root_response)
            except Exception as chip_error:
                print(
                    f"Nocky YouTube root chips unavailable: {chip_error}",
                    file=sys.stderr,
                )
        page = build_structured_home(
            rows,
            offset=offset,
            section_limit=section_limit,
            selected_chip_params=params,
        )
        if chips:
            page["chips"] = chips
        if include_native_v3_source:
            try:
                page["native_v3_source"] = build_home_v3_source(
                    raw_response,
                    selected_chip_params=params,
                    section_limit=section_limit,
                )
            except Exception as native_v3_error:
                print(
                    f"Nocky YouTube native Home V3 source unavailable: {native_v3_error}",
                    file=sys.stderr,
                )
        save_cached_page(_home_feed_cache_path(), cache_key, page)
        return page
    except Exception as error:
        cached = load_cached_page(
            _home_feed_cache_path(),
            cache_key,
            allow_stale=True,
        )
        if cached is not None:
            print(f"Nocky YouTube home v2 using cached data: {error}", file=sys.stderr)
            return cached
        raise


def _account_library_page(
    client: Any,
    limit: int,
    mode: str,
) -> dict[str, Any]:
    if mode not in {"overview", "library", "liked"}:
        raise RuntimeError("Invalid YouTube Music account-page mode")

    songs_data: Any = []
    liked_results: Any = []
    playlists_data: Any = []
    albums_data: Any = []
    artists_data: Any = []

    if mode in {"overview", "library"}:
        songs_data = _library_method(
            client,
            "get_library_songs",
            limit,
            order="recently_added",
        )
        playlists_data = _library_method(client, "get_library_playlists", limit)
        albums_data = _library_method(client, "get_library_albums", limit)
        artists_data = _library_method(client, "get_library_artists", limit)

    if mode in {"overview", "liked"}:
        liked_data = _library_method(client, "get_liked_songs", limit)
        liked_results = (
            liked_data.get("tracks") or []
            if isinstance(liked_data, dict)
            else liked_data or []
        )

    songs = _dedupe(
        [
            item
            for result in songs_data or []
            if isinstance(result, dict)
            if (item := _song_item(result))
        ]
    )
    liked = _dedupe(
        [
            item
            for result in liked_results or []
            if isinstance(result, dict)
            if (item := _song_item(result))
        ]
    )
    playlists = _dedupe(
        [
            item
            for result in playlists_data or []
            if isinstance(result, dict)
            if (item := _playlist_item(result, "Library playlist"))
        ]
    )
    explicit_albums = _dedupe(
        [
            item
            for result in albums_data or []
            if isinstance(result, dict)
            if (item := _album_item(result, "Álbum salvo"))
        ]
    )
    explicit_artists = _dedupe(
        [
            item
            for result in artists_data or []
            if isinstance(result, dict)
            if (item := _artist_item(result, "Artista salvo"))
        ]
    )

    library_albums, library_artists = _song_collection_items(
        songs_data,
        "Na biblioteca",
    )
    liked_albums, liked_artists = _song_collection_items(
        liked_results,
        "Nas curtidas",
    )

    if mode == "overview":
        albums = _dedupe(explicit_albums + library_albums + liked_albums)
        artists = _dedupe(explicit_artists + library_artists + liked_artists)
        sections = [
            ("Suas playlists", "carousel", playlists[:24]),
            ("Álbuns", "carousel", albums[:36]),
            ("Artistas", "carousel", artists[:36]),
            ("Adicionadas recentemente", "list", songs[:60]),
            ("Músicas curtidas", "list", liked[:60]),
        ]
    elif mode == "library":
        albums = _dedupe(explicit_albums + library_albums)
        artists = _dedupe(explicit_artists + library_artists)
        sections = [
            ("Playlists", "carousel", playlists[:36]),
            ("Álbuns", "carousel", albums[:36]),
            ("Artistas", "carousel", artists[:36]),
            ("Músicas da biblioteca", "list", songs[:120]),
        ]
    else:
        sections = [
            ("Álbuns das curtidas", "carousel", liked_albums[:36]),
            ("Artistas das curtidas", "carousel", liked_artists[:36]),
            ("Músicas curtidas", "list", liked[:120]),
        ]

    return build_library_overview(sections)


def _cached_account_page(
    payload: dict[str, Any],
    mode: str,
) -> dict[str, Any]:
    if not _load_session().get("headers"):
        raise RuntimeError("Connect a YouTube Music browser session first")

    limit = max(12, min(250, int(payload.get("limit") or 120)))
    # v4 keys bypass older account pages whose long song lists appeared
    # before collection carousels.
    cache_key = _feed_cache_key(f"{mode}_v4", "", limit)
    client = _create_client(authenticated=True)

    try:
        page = _account_library_page(client, limit, mode)
        save_cached_page(_home_feed_cache_path(), cache_key, page)
        return page
    except Exception as error:
        cached = load_cached_page(
            _home_feed_cache_path(),
            cache_key,
            allow_stale=True,
        )
        if cached is not None:
            print(
                f"Nocky YouTube {mode} page using cached data: {error}",
                file=sys.stderr,
            )
            return cached
        raise


def command_library_v2(payload: dict[str, Any]) -> dict[str, Any]:
    return _cached_account_page(payload, "overview")


def command_library_page_v2(payload: dict[str, Any]) -> dict[str, Any]:
    return _cached_account_page(payload, "library")


def command_liked_v2(payload: dict[str, Any]) -> dict[str, Any]:
    return _cached_account_page(payload, "liked")


def command_playlist(payload: dict[str, Any]) -> list[dict[str, Any]]:
    client = _create_client(authenticated=True)
    browse_id = str(payload.get("browse_id") or "").strip()
    video_id = str(payload.get("video_id") or "").strip()
    playlist_kind = str(payload.get("playlist_kind") or "").strip()
    if not browse_id and not video_id:
        return []
    limit = max(1, min(500, int(payload.get("limit") or 200)))
    tracks: list[dict[str, Any]] = []
    playlist_error = None
    if browse_id and playlist_kind != "mix":
        try:
            data = client.get_playlist(browse_id, limit=limit)
            tracks = [
                item
                for result in (data.get("tracks") or [])
                if isinstance(result, dict)
                if (item := _song_item(result))
            ]
        except Exception as error:
            playlist_error = error

    if not tracks:
        tracks = _playlist_tracks_from_watch(client, browse_id, video_id, limit, playlist_kind == "mix")
    if not tracks and playlist_error is not None:
        raise playlist_error
    if not tracks:
        raise RuntimeError("No playable tracks were returned for this YouTube Music playlist")
    return _dedupe(tracks)


def command_collection(payload: dict[str, Any]) -> list[dict[str, Any]]:
    client = _create_client(authenticated=True)
    result_type = str(payload.get("result_type") or "").strip().lower()
    browse_id = str(payload.get("browse_id") or "").strip()
    title = str(payload.get("title") or "").strip()
    limit = max(1, min(200, int(payload.get("limit") or 120)))

    if not browse_id and title:
        filter_name = "artists" if result_type == "artist" else "albums"
        results = client.search(title, filter=filter_name, limit=5)
        candidates = [
            item
            for result in results
            if isinstance(result, dict)
            if (item := _search_item(result))
            if item.get("result_type") == result_type
        ]
        exact = next(
            (
                item
                for item in candidates
                if _text(item.get("title")).casefold() == title.casefold()
            ),
            None,
        )
        selected = exact or (candidates[0] if candidates else None)
        browse_id = _text((selected or {}).get("browse_id"))

    if not browse_id:
        return []

    tracks: list[dict[str, Any]] = []
    if result_type == "album":
        data = client.get_album(browse_id)
        tracks = [
            item
            for result in (data.get("tracks") or [])
            if isinstance(result, dict)
            if (item := _song_item(result))
        ]
    elif result_type == "artist":
        data = client.get_artist(browse_id)
        songs = data.get("songs") or {}
        tracks = [
            item
            for result in (songs.get("results") or [])
            if isinstance(result, dict)
            if (item := _song_item(result))
        ]

        songs_browse_id = _text(songs.get("browseId") or songs.get("browse_id"))
        if songs_browse_id:
            try:
                playlist = client.get_playlist(songs_browse_id, limit=limit)
                expanded = [
                    item
                    for result in (playlist.get("tracks") or [])
                    if isinstance(result, dict)
                    if (item := _song_item(result))
                ]
                if expanded:
                    tracks = expanded
            except Exception as error:
                print(
                    f"Nocky artist songs expansion skipped for {browse_id}: {error}",
                    file=sys.stderr,
                )

        if not tracks:
            radio_id = _text(data.get("radioId") or data.get("shuffleId"))
            if radio_id:
                tracks = _playlist_tracks_from_watch(
                    client,
                    radio_id,
                    "",
                    limit,
                    True,
                )
    else:
        raise RuntimeError("Unsupported YouTube collection type")

    tracks = _dedupe(tracks)[:limit]
    if not tracks:
        raise RuntimeError(
            f"No playable tracks were returned for this YouTube Music {result_type}"
        )
    return tracks


def _artist_release_items(
    results: Any,
    artist_name: str,
) -> list[dict[str, Any]]:
    output: list[dict[str, Any]] = []
    for result in results or []:
        if not isinstance(result, dict):
            continue
        item = _album_item(result, artist_name)
        if item is None:
            continue
        if not item.get("artist"):
            item["artist"] = artist_name
        if not item.get("subtitle"):
            item["subtitle"] = artist_name
        output.append(item)
    return output


def _get_all_artist_releases(
    client: Any,
    browse_id: str,
    params: str,
    limit: int,
) -> list[dict[str, Any]]:
    try:
        return client.get_artist_albums(
            browse_id,
            params,
            limit=limit,
        ) or []
    except TypeError:
        return client.get_artist_albums(browse_id, params) or []


def command_artist(payload: dict[str, Any]) -> dict[str, Any]:
    client = _create_client(authenticated=True)
    browse_id = str(payload.get("browse_id") or "").strip()
    title = str(payload.get("title") or "").strip()
    limit = max(1, min(250, int(payload.get("limit") or 160)))

    if not browse_id and title:
        results = client.search(title, filter="artists", limit=5)
        candidates = [
            item
            for result in results
            if isinstance(result, dict)
            if (item := _artist_item(result))
        ]
        exact = next(
            (
                item
                for item in candidates
                if _text(item.get("title")).casefold() == title.casefold()
            ),
            None,
        )
        selected = exact or (candidates[0] if candidates else None)
        browse_id = _text((selected or {}).get("browse_id"))

    if not browse_id:
        raise RuntimeError("No YouTube Music artist could be resolved")

    data = client.get_artist(browse_id)
    artist_name = _text(data.get("name")) or title or "YouTube Music artist"
    profile = _artist_item(
        {
            "title": artist_name,
            "browseId": browse_id,
            "subscribers": data.get("subscribers") or data.get("monthlyListeners"),
            "thumbnails": data.get("thumbnails") or [],
        }
    )
    if profile is None:
        raise RuntimeError("YouTube Music returned no artist profile")

    releases: list[dict[str, Any]] = []
    expansion_errors: list[str] = []

    for section_name in ("albums", "singles"):
        section = data.get(section_name) or {}
        if not isinstance(section, dict):
            continue

        releases.extend(
            _artist_release_items(
                section.get("results") or section.get("items"),
                artist_name,
            )
        )

        section_browse_id = _text(
            section.get("browseId")
            or section.get("browse_id")
            or browse_id
        )
        section_params = _text(section.get("params"))
        if not section_browse_id or not section_params:
            continue

        try:
            expanded = _get_all_artist_releases(
                client,
                section_browse_id,
                section_params,
                limit,
            )
            releases.extend(_artist_release_items(expanded, artist_name))
        except Exception as error:
            expansion_errors.append(f"{section_name}: {error}")

    releases = _dedupe(releases)[:limit]

    if not releases:
        details = "; ".join(expansion_errors)
        suffix = f" ({details})" if details else ""
        raise RuntimeError(
            f"No albums or singles were returned for {artist_name}{suffix}"
        )

    return {
        "profile": profile,
        "albums": releases,
    }


def command_resolve(payload: dict[str, Any]) -> dict[str, Any]:
    return _resolve_stream(str(payload.get("video_id") or payload.get("url") or ""), bool(payload.get("force")))


COMMANDS = {
    "status": command_status,
    "stream_clients": command_stream_clients,
    "connect": command_connect,
    "disconnect": command_disconnect,
    "search": command_search,
    "library": command_library,
    "liked": command_liked,
    "rate": command_rate,
    "home": command_home,
    "home_v2": command_home_v2,
    "library_v2": command_library_v2,
    "library_page_v2": command_library_page_v2,
    "liked_v2": command_liked_v2,
    "playlists": command_playlists,
    "playlist": command_playlist,
    "collection": command_collection,
    "artist": command_artist,
    "resolve": command_resolve,
}


def main() -> int:
    try:
        if len(sys.argv) != 2 or sys.argv[1] not in COMMANDS:
            raise RuntimeError("Usage: nocky_youtube.py <status|stream_clients|connect|disconnect|search|library|library_v2|library_page_v2|liked|liked_v2|rate|home|home_v2|playlists|playlist|collection|artist|resolve>")
        payload = _read_input()
        result = COMMANDS[sys.argv[1]](payload)
        _emit({"ok": True, "result": result})
        return 0
    except (YTMusicServerError, YTMusicUserError) as error:
        _emit({"ok": False, "error": str(error)})
        return 2
    except subprocess.TimeoutExpired:
        _emit({"ok": False, "error": "The YouTube stream resolver timed out"})
        return 2
    except Exception as error:
        _emit({"ok": False, "error": str(error) or error.__class__.__name__})
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
