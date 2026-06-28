#!/usr/bin/env python3
from pathlib import Path


def replace(path: str, old: str, new: str, expected: int = 1) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(
            f"{path}: expected {expected} occurrence(s), found {count}: {old[:100]!r}"
        )
    file.write_text(text.replace(old, new), encoding="utf-8")


replace(
    "helpers/nocky_youtube.py",
    '''def _config_dir() -> Path:
    root = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    return root / "nocky"
''',
    '''def _config_dir() -> Path:
    root = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    return root / "nocky"


def _app_config_path() -> Path:
    override = os.environ.get("NOCKY_CONFIG_FILE", "").strip()
    return Path(override).expanduser() if override else _config_dir() / "config.json"
''',
)

replace(
    "helpers/nocky_youtube.py",
    '''def _language() -> str:
    value = _system_locale().lower()
    for language in ("pt", "es", "fr", "de", "it"):
        if value.startswith(language):
            return language
    return "en"


def _location() -> str:
    value = _system_locale().replace("-", "_").upper()
    return "BR" if "_BR" in value else ""
''',
    '''APP_LANGUAGE_CODES = {
    "portuguese": "pt",
    "english": "en",
    "spanish": "es",
}

ACCEPT_LANGUAGE_HEADERS = {
    "pt": "pt-BR,pt;q=0.9,en;q=0.7",
    "es": "es-ES,es;q=0.9,en;q=0.7",
    "en": "en-US,en;q=0.9",
}


def _configured_language() -> str:
    try:
        payload = json.loads(_app_config_path().read_text(encoding="utf-8"))
    except (OSError, ValueError, TypeError):
        return ""
    if not isinstance(payload, dict):
        return ""
    return APP_LANGUAGE_CODES.get(str(payload.get("language") or "").strip().lower(), "")


def _language() -> str:
    configured = _configured_language()
    if configured:
        return configured

    value = _system_locale().lower()
    if value.startswith("pt") or ":pt" in value:
        return "pt"
    if value.startswith("es") or ":es" in value:
        return "es"
    return "en"


def _location() -> str:
    value = _system_locale().split(":", 1)[0].replace("-", "_")
    locale_name = re.split(r"[.@]", value, maxsplit=1)[0]
    parts = locale_name.split("_")
    if len(parts) >= 2 and len(parts[1]) == 2 and parts[1].isalpha():
        return parts[1].upper()
    return ""


def _accept_language(language: str = "") -> str:
    return ACCEPT_LANGUAGE_HEADERS.get(language or _language(), ACCEPT_LANGUAGE_HEADERS["en"])


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
            normalized = {str(k): str(v) for k, v in headers.items()}
            normalized["accept-language"] = _accept_language(language)
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

Path("tests/test_youtube_locale.py").write_text(
    '''import json
import os
import sys
import tempfile
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

    def test_configured_app_language_has_priority_over_system_locale(self):
        with tempfile.TemporaryDirectory() as directory:
            config = Path(directory) / "config.json"
            config.write_text(json.dumps({"language": "spanish"}), encoding="utf-8")
            with mock.patch.dict(
                os.environ,
                {"NOCKY_CONFIG_FILE": str(config), "LC_ALL": "pt_BR.UTF-8"},
                clear=False,
            ):
                self.assertEqual(helper._language(), "es")
                self.assertEqual(helper._location(), "BR")
                self.assertEqual(
                    helper._accept_language(),
                    "es-ES,es;q=0.9,en;q=0.7",
                )

    def test_authenticated_client_receives_app_locale_and_header(self):
        with tempfile.TemporaryDirectory() as directory:
            config = Path(directory) / "config.json"
            config.write_text(json.dumps({"language": "portuguese"}), encoding="utf-8")
            session_payload = {
                "headers": {
                    "cookie": "SAPISID=test-secret",
                    "origin": "https://music.youtube.com",
                    "accept-language": "en-US,en;q=0.9",
                }
            }
            with (
                mock.patch.dict(
                    os.environ,
                    {"NOCKY_CONFIG_FILE": str(config), "LC_ALL": "pt_BR.UTF-8"},
                    clear=False,
                ),
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
        with (
            mock.patch.object(helper, "_language", return_value="pt"),
            mock.patch.object(helper, "_location", return_value="BR"),
        ):
            portuguese = helper._feed_cache_key("home", "", 6, "")
        with (
            mock.patch.object(helper, "_language", return_value="es"),
            mock.patch.object(helper, "_location", return_value="BR"),
        ):
            spanish = helper._feed_cache_key("home", "", 6, "")

        self.assertNotEqual(portuguese, spanish)
        self.assertTrue(portuguese.startswith("pt-BR:"))
        self.assertTrue(spanish.startswith("es-BR:"))


if __name__ == "__main__":
    unittest.main()
''',
    encoding="utf-8",
)
