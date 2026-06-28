#!/usr/bin/env python3
from pathlib import Path


def replace(path: str, old: str, new: str, expected: int = 1) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(
            f"{path}: expected {expected} occurrence(s), found {count}: {old[:120]!r}"
        )
    file.write_text(text.replace(old, new), encoding="utf-8")


replace(
    "helpers/nocky_youtube.py",
    '''def _system_locale() -> str:
    for key in ("LC_ALL", "LC_MESSAGES", "LANG"):
        value = os.environ.get(key, "").strip()
        if value:
            return value
    try:
        language, _encoding = locale.getlocale()
    except Exception:
        language = None
    return language or ""


def _language() -> str:
    value = _system_locale().lower()
    for language in ("pt", "es", "fr", "de", "it"):
        if value.startswith(language):
            return language
    return "en"


def _location() -> str:
    value = _system_locale().replace("-", "_").upper()
    return "BR" if "_BR" in value else ""
''',
    '''def _system_locale() -> str:
    for key in ("LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"):
        value = os.environ.get(key, "").strip()
        if value:
            return value
    try:
        language, _encoding = locale.getlocale()
    except Exception:
        language = None
    return language or ""


def _locale_candidates() -> list[str]:
    return [
        candidate.strip().replace("-", "_")
        for candidate in re.split(r"[:;,]", _system_locale())
        if candidate.strip()
    ]


def _language() -> str:
    for candidate in _locale_candidates():
        language = candidate.split("_", 1)[0].split(".", 1)[0].lower()
        if language in {"pt", "es", "en"}:
            return language
    return "en"


def _location() -> str:
    for candidate in _locale_candidates():
        locale_name = re.split(r"[.@]", candidate, maxsplit=1)[0]
        parts = locale_name.split("_")
        if len(parts) >= 2 and len(parts[1]) == 2 and parts[1].isalpha():
            return parts[1].upper()
    return ""


def _accept_language(language: str = "", location: str = "") -> str:
    language = language or _language()
    location = location or _location()
    primary = f"{language}-{location}" if location else language
    if language == "en":
        return f"{primary},en;q=0.9" if primary != "en" else "en-US,en;q=0.9"
    return f"{primary},{language};q=0.9,en;q=0.7"


def _locale_cache_namespace() -> str:
    return f"{_language()}-{_location() or 'global'}"
''',
)

replace(
    "helpers/nocky_youtube.py",
    '    normalized.setdefault("accept-language", "en-US,en;q=0.9")\n',
    '    normalized.setdefault("accept-language", _accept_language())\n',
)

replace(
    "helpers/nocky_youtube.py",
    '''def _create_client(authenticated: bool = True):
    _require_dependencies()
    session = _session()
    if authenticated:
        payload = _load_session()
        headers = payload.get("headers")
        if isinstance(headers, dict) and headers:
            normalized = {str(k): str(v) for k, v in headers.items()}
            _ensure_authorization(normalized)
            return YTMusic(normalized, requests_session=session)
    return YTMusic(
        requests_session=session,
        language=_language(),
        location=_location(),
    )
''',
    '''def _create_client(authenticated: bool = True):
    _require_dependencies()
    session = _session()
    language = _language()
    location = _location()
    if authenticated:
        payload = _load_session()
        headers = payload.get("headers")
        if isinstance(headers, dict) and headers:
            normalized = {
                str(key).strip().lower(): str(value).strip()
                for key, value in headers.items()
                if str(key).strip() and str(value).strip()
            }
            normalized["accept-language"] = _accept_language(language, location)
            _ensure_authorization(normalized)
            return YTMusic(
                normalized,
                requests_session=session,
                language=language,
                location=location,
            )
    return YTMusic(
        requests_session=session,
        language=language,
        location=location,
    )
''',
)

replace(
    "helpers/nocky_youtube.py",
    '    return f"{kind}:{params_key}:{continuation or \'0\'}:{section_limit}"\n',
    '    return f"{_locale_cache_namespace()}:{kind}:{params_key}:{continuation or \'0\'}:{section_limit}"\n',
)

replace(
    "helpers/nocky_youtube.py",
    '''    headers = {
        str(key).strip().lower(): str(value).strip()
        for key, value in stored_headers.items()
        if str(key).strip() and str(value).strip()
    }

    args: list[str] = []
''',
    '''    headers = {
        str(key).strip().lower(): str(value).strip()
        for key, value in stored_headers.items()
        if str(key).strip() and str(value).strip()
    }
    headers["accept-language"] = _accept_language()

    args: list[str] = []
''',
)

Path("tests/test_youtube_locale.py").write_text(
    '''import os
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
''',
    encoding="utf-8",
)
