from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube  # noqa: E402


class FakeHomeClient:
    def __init__(self, response: dict) -> None:
        self.response = response
        self.requests: list[tuple[str, dict, str | None]] = []

    def _send_request(
        self,
        endpoint: str,
        body: dict,
        additional_params: str | None = None,
    ) -> dict:
        self.requests.append((endpoint, body, additional_params))
        return self.response

    def get_home(self, *args, **kwargs):  # pragma: no cover - regression tripwire
        raise AssertionError("Home V3 must use direct InnerTube browse responses")


class HomeV3Tests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.fixture = json.loads(
            (ROOT / "tests" / "fixtures" / "youtube_home_innertube_raw.json").read_text(
                encoding="utf-8"
            )
        )

    def test_home_v3_uses_direct_browse_response(self) -> None:
        client = FakeHomeClient(self.fixture)
        original_load_session = nocky_youtube._load_session
        original_create_client = nocky_youtube._create_client
        original_cache_home = os.environ.get("XDG_CACHE_HOME")

        with tempfile.TemporaryDirectory() as cache_home:
            os.environ["XDG_CACHE_HOME"] = cache_home
            nocky_youtube._load_session = lambda: {"headers": {"Cookie": "ok"}}
            nocky_youtube._create_client = lambda authenticated=True: client
            try:
                page = nocky_youtube.command_home_v3({"section_limit": 2})
            finally:
                nocky_youtube._load_session = original_load_session
                nocky_youtube._create_client = original_create_client
                if original_cache_home is None:
                    os.environ.pop("XDG_CACHE_HOME", None)
                else:
                    os.environ["XDG_CACHE_HOME"] = original_cache_home

        self.assertEqual(client.requests, [("browse", {"browseId": "FEmusic_home"}, None)])
        self.assertEqual(page["selected_chip_params"], "")
        self.assertEqual(len(page["sections"]), 2)


if __name__ == "__main__":
    unittest.main()
