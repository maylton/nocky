#!/usr/bin/env python3
"""Apply the Clippy-approved form of the Home cover scheduling test fixture."""

from pathlib import Path

path = Path("src/youtube/mod.rs")
text = path.read_text(encoding="utf-8")
old = '''                    video_id: (result_type == "song")
                        .then(|| format!("song{index:07}"))
                        .unwrap_or_default(),
'''
new = '''                    video_id: if result_type == "song" {
                        format!("song{index:07}")
                    } else {
                        String::new()
                    },
'''
if text.count(old) != 1:
    raise RuntimeError("Expected one Home cover test fixture expression")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
print("Home cover test fixture updated for Clippy")
