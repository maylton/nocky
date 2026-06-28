#!/usr/bin/env python3
from pathlib import Path


def replace(path: str, old: str, new: str, expected: int = 1) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(
            f"{path}: expected {expected} occurrence(s), found {count}: {old[:140]!r}"
        )
    file.write_text(text.replace(old, new), encoding="utf-8")


replace(
    "helpers/nocky_youtube.py",
    '''from nocky_youtube_feed import (
    build_library_overview,
    build_structured_home,
    load_cached_page,
    save_cached_page,
)
''',
    '''from nocky_youtube_feed import (
    build_library_overview,
    build_structured_home,
    extract_inner_tube_home_chips,
    find_inner_tube_home_section_list,
    load_cached_page,
    save_cached_page,
)
''',
)

replace(
    "helpers/nocky_youtube.py",
    '''    from ytmusicapi import YTMusic
    from ytmusicapi.exceptions import YTMusicServerError, YTMusicUserError
    try:
        from ytmusicapi.setup import setup as ytmusicapi_setup
''',
    '''    from ytmusicapi import YTMusic
    from ytmusicapi.exceptions import YTMusicServerError, YTMusicUserError
    try:
        from ytmusicapi.continuations import get_continuations as ytmusic_get_continuations
        from ytmusicapi.parsers.browsing import parse_mixed_content as ytmusic_parse_mixed_content
    except Exception:
        ytmusic_get_continuations = None
        ytmusic_parse_mixed_content = None
    try:
        from ytmusicapi.setup import setup as ytmusicapi_setup
''',
)

replace(
    "helpers/nocky_youtube.py",
    '''    YTMusicServerError = RuntimeError
    YTMusicUserError = RuntimeError
    ytmusicapi_setup = None
    IMPORT_ERROR = error
''',
    '''    YTMusicServerError = RuntimeError
    YTMusicUserError = RuntimeError
    ytmusic_get_continuations = None
    ytmusic_parse_mixed_content = None
    ytmusicapi_setup = None
    IMPORT_ERROR = error
''',
)

replace(
    "helpers/nocky_youtube_feed.py",
    '''def _chips(source: Any) -> list[dict[str, str]]:
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
''',
    '''def _walk_dicts(value: Any):
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
''',
)

replace(
    "helpers/nocky_youtube.py",
    '''def command_home_v2(payload: dict[str, Any]) -> dict[str, Any]:
''',
    '''def _inner_tube_home_response(client: Any, params: str = "") -> tuple[dict[str, Any], dict[str, Any]]:
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
    if ytmusic_parse_mixed_content is None:
        raise RuntimeError("The installed ytmusicapi version cannot parse filtered Home responses")
    body, response = _inner_tube_home_response(client, params)
    section_list = find_inner_tube_home_section_list(response)
    contents = section_list.get("contents") if isinstance(section_list.get("contents"), list) else []
    if not contents:
        raise RuntimeError("YouTube Music did not return filtered Home sections")
    rows = list(ytmusic_parse_mixed_content(contents) or [])
    remaining = max(0, limit - len(rows))
    if (
        remaining > 0
        and section_list.get("continuations")
        and ytmusic_get_continuations is not None
    ):
        sender = getattr(client, "_send_request")

        def request_func(additional_params: dict[str, Any]):
            return sender("browse", body, additional_params)

        rows.extend(
            ytmusic_get_continuations(
                section_list,
                "sectionListContinuation",
                remaining,
                request_func,
                ytmusic_parse_mixed_content,
            )
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
''',
)

