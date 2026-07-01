from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

from nocky_youtube_feed import build_structured_home  # noqa: E402
from nocky_youtube_innertube_home import (  # noqa: E402
    missing_artwork_by_section,
    parse_inner_tube_home_sections,
)


class DirectInnerTubeHomeParserTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.fixture = json.loads(
            (ROOT / "tests" / "fixtures" / "youtube_home_innertube_raw.json").read_text(
                encoding="utf-8"
            )
        )
        cls.rows = parse_inner_tube_home_sections(cls.fixture)
        cls.page = build_structured_home(cls.rows, section_limit=12)

    def test_preserves_raw_section_order(self) -> None:
        self.assertEqual(
            [row["title"] for row in self.rows],
            [
                "Álbuns para você",
                "Escolha a dedo",
                "Em alta nos Shorts",
                "Mixes longos",
                "Apresentações ao vivo",
                "Seus episódios",
            ],
        )
        self.assertEqual(
            [section["title"] for section in self.page["sections"]],
            [row["title"] for row in self.rows],
        )

    def test_parses_album_identity_and_standard_artwork(self) -> None:
        item = self.page["sections"][0]["items"][0]
        self.assertEqual(item["result_type"], "album")
        self.assertEqual(item["browse_id"], "MPREb_album001")
        self.assertEqual(item["artist"], "Dua Lipa")
        self.assertIn("album=s1200", item["thumbnail_url"])

    def test_parses_responsive_quick_pick_with_cropped_artwork(self) -> None:
        item = self.page["sections"][1]["items"][0]
        self.assertEqual(item["result_type"], "song")
        self.assertEqual(item["video_id"], "abc123DEF45")
        self.assertEqual(item["artist"], "Lady Gaga")
        self.assertEqual(item["album"], "MAYHEM")
        self.assertEqual(item["duration_seconds"], 244)
        self.assertIn("vanish=s1200", item["thumbnail_url"])

    def test_parses_shorts_view_model_as_video(self) -> None:
        item = self.page["sections"][2]["items"][0]
        self.assertEqual(item["result_type"], "video")
        self.assertEqual(item["video_id"], "shortsID01A")
        self.assertEqual(item["artist"], "Gilson")
        self.assertIn("hq720.jpg", item["thumbnail_url"])

    def test_prefers_static_backup_for_animated_mix_artwork(self) -> None:
        item = self.page["sections"][3]["items"][0]
        self.assertEqual(item["result_type"], "playlist")
        self.assertEqual(item["browse_id"], "RD_mixlong001")
        self.assertIn("longmix=s1200", item["thumbnail_url"])
        self.assertNotIn("animated", item["thumbnail_url"])

    def test_parses_live_video_and_static_backup(self) -> None:
        item = self.page["sections"][4]["items"][0]
        self.assertEqual(item["result_type"], "video")
        self.assertEqual(item["video_id"], "liveVideo01")
        self.assertIn("live=s1200", item["thumbnail_url"])

    def test_parses_multi_row_episode(self) -> None:
        item = self.page["sections"][5]["items"][0]
        self.assertEqual(item["result_type"], "episode")
        self.assertEqual(item["video_id"], "episodeID01")
        self.assertEqual(item["duration_seconds"], 1930)
        self.assertIn("episode=s1200", item["thumbnail_url"])

    def test_fixture_has_no_missing_artwork(self) -> None:
        self.assertEqual(missing_artwork_by_section(self.rows), [])

    def test_unknown_renderer_is_skipped_without_dropping_section(self) -> None:
        source = [
            {
                "musicCarouselShelfRenderer": {
                    "header": {
                        "musicCarouselShelfBasicHeaderRenderer": {
                            "title": {"runs": [{"text": "Mixed shelf"}]}
                        }
                    },
                    "contents": [
                        {"unknownExperimentalRenderer": {"title": "Ignore me"}},
                        {
                            "musicTwoRowItemRenderer": {
                                "title": {"runs": [{"text": "Playable"}]},
                                "subtitle": {"runs": [{"text": "Artist"}]},
                                "navigationEndpoint": {
                                    "watchEndpoint": {"videoId": "abcdefghijk"}
                                },
                                "thumbnailRenderer": {
                                    "musicThumbnailRenderer": {
                                        "thumbnail": {
                                            "thumbnails": [
                                                {
                                                    "url": "https://lh3.googleusercontent.com/playable=s240",
                                                    "width": 240,
                                                    "height": 240,
                                                }
                                            ]
                                        }
                                    }
                                },
                            }
                        },
                    ],
                }
            }
        ]
        rows = parse_inner_tube_home_sections(source)
        self.assertEqual(len(rows), 1)
        self.assertEqual(len(rows[0]["contents"]), 1)
        self.assertEqual(rows[0]["contents"][0]["title"], "Playable")


if __name__ == "__main__":
    unittest.main()
