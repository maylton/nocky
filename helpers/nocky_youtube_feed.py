#!/usr/bin/env python3
"""Structured YouTube Music feed parsing and cache helpers for Nocky.

The module is deliberately independent from ytmusicapi so parser behavior can be
validated with sanitized fixtures. The main helper supplies the raw payload from
``YTMusic.get_home`` and this module preserves section boundaries, ordering,
layout hints and continuation state for the native GTK client.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import time
from pathlib import Path
from typing import Any, Callable
from urllib.parse import urlsplit, urlunsplit

CONTRACT_VERSION = 3
DEFAULT_CACHE_MAX_AGE = 12 * 60 * 60
VIDEO_ID_PATTERN = re.compile(r"^[A-Za-z0-9_-]{11}$")
ItemFactory = Callable[[dict[str, Any], str], dict[str, Any] | None]


def _text(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, str):
        return value.strip()
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, dict):
        for key in ("text", "title", "name", "label"):
            text = _text(value.get(key))
            if text:
                return text
        runs = value.get("runs")
        if isinstance(runs, list):
            return "".join(_text(run) for run in runs if isinstance(run, dict)).strip()
    return ""


def _names(value: Any) -> str:
    if isinstance(value, list):
        values = [_text(item) for item in value]
        return ", ".join(item for item in values if item)
    return _text(value)


def _upgrade_thumbnail_url(url: str, size: int = 1200) -> str:
    url = (url or "").strip()
    if not url:
        return ""
    parts = urlsplit(url)
    path = parts.path
    upgraded = re.sub(r"=w\d+-h\d+([^/?#]*)$", f"=w{size}-h{size}\\1", path)
    upgraded = re.sub(r"=s\d+([^/?#]*)$", f"=s{size}\\1", upgraded)
    if (
        upgraded == path
        and "googleusercontent.com" in parts.netloc
        and "=" not in path.rsplit("/", 1)[-1]
    ):
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
            url = _text(node.get("url"))
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


def _best_thumbnail(value: Any) -> str:
    candidates = _thumbnail_candidates(value)
    if not candidates:
        return ""
    candidate = max(candidates, key=_thumbnail_area)
    return _upgrade_thumbnail_url(_text(candidate.get("url")))


def _video_thumbnail(video_id: str) -> str:
    video_id = _text(video_id)
    if not VIDEO_ID_PATTERN.fullmatch(video_id):
        return ""
    return f"https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"


def _duration_seconds(result: dict[str, Any]) -> int:
    for key in ("duration_seconds", "durationSeconds"):
        try:
            value = int(result.get(key) or 0)
        except (TypeError, ValueError):
            value = 0
        if value > 0:
            return value
    duration = _text(result.get("duration") or result.get("length"))
    parts = duration.split(":")
    if len(parts) > 1 and all(part.isdigit() for part in parts):
        total = 0
        for part in parts:
            total = total * 60 + int(part)
        return total
    return 0


def _playlist_id(result: dict[str, Any]) -> str:
    for key in ("playlistId", "playlist_id", "audioPlaylistId", "playlist"):
        value = _text(result.get(key))
        if value:
            return value
    browse_id = _text(result.get("browseId") or result.get("browse_id"))
    if browse_id.startswith("VL"):
        return browse_id[2:]
    if browse_id.startswith(("PL", "RD", "OLAK5uy_")):
        return browse_id
    return ""


def _generic_item(result: dict[str, Any], section_title: str) -> dict[str, Any] | None:
    result_type = _text(result.get("resultType") or result.get("result_type")).lower()
    video_id = _text(result.get("videoId") or result.get("video_id"))
    browse_id = _text(result.get("browseId") or result.get("browse_id"))
    playlist_id = _playlist_id(result)
    title = _text(result.get("title") or result.get("name"))
    if not title:
        return None

    if not result_type:
        if video_id:
            result_type = "song"
        elif browse_id.startswith("MPRE"):
            result_type = "album"
        elif browse_id.startswith("UC"):
            result_type = "artist"
        elif playlist_id:
            result_type = "playlist"
        else:
            return None

    aliases = {
        "podcast_episode": "episode",
        "podcast-episode": "episode",
        "upload": "song",
        "uploaded_song": "song",
    }
    result_type = aliases.get(result_type, result_type)

    artists = _names(result.get("artists") or result.get("artist") or result.get("author"))
    album_value = result.get("album") or {}
    album = _text(album_value)
    duration = _duration_seconds(result)
    count = _text(result.get("count") or result.get("itemCount") or result.get("trackCount"))
    year = _text(result.get("year"))

    if result_type in {"song", "video", "episode"}:
        if not video_id:
            return None
        subtitle_parts = [artists, album]
        if duration:
            minutes, seconds = divmod(duration, 60)
            subtitle_parts.append(f"{minutes}:{seconds:02d}")
        subtitle = " • ".join(value for value in subtitle_parts if value)
    elif result_type == "album":
        if not browse_id:
            return None
        subtitle = " • ".join(value for value in (artists, year, section_title) if value)
        album = title
    elif result_type in {"artist", "podcast"}:
        if not browse_id:
            return None
        subtitle = section_title or ("Podcast" if result_type == "podcast" else "Artist")
        artists = title if result_type == "artist" else artists
    elif result_type == "playlist":
        browse_id = playlist_id or browse_id
        if not browse_id:
            return None
        subtitle = " • ".join(value for value in (artists, f"{count} tracks" if count else section_title) if value)
    else:
        return None

    return {
        "result_type": result_type,
        "title": title,
        "subtitle": subtitle,
        "video_id": video_id,
        "browse_id": browse_id,
        "album": album,
        "artist": artists,
        "playlist_kind": "mix" if result_type == "playlist" and any(
            token in f"{title} {section_title}".lower() for token in ("mix", "radio", "supermix")
        ) else ("recommended" if result_type == "playlist" else ""),
        "params": _text(result.get("params")),
        "duration_seconds": duration,
        "thumbnail_url": _best_thumbnail(result) or _video_thumbnail(video_id),
    }


def _identity(item: dict[str, Any]) -> tuple[str, str, str]:
    return (
        _text(item.get("result_type")),
        _text(item.get("video_id") or item.get("browse_id")),
        _text(item.get("title")).casefold(),
    )


def _dedupe(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    seen: set[tuple[str, str, str]] = set()
    output: list[dict[str, Any]] = []
    for item in items:
        key = _identity(item)
        if key in seen:
            continue
        seen.add(key)
        output.append(item)
    return output


def _layout(section: dict[str, Any], items: list[dict[str, Any]]) -> str:
    explicit = _text(section.get("layout") or section.get("sectionType")).lower()
    if explicit in {"carousel", "quick_picks", "grid", "list", "mixed"}:
        return explicit
    title = _text(section.get("title")).casefold()
    item_types = {_text(item.get("result_type")) for item in items}
    if any(token in title for token in ("quick picks", "escolhas rápidas", "ouça novamente", "listen again")):
        return "quick_picks"
    if item_types <= {"song", "video", "episode"}:
        return "list" if len(items) > 8 else "quick_picks"
    if len(item_types) > 1:
        return "mixed"
    return "carousel"


def _section_id(section: dict[str, Any], title: str, absolute_index: int) -> str:
    explicit = _text(
        section.get("id")
        or section.get("browseId")
        or section.get("browse_id")
        or section.get("params")
    )
    basis = explicit or f"{absolute_index}:{title}"
    digest = hashlib.sha1(basis.encode("utf-8")).hexdigest()[:12]
    return f"ytm-{digest}"


def _endpoint(section: dict[str, Any]) -> dict[str, str]:
    endpoint = section.get("endpoint") if isinstance(section.get("endpoint"), dict) else {}
    return {
        "browse_id": _text(
            endpoint.get("browseId")
            or endpoint.get("browse_id")
            or section.get("browseId")
            or section.get("browse_id")
        ),
        "params": _text(endpoint.get("params") or section.get("params")),
    }


def _walk_dicts(value: Any):
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from _walk_dicts(child)
    elif isinstance(value, list):
        for child in value:
            yield from _walk_dicts(child)


def find_inner_tube_home_section_list(source: Any) -> dict[str, Any]:
    candidates: list[tuple[int, dict[str, Any]]] = []
    for node in _walk_dicts(source):
        renderer = node.get("sectionListRenderer")
        if not isinstance(renderer, dict):
            continue
        header = renderer.get("header") if isinstance(renderer.get("header"), dict) else {}
        chip_cloud = (
            header.get("chipCloudRenderer")
            if isinstance(header.get("chipCloudRenderer"), dict)
            else {}
        )
        chips = chip_cloud.get("chips") if isinstance(chip_cloud.get("chips"), list) else []
        contents = renderer.get("contents") if isinstance(renderer.get("contents"), list) else []
        score = (1000 if chips else 0) + len(contents)
        candidates.append((score, renderer))
    return max(candidates, key=lambda candidate: candidate[0])[1] if candidates else {}


def _chip_browse_endpoint(renderer: dict[str, Any]) -> dict[str, Any]:
    for key in ("navigationEndpoint", "onSelectedCommand", "serviceEndpoint"):
        endpoint = renderer.get(key)
        if not isinstance(endpoint, dict):
            continue
        for endpoint_key in ("browseEndpoint", "browseSectionListReloadEndpoint"):
            browse = endpoint.get(endpoint_key)
            if isinstance(browse, dict):
                return browse
    return {}


def extract_inner_tube_home_chips(source: Any) -> list[dict[str, str]]:
    section_list = find_inner_tube_home_section_list(source)
    header = section_list.get("header") if isinstance(section_list.get("header"), dict) else {}
    chip_cloud = (
        header.get("chipCloudRenderer")
        if isinstance(header.get("chipCloudRenderer"), dict)
        else {}
    )
    candidates = chip_cloud.get("chips") if isinstance(chip_cloud.get("chips"), list) else []
    if not candidates:
        candidates = [
            node
            for node in _walk_dicts(source)
            if isinstance(node.get("chipCloudChipRenderer"), dict)
        ]

    output: list[dict[str, str]] = []
    seen: set[str] = set()
    for candidate in candidates:
        if not isinstance(candidate, dict):
            continue
        renderer = candidate.get("chipCloudChipRenderer")
        if not isinstance(renderer, dict):
            continue
        title = _text(renderer.get("text") or renderer.get("title"))
        endpoint = _chip_browse_endpoint(renderer)
        params = _text(endpoint.get("params"))
        if not title or not params or params in seen:
            continue
        seen.add(params)
        output.append(
            {
                "title": title,
                "browse_id": _text(endpoint.get("browseId") or endpoint.get("browse_id")),
                "params": params,
            }
        )
    return output


def _dig(value: Any, *path: str) -> Any:
    current = value
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def _normalized_identity(value: Any) -> str:
    return re.sub(r"\s+", " ", _text(value).casefold()).strip()


def _renderer_title(renderer: dict[str, Any]) -> str:
    title = _text(renderer.get("title"))
    if title:
        return title
    for column_key in ("flexColumns", "fixedColumns"):
        columns = renderer.get(column_key)
        if not isinstance(columns, list):
            continue
        for column in columns:
            if not isinstance(column, dict):
                continue
            title = _text(
                _dig(column, "musicResponsiveListItemFlexColumnRenderer", "text")
                or _dig(column, "musicResponsiveListItemFixedColumnRenderer", "text")
            )
            if title:
                return title
    return ""


def _renderer_video_id(renderer: dict[str, Any]) -> str:
    direct_paths = (
        ("playlistItemData", "videoId"),
        ("navigationEndpoint", "watchEndpoint", "videoId"),
        ("onTap", "watchEndpoint", "videoId"),
        (
            "overlay",
            "musicItemThumbnailOverlayRenderer",
            "content",
            "musicPlayButtonRenderer",
            "playNavigationEndpoint",
            "watchEndpoint",
            "videoId",
        ),
        (
            "thumbnailOverlay",
            "musicItemThumbnailOverlayRenderer",
            "content",
            "musicPlayButtonRenderer",
            "playNavigationEndpoint",
            "watchEndpoint",
            "videoId",
        ),
    )
    for path in direct_paths:
        video_id = _text(_dig(renderer, *path))
        if video_id:
            return video_id
    for node in _walk_dicts(renderer):
        endpoint = node.get("watchEndpoint")
        if isinstance(endpoint, dict):
            video_id = _text(endpoint.get("videoId"))
            if video_id:
                return video_id
    return ""


def _renderer_playlist_id(renderer: dict[str, Any]) -> str:
    for node in _walk_dicts(renderer):
        for endpoint_key in ("watchPlaylistEndpoint", "watchEndpoint"):
            endpoint = node.get(endpoint_key)
            if not isinstance(endpoint, dict):
                continue
            playlist_id = _text(endpoint.get("playlistId"))
            if playlist_id:
                return playlist_id
    return ""


def _renderer_browse_id(renderer: dict[str, Any]) -> str:
    for node in _walk_dicts(renderer):
        endpoint = node.get("browseEndpoint")
        if not isinstance(endpoint, dict):
            continue
        browse_id = _text(endpoint.get("browseId"))
        if browse_id:
            return browse_id
    return ""


def _renderer_thumbnail_candidates(renderer: dict[str, Any]) -> list[dict[str, Any]]:
    preferred_paths = (
        ("thumbnailRenderer", "musicThumbnailRenderer", "thumbnail", "thumbnails"),
        ("thumbnailRenderer", "croppedSquareThumbnailRenderer", "thumbnail", "thumbnails"),
        (
            "thumbnailRenderer",
            "musicAnimatedThumbnailRenderer",
            "backupRenderer",
            "thumbnail",
            "thumbnails",
        ),
        ("thumbnail", "musicThumbnailRenderer", "thumbnail", "thumbnails"),
        ("thumbnail", "croppedSquareThumbnailRenderer", "thumbnail", "thumbnails"),
        (
            "thumbnail",
            "musicAnimatedThumbnailRenderer",
            "backupRenderer",
            "thumbnail",
            "thumbnails",
        ),
        (
            "thumbnailRenderer",
            "musicAnimatedThumbnailRenderer",
            "animatedThumbnail",
            "thumbnails",
        ),
        ("thumbnail", "musicAnimatedThumbnailRenderer", "animatedThumbnail", "thumbnails"),
    )
    for path in preferred_paths:
        value = _dig(renderer, *path)
        if isinstance(value, list):
            candidates = [item for item in value if isinstance(item, dict) and _text(item.get("url"))]
            if candidates:
                return candidates
    return _thumbnail_candidates(renderer)


def _raw_renderer_item(renderer: dict[str, Any]) -> dict[str, Any]:
    return {
        "title": _renderer_title(renderer),
        "videoId": _renderer_video_id(renderer),
        "playlistId": _renderer_playlist_id(renderer),
        "browseId": _renderer_browse_id(renderer),
        "rawRendererThumbnails": _renderer_thumbnail_candidates(renderer),
    }


def _raw_inner_tube_home_sections(source: Any) -> list[dict[str, Any]]:
    if isinstance(source, list):
        contents = source
    elif isinstance(source, dict):
        section_list = find_inner_tube_home_section_list(source)
        contents = section_list.get("contents") if isinstance(section_list.get("contents"), list) else []
        if not contents and isinstance(source.get("contents"), list):
            contents = source["contents"]
    else:
        contents = []

    sections: list[dict[str, Any]] = []
    for content in contents:
        if not isinstance(content, dict):
            continue
        carousel = content.get("musicCarouselShelfRenderer")
        if not isinstance(carousel, dict):
            continue
        header = carousel.get("header") if isinstance(carousel.get("header"), dict) else {}
        title = _text(_dig(header, "musicCarouselShelfBasicHeaderRenderer", "title"))
        raw_items: list[dict[str, Any]] = []
        for entry in carousel.get("contents") or []:
            if not isinstance(entry, dict):
                continue
            renderer = next(
                (
                    entry.get(key)
                    for key in (
                        "musicTwoRowItemRenderer",
                        "musicResponsiveListItemRenderer",
                        "musicMultiRowListItemRenderer",
                    )
                    if isinstance(entry.get(key), dict)
                ),
                None,
            )
            if isinstance(renderer, dict):
                raw_items.append(_raw_renderer_item(renderer))
        if title and raw_items:
            sections.append({"title": title, "items": raw_items})
    return sections


def _item_identifiers(item: dict[str, Any]) -> set[str]:
    identifiers: set[str] = set()
    for key in ("videoId", "video_id", "playlistId", "playlist_id", "browseId", "browse_id"):
        value = _normalized_identity(item.get(key))
        if value:
            identifiers.add(value)
    return identifiers


def _raw_item_match_score(
    parsed: dict[str, Any],
    raw: dict[str, Any],
    parsed_index: int,
    raw_index: int,
) -> int:
    score = 0
    parsed_ids = _item_identifiers(parsed)
    raw_ids = _item_identifiers(raw)
    if parsed_ids and raw_ids and parsed_ids.intersection(raw_ids):
        score += 100
    parsed_title = _normalized_identity(parsed.get("title") or parsed.get("name"))
    raw_title = _normalized_identity(raw.get("title"))
    if parsed_title and raw_title and parsed_title == raw_title:
        score += 30
    if parsed_index == raw_index:
        score += 5
    return score


def _enrich_home_item(parsed: dict[str, Any], raw: dict[str, Any]) -> dict[str, Any]:
    item = dict(parsed)
    for key in ("videoId", "playlistId", "browseId"):
        if not _text(item.get(key)) and _text(raw.get(key)):
            item[key] = raw[key]
    raw_thumbnails = raw.get("rawRendererThumbnails")
    if isinstance(raw_thumbnails, list) and raw_thumbnails:
        item["rawRendererThumbnails"] = raw_thumbnails
    return item


def enrich_inner_tube_home_rows(rows: Any, raw_source: Any) -> list[dict[str, Any]]:
    """Restore renderer fields discarded by ytmusicapi's mixed-content parser.

    The Android reference client parses each WEB_REMIX renderer directly. Nocky
    keeps ytmusicapi's stable metadata parsing, then overlays thumbnail and endpoint
    identity from the matching raw renderer before building the native feed.
    """

    parsed_rows = [dict(row) for row in (rows or []) if isinstance(row, dict)]
    raw_sections = _raw_inner_tube_home_sections(raw_source)
    if not raw_sections:
        return parsed_rows

    used_sections: set[int] = set()
    for row_index, row in enumerate(parsed_rows):
        title = _normalized_identity(row.get("title"))
        section_index = next(
            (
                index
                for index, section in enumerate(raw_sections)
                if index not in used_sections
                and _normalized_identity(section.get("title")) == title
            ),
            None,
        )
        if section_index is None and row_index < len(raw_sections) and row_index not in used_sections:
            section_index = row_index
        if section_index is None:
            continue
        used_sections.add(section_index)
        raw_items = raw_sections[section_index]["items"]
        contents = row.get("contents") or row.get("items") or row.get("results") or []
        enriched_contents: list[Any] = []
        used_items: set[int] = set()
        for parsed_index, parsed in enumerate(contents):
            if not isinstance(parsed, dict):
                enriched_contents.append(parsed)
                continue
            candidates = [
                (
                    _raw_item_match_score(parsed, raw, parsed_index, raw_index),
                    raw_index,
                    raw,
                )
                for raw_index, raw in enumerate(raw_items)
                if raw_index not in used_items
            ]
            score, raw_index, raw = max(candidates, default=(0, -1, {}), key=lambda candidate: candidate[0])
            if score > 0 and raw_index >= 0:
                used_items.add(raw_index)
                enriched_contents.append(_enrich_home_item(parsed, raw))
            else:
                enriched_contents.append(parsed)
        row["contents"] = enriched_contents
    return parsed_rows


def _chips(source: Any) -> list[dict[str, str]]:
    candidates: Any = []
    if isinstance(source, dict):
        candidates = source.get("chips") or source.get("filters") or []
    output: list[dict[str, str]] = []
    for chip in candidates or []:
        if not isinstance(chip, dict):
            continue
        title = _text(chip.get("title") or chip.get("text") or chip.get("name"))
        if not title:
            continue
        output.append(
            {
                "title": title,
                "browse_id": _text(chip.get("browseId") or chip.get("browse_id")),
                "params": _text(chip.get("params")),
            }
        )
    return output


def build_structured_home(
    source: Any,
    *,
    offset: int = 0,
    section_limit: int = 6,
    selected_chip_params: str = "",
    item_factory: ItemFactory | None = None,
) -> dict[str, Any]:
    """Convert a ytmusicapi home payload into Nocky's versioned feed contract."""

    if isinstance(source, dict):
        rows = source.get("sections") or source.get("contents") or source.get("results") or []
    else:
        rows = source or []
    rows = [row for row in rows if isinstance(row, dict)]
    offset = max(0, int(offset or 0))
    section_limit = max(1, min(24, int(section_limit or 6)))
    factory = item_factory or _generic_item

    sections: list[dict[str, Any]] = []
    selected_rows = rows[offset : offset + section_limit]
    for relative_index, row in enumerate(selected_rows):
        absolute_index = offset + relative_index
        title = _text(row.get("title")) or "Recommended"
        label = _text(row.get("strapline") or row.get("subtitle") or row.get("label"))
        contents = row.get("contents") or row.get("items") or row.get("results") or []
        items = []
        for result in contents or []:
            if not isinstance(result, dict):
                continue
            item = factory(result, title)
            if item is not None:
                items.append(item)
        items = _dedupe(items)
        if not items:
            continue
        sections.append(
            {
                "id": _section_id(row, title, absolute_index),
                "title": title,
                "label": label,
                "thumbnail_url": _best_thumbnail(row),
                "layout": _layout(row, items),
                "endpoint": _endpoint(row),
                "items": items,
            }
        )

    next_offset = offset + section_limit
    continuation = str(next_offset) if next_offset < len(rows) else ""
    return {
        "version": CONTRACT_VERSION,
        "generated_at": int(time.time()),
        "stale": False,
        "selected_chip_params": _text(selected_chip_params),
        "chips": _chips(source),
        "sections": sections,
        "continuation": continuation,
    }


