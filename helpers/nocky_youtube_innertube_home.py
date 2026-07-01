#!/usr/bin/env python3
"""Direct parser for YouTube Music Home InnerTube renderers.

The Home endpoint contains richer artwork and playback identity than the normalized
objects returned by ``ytmusicapi.parse_mixed_content``. This module follows the
same architecture as the Android reference client: each shelf and card is converted
from its raw WEB_REMIX renderer before an intermediate parser can discard fields.
"""

from __future__ import annotations

import re
from typing import Any, Iterable


VIDEO_ID_PATTERN = re.compile(r"^[A-Za-z0-9_-]{11}$")
DURATION_PATTERN = re.compile(r"(?<!\d)(?:\d{1,2}:)?\d{1,2}:\d{2}(?!\d)")
YEAR_PATTERN = re.compile(r"^(?:19|20)\d{2}$")
SEPARATOR_TEXTS = {"•", "·", "|", "—", "–", "-"}

RENDERER_KEYS = (
    "musicTwoRowItemRenderer",
    "musicResponsiveListItemRenderer",
    "musicMultiRowListItemRenderer",
    "reelItemRenderer",
    "shortsLockupViewModel",
)

SECTION_KEYS = (
    "musicCarouselShelfRenderer",
    "musicImmersiveCarouselShelfRenderer",
    "musicShelfRenderer",
    "gridRenderer",
)

PAGE_TYPE_ALIASES = {
    "MUSIC_PAGE_TYPE_ALBUM": "album",
    "MUSIC_PAGE_TYPE_AUDIOBOOK": "album",
    "MUSIC_PAGE_TYPE_ARTIST": "artist",
    "MUSIC_PAGE_TYPE_LIBRARY_ARTIST": "artist",
    "MUSIC_PAGE_TYPE_USER_CHANNEL": "artist",
    "MUSIC_PAGE_TYPE_PLAYLIST": "playlist",
    "MUSIC_PAGE_TYPE_PODCAST_SHOW_DETAIL_PAGE": "podcast",
    "MUSIC_PAGE_TYPE_NON_MUSIC_AUDIO_TRACK_PAGE": "episode",
}

VIDEO_SECTION_TOKENS = (
    "video",
    "vídeo",
    "short",
    "live",
    "ao vivo",
    "performance",
    "apresenta",
    "cover",
    "remix",
)


# ---------------------------------------------------------------------------
# Generic JSON helpers
# ---------------------------------------------------------------------------


def _text(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, str):
        return value.strip()
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, dict):
        for key in ("text", "simpleText", "content", "title", "name", "label"):
            result = _text(value.get(key))
            if result:
                return result
        runs = value.get("runs")
        if isinstance(runs, list):
            return "".join(_text(run) for run in runs if isinstance(run, dict)).strip()
    return ""


def _dig(value: Any, *path: str) -> Any:
    current = value
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def _walk_dicts(value: Any) -> Iterable[dict[str, Any]]:
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from _walk_dicts(child)
    elif isinstance(value, (list, tuple)):
        for child in value:
            yield from _walk_dicts(child)


def _runs(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, dict):
        return []
    runs = value.get("runs")
    if isinstance(runs, list):
        return [run for run in runs if isinstance(run, dict) and _text(run)]
    text = _text(value)
    return [{"text": text}] if text else []


def _column_runs(renderer: dict[str, Any]) -> list[list[dict[str, Any]]]:
    columns: list[list[dict[str, Any]]] = []
    for key in ("flexColumns", "fixedColumns"):
        value = renderer.get(key)
        if not isinstance(value, list):
            continue
        for column in value:
            if not isinstance(column, dict):
                continue
            column_renderer = next(
                (
                    candidate
                    for candidate_key, candidate in column.items()
                    if candidate_key.endswith("ColumnRenderer") and isinstance(candidate, dict)
                ),
                None,
            )
            if isinstance(column_renderer, dict):
                column_runs = _runs(column_renderer.get("text"))
                if column_runs:
                    columns.append(column_runs)
    return columns


