import os
import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "helpers"))

import nocky_youtube as helper


class FakeYTMusic:
    calls = []

    def __init__(self, *args, **kwargs):
        self.__class__.calls.append((args, kwargs))


class YouTubeLocaleTests(unittest.TestCase):
    def setUp(self):
        FakeYTMusic.calls.clear()

    def locale_environment(self, value: str):
        return mock.patch.dict(
            os.environ,
            {
                "LC_ALL": value,
                "LC_MESSAGES": "",
                "LANGUAGE": "",
                "LANG": "",
            },
            clear=False,
        )

    def test_detects_supported_system_languages_and_regions(self):
        for locale_name, language, location in (
            ("pt_BR.UTF-8", "pt", "BR"),
            ("es_ES.UTF-8", "es", "ES"),
            ("en_US.UTF-8", "en", "US"),
        ):
            with self.subTest(locale=locale_name), self.locale_environment(locale_name):
                self.assertEqual(helper._language(), language)
                self.assertEqual(helper._location(), location)

    def test_neutral_lc_all_does_not_hide_lang_locale(self):
        with mock.patch.dict(
            os.environ,
            {
                "LC_ALL": "C.UTF-8",
                "LC_MESSAGES": "",
                "LANGUAGE": "",
                "LANG": "pt_BR.UTF-8",
            },
            clear=False,
        ):
            self.assertEqual(helper._language(), "pt")
            self.assertEqual(helper._location(), "BR")
            self.assertEqual(helper._accept_language(), "pt-BR,pt;q=0.9,en;q=0.7")

    def test_unsupported_system_language_falls_back_to_english(self):
        with self.locale_environment("fr_FR.UTF-8"):
            self.assertEqual(helper._language(), "en")
            self.assertEqual(helper._location(), "FR")

    def test_accept_language_uses_detected_locale(self):
        with self.locale_environment("pt_BR.UTF-8"):
            self.assertEqual(
                helper._accept_language(),
                "pt-BR,pt;q=0.9,en;q=0.7",
            )
        with self.locale_environment("es_ES.UTF-8"):
            self.assertEqual(
                helper._accept_language(),
                "es-ES,es;q=0.9,en;q=0.7",
            )
        with self.locale_environment("en_US.UTF-8"):
            self.assertEqual(helper._accept_language(), "en-US,en;q=0.9")

    def test_authenticated_client_receives_system_locale_and_header(self):
        session_payload = {
            "headers": {
                "cookie": "SAPISID=test-secret",
                "origin": "https://music.youtube.com",
                "accept-language": "en-US,en;q=0.9",
            }
        }
        with (
            self.locale_environment("pt_BR.UTF-8"),
            mock.patch.object(helper, "YTMusic", FakeYTMusic),
            mock.patch.object(helper, "_session", return_value=object()),
            mock.patch.object(helper, "_load_session", return_value=session_payload),
        ):
            helper._create_client(authenticated=True)

        args, kwargs = FakeYTMusic.calls[-1]
        self.assertEqual(kwargs["language"], "pt")
        self.assertEqual(kwargs["location"], "BR")
        self.assertEqual(
            args[0]["accept-language"],
            "pt-BR,pt;q=0.9,en;q=0.7",
        )

    def test_feed_cache_is_separated_by_language_and_region(self):
        with self.locale_environment("pt_BR.UTF-8"):
            portuguese = helper._feed_cache_key("home", "", 6, "")
        with self.locale_environment("es_ES.UTF-8"):
            spanish = helper._feed_cache_key("home", "", 6, "")

        self.assertNotEqual(portuguese, spanish)
        self.assertTrue(portuguese.startswith("pt-BR:"))
        self.assertTrue(spanish.startswith("es-ES:"))


if __name__ == "__main__":
    unittest.main()
