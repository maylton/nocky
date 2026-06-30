#!/usr/bin/env python3
"""Apply raw InnerTube Home artwork enrichment inspired by nocky-android."""

from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match in {path}, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "helpers/nocky_youtube_feed.py",
    "CONTRACT_VERSION = 2\n",
    "CONTRACT_VERSION = 3\n",
    "Home feed contract version",
)

replace_once(
    "helpers/nocky_youtube_feed.py",
    '''def _chips(source: Any) -> list[dict[str, str]]:
''',
    '''def _dig(value: Any, *path: str) -> Any:
    current = value
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def _normalized_identity(value: Any) -> str:
    return re.sub(r"\\s+", " ", _text(value).casefold()).strip()


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
''',
    "Raw InnerTube artwork enrichment",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''    build_library_overview,
    build_structured_home,
    extract_inner_tube_home_chips,
''',
    '''    build_library_overview,
    build_structured_home,
    enrich_inner_tube_home_rows,
    extract_inner_tube_home_chips,
''',
    "Import raw Home enrichment",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''        from ytmusicapi.continuations import get_continuations as ytmusic_get_continuations
        from ytmusicapi.parsers.browsing import parse_mixed_content as ytmusic_parse_mixed_content
    except Exception:
        ytmusic_get_continuations = None
        ytmusic_parse_mixed_content = None
''',
    '''        from ytmusicapi.continuations import get_continuation_params as ytmusic_get_continuation_params
        from ytmusicapi.parsers.browsing import parse_mixed_content as ytmusic_parse_mixed_content
    except Exception:
        ytmusic_get_continuation_params = None
        ytmusic_parse_mixed_content = None
''',
    "Import raw continuation params",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''    ytmusic_playlist_parsers = None
    ytmusic_get_continuations = None
    ytmusic_parse_mixed_content = None
''',
    '''    ytmusic_playlist_parsers = None
    ytmusic_get_continuation_params = None
    ytmusic_parse_mixed_content = None
''',
    "Fallback raw continuation params",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''def _home_feed_cache_path() -> Path:
    return _cache_dir() / "home-feed-v2.json"
''',
    '''def _home_feed_cache_path() -> Path:
    return _cache_dir() / "home-feed-v3.json"
''',
    "Invalidate legacy Home artwork cache",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''def _inner_tube_home_rows(
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
''',
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
    "Preserve raw Home renderers across continuations",
)

replace_once(
    "helpers/nocky_youtube.py",
    '''        if params:
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
''',
    '''        rows, raw_response = _inner_tube_home_rows(client, params, fetch_limit)
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
''',
    "Use raw InnerTube Home for every feed page",
)

replace_once(
    "tests/test_youtube_feed.py",
    '''    build_library_overview,
    build_structured_home,
    extract_inner_tube_home_chips,
''',
    '''    build_library_overview,
    build_structured_home,
    enrich_inner_tube_home_rows,
    extract_inner_tube_home_chips,
''',
    "Import enrichment tests",
)

replace_once(
    "tests/test_youtube_feed.py",
    '''    def test_extracts_nested_renderer_artwork(self) -> None:
''',
    '''    def test_enriches_two_row_cropped_artwork_and_watch_identity(self) -> None:
        parsed = [{"title": "Escolha a dedo", "contents": [{"title": "Vanish Into You"}]}]
        raw = {
            "contents": {
                "singleColumnBrowseResultsRenderer": {
                    "tabs": [{
                        "tabRenderer": {
                            "content": {
                                "sectionListRenderer": {
                                    "contents": [{
                                        "musicCarouselShelfRenderer": {
                                            "header": {
                                                "musicCarouselShelfBasicHeaderRenderer": {
                                                    "title": {"runs": [{"text": "Escolha a dedo"}]}
                                                }
                                            },
                                            "contents": [{
                                                "musicTwoRowItemRenderer": {
                                                    "title": {"runs": [{"text": "Vanish Into You"}]},
                                                    "navigationEndpoint": {
                                                        "watchEndpoint": {"videoId": "abc123DEF45"}
                                                    },
                                                    "thumbnailRenderer": {
                                                        "croppedSquareThumbnailRenderer": {
                                                            "thumbnail": {
                                                                "thumbnails": [{
                                                                    "url": "https://lh3.googleusercontent.com/cropped=s320",
                                                                    "width": 320,
                                                                    "height": 320,
                                                                }]
                                                            }
                                                        }
                                                    },
                                                }
                                            }],
                                        }
                                    }]
                                }
                            }
                        }
                    }]
                }
            }
        }
        enriched = enrich_inner_tube_home_rows(parsed, raw)
        page = build_structured_home(enriched, section_limit=1)
        item = page["sections"][0]["items"][0]
        self.assertEqual(item["video_id"], "abc123DEF45")
        self.assertIn("cropped=s1200", item["thumbnail_url"])

    def test_enriches_responsive_overlay_with_animated_backup_artwork(self) -> None:
        parsed = [{"title": "Apresentações ao vivo", "contents": [{"title": "Mandinga"}]}]
        raw_contents = [{
            "musicCarouselShelfRenderer": {
                "header": {
                    "musicCarouselShelfBasicHeaderRenderer": {
                        "title": {"runs": [{"text": "Apresentações ao vivo"}]}
                    }
                },
                "contents": [{
                    "musicResponsiveListItemRenderer": {
                        "flexColumns": [{
                            "musicResponsiveListItemFlexColumnRenderer": {
                                "text": {"runs": [{"text": "Mandinga"}]}
                            }
                        }],
                        "overlay": {
                            "musicItemThumbnailOverlayRenderer": {
                                "content": {
                                    "musicPlayButtonRenderer": {
                                        "playNavigationEndpoint": {
                                            "watchEndpoint": {"videoId": "ZYX987abc_1"}
                                        }
                                    }
                                }
                            }
                        },
                        "thumbnail": {
                            "musicAnimatedThumbnailRenderer": {
                                "animatedThumbnail": {
                                    "thumbnails": [{
                                        "url": "https://example.invalid/animated.webp",
                                        "width": 640,
                                        "height": 640,
                                    }]
                                },
                                "backupRenderer": {
                                    "thumbnail": {
                                        "thumbnails": [{
                                            "url": "https://lh3.googleusercontent.com/live=s480",
                                            "width": 480,
                                            "height": 480,
                                        }]
                                    }
                                },
                            }
                        },
                    }
                }],
            }
        }]
        enriched = enrich_inner_tube_home_rows(parsed, raw_contents)
        page = build_structured_home(enriched, section_limit=1)
        item = page["sections"][0]["items"][0]
        self.assertEqual(item["video_id"], "ZYX987abc_1")
        self.assertIn("live=s1200", item["thumbnail_url"])
        self.assertNotIn("animated", item["thumbnail_url"])

    def test_enrichment_matches_reordered_items_by_title(self) -> None:
        parsed = [{
            "title": "Covers e remixes",
            "contents": [
                {"title": "Diver", "videoId": "abcdefghijk"},
                {"title": "Toumei Datta Sekai", "videoId": "lmnopqrstuv"},
            ],
        }]
        raw_contents = [{
            "musicCarouselShelfRenderer": {
                "header": {
                    "musicCarouselShelfBasicHeaderRenderer": {
                        "title": {"runs": [{"text": "Covers e remixes"}]}
                    }
                },
                "contents": [
                    {
                        "musicTwoRowItemRenderer": {
                            "title": {"runs": [{"text": "Toumei Datta Sekai"}]},
                            "navigationEndpoint": {"watchEndpoint": {"videoId": "lmnopqrstuv"}},
                            "thumbnailRenderer": {
                                "musicThumbnailRenderer": {
                                    "thumbnail": {"thumbnails": [{
                                        "url": "https://lh3.googleusercontent.com/toumei=s200",
                                        "width": 200,
                                        "height": 200,
                                    }]}
                                }
                            },
                        }
                    },
                    {
                        "musicTwoRowItemRenderer": {
                            "title": {"runs": [{"text": "Diver"}]},
                            "navigationEndpoint": {"watchEndpoint": {"videoId": "abcdefghijk"}},
                            "thumbnailRenderer": {
                                "musicThumbnailRenderer": {
                                    "thumbnail": {"thumbnails": [{
                                        "url": "https://lh3.googleusercontent.com/diver=s200",
                                        "width": 200,
                                        "height": 200,
                                    }]}
                                }
                            },
                        }
                    },
                ],
            }
        }]
        enriched = enrich_inner_tube_home_rows(parsed, raw_contents)
        page = build_structured_home(enriched, section_limit=1)
        self.assertIn("diver=s1200", page["sections"][0]["items"][0]["thumbnail_url"])
        self.assertIn("toumei=s1200", page["sections"][0]["items"][1]["thumbnail_url"])

    def test_extracts_nested_renderer_artwork(self) -> None:
''',
    "Raw renderer artwork tests",
)

print("Raw InnerTube Home artwork patch applied")
