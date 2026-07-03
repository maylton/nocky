#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path.cwd()
THEME_CSS = ROOT / "src/theme_css.rs"
SEARCH_STYLE = ROOT / "assets/themes/material-expressive/102-search-history.css"
PLAYER_STYLE = ROOT / "assets/themes/material-expressive/103-home-player-polish.css"


class PatchError(RuntimeError):
    pass


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count == 0 and new in text:
        print(f"[already applied] {label}")
        return text
    if count != 1:
        raise PatchError(f"{label}: expected one match, found {count}")
    print(f"[changed] {label}")
    return text.replace(old, new, 1)


SEARCH_STYLE_SOURCE = '''window.theme-material-expressive .search-history-dropdown {
  margin: 0;
  background-color: transparent;
}

window.theme-material-expressive .search-history-content {
  margin: 4px 0 10px;
  padding: 6px 8px 8px;
  border-radius: 20px;
  color: @m3_on_surface;
  background-color: alpha(@m3_surface_container_high, 0.94);
  border: 1px solid alpha(@m3_outline_variant, 0.62);
  box-shadow: none;
}

window.theme-material-expressive .search-history-header {
  min-height: 32px;
  margin: 0 2px 2px;
  padding: 0 2px 0 8px;
}

window.theme-material-expressive .search-history-title {
  color: @m3_on_surface_variant;
  font-size: 0.78rem;
  font-weight: 760;
  letter-spacing: 0.15px;
}

window.theme-material-expressive .search-history-clear {
  min-height: 32px;
  padding: 0 10px;
  color: @m3_primary;
  background-color: transparent;
}

window.theme-material-expressive .search-history-clear:hover {
  background-color: alpha(@m3_primary, 0.10);
}

window.theme-material-expressive .search-history-list,
window.theme-material-expressive .search-history-list > row,
window.theme-material-expressive .search-history-scroll,
window.theme-material-expressive .search-history-scroll > viewport {
  background-color: transparent;
  border: none;
  box-shadow: none;
}

window.theme-material-expressive .search-history-row {
  min-height: 40px;
  margin: 1px 0;
  padding: 0 2px;
  border-radius: 16px;
  color: @m3_on_surface_variant;
  background-color: transparent;
  border: 1px solid transparent;
}

window.theme-material-expressive .search-history-row:hover,
window.theme-material-expressive .search-history-row:focus-within {
  color: @m3_on_surface;
  background-color: alpha(@m3_primary, 0.09);
  border-color: alpha(@m3_primary, 0.18);
}

window.theme-material-expressive .search-history-row > box {
  min-height: 40px;
}

window.theme-material-expressive .search-history-icon {
  margin-left: 8px;
  color: @m3_on_surface_variant;
}

window.theme-material-expressive .search-history-query {
  min-height: 38px;
  padding: 0 10px;
  color: @m3_on_surface;
  background-color: transparent;
  box-shadow: none;
}

window.theme-material-expressive .search-history-query:hover,
window.theme-material-expressive .search-history-query:active,
window.theme-material-expressive .search-history-query:focus {
  background-color: transparent;
  box-shadow: none;
}

window.theme-material-expressive .search-history-remove {
  min-width: 34px;
  min-height: 34px;
  color: @m3_on_surface_variant;
  background-color: transparent;
}

window.theme-material-expressive .search-history-remove:hover {
  color: @m3_on_surface;
  background-color: alpha(@m3_primary, 0.12);
}
'''


PLAYER_STYLE_SOURCE = '''/*
 * Home player surface polish
 * --------------------------
 * The Rust view already exposes semantic wrappers for metadata, transport,
 * visualizer and inline lyrics. Keep the outer card hierarchy, but ensure the
 * child widgets cannot paint square backgrounds over those rounded surfaces.
 */

window.theme-material-expressive .player-transport-surface {
  padding: 10px 14px 12px;
  border-radius: 28px;
  color: @m3_on_surface;
  background-color: alpha(@m3_surface_container_low, 0.86);
  border: 1px solid alpha(@m3_outline_variant, 0.48);
  box-shadow: none;
}

window.theme-material-expressive .player-visualizer-surface,
window.theme-material-expressive .player-lyrics-surface {
  border-radius: 28px;
  color: @m3_on_surface;
  background-color: alpha(@m3_surface_container_low, 0.78);
  border: 1px solid alpha(@m3_outline_variant, 0.42);
  box-shadow: none;
}

window.theme-material-expressive .player-visualizer-surface .audio-visualizer,
window.theme-material-expressive .player-lyrics-surface .inline-lyrics-viewport,
window.theme-material-expressive .player-lyrics-surface .inline-lyrics-viewport > viewport,
window.theme-material-expressive .player-lyrics-surface .inline-lyrics-panel {
  border-radius: 22px;
  background-color: transparent;
  background-image: none;
  border: none;
  box-shadow: none;
}

window.theme-material-expressive .player-visualizer-surface .audio-visualizer {
  margin: 5px 14px;
}

window.theme-material-expressive .player-lyrics-surface .inline-lyrics-viewport {
  margin: 0 14px;
}

window.theme-material-expressive .player-time-row,
window.theme-material-expressive .player-progress-track,
window.theme-material-expressive .player-progress-wave,
window.theme-material-expressive .player-transport-controls {
  background-color: transparent;
  background-image: none;
  box-shadow: none;
}
'''


def patch_theme_css(text: str) -> str:
    return replace_once(
        text,
        '''    (
        "102-search-history.css",
        include_str!("../assets/themes/material-expressive/102-search-history.css"),
    ),
];
''',
        '''    (
        "102-search-history.css",
        include_str!("../assets/themes/material-expressive/102-search-history.css"),
    ),
    (
        "103-home-player-polish.css",
        include_str!("../assets/themes/material-expressive/103-home-player-polish.css"),
    ),
];
''',
        "Register late home-player surface polish",
    )


def main() -> int:
    required = [THEME_CSS, SEARCH_STYLE]
    missing = [path for path in required if not path.is_file()]
    if missing:
        for path in missing:
            print(f"missing: {path}")
        raise SystemExit(
            "Run this script from the Nocky repository root after applying the search-history dropdown patches."
        )

    theme_original = THEME_CSS.read_text(encoding="utf-8")
    search_original = SEARCH_STYLE.read_text(encoding="utf-8")
    if PLAYER_STYLE.exists() and PLAYER_STYLE.read_text(encoding="utf-8") != PLAYER_STYLE_SOURCE:
        raise SystemExit(f"ERROR: {PLAYER_STYLE} already exists with different content. No files were written.")

    try:
        theme_updated = patch_theme_css(theme_original)
    except PatchError as error:
        print(f"ERROR: {error}")
        print("No files were written.")
        return 1

    changed = []
    if theme_updated != theme_original:
        THEME_CSS.write_text(theme_updated, encoding="utf-8")
        changed.append(THEME_CSS.relative_to(ROOT))
    if search_original != SEARCH_STYLE_SOURCE:
        SEARCH_STYLE.write_text(SEARCH_STYLE_SOURCE, encoding="utf-8")
        changed.append(SEARCH_STYLE.relative_to(ROOT))
    if not PLAYER_STYLE.exists():
        PLAYER_STYLE.write_text(PLAYER_STYLE_SOURCE, encoding="utf-8")
        changed.append(PLAYER_STYLE.relative_to(ROOT))

    print("Search dropdown and home-player surface polish applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
