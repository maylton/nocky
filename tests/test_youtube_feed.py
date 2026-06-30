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
    enrich_inner_tube_home_rows,
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
        self.assertEqual(page["version"], 4)
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

    def test_enriches_two_row_cropped_artwork_and_watch_identity(self) -> None:
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
