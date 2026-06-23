#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
I18N = ROOT / "src" / "i18n.rs"
SETTINGS = ROOT / "src" / "settings_page.rs"
MAIN = ROOT / "src" / "main.rs"


def fail(message: str) -> None:
    raise SystemExit(message)


def matching_delimiter(source: str, start: int, opening: str, closing: str) -> int:
    depth = 0
    in_string = False
    escaped = False
    index = start

    while index < len(source):
        char = source[index]

        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            index += 1
            continue

        if char == '"':
            in_string = True
        elif char == opening:
            depth += 1
        elif char == closing:
            depth -= 1
            if depth == 0:
                return index
        index += 1

    fail(f"Unclosed delimiter at offset {start}")


def split_top_level(arguments: str) -> list[str]:
    parts: list[str] = []
    start = 0
    parens = brackets = braces = 0
    in_string = False
    escaped = False

    for index, char in enumerate(arguments):
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            continue

        if char == '"':
            in_string = True
        elif char == "(":
            parens += 1
        elif char == ")":
            parens -= 1
        elif char == "[":
            brackets += 1
        elif char == "]":
            brackets -= 1
        elif char == "{":
            braces += 1
        elif char == "}":
            braces -= 1
        elif char == "," and parens == brackets == braces == 0:
            parts.append(arguments[start:index].strip())
            start = index + 1

    tail = arguments[start:].strip()
    if tail:
        parts.append(tail)
    return parts


def audit_i18n() -> int:
    source = I18N.read_text(encoding="utf-8")

    enum_match = re.search(
        r"pub enum Message\s*\{(?P<body>.*?)^\}",
        source,
        re.MULTILINE | re.DOTALL,
    )
    if not enum_match:
        fail("Message enum not found")

    variants = re.findall(
        r"^\s{4}([A-Za-z0-9_]+),\s*$",
        enum_match.group("body"),
        re.MULTILINE,
    )
    if len(variants) != len(set(variants)):
        fail("Duplicate Message variants found")

    all_match = re.search(
        r"const ALL_MESSAGES:\s*&\[Message\]\s*=\s*&\[(?P<body>.*?)\];",
        source,
        re.DOTALL,
    )
    if not all_match:
        fail("ALL_MESSAGES not found")

    listed = re.findall(r"Message::([A-Za-z0-9_]+)", all_match.group("body"))
    if set(listed) != set(variants):
        missing = sorted(set(variants) - set(listed))
        extra = sorted(set(listed) - set(variants))
        fail(f"ALL_MESSAGES mismatch: missing={missing}, extra={extra}")

    text_start = source.find("pub fn text(")
    tests_start = source.find("#[cfg(test)]", text_start)
    if text_start < 0 or tests_start < 0:
        fail("Translation function boundaries not found")
    text_body = source[text_start:tests_start]

    invalid: list[str] = []
    for variant in variants:
        count = len(
            re.findall(
                rf"Message::{re.escape(variant)}\s*=>",
                text_body,
            )
        )
        if count != 3:
            invalid.append(f"{variant}={count}")

    if invalid:
        fail(
            "Every Message must have PT/EN/ES arms: " + ", ".join(invalid)
        )

    return len(variants)


def audit_group_text() -> int:
    source = SETTINGS.read_text(encoding="utf-8")
    cursor = 0
    calls = 0

    while True:
        start = source.find("group_text(", cursor)
        if start < 0:
            break

        open_paren = source.find("(", start)
        close_paren = matching_delimiter(source, open_paren, "(", ")")
        arguments = source[open_paren + 1 : close_paren]
        parts = split_top_level(arguments)

        if len(parts) != 3:
            line = source.count("\n", 0, start) + 1
            fail(
                f"group_text at line {line} has {len(parts)} arguments, expected 3"
            )
        if any(not part.strip() for part in parts):
            line = source.count("\n", 0, start) + 1
            fail(f"group_text at line {line} contains an empty argument")

        calls += 1
        cursor = close_paren + 1

    if calls == 0:
        fail("No group_text calls found in settings_page.rs")
    return calls


def extract_function(source: str, signature: str) -> str:
    start = source.find(signature)
    if start < 0:
        fail(f"Function not found: {signature}")
    brace = source.find("{", start)
    if brace < 0:
        fail(f"Function body not found: {signature}")
    end = matching_delimiter(source, brace, "{", "}")
    return source[start : end + 1]


def audit_popup_languages() -> None:
    source = MAIN.read_text(encoding="utf-8")

    for signature in (
        "fn show_about_window",
        "fn show_shortcuts_window",
    ):
        block = extract_function(source, signature)
        counts = {
            language: block.count(f"AppLanguage::{language}")
            for language in ("Portuguese", "English", "Spanish")
        }
        if min(counts.values()) == 0 or len(set(counts.values())) != 1:
            fail(f"{signature} language coverage is uneven: {counts}")

    about = extract_function(source, "fn show_about_window")
    if "version_prefix" not in about:
        fail("About window version label is not localized")
    if '"Version {}"' in about:
        fail("About window still contains a hard-coded English version label")


def main() -> None:
    for path in (I18N, SETTINGS, MAIN):
        if not path.is_file():
            fail(f"Missing source file: {path}")

    message_count = audit_i18n()
    group_text_count = audit_group_text()
    audit_popup_languages()

    print(
        "Translation audit passed: "
        f"{message_count} Message variants × PT/EN/ES, "
        f"{group_text_count} localized settings calls, "
        "themed popup language coverage OK."
    )


if __name__ == "__main__":
    main()
