from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube  # noqa: E402,F401

try:
    from ytmusicapi.parsers import playlists as playlist_parsers  # noqa: E402
except Exception:
    playlist_parsers = None


@unittest.skipIf(playlist_parsers is None, "ytmusicapi is unavailable")
class PlaylistCountCompatibilityTests(unittest.TestCase):
    def test_digitless_playlist_count_is_zero(self) -> None:
        self.assertEqual(playlist_parsers.to_int(""), 0)
        self.assertEqual(playlist_parsers.to_int("No songs"), 0)

    def test_valid_playlist_counts_keep_upstream_conversion(self) -> None:
        self.assertEqual(playlist_parsers.to_int("12 songs"), 12)
        self.assertEqual(playlist_parsers.to_int("1.234 músicas"), 1234)

    def test_compatibility_installation_is_idempotent(self) -> None:
        converter = playlist_parsers.to_int
        nocky_youtube._install_ytmusicapi_playlist_count_compat()
        self.assertIs(playlist_parsers.to_int, converter)


if __name__ == "__main__":
    unittest.main()
