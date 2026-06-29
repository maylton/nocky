from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_playlist_mutations  # noqa: E402
import nocky_youtube_playlist_create  # noqa: E402


class FakeClient:
    def __init__(self, result="PL_created_123"):
        self.result = result
        self.calls = []

    def create_playlist(self, title, description, privacy_status="PRIVATE"):
        self.calls.append((title, description, privacy_status))
        return self.result


class PlaylistCreationContractTests(unittest.TestCase):
    def test_normalizes_title_description_and_default_privacy(self) -> None:
        self.assertEqual(
            nocky_playlist_mutations.normalize_create_request(
                {"title": "  Focus  ", "description": "  Study mix  "}
            ),
            {
                "title": "Focus",
                "description": "Study mix",
                "privacy": "PRIVATE",
            },
        )

    def test_accepts_supported_privacy_values_case_insensitively(self) -> None:
        for privacy in ("private", "Unlisted", "PUBLIC"):
            normalized = nocky_playlist_mutations.normalize_create_request(
                {"title": "Playlist", "privacy": privacy}
            )
            self.assertEqual(normalized["privacy"], privacy.upper())

    def test_rejects_invalid_requests_before_network_access(self) -> None:
        invalid_payloads = (
            {},
            {"title": "   "},
            {"title": "Bad <title>"},
            {"title": "Playlist", "privacy": "FRIENDS"},
            {"title": "Playlist", "video_ids": ["abcdefghijk"]},
            {"title": "Playlist", "source_playlist": "PL_source"},
        )
        for payload in invalid_payloads:
            with self.subTest(payload=payload):
                with self.assertRaises(RuntimeError):
                    nocky_playlist_mutations.normalize_create_request(payload)

    def test_sanitizes_string_and_dictionary_results(self) -> None:
        expected = {
            "playlist_id": "PL_created_123",
            "title": "Focus",
            "privacy": "PRIVATE",
        }
        self.assertEqual(
            nocky_playlist_mutations.sanitize_create_result(
                "PL_created_123",
                title="Focus",
                privacy="PRIVATE",
            ),
            expected,
        )
        self.assertEqual(
            nocky_playlist_mutations.sanitize_create_result(
                {"playlistId": "PL_created_123", "private_context": "ignored"},
                title="Focus",
                privacy="PRIVATE",
            ),
            expected,
        )

    def test_rejects_unconfirmed_or_invalid_results_without_leaking_payload(self) -> None:
        for result in (
            {},
            {"status": "FAILED", "private_context": "do-not-return"},
            "not a valid id",
        ):
            with self.subTest(result=result):
                with self.assertRaisesRegex(
                    RuntimeError,
                    "did not confirm playlist creation",
                ) as raised:
                    nocky_playlist_mutations.sanitize_create_result(
                        result,
                        title="Focus",
                        privacy="PRIVATE",
                    )
                self.assertNotIn("private_context", str(raised.exception))
                self.assertNotIn("do-not-return", str(raised.exception))


class PlaylistCreationHelperTests(unittest.TestCase):
    def test_invalid_request_does_not_load_session_or_create_client(self) -> None:
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
        ) as load_session:
            with patch.object(
                nocky_youtube_playlist_create.nocky_youtube,
                "_create_client",
            ) as create_client:
                with self.assertRaisesRegex(RuntimeError, "title is required"):
                    nocky_youtube_playlist_create.create_empty_playlist({})
                load_session.assert_not_called()
                create_client.assert_not_called()

    def test_disconnected_session_does_not_create_client(self) -> None:
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
            return_value={},
        ):
            with patch.object(
                nocky_youtube_playlist_create.nocky_youtube,
                "_create_client",
            ) as create_client:
                with self.assertRaisesRegex(RuntimeError, "Connect a YouTube Music"):
                    nocky_youtube_playlist_create.create_empty_playlist(
                        {"title": "Focus"}
                    )
                create_client.assert_not_called()

    def test_valid_request_calls_create_once_and_returns_allowlist(self) -> None:
        client = FakeClient()
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test_header": "present"}},
        ):
            with patch.object(
                nocky_youtube_playlist_create.nocky_youtube,
                "_create_client",
                return_value=client,
            ) as create_client:
                result = nocky_youtube_playlist_create.create_empty_playlist(
                    {
                        "title": " Focus ",
                        "description": " Study ",
                        "privacy": "private",
                    }
                )

        create_client.assert_called_once_with(authenticated=True)
        self.assertEqual(client.calls, [("Focus", "Study", "PRIVATE")])
        self.assertEqual(
            result,
            {
                "playlist_id": "PL_created_123",
                "title": "Focus",
                "privacy": "PRIVATE",
            },
        )
        self.assertEqual(set(result), {"playlist_id", "title", "privacy"})

    def test_service_failure_is_sanitized(self) -> None:
        client = FakeClient(
            {"status": "FAILED", "private_context": "must-not-escape"}
        )
        with patch.object(
            nocky_youtube_playlist_create.nocky_youtube,
            "_load_session",
            return_value={"headers": {"test_header": "present"}},
        ):
            with patch.object(
                nocky_youtube_playlist_create.nocky_youtube,
                "_create_client",
                return_value=client,
            ):
                with self.assertRaisesRegex(
                    RuntimeError,
                    "did not confirm playlist creation",
                ) as raised:
                    nocky_youtube_playlist_create.create_empty_playlist(
                        {"title": "Focus"}
                    )
        self.assertNotIn("private_context", str(raised.exception))
        self.assertNotIn("must-not-escape", str(raised.exception))


class PlaylistCreationPackagingTests(unittest.TestCase):
    def test_installer_packages_creation_helper_contract_and_documentation(self) -> None:
        installer = (ROOT / "install.sh").read_text(encoding="utf-8")
        for required_path in (
            "helpers/nocky_youtube_playlist_create.py",
            "helpers/nocky_playlist_mutations.py",
            "docs/YOUTUBE_PLAYLIST_CREATION.md",
        ):
            with self.subTest(required_path=required_path):
                self.assertIn(required_path, installer)

    def test_smoke_script_requires_explicit_remote_confirmation(self) -> None:
        smoke = (ROOT / "scripts/smoke-youtube-playlist-create.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn("NOCKY_CONFIRM_PLAYLIST_CREATE", smoke)
        self.assertIn("NOCKY_PLAYLIST_TITLE", smoke)


if __name__ == "__main__":
    unittest.main()
