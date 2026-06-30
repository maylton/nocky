#!/usr/bin/env python3
"""Integrate the direct raw InnerTube Home parser into the YouTube helper."""

from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match in {path}, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "helpers/nocky_youtube.py",
    '''from nocky_youtube_feed import (
    build_library_overview,
    build_structured_home,
    enrich_inner_tube_home_rows,
    extract_inner_tube_home_chips,
    find_inner_tube_home_section_list,
    load_cached_page,
    save_cached_page,
)

from nocky_stream_clients import (
''',
    '''from nocky_youtube_feed import (
    build_library_overview,
    build_structured_home,
    extract_inner_tube_home_chips,
    find_inner_tube_home_section_list,
    load_cached_page,
    save_cached_page,
)
from nocky_youtube_innertube_home import (
    missing_artwork_by_section,
    parse_inner_tube_home_sections,
)

from nocky_stream_clients import (
''',
    "direct Home parser imports",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''def _home_feed_cache_path() -> Path:
    return _cache_dir() / "home-feed-v3.json"
''',
    '''def _home_feed_cache_path() -> Path:
    return _cache_dir() / "home-feed-v4.json"
''',
    "Home V4 cache path",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''def _inner_tube_home_rows(
    client: Any,
    params: str,
    limit: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    if ytmusic_parse_mixed_content is None:
        raise RuntimeError("The installed ytmusicapi version cannot parse Home responses")
    body, response = _inner_tube_home_response(client, params)
    section_list = find_inner_tube_home_section_list(response)
    contents = section_list.get("contents") if isinstance(section_list.get("contents"), list) else []
    if not contents:
        raise RuntimeError("YouTube Music did not return Home sections")

    rows = enrich_inner_tube_home_rows(
        list(ytmusic_parse_mixed_content(contents) or []),
        contents,
    )
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
        continuation_contents = continuation_response.get("continuationContents")
        if not isinstance(continuation_contents, dict):
            break
        current = continuation_contents.get("sectionListContinuation")
        if not isinstance(current, dict):
            break
        raw_contents = current.get("contents") if isinstance(current.get("contents"), list) else []
        if not raw_contents:
            break
        parsed = list(ytmusic_parse_mixed_content(raw_contents) or [])
        enriched = enrich_inner_tube_home_rows(parsed, raw_contents)
        if not enriched:
            break
        rows.extend(enriched)
    return rows, response
''',
    '''def _inner_tube_home_rows(
    client: Any,
    params: str,
    limit: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    body, response = _inner_tube_home_response(client, params)
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
    return rows, response
''',
    "direct raw Home row parsing",
)

replace_once(
    "helpers/nocky_youtube_feed.py",
    "CONTRACT_VERSION = 3\n",
    "CONTRACT_VERSION = 4\n",
    "Home feed contract V4",
)

replace_once(
    "tests/test_youtube_feed.py",
    'self.assertEqual(page["version"], 3)',
    'self.assertEqual(page["version"], 4)',
    "Home feed version test",
)

print("Direct InnerTube Home parser integrated")
