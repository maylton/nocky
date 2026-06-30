from __future__ import annotations

import json
import tempfile
import time
import unittest
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

from nocky_youtube_feed import (  # noqa: E402
    build_library_overview,
    build_structured_home,
    extract_inner_tube_home_chips,
    find_inner_tube_home_section_list,
    load_cached_page,
    save_cached_page,
)


class StructuredHomeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.fixture = json.loads(
            (ROOT / "tests" / "fixtures" / "youtube_home.json").read_text(encoding="utf-8")
        )

    def test_preserves_sections_order_and_chips(self) -> None:
        page = build_structured_home(
            self.fixture,
            section_limit=10,
            selected_chip_params="mood-energy",
        )
        self.assertEqual(page["version"], 2)
        self.assertEqual(page["selected_chip_params"], "mood-energy")
        self.assertEqual([section["title"] for section in page["sections"]], [
            "Quick picks",
            "Albums for you",
            "Your shows",
        ])
        self.assertEqual([chip["title"] for chip in page["chips"]], ["Energize", "Relax"])

    def test_extracts_real_inner_tube_chip_cloud(self) -> None:
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

    def test_extracts_nested_renderer_artwork(self) -> None:
        source = {
            "sections": [
                {
                    "title": "Albums for you",
                    "contents": [
                        {
                            "resultType": "album",
                            "title": "Nested artwork",
                            "browseId": "MPREnested",
                            "thumbnailRenderer": {
                                "musicThumbnailRenderer": {
                                    "thumbnail": {
                                        "thumbnails": [
                                            {
                                                "url": "https://lh3.googleusercontent.com/example=s60",
                                                "width": 60,
                                                "height": 60,
                                            },
                                            {
                                                "url": "https://lh3.googleusercontent.com/example=s240",
                                                "width": 240,
                                                "height": 240,
                                            },
                                        ]
                                    }
                                }
                            },
                        }
                    ],
                }
            ]
        }
        page = build_structured_home(source, section_limit=1)
        thumbnail = page["sections"][0]["items"][0]["thumbnail_url"]
        self.assertIn("example=s1200", thumbnail)

    def test_uses_video_thumbnail_when_playlist_artwork_is_missing(self) -> None:
        source = {
            "sections": [
                {
                    "title": "Playlists",
                    "contents": [
                        {
                            "resultType": "playlist",
                            "title": "Fallback playlist",
                            "playlistId": "PL-fallback",
                            "videoId": "abcdefghijk",
                        }
                    ],
                }
            ]
        }
        page = build_structured_home(source, section_limit=1)
        item = page["sections"][0]["items"][0]
        self.assertEqual(
            item["thumbnail_url"],
            "https://i.ytimg.com/vi/abcdefghijk/hqdefault.jpg",
        )

    def test_deduplicates_items_without_flattening_sections(self) -> None:
        page = build_structured_home(self.fixture, section_limit=10)
        quick_picks = page["sections"][0]
        self.assertEqual(quick_picks["layout"], "quick_picks")
        self.assertEqual(len(quick_picks["items"]), 2)
        self.assertEqual(quick_picks["items"][0]["duration_seconds"], 200)

    def test_supports_podcast_and_episode_items(self) -> None:
        page = build_structured_home(self.fixture, section_limit=10)
        kinds = [item["result_type"] for item in page["sections"][2]["items"]]
        self.assertEqual(kinds, ["podcast", "episode"])

    def test_paginates_sections_with_synthetic_continuation(self) -> None:
        first = build_structured_home(self.fixture, section_limit=2)
        self.assertEqual(first["continuation"], "2")
        second = build_structured_home(self.fixture, offset=2, section_limit=2)
        self.assertEqual([section["title"] for section in second["sections"]], ["Your shows"])
        self.assertEqual(second["continuation"], "")

    def test_library_overview_uses_same_contract(self) -> None:
        song = build_structured_home(self.fixture, section_limit=1)["sections"][0]["items"][0]
        page = build_library_overview([("Songs", "list", [song])])
        self.assertEqual(page["sections"][0]["layout"], "list")
        self.assertEqual(page["sections"][0]["items"][0]["video_id"], "abc123DEF45")

    def test_cache_can_return_stale_page_after_network_failure(self) -> None:
        page = build_structured_home(self.fixture, section_limit=1)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "home-v2.json"
            save_cached_page(path, "home:0:1", page)
            payload = json.loads(path.read_text(encoding="utf-8"))
            payload["pages"]["home:0:1"]["saved_at"] = int(time.time()) - 100
            path.write_text(json.dumps(payload), encoding="utf-8")
            self.assertIsNone(load_cached_page(path, "home:0:1", max_age=10))
            stale = load_cached_page(path, "home:0:1", max_age=10, allow_stale=True)
            self.assertIsNotNone(stale)
            self.assertTrue(stale["stale"])


if __name__ == "__main__":
    unittest.main()
