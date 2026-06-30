#!/usr/bin/env python3
"""Increase the Home V2 chip carousel's vertical breathing room."""

from pathlib import Path

path = Path("src/browser.rs")
text = path.read_text(encoding="utf-8")
old = '''    rail.set_margin_start(2);
    rail.set_margin_end(28);
    rail.set_margin_bottom(10);
'''
new = '''    rail.set_margin_start(2);
    rail.set_margin_end(28);
    rail.set_margin_bottom(18);
'''
if text.count(old) != 1:
    raise RuntimeError("Expected one Home V2 chip rail margin block")
text = text.replace(old, new, 1)

old = '''    scroll.set_hexpand(true);
    scroll.set_min_content_height(52);
    scroll.set_propagate_natural_height(true);
'''
new = '''    scroll.set_hexpand(true);
    scroll.set_min_content_height(64);
    scroll.set_propagate_natural_height(true);
'''
if text.count(old) != 1:
    raise RuntimeError("Expected one Home V2 chip scroll height block")
text = text.replace(old, new, 1)

path.write_text(text, encoding="utf-8")
print("Home V2 chip carousel height adjusted")
