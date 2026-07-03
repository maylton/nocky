#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path.cwd()
NAVIGATION = ROOT / "src/app/controller/navigation.rs"

if not NAVIGATION.is_file():
    raise SystemExit("Run this script from the Nocky repository root.")

text = NAVIGATION.read_text(encoding="utf-8")
replacements = {
    "    fn global_youtube_search_visible(&self, query: &str) -> bool {":
        "    pub(crate) fn global_youtube_search_visible(&self, query: &str) -> bool {",
    "    fn cancel_global_youtube_search_for_route_change(&self) {":
        "    pub(crate) fn cancel_global_youtube_search_for_route_change(&self) {",
    "    fn restart_global_youtube_search_after_route_return(&self) {":
        "    pub(crate) fn restart_global_youtube_search_after_route_return(&self) {",
}

changed = False
for old, new in replacements.items():
    if old in text:
        text = text.replace(old, new, 1)
        changed = True
    elif new in text:
        pass
    else:
        raise SystemExit(
            f"Could not find route-aware helper signature: {old.strip()}\nNo files were written."
        )

if changed:
    NAVIGATION.write_text(text, encoding="utf-8")
    print("Route-aware search helpers exposed across controller modules successfully.")
else:
    print("Route-aware search helpers were already visible across controller modules.")

missing = []
for module_path, marker in [
    (ROOT / "src/search_history.rs", "mod search_history;"),
    (ROOT / "src/app/controller/search_history.rs", "mod search_history;"),
    (ROOT / "src/search_ranking.rs", "mod search_ranking;"),
]:
    if module_path.exists():
        continue
    for source in [ROOT / "src/main.rs", ROOT / "src/app/controller/mod.rs"]:
        if source.is_file() and marker in source.read_text(encoding="utf-8"):
            missing.append(str(module_path))
            break

if missing:
    print("\nStill missing Rust module file(s) referenced by mod declarations:")
    for path in missing:
        print(f"  {path}")
    print("Run cargo check again and send the first E0583 block if one remains.")
