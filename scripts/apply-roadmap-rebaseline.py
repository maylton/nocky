#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path.cwd()
ROADMAP = ROOT / "ROADMAP.md"
MARKER = "<!-- roadmap_search_visual_rebaseline_2026_07_03_v1 -->"


def replace_section(text: str, start_heading: str, end_heading: str, replacement: str) -> str:
    start = text.find(start_heading)
    if start == -1:
        raise RuntimeError(f"Could not find section start: {start_heading!r}")
    end = text.find(end_heading, start + len(start_heading))
    if end == -1:
        raise RuntimeError(f"Could not find section end: {end_heading!r}")
    return text[:start] + replacement.rstrip() + "\n\n" + text[end:]


def replace_line(text: str, pattern: str, replacement: str) -> str:
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.MULTILINE)
    if count == 0:
        return text
    return updated


def dedupe_lines(block: str) -> str:
    seen: set[str] = set()
    lines: list[str] = []
    for line in block.splitlines():
        if line.startswith("- "):
            key = line.strip()
            if key in seen:
                continue
            seen.add(key)
        lines.append(line)
    return "\n".join(lines)


SECTION_2 = """## 2. 🟡 Material Expressive visual-system consolidation

The active priority is making Nocky's reusable GTK widgets, loading states and
surface treatments feel coherent with Material 3 Expressive while preserving
Noctalia, Frosted Glass and dynamic album-palette identities.

### Completed in the current visual/search cycle

- ✅ Recent-search history is implemented with persisted MRU ordering, duplicate
  cleanup, individual removal and clear-all controls.
- ✅ The recent-search surface is an inline dropdown aligned with the global
  search entry and styled as a tonal Material card.
- ✅ Search result pages expose accessible summaries, source-aware row labels and
  stable keyboard focus rings.
- ✅ The Home player, transport, lyrics and visualizer surfaces share the same
  rounded tonal hierarchy.

### Active checkpoint

- 🟡 Menus and contextual surfaces.

### Planned checkpoints

- Expressive buttons and button-state motion.
- Dialogs and confirmation surfaces.
- Switches, toggles and segmented controls.
- Cards, containers and shape hierarchy.
- Shared navigation transitions.
- Global motion tokens.
- Reduced-motion behavior.
- Accessibility and contrast audit.
"""

SECTION_4 = """## 4. 🟡 Categorized and incremental search

### Implemented

- Separate track, album, artist and playlist result groups.
- Immediate local-library filtering.
- Debounced remote YouTube search.
- Independent incremental limits for each result category.
- Loading, empty and error state foundations.
- Local and YouTube results remain source-aware.
- ✅ Expiring search cache with a 10-minute fresh TTL, one-hour stale-while-revalidate window and bounded LRU eviction.
- ✅ Real per-category remote pagination backed by opaque YouTube Music continuations.
- ✅ Album and playlist results expose one compact play/pause action.
- ✅ Collection-result rows support arrow navigation and Enter/Space activation.
- ✅ Local recent-query history with MRU ordering, individual removal and clear-all controls.
- ✅ Route-aware cancellation for stale YouTube Music search responses.
- ✅ Accessible search-result summaries update after each categorized result rebuild.
- ✅ Search result rows expose source-aware accessible labels and stable focus-within rings.

### Remaining

- Source-aware ranking within the active source only: Local mode ranks local
  metadata; YouTube Music mode ranks synchronized/cache/remote results without
  mixing local files into the remote result list.
- Final search release polish after the source-aware ranking pass.
"""

SEARCH_PRINCIPLE = "- Keep local-library and YouTube Music behavior clearly separated."
SEARCH_REFINEMENT = (
    "- Treat search ranking as source-aware: the active source may improve its own\n"
    "  ordering, but Local and YouTube Music results should not be mixed into one\n"
    "  global list."
)


def main() -> int:
    if not ROADMAP.is_file():
        print("Run this script from the Nocky repository root.", file=sys.stderr)
        return 1

    original = ROADMAP.read_text(encoding="utf-8")
    text = original

    if MARKER not in text:
        text = text.replace("# Nocky Roadmap\n", f"{MARKER}\n# Nocky Roadmap\n", 1)

    # Keep the update date stable for this rebaseline and avoid moving the roadmap
    # to a future date when local branches are applied out of order.
    text = replace_line(text, r"^> Last updated: .*$", "> Last updated: 2026-07-03")

    if SEARCH_PRINCIPLE in text and SEARCH_REFINEMENT not in text:
        text = text.replace(SEARCH_PRINCIPLE, SEARCH_PRINCIPLE + "\n" + SEARCH_REFINEMENT, 1)

    text = replace_section(
        text,
        "## 2. 🟡 Material Expressive visual-system consolidation",
        "## 3. 🟡 Richer album, artist and playlist cards",
        SECTION_2,
    )
    text = replace_section(
        text,
        "## 4. 🟡 Categorized and incremental search",
        "## 5. 🟡 YouTube Music robustness",
        SECTION_4,
    )

    # Remove stale duplicate lines that may have been inserted by earlier local
    # applicators before the rebaseline, especially the discarded mixed-ranking
    # wording.
    stale_lines = [
        "- Better ranking across mixed local and remote results.",
        "- Cancellation of unnecessary remote requests after route changes.",
        "- Accessibility announcements when results update.",
        "- 🟡 Search result update announcements and accessibility polish.",
        "- 🟡 Final search release polish and keyboard/a11y audit.",
        "- 🟡 Search history and recent queries.",
    ]
    for line in stale_lines:
        text = text.replace(line + "\n", "")

    text = dedupe_lines(text).rstrip() + "\n"

    if text == original:
        print("ROADMAP.md is already rebaselined for the current search/visual cycle.")
        return 0

    ROADMAP.write_text(text, encoding="utf-8")
    print("ROADMAP.md rebaselined successfully.")
    print("  ROADMAP.md")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
