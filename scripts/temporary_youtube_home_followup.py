#!/usr/bin/env python3
from pathlib import Path

# Applied after the main deterministic patch so Clippy sees the final signature.
path = Path("src/browser.rs")
text = path.read_text(encoding="utf-8")
old = "    fn rebuild_home(\n"
new = '''    #[expect(
        clippy::too_many_arguments,
        reason = "Home rendering keeps its source-aware dependencies and loading state explicit"
    )]
    fn rebuild_home(
'''
count = text.count(old)
if count != 1:
    raise SystemExit(f"src/browser.rs: expected one rebuild_home function, found {count}")
path.write_text(text.replace(old, new), encoding="utf-8")
