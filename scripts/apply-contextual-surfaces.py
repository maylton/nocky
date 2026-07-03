#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path.cwd()
THEME = ROOT / "src/theme_css.rs"
CSS = ROOT / "assets/themes/material-expressive/104-contextual-surfaces.css"
ROADMAP = ROOT / "ROADMAP.md"
DOC = ROOT / "docs/CONTEXTUAL_SURFACES.md"

CSS_SOURCE = r'''/* Material Expressive contextual surfaces: menus, popovers and transient action sheets. */

window.theme-material-expressive popover.background > contents,
window.theme-material-expressive popover.menu > contents,
window.theme-material-expressive popover.context-menu > contents,
window.theme-material-expressive .contextual-surface,
window.theme-material-expressive .material-contextual-surface {
  background-color: @m3_surface_container_high;
  color: @m3_on_surface;
  border: 1px solid alpha(@m3_outline, 0.28);
  border-radius: 24px;
  padding: 8px;
  box-shadow:
    0 14px 34px alpha(black, 0.24),
    0 2px 6px alpha(black, 0.18),
    inset 0 0 0 1px alpha(@m3_primary, 0.04);
}

window.theme-material-expressive popover.background > arrow,
window.theme-material-expressive popover.menu > arrow {
  background-color: @m3_surface_container_high;
  border-color: alpha(@m3_outline, 0.28);
}

window.theme-material-expressive popover.menu modelbutton,
window.theme-material-expressive popover.background modelbutton,
window.theme-material-expressive .contextual-surface modelbutton,
window.theme-material-expressive .material-contextual-menu-item {
  min-height: 42px;
  padding: 8px 14px;
  border-radius: 16px;
  color: @m3_on_surface;
}

window.theme-material-expressive popover.menu modelbutton:hover,
window.theme-material-expressive popover.background modelbutton:hover,
window.theme-material-expressive .contextual-surface modelbutton:hover,
window.theme-material-expressive .material-contextual-menu-item:hover {
  background-color: alpha(@m3_primary, 0.10);
}

window.theme-material-expressive popover.menu modelbutton:active,
window.theme-material-expressive popover.background modelbutton:active,
window.theme-material-expressive .contextual-surface modelbutton:active,
window.theme-material-expressive .material-contextual-menu-item:active {
  background-color: alpha(@m3_primary, 0.16);
}

window.theme-material-expressive popover.menu modelbutton:focus-visible,
window.theme-material-expressive popover.background modelbutton:focus-visible,
window.theme-material-expressive .contextual-surface modelbutton:focus-visible,
window.theme-material-expressive .material-contextual-menu-item:focus-visible {
  outline: 2px solid alpha(@m3_primary, 0.76);
  outline-offset: 2px;
  background-color: alpha(@m3_primary, 0.12);
}

window.theme-material-expressive popover.menu separator,
window.theme-material-expressive popover.background separator,
window.theme-material-expressive .contextual-surface separator,
window.theme-material-expressive .material-contextual-separator {
  min-height: 1px;
  margin: 6px 8px;
  background-color: alpha(@m3_outline_variant, 0.56);
}

window.theme-material-expressive popover.menu label,
window.theme-material-expressive popover.background label,
window.theme-material-expressive .contextual-surface label {
  color: @m3_on_surface;
}

window.theme-material-expressive popover.menu .dim-label,
window.theme-material-expressive popover.background .dim-label,
window.theme-material-expressive .contextual-surface .dim-label {
  color: @m3_on_surface_variant;
}

window.theme-material-expressive .destructive-action,
window.theme-material-expressive popover.menu modelbutton.destructive-action,
window.theme-material-expressive popover.background modelbutton.destructive-action {
  color: @m3_error;
}

window.theme-material-expressive .destructive-action:hover,
window.theme-material-expressive popover.menu modelbutton.destructive-action:hover,
window.theme-material-expressive popover.background modelbutton.destructive-action:hover {
  background-color: alpha(@m3_error, 0.12);
}

window.theme-material-expressive .destructive-action:active,
window.theme-material-expressive popover.menu modelbutton.destructive-action:active,
window.theme-material-expressive popover.background modelbutton.destructive-action:active {
  background-color: alpha(@m3_error, 0.18);
}

window.theme-material-expressive .collection-action-popover,
window.theme-material-expressive .search-action-popover,
window.theme-material-expressive .playlist-action-popover,
window.theme-material-expressive .queue-action-popover {
  min-width: 220px;
}
'''

DOC_SOURCE = r'''# Material Expressive contextual surfaces

## Scope

This checkpoint adds the first Material Expressive treatment for transient
contextual surfaces: menu popovers, contextual action menus and future action
sheets.

## Styling contract

- popovers use high tonal surfaces instead of flat GTK defaults;
- menu contents get rounded 24 px containers with outline and subtle elevation;
- menu rows use 16 px state-layer rounding;
- hover, active and focus-visible states follow Material 3 Expressive tonal
  layering;
- destructive actions receive error-color state layers;
- generic helper classes are available for future custom contextual surfaces:
  - `contextual-surface`;
  - `material-contextual-surface`;
  - `material-contextual-menu-item`;
  - `material-contextual-separator`.

## Non-goals

This does not rewrite menu behavior or change any queue, playlist, search or
collection action semantics. It is a visual-system checkpoint that lets existing
GTK popovers and modelbutton menus inherit coherent Material Expressive surfaces.

## Next checkpoints

Dialogs and confirmation surfaces can now reuse the same tonal/elevation logic
instead of reintroducing ad-hoc popover styling.
'''


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


