#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
I18N = ROOT / "src" / "i18n.rs"
LYRICS = ROOT / "src" / "lyrics_view.rs"
BROWSER = ROOT / "src" / "browser.rs"
ONBOARDING = ROOT / "src" / "onboarding.rs"


def fail(message: str) -> None:
    raise SystemExit(message)


def extract_braced(source: str, start: int) -> str:
    brace = source.find("{", start)
    if brace < 0:
        fail(f"Opening brace not found at {start}")

    depth = 0
    in_string = False
    escaped = False
    index = brace

    while index < len(source):
        char = source[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
        elif char == '"':
            in_string = True
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1]
        index += 1

    fail(f"Unclosed block at {start}")


def audit_i18n() -> int:
    source = I18N.read_text(encoding="utf-8")
    enum = extract_braced(source, source.find("pub enum Message"))
    variants = re.findall(
        r"^\s{4}([A-Za-z0-9_]+),\s*$",
        enum,
        re.MULTILINE,
    )

    if len(variants) != len(set(variants)):
        fail("Duplicate Message variants")

    all_match = re.search(
        r"const ALL_MESSAGES:\s*&\[Message\]\s*=\s*&\[(.*?)\];",
        source,
        re.DOTALL,
    )
    if not all_match:
        fail("ALL_MESSAGES not found")
    listed = re.findall(r"Message::([A-Za-z0-9_]+)", all_match.group(1))
    if set(variants) != set(listed):
        fail("ALL_MESSAGES does not match Message")

    text_start = source.find("pub fn text(")
    tests_start = source.find("#[cfg(test)]", text_start)
    text = source[text_start:tests_start]

    bad = [
        variant
        for variant in variants
        if len(
            re.findall(
                rf"Message::{re.escape(variant)}\s*=>",
                text,
            )
        )
        != 3
    ]
    if bad:
        fail("Missing PT/EN/ES Message arms: " + ", ".join(bad))

    return len(variants)


def audit_copy_struct(
    source: str,
    struct_name: str,
    function_name: str,
) -> int:
    struct_start = source.find(f"struct {struct_name}")
    if struct_start < 0:
        fail(f"{struct_name} not found")
    struct = extract_braced(source, struct_start)
    fields = re.findall(
        r"^\s{4}([A-Za-z0-9_]+):",
        struct,
        re.MULTILINE,
    )
    if not fields:
        fail(f"No fields found in {struct_name}")

    function_start = source.find(f"fn {function_name}")
    if function_start < 0:
        fail(f"{function_name} not found")
    function = extract_braced(source, function_start)

    for language in ("Portuguese", "English", "Spanish"):
        language_start = function.find(f"AppLanguage::{language} =>")
        if language_start < 0:
            fail(f"{function_name}: {language} branch missing")
        branch = extract_braced(function, language_start)
        assigned = re.findall(
            r"^\s{12}([A-Za-z0-9_]+):",
            branch,
            re.MULTILINE,
        )
        if set(assigned) != set(fields):
            missing = sorted(set(fields) - set(assigned))
            extra = sorted(set(assigned) - set(fields))
            fail(
                f"{function_name}/{language}: "
                f"missing={missing}, extra={extra}"
            )

    return len(fields)


def audit_onboarding() -> int:
    source = ONBOARDING.read_text(encoding="utf-8")
    return audit_copy_struct(source, "Copy", "copy")


def audit_lyrics() -> int:
    source = LYRICS.read_text(encoding="utf-8")
    fields = audit_copy_struct(source, "LyricsCopy", "lyrics_copy")
    if "LyricsPresenter::new(language" not in (
        ROOT / "src" / "player_view.rs"
    ).read_text(encoding="utf-8"):
        fail("LyricsPresenter is not initialized with the selected language")

    forbidden = [
        '"As letras aparecerão aqui"',
        '"Reproduza uma música com letras sincronizadas',
    ]
    presenter_start = source.find("impl LyricsPresenter")
    presenter = source[presenter_start:]
    for literal in forbidden:
        if literal in presenter:
            fail(f"Hard-coded Portuguese remains in LyricsPresenter: {literal}")
    return fields


def audit_home() -> int:
    source = BROWSER.read_text(encoding="utf-8")
    fields = audit_copy_struct(source, "HomeCopy", "home_copy")

    home_start = source.find("fn rebuild_home")
    if home_start < 0:
        fail("rebuild_home not found")
    home = extract_braced(source, home_start)

    forbidden = [
        "Mixtapes criadas para você",
        "Mixes e rádios sincronizadas",
        "Seus álbuns",
        "Mais ouvidos e reproduzidos recentemente",
        "Seus artistas",
        "Com base no que você mais escuta",
        "Playlists sugeridas",
        "Playlists e recomendações sincronizadas",
        "faixas locais",
    ]
    for literal in forbidden:
        if literal in home:
            fail(f"Hard-coded Portuguese remains in Home: {literal}")

    for helper in (
        "home_section",
        "home_card_button",
        "home_empty_card",
        "home_syncing_hint",
        "ranked_home_album_cards",
        "ranked_home_artist_cards",
        "listening_rank_detail",
    ):
        if f"fn {helper}" not in source:
            fail(f"Home localization helper missing: {helper}")

    return fields


def main() -> None:
    required = [I18N, LYRICS, BROWSER, ONBOARDING]
    for path in required:
        if not path.is_file():
            fail(f"Missing source file: {path}")

    messages = audit_i18n()
    lyrics_fields = audit_lyrics()
    home_fields = audit_home()
    onboarding_fields = audit_onboarding()

    print(
        "Localization audit passed: "
        f"{messages} i18n messages, "
        f"{lyrics_fields} lyrics fields, "
        f"{home_fields} Home fields, "
        f"{onboarding_fields} onboarding fields — PT/EN/ES complete."
    )


if __name__ == "__main__":
    main()