def build_library_overview(
    sections: list[tuple[str, str, list[dict[str, Any]]]],
) -> dict[str, Any]:
    """Build the same feed contract for account-library collections."""

    normalized_sections = []
    for index, (title, layout, items) in enumerate(sections):
        items = _dedupe([item for item in items if isinstance(item, dict)])
        if not items:
            continue
        normalized_sections.append(
            {
                "id": _section_id({}, title, index),
                "title": title,
                "label": "",
                "thumbnail_url": "",
                "layout": layout,
                "endpoint": {"browse_id": "", "params": ""},
                "items": items,
            }
        )
    return {
        "version": CONTRACT_VERSION,
        "generated_at": int(time.time()),
        "stale": False,
        "chips": [],
        "sections": normalized_sections,
        "continuation": "",
    }


def _read_cache(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}
    return payload if isinstance(payload, dict) else {}


def load_cached_page(
    path: Path,
    key: str,
    *,
    max_age: int = DEFAULT_CACHE_MAX_AGE,
    allow_stale: bool = False,
) -> dict[str, Any] | None:
    payload = _read_cache(path)
    entry = (payload.get("pages") or {}).get(key)
    if not isinstance(entry, dict) or not isinstance(entry.get("page"), dict):
        return None
    saved_at = int(entry.get("saved_at") or 0)
    expired = saved_at <= 0 or int(time.time()) - saved_at > max_age
    if expired and not allow_stale:
        return None
    page = dict(entry["page"])
    page["stale"] = expired or allow_stale
    return page


def save_cached_page(path: Path, key: str, page: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = _read_cache(path)
    pages = payload.get("pages") if isinstance(payload.get("pages"), dict) else {}
    pages[key] = {"saved_at": int(time.time()), "page": page}
    payload = {"version": CONTRACT_VERSION, "pages": pages}
    temporary = path.with_name(f"{path.name}.{os.getpid()}.tmp")
    temporary.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    os.chmod(temporary, 0o600)
    temporary.replace(path)
    os.chmod(path, 0o600)
