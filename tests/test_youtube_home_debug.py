from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

from nocky_youtube_home_debug import (  # noqa: E402
    sanitize_home_payload,
    write_home_debug_dump,
)


class HomeRendererDebugTests(unittest.TestCase):
    def test_sanitizer_removes_tracking_and_query_parameters(self) -> None:
        source = {
            "trackingParams": "secret-tracking",
            "responseContext": {"visitorData": "secret-visitor"},
            "renderer": {
                "clickTrackingParams": "secret-click",
                "thumbnail": {
                    "url": "https://lh3.googleusercontent.com/cover=s240?token=secret#fragment",
                    "width": 240,
                    "height": 240,
                },
                "title": {"runs": [{"text": "Visible title"}]},
            },
        }

        sanitized = sanitize_home_payload(source)

        self.assertNotIn("trackingParams", sanitized)
        self.assertNotIn("responseContext", sanitized)
        renderer = sanitized["renderer"]
        self.assertNotIn("clickTrackingParams", renderer)
        self.assertEqual(renderer["title"]["runs"][0]["text"], "Visible title")
        self.assertEqual(
            renderer["thumbnail"]["url"],
            "https://lh3.googleusercontent.com/cover=s240",
        )

    def test_debug_dump_preserves_renderer_shape_and_parsed_summary(self) -> None:
        pages = [
            {
                "kind": "root",
                "response": {
                    "musicTwoRowItemRenderer": {
                        "title": {"runs": [{"text": "Example song"}]},
                        "thumbnailRenderer": {
                            "musicThumbnailRenderer": {
                                "thumbnail": {
                                    "thumbnails": [
                                        {
                                            "url": "https://lh3.googleusercontent.com/example=s240?x=1",
                                            "width": 240,
                                            "height": 240,
                                        }
                                    ]
                                }
                            }
                        },
                    }
                },
            }
        ]
        rows = [
            {
                "title": "Escolha a dedo",
                "contents": [
                    {
                        "title": "Example song",
                        "resultType": "song",
                        "rendererType": "musicTwoRowItemRenderer",
                        "videoId": "abcdefghijk",
                        "thumbnails": [
                            {
                                "url": "https://lh3.googleusercontent.com/example=s240?x=1",
                                "width": 240,
                                "height": 240,
                            }
                        ],
                    }
                ],
            }
        ]

        with tempfile.TemporaryDirectory() as temporary:
            path = write_home_debug_dump(
                Path(temporary) / "home.json",
                pages=pages,
                rows=rows,
                selected_params="chip-params",
            )
            payload = json.loads(path.read_text(encoding="utf-8"))

        self.assertTrue(payload["selectedChip"])
        self.assertEqual(payload["rendererCounts"]["musicTwoRowItemRenderer"], 1)
        self.assertEqual(payload["parsedSections"][0]["title"], "Escolha a dedo")
        self.assertEqual(
            payload["parsedSections"][0]["items"][0]["thumbnailUrls"][0],
            "https://lh3.googleusercontent.com/example=s240",
        )
        self.assertTrue(
            any("thumbnail" in entry["path"].casefold() for entry in payload["thumbnailPaths"])
        )


if __name__ == "__main__":
    unittest.main()
