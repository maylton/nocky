#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HELPERS = ROOT / "helpers"
sys.path.insert(0, str(HELPERS))

SPEC = importlib.util.spec_from_file_location("nocky_youtube", HELPERS / "nocky_youtube.py")
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class SearchPaginationHelperTests(unittest.TestCase):
    def test_finds_initial_and_append_action_renderers(self) -> None:
        initial = {
            "contents": {
                "tabbedSearchResultsRenderer": {
                    "tabs": [
                        {
                            "tabRenderer": {
                                "content": {
                                    "sectionListRenderer": {
                                        "contents": [
                                            {
                                                "musicShelfRenderer": {
                                                    "contents": [{"first": True}]
                                                }
                                            }
                                        ]
                                    }
                                }
                            }
                        }
                    ]
                }
            }
        }
        self.assertEqual(
            MODULE._search_page_renderer(initial, ""),
            {"contents": [{"first": True}]},
        )

        continuation = {
            "onResponseReceivedActions": [
                {
                    "appendContinuationItemsAction": {
                        "continuationItems": [{"next": True}]
                    }
                }
            ]
        }
        self.assertEqual(
            MODULE._search_page_renderer(continuation, "opaque"),
            {"contents": [{"next": True}]},
        )

    def test_builds_next_continuation_from_classic_renderer(self) -> None:
        original = MODULE.ytmusic_get_continuation_params
        MODULE.ytmusic_get_continuation_params = lambda _renderer: (
            "&ctoken=next&continuation=next"
        )
        try:
            value = MODULE._search_page_continuation(
                {"continuations": [{"nextContinuationData": {}}]}
            )
        finally:
            MODULE.ytmusic_get_continuation_params = original
        self.assertEqual(value, "&ctoken=next&continuation=next")

    def test_parses_only_responsive_search_rows(self) -> None:
        original = MODULE.ytmusic_parse_search_results
        MODULE.ytmusic_parse_search_results = lambda rows, result_type, _category: [
            {
                "resultType": result_type,
                "title": "Starlight",
                "videoId": "abcdefghijk",
                "artists": [{"name": "Muse"}],
            }
            for _row in rows
        ]
        try:
            items = MODULE._search_page_items(
                {
                    "contents": [
                        {"musicResponsiveListItemRenderer": {}},
                        {"continuationItemRenderer": {}},
                    ]
                },
                "song",
            )
        finally:
            MODULE.ytmusic_parse_search_results = original
        self.assertEqual(len(items), 1)
        self.assertEqual(items[0]["title"], "Starlight")


if __name__ == "__main__":
    unittest.main()
