#!/usr/bin/env python3
'Validate and report the modular Material Expressive CSS architecture.'

from __future__ import annotations

import argparse
import re
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
THEME_DIR = ROOT / "assets" / "themes" / "material-expressive"
MONOLITH = ROOT / "assets" / "themes" / "material-expressive.css"
THEME_MODULE = ROOT / "src" / "theme_css.rs"

RULE_RE = re.compile(r"(?s)([^{}]+)\{([^{}]*)\}")
MARKER_RE = re.compile(r"/\*\s*([A-Za-z0-9_-]+_v\d+)\s*\*/")
INCLUDE_RE = re.compile(
    r'include_str!\("\.\./assets/themes/material-expressive/([^"]+)"\)'
)
EXPECTED_BYTES_RE = re.compile(
    r"EXPECTED_MATERIAL_EXPRESSIVE_BYTES:\s*usize\s*=\s*(\d+)"
)
MODULE_NAME_RE = re.compile(r"\d{3}-[a-z0-9-]+\.css")


def normalized_selector(selector: str) -> str:
    selector = re.sub(r"/\*.*?\*/", " ", selector, flags=re.DOTALL)
    return " ".join(selector.split())


def validate_balanced_css(text: str, module_name: str) -> list[str]:
    errors: list[str] = []
    depth = 0
    in_comment = False
    quote: str | None = None
    escaped = False
    index = 0

    while index < len(text):
        char = text[index]
        nxt = text[index + 1] if index + 1 < len(text) else ""

        if in_comment:
            if char == "*" and nxt == "/":
                in_comment = False
                index += 2
                continue
            index += 1
            continue

        if quote is not None:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
            index += 1
            continue

        if char == "/" and nxt == "*":
            in_comment = True
            index += 2
            continue

        if char in {'"', "'"}:
            quote = char
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth < 0:
                errors.append(
                    f"{module_name}: closing brace without opening brace"
                )
                depth = 0

        index += 1

    if in_comment:
        errors.append(f"{module_name}: unclosed comment")
    if quote is not None:
        errors.append(f"{module_name}: unclosed string")
    if depth != 0:
        errors.append(f"{module_name}: unbalanced braces ({depth})")

    return errors


def loaded_module_names(errors: list[str]) -> tuple[list[str], int | None]:
    if not THEME_MODULE.exists():
        errors.append("src/theme_css.rs does not exist")
        return [], None

    text = THEME_MODULE.read_text(encoding="utf-8")
    names = INCLUDE_RE.findall(text)

    if not names:
        errors.append("theme_css.rs embeds no Material Expressive modules")

    if len(names) != len(set(names)):
        errors.append("theme_css.rs embeds a module more than once")

    expected_match = EXPECTED_BYTES_RE.search(text)
    expected_bytes = (
        int(expected_match.group(1))
        if expected_match is not None
        else None
    )

    if expected_bytes is None:
        errors.append(
            "EXPECTED_MATERIAL_EXPRESSIVE_BYTES is missing"
        )

    return names, expected_bytes


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail on structural architecture errors",
    )
    args = parser.parse_args()

    modules = sorted(THEME_DIR.glob("*.css"))
    disk_names = [module.name for module in modules]
    errors: list[str] = []

    if MONOLITH.exists():
        errors.append("the old material-expressive.css monolith still exists")
    if not modules:
        errors.append("no modular CSS files were found")

    loaded_names, expected_bytes = loaded_module_names(errors)

    if loaded_names != disk_names:
        missing = sorted(set(loaded_names) - set(disk_names))
        orphaned = sorted(set(disk_names) - set(loaded_names))
        if missing:
            errors.append(
                "loaded modules missing on disk: " + ", ".join(missing)
            )
        if orphaned:
            errors.append(
                "orphan modules not loaded: " + ", ".join(orphaned)
            )
        if not missing and not orphaned:
            errors.append(
                "module loader order differs from numeric filename order"
            )

    invalid_names = [
        name for name in disk_names if MODULE_NAME_RE.fullmatch(name) is None
    ]
    if invalid_names:
        errors.append(
            "invalid module names: " + ", ".join(invalid_names)
        )

    prefixes = [name[:3] for name in disk_names]
    duplicate_prefixes = sorted(
        prefix for prefix, count in Counter(prefixes).items() if count > 1
    )
    if duplicate_prefixes:
        errors.append(
            "duplicate numeric prefixes: " + ", ".join(duplicate_prefixes)
        )

    selector_counts: Counter[str] = Counter()
    property_counts: Counter[str] = Counter()
    marker_counts: Counter[str] = Counter()
    total_lines = 0
    total_bytes = 0

    print("Material Expressive CSS architecture audit")
    print("=" * 42)

    for index, module in enumerate(modules, start=1):
        text = module.read_text(encoding="utf-8")
        lines = text.count("\n") + (0 if text.endswith("\n") else 1)
        size = len(text.encode("utf-8"))
        total_lines += lines
        total_bytes += size

        status = "loaded" if module.name in loaded_names else "orphan"
        print(
            f"{index:02}. {module.name:32} "
            f"{lines:5} lines  {size:7} bytes  {status}"
        )

        if not text.strip():
            errors.append(f"{module.name}: empty module")

        errors.extend(validate_balanced_css(text, module.name))

        for selector, body in RULE_RE.findall(text):
            selector = normalized_selector(selector)
            if not selector or selector.startswith("@"):
                continue
            selector_counts[selector] += 1

            properties = []
            for declaration in body.split(";"):
                if ":" not in declaration:
                    continue
                name = declaration.split(":", 1)[0].strip()
                if name:
                    properties.append(name)
            if len(properties) >= 3:
                property_counts[" | ".join(properties)] += 1

        marker_counts.update(MARKER_RE.findall(text))

    if expected_bytes is not None and expected_bytes != total_bytes:
        errors.append(
            "Material CSS byte contract mismatch: "
            f"expected {expected_bytes}, got {total_bytes}"
        )

    print("-" * 72)
    print(
        f"Total: {len(modules)} modules, "
        f"{total_lines} lines, {total_bytes} bytes"
    )
    print()

    duplicate_selectors = [
        (selector, count)
        for selector, count in selector_counts.most_common()
        if count > 1
    ]
    print(f"Repeated selectors: {len(duplicate_selectors)}")
    for selector, count in duplicate_selectors[:30]:
        print(f"  {count:3}x  {selector[:110]}")

    print()
    repeated_shapes = [
        (shape, count)
        for shape, count in property_counts.most_common()
        if count >= 4
    ]
    print(f"Repeated declaration shapes: {len(repeated_shapes)}")
    for shape, count in repeated_shapes[:20]:
        print(f"  {count:3}x  {shape[:110]}")

    print()
    repeated_markers = [
        (name, count)
        for name, count in marker_counts.most_common()
        if count > 1
    ]
    print(f"Version markers: {sum(marker_counts.values())}")
    print(f"Repeated version markers: {len(repeated_markers)}")
    for name, count in repeated_markers:
        print(f"  {count:3}x  {name}")

    if errors:
        print()
        print("Structural errors:")
        for error in errors:
            print(f"  - {error}")
    else:
        print()
        print("Structural result: OK")

    return 1 if args.check and errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