old_command = '''    try:
        fetch_limit = max(12, min(36, offset + section_limit + 1))
        if params:
            try:
                rows = client.get_home(limit=fetch_limit, params=params)
            except TypeError:
                try:
                    rows = client.get_home(params=params)
                except TypeError as error:
                    raise RuntimeError(
                        "The installed ytmusicapi version does not support YouTube Home chip params"
                    ) from error
        else:
            try:
                rows = client.get_home(limit=fetch_limit)
            except TypeError:
                rows = client.get_home()
        page = build_structured_home(
            rows,
            offset=offset,
            section_limit=section_limit,
            selected_chip_params=params,
        )
        if params and not page.get("chips"):
            root = load_cached_page(
                _home_feed_cache_path(),
                _feed_cache_key("home", "", section_limit, ""),
                allow_stale=True,
            )
            if root is not None:
                page["chips"] = root.get("chips") or []
        save_cached_page(_home_feed_cache_path(), cache_key, page)
        return page
'''
new_command = '''    try:
        fetch_limit = max(12, min(36, offset + section_limit + 1))
        chips = _cached_root_home_chips(section_limit)
        if params:
            rows, raw_response = _inner_tube_home_rows(client, params, fetch_limit)
            if not chips:
                chips = extract_inner_tube_home_chips(raw_response)
            if not chips:
                try:
                    _body, root_response = _inner_tube_home_response(client)
                    chips = extract_inner_tube_home_chips(root_response)
                except Exception as chip_error:
                    print(
                        f"Nocky YouTube root chips unavailable: {chip_error}",
                        file=sys.stderr,
                    )
        else:
            try:
                rows = client.get_home(limit=fetch_limit)
            except TypeError:
                rows = client.get_home()
            if offset == 0 or not chips:
                try:
                    _body, raw_response = _inner_tube_home_response(client)
                    chips = extract_inner_tube_home_chips(raw_response) or chips
                except Exception as chip_error:
                    print(
                        f"Nocky YouTube Home chips unavailable: {chip_error}",
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
        save_cached_page(_home_feed_cache_path(), cache_key, page)
        return page
'''
replace("helpers/nocky_youtube.py", old_command, new_command)

replace(
    "tests/test_youtube_feed.py",
    '''    build_library_overview,
    build_structured_home,
    load_cached_page,
    save_cached_page,
''',
    '''    build_library_overview,
    build_structured_home,
    extract_inner_tube_home_chips,
    find_inner_tube_home_section_list,
    load_cached_page,
    save_cached_page,
''',
)

replace(
    "tests/test_youtube_feed.py",
    '''    def test_deduplicates_items_without_flattening_sections(self) -> None:
''',
    '''    def test_extracts_real_inner_tube_chip_cloud(self) -> None:
        response = {
            "contents": {
                "singleColumnBrowseResultsRenderer": {
                    "tabs": [
                        {
                            "tabRenderer": {
                                "content": {
                                    "sectionListRenderer": {
                                        "header": {
                                            "chipCloudRenderer": {
                                                "chips": [
                                                    {
                                                        "chipCloudChipRenderer": {
                                                            "isSelected": True,
                                                            "text": {"runs": [{"text": "All"}]},
                                                            "navigationEndpoint": {
                                                                "browseEndpoint": {
                                                                    "browseId": "FEmusic_home"
                                                                }
                                                            },
                                                        }
                                                    },
                                                    {
                                                        "chipCloudChipRenderer": {
                                                            "isSelected": False,
                                                            "text": {"runs": [{"text": "Energize"}]},
                                                            "navigationEndpoint": {
                                                                "browseEndpoint": {
                                                                    "browseId": "FEmusic_home",
                                                                    "params": "mood-energy",
                                                                }
                                                            },
                                                        }
                                                    },
                                                    {
                                                        "chipCloudChipRenderer": {
                                                            "isSelected": False,
                                                            "text": {"runs": [{"text": "Relax"}]},
                                                            "navigationEndpoint": {
                                                                "browseEndpoint": {
                                                                    "browseId": "FEmusic_home",
                                                                    "params": "mood-relax",
                                                                }
                                                            },
                                                        }
                                                    },
                                                ]
                                            }
                                        },
                                        "contents": [{"musicCarouselShelfRenderer": {}}],
                                    }
                                }
                            }
                        }
                    ]
                }
            }
        }
        section_list = find_inner_tube_home_section_list(response)
        self.assertIn("header", section_list)
        chips = extract_inner_tube_home_chips(response)
        self.assertEqual([chip["title"] for chip in chips], ["Energize", "Relax"])
        self.assertEqual([chip["params"] for chip in chips], ["mood-energy", "mood-relax"])
        self.assertEqual(chips[0]["browse_id"], "FEmusic_home")

    def test_deduplicates_items_without_flattening_sections(self) -> None:
''',
)