def _primary_endpoint(renderer: dict[str, Any]) -> dict[str, Any]:
    endpoint = renderer.get("navigationEndpoint")
    if isinstance(endpoint, dict):
        return endpoint

    for path in (
        ("onTap",),
        ("onTap", "innertubeCommand"),
        (
            "overlay",
            "musicItemThumbnailOverlayRenderer",
            "content",
            "musicPlayButtonRenderer",
            "playNavigationEndpoint",
        ),
        (
            "thumbnailOverlay",
            "musicItemThumbnailOverlayRenderer",
            "content",
            "musicPlayButtonRenderer",
            "playNavigationEndpoint",
        ),
    ):
        endpoint = _dig(renderer, *path)
        if isinstance(endpoint, dict):
            return endpoint

    title_runs = _title_runs(renderer)
    for run in title_runs:
        endpoint = run.get("navigationEndpoint")
        if isinstance(endpoint, dict):
            return endpoint
    return {}


def _endpoint_of_type(value: Any, endpoint_key: str) -> dict[str, Any]:
    for node in _walk_dicts(value):
        endpoint = node.get(endpoint_key)
        if isinstance(endpoint, dict):
            return endpoint
    return {}


def _browse_page_type(endpoint: dict[str, Any]) -> str:
    browse = endpoint.get("browseEndpoint") if isinstance(endpoint, dict) else None
    if not isinstance(browse, dict):
        return ""
    return _text(
        _dig(
            browse,
            "browseEndpointContextSupportedConfigs",
            "browseEndpointContextMusicConfig",
            "pageType",
        )
    )


def _browse_id(endpoint: dict[str, Any]) -> str:
    browse = endpoint.get("browseEndpoint") if isinstance(endpoint, dict) else None
    return _text(browse.get("browseId")) if isinstance(browse, dict) else ""


def _endpoint_params(endpoint: dict[str, Any]) -> str:
    browse = endpoint.get("browseEndpoint") if isinstance(endpoint, dict) else None
    return _text(browse.get("params")) if isinstance(browse, dict) else ""


# ---------------------------------------------------------------------------
# Text and metadata extraction
# ---------------------------------------------------------------------------


def _title_runs(renderer: dict[str, Any]) -> list[dict[str, Any]]:
    for key in ("title", "headline"):
        runs = _runs(renderer.get(key))
        if runs:
            return runs

    columns = _column_runs(renderer)
    if columns:
        return columns[0]

    for path in (
        ("overlayMetadata", "primaryText"),
        ("overlayMetadata", "primaryText", "content"),
        ("accessibilityText",),
    ):
        value = _dig(renderer, *path)
        text = _text(value)
        if text:
            return [{"text": text}]
    return []


def _subtitle_runs(renderer: dict[str, Any]) -> list[dict[str, Any]]:
    runs = _runs(renderer.get("subtitle"))
    if runs:
        return runs

    columns = _column_runs(renderer)
    if len(columns) > 1:
        output: list[dict[str, Any]] = []
        for index, column in enumerate(columns[1:]):
            if index and output:
                output.append({"text": " • "})
            output.extend(column)
        return output

    for path in (
        ("overlayMetadata", "secondaryText"),
        ("overlayMetadata", "secondaryText", "content"),
        ("description",),
    ):
        value = _dig(renderer, *path)
        text = _text(value)
        if text:
            return [{"text": text}]
    return []


def _run_browse_endpoint(run: dict[str, Any]) -> dict[str, Any]:
    endpoint = run.get("navigationEndpoint")
    if not isinstance(endpoint, dict):
        return {}
    browse = endpoint.get("browseEndpoint")
    return browse if isinstance(browse, dict) else {}


def _run_page_type(run: dict[str, Any]) -> str:
    browse = _run_browse_endpoint(run)
    return _text(
        _dig(
            browse,
            "browseEndpointContextSupportedConfigs",
            "browseEndpointContextMusicConfig",
            "pageType",
        )
    )