def insert_material_module(text: str) -> str:
    if '"104-contextual-surfaces.css"' in text:
        print("[already applied] Register contextual surfaces CSS module")
        return text

    anchors = [
        '''    (
        "103-home-player-polish.css",
        include_str!("../assets/themes/material-expressive/103-home-player-polish.css"),
    ),
''',
        '''    (
        "102-search-history.css",
        include_str!("../assets/themes/material-expressive/102-search-history.css"),
    ),
''',
        '''    (
        "101-keyboard-search.css",
        include_str!("../assets/themes/material-expressive/101-keyboard-search.css"),
    ),
''',
    ]
    insert = '''    (
        "104-contextual-surfaces.css",
        include_str!("../assets/themes/material-expressive/104-contextual-surfaces.css"),
    ),
'''
    for anchor in anchors:
        if anchor in text:
            print("[changed] Register contextual surfaces CSS module")
            return text.replace(anchor, anchor + insert, 1)

    marker = "\n];"
    if marker not in text:
        raise PatchError("Register contextual surfaces CSS module: manifest terminator not found")
    print("[changed] Register contextual surfaces CSS module before manifest terminator")
    return text.replace(marker, "\n" + insert + "];", 1)


def patch_theme_tests(text: str) -> str:
    if '".contextual-surface"' in text and '".material-contextual-menu-item"' in text:
        print("[already applied] Require contextual surface CSS tokens in theme tests")
        return text
    anchor = '''            ".search-result-primary-action",
'''
    insert = '''            ".search-result-primary-action",
            ".contextual-surface",
            ".material-contextual-menu-item",
'''
    return replace_once(
        text,
        anchor,
        insert,
        "Require contextual surface CSS tokens in theme tests",
    )


def patch_roadmap(text: str) -> str:
    active_candidates = [
        "- 🟡 Menus and contextual surfaces.\n",
        "- 🟡 Expressive buttons and button-state motion.\n",
    ]
    for candidate in active_candidates:
        if candidate in text:
            text = text.replace(candidate, "- 🟡 Dialogs and confirmation surfaces.\n", 1)
            print("[changed] Advance active checkpoint to dialogs")
            break
    else:
        print("[already applied] Advance active checkpoint to dialogs")

    planned = "- Menus and contextual surfaces.\n"
    if planned in text:
        text = text.replace(planned, "", 1)
        print("[changed] Remove contextual surfaces from planned checkpoints")

    completed = "- ✅ Material Expressive contextual menu and popover surfaces.\n"
    anchors = [
        "## 2. 🟡 Material Expressive visual-system consolidation\n",
        "### Planned checkpoints\n",
    ]
    if completed not in text:
        if anchors[0] not in text:
            raise PatchError("Document completed contextual surfaces: visual-system section not found")
        marker = "### Active checkpoint\n\n"
        if marker in text:
            text = text.replace(marker, f"### Implemented\n\n{completed}\n{marker}", 1)
        else:
            text = text.replace(anchors[0], anchors[0] + f"\n### Implemented\n\n{completed}\n", 1)
        print("[changed] Document completed contextual surfaces")
    else:
        print("[already applied] Document completed contextual surfaces")

    for order in [
        "8. Continue with expressive button-state motion.\n",
        "8. Complete final search release polish and accessibility audit.\n",
    ]:
        if order in text:
            text = text.replace(order, "8. Continue with dialogs and confirmation surfaces.\n", 1)
            print("[changed] Advance recommended implementation order")
            break
    else:
        print("[already applied] Advance recommended implementation order")
    return text


def main() -> int:
    required = [THEME, ROADMAP]
    missing = [path for path in required if not path.is_file()]
    if missing:
        print("Run this script from the Nocky repository root.", file=sys.stderr)
        for path in missing:
            print(f"missing: {path}", file=sys.stderr)
        return 1

    if CSS.exists() and CSS.read_text(encoding="utf-8") != CSS_SOURCE:
        print(f"ERROR: {CSS} already exists with different content. No files were written.", file=sys.stderr)
        return 1
    if DOC.exists() and DOC.read_text(encoding="utf-8") != DOC_SOURCE:
        print(f"ERROR: {DOC} already exists with different content. No files were written.", file=sys.stderr)
        return 1

    original_theme = THEME.read_text(encoding="utf-8")
    original_roadmap = ROADMAP.read_text(encoding="utf-8")

    try:
        updated_theme = insert_material_module(original_theme)
        updated_theme = patch_theme_tests(updated_theme)
        updated_roadmap = patch_roadmap(original_roadmap)
    except PatchError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print("No files were written.", file=sys.stderr)
        return 1

    changed: list[Path] = []
    if updated_theme != original_theme:
        THEME.write_text(updated_theme, encoding="utf-8")
        changed.append(THEME.relative_to(ROOT))
    if updated_roadmap != original_roadmap:
        ROADMAP.write_text(updated_roadmap, encoding="utf-8")
        changed.append(ROADMAP.relative_to(ROOT))
    if not CSS.exists():
        CSS.write_text(CSS_SOURCE, encoding="utf-8")
        changed.append(CSS.relative_to(ROOT))
    if not DOC.exists():
        DOC.write_text(DOC_SOURCE, encoding="utf-8")
        changed.append(DOC.relative_to(ROOT))

    print("Contextual surfaces patch applied successfully.")
    for path in changed:
        print(f"  {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
