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
import time
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, urlparse, urlsplit, urlunsplit

try:
    import requests
    from ytmusicapi import YTMusic
    from ytmusicapi.exceptions import YTMusicServerError, YTMusicUserError
    try:
        from ytmusicapi.setup import setup as ytmusicapi_setup
    except Exception:
        ytmusicapi_setup = None
except Exception as error:  # pragma: no cover - reported to the native app
    requests = None
    YTMusic = None
    YTMusicServerError = RuntimeError
    YTMusicUserError = RuntimeError
    ytmusicapi_setup = None
    IMPORT_ERROR = error
else:
    IMPORT_ERROR = None

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
    normalized.setdefault("accept-language", "en-US,en;q=0.9")
    _ensure_authorization(normalized)
    return normalized


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
    for key in ("LC_ALL", "LC_MESSAGES", "LANG"):
        value = os.environ.get(key, "").strip()
        if value:
            return value
    try:
        language, _encoding = locale.getlocale()
    except Exception:
        language = None
    return language or ""


def _language() -> str:
    value = _system_locale().lower()
    for language in ("pt", "es", "fr", "de", "it"):
        if value.startswith(language):
            return language
    return "en"


def _location() -> str:
    value = _system_locale().replace("-", "_").upper()
    return "BR" if "_BR" in value else ""


def _create_client(authenticated: bool = True):
    _require_dependencies()
    session = _session()
    if authenticated:
        payload = _load_session()
        headers = payload.get("headers")
        if isinstance(headers, dict) and headers:
            normalized = {str(k): str(v) for k, v in headers.items()}
            _ensure_authorization(normalized)
            return YTMusic(normalized, requests_session=session)
    return YTMusic(
        requests_session=session,
        language=_language(),
        location=_location(),
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
    upgraded = re.sub(r"=s\d+([^/?#]*)$", f"=s{size}\\1", upgraded)
    if upgraded == path and "googleusercontent.com" in parts.netloc and "=" not in path.rsplit("/", 1)[-1]:
        upgraded = f"{path}=s{size}"
    return urlunsplit(parts._replace(path=upgraded))


def _best_thumbnail(thumbnails: Any) -> str:
    candidates = [item for item in (thumbnails or []) if isinstance(item, dict)]
    if not candidates:
        return ""
    candidate = max(candidates, key=lambda item: int(item.get("width") or 0) * int(item.get("height") or 0))
    return _upgrade_thumbnail_url(str(candidate.get("url") or ""))


def _thumbnails(result: dict[str, Any]) -> Any:
    return result.get("thumbnails") or result.get("thumbnail") or []


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
        "thumbnail_url": _best_thumbnail(_thumbnails(result)),
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


def _resolve_stream(video_or_url: str, force: bool = False) -> dict[str, Any]:
    video_id = _extract_video_id(video_or_url)
    cache = _load_stream_cache()
    cached = cache.get(video_id)
    if force and isinstance(cached, dict):
        # Overwrite the cached entry with an expired marker so _save_stream_cache
        # removes a URL that the CDN has already rejected.
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
    process = subprocess.run(command, capture_output=True, text=True, timeout=60, check=False)
    if process.returncode != 0:
        lines = [line.strip() for line in (process.stderr or process.stdout).splitlines() if line.strip()]
        raise RuntimeError("\n".join(lines[-6:]) or "yt-dlp could not resolve this track")
    payload = json.loads(process.stdout)
    stream_url = str(payload.get("url") or "").strip()
    if not stream_url:
        raise RuntimeError("yt-dlp returned no playable stream URL")
    headers = {
        str(key): str(value)
        for key, value in (payload.get("http_headers") or {}).items()
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
        "format_id": str(payload.get("format_id") or ""),
        "protocol": str(payload.get("protocol") or ""),
        "container": str(payload.get("ext") or ""),
        "audio_codec": str(payload.get("acodec") or ""),
        "expires_at": time.time() + STREAM_CACHE_TTL,
    }
    cache[video_id] = result
    _save_stream_cache(cache)
    return result


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


def command_resolve(payload: dict[str, Any]) -> dict[str, Any]:
    return _resolve_stream(str(payload.get("video_id") or payload.get("url") or ""), bool(payload.get("force")))


COMMANDS = {
    "status": command_status,
    "connect": command_connect,
    "disconnect": command_disconnect,
    "search": command_search,
    "library": command_library,
    "liked": command_liked,
    "home": command_home,
    "playlists": command_playlists,
    "playlist": command_playlist,
    "resolve": command_resolve,
}


def main() -> int:
    try:
        if len(sys.argv) != 2 or sys.argv[1] not in COMMANDS:
            raise RuntimeError("Usage: nocky_youtube.py <status|connect|disconnect|search|library|liked|home|playlists|playlist|resolve>")
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