def _clean_subtitle_text(runs: list[dict[str, Any]]) -> str:
    parts: list[str] = []
    for run in runs:
        text = _text(run)
        if not text:
            continue
        if text.strip() in SEPARATOR_TEXTS:
            if parts and parts[-1] != " • ":
                parts.append(" • ")
            continue
        text = re.sub(r"\s*[•·|]\s*", " • ", text)
        parts.append(text)
    result = "".join(parts)
    result = re.sub(r"\s*•\s*", " • ", result)
    result = re.sub(r"\s+", " ", result).strip(" •")
    return result


def _artists(runs: list[dict[str, Any]], result_type: str) -> list[dict[str, str]]:
    artists: list[dict[str, str]] = []
    seen: set[str] = set()
    for run in runs:
        text = _text(run)
        if not text or text.strip() in SEPARATOR_TEXTS:
            continue
        browse = _run_browse_endpoint(run)
        browse_id = _text(browse.get("browseId"))
        page_type = _run_page_type(run)
        is_artist = (
            page_type in {
                "MUSIC_PAGE_TYPE_ARTIST",
                "MUSIC_PAGE_TYPE_LIBRARY_ARTIST",
                "MUSIC_PAGE_TYPE_USER_CHANNEL",
            }
            or browse_id.startswith("UC")
        )
        if not is_artist:
            continue
        key = browse_id or text.casefold()
        if key in seen:
            continue
        seen.add(key)
        artists.append({"name": text, "id": browse_id})

    if artists or result_type not in {"song", "video", "playlist", "album"}:
        return artists

    # Some recommendation shelves omit endpoint metadata from the author run. Use
    # the first human-readable subtitle segment as a conservative fallback.
    for run in runs:
        text = _text(run)
        if not text or text.strip() in SEPARATOR_TEXTS:
            continue
        if DURATION_PATTERN.fullmatch(text) or YEAR_PATTERN.fullmatch(text):
            continue
        lowered = text.casefold()
        if lowered in {"song", "video", "album", "playlist", "mix", "single", "ep"}:
            continue
        return [{"name": text, "id": ""}]
    return []


def _album(runs: list[dict[str, Any]]) -> dict[str, str] | None:
    for run in runs:
        browse = _run_browse_endpoint(run)
        browse_id = _text(browse.get("browseId"))
        page_type = _run_page_type(run)
        if page_type in {"MUSIC_PAGE_TYPE_ALBUM", "MUSIC_PAGE_TYPE_AUDIOBOOK"} or browse_id.startswith("MPRE"):
            name = _text(run)
            if name and browse_id:
                return {"name": name, "id": browse_id}
    return None


def _duration_seconds(renderer: dict[str, Any], subtitle_runs: list[dict[str, Any]]) -> int:
    candidates = [_text(run) for run in subtitle_runs]
    for column in _column_runs(renderer):
        candidates.extend(_text(run) for run in column)
    for candidate in reversed(candidates):
        match = DURATION_PATTERN.search(candidate)
        if not match:
            continue
        parts = [int(part) for part in match.group(0).split(":")]
        total = 0
        for part in parts:
            total = total * 60 + part
        if total > 0:
            return total
    return 0


def _year(subtitle_runs: list[dict[str, Any]]) -> str:
    for run in reversed(subtitle_runs):
        text = _text(run)
        if YEAR_PATTERN.fullmatch(text):
            return text
    return ""


def _count_text(subtitle_runs: list[dict[str, Any]]) -> str:
    for run in subtitle_runs:
        text = _text(run)
        if re.search(r"\b\d[\d.,]*\s+(?:songs?|tracks?|músicas?|faixas?|episodes?|episódios?)\b", text, re.I):
            return text
    return ""


# ---------------------------------------------------------------------------
# Artwork and identity extraction
# ---------------------------------------------------------------------------


def _thumbnail_area(candidate: dict[str, Any]) -> int:
    try:
        return max(0, int(candidate.get("width") or 0)) * max(0, int(candidate.get("height") or 0))
    except (TypeError, ValueError):
        return 0


