#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
expected_version="0.3.0"

echo "==> Checking package version"
python3 - "$expected_version" <<'PY'
from pathlib import Path
import re
import sys

expected = sys.argv[1]
cargo = Path("Cargo.toml").read_text(encoding="utf-8")
match = re.search(r'(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"', cargo)
if not match:
    raise SystemExit("Cargo.toml package version not found")
if match.group(1) != expected:
    raise SystemExit(
        f"Cargo.toml version is {match.group(1)}, expected {expected}"
    )

lock = Path("Cargo.lock").read_text(encoding="utf-8")
match = re.search(
    r'(?ms)^\[\[package\]\]\s*\nname\s*=\s*"nocky"\s*\n'
    r'version\s*=\s*"([^"]+)"',
    lock,
)
if not match:
    raise SystemExit("Nocky package not found in Cargo.lock")
if match.group(1) != expected:
    raise SystemExit(
        f"Cargo.lock version is {match.group(1)}, expected {expected}"
    )

print(f"Package version: {expected}")
PY

echo "==> Auditing PT/EN/ES translations"
python3 - <<'PY'
from pathlib import Path
import re

source = Path("src/i18n.rs").read_text(encoding="utf-8")
enum_match = re.search(
    r'pub enum Message\s*\{(?P<body>.*?)^\}',
    source,
    re.MULTILINE | re.DOTALL,
)
if not enum_match:
    raise SystemExit("Message enum not found")

variants = re.findall(
    r'^\s{4}([A-Za-z0-9_]+),\s*$',
    enum_match.group("body"),
    re.MULTILINE,
)
if len(variants) != len(set(variants)):
    raise SystemExit("Duplicate Message variants found")

all_match = re.search(
    r'const ALL_MESSAGES:\s*&\[Message\]\s*=\s*&\[(?P<body>.*?)\];',
    source,
    re.DOTALL,
)
if not all_match:
    raise SystemExit("ALL_MESSAGES not found")

listed = re.findall(r'Message::([A-Za-z0-9_]+)', all_match.group("body"))
if set(listed) != set(variants):
    missing = sorted(set(variants) - set(listed))
    extra = sorted(set(listed) - set(variants))
    raise SystemExit(
        f"ALL_MESSAGES mismatch. Missing={missing}, extra={extra}"
    )

text_start = source.find("pub fn text(")
tests_start = source.find("#[cfg(test)]", text_start)
text_body = source[text_start:tests_start]

bad = []
for variant in variants:
    count = len(
        re.findall(
            rf'Message::{re.escape(variant)}\s*=>',
            text_body,
        )
    )
    if count != 3:
        bad.append((variant, count))

if bad:
    raise SystemExit(
        "Translation arm count must be 3 for every message: "
        + ", ".join(f"{name}={count}" for name, count in bad)
    )

print(f"{len(variants)} messages × 3 languages: structural coverage OK")
PY

echo "==> Reviewing localized UI copy"
python3 scripts/audit-translations.py

echo "==> Auditing Lyrics, Home and onboarding localization"
python3 scripts/audit-surface-localization.py

echo "==> Formatting"
cargo fmt --check

echo "==> Compilation"
cargo check --all-targets --all-features

echo "==> Tests"
cargo test --all-targets --all-features

echo "==> Clippy"
cargo clippy --all-targets --all-features -- -D warnings

if command -v appstreamcli >/dev/null 2>&1; then
  while IFS= read -r -d '' file; do
    echo "==> AppStream: $file"
    appstreamcli validate --no-net "$file"
  done < <(
    find . -type f \( -name '*.metainfo.xml' -o -name '*.appdata.xml' \) \
      -not -path './target/*' -not -path './.git/*' -print0
  )
fi

if command -v desktop-file-validate >/dev/null 2>&1; then
  while IFS= read -r -d '' file; do
    echo "==> Desktop entry: $file"
    desktop-file-validate "$file"
  done < <(
    find . -type f -name '*.desktop' \
      -not -path './target/*' -not -path './.git/*' -print0
  )
fi

echo
echo "Nocky 0.3.0 release gate passed."
