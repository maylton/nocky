from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube  # noqa: E402


class YouTubeTrackArtworkTests(unittest.TestCase):
    def test_song_without_thumbnail_uses_video_thumbnail(self) -> None:
        item = nocky_youtube._song_item(
            {
                "videoId": "abcdefghijk",
                "title": "Track without artwork",
                "artists": [{"name": "Artist"}],
            }
        )
        self.assertIsNotNone(item)
        self.assertEqual(
            item["thumbnail_url"],
            "https://i.ytimg.com/vi/abcdefghijk/hqdefault.jpg",
        )

    def test_song_keeps_explicit_youtube_music_thumbnail(self) -> None:
        item = nocky_youtube._song_item(
            {
                "videoId": "abcdefghijk",
                "title": "Track with artwork",
                "thumbnails": [
                    {
                        "url": "https://lh3.googleusercontent.com/track=s240",
                        "width": 240,
                        "height": 240,
                    }
                ],
            }
        )
        self.assertIsNotNone(item)
        self.assertIn("track=s1200", item["thumbnail_url"])

    def test_invalid_video_id_is_not_accepted_as_a_song(self) -> None:
        self.assertIsNone(
            nocky_youtube._song_item(
                {"videoId": "", "title": "Invalid track"}
            )
        )


if __name__ == "__main__":
    unittest.main()