def _valid_thumbnail_list(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [
        dict(candidate)
        for candidate in value
        if isinstance(candidate, dict) and _text(candidate.get("url"))
    ]


def _thumbnail_candidates(renderer: dict[str, Any]) -> list[dict[str, Any]]:
    # Prefer static artwork over animated WebP sources. GTK can display the latter,
    # but the backup renderer is the same stable cover used by the Android client.
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
        ("thumbnail", "thumbnails"),
        ("thumbnail", "sources"),
        ("thumbnailRenderer", "thumbnail", "thumbnails"),
        ("thumbnailRenderer", "thumbnail", "sources"),
    )
    for path in preferred_paths:
        candidates = _valid_thumbnail_list(_dig(renderer, *path))
        if candidates:
            return candidates

    candidates: list[dict[str, Any]] = []
    for node in _walk_dicts(renderer):
        url = _text(node.get("url"))
        if not url:
            continue
        # Exclude navigation icons, badges and avatars when a card renderer contains
        # more than one image-like object. Artwork URLs normally carry dimensions.
        if "width" not in node and "height" not in node:
            continue
        candidates.append(dict(node))
    if not candidates:
        return []
    largest = max(candidates, key=_thumbnail_area)
    return [largest]


def _video_id(renderer: dict[str, Any]) -> str:
    direct_paths = (
        ("playlistItemData", "videoId"),
        ("videoId",),
        ("navigationEndpoint", "watchEndpoint", "videoId"),
        ("navigationEndpoint", "reelWatchEndpoint", "videoId"),
        ("onTap", "watchEndpoint", "videoId"),
        ("onTap", "reelWatchEndpoint", "videoId"),
        ("onTap", "innertubeCommand", "watchEndpoint", "videoId"),
        ("onTap", "innertubeCommand", "reelWatchEndpoint", "videoId"),
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
        candidate = _text(_dig(renderer, *path))
        if VIDEO_ID_PATTERN.fullmatch(candidate):
            return candidate

    for node in _walk_dicts(renderer):
        for key in ("watchEndpoint", "reelWatchEndpoint"):
            endpoint = node.get(key)
            if not isinstance(endpoint, dict):
                continue
            candidate = _text(endpoint.get("videoId"))
            if VIDEO_ID_PATTERN.fullmatch(candidate):
                return candidate
    return ""


def _playlist_id(renderer: dict[str, Any]) -> str:
    for node in _walk_dicts(renderer):
        for key in ("watchPlaylistEndpoint", "watchEndpoint"):
            endpoint = node.get(key)
            if not isinstance(endpoint, dict):
                continue
            playlist_id = _text(endpoint.get("playlistId"))
            if playlist_id:
                return playlist_id
    return ""


def _primary_browse(renderer: dict[str, Any]) -> dict[str, Any]:
    endpoint = _primary_endpoint(renderer)
    browse = endpoint.get("browseEndpoint") if isinstance(endpoint, dict) else None
    if isinstance(browse, dict):
        return browse

    for run in _title_runs(renderer):
        browse = _run_browse_endpoint(run)
        if browse:
            return browse
    return {}


def _music_video_type(renderer: dict[str, Any]) -> str:
    for node in _walk_dicts(renderer):
        value = _text(node.get("musicVideoType"))
        if value:
            return value
    return ""


def _result_type(
    renderer: dict[str, Any],
    renderer_key: str,
    section_title: str,
    video_id: str,
    browse_id: str,
    playlist_id: str,
) -> str:
    endpoint = _primary_endpoint(renderer)
    page_type = _browse_page_type(endpoint)
    mapped = PAGE_TYPE_ALIASES.get(page_type)
    if mapped:
        return mapped

    if browse_id.startswith("MPRE"):
        return "album"
    if browse_id.startswith("UC"):
        return "artist"
    if browse_id.startswith("VL") or browse_id.startswith(("PL", "RD", "OLAK5uy_")):
        return "playlist"
    if playlist_id and not video_id:
        return "playlist"

    if renderer_key == "musicMultiRowListItemRenderer":
        return "episode"
    if renderer_key in {"reelItemRenderer", "shortsLockupViewModel"}:
        return "video"
    if video_id:
        video_type = _music_video_type(renderer)
        if any(token in video_type for token in ("OMV", "UGC")):
            return "video"
        if any(token in section_title.casefold() for token in VIDEO_SECTION_TOKENS):
            return "video"
        return "song"
    return ""


# ---------------------------------------------------------------------------
# Renderer conversion
# ---------------------------------------------------------------------------


def _renderer_item(
    renderer: dict[str, Any],
    renderer_key: str,
    section_title: str,
) -> dict[str, Any] | None:
    title_runs = _title_runs(renderer)
    title = _text({"runs": title_runs})
    if not title:
        return None

    subtitle_runs = _subtitle_runs(renderer)
    video_id = _video_id(renderer)
    primary_browse = _primary_browse(renderer)
    browse_id = _text(primary_browse.get("browseId"))
    playlist_id = _playlist_id(renderer)
    result_type = _result_type(
        renderer,
        renderer_key,
        section_title,
        video_id,
        browse_id,
        playlist_id,
    )
    if not result_type:
        return None

    if result_type == "playlist":
        normalized = browse_id[2:] if browse_id.startswith("VL") else browse_id
        playlist_id = playlist_id or normalized
        browse_id = playlist_id or normalized

    artists = _artists(subtitle_runs, result_type)
    album = _album(subtitle_runs)
    thumbnails = _thumbnail_candidates(renderer)
    duration_seconds = _duration_seconds(renderer, subtitle_runs)
    subtitle = _clean_subtitle_text(subtitle_runs)

    item: dict[str, Any] = {
        "resultType": result_type,
        "title": title,
        "subtitle": subtitle,
        "videoId": video_id,
        "browseId": browse_id,
        "playlistId": playlist_id,
        "params": _text(primary_browse.get("params")),
        "artists": artists,
        "album": album or {},
        "duration_seconds": duration_seconds,
        "year": _year(subtitle_runs),
        "count": _count_text(subtitle_runs),
        "thumbnails": thumbnails,
        "rendererType": renderer_key,
    }

    # Preserve a playable fallback for generated mixes whose primary action is a
    # watch endpoint carrying both video and playlist identity.
    if result_type == "playlist" and video_id:
        item["seedVideoId"] = video_id
    return item


def _renderer_from_entry(entry: dict[str, Any]) -> tuple[str, dict[str, Any]] | None:
    for key in RENDERER_KEYS:
        renderer = entry.get(key)
        if isinstance(renderer, dict):
            return key, renderer

    # Some experiments wrap a renderer one level deeper in a generic content/view
    # model. Restrict the fallback to known keys to avoid duplicate nested cards.
    for node in entry.values():
        if not isinstance(node, dict):
            continue
        for key in RENDERER_KEYS:
            renderer = node.get(key)
            if isinstance(renderer, dict):
                return key, renderer
    return None


def _section_header(section: dict[str, Any]) -> dict[str, Any]:
    header = section.get("header")
    if not isinstance(header, dict):
        return {}
    for key in (
        "musicCarouselShelfBasicHeaderRenderer",
        "musicShelfRenderer",
        "gridHeaderRenderer",
        "musicDescriptionShelfRenderer",
    ):
        renderer = header.get(key)
        if isinstance(renderer, dict):
            return renderer
    return header


def _section_title(section: dict[str, Any]) -> str:
    header = _section_header(section)
    for key in ("title", "headline"):
        title = _text(header.get(key))
        if title:
            return title
    return _text(section.get("title"))


def _section_label(section: dict[str, Any]) -> str:
    header = _section_header(section)
    return _text(header.get("strapline") or header.get("subtitle") or header.get("label"))


def _section_endpoint(section: dict[str, Any]) -> dict[str, str]:
    header = _section_header(section)
    endpoint: dict[str, Any] = {}
    for path in (
        ("moreContentButton", "buttonRenderer", "navigationEndpoint"),
        ("navigationEndpoint",),
        ("bottomEndpoint",),
    ):
        candidate = _dig(header, *path)
        if isinstance(candidate, dict):
            endpoint = candidate
            break
    browse = endpoint.get("browseEndpoint") if isinstance(endpoint, dict) else None
    if not isinstance(browse, dict):
        return {"browseId": "", "params": ""}
    return {
        "browseId": _text(browse.get("browseId")),
        "params": _text(browse.get("params")),
    }


def _section_contents(section: dict[str, Any]) -> list[dict[str, Any]]:
    for key in ("contents", "items"):
        value = section.get(key)
        if isinstance(value, list):
            return [entry for entry in value if isinstance(entry, dict)]
    return []


def _parse_section(section: dict[str, Any], section_type: str) -> dict[str, Any] | None:
    title = _section_title(section)
    if not title:
        return None

    items: list[dict[str, Any]] = []
    for entry in _section_contents(section):
        pair = _renderer_from_entry(entry)
        if pair is None:
            continue
        renderer_key, renderer = pair
        item = _renderer_item(renderer, renderer_key, title)
        if item is not None:
            items.append(item)
    if not items:
        return None

    header = _section_header(section)
    return {
        "title": title,
        "strapline": _section_label(section),
        "thumbnailRenderer": header.get("thumbnail") or header.get("thumbnailRenderer") or {},
        "endpoint": _section_endpoint(section),
        "sectionType": section_type,
        "contents": items,
    }


def _top_level_contents(source: Any) -> list[dict[str, Any]]:
    if isinstance(source, list):
        return [entry for entry in source if isinstance(entry, dict)]
    if not isinstance(source, dict):
        return []

    continuation = source.get("sectionListContinuation")
    if isinstance(continuation, dict):
        return _section_contents(continuation)

    continuation_contents = source.get("continuationContents")
    if isinstance(continuation_contents, dict):
        continuation = continuation_contents.get("sectionListContinuation")
        if isinstance(continuation, dict):
            return _section_contents(continuation)

    # Prefer the section list with chips or the greatest number of contents.
    candidates: list[tuple[int, list[dict[str, Any]]]] = []
    for node in _walk_dicts(source):
        renderer = node.get("sectionListRenderer")
        if not isinstance(renderer, dict):
            continue
        contents = _section_contents(renderer)
        header = renderer.get("header") if isinstance(renderer.get("header"), dict) else {}
        has_chips = any("chipCloudRenderer" in candidate for candidate in _walk_dicts(header))
        candidates.append(((1000 if has_chips else 0) + len(contents), contents))
    if candidates:
        return max(candidates, key=lambda candidate: candidate[0])[1]

    return _section_contents(source)


def parse_inner_tube_home_sections(source: Any) -> list[dict[str, Any]]:
    """Convert raw Home/continuation payloads into rows for ``build_structured_home``.

    Sections and items retain the exact order of the raw response. Unknown renderer
    experiments are skipped individually instead of causing the whole Home to fail.
    """

    rows: list[dict[str, Any]] = []
    for content in _top_level_contents(source):
        parsed = False
        for key in SECTION_KEYS:
            section = content.get(key)
            if not isinstance(section, dict):
                continue
            row = _parse_section(section, key)
            if row is not None:
                rows.append(row)
            parsed = True
            break
        if parsed:
            continue

        # Home experiments occasionally wrap shelves in itemSectionRenderer.
        item_section = content.get("itemSectionRenderer")
        if isinstance(item_section, dict):
            rows.extend(parse_inner_tube_home_sections(item_section.get("contents") or []))
    return rows


def missing_artwork_by_section(rows: Any) -> list[tuple[str, int, int]]:
    """Return ``(section, missing, total)`` diagnostics for real-account smoke tests."""

    output: list[tuple[str, int, int]] = []
    for row in rows or []:
        if not isinstance(row, dict):
            continue
        items = [item for item in row.get("contents") or [] if isinstance(item, dict)]
        if not items:
            continue
        missing = sum(
            1
            for item in items
            if not _thumbnail_candidates(item)
            and not VIDEO_ID_PATTERN.fullmatch(_text(item.get("videoId")))
        )
        if missing:
            output.append((_text(row.get("title")) or "Untitled", missing, len(items)))
    return output
