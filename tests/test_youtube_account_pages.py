from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube  # noqa: E402


class FakeLibraryClient:
    def get_library_songs(self, limit=100, order=None):
        return [
            {
                "videoId": "song0000001",
                "title": "Library Song",
                "artists": [{"name": "Library Artist", "id": "UClibrary"}],
                "album": {"name": "Library Album", "id": "MPRElibrary"},
                "duration": "3:20",
                "thumbnails": [{"url": "https://example.test/library.jpg", "width": 120}],
            }
        ]

    def get_liked_songs(self, limit=100):
        return {
            "tracks": [
                {
                    "videoId": "liked000001",
                    "title": "Liked Song",
                    "artists": [{"name": "Liked Artist", "id": "UCliked"}],
                    "album": {"name": "Liked Album", "id": "MPREliked"},
                    "duration": "4:01",
                    "thumbnails": [{"url": "https://example.test/liked.jpg", "width": 120}],
                }
            ]
        }

    def get_library_playlists(self, limit=100):
        return [
            {
                "title": "Saved Playlist",
                "playlistId": "PLsaved",
                "count": "12",
                "thumbnails": [{"url": "https://example.test/playlist.jpg", "width": 120}],
            }
        ]

    def get_library_albums(self, limit=100):
        return []

    def get_library_artists(self, limit=100):
        return []


class AccountPageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.client = FakeLibraryClient()

    def test_overview_keeps_all_account_sections_with_derived_collections(self) -> None:
        page = nocky_youtube._account_library_page(self.client, 80, "overview")
        titles = [section["title"] for section in page["sections"]]
        self.assertEqual(
            titles,
            [
                "Adicionadas recentemente",
                "Músicas curtidas",
                "Suas playlists",
                "Álbuns",
                "Artistas",
            ],
        )

        albums = next(section for section in page["sections"] if section["title"] == "Álbuns")
        artists = next(section for section in page["sections"] if section["title"] == "Artistas")
        self.assertEqual(
            {item["browse_id"] for item in albums["items"]},
            {"MPRElibrary", "MPREliked"},
        )
        self.assertEqual(
            {item["browse_id"] for item in artists["items"]},
            {"UClibrary", "UCliked"},
        )

    def test_library_page_is_structured_instead_of_a_flat_song_vector(self) -> None:
        page = nocky_youtube._account_library_page(self.client, 80, "library")
        self.assertEqual(
            [section["title"] for section in page["sections"]],
            ["Músicas da biblioteca", "Playlists", "Álbuns", "Artistas"],
        )
        self.assertEqual(page["sections"][0]["layout"], "list")
        self.assertEqual(page["sections"][2]["items"][0]["result_type"], "album")
        self.assertEqual(page["sections"][3]["items"][0]["result_type"], "artist")

    def test_liked_page_includes_derived_album_and_artist_sections(self) -> None:
        page = nocky_youtube._account_library_page(self.client, 80, "liked")
        self.assertEqual(
            [section["title"] for section in page["sections"]],
            ["Músicas curtidas", "Álbuns das curtidas", "Artistas das curtidas"],
        )
        self.assertEqual(page["sections"][1]["items"][0]["browse_id"], "MPREliked")
        self.assertEqual(page["sections"][2]["items"][0]["browse_id"], "UCliked")

    def test_title_only_collections_remain_navigable_fallbacks(self) -> None:
        raw = {
            "videoId": "fallback001",
            "title": "Fallback Song",
            "artists": [{"name": "Artist Without Id"}],
            "album": {"name": "Album Without Id"},
        }
        albums, artists = nocky_youtube._song_collection_items(
            [raw],
            "Derived collection",
        )
        self.assertEqual(albums[0]["title"], "Album Without Id")
        self.assertEqual(albums[0]["browse_id"], "")
        self.assertEqual(artists[0]["title"], "Artist Without Id")
        self.assertEqual(artists[0]["browse_id"], "")


if __name__ == "__main__":
    unittest.main()
