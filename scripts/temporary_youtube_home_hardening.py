#!/usr/bin/env python3
from pathlib import Path

path = Path("docs/YOUTUBE_LIBRARY_ROADMAP.md")
text = path.read_text(encoding="utf-8")
replacements = [
    (
        "| 10. Android-parity YouTube Home organization | Implemented; real-chip manual validation pending | PR #46 |",
        "| 10. Android-parity YouTube Home organization | Complete and manually validated | PR #46 |",
    ),
    (
        """Manual validation pending:

- Confirm the connected account returns more than the root **Tudo** chip.
- Select every returned chip and confirm the section feed changes.
- Return to **Tudo** and confirm the root feed and chip list are restored.
- Confirm filtered load-more requests remain on the selected chip.
""",
        """Manual validation completed:

- The connected account returns localized server-provided chips beyond **Tudo**.
- Selecting chips highlights the active choice immediately and displays localized loading feedback in the main Home.
- Filtered responses replace the feed sections; identical server responses produce explicit feedback instead of appearing inert.
- Returning to **Tudo** restores the root feed and preserves the chip list.
- Rapid chip switching keeps the final request and selection.
- Filtered load-more requests retain the selected chip params.
- The horizontal scrollbar remains below the chip controls without overlap at narrow widths.
- Local Home behavior remains unchanged.
""",
    ),
    (
        """- Add fixture tests for chip extraction, selection request bodies,
  continuation params, section order and header preservation.
""",
        """- Add fixture tests for chip extraction, selection request bodies,
  continuation params, section order and header preservation.
- Provide optimistic chip selection, localized loading feedback and explicit
  feedback when YouTube returns unchanged recommendation sections.
- Keep the horizontal scrollbar below the chip controls without overlaying them.
""",
    ),
]
for old, new in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"roadmap replacement expected once, found {count}: {old[:100]!r}")
    text = text.replace(old, new)
path.write_text(text, encoding="utf-8")
