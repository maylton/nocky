#!/usr/bin/env python3
"""Apply the narrow ytmusicapi empty playlist count compatibility fix."""

from __future__ import annotations

from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match in {path}, found {count}")
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "helpers/nocky_youtube.py",
    '''    from ytmusicapi import YTMusic
    from ytmusicapi.exceptions import YTMusicServerError, YTMusicUserError
    try:
''',
    '''    from ytmusicapi import YTMusic
    from ytmusicapi.exceptions import YTMusicServerError, YTMusicUserError
    from ytmusicapi.parsers import playlists as ytmusic_playlist_parsers
    try:
''',
    "import playlist parser",
)
replace_once(
    "helpers/nocky_youtube.py",
    '''    YTMusicServerError = RuntimeError
    YTMusicUserError = RuntimeError
    ytmusic_get_continuations = None
''',
    '''    YTMusicServerError = RuntimeError
    YTMusicUserError = RuntimeError
    ytmusic_playlist_parsers = None
    ytmusic_get_continuations = None
''',
    "playlist parser fallback",
)
replace_once(
    "helpers/nocky_youtube.py",
    '''else:
    IMPORT_ERROR = None

try:
    import gi
''',
    '''else:
    IMPORT_ERROR = None


def _install_ytmusicapi_playlist_count_compat() -> None:
    """Prevent ytmusicapi from converting an empty playlist count with int("").

    ytmusicapi 1.12.1 extracts digits from the playlist subtitle and checks the
    resulting list with ``is not None``. An empty playlist therefore reaches
    ``to_int("")`` and raises ValueError. Patch only the playlists parser's local
    converter so digit-less count labels become zero while valid counts retain
    the upstream implementation.
    """

    parser = ytmusic_playlist_parsers
    if parser is None:
        return
    converter = getattr(parser, "to_int", None)
    if not callable(converter) or getattr(converter, "_nocky_empty_count_safe", False):
        return

    def safe_playlist_count(value: Any) -> int:
        text = str(value or "")
        if not re.search(r"\\d", text):
            return 0
        return converter(text)

    safe_playlist_count._nocky_empty_count_safe = True  # type: ignore[attr-defined]
    parser.to_int = safe_playlist_count


_install_ytmusicapi_playlist_count_compat()

try:
    import gi
''',
    "install playlist count compatibility",
)

Path("tests/test_youtube_playlist_count_compat.py").write_text(
    '''from __future__ import annotations

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
''',
    encoding="utf-8",
)

print("ytmusicapi empty-count compatibility fix applied")