Path("tests/test_youtube_home_chips.py").write_text(
    '''from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube as helper


class FakeClient:
    def __init__(self, response):
        self.response = response
        self.calls = []

    def _send_request(self, endpoint, body, additional_params=None):
        self.calls.append((endpoint, dict(body), additional_params))
        return self.response


class YouTubeHomeChipTests(unittest.TestCase):
    def test_filtered_home_uses_web_browse_params_and_parser(self):
        response = {
            "contents": {
                "singleColumnBrowseResultsRenderer": {
                    "tabs": [
                        {
                            "tabRenderer": {
                                "content": {
                                    "sectionListRenderer": {
                                        "contents": [{"musicCarouselShelfRenderer": {}}]
                                    }
                                }
                            }
                        }
                    ]
                }
            }
        }
        client = FakeClient(response)
        parsed = [{"title": "Energy", "contents": [{"title": "Song"}]}]
        with (
            mock.patch.object(helper, "ytmusic_parse_mixed_content", return_value=parsed),
            mock.patch.object(helper, "ytmusic_get_continuations", None),
        ):
            rows, raw = helper._inner_tube_home_rows(client, "mood-energy", 6)

        self.assertEqual(rows, parsed)
        self.assertIs(raw, response)
        self.assertEqual(client.calls[0][0], "browse")
        self.assertEqual(
            client.calls[0][1],
            {"browseId": "FEmusic_home", "params": "mood-energy"},
        )

    def test_filtered_home_continuation_reuses_the_same_params(self):
        response = {
            "contents": {
                "singleColumnBrowseResultsRenderer": {
                    "tabs": [
                        {
                            "tabRenderer": {
                                "content": {
                                    "sectionListRenderer": {
                                        "contents": [{"musicCarouselShelfRenderer": {}}],
                                        "continuations": [{"nextContinuationData": {"continuation": "next"}}],
                                    }
                                }
                            }
                        }
                    ]
                }
            }
        }
        client = FakeClient(response)

        def continuations(section_list, key, remaining, request_func, parser):
            request_func({"continuation": "next"})
            self.assertEqual(key, "sectionListContinuation")
            self.assertEqual(remaining, 5)
            return [{"title": "More", "contents": []}]

        with (
            mock.patch.object(
                helper,
                "ytmusic_parse_mixed_content",
                return_value=[{"title": "First", "contents": []}],
            ),
            mock.patch.object(helper, "ytmusic_get_continuations", side_effect=continuations),
        ):
            rows, _raw = helper._inner_tube_home_rows(client, "mood-relax", 6)

        self.assertEqual([row["title"] for row in rows], ["First", "More"])
        self.assertEqual(client.calls[1][1]["params"], "mood-relax")
        self.assertEqual(client.calls[1][2], {"continuation": "next"})


if __name__ == "__main__":
    unittest.main()
''',
    encoding="utf-8",
)

replace(
    "docs/YOUTUBE_LIBRARY_ROADMAP.md",
    '| 10. Android-parity YouTube Home organization | Planned | after Phase 9 |\n',
    '| 10. Android-parity YouTube Home organization | Implemented; real-chip manual validation pending | PR #46 |\n',
)

replace(
    "docs/YOUTUBE_LIBRARY_ROADMAP.md",
    '''Planned deliverables:

- Render YouTube feed chips at the top of the YouTube Home.
- Selecting a chip loads the corresponding feed params and preserves the chip
  list from the root feed.
- Render each YouTube section using the returned header title, optional label,
  thumbnail shape hint and endpoint.
- Keep section continuation/load-more behavior tied to the YouTube endpoint.
- Treat Quick Picks as a feed/pinned online section, not as a local history
  filter.
- Keep Local Home personalized sections separate from the YouTube Home.
- Add fixture tests for chip selection, section order and header preservation.

Acceptance criteria:
''',
    '''Implemented in PR #46:

- Render YouTube feed chips at the top of the YouTube Home.
- Extract localized chip titles and browse params from the Web InnerTube Home
  response before `ytmusicapi` flattens the page into section rows.
- Selecting a chip loads the corresponding `FEmusic_home` browse params and
  preserves the chip list from the root feed.
- Render each YouTube section using the returned header title, optional label,
  thumbnail shape hint and endpoint.
- Keep section continuation/load-more behavior tied to the selected YouTube
  Home params.
- Treat Quick Picks as a feed/pinned online section, not as a local history
  filter.
- Keep Local Home personalized sections separate from the YouTube Home.
- Add fixture tests for chip extraction, selection request bodies,
  continuation params, section order and header preservation.

Manual validation pending:

- Confirm the connected account returns more than the root **Tudo** chip.
- Select every returned chip and confirm the section feed changes.
- Return to **Tudo** and confirm the root feed and chip list are restored.
- Confirm filtered load-more requests remain on the selected chip.

Acceptance criteria:
''',
)
