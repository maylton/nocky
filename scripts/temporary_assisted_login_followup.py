#!/usr/bin/env python3
from pathlib import Path

path = Path("src/youtube/mod.rs")
text = path.read_text(encoding="utf-8")
old = "mod login_policy;\n"
new = '#[cfg(feature = "assisted-login")]\nmod login_policy;\n'
count = text.count(old)
if count != 1:
    raise SystemExit(f"src/youtube/mod.rs: expected one login_policy module, found {count}")
path.write_text(text.replace(old, new), encoding="utf-8")
