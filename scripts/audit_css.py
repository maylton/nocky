#!/usr/bin/env python3
"""Audit the modular Material Expressive CSS without changing it."""

from __future__ import annotations

import argparse
import re
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
THEME_DIR = ROOT / "assets" / "themes" / "material-expressive"
MONOLITH = ROOT / "assets" / "themes" / "material-expressive.css"

RULE_RE = re.compile(r"(?s)([^{}]+)\{([^{}]*)\}")
MARKER_RE = re.compile(r"/\*\s*([A-Za-z0-9_-]+_v\d+)\s*\*/")


def normalized_selector(selector: str) -> str:
    return " ".join(selector.split())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail only on structural architecture errors",
    )
    args = parser.parse_args()

    modules = sorted(THEME_DIR.glob("*.css"))
    errors: list[str] = []

    if MONOLITH.exists():
        errors.append("the old material-expressive.css monolith still exists")
    if not modules:
        errors.append("no modular CSS files were found")

    selector_counts: Counter[str] = Counter()
    property_counts: Counter[str] = Counter()
    marker_counts: Counter[str] = Counter()
    total_lines = 0
    total_bytes = 0

    print("Material Expressive CSS audit")
    print("=" * 31)

    for module in modules:
        text = module.read_text(encoding="utf-8")
        lines = text.count("\n") + (0 if text.endswith("\n") else 1)
        size = len(text.encode("utf-8"))
        total_lines += lines
        total_bytes += size
        print(f"{module.name:34} {lines:5} lines  {size:7} bytes")

        for selector, body in RULE_RE.findall(text):
            selector = normalized_selector(selector)
            if selector.startswith("@"):
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

    print("-" * 58)
    print(f"Total: {total_lines} lines, {total_bytes} bytes")
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
    print(f"Legacy/version markers: {sum(marker_counts.values())}")
    for marker, count in marker_counts.most_common():
        print(f"  {count:3}x  {marker}")

    if errors:
        print()
        print("Structural errors:")
        for error in errors:
            print(f"  - {error}")

    return 1 if args.check and errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
